#!/usr/bin/env bash
# =============================================================================
# tests/dotctl-selfupdate.sh -- drive dotctl's self-rebuild end to end
#
# `dotctl all` pulls the checkout, then runs install() and deploy() -- the ones
# compiled into the binary already running. So the binary fingerprints the
# source it was built from, and when the checkout's differs it rebuilds itself
# with `cargo install --path` and execs the new one with the same arguments.
# The Rust tests pin the fingerprint; this pins the behaviour around it, on the
# real binary: one rebuild, the same arguments, no rebuild loop, and only a
# warning from `install`.
#
# Hermetic, and nothing in it can reach install()'s real work: the stale `all`
# case is exec'd away into a fake dotctl first, and the other cases run with no
# cargo on PATH, so install() stops at "cargo not found" before touching
# anything. The fake checkout is `git init` plus copies, not a clone: CI's
# checkout is shallow, and dotctl only needs a .git for its pull to fail on.
#
# Usage: tests/dotctl-selfupdate.sh <path-to-built-dotctl>
# =============================================================================
set -euo pipefail

DOTCTL="$(realpath "${1:?usage: tests/dotctl-selfupdate.sh <path-to-built-dotctl>}")"
SUITE_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(cd -- "$SUITE_DIR/.." && pwd)"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/dotctl-selfupdate-test.XXXXXX")"
# shellcheck disable=SC2329  # invoked from the trap below
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT INT TERM

# The files the binary fingerprints, read from SELF_SOURCE rather than copied
# here, so this cannot drift from src/main.rs.
mapfile -t SOURCES < <(sed -n '/^const SELF_SOURCE/,/^\];/p' "$REPO_ROOT/src/main.rs" |
  grep -o '"[^"]*"' | tr -d '"')

REPO="$WORK/repo"
mkdir -p "$REPO"
git -C "$REPO" init -q
for f in "${SOURCES[@]}"; do
  mkdir -p "$REPO/$(dirname "$f")"
  cp "$REPO_ROOT/$f" "$REPO/$f"
done
# A non-empty wallpapers/ that is not the private clone: dotctl leaves it alone,
# so the case that reaches ensure_wallpapers() never touches the network.
mkdir -p "$REPO/wallpapers"
: >"$REPO/wallpapers/placeholder"

# sh and git are all dotctl needs before install(); cargo is deliberately absent.
MINBIN="$WORK/minbin"
FAKES="$WORK/fakes"
mkdir -p "$MINBIN" "$FAKES"
for t in sh git; do ln -s "$(command -v "$t")" "$MINBIN/$t"; done
# shellcheck disable=SC2016  # the fakes' $vars expand when they run
{
  printf '#!/bin/sh\necho "cargo $*" >>"%s/calls"\n' "$WORK" >"$FAKES/cargo"
  printf '#!/bin/sh\necho "dotctl $* DOTCTL_REBUILT=${DOTCTL_REBUILT:-unset}" >>"%s/calls"\n' "$WORK" >"$FAKES/dotctl"
}
chmod +x "$FAKES/cargo" "$FAKES/dotctl"

CASES=0
FAILED=0
CURRENT=""
PROBLEMS=()
RC=0
OUT=""

start_case() {
  CASES=$((CASES + 1))
  CURRENT="$1"
  PROBLEMS=()
  rm -f "$WORK/calls"
}
note() { PROBLEMS+=("$1"); }
calls() { cat "$WORK/calls" 2>/dev/null || true; }
run() {  # run <PATH> [VAR=value ...] -- <dotctl args...>
  local path=$1
  shift
  local -a vars=()
  while (($#)) && [[ "$1" != "--" ]]; do vars+=("$1"); shift; done
  shift
  RC=0
  OUT="$(env -i HOME="$WORK" PATH="$path" "${vars[@]}" "$DOTCTL" "$@" 2>&1)" || RC=$?
  OUT="$(printf '%s\n' "$OUT" | sed "s/$(printf '\033')\[[0-9;]*m//g")"
}
end_case() {
  local p
  if ((${#PROBLEMS[@]} == 0)); then
    printf 'PASS  %s\n' "$CURRENT"
    return 0
  fi
  FAILED=$((FAILED + 1))
  printf 'FAIL  %s\n' "$CURRENT"
  for p in "${PROBLEMS[@]}"; do printf '        %s\n' "$p"; done
  printf '        rc: %s\n        calls: %s\n' "$RC" "$(calls | tr '\n' ';')"
  printf '        output:\n'
  printf '%s\n' "$OUT" | sed 's/^/          | /'
}

start_case "the_fingerprinted_files_were_found"
((${#SOURCES[@]} >= 3)) || note "SELF_SOURCE not found in src/main.rs (got ${#SOURCES[@]} files)"
end_case

start_case "a_checkout_that_matches_the_binary_draws_no_warning"
run "$MINBIN" -- install --repo "$REPO"
[[ "$OUT" != *"built from different source"* ]] || note "warned about a checkout that matches"
end_case

# Every case below runs against a checkout whose dotctl source has moved on.
printf '\n' >>"$REPO/Cargo.lock"

start_case "a_stale_checkout_rebuilds_once_and_reexecs_with_the_same_arguments"
run "$FAKES:$MINBIN" -- all --repo "$REPO" --no-aur-helper
[[ "$RC" == 0 ]] || note "expected rc 0 (the fake new dotctl's)"
[[ "$(calls)" == "cargo install --path $REPO --locked --force"$'\n'"dotctl all --repo $REPO --no-aur-helper DOTCTL_REBUILT=1" ]] ||
  note "expected exactly one rebuild, then the new dotctl with the same arguments and DOTCTL_REBUILT=1"
end_case

start_case "a_binary_still_stale_after_its_rebuild_warns_instead_of_looping"
run "$MINBIN" DOTCTL_REBUILT=1 -- all --repo "$REPO" --no-aur-helper
[[ -z "$(calls)" ]] || note "it rebuilt again"
[[ "$OUT" == *"just rebuilt and still differs"* ]] || note "no warning about the loop it refused"
[[ "$OUT" == *"cargo not found"* ]] || note "expected the run to carry on into install()"
end_case

start_case "a_stale_checkout_only_warns_under_install"
run "$MINBIN" -- install --repo "$REPO"
[[ -z "$(calls)" ]] || note "install rebuilt dotctl; only all may"
[[ "$OUT" == *"built from different source"* ]] || note "no staleness warning"
[[ "$OUT" == *"cargo install --path $REPO --locked --force"* ]] || note "the warning does not give the rebuild command"
end_case

printf '\n%s case(s), %s failed\n' "$CASES" "$FAILED"
if ((FAILED)); then exit 1; fi
exit 0
