<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/header.svg"/>
    <source media="(prefers-color-scheme: light)" srcset="assets/header-light.svg"/>
    <img src="assets/header.svg" alt="Dotfiles — Arch Linux + BlackArch + Niri + Noctalia + Tokyo Night" width="900"/>
  </picture>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Arch_Linux-1793D1?style=flat-square&logo=archlinux&logoColor=white" alt="Arch Linux"/>
  <img src="https://img.shields.io/badge/Wayland-FFBc00?style=flat-square&logo=wayland&logoColor=black" alt="Wayland"/>
  <img src="https://img.shields.io/badge/Neovim-57A143?style=flat-square&logo=neovim&logoColor=white" alt="Neovim"/>
  <img src="https://img.shields.io/badge/Zsh-F15A24?style=flat-square&logo=zsh&logoColor=white" alt="Zsh"/>
  <img src="https://img.shields.io/badge/tmux-1BB91F?style=flat-square&logo=tmux&logoColor=white" alt="tmux"/>
  <img src="https://img.shields.io/badge/Kitty-000000?style=flat-square&logo=gnometerminal&logoColor=white" alt="Kitty"/>
  <img src="https://img.shields.io/badge/Docker-2496ED?style=flat-square&logo=docker&logoColor=white" alt="Docker"/>
  <img src="https://img.shields.io/badge/Rust-000000?style=flat-square&logo=rust&logoColor=white" alt="Rust"/>
  <img src="https://img.shields.io/badge/Python-3776AB?style=flat-square&logo=python&logoColor=white" alt="Python"/>
  <img src="https://img.shields.io/badge/Go-00ADD8?style=flat-square&logo=go&logoColor=white" alt="Go"/>
  <img src="https://img.shields.io/badge/License-MIT-7aa2f7?style=flat-square" alt="MIT License"/>
</p>

<p align="center">
  Personal dotfiles for a scrollable-tiling Wayland desktop built for coding and offensive security.
</p>

<img src="assets/divider.svg" alt="" width="900"/>

## Stack

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/stack.svg"/>
  <source media="(prefers-color-scheme: light)" srcset="assets/stack-light.svg"/>
  <img src="assets/stack.svg" alt="Architecture — Arch, Niri, Noctalia, Neovim, Docker, LM Studio stack diagram" width="900"/>
</picture>

