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
    /// Path to the dotfiles repo (defaults to $HOME/niri-dotfiles)
    #[arg(long, env = "DOTFILES_REPO", global = true)]
    repo: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install tools: grogu, HexStrike AI, Neovim, tmux, fastfetch, Noctalia (shell + qs + SDDM noctalia login, greetd/ReGreet fallback + auth agent)
    Install {
        /// Never install an AUR helper, even when one is available from a
        /// configured repo. AUR-only add-ons are then skipped with a warning.
        #[arg(long)]
        no_aur_helper: bool,
    },
    /// Symlink .config/* + home dotfiles + .local/bin/* + wallpapers into $HOME, set up gitconfig, enable user services
    Deploy,
    /// Clone (or pull) the repo, then install + deploy
    All {
        /// Never install an AUR helper, even when one is available from a
        /// configured repo. AUR-only add-ons are then skipped with a warning.
        #[arg(long)]
        no_aur_helper: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let repo = cli.repo.unwrap_or_else(|| home().join("niri-dotfiles"));
    // Read once, thread everywhere: `install()` takes the distro as a plain
    // parameter rather than re-probing /etc/os-release at each decision point.
    let distro = detect_distro();
    match cli.cmd {
        Cmd::Install { no_aur_helper } => install(distro, no_aur_helper),
        Cmd::Deploy => deploy(&repo),
        Cmd::All { no_aur_helper } => {
            ensure_repo(&repo)?;
            install(distro, no_aur_helper)?;
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

/// [`sudo_write`], but never destroys an existing different file. Identical
/// content is a no-op; differing content is copied to `<path>.dotctl-bak`
/// first.
///
/// Used for `/etc/greetd/config.toml`, which dotctl does not exclusively own:
/// the greetd package ships that exact path as a pacman *backup file* (its
/// stock body runs `agreety`), and on CachyOS the first-party
/// `noctalia-greeter` package is configured through it too.
fn sudo_write_owned(path: &str, content: &str) -> Result<()> {
    match fs::read_to_string(path) {
        Ok(existing) if existing == content => {
            ok(&format!("{path} already current"));
            Ok(())
        }
        Ok(_) => {
            let bak = format!("{path}.dotctl-bak");
            run("sudo", &["cp", "-a", path, &bak])?;
            warn(&format!("Backed up: {path} -> {bak}"));
            sudo_write(path, content)
        }
        Err(_) => sudo_write(path, content),
    }
}

// ── Distro detection ──────────────────────────────────────────────────────────

/// Which pacman-family distribution we're running on. Every distro-conditional
/// decision in dotctl is a `match` on this, so adding the next derivative is a
/// compile error at each site that has to care rather than a silently-missed
/// branch.
///
/// The safety rule for every one of those matches: **only [`Distro::CachyOs`]
/// may diverge**. `Arch`, `ArchDerivative` and `Unknown` share the arm that
/// encodes dotctl's pre-CachyOS behaviour, so a host we can't identify — or one
/// with no readable os-release at all — keeps doing exactly what it did before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Distro {
    /// `ID=cachyos`. Ships the `[cachyos]` repo, which carries yay, paru,
    /// noctalia-shell and noctalia-qs.
    CachyOs,
    /// `ID=arch` — stock Arch Linux.
    Arch,
    /// Not Arch or CachyOS by `ID`, but `ID_LIKE` lists `arch`
    /// (EndeavourOS, Garuda, Artix, Manjaro, …). Treated as Arch.
    ArchDerivative,
    /// No os-release, unreadable, or an `ID` we don't recognise. Treated as
    /// Arch.
    Unknown,
}

impl Distro {
    /// Human-readable name for the banner `install()` prints, so a bug report
    /// says which branch the run took.
    fn label(self) -> &'static str {
        match self {
            Distro::CachyOs => "CachyOS",
            Distro::Arch => "Arch Linux",
            Distro::ArchDerivative => "an Arch derivative (treated as Arch)",
            Distro::Unknown => "an unrecognised distro (treated as Arch)",
        }
    }
}

/// Strip one matching pair of surrounding single or double quotes, if present.
fn unquote(v: &str) -> &str {
    let b = v.as_bytes();
    if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] {
        return &v[1..v.len() - 1];
    }
    v
}

/// Value of `key` in an os-release `contents`, unquoted.
///
/// os-release is shell-like: `KEY=value`, `KEY="value"`, `KEY='value'`, `#`
/// comments, blank lines, and — on a file that has travelled through Windows —
/// CRLF endings, which the leading `trim()` absorbs since `\r` is whitespace.
/// Later assignments win, matching a shell sourcing the file. Matching is on
/// the whole key, so `ID_LIKE=arch` alone never answers a query for `ID`.
fn os_release_value(contents: &str, key: &str) -> Option<String> {
    let mut found = None;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() == key {
            found = Some(unquote(v.trim()).to_string());
        }
    }
    found
}

/// Classify an os-release file's *contents*. Pure, so every shape it must
/// survive is testable from a fixture string with no filesystem involved —
/// which is what keeps these tests green in CI (Ubuntu) and on stock Arch.
///
/// Only an exact `ID=cachyos` yields [`Distro::CachyOs`]. A hypothetical
/// derivative-of-CachyOS (`ID_LIKE="cachyos arch"`) deliberately falls through
/// to [`Distro::ArchDerivative`]: guessing wrong towards CachyOS skips the
/// chaotic-aur setup on a box that may need it, whereas guessing wrong towards
/// Arch is just today's behaviour.
fn parse_distro(os_release: &str) -> Distro {
    match os_release_value(os_release, "ID")
        .unwrap_or_default()
        .as_str()
    {
        "cachyos" => Distro::CachyOs,
        "arch" => Distro::Arch,
        _ => {
            let like = os_release_value(os_release, "ID_LIKE").unwrap_or_default();
            if like.split_whitespace().any(|w| w == "arch") {
                Distro::ArchDerivative
            } else {
                Distro::Unknown
            }
        }
    }
}

/// [`parse_distro`] over the real file. `/etc/os-release` is the
/// admin-overridable copy and wins; `/usr/lib/os-release` is the vendor
/// default and is the fallback — systemd's own lookup order. Neither readable
/// ⇒ [`Distro::Unknown`], which every match arm treats as Arch.
fn detect_distro() -> Distro {
    fs::read_to_string("/etc/os-release")
        .or_else(|_| fs::read_to_string("/usr/lib/os-release"))
        .map(|c| parse_distro(&c))
        .unwrap_or(Distro::Unknown)
}

/// Whether `install()` should wire up the Chaotic-AUR repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChaoticAur {
    /// Add it. On stock Arch it is dotctl's only source for an AUR helper —
    /// neither yay nor paru is in any Arch official repo.
    Add,
    /// Skip it. `[cachyos]` already carries yay and paru, and chaotic-aur
    /// carries neither noctalia-shell nor noctalia-qs (it never did).
    SkipRedundant,
}

fn chaotic_aur_policy(distro: Distro) -> ChaoticAur {
    match distro {
        Distro::CachyOs => ChaoticAur::SkipRedundant,
        Distro::Arch | Distro::ArchDerivative | Distro::Unknown => ChaoticAur::Add,
    }
}

