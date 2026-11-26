use crate::distro::{aur_helper_hint, Distro};
use crate::util::{cache_dir, command_exists, info, ok, run, run_in, warn};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

fn pacman_install(label: &str, packages: &[&str]) -> Result<()> {
    if !command_exists("pacman") {
        warn(&format!(
            "{label} not installed and pacman not found — install {} manually",
            packages.join(", ")
        ));
        return Ok(());
    }
    info(&format!("Installing {label} ..."));
    let mut args = vec!["pacman", "-S", "--needed", "--noconfirm"];
    args.extend(packages.iter().copied());
    run("sudo", &args)?;
    ok(&format!("{label} installed"));
    Ok(())
}

pub(crate) fn ensure_pacman(label: &str, cmd: &str, packages: &[&str]) -> Result<()> {
    if command_exists(cmd) {
        ok(&format!("{label} already installed"));
        Ok(())
    } else {
        pacman_install(label, packages)
    }
}

/// [`ensure_pacman`] for a package that nothing later in `install()` and
/// nothing in `deploy()` reads back: the failure warns, names what the user
/// loses, and the run carries on.
///
/// The reason is `main`: `dotctl all` is `ensure_repo()?` then `install()?`
/// then `deploy()`, so ANY `?` that fires inside `install()` ends the run with
/// packages half installed and not one config deployed — the inverse of this
/// repo's opening invariant, that a fresh clone produces a bootable desktop.
/// A dead volume key is worth a red line; it is not worth throwing away the
/// whole deploy. Deliberately not used for niri, kitty, fuzzel or git: without
/// those there is no session to read the warning in, and warning would only
/// turn a broken install into a silent one.
///
/// `lost` is a sentence fragment continuing "<label> not installed: <err> — ".
pub(crate) fn optional_pacman(label: &str, cmd: &str, packages: &[&str], lost: &str) {
    if let Err(e) = ensure_pacman(label, cmd, packages) {
        warn(&format!("{label} not installed: {e} — {lost}"));
    }
}

/// True if `pkg` is currently installed according to pacman's database.
/// Used for packages that ship config / QML / library files instead of
/// a PATH binary, where `command_exists` would always miss them.
fn pacman_pkg_installed(pkg: &str) -> bool {
    Command::new("pacman")
        .args(["-Q", pkg])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// True if `pkg` can be resolved from a currently-configured binary
/// repository. Sibling of [`pacman_pkg_installed`]: that one asks "is it
/// installed?", this one asks "could it be?".
///
/// `pacman -Si` exits 0 for a package some enabled repo carries and non-zero
/// for one that lives only in the AUR or nowhere at all. A missing pacman, or
/// a sync database that has never been refreshed, also answers false — which
/// routes every caller to its conservative fallback rather than to a hard
/// error.
fn repo_has_pkg(pkg: &str) -> bool {
    Command::new("pacman")
        .args(["-Si", pkg])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Same shape as [`ensure_pacman`] but detects already-installed state
/// via `pacman -Q PKG` instead of `command -v BINARY`. Required for
/// packages that put no command of their own name on PATH — fonts, icon
/// themes, the NPU stack — where the binary check always missed and
/// `dotctl install` re-invoked `sudo pacman -S` (prompting for a password
/// each run) even when the package was already up to date.
pub(crate) fn ensure_pacman_pkg(label: &str, pkg: &str, packages: &[&str]) -> Result<()> {
    if pacman_pkg_installed(pkg) {
        ok(&format!("{label} already installed"));
        Ok(())
    } else {
        pacman_install(label, packages)
    }
}

/// [`optional_pacman`] with the `pacman -Q` gate of [`ensure_pacman_pkg`], for
/// the optional packages that ship no binary of their own (a font, an icon
/// theme). Same warn-and-continue contract, same `lost` fragment.
pub(crate) fn optional_pacman_pkg(label: &str, pkg: &str, packages: &[&str], lost: &str) {
    if let Err(e) = ensure_pacman_pkg(label, pkg, packages) {
        warn(&format!("{label} not installed: {e} — {lost}"));
    }
}

/// AUR helpers dotctl knows how to drive, most preferred first. Both accept
/// the identical `-S --needed --noconfirm` argument vector, so the choice is
/// only about which binary to spawn. `yay` stays first because it is the one
/// every pre-CachyOS dotctl used: any box that has it keeps behaving exactly
/// as it did, and it is the helper the deployed `.zshrc` aliases
/// (`yays`/`yayi`/`yayu`) target. `paru` — the CachyOS-idiomatic helper — is a
/// pure addition for hosts that have only it.
const AUR_HELPERS: [&str; 2] = ["yay", "paru"];

/// First entry of [`AUR_HELPERS`] for which `exists` reports true. The probe
/// is a parameter so the preference order is unit-testable without installing
/// anything.
fn preferred_aur_helper<F: Fn(&str) -> bool>(exists: F) -> Option<&'static str> {
    AUR_HELPERS.into_iter().find(|h| exists(h))
}

pub(crate) fn aur_helper() -> Option<&'static str> {
    preferred_aur_helper(command_exists)
}