| Layer | Tool |
|---|---|
| Distro | Arch Linux + [BlackArch](https://blackarch.org) + [Chaotic AUR](https://aur.chaotic.cx) repos |
| Compositor | [Niri](https://github.com/YaLTeR/niri) (scrollable tiling, Wayland) |
| Desktop Shell | [Noctalia](https://github.com/noctalia-dev/noctalia-shell) (bar, dock, panels, notifications, lock screen) |
| File Manager | Thunar (`Super+E`) |
| Terminal | Kitty |
| Multiplexer | tmux (Ctrl-a prefix, lazygit/btop/fzf popups) |
| Shell | Zsh + Zinit + Starship |
| Editor | Neovim (lazy.nvim, 16 LSP servers, DAP, Treesitter) |
| AI / LLM | [LM Studio](https://lmstudio.ai) (local models) + [OpenCode](https://opencode.ai) (AI coding agent) |
| AI Security | [HexStrike AI](https://github.com/0x4m4/hexstrike-ai) MCP (150+ security tools via MCP) |
| Audio | PipeWire + WirePlumber (pipewire-pulse, pipewire-alsa, pipewire-jack) |
| Network | NetworkManager + iwd backend |
| Git | delta side-by-side diffs, 30+ aliases, lazygit TUI |
| Launcher | Noctalia app launcher (`Mod+Space` / `Mod+D`) + Fuzzel |
| Display Manager | [SDDM](https://github.com/sddm/sddm) (5 themes, cyberpunk default, `sddm-theme` switcher) |
| Theme | Tokyo Night (dark, transparent) + [pywal](https://github.com/dylanaraps/pywal) (wallpaper-driven colors) |
| Wallpapers | 23 curated Tokyo Night wallpapers (Arch, cyberpunk, Japanese art, minimal) |
| Fetch | fastfetch (system info on shell start) |

<img src="assets/divider.svg" alt="" width="900"/>

## Screenshots

<!-- Capture via niri: Print (region), Mod+Print (screen), Mod+Shift+Print (window).
     Drop PNGs in ./screenshots/ and reference them below. -->

<p align="center">
  <img src="screenshots/desktop.png" alt="Niri + Noctalia desktop (cafe-at-night wallpaper)" width="900"/>
</p>

| | |
|---|---|
| <img src="screenshots/noctalia-launcher.png" alt="Launcher" width="440"/> | <img src="screenshots/nvim.png" alt="Neovim" width="440"/> |
| <img src="screenshots/noctalia-control.png" alt="Control Center" width="440"/> | <img src="screenshots/terminal.png" alt="Terminal" width="440"/> |

<img src="assets/divider.svg" alt="" width="900"/>

## Quick Start

**One-liner (fresh Arch install):**

```bash
curl -fsSL https://raw.githubusercontent.com/foolish-dev/niri-dotfiles/main/bootstrap.sh | bash
```

**Manual install:**

```bash
git clone https://github.com/foolish-dev/niri-dotfiles.git ~/niri-dotfiles
cd ~/niri-dotfiles
./install.sh         # Arch only -- adds BlackArch + Chaotic AUR repos, installs 250+ packages
./deploy.sh          # symlinks all configs into ~/.config/
dotfiles-check       # verify the deploy took (failed units, broken symlinks)
```

First `nvim` launch auto-installs all plugins and LSP servers.

> **Git identity is automatic.** `deploy.sh` generates `~/.gitconfig.local` (git-ignored, per-machine) and prompts for your name + email. Override non-interactively with env vars:
>
> ```bash
> GIT_USER_NAME="Your Name" GIT_USER_EMAIL="you@example.com" ./deploy.sh
> ```
>
> Edit `~/.gitconfig.local` any time — it's `[include]`d by the tracked `.gitconfig`. Commit signing is opt-in (uncomment `signingkey` + `[commit] gpgsign`).

> **Per-machine zsh overrides.** The tracked `.zshrc` sources `~/.zshrc.local` (untracked) as its final step, so anything you put there wins. Use it for hardware-specific env (ROCm `HSA_OVERRIDE_GFX_VERSION`, CUDA paths), private aliases, work secrets, etc. Create it by hand — `deploy.sh` does not generate one.

<img src="assets/divider.svg" alt="" width="900"/>

## Layout

```text
.config/
  niri/config.kdl              compositor keybinds, layout, window rules
  noctalia/
    settings.json              bar, dock, panels, launcher settings
    colors.json                Tokyo Night material colors (pywal-managed)
  nvim/
    init.lua                   lazy.nvim bootstrap
    lua/config/                options, keymaps, autocmds
    lua/plugins/               colorscheme, treesitter, lsp, editor, ui, coding
  kitty/kitty.conf             terminal (0.92 opacity, pywal + Tokyo Night fallback)
  tmux/tmux.conf               multiplexer (vim nav, popups, Tokyo Night)
  lazygit/config.yml           git TUI (delta pager, Tokyo Night)
  fuzzel/fuzzel.ini            app launcher
  starship.toml                prompt
  opencode/opencode.json       AI agent config (LM Studio provider, HexStrike MCP)
  wal/templates/               pywal templates (kitty, noctalia)
  neofetch/                    system fetch display (fallback for fastfetch)
  systemd/user/                cliphist, awww, hexstrike-server services
.zshrc                         shell -- 80+ aliases, BlackArch tool shortcuts, pywal init
.gitconfig                     delta diffs, 30+ aliases, nvim mergetool
.gitignore_global              universal project ignores
.editorconfig                  per-language formatting rules
.local/bin/
  proj                         fuzzy project opener (fzf + tmux)
  mkproj                       scaffold projects (python/rust/go/c/node/shell)
  dev                          3-pane tmux IDE session
  gclone                       smart git clone (gh:user/repo shorthand)
  cheat                        quick reference sheets (nmap, ffuf, revshells, blackarch, privesc, ad, git, docker, tmux, nvim)
  wallpaper                    set wallpaper + regenerate pywal colors + reload kitty/noctalia
  hexstrike-mcp                MCP stdio bridge to HexStrike AI server
  sddm-theme                   fzf-powered SDDM theme switcher
  colorbars                    display full 256-color terminal palette
  colorblocks                  display basic 16-color palette
  pipes                        animated pipes screensaver
  dotfiles-check               post-deploy health check (failed units, symlinks)
.local/share/applications/     147 BlackArch .desktop entries (all 21 categories)
wallpapers/                    23 curated Tokyo Night wallpapers (4K)
assets/                        README SVG images (header, dividers, palette, architecture)
etc/
  sddm.conf.d/niri.conf       SDDM display manager config (deployed to /etc)
  sddm-themes/                 custom astronaut theme configs (tokyo-night, cyberpunk)
bootstrap.sh                   one-liner installer (curl | bash)
install.sh                     Arch + BlackArch + Chaotic AUR package bootstrap
deploy.sh                      symlink deployer with auto-backup
```

<img src="assets/divider.svg" alt="" width="900"/>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/keybinds.svg"/>
    <source media="(prefers-color-scheme: light)" srcset="assets/keybinds-light.svg"/>
    <img src="assets/keybinds.svg" alt="Keybinds section header — Niri, tmux, Neovim modifier key reference" width="900"/>
  </picture>
</p>

## Keybinds

### Niri (compositor)

| Key | Action |
|---|---|
| `Super+Space` | Noctalia app launcher (toggle) |
| `Super+D` | Noctalia app launcher |
| `Super+S` | Noctalia control center |
| `Super+Comma` | Noctalia settings |
| `Super+Return` | Terminal |
| `Super+Shift+Return` | Floating terminal |
| `Super+B` | Firefox |
| `Super+N` | Neovim |
| `Super+E` | File manager (Thunar) |
| `Super+H/J/K/L` | Focus window |
| `Super+Shift+H/J/K/L` | Move window |
| `Super+1-9` | Workspace |
| `Super+Tab` | Previous workspace |
| `Super+F` | Maximize column |
| `Super+Shift+F` | Fullscreen |
| `Super+R` | Cycle column width |
| `Super+Minus/Equal` | Shrink/grow column |
| `Super+[` / `Super+]` | Consume / expel window from column |
| `Super+Q` | Close window |
| `Super+V` | Clipboard history |
| `Super+Escape` | Lock screen |
| `Super+Shift+P` | Power off monitors |
| `Super+Shift+E` | Quit niri |
| `Super+Shift+C` | Reload niri config |
| `Super+Ctrl+M` | msfconsole |
| `Super+Ctrl+W` | Wireshark |
| `Super+Ctrl+B` | Burp Suite |
| `Super+Ctrl+N` | Nmap terminal |
| `Super+Ctrl+T` | btop |
| `Super+Ctrl+A` | LM Studio |
| `Print` | Screenshot region |
| `Super+Print` | Screenshot screen |
| `Super+Shift+Print` | Screenshot window |

### Tmux (prefix = Ctrl-a)

| Key | Action |
|---|---|
| `C-a \|` | Split horizontal |
| `C-a -` | Split vertical |
| `C-a h/j/k/l` | Navigate panes |
| `C-a H/J/K/L` | Resize pane |
| `C-a >`/`<` | Swap pane down/up |
| `C-a g` | Lazygit popup |
| `C-a b` | btop popup |
| `C-a f` | fzf file opener |
| `C-a c` | New window |
| `C-a C-h/C-l` | Prev/next window |
| `C-a Tab` | Last window |
| `C-a S` | New session |
| `C-a X` | Kill session |
| `C-a R` | Rename session |
| `C-a r` | Reload config |
| `C-a M-i` | IDE layout (main-vertical) |
| `C-a M-w` | Wide layout (main-horizontal) |
| `C-a M-g` | Grid layout (tiled) |

<img src="assets/divider.svg" alt="" width="900"/>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/neovim.svg"/>
    <source media="(prefers-color-scheme: light)" srcset="assets/neovim-light.svg"/>
    <img src="assets/neovim.svg" alt="Neovim section header — LSP, DAP, Treesitter, lazy.nvim stack" width="900"/>
  </picture>
</p>

## Neovim

**LSP servers** (auto-installed via Mason):
pyright, ruff, clangd, rust_analyzer, gopls, zls, ts_ls, bashls, lua_ls, html, cssls, jsonls, yamlls, dockerls, terraformls, tailwindcss

**DAP debuggers**: Python (debugpy), C/C++/Rust (GDB)

**Key plugins**: Telescope, Neo-tree, Gitsigns, Trouble, Bufferline, Lualine, Noice, nvim-cmp, LuaSnip, Conform (format-on-save), hex.nvim, rest.nvim, toggleterm, diffview

| Key | Action |
|---|---|
| `<leader>ff` | Find files |
| `<leader>fg` | Live grep |
| `<leader>fb` | Buffers |
| `<leader>fr` | Recent files |
| `<leader>fw` | Grep word under cursor |
| `<leader>t` | File tree |
| `<leader>xx` | Diagnostics (Trouble) |
| `<leader>cf` | Format buffer |
| `<leader>ca` | Code action |
| `<leader>rn` | Rename symbol |
| `gd` / `gr` | Go to definition / references |
| `K` | Hover docs |
| `<leader>db` | Toggle breakpoint |
| `<leader>dc` | Debug continue |
| `<leader>do` / `<leader>di` | Step over / step into |
| `<leader>du` | DAP UI toggle |
| `<leader>gg` | Git status (fugitive) |
| `<leader>gv` | Diffview |
| `<leader>gh` | File history |
| `]h` / `[h` | Next / prev git hunk |
| `<leader>hs` / `<leader>hp` | Stage / preview hunk |
| `<leader>xH` | Hex editor (hex.nvim) |
| `<leader>xh` / `<leader>xr` | xxd hex view / revert |
| `<leader>mp` | Markdown preview |
| `<leader>rr` / `<leader>rl` | Run HTTP request / Re-run last (rest.nvim) |
| `<C-\>` / `<leader>tf` | Float terminal |
| `<leader>th` / `<leader>tv` | Terminal horizontal / vertical |

<img src="assets/divider.svg" alt="" width="900"/>

## LM Studio

Local LLM inference via [LM Studio](https://lmstudio.ai). The installer pulls `lmstudio-bin` from Chaotic AUR.

OpenCode is pre-configured as a provider (`http://127.0.0.1:1234/v1`) with starter models in `.config/opencode/opencode.json`. Load a model in LM Studio, start the server, then use `/models` in OpenCode.

```bash
lms --help             # real LM Studio CLI (on $PATH via ~/.lmstudio/bin)
lms ls                 # list installed models
lms load <model>       # load a model

lmsgui                 # open the LM Studio GUI
lms-server             # start headless API server on :1234 (wrapper)
lms-stop               # stop the server
lms-status             # curl /v1/models and list loaded IDs
lms-chat               # quick /v1/chat/completions smoke test
```

<img src="assets/divider.svg" alt="" width="900"/>

## HexStrike AI MCP

[HexStrike AI](https://github.com/0x4m4/hexstrike-ai) exposes 150+ offensive security tools to AI agents via MCP.

The installer clones the repo to `~/tools/hexstrike-ai`, sets up a Python venv, and enables a systemd user service (`hexstrike-server.service`) running the Flask backend on `127.0.0.1:8888`. OpenCode connects through the `hexstrike-mcp` stdio bridge.

```bash
sysu status hexstrike-server   # check service status
sysu restart hexstrike-server  # restart the backend
```

<img src="assets/divider.svg" alt="" width="900"/>

## OpenCode Agents

OpenCode is configured in `.config/opencode/opencode.json` with local providers (LM Studio + Ollama), the HexStrike AI MCP, Context7, and a weather MCP demo server. Agents live in `.config/opencode/agent/`.

| Agent | Purpose |
|---|---|
| `hexstrike-analyst-context7` | Authorized pentest / CTF recon via HexStrike + Context7 docs |
| `superclaude-architect-context7` | Large-codebase architect with Context7 lookups |
| `build` | Build systems, CI/CD, infrastructure code |
| `analyze` | Security-focused code review |
| `docs` | Documentation-writing agent (from heimdall_opencode) |
| `git-committer` | Conventional commits + push helper (from heimdall_opencode) |

The [`SuperClaude-Org/heimdall_opencode`](https://github.com/SuperClaude-Org/heimdall_opencode) fork ships curated agents and MCP wiring. It's vendored as a pinned git submodule at `.config/opencode/heimdall_opencode/` so the agent pack travels with the repo. `bootstrap.sh` clones with `--recurse-submodules --shallow-submodules`; `install.sh` re-inits the submodule as a safety net for existing clones.

```bash
# Refresh to the latest upstream heimdall commit on its dev branch
git submodule update --remote --merge .config/opencode/heimdall_opencode
```

<img src="assets/divider.svg" alt="" width="900"/>

## Pywal

[pywal](https://github.com/dylanaraps/pywal) generates a color scheme from your wallpaper and applies it to kitty, noctalia, and the terminal.

```bash
wallpaper ~/Pictures/bg.jpg    # set wallpaper + regenerate all colors
```

The `wallpaper` script:
1. Runs `wal -i` to generate colors
2. Sets the wallpaper via `awww img` with a grow transition
3. Live-reloads kitty colors
4. Copies the generated `colors-noctalia.json` into noctalia's config

Tokyo Night is the static fallback before the first `wal` run.

<img src="assets/divider.svg" alt="" width="900"/>

## SDDM Themes

Five SDDM themes are installed, with two custom Tokyo Night configs for the [astronaut](https://github.com/Keyitdev/sddm-astronaut-theme) theme (Qt6):

| Theme | Style |
|---|---|
| `noctalia` | Built for Noctalia Shell, follows shell wallpaper via `sync-shell-wallpaper.sh` **(default)** |
| `cyberpunk` | Built-in astronaut variant |
| `tokyo-night` | Tokyo Night palette, blurred form, JetBrainsMono, samurai background |
| `tokyo-night-cyberpunk` | Neon red/cyan accents, sharp edges, matrix background |
| `astronaut` | Default astronaut (space theme) |
| `japanese_aesthetic` | Built-in astronaut variant |
| `tokyo-night-sddm` | Original Tokyo Night SDDM (Qt5) |
| `sddm-sugar-dark` | Clean minimal dark |
| `sddm-lain-wired` | Serial Experiments Lain / cyberpunk |

```bash
sddm-theme                     # fzf picker for all installed themes
sddm-theme tokyo-night         # switch directly
sddm-theme cyberpunk            # any astronaut variant or standalone theme
```

The switcher handles config deployment, wallpaper copying, and SDDM metadata patching. Customizations are applied to an upgrade-proof local copy at `/usr/share/sddm/themes/sddm-astronaut-local/`, created by `deploy.sh` — so `pacman -Syu` on `sddm-astronaut-theme` can't clobber your variant selection or backgrounds. To refresh against upstream changes, `sudo rm -rf /usr/share/sddm/themes/sddm-astronaut-local && ./deploy.sh`.

<img src="assets/divider.svg" alt="" width="900"/>

## Wallpapers

23 curated Tokyo Night wallpapers are bundled in `wallpapers/` and symlinked to `~/Pictures/Wallpapers/` by `deploy.sh`:

| Category | Wallpapers |
|---|---|
| Arch / Distro | `arch-night`, `arch-night-alt`, `arch-czechbol`, `kali` |
| Dev / Minimal | `neovim`, `rust`, `python`, `c-lang`, `vim`, `stripes` |
| Japanese Art | `samurai`, `dragon`, `hannya` |
| Cyber / Tech | `matrix`, `tron`, `keyboard`, `heroes` |
| Cityscapes | `cosmic-neo-tokyo`, `cosmic-tokyo`, `tokyonight-skyline`, `cafe-at-night` |
| Abstract | `abstract-lock`, `fly` |

```bash
wallpaper ~/Pictures/Wallpapers/samurai.png    # set + regenerate pywal colors
```

Sourced from [tokyo-night/wallpapers](https://github.com/tokyo-night/wallpapers) (MIT), [czechbol/tokyonight-backgrounds](https://github.com/czechbol/tokyonight-backgrounds) (CC), and [atraxsrc/tokyonight-wallpapers](https://github.com/atraxsrc/tokyonight-wallpapers) (GPL-2.0).

<img src="assets/divider.svg" alt="" width="900"/>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/launcher.svg"/>
    <source media="(prefers-color-scheme: light)" srcset="assets/launcher-light.svg"/>
    <img src="assets/launcher.svg" alt="Noctalia launcher section header — 147 pinned apps across 21 categories" width="900"/>
  </picture>
</p>

## Noctalia Dock & Launcher

The Noctalia dock and app launcher are pre-configured with pinned security tools. Press `Super` (tap) or `Super+D` to open the launcher.

**Dock** (bottom bar): Kitty, Firefox, Thunar, LM Studio, Wireshark, Burp Suite, Metasploit, Nmap, Iaito, Autopsy, btop

**Launcher** (147 pinned apps across 21 categories, ordered by workflow):

| Group | Apps |
|---|---|
| Core | Kitty, Firefox, Neovim, LM Studio |
| Web Testing | Wireshark, Burp Suite, ZAP, Mitmproxy |
| Recon / OSINT | theHarvester, Sherlock, Recon-ng, SpiderFoot, Katana, GAU, Waybackurls, Hakrawler, GoSpider, DMitry, Legion, Maltego |
| DNS / Subdomain | DNSenum, MassDNS, ShuffleDNS, PureDNS, DNSx, AlterX, Naabu, ASNmap, MapCIDR, CDNcheck, Fierce, DNSmap |
| Web Exploitation | Nikto, WPScan, Commix, Dalfox, Arjun, JWT Tool, NoSQLMap, GraphQLmap, Skipfish, Cadaver, Dirsearch, Wfuzz, Ffuf, FeroxBuster, Dirb, Gobuster |
| Scanning | Nmap, SQLMap |
| Exploitation | Metasploit, Empire, BeEF, Sliver, RouterSploit, SearchSploit, Veil, MSFvenom |
| AD / Windows | NetExec, Kerbrute, BloodHound, Evil-WinRM, Certipy, Enum4linux-ng, BloodyAD, Ldeep, Windapsearch, Coercer, Manspider, SCCMHunter, Nishang |
| Passwords | Hydra, Hashcat, John, CeWL, Crunch, Medusa, Patator, CUPP, Changeme, Name-That-Hash |
| Wireless | Responder, Bettercap, Wifite, Reaver, Fluxion, Airgeddon, Yersinia, Aircrack-ng |
| Privesc / Post | LinPEAS, WinPEAS, Pspy, Mimikatz, Chisel, Ligolo-ng, Pwncat, Rubeus, SharpHound |
| Reversing | Iaito, Ghidra, Cutter, Rizin, Radare2, Angr, Ropper, RetDec |
| Mobile | MobSF, JADX, APKTool, Objection, Drozer, APKLeaks |
| Forensics | Autopsy, Volatility3, YARA, Binwalk, Foremost, Bulk Extractor, OLEtools, RegRipper, PDF Parser |
| Networking / MITM | Ettercap, dSniff, SSLstrip, Hping3, SNMPcheck, Tcpdump, dns2tcp, Iodine, Ptunnel, Proxify |
| Social Engineering | SET, GoPhish, Evilginx2, King Phisher |
| Crypto | HashPump, RsaCtfTool, Xortool, FeatherDuster |
| Steganography | StegSeek, Steghide, Zsteg, StegSolve, OpenStego, Snow |
| Fuzzing | AFL++, Boofuzz, Radamsa, zzuf |
| Secret Scanning | TruffleHog, Gitleaks, Interactsh |
| Cloud Security | Pacu, ScoutSuite, Prowler |

147 custom `.desktop` entries in `.local/share/applications/` provide launcher integration for terminal-based BlackArch tools. Each opens in Kitty with a usage hint.

<img src="assets/divider.svg" alt="" width="900"/>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/blackarch.svg"/>
    <source media="(prefers-color-scheme: light)" srcset="assets/blackarch-light.svg"/>
    <img src="assets/blackarch.svg" alt="BlackArch section header — 300+ offensive security tools across 21 categories" width="900"/>
  </picture>
</p>

## BlackArch Tools

The installer adds the BlackArch repository and pulls 300+ tools across 19 categories:

| Category | Tools |
|---|---|
| Recon / OSINT | theharvester, sherlock, recon-ng, spiderfoot, katana, gau, waybackurls, hakrawler, gospider, dmitry, legion, maltego |
| DNS / Subdomain | dnsenum, massdns, shuffledns, puredns, dnsx, alterx, naabu, asnmap, mapcidr, cdncheck, fierce, dnsmap |
| Web | wpscan, commix, dalfox, arjun, jwt-tool, nosqlmap, graphqlmap, zaproxy, skipfish, cadaver, dirsearch, wfuzz |
| Exploitation | evil-winrm, sliver, empire, netexec, beef-xss, routersploit, searchsploit, veil, unicorn-powershell |
| Active Directory | kerbrute, certipy, enum4linux-ng, bloodyad, ldeep, windapsearch, coercer, manspider, sccmhunter, nishang |
| Passwords | hashcat-utils, hcxtools, cewl, crunch, medusa, patator, cupp, changeme, rsmangler, name-that-hash |
| Wireless | bettercap, wifite, reaver, fluxion, airgeddon, yersinia |
| Privesc / Post | linpeas, winpeas, pspy, mimikatz, bloodhound, chisel, ligolo-ng, pwncat, rubeus, sharphound |
| Reversing | rizin, cutter, angr, ropper, one_gadget, retdec, ghidra, radare2 |
| Mobile | apktool, jadx, dex2jar, objection, drozer, apkleaks, mobsf |
| Forensics | autopsy, yara, volatility3, bulk-extractor, oletools, regripper, pdf-parser |
| Networking / MITM | mitmproxy, ettercap, dsniff, sslstrip, dns2tcp, iodine, ptunnel, hping3, snmpcheck |
| Social Eng | SET, gophish, evilginx2, king-phisher |
| Crypto | hashpump, rsactftool, xortool, featherduster |
| Stego | stegseek, zsteg, stegsolve, openstego, snow |
| Fuzzing | afl++, boofuzz, radamsa, zzuf |
| Secret Scanning | trufflehog, gitleaks, interactsh, proxify |
| Cloud | pacu, scoutsuite, prowler |
| Wordlists | seclists, payloadsallthethings, fuzzdb, rockyou, dirbuster-wordlists |

<img src="assets/divider.svg" alt="" width="900"/>

## Chaotic AUR

[Chaotic AUR](https://aur.chaotic.cx) provides pre-built AUR packages so you skip compilation. The installer adds the repo automatically (GPG key + mirrorlist + pacman.conf entry). Packages like `lmstudio-bin`, `opencode-bin`, and many others install instantly via pacman.

<img src="assets/divider.svg" alt="" width="900"/>

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/terminal.svg"/>
    <source media="(prefers-color-scheme: light)" srcset="assets/terminal-light.svg"/>
    <img src="assets/terminal.svg" alt="Terminal section header — Kitty, tmux, zsh, zinit, starship stack" width="900"/>
  </picture>
</p>

## Zsh Security Toolkit

```bash
# Quick reference
cheat blackarch                    # BlackArch tools overview
cheat privesc                      # Privilege escalation
cheat ad                           # Active Directory attacks
cheat revshells                    # Reverse shell one-liners
cheat nmap                         # Nmap scanning
cheat ffuf                         # Web fuzzing
cheat git                          # Git workflows
cheat docker                       # Docker commands
cheat tmux                         # tmux keybinds
cheat nvim                         # Neovim keybinds

# Functions
revshell 10.10.14.1 4444           # Generate bash/python/nc/ps reverse shells
listen 4444                        # nc listener
serve 8000                         # Python HTTP server
recon example.com                  # Whois + DNS summary
quickscan 10.10.10.0/24            # Ping sweep
hashid <hash>                      # Identify hash type

# Dev scripts
proj                               # Fuzzy open a project in tmux
mkproj myapp python                # Scaffold a Python project
dev                                # Launch 3-pane tmux IDE
gclone user/repo                   # Clone from GitHub + cd
gclone gh:user/repo                # GitHub shorthand
gclone gl:user/repo                # GitLab shorthand

# Pywal / theming
wallpaper ~/Pictures/bg.jpg        # Set wallpaper + regenerate colors
sddm-theme                         # fzf picker for all installed themes
sddm-theme tokyo-night             # switch SDDM theme directly

# Terminal utilities
colorbars                          # Show full 256-color palette
colorblocks                        # Show basic 16-color palette
pipes                              # Animated pipes screensaver

# LM Studio
lms                                # Open LM Studio
lms-server                         # Start local API server
lms-status                         # Show loaded models

# Aliases (samples)
nmap-stealth 10.10.10.1            # SYN scan, fragmented
msf                                # msfconsole -q
wpscan-enum http://target          # WordPress full enum
evilwinrm -i IP -u user -p pass   # WinRM shell
lg                                 # lazygit
```

## Color Palette

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/palette.svg"/>
  <source media="(prefers-color-scheme: light)" srcset="assets/palette-light.svg"/>
  <img src="assets/palette.svg" alt="Tokyo Night palette — bg, comment, blue, cyan, purple, green, yellow, red swatches with hex codes" width="900"/>
</picture>

<img src="assets/divider.svg" alt="" width="900"/>

<p align="center">
  <sub>MIT License &copy; <a href="https://github.com/foolish-dev">foolish-dev</a></sub>
</p>
