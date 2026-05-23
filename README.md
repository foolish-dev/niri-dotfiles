<p align="center">
  <img src="assets/header.svg" alt="dotfiles — Telia + grogu + HexStrike AI MCP + Neovim + Noctalia + fastfetch" width="900"/>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Telia-bb9af7?style=flat-square&logo=rust&logoColor=1a1b26" alt="Telia"/>
  <img src="https://img.shields.io/badge/grogu-7dcfff?style=flat-square&logo=rust&logoColor=1a1b26" alt="grogu"/>
  <img src="https://img.shields.io/badge/HexStrike%20AI-f7768e?style=flat-square&logoColor=white" alt="HexStrike AI"/>
  <img src="https://img.shields.io/badge/Neovim-9ece6a?style=flat-square&logo=neovim&logoColor=1a1b26" alt="Neovim"/>
  <img src="https://img.shields.io/badge/tmux-7aa2f7?style=flat-square&logo=tmux&logoColor=1a1b26" alt="tmux"/>
  <img src="https://img.shields.io/badge/Noctalia-c0caf5?style=flat-square&logoColor=1a1b26" alt="Noctalia"/>
  <img src="https://img.shields.io/badge/fastfetch-7dcfff?style=flat-square&logoColor=1a1b26" alt="fastfetch"/>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/MCP-9ece6a?style=flat-square&logoColor=1a1b26" alt="MCP"/>
  <img src="https://img.shields.io/badge/systemd-e0af68?style=flat-square&logo=linux&logoColor=1a1b26" alt="systemd"/>
  <img src="https://img.shields.io/badge/Rust-bb9af7?style=flat-square&logo=rust&logoColor=white" alt="Rust"/>
  <img src="https://img.shields.io/badge/Arch_Linux-1793D1?style=flat-square&logo=archlinux&logoColor=white" alt="Arch Linux"/>
  <img src="https://img.shields.io/badge/Chaotic_AUR-f7768e?style=flat-square&logoColor=white" alt="Chaotic AUR"/>
  <img src="https://img.shields.io/badge/Wayland-FFBc00?style=flat-square&logo=wayland&logoColor=black" alt="Wayland"/>
  <img src="https://img.shields.io/badge/Tokyo_Night-7aa2f7?style=flat-square&logoColor=white" alt="Tokyo Night"/>
</p>

<p align="center">
  Minimal portable config bundle: an AI coding agent, a wallpaper-driven theming engine, an offensive-security MCP backend, and the editor/shell/banner that round out the desktop. <code>curl | bash</code> on a fresh Arch box.
</p>

