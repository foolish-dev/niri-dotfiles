use crate::util::{home, info, ok, run, warn};
use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

/// What [`patch_hexstrike_bind`] concluded about a `hexstrike_server.py`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BindState {
    /// An all-interfaces bind was found and rewritten; the payload is the new
    /// file contents, ready to write back.
    Rewritten(String),
    /// The bind is already `API_HOST` or a loopback literal — nothing to do.
    AlreadySafe,
    /// No `app.run(host=...)` spelling this function knows. Upstream has
    /// drifted and the bind cannot be proven loopback-only.
    Unrecognized,
}

/// Every spelling of "listen on every interface" that we know how to rewrite.
/// `""`/`"::"` bind all interfaces just as `0.0.0.0` does.
const ALL_INTERFACE_BINDS: [&str; 6] = [
    r#"app.run(host="0.0.0.0""#,
    r#"app.run(host='0.0.0.0'"#,
    r#"app.run(host="::""#,
    r#"app.run(host='::'"#,
    r#"app.run(host="""#,
    r#"app.run(host=''"#,
];

/// Every spelling that already keeps the API off the network. `API_HOST` is
/// upstream's own variable: it reads `HEXSTRIKE_HOST` and defaults to
/// `127.0.0.1`, and the unit pins that env var to loopback. Listing the plain
/// loopback literals too means an upstream that fixes itself is accepted
/// rather than fought over on every run.
const LOOPBACK_BINDS: [&str; 7] = [
    "app.run(host=API_HOST",
    r#"app.run(host="127.0.0.1""#,
    r#"app.run(host='127.0.0.1'"#,
    r#"app.run(host="localhost""#,
    r#"app.run(host='localhost'"#,
    r#"app.run(host="::1""#,
    r#"app.run(host='::1'"#,
];

/// Rewrite HexStrike's hardcoded all-interfaces bind to honour the `API_HOST`
/// the module already computes.
///
/// Upstream reads `API_HOST = os.environ.get('HEXSTRIKE_HOST', '127.0.0.1')`,
/// logs it, and then ignores it: the final line is
/// `app.run(host="0.0.0.0", port=API_PORT, ...)`. The service is an
/// unauthenticated remote-shell API — `/api/command` passes its `command`
/// field straight to `execute_command` — so a 0.0.0.0 bind puts arbitrary
/// code execution on every interface the box has.
///
/// The unit's `Environment=HEXSTRIKE_HOST=127.0.0.1` cannot fix that on its
/// own, and neither can its `IPAddressDeny=any`: systemd's IP firewall is a
/// cgroup BPF feature the *user* manager cannot install, and it says so —
/// "unit configures an IP firewall, but not running as root". Both were
/// load-bearing in name only.
///
/// Idempotent, and a no-op on a file that has already been patched or that
/// upstream has fixed.
///
/// Fails *closed*. An earlier version returned `Option<String>` and so had no
/// way to say "this file does not look like anything I recognise": a `None`
/// meant both "already loopback" and "needle not found", and the caller
/// reported the first for both. Any upstream edit that moved the bind out of
/// the single byte-exact literal this used to match — swapping the quote style
/// would have been enough, and upstream's own
/// `API_HOST = os.environ.get('HEXSTRIKE_HOST', '127.0.0.1')` is single-quoted,
/// so a quote-style sweep over the module is entirely plausible — would then
/// have printed a green
/// "already loopback-only" over a server still listening on every interface.
/// [`BindState::Unrecognized`] is that missing third answer, and the caller
/// treats it as fatal.
pub(crate) fn patch_hexstrike_bind(src: &str) -> BindState {
    if let Some(needle) = ALL_INTERFACE_BINDS.iter().find(|n| src.contains(*n)) {
        return BindState::Rewritten(src.replace(needle, "app.run(host=API_HOST"));
    }
    if LOOPBACK_BINDS.iter().any(|n| src.contains(n)) {
        return BindState::AlreadySafe;
    }
    BindState::Unrecognized
}

