use std::path::Path;
use std::process::{Command, Stdio};

/// Run git in `dir` with a deterministic identity (CI configures none)
/// and assert success.
pub(crate) fn git(dir: &Path, args: &[&str]) {
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
