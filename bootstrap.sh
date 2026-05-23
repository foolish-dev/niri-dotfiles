#!/usr/bin/env bash
# =============================================================================
# bootstrap.sh -- One-liner installer for foolish-dev/dotfiles
# curl -fsSL https://raw.githubusercontent.com/foolish-dev/dotfiles/main/bootstrap.sh | bash
#
# Ensures git + rust are present, clones (or pulls) the repo, builds the
# `dotctl` Rust binary, and runs `dotctl all` (install + deploy).
# =============================================================================
set -euo pipefail

info() { printf "[*] %s\n" "$*"; }
ok()   { printf "[+] %s\n" "$*"; }
warn() { printf "[!] %s\n" "$*" >&2; }

trap 'warn "bootstrap.sh failed at line $LINENO"' ERR

[[ $EUID -eq 0 ]] && { warn "Do not run as root"; exit 1; }

ensure_pkg() {
  local cmd="$1" pkg="$2"
  command -v "$cmd" >/dev/null && return
  if command -v pacman >/dev/null; then
    info "Installing $pkg ..."
    sudo pacman -S --needed --noconfirm "$pkg"
  else
    warn "$cmd not found and pacman unavailable -- install $pkg first"
    exit 1
  fi
}

ensure_pkg git git
ensure_pkg cargo rust

DOTFILES_DIR="${DOTFILES_DIR:-$HOME/dotfiles}"

if [[ -d "$DOTFILES_DIR/.git" ]]; then
  info "Updating $DOTFILES_DIR ..."
  git -C "$DOTFILES_DIR" pull --ff-only || warn "git pull skipped (local changes?)"
else
  info "Cloning to $DOTFILES_DIR ..."
  git clone https://github.com/foolish-dev/dotfiles.git "$DOTFILES_DIR"
fi

cd "$DOTFILES_DIR"

info "Building dotctl (release) ..."
cargo build --release

info "=== dotctl all ==="
./target/release/dotctl all

ok "Bootstrap complete"
