use crate::util::{info, run, warn};
use crate::SELF_SOURCE;
use anyhow::{anyhow, Result};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

/// `git -C <repo> pull --ff-only --autostash`. The `--autostash` matters:
/// `dotctl deploy` symlinks `~/.config/*` straight at this repo, so an app
/// that saves its own settings (qt5ct, a GUI settings page) rewrites a
/// tracked file in place. A plain `git pull --ff-only` then bails on the
/// dirty tree, so re-running `dotctl all` to update would silently skip the
/// pull from then on. Autostash shelves the change, fast-forwards, then
/// restores it — updates keep flowing and the user keeps their settings.
fn git_pull(repo_str: &str) -> Result<()> {
    run("git", &["-C", repo_str, "pull", "--ff-only", "--autostash"])?;
    // `--autostash` re-applies the shelved change after the fast-forward — but
    // `git pull` exits 0 even when that apply CONFLICTS. It prints a notice,
    // leaves conflict markers in the working tree, and keeps the stash. Since
    // `deploy()` symlinks ~/.config/* straight at this repo, a conflicted
    // file would be handed to the live desktop as its config while
    // `dotctl all` reported success. Refuse rather than deploy that.
    if git_has_conflicts(repo_str) {
        return Err(anyhow!(
            "`git pull --ff-only --autostash` fast-forwarded {repo_str}, but re-applying your \
             local changes conflicted: the working tree has conflict markers and the autostash \
             is still in `git stash list`. To recover, edit each conflicted file, then \
             `git -C {repo_str} add` it — this guard tests `git ls-files --unmerged`, which \
             deleting the markers alone does not clear — and finally `git -C {repo_str} stash \
             drop`. Do not `reset --hard` and `stash pop` again: that replays the same conflict, \
             and until you resolve it the stash holds the only copy of your changes. \
             Refusing to deploy a conflicted config."
        ));
    }
    Ok(())
}

/// True if `repo_str` has any path in an unmerged (conflicted) index state.
/// `git ls-files --unmerged` prints a line per conflicted stage and nothing at
/// all for a clean index, so emptiness is the signal. A git that can't run at
/// all answers false — the caller is already reporting that failure.
pub(crate) fn git_has_conflicts(repo: impl AsRef<std::ffi::OsStr>) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-files", "--unmerged"])
        .stderr(Stdio::null())
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

pub(crate) fn ensure_repo(repo: &Path) -> Result<()> {
    let repo_str = repo
        .to_str()
        .ok_or_else(|| anyhow!("repo path is not valid utf-8: {}", repo.display()))?;
    if repo.join(".git").exists() {
        info(&format!("Updating {repo_str} ..."));
        if let Err(e) = git_pull(repo_str) {
            // A conflicted autostash is not a "skip": the tree now holds
            // conflict markers that deploy() would symlink into live configs.
            // Everything else (diverged history, no network) leaves the tree
            // untouched and is safe to continue past, as it always was.
            if git_has_conflicts(repo_str) {
                return Err(e);
            }
            warn("git pull skipped (diverged history, or local changes git couldn't autostash?)");
        }
    } else {
        info(&format!("Cloning to {repo_str} ..."));
        if let Some(parent) = repo.parent() {
            fs::create_dir_all(parent)?;
        }
        run(
            "git",
            &[
                "clone",
                "https://github.com/foolish-dev/niri-dotfiles.git",
                repo_str,
            ],
        )?;
    }
    Ok(())
}

// ── Self-update ───────────────────────────────────────────────────────────────
//
// `dotctl all` pulls the checkout and then runs install() and deploy() — the
// ones compiled into the binary already running, not the ones it just pulled.
// So a fix to dotctl reached nobody until they thought to reinstall it by
// hand. The binary therefore carries a fingerprint of the source it was built
// from, and compares it with the checkout's.

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a, 64-bit. `const` so the build can fingerprint its own source; the
/// run-time check calls the same function on the checkout's files.
const fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    hash
}

/// One file's contribution: its path, a separator, its bytes. The path is in
/// it so that moving bytes from one file to the next still changes the hash.
const fn fingerprint_file(hash: u64, path: &str, bytes: &[u8]) -> u64 {
    fnv1a(fnv1a(fnv1a(hash, path.as_bytes()), &[0]), bytes)
}

