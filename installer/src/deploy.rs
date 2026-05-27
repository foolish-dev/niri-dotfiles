// Rust port of deploy.sh -- symlinks the tracked dotfiles into $HOME and
// sets up the bits that can't be expressed as a plain symlink (git stub,
// SDDM unit templating, SDDM theme upgrade-proofing).
//
// Section order mirrors deploy.sh. Shell-out helpers are duplicated from
// main.rs intentionally -- two ~80-line copies is cheaper than a lib.rs
// refactor for now.

use anyhow::{anyhow, bail, Context, Result};
use std::env;
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::os::unix::fs as unix_fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ── ANSI colors ───────────────────────────────────────────────────────────
const RED: &str = "\x1b[0;31m";
const GRN: &str = "\x1b[0;32m";
const BLU: &str = "\x1b[0;34m";
const YLW: &str = "\x1b[1;33m";
const RST: &str = "\x1b[0m";

fn info<S: AsRef<str>>(msg: S) { println!("{BLU}[*]{RST} {}", msg.as_ref()); }
fn ok<S: AsRef<str>>(msg: S)   { println!("{GRN}[+]{RST} {}", msg.as_ref()); }
fn warn<S: AsRef<str>>(msg: S) { println!("{YLW}[!]{RST} {}", msg.as_ref()); }

// ── Shell helpers (mirror main.rs) ────────────────────────────────────────

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

// Best-effort runner: returns bool, silences stdout+stderr. Matches the
// bash `cmd 2>/dev/null || true` / `cmd 2>/dev/null || warn ...` pattern
// so failures on optional services (hexstrike not yet installed,
// noctalia SDDM theme not present) stay quiet until our own warn fires.
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

// Pipe `content` into `sudo tee [-a] path` so we can write root-owned files
// without escalating the whole binary.
fn sudo_tee(path: &str, content: &str, append: bool) -> Result<()> {
    let mut cmd = Command::new("sudo");
    cmd.arg("tee");
    if append { cmd.arg("-a"); }
    cmd.arg(path).stdin(Stdio::piped()).stdout(Stdio::null());
    let mut child = cmd.spawn().context("spawn sudo tee")?;
    child.stdin.as_mut().unwrap().write_all(content.as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        bail!("sudo tee{} {path} failed: {status}",
              if append { " -a" } else { "" });
    }
    Ok(())
}

// ── Path / env lookup ─────────────────────────────────────────────────────

fn home() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("$HOME is not set"))
}

// Resolve the dotfiles repo root. deploy.sh uses $(dirname "$0"); for the
// Rust binary that path isn't meaningful, so we look in three places:
//   1. $DOTFILES_DIR if set (matches the bootstrap.sh env var)
//   2. walk up from CWD looking for `.gitmodules` + `.git`
//   3. walk up from current_exe (lets `cargo run` from installer/ work)
fn find_dotfiles_root() -> Result<PathBuf> {
    if let Some(p) = env::var_os("DOTFILES_DIR").map(PathBuf::from) {
        if p.join(".gitmodules").is_file() {
            return Ok(p);
        }
    }
    if let Ok(cwd) = env::current_dir() {
        if let Some(p) = walk_up_for_marker(&cwd) {
            return Ok(p);
        }
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(p) = walk_up_for_marker(&exe) {
            return Ok(p);
        }
    }
    bail!("could not locate dotfiles repo (.gitmodules + .git not found above CWD or binary)");
}

fn walk_up_for_marker(start: &Path) -> Option<PathBuf> {
    let mut p = start.to_path_buf();
    if p.is_file() { p.pop(); }
    loop {
        if p.join(".gitmodules").is_file() && p.join(".git").exists() {
            return Some(p);
        }
        if !p.pop() { return None; }
    }
}

