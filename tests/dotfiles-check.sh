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
CHECK_PATH=""

start_case() {
  CASES=$((CASES + 1))
  CURRENT="$1"
  PROBLEMS=()
  RAN=0
  RC=0
  OUT=""
  FREPO="$WORK/repo.$CASES"
  FHOME="$WORK/home.$CASES"
  CHECK_PATH="$STUB_BIN:$PATH"
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
  printf '{}\n' >"$FREPO/.config/noctalia/settings.json"
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

# run_check [DOTFILES-override] -- run the subject against this case's fixtures
# with a hermetic HOME. Colour is stripped so the assertions read as the words
# the script prints.
run_check() {
  RAN=1
  RC=0
  OUT="$(env PATH="$CHECK_PATH" HOME="$FHOME" DOTFILES="${1:-$FREPO}" \
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
printf '\n%s case(s), %s failed\n' "$CASES" "$FAILED"
if ((FAILED)); then exit 1; fi
exit 0
