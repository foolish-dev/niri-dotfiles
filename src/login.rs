use crate::util::{command_exists, info, ok, run, sudo_write, sudo_write_owned, warn};
use anyhow::Result;
use std::fs;

/// The single SDDM drop-in this repo writes. `.local/bin/sddm-theme` writes
/// *this same path* rather than one of its own, deliberately: verified against
/// sddm 0.21.0 (`man 5 sddm.conf`, plus stracing the order it opens them),
/// SDDM reads **every file** in /etc/sddm.conf.d — not only `*.conf` — in
/// name order, then /etc/sddm.conf, with the last value read winning. Two
/// drop-ins would therefore be arbitrated by a filename sort: the
/// `theme.conf` older `sddm-theme` versions wrote sorts after this one and
/// silently shadowed it, while `dotctl install` still reported success.
///
/// A drop-in rather than /etc/sddm.conf so other settings (autologin,
/// numlock, …) are left alone. SDDM stays installed as a themed fallback even
/// though greetd is the active display manager.
const SDDM_THEME_CONF: &str = "/etc/sddm.conf.d/10-noctalia-theme.conf";

/// The theme `dotctl install` selects, and the marker for "this drop-in is
/// still ours to rewrite" — see [`sddm_theme_is_ours`].
const SDDM_THEME_NAME: &str = "noctalia";

/// The cursor theme this repo names, and the package that ships it. Kept
/// adjacent because they were free to drift and did: the *name* is in
/// .config/gtk-3.0/settings.ini, .config/gtk-4.0/settings.ini,
/// .config/niri/config.kdl (twice — `xcursor-theme` and `XCURSOR_THEME`),
/// assets/greetd/regreet.toml, `.local/bin/sddm-theme` and the drop-in below,
/// while the *package* was installed by nobody — `pacman -Qq
/// bibata-cursor-theme` fails on this box, so every one of those strings has
/// been falling back to the default Adwaita cursor with no error anywhere.
///
/// `bibata-cursor-theme` is the right one of the four `bibata-*` packages:
/// its file list carries /usr/share/icons/Bibata-Modern-Classic/index.theme
/// (verified against the chaotic-aur files database, which ships the six
/// Modern/Original × Classic/Ice/Amber themes plus `-Right` variants).
/// `bibata-extra-cursor-theme` ships only DarkRed/DodgerBlue/Pink/Turquoise
/// and would not provide this name at all.
pub(crate) const CURSOR_THEME_PKG: &str = "bibata-cursor-theme";
const CURSOR_THEME_NAME: &str = "Bibata-Modern-Classic";

/// The body of the [`SDDM_THEME_CONF`] drop-in. Pure so the three keys — and
/// the cursor name shared with the deployed configs — can be asserted without
/// root; writing it is [`setup_noctalia_login`]'s job.
fn sddm_theme_conf_body() -> String {
    format!("[Theme]\nCurrent={SDDM_THEME_NAME}\nCursorTheme={CURSOR_THEME_NAME}\nCursorSize=24\n")
}

/// Whether the drop-in at [`SDDM_THEME_CONF`] is still dotctl's to write.
/// Now that `.local/bin/sddm-theme` writes the same file, a `Current=` naming
/// a different theme is a choice the user made *after* install, and
/// re-asserting noctalia on every `dotctl all` would silently undo it. Same
/// rule as [`greetd_is_replaceable`], and pure for the same reason: absent,
/// or without a parseable `Current=`, counts as ours.
fn sddm_theme_is_ours(body: Option<&str>) -> bool {
    let Some(body) = body else {
        return true;
    };
    body.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .find_map(|l| l.strip_prefix("Current")?.trim_start().strip_prefix('='))
        .is_none_or(|v| v.trim() == SDDM_THEME_NAME)
}

