#!/usr/bin/env bash
# =============================================================================
# tests/dotfiles-check.sh -- exercise the post-deploy health check
#
# The subject is .local/bin/dotfiles-check, the only thing guarding this repo's
# stated invariant: a fresh clone must produce a bootable desktop. `exit 0` and
# "OK: all checks passed" from it mean "this deploy is intact", so every case
# below asks the same question: can it be talked into a green it did not earn?
#
# It could. The old check decided a symlink was healthy with
# `readlink -f "$path" == $DOTFILES/*`, and readlink -f prints a path whose
# final component does not exist -- so ~/.config/niri pointing at a tracked
# config that had been deleted read as OK, and a desktop with no compositor
# config passed. A $DOTFILES that did not exist at all skipped the symlink
# section entirely and still printed "OK: all checks passed", on the one
# machine state that most needed to shout.
#
# Hermetic by construction. Every case builds its own fake repo and fake home
# under one mktemp -d and hands them to the check as $DOTFILES and $HOME; the
# real ~ and the real repo are never read except by the last case, which reads
# src/main.rs to compare the deploy lists. A fake `systemctl` goes first on
# PATH so the units section is a fixed "no failed units" and cannot make a
# case depend on the developer's session.
#
# Usage: tests/dotfiles-check.sh [path-to-dotfiles-check]
#        DOTFILES_CHECK=... tests/dotfiles-check.sh
# =============================================================================
set -euo pipefail

# ---------------------------------------------------------------------------
# Locate the subject relative to this file, so the suite runs from any cwd.
# ---------------------------------------------------------------------------
SUITE_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(cd -- "$SUITE_DIR/.." && pwd)"
CHECK="${1:-${DOTFILES_CHECK:-$REPO_ROOT/.local/bin/dotfiles-check}}"
MAIN_RS="${DOTCTL_MAIN_RS:-$REPO_ROOT/src/main.rs}"

if [[ ! -r "$CHECK" ]]; then
  printf 'cannot read subject: %s\n' "$CHECK" >&2
  exit 2
fi

# Absolute path to this shell: the check is invoked as `bash "$CHECK"` rather
# than by its shebang, so the cases that strip PATH down to one tool still
# start.
BASH_BIN="${BASH:-$(command -v bash)}"

# ---------------------------------------------------------------------------
# Scratch space. Cleaned on every exit path, including a failing one.
# ---------------------------------------------------------------------------
WORK="$(mktemp -d "${TMPDIR:-/tmp}/dotfiles-check-test.XXXXXX")"
# shellcheck disable=SC2329  # invoked from the trap below
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT INT TERM

# ---------------------------------------------------------------------------
# The fake `systemctl`. The units section is not what these cases are about,
# and the developer's real session must not decide whether they pass: this
# answers every probe with success and no failed units.
# ---------------------------------------------------------------------------
STUB_BIN="$WORK/stub-bin"
mkdir -p "$STUB_BIN"
{
  printf '#!%s\n' "$BASH_BIN"
  printf '# Stand-in for systemctl: a reachable user manager, nothing failed.\n'
  printf 'exit 0\n'
} >"$STUB_BIN/systemctl"
chmod +x "$STUB_BIN/systemctl"

# A PATH with no systemctl on it at all, for the skip cases. readlink is the
# only external tool the symlink section uses; awk belongs to the units
# section, which is exactly what is being skipped.
NOSYSTEMD_BIN="$WORK/nosystemd-bin"
mkdir -p "$NOSYSTEMD_BIN"
ln -sf "$(command -v readlink)" "$NOSYSTEMD_BIN/readlink"
# timeout wraps the Noctalia probe, which these cases are not about either.
ln -sf "$(command -v timeout)" "$NOSYSTEMD_BIN/timeout"

# ---------------------------------------------------------------------------
# Stand-ins for what `dotctl install` provides, so the install sections are
# decided by each case's fixtures, never by what the developer or the runner
# happens to have. Each reads its state from a file under the case's fake HOME.
# They go into STUB_BIN and NOSYSTEMD_BIN alike: the cases that strip PATH down
# to drop systemctl are about systemctl, not about these.
# ---------------------------------------------------------------------------
stub() {  # stub <name> <line>... -- a bash script in STUB_BIN
  local name=$1
  shift
  { printf '#!%s\n' "$BASH_BIN"; printf '%s\n' "$@"; } >"$STUB_BIN/$name"
  chmod +x "$STUB_BIN/$name"
}
for t in npx github-mcp-server lazygit clangd; do stub "$t" 'exit 0'; done
# rustup's proxy answers on PATH even without the component; ~/.fake-ra-broken
# makes this one behave like that proxy.
# shellcheck disable=SC2016  # the stubs' $vars expand when they run
stub rust-analyzer \
  '[[ -e "$HOME/.fake-ra-broken" ]] && { echo "error: Unknown binary rust-analyzer" >&2; exit 1; }' \
  'echo "rust-analyzer 1.0.0"'