/// The fingerprint of the source this binary was compiled from.
const BUILT_FROM: u64 = {
    let h = fingerprint_file(FNV_OFFSET, SELF_SOURCE[0], include_bytes!("main.rs"));
    let h = fingerprint_file(h, SELF_SOURCE[1], include_bytes!("../Cargo.toml"));
    let h = fingerprint_file(h, SELF_SOURCE[2], include_bytes!("../Cargo.lock"));
    let h = fingerprint_file(
        h,
        SELF_SOURCE[3],
        include_bytes!("../assets/greetd/regreet.toml"),
    );
    let h = fingerprint_file(
        h,
        SELF_SOURCE[4],
        include_bytes!("../assets/greetd/regreet.css"),
    );
    let h = fingerprint_file(
        h,
        SELF_SOURCE[5],
        include_bytes!("../assets/hexstrike-host-guard.py"),
    );
    let h = fingerprint_file(h, SELF_SOURCE[6], include_bytes!("deploy.rs"));
    let h = fingerprint_file(h, SELF_SOURCE[7], include_bytes!("distro.rs"));
    let h = fingerprint_file(h, SELF_SOURCE[8], include_bytes!("hardware.rs"));
    let h = fingerprint_file(h, SELF_SOURCE[9], include_bytes!("hexstrike.rs"));
    let h = fingerprint_file(h, SELF_SOURCE[10], include_bytes!("install.rs"));
    let h = fingerprint_file(h, SELF_SOURCE[11], include_bytes!("login.rs"));
    let h = fingerprint_file(h, SELF_SOURCE[12], include_bytes!("packages.rs"));
    let h = fingerprint_file(h, SELF_SOURCE[13], include_bytes!("testutil.rs"));
    let h = fingerprint_file(h, SELF_SOURCE[14], include_bytes!("update.rs"));
    fingerprint_file(h, SELF_SOURCE[15], include_bytes!("util.rs"))
};

/// Set on the rebuilt dotctl that [`rebuild_and_reexec`] starts, so a binary
/// that still differs after a rebuild warns instead of rebuilding forever.
pub(crate) const REBUILT_ENV: &str = "DOTCTL_REBUILT";

/// The fingerprint of the dotctl source in `repo`, or None when a file is
/// missing — not a dotctl checkout, so nothing to compare.
fn checkout_fingerprint(repo: &Path) -> Option<u64> {
    let mut hash = FNV_OFFSET;
    for rel in SELF_SOURCE {
        hash = fingerprint_file(hash, rel, &fs::read(repo.join(rel)).ok()?);
    }
    Some(hash)
}

/// True when `repo` holds dotctl source other than what this binary was built
/// from: the pull brought new code, or the binary predates the checkout.
pub(crate) fn binary_is_stale(repo: &Path) -> bool {
    checkout_fingerprint(repo).is_some_and(|h| h != BUILT_FROM)
}

/// Rebuild dotctl from `repo` and run the new binary in this one's place with
/// the same arguments. Returns only when that could not happen, after saying
/// why; the caller then carries on with the binary it has.
pub(crate) fn rebuild_and_reexec(repo: &Path) {
    use std::os::unix::process::CommandExt;
    info("dotctl's source in the checkout differs from this binary — rebuilding ...");
    let Some(repo_str) = repo.to_str() else {
        warn("repo path is not valid utf-8 — continuing with this dotctl");
        return;
    };
    if let Err(e) = run(
        "cargo",
        &["install", "--path", repo_str, "--locked", "--force"],
    ) {
        warn(&format!(
            "could not rebuild dotctl: {e} — continuing with this binary, which predates the checkout"
        ));
        return;
    }
    // `dotctl` by name: cargo just installed the new one onto PATH, wherever
    // this one was started from.
    let err = Command::new("dotctl")
        .args(std::env::args_os().skip(1))
        .env(REBUILT_ENV, "1")
        .exec();
    warn(&format!(
        "rebuilt dotctl but could not start it: {err} — continuing with this binary"
    ));
}