/// Resolve an AUR helper, installing one from a configured binary repository
/// when none is on PATH and one is available there.
///
/// This is what makes the genuinely AUR-only packages
/// (`sddm-theme-noctalia-git`, `wvkbd`, `iio-niri`,
/// `noctalia-unofficial-auth-agent-git`) reachable on a fresh box. A fresh
/// CachyOS install ships neither yay nor paru, so before this every one of
/// them warn-and-skipped, leaving no SDDM theme, no on-screen keyboard and no
/// auto-rotation; the `[cachyos]` repo carries both helpers, so one
/// `pacman -S` fixes it there. On stock Arch neither is in an official repo,
/// so the bootstrap can only succeed once dotctl's own `setup_chaotic_aur`
/// has run — which is why `install()` asks twice, before and after that step,
/// rather than reordering the existing `pacman` calls.
///
/// Never hard-fails: `pacman -S` is attempted only when [`repo_has_pkg`] says
/// it will resolve, so a host with no source for a helper lands on the same
/// warn-and-continue dotctl has always done.
pub(crate) fn ensure_aur_helper(distro: Distro) -> Option<&'static str> {
    if let Some(helper) = aur_helper() {
        ok(&format!("AUR helper: {helper}"));
        return Some(helper);
    }
    if let Some(pkg) = AUR_HELPERS.into_iter().find(|p| repo_has_pkg(p)) {
        info(&format!(
            "No AUR helper on PATH — installing {pkg} from a configured repo ..."
        ));
        if pacman_install(&format!("{pkg} (AUR helper)"), &[pkg]).is_ok() {
            if let Some(helper) = aur_helper() {
                ok(&format!("AUR helper ready: {helper}"));
                return Some(helper);
            }
        }
    }
    warn(&format!(
        "no AUR helper found (looked for {}) — AUR-only packages will be skipped; {}",
        AUR_HELPERS.join(", "),
        aur_helper_hint(distro)
    ));
    None
}

/// AUR-only counterpart to [`pacman_install`], driven by an already-resolved
/// helper binary (see [`ensure_aur_helper`]). Used for the packages that are
/// genuinely in no binary repo on either distro: `sddm-theme-noctalia-git`,
/// `wvkbd`, `iio-niri`, `noctalia-unofficial-auth-agent-git`.
///
/// Best-effort: an AUR build failure (e.g. an upstream PKGBUILD broken against
/// the current toolchain) warns and returns Ok(()). AUR packages are optional
/// add-ons; a single broken one shouldn't take down the rest of
/// `dotctl install` (HexStrike clone, venv, pip install) or the downstream
/// `deploy` step when running `dotctl all`.
fn aur_install(helper: &str, label: &str, packages: &[&str]) -> Result<()> {
    info(&format!("Installing {label} (AUR, via {helper}) ..."));
    let mut args = vec!["-S", "--needed", "--noconfirm"];
    args.extend(packages.iter().copied());
    if let Err(e) = run(helper, &args) {
        warn(&format!(
            "{label} install failed (AUR build break or sudo unavailable): {e} — continuing"
        ));
        return Ok(());
    }
    ok(&format!("{label} installed"));
    Ok(())
}

/// `cargo install --git` one of our own crates, best-effort. grogu and teleia
/// are source-installed for the same reason (packaged nowhere) and fail for the
/// same reasons — no network, a broken upstream commit, a toolchain break in a
/// third-party dependency tree — none of which anything else in `install()`
/// depends on, and `deploy()` symlinks their configs whether or not the binary
/// landed. Warning here is what keeps a bad day at foolish-dev/grogu from
/// costing the user their whole deploy. See [`optional_pacman`] for the
/// invariant this protects.
///
/// `min` is the oldest version this repo's configs work with. An installed
/// binary older than that, or one whose version cannot be read, is rebuilt
/// from the branch tip; any other installed one is left alone, so a
/// hand-installed development build is never replaced behind the user's back.
pub(crate) fn ensure_cargo_git(
    label: &str,
    bin: &str,
    url: &str,
    lost: &str,
    min: Option<Version>,
) {
    let mut args = vec!["install", "--git", url, "--branch", "main", "--locked"];
    if command_exists(bin) {
        let have = installed_version(bin);
        match min {
            Some(want) if needs_update(have, want) => {
                let have = have.map_or_else(|| "of unreadable version".to_string(), version_string);
                info(&format!(
                    "{bin} {have} predates {}, the first this repo's configs work with — updating ...",
                    version_string(want)
                ));
                args.push("--force");
            }
            _ => {
                ok(&format!("{bin} already installed"));
                return;
            }
        }
    } else {
        info(&format!("Installing {label} ..."));
    }
    if let Err(e) = run("cargo", &args) {
        warn(&format!("{label} install failed: {e} — {lost}"));
        return;
    }
    ok(&format!("{bin} installed -> ~/.cargo/bin/{bin}"));
}

