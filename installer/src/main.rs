// Rust port of install.sh -- Arch + BlackArch package bootstrap for the
// niri-dotfiles setup. Section order and side-effects mirror the bash
// script line-for-line; package data lives in packages.rs.

use anyhow::{anyhow, bail, Context, Result};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

mod packages;
use packages::GROUPS;

// ── ANSI colors ───────────────────────────────────────────────────────────
const RED: &str = "\x1b[0;31m";
const GRN: &str = "\x1b[0;32m";
const BLU: &str = "\x1b[0;34m";
const YLW: &str = "\x1b[1;33m";
const CYN: &str = "\x1b[0;36m";
const BLD: &str = "\x1b[1m";
const RST: &str = "\x1b[0m";

fn info<S: AsRef<str>>(msg: S) { println!("{BLU}[*]{RST} {}", msg.as_ref()); }
fn ok<S: AsRef<str>>(msg: S)   { println!("{GRN}[+]{RST} {}", msg.as_ref()); }
fn warn<S: AsRef<str>>(msg: S) { println!("{YLW}[!]{RST} {}", msg.as_ref()); }

const BANNER: &str = r#"
    _   ___      _   _  __         __       ___
   / | / (_)____(_) / |/ /___  ___/ /_____ / (_)___ _
  /  |/ / / ___/ / /    / __ \/ __/ __/ _ `/ / / _ `/
 / /|  / / /  / / / /| / /_/ / /_/ /_/ /_,/ / / \_,_/
/_/ |_/_/_/  /_/ /_/ |_\____/\__/\__/\__,_/_/_/\__,_/
"#;

fn banner() {
    println!("{CYN}{BANNER}{RST}");
    println!("{BLD}Arch Linux + BlackArch -- Niri + Noctalia + Cybersec{RST}");
    println!("{CYN}{}{RST}", "-".repeat(52));
    println!();
}

// ── Shell helpers ─────────────────────────────────────────────────────────

fn command_exists(cmd: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {cmd} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run(prog: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(prog)
        .args(args)
        .status()
        .with_context(|| format!("spawn {prog}"))?;
    if !status.success() {
        bail!("{prog} {} exited with status {status}", args.join(" "));
    }
    Ok(())
}

fn run_in(dir: &Path, prog: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(prog)
        .args(args)
        .current_dir(dir)
        .status()
        .with_context(|| format!("spawn {prog}"))?;
    if !status.success() {
        bail!("{prog} {} exited with status {status}", args.join(" "));
    }
    Ok(())
}

// Best-effort runner: returns bool, silences stdout+stderr. Mirrors bash's
// `cmd &>/dev/null || true` / `cmd 2>/dev/null || warn ...` pattern so the
// terminal stays clean on systems where (e.g.) docker.service or
// power-profiles-daemon isn't installed yet.
fn run_ok(prog: &str, args: &[&str]) -> bool {
    Command::new(prog)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn sudo(args: &[&str]) -> Result<()> { run("sudo", args) }
fn sudo_ok(args: &[&str]) -> bool   { run_ok("sudo", args) }

// Append stdout+stderr to `log`. Never errors on non-zero exit -- callers
// check the returned status (matches the install_pkgs retry semantics).
fn run_logged(log: &Path, prog: &str, args: &[&str]) -> Result<ExitStatus> {
    let f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .with_context(|| format!("open log {}", log.display()))?;
    let f2 = f.try_clone()?;
    let status = Command::new(prog)
        .args(args)
        .stdout(Stdio::from(f))
        .stderr(Stdio::from(f2))
        .status()
        .with_context(|| format!("spawn {prog}"))?;
    Ok(status)
}

// Write a root-owned file via `sudo tee` -- mirrors `printf ... | sudo tee`.
fn sudo_tee(path: &str, content: &str, append: bool) -> Result<()> {
    let mut cmd = Command::new("sudo");
    cmd.arg("tee");
    if append { cmd.arg("-a"); }
    cmd.arg(path).stdin(Stdio::piped()).stdout(Stdio::null());
    let mut child = cmd.spawn().context("spawn sudo tee")?;
    child.stdin.as_mut().unwrap().write_all(content.as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        bail!("sudo tee{} {path} failed: {status}", if append { " -a" } else { "" });
    }
    Ok(())
}

// ── Temp-dir RAII guard (mirrors install.sh's _TEMP_DIRS + EXIT trap) ─────

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Result<Self> {
        let out = Command::new("mktemp").arg("-d").output().context("mktemp -d")?;
        if !out.status.success() { bail!("mktemp -d failed"); }
        Ok(Self(PathBuf::from(String::from_utf8(out.stdout)?.trim())))
    }
    fn path(&self) -> &Path { &self.0 }
}
impl Drop for TempDir {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

// ── Path / env lookup ─────────────────────────────────────────────────────

fn home() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("$HOME is not set"))
}

