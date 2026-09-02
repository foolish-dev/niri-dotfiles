#!/usr/bin/env bash
# =============================================================================
# tests/hexstrike-assert-loopback.sh -- exercise the ExecStartPost= guard
#
# The subject is .local/bin/hexstrike-assert-loopback, which decides whether an
# UNAUTHENTICATED API whose /api/command runs arbitrary shell is allowed to
# keep running. `exit 0` from it means "verified: the socket attributed to
# $MAINPID is loopback-only", so every case below is really asking the same
# question: can this thing be talked into a pass it did not earn?
#
# Hermetic by construction. A fake `ss` goes first on PATH for every address
# case, so nothing here needs iproute2, opens a socket, or can see -- let alone
# disturb -- the real hexstrike-server.service. The guard is invoked as
# `bash "$copy"` rather than by its shebang, so the cases that strip PATH down
# to nothing still start.
#
# The copy: the guard hardcodes WAIT=15 and `timeout 5`, and a suite that waits
# 15s per negative case does not get run. Rather than growing a test-only env
# knob in the guard -- a knob whose whole purpose is to make the shipped
# behaviour differ from the tested behaviour -- the suite tests a COPY with
# those constants rewritten short, and the last three cases assert that the
# SHIPPED file still spells them the way the rewrite expects. If the guard's
# constants drift, the rewrite silently stops applying; those cases are what
# turns that into a failure instead of a suite that quietly tests nothing.
#
# Usage: tests/hexstrike-assert-loopback.sh [path-to-guard]
#        HEXSTRIKE_GUARD=... HEXSTRIKE_UNIT=... tests/hexstrike-assert-loopback.sh
# =============================================================================
set -euo pipefail

# ---------------------------------------------------------------------------
# Locate the subject relative to this file, so the suite runs from any cwd.
# ---------------------------------------------------------------------------
SUITE_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(cd -- "$SUITE_DIR/.." && pwd)"
GUARD="${1:-${HEXSTRIKE_GUARD:-$REPO_ROOT/.local/bin/hexstrike-assert-loopback}}"
UNIT="${HEXSTRIKE_UNIT:-$REPO_ROOT/.config/systemd/user/hexstrike-server.service}"

if [[ ! -r "$GUARD" ]]; then
  printf 'cannot read guard: %s\n' "$GUARD" >&2
  exit 2
fi

# Absolute path to this shell: the guard is started with `env -i`, so nothing
# may be looked up on the stripped PATH the cases hand it.
BASH_BIN="${BASH:-$(command -v bash)}"

# Host tools the harness itself needs. The guard's own four (ss/timeout/awk/
# sleep) are provided per-case; ss is always the fake.
declare -A REAL=()
for _t in timeout awk sleep; do
  if ! _p="$(command -v "$_t")"; then
    printf 'harness needs %s on PATH\n' "$_t" >&2
    exit 2
  fi
  REAL["$_t"]="$_p"
done
unset _t _p

