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
