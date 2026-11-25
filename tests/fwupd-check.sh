#!/usr/bin/env bash
# =============================================================================
# tests/fwupd-check.sh -- pin how .local/bin/fwupd-check delivers its notice
#
# The monthly firmware check has one way to reach anyone: a notification. Its
# timer is Persistent=true, so a missed run fires at login, before the session's
# notification server exists -- which is how a High-urgency Secure Boot update
# found on 2026-11-25 reached nobody. notify() now retries for ten minutes; this
# pins that it does, that it stops as soon as the notice gets through, that the
# per-attempt errors stay out of the journal, and that it falls back to the log.
#
# The script's top level talks to fwupd, so it is never run. log() and notify()
# are cut out of the shipped text and run against a fake notify-send that fails
# a set number of times, with sleep replaced by a counter. Hermetic: one
# mktemp -d, no notification daemon, no fwupd.
#
# Usage: tests/fwupd-check.sh
# =============================================================================
set -euo pipefail

SUITE_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
SUBJECT="$SUITE_DIR/../.local/bin/fwupd-check"
BASH_BIN="${BASH:-$(command -v bash)}"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/fwupd-check-test.XXXXXX")"
# shellcheck disable=SC2329  # invoked from the trap below
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT INT TERM

# The two functions, exactly as shipped.
sed -n '/^log() {/p; /^notify() {/,/^}/p' "$SUBJECT" >"$WORK/fns.sh"

# notify-send that fails until attempt $SUCCEED_ON, the way it fails while no
# notification server owns the name.
BIN="$WORK/bin"
mkdir -p "$BIN"
# shellcheck disable=SC2016  # these expand when the fake runs
{
  printf '#!%s\n' "$BASH_BIN"
  printf '%s\n' 'n=$(( $(cat "$COUNT" 2>/dev/null || echo 0) + 1 )); echo "$n" >"$COUNT"' \
    '(( n >= SUCCEED_ON )) && exit 0' \
    'echo "GDBus.Error:org.freedesktop.DBus.Error.ServiceUnknown: The name is not activatable" >&2' \
    'exit 1'
} >"$BIN/notify-send"
chmod +x "$BIN/notify-send"
# No notify-send at all, but the coreutils the fake itself would need.
NONE="$WORK/none"
mkdir -p "$NONE"

CASES=0
FAILED=0
CURRENT=""
PROBLEMS=()
ATTEMPTS=0
SLEEPS=0
STDERR=""
LOGGED=""

start_case() {
  CASES=$((CASES + 1))
  CURRENT="$1"
  PROBLEMS=()
  rm -f "$WORK/count" "$WORK/sleeps" "$WORK/log"
}
note() { PROBLEMS+=("$1"); }

# notify_with <succeed-on> <PATH>: run the shipped notify() once.
notify_with() {
  # shellcheck disable=SC2016  # the script's $vars expand in the inner bash
  STDERR="$(env -i PATH="$2" COUNT="$WORK/count" SUCCEED_ON="$1" LOG="$WORK/log" \
    SLEEPS="$WORK/sleeps" "$BASH_BIN" -c \
    'sleep() { echo x >>"$SLEEPS"; }; source "$1"; notify -u normal "Firmware updates available" "body"' \
    _ "$WORK/fns.sh" 2>&1 >/dev/null)" || true
  ATTEMPTS="$(cat "$WORK/count" 2>/dev/null || echo 0)"
  SLEEPS="$({ wc -l <"$WORK/sleeps"; } 2>/dev/null || echo 0)"
  LOGGED="$(cat "$WORK/log" 2>/dev/null || true)"
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
  printf '        attempts=%s sleeps=%s\n        log: %s\n        stderr: %s\n' \
    "$ATTEMPTS" "$SLEEPS" "$LOGGED" "$STDERR"
}

start_case "the_shipped_functions_were_found"
# Without this, a rename would leave fns.sh empty and every case below would be
# testing nothing.
grep -q '^notify() {' "$WORK/fns.sh" || note "notify() not found in $SUBJECT"
grep -q '^log() {' "$WORK/fns.sh" || note "log() not found in $SUBJECT"
end_case

start_case "a_notification_server_that_appears_late_still_gets_the_notice"
notify_with 3 "$BIN:/usr/bin:/bin"
((ATTEMPTS == 3)) || note "expected 3 attempts, got $ATTEMPTS"
((SLEEPS == 2)) || note "expected 2 waits, got $SLEEPS"
[[ -z "$LOGGED" ]] || note "a delivered notice should log nothing"
end_case

start_case "it_stops_as_soon_as_the_notice_gets_through"
notify_with 1 "$BIN:/usr/bin:/bin"
((ATTEMPTS == 1)) || note "expected 1 attempt, got $ATTEMPTS"
((SLEEPS == 0)) || note "expected no wait, got $SLEEPS"
end_case

start_case "a_server_that_never_appears_gets_ten_minutes_then_one_log_line"
notify_with 999 "$BIN:/usr/bin:/bin"
((ATTEMPTS == 40)) || note "expected 40 attempts, got $ATTEMPTS"
((SLEEPS == 39)) || note "expected 39 waits (none after the last try), got $SLEEPS"
[[ "$(printf '%s\n' "$LOGGED" | grep -c .)" == 1 ]] || note "expected exactly one log line"
[[ "$LOGGED" == *"Firmware updates available"* ]] || note "the log does not keep what it would have said"
[[ "$STDERR" != *GDBus* ]] || note "per-attempt notify-send errors reached stderr, i.e. the journal"
end_case

start_case "with_no_notify_send_it_logs_and_returns"
notify_with 1 "$NONE"
((ATTEMPTS == 0)) || note "nothing should have been attempted"
[[ "$LOGGED" == *"notify-send unavailable"* ]] || note "expected the unavailable line in the log"
end_case

printf '\n%s case(s), %s failed\n' "$CASES" "$FAILED"
if ((FAILED)); then exit 1; fi
exit 0
