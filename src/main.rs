use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

#[derive(Parser)]
#[command(
    version,
    about = "Installer/deployer for foolish-dev/niri-dotfiles",
    propagate_version = true
)]
struct Cli {
    /// Path to the dotfiles repo (defaults to $HOME/dotfiles)
    #[arg(long, env = "DOTFILES_REPO", global = true)]
    repo: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install tools: grogu, HexStrike AI, Neovim, tmux, fastfetch, Noctalia (shell + qs + SDDM noctalia login, greetd/ReGreet fallback + auth agent)
    Install,
    /// Symlink .config/* + .local/bin/* into $HOME and enable user services
    Deploy,
    /// Clone (or pull) the repo, then install + deploy
    All,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let repo = cli.repo.unwrap_or_else(|| home().join("dotfiles"));
    match cli.cmd {
        Cmd::Install => install(),
        Cmd::Deploy => deploy(&repo),
        Cmd::All => {
            ensure_repo(&repo)?;
            install()?;
            deploy(&repo)
        }
    }
}

// ── Output helpers ────────────────────────────────────────────────────────────

fn info(msg: &str) {
    println!("\x1b[34m[*]\x1b[0m {msg}");
}
fn ok(msg: &str) {
    println!("\x1b[32m[+]\x1b[0m {msg}");
}
fn warn(msg: &str) {
    eprintln!("\x1b[31m[!]\x1b[0m {msg}");
}

// ── Process / FS helpers ──────────────────────────────────────────────────────

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .expect("HOME unset")
}

fn command_exists(cmd: &str) -> bool {
    // Pass `cmd` as a positional argument to sh rather than interpolating
    // it into the script. All current callers pass literals, but the
    // interpolated form would treat a cmd of e.g. `foo; rm -rf $HOME` as
    // two statements and run the trailing one — a footgun the moment a
    // future caller forwards a user-supplied name.
    Command::new("sh")
        .args(["-c", r#"command -v "$1" >/dev/null"#, "_", cmd])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run(prog: &str, args: &[&str]) -> Result<()> {
    run_in(".", prog, args)
}

/// Like [`run`] but with an explicit working directory. `makepkg` builds
/// in the directory that holds the PKGBUILD, so it can't just inherit
/// dotctl's cwd the way every other command can.
fn run_in(dir: &str, prog: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(prog)
        .current_dir(dir)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("spawn `{prog}`"))?;
    if !status.success() {
        return Err(anyhow!("`{prog}` exited with {status}"));
    }
    Ok(())
}

/// Write `content` to a root-owned `path` via `sudo tee`, creating parent
/// directories first. Used for greeter config files whose multi-line TOML/CSS
/// bodies don't fit the inline `printf` pattern the smaller drop-ins use.
fn sudo_write(path: &str, content: &str) -> Result<()> {
    if let Some(dir) = Path::new(path).parent() {
        run("sudo", &["mkdir", "-p", &dir.display().to_string()])?;
    }
    let mut child = Command::new("sudo")
        .args(["tee", path])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("spawn `sudo tee {path}`"))?;
    child
        .stdin
        .take()
        .context("sudo tee stdin")?
        .write_all(content.as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        return Err(anyhow!("`sudo tee {path}` exited with {status}"));
    }
    Ok(())
}

fn has_pacman_repo(name: &str) -> bool {
    let header = format!("[{name}]");
    fs::read_to_string("/etc/pacman.conf")
        .map(|c| c.lines().any(|l| l.trim() == header))
        .unwrap_or(false)
}

fn setup_chaotic_aur() -> Result<()> {
    if !command_exists("pacman") {
        return Ok(());
    }
    if has_pacman_repo("chaotic-aur") {
        ok("Chaotic AUR repo already present");
        return Ok(());
    }
    info("Adding Chaotic AUR repository ...");
    run(
        "sudo",
        &[
            "pacman-key",
            "--recv-key",
            "3056513887B78AEB",
            "--keyserver",
            "keyserver.ubuntu.com",
        ],
    )?;
    run("sudo", &["pacman-key", "--lsign-key", "3056513887B78AEB"])?;
    run(
        "sudo",
        &[
            "pacman",
            "-U",
            "--noconfirm",
            "https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-keyring.pkg.tar.zst",
            "https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst",
        ],
    )?;
    run(
        "sudo",
        &[
            "sh",
            "-c",
            "printf '\\n[chaotic-aur]\\nInclude = /etc/pacman.d/chaotic-mirrorlist\\n' >> /etc/pacman.conf",
        ],
    )?;
    run("sudo", &["pacman", "-Sy"])?;
    ok("Chaotic AUR repo added");
    Ok(())
}