/// A crate version as (major, minor, patch): tuples order the way versions do.
type Version = (u32, u32, u32);

/// The oldest grogu the tracked Noctalia config works with. 0.2.0 is the first
/// to write v5's palettes/Grogu.json, which .config/noctalia/config.toml
/// selects. 0.1.x writes only v4's files, and its first step fails without
/// v4's settings.json, taking every other theme target down with it.
pub(crate) const GROGU_MIN_VERSION: Version = (0, 2, 0);

/// The version `<bin> --version` reports, or None when it will not say.
fn installed_version(bin: &str) -> Option<Version> {
    let out = Command::new(bin)
        .arg("--version")
        .stderr(Stdio::null())
        .output()
        .ok()?;
    parse_version_line(&String::from_utf8_lossy(&out.stdout))
}

/// clap's `--version` line: `<name> <major>.<minor>.<patch>`.
fn parse_version_line(line: &str) -> Option<Version> {
    let mut parts = line.split_whitespace().nth(1)?.split('.');
    let mut next = || -> Option<u32> { parts.next()?.parse().ok() };
    Some((next()?, next()?, next()?))
}

/// An unreadable version is treated as too old: rebuilding costs a minute,
/// keeping a binary that breaks the theme costs every wallpaper change.
fn needs_update(have: Option<Version>, want: Version) -> bool {
    have.is_none_or(|h| h < want)
}

fn version_string((major, minor, patch): Version) -> String {
    format!("{major}.{minor}.{patch}")
}

/// rust-analyzer for teleia's `[lsps.rust]`, plus the standard-library source
/// it loads std from: without `rust-src` the server starts but cannot load the
/// sysroot, so no std type resolves. Who provides the pair depends on where
/// rustc comes from:
///   rust   — the `rust-analyzer` and `rust-src` repo packages. The sysroot is
///            /usr, which is where `rust-src` puts the source.
///   rustup — the components of the same names. The sysroot is the toolchain
///            under ~/.rustup, so the repo `rust-src` would land where nothing
///            reads it. rustup also puts a rust-analyzer proxy on PATH (Arch's
///            package appends /usr/lib/rustup/bin via profile.d; rustup-init
///            uses ~/.cargo/bin) that exists whether or not the component
///            does, so a `command -v` gate would pass on the proxy alone.
///            `component add` is idempotent and needs no sudo, so it just runs.
/// Warn-and-continue either way, per [`optional_pacman`]: a missing language
/// server costs one LSP, not the deploy.
pub(crate) fn ensure_rust_analyzer() {
    let lost = "teleia's `rust` language server fails to start in Cargo projects";
    if command_exists("rustup") {
        info("Installing rust-analyzer + rust-src (rustup components) ...");
        match run("rustup", &["component", "add", "rust-analyzer", "rust-src"]) {
            Ok(()) => ok("rust-analyzer + rust-src installed (rustup components)"),
            Err(e) => warn(&format!("rust-analyzer not installed: {e} — {lost}")),
        }
        return;
    }
    if command_exists("rust-analyzer") && pacman_pkg_installed("rust-src") {
        ok("rust-analyzer + rust-src already installed");
        return;
    }
    if let Err(e) = pacman_install(
        "rust-analyzer + rust-src (teleia's Rust language server)",
        &["rust-analyzer", "rust-src"],
    ) {
        warn(&format!("rust-analyzer not installed: {e} — {lost}"));
    }
}

/// pkgfile keeps one `<repo>.files` database per sync repo in `dir`
/// (`/var/cache/pkgfile` by default) once it has run at all. Until then the
/// command-not-found handler has nothing to search.
fn pkgfile_db_present(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .any(|e| e.file_name().to_string_lossy().ends_with(".files"))
        })
        .unwrap_or(false)
}

