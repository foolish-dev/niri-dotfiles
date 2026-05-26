use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

#[derive(Parser)]
#[command(
    version,
    about = "Installer/deployer for foolish-dev/dotfiles",
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
    /// Install tools: grogu, HexStrike AI, Neovim, tmux, fastfetch, Noctalia (shell + qs + SDDM theme + auth agent)
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
    let status = Command::new(prog)
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
/// If `yay` is missing we warn and skip rather than fail — the rest
/// of `dotctl install` is still useful, and bootstrapping yay needs
/// `base-devel` + a clone + makepkg which is out of scope here.
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
    run("yay", &args)?;
    ok(&format!("{label} installed"));
    Ok(())
}

/// `ensure_pacman_pkg` for AUR packages: pacman owns the local DB even
/// for AUR-installed packages, so the "already installed" check uses
/// the same [`pacman_pkg_installed`] helper.
fn ensure_aur_pkg(label: &str, pkg: &str) -> Result<()> {
    if pacman_pkg_installed(pkg) {
        ok(&format!("{label} already installed"));
        Ok(())
    } else {
        aur_install(label, &[pkg])
    }
}

fn ensure_repo(repo: &Path) -> Result<()> {
    let repo_str = repo
        .to_str()
        .ok_or_else(|| anyhow!("repo path is not valid utf-8: {}", repo.display()))?;
    if repo.join(".git").exists() {
        info(&format!("Updating {repo_str} ..."));
        if run("git", &["-C", repo_str, "pull", "--ff-only"]).is_err() {
            warn("git pull skipped (local changes?)");
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
                "https://github.com/foolish-dev/dotfiles.git",
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
    // AUR add-ons for the full Noctalia ecosystem (yay required).
    // sddm-theme-noctalia-git: login-screen theme matched to the shell.
    // noctalia-unofficial-auth-agent-git: ships /usr/bin/bb-auth, the
    //   polkit agent + GNOME-keyring prompter. Wired up as a systemd
    //   user unit by `dotctl deploy`.
    ensure_aur_pkg("Noctalia SDDM theme", "sddm-theme-noctalia-git")?;
    ensure_aur_pkg("Noctalia auth agent", "noctalia-unofficial-auth-agent-git")?;

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

    link_item(
        &repo.join(".config/systemd/user/noctalia-auth-agent.service"),
        &h.join(".config/systemd/user/noctalia-auth-agent.service"),
        &backup_dir,
        &h,
    )?;

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

    if let Err(e) = run("systemctl", &["--user", "daemon-reload"]) {
        warn(&format!("systemctl --user daemon-reload failed: {e}"));
    }
    let enabled = Command::new("systemctl")
        .args(["--user", "enable", "--now", "hexstrike-server.service"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if enabled {
        ok("hexstrike-server.service enabled");
    } else {
        warn("hexstrike-server.service not enabled (run `dotctl install` first to clone hexstrike-ai)");
    }

    let na_enabled = Command::new("systemctl")
        .args(["--user", "enable", "--now", "noctalia-auth-agent.service"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if na_enabled {
        ok("noctalia-auth-agent.service enabled");
    } else {
        warn("noctalia-auth-agent.service not enabled (run `dotctl install` first to install noctalia-unofficial-auth-agent-git)");
    }

    ok("Deploy complete");
    Ok(())
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
    use super::{command_exists, link_item, pacman_pkg_installed};
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
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
}
