# dotfiles

<p align="center">
  <img src="assets/desktop.png" alt="Desktop — niri scrolling-tile Wayland session, tmux running nvim + teleia side-by-side under the Tokyo Night palette" width="900"/>
</p>

An AI coding agent, a wallpaper-driven theming engine, an offensive-security MCP backend, and the editor / multiplexer / shell / banner that round out the desktop — all installed and deployed by a single Rust binary.

## At a glance

| | |
| --- | --- |
| **Binary** | `dotctl` — one Rust crate, three subcommands |
| **Editor** | Neovim with **109 plugins**, **35 LSPs**, 21 formatters/linters, 4 DAP adapters |
| **Agent** | `teleia` wired to **10 MCP servers** (context7 · filesystem · github · fetch · hexstrike-ai · playwright · sequential-thinking · memory · git · weather) |
| **Offsec** | HexStrike AI on a sandboxed systemd unit + BlackArch repo (5,000+ packages, `pacman -S` away) |
| **Theming** | `grogu` repaints **9 targets** from the current wallpaper, in one shot |
| **Palette** | Grogu (`#0e112b` surface · `#6ecfdc` on-surface · `#61c3cf` primary) across every themed target |

## Install

> **The base desktop.** `dotctl install` installs the Niri base desktop (`niri`, `fuzzel`, `kitty`, `wl-clip-persist`) alongside the curated tools (grogu, HexStrike, Neovim, tmux, fastfetch, Noctalia, the greeter), then `dotctl deploy` lays down the dotfiles. Running `dotctl deploy` on its own installs nothing, so it warns if `niri` is missing — run `dotctl install` (or `dotctl all`) first. For the heavier full bundle (ghostty, lazygit, GTK/Qt theming, 300+ BlackArch tools), see **[foolish-dev/distro-work](https://github.com/foolish-dev/distro-work)**.

```bash
cargo install --git https://github.com/foolish-dev/niri-dotfiles --locked && dotctl all
```

Needs `cargo` (install rustup if you don't have it). `cargo install` fetches the source, builds, and drops `dotctl` into `~/.cargo/bin`. `dotctl all` then clones (or pulls) the repo into `~/niri-dotfiles`, runs `install`, and runs `deploy`.

Manual:

```bash
git clone https://github.com/foolish-dev/niri-dotfiles.git ~/niri-dotfiles
cd ~/niri-dotfiles
cargo build --release
./target/release/dotctl install   # tools + BlackArch (+ Chaotic AUR on Arch), idempotent
./target/release/dotctl deploy    # symlink .config/* + .local/bin/*, enable hexstrike-server
# or: dotctl all
```

Both `install` and `deploy` are idempotent. `deploy` moves pre-existing non-symlink files to `~/.dotfiles-backup/<unix-ts>/` before replacing them. `--repo <path>` or the `DOTFILES_REPO` env var overrides the default `~/niri-dotfiles` source.

| Subcommand | What it does |
|---|---|
| `dotctl install` | Wire BlackArch, plus Chaotic AUR on non-CachyOS hosts (both idempotent); ensure an AUR helper (`yay` from `[cachyos]` on CachyOS, from Chaotic-AUR on Arch, or whichever of `yay`/`paru` is already present), `cargo install` grogu, clone+venv HexStrike AI, `pacman -S` tmux/fastfetch/neovim/wl-clip-persist and `noctalia-shell`+`noctalia-qs` (CachyOS `[cachyos]` repo only — warns and continues elsewhere), AUR-install `sddm-theme-noctalia-git` (via yay or paru, a themed SDDM fallback), `pacman -S` `greetd`/`greetd-regreet`/`cage` (then select the noctalia **SDDM** theme via `/etc/sddm.conf.d/` and enable `sddm.service` as the graphical login screen — disabling `greetd`, but never overriding a third-party display manager; greetd + a noctalia-themed ReGreet config under `/etc/greetd/` are kept as a disabled fallback), and build `noctalia-unofficial-auth-agent-git` from a locally patched PKGBUILD (GCC 16 fix). |
| `dotctl deploy` | Symlink the full `.config` set (`teleia, nvim, noctalia, fastfetch, tmux, fuzzel, gtk-3.0/4.0, kitty, lazygit, neofetch, niri, opencode, qt5ct/6ct, wal, starship.toml`), home dotfiles (`.zshrc, .editorconfig, .gitignore_global`), and every `.local/bin/*`. Copy the curated `wallpapers/*` into `~/Pictures/Wallpapers` as real files (applied on all monitors; copies, not symlinks, so a stray wallpaper `cp` — e.g. the greeter background-sync — can't write back into the repo). Deploy the tracked `.gitconfig` via an untracked `~/.gitconfig` include-stub + seed `~/.gitconfig.local` identity. Symlink the user systemd units (real dir), `daemon-reload`, then `enable --now` hexstrike-server + `bb-auth.service`. Back up displaced files. |
| `dotctl all` | `install` then `deploy`. |

## Supported distros

Arch Linux and CachyOS. `dotctl` reads `/etc/os-release` and branches on `ID`:

| | Arch (and `ID_LIKE=arch` derivatives) | CachyOS |
|---|---|---|
| AUR helper | bootstrapped from Chaotic-AUR (`yay`/`paru` are in no Arch official repo) | `pacman -S yay` straight from `[cachyos]` |
| Chaotic-AUR | added — it is the only helper source | skipped; `[cachyos]` already provides yay/paru |
| BlackArch | added (appended last, below every distro repo) | same |
| Noctalia | `noctalia-shell`/`noctalia-qs` are packaged **only** by CachyOS — add the `[cachyos]` repo, or this step warns and is skipped | installs from `[cachyos]` |
| Display manager | dotctl enables `sddm.service` | same, unless another greeter already owns `/etc/greetd/config.toml` |

An unrecognised distro is treated as Arch. On any host without `pacman`, package
steps warn and continue. Pass `--no-aur-helper` to `install`/`all` to stop
dotctl installing an AUR helper for you; AUR-only add-ons are then skipped.

`dotctl` never takes over a login screen somebody else configured: a `greetd`
whose session command is neither `regreet` (ours) nor the stock `agreety` — for
instance CachyOS's `noctalia-greeter` — is left alone, config and unit both.

## Architecture

`dotctl` is the only thing that needs to run. What you end up with: `teleia` driving 10 MCP servers (the `hexstrike-ai` edge is the offsec one), and `grogu` repainting Neovim · tmux · Noctalia · kitty every time the wallpaper changes.

## Showcase

<p align="center">
  <img src="assets/teleia.png" alt="teleia — minimal TUI coding agent on the Janus-35B model, Tokyo Night with translucent wallpaper bleed-through" width="900"/>
</p>

<p align="center">
  <img src="assets/nvim.png" alt="Neovim — Neo-tree sidebar, Tokyo Night, treesitter highlighting on init.lua, lualine status bar" width="900"/>
</p>

<p align="center">
  <img src="assets/tmux.png" alt="tmux — multi-pane session under kitty, Tokyo Night powerline status bar" width="900"/>
</p>

<p align="center">
  <img src="assets/fastfetch.png" alt="fastfetch — terminal banner with Arch logo, kernel, niri/zsh/tmux versions, AMD Ryzen AI MAX+ 395, Radeon 8060S, palette swatches" width="900"/>
</p>

<p align="center">
  <img src="assets/noctalia.png" alt="Noctalia bar — clock + date, CPU/MEM/GPU mini-stats, active-window title, workspace pills, notification / battery / volume / brightness / timer tray" width="900"/>
</p>

## Components

**`dotctl`** is the in-tree Rust binary that replaced the old `install.sh` + `deploy.sh` pair. Clap-derive CLI, anyhow errors, no async runtime. See the subcommand table under [Install](#install).

[**teleia**](https://github.com/foolish-dev/teleia) (τέλεια — "perfect") is a single-binary TUI coding agent. `.config/teleia/config.toml` wires `context7`, `filesystem`, `github`, `fetch`, `hexstrike-ai`, `playwright`, `sequential-thinking`, `memory`, `git`, and `weather` — drop-in compatible with any other MCP client.

[**grogu**](https://github.com/foolish-dev/grogu) extracts a palette from the current wallpaper and writes themed fragments for niri, kitty, ghostty, tmux, Neovim, teleia, Noctalia, the SDDM greeter background, and the keyboard backlight in one shot. `dotctl install` cargo-installs it from upstream.

[**HexStrike AI**](https://github.com/0x4m4/hexstrike-ai) is a Flask MCP backend exposing 150+ offensive-security tools. Shipped here as a systemd user unit (`ProtectSystem=strict`, `ProtectHome=read-only`, `PrivateTmp`, `NoNewPrivileges`) plus the `hexstrike-mcp` stdio bridge MCP clients call. **The API is unauthenticated — `/api/command` executes arbitrary shell as your user — so it must never be reachable off loopback.** Upstream hardcodes `app.run(host="0.0.0.0")`, ignoring its own `HEXSTRIKE_HOST`; `dotctl install` rewrites that to honour it, on every run. Do not rely on systemd's `IPAddress*` directives here: a *user* manager cannot install a cgroup BPF firewall, and silently doesn't. The underlying CLI tools live in [BlackArch](https://blackarch.org) — `dotctl install` wires its repo via the upstream `strap.sh` so `pacman -S <tool>` reaches over 5,000 packages.

**Neovim** — lazy.nvim + Mason, kitchen-sink config:

- **109 plugins** lazy-loaded by event, ft, or key.
- **35 LSPs** auto-installed via `mason-lspconfig` — lua, rust, go, python (pyright + ruff), C/C++, zig, every JS/TS framework (ts, vue, svelte, astro, prisma, tailwind, graphql), the IaC set (yaml, terraform, helm, ansible, docker), and the long tail (elixir, kotlin, jdtls, intelephense, solargraph, texlab, marksman). `ocamllsp`, `hls`, `nil_ls`, `cmake` are omitted by default — install opam / ghcup / etc. and append to `lsp.lua` to re-enable.
- **21 formatters + linters** auto-installed via `mason-tool-installer` (stylua, prettierd, ruff, black, shellcheck, hadolint, eslint_d, gofumpt, alejandra, buf, …) and **4 DAP adapters** via `mason-nvim-dap` (codelldb, debugpy, delve, js-debug-adapter). `nvim-lint` silently skips any binary that hasn't landed yet.
- **AI**: `opencode` (built-in teleia popup), `copilot.lua` (inline ghost text, `<M-l>` to accept), `avante`, `codecompanion`.
- **Testing & debug**: `neotest` (python · go · rust · jest · vitest · plenary), `nvim-dap` + `dap-ui` + `dap-virtual-text`, `rustaceanvim`, `crates.nvim`, `go.nvim`, `typescript-tools`, `venv-selector`.
- **Editor**: `flash`, `vim-illuminate`, `nvim-spectre`, `nvim-ufo`, `harpoon` v2, `oil`, `mini.{ai,bracketed,indentscope}`, `yanky`, `undotree`.
- **UI**: `lualine`, `bufferline`, `barbecue` winbar, `noice`, `alpha`, `fidget`, `aerial`, `zen-mode`, `twilight`, `nvim-colorizer`, `rainbow-delimiters`, `treesitter-context`.
- **Sessions & projects**: `persistence.nvim`, `project.nvim`.

Tokyo Night base, repainted live by `grogu` via `colors/grogu.vim` (gitignored). `nvim-treesitter` and `-textobjects` pinned to `master` so the legacy `configs.setup()` API stays available.

**tmux** — `C-a` prefix, Tokyo Night powerline status, RAM-bar fragment under `.config/tmux/scripts/mem.sh`, and a `grogu.conf` slot (gitignored) the wallpaper-driven theme propagator writes to.

[**Noctalia**](https://github.com/noctalia-dev/noctalia-shell) is a Quickshell-based Wayland shell: bar, dock, panels, notifications, lock screen, app launcher. `settings.json` + `plugins.json` + bundled `Grogu` colorscheme. Bar tuned to transparency-blur (`backgroundOpacity: 0.2`, global `enableBlurBehind: true`) and `ActiveWindow` widget dropped from the layout.

**fastfetch** — a Tokyo-Night-tinted `config.jsonc` for [fastfetch](https://github.com/fastfetch-cli/fastfetch) — terminal banner with OS, kernel, shell, and color blocks.

## Layout

```
.
├── assets/                                  # README screenshots (desktop, teleia, nvim, tmux, fastfetch, noctalia)
├── .config/
│   ├── teleia/config.toml                   # 10 MCP servers
│   ├── nvim/                                # lazy.nvim + Mason, 109 plugins, 35 LSPs
│   ├── tmux/                                # tmux.conf, scripts/mem.sh
│   ├── noctalia/                            # settings.json, plugins.json, colorschemes/Grogu/
│   ├── fastfetch/config.jsonc               # terminal banner
│   └── systemd/user/
│       └── hexstrike-server.service         # :8888, loopback-pinned at install
├── .local/bin/
│   └── hexstrike-mcp                        # stdio bridge for MCP clients
├── src/main.rs                              # dotctl — Rust installer/deployer
├── Cargo.toml                               # crate manifest (clap-derive + anyhow)
└── Cargo.lock                               # locked
```

## See also

For the full Arch / CachyOS + Niri + BlackArch desktop bundle (kitty, ghostty, lazygit, opencode, 147 BlackArch launcher entries, SDDM themes, 300+ tools), see **[foolish-dev/distro-work](https://github.com/foolish-dev/distro-work)** (formerly `niri-dotfiles`).