/// greetd's whole config. We do *not* exclusively own this file: the greetd
/// package ships this exact path as a pacman backup file, whose stock body
/// runs `agreety --cmd /bin/sh` — so a fresh box always has a real config here
/// before dotctl ever runs (see [`greetd_is_replaceable`]). cage hosts
/// ReGreet, which lists the niri session it discovers under
/// /usr/share/wayland-sessions (niri.desktop → niri-session). The `greeter`
/// system user is created by the greetd package.
const GREETD_CONF: &str = "/etc/greetd/config.toml";
const GREETD_CONFIG_BODY: &str = "[terminal]\nvt = 1\n\n[default_session]\ncommand = \"dbus-run-session cage -s -mlast -d -- regreet\"\nuser = \"greeter\"\n";

/// greetd session commands dotctl is entitled to replace: `regreet` is the
/// greeter we configure ourselves, and `agreety` is the stock command in the
/// `config.toml` the greetd package itself ships (verified: `pacman -Ql
/// greetd` lists /etc/greetd/config.toml as a Backup File whose body is
/// `command = "agreety --cmd /bin/sh"`).
///
/// Anything else — CachyOS's `noctalia-greeter`, `tuigreet`, `gtkgreet` — is a
/// greeter somebody chose on purpose, and dotctl must neither rewrite its
/// config nor switch greetd.service off underneath it.
const GREETD_REPLACEABLE_GREETERS: [&str; 2] = ["regreet", "agreety"];

/// The `command = "…"` value from a greetd config body, if present.
fn greetd_session_command(body: &str) -> Option<&str> {
    body.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .find_map(|l| l.strip_prefix("command")?.trim_start().strip_prefix('='))
        .map(|v| v.trim().trim_matches('"').trim_matches('\''))
}

/// Whether the greetd config at [`GREETD_CONF`] is one dotctl may take over.
/// Pure, so the "never steal a greeter somebody configured" rule is testable
/// without root. `None` (no file) counts as replaceable.
///
/// Deliberately NOT "is this config byte-identical to ours?" and NOT "does it
/// mention regreet?": both answer *false* on a fresh box, where greetd's own
/// package has just written the stock agreety config — which would stop dotctl
/// writing the ReGreet fallback on every clean install of both distros.
fn greetd_is_replaceable(body: Option<&str>) -> bool {
    let Some(body) = body else {
        return true;
    };
    let Some(cmd) = greetd_session_command(body) else {
        return true;
    };
    cmd.split_whitespace()
        .map(|w| w.rsplit('/').next().unwrap_or(w))
        .any(|w| GREETD_REPLACEABLE_GREETERS.contains(&w))
}

/// ReGreet's own config + GTK CSS, embedded at build time and written to
/// /etc/greetd/ at install. The CSS approximates the noctalia look; sessions
/// are auto-discovered from /usr/share/wayland-sessions (niri-session).
const REGREET_CONF: &str = "/etc/greetd/regreet.toml";
const REGREET_TOML: &str = include_str!("../assets/greetd/regreet.toml");
const REGREET_CSS_PATH: &str = "/etc/greetd/regreet.css";
const REGREET_CSS: &str = include_str!("../assets/greetd/regreet.css");

/// The systemd alias symlinked to whichever display manager is enabled.
/// `systemctl enable sddm.service` writes this; if it already points
/// elsewhere, another DM owns the login screen.
const DISPLAY_MANAGER_UNIT: &str = "/etc/systemd/system/display-manager.service";

/// Basename of the unit `display-manager.service` points at (e.g.
/// `sddm.service`, `gdm.service`), or `None` when no display manager is
/// enabled. Reading the symlink needs no root, so the "is another DM
/// already wired up?" decision stays cheap and side-effect-free.
fn current_display_manager() -> Option<String> {
    let target = fs::read_link(DISPLAY_MANAGER_UNIT).ok()?;
    target.file_name()?.to_str().map(str::to_string)
}

/// What to do about `sddm.service` given the currently-enabled display
/// manager. Pure, so the "never steal the login screen from a third-party
/// DM" rule is unit-testable without touching systemd. `greetd.service` is
/// ours to replace only when `greetd_replaceable` — i.e. its config runs
/// regreet (ours) or the stock agreety (see [`greetd_is_replaceable`]); a
/// greetd hosting somebody else's greeter is treated like gdm/lightdm/ly and
/// left alone.
enum LoginAction {
    AlreadySddm,
    OtherDm(String),
    Enable,
}