/// Enable pkgfile's daily update timer, and start a first build in the
/// background when there is no database yet. `--no-block` because the download
/// need not hold up the rest of the install. Warn-and-continue throughout: a
/// missing database costs a hint, not the deploy.
pub(crate) fn ensure_pkgfile_db() {
    let lost = "zsh's command-not-found cannot name the package a command is in";
    let enabled = Command::new("systemctl")
        .args(["is-enabled", "--quiet", "pkgfile-update.timer"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if enabled {
        ok("pkgfile-update.timer already enabled");
    } else if let Err(e) = run(
        "sudo",
        &["systemctl", "enable", "--now", "pkgfile-update.timer"],
    ) {
        warn(&format!("pkgfile-update.timer not enabled: {e} — {lost}"));
        return;
    } else {
        ok("pkgfile-update.timer enabled (daily)");
    }
    if !pkgfile_db_present(Path::new("/var/cache/pkgfile")) {
        info("Building the pkgfile database in the background ...");
        if let Err(e) = run(
            "sudo",
            &["systemctl", "start", "--no-block", "pkgfile-update.service"],
        ) {
            warn(&format!(
                "pkgfile-update.service did not start: {e} — {lost}"
            ));
        }
    }
}

fn aur_failure_marker(pkg: &str) -> PathBuf {
    aur_failure_marker_in(&cache_dir(), pkg)
}

fn aur_failure_marker_in(root: &Path, pkg: &str) -> PathBuf {
    root.join("dotctl/aur-failed").join(pkg)
}

/// File mtime as a `SystemTime`, or `None` if it can't be read.
fn mtime_of(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Whether an AUR failure `marker` should still be honored (i.e. skip the
/// rebuild). The marker is only trusted while it's at least as new as the
/// running dotctl binary: reinstalling dotctl ships a new build strategy or
/// fix (e.g. the GCC 16 auth-agent patch), which supersedes every prior
/// failure and earns it one more attempt. A missing marker is never valid;
/// an unreadable mtime falls back to honoring the marker so a known-broken
/// build is never hammered every run.
fn marker_still_valid(marker: &Path) -> bool {
    if !marker.exists() {
        return false;
    }
    let exe_mtime = std::env::current_exe().ok().and_then(|p| mtime_of(&p));
    marker_still_valid_at(mtime_of(marker), exe_mtime)
}

/// [`marker_still_valid`] with both mtimes injected, so the
/// retry-on-reinstall rule is unit-testable without touching real files.
fn marker_still_valid_at(marker_mtime: Option<SystemTime>, exe_mtime: Option<SystemTime>) -> bool {
    match (marker_mtime, exe_mtime) {
        (Some(marker), Some(exe)) => marker >= exe,
        _ => true,
    }
}

/// `ensure_pacman_pkg` for AUR packages: pacman owns the local DB even
/// for AUR-installed packages, so the "already installed" check uses
/// the same [`pacman_pkg_installed`] helper.
///
/// Caches build failures under `$XDG_CACHE_HOME/dotctl/aur-failed/<pkg>`
/// (or `~/.cache/dotctl/aur-failed/<pkg>`). [`aur_install`] is best-effort
/// and returns Ok even when the AUR build fails, so without a marker each
/// subsequent `dotctl all` would re-run the same failing build — observed
/// in the wild when GCC 16 broke `noctalia-unofficial-auth-agent-git` and
/// every reinvocation burned ~30s on cmake+make before failing identically.
/// The marker is cleared as soon as the package shows up in pacman's DB
/// (user patched upstream / ran `yay -S` manually), and [`marker_still_valid`]
/// also ignores any marker older than the running dotctl binary — so
/// reinstalling dotctl retries each previously-failed build once. It
/// self-heals either way.
pub(crate) fn ensure_aur_pkg(helper: Option<&str>, label: &str, pkg: &str) -> Result<()> {
    let marker = aur_failure_marker(pkg);
    if pacman_pkg_installed(pkg) {
        let _ = fs::remove_file(&marker);
        ok(&format!("{label} already installed"));
        return Ok(());
    }
    if marker_still_valid(&marker) {
        warn(&format!(
            "{label} AUR build previously failed — skipping (rm {} or reinstall dotctl to retry)",
            marker.display()
        ));
        return Ok(());
    }
    // No helper ⇒ no build was attempted, so record nothing: a failure marker
    // here would wrongly claim this package is known-broken.
    let Some(helper) = helper else {
        warn(&format!(
            "{label} skipped — no AUR helper available (package: {pkg})"
        ));
        return Ok(());
    };
    aur_install(helper, label, &[pkg])?;
    // A build was attempted but pacman still doesn't see the package → real
    // build failure, record it.
    if !pacman_pkg_installed(pkg) {
        record_aur_failure(&marker);
    }
    Ok(())
}

/// Cache an AUR build failure so the next `dotctl` run skips the known-bad
/// rebuild instead of burning minutes on it again. Shared by
/// [`ensure_aur_pkg`] and [`ensure_noctalia_auth_agent`].
fn record_aur_failure(marker: &Path) {
    if let Some(parent) = marker.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(marker, "");
    warn(&format!(
        "cached failure marker at {} — rm to retry next run",
        marker.display()
    ));
}

/// Force-include `<unistd.h>` into a PKGBUILD's `build()` step. GCC 16
/// stopped leaking `<unistd.h>` through unrelated headers, so sources that
/// call `getpid()`/`read()`/etc. without including it no longer compile;
/// `-include unistd.h` puts it back for every translation unit, and CMake
/// folds the appended CXXFLAGS/CFLAGS into the configure step. Idempotent,
/// and a no-op for any PKGBUILD without a `build() {` line to anchor on.
fn patch_pkgbuild_unistd(pkgbuild: &str) -> String {
    const MARKER: &str = "# dotctl: force-include <unistd.h> (GCC 16 transitive-include fix)";
    if pkgbuild.contains(MARKER) {
        return pkgbuild.to_string();
    }
    let replacement = format!(
        "build() {{\n    {MARKER}\n    export CXXFLAGS+=\" -include unistd.h\"\n    export CFLAGS+=\" -include unistd.h\"\n"
    );
    pkgbuild.replacen("build() {\n", &replacement, 1)
}

/// Bring the AUR clone at `dir` back to upstream: discard whatever the last
/// run left in the working tree, then fast-forward.
///
/// The `reset --hard` is the whole point. [`build_noctalia_auth_agent`] writes
/// a patched PKGBUILD (see [`patch_pkgbuild_unistd`]) straight into the clone
/// and never restores it, so from the second run onwards the tree is
/// permanently dirty and the `--ff-only` pull aborts the moment upstream
/// touches PKGBUILD — the exact case the refresh exists to catch. Reproduced
/// against this box's own clone with a published upstream commit:
///   error: Your local changes to the following files would be overwritten by
///   merge: PKGBUILD / Please commit your changes or stash them before you
///   merge. / Aborting        (exit 1)
/// The caller discarded that status, printed "Refreshing ... AUR clone", and
/// rebuilt the same frozen PKGBUILD forever — so an upstream fix for the very
/// GCC 16 break we patch around could never arrive. It also wasted the one
/// retry [`marker_still_valid`] grants per dotctl reinstall, which re-runs the
/// identical build and re-records the identical failure.
///
/// `reset --hard` and not `git checkout -- PKGBUILD`: checkout restores the
/// working tree from the *index*, so anything a previous run or a user had
/// staged still blocks the merge, and it only covers the paths we remember
/// dirtying — the reset needs no such list and cannot drift as the patch grows.
/// Not a re-clone either: that throws away makepkg's downloaded sources and
/// costs a full refetch every run to fix a dirty file. And deliberately not
/// `git clean -fdx`: `src/`, `pkg/` and the built `*.pkg.tar.zst` are
/// makepkg's, untracked, and never block a fast-forward.
///
/// A clone the user has hand-edited loses those edits, and that is correct.
/// This lives under `$XDG_CACHE_HOME` — data dotctl creates and must be free
/// to recreate — and dotctl rewrites the PKGBUILD in it on every run anyway,
/// so it was never a place to keep work. The sanctioned override is the one
/// [`ensure_noctalia_auth_agent`] already documents: build and install the
/// package yourself, and `pacman_pkg_installed` short-circuits before this
/// function is ever reached.
fn refresh_aur_clone(dir: &str) -> Result<()> {
    // Undo last run's PKGBUILD patch before the pull, not after: a dirty
    // tracked file is what aborts the fast-forward.
    run("git", &["-C", dir, "reset", "--hard", "HEAD"])?;
    run("git", &["-C", dir, "pull", "--ff-only"])
}

/// Clone (or refresh) the AUR repo for `pkg`, patch its PKGBUILD to
/// force-include `<unistd.h>`, then build + install with `makepkg -si`.
fn build_noctalia_auth_agent(pkg: &str) -> Result<()> {
    let dir = cache_dir().join("dotctl/aur").join(pkg);
    let dir_str = dir.to_str().context("AUR build dir path is not utf-8")?;
    if dir.join(".git").exists() {
        info(&format!("Refreshing {pkg} AUR clone ..."));
        // Best-effort: a refresh that fails (no network, rewritten upstream
        // history) is no reason to skip building the checkout already on disk.
        // But it is reported now — the `let _` this replaced hid a pull that
        // had been failing on every run since the first build.
        if let Err(e) = refresh_aur_clone(dir_str) {
            warn(&format!(
                "could not refresh {pkg} AUR clone: {e} — building the checkout already on disk"
            ));
        }
    } else {
        info(&format!("Cloning {pkg} from the AUR ..."));
        if let Some(parent) = dir.parent() {
            fs::create_dir_all(parent)?;
        }
        run(
            "git",
            &[
                "clone",
                &format!("https://aur.archlinux.org/{pkg}.git"),
                dir_str,
            ],
        )?;
    }
    let pkgbuild = dir.join("PKGBUILD");
    let patched = patch_pkgbuild_unistd(&fs::read_to_string(&pkgbuild)?);
    fs::write(&pkgbuild, patched)?;
    info("Building patched PKGBUILD (force-include <unistd.h>) ...");
    // -s pulls any missing makedepends, -i installs the built package
    // (both shell out to sudo pacman — interactive, like the rest of dotctl).
    run_in(dir_str, "makepkg", &["-si", "--noconfirm"])
}

/// Build + install `noctalia-unofficial-auth-agent-git` (ships
/// `/usr/libexec/bb-auth` + a packaged `bb-auth.service` user unit) from a
/// locally patched PKGBUILD. A plain
/// [`ensure_aur_pkg`] won't do: the upstream sources omit `<unistd.h>` and
/// so fail to compile under GCC 16, and `yay -S` would refetch that broken
/// PKGBUILD and lose the fix on every run. Reuses the same failure-marker
/// bookkeeping, so a still-broken build (a *different* toolchain break) is
/// skipped next run and the marker self-heals once pacman reports the
/// package installed.
pub(crate) fn ensure_noctalia_auth_agent() -> Result<()> {
    const PKG: &str = "noctalia-unofficial-auth-agent-git";
    let label = "Noctalia auth agent";
    let marker = aur_failure_marker(PKG);
    if pacman_pkg_installed(PKG) {
        let _ = fs::remove_file(&marker);
        ok(&format!("{label} already installed"));
        return Ok(());
    }
    if marker_still_valid(&marker) {
        warn(&format!(
            "{label} build previously failed — skipping (rm {} or reinstall dotctl to retry)",
            marker.display()
        ));
        return Ok(());
    }
    if !command_exists("git") || !command_exists("makepkg") {
        warn(&format!(
            "{label} needs git + makepkg (base-devel) to build — install them then rerun `dotctl install`"
        ));
        return Ok(());
    }
    if let Err(e) = build_noctalia_auth_agent(PKG) {
        warn(&format!("{label} build failed: {e} — continuing"));
    }
    if !pacman_pkg_installed(PKG) {
        record_aur_failure(&marker);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::git;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn pacman_pkg_installed_returns_false_for_missing_pkg() {
        // `pacman -Q __dotctl_missing_xyz` exits non-zero on any pacman
        // host (Arch, CachyOS, EndeavourOS, …) and
        // also returns false on hosts without pacman at all (spawn
        // failure caught by unwrap_or(false)) — covers both CI shapes.
        assert!(!pacman_pkg_installed("__dotctl_missing_pkg_xyz"));
    }

    #[test]
    fn aur_failure_marker_lives_under_dotctl_aur_failed() {
        // The marker path is a user-visible contract: the warn message
        // tells the user `rm <path>` to retry, so the layout must not
        // drift silently.
        let tmp = TempDir::new().expect("tempdir");
        let m = aur_failure_marker_in(tmp.path(), "some-aur-pkg");
        assert_eq!(m, tmp.path().join("dotctl/aur-failed/some-aur-pkg"));
    }

    #[test]
    fn pacman_pkg_installed_finds_base_filesystem_pkg_on_pacman_hosts() {
        // The `filesystem` package is in `[core]`, is required by `base`, and
        // is present on every Arch-family host including CachyOS. It owns
        // /usr/bin itself and has no command of the same name — so
        // command_exists("filesystem") always misses. This is exactly
        // the shape that motivated the helper.
        if command_exists("pacman") {
            assert!(pacman_pkg_installed("filesystem"));
        }
    }

    #[test]
    fn pkgfile_db_counts_only_repo_databases() {
        let tmp = TempDir::new().expect("tempdir");
        let dir = tmp.path().join("pkgfile");
        assert!(
            !pkgfile_db_present(&dir),
            "a missing cache dir is no database"
        );
        fs::create_dir(&dir).unwrap();
        assert!(
            !pkgfile_db_present(&dir),
            "an empty cache dir is no database"
        );
        fs::write(dir.join("core.files.lock"), "").unwrap();
        fs::write(dir.join("README"), "").unwrap();
        assert!(!pkgfile_db_present(&dir), "only <repo>.files counts");
        fs::write(dir.join("extra.files"), "").unwrap();
        assert!(pkgfile_db_present(&dir));
    }

    // ── patch_pkgbuild_unistd ──────────────────────────────────────────────
    //
    // The GCC 16 fix that lets noctalia-unofficial-auth-agent-git compile.
    // It's pure string surgery on a fetched PKGBUILD, so pin the contract:
    // inject the force-include at the top of build(), do it exactly once,
    // and never corrupt a PKGBUILD it can't anchor on.

    #[test]
    fn patch_pkgbuild_unistd_injects_force_include_at_top_of_build() {
        let pkgbuild = "pkgname=foo\nbuild() {\n    cmake -B build .\n    cmake --build build\n}\n";
        let patched = patch_pkgbuild_unistd(pkgbuild);

        let build_open = patched.find("build() {").expect("build() kept");
        let flag = patched.find("-include unistd.h").expect("flag injected");
        let first_cmake = patched.find("cmake -B build").expect("cmake kept");
        assert!(
            build_open < flag && flag < first_cmake,
            "force-include must sit at the top of build(), before the build commands"
        );
        assert!(patched.contains("export CXXFLAGS+=\" -include unistd.h\""));
        assert!(patched.contains("export CFLAGS+=\" -include unistd.h\""));
    }

    #[test]
    fn patch_pkgbuild_unistd_is_idempotent() {
        let pkgbuild = "build() {\n    cmake --build build\n}\n";
        let once = patch_pkgbuild_unistd(pkgbuild);
        let twice = patch_pkgbuild_unistd(&once);
        assert_eq!(once, twice, "re-patching must not stack a second copy");
        assert_eq!(
            once.matches("-include unistd.h").count(),
            2,
            "exactly two exports (CXXFLAGS + CFLAGS), even across repeated patches"
        );
    }

    #[test]
    fn patch_pkgbuild_unistd_leaves_pkgbuild_without_build_untouched() {
        // No `build() {` to anchor on (e.g. a -bin package) → return the
        // input verbatim rather than emit a broken file.
        let pkgbuild =
            "pkgname=foo-bin\npackage() {\n    install -Dm755 foo \"$pkgdir/usr/bin/foo\"\n}\n";
        assert_eq!(patch_pkgbuild_unistd(pkgbuild), pkgbuild);
    }

    // ── marker_still_valid_at ──────────────────────────────────────────────
    //
    // A cached AUR failure is only trusted while it's at least as new as the
    // running dotctl binary, so reinstalling dotctl (e.g. shipping the GCC 16
    // auth-agent fix) retries previously-failed builds exactly once.

    #[test]
    fn marker_honored_while_at_least_as_new_as_binary() {
        let exe = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let after = exe + Duration::from_secs(5);
        assert!(marker_still_valid_at(Some(after), Some(exe)));
        assert!(
            marker_still_valid_at(Some(exe), Some(exe)),
            "a marker written by this very binary (equal mtime) is still honored"
        );
    }

    #[test]
    fn marker_superseded_when_binary_is_newer() {
        // dotctl was reinstalled after the marker was written → give the build
        // another chance instead of skipping it forever.
        let marker = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let exe = marker + Duration::from_secs(5);
        assert!(!marker_still_valid_at(Some(marker), Some(exe)));
    }

    #[test]
    fn marker_honored_when_an_mtime_is_unknown() {
        // Can't compare → keep skipping rather than hammer a known-broken build.
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        assert!(marker_still_valid_at(None, Some(t)));
        assert!(marker_still_valid_at(Some(t), None));
        assert!(marker_still_valid_at(None, None));
    }

    // ── refresh_aur_clone ──────────────────────────────────────────────────
    //
    // `build_noctalia_auth_agent` writes a patched PKGBUILD into the clone and
    // never restores it, so the refresh above it had been a no-op ever since
    // the first build. Real git in a TempDir, same shape as the git_pull test
    // in update.rs: no network, no sudo, no makepkg.

    #[test]
    fn refresh_aur_clone_discards_last_runs_pkgbuild_patch_so_the_pull_lands() {
        // The bug: from run 2 onwards `git pull --ff-only` aborted with
        // "Your local changes to the following files would be overwritten by
        // merge: PKGBUILD", the caller discarded that exit status, and dotctl
        // rebuilt the same frozen PKGBUILD forever while printing
        // "Refreshing ... AUR clone".
        if !command_exists("git") {
            return; // git-less host; nothing to exercise
        }
        let tmp = TempDir::new().expect("tempdir");
        let upstream = tmp.path().join("aur");
        let clone = tmp.path().join("clone");
        fs::create_dir_all(&upstream).expect("mkdir upstream");

        git(&upstream, &["init", "-q", "-b", "master"]);
        git(&upstream, &["config", "user.email", "t@t"]);
        git(&upstream, &["config", "user.name", "t"]);
        fs::write(
            upstream.join("PKGBUILD"),
            "pkgver=1\nbuild() {\n  make\n}\n",
        )
        .expect("seed pkgbuild");
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

        // Upstream publishes the fix we have been patching around locally...
        let fixed = "pkgver=2\nbuild() {\n  make\n}\n";
        fs::write(upstream.join("PKGBUILD"), fixed).expect("bump pkgbuild");
        git(&upstream, &["commit", "-qam", "fix build under GCC 16"]);

        // ...while the clone still carries last run's in-place patch.
        fs::write(
            clone.join("PKGBUILD"),
            patch_pkgbuild_unistd("pkgver=1\nbuild() {\n  make\n}\n"),
        )
        .expect("patch pkgbuild");

        refresh_aur_clone(clone.to_str().unwrap()).expect("refresh should reset then fast-forward");

        assert_eq!(
            fs::read_to_string(clone.join("PKGBUILD")).expect("read pkgbuild"),
            fixed,
            "the upstream PKGBUILD must reach a clone dotctl dirtied on the previous run"
        );
    }

    #[test]
    fn refresh_aur_clone_leaves_makepkgs_untracked_build_output_alone() {
        // Why `reset --hard` and not `clean -fdx`: src/ holds sources makepkg
        // already downloaded, and untracked files never block a fast-forward.
        // Deleting them would cost a full refetch on every single run.
        if !command_exists("git") {
            return; // git-less host; nothing to exercise
        }
        let tmp = TempDir::new().expect("tempdir");
        let upstream = tmp.path().join("aur");
        let clone = tmp.path().join("clone");
        fs::create_dir_all(&upstream).expect("mkdir upstream");

        git(&upstream, &["init", "-q", "-b", "master"]);
        git(&upstream, &["config", "user.email", "t@t"]);
        git(&upstream, &["config", "user.name", "t"]);
        fs::write(upstream.join("PKGBUILD"), "pkgver=1\n").expect("seed pkgbuild");
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

        fs::create_dir_all(clone.join("src")).expect("mkdir src");
        fs::write(clone.join("src/upstream.tar.gz"), "sources").expect("seed sources");
        fs::write(clone.join("pkg-1-x86_64.pkg.tar.zst"), "built").expect("seed artifact");
        fs::write(clone.join("PKGBUILD"), "pkgver=1\n# dirtied\n").expect("dirty pkgbuild");

        refresh_aur_clone(clone.to_str().unwrap()).expect("refresh on an up-to-date clone");

        assert_eq!(
            fs::read_to_string(clone.join("PKGBUILD")).expect("read pkgbuild"),
            "pkgver=1\n",
            "the tracked PKGBUILD must be restored"
        );
        assert!(
            clone.join("src/upstream.tar.gz").exists(),
            "makepkg's downloaded sources must survive the refresh"
        );
        assert!(
            clone.join("pkg-1-x86_64.pkg.tar.zst").exists(),
            "makepkg's built package must survive the refresh"
        );
    }

    #[test]
    fn parse_version_line_reads_what_clap_prints() {
        assert_eq!(parse_version_line("grogu 0.2.0\n"), Some((0, 2, 0)));
        assert_eq!(parse_version_line("grogu 0.10.3"), Some((0, 10, 3)));
        assert_eq!(parse_version_line("grogu"), None);
        assert_eq!(parse_version_line("grogu 0.2"), None);
        assert_eq!(parse_version_line("error: unexpected argument"), None);
    }

    #[test]
    fn needs_update_compares_numerically_and_distrusts_an_unreadable_version() {
        assert!(needs_update(Some((0, 1, 0)), GROGU_MIN_VERSION));
        assert!(!needs_update(Some((0, 2, 0)), GROGU_MIN_VERSION));
        // Numeric, not lexical: as strings, "0.10.0" sorts before "0.2.0".
        assert!(!needs_update(Some((0, 10, 0)), GROGU_MIN_VERSION));
        assert!(needs_update(None, GROGU_MIN_VERSION));
    }

    #[test]
    fn preferred_aur_helper_keeps_yay_first_then_falls_back_to_paru() {
        // The ordering *is* the Arch no-regression guarantee: every pre-port
        // dotctl ran `yay` unconditionally, so yay-first means no host that
        // has yay changes which binary gets spawned.
        assert_eq!(AUR_HELPERS, ["yay", "paru"]);
        assert_eq!(preferred_aur_helper(|_| true), Some("yay"));
        assert_eq!(preferred_aur_helper(|h| h == "paru"), Some("paru"));
        assert_eq!(preferred_aur_helper(|h| h == "yay"), Some("yay"));
        assert_eq!(preferred_aur_helper(|_| false), None);
    }
}
