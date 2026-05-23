#!/usr/bin/env bash
# =============================================================================
# bootstrap.sh -- One-liner installer for foolish-dev/dotfiles
# curl -fsSL https://raw.githubusercontent.com/foolish-dev/dotfiles/main/bootstrap.sh | bash
# =============================================================================
set -euo pipefail

info() { printf "[*] %s\n" "$*"; }
ok() { printf "[+] %s\n" "$*"; }
warn() { printf "[!] %s\n" "$*" >&2; }

trap 'warn "bootstrap.sh failed at line $LINENO"' ERR

[[ $EUID -eq 0 ]] && { warn "Do not run as root"; exit 1; }

# Ensure git
if ! command -v git &>/dev/null; then
  if command -v pacman &>/dev/null; then
    info "Installing git ..."
    sudo pacman -S --needed --noconfirm git
  else
    warn "git not found and pacman unavailable -- install git first"
    exit 1
  fi
fi

DOTFILES_DIR="${DOTFILES_DIR:-$HOME/dotfiles}"

if [[ -d "$DOTFILES_DIR/.git" ]]; then
  info "Updating $DOTFILES_DIR ..."
  git -C "$DOTFILES_DIR" pull --ff-only || warn "  git pull skipped (local changes?)"
else
  info "Cloning to $DOTFILES_DIR ..."
  git clone https://github.com/foolish-dev/dotfiles.git "$DOTFILES_DIR"
fi

cd "$DOTFILES_DIR"
chmod +x install.sh deploy.sh bootstrap.sh

info "=== install.sh (tool installs) ==="
./install.sh

info "=== deploy.sh (symlinks + service) ==="
./deploy.sh

ok "Bootstrap complete"