/// The fix-it hint printed when no AUR helper is available and none can be
/// installed from a configured repository. Distro-specific because the fix
/// genuinely differs: on CachyOS a helper is a plain `pacman -S` away; on
/// stock Arch it is a git clone + makepkg. Telling an Arch user to run
/// `sudo pacman -S yay` sends them to `error: target not found`.
fn aur_helper_hint(distro: Distro) -> &'static str {
    match distro {
        Distro::CachyOs => "run `sudo pacman -S yay` — the [cachyos] repo carries both yay and paru",
        Distro::Arch | Distro::ArchDerivative | Distro::Unknown => {
            "install one by hand (`git clone https://aur.archlinux.org/yay.git && cd yay && makepkg -si`), \
             or rerun `dotctl install` now that the chaotic-aur repo is configured"
        }
    }
}

/// Noctalia's v4 quickshell package. Exists ONLY in CachyOS's `[cachyos]` repo
/// — not Arch `extra`, not chaotic-aur, and not the AUR under this name.
const NOCTALIA_SHELL_PKG: &str = "noctalia-shell";

/// What to do about the Noctalia shell, given whether pacman can resolve it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoctaliaPlan {
    /// A configured repository carries it — install it.
    Install,
    /// It doesn't resolve; carry the distro-appropriate way to fix that.
    Unavailable(&'static str),
}

/// Deliberately never substitutes `extra/noctalia` (the upstream v5 rename):
/// v5 is a different shell with a different config schema, and this repo's
/// tracked ~/.config/noctalia plus the `qs -c noctalia-shell` spawns in
/// .config/niri/config.kdl target the v4 quickshell shell. Silently swapping
/// it would break the deployed configs on *both* distros.
fn noctalia_plan(distro: Distro, in_repos: bool) -> NoctaliaPlan {
    if in_repos {
        return NoctaliaPlan::Install;
    }
    match distro {
        Distro::CachyOs => NoctaliaPlan::Unavailable(
            "the [cachyos] repo looks to be missing from /etc/pacman.conf — restore it with \
             the official cachyos-repo.sh, then rerun `dotctl install`",
        ),
        Distro::Arch | Distro::ArchDerivative | Distro::Unknown => NoctaliaPlan::Unavailable(
            "it is packaged only by CachyOS — add the [cachyos] repo \
             (https://mirror.cachyos.org/cachyos-repo.tar.xz) and rerun `dotctl install`. \
             `extra/noctalia` is the upstream v5 rename, NOT a drop-in for the v4 shell \
             this repo's ~/.config/noctalia targets",
        ),
    }
}

/// True if `contents` (an /etc/pacman.conf) declares a `[name]` section.
///
/// Pure so the parse's two quirks are pinned by tests. The `trim()` is
/// load-bearing: CachyOS's shipped pacman.conf writes its section headers with
/// a trailing space (`[cachyos-znver4] `). And a commented-out `#[blackarch]`
/// must NOT count as present, or dotctl would skip a setup step the box needs.
fn pacman_conf_has_repo(contents: &str, name: &str) -> bool {
    let header = format!("[{name}]");
    contents.lines().any(|l| l.trim() == header)
}

