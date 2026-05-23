#!/usr/bin/env bash
# Bootstrap tools used by this dotfiles repo.
set -euo pipefail

if ! command -v cargo &>/dev/null; then
  echo "[!] cargo not found — install rust first (https://rustup.rs)" >&2
  exit 1
fi

if ! command -v grogu &>/dev/null; then
  echo "[*] Installing grogu (wallpaper-driven theme propagator) ..."
  cargo install --git https://github.com/foolish-dev/grogu \
    --branch main --locked
  echo "[+] grogu installed -> ~/.cargo/bin/grogu"
else
  echo "[+] grogu already installed (use 'cargo install --force ...' to rebuild)"
fi

# ── HexStrike AI MCP ──────────────────────────────────────────────────────
# Loopback Flask server on :8888; .local/bin/hexstrike-mcp is the stdio
# bridge MCP clients (opencode, telia) point at. The systemd user unit
# under .config/systemd/user/ auto-starts it on login.
HEXSTRIKE_DIR="$HOME/tools/hexstrike-ai"
if [[ ! -d "$HEXSTRIKE_DIR" ]]; then
  echo "[*] Cloning HexStrike AI ..."
  mkdir -p "$HOME/tools"
  git clone https://github.com/0x4m4/hexstrike-ai.git "$HEXSTRIKE_DIR"
fi

if [[ ! -d "$HEXSTRIKE_DIR/hexstrike-env" ]]; then
  echo "[*] Creating HexStrike Python venv ..."
  python3 -m venv "$HEXSTRIKE_DIR/hexstrike-env"
fi

echo "[*] Installing HexStrike Python dependencies ..."
"$HEXSTRIKE_DIR/hexstrike-env/bin/pip" install --quiet --upgrade pip
"$HEXSTRIKE_DIR/hexstrike-env/bin/pip" install --quiet -r "$HEXSTRIKE_DIR/requirements.txt"
echo "[+] HexStrike ready (loopback :8888 via hexstrike-server.service)"

# ── Noctalia shell ─────────────────────────────────────────────────────────
# Quickshell-based Wayland shell (bar, dock, panels, notifications, lock).
# Arch-only auto-install; on other distros it warns and lets you handle it.
if ! command -v noctalia-shell &>/dev/null; then
  if command -v pacman &>/dev/null; then
    echo "[*] Installing Noctalia ..."
    sudo pacman -S --needed --noconfirm noctalia-shell noctalia-qs
    echo "[+] Noctalia installed"
  else
    echo "[!] Noctalia not installed and pacman not found — install noctalia-shell + noctalia-qs manually" >&2
  fi
else
  echo "[+] Noctalia already installed"
fi