// `date +%Y%m%d-%H%M%S` -- shell out so we get local time without pulling
// chrono. Matches deploy.sh exactly.
fn timestamp() -> String {
    Command::new("date")
        .arg("+%Y%m%d-%H%M%S")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

// ── link_item (the workhorse) ─────────────────────────────────────────────

// Backs up dest (if it exists and isn't already our symlink), then
// symlinks src -> dest. backup_dir is created lazily on first backup.
fn link_item(src: &Path, dest: &Path, backup_dir: &Path, home: &Path) -> Result<()> {
    let exists = dest.exists() || dest.is_symlink();

    if exists {
        // Already our symlink? (compare canonical paths)
        let src_real = fs::canonicalize(src).ok();
        let dest_real = fs::canonicalize(dest).ok();
        if src_real.is_some() && src_real == dest_real {
            return Ok(());
        }

        // Back up.
        fs::create_dir_all(backup_dir)
            .with_context(|| format!("create backup dir {}", backup_dir.display()))?;
        let rel = dest.strip_prefix(home).unwrap_or(dest);
        let backup_dest = backup_dir.join(rel);
        if let Some(parent) = backup_dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(dest, &backup_dest)
            .with_context(|| format!("mv {} -> {}", dest.display(), backup_dest.display()))?;
        warn(format!("Backed up: ~/{} -> {}", rel.display(), backup_dest.display()));
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    // In case a dangling symlink survived the canonicalize/backup branch.
    let _ = fs::remove_file(dest);
    unix_fs::symlink(src, dest)
        .with_context(|| format!("symlink {} -> {}", src.display(), dest.display()))?;
    ok(format!("Linked: {} -> {}", src.display(), dest.display()));
    Ok(())
}

// ── Section: .config directories ──────────────────────────────────────────

const CONFIG_DIRS: &[&str] = &[
    "niri", "noctalia", "kitty", "fuzzel", "nvim", "tmux", "lazygit",
    "systemd/user", "opencode", "wal", "fastfetch", "neofetch",
    "gtk-3.0", "gtk-4.0", "qt5ct", "qt6ct",
];

fn link_config_dirs(dotfiles: &Path, home: &Path, backup: &Path) -> Result<()> {
    for dir in CONFIG_DIRS {
        let src = dotfiles.join(".config").join(dir);
        let dst = home.join(".config").join(dir);
        link_item(&src, &dst, backup, home)?;
    }
    Ok(())
}

// ── Section: home-level dotfiles ──────────────────────────────────────────

fn link_home_dotfiles(dotfiles: &Path, home: &Path, backup: &Path) -> Result<()> {
    for name in [".zshrc", ".gitignore_global", ".editorconfig"] {
        link_item(&dotfiles.join(name), &home.join(name), backup, home)?;
    }
    Ok(())
}

// ── Section: git config stub + identity ───────────────────────────────────

const GITCONFIG_STUB: &str = "\
# ~/.gitconfig -- per-machine stub generated by deploy.sh. NOT tracked.
#
# Tracked dotfiles config lives at ~/.config/git/dotfiles.config (symlink
# into $DOTFILES). Identity + signing live in ~/.gitconfig.local. Any
# `git config --global ...` mutations land HERE so the dotfiles repo stays
# clean of machine-specific drift.
[include]
    path = ~/.config/git/dotfiles.config
[include]
    path = ~/.gitconfig.local
";

fn deploy_gitconfig(dotfiles: &Path, home: &Path, backup: &Path) -> Result<()> {
    fs::create_dir_all(home.join(".config/git"))?;
    link_item(
        &dotfiles.join(".gitconfig"),
        &home.join(".config/git/dotfiles.config"),
        backup,
        home,
    )?;

    // Replace any legacy ~/.gitconfig -> $DOTFILES/.gitconfig symlink with
    // the include stub. (Symlinking the tracked file directly lets
    // `git config --global` mutate the repo file.)
    let gc = home.join(".gitconfig");
    let tracked = dotfiles.join(".gitconfig");
    if gc.is_symlink() {
        let resolved = fs::canonicalize(&gc).ok();
        let tracked_resolved = fs::canonicalize(&tracked).ok();
        if resolved.is_some() && resolved == tracked_resolved {
            fs::remove_file(&gc)?;
            info("Removed legacy symlink: ~/.gitconfig -> $DOTFILES/.gitconfig");
        }
    }
    // `exists()` follows symlinks; pair with is_symlink() to catch dangling.
    if !gc.exists() && !gc.is_symlink() {
        fs::write(&gc, GITCONFIG_STUB)?;
        ok("Wrote ~/.gitconfig stub");
    }
    Ok(())
}

fn setup_git_identity(home: &Path) -> Result<()> {
    let path = home.join(".gitconfig.local");
    if path.is_file() {
        ok("Existing ~/.gitconfig.local kept as-is.");
        return Ok(());
    }

    info("Setting up git identity (~/.gitconfig.local) ...");
    let mut name = env::var("GIT_USER_NAME").unwrap_or_default();
    let mut email = env::var("GIT_USER_EMAIL").unwrap_or_default();

    let stdin = io::stdin();
    let interactive = stdin.is_terminal();
    if name.is_empty() && interactive {
        name = prompt("  git user.name  : ")?;
    }
    if email.is_empty() && interactive {
        email = prompt("  git user.email : ")?;
    }
    if name.is_empty()  { name  = "Your Name".into(); }
    if email.is_empty() { email = "you@example.com".into(); }

    let content = format!(
        "# ~/.gitconfig.local -- per-machine identity + local overrides
# Included by ~/.gitconfig. Not tracked in the dotfiles repo.
[user]
    name = {name}
    email = {email}
    # Uncomment and point at your signing key to sign commits:
    # signingkey = ~/.ssh/id_ed25519.pub

# Uncomment to sign every commit (requires signingkey + allowed_signers):
# [commit]
#     gpgsign = true
"
    );
    fs::write(&path, content)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    ok(format!("Wrote {} (name={name}, email={email})", path.display()));
    Ok(())
}

fn prompt(label: &str) -> Result<String> {
    print!("{label}");
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().lock().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

// ── Section: single-file .config/* drops ──────────────────────────────────

fn link_single_config_files(dotfiles: &Path, home: &Path, backup: &Path) -> Result<()> {
    let starship = dotfiles.join(".config/starship.toml");
    if starship.is_file() {
        link_item(
            &starship,
            &home.join(".config/starship.toml"),
            backup,
            home,
        )?;
    }
    Ok(())
}

// ── Section: scripts, desktop entries, wallpapers ─────────────────────────

// Symlink every regular file in `src_dir` into `dst_dir`, optionally
// filtering by extension.
fn link_dir_contents(
    src_dir: &Path,
    dst_dir: &Path,
    ext_filter: Option<&str>,
    backup: &Path,
    home: &Path,
) -> Result<()> {
    if !src_dir.is_dir() { return Ok(()); }
    fs::create_dir_all(dst_dir)?;
    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() { continue; }
        let path = entry.path();
        if let Some(ext) = ext_filter {
            if path.extension().and_then(|e| e.to_str()) != Some(ext) {
                continue;
            }
        }
        let name = entry.file_name();
        link_item(&path, &dst_dir.join(&name), backup, home)?;
    }
    Ok(())
}

// ── Section: user systemd services ────────────────────────────────────────

fn enable_user_services() {
    info("Enabling user systemd services ...");
    let _ = run_ok("systemctl", &["--user", "daemon-reload"]);

    if !run_ok("systemctl", &["--user", "enable", "--now",
                              "hexstrike-server.service"]) {
        warn("  hexstrike-server.service failed to start (run install.sh first)");
    }
}

// ── Section: SDDM noctalia background-sync units ──────────────────────────

fn deploy_sddm_sync_units(dotfiles: &Path, home: &Path) -> Result<()> {
    if env::var("DOTFILES_SKIP_SDDM_SYNC").as_deref() == Ok("1") {
        info("Skipping SDDM noctalia background-sync (DOTFILES_SKIP_SDDM_SYNC=1)");
        return Ok(());
    }
    let unit_dir = dotfiles.join("etc/systemd/system");
    let path_unit = unit_dir.join("sddm-noctalia-sync.path");
    if !path_unit.is_file() { return Ok(()); }

    info("Deploying SDDM noctalia background-sync units ...");
    let home_s = home.display().to_string();
    for name in ["sddm-noctalia-sync.path", "sddm-noctalia-sync.service"] {
        let tracked = unit_dir.join(name);
        let content = fs::read_to_string(&tracked)
            .with_context(|| format!("read {}", tracked.display()))?;
        let rendered = content.replace("__USER_HOME__", &home_s);
        sudo_tee(&format!("/etc/systemd/system/{name}"), &rendered, false)?;
    }
    ok(format!("  Installed sddm-noctalia-sync.{{path,service}} (HOME={home_s})"));

    let _ = sudo_ok(&["systemctl", "daemon-reload"]);

    if Path::new("/usr/share/sddm/themes/noctalia").is_dir() {
        if sudo_ok(&["systemctl", "enable", "--now", "sddm-noctalia-sync.path"]) {
            ok("  sddm-noctalia-sync.path enabled");
        } else {
            warn("  sddm-noctalia-sync.path failed to enable");
        }
    } else {
        warn("  /usr/share/sddm/themes/noctalia not found -- skipping enable");
    }
    Ok(())
}

// ── Section: /etc/greetd/ ─────────────────────────────────────────────────

fn deploy_greetd_conf(dotfiles: &Path) -> Result<()> {
    let src_dir = dotfiles.join("etc/greetd");
    if !src_dir.is_dir() { return Ok(()); }

    info("Deploying greetd config to /etc/greetd/ ...");
    sudo(&["mkdir", "-p", "/etc/greetd"])?;
    for entry in fs::read_dir(&src_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() { continue; }
        let name = entry.file_name();
        let name_s = name.to_string_lossy();
        sudo(&["cp", &entry.path().display().to_string(),
               &format!("/etc/greetd/{name_s}")])?;
        ok(format!("  Copied {name_s}"));
    }
    Ok(())
}

// ── Section: /etc/sddm.conf.d/ ────────────────────────────────────────────

fn deploy_sddm_conf(dotfiles: &Path) -> Result<()> {
    let src_dir = dotfiles.join("etc/sddm.conf.d");
    if !src_dir.is_dir() { return Ok(()); }

    info("Deploying SDDM config to /etc/sddm.conf.d/ ...");
    sudo(&["mkdir", "-p", "/etc/sddm.conf.d"])?;
    for entry in fs::read_dir(&src_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() { continue; }
        let name = entry.file_name();
        let name_s = name.to_string_lossy();
        sudo(&["cp", &entry.path().display().to_string(),
               &format!("/etc/sddm.conf.d/{name_s}")])?;
        ok(format!("  Copied {name_s}"));
    }
    Ok(())
}

// ── Section: SDDM astronaut theme upgrade-proof local copy ────────────────

fn deploy_sddm_astronaut_theme(dotfiles: &Path) -> Result<()> {
    let upstream = Path::new("/usr/share/sddm/themes/sddm-astronaut-theme");
    let local = Path::new("/usr/share/sddm/themes/sddm-astronaut-local");
    let themes_src = dotfiles.join("etc/sddm-themes");

    if !upstream.is_dir() || !themes_src.is_dir() { return Ok(()); }

    info("Deploying local SDDM astronaut theme copy ...");
    if !local.is_dir() {
        sudo(&["cp", "-a", &upstream.display().to_string(),
               &local.display().to_string()])?;
        ok(format!("  Created local theme dir: {}", local.display()));
    }

    for entry in fs::read_dir(&themes_src)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() { continue; }
        if entry.path().extension().and_then(|e| e.to_str()) != Some("conf") {
            continue;
        }
        let name = entry.file_name();
        let name_s = name.to_string_lossy();
        sudo(&["cp", &entry.path().display().to_string(),
               &format!("{}/Themes/{name_s}", local.display())])?;
        ok(format!("  Copied {name_s} -> local Themes/"));
    }

    let samurai = dotfiles.join("wallpapers/samurai.png");
    if samurai.is_file() {
        sudo(&["cp", &samurai.display().to_string(),
               &format!("{}/Backgrounds/tokyonight.png", local.display())])?;
        ok("  Set default SDDM background: samurai.png");
    }

    sudo(&[
        "sed", "-i",
        "s|^ConfigFile=.*|ConfigFile=Themes/cyberpunk.conf|",
        &format!("{}/metadata.desktop", local.display()),
    ])?;
    ok("  Activated: cyberpunk variant (in local copy)");
    Ok(())
}

// ── Section: summary ──────────────────────────────────────────────────────

fn count_files(dir: &Path, ext_filter: Option<&str>) -> usize {
    let Ok(entries) = fs::read_dir(dir) else { return 0; };
    entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter(|e| match ext_filter {
            None => true,
            Some(want) => e.path().extension().and_then(|x| x.to_str()) == Some(want),
        })
        .count()
}

fn print_summary(dotfiles: &Path, backup_used: bool, backup_dir: &Path) {
    let scripts = count_files(&dotfiles.join(".local/bin"), None);
    let desktops = count_files(&dotfiles.join(".local/share/applications"), Some("desktop"));
    let walls = count_files(&dotfiles.join("wallpapers"), None);
    let config_list = CONFIG_DIRS.join(",");

    println!();
    ok("=== Deployment complete ===");
    println!();
    info("Summary:");
    info(format!("  Configs: ~/.config/{{{config_list}}}"));
    info("  Shell:   ~/.zshrc");
    info("  Git:     ~/.gitconfig, ~/.gitignore_global");
    info("  Editor:  ~/.editorconfig");
    info("  Prompt:  ~/.config/starship.toml");
    info(format!("  Scripts: ~/.local/bin/ ({scripts} scripts)"));
    info(format!("  Apps:    ~/.local/share/applications/ ({desktops} desktop entries)"));
    info(format!("  Walls:   ~/Pictures/Wallpapers/ ({walls} wallpapers)"));
    info("  SDDM:    /etc/sddm.conf.d/niri.conf (theme: noctalia, with shell-wallpaper sync)");
    info("");
    if backup_used {
        info(format!("  Backups: {}", backup_dir.display()));
    }
    println!();
    info("Log out, select 'niri' from SDDM, and log back in.");
    info("Open a terminal (Super+Return) and run 'nvim' -- plugins install automatically.");
    println!();
}

// ── main ──────────────────────────────────────────────────────────────────

fn main() {
    if let Err(e) = real_main() {
        eprintln!("{RED}[-]{RST} {e:#}");
        std::process::exit(1);
    }
}

fn real_main() -> Result<()> {
    let home = home()?;
    let dotfiles = find_dotfiles_root()?;
    let backup_dir = home.join(".dotfiles-backup").join(timestamp());

    println!();
    info(format!("=== Deploying dotfiles from {} ===", dotfiles.display()));
    println!();

    link_config_dirs(&dotfiles, &home, &backup_dir)?;
    link_home_dotfiles(&dotfiles, &home, &backup_dir)?;
    deploy_gitconfig(&dotfiles, &home, &backup_dir)?;
    setup_git_identity(&home)?;
    link_single_config_files(&dotfiles, &home, &backup_dir)?;

    // Scripts
    fs::create_dir_all(home.join(".local/bin"))?;
    link_dir_contents(
        &dotfiles.join(".local/bin"),
        &home.join(".local/bin"),
        None,
        &backup_dir,
        &home,
    )?;

    // Desktop entries
    fs::create_dir_all(home.join(".local/share/applications"))?;
    let desktops_src = dotfiles.join(".local/share/applications");
    if desktops_src.is_dir() {
        info("Deploying BlackArch .desktop entries ...");
        link_dir_contents(
            &desktops_src,
            &home.join(".local/share/applications"),
            Some("desktop"),
            &backup_dir,
            &home,
        )?;
    }

    // Wallpapers
    let walls_src = dotfiles.join("wallpapers");
    if walls_src.is_dir() {
        info("Deploying wallpapers to ~/Pictures/Wallpapers ...");
        link_dir_contents(
            &walls_src,
            &home.join("Pictures/Wallpapers"),
            None,
            &backup_dir,
            &home,
        )?;
    }

    enable_user_services();
    deploy_greetd_conf(&dotfiles)?;
    deploy_sddm_sync_units(&dotfiles, &home)?;
    deploy_sddm_conf(&dotfiles)?;
    deploy_sddm_astronaut_theme(&dotfiles)?;

    print_summary(&dotfiles, backup_dir.exists(), &backup_dir);
    Ok(())
}