// Walk up from CWD looking for the niri-dotfiles checkout (has .gitmodules
// and a .git entry). Used for submodule bootstrapping.
fn find_repo_root() -> Option<PathBuf> {
    let mut p = env::current_dir().ok()?;
    loop {
        if p.join(".gitmodules").is_file() && p.join(".git").exists() {
            return Some(p);
        }
        if !p.pop() { return None; }
    }
}

// ── Sections ──────────────────────────────────────────────────────────────

fn bootstrap_submodules(repo: &Path) -> Result<()> {
    let sub = repo.join(".config/opencode/heimdall_opencode");
    let empty = fs::read_dir(&sub).map(|mut it| it.next().is_none()).unwrap_or(true);
    if !empty { return Ok(()); }

    info("Bootstrapping git submodules (heimdall_opencode) ...");
    let status = Command::new("git")
        .args(["-C", &repo.display().to_string(),
               "submodule", "update", "--init", "--recursive", "--depth", "1"])
        .status()
        .context("spawn git submodule update")?;
    if status.success() {
        ok("Submodules ready.");
    } else {
        warn("  submodule init failed; heimdall_opencode agents may be missing");
    }
    Ok(())
}

fn ensure_aur_helper() -> Result<String> {
    if command_exists("yay")  { ok("AUR helper: yay");  return Ok("yay".into());  }
    if command_exists("paru") { ok("AUR helper: paru"); return Ok("paru".into()); }

    info("Installing yay (AUR helper) ...");
    sudo(&["pacman", "-S", "--needed", "--noconfirm", "base-devel", "git"])?;
    let tmp = TempDir::new()?;
    let dst = tmp.path().join("yay-bin");
    run("git", &["clone", "https://aur.archlinux.org/yay-bin.git",
                 &dst.display().to_string()])?;
    run_in(&dst, "makepkg", &["-si", "--noconfirm"])?;
    ok("AUR helper: yay");
    Ok("yay".into())
}

fn add_blackarch_repo() -> Result<()> {
    if run_ok("pacman", &["-Sl", "blackarch"]) {
        ok("BlackArch repo already present.");
        return Ok(());
    }
    info("Adding BlackArch repository ...");
    let tmp = TempDir::new()?;
    let strap = tmp.path().join("strap.sh");
    let strap_s = strap.display().to_string();
    run("curl", &["-sL", "https://blackarch.org/strap.sh", "-o", &strap_s])?;
    run("chmod", &["+x", &strap_s])?;
    sudo(&[&strap_s])?;
    sudo(&["pacman", "-Sy"])?;
    ok("BlackArch repo added.");
    Ok(())
}

fn chaotic_aur_configured() -> bool {
    fs::read_to_string("/etc/pacman.conf")
        .map(|s| s.lines().any(|l| l.starts_with("[chaotic-aur]")))
        .unwrap_or(false)
}

fn add_chaotic_aur() -> Result<()> {
    if chaotic_aur_configured() {
        ok("Chaotic AUR repo already present.");
        return Ok(());
    }
    info("Adding Chaotic AUR repository ...");
    sudo(&["pacman-key", "--recv-key", "3056513887B78AEB",
           "--keyserver", "keyserver.ubuntu.com"])?;
    sudo(&["pacman-key", "--lsign-key", "3056513887B78AEB"])?;
    sudo(&["pacman", "-U", "--noconfirm",
           "https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-keyring.pkg.tar.zst",
           "https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst"])?;
    sudo_tee("/etc/pacman.conf",
             "\n[chaotic-aur]\nInclude = /etc/pacman.d/chaotic-mirrorlist\n",
             true)?;
    sudo(&["pacman", "-Sy"])?;
    ok("Chaotic AUR repo added.");
    Ok(())
}

