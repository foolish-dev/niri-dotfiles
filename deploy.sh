#!/usr/bin/env bash
# =============================================================================
# deploy.sh -- Symlink dotfiles into $HOME and enable user services
# =============================================================================
set -euo pipefail

DOTFILES="$(cd "$(dirname "$0")" && pwd)"
BACKUP_DIR="$HOME/.dotfiles-backup/$(date +%Y%m%d-%H%M%S)"

info() { printf "[*] %s\n" "$*"; }
ok() { printf "[+] %s\n" "$*"; }
warn() { printf "[!] %s\n" "$*" >&2; }

link_item() {
  local src="$1" dest="$2"
  if [[ -e "$dest" || -L "$dest" ]]; then
    if [[ "$(readlink -f "$dest" 2>/dev/null)" == "$(readlink -f "$src" 2>/dev/null)" ]]; then
      return
    fi
    mkdir -p "$BACKUP_DIR"
    local rel="${dest#"$HOME"/}"
    mkdir -p "$BACKUP_DIR/$(dirname "$rel")"
    mv "$dest" "$BACKUP_DIR/$rel"
    warn "Backed up: ~/$rel -> $BACKUP_DIR/$rel"
  fi
  mkdir -p "$(dirname "$dest")"
  ln -sf "$src" "$dest"
  ok "Linked: ~/${dest#"$HOME"/}"
}

info "=== Deploying from $DOTFILES ==="

# .config/<dir> symlinks
for d in telia nvim noctalia fastfetch tmux; do
  [[ -d "$DOTFILES/.config/$d" ]] && link_item "$DOTFILES/.config/$d" "$HOME/.config/$d"
done

# systemd user unit (file, not dir — leaves room for other units later)
link_item "$DOTFILES/.config/systemd/user/hexstrike-server.service" \
          "$HOME/.config/systemd/user/hexstrike-server.service"

# .local/bin scripts
mkdir -p "$HOME/.local/bin"
for f in "$DOTFILES/.local/bin"/*; do
  [[ -f "$f" ]] && link_item "$f" "$HOME/.local/bin/$(basename "$f")"
done

# Enable hexstrike service (WantedBy=default.target → systemctl enable creates the wants symlink)
systemctl --user daemon-reload 2>/dev/null || true
if systemctl --user enable --now hexstrike-server.service 2>/dev/null; then
  ok "hexstrike-server.service enabled"
else
  warn "hexstrike-server.service not enabled (run install.sh first to clone hexstrike-ai)"
fi

ok "Deploy complete"
