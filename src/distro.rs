use crate::util::{cache_dir, command_exists, info, ok, run};
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

// ── Distro detection ──────────────────────────────────────────────────────────

/// Which pacman-family distribution we're running on. Every distro-conditional
/// decision in dotctl is a `match` on this, so adding the next derivative is a
/// compile error at each site that has to care rather than a silently-missed
/// branch.
///
/// The safety rule for every one of those matches: **only [`Distro::CachyOs`]
/// may diverge**. `Arch`, `ArchDerivative` and `Unknown` share the arm that
/// encodes dotctl's pre-CachyOS behaviour, so a host we can't identify — or one
/// with no readable os-release at all — keeps doing exactly what it did before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Distro {
    /// `ID=cachyos`. Ships the `[cachyos]` repo, which carries yay and paru.
    CachyOs,
    /// `ID=arch` — stock Arch Linux.
    Arch,
    /// Not Arch or CachyOS by `ID`, but `ID_LIKE` lists `arch`
    /// (EndeavourOS, Garuda, Artix, Manjaro, …). Treated as Arch.
    ArchDerivative,
    /// No os-release, unreadable, or an `ID` we don't recognise. Treated as
    /// Arch.
    Unknown,
}

impl Distro {
    /// Human-readable name for the banner `install()` prints, so a bug report
    /// says which branch the run took.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Distro::CachyOs => "CachyOS",
            Distro::Arch => "Arch Linux",
            Distro::ArchDerivative => "an Arch derivative (treated as Arch)",
            Distro::Unknown => "an unrecognised distro (treated as Arch)",
        }
    }
}

/// Strip one matching pair of surrounding single or double quotes, if present.
fn unquote(v: &str) -> &str {
    let b = v.as_bytes();
    if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] {
        return &v[1..v.len() - 1];
    }
    v
}

/// Value of `key` in an os-release `contents`, unquoted.
///
/// os-release is shell-like: `KEY=value`, `KEY="value"`, `KEY='value'`, `#`
/// comments, blank lines, and — on a file that has travelled through Windows —
/// CRLF endings, which the leading `trim()` absorbs since `\r` is whitespace.
/// Later assignments win, matching a shell sourcing the file. Matching is on
/// the whole key, so `ID_LIKE=arch` alone never answers a query for `ID`.
fn os_release_value(contents: &str, key: &str) -> Option<String> {
    let mut found = None;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() == key {
            found = Some(unquote(v.trim()).to_string());
        }
    }
    found
}

/// Classify an os-release file's *contents*. Pure, so every shape it must
/// survive is testable from a fixture string with no filesystem involved —
/// which is what keeps these tests green in CI (Ubuntu) and on stock Arch.
///
/// Only an exact `ID=cachyos` yields [`Distro::CachyOs`]. A hypothetical
/// derivative-of-CachyOS (`ID_LIKE="cachyos arch"`) deliberately falls through
/// to [`Distro::ArchDerivative`]: guessing wrong towards CachyOS skips the
/// chaotic-aur setup on a box that may need it, whereas guessing wrong towards
/// Arch is just today's behaviour.
fn parse_distro(os_release: &str) -> Distro {
    match os_release_value(os_release, "ID")
        .unwrap_or_default()
        .as_str()
    {
        "cachyos" => Distro::CachyOs,
        "arch" => Distro::Arch,
        _ => {
            let like = os_release_value(os_release, "ID_LIKE").unwrap_or_default();
            if like.split_whitespace().any(|w| w == "arch") {
                Distro::ArchDerivative
            } else {
                Distro::Unknown
            }
        }
    }
}

/// [`parse_distro`] over the real file. `/etc/os-release` is the
/// admin-overridable copy and wins; `/usr/lib/os-release` is the vendor
/// default and is the fallback — systemd's own lookup order. Neither readable
/// ⇒ [`Distro::Unknown`], which every match arm treats as Arch.
pub(crate) fn detect_distro() -> Distro {
    fs::read_to_string("/etc/os-release")
        .or_else(|_| fs::read_to_string("/usr/lib/os-release"))
        .map(|c| parse_distro(&c))
        .unwrap_or(Distro::Unknown)
}

