use crate::distro::{chaotic_aur_policy, setup_blackarch, setup_chaotic_aur, ChaoticAur, Distro};
use crate::hardware::{
    amdxdna_memlock_conf, asus_pack, memlock_user, npu_pack, pci_devices, platform_devices,
    AsusPack, NpuPack,
};
use crate::hexstrike::setup_hexstrike;
use crate::login::{setup_noctalia_login, CURSOR_THEME_PKG};
use crate::packages::{
    aur_helper, ensure_aur_helper, ensure_aur_pkg, ensure_cargo_git, ensure_noctalia_auth_agent,
    ensure_pacman, ensure_pacman_pkg, ensure_pkgfile_db, ensure_rust_analyzer, optional_pacman,
    optional_pacman_pkg, GROGU_MIN_VERSION,
};
use crate::util::{command_exists, info, ok, sudo_write, warn};
use anyhow::{anyhow, Result};

// ── install ───────────────────────────────────────────────────────────────────

pub(crate) fn install(distro: Distro, no_aur_helper: bool) -> Result<()> {
    if !command_exists("cargo") {
        return Err(anyhow!(
            "cargo not found — install rust first (https://rustup.rs)"
        ));
    }
    info(&format!("Distro: {}", distro.label()));

    // Prerequisites assumed by later steps. Idempotent — `ensure_pacman`
    // is a no-op when the binary is already on PATH.
    //
    // Only `git` is fatal, and it is fatal here rather than at its uses so the
    // run dies at step two with nothing half-done: every source install below
    // (grogu, teleia, the HexStrike clone, the auth agent's makepkg) shells out
    // to it, and so does dotctl's own update path. That path is also why the
    // older "git → ensure_repo" note here was backwards: `main` runs
    // `ensure_repo(&repo)?` BEFORE `install()`, so a `dotctl all` on a box with
    // no git dies spawning `git clone` long before this line could install it.
    // This line covers `dotctl install` on its own.
    //   curl   → setup_blackarch fetches strap.sh (itself warn-and-continue).
    //            Never actually fires on a pacman host: pacman depends on curl.
    //   python → the HexStrike venv only (the Arch-family package is `python`,
    //            the binary is `python3`), and that whole step now warns.
    ensure_pacman("git", "git", &["git"])?;
    optional_pacman(
        "curl",
        "curl",
        &["curl"],
        "the BlackArch repo step will be skipped",
    );
    optional_pacman(
        "Python 3",
        "python3",
        &["python"],
        "HexStrike AI cannot create its venv",
    );

    // Niri base desktop — compositor, terminal, launcher, clipboard persistence.
    // All four are in Arch `extra` (and rebuilt in cachyos-extra-znver4 on
    // CachyOS). wl-clip-persist keeps clipboard contents alive after the
    // source window closes; it was routed through the AUR helper here for a
    // long time, but it has been a plain repo package on both distros all
    // along (extra 0.5.0-2, ships /usr/bin/wl-clip-persist, and is in no AUR
    // at all) — so on a helper-less box, which is every fresh CachyOS install,
    // it silently never installed.
    ensure_pacman("niri", "niri", &["niri"])?;
    ensure_pacman("fuzzel", "fuzzel", &["fuzzel"])?;
    ensure_pacman("kitty", "kitty", &["kitty"])?;
    // wl-clipboard is what actually provides wl-copy/wl-paste, and nothing here
    // installed it. config.kdl's `wl-paste --watch cliphist store` startup spawn
    // and the Mod+V pipe both need it; it was present on this box only because
    // CachyOS's niri meta-package happens to pull it in. wl-clip-persist is a
    // different package that depends on neither, so having it proved nothing.
    optional_pacman(
        "wl-clipboard",
        "wl-paste",
        &["wl-clipboard"],
        "config.kdl's `wl-paste --watch` spawn and the Mod+V pipe will do nothing",
    );
    optional_pacman(
        "wl-clip-persist",
        "wl-clip-persist",
        &["wl-clip-persist"],
        "clipboard contents vanish when the source window closes",
    );
    // The history store behind that same spawn and Mod+V — advertised by
    // config.kdl, installed by nobody, and absent from PATH here, so Mod+V has
    // always opened an empty fuzzel. Plain repo package on both distros
    // (extra 1:0.7.0-2); it is in no AUR, so no helper is involved.
    optional_pacman(
        "cliphist",
        "cliphist",
        &["cliphist"],
        "Mod+V opens an empty fuzzel",
    );

    // Fonts. kitty.conf, fuzzel.ini, qt5ct.conf and qt6ct.conf every one pin
    // `JetBrainsMono Nerd Font` by name, and no font package was installed at
    // all — so fontconfig substituted silently, with no error anywhere:
    // `fc-list | grep -ci "jetbrainsmono nerd"` returns 0 here and
    // `fc-match "JetBrainsMono Nerd Font"` answers NotoSansMono-Regular, which
    // carries none of the powerline/Nerd glyphs starship, tmux and fastfetch
    // draw with. Gated by package, not binary: a font ships no executable, so a
    // `command_exists` gate would miss forever and re-run pacman every install.
    optional_pacman_pkg(
        "JetBrainsMono Nerd Font",
        "ttf-jetbrains-mono-nerd",
        &["ttf-jetbrains-mono-nerd"],
        "fontconfig substitutes a font with no powerline/Nerd glyphs",
    );
    // The rest of the theme names the tracked configs hand to GTK, Qt and
    // fuzzel. Same silent-fallback class as the font above and the cursor
    // below, and just as invisible: fontconfig, GTK and Qt every one substitute
    // without a word on stdout or in any log.
    //   Papirus-Dark   `gtk-icon-theme-name` in .config/gtk-3.0/settings.ini and
    //                  .config/gtk-4.0/settings.ini, `icon_theme` in
    //                  .config/qt5ct/qt5ct.conf and .config/qt6ct/qt6ct.conf,
    //                  `icon-theme` in .config/fuzzel/fuzzel.ini — five tracked
    //                  files. /usr/share/icons on this box holds only Adwaita,
    //                  AdwaitaLegacy, capitaine-cursors, default, hicolor and
    //                  locolor, so all five have been falling back. Shipped by
    //                  extra/papirus-icon-theme, whose payload is exactly
    //                  Papirus, Papirus-Dark and Papirus-Light.
    //   Cantarell      `gtk-font-name` in both settings.ini and the `general`
    //                  font in both qt*ct.conf. extra/cantarell-fonts. Present
    //                  on this machine only because someone installed it by
    //                  hand — `pactree -r cantarell-fonts` names nothing but
    //                  itself, so no dependency chain brings it to a fresh box.
    //   qt5ct / qt6ct  .config/niri/config.kdl exports QT_QPA_PLATFORMTHEME and
    //                  this repo tracks a config file for each, but neither
    //                  package was installed — so the platformtheme plugin Qt
    //                  would load (libqt5ct.so / libqt6ct.so) does not exist
    //                  and both tracked files are dead text. They also own the
    //                  palettes both files point `color_scheme_path` at,
    //                  /usr/share/qt{5,6}ct/colors/darker.conf.
    //   Kvantum        `style=kvantum-dark` in both qt*ct.conf. That style key
    //                  is registered by the Kvantum plugin and by nothing else
    //                  — `strings libkvantum.so` lists exactly `kvantum` and
    //                  `kvantum-dark` — with the Qt6 plugin in `kvantum` and
    //                  the Qt5 one in `kvantum-qt5`, so both are needed to
    //                  cover both config files.
    // All of these are plain repo packages: Arch `extra` with
    // cachyos-extra-znver4 rebuilds, no helper and no distro branch. Gated by
    // package rather than binary wherever the package ships no command of its
    // own — an icon theme and a font ship none at all, and Kvantum's binaries
    // (kvantummanager, kvantumpreview) are named after neither package.
    optional_pacman_pkg(
        "Papirus icon theme",
        "papirus-icon-theme",
        &["papirus-icon-theme"],
        "GTK, Qt and fuzzel fall back to Adwaita/hicolor icons",
    );
    optional_pacman_pkg(
        "Cantarell",
        "cantarell-fonts",
        &["cantarell-fonts"],
        "fontconfig substitutes the GTK and Qt interface font",
    );
    optional_pacman(
        "qt5ct",
        "qt5ct",
        &["qt5ct"],
        "QT_QPA_PLATFORMTHEME names a plugin that is not installed, so qt5ct.conf is ignored",
    );
    optional_pacman(
        "qt6ct",
        "qt6ct",
        &["qt6ct"],
        "qt6ct.conf is ignored and Qt6 apps keep the default palette",
    );
    optional_pacman_pkg(
        "Kvantum",
        "kvantum",
        &["kvantum", "kvantum-qt5"],
        "`style=kvantum-dark` in qt5ct.conf and qt6ct.conf resolves to nothing and Qt keeps Fusion",
    );

    // Spawned at startup by config.kdl and in exactly the same boat as
    // wl-clipboard: Arch `extra`, never installed here, present on this box only
    // as a dependency of the CachyOS niri meta-package. Without it every
    // XWayland client has no server to connect to.
    optional_pacman(
        "xwayland-satellite",
        "xwayland-satellite",
        &["xwayland-satellite"],
        "X11 clients have no server to connect to (Wayland clients are unaffected)",
    );

    // Media and brightness keys. config.kdl binds XF86Audio* to `wpctl` and
    // XF86MonBrightness* to `brightnessctl`; both are in `extra`, neither was
    // installed, and both are here only via that same meta-package — so on
    // stock Arch every volume and brightness key was dead. `wpctl` is
    // wireplumber's binary, so the package name and the gate deliberately differ.
    optional_pacman(
        "wireplumber",
        "wpctl",
        &["wireplumber"],
        "the XF86Audio* keys stay dead",
    );
    optional_pacman(
        "brightnessctl",
        "brightnessctl",
        &["brightnessctl"],
        "the XF86MonBrightness* keys stay dead",
    );

    // `.zshrc` is tracked and deployed and Mod+Ctrl+N drops into `zsh` by name,
    // but nothing ever installed the shell itself.
    optional_pacman(
        "zsh",
        "zsh",
        &["zsh"],
        "Mod+Ctrl+N fails and the deployed .zshrc stays inert",
    );

    // Resolve (or bootstrap) an AUR helper before the first AUR package. On
    // CachyOS this installs one straight from [cachyos]; on stock Arch it can
    // only succeed once setup_chaotic_aur further down has run, so we ask
    // again there rather than reorder the existing pacman calls.
    let mut helper = if no_aur_helper {
        info("--no-aur-helper: leaving AUR helper availability as-is");
        aur_helper()
    } else {
        ensure_aur_helper(distro)
    };

    // ROG Flow Z13 GZ302EA hardware extras (see `gz302ea-pack bringup`). Detected
    // by package name, not binary: XRT installs to /opt/xilinx (not on PATH).
    //   NPU userspace — the in-tree amdxdna driver ships with the kernel; this is
    //     the XRT + FastFlowLM userspace. Model loads need unlimited memlock.
    //   Tablet auto-rotation — iio-sensor-proxy (D-Bus activated) + the iio-niri
    //     bridge (AUR) that feeds orientation to niri.
    //   On-screen keyboard — wvkbd (AUR); the Mod+O keybind is already in niri.
    // All three pacman packages here are in Arch `extra` with cachyos-extra-znver4
    // rebuilds — correct on both distros, no distro branch wanted.
    //
    // The three NPU steps are gated on an XDNA device actually being on the PCI
    // bus, and warn-and-continue rather than `?` — like the chaotic-aur,
    // blackarch and fwupd steps. They used to run on every host: `xrt` is the
    // Xilinx FPGA/AIE runtime, and both installs were hard `?`, so on any box
    // without the NPU — every reader the README invites in — a resolution
    // failure on a package that does nothing for it aborted install(), and
    // under `dotctl all` the `install(...)?` in `main` meant deploy() never ran
    // and the user was left with no configs at all. The limits drop-in was
    // worse than fatal: a system-wide locked-memory policy written into /etc
    // for hardware the machine does not have.
    match npu_pack(&pci_devices()) {
        NpuPack::Install => {
            if let Err(e) = ensure_pacman_pkg("XRT (NPU)", "xrt", &["xrt", "xrt-plugin-amdxdna"]) {
                warn(&format!(
                    "XRT not installed: {e} — the NPU is present but has no userspace runtime"
                ));
            }
            if let Err(e) = ensure_pacman_pkg("FastFlowLM", "fastflowlm", &["fastflowlm"]) {
                warn(&format!(
                    "FastFlowLM not installed: {e} — no NPU model runner"
                ));
            }
            // A pam_limits drop-in, read at login. See `amdxdna_memlock_conf`
            // for what it is for and why it is scoped to one account rather
            // than the `*` this used to write.
            let sudo_user = std::env::var("SUDO_USER").unwrap_or_default();
            let login_user = std::env::var("USER").unwrap_or_default();
            match memlock_user(Some(&sudo_user), Some(&login_user)) {
                Some(user) => {
                    if let Err(e) = sudo_write(
                        "/etc/security/limits.d/99-amdxdna.conf",
                        &amdxdna_memlock_conf(user),
                    ) {
                        warn(&format!(
                            "memlock limit for the NPU not written: {e} — NPU model loads may fail with ENOMEM"
                        ));
                    }
                }
                // Only reachable as root with no SUDO_USER, where the grant
                // would be a no-op: CAP_IPC_LOCK already exempts root.
                None => info("Running as root — skipping the NPU memlock drop-in"),
            }
        }
        NpuPack::Skip => info(
            "No AMD XDNA NPU on this host — skipping XRT/FastFlowLM and the amdxdna memlock policy",
        ),
    }
    ensure_pacman_pkg(
        "iio-sensor-proxy",
        "iio-sensor-proxy",
        &["iio-sensor-proxy"],
    )?;
    // The two AUR members of this pack are deferred to after the chaotic-aur
    // bootstrap below — see the note there.

    // Chaotic AUR. On stock Arch this is dotctl's only source for an AUR
    // helper — neither yay nor paru is in any Arch official repo. Skipped on
    // CachyOS, where [cachyos] already carries yay and paru and where adding a
    // lower-priority third-party repo only widens the versioned-dependency
    // fallthrough surface for nothing.
    // Warn-and-continue, not `?`. Both of these reach the network for a
    // third-party key and CDN, and neither is a dependency of anything below:
    // chaotic-aur exists only as an AUR-helper source (and `ensure_aur_helper`
    // already degrades gracefully without one), while nothing in `install()`
    // pulls a single package out of blackarch. A five-second keyserver hiccup
    // used to abort the whole run here — after niri/kitty/fuzzel but before
    // grogu, nvim, noctalia, the greeter and HexStrike — and under
    // `dotctl all` the `install(...)?` in `main` meant `deploy()` never ran
    // either, so the user was left with a half-installed desktop and no
    // configs at all. Every other optional component in this function already
    // warns and carries on; these two were the outliers.
    match chaotic_aur_policy(distro) {
        ChaoticAur::Add => {
            if let Err(e) = setup_chaotic_aur() {
                warn(&format!(
                    "Chaotic AUR setup failed: {e} — continuing; AUR-only add-ons may be skipped"
                ));
            }
        }
        ChaoticAur::SkipRedundant => {
            ok("Chaotic AUR not needed on CachyOS — [cachyos] already provides yay/paru")
        }
    }

    // Second chance for the helper: on stock Arch the repo that can supply one
    // only came into existence a few lines ago.
    if helper.is_none() && !no_aur_helper {
        helper = ensure_aur_helper(distro);
    }

    // GZ302EA AUR add-ons, deliberately down here rather than beside the
    // pacman half of the same hardware pack. They used to sit up there and so
    // consumed the *first* helper resolution, which on stock Arch always
    // answers `None` — neither yay nor paru is in any Arch official repo, and
    // chaotic-aur, dotctl's only source for one, was configured twenty lines
    // later. A fresh `dotctl all` therefore printed "skipped — no AUR helper
    // available" for both, bootstrapped yay immediately afterwards, and never
    // came back: Mod+O did nothing and tablet rotation was dead until the user
    // happened to run `dotctl install` a second time. CachyOS hid it, because
    // there the first ask already succeeds from [cachyos].
    //
    // Neither of these two is gated the way the ASUS pack below is, and neither
    // should be. wvkbd is a plain wlroots on-screen keyboard — no vendor, no
    // driver, no daemon, ~200 KB that sits on disk until Mod+O spawns it — and
    // it is as useful on any touchscreen or pen device as it is here.
    // iio-niri is the narrower of the two (it is inert without an IIO
    // accelerometer, which `/sys/bus/iio/devices` would report), but it ships a
    // single binary, no unit and no service, its dependencies are dbus,
    // iio-sensor-proxy and niri — all already installed by this point — and its
    // idle cost on a host with no sensor is zero. Gating buys nothing there;
    // the ASUS pack is gated because it pulls a vendor daemon that a udev rule
    // then starts.
    ensure_aur_pkg(helper, "iio-niri", "iio-niri")?;
    ensure_aur_pkg(helper, "wvkbd", "wvkbd")?;
    // The Armoury Crate key (XF86Launch1) is bound to `rog-control-center` in
    // config.kdl and nothing installed it — missing from PATH here too, so that
    // key has never done anything.
    //
    // Gated on the ASUS platform device, because it was not: this pulls in
    // `asusctl`, an ASUS vendor daemon that the package's own udev rule starts
    // at boot, and the ungated call installed it on every host the README
    // invites in — the same bug the NPU pack above was fixed for one commit
    // later. See [`asus_pack`] for why the gate is the platform device rather
    // than DMI.
    //
    // `ensure_aur_pkg` rather than `optional_pacman_pkg` because the AUR is the
    // only source guaranteed to be reachable — but NOT for the reason the note
    // here used to give. It is not AUR-only on either distro: [cachyos] carries
    // asusctl/rog-control-center 6.4.0-1 and chaotic-aur carries 6.3.8-1. What
    // is true is that no Arch official repo carries either, and on stock Arch
    // chaotic-aur is configured a few lines above by a step that only warns on
    // failure. A helper takes a repo build wherever one exists and falls back
    // to the AUR (pkgbase `asusctl`, which splits both names) where none does.
    match asus_pack(&platform_devices()) {
        AsusPack::Install => ensure_aur_pkg(helper, "rog-control-center", "rog-control-center")?,
        AsusPack::Skip => info(
            "No ASUS notebook WMI platform device on this host — skipping rog-control-center \
             and the asusctl daemon it depends on (the XF86Launch1 bind stays dead)",
        ),
    }

    // BlackArch (2800+ offensive-security tools, paired with HexStrike AI)
    if let Err(e) = setup_blackarch() {
        warn(&format!(
            "BlackArch repo setup failed: {e} — continuing; `pacman -S <tool>` will not reach \
             BlackArch until you rerun `dotctl install`"
        ));
    }

    // grogu
    ensure_cargo_git(
        "grogu (wallpaper-driven theme propagator)",
        "grogu",
        "https://github.com/foolish-dev/grogu",
        "wallpaper changes will not repaint the theme until you rerun `dotctl install`",
        Some(GROGU_MIN_VERSION),
    );

    // teleia — the TUI agent Mod+T spawns, and the owner of the tracked
    // .config/teleia/config.toml that wires its MCP servers. Both the bind and
    // the config were deployed by dotctl while the binary was installed by
    // nobody: it exists on this machine only as an untracked 11MB file someone
    // dropped into ~/.local/bin by hand, so a fresh clone got a keybind that
    // opens a kitty window and closes it again. Same source-install shape as
    // grogu above, for the same reason — our own crate, not packaged anywhere.
    ensure_cargo_git(
        "teleia (TUI coding agent)",
        "teleia",
        "https://github.com/foolish-dev/teleia",
        "Mod+T opens a kitty window that closes again until you rerun `dotctl install`",
        None,
    );
    // The language servers the tracked config's `[lsps.*]` tables name. teleia
    // spawns each only where its root_patterns match and reports a missing
    // binary through `/lsps`, so neither is worth stopping the run for.
    // clangd ships in `clang` (extra on Arch, cachyos-znver4 on CachyOS).
    optional_pacman(
        "clangd (teleia's C/C++ language server)",
        "clangd",
        &["clang"],
        "teleia's `c` language server fails to start in C/C++ projects",
    );
    ensure_rust_analyzer();
    // Most of the MCP servers in the tracked teleia and opencode configs launch
    // through `npx`, and nothing installed node: on this machine every one of
    // them failed to start, leaving teleia with HexStrike as its only server.
    // `npm` ships npx; `nodejs` is its runtime.
    optional_pacman(
        "Node.js + npm (npx runs teleia's MCP servers)",
        "npx",
        &["nodejs", "npm"],
        "every npx-launched MCP server in the teleia and opencode configs fails to start",
    );
    // The GitHub MCP server, the one that holds GITHUB_PERSONAL_ACCESS_TOKEN.
    // It used to be the npm `@modelcontextprotocol/server-github`, which
    // upstream archived as unmaintained with "no security guarantees"; GitHub's
    // own server replaced it. The -bin package installs GitHub's release binary
    // against a pinned checksum. Gated on the binary, so a copy installed any
    // other way is left alone rather than fought over with a conflicting package.
    if command_exists("github-mcp-server") {
        ok("github-mcp-server already installed");
    } else {
        ensure_aur_pkg(helper, "GitHub MCP server", "github-mcp-server-bin")?;
    }
    // secret-tool reads keys from the keyring at launch: the GitHub token for
    // ~/.local/bin/github-mcp, and avante's Anthropic key (plugins/ai.lua). It
    // ships in libsecret; answering it is the session's Secret Service
    // (gnome-keyring here).
    optional_pacman(
        "secret-tool (libsecret)",
        "secret-tool",
        &["libsecret"],
        "the GitHub MCP server and avante cannot read their keys from the keyring",
    );

    optional_pacman(
        "tmux",
        "tmux",
        &["tmux"],
        "the deployed tmux config has nothing to load it",
    );
    optional_pacman(
        "fastfetch",
        "fastfetch",
        &["fastfetch"],
        "the deployed fastfetch config has nothing to load it",
    );
    // `.zshrc` runs `eval "$(starship init zsh)"` behind a `command -v` guard
    // and .config/starship.toml is deployed, but the binary was never installed
    // — the guard turned that into a silently missing prompt rather than an
    // error. It is also the loudest consumer of the Nerd font above.
    optional_pacman(
        "starship",
        "starship",
        &["starship"],
        "the `command -v` guard in .zshrc leaves you on the bare zsh prompt",
    );
    // The other two `command -v`-guarded eval-inits in the same .zshrc, and
    // silent in exactly the same way starship was: neither binary is on PATH
    // here and nothing in dotctl ever installed either, so the guard turns a
    // missing tool into a feature that simply never existed.
    //   zoxide — `eval "$(zoxide init zsh)"` never runs, so `z` is never
    //            defined; the fzf-tab preview zstyle .zshrc registers against
    //            `__zoxide_z` is dead with it.
    //   direnv — `eval "$(direnv hook zsh)"` never installs the chpwd hook, so
    //            every .envrc in every project is ignored without a word.
    // Both are plain repo packages on both distros — extra/zoxide 0.10.0-1 and
    // extra/direnv 2.37.1-1, with cachyos-extra-znver4 rebuilds at 0.10.0-1.1
    // and 2.37.1-1.1 — so no AUR helper and no distro branch, and both ship a
    // binary of their own name, so `command_exists` is the right gate.
    optional_pacman(
        "zoxide",
        "zoxide",
        &["zoxide"],
        "the `command -v` guard in .zshrc leaves `z` undefined, along with its fzf-tab preview",
    );
    optional_pacman(
        "direnv",
        "direnv",
        &["direnv"],
        "the `command -v` guard in .zshrc means every .envrc is silently ignored",
    );
    // pkgfile's command-not-found handler, which .zshrc sources, names the
    // package a missing command lives in -- from a database that
    // pkgfile-update.timer builds, and the timer ships disabled. Nothing enabled
    // it, so the handler always fell back to zsh's bare "command not found".
    optional_pacman(
        "pkgfile",
        "pkgfile",
        &["pkgfile"],
        "zsh's command-not-found stays a bare error instead of naming the package",
    );
    if command_exists("pkgfile") {
        ensure_pkgfile_db();
    }
    optional_pacman(
        "Neovim",
        "nvim",
        &["neovim"],
        "the deployed nvim config has no editor to load it",
    );
    // Neovim's <leader>gG (lazygit.nvim) and tmux's `C-a g` popup both run
    // lazygit, and deploy lays down .config/lazygit, but nothing installed it --
    // the README left it to distro-work's heavier bundle -- so both bindings
    // opened onto "command not found". A single binary in `extra`.
    optional_pacman(
        "lazygit",
        "lazygit",
        &["lazygit"],
        "Neovim's <leader>gG and tmux's C-a g open onto a missing lazygit",
    );
    // Firmware updates. `fwupd-check.timer` (armed by `dotctl deploy`) runs
    // `fwupdmgr refresh` + `get-updates` monthly and speaks up only when LVFS
    // has something. Nothing in dotctl ever installed the package, so
    // `command -v fwupdmgr` failed here while the units sat symlinked into
    // ~/.config/systemd/user and unenabled — the check had never run once.
    // In `extra` on Arch and cachyos-extra-znver4 on CachyOS, so no distro
    // branch, and no `systemctl enable fwupd.service` needs to follow: the
    // package ships a D-Bus service file, so the daemon is activated by the
    // first fwupdmgr call.
    // Warn-and-continue rather than `?`, like the chaotic-aur and blackarch
    // steps: a monthly firmware reporter is the most optional thing in this
    // function, and its ~25 dependencies give it the widest failure surface of
    // any single package here. Nothing below reads fwupd, and deploy() skips
    // the timer on its own when the binary is absent.
    optional_pacman(
        "fwupd",
        "fwupdmgr",
        &["fwupd"],
        "`dotctl deploy` will skip fwupd-check.timer",
    );
    // Noctalia v5 (`noctalia`), the shell niri starts. It replaced the
    // Quickshell-based v4 (noctalia-shell + noctalia-qs, packaged only by
    // CachyOS): from noctalia-shell 5.6.0 on, that package no longer holds the
    // v4 shell, so `qs -c noctalia-shell` has nothing to start. v5 is in Arch
    // `extra`, so every distro installs it the same way.
    optional_pacman(
        "Noctalia",
        "noctalia",
        &["noctalia"],
        "you get bare niri — no bar, no notifications, no shell",
    );
    // The cursor theme every config in this repo names (see
    // [`CURSOR_THEME_NAME`]) and that nothing installed. Placed here, just
    // ahead of the greeter setup that writes `CursorTheme=` into
    // /etc/sddm.conf.d, because the login screen is the one consumer that
    // cannot fall back to a user-session default.
    //
    // `ensure_aur_pkg`, not `ensure_pacman*`, for exactly the reason spelled
    // out for rog-control-center above: it is in no Arch repo and in no
    // [cachyos]* repo (checked against every synced sync db on this box), and
    // `pacman -Si bibata-cursor-theme` answers here only because chaotic-aur
    // is configured — which dotctl deliberately skips on CachyOS. Gating on
    // the repo would therefore install it on this machine and silently skip it
    // on the distro the port targets. A helper resolves it from chaotic-aur as
    // a binary where that repo exists and builds it from the AUR where it does
    // not; `ensure_aur_pkg` detects it via `pacman -Q` (a cursor theme ships
    // no binary, so a `command_exists` gate would miss it forever).
    ensure_aur_pkg(helper, "Bibata cursor theme", CURSOR_THEME_PKG)?;
    // The GTK theme both settings.ini files name, in the same boat as the
    // cursor and installed by nobody: /usr/share/themes on this box holds only
    // adw-gtk3, adw-gtk3-dark, Default and Emacs, so GTK has been silently
    // falling back to Adwaita for every application in the session. AUR (and
    // chaotic-aur) only — `pacman -Ss tokyonight` over every synced db here
    // returns exactly one hit, chaotic-aur/tokyonight-gtk-theme-git — so the
    // same `ensure_aur_pkg` reasoning as Bibata above applies unchanged.
    //
    // Note the name mismatch this does NOT fix: the package installs
    // Tokyonight-{Dark,Light} plus -hdpi/-xhdpi variants, and no
    // `Tokyonight-Dark-BL`. Upstream shipped prebuilt `Tokyonight-*-BL`
    // directories until commit 58a04f1d (2024-03-18) replaced them with a
    // generator that emits neither the -BL nor the -BL-LB suffix, so the name
    // the two settings.ini files ask for has not existed in any package for
    // over a year. `Tokyonight-Dark` — what .config/niri/config.kdl already
    // exports as GTK_THEME, and what GTK3 apps therefore actually get — is the
    // installable spelling; the two settings.ini files want their `-BL`
    // dropped to agree with it.
    ensure_aur_pkg(helper, "Tokyonight GTK theme", "tokyonight-gtk-theme-git")?;

    // AUR add-ons for the full Noctalia ecosystem.
    // sddm-theme-noctalia-git (AUR): login-screen theme matched to the shell;
    //   depends on sddm, so this also pulls in the display manager itself.
    //   setup_noctalia_login then selects the theme + enables sddm.service.
    // noctalia-unofficial-auth-agent-git: ships /usr/libexec/bb-auth (polkit
    //   agent + GNOME-keyring prompter) and its own bb-auth.service user unit,
    //   which `dotctl deploy` enables. Built from a locally patched PKGBUILD —
    //   see ensure_noctalia_auth_agent for the GCC 16 fix.
    // Genuinely AUR-only on both distros — do NOT swap in CachyOS's
    //   `noctalia-greeter`, which is a greetd greeter with a bundled wlroots
    //   compositor, not an SDDM theme.
    ensure_aur_pkg(helper, "Noctalia SDDM theme", "sddm-theme-noctalia-git")?;
    // The noctalia SDDM theme above is the active login screen; greetd +
    // ReGreet (graphical, hosted by cage) is configured as a disabled
    // fallback. All in Arch `extra` (rebuilt in cachyos-extra-znver4).
    // Optional by construction: this greeter is installed *disabled*, SDDM is
    // the active login screen, and `setup_noctalia_login` already gates its
    // config writes on `command_exists("greetd")`. A missing fallback costs
    // nothing until someone enables it by hand.
    optional_pacman(
        "greetd",
        "greetd",
        &["greetd"],
        "the greetd + ReGreet fallback greeter will not be available",
    );
    optional_pacman(
        "regreet",
        "regreet",
        &["greetd-regreet"],
        "greetd would fall back to its text-mode agreety greeter",
    );
    optional_pacman(
        "cage",
        "cage",
        &["cage"],
        "greetd has no compositor to host ReGreet in",
    );
    setup_noctalia_login()?;
    ensure_noctalia_auth_agent()?;

    // HexStrike AI. Warn-and-continue, and the widest failure surface in this
    // function by a distance: a GitHub clone, a Python venv and a `pip install
    // -r` against PyPI, re-run on every invocation. It is also LAST, so a `?`
    // here threw the deploy away after every other step had already succeeded.
    // Nothing below reads it, and `deploy()` already handles its absence on its
    // own — `enable_user_unit` checks for the venv python before enabling the
    // unit.
    //
    // That is also why the loopback refusals inside can be warnings without
    // loosening anything. This rewrite is not the control that confines the
    // API, and hexstrike-server.service says so in as many words: `deploy()`
    // re-derives `BindState` from the file and refuses to enable the service
    // unless it is `AlreadySafe`, and the unit's
    // `ExecStartPost=hexstrike-assert-loopback` then checks the socket the
    // process actually bound. Both fail closed on the state a warn leaves
    // behind, so the service simply never starts.
    if let Err(e) = setup_hexstrike() {
        warn(&format!(
            "HexStrike AI setup failed: {e} — continuing; hexstrike-server.service stays \
             unenabled until you rerun `dotctl install`"
        ));
    }

    Ok(())
}
