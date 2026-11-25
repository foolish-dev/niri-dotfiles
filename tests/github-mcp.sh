#!/usr/bin/env bash
# =============================================================================
# tests/github-mcp.sh -- pin how .local/bin/github-mcp handles the token
#
# The launcher holds a credential for one moment: it reads the GitHub token
# from the keyring and execs github-mcp-server with it. Two failure modes
# matter. The server must never start without a token -- it would fall back to
# an OAuth flow nobody is there to see -- and the token must never be printed:
# stdout is the MCP protocol stream, and stderr ends up in teleia's logs.
#
# Hermetic: a fake secret-tool and a fake github-mcp-server are the only things
# on PATH, everything lives under one mktemp -d, and no keyring is touched.
#
# Usage: tests/github-mcp.sh
# =============================================================================
set -euo pipefail

SUITE_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
SUBJECT="$SUITE_DIR/../.local/bin/github-mcp"
BASH_BIN="${BASH:-$(command -v bash)}"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/github-mcp-test.XXXXXX")"
# shellcheck disable=SC2329  # invoked from the trap below
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT INT TERM

# The fake keyring answers `lookup service github-mcp-server` with
# $FAKE_TOKEN (no newline, as the real one prints it) and records the call;
# with FAKE_TOKEN unset it finds nothing and exits 1, like the real one.
# The fake server records its arguments and the token it was handed.
BIN="$WORK/bin"
NOST="$WORK/no-secret-tool"
mkdir -p "$BIN" "$NOST"
# shellcheck disable=SC2016  # these expand when the fakes run
{
  printf '#!%s\n' "$BASH_BIN"
  printf '%s\n' 'echo "$*" >>"$CALLS"' \
    '[[ -n "${FAKE_TOKEN:-}" ]] && { printf "%s" "$FAKE_TOKEN"; exit 0; }' 'exit 1'
} >"$BIN/secret-tool"
# shellcheck disable=SC2016
{
  printf '#!%s\n' "$BASH_BIN"
  printf '%s\n' 'printf "%s" "$*" >"$SERVER_ARGS"' \
    'printf "%s" "${GITHUB_PERSONAL_ACCESS_TOKEN-<unset>}" >"$SERVER_TOKEN"'
} >"$BIN/github-mcp-server"
chmod +x "$BIN/secret-tool" "$BIN/github-mcp-server"
ln -s "$BIN/github-mcp-server" "$NOST/github-mcp-server"

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
  rm -f "$WORK/calls" "$WORK/server.args" "$WORK/server.token"
}
note() { PROBLEMS+=("$1"); }

# run [VAR=value ...] -- [args...]: the launcher with a scrubbed environment,
# so nothing exported on the developer's machine can leak into a case.
run() {
  local -a vars=()
  while (($#)) && [[ "$1" != "--" ]]; do vars+=("$1"); shift; done
  shift
  RC=0
  OUT="$(env -i PATH="$BIN" CALLS="$WORK/calls" SERVER_ARGS="$WORK/server.args" \
    SERVER_TOKEN="$WORK/server.token" "${vars[@]}" "$BASH_BIN" "$SUBJECT" "$@" 2>&1)" || RC=$?
}

server_started() { [[ -f "$WORK/server.args" ]]; }
server_token() { cat "$WORK/server.token" 2>/dev/null; }

end_case() {
  local p
  if ((${#PROBLEMS[@]} == 0)); then
    printf 'PASS  %s\n' "$CURRENT"
    return 0
  fi
  FAILED=$((FAILED + 1))
  printf 'FAIL  %s\n' "$CURRENT"
  for p in "${PROBLEMS[@]}"; do printf '        %s\n' "$p"; done
  printf '        rc: %s\n        output: %s\n' "$RC" "$OUT"
}

start_case "an_exported_token_wins_and_the_keyring_is_not_asked"
run GITHUB_PERSONAL_ACCESS_TOKEN=tok-from-env FAKE_TOKEN=tok-from-keyring --
[[ "$RC" == 0 ]] || note "expected rc 0"
[[ "$(server_token)" == tok-from-env ]] || note "server did not get the exported token"
[[ ! -f "$WORK/calls" ]] || note "secret-tool was asked although a token was exported"
end_case

start_case "the_keyring_token_reaches_the_server_in_stdio_mode"
run FAKE_TOKEN=tok-from-keyring --
[[ "$RC" == 0 ]] || note "expected rc 0"
[[ "$(server_token)" == tok-from-keyring ]] || note "server did not get the keyring token"
[[ "$(cat "$WORK/server.args" 2>/dev/null)" == stdio ]] || note "server was not started as 'stdio'"
[[ "$(cat "$WORK/calls" 2>/dev/null)" == "lookup service github-mcp-server" ]] ||
  note "secret-tool was not asked for service github-mcp-server"
end_case

start_case "extra_arguments_follow_stdio"
run FAKE_TOKEN=tok -- --toolsets repos
[[ "$(cat "$WORK/server.args" 2>/dev/null)" == "stdio --toolsets repos" ]] ||
  note "arguments were not passed through after stdio"
end_case

start_case "an_empty_keyring_never_starts_the_server"
run --
[[ "$RC" == 1 ]] || note "expected rc 1"
server_started && note "the server started without a token"
[[ "$OUT" == *"secret-tool store"* ]] || note "the message does not say how to store a token"
end_case

start_case "no_secret_tool_never_starts_the_server"
RC=0
OUT="$(env -i PATH="$NOST" SERVER_ARGS="$WORK/server.args" SERVER_TOKEN="$WORK/server.token" \
  "$BASH_BIN" "$SUBJECT" 2>&1)" || RC=$?
[[ "$RC" == 1 ]] || note "expected rc 1"
server_started && note "the server started without a token"
[[ "$OUT" == *libsecret* ]] || note "the message does not name the package to install"
end_case

start_case "the_token_is_never_printed"
run FAKE_TOKEN=tok-SHOULD-NOT-LEAK --
[[ "$OUT" != *SHOULD-NOT-LEAK* ]] || note "the token appeared in the launcher's output"
end_case

printf '\n%s case(s), %s failed\n' "$CASES" "$FAILED"
if ((FAILED)); then exit 1; fi
exit 0
