# dotfiles

Minimal portable config bundle: an AI coding agent, a wallpaper-driven theming engine, an offensive-security MCP backend, and the editor / multiplexer / shell / banner that round out the desktop — all installed and deployed by a single Rust binary.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/foolish-dev/dotfiles/main/bootstrap.sh | bash
```

`bootstrap.sh` ensures `git` and `rust`/`cargo` are present, clones (or pulls) into `~/dotfiles`, builds the `dotctl` Rust binary, and runs `dotctl all` end-to-end.

Manual:

```bash
git clone https://github.com/foolish-dev/dotfiles.git ~/dotfiles
cd ~/dotfiles
cargo build --release
./target/release/dotctl install   # tools + Chaotic AUR repo, idempotent
./target/release/dotctl deploy    # symlinks .config/* and .local/bin/* into $HOME, enables hexstrike-server
# or: dotctl all
```

`deploy` is idempotent — pre-existing non-symlink files are moved to `~/.dotfiles-backup/<unix-ts>/` before being replaced. `--repo <path>` or `DOTFILES_REPO` env overrides the default `~/dotfiles` source.

## Components

**`dotctl`** is the in-tree Rust binary that replaces the old `install.sh` + `deploy.sh` pair. ~250 LoC, clap-derive CLI, anyhow errors. Three subcommands:

| Subcommand | What it does |
|---|---|
| `dotctl install` | Wire Chaotic AUR (idempotent), `cargo install` grogu, clone+venv HexStrike AI, `pacman -S` tmux/fastfetch/neovim/noctalia-shell. |
| `dotctl deploy` | Symlink `.config/{telia,nvim,noctalia,fastfetch,tmux}` and every `.local/bin/*` into `$HOME`. Symlink the hexstrike systemd unit, `daemon-reload`, `enable --now`. Back up displaced files to `~/.dotfiles-backup/<unix-ts>/`. |
| `dotctl all` | `install` then `deploy`. |

[Telia](https://github.com/foolish-dev/telia) is a single-binary TUI coding agent. `.config/telia/config.toml` wires `context7`, `filesystem`, `github`, `fetch`, `hexstrike-ai`, `playwright`, `sequential-thinking`, `memory`, `git`, and `weather` — drop-in compatible with any other MCP client.

[grogu](https://github.com/foolish-dev/grogu) extracts a palette from the current wallpaper and writes themed fragments for niri, kitty, ghostty, tmux, Neovim, Telia, and Noctalia in one shot. `dotctl install` cargo-installs it from upstream.

[HexStrike AI](https://github.com/0x4m4/hexstrike-ai) is a Flask MCP backend exposing 150+ offensive-security tools. Shipped here as a hardened systemd user unit (loopback `:8888`, `IPAddressDeny=any` except `127.0.0.0/8`, `ProtectSystem=strict`, `ProtectHome=read-only`) plus the `hexstrike-mcp` stdio bridge MCP clients call.

**Neovim** — lazy.nvim + Mason. Tokyo Night base, treesitter, LSP, AI plugins (Copilot, Avante). First `nvim` launch auto-installs everything. Generated `colors/grogu.vim` (live-repainted by grogu) and `.luarc.json` stay gitignored.

**tmux** — `C-a` prefix, Tokyo Night powerline status, RAM-bar fragment under `.config/tmux/scripts/mem.sh`, and a `grogu.conf` slot (gitignored) the wallpaper-driven theme propagator writes to.

[Noctalia](https://github.com/noctalia-dev/noctalia-shell) is a Quickshell-based Wayland shell: bar, dock, panels, notifications, lock screen, app launcher. `settings.json` + `plugins.json` + bundled `Grogu` color scheme. Bar tuned to transparency-blur (`backgroundOpacity: 0.2`, global `enableBlurBehind: true`) and `ActiveWindow` widget dropped from the layout.

**fastfetch** — a Tokyo-Night-tinted `config.jsonc` for [fastfetch](https://github.com/fastfetch-cli/fastfetch); terminal banner with OS, kernel, shell, and color blocks.

## Layout

```
.
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
├── src/main.rs                              # dotctl — Rust installer/deployer (~250 LoC)
├── Cargo.toml                               # crate manifest (clap-derive + anyhow)
├── Cargo.lock                               # locked
└── bootstrap.sh                             # one-liner: ensure rust → cargo build → dotctl all
```

## See also

For the full Arch + Niri + BlackArch desktop bundle (kitty, ghostty, lazygit, opencode, 147 BlackArch launcher entries, SDDM themes, 300+ tools), see **[foolish-dev/distro-work](https://github.com/foolish-dev/distro-work)** (formerly `niri-dotfiles`).