# `lookup service NAME` prints NAME's value from ~/.fake-keyring ("NAME VALUE"
# lines) with no newline, as the real one does, or exits 1 when nothing is stored.
# shellcheck disable=SC2016
stub secret-tool \
  '[[ "$1 $2" == "lookup service" ]] || exit 2' \
  'while read -r svc val; do [[ "$svc" == "$3" ]] && { printf "%s" "$val"; exit 0; }; done 2>/dev/null <"$HOME/.fake-keyring"' \
  'exit 1'
# shellcheck disable=SC2016
stub pkgfile \
  '[[ -e "$HOME/.fake-pkgfile-db" ]] && exit 0' \
  'echo "error: Database version file not found" >&2; exit 1'
# Records what it was handed -- argv and the header on stdin -- so a case can
# prove the token travels only on stdin, then replays ~/.fake-github as the
# response headers. No such file means "could not connect", curl's exit 7.
# Builtins only: the stripped-PATH cases have no cat to lean on.
# shellcheck disable=SC2016
stub curl \
  'IFS= read -r -d "" hdr || true' \
  'printf "%s" "$hdr" >"$HOME/.curl-stdin"' \
  'printf "%s\n" "$*" >"$HOME/.curl-argv"' \
  '[[ -f "$HOME/.fake-github" ]] || exit 7' \
  'printf "%s\n" "$(<"$HOME/.fake-github")"'
# The shell answers `msg status` unless ~/.fake-noctalia-down says it is not
# running, or ~/.fake-noctalia-wedged makes it hang the way a stuck one would.
# shellcheck disable=SC2016
stub noctalia \
  '[[ "$1 $2" == "msg status" ]] || exit 2' \
  '[[ -e "$HOME/.fake-noctalia-wedged" ]] && exec sleep 60' \
  '[[ -e "$HOME/.fake-noctalia-down" ]] && { echo "error: noctalia is not running" >&2; exit 1; }' \
  'echo "{}"'
# grogu reports the version in ~/.fake-grogu-version, or 0.2.0 -- the first that
# writes Noctalia v5's palette.
# shellcheck disable=SC2016
stub grogu \
  '[[ "${1:-}" == --version ]] || exit 0' \
  'v=0.2.0; [[ -f "$HOME/.fake-grogu-version" ]] && v="$(<"$HOME/.fake-grogu-version")"' \
  'echo "grogu $v"'
for t in npx github-mcp-server lazygit clangd rust-analyzer secret-tool pkgfile curl noctalia grogu; do
  ln -sf "$STUB_BIN/$t" "$NOSYSTEMD_BIN/$t"
done

# For the cases that take one stand-in away: the stand-ins plus only the real
# tools the check itself runs (readlink, awk for the units section, timeout for
# the shell probe, date for the token's expiry), so nothing on the developer's
# PATH can fill the gap.
BASE_BIN="$WORK/base-bin"
mkdir -p "$BASE_BIN"
for t in readlink awk timeout date; do ln -sf "$(command -v "$t")" "$BASE_BIN/$t"; done

# ---------------------------------------------------------------------------
# Case machinery. A failing case records why and keeps going: the point is the
# whole picture, not the first thing that broke.
# ---------------------------------------------------------------------------
CASES=0
FAILED=0
CURRENT=""
RAN=0
RC=0
OUT=""
PROBLEMS=()
FREPO=""
FHOME=""
FROOT=""
CHECK_PATH=""
CHECK_WAYLAND=""

start_case() {
  CASES=$((CASES + 1))
  CURRENT="$1"
  PROBLEMS=()
  RAN=0
  RC=0
  OUT=""
  FREPO="$WORK/repo.$CASES"
  FHOME="$WORK/home.$CASES"
  FROOT="$WORK/root.$CASES"  # the /etc and /usr the check reads
  CHECK_PATH="$STUB_BIN:$PATH"
  # Every case is "inside a Wayland session" unless it says otherwise: the
  # developer's WAYLAND_DISPLAY and CI's lack of one must not decide a case.
  CHECK_WAYLAND="wayland-test"
}

note() { PROBLEMS+=("$1"); }

# seed_repo -- a fake $DOTFILES holding one of each thing dotctl deploys: a
# .config tree, a bare .config file, a home dotfile, the tracked .gitconfig, a
# unit and a .local/bin script. The last two are the per-file cases, which
# link_dotfiles() enumerates rather than naming.
seed_repo() {
  mkdir -p "$FREPO/.config/niri" "$FREPO/.config/noctalia" "$FREPO/.config/kitty" \
    "$FREPO/.config/systemd/user" "$FREPO/.local/bin"
  printf 'output "eDP-1" {}\n' >"$FREPO/.config/niri/config.kdl"
  printf '[theme]\nsource = "custom"\n' >"$FREPO/.config/noctalia/config.toml"
  printf 'font_size 12\n' >"$FREPO/.config/kitty/kitty.conf"
  printf 'add_newline = false\n' >"$FREPO/.config/starship.toml"
  printf 'export EDITOR=nvim\n' >"$FREPO/.zshrc"
  printf '[core]\n\tpager = delta\n' >"$FREPO/.gitconfig"
  printf '[Unit]\nDescription=fake\n' >"$FREPO/.config/systemd/user/awww.service"
  printf '#!/bin/sh\n:\n' >"$FREPO/.local/bin/wallpaper"
}