/// For `install` and `deploy`, which do not pull: say so, but do not rebuild
/// behind the user's back.
pub(crate) fn warn_if_stale(repo: &Path) {
    if binary_is_stale(repo) {
        warn(&format!(
            "this dotctl was built from different source than {} — `dotctl all` rebuilds \
             it, or run `cargo install --path {} --locked --force`",
            repo.display(),
            repo.display()
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::git;
    use crate::util::command_exists;
    use tempfile::TempDir;

    #[test]
    fn git_pull_refuses_when_the_autostash_reapply_conflicts() {
        // `git pull --ff-only --autostash` exits 0 even when re-applying the
        // stash conflicts — it just prints a notice and leaves conflict
        // markers behind. deploy() symlinks ~/.config/* at this repo, so
        // without this guard `dotctl all` would hand the live desktop a config
        // full of `<<<<<<<` and report success.
        if !command_exists("git") {
            return; // git-less host; nothing to exercise
        }
        let tmp = TempDir::new().expect("tempdir");
        let upstream = tmp.path().join("upstream");
        let clone = tmp.path().join("clone");
        fs::create_dir_all(&upstream).expect("mkdir upstream");

        git(&upstream, &["init", "-q", "-b", "main"]);
        git(&upstream, &["config", "user.email", "t@t"]);
        git(&upstream, &["config", "user.name", "t"]);
        fs::write(upstream.join("qt5ct.conf"), "[Appearance]\nstyle=Fusion\n").expect("seed");
        git(&upstream, &["add", "-A"]);
        git(&upstream, &["commit", "-qm", "init"]);

        git(
            tmp.path(),
            &[
                "clone",
                "-q",
                upstream.to_str().unwrap(),
                clone.to_str().unwrap(),
            ],
        );
        git(&clone, &["config", "user.email", "t@t"]);
        git(&clone, &["config", "user.name", "t"]);

        // Upstream and the local save touch THE SAME line — the one case
        // autostash cannot resolve.
        fs::write(upstream.join("qt5ct.conf"), "[Appearance]\nstyle=kvantum\n")
            .expect("upstream edit");
        git(&upstream, &["commit", "-qam", "upstream edit"]);
        fs::write(clone.join("qt5ct.conf"), "[Appearance]\nstyle=Breeze\n").expect("local save");

        let err = git_pull(clone.to_str().unwrap())
            .expect_err("a conflicted autostash re-apply must not be reported as success");
        let msg = err.to_string();
        assert!(
            msg.contains("conflict"),
            "the error must name the problem: {msg}"
        );
        assert!(
            git_has_conflicts(clone.to_str().unwrap()),
            "the clone really should be in a conflicted state"
        );
    }

    #[test]
    fn git_has_conflicts_is_false_for_a_clean_checkout() {
        if !command_exists("git") {
            return;
        }
        let tmp = TempDir::new().expect("tempdir");
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).expect("mkdir");
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["config", "user.email", "t@t"]);
        git(&repo, &["config", "user.name", "t"]);
        fs::write(repo.join("f"), "x\n").expect("seed");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-qm", "init"]);
        assert!(!git_has_conflicts(repo.to_str().unwrap()));
        // A merely *dirty* tree is not a conflicted one — the guard must not
        // fire on every grogu repaint.
        fs::write(repo.join("f"), "y\n").expect("dirty");
        assert!(!git_has_conflicts(repo.to_str().unwrap()));
    }

    #[test]
    fn fnv1a_matches_the_published_test_vectors() {
        assert_eq!(fnv1a(FNV_OFFSET, b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(FNV_OFFSET, b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a(FNV_OFFSET, b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn the_source_tree_fingerprints_to_what_was_compiled() {
        // The tests are built from this tree, so hashing it at run time must
        // reproduce BUILT_FROM. If it does not, SELF_SOURCE and the
        // include_bytes! list in BUILT_FROM have drifted apart, and every
        // `dotctl all` would rebuild itself for nothing.
        let tree = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(checkout_fingerprint(tree), Some(BUILT_FROM));
        assert!(!binary_is_stale(tree));
    }

    #[test]
    fn a_changed_or_missing_source_file_is_noticed() {
        let tree = Path::new(env!("CARGO_MANIFEST_DIR"));
        let tmp = TempDir::new().expect("tempdir");
        for rel in SELF_SOURCE {
            let dest = tmp.path().join(rel);
            fs::create_dir_all(dest.parent().unwrap()).unwrap();
            fs::copy(tree.join(rel), &dest).unwrap();
        }
        assert!(!binary_is_stale(tmp.path()), "an exact copy is not stale");

        // One byte of the lockfile: a dependency bump is a different binary.
        let lock = tmp.path().join("Cargo.lock");
        let mut bytes = fs::read(&lock).unwrap();
        bytes.push(b'\n');
        fs::write(&lock, bytes).unwrap();
        assert!(binary_is_stale(tmp.path()), "a changed file is stale");

        // Not a dotctl checkout at all: nothing to compare, so not stale.
        fs::remove_file(&lock).unwrap();
        assert_eq!(checkout_fingerprint(tmp.path()), None);
        assert!(!binary_is_stale(tmp.path()));
    }

    #[test]
    fn every_compiled_in_file_is_fingerprinted() {
        // A new include_str! asset changes what dotctl does; if SELF_SOURCE
        // missed it, editing that asset would never trigger a rebuild.
        let tree = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut src = String::new();
        for entry in fs::read_dir(tree.join("src")).expect("read src/") {
            let file = entry.expect("src/ entry").file_name();
            let Some(name) = file.to_str().filter(|n| n.ends_with(".rs")) else {
                continue;
            };
            // Each module is compiled in too, and may include assets of its own.
            let rel = format!("src/{name}");
            assert!(
                SELF_SOURCE.contains(&rel.as_str()),
                "{rel} is compiled in but not in SELF_SOURCE"
            );
            src += &fs::read_to_string(tree.join(&rel)).expect("read module");
        }
        let mut rest = src.as_str();
        let mut seen = 0;
        while let Some(i) = rest.find("include_") {
            rest = &rest[i + "include_".len()..];
            let Some(arg) = ["str!(\"", "bytes!(\""]
                .iter()
                .find_map(|p| rest.strip_prefix(p))
            else {
                continue;
            };
            let path = &arg[..arg.find('"').unwrap()];
            let rel = path
                .strip_prefix("../")
                .map(str::to_owned)
                .unwrap_or_else(|| format!("src/{path}"));
            assert!(
                SELF_SOURCE.contains(&rel.as_str()),
                "{rel} is compiled in but not in SELF_SOURCE"
            );
            seen += 1;
        }
        assert!(seen >= SELF_SOURCE.len(), "found only {seen} includes");
    }

    // ── git_pull ───────────────────────────────────────────────────────────
    //
    // `dotctl deploy` symlinks ~/.config/* at the repo, so an app that saves
    // its own settings rewrites a tracked file in place. Without --autostash
    // the ff-only pull would bail on that dirty tree and `dotctl all` would
    // silently stop updating. Pin that the pull fast-forwards anyway and
    // keeps the local change.

    #[test]
    fn git_pull_fast_forwards_over_a_locally_saved_tracked_file() {
        if !command_exists("git") {
            return; // git-less host; nothing to exercise
        }
        let tmp = TempDir::new().expect("tempdir");
        let upstream = tmp.path().join("upstream");
        let clone = tmp.path().join("clone");
        fs::create_dir_all(&upstream).expect("mkdir upstream");

        // Seed upstream with two tracked files: a config an app saves into
        // (qt5ct.conf) and an unrelated one (README).
        git(&upstream, &["init", "-q", "-b", "main"]);
        git(&upstream, &["config", "user.email", "t@t"]);
        git(&upstream, &["config", "user.name", "t"]);
        fs::write(upstream.join("qt5ct.conf"), "[Appearance]\nstyle=Fusion\n").expect("seed qt5ct");
        fs::write(upstream.join("README.md"), "v1\n").expect("seed readme");
        git(&upstream, &["add", "-A"]);
        git(&upstream, &["commit", "-qm", "init"]);

        git(
            tmp.path(),
            &[
                "clone",
                "-q",
                upstream.to_str().unwrap(),
                clone.to_str().unwrap(),
            ],
        );
        git(&clone, &["config", "user.email", "t@t"]);
        git(&clone, &["config", "user.name", "t"]);

        // Upstream advances an *unrelated* file (a real dotfiles update)...
        fs::write(upstream.join("README.md"), "v2\n").expect("bump readme");
        git(&upstream, &["commit", "-qam", "update readme"]);

        // ...while qt5ct has saved a setting into its tracked config here.
        let saved = "[Appearance]\nstyle=Breeze\n";
        fs::write(clone.join("qt5ct.conf"), saved).expect("save qt5ct");

        git_pull(clone.to_str().unwrap()).expect("autostash pull should fast-forward");

        assert_eq!(
            fs::read_to_string(clone.join("README.md")).expect("read readme"),
            "v2\n",
            "the upstream update must land despite the dirty tree"
        );
        assert_eq!(
            fs::read_to_string(clone.join("qt5ct.conf")).expect("read qt5ct"),
            saved,
            "the local save must survive the pull (autostash restores it)"
        );
    }
}