/// The DNS-rebinding guard [`patch_hexstrike_host_guard`] inserts. It lives in
/// a real Python file so tests/config-syntax.sh can parse it and it can be run
/// against Flask on its own.
const HEXSTRIKE_HOST_GUARD: &str = include_str!("../assets/hexstrike-host-guard.py");

/// The guard's first line. Its presence is what "already guarded" means.
const HOST_GUARD_SENTINEL: &str = "# dotctl: loopback-Host guard.";

/// The guard decorates `app`, so it goes after the line that creates it.
const HOST_GUARD_ANCHOR: &str = "app = Flask(__name__)\n";

/// What [`patch_hexstrike_host_guard`] concluded about a `hexstrike_server.py`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HostGuardState {
    /// The guard was inserted; the payload is the new file contents.
    Inserted(String),
    /// The guard is already in the file.
    Present,
    /// No `app = Flask(__name__)` line to put it after: upstream drifted.
    NoAnchor,
}

/// Insert [`HEXSTRIKE_HOST_GUARD`] after the line that creates the Flask app.
///
/// [`patch_hexstrike_bind`] keeps other machines off the API; this keeps web
/// pages off it. `/api/command` runs any shell command it is sent, with no
/// token, and a loopback bind does nothing about the browser on the same box:
/// a page that DNS-rebinds its own hostname to 127.0.0.1 reaches the port
/// same-origin, so there is no CORS preflight to fail. The Host header still
/// carries the page's own name, so the guard answers only loopback names.
/// Flask's own TRUSTED_HOSTS setting would do this in one line, but it arrived
/// in Flask 3.1 and the HexStrike venv runs 3.0.
pub(crate) fn patch_hexstrike_host_guard(src: &str) -> HostGuardState {
    if src.contains(HOST_GUARD_SENTINEL) {
        return HostGuardState::Present;
    }
    let at_line_start = |i: &usize| *i == 0 || src.as_bytes()[i - 1] == b'\n';
    match src
        .match_indices(HOST_GUARD_ANCHOR)
        .map(|(i, _)| i)
        .find(at_line_start)
    {
        Some(i) => {
            let end = i + HOST_GUARD_ANCHOR.len();
            HostGuardState::Inserted(format!(
                "{}\n{}\n{}",
                &src[..end],
                HEXSTRIKE_HOST_GUARD,
                &src[end..]
            ))
        }
        None => HostGuardState::NoAnchor,
    }
}

/// Whether a Python venv at `venv` is usable. The directory merely existing
/// isn't enough — a run interrupted mid-creation (Ctrl-C, disk full, a failed
/// ensurepip) leaves it present but without `bin/pip`, the binary install's
/// dependency steps actually invoke. That binary is the real readiness signal.
fn venv_ready(venv: &Path) -> bool {
    venv.join("bin/pip").exists()
}