# seed_home -- the home a successful `dotctl deploy` leaves behind for that
# repo. ~/.gitconfig is a real file on purpose: dotctl writes a stub there and
# links the tracked config to ~/.config/git/dotfiles.config, so a check that
# demanded a symlink at ~/.gitconfig would be wrong about a healthy machine.
seed_home() {
  mkdir -p "$FHOME/.config/git" "$FHOME/.config/systemd/user" "$FHOME/.local/bin"
  ln -sfn "$FREPO/.config/niri" "$FHOME/.config/niri"
  ln -sfn "$FREPO/.config/noctalia" "$FHOME/.config/noctalia"
  ln -sfn "$FREPO/.config/kitty" "$FHOME/.config/kitty"
  ln -sfn "$FREPO/.config/starship.toml" "$FHOME/.config/starship.toml"
  ln -sfn "$FREPO/.zshrc" "$FHOME/.zshrc"
  ln -sfn "$FREPO/.gitconfig" "$FHOME/.config/git/dotfiles.config"
  ln -sfn "$FREPO/.config/systemd/user/awww.service" \
    "$FHOME/.config/systemd/user/awww.service"
  ln -sfn "$FREPO/.local/bin/wallpaper" "$FHOME/.local/bin/wallpaper"
  printf '[include]\n\tpath = ~/.config/git/dotfiles.config\n' >"$FHOME/.gitconfig"
}

# seed_installed -- what a completed `dotctl install` leaves: HexStrike with its
# Host guard, both secrets in the keyring, a token GitHub accepts (no expiry
# header, so the date arithmetic is not reached on a stripped PATH), a pkgfile
# database, grogu's palette, the SDDM theme and ReGreet background dotctl names,
# and the wallpapers clone.
seed_installed() {
  mkdir -p "$FHOME/tools/hexstrike-ai" "$FREPO/wallpapers/.git"
  printf 'app = Flask(__name__)\n\n# dotctl: loopback-Host guard. inserted\n' \
    >"$FHOME/tools/hexstrike-ai/hexstrike_server.py"
  printf 'github-mcp-server github_pat_FAKE\nanthropic sk-ant-FAKE\n' >"$FHOME/.fake-keyring"
  printf 'HTTP/2 200\r\n\r\n' >"$FHOME/.fake-github"
  : >"$FHOME/.fake-pkgfile-db"
  mkdir -p "$FREPO/.config/noctalia/palettes" "$FROOT/etc/sddm.conf.d" "$FROOT/etc/greetd" \
    "$FROOT/usr/share/sddm/themes/noctalia" "$FROOT/usr/share/noctalia/assets"
  printf '{"dark": {}, "light": {}}\n' >"$FREPO/.config/noctalia/palettes/Grogu.json"
  printf '[Theme]\nCurrent=noctalia\n' >"$FROOT/etc/sddm.conf.d/10-noctalia-theme.conf"
  printf '[background]\npath = "/usr/share/noctalia/assets/noctalia-wallpaper.png"\n' \
    >"$FROOT/etc/greetd/regreet.toml"
  : >"$FROOT/usr/share/noctalia/assets/noctalia-wallpaper.png"
}