/// Whether `install()` should wire up the Chaotic-AUR repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChaoticAur {
    /// Add it. On stock Arch it is dotctl's only source for an AUR helper —
    /// neither yay nor paru is in any Arch official repo.
    Add,
    /// Skip it. `[cachyos]` already carries yay and paru.
    SkipRedundant,
}

pub(crate) fn chaotic_aur_policy(distro: Distro) -> ChaoticAur {
    match distro {
        Distro::CachyOs => ChaoticAur::SkipRedundant,
        Distro::Arch | Distro::ArchDerivative | Distro::Unknown => ChaoticAur::Add,
    }
}

/// The fix-it hint printed when no AUR helper is available and none can be
/// installed from a configured repository. Distro-specific because the fix
/// genuinely differs: on CachyOS a helper is a plain `pacman -S` away; on
/// stock Arch it is a git clone + makepkg. Telling an Arch user to run
/// `sudo pacman -S yay` sends them to `error: target not found`.
pub(crate) fn aur_helper_hint(distro: Distro) -> &'static str {
    match distro {
        Distro::CachyOs => "run `sudo pacman -S yay` — the [cachyos] repo carries both yay and paru",
        Distro::Arch | Distro::ArchDerivative | Distro::Unknown => {
            "install one by hand (`git clone https://aur.archlinux.org/yay.git && cd yay && makepkg -si`), \
             or rerun `dotctl install` now that the chaotic-aur repo is configured"
        }
    }
}

/// True if `contents` (an /etc/pacman.conf) declares a `[name]` section.
///
/// Pure so the parse's two quirks are pinned by tests. The `trim()` is
/// load-bearing: CachyOS's shipped pacman.conf writes its section headers with
/// a trailing space (`[cachyos-znver4] `). And a commented-out `#[blackarch]`
/// must NOT count as present, or dotctl would skip a setup step the box needs.
fn pacman_conf_has_repo(contents: &str, name: &str) -> bool {
    let header = format!("[{name}]");
    contents.lines().any(|l| l.trim() == header)
}

fn has_pacman_repo(name: &str) -> bool {
    fs::read_to_string("/etc/pacman.conf")
        .map(|c| pacman_conf_has_repo(&c, name))
        .unwrap_or(false)
}

/// Where `pacman -Sy` drops the sync database for a repo. Its presence is the
/// difference between "the repo is written into pacman.conf" and "the repo can
/// actually resolve a package".
fn sync_db_path(repo: &str) -> String {
    format!("/var/lib/pacman/sync/{repo}.db")
}

/// How far the chaotic-aur setup actually got on this box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChaoticState {
    /// In pacman.conf *and* its sync db is on disk — `pacman -S yay`
    /// resolves. Nothing to do.
    Ready,
    /// The repo line is in pacman.conf but no sync db was ever written, so
    /// the repo is named and cannot install a thing. One `pacman -Sy`
    /// repairs it; the keyring and mirrorlist are already in place.
    ConfiguredUnsynced,
    /// Not in pacman.conf at all — the full key + keyring + mirrorlist +
    /// repo-line bootstrap.
    Absent,
}

/// Configured and synced are two different states, and only the second one can
/// install anything. The repo line goes into pacman.conf one step before the
/// `pacman -Sy` that populates its db, so an interrupted (or, now that the
/// caller no longer aborts on it, a *failed*) run leaves a repo that
/// [`has_pacman_repo`] reports as present while `pacman -S yay` still says
/// "target not found". Folding the two into one boolean *was* the bug: every
/// later run early-returned "already present" and never repaired it.
///
/// `configured` decides alone when it is false — a sync db left behind by a
/// repo that has since been removed from pacman.conf is not a configured repo,
/// and must not short-circuit the bootstrap.
fn chaotic_state(configured: bool, synced: bool) -> ChaoticState {
    match (configured, synced) {
        (false, _) => ChaoticState::Absent,
        (true, false) => ChaoticState::ConfiguredUnsynced,
        (true, true) => ChaoticState::Ready,
    }
}