/// Clone, venv, loopback-pin and pip-install HexStrike AI.
///
/// Split out of `install()` for one reason: so the caller can warn instead of
/// aborting (see the note there). The body is the code that used to sit inline,
/// unchanged — every `?` in here now ends this step rather than the whole run.
pub(crate) fn setup_hexstrike() -> Result<()> {
    let hex_dir = home().join("tools/hexstrike-ai");
    if hex_dir.exists() {
        info("HexStrike AI already cloned");
    } else {
        info("Cloning HexStrike AI ...");
        fs::create_dir_all(home().join("tools"))?;
        run(
            "git",
            &[
                "clone",
                "https://github.com/0x4m4/hexstrike-ai.git",
                hex_dir.to_str().unwrap(),
            ],
        )?;
    }

    let hex_env = hex_dir.join("hexstrike-env");
    // Gate on the venv's `bin/pip`, not just the directory: a run interrupted
    // mid-venv leaves hexstrike-env/ present but pip-less, which would then
    // make the pip steps below fail on every retry. `--clear` resets such a
    // partial venv (no-op on a fresh path) so install self-heals.
    if !venv_ready(&hex_env) {
        info("Creating HexStrike Python venv ...");
        run(
            "python3",
            &["-m", "venv", "--clear", hex_env.to_str().unwrap()],
        )?;
    }

    // Re-applied on EVERY run, not just after the clone: the checkout above is
    // never `git pull`ed, but a user who updates it by hand would otherwise
    // silently restore the 0.0.0.0 bind. Cheap, and idempotent.
    let server_py = hex_dir.join("hexstrike_server.py");
    match fs::read_to_string(&server_py) {
        Ok(src) => match patch_hexstrike_bind(&src) {
            BindState::Rewritten(patched) => {
                fs::write(&server_py, &patched).with_context(|| {
                    format!("rewriting the bind address in {}", server_py.display())
                })?;
                if patch_hexstrike_bind(&patched) != BindState::AlreadySafe {
                    return Err(anyhow!(
                        "failed to pin {} to a loopback bind — refusing to continue, \
                         as the API would be reachable from the network",
                        server_py.display()
                    ));
                }
                ok("HexStrike bind pinned to $HEXSTRIKE_HOST (default 127.0.0.1)");
                restart_hexstrike_if_running("bind");
            }
            BindState::AlreadySafe => ok("HexStrike bind already loopback-only"),
            BindState::Unrecognized => {
                return Err(anyhow!(
                    "no recognisable `app.run(host=...)` in {} — upstream changed shape, so \
                     dotctl cannot prove the API is bound to loopback. `/api/command` runs \
                     arbitrary shell unauthenticated, so this refuses rather than guess. \
                     Check the bind by hand and teach `patch_hexstrike_bind` the new spelling.",
                    server_py.display()
                ));
            }
        },
        Err(e) => warn(&format!(
            "could not read {} to check its bind address: {e}",
            server_py.display()
        )),
    }

    // The Host guard, re-applied on every run for the same reason as the bind.
    // A missing anchor warns rather than failing the run: the API is still
    // loopback-only, and deploy() will not *enable* a unit without the guard.
    match fs::read_to_string(&server_py).map(|src| patch_hexstrike_host_guard(&src)) {
        Ok(HostGuardState::Inserted(patched)) => {
            fs::write(&server_py, patched).with_context(|| {
                format!("inserting the Host guard into {}", server_py.display())
            })?;
            ok("HexStrike now answers only requests addressed to a loopback name");
            restart_hexstrike_if_running("Host guard");
        }
        Ok(HostGuardState::Present) => ok("HexStrike Host guard already in place"),
        Ok(HostGuardState::NoAnchor) => warn(&format!(
            "no `app = Flask(__name__)` line in {} to put the Host guard after — upstream \
             changed shape, so any web page that DNS-rebinds its name to 127.0.0.1 could \
             drive /api/command. Teach `patch_hexstrike_host_guard` the new spelling.",
            server_py.display()
        )),
        Err(e) => warn(&format!(
            "could not read {} to add its Host guard: {e}",
            server_py.display()
        )),
    }

    info("Installing HexStrike Python dependencies ...");
    let pip = hex_env.join("bin/pip");
    let pip_str = pip.to_str().unwrap();
    run(pip_str, &["install", "--quiet", "--upgrade", "pip"])?;
    run(
        pip_str,
        &[
            "install",
            "--quiet",
            "-r",
            hex_dir.join("requirements.txt").to_str().unwrap(),
        ],
    )?;
    ok("HexStrike ready (loopback :8888 via hexstrike-server.service)");

    Ok(())
}