# run_check [DOTFILES-override] -- run the subject against this case's fixtures
# with a hermetic HOME. Colour is stripped so the assertions read as the words
# the script prints.
run_check() {
  RAN=1
  RC=0
  OUT="$(env PATH="$CHECK_PATH" HOME="$FHOME" DOTFILES="${1:-$FREPO}" WAYLAND_DISPLAY="$CHECK_WAYLAND" \
    DOTFILES_CHECK_ROOT="$FROOT" \
    "$BASH_BIN" "$CHECK" 2>&1)" || RC=$?
  OUT="$(printf '%s\n' "$OUT" | sed "s/$(printf '\033')\[[0-9;]*m//g")"
}

# run_check_repovar -- run the subject with the repo location supplied as
# whatever env assignments the caller passes (DOTFILES / DOTFILES_REPO), rather
# than run_check's fixed DOTFILES=$FREPO. Both names are stripped from the
# inherited environment first so each case states exactly what is set. Used only
# by the repo-path precedence cases; everything else goes through run_check.
run_check_repovar() {
  RAN=1
  RC=0
  OUT="$(env -u DOTFILES -u DOTFILES_REPO PATH="$CHECK_PATH" HOME="$FHOME" WAYLAND_DISPLAY="$CHECK_WAYLAND" DOTFILES_CHECK_ROOT="$FROOT" "$@" \
    "$BASH_BIN" "$CHECK" 2>&1)" || RC=$?
  OUT="$(printf '%s\n' "$OUT" | sed "s/$(printf '\033')\[[0-9;]*m//g")"
}

expect_rc() {
  if [[ "$RC" != "$1" ]]; then note "expected rc $1, actual rc $RC"; fi
}

expect_out() {
  if [[ "$OUT" != *"$1"* ]]; then note "output does not contain: $1"; fi
}

expect_no_out() {
  if [[ "$OUT" == *"$1"* ]]; then note "output should not contain: $1"; fi
}

# expect_fail_count -- the summary must report how many checks failed, not
# whether any did. It used to be a 0/1 flag printed as a count, so a machine
# with a dozen broken links reported "1 check(s) failed".
expect_fail_count() {
  if [[ "$OUT" != *"FAIL: $1 check(s) failed"* ]]; then
    note "expected the summary to say $1 check(s) failed"
  fi
}

# expect_healthy_line -- the given path must be reported OK by name, so a case
# cannot pass because the path silently fell off the list of things checked.
expect_healthy_line() {
  if [[ "$OUT" != *"OK   $1"* ]]; then note "expected a green line for $1"; fi
}

end_case() {
  local p line
  if ((${#PROBLEMS[@]} == 0)); then
    printf 'PASS  %s\n' "$CURRENT"
    return 0
  fi
  FAILED=$((FAILED + 1))
  printf 'FAIL  %s\n' "$CURRENT"
  for p in "${PROBLEMS[@]}"; do
    printf '        %s\n' "$p"
  done
  if ((RAN)); then
    printf '        actual rc: %s\n' "$RC"
    printf '        actual output:\n'
    while IFS= read -r line; do
      printf '          | %s\n' "$line"
    done <<<"$OUT"
  fi
}

# =============================================================================
# 1. The baseline. If a correctly deployed home does not come back green, every
#    negative case below is meaningless.
# =============================================================================
start_case "a_fully_deployed_home_is_reported_healthy"
seed_repo
seed_home
run_check
expect_rc 0
expect_out "OK: all checks passed"
expect_no_out "FAIL"
# Named individually: the per-file halves of the deploy (units, .local/bin)
# are enumerated from the repo rather than listed, and a bug there would show
# up as silence, not as red.
expect_healthy_line "$FHOME/.config/niri"
expect_healthy_line "$FHOME/.config/starship.toml"
expect_healthy_line "$FHOME/.zshrc"
expect_healthy_line "$FHOME/.config/git/dotfiles.config"
expect_healthy_line "$FHOME/.config/systemd/user/awww.service"
expect_healthy_line "$FHOME/.local/bin/wallpaper"
end_case

# =============================================================================
# 2. The bug this suite exists for: readlink -f resolves a path whose final
#    component is missing, so a link into the repo at a tracked config that has
#    since been deleted printed a green OK. That is a desktop with no niri
#    config certified healthy.
# =============================================================================
start_case "a_link_into_the_repo_whose_target_was_deleted_is_reported_dangling"
seed_repo
seed_home
rm -rf "$FREPO/.config/niri"
run_check
expect_rc 1
expect_out "$FHOME/.config/niri"
expect_out "dangling"
expect_fail_count 1
expect_no_out "OK: all checks passed"
end_case

# =============================================================================
# 3. A link that aims somewhere else entirely -- another dotfiles manager, or a
#    repo that was moved and re-linked by hand. Deliberately ~/.config/kitty:
#    the old check looked at four paths and kitty was not one of them, so this
#    machine came back completely green.
# =============================================================================
start_case "a_link_pointing_outside_dotfiles_is_reported"
seed_repo
seed_home
mkdir -p "$WORK/elsewhere.$CASES/kitty"
ln -sfn "$WORK/elsewhere.$CASES/kitty" "$FHOME/.config/kitty"
run_check
expect_rc 1
expect_out "$FHOME/.config/kitty"
expect_out "outside repo"
expect_fail_count 1
end_case

# =============================================================================
# 4. Nothing deployed at that path at all -- an interrupted deploy, or a config
#    the user deleted by hand.
# =============================================================================
start_case "a_link_that_was_never_deployed_is_reported_missing"
seed_repo
seed_home
rm "$FHOME/.config/niri"
run_check
expect_rc 1
expect_out "$FHOME/.config/niri is missing"
expect_fail_count 1
end_case

# =============================================================================
# 5. A real file standing where a link belongs: the config is not the tracked
#    one, edits to the repo never reach it, and the next deploy will move it
#    aside into ~/.dotfiles-backup.
# =============================================================================
start_case "a_real_file_where_a_link_belongs_is_reported"
seed_repo
seed_home
rm "$FHOME/.zshrc"
printf 'export EDITOR=vi\n' >"$FHOME/.zshrc"
run_check
expect_rc 1
expect_out "$FHOME/.zshrc is a real file, not a symlink"
expect_fail_count 1
end_case

# =============================================================================
# 6. The same, one directory up: a whole real ~/.config/niri directory, which
#    is what a user gets after installing niri and letting it write defaults.
# =============================================================================
start_case "a_real_directory_where_a_link_belongs_is_reported"
seed_repo
seed_home
rm "$FHOME/.config/niri"
mkdir -p "$FHOME/.config/niri"
printf 'output "eDP-1" {}\n' >"$FHOME/.config/niri/config.kdl"
run_check
expect_rc 1
expect_out "$FHOME/.config/niri is a real directory, not a symlink"
expect_fail_count 1
end_case

# =============================================================================
# 7. No repo at all -- the state that produces a completely dead desktop, since
#    every link deployed into $HOME dangles at once. It used to skip the whole
#    symlink section and print "OK: all checks passed", because skip() never
#    touched the failure flag.
# =============================================================================
start_case "a_missing_dotfiles_repo_is_a_hard_failure_not_a_skip"
seed_repo
seed_home
run_check "$WORK/no-such-repo.$CASES"
expect_rc 1
expect_out "\$DOTFILES does not exist"
expect_fail_count 1
expect_no_out "OK: all checks passed"
expect_no_out "SKIP \$DOTFILES"
end_case

# =============================================================================
# 8. A repo that exists but tracks none of what dotctl deploys: a partial
#    clone, or $DOTFILES aimed at the wrong directory. The list of things to
#    check comes out empty, so every loop passes vacuously -- the same false
#    green as the skip above, one step further along.
# =============================================================================
start_case "a_repo_that_tracks_nothing_dotctl_deploys_is_not_a_clean_bill"
mkdir -p "$FREPO" "$FHOME"
run_check
expect_rc 1
expect_out "tracks none of the paths dotctl deploys"
expect_no_out "OK: all checks passed"
end_case

# =============================================================================
# 9. Units and .local/bin scripts are linked file by file, so a tracked unit
#    that was deleted or renamed leaves a dangling link behind in a directory
#    the repo no longer mentions. Checking only what the repo currently ships
#    would look right past it.
# =============================================================================
start_case "a_deployed_unit_link_whose_tracked_file_vanished_is_still_checked"
seed_repo
seed_home
rm "$FREPO/.config/systemd/user/awww.service"
run_check
expect_rc 1
expect_out "$FHOME/.config/systemd/user/awww.service"
expect_out "dangling"
expect_fail_count 1
end_case

# =============================================================================
# 10. The count in the summary is the number of failures. It used to be a 0/1
#     flag printed as a count, so every unhealthy machine, whatever was wrong
#     with it, reported "1 check(s) failed".
# =============================================================================
start_case "the_summary_counts_every_failure_not_just_the_first"
seed_repo
seed_home
rm -rf "$FREPO/.config/niri"                                  # dangling
rm "$FHOME/.zshrc"                                            # missing
mkdir -p "$WORK/elsewhere.$CASES"
ln -sfn "$WORK/elsewhere.$CASES" "$FHOME/.config/kitty"       # outside repo
rm "$FHOME/.config/starship.toml"
printf 'add_newline = true\n' >"$FHOME/.config/starship.toml" # real file
run_check
expect_rc 1
expect_fail_count 4
end_case

# =============================================================================
# 11. SKIP still exists for what is genuinely optional -- a host with no user
#     systemd manager has no units to have failed -- but it must never be the
#     reason an unwell machine comes back green. Same broken home as case 2,
#     with systemctl taken off PATH.
# =============================================================================
start_case "a_skip_cannot_make_an_unwell_machine_report_ok"
seed_repo
seed_home
rm -rf "$FREPO/.config/niri"
CHECK_PATH="$NOSYSTEMD_BIN"
run_check
expect_rc 1
expect_out "SKIP"
expect_out "dangling"
expect_fail_count 1
expect_no_out "OK: all checks passed"
end_case

# =============================================================================
# 12. And when a skip does happen on a machine that is otherwise fine, the
#     summary says so, so "all checks passed" is never read as "every check
#     ran".
# =============================================================================
start_case "a_healthy_run_that_skipped_something_says_how_much_it_skipped"
seed_repo
seed_home
seed_installed # so the systemctl skip is the only one, and the count is exact
CHECK_PATH="$NOSYSTEMD_BIN"
run_check
expect_rc 0
expect_out "SKIP"
expect_out "OK: all checks passed (1 skipped)"
end_case

# =============================================================================
# 13. The drift guard.
#
# The by-name half of the deploy list is spelled twice: once as Rust literals
# in link_dotfiles(), once as CONFIG_ITEMS/HOME_ITEMS in the check. Bash cannot
# read the Rust and the check must run before dotctl is built, so the copy
# stays -- and this case is what stops it from silently going stale, which is
# how the old check ended up testing four of the twenty paths it deploys.
#
# `dotfiles-check --list` prints that half, repo-relative. Anything added to
# either Rust list, or dropped from it, fails here.
# =============================================================================
start_case "the_checks_deploy_list_still_matches_link_dotfiles_in_src_main_rs"
if [[ ! -r "$MAIN_RS" ]]; then
  note "cannot read $MAIN_RS -- the drift guard cannot run (DOTCTL_MAIN_RS=...)"
else
  # Only link_dotfiles()'s own body: `for d in [` also appears in the tests
  # further down main.rs.
  _body="$(awk '/^fn link_dotfiles/{inside = 1} inside {print} inside && /^}/{exit}' "$MAIN_RS")"
  _configs="$(printf '%s\n' "$_body" |
    awk '/for d in \[/{grab = 1; next} grab && /^ *\] \{/{exit} grab' |
    grep -o '"[^"]*"' | tr -d '"' | sed 's|^|.config/|' || true)"
  _homes="$(printf '%s\n' "$_body" | grep -m1 'for f in \[' |
    grep -o '"[^"]*"' | tr -d '"' || true)"
  if [[ -z "$_configs" || -z "$_homes" ]]; then
    # The parse, not the lists, is what broke. Say so: a guard that quietly
    # compares two empty sets is worse than no guard.
    note "could not parse the deploy lists out of $MAIN_RS -- this case is testing nothing"
  else
    # .gitconfig is deployed by setup_gitconfig(), which is not one of the two
    # array literals, so it is added here by hand rather than parsed.
    _expected="$(printf '%s\n%s\n.gitconfig\n' "$_configs" "$_homes" | LC_ALL=C sort)"
    _actual="$(env PATH="$CHECK_PATH" "$BASH_BIN" "$CHECK" --list | LC_ALL=C sort)"
    if [[ "$_expected" != "$_actual" ]]; then
      note "dotfiles-check --list has drifted from link_dotfiles():"
      while IFS= read -r _line; do
        note "  $_line"
      done < <(diff <(printf '%s\n' "$_expected") <(printf '%s\n' "$_actual") |
        sed 's/^</  only in src\/main.rs:/; s/^>/  only in dotfiles-check:/')
    fi
  fi
fi
end_case
unset _body _configs _homes _expected _actual _line

# ---------------------------------------------------------------------------
# =============================================================================
# 14-16. Repo-path variable precedence.
#
# dotctl's own flag is `#[arg(long, env = "DOTFILES_REPO")]`, so DOTFILES_REPO
# is the name a user who clones elsewhere has already exported. The check keyed
# off DOTFILES alone, so on such a machine it aimed at the default path while
# dotctl deployed to another -- and reported FAIL for links that were fine,
# which is the one thing it must never get wrong. The resolution is now
# DOTFILES_REPO > DOTFILES > default; these three pin it. run_check_repovar
# strips both names from the environment first, so each case sets exactly one.
# =============================================================================
start_case "dotfiles_repo_the_name_dotctl_uses_points_the_check"
seed_repo
seed_home
# FREPO is not at $HOME/niri-dotfiles, so a green run can only mean DOTFILES_REPO
# was read: a check that ignored it would fall back to the absent default here.
run_check_repovar DOTFILES_REPO="$FREPO"
expect_rc 0
expect_out "OK: all checks passed"
expect_no_out "FAIL"
end_case

start_case "the_legacy_dotfiles_name_still_works_when_dotfiles_repo_is_unset"
seed_repo
seed_home
# Nobody's muscle memory breaks: DOTFILES on its own still locates the repo.
run_check_repovar DOTFILES="$FREPO"
expect_rc 0
expect_out "OK: all checks passed"
expect_no_out "FAIL"
end_case

start_case "dotfiles_repo_wins_over_dotfiles_when_both_are_set"
seed_repo
seed_home
# DOTFILES points at a path that does not exist; were it consulted the run would
# report "$DOTFILES does not exist". A green run proves DOTFILES_REPO took
# precedence -- the fix for someone who set dotctl's name over a stale DOTFILES.
run_check_repovar DOTFILES_REPO="$FREPO" DOTFILES="$WORK/stale-dotfiles.$CASES"
expect_rc 0
expect_out "OK: all checks passed"
expect_no_out "does not exist"
end_case

# =============================================================================
# The install sections: what `dotctl install` sets up, checked by name.
# =============================================================================
# path_without <tool> -- every stand-in except <tool>, plus only the real tools
# the check runs, so a missing program cannot be found anywhere else on PATH.
path_without() {
  local dir="$WORK/bin-without.$CASES" f
  mkdir -p "$dir"
  for f in "$STUB_BIN"/*; do
    [[ "${f##*/}" == "$1" ]] || ln -sf "$f" "$dir/${f##*/}"
  done
  CHECK_PATH="$dir:$BASE_BIN"
}

start_case "a_machine_dotctl_install_has_finished_is_green_in_every_install_section"
seed_repo
seed_home
seed_installed
run_check
expect_rc 0
expect_out "OK: all checks passed"
expect_no_out "FAIL"
expect_no_out "WARN"
for t in npx github-mcp-server secret-tool lazygit clangd pkgfile rust-analyzer; do
  expect_healthy_line "$t"
done
expect_out "Host guard present"
expect_out "something is stored under service github-mcp-server"
expect_out "something is stored under service anthropic"
expect_out "GitHub accepts the stored token"
expect_out "command-not-found can name packages"
expect_out "private wallpapers clone present"
expect_out "OK   grogu 0.2.0"
expect_out "Noctalia's Grogu palette present"
expect_out "SDDM theme noctalia installed"
expect_out "ReGreet background present"
end_case

start_case "a_program_the_configs_call_that_is_missing_fails_by_name"
seed_repo
seed_home
seed_installed
path_without lazygit
run_check
expect_rc 1
expect_out "FAIL lazygit not installed"
expect_fail_count 1
end_case

start_case "a_rust_analyzer_proxy_without_its_component_fails"
seed_repo
seed_home
seed_installed
: >"$FHOME/.fake-ra-broken"
run_check
expect_rc 1
expect_out "rust-analyzer does not run"
expect_fail_count 1
end_case

start_case "hexstrike_without_the_host_guard_fails"
seed_repo
seed_home
seed_installed
printf 'app = Flask(__name__)\n' >"$FHOME/tools/hexstrike-ai/hexstrike_server.py"
run_check
expect_rc 1
expect_out "no loopback-Host guard"
expect_fail_count 1
end_case

start_case "a_secret_nobody_stored_warns_and_does_not_fail"
seed_repo
seed_home
seed_installed
printf 'github-mcp-server github_pat_FAKE\n' >"$FHOME/.fake-keyring"
run_check
expect_rc 0
expect_out "WARN nothing stored under service anthropic"
expect_no_out "FAIL"
end_case

start_case "a_github_token_close_to_expiry_warns"
seed_repo
seed_home
seed_installed
printf 'HTTP/2 200\r\ngithub-authentication-token-expiration: %s\r\n\r\n' \
  "$(date -u -d '+5 days' '+%Y-%m-%d %H:%M:%S UTC')" >"$FHOME/.fake-github"
run_check
expect_rc 0
expect_out "WARN the GitHub token expires in"
expect_no_out "FAIL"
end_case

start_case "a_github_token_far_from_expiry_is_reported_with_its_date"
seed_repo
seed_home
seed_installed
EXP="$(date -u -d '+60 days' '+%Y-%m-%d %H:%M:%S UTC')"
printf 'HTTP/2 200\r\ngithub-authentication-token-expiration: %s\r\n\r\n' "$EXP" >"$FHOME/.fake-github"
run_check
expect_rc 0
expect_out "valid until $EXP"
expect_no_out "WARN"
end_case

start_case "a_github_token_github_rejects_fails"
seed_repo
seed_home
seed_installed
printf 'HTTP/2 401\r\n\r\n' >"$FHOME/.fake-github"
run_check
expect_rc 1
expect_out "GitHub rejects the stored token"
expect_fail_count 1
end_case

start_case "the_github_token_travels_only_on_stdin_and_is_never_printed"
seed_repo
seed_home
seed_installed
printf 'github-mcp-server github_pat_SHOULDNOTLEAK\nanthropic sk-ant-FAKE\n' >"$FHOME/.fake-keyring"
run_check
expect_rc 0
expect_no_out "SHOULDNOTLEAK"
if grep -q SHOULDNOTLEAK "$FHOME/.curl-argv" 2>/dev/null; then
  note "the token reached curl's argv"
fi
if [[ "$(cat "$FHOME/.curl-stdin" 2>/dev/null)" != "Authorization: Bearer github_pat_SHOULDNOTLEAK" ]]; then
  note "curl was not handed exactly the Authorization header on stdin"
fi
end_case

start_case "github_out_of_reach_skips_the_token_check_instead_of_failing"
seed_repo
seed_home
seed_installed
rm -f "$FHOME/.fake-github"
run_check
expect_rc 0
expect_out "could not reach api.github.com"
expect_no_out "FAIL"
end_case

start_case "no_pkgfile_database_warns_and_does_not_fail"
seed_repo
seed_home
seed_installed
rm -f "$FHOME/.fake-pkgfile-db"
run_check
expect_rc 0
expect_out "WARN no database yet"
expect_no_out "FAIL"
end_case

start_case "the_guard_sentinel_checked_is_the_one_dotctl_writes"
# bash cannot share the Rust const, so the two spellings are diffed here.
WANT="$(sed -n 's/^const HOST_GUARD_SENTINEL: &str = "\(.*\)";$/\1/p' "$REPO_ROOT/src/main.rs")"
if [[ -z "$WANT" ]]; then
  note "HOST_GUARD_SENTINEL not found in src/main.rs"
elif ! grep -qF "*\"$WANT\"*" "$CHECK"; then
  note "dotfiles-check does not test for the sentinel dotctl writes: $WANT"
fi
end_case

# =============================================================================
# The Noctalia shell. niri spawning a shell that does not run is a session
# with no notifications and no lock screen, and nothing else reports it.
# =============================================================================
start_case "a_running_noctalia_is_reported_ok"
seed_repo
seed_home
seed_installed
run_check
expect_rc 0
expect_out "OK   Noctalia is running"
end_case

start_case "a_noctalia_that_does_not_answer_fails"
seed_repo
seed_home
seed_installed
: >"$FHOME/.fake-noctalia-down"
run_check
expect_rc 1
expect_out "FAIL Noctalia is installed but not answering"
expect_fail_count 1
end_case

start_case "noctalia_not_installed_fails_by_name"
seed_repo
seed_home
seed_installed
path_without noctalia
run_check
expect_rc 1
expect_out "FAIL noctalia not installed"
expect_fail_count 1
end_case

# Over ssh there is no session to ask. The shell is down in the fixtures, so a
# check that asked anyway would go red: the skip has to come first.
start_case "outside_a_wayland_session_the_shell_check_skips"
seed_repo
seed_home
seed_installed
: >"$FHOME/.fake-noctalia-down"
CHECK_WAYLAND=""
run_check
expect_rc 0
expect_out "SKIP not in a Wayland session"
expect_no_out "FAIL"
end_case

start_case "a_wedged_noctalia_cannot_hang_the_check"
seed_repo
seed_home
seed_installed
: >"$FHOME/.fake-noctalia-wedged"
STARTED=$SECONDS
run_check
ELAPSED=$((SECONDS - STARTED))
expect_rc 1
expect_out "FAIL Noctalia is installed but not answering"
((ELAPSED < 30)) || note "the check waited ${ELAPSED}s on a shell that never answered"
end_case

# What retiring a config leaves behind: the repo drops .config/wal, and the
# link an older deploy made dangles under a name no list holds any more.
start_case "a_dangling_link_for_a_config_the_repo_dropped_is_reported"
seed_repo
seed_home
seed_installed
ln -s "$FREPO/.config/wal" "$FHOME/.config/wal"
ln -s "$FREPO/.retired-dotfile" "$FHOME/.retired-dotfile"
ln -s "$WORK/nowhere" "$FHOME/.config/their-own"   # not into the repo: not ours
run_check
expect_rc 1
expect_out "$FHOME/.config/wal -> $FREPO/.config/wal (dangling"
expect_out "$FHOME/.retired-dotfile -> $FREPO/.retired-dotfile (dangling"
expect_no_out "their-own"
expect_fail_count 2
end_case

# =============================================================================
# The theme pipeline and the login screens.
# =============================================================================
start_case "a_grogu_older_than_0_2_fails"
seed_repo
seed_home
seed_installed
printf '0.1.0\n' >"$FHOME/.fake-grogu-version"
run_check
expect_rc 1
expect_out "FAIL grogu 0.1.0 predates 0.2.0"
expect_fail_count 1
end_case

# Numeric, not lexical: as strings, "0.10.0" sorts before "0.2.0".
start_case "grogu_0_10_is_not_older_than_0_2"
seed_repo
seed_home
seed_installed
printf '0.10.0\n' >"$FHOME/.fake-grogu-version"
run_check
expect_rc 0
expect_out "OK   grogu 0.10.0"
end_case

start_case "a_grogu_that_reports_no_version_fails"
seed_repo
seed_home
seed_installed
printf 'unknown\n' >"$FHOME/.fake-grogu-version"
run_check
expect_rc 1
expect_out "FAIL grogu reports no version"
end_case

start_case "grogu_not_installed_fails_by_name"
seed_repo
seed_home
seed_installed
path_without grogu
run_check
expect_rc 1
expect_out "FAIL grogu not installed"
expect_fail_count 1
end_case

# The hook names ~/.cargo/bin/grogu, which need not be on PATH.
start_case "grogu_in_cargo_bin_counts_without_being_on_path"
seed_repo
seed_home
seed_installed
path_without grogu
mkdir -p "$FHOME/.cargo/bin"
ln -s "$STUB_BIN/grogu" "$FHOME/.cargo/bin/grogu"
run_check
expect_rc 0
expect_out "OK   grogu 0.2.0"
end_case

start_case "no_palette_yet_warns_and_does_not_fail"
seed_repo
seed_home
seed_installed
rm "$FREPO/.config/noctalia/palettes/Grogu.json"
run_check
expect_rc 0
expect_out "WARN no Grogu palette yet"
expect_no_out "FAIL"
end_case

start_case "an_sddm_theme_that_is_not_installed_fails"
seed_repo
seed_home
seed_installed
rm -r "$FROOT/usr/share/sddm/themes/noctalia"
run_check
expect_rc 1
expect_out "FAIL SDDM theme noctalia is not installed"
expect_fail_count 1
end_case

# .local/bin/sddm-theme switches the drop-in: check the theme it names.
start_case "the_sddm_theme_checked_is_the_one_the_drop_in_names"
seed_repo
seed_home
seed_installed
printf '[Theme]\nCurrent=maya\n' >"$FROOT/etc/sddm.conf.d/10-noctalia-theme.conf"
run_check
expect_rc 1
expect_out "FAIL SDDM theme maya is not installed"
mkdir -p "$FROOT/usr/share/sddm/themes/maya"
run_check
expect_rc 0
expect_out "OK   SDDM theme maya installed"
end_case

# What the Noctalia v5 update did: the image ReGreet named was deleted.
start_case "a_regreet_background_that_is_gone_warns"
seed_repo
seed_home
seed_installed
rm "$FROOT/usr/share/noctalia/assets/noctalia-wallpaper.png"
run_check
expect_rc 0
expect_out "WARN ReGreet's background /usr/share/noctalia/assets/noctalia-wallpaper.png is gone"
expect_no_out "FAIL"
end_case

start_case "the_regreet_path_checked_is_the_one_dotctl_ships"
# Drift guard: the tracked asset's background must be a path this suite's
# installed fixture provides, or the cases above test a different file.
WANT="$(sed -n 's/^path = "\(.*\)"$/\1/p' "$REPO_ROOT/assets/greetd/regreet.toml")"
[[ "$WANT" == /usr/share/noctalia/assets/noctalia-wallpaper.png ]] ||
  note "assets/greetd/regreet.toml names $WANT; update seed_installed to match"
end_case

printf '\n%s case(s), %s failed\n' "$CASES" "$FAILED"
if ((FAILED)); then exit 1; fi
exit 0