fn has_pacman_repo(name: &str) -> bool {
    fs::read_to_string("/etc/pacman.conf")
        .map(|c| pacman_conf_has_repo(&c, name))
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
    // Stage strap.sh in dotctl's own cache dir, not a predictable path in
    // world-writable /tmp. strap.sh is executed as root, so a symlink/TOCTOU
    // swap between download and exec in shared /tmp would be a root-exec
    // primitive (CWE-377/CWE-379); ~/.cache is owned by and writable only by
    // the user. Remove it afterwards rather than leaving it around.
    let dir = cache_dir().join("dotctl");
    fs::create_dir_all(&dir)?;
    let strap = dir.join("blackarch-strap.sh");
    let strap_str = strap.to_str().context("strap.sh path is not valid utf-8")?;
    run(
        "curl",
        &["-fsSL", "-o", strap_str, "https://blackarch.org/strap.sh"],
    )?;
    run("sudo", &["chmod", "+x", strap_str])?;
    // strap.sh prompts for confirmation; pipe `yes` through so the install is
    // non-interactive. Pass the path as a positional arg ("$1") rather than
    // interpolating it into the script — same injection-safe shape as
    // `command_exists`, so a path with shell metacharacters can't break out.
    let res = run("sh", &["-c", r#"yes | sudo "$1""#, "sh", strap_str]);
    let _ = fs::remove_file(&strap);
    res?;
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

fn aur_helper() -> Option<&'static str> {
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
fn ensure_aur_helper(distro: Distro) -> Option<&'static str> {
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
/// (An earlier version of this comment claimed the chaotic-aur setup covered
/// noctalia-shell + noctalia-qs, and that it ran earlier in `install()`. Both
/// halves were false — chaotic-aur carries neither package, neither name
/// exists in the AUR, and `setup_chaotic_aur` ran *after* these calls. See
/// [`noctalia_plan`].)
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
fn ensure_aur_pkg(helper: Option<&str>, label: &str, pkg: &str) -> Result<()> {
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
/// upstream has fixed. Returns `None` when there was nothing to change, so
/// the caller can tell "already correct" from "rewritten".
fn patch_hexstrike_bind(src: &str) -> Option<String> {
    const NEEDLE: &str = r#"app.run(host="0.0.0.0""#;
    if !src.contains(NEEDLE) {
        return None;
    }
    Some(src.replace(NEEDLE, "app.run(host=API_HOST"))
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

/// greetd's whole config. We do *not* exclusively own this file: the greetd
/// package ships this exact path as a pacman backup file, whose stock body
/// runs `agreety --cmd /bin/sh` — so a fresh box always has a real config here
/// before dotctl ever runs (see [`greetd_is_replaceable`]). cage hosts
/// ReGreet, which lists the niri session it discovers under
/// /usr/share/wayland-sessions (niri.desktop → niri-session). The `greeter`
/// system user is created by the greetd package.
const GREETD_CONF: &str = "/etc/greetd/config.toml";
const GREETD_CONFIG_BODY: &str = "[terminal]\nvt = 1\n\n[default_session]\ncommand = \"dbus-run-session cage -s -mlast -d -- regreet\"\nuser = \"greeter\"\n";

/// greetd session commands dotctl is entitled to replace: `regreet` is the
/// greeter we configure ourselves, and `agreety` is the stock command in the
/// `config.toml` the greetd package itself ships (verified: `pacman -Ql
/// greetd` lists /etc/greetd/config.toml as a Backup File whose body is
/// `command = "agreety --cmd /bin/sh"`).
///
/// Anything else — CachyOS's `noctalia-greeter`, `tuigreet`, `gtkgreet` — is a
/// greeter somebody chose on purpose, and dotctl must neither rewrite its
/// config nor switch greetd.service off underneath it.
const GREETD_REPLACEABLE_GREETERS: [&str; 2] = ["regreet", "agreety"];

/// The `command = "…"` value from a greetd config body, if present.
fn greetd_session_command(body: &str) -> Option<&str> {
    body.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .find_map(|l| l.strip_prefix("command")?.trim_start().strip_prefix('='))
        .map(|v| v.trim().trim_matches('"').trim_matches('\''))
}

/// Whether the greetd config at [`GREETD_CONF`] is one dotctl may take over.
/// Pure, so the "never steal a greeter somebody configured" rule is testable
/// without root. `None` (no file) counts as replaceable.
///
/// Deliberately NOT "is this config byte-identical to ours?" and NOT "does it
/// mention regreet?": both answer *false* on a fresh box, where greetd's own
/// package has just written the stock agreety config — which would stop dotctl
/// writing the ReGreet fallback on every clean install of both distros.
fn greetd_is_replaceable(body: Option<&str>) -> bool {
    let Some(body) = body else {
        return true;
    };
    let Some(cmd) = greetd_session_command(body) else {
        return true;
    };
    cmd.split_whitespace()
        .map(|w| w.rsplit('/').next().unwrap_or(w))
        .any(|w| GREETD_REPLACEABLE_GREETERS.contains(&w))
}

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
/// ours to replace only when `greetd_replaceable` — i.e. its config runs
/// regreet (ours) or the stock agreety (see [`greetd_is_replaceable`]); a
/// greetd hosting somebody else's greeter is treated like gdm/lightdm/ly and
/// left alone.
enum LoginAction {
    AlreadySddm,
    OtherDm(String),
    Enable,
}

fn login_action(current_dm: Option<String>, greetd_replaceable: bool) -> LoginAction {
    match current_dm {
        Some(dm) if dm == "sddm.service" => LoginAction::AlreadySddm,
        Some(dm) if dm == "greetd.service" && greetd_replaceable => LoginAction::Enable,
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
    // Read before we would overwrite it: greetd is a shared entry point, and
    // the answer gates both the config writes and whether greetd.service is
    // ours to switch off.
    let greetd_replaceable = greetd_is_replaceable(fs::read_to_string(GREETD_CONF).ok().as_deref());

    // Keep greetd + ReGreet configured as a disabled fallback, so a manual
    // `systemctl enable greetd` still lands on the themed graphical greeter.
    if command_exists("greetd") {
        if greetd_replaceable {
            info("Writing greetd + ReGreet fallback config ...");
            // config.toml is a pacman backup file shipped by greetd itself, so
            // back up whatever is there before replacing it. regreet.toml and
            // regreet.css are paths dotctl solely owns.
            sudo_write_owned(GREETD_CONF, GREETD_CONFIG_BODY)?;
            sudo_write(REGREET_CONF, REGREET_TOML)?;
            sudo_write(REGREET_CSS_PATH, REGREET_CSS)?;
            ok("greetd + ReGreet fallback config written");
        } else {
            warn(&format!(
                "{GREETD_CONF} configures another greeter (on CachyOS, `noctalia-greeter` \
                 owns this file) — leaving it and greetd.service alone"
            ));
        }
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

    match login_action(current_display_manager(), greetd_replaceable) {
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
/// grogu rewrites the tracked `colorschemes/Grogu/Grogu.json` in place on
/// every wallpaper change (live theme repaint). A plain
/// `git pull --ff-only` then bails on the dirty tree, so re-running
/// `dotctl all` to update would silently skip the pull once the theme had
/// ever been repainted. Autostash shelves the repaint, fast-forwards, then
/// restores it — updates keep flowing and the user keeps their colors.
/// (`Grogu.json` can't just be gitignored away like the nvim/tmux grogu
/// fragments: settings.json pins `predefinedScheme=Grogu`, so the committed
/// scheme is the required first-boot default.)
fn git_pull(repo_str: &str) -> Result<()> {
    run("git", &["-C", repo_str, "pull", "--ff-only", "--autostash"])?;
    // `--autostash` re-applies the shelved repaint after the fast-forward — but
    // `git pull` exits 0 even when that apply CONFLICTS. It prints a notice,
    // leaves conflict markers in the working tree, and keeps the stash. Since
    // `deploy()` symlinks ~/.config/noctalia straight at this repo, a
    // conflicted Grogu.json would be handed to noctalia as its live config
    // while `dotctl all` reported success. Refuse rather than deploy that.
    if git_has_conflicts(repo_str) {
        return Err(anyhow!(
            "`git pull --autostash` fast-forwarded {repo_str}, but re-applying your local changes \
             conflicted: the working tree has conflict markers and the autostash is still in \
             `git stash list`. Resolve them and `git stash drop`, or run \
             `git -C {repo_str} reset --hard && git -C {repo_str} stash pop`. \
             Refusing to deploy a conflicted config."
        ));
    }
    Ok(())
}

/// True if `repo_str` has any path in an unmerged (conflicted) index state.
/// `git ls-files --unmerged` prints a line per conflicted stage and nothing at
/// all for a clean index, so emptiness is the signal. A git that can't run at
/// all answers false — the caller is already reporting that failure.
fn git_has_conflicts(repo_str: &str) -> bool {
    Command::new("git")
        .args(["-C", repo_str, "ls-files", "--unmerged"])
        .stderr(Stdio::null())
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

fn ensure_repo(repo: &Path) -> Result<()> {
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

// ── install ───────────────────────────────────────────────────────────────────

/// Whether a Python venv at `venv` is usable. The directory merely existing
/// isn't enough — a run interrupted mid-creation (Ctrl-C, disk full, a failed
/// ensurepip) leaves it present but without `bin/pip`, the binary install's
/// dependency steps actually invoke. That binary is the real readiness signal.
fn venv_ready(venv: &Path) -> bool {
    venv.join("bin/pip").exists()
}

fn install(distro: Distro, no_aur_helper: bool) -> Result<()> {
    if !command_exists("cargo") {
        return Err(anyhow!(
            "cargo not found — install rust first (https://rustup.rs)"
        ));
    }
    info(&format!("Distro: {}", distro.label()));

    // Prerequisites assumed by later steps. Idempotent — `ensure_pacman`
    // is a no-op when the binary is already on PATH. Without these, a
    // fresh-box install would fail later with cryptic errors:
    //   git    → ensure_repo, cargo install --git for grogu
    //   curl   → setup_blackarch fetches strap.sh
    //   python → HexStrike AI venv (the Arch-family package is `python`, the
    //            binary is `python3`)
    ensure_pacman("git", "git", &["git"])?;
    ensure_pacman("curl", "curl", &["curl"])?;
    ensure_pacman("Python 3", "python3", &["python"])?;

    // Niri base desktop — compositor, terminal, launcher, clipboard persistence.
    // All four are in Arch `extra` (and rebuilt in cachyos-extra-znver4 on
    // CachyOS). wl-clip-persist keeps clipboard contents alive after the
    // source window closes; it was routed through the AUR helper here for a
    // long time, but it has been a plain repo package on both distros all
    // along (extra 0.5.0-2, ships /usr/bin/wl-clip-persist, and is in no AUR
    // at all) — so on a helper-less box, which is every fresh CachyOS install,
    // it silently never installed.
    ensure_pacman("niri", "niri", &["niri"])?;
    ensure_pacman("fuzzel", "fuzzel", &["fuzzel"])?;
    ensure_pacman("kitty", "kitty", &["kitty"])?;
    ensure_pacman("wl-clip-persist", "wl-clip-persist", &["wl-clip-persist"])?;

    // Resolve (or bootstrap) an AUR helper before the first AUR package. On
    // CachyOS this installs one straight from [cachyos]; on stock Arch it can
    // only succeed once setup_chaotic_aur further down has run, so we ask
    // again there rather than reorder the existing pacman calls.
    let mut helper = if no_aur_helper {
        info("--no-aur-helper: leaving AUR helper availability as-is");
        aur_helper()
    } else {
        ensure_aur_helper(distro)
    };

    // ROG Flow Z13 GZ302EA hardware extras (see `gz302ea-pack bringup`). Detected
    // by package name, not binary: XRT installs to /opt/xilinx (not on PATH).
    //   NPU userspace — the in-tree amdxdna driver ships with the kernel; this is
    //     the XRT + FastFlowLM userspace. Model loads need unlimited memlock.
    //   Tablet auto-rotation — iio-sensor-proxy (D-Bus activated) + the iio-niri
    //     bridge (AUR) that feeds orientation to niri.
    //   On-screen keyboard — wvkbd (AUR); the Mod+O keybind is already in niri.
    // All three pacman packages here are in Arch `extra` with cachyos-extra-znver4
    // rebuilds — correct on both distros, no distro branch wanted.
    ensure_pacman_pkg("XRT (NPU)", "xrt", &["xrt", "xrt-plugin-amdxdna"])?;
    ensure_pacman_pkg("FastFlowLM", "fastflowlm", &["fastflowlm"])?;
    ensure_pacman_pkg(
        "iio-sensor-proxy",
        "iio-sensor-proxy",
        &["iio-sensor-proxy"],
    )?;
    ensure_aur_pkg(helper, "iio-niri", "iio-niri")?;
    ensure_aur_pkg(helper, "wvkbd", "wvkbd")?;
    sudo_write(
        "/etc/security/limits.d/99-amdxdna.conf",
        "*  soft  memlock  unlimited\n*  hard  memlock  unlimited\n",
    )?;

    // Chaotic AUR. On stock Arch this is dotctl's only source for an AUR
    // helper — neither yay nor paru is in any Arch official repo. It is NOT a
    // source for noctalia-shell/noctalia-qs; the comment that used to claim so
    // was wrong (chaotic-aur carries neither, and neither name exists in the
    // AUR). Skipped on CachyOS, where [cachyos] already carries yay and paru
    // and where adding a lower-priority third-party repo only widens the
    // versioned-dependency fallthrough surface for nothing.
    match chaotic_aur_policy(distro) {
        ChaoticAur::Add => setup_chaotic_aur()?,
        ChaoticAur::SkipRedundant => {
            ok("Chaotic AUR not needed on CachyOS — [cachyos] already provides yay/paru")
        }
    }

    // Second chance for the helper: on stock Arch the repo that can supply one
    // only came into existence a few lines ago.
    if helper.is_none() && !no_aur_helper {
        helper = ensure_aur_helper(distro);
    }

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
    // Noctalia: the v4 quickshell shell plus its quickshell fork. Both
    // packages exist ONLY in CachyOS's [cachyos] repo — not Arch `extra`, not
    // chaotic-aur, not the AUR — so `pacman -Si` is the honest gate. On
    // CachyOS it resolves and nothing changes; elsewhere we warn with a fix
    // instead of taking down the rest of install() (and, under `dotctl all`,
    // the whole deploy) with a hard `?`.
    //
    // Do NOT substitute `extra/noctalia` (the v5 rename): different shell,
    // different config schema, and the tracked ~/.config/noctalia plus the
    // `qs -c noctalia-shell` spawns in .config/niri/config.kdl target v4. Do
    // NOT add `quickshell` either — noctalia-qs both provides *and* conflicts
    // with it, so naming it is a hard conflict on CachyOS.
    if pacman_pkg_installed(NOCTALIA_SHELL_PKG) {
        ok("Noctalia already installed");
    } else {
        match noctalia_plan(distro, repo_has_pkg(NOCTALIA_SHELL_PKG)) {
            NoctaliaPlan::Install => ensure_pacman_pkg(
                "Noctalia",
                NOCTALIA_SHELL_PKG,
                &[NOCTALIA_SHELL_PKG, "noctalia-qs"],
            )?,
            NoctaliaPlan::Unavailable(hint) => warn(&format!(
                "Noctalia ({NOCTALIA_SHELL_PKG}) is in no configured repository — {hint}"
            )),
        }
    }
    // AUR add-ons for the full Noctalia ecosystem.
    // sddm-theme-noctalia-git (AUR): login-screen theme matched to the shell;
    //   depends on sddm, so this also pulls in the display manager itself.
    //   setup_noctalia_login then selects the theme + enables sddm.service.
    // noctalia-unofficial-auth-agent-git: ships /usr/libexec/bb-auth (polkit
    //   agent + GNOME-keyring prompter) and its own bb-auth.service user unit,
    //   which `dotctl deploy` enables. Built from a locally patched PKGBUILD —
    //   see ensure_noctalia_auth_agent for the GCC 16 fix.
    // Genuinely AUR-only on both distros — do NOT swap in CachyOS's
    //   `noctalia-greeter`, which is a greetd greeter with a bundled wlroots
    //   compositor, not an SDDM theme.
    ensure_aur_pkg(helper, "Noctalia SDDM theme", "sddm-theme-noctalia-git")?;
    // The noctalia SDDM theme above is the active login screen; greetd +
    // ReGreet (graphical, hosted by cage) is configured as a disabled
    // fallback. All in Arch `extra` (rebuilt in cachyos-extra-znver4).
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
            Some(patched) => {
                fs::write(&server_py, &patched).with_context(|| {
                    format!("rewriting the bind address in {}", server_py.display())
                })?;
                if patch_hexstrike_bind(&patched).is_some() {
                    return Err(anyhow!(
                        "failed to pin {} to a loopback bind — refusing to continue, \
                         as the API would be reachable from the network",
                        server_py.display()
                    ));
                }
                ok("HexStrike bind pinned to $HEXSTRIKE_HOST (default 127.0.0.1)");
            }
            None => ok("HexStrike bind already loopback-only"),
        },
        Err(e) => warn(&format!(
            "could not read {} to check its bind address: {e}",
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

// ── deploy ────────────────────────────────────────────────────────────────────

/// Untracked ~/.gitconfig stub. git follows symlinks when writing, so pointing
/// ~/.gitconfig straight at the tracked file would let `git config --global`
/// mutate the repo; the stub [include]s the tracked config + machine-local
/// identity instead, keeping all global writes out of the repo.
const GITCONFIG_STUB: &str = "\
# ~/.gitconfig -- per-machine stub generated by dotctl. NOT tracked.
# Tracked config: ~/.config/git/dotfiles.config (symlink into the repo).
# Identity + `git config --global` writes land here / in ~/.gitconfig.local.
[include]
    path = ~/.config/git/dotfiles.config
[include]
    path = ~/.gitconfig.local
";

/// Deploy the tracked `.gitconfig` to a neutral XDG path with an untracked
/// ~/.gitconfig include-stub, and seed ~/.gitconfig.local identity once.
/// Idempotent: never clobbers an existing stub or identity file (honors
/// `GIT_USER_NAME`/`GIT_USER_EMAIL`, else placeholders so a fresh user never
/// commits under the repo author's name).
fn setup_gitconfig(repo: &Path, h: &Path, backup_dir: &Path) -> Result<()> {
    let tracked = repo.join(".gitconfig");
    if !tracked.exists() {
        return Ok(());
    }
    fs::create_dir_all(h.join(".config/git"))?;
    link_item(
        &tracked,
        &h.join(".config/git/dotfiles.config"),
        backup_dir,
        h,
    )?;

    // Replace a legacy ~/.gitconfig that symlinked straight at the tracked file.
    let gitconfig = h.join(".gitconfig");
    if gitconfig.is_symlink() && gitconfig.canonicalize().ok() == tracked.canonicalize().ok() {
        fs::remove_file(&gitconfig)?;
    }
    if !gitconfig.exists() {
        fs::write(&gitconfig, GITCONFIG_STUB)?;
        ok("Wrote ~/.gitconfig stub");
    }

    let local = h.join(".gitconfig.local");
    if !local.exists() {
        let name = std::env::var("GIT_USER_NAME").unwrap_or_else(|_| "Your Name".into());
        let email = std::env::var("GIT_USER_EMAIL").unwrap_or_else(|_| "you@example.com".into());
        let body = format!(
            "# ~/.gitconfig.local -- per-machine identity + local overrides.\n\
             # Included by ~/.gitconfig. Not tracked in the dotfiles repo.\n\
             [user]\n    name = {name}\n    email = {email}\n"
        );
        fs::write(&local, body)?;
        ok("Wrote ~/.gitconfig.local identity");
    }
    Ok(())
}

/// Canonical marker that the Niri base desktop is present. `dotctl install`
/// installs it (niri/fuzzel/kitty/wl-clip-persist); `dotctl deploy` alone does
/// not, so it warns when this marker binary is missing.
const BASE_DESKTOP_MARKER: &str = "niri";
const BASE_DESKTOP_HINT: &str = "niri not found — `dotctl deploy` lays down configs but does not \
    install the base desktop (compositor / terminal / launcher / clipboard). Run `dotctl install` \
    (or `dotctl all`) first; otherwise the niri/fuzzel/kitty/etc. configs deployed here target \
    software that isn't present.";

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

    // `dotctl deploy` lays down dotfiles; `dotctl install` is what installs the
    // base desktop (compositor/terminal/launcher/clipboard). `niri` absent ⇒
    // deploy onto a box that hasn't been `install`ed: deploy the configs anyway
    // (best-effort), but warn so the user isn't left with configs for software
    // that isn't installed.
    if !command_exists(BASE_DESKTOP_MARKER) {
        warn(BASE_DESKTOP_HINT);
    }

    // .config trees (and starship.toml, a bare file — link_item handles both).
    for d in [
        "teleia",
        "nvim",
        "noctalia",
        "fastfetch",
        "tmux",
        "fuzzel",
        "gtk-3.0",
        "gtk-4.0",
        "kitty",
        "lazygit",
        "neofetch",
        "niri",
        "opencode",
        "qt5ct",
        "qt6ct",
        "wal",
        "starship.toml",
    ] {
        let src = repo.join(".config").join(d);
        if src.exists() {
            let dest = h.join(".config").join(d);
            link_item(&src, &dest, &backup_dir, &h)?;
        }
    }

    // Home-level dotfiles.
    for f in [".zshrc", ".editorconfig", ".gitignore_global"] {
        let src = repo.join(f);
        if src.exists() {
            link_item(&src, &h.join(f), &backup_dir, &h)?;
        }
    }
    setup_gitconfig(repo, &h, &backup_dir)?;

    // systemd/user must be a real directory: an earlier deploy may have made it
    // a whole-dir symlink into another repo. Drop that symlink (its target dir
    // is untouched), then symlink each tracked unit individually — so enabling
    // units writes .wants/ into ~/.config, never back into the repo.
    let sysd_dest = h.join(".config/systemd/user");
    if sysd_dest.is_symlink() {
        fs::remove_file(&sysd_dest)?;
        info("Converted ~/.config/systemd/user from symlink to a real directory");
    }
    fs::create_dir_all(&sysd_dest)?;
    let sysd_src = repo.join(".config/systemd/user");
    if sysd_src.is_dir() {
        for entry in fs::read_dir(&sysd_src)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let dst = sysd_dest.join(entry.file_name());
                link_item(&entry.path(), &dst, &backup_dir, &h)?;
            }
        }
    }

    // The noctalia auth agent now uses its packaged `bb-auth.service` unit
    // (enabled below), so drop the obsolete custom unit symlink an earlier
    // deploy may have left behind — otherwise it lingers as a dangling link.
    let stale_unit = h.join(".config/systemd/user/noctalia-auth-agent.service");
    if stale_unit.is_symlink() {
        let _ = fs::remove_file(&stale_unit);
    }

    // `telia` was renamed to `teleia`; drop the obsolete config symlink an
    // older deploy left behind so it doesn't dangle at ~/.config/telia.
    let stale_telia = h.join(".config/telia");
    if stale_telia.is_symlink() {
        let _ = fs::remove_file(&stale_telia);
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

    // Wallpapers: copy the curated set into ~/Pictures/Wallpapers, the
    // directory noctalia's wallpaper picker watches (setWallpaperOnAllMonitors
    // is on, so they apply to every output). Copies, NOT symlinks: noctalia's
    // SDDM-greeter background-sync `cp`s the current wallpaper into the theme,
    // and a symlink here would carry through — leaving that copy pointed back
    // into the tracked repo, which then gets clobbered on the next wallpaper
    // change. copy_item backs up any differing real file first and is a no-op
    // once deployed.
    let wall_src = repo.join("wallpapers");
    let wall_dest = h.join("Pictures/Wallpapers");
    if wall_src.is_dir() {
        fs::create_dir_all(&wall_dest)?;
        for entry in fs::read_dir(&wall_src)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                let dst = wall_dest.join(entry.file_name());
                copy_item(&entry.path(), &dst, &backup_dir, &h)?;
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

/// Copy `src` to `dest` as a real file, idempotently. Unlike [`link_item`],
/// this must NOT leave a symlink into the repo: noctalia's SDDM-greeter
/// background-sync `cp`s the current wallpaper into the theme's assets, and a
/// symlink here would carry through, leaving that copy pointed back at the
/// tracked repo file — which then gets clobbered on the next wallpaper change.
/// A real copy is inert.
///
/// No-op when an identical file is already present. A leftover symlink from an
/// older symlink-based deploy is dropped (its target — our repo — is left
/// alone) and replaced with a copy. A *different* real file of the same name
/// is backed up first, mirroring [`link_item`].
fn copy_item(src: &Path, dest: &Path, backup_dir: &Path, home: &Path) -> Result<()> {
    if dest.is_symlink() {
        fs::remove_file(dest)
            .with_context(|| format!("remove stale symlink {}", dest.display()))?;
    } else if dest.is_file() {
        if fs::read(src)? == fs::read(dest)? {
            return Ok(());
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
    fs::copy(src, dest).with_context(|| format!("copy {} -> {}", src.display(), dest.display()))?;
    let pretty = dest
        .strip_prefix(home)
        .map(|p| format!("~/{}", p.display()))
        .unwrap_or_else(|_| dest.display().to_string());
    ok(&format!("Copied: {pretty}"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        aur_failure_marker_in, aur_helper_hint, chaotic_aur_policy, command_exists, copy_item,
        git_has_conflicts, git_pull, greetd_is_replaceable, greetd_session_command, link_item,
        login_action, marker_still_valid_at, noctalia_plan, os_release_value, pacman_conf_has_repo,
        pacman_pkg_installed, parse_distro, patch_hexstrike_bind, patch_pkgbuild_unistd,
        preferred_aur_helper, venv_ready, ChaoticAur, Distro, LoginAction, NoctaliaPlan,
        AUR_HELPERS, BASE_DESKTOP_HINT, BASE_DESKTOP_MARKER, GITCONFIG_STUB, GREETD_CONFIG_BODY,
        REGREET_CSS, REGREET_TOML,
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
    fn git_pull_refuses_when_the_autostash_reapply_conflicts() {
        // `git pull --ff-only --autostash` exits 0 even when re-applying the
        // stash conflicts — it just prints a notice and leaves conflict
        // markers behind. deploy() symlinks ~/.config/noctalia at this repo,
        // so without this guard `dotctl all` would hand noctalia a JSON file
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
        fs::write(upstream.join("Grogu.json"), "{\"mPrimary\":\"#000\"}\n").expect("seed");
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

        // Upstream and the local repaint touch THE SAME line — the one case
        // autostash cannot resolve.
        fs::write(upstream.join("Grogu.json"), "{\"mPrimary\":\"#fff\"}\n").expect("upstream edit");
        git(&upstream, &["commit", "-qam", "upstream repaint"]);
        fs::write(clone.join("Grogu.json"), "{\"mPrimary\":\"#61c3cf\"}\n").expect("local repaint");

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
    fn copy_item_writes_real_file_when_dest_missing() {
        let f = LinkItemFixture::new();
        let src = f.write_src("wall.png", "image-bytes");
        let dest = f.home.join("Pictures/Wallpapers/wall.png");

        copy_item(&src, &dest, &f.backup, &f.home).expect("copy_item");

        assert!(
            dest.is_file() && !dest.is_symlink(),
            "dest must be a real file, not a symlink"
        );
        assert_eq!(fs::read_to_string(&dest).expect("read dest"), "image-bytes");
        assert!(!f.backup.exists(), "no backup when dest was missing");
    }

    #[test]
    fn copy_item_is_noop_when_identical_file_present() {
        let f = LinkItemFixture::new();
        let src = f.write_src("wall.png", "same");
        let dest = f.write_dest("Pictures/Wallpapers/wall.png", "same");

        copy_item(&src, &dest, &f.backup, &f.home).expect("copy_item");

        assert!(dest.is_file() && !dest.is_symlink());
        assert!(
            !f.backup.exists(),
            "an identical file must not trigger a backup"
        );
    }

    #[test]
    fn copy_item_replaces_stale_symlink_without_touching_repo() {
        // Migration case: an older deploy symlinked wallpapers into the repo.
        // copy_item must drop that symlink and write a real copy, leaving the
        // repo file untouched — otherwise a greeter `cp` through the symlink
        // would clobber the tracked wallpaper.
        let f = LinkItemFixture::new();
        let src = f.write_src("wall.png", "repo-image");
        let dest = f.home.join("Pictures/Wallpapers/wall.png");
        fs::create_dir_all(dest.parent().unwrap()).expect("mkdir dest parent");
        symlink(&src, &dest).expect("seed symlink");

        copy_item(&src, &dest, &f.backup, &f.home).expect("copy_item");

        assert!(!dest.is_symlink(), "stale symlink must be replaced");
        assert!(dest.is_file());
        assert_eq!(fs::read_to_string(&dest).expect("read dest"), "repo-image");
        assert_eq!(
            fs::read_to_string(&src).expect("read src"),
            "repo-image",
            "the tracked repo file must be left untouched"
        );
        assert!(
            !f.backup.exists(),
            "dropping our own symlink is not a backup-worthy event"
        );
    }

    #[test]
    fn copy_item_backs_up_differing_real_file() {
        let f = LinkItemFixture::new();
        let src = f.write_src("wall.png", "from-repo");
        let dest = f.write_dest("Pictures/Wallpapers/wall.png", "user-edited");

        copy_item(&src, &dest, &f.backup, &f.home).expect("copy_item");

        assert_eq!(fs::read_to_string(&dest).expect("read dest"), "from-repo");
        let backed_up = f.backup.join("Pictures/Wallpapers/wall.png");
        assert!(
            backed_up.is_file(),
            "a differing user file must be backed up"
        );
        assert_eq!(
            fs::read_to_string(&backed_up).expect("read backup"),
            "user-edited"
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
    fn patch_hexstrike_bind_rewrites_the_all_interfaces_bind() {
        // The upstream line, verbatim.
        let src = "    app.run(host=\"0.0.0.0\", port=API_PORT, debug=DEBUG_MODE)\n";
        let out = patch_hexstrike_bind(src).expect("the 0.0.0.0 bind must be rewritten");
        assert!(out.contains("app.run(host=API_HOST, port=API_PORT"));
        assert!(
            !out.contains("0.0.0.0"),
            "no all-interfaces bind may survive: {out}"
        );
    }

    #[test]
    fn patch_hexstrike_bind_is_idempotent_and_leaves_a_fixed_file_alone() {
        // None means "nothing to do" — the caller reports "already loopback"
        // rather than rewriting the file on every single run.
        assert_eq!(
            patch_hexstrike_bind("app.run(host=API_HOST, port=API_PORT)\n"),
            None
        );
        assert_eq!(
            patch_hexstrike_bind("app.run(host=\"127.0.0.1\", port=API_PORT)\n"),
            None
        );
        let once = patch_hexstrike_bind("app.run(host=\"0.0.0.0\", port=1)\n").unwrap();
        assert_eq!(patch_hexstrike_bind(&once), None, "second pass is a no-op");
    }

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
        assert!(matches!(login_action(None, true), LoginAction::Enable));
    }

    #[test]
    fn login_action_switches_to_sddm_from_our_own_greetd() {
        assert!(matches!(
            login_action(Some("greetd.service".into()), true),
            LoginAction::Enable
        ));
    }

    #[test]
    fn login_action_is_noop_when_sddm_already_active() {
        assert!(matches!(
            login_action(Some("sddm.service".into()), true),
            LoginAction::AlreadySddm
        ));
    }

    #[test]
    fn login_action_leaves_another_dm_alone() {
        match login_action(Some("gdm.service".into()), true) {
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

    // The base-desktop preflight is a user-visible contract: if it fires, it
    // must name the actionable marker and how to get the desktop, else it's
    // just noise. Pin both so a reworded hint can't silently drop them.
    #[test]
    fn base_desktop_hint_names_marker_and_install_cmd() {
        assert!(BASE_DESKTOP_HINT.contains(BASE_DESKTOP_MARKER));
        assert!(BASE_DESKTOP_HINT.contains("dotctl install"));
    }

    // The ~/.gitconfig stub must [include] both the tracked config and the
    // machine-local identity, or `git config --global` writes leak into the repo.
    #[test]
    fn gitconfig_stub_includes_tracked_config_and_local_identity() {
        assert!(GITCONFIG_STUB.contains("path = ~/.config/git/dotfiles.config"));
        assert!(GITCONFIG_STUB.contains("path = ~/.gitconfig.local"));
        assert_eq!(GITCONFIG_STUB.matches("[include]").count(), 2);
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

    // ── Distro detection ─────────────────────────────────────────────────
    //
    // Every one of these is a pure string→enum assertion, so they run
    // identically on CI (Ubuntu, no pacman), on stock Arch, and on CachyOS.

    /// The live /etc/os-release from the CachyOS box this port was developed
    /// on, verbatim: quoted and unquoted values mixed, and a value containing
    /// semicolons.
    const CACHYOS_OS_RELEASE: &str = r#"NAME="CachyOS Linux"
PRETTY_NAME="CachyOS"
ID=cachyos
ID_LIKE=arch
BUILD_ID=rolling
ANSI_COLOR="38;2;23;147;209"
HOME_URL="https://cachyos.org/"
DOCUMENTATION_URL="https://wiki.cachyos.org/"
SUPPORT_URL="https://discuss.cachyos.org/"
BUG_REPORT_URL="https://github.com/cachyos"
PRIVACY_POLICY_URL="https://terms.archlinux.org/docs/privacy-policy/"
LOGO=cachyos
"#;

    #[test]
    fn parse_distro_identifies_cachyos_from_the_live_os_release() {
        assert_eq!(parse_distro(CACHYOS_OS_RELEASE), Distro::CachyOs);
    }

    #[test]
    fn parse_distro_identifies_stock_arch() {
        // No ID_LIKE at all — pins that the ID_LIKE lookup is never *required*
        // to succeed for a host to be classified.
        assert_eq!(
            parse_distro("NAME=\"Arch Linux\"\nID=arch\nBUILD_ID=rolling\n"),
            Distro::Arch
        );
    }

    #[test]
    fn parse_distro_strips_double_and_single_quotes() {
        // An unstripped quote silently disables the whole CachyOS path:
        // invisible on the author's box, visible on someone else's.
        assert_eq!(parse_distro("ID=\"cachyos\"\n"), Distro::CachyOs);
        assert_eq!(parse_distro("ID='arch'\n"), Distro::Arch);
        assert_eq!(
            parse_distro("ID=\"\"\nID_LIKE=\"arch\"\n"),
            Distro::ArchDerivative
        );
        assert_eq!(
            os_release_value("ID=\"\"", "ID").as_deref(),
            Some(""),
            "an empty quoted value unquotes to empty, not to a stray quote"
        );
    }

    #[test]
    fn parse_distro_survives_crlf_line_endings() {
        // `str::lines()` strips \n but leaves \r, so without the trim the
        // comparison would be against "cachyos\r". Pins the trim as
        // load-bearing.
        assert_eq!(
            parse_distro("ID=cachyos\r\nID_LIKE=arch\r\n"),
            Distro::CachyOs
        );
        assert_eq!(parse_distro("ID=\"cachyos\"\r\n"), Distro::CachyOs);
    }

    #[test]
    fn parse_distro_reads_arch_out_of_a_multi_valued_id_like() {
        // Two rules at once: ID_LIKE is tokenised rather than substring
        // matched (so `archlinux` alone must not count), and `cachyos`
        // appearing in ID_LIKE does not make a host CachyOS.
        assert_eq!(
            parse_distro("ID=garuda\nID_LIKE=\"cachyos arch\"\n"),
            Distro::ArchDerivative
        );
        assert_eq!(
            parse_distro("ID=endeavouros\nID_LIKE=arch\n"),
            Distro::ArchDerivative
        );
        assert_eq!(
            parse_distro("ID=manjaro\nID_LIKE=\"archlinux arch\"\n"),
            Distro::ArchDerivative
        );
    }

    #[test]
    fn parse_distro_treats_unknown_missing_and_empty_as_unknown() {
        // The fallback the entire no-regression argument rests on.
        assert_eq!(parse_distro("ID=fedora\nID_LIKE=rhel\n"), Distro::Unknown);
        assert_eq!(parse_distro(""), Distro::Unknown);
        assert_eq!(parse_distro("# nothing\n\n   \n"), Distro::Unknown);
        assert_eq!(parse_distro("NAME=\"Some Linux\"\n"), Distro::Unknown);
    }

    #[test]
    fn os_release_value_matches_whole_keys_and_takes_the_last_assignment() {
        // A `starts_with("ID=")`-shaped parse would answer `ID` with
        // ID_LIKE's value and mislabel every derivative.
        assert_eq!(os_release_value("ID_LIKE=arch\n", "ID"), None);
        assert_eq!(
            os_release_value("ID=arch\nID=cachyos\n", "ID").as_deref(),
            Some("cachyos")
        );
        assert_eq!(os_release_value("#ID=fedora\n", "ID"), None);
        assert_eq!(os_release_value("no-equals-here\n", "ID"), None);
    }

    #[test]
    fn only_cachyos_diverges_from_todays_behaviour() {
        // The port's central safety invariant, as an executable assertion:
        // adding a fifth Distro variant forces a reviewer through here, and
        // any refactor that flips an Arch-family arm fails CI rather than
        // silently disabling the yay bootstrap on stock Arch.
        for d in [Distro::Arch, Distro::ArchDerivative, Distro::Unknown] {
            assert_eq!(
                chaotic_aur_policy(d),
                ChaoticAur::Add,
                "{d:?} must keep dotctl's pre-CachyOS behaviour"
            );
        }
        assert_eq!(
            chaotic_aur_policy(Distro::CachyOs),
            ChaoticAur::SkipRedundant
        );
    }

    #[test]
    fn aur_helper_hint_offers_pacman_only_on_cachyos() {
        // A hint that sends an Arch user to `error: target not found` is
        // worse than no hint at all.
        assert!(aur_helper_hint(Distro::CachyOs).contains("pacman -S"));
        for d in [Distro::Arch, Distro::ArchDerivative, Distro::Unknown] {
            let h = aur_helper_hint(d);
            assert!(h.contains("makepkg"), "{d:?} hint should point at makepkg");
            assert!(
                !h.contains("pacman -S"),
                "{d:?} has no official-repo yay/paru to pacman -S"
            );
        }
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

    #[test]
    fn noctalia_plan_installs_whenever_the_package_resolves() {
        // Availability-driven, not distro-driven: CachyOS is bit-for-bit
        // unchanged, and an Arch box that added [cachyos] works identically.
        for d in [
            Distro::CachyOs,
            Distro::Arch,
            Distro::ArchDerivative,
            Distro::Unknown,
        ] {
            assert_eq!(noctalia_plan(d, true), NoctaliaPlan::Install, "{d:?}");
        }
    }

    #[test]
    fn noctalia_plan_hints_at_the_cachyos_repo_and_warns_off_the_v5_substitution() {
        // Keeps the anti-substitution warning in the binary, guarding the
        // single most damaging "fix" available at that call site.
        for d in [Distro::Arch, Distro::ArchDerivative, Distro::Unknown] {
            match noctalia_plan(d, false) {
                NoctaliaPlan::Unavailable(h) => {
                    assert!(h.contains("[cachyos]"), "{d:?} should name the repo");
                    assert!(h.contains("v5"), "{d:?} should warn off extra/noctalia");
                }
                other => panic!("{d:?} expected Unavailable, got {other:?}"),
            }
        }
        match noctalia_plan(Distro::CachyOs, false) {
            NoctaliaPlan::Unavailable(h) => assert!(h.contains("/etc/pacman.conf")),
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    // ── pacman.conf parsing ──────────────────────────────────────────────

    #[test]
    fn pacman_conf_has_repo_tolerates_cachyos_trailing_space_headers() {
        // CachyOS really does ship `[cachyos-znver4] ` with a trailing space.
        // Nothing pinned that before, so a "simplify the trim away" cleanup
        // would make dotctl re-add repos and re-run strap.sh as root on every
        // single run.
        assert!(pacman_conf_has_repo(
            "[cachyos-znver4] \nInclude = /etc/pacman.d/cachyos-v4-mirrorlist\n",
            "cachyos-znver4"
        ));
        assert!(pacman_conf_has_repo("  [extra]\n", "extra"));
        assert!(!pacman_conf_has_repo("[core]\n[extra]\n", "blackarch"));
    }

    #[test]
    fn pacman_conf_has_repo_ignores_commented_out_sections() {
        // The inverse hazard: a `contains()`-shaped parse would see a
        // commented-out section and no-op both repo setups forever.
        assert!(!pacman_conf_has_repo(
            "#[blackarch]\n# Include = /etc/pacman.d/blackarch-mirrorlist\n",
            "blackarch"
        ));
        assert!(!pacman_conf_has_repo("  # [blackarch]\n", "blackarch"));
    }

    // ── greetd ownership ─────────────────────────────────────────────────

    /// The verbatim `/etc/greetd/config.toml` shipped by the greetd package
    /// (`pacman -Ql greetd` lists it as a Backup File). This is the state of a
    /// fresh box the moment `install()` installs greetd.
    const STOCK_GREETD_CONFIG: &str = r#"[terminal]
# The VT to run the greeter on. Can be "next", "current" or a number
# designating the VT.
vt = 1

# The default session, also known as the greeter.
[default_session]

# `agreety` is the bundled agetty/login-lookalike. You can replace `/bin/sh`
# with whatever you want started, such as `sway`.
command = "agreety --cmd /bin/sh"

# The user to run the command as. The privileges this user must have depends
# on the greeter. A graphical greeter may for example require the user to be
# in the `video` group.
user = "greeter"
"#;

    #[test]
    fn greetd_session_command_extracts_the_default_session_command() {
        assert_eq!(
            greetd_session_command(GREETD_CONFIG_BODY),
            Some("dbus-run-session cage -s -mlast -d -- regreet")
        );
        assert_eq!(
            greetd_session_command(STOCK_GREETD_CONFIG),
            Some("agreety --cmd /bin/sh")
        );
        assert_eq!(greetd_session_command("[terminal]\nvt = 1\n"), None);
        assert_eq!(
            greetd_session_command("commandx = \"y\"\n"),
            None,
            "`commandx` must not be matched as `command`"
        );
    }

    #[test]
    fn greetd_is_replaceable_accepts_the_stock_agreety_config() {
        // The most important test in the port. This is the fresh-install
        // state on both distros; a rule that answers false here stops the
        // ReGreet fallback ever being written on a clean box.
        assert!(greetd_is_replaceable(Some(STOCK_GREETD_CONFIG)));
    }

    #[test]
    fn greetd_is_replaceable_accepts_our_own_config_and_a_missing_file() {
        // Self-consistency: the predicate must accept the exact bytes the
        // constant beside it writes, or dotctl classifies its own output as
        // foreign on the second run. Also catches an edit to
        // GREETD_CONFIG_BODY that drops `regreet`.
        assert!(greetd_is_replaceable(Some(GREETD_CONFIG_BODY)));
        assert!(greetd_is_replaceable(None));
    }

    #[test]
    fn greetd_is_replaceable_rejects_a_third_party_greeter() {
        // CachyOS publishes `noctalia-greeter` in [cachyos] — a greeter that
        // lives in exactly this file. Without this, the port's most
        // user-hostile failure mode is live.
        assert!(!greetd_is_replaceable(Some(
            "[default_session]\ncommand = \"noctalia-greeter\"\n"
        )));
        assert!(!greetd_is_replaceable(Some(
            "[default_session]\ncommand = \"tuigreet --cmd niri-session\"\n"
        )));
        assert!(
            greetd_is_replaceable(Some("[default_session]\ncommand = \"/usr/bin/regreet\"\n")),
            "an absolute path to our own greeter still matches on basename"
        );
        // Whole-token, not substring: a `contains()`-shaped rewrite would keep
        // every other assertion here green while making dotctl clobber the
        // config of a greeter that merely has one of our names inside its own.
        assert!(
            !greetd_is_replaceable(Some(
                "[default_session]\ncommand = \"cage -s -- regreet-ng\"\n"
            )),
            "a greeter whose name merely *contains* regreet is not ours"
        );
        assert!(
            !greetd_is_replaceable(Some(
                "[default_session]\ncommand = \"my-agreety-wrapper\"\n"
            )),
            "…and the same for agreety"
        );
    }

    #[test]
    fn login_action_leaves_a_third_party_greetd_alone() {
        // Counterpart to login_action_switches_to_sddm_from_our_own_greetd;
        // together they state the whole rule.
        match login_action(Some("greetd.service".into()), false) {
            LoginAction::OtherDm(dm) => assert_eq!(dm, "greetd.service"),
            _ => panic!("a greetd hosting someone else's greeter must be left alone"),
        }
    }

    #[test]
    fn login_action_still_enables_sddm_when_a_foreign_greetd_is_not_the_dm() {
        // Guards against over-correcting: a stray third-party greetd config on
        // disk must not block enabling sddm when greetd is not the enabled DM.
        assert!(matches!(login_action(None, false), LoginAction::Enable));
    }
}