/// Called after `hexstrike_server.py` was patched (`what` names the patch).
/// The file is fixed, but a server that is *already running* keeps its old
/// code until it restarts, and the `enable --now` that deploy() runs later is a
/// no-op on an active unit.
///
/// Only restart when the unit is genuinely active. On a fresh machine deploy()
/// has not symlinked hexstrike-server.service yet, and `try-restart` on an
/// unknown unit exits 5 — which would print "may still be serving on 0.0.0.0"
/// on every first install, when nothing was ever running. A scary warning that
/// cries wolf on the common path teaches people to ignore the one case where it
/// is real. `is-active` also answers no when there is no user bus at all (SSH
/// without lingering), where nothing can be running either.
fn restart_hexstrike_if_running(what: &str) {
    let running = Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", "hexstrike-server.service"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if running {
        if let Err(e) = run(
            "systemctl",
            &["--user", "try-restart", "hexstrike-server.service"],
        ) {
            warn(&format!(
                "hexstrike-server.service did not restart, so it may still be serving \
                 without the new {what} until you restart it by hand: {e}"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn venv_ready_requires_pip_not_just_the_directory() {
        // The bug this guards: a venv dir left behind by an interrupted run
        // (no bin/pip) must trigger a rebuild, not a skip.
        let tmp = TempDir::new().expect("tempdir");
        let venv = tmp.path().join("hexstrike-env");
        fs::create_dir_all(&venv).expect("mkdir venv");
        assert!(
            !venv_ready(&venv),
            "a bare directory (interrupted venv) must not count as ready"
        );
        fs::create_dir_all(venv.join("bin")).expect("mkdir bin");
        fs::write(venv.join("bin/pip"), "#!/bin/sh\n").expect("write pip");
        assert!(venv_ready(&venv), "bin/pip present ⇒ ready");
    }

    #[test]
    fn patch_hexstrike_bind_rewrites_the_all_interfaces_bind() {
        // The upstream line, verbatim.
        let src = "    app.run(host=\"0.0.0.0\", port=API_PORT, debug=DEBUG_MODE)\n";
        let BindState::Rewritten(out) = patch_hexstrike_bind(src) else {
            panic!("the 0.0.0.0 bind must be rewritten");
        };
        assert!(out.contains("app.run(host=API_HOST, port=API_PORT"));
        assert!(
            !out.contains("0.0.0.0"),
            "no all-interfaces bind may survive: {out}"
        );
    }

    #[test]
    fn patch_hexstrike_bind_is_idempotent_and_leaves_a_fixed_file_alone() {
        // AlreadySafe means "nothing to do" — the caller reports "already
        // loopback" rather than rewriting the file on every single run.
        assert_eq!(
            patch_hexstrike_bind("app.run(host=API_HOST, port=API_PORT)\n"),
            BindState::AlreadySafe
        );
        assert_eq!(
            patch_hexstrike_bind("app.run(host=\"127.0.0.1\", port=API_PORT)\n"),
            BindState::AlreadySafe
        );
        let BindState::Rewritten(once) =
            patch_hexstrike_bind("app.run(host=\"0.0.0.0\", port=1)\n")
        else {
            panic!("first pass must rewrite");
        };
        assert_eq!(
            patch_hexstrike_bind(&once),
            BindState::AlreadySafe,
            "second pass is a no-op"
        );
    }

    #[test]
    fn host_guard_goes_right_after_the_app_and_only_once() {
        // Upstream's import and its lines 94-95, verbatim.
        let src = "from flask import Flask, request, jsonify\n\n\
                   app = Flask(__name__)\n\
                   app.config['JSON_SORT_KEYS'] = False\n";
        let HostGuardState::Inserted(out) = patch_hexstrike_host_guard(src) else {
            panic!("the guard must be inserted after `app = Flask(__name__)`");
        };
        let app = out.find("app = Flask(__name__)\n").unwrap();
        let guard = out.find(HOST_GUARD_SENTINEL).unwrap();
        let next = out.find("app.config['JSON_SORT_KEYS']").unwrap();
        assert!(
            app < guard && guard < next,
            "guard must follow the app line: {out}"
        );
        assert_eq!(
            out.replacen(&format!("\n{HEXSTRIKE_HOST_GUARD}\n"), "", 1),
            src,
            "nothing but the guard may change"
        );
        assert_eq!(
            patch_hexstrike_host_guard(&out),
            HostGuardState::Present,
            "second pass is a no-op"
        );
    }

    #[test]
    fn host_guard_anchors_only_on_a_real_app_line() {
        assert_eq!(
            patch_hexstrike_host_guard("app = create_app()\n"),
            HostGuardState::NoAnchor
        );
        // A comment quoting the line is not the line...
        assert_eq!(
            patch_hexstrike_host_guard("# app = Flask(__name__)\n"),
            HostGuardState::NoAnchor
        );
        // ...but the real line after such a comment is, as is a first line.
        for src in [
            "# app = Flask(__name__)\napp = Flask(__name__)\n",
            "app = Flask(__name__)\n",
        ] {
            assert!(
                matches!(patch_hexstrike_host_guard(src), HostGuardState::Inserted(_)),
                "{src:?}"
            );
        }
    }

    #[test]
    fn host_guard_sentinel_is_the_guards_own_first_line() {
        // `Present` is decided by the sentinel. If the asset's first line drifted
        // from it, every run would insert the guard again.
        assert!(HEXSTRIKE_HOST_GUARD.starts_with(HOST_GUARD_SENTINEL));
    }

    #[test]
    fn every_known_bind_spelling_is_classified() {
        // Table-driven so the const arrays cannot grow an entry that nothing
        // exercises. The security contract is per-spelling, not per-example.
        for needle in ALL_INTERFACE_BINDS {
            let src = format!("{needle}, port=API_PORT)\n");
            let BindState::Rewritten(out) = patch_hexstrike_bind(&src) else {
                panic!("{needle} must be recognised as an all-interfaces bind");
            };
            assert!(
                out.contains("app.run(host=API_HOST"),
                "{needle} must be rewritten to API_HOST, got: {out}"
            );
            assert_eq!(
                patch_hexstrike_bind(&out),
                BindState::AlreadySafe,
                "{needle} must be safe after one rewrite"
            );
        }
        for needle in LOOPBACK_BINDS {
            let src = format!("{needle}, port=API_PORT)\n");
            assert_eq!(
                patch_hexstrike_bind(&src),
                BindState::AlreadySafe,
                "{needle} is a loopback bind and must be left alone"
            );
        }
    }

    #[test]
    fn patch_hexstrike_bind_fails_closed_on_an_unrecognised_bind() {
        // The regression this guards: a file whose bind we cannot classify must
        // never be reported as safe. Single quotes are the cheap real-world
        // case — upstream's own `os.environ.get('HEXSTRIKE_HOST', '127.0.0.1')`
        // is single-quoted, so a quote-style sweep is entirely plausible.
        assert_eq!(
            patch_hexstrike_bind("socketio.run(app, host=\"0.0.0.0\", port=API_PORT)\n"),
            BindState::Unrecognized,
            "a bind shape we do not understand must not pass as safe"
        );
        assert_eq!(
            patch_hexstrike_bind("app.run(host = \"0.0.0.0\", port=1)\n"),
            BindState::Unrecognized,
            "whitespace we do not handle must fail closed, not silently pass"
        );
        assert_eq!(
            patch_hexstrike_bind("if __name__ == '__main__':\n    main()\n"),
            BindState::Unrecognized,
            "no bind at all is not evidence of a safe bind"
        );

        // Single-quoted spellings ARE understood, in both directions.
        let BindState::Rewritten(out) = patch_hexstrike_bind("app.run(host='0.0.0.0', port=1)\n")
        else {
            panic!("single-quoted all-interfaces bind must be rewritten");
        };
        assert!(
            !out.contains("0.0.0.0"),
            "no all-interfaces bind survives: {out}"
        );
        assert_eq!(
            patch_hexstrike_bind("app.run(host='127.0.0.1', port=1)\n"),
            BindState::AlreadySafe
        );
    }
}
