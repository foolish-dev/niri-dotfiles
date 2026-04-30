#!/usr/bin/env bash
# =============================================================================
# Bootstrap -- One-liner installer for foolish-dev/niri-dotfiles
# curl -fsSL https://raw.githubusercontent.com/foolish-dev/niri-dotfiles/main/bootstrap.sh | bash
# =============================================================================
set -euo pipefail

on_error() {
  local line=$1
  warn "bootstrap.sh failed at line $line. Check the output above for details."
}
trap 'on_error $LINENO' ERR

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info() { printf "${BLUE}[*]${NC} %s\n" "$*"; }
ok() { printf "${GREEN}[+]${NC} %s\n" "$*"; }
warn() { printf "${RED}[!]${NC} %s\n" "$*"; }
banner() {
  printf '%b' "$CYAN"
  cat <<'EOF'
        ██████╗  ██████╗ ████████╗███████╗██╗██╗     ███████╗███████╗
        ██╔══██╗██╔═══██╗╚══██╔══╝██╔════╝██║██║     ██╔════╝██╔════╝
        ██║  ██║██║   ██║   ██║   █████╗  ██║██║     █████╗  ███████╗
        ██║  ██║██║   ██║   ██║   ██╔══╝  ██║██║     ██╔══╝  ╚════██║
        ██████╔╝╚██████╔╝   ██║   ██║     ██║███████╗███████╗███████║
        ╚═════╝  ╚═════╝    ╚═╝   ╚═╝     ╚═╝╚══════╝╚══════╝╚══════╝
EOF
  printf '%b\n' "$NC"
  printf '%b        Arch Linux + BlackArch  //  Niri + Noctalia  //  Tokyo Night%b\n\n' "$BOLD" "$NC"
}

# ── Preflight checks ──────────────────────────────────────────────────────────
banner

if [[ ! -f /etc/arch-release ]]; then
  warn "This installer is designed for Arch Linux."
  warn "Detected: $(grep ^NAME= /etc/os-release 2>/dev/null | cut -d= -f2)"
  read -rp "Continue anyway? [y/N] " confirm
  [[ "$confirm" =~ ^[Yy]$ ]] || exit 1
fi

if [[ $EUID -eq 0 ]]; then
  warn "Do not run this script as root. Run as your normal user."
  exit 1
fi

# ── Dependencies for bootstrap ────────────────────────────────────────────────
info "Checking bootstrap dependencies..."
for cmd in git curl; do
  if ! command -v "$cmd" &>/dev/null; then
    info "Installing $cmd..."
    sudo pacman -S --noconfirm --needed "$cmd"
  fi
done
ok "Dependencies ready."

# ── Clone dotfiles ────────────────────────────────────────────────────────────
DOTFILES_DIR="${DOTFILES_DIR:-$HOME/niri-dotfiles}"

if [[ -d "$DOTFILES_DIR" ]]; then
  warn "Dotfiles directory already exists: $DOTFILES_DIR"
  read -rp "Remove and re-clone? [y/N] " confirm
  if [[ "$confirm" =~ ^[Yy]$ ]]; then
    rm -rf "$DOTFILES_DIR"
  else
    info "Using existing directory."
  fi
fi

if [[ ! -d "$DOTFILES_DIR" ]]; then
  info "Cloning dotfiles to $DOTFILES_DIR..."
  # --recurse-submodules pulls heimdall_opencode (agent pack + MCP source).
  # --shallow-submodules keeps the submodule to the pinned commit (no history).
  git clone --recurse-submodules --shallow-submodules \
    https://github.com/foolish-dev/niri-dotfiles.git "$DOTFILES_DIR"
  ok "Cloned."
fi

cd "$DOTFILES_DIR"

# ── Make scripts executable ───────────────────────────────────────────────────
chmod +x install.sh deploy.sh

# ── Run install.sh ────────────────────────────────────────────────────────────
echo ""
info "=== Running install.sh (packages, repos, tools) ==="
echo ""
./install.sh

# ── Run deploy.sh ─────────────────────────────────────────────────────────────
echo ""
info "=== Running deploy.sh (symlinks, configs) ==="
echo ""
./deploy.sh

# ── Done ──────────────────────────────────────────────────────────────────────
echo ""
printf '%b%b' "$GREEN" "$BOLD"
cat <<'EOF'
   ╔═══════════════════════════════════════════════════════════════════╗
   ║                      Installation complete!                       ║
   ╚═══════════════════════════════════════════════════════════════════╝
EOF
printf '%b\n' "$NC"

info "Next steps:"
info "  1. Log out"
info "  2. Select 'niri' from SDDM"
info "  3. Log in and enjoy!"
echo ""
info "Optional: Set a wallpaper to generate pywal colors:"
info "  wallpaper ~/Pictures/your-wallpaper.jpg"
echo ""
ok "Done. Happy hacking!"
