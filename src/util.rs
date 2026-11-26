use anyhow::{anyhow, Context, Result};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ── Output helpers ────────────────────────────────────────────────────────────

pub(crate) fn info(msg: &str) {
    println!("\x1b[34m[*]\x1b[0m {msg}");
}
pub(crate) fn ok(msg: &str) {
    println!("\x1b[32m[+]\x1b[0m {msg}");
}
pub(crate) fn warn(msg: &str) {
    eprintln!("\x1b[31m[!]\x1b[0m {msg}");
}

// ── Process / FS helpers ──────────────────────────────────────────────────────

pub(crate) fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .expect("HOME unset")
}

pub(crate) fn command_exists(cmd: &str) -> bool {
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

pub(crate) fn run(prog: &str, args: &[&str]) -> Result<()> {
    run_in(".", prog, args)
}

/// Like [`run`] but with an explicit working directory. `makepkg` builds
/// in the directory that holds the PKGBUILD, so it can't just inherit
/// dotctl's cwd the way every other command can.
pub(crate) fn run_in(dir: &str, prog: &str, args: &[&str]) -> Result<()> {
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
pub(crate) fn sudo_write(path: &str, content: &str) -> Result<()> {
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
pub(crate) fn sudo_write_owned(path: &str, content: &str) -> Result<()> {
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

pub(crate) fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".cache"))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