fn setup_blackarch() -> Result<()> {
    if !command_exists("pacman") {
        return Ok(());
    }
    if has_pacman_repo("blackarch") {
        ok("BlackArch repo already present");
        return Ok(());
    }
    info("Adding BlackArch repository (via strap.sh) ...");
    let strap = "/tmp/blackarch-strap.sh";
    run(
        "curl",
        &["-fsSL", "-o", strap, "https://blackarch.org/strap.sh"],
    )?;
    run("sudo", &["chmod", "+x", strap])?;
    // strap.sh prompts for confirmation; pipe `yes` through so the install is
    // non-interactive like the rest of dotctl.
    run("sh", &["-c", &format!("yes | sudo {strap}")])?;
    ok("BlackArch repo added");
    Ok(())
}

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

fn ensure_pacman(label: &str, cmd: &str, packages: &[&str]) -> Result<()> {
    if command_exists(cmd) {
        ok(&format!("{label} already installed"));
        Ok(())
    } else {
        pacman_install(label, packages)
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

/// Same shape as [`ensure_pacman`] but detects already-installed state
/// via `pacman -Q PKG` instead of `command -v BINARY`. Required for
/// noctalia-shell, whose pacman package places QML config under
/// /etc/xdg/quickshell/noctalia-shell/ and exposes no command of the
/// same name — so the binary check always missed and `dotctl install`
/// re-invoked `sudo pacman -S` (prompting for a password each run)
/// even when the package was already up to date.
fn ensure_pacman_pkg(label: &str, pkg: &str, packages: &[&str]) -> Result<()> {
    if pacman_pkg_installed(pkg) {
        ok(&format!("{label} already installed"));
        Ok(())
    } else {
        pacman_install(label, packages)
    }
}

/// AUR-only counterpart to [`pacman_install`]. Requires `yay` on PATH
/// (the chaotic-aur setup that runs earlier in `install()` covers
/// noctalia-shell + noctalia-qs without yay; this is for the AUR-only
/// ecosystem add-ons like `sddm-theme-noctalia-git` and
/// `noctalia-unofficial-auth-agent-git`).
///
/// Best-effort: `yay` is missing OR the AUR build fails (e.g. upstream
/// PKGBUILD broken against the current toolchain) → warn and return
/// Ok(()). AUR packages are optional add-ons; a single broken one
/// shouldn't take down the rest of `dotctl install` (HexStrike clone,
/// venv, pip install) or the downstream `deploy` step when running
/// `dotctl all`.
fn aur_install(label: &str, packages: &[&str]) -> Result<()> {
    if !command_exists("yay") {
        warn(&format!(
            "{label} not installed and `yay` not found — install yay (or another AUR helper) then rerun `dotctl install`; packages: {}",
            packages.join(" ")
        ));
        return Ok(());
    }
    info(&format!("Installing {label} (AUR) ..."));
    let mut args = vec!["-S", "--needed", "--noconfirm"];
    args.extend(packages.iter().copied());
    if let Err(e) = run("yay", &args) {
        warn(&format!(
            "{label} install failed (AUR build break or sudo unavailable): {e} — continuing"
        ));
        return Ok(());
    }
    ok(&format!("{label} installed"));
    Ok(())
}

fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".cache"))
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
fn ensure_aur_pkg(label: &str, pkg: &str) -> Result<()> {
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
    aur_install(label, &[pkg])?;
    // Yay was present and a build was attempted but pacman still doesn't
    // see the package → real build failure, record it.
    if command_exists("yay") && !pacman_pkg_installed(pkg) {
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

/// Clone (or refresh) the AUR repo for `pkg`, patch its PKGBUILD to
/// force-include `<unistd.h>`, then build + install with `makepkg -si`.
fn build_noctalia_auth_agent(pkg: &str) -> Result<()> {
    let dir = cache_dir().join("dotctl/aur").join(pkg);
    let dir_str = dir.to_str().context("AUR build dir path is not utf-8")?;
    if dir.join(".git").exists() {
        info(&format!("Refreshing {pkg} AUR clone ..."));
        // Best-effort: upstream may have fixed the build since last time.
        let _ = run("git", &["-C", dir_str, "pull", "--ff-only"]);
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
fn ensure_noctalia_auth_agent() -> Result<()> {
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

/// SDDM reads every `*.conf` here; we own one drop-in instead of editing
/// /etc/sddm.conf so other settings (autologin, numlock, …) are left alone.
/// SDDM stays installed as a themed fallback even though greetd is the
/// active display manager.
const SDDM_THEME_CONF: &str = "/etc/sddm.conf.d/10-noctalia-theme.conf";

/// greetd's whole config (we own the file; the package ships only an
/// example). cage hosts ReGreet, which lists the niri session it discovers
/// under /usr/share/wayland-sessions (niri.desktop → niri-session). The
/// `greeter` system user is created by the greetd package.
const GREETD_CONF: &str = "/etc/greetd/config.toml";
const GREETD_CONFIG_BODY: &str = "[terminal]\nvt = 1\n\n[default_session]\ncommand = \"dbus-run-session cage -s -mlast -d -- regreet\"\nuser = \"greeter\"\n";

/// ReGreet's own config + GTK CSS, embedded at build time and written to
/// /etc/greetd/ at install. The CSS approximates the noctalia look; sessions
/// are auto-discovered from /usr/share/wayland-sessions (niri-session).
const REGREET_CONF: &str = "/etc/greetd/regreet.toml";
const REGREET_TOML: &str = include_str!("../assets/greetd/regreet.toml");
const REGREET_CSS_PATH: &str = "/etc/greetd/regreet.css";
const REGREET_CSS: &str = include_str!("../assets/greetd/regreet.css");

/// The systemd alias symlinked to whichever display manager is enabled.
/// `systemctl enable sddm.service` writes this; if it already points
/// elsewhere, another DM owns the login screen.
const DISPLAY_MANAGER_UNIT: &str = "/etc/systemd/system/display-manager.service";

/// Basename of the unit `display-manager.service` points at (e.g.
/// `sddm.service`, `gdm.service`), or `None` when no display manager is
/// enabled. Reading the symlink needs no root, so the "is another DM
/// already wired up?" decision stays cheap and side-effect-free.
fn current_display_manager() -> Option<String> {
    let target = fs::read_link(DISPLAY_MANAGER_UNIT).ok()?;
    target.file_name()?.to_str().map(str::to_string)
}

/// What to do about `sddm.service` given the currently-enabled display
/// manager. Pure, so the "never steal the login screen from a third-party
/// DM" rule is unit-testable without touching systemd. `greetd.service` is
/// treated as ours to replace (it's the fallback greeter we also configure);
/// gdm/lightdm/ly are left alone.
enum LoginAction {
    AlreadySddm,
    OtherDm(String),
    Enable,
}

fn login_action(current_dm: Option<String>) -> LoginAction {
    match current_dm {
        Some(dm) if dm == "sddm.service" => LoginAction::AlreadySddm,
        Some(dm) if dm == "greetd.service" => LoginAction::Enable,
        Some(other) => LoginAction::OtherDm(other),
        None => LoginAction::Enable,
    }
}

/// Set up the login screen: the graphical noctalia **SDDM** theme as the
/// active display manager, with greetd + ReGreet (also noctalia-themed) left
/// installed and configured as a disabled fallback. Selects the noctalia SDDM
/// theme and enables `sddm.service` — but only when no third-party DM
/// (gdm/lightdm/ly) is already enabled, so we never silently steal someone
/// else's login screen. `greetd.service` is the fallback we also configure, so
/// switching off it to sddm is allowed. Best-effort like the rest of
/// `install()`: a missing sddm or a failed `systemctl` call warns and returns
/// Ok rather than aborting.
fn setup_noctalia_login() -> Result<()> {
    // Keep greetd + ReGreet configured as a disabled fallback, so a manual
    // `systemctl enable greetd` still lands on the themed graphical greeter.
    if command_exists("greetd") {
        info("Writing greetd + ReGreet fallback config ...");
        sudo_write(GREETD_CONF, GREETD_CONFIG_BODY)?;
        sudo_write(REGREET_CONF, REGREET_TOML)?;
        sudo_write(REGREET_CSS_PATH, REGREET_CSS)?;
        ok("greetd + ReGreet fallback config written");
    }

    if !command_exists("sddm") {
        warn("sddm not found (theme install may have failed) — skipping login-screen setup");
        return Ok(());
    }
    info("Selecting the noctalia SDDM theme ...");
    run(
        "sudo",
        &[
            "sh",
            "-c",
            &format!(
                "mkdir -p /etc/sddm.conf.d && printf '[Theme]\\nCurrent=noctalia\\n' > {SDDM_THEME_CONF}"
            ),
        ],
    )?;
    ok("SDDM theme set to noctalia");

    match login_action(current_display_manager()) {
        LoginAction::AlreadySddm => ok("sddm is already the active display manager"),
        LoginAction::OtherDm(other) => warn(&format!(
            "{other} is already the enabled display manager — leaving it in place; \
             run `sudo systemctl disable {other} && sudo systemctl enable sddm.service` to switch"
        )),
        LoginAction::Enable => {
            // Disable greetd first: a second DM can't claim display-manager.service
            // while greetd still owns the alias. Harmless no-op when greetd isn't set.
            let _ = run("sudo", &["systemctl", "disable", "greetd.service"]);
            info("Enabling sddm.service ...");
            if let Err(e) = run("sudo", &["systemctl", "enable", "sddm.service"]) {
                warn(&format!("failed to enable sddm.service: {e} — continuing"));
            } else {
                ok("sddm.service enabled — noctalia login screen active on next boot");
            }
        }
    }
    Ok(())
}

/// `git -C <repo> pull --ff-only --autostash`. The `--autostash` matters:
/// `dotctl deploy` symlinks `~/.config/noctalia` straight at this repo, and
/// grogu rewrites the tracked `colors.json` + `colorschemes/Grogu/Grogu.json`
/// in place on every wallpaper change (live theme repaint). A plain
/// `git pull --ff-only` then bails on the dirty tree, so re-running
/// `dotctl all` to update would silently skip the pull once the theme had
/// ever been repainted. Autostash shelves the repaint, fast-forwards, then
/// restores it — updates keep flowing and the user keeps their colors.
/// (`Grogu.json` can't just be gitignored away like the nvim/tmux grogu
/// fragments: settings.json pins `predefinedScheme=Grogu`, so the committed
/// scheme is the required first-boot default.)
fn git_pull(repo_str: &str) -> Result<()> {
    run("git", &["-C", repo_str, "pull", "--ff-only", "--autostash"])
}

fn ensure_repo(repo: &Path) -> Result<()> {
    let repo_str = repo
        .to_str()
        .ok_or_else(|| anyhow!("repo path is not valid utf-8: {}", repo.display()))?;
    if repo.join(".git").exists() {
        info(&format!("Updating {repo_str} ..."));
        if git_pull(repo_str).is_err() {
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

// ── install ───────────────────────────────────────────────────────────────────

fn install() -> Result<()> {
    if !command_exists("cargo") {
        return Err(anyhow!(
            "cargo not found — install rust first (https://rustup.rs)"
        ));
    }

    // Prerequisites assumed by later steps. Idempotent — `ensure_pacman`
    // is a no-op when the binary is already on PATH. Without these, a
    // fresh-box install would fail later with cryptic errors:
    //   git    → ensure_repo, cargo install --git for grogu
    //   curl   → setup_blackarch fetches strap.sh
    //   python → HexStrike AI venv (Arch's package is `python`, the
    //            binary is `python3`)
    ensure_pacman("git", "git", &["git"])?;
    ensure_pacman("curl", "curl", &["curl"])?;
    ensure_pacman("Python 3", "python3", &["python"])?;

    // Chaotic AUR (prereq for noctalia-shell, noctalia-qs on Arch)
    setup_chaotic_aur()?;

    // BlackArch (2800+ offensive-security tools, paired with HexStrike AI)
    setup_blackarch()?;

    // grogu
    if command_exists("grogu") {
        ok("grogu already installed");
    } else {
        info("Installing grogu (wallpaper-driven theme propagator) ...");
        run(
            "cargo",
            &[
                "install",
                "--git",
                "https://github.com/foolish-dev/grogu",
                "--branch",
                "main",
                "--locked",
            ],
        )?;
        ok("grogu installed -> ~/.cargo/bin/grogu");
    }

    ensure_pacman("tmux", "tmux", &["tmux"])?;
    ensure_pacman("fastfetch", "fastfetch", &["fastfetch"])?;
    ensure_pacman("Neovim", "nvim", &["neovim"])?;
    ensure_pacman_pkg(
        "Noctalia",
        "noctalia-shell",
        &["noctalia-shell", "noctalia-qs"],
    )?;
    // AUR add-ons for the full Noctalia ecosystem.
    // sddm-theme-noctalia-git (yay): login-screen theme matched to the shell;
    //   depends on sddm, so this also pulls in the display manager itself.
    //   setup_noctalia_login then selects the theme + enables sddm.service.
    // noctalia-unofficial-auth-agent-git: ships /usr/libexec/bb-auth (polkit
    //   agent + GNOME-keyring prompter) and its own bb-auth.service user unit,
    //   which `dotctl deploy` enables. Built from a locally patched PKGBUILD —
    //   see ensure_noctalia_auth_agent for the GCC 16 fix.
    ensure_aur_pkg("Noctalia SDDM theme", "sddm-theme-noctalia-git")?;
    // The noctalia SDDM theme above is the active login screen; greetd +
    // ReGreet (graphical, hosted by cage) is configured as a disabled
    // fallback. All in `extra`.
    ensure_pacman("greetd", "greetd", &["greetd"])?;
    ensure_pacman("regreet", "regreet", &["greetd-regreet"])?;
    ensure_pacman("cage", "cage", &["cage"])?;
    setup_noctalia_login()?;
    ensure_noctalia_auth_agent()?;

    // HexStrike AI
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
    if !hex_env.exists() {
        info("Creating HexStrike Python venv ...");
        run("python3", &["-m", "venv", hex_env.to_str().unwrap()])?;
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

// ── deploy ────────────────────────────────────────────────────────────────────

fn deploy(repo: &Path) -> Result<()> {
    if !repo.exists() {
        return Err(anyhow!("repo path not found: {}", repo.display()));
    }
    let h = home();

    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup_dir = h.join(format!(".dotfiles-backup/{ts}"));

    info(&format!("=== Deploying from {} ===", repo.display()));

    for d in ["teleia", "nvim", "noctalia", "fastfetch", "tmux"] {
        let src = repo.join(".config").join(d);
        if src.exists() {
            let dest = h.join(".config").join(d);
            link_item(&src, &dest, &backup_dir, &h)?;
        }
    }

    link_item(
        &repo.join(".config/systemd/user/hexstrike-server.service"),
        &h.join(".config/systemd/user/hexstrike-server.service"),
        &backup_dir,
        &h,
    )?;

    // The noctalia auth agent now uses its packaged `bb-auth.service` unit
    // (enabled below), so drop the obsolete custom unit symlink an earlier
    // deploy may have left behind — otherwise it lingers as a dangling link.
    let stale_unit = h.join(".config/systemd/user/noctalia-auth-agent.service");
    if stale_unit.is_symlink() {
        let _ = fs::remove_file(&stale_unit);
    }

    let bin_src = repo.join(".local/bin");
    let bin_dest = h.join(".local/bin");
    fs::create_dir_all(&bin_dest)?;
    if bin_src.is_dir() {
        for entry in fs::read_dir(&bin_src)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let dst = bin_dest.join(entry.file_name());
                link_item(&entry.path(), &dst, &backup_dir, &h)?;
            }
        }
    }

    // Wallpapers: symlink the curated set into ~/Pictures/Wallpapers, the
    // directory noctalia's wallpaper picker watches (setWallpaperOnAllMonitors
    // is on, so they apply to every output). link_item backs up any real file
    // of the same name first, and re-linking ours is idempotent.
    let wall_src = repo.join("wallpapers");
    let wall_dest = h.join("Pictures/Wallpapers");
    if wall_src.is_dir() {
        fs::create_dir_all(&wall_dest)?;
        for entry in fs::read_dir(&wall_src)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let dst = wall_dest.join(entry.file_name());
                link_item(&entry.path(), &dst, &backup_dir, &h)?;
            }
        }
    }

    if let Err(e) = run("systemctl", &["--user", "daemon-reload"]) {
        warn(&format!("systemctl --user daemon-reload failed: {e}"));
    }
    enable_user_unit(
        "hexstrike-server.service",
        &h.join("tools/hexstrike-ai/hexstrike-env/bin/python3"),
        "run `dotctl install` first to clone + venv hexstrike-ai",
    );
    enable_user_unit(
        "bb-auth.service",
        Path::new("/usr/libexec/bb-auth"),
        "reinstall dotctl (or run `dotctl install`) to build noctalia-unofficial-auth-agent-git",
    );

    ok("Deploy complete");
    Ok(())
}

/// `systemctl --user enable --now UNIT`, but first verify the unit's
/// ExecStart prerequisite exists — otherwise enable creates a wants
/// symlink and the unit immediately enters a failed state, retry-looping
/// per `Restart=on-failure` and spamming the journal. Skip cleanly and
/// tell the user what to run instead.
fn enable_user_unit(unit: &str, prereq: &Path, fix_hint: &str) {
    if !prereq.exists() {
        warn(&format!(
            "{unit} not enabled — prerequisite missing: {} ({fix_hint})",
            prereq.display()
        ));
        return;
    }
    let ok_status = Command::new("systemctl")
        .args(["--user", "enable", "--now", unit])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok_status {
        ok(&format!("{unit} enabled"));
    } else {
        warn(&format!("{unit} not enabled (systemctl returned non-zero)"));
    }
}

fn link_item(src: &Path, dest: &Path, backup_dir: &Path, home: &Path) -> Result<()> {
    if dest.exists() || dest.is_symlink() {
        if let (Ok(a), Ok(b)) = (dest.canonicalize(), src.canonicalize()) {
            if a == b {
                return Ok(());
            }
        }
        let rel = dest.strip_prefix(home).unwrap_or(dest);
        let backup_target = backup_dir.join(rel);
        if let Some(parent) = backup_target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(dest, &backup_target)
            .with_context(|| format!("backup {} -> {}", dest.display(), backup_target.display()))?;
        warn(&format!(
            "Backed up: {} -> {}",
            dest.display(),
            backup_target.display()
        ));
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    symlink(src, dest)
        .with_context(|| format!("symlink {} -> {}", src.display(), dest.display()))?;
    let pretty = dest
        .strip_prefix(home)
        .map(|p| format!("~/{}", p.display()))
        .unwrap_or_else(|_| dest.display().to_string());
    ok(&format!("Linked: {pretty}"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        aur_failure_marker_in, command_exists, git_pull, link_item, login_action,
        marker_still_valid_at, pacman_pkg_installed, patch_pkgbuild_unistd, LoginAction,
        GREETD_CONFIG_BODY, REGREET_CSS, REGREET_TOML,
    };
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

    #[test]
    fn finds_a_command_that_definitely_exists() {
        // `sh` is guaranteed by POSIX and by the Ubuntu CI image.
        assert!(command_exists("sh"));
    }

    #[test]
    fn does_not_find_a_command_that_does_not_exist() {
        assert!(!command_exists("__dotctl_nonexistent_xyz123"));
    }

    #[test]
    fn rejects_shell_metacharacters_instead_of_evaluating_them() {
        // Pre-fix, the format!() form would have run `; true` as a
        // separate statement after `command -v foo` failed, yielding
        // overall exit 0 — making this assert flip true. With $1
        // quoting, sh looks up a binary literally named
        // `foo; true` which can't exist.
        assert!(!command_exists("foo; true"));
        assert!(!command_exists("$(true)"));
        assert!(!command_exists("`true`"));
    }

    #[test]
    fn pacman_pkg_installed_returns_false_for_missing_pkg() {
        // `pacman -Q __dotctl_missing_xyz` exits non-zero on Arch and
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
    fn pacman_pkg_installed_finds_base_filesystem_pkg_on_arch() {
        // The `filesystem` package is part of `base` and is installed
        // on every Arch host, but it ships /usr/bin contents owned by
        // other packages and has no command of the same name — so
        // command_exists("filesystem") always misses. This is exactly
        // the shape that motivated the helper.
        if command_exists("pacman") {
            assert!(pacman_pkg_installed("filesystem"));
        }
    }

    // ── link_item ────────────────────────────────────────────────────────
    //
    // link_item is the deploy primitive that moves user files out of the
    // way before symlinking the repo file in. Bugs here would silently
    // eat a user's existing config, so cover the branches explicitly.

    struct LinkItemFixture {
        _tmp: TempDir,
        home: PathBuf,
        repo: PathBuf,
        backup: PathBuf,
    }

    impl LinkItemFixture {
        fn new() -> Self {
            let tmp = TempDir::new().expect("tempdir");
            let home = tmp.path().to_path_buf();
            let repo = home.join("repo");
            let backup = home.join(".dotfiles-backup/run");
            fs::create_dir_all(&repo).expect("mkdir repo");
            Self {
                _tmp: tmp,
                home,
                repo,
                backup,
            }
        }

        fn write_src(&self, name: &str, body: &str) -> PathBuf {
            let p = self.repo.join(name);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).expect("mkdir src parent");
            }
            fs::write(&p, body).expect("write src");
            p
        }

        fn write_dest(&self, name: &str, body: &str) -> PathBuf {
            let p = self.home.join(name);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).expect("mkdir dest parent");
            }
            fs::write(&p, body).expect("write dest");
            p
        }
    }

    #[test]
    fn link_item_creates_symlink_when_dest_missing() {
        let f = LinkItemFixture::new();
        let src = f.write_src("foo.txt", "src");
        let dest = f.home.join("foo.txt");

        link_item(&src, &dest, &f.backup, &f.home).expect("link_item");

        assert!(dest.is_symlink(), "dest should be a symlink");
        assert_eq!(fs::read_link(&dest).expect("read_link"), src);
        assert!(!f.backup.exists(), "no backup when dest was missing");
    }

    #[test]
    fn link_item_is_noop_when_dest_already_points_to_src() {
        let f = LinkItemFixture::new();
        let src = f.write_src("foo.txt", "src");
        let dest = f.home.join("foo.txt");
        symlink(&src, &dest).expect("seed symlink");

        link_item(&src, &dest, &f.backup, &f.home).expect("link_item");

        assert!(dest.is_symlink());
        assert_eq!(fs::read_link(&dest).expect("read_link"), src);
        assert!(
            !f.backup.exists(),
            "no backup directory should appear on a no-op"
        );
    }

    #[test]
    fn link_item_backs_up_existing_regular_file() {
        let f = LinkItemFixture::new();
        let src = f.write_src("foo.txt", "from-repo");
        let dest = f.write_dest("foo.txt", "user-precious");

        link_item(&src, &dest, &f.backup, &f.home).expect("link_item");

        assert!(dest.is_symlink());
        assert_eq!(fs::read_link(&dest).expect("read_link"), src);

        let backed_up = f.backup.join("foo.txt");
        assert!(backed_up.is_file(), "user file should land in backup");
        assert_eq!(
            fs::read_to_string(&backed_up).expect("read backup"),
            "user-precious",
            "backup must preserve user content byte-for-byte"
        );
    }

    #[test]
    fn link_item_backs_up_existing_directory() {
        let f = LinkItemFixture::new();
        let src = f.repo.join("cfg");
        fs::create_dir_all(&src).expect("mkdir src cfg");
        fs::write(src.join("repo-file"), "repo").expect("write src/cfg/repo-file");

        let dest = f.home.join("cfg");
        fs::create_dir_all(&dest).expect("mkdir dest cfg");
        fs::write(dest.join("user-file"), "user").expect("write dest/cfg/user-file");

        link_item(&src, &dest, &f.backup, &f.home).expect("link_item");

        assert!(dest.is_symlink());
        assert_eq!(fs::read_link(&dest).expect("read_link"), src);

        let backed_up = f.backup.join("cfg");
        assert!(backed_up.is_dir(), "user dir should land in backup");
        assert_eq!(
            fs::read_to_string(backed_up.join("user-file")).expect("read"),
            "user",
            "directory contents must survive the backup move"
        );
    }

    #[test]
    fn link_item_backs_up_wrong_symlink() {
        let f = LinkItemFixture::new();
        let src = f.write_src("foo.txt", "src");

        let other = f.home.join("other/foo.txt");
        fs::create_dir_all(other.parent().expect("parent")).expect("mkdir other");
        fs::write(&other, "other").expect("write other");

        let dest = f.home.join("foo.txt");
        symlink(&other, &dest).expect("seed wrong symlink");

        link_item(&src, &dest, &f.backup, &f.home).expect("link_item");

        assert_eq!(
            fs::read_link(&dest).expect("read_link"),
            src,
            "dest now points at src"
        );

        let backed_up = f.backup.join("foo.txt");
        assert!(
            backed_up.is_symlink(),
            "the old symlink itself is backed up"
        );
        assert_eq!(
            fs::read_link(&backed_up).expect("read backup link"),
            other,
            "the backed-up symlink still points at its original target"
        );
    }

    #[test]
    fn link_item_creates_missing_dest_parent_dirs() {
        let f = LinkItemFixture::new();
        let src = f.write_src("foo.txt", "src");
        let dest = f.home.join("a/b/c/foo.txt");
        assert!(!dest.parent().expect("parent").exists());

        link_item(&src, &dest, &f.backup, &f.home).expect("link_item");

        assert!(dest.is_symlink());
        assert_eq!(fs::read_link(&dest).expect("read_link"), src);
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

    // ── login_action ───────────────────────────────────────────────────────
    //
    // The protective invariant for the login screen: enable sddm.service
    // only when no third-party DM is already wired up, so dotctl never
    // silently steals the login screen from gdm/lightdm/ly. greetd is the
    // fallback greeter we also configure, so switching off it to sddm is OK.

    #[test]
    fn login_action_enables_sddm_when_no_dm_is_set() {
        assert!(matches!(login_action(None), LoginAction::Enable));
    }

    #[test]
    fn login_action_switches_to_sddm_from_our_own_greetd() {
        assert!(matches!(
            login_action(Some("greetd.service".into())),
            LoginAction::Enable
        ));
    }

    #[test]
    fn login_action_is_noop_when_sddm_already_active() {
        assert!(matches!(
            login_action(Some("sddm.service".into())),
            LoginAction::AlreadySddm
        ));
    }

    #[test]
    fn login_action_leaves_another_dm_alone() {
        match login_action(Some("gdm.service".into())) {
            LoginAction::OtherDm(dm) => assert_eq!(dm, "gdm.service"),
            _ => panic!("a third-party DM must be left in place, never overridden"),
        }
    }

    // The greeter is wired greetd → cage → regreet, with the noctalia-themed
    // ReGreet config embedded at build time. Guard the wiring so a stray edit
    // to the command string or an emptied asset fails CI instead of the boot.
    #[test]
    fn greetd_launches_cage_and_regreet() {
        assert!(GREETD_CONFIG_BODY.contains("cage"));
        assert!(GREETD_CONFIG_BODY.contains("regreet"));
        assert!(!REGREET_TOML.trim().is_empty());
        assert!(!REGREET_CSS.trim().is_empty());
    }

    // ── git_pull ───────────────────────────────────────────────────────────
    //
    // `dotctl deploy` symlinks ~/.config/noctalia at the repo, and grogu
    // rewrites the tracked colors.json / Grogu.json in place on every
    // wallpaper change. Without --autostash the ff-only pull would bail on
    // that dirty tree and `dotctl all` would silently stop updating. Pin
    // that the pull fast-forwards anyway and keeps the local repaint.

    /// Run git in `dir` with a deterministic identity (CI configures none)
    /// and assert success.
    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .current_dir(dir)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("spawn git")
            .success();
        assert!(ok, "git {args:?} failed in {}", dir.display());
    }

    #[test]
    fn git_pull_fast_forwards_over_a_grogu_dirtied_tracked_file() {
        if !command_exists("git") {
            return; // git-less host; nothing to exercise
        }
        let tmp = TempDir::new().expect("tempdir");
        let upstream = tmp.path().join("upstream");
        let clone = tmp.path().join("clone");
        fs::create_dir_all(&upstream).expect("mkdir upstream");

        // Seed upstream with the two files dotctl cares about: a generated
        // theme file (colors.json) and an unrelated tracked file (README).
        git(&upstream, &["init", "-q", "-b", "main"]);
        git(&upstream, &["config", "user.email", "t@t"]);
        git(&upstream, &["config", "user.name", "t"]);
        fs::write(upstream.join("colors.json"), "{\"mPrimary\":\"#000\"}\n").expect("seed colors");
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

        // ...while grogu has repainted the tracked colors.json in the clone.
        let repaint = "{\"mPrimary\":\"#61c3cf\"}\n";
        fs::write(clone.join("colors.json"), repaint).expect("repaint colors");

        git_pull(clone.to_str().unwrap()).expect("autostash pull should fast-forward");

        assert_eq!(
            fs::read_to_string(clone.join("README.md")).expect("read readme"),
            "v2\n",
            "the upstream update must land despite the dirty tree"
        );
        assert_eq!(
            fs::read_to_string(clone.join("colors.json")).expect("read colors"),
            repaint,
            "the local grogu repaint must survive the pull (autostash restores it)"
        );
    }
}
