use crate::deploy::{deploy, ensure_wallpapers};
use crate::distro::detect_distro;
use crate::install::install;
use crate::update::{binary_is_stale, ensure_repo, rebuild_and_reexec, warn_if_stale, REBUILT_ENV};
use crate::util::{home, warn};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod deploy;
mod distro;
mod hardware;
mod hexstrike;
mod install;
mod login;
mod packages;
#[cfg(test)]
mod testutil;
mod update;
mod util;

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
        Cmd::Install { no_aur_helper } => {
            warn_if_stale(&repo);
            install(distro, no_aur_helper)
        }
        Cmd::Deploy => {
            warn_if_stale(&repo);
            deploy(&repo, &home())
        }
        Cmd::All { no_aur_helper } => {
            ensure_repo(&repo)?;
            if binary_is_stale(&repo) {
                if std::env::var_os(REBUILT_ENV).is_some() {
                    warn(
                        "dotctl was just rebuilt and still differs from the checkout — is \
                         another dotctl ahead of cargo's on PATH? Continuing with this one.",
                    );
                } else {
                    rebuild_and_reexec(&repo);
                }
            }
            ensure_wallpapers(&repo);
            install(distro, no_aur_helper)?;
            deploy(&repo, &home())
        }
    }
}

/// Every file whose contents decide what this binary does, relative to the
/// repo root: the source, the manifest and lockfile, and each asset compiled
/// in with `include_str!`. `BUILT_FROM` hashes the same files in the same
/// order; a test holds the two lists together.
const SELF_SOURCE: [&str; 16] = [
    "src/main.rs",
    "Cargo.toml",
    "Cargo.lock",
    "assets/greetd/regreet.toml",
    "assets/greetd/regreet.css",
    "assets/hexstrike-host-guard.py",
    "src/deploy.rs",
    "src/distro.rs",
    "src/hardware.rs",
    "src/hexstrike.rs",
    "src/install.rs",
    "src/login.rs",
    "src/packages.rs",
    "src/testutil.rs",
    "src/update.rs",
    "src/util.rs",
];