fn login_action(current_dm: Option<String>, greetd_replaceable: bool) -> LoginAction {
    match current_dm {
        Some(dm) if dm == "sddm.service" => LoginAction::AlreadySddm,
        Some(dm) if dm == "greetd.service" && greetd_replaceable => LoginAction::Enable,
        Some(other) => LoginAction::OtherDm(other),
        None => LoginAction::Enable,
    }
}

/// Write the three files that make up the greetd + ReGreet fallback, in the
/// only order that is safe: config.toml first, because it is the one that is
/// load-bearing for the other two.
///
/// Kept as its own `Result` — with the `?`s intact — rather than three
/// warn-and-continue calls inlined into [`setup_noctalia_login`]. The three
/// writes are one unit: config.toml is what makes greetd launch regreet at
/// all, so regreet.toml and regreet.css mean nothing without it, and a run
/// that lands only some of them must not print "greetd + ReGreet fallback
/// config written". The caller turns *this* function's error into a single
/// warning, so a half-written fallback is reported as a failure rather than
/// either aborting `install()` or being papered over per-file.
///
/// (Ordering matters the other way too: config.toml is written first, so if a
/// later write fails greetd is at worst pointed at a regreet with its own
/// defaults — never at a regreet config for a greeter greetd was never told
/// to launch.)
fn write_greetd_fallback() -> Result<()> {
    // config.toml is a pacman backup file shipped by greetd itself, so
    // back up whatever is there before replacing it. regreet.toml and
    // regreet.css are paths dotctl solely owns.
    sudo_write_owned(GREETD_CONF, GREETD_CONFIG_BODY)?;
    sudo_write(REGREET_CONF, REGREET_TOML)?;
    sudo_write(REGREET_CSS_PATH, REGREET_CSS)?;
    Ok(())
}