// label -> "core-niri-noctalia-wayland". Mirrors the bash:
//   tr [:upper:] [:lower:] | tr -cs 'a-z0-9' '-' | sed 's/^-//;s/-$//'
fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = false;
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn install_pkgs(aur: &str, label: &str, pkgs: &[&str]) -> Result<()> {
    let log_path = PathBuf::from(format!("/tmp/install-{}.log", slugify(label)));
    fs::write(&log_path, "")?; // truncate

    info(format!(
        "Installing {BLD}{label}{RST} ({} packages) -- log: {}",
        pkgs.len(),
        log_path.display(),
    ));

    let mut args: Vec<&str> = vec!["-S", "--needed", "--noconfirm"];
    args.extend_from_slice(pkgs);
    let status = run_logged(&log_path, aur, &args)?;

    if !status.success() {
        warn(format!(
            "Batch install had failures (see {}); retrying individually ...",
            log_path.display(),
        ));
        for pkg in pkgs {
            let s = run_logged(&log_path, aur, &["-S", "--needed", "--noconfirm", pkg])?;
            if !s.success() {
                warn(format!("  skip: {pkg} (see {})", log_path.display()));
            }
        }
    }
    ok(format!("{label} done."));
    println!();
    Ok(())
}

fn install_groups(aur: &str) -> Result<()> {
    for g in GROUPS {
        install_pkgs(aur, g.label, g.pkgs)?;
    }
    Ok(())
}

fn set_default_shell() -> Result<()> {
    let current = env::var("SHELL").unwrap_or_default();
    if current.ends_with("/zsh") { return Ok(()); }
    info("Setting zsh as default shell ...");
    let out = Command::new("sh").args(["-c", "command -v zsh"]).output()?;
    if !out.status.success() {
        bail!("zsh not found on $PATH");
    }
    let zsh = String::from_utf8(out.stdout)?.trim().to_string();
    run("chsh", &["-s", &zsh])?;
    ok("Default shell set to zsh (re-login to activate).");
    Ok(())
}

fn enable_services(user: &str) -> Result<()> {
    info("Enabling system services ...");

    // NetworkManager + iwd backend (avoid wpa_supplicant conflict).
    if !Path::new("/etc/NetworkManager/conf.d/wifi-backend.conf").is_file() {
        info("Configuring NetworkManager to use iwd backend ...");
        sudo(&["mkdir", "-p", "/etc/NetworkManager/conf.d"])?;
        sudo_tee(
            "/etc/NetworkManager/conf.d/wifi-backend.conf",
            "[device]\nwifi.backend=iwd\n",
            false,
        )?;
    }
    let _ = sudo_ok(&["systemctl", "disable", "--now", "wpa_supplicant"]);

    for svc in ["NetworkManager", "iwd", "bluetooth", "docker", "power-profiles-daemon"] {
        let _ = sudo_ok(&["systemctl", "enable", "--now", svc]);
    }
    // Display manager: greetd + tuigreet. SDDM stays installed as a disabled
    // fallback (flip back with: systemctl disable greetd && enable sddm).
    let _ = sudo_ok(&["systemctl", "disable", "sddm"]);
    let _ = sudo_ok(&["systemctl", "enable", "greetd"]);

    // User-scope audio stack.
    let _ = run_ok("systemctl", &["--user", "enable", "--now",
                                  "pipewire", "pipewire-pulse", "wireplumber"]);

    let _ = sudo_ok(&["usermod", "-aG", "docker", user]);
    let _ = sudo_ok(&["usermod", "-aG", "wireshark", user]);
    Ok(())
}

fn install_zinit() -> Result<()> {
    let xdg = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or(home()?.join(".local/share"));
    let dst = xdg.join("zinit/zinit.git");
    if dst.exists() { return Ok(()); }

    info("Installing Zinit (zsh plugin manager) ...");
    fs::create_dir_all(dst.parent().unwrap())?;
    run("git", &["clone", "https://github.com/zdharma-continuum/zinit.git",
                 &dst.display().to_string()])?;
    ok("Zinit installed.");
    Ok(())
}

fn decompress_rockyou() -> Result<()> {
    let plain = Path::new("/usr/share/wordlists/rockyou.txt");
    let gz = Path::new("/usr/share/wordlists/rockyou.txt.gz");
    if !plain.exists() && gz.exists() {
        info("Decompressing rockyou.txt ...");
        sudo(&["gzip", "-dk", "/usr/share/wordlists/rockyou.txt.gz"])?;
        ok("rockyou.txt ready.");
    }
    Ok(())
}