pub(crate) fn setup_chaotic_aur() -> Result<()> {
    if !command_exists("pacman") {
        return Ok(());
    }
    match chaotic_state(
        has_pacman_repo("chaotic-aur"),
        Path::new(&sync_db_path("chaotic-aur")).exists(),
    ) {
        ChaoticState::Ready => {
            ok("Chaotic AUR repo already present");
            return Ok(());
        }
        ChaoticState::ConfiguredUnsynced => {
            info("Chaotic AUR configured but never synced — refreshing ...");
            run("sudo", &["pacman", "-Sy"])?;
            ok("Chaotic AUR repo synced");
            return Ok(());
        }
        ChaoticState::Absent => {}
    }
    info("Adding Chaotic AUR repository ...");
    run(
        "sudo",
        &[
            "pacman-key",
            "--recv-key",
            "3056513887B78AEB",
            "--keyserver",
            "keyserver.ubuntu.com",
        ],
    )?;
    run("sudo", &["pacman-key", "--lsign-key", "3056513887B78AEB"])?;
    run(
        "sudo",
        &[
            "pacman",
            "-U",
            "--noconfirm",
            "https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-keyring.pkg.tar.zst",
            "https://cdn-mirror.chaotic.cx/chaotic-aur/chaotic-mirrorlist.pkg.tar.zst",
        ],
    )?;
    run(
        "sudo",
        &[
            "sh",
            "-c",
            "printf '\\n[chaotic-aur]\\nInclude = /etc/pacman.d/chaotic-mirrorlist\\n' >> /etc/pacman.conf",
        ],
    )?;
    run("sudo", &["pacman", "-Sy"])?;
    ok("Chaotic AUR repo added");
    Ok(())
}