/// Set up the login screen: the graphical noctalia **SDDM** theme as the
/// active display manager, with greetd + ReGreet (also noctalia-themed) left
/// installed and configured as a disabled fallback. Selects the noctalia SDDM
/// theme and enables `sddm.service` — but only when no third-party DM
/// (gdm/lightdm/ly) is already enabled, so we never silently steal someone
/// else's login screen. `greetd.service` is the fallback we also configure, so
/// switching off it to sddm is allowed. Best-effort like the rest of
/// `install()`: a missing sddm, a failed `sudo tee` or a failed `systemctl`
/// call warns and returns Ok rather than aborting.
///
/// That promise used to be a lie: every `sudo` write in here was a hard `?`.
/// This runs after two `cargo install --git` builds and an AUR `makepkg`,
/// comfortably past sudo's 15-minute `timestamp_timeout` default, so the
/// first write hits a fresh password prompt — and on an unattended install
/// that prompt fails, `sudo tee` exits non-zero, and the `?` took down the
/// whole of `install()` with it. Under `dotctl all` the `install(...)?` in
/// `main` then meant `deploy()` never ran either: no configs, no symlinks,
/// over a login screen that had already been half-configured.
pub(crate) fn setup_noctalia_login() -> Result<()> {
    // Read before we would overwrite it: greetd is a shared entry point, and
    // the answer gates both the config writes and whether greetd.service is
    // ours to switch off.
    let greetd_replaceable = greetd_is_replaceable(fs::read_to_string(GREETD_CONF).ok().as_deref());

    // Keep greetd + ReGreet configured as a disabled fallback, so a manual
    // `systemctl enable greetd` still lands on the themed graphical greeter.
    if command_exists("greetd") {
        if greetd_replaceable {
            info("Writing greetd + ReGreet fallback config ...");
            match write_greetd_fallback() {
                Ok(()) => ok("greetd + ReGreet fallback config written"),
                // Deliberately not per-file: see `write_greetd_fallback`. The
                // fallback stays disabled either way, so the cost of stopping
                // here is a fallback greeter that is not ready — not a broken
                // login screen — and that is worth strictly less than the rest
                // of `install()` plus, under `dotctl all`, the whole `deploy()`.
                Err(e) => warn(&format!(
                    "greetd + ReGreet fallback config incomplete: {e} — greetd.service stays \
                     disabled; rerun `dotctl install` to finish it"
                )),
            }
        } else {
            warn(&format!(
                "{GREETD_CONF} configures another greeter (on CachyOS, `noctalia-greeter` \
                 owns this file) — leaving it and greetd.service alone"
            ));
        }
    }

    if !command_exists("sddm") {
        warn("sddm not found (theme install may have failed) — skipping login-screen setup");
        return Ok(());
    }
    // Read before overwriting, exactly as for greetd: `.local/bin/sddm-theme`
    // writes this same file now, so a different Current= is the user's choice.
    let existing = fs::read_to_string(SDDM_THEME_CONF).ok();
    if sddm_theme_is_ours(existing.as_deref()) {
        info("Selecting the noctalia SDDM theme ...");
        // The same three keys `sddm-theme` writes, so whichever of the two ran
        // last leaves the same shape and the cursor settings don't vanish on
        // the next install.
        // Warn-and-continue, and the `match` on `login_action` below still
        // runs: a drop-in we failed to write leaves SDDM on its stock theme,
        // which is an ugly login screen rather than a broken one, and is
        // fixed by `sddm-theme noctalia` or another `dotctl install`. If the
        // cause was a dead sudo prompt, the `systemctl enable` below fails the
        // same way and says so — so nothing is reported green either way.
        match sudo_write(SDDM_THEME_CONF, &sddm_theme_conf_body()) {
            Ok(()) => ok("SDDM theme set to noctalia"),
            Err(e) => warn(&format!(
                "could not write {SDDM_THEME_CONF}: {e} — the login screen will use SDDM's \
                 default theme; rerun `dotctl install` or `sddm-theme {SDDM_THEME_NAME}`"
            )),
        }
    } else {
        ok(&format!(
            "{SDDM_THEME_CONF} selects another theme (set with `sddm-theme`) — leaving it"
        ));
    }

    match login_action(current_display_manager(), greetd_replaceable) {
        LoginAction::AlreadySddm => ok("sddm is already the active display manager"),
        LoginAction::OtherDm(other) => warn(&format!(
            "{other} is already the enabled display manager — leaving it in place; \
             run `sudo systemctl disable {other} && sudo systemctl enable sddm.service` to switch"
        )),
        LoginAction::Enable => {
            // Disable greetd first: a second DM can't claim display-manager.service
            // while greetd still owns the alias. Harmless no-op when greetd isn't set.
            let _ = run("sudo", &["systemctl", "disable", "greetd.service"]);
            info("Enabling sddm.service ...");
            if let Err(e) = run("sudo", &["systemctl", "enable", "sddm.service"]) {
                warn(&format!("failed to enable sddm.service: {e} — continuing"));
            } else {
                ok("sddm.service enabled — noctalia login screen active on next boot");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── login_action ───────────────────────────────────────────────────────
    //
    // The protective invariant for the login screen: enable sddm.service
    // only when no third-party DM is already wired up, so dotctl never
    // silently steals the login screen from gdm/lightdm/ly. greetd is the
    // fallback greeter we also configure, so switching off it to sddm is OK.

    #[test]
    fn login_action_enables_sddm_when_no_dm_is_set() {
        assert!(matches!(login_action(None, true), LoginAction::Enable));
    }

    #[test]
    fn login_action_switches_to_sddm_from_our_own_greetd() {
        assert!(matches!(
            login_action(Some("greetd.service".into()), true),
            LoginAction::Enable
        ));
    }

    #[test]
    fn login_action_is_noop_when_sddm_already_active() {
        assert!(matches!(
            login_action(Some("sddm.service".into()), true),
            LoginAction::AlreadySddm
        ));
    }

    #[test]
    fn login_action_leaves_another_dm_alone() {
        match login_action(Some("gdm.service".into()), true) {
            LoginAction::OtherDm(dm) => assert_eq!(dm, "gdm.service"),
            _ => panic!("a third-party DM must be left in place, never overridden"),
        }
    }

    // ── the cursor theme ───────────────────────────────────────────────────
    //
    // `Bibata-Modern-Classic` was named by six deployed files and installed by
    // nobody. The install itself needs pacman and cannot be unit-tested; what
    // *can* be pinned without root is that the name dotctl writes and the name
    // the deployed configs ask for are the same string.

    #[test]
    fn the_sddm_drop_in_names_the_same_cursor_theme_as_the_deployed_regreet_config() {
        // regreet.toml is compiled in, so it is an independent witness: if
        // someone renames the cursor in one place and not the other, the
        // greeter and the login screen silently disagree and one of them falls
        // back to Adwaita with no error anywhere.
        assert!(
            REGREET_TOML.contains(&format!("cursor_theme_name = \"{CURSOR_THEME_NAME}\"")),
            "assets/greetd/regreet.toml must name {CURSOR_THEME_NAME}"
        );
        assert!(
            sddm_theme_conf_body().contains(&format!("CursorTheme={CURSOR_THEME_NAME}")),
            "the SDDM drop-in must name the cursor theme dotctl installs"
        );
    }

    #[test]
    fn the_sddm_drop_in_we_write_is_one_dotctl_still_recognises_as_its_own() {
        // sddm_theme_is_ours gates every later rewrite of this file. If the
        // body we write ever stopped satisfying it, the *second* `dotctl
        // install` would report "selects another theme — leaving it" over a
        // drop-in dotctl had just written itself.
        assert!(sddm_theme_is_ours(Some(&sddm_theme_conf_body())));
    }

    // ── sddm_theme_is_ours ─────────────────────────────────────────────────
    //
    // .local/bin/sddm-theme writes SDDM_THEME_CONF too — one file, one
    // precedence, no filename-sort tiebreak. The cost of sharing it is that
    // `dotctl all` must not stomp a theme the user picked with that script.

    #[test]
    fn sddm_drop_in_is_ours_when_absent() {
        assert!(sddm_theme_is_ours(None));
    }

    #[test]
    fn sddm_drop_in_is_ours_when_it_still_names_noctalia() {
        assert!(sddm_theme_is_ours(Some(
            "[Theme]\nCurrent=noctalia\nCursorSize=24\n"
        )));
    }

    #[test]
    fn sddm_drop_in_is_left_alone_once_sddm_theme_picked_another() {
        assert!(
            !sddm_theme_is_ours(Some("[Theme]\nCurrent=elarun\nCursorSize=24\n")),
            "a theme chosen with `sddm-theme` must survive the next `dotctl install`"
        );
    }

    #[test]
    fn sddm_drop_in_without_a_current_key_is_ours() {
        // Nothing to preserve — write our own rather than leave a themeless
        // login screen.
        assert!(sddm_theme_is_ours(Some("[Theme]\n# Current=elarun\n")));
    }

    // The greeter is wired greetd → cage → regreet, with the noctalia-themed
    // ReGreet config embedded at build time. Guard the wiring so a stray edit
    // to the command string or an emptied asset fails CI instead of the boot.
    #[test]
    fn greetd_launches_cage_and_regreet() {
        assert!(GREETD_CONFIG_BODY.contains("cage"));
        assert!(GREETD_CONFIG_BODY.contains("regreet"));
        assert!(!REGREET_TOML.trim().is_empty());
        assert!(!REGREET_CSS.trim().is_empty());
    }

    // ── greetd ownership ─────────────────────────────────────────────────

    /// The verbatim `/etc/greetd/config.toml` shipped by the greetd package
    /// (`pacman -Ql greetd` lists it as a Backup File). This is the state of a
    /// fresh box the moment `install()` installs greetd.
    const STOCK_GREETD_CONFIG: &str = r#"[terminal]
# The VT to run the greeter on. Can be "next", "current" or a number
# designating the VT.
vt = 1

# The default session, also known as the greeter.
[default_session]

# `agreety` is the bundled agetty/login-lookalike. You can replace `/bin/sh`
# with whatever you want started, such as `sway`.
command = "agreety --cmd /bin/sh"

# The user to run the command as. The privileges this user must have depends
# on the greeter. A graphical greeter may for example require the user to be
# in the `video` group.
user = "greeter"
"#;

    #[test]
    fn greetd_session_command_extracts_the_default_session_command() {
        assert_eq!(
            greetd_session_command(GREETD_CONFIG_BODY),
            Some("dbus-run-session cage -s -mlast -d -- regreet")
        );
        assert_eq!(
            greetd_session_command(STOCK_GREETD_CONFIG),
            Some("agreety --cmd /bin/sh")
        );
        assert_eq!(greetd_session_command("[terminal]\nvt = 1\n"), None);
        assert_eq!(
            greetd_session_command("commandx = \"y\"\n"),
            None,
            "`commandx` must not be matched as `command`"
        );
    }

    #[test]
    fn greetd_is_replaceable_accepts_the_stock_agreety_config() {
        // The most important test in the port. This is the fresh-install
        // state on both distros; a rule that answers false here stops the
        // ReGreet fallback ever being written on a clean box.
        assert!(greetd_is_replaceable(Some(STOCK_GREETD_CONFIG)));
    }

    #[test]
    fn greetd_is_replaceable_accepts_our_own_config_and_a_missing_file() {
        // Self-consistency: the predicate must accept the exact bytes the
        // constant beside it writes, or dotctl classifies its own output as
        // foreign on the second run. Also catches an edit to
        // GREETD_CONFIG_BODY that drops `regreet`.
        assert!(greetd_is_replaceable(Some(GREETD_CONFIG_BODY)));
        assert!(greetd_is_replaceable(None));
    }

    #[test]
    fn greetd_is_replaceable_rejects_a_third_party_greeter() {
        // CachyOS publishes `noctalia-greeter` in [cachyos] — a greeter that
        // lives in exactly this file. Without this, the port's most
        // user-hostile failure mode is live.
        assert!(!greetd_is_replaceable(Some(
            "[default_session]\ncommand = \"noctalia-greeter\"\n"
        )));
        assert!(!greetd_is_replaceable(Some(
            "[default_session]\ncommand = \"tuigreet --cmd niri-session\"\n"
        )));
        assert!(
            greetd_is_replaceable(Some("[default_session]\ncommand = \"/usr/bin/regreet\"\n")),
            "an absolute path to our own greeter still matches on basename"
        );
        // Whole-token, not substring: a `contains()`-shaped rewrite would keep
        // every other assertion here green while making dotctl clobber the
        // config of a greeter that merely has one of our names inside its own.
        assert!(
            !greetd_is_replaceable(Some(
                "[default_session]\ncommand = \"cage -s -- regreet-ng\"\n"
            )),
            "a greeter whose name merely *contains* regreet is not ours"
        );
        assert!(
            !greetd_is_replaceable(Some(
                "[default_session]\ncommand = \"my-agreety-wrapper\"\n"
            )),
            "…and the same for agreety"
        );
    }

    #[test]
    fn login_action_leaves_a_third_party_greetd_alone() {
        // Counterpart to login_action_switches_to_sddm_from_our_own_greetd;
        // together they state the whole rule.
        match login_action(Some("greetd.service".into()), false) {
            LoginAction::OtherDm(dm) => assert_eq!(dm, "greetd.service"),
            _ => panic!("a greetd hosting someone else's greeter must be left alone"),
        }
    }

    #[test]
    fn login_action_still_enables_sddm_when_a_foreign_greetd_is_not_the_dm() {
        // Guards against over-correcting: a stray third-party greetd config on
        // disk must not block enabling sddm when greetd is not the enabled DM.
        assert!(matches!(login_action(None, false), LoginAction::Enable));
    }
}
