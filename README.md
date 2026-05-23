<p align="center">
  <img src="https://raw.githubusercontent.com/foolish-dev/niri-dotfiles/main/assets/divider.svg" alt="" width="900"/>
</p>

<h1 align="center">dotfiles</h1>

<p align="center">
  <sub>Minimal portable config bundle: <a href="https://github.com/foolish-dev/telia">telia</a> + <a href="https://github.com/foolish-dev/grogu">grogu</a> + <a href="https://github.com/0x4m4/hexstrike-ai">HexStrike AI</a> MCP, ready to drop onto a fresh box.</sub>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Telia-bb9af7?style=flat-square&logo=rust&logoColor=1a1b26" alt="Telia"/>
  <img src="https://img.shields.io/badge/grogu-7aa2f7?style=flat-square&logo=rust&logoColor=1a1b26" alt="grogu"/>
  <img src="https://img.shields.io/badge/HexStrike%20AI-f7768e?style=flat-square&logoColor=white" alt="HexStrike AI"/>
  <img src="https://img.shields.io/badge/Noctalia-c0caf5?style=flat-square&logoColor=1a1b26" alt="Noctalia"/>
  <img src="https://img.shields.io/badge/Neovim-9ece6a?style=flat-square&logo=neovim&logoColor=1a1b26" alt="Neovim"/>
  <img src="https://img.shields.io/badge/MCP-9ece6a?style=flat-square&logoColor=1a1b26" alt="MCP"/>
  <img src="https://img.shields.io/badge/systemd-e0af68?style=flat-square&logo=linux&logoColor=1a1b26" alt="systemd"/>
  <img src="https://img.shields.io/badge/Rust-bb9af7?style=flat-square&logo=rust&logoColor=white" alt="Rust"/>
  <img src="https://img.shields.io/badge/Arch_Linux-1793D1?style=flat-square&logo=archlinux&logoColor=white" alt="Arch Linux"/>
</p>

<img src="https://raw.githubusercontent.com/foolish-dev/niri-dotfiles/main/assets/divider.svg" alt="" width="900"/>

## What's here

- **[telia](https://github.com/foolish-dev/telia)** — TUI coding agent config (`.config/telia/config.toml`) wiring 10 MCP servers: context7, filesystem, github, fetch, hexstrike-ai, playwright, sequential-thinking, memory, git, weather.
- **[grogu](https://github.com/foolish-dev/grogu)** — `install.sh` cargo-installs the wallpaper-driven theme propagator from upstream.
- **[HexStrike AI](https://github.com/0x4m4/hexstrike-ai) MCP** — hardened systemd user unit (loopback `:8888`, `IPAddressDeny=any` except `127.0.0.0/8`) plus the `hexstrike-mcp` stdio bridge MCP clients point at.
- **[Noctalia](https://github.com/noctalia-dev/noctalia-shell)** — Quickshell-based Wayland shell (bar, dock, panels, notifications, lock screen). `settings.json` + `plugins.json` + bundled `Grogu` color scheme.
- **[Neovim](https://neovim.io)** — lazy.nvim + Mason setup: Tokyo Night base, treesitter, LSP, AI plugins. First launch auto-installs every plugin. Generated `colors/grogu.vim` (live-repainted by grogu) and `.luarc.json` are gitignored.

For the full Arch + Niri desktop experience, see **[niri-dotfiles](https://github.com/foolish-dev/niri-dotfiles)**.

<img src="https://raw.githubusercontent.com/foolish-dev/niri-dotfiles/main/assets/divider.svg" alt="" width="900"/>

## Install

```bash
git clone https://github.com/foolish-dev/dotfiles.git ~/dotfiles
cd ~/dotfiles
./install.sh                                                          # cargo-installs grogu, builds HexStrike under ~/tools/hexstrike-ai, pacman-installs Neovim + Noctalia on Arch
ln -sf ~/dotfiles/.config/telia ~/.config/telia
ln -sf ~/dotfiles/.config/nvim ~/.config/nvim
ln -sf ~/dotfiles/.config/noctalia ~/.config/noctalia
ln -sf ~/dotfiles/.local/bin/hexstrike-mcp ~/.local/bin/hexstrike-mcp
ln -sf ~/dotfiles/.config/systemd/user/hexstrike-server.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now hexstrike-server.service
```

<img src="https://raw.githubusercontent.com/foolish-dev/niri-dotfiles/main/assets/divider.svg" alt="" width="900"/>

## Layout

```
.config/
  telia/config.toml                       # 10 MCP servers
  nvim/                                   # lazy.nvim + Mason, Tokyo Night, LSP, AI
  noctalia/                               # settings.json, plugins.json, colorschemes/Grogu/
  systemd/user/
    hexstrike-server.service              # loopback :8888, hardened
    default.target.wants/...              # auto-enable on login
.local/bin/
  hexstrike-mcp                           # stdio bridge for MCP clients
install.sh                                # bootstrap grogu + HexStrike + Neovim + Noctalia
```