# ---------------------------------------------------------------------------
# Scratch space. Cleaned on every exit path, including a failing one.
# ---------------------------------------------------------------------------
WORK="$(mktemp -d "${TMPDIR:-/tmp}/hexstrike-guard-test.XXXXXX")"
VICTIMS=()
# shellcheck disable=SC2329  # invoked from the trap below
cleanup() {
  local v
  if ((${#VICTIMS[@]})); then
    for v in "${VICTIMS[@]}"; do
      kill "$v" 2>/dev/null || true
    done
  fi
  rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

# ---------------------------------------------------------------------------
# The shipped text, read once. Section 0 judges it; the copy is derived from it.
# ---------------------------------------------------------------------------
SHIPPED="$(<"$GUARD")"
# A pid that provably does not exist: the kernel never hands out a pid above
# pid_max, so /proc/$GHOST_PID cannot appear mid-run and make a case flaky.
if [[ -r /proc/sys/kernel/pid_max ]]; then
  GHOST_PID=$(( $(</proc/sys/kernel/pid_max) + 1 ))
else
  GHOST_PID=4194305
fi

# ---------------------------------------------------------------------------
# The fake `ss`. Reads its script from $FAKE_SS_DIR:
#   p.out       stdout for the attributed probe   (ss -ltnpH)
#   n.out       stdout for the unattributed sweep (ss -ltnH)
#   rc          exit status to return instead of answering
#   hang        seconds to block forever-ish, via exec so `timeout` can reap it
#   kill_after  kill the pid in `victim` once the call count passes this
#   argv.log    every argument received, one per line, for the injection case
# ---------------------------------------------------------------------------
FAKE_SS="$WORK/fake-ss"
printf '#!%s\n' "$BASH_BIN" >"$FAKE_SS"
cat >>"$FAKE_SS" <<'FAKE_SS_EOF'
# Stand-in for iproute2's ss: replays a canned listing, so no case in this
# suite needs a real socket, a real port, or the real service.
set -euo pipefail
d="${FAKE_SS_DIR:?fake ss needs FAKE_SS_DIR}"

mode=plain
for a in "$@"; do
  printf 'arg:%s\n' "$a" >>"$d/argv.log"
  if [[ "$a" == "-ltnpH" ]]; then mode=proc; fi
done

n=0
if [[ -f "$d/calls" ]]; then n="$(<"$d/calls")"; fi
n=$((n + 1))
printf '%s\n' "$n" >"$d/calls"

# exec, not a child: `timeout` must be able to kill the thing holding the
# guard's command-substitution pipe open, or the probe timeout does nothing.
if [[ -f "$d/hang" ]]; then exec sleep "$(<"$d/hang")"; fi

if [[ -f "$d/kill_after" && -f "$d/victim" ]] && ((n > $(<"$d/kill_after"))); then
  v="$(<"$d/victim")"
  kill "$v" 2>/dev/null || true
  # Wait for the reap, not just the signal: a zombie still has a /proc entry,
  # which is exactly what the guard's liveness check reads.
  i=0
  while [[ -d "/proc/$v" ]] && ((i < 200)); do
    sleep 0.02
    i=$((i + 1))
  done
fi

if [[ -f "$d/rc" ]]; then
  rc="$(<"$d/rc")"
  if ((rc != 0)); then exit "$rc"; fi
fi

f="$d/n.out"
if [[ "$mode" == "proc" ]]; then f="$d/p.out"; fi
if [[ -s "$f" ]]; then printf '%s' "$(<"$f")"; printf '\n'; fi
exit 0
FAKE_SS_EOF
chmod +x "$FAKE_SS"

# ---------------------------------------------------------------------------
# Per-case fixtures.
# ---------------------------------------------------------------------------
SSDIR=""
BIN=""

# ss_fixture <p.out contents> [n.out contents]
ss_fixture() {
  SSDIR="$WORK/ss.$CASES"
  mkdir -p "$SSDIR"
  printf '%s' "$1" >"$SSDIR/p.out"
  printf '%s' "${2:-}" >"$SSDIR/n.out"
  : >"$SSDIR/argv.log"
}

# bin_fixture [tool-to-omit] -- the PATH the guard is given
bin_fixture() {
  local omit="${1:-}" t
  BIN="$WORK/bin.$CASES"
  mkdir -p "$BIN"
  for t in timeout awk sleep; do
    if [[ "$t" != "$omit" ]]; then ln -sf "${REAL["$t"]}" "$BIN/$t"; fi
  done
  if [[ "$omit" != "ss" ]]; then ln -sf "$FAKE_SS" "$BIN/ss"; fi
}

# A realistic `ss -ltnpH` row: State Recv-Q Send-Q Local Peer Process.
# The guard reads $4, the local address.
listen_line() {
  printf 'LISTEN 0      128           %s          0.0.0.0:*    users:(("python3",pid=%s,fd=3))' "$1" "$2"
}

# The same row as it arrives when ss could not attribute the socket.
bare_line() {
  printf 'LISTEN 0      128           %s          0.0.0.0:*' "$1"
}

# ---------------------------------------------------------------------------
# Case machinery. A failing case records why and keeps going: the point is the
# whole picture, not the first thing that broke.
# ---------------------------------------------------------------------------
CASES=0
FAILED=0
CURRENT=""
RAN=0
RC=0
ERR=""
PROBLEMS=()

start_case() {
  CASES=$((CASES + 1))
  CURRENT="$1"
  PROBLEMS=()
  RAN=0
  RC=0
  ERR=""
  SSDIR=""
  BIN=""
}

note() { PROBLEMS+=("$1"); }

# run_guard [VAR=VAL ...] -- start the copy with a hermetic environment
run_guard() {
  RAN=1
  RC=0
  ERR="$(env -i PATH="$BIN" FAKE_SS_DIR="$SSDIR" "$@" \
    "$BASH_BIN" "$COPY" 2>&1 >/dev/null)" || RC=$?
}

expect_rc() {
  if [[ "$RC" != "$1" ]]; then note "expected rc $1, actual rc $RC"; fi
}

expect_refused() {
  if [[ "$RC" == "0" ]]; then note "expected a non-zero rc, actual rc 0"; fi
}

expect_err() {
  if [[ "$ERR" != *"$1"* ]]; then note "stderr does not contain: $1"; fi
}

expect_no_err() {
  if [[ "$ERR" == *"$1"* ]]; then note "stderr should not contain: $1"; fi
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
    printf '        actual stderr:\n'
    while IFS= read -r line; do
      printf '          | %s\n' "$line"
    done <<<"$ERR"
  fi
}

# spawn_victim -- an orphaned `sleep` this suite can hand to the guard as
# MAINPID and have the fake ss kill mid-run. Orphaned deliberately: if this
# shell were its parent, killing it would leave a zombie, and a zombie still
# has the /proc entry the guard's liveness check reads.
VICTIM=""
spawn_victim() {
  local pf="$WORK/victim.$CASES"
  # shellcheck disable=SC2016  # $! must expand in the inner shell, not here
  "$BASH_BIN" -c 'sleep 30 & printf "%s\n" "$!" >"$1"' _ "$pf"
  VICTIM="$(<"$pf")"
  VICTIMS+=("$VICTIM")
}

# =============================================================================
# 0. The shipped constants, judged before anything else runs.
#
# Every case below this one runs against a COPY whose WAIT and probe timeout
# have been shortened, so these are what stop the suite from quietly testing a
# guard that no longer resembles the shipped one. They come first on purpose:
# when the rewrite was keyed to the exact literals and refused to run without
# them, a drifted constant aborted the whole suite before a single case ran,
# and in CI that reads as "the harness is broken" rather than "the guard's
# deadline now outlives systemd's".
# =============================================================================
start_case "the_shipped_guard_still_pins_the_wait_this_suite_shortens"
if [[ "$SHIPPED" != *"WAIT=15"* ]]; then
  note "$GUARD no longer spells WAIT=15; the copy below is testing a different guard"
fi
end_case

start_case "the_shipped_guard_still_probes_with_timeout_5_ss"
if [[ "$SHIPPED" != *"timeout 5 ss -ltnpH \"sport = :\$PORT\""* ]]; then
  note "$GUARD no longer runs 'timeout 5 ss -ltnpH \"sport = :\$PORT\"'"
fi
if [[ "$SHIPPED" != *"timeout 5 ss -ltnH \"sport = :\$PORT\""* ]]; then
  note "$GUARD no longer sweeps with 'timeout 5 ss -ltnH \"sport = :\$PORT\"'"
fi
end_case

start_case "the_shipped_unit_still_runs_the_guard_outside_its_sandbox"
if [[ ! -r "$UNIT" ]]; then
  note "cannot read unit: $UNIT"
else
  # The `+` is what lets ss read /proc/<MAINPID>/fd. Without it every ss line
  # arrives with no users:(...) field, the pid filter matches nothing, and a
  # healthy service is taken down -- see the unattributable-socket case below.
  if [[ "$(<"$UNIT")" != *"ExecStartPost=+%h/.local/bin/hexstrike-assert-loopback"* ]]; then
    note "$UNIT no longer runs %h/.local/bin/hexstrike-assert-loopback as ExecStartPost=+"
  fi
fi
end_case

start_case "the_guards_deadline_expires_before_the_units_start_timeout"
# Two literals pinned in two files are not a relation between them: either can
# be re-spelled and the pair silently stops meaning anything. Compare the
# numbers. If the guard's WAIT ever outlives the unit's TimeoutStartSec,
# systemd kills the start job first and every diagnostic the cases below assert
# on is replaced by a generic "start-post operation timed out" -- still
# fail-closed, but mute, which is how this went unnoticed the first time.
_wait=""
while IFS= read -r _line; do
  if [[ "$_line" =~ ^WAIT=([0-9]+)$ ]]; then _wait="${BASH_REMATCH[1]}"; fi
done <<<"$SHIPPED"
_tmo=""
if [[ -r "$UNIT" ]]; then
  while IFS= read -r _line; do
    if [[ "$_line" =~ ^TimeoutStartSec=([0-9]+)s?$ ]]; then _tmo="${BASH_REMATCH[1]}"; fi
  done <"$UNIT"
fi
if [[ -z "$_wait" ]]; then
  note "no plain WAIT=<seconds> in $GUARD, so its deadline cannot be compared"
elif [[ -z "$_tmo" ]]; then
  note "no plain TimeoutStartSec=<seconds> in $UNIT -- if it is spelled another way (\"1min\"), teach this case that spelling rather than dropping the check"
elif ((_wait >= _tmo)); then
  note "the guard waits ${_wait}s but systemd kills the start job at ${_tmo}s, so the guard's own diagnostics can never reach the journal"
fi
end_case
unset _wait _tmo _line

# ---------------------------------------------------------------------------
# The copy under test, with those two constants shortened -- a suite that waits
# 15s per negative case does not get run. The rewrite matches whatever numbers
# the guard currently carries rather than the ones section 0 expects, so a
# drifted guard still runs every case below it and the drift is reported by a
# named case instead of by the suite refusing to start.
# ---------------------------------------------------------------------------
COPY="$WORK/hexstrike-assert-loopback"
# WAIT in the copy, in seconds. Cases 10 and 14 assert against this.
COPY_WAIT=2
{
  while IFS= read -r _line; do
    if [[ "$_line" =~ ^WAIT=[0-9]+$ ]]; then
      _line="WAIT=$COPY_WAIT"
    elif [[ "$_line" =~ ^(.*)timeout[[:space:]]+[0-9]+[[:space:]]+ss(.*)$ ]]; then
      _line="${BASH_REMATCH[1]}timeout 1 ss${BASH_REMATCH[2]}"
    fi
    printf '%s\n' "$_line"
  done <"$GUARD"
} >"$COPY"
unset _line

# =============================================================================
# 1. A loopback bind attributed to MAINPID is what a pass is supposed to mean.
# =============================================================================
start_case "a_loopback_ipv4_socket_owned_by_mainpid_is_verified"
bin_fixture
ss_fixture "$(listen_line "127.0.0.1:8888" "$$")"
run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
expect_rc 0
end_case

start_case "a_loopback_ipv6_socket_owned_by_mainpid_is_verified"
bin_fixture
ss_fixture "$(listen_line "[::1]:8888" "$$")"
run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
expect_rc 0
end_case

start_case "both_loopback_families_on_one_port_are_verified_together"
bin_fixture
ss_fixture "$(listen_line "127.0.0.1:8888" "$$")
$(listen_line "[::1]:8888" "$$")"
run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
expect_rc 0
end_case

# =============================================================================
# 2. Every off-loopback spelling is refused. These are the binds the whole
#    unit exists to prevent: /api/command is unauthenticated shell.
# =============================================================================
for _pair in "a_wildcard_ipv4_bind_owned_by_mainpid_is_refused|0.0.0.0:8888" \
  "a_wildcard_ipv6_bind_owned_by_mainpid_is_refused|[::]:8888" \
  "a_star_wildcard_bind_owned_by_mainpid_is_refused|*:8888" \
  "a_lan_address_bind_owned_by_mainpid_is_refused|192.168.1.42:8888"; do
  start_case "${_pair%%|*}"
  bin_fixture
  ss_fixture "$(listen_line "${_pair#*|}" "$$")"
  run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
  expect_refused
  expect_err "not loopback"
  expect_err "${_pair#*|}"
  end_case
done
unset _pair

# =============================================================================
# 2b. Loopback addresses that are nonetheless not the pinned one. The guard
#     refuses these too -- `dotctl install` and HEXSTRIKE_HOST pin 127.0.0.1,
#     so anything else is drift, and noticing drift is why this second check
#     exists at all. What must NOT happen is the guard calling them "not
#     loopback": that is false, and it sends whoever reads the journal looking
#     for an exposure that is not there.
# =============================================================================
for _pair in "a_loopback_address_other_than_the_pinned_one_is_refused_as_drift|127.0.0.53:8888" \
  "a_v4_mapped_loopback_bind_is_refused_as_drift|[::ffff:127.0.0.1]:8888"; do
  start_case "${_pair%%|*}"
  bin_fixture
  ss_fixture "$(listen_line "${_pair#*|}" "$$")"
  run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
  expect_refused
  expect_err "loopback, but not the"
  expect_err "${_pair#*|}"
  # The exposure wording belongs to genuinely reachable binds only.
  expect_no_err "not loopback."
  end_case
done
unset _pair

# =============================================================================
# 3. One good socket does not launder a bad one, in either listing order. ss
#    does not promise an order, so both have to be pinned.
# =============================================================================
start_case "an_exposed_socket_is_refused_even_when_a_loopback_socket_is_listed_first"
bin_fixture
ss_fixture "$(listen_line "127.0.0.1:8888" "$$")
$(listen_line "0.0.0.0:8888" "$$")"
run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
expect_refused
expect_err "not loopback"
end_case

start_case "an_exposed_socket_is_refused_even_when_it_is_listed_before_the_loopback_one"
bin_fixture
ss_fixture "$(listen_line "0.0.0.0:8888" "$$")
$(listen_line "127.0.0.1:8888" "$$")"
run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
expect_refused
expect_err "not loopback"
end_case

# =============================================================================
# 4/5. The check is scoped to MAINPID's socket, not to the port. A squatter
#      must not answer for us, and must not speak for us either.
# =============================================================================
start_case "a_port_held_only_by_a_foreign_pid_is_not_accepted_as_our_bind"
bin_fixture
# Loopback on purpose: "something loopback-ish holds the port" is the weaker
# question, and answering it would be a pass here.
ss_fixture "$(listen_line "127.0.0.1:8888" "999999")"
run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
expect_refused
expect_err "That port is held by something else:"
expect_err "pid=999999"
end_case

start_case "a_foreign_listener_beside_our_own_loopback_socket_does_not_fail_the_check"
bin_fixture
ss_fixture "$(listen_line "127.0.0.1:8888" "999999")
$(listen_line "127.0.0.1:8888" "$$")"
run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
expect_rc 0
end_case

# =============================================================================
# 6. The attribution filter matches "pid=$MAINPID," with the comma, so a pid
#    that is a digit-prefix of another pid cannot borrow its socket -- the
#    MAINPID=1430 vs pid=14300 trap. Spelled here as $$ vs ${$$}0 so the
#    non-matching pid is derived rather than hoped for, and MAINPID is a pid
#    that stays alive for the whole case (this shell), so the run ends at the
#    deadline rather than down the "server exited" door.
# =============================================================================
start_case "a_mainpid_that_is_a_digit_prefix_of_another_pid_does_not_match_that_pids_socket"
bin_fixture
ss_fixture "$(listen_line "127.0.0.1:8888" "$(($$ * 10))")"
run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
expect_refused
expect_err "is not listening on port 8888"
end_case

# =============================================================================
# 7. A socket with no users:(...) field at all.
#
#    Why this exists: it is what a --user unit's sandbox does to ss. Any
#    mount-namespace setting (ProtectSystem, ProtectHome, PrivateTmp) puts the
#    unit in its own user namespace, from which ss cannot read /proc/<pid>/fd
#    and every line comes back unattributed -- so the guard matched nothing and
#    took a healthy service down. The fix was `+` on ExecStartPost=, and the
#    message has to point there. Blaming a squatter that does not exist sent
#    the operator hunting the wrong thing, which is why the "held by something
#    else" wording is asserted absent here.
# =============================================================================
start_case "an_unattributable_socket_blames_the_units_missing_execstartpost_plus_not_a_squatter"
bin_fixture
ss_fixture "$(bare_line "127.0.0.1:8888")"
run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
expect_refused
expect_err "could not be attributed"
expect_err "ExecStartPost="
expect_err "\`+\`"
expect_no_err "held by something else"
end_case

# =============================================================================
# 8. ss never answering is a different failure from ss answering "nothing",
#    and must not be reported as a mismatched --port.
# =============================================================================
start_case "every_ss_probe_failing_is_reported_as_such_rather_than_blamed_on_the_port"
bin_fixture
ss_fixture ""
printf '1\n' >"$SSDIR/rc"
run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
expect_refused
expect_err "Every \`ss\` probe failed or timed out"
expect_no_err "--port matches"
end_case

# =============================================================================
# 9. A MAINPID that is not a usable pid fails closed. Each of these is handed a
#    perfectly good loopback listener owned by a real pid, so anything that
#    stopped scoping to MAINPID would pass instead of failing.
# =============================================================================
start_case "an_unset_mainpid_is_refused"
bin_fixture
ss_fixture "$(listen_line "127.0.0.1:8888" "$$")"
run_guard HEXSTRIKE_PORT=8888
expect_refused
end_case

start_case "an_empty_mainpid_is_refused"
bin_fixture
ss_fixture "$(listen_line "127.0.0.1:8888" "$$")"
run_guard MAINPID="" HEXSTRIKE_PORT=8888
expect_refused
end_case

start_case "a_mainpid_of_zero_is_refused"
bin_fixture
ss_fixture "$(listen_line "127.0.0.1:8888" "$$")"
run_guard MAINPID=0 HEXSTRIKE_PORT=8888
expect_refused
end_case

start_case "a_non_numeric_mainpid_is_refused"
bin_fixture
ss_fixture "$(listen_line "127.0.0.1:8888" "$$")"
run_guard MAINPID=abc HEXSTRIKE_PORT=8888
expect_refused
end_case

start_case "a_negative_mainpid_is_refused"
bin_fixture
ss_fixture "$(listen_line "127.0.0.1:8888" "$$")"
run_guard MAINPID=-1 HEXSTRIKE_PORT=8888
expect_refused
end_case

# =============================================================================
# 10. A numeric MAINPID that never existed.
#
#     Why this exists: the "server exited before binding" door returns success
#     without having verified a bind, and it used to be opened by `kill -0`,
#     which answers "already gone" for a pid that was never alive in the first
#     place. Under PrivatePIDs=yes systemd handed the guard exactly that -- the
#     pid of a short-lived setup fork, dead before ExecStartPost ran -- and the
#     guard rubber-stamped the unit in 23ms while Flask bound 627ms later. The
#     seen_alive gate now requires a pid this run watched be alive via /proc,
#     so a pid above pid_max has to fall through to the deadline. The timing is
#     half the guarantee: a fast exit is the regression even when the rc is
#     right, so the elapsed time is asserted too.
# =============================================================================
start_case "a_mainpid_that_never_existed_waits_out_the_deadline_instead_of_passing_instantly"
bin_fixture
ss_fixture ""
_t0=$SECONDS
run_guard MAINPID="$GHOST_PID" HEXSTRIKE_PORT=8888
_elapsed=$((SECONDS - _t0))
expect_refused
expect_err "is not listening on port 8888"
if ((_elapsed < COPY_WAIT)); then
  note "returned in ${_elapsed}s, before the ${COPY_WAIT}s deadline it must wait out"
fi
end_case
unset _t0 _elapsed

# =============================================================================
# 11/12. The one sanctioned pass-without-verifying: MAINPID seen alive, then
#        gone, and nothing listening. If anything still holds the port, the
#        door closes -- a child or wrapper could be holding it on 0.0.0.0 while
#        the pid the guard watched disappears.
# =============================================================================
start_case "a_mainpid_that_dies_with_the_port_free_passes_and_says_it_verified_nothing"
bin_fixture
ss_fixture "" ""
spawn_victim
printf '%s\n' "$VICTIM" >"$SSDIR/victim"
printf '1\n' >"$SSDIR/kill_after"
run_guard MAINPID="$VICTIM" HEXSTRIKE_PORT=8888
expect_rc 0
expect_err "exited before binding"
expect_err "nothing to verify"
end_case

start_case "a_mainpid_that_dies_while_something_still_holds_the_port_is_refused"
bin_fixture
ss_fixture "" "$(bare_line "0.0.0.0:8888")"
spawn_victim
printf '%s\n' "$VICTIM" >"$SSDIR/victim"
printf '1\n' >"$SSDIR/kill_after"
run_guard MAINPID="$VICTIM" HEXSTRIKE_PORT=8888
expect_refused
expect_err "still holds"
expect_err "cannot attribute"
end_case

# =============================================================================
# 13. Each required tool, missing in turn. A guard that cannot run is not a
#     guard: it has to say which tool and stop, not discover the gap mid-poll.
# =============================================================================
for _tool in ss timeout awk sleep; do
  start_case "a_missing_${_tool}_is_named_and_fails_closed"
  bin_fixture "$_tool"
  ss_fixture "$(listen_line "127.0.0.1:8888" "$$")"
  run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
  expect_refused
  expect_err "$_tool not found"
  expect_err "Refusing to leave an unauthenticated /api/command up unverified."
  for _other in ss timeout awk sleep; do
    if [[ "$_other" != "$_tool" ]]; then expect_no_err "$_other not found"; fi
  done
  end_case
done
unset _tool _other

# =============================================================================
# 14. A wedged ss. The deadline is only consulted between polls, so without
#     `timeout` around the probe itself this hangs the start job for as long as
#     systemd allows and then blames the server for it.
# =============================================================================
start_case "an_ss_that_never_answers_is_bounded_by_the_probe_timeout"
bin_fixture
ss_fixture ""
printf '30\n' >"$SSDIR/hang"
_t0=$SECONDS
run_guard MAINPID="$$" HEXSTRIKE_PORT=8888
_elapsed=$((SECONDS - _t0))
expect_refused
expect_err "Every \`ss\` probe failed or timed out"
if ((_elapsed > 20)); then
  note "took ${_elapsed}s: the probe timeout did not bound a hung ss"
fi
end_case
unset _t0 _elapsed

# =============================================================================
# 15. HEXSTRIKE_PORT comes from the unit's Environment= and lands inside the ss
#     filter expression. It must travel as data.
# =============================================================================
start_case "shell_metacharacters_in_hexstrike_port_are_passed_as_data_and_never_executed"
bin_fixture
ss_fixture ""
_canary="$WORK/injected-$CASES"
# Every branch of the payload creates the canary with a redirection and the
# `:` builtin alone. `touch` is not on the PATH the guard is handed, so a
# payload that shelled out could not fail this case even if it were executed.
_payload="8888; : >$_canary \$(: >$_canary) \`: >$_canary\` && : >$_canary"
run_guard MAINPID="$$" HEXSTRIKE_PORT="$_payload"
expect_refused
if [[ -e "$_canary" ]]; then
  note "injection succeeded: $_canary was created"
fi
_saw_literal_arg=0
while IFS= read -r _a; do
  if [[ "$_a" == "arg:sport = :$_payload" ]]; then _saw_literal_arg=1; fi
done <"$SSDIR/argv.log"
if ((_saw_literal_arg == 0)); then
  note "the port did not reach ss as one literal argument"
fi
end_case
unset _canary _payload _saw_literal_arg _a

# ---------------------------------------------------------------------------
printf '\n%s case(s), %s failed\n' "$CASES" "$FAILED"
if ((FAILED)); then exit 1; fi
exit 0