<img src="assets/divider.svg" alt="" width="900"/>

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/foolish-dev/dotfiles/main/bootstrap.sh | bash
```

`bootstrap.sh` ensures `git` and `rust`/`cargo` are present, clones (or pulls) into `~/dotfiles`, builds the **`dotctl`** Rust binary, and runs `dotctl all` end-to-end. `dotctl install` wires [Chaotic AUR](https://aur.chaotic.cx) into `/etc/pacman.conf` before the first pacman call — required for `noctalia-shell` and `noctalia-qs` (and a useful general-purpose AUR mirror).

Manual:

```bash
git clone https://github.com/foolish-dev/dotfiles.git ~/dotfiles
cd ~/dotfiles
cargo build --release
./target/release/dotctl install   # tools: grogu (cargo), HexStrike AI (clone+venv), Neovim/tmux/Noctalia/fastfetch (pacman on Arch)
./target/release/dotctl deploy    # symlinks .config/* and .local/bin/* into $HOME, enables hexstrike-server.service
# or: dotctl all  — install + deploy
```

`deploy` is idempotent. Pre-existing non-symlink files are moved to `~/.dotfiles-backup/<unix-ts>/` before being replaced. `--repo <path>` (or `DOTFILES_REPO` env) overrides the default `~/dotfiles` source.

<img src="assets/divider.svg" alt="" width="900"/>

## Architecture

<p align="center">
  <img src="assets/stack.svg" alt="Architecture — Telia drives HexStrike AI MCP; grogu repaints Neovim / Noctalia from the wallpaper; fastfetch banners the terminal" width="900"/>
</p>

<img src="assets/divider.svg" alt="" width="900"/>

## Components

<img src="assets/telia.svg" alt="Telia — TUI coding agent (Rust, MCP-aware), 10 MCP servers wired" width="900"/>

[Telia](https://github.com/foolish-dev/telia) is a single-binary TUI coding agent. The bundled `.config/telia/config.toml` wires `context7`, `filesystem`, `github`, `fetch`, `hexstrike-ai`, `playwright`, `sequential-thinking`, `memory`, `git`, and `weather` — drop-in compatible with any other MCP client.

<img src="assets/grogu.svg" alt="grogu — wallpaper-driven palette propagator, repaints 7 targets" width="900"/>

[grogu](https://github.com/foolish-dev/grogu) extracts a palette from the current wallpaper and writes themed fragments for niri, kitty, ghostty, tmux, Neovim, Telia, and Noctalia in one shot. `install.sh` cargo-installs it from upstream.

<img src="assets/hexstrike.svg" alt="HexStrike AI — MCP backend, 150+ offensive-security tools" width="900"/>

[HexStrike AI](https://github.com/0x4m4/hexstrike-ai) is a Flask MCP backend exposing 150+ offensive-security tools. Shipped here as a hardened systemd user unit (loopback `:8888`, `IPAddressDeny=any` except `127.0.0.0/8`, `ProtectSystem=strict`, `ProtectHome=read-only`) plus the `hexstrike-mcp` stdio bridge MCP clients call.

<img src="assets/neovim.svg" alt="Neovim — lazy.nvim, Mason, Tokyo Night, LSP, AI plugins" width="900"/>

lazy.nvim + Mason setup. Tokyo Night base, treesitter, LSP, AI plugins (Copilot, Avante). First `nvim` launch auto-installs everything. Generated `colors/grogu.vim` (live-repainted by grogu) and `.luarc.json` stay gitignored.

<img src="assets/tmux.svg" alt="tmux — terminal multiplexer, panes, windows, status bar" width="900"/>

Terminal multiplexer config with the `C-a` prefix, `tokyonight` status palette, RAM-bar fragment under `.config/tmux/scripts/mem.sh`, and a `grogu.conf` slot (gitignored) the wallpaper-driven theme propagator writes to.

<img src="assets/noctalia.svg" alt="Noctalia — Quickshell-based Wayland shell" width="900"/>

[Noctalia](https://github.com/noctalia-dev/noctalia-shell) is a Quickshell-based Wayland shell: bar, dock, panels, notifications, lock screen, app launcher. `settings.json` + `plugins.json` + the bundled `Grogu` color scheme.

<img src="assets/fastfetch.svg" alt="fastfetch — terminal system info banner" width="900"/>

A Tokyo-Night-tinted `config.jsonc` for [fastfetch](https://github.com/fastfetch-cli/fastfetch) — terminal banner with OS, kernel, shell, and color blocks.

<img src="assets/divider.svg" alt="" width="900"/>

## Layout

```
.
├── assets/                                  # README artwork
├── .config/
│   ├── telia/config.toml                    # 10 MCP servers
│   ├── nvim/                                # lazy.nvim + Mason, Tokyo Night, LSP, AI
│   ├── tmux/                                # tmux.conf, scripts/mem.sh
│   ├── noctalia/                            # settings.json, plugins.json, colorschemes/Grogu/
│   ├── fastfetch/config.jsonc               # terminal banner
│   └── systemd/user/
│       ├── hexstrike-server.service         # loopback :8888, hardened
│       └── default.target.wants/...         # auto-enable on login
├── .local/bin/
│   └── hexstrike-mcp                        # stdio bridge for MCP clients
├── src/main.rs                              # dotctl — Rust installer/deployer
├── Cargo.toml                               # crate manifest (clap + anyhow)
└── bootstrap.sh                             # one-liner: ensure rust → cargo build → dotctl all
```

<img src="assets/divider.svg" alt="" width="900"/>

## See also

For the full Arch + Niri + BlackArch desktop bundle (kitty, tmux, ghostty, lazygit, opencode, 147 BlackArch launcher entries, SDDM themes), see **[niri-dotfiles](https://github.com/foolish-dev/niri-dotfiles)**.