fn install_hexstrike() -> Result<()> {
    let dir = home()?.join("tools/hexstrike-ai");
    if !dir.exists() {
        info("Cloning HexStrike AI ...");
        fs::create_dir_all(dir.parent().unwrap())?;
        run("git", &["clone", "https://github.com/0x4m4/hexstrike-ai.git",
                     &dir.display().to_string()])?;
        ok("HexStrike AI cloned.");
    } else {
        info("Updating HexStrike AI ...");
        if !run_ok("git", &["-C", &dir.display().to_string(), "pull", "--ff-only"]) {
            warn("  git pull skipped (local changes?)");
        }
    }

    let venv = dir.join("hexstrike-env");
    if !venv.exists() {
        info("Creating HexStrike Python venv ...");
        run("python3", &["-m", "venv", &venv.display().to_string()])?;
        ok("Venv created.");
    }

    info("Installing HexStrike Python dependencies ...");
    let pip = venv.join("bin/pip").display().to_string();
    run(&pip, &["install", "--quiet", "--upgrade", "pip"])?;
    let req = dir.join("requirements.txt").display().to_string();
    run(&pip, &["install", "--quiet", "-r", &req])?;
    ok("HexStrike dependencies installed.");
    info("  hexstrike-server.service is enabled by deploy.sh (unit ships with it).");
    Ok(())
}

// `cargo install` from the upstream repo so the binary lands at
// ~/.cargo/bin/telia (already on $PATH via .zshrc). Skip if already
// installed; re-run with `cargo install --force ...` to rebuild.
fn install_telia() -> Result<()> {
    if command_exists("telia") {
        ok("telia already installed (use 'cargo install --force ...' to rebuild).");
        return Ok(());
    }
    info("Installing telia (TUI coding agent) ...");
    run("cargo", &[
        "install",
        "--git", "https://github.com/foolish-dev/telia",
        "--branch", "dev",
        "--locked",
        "telia-cli",
    ])?;
    ok("telia installed -> ~/.cargo/bin/telia");
    Ok(())
}

fn install_grogu() -> Result<()> {
    if command_exists("grogu") {
        ok("grogu already installed (use 'cargo install --force ...' to rebuild).");
        return Ok(());
    }
    info("Installing grogu (wallpaper-driven theme propagator) ...");
    run("cargo", &[
        "install",
        "--git", "https://github.com/foolish-dev/grogu",
        "--branch", "main",
        "--locked",
    ])?;
    ok("grogu installed -> ~/.cargo/bin/grogu");
    Ok(())
}

fn ensure_screenshots_dir() -> Result<()> {
    fs::create_dir_all(home()?.join("Pictures/Screenshots"))?;
    Ok(())
}

// ── main ──────────────────────────────────────────────────────────────────

fn main() {
    if let Err(e) = real_main() {
        eprintln!("{RED}[-]{RST} {e:#}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<()> {
    if !Path::new("/etc/arch-release").is_file() {
        bail!("This script is for Arch Linux only.");
    }

    banner();

    if let Some(root) = find_repo_root() {
        bootstrap_submodules(&root)?;
    }

    let aur = ensure_aur_helper()?;
    add_blackarch_repo()?;
    add_chaotic_aur()?;
    install_groups(&aur)?;
    set_default_shell()?;

    let user = env::var("USER")
        .or_else(|_| env::var("LOGNAME"))
        .context("$USER / $LOGNAME unset")?;
    enable_services(&user)?;

    install_zinit()?;
    decompress_rockyou()?;
    install_hexstrike()?;
    install_telia()?;
    install_grogu()?;
    ensure_screenshots_dir()?;

    println!();
    println!("{CYN}{}{RST}", "-".repeat(52));
    ok("Installation complete.");
    println!();
    info("Next steps:");
    info(format!("  1. Run {BLD}./deploy.sh{RST} to symlink configs into place."));
    info(format!("  2. Log out, select {BLD}niri{RST} from your display manager."));
    info(format!("  3. Press {BLD}Super+Return{RST} to open a terminal."));
    info(format!("  4. Run {BLD}nvim{RST} -- plugins install automatically on first launch."));
    println!();
    Ok(())
}