pub(crate) fn setup_blackarch() -> Result<()> {
    if !command_exists("pacman") {
        return Ok(());
    }
    if has_pacman_repo("blackarch") {
        ok("BlackArch repo already present");
        return Ok(());
    }
    info("Adding BlackArch repository (via strap.sh) ...");
    // Stage strap.sh in dotctl's own cache dir, not a predictable path in
    // world-writable /tmp. strap.sh is executed as root, so a symlink/TOCTOU
    // swap between download and exec in shared /tmp would be a root-exec
    // primitive (CWE-377/CWE-379); ~/.cache is owned by and writable only by
    // the user. Remove it afterwards rather than leaving it around.
    let dir = cache_dir().join("dotctl");
    fs::create_dir_all(&dir)?;
    let strap = dir.join("blackarch-strap.sh");
    let strap_str = strap.to_str().context("strap.sh path is not valid utf-8")?;
    run(
        "curl",
        &["-fsSL", "-o", strap_str, "https://blackarch.org/strap.sh"],
    )?;
    run("sudo", &["chmod", "+x", strap_str])?;
    // strap.sh prompts for confirmation; pipe `yes` through so the install is
    // non-interactive. Pass the path as a positional arg ("$1") rather than
    // interpolating it into the script — same injection-safe shape as
    // `command_exists`, so a path with shell metacharacters can't break out.
    let res = run("sh", &["-c", r#"yes | sudo "$1""#, "sh", strap_str]);
    let _ = fs::remove_file(&strap);
    res?;
    ok("BlackArch repo added");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Distro detection ─────────────────────────────────────────────────
    //
    // Every one of these is a pure string→enum assertion, so they run
    // identically on CI (Ubuntu, no pacman), on stock Arch, and on CachyOS.

    /// The live /etc/os-release from the CachyOS box this port was developed
    /// on, verbatim: quoted and unquoted values mixed, and a value containing
    /// semicolons.
    const CACHYOS_OS_RELEASE: &str = r#"NAME="CachyOS Linux"
PRETTY_NAME="CachyOS"
ID=cachyos
ID_LIKE=arch
BUILD_ID=rolling
ANSI_COLOR="38;2;23;147;209"
HOME_URL="https://cachyos.org/"
DOCUMENTATION_URL="https://wiki.cachyos.org/"
SUPPORT_URL="https://discuss.cachyos.org/"
BUG_REPORT_URL="https://github.com/cachyos"
PRIVACY_POLICY_URL="https://terms.archlinux.org/docs/privacy-policy/"
LOGO=cachyos
"#;

    #[test]
    fn parse_distro_identifies_cachyos_from_the_live_os_release() {
        assert_eq!(parse_distro(CACHYOS_OS_RELEASE), Distro::CachyOs);
    }

    #[test]
    fn parse_distro_identifies_stock_arch() {
        // No ID_LIKE at all — pins that the ID_LIKE lookup is never *required*
        // to succeed for a host to be classified.
        assert_eq!(
            parse_distro("NAME=\"Arch Linux\"\nID=arch\nBUILD_ID=rolling\n"),
            Distro::Arch
        );
    }

    #[test]
    fn parse_distro_strips_double_and_single_quotes() {
        // An unstripped quote silently disables the whole CachyOS path:
        // invisible on the author's box, visible on someone else's.
        assert_eq!(parse_distro("ID=\"cachyos\"\n"), Distro::CachyOs);
        assert_eq!(parse_distro("ID='arch'\n"), Distro::Arch);
        assert_eq!(
            parse_distro("ID=\"\"\nID_LIKE=\"arch\"\n"),
            Distro::ArchDerivative
        );
        assert_eq!(
            os_release_value("ID=\"\"", "ID").as_deref(),
            Some(""),
            "an empty quoted value unquotes to empty, not to a stray quote"
        );
    }

    #[test]
    fn parse_distro_survives_crlf_line_endings() {
        // `str::lines()` strips \n but leaves \r, so without the trim the
        // comparison would be against "cachyos\r". Pins the trim as
        // load-bearing.
        assert_eq!(
            parse_distro("ID=cachyos\r\nID_LIKE=arch\r\n"),
            Distro::CachyOs
        );
        assert_eq!(parse_distro("ID=\"cachyos\"\r\n"), Distro::CachyOs);
    }

    #[test]
    fn parse_distro_reads_arch_out_of_a_multi_valued_id_like() {
        // Two rules at once: ID_LIKE is tokenised rather than substring
        // matched (so `archlinux` alone must not count), and `cachyos`
        // appearing in ID_LIKE does not make a host CachyOS.
        assert_eq!(
            parse_distro("ID=garuda\nID_LIKE=\"cachyos arch\"\n"),
            Distro::ArchDerivative
        );
        assert_eq!(
            parse_distro("ID=endeavouros\nID_LIKE=arch\n"),
            Distro::ArchDerivative
        );
        assert_eq!(
            parse_distro("ID=manjaro\nID_LIKE=\"archlinux arch\"\n"),
            Distro::ArchDerivative
        );
    }

    #[test]
    fn parse_distro_treats_unknown_missing_and_empty_as_unknown() {
        // The fallback the entire no-regression argument rests on.
        assert_eq!(parse_distro("ID=fedora\nID_LIKE=rhel\n"), Distro::Unknown);
        assert_eq!(parse_distro(""), Distro::Unknown);
        assert_eq!(parse_distro("# nothing\n\n   \n"), Distro::Unknown);
        assert_eq!(parse_distro("NAME=\"Some Linux\"\n"), Distro::Unknown);
    }

    #[test]
    fn os_release_value_matches_whole_keys_and_takes_the_last_assignment() {
        // A `starts_with("ID=")`-shaped parse would answer `ID` with
        // ID_LIKE's value and mislabel every derivative.
        assert_eq!(os_release_value("ID_LIKE=arch\n", "ID"), None);
        assert_eq!(
            os_release_value("ID=arch\nID=cachyos\n", "ID").as_deref(),
            Some("cachyos")
        );
        assert_eq!(os_release_value("#ID=fedora\n", "ID"), None);
        assert_eq!(os_release_value("no-equals-here\n", "ID"), None);
    }

    #[test]
    fn only_cachyos_diverges_from_todays_behaviour() {
        // The port's central safety invariant, as an executable assertion:
        // adding a fifth Distro variant forces a reviewer through here, and
        // any refactor that flips an Arch-family arm fails CI rather than
        // silently disabling the yay bootstrap on stock Arch.
        for d in [Distro::Arch, Distro::ArchDerivative, Distro::Unknown] {
            assert_eq!(
                chaotic_aur_policy(d),
                ChaoticAur::Add,
                "{d:?} must keep dotctl's pre-CachyOS behaviour"
            );
        }
        assert_eq!(
            chaotic_aur_policy(Distro::CachyOs),
            ChaoticAur::SkipRedundant
        );
    }

    #[test]
    fn aur_helper_hint_offers_pacman_only_on_cachyos() {
        // A hint that sends an Arch user to `error: target not found` is
        // worse than no hint at all.
        assert!(aur_helper_hint(Distro::CachyOs).contains("pacman -S"));
        for d in [Distro::Arch, Distro::ArchDerivative, Distro::Unknown] {
            let h = aur_helper_hint(d);
            assert!(h.contains("makepkg"), "{d:?} hint should point at makepkg");
            assert!(
                !h.contains("pacman -S"),
                "{d:?} has no official-repo yay/paru to pacman -S"
            );
        }
    }

    // ── pacman.conf parsing ──────────────────────────────────────────────

    #[test]
    fn pacman_conf_has_repo_tolerates_cachyos_trailing_space_headers() {
        // CachyOS really does ship `[cachyos-znver4] ` with a trailing space.
        // Nothing pinned that before, so a "simplify the trim away" cleanup
        // would make dotctl re-add repos and re-run strap.sh as root on every
        // single run.
        assert!(pacman_conf_has_repo(
            "[cachyos-znver4] \nInclude = /etc/pacman.d/cachyos-v4-mirrorlist\n",
            "cachyos-znver4"
        ));
        assert!(pacman_conf_has_repo("  [extra]\n", "extra"));
        assert!(!pacman_conf_has_repo("[core]\n[extra]\n", "blackarch"));
    }

    #[test]
    fn pacman_conf_has_repo_ignores_commented_out_sections() {
        // The inverse hazard: a `contains()`-shaped parse would see a
        // commented-out section and no-op both repo setups forever.
        assert!(!pacman_conf_has_repo(
            "#[blackarch]\n# Include = /etc/pacman.d/blackarch-mirrorlist\n",
            "blackarch"
        ));
        assert!(!pacman_conf_has_repo("  # [blackarch]\n", "blackarch"));
    }

    #[test]
    fn a_repo_line_without_a_sync_db_is_not_a_usable_repo() {
        // The bug this pins: an install that died between appending
        // `[chaotic-aur]` to pacman.conf and the `pacman -Sy` that fetches its
        // db. has_pacman_repo saw the line and said "present", `pacman -S yay`
        // still said "target not found", and every later run took the same
        // early return and never repaired it.
        assert_eq!(
            chaotic_state(true, false),
            ChaoticState::ConfiguredUnsynced,
            "a repo line with no sync db is the half-finished install, and must be re-synced \
             rather than reported as already present"
        );
        assert_eq!(
            chaotic_state(true, true),
            ChaoticState::Ready,
            "configured plus a sync db on disk is the only state in which `pacman -S yay` \
             can actually resolve"
        );
    }

    #[test]
    fn an_unconfigured_chaotic_aur_is_bootstrapped_whatever_is_in_the_sync_dir() {
        // The other half of the table. No `[chaotic-aur]` in pacman.conf means
        // the key, keyring, mirrorlist and repo line all still have to be
        // installed — and a stale chaotic-aur.db left behind by a repo someone
        // has since commented out must not be mistaken for a configured repo.
        assert_eq!(
            chaotic_state(false, false),
            ChaoticState::Absent,
            "a box that has never seen chaotic-aur gets the full bootstrap"
        );
        assert_eq!(
            chaotic_state(false, true),
            ChaoticState::Absent,
            "a leftover sync db from a removed repo must not skip the bootstrap"
        );
    }

    #[test]
    fn sync_db_path_names_the_file_pacman_actually_writes() {
        // The whole configured-vs-synced distinction rests on this one path:
        // `pacman -Sy` writes /var/lib/pacman/sync/<repo>.db, and on this
        // machine chaotic-aur.db sits there beside core.db and extra.db. Get
        // the directory or the `.db` suffix wrong and every configured repo
        // reads as unsynced, so `dotctl install` runs `pacman -Sy` on every
        // single invocation forever.
        assert_eq!(
            sync_db_path("chaotic-aur"),
            "/var/lib/pacman/sync/chaotic-aur.db",
            "this is the exact path pacman populates for [chaotic-aur]"
        );
        assert_eq!(
            sync_db_path("blackarch"),
            "/var/lib/pacman/sync/blackarch.db",
            "the convention is per-repo, not special-cased for chaotic-aur"
        );
    }
}
