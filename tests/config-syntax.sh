#!/usr/bin/env bash
# =============================================================================
# tests/config-syntax.sh -- parse the configs this repo deploys verbatim
#
# The tracked configs ARE the live desktop: ~/.config/{niri,nvim,noctalia,...}
# are symlinks into this repo, so a malformed file is not a failed build, it is
# a broken session on the next login. Nothing else in CI opens any of them.
#
# The sharp edge is self-inflicted. CLAUDE.md says .config/noctalia/settings.json
# is rewritten by the noctalia shell on every settings change and should be
# committed as its own chore() commit without normalising it -- i.e. committed
# without being read. This is what reads it.
#
# JSON, JSONC and TOML need nothing but python3's standard library, so they are
# always parsed. KDL and Lua need a parser this suite will not install: `niri
# validate` for the one, LuaJIT -- standalone or the copy inside nvim -- for
# the other. Where the tool is missing the files are named in an explicit SKIP
# giving the reason, because the failure this file exists to prevent is a green
# run that quietly checked nothing.
#
# Usage: tests/config-syntax.sh
# =============================================================================
set -euo pipefail

SUITE_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(cd -- "$SUITE_DIR/.." && pwd)"
cd "$REPO_ROOT"

if ! git rev-parse --git-dir >/dev/null 2>&1; then
  echo "not a git checkout: this suite enumerates tracked files with git ls-files" >&2
  exit 2
fi
command -v python3 >/dev/null 2>&1 || {
  echo "python3 not found: json and tomllib come from its standard library" >&2
  exit 2
}

CHECKED=0
FAILED=0
SKIPPED=0

ok()   { printf 'OK    %s\n' "$1"; CHECKED=$((CHECKED + 1)); }
bad()  {
  # The body is indented line by line rather than with a single %s: niri's KDL
  # diagnostic is a multi-line miette report, not the one-liner the python
  # parsers hand back.
  printf 'FAIL  %s\n' "$1"
  printf '%s\n' "$2" | sed 's/^/        /'
  CHECKED=$((CHECKED + 1)); FAILED=$((FAILED + 1))
}
skip() { printf 'SKIP  %-46s %s\n' "$1" "$2"; SKIPPED=$((SKIPPED + 1)); }

# git ls-files reports what the index holds, which is not always what the
# worktree has -- a file that was `rm`'d, or `git rm`'d without a commit, is
# still listed. Opening it would print FileNotFoundError under FAIL, blaming a
# syntax error for a missing file.
present() {  # present <path>
  [[ -e "$1" ]] && return 0
  skip "$1" "tracked, but not in the worktree"
  return 1
}

# JSONC: a comment stripper that respects string literals. Naive line-stripping
# would eat the `//` in "https://opencode.ai/config.json" on line 2 of
# .config/opencode/opencode.json and report a syntax error in a valid file.
read -r -d '' PARSE <<'PY' || true
import json, sys, tomllib

def strip_jsonc(text):
    out, i, n = [], 0, len(text)
    while i < n:
        c = text[i]
        if c == '"':                      # copy a string literal verbatim
            out.append(c); i += 1
            while i < n:
                out.append(text[i])
                if text[i] == '\\':
                    if i + 1 < n:
                        out.append(text[i + 1]); i += 2
                        continue
                elif text[i] == '"':
                    i += 1
                    break
                i += 1
            continue
        if c == '/' and i + 1 < n and text[i + 1] == '/':
            while i < n and text[i] != '\n':
                i += 1
            continue
        if c == '/' and i + 1 < n and text[i + 1] == '*':
            i += 2
            while i + 1 < n and not (text[i] == '*' and text[i + 1] == '/'):
                i += 1
            i += 2
            continue
        out.append(c); i += 1
    return "".join(out)

kind, path = sys.argv[1], sys.argv[2]
try:
    if kind == "toml":
        with open(path, "rb") as fh:
            tomllib.load(fh)
    else:
        text = open(path, encoding="utf-8").read()
        json.loads(strip_jsonc(text) if kind == "jsonc" else text)
except Exception as exc:
    print(f"{type(exc).__name__}: {exc}")
    sys.exit(1)
PY

check() {  # check <kind> <path>
  local err
  present "$2" || return 0
  if err="$(python3 -c "$PARSE" "$1" "$2" 2>&1)"; then
    ok "$2"
  else
    bad "$2" "$err"
  fi
}

# `niri validate -c FILE` reads the file and exits; it needs no compositor and
# no session (verified under `env -i`, with WAYLAND_DISPLAY unset). Everything
# it prints -- the tracing lines and the miette diagnostic alike -- goes to
# stderr, and it colours the tracing lines even when stderr is a pipe, so
# NO_COLOR is what keeps escape sequences out of a FAIL body.
#
# The file is validated where it lies, because config.kdl's `include
# "grogu.kdl" optional=true` resolves relative to it: on this machine that
# pulls in the generated, gitignored grogu.kdl as niri itself would, and on a
# runner without one niri warns and still exits 0.
check_kdl() {  # check_kdl <path>
  local err
  present "$1" || return 0
  if err="$(NO_COLOR=1 niri validate -c "$1" 2>&1)"; then
    ok "$1"
  else
    bad "$1" "$err"
  fi
}

# The parser has to be LuaJIT's, because that is what neovim embeds. `luac`
# from Lua 5.4/5.5 compiles `a // b` happily and LuaJIT rejects it outright, so
# luac would pass files neovim cannot load -- a checker that disagrees with the
# runtime is worse than none. Both forms below COMPILE and never execute:
# `luajit -b` writes bytecode (discarded), `nvim -l` runs a script that only
# loadfile()s its argument. That is the point -- .config/nvim/lua/plugins/*.lua
# are lazy.nvim specs, and executing init.lua would bootstrap lazy and reach
# the network.
LUA_TOOL=""
if command -v luajit >/dev/null 2>&1; then
  LUA_TOOL=luajit
elif command -v nvim >/dev/null 2>&1; then
  # `nvim -l` does not source the user's init.lua (verified: package.loaded.lazy
  # is nil inside it), so it cannot be perturbed by the very config it checks.
  LUA_TOOL=nvim
fi

if [[ "$LUA_TOOL" == nvim ]]; then
  SCRATCH="$(mktemp -d)"
  trap 'rm -rf "$SCRATCH"' EXIT
  LUA_LOADFILE="$SCRATCH/loadfile.lua"
  cat > "$LUA_LOADFILE" <<'LUA'
local _, err = loadfile(arg[1])
if err then
  io.stderr:write(err, "\n")
  os.exit(1)
end
LUA
fi

lua_check() {  # lua_check <path>
  local err
  present "$1" || return 0
  if err="$(
    case "$LUA_TOOL" in
      luajit) luajit -b "$1" /dev/null ;;
      nvim)   nvim -l "$LUA_LOADFILE" "$1" ;;
    esac 2>&1
  )"; then
    ok "$1"
  else
    bad "$1" "$err"
  fi
}

# -z everywhere, and `read -d ''`, because git C-quotes any path holding a
# quote, a backslash or a control character -- `git ls-files` prints
# "quo\"te.json" with the quotes -- and the checker would then be handed a name
# no filesystem has. A plain space is never quoted and survives either way.
echo "==> JSON"
while IFS= read -r -d '' f; do
  case "$f" in
    .config/wal/templates/*)
      # pywal templates: {color0} placeholders, not JSON until grogu renders them.
      skip "$f" "pywal template, not JSON until rendered" ;;
    .config/opencode/opencode.json)
      check jsonc "$f" ;;
    *)
      check json "$f" ;;
  esac
done < <(git ls-files -z '*.json')

echo
echo "==> JSONC"
while IFS= read -r -d '' f; do check jsonc "$f"; done < <(git ls-files -z '*.jsonc')

echo
echo "==> TOML"
while IFS= read -r -d '' f; do check toml "$f"; done < <(git ls-files -z '*.toml')

echo
echo "==> KDL"
if command -v niri >/dev/null 2>&1; then
  while IFS= read -r -d '' f; do
    case "$f" in
      # niri validate checks a file against niri's schema, not KDL in general,
      # so it is the wrong tool for any other .kdl this repo might grow.
      .config/niri/*) check_kdl "$f" ;;
      *) skip "$f" "not a niri config; niri validate would reject the schema" ;;
    esac
  done < <(git ls-files -z '*.kdl')
else
  skip "$(git ls-files '*.kdl' | tr '\n' ' ')" \
    "niri not installed; niri validate is the only KDL parser here"
fi

echo
echo "==> Lua"
if [[ -n "$LUA_TOOL" ]]; then
  # Syntax only. These are lazy.nvim plugin specs: a file can compile cleanly
  # and still name a plugin that does not exist or hand lazy a key it rejects.
  # This catches the unclosed brace, not the wrong spec.
  while IFS= read -r -d '' f; do
    case "$f" in
      .config/nvim/*) lua_check "$f" ;;
      *) skip "$f" "not a neovim file; LuaJIT is not necessarily its dialect" ;;
    esac
  done < <(git ls-files -z '*.lua')
else
  skip ".config/nvim/**.lua ($(git ls-files '*.lua' | wc -l) files)" \
    "no luajit and no nvim; luac 5.4/5.5 is the wrong dialect"
fi

echo
# Said out loud rather than left to be inferred from what is absent.
echo "==> Not covered"
skip "$(git ls-files '*.ini' '*.conf' '*.yml' '*.service' '*.timer' | wc -l) ini/conf/yml/unit file(s)" \
  "no parser wired up for these formats yet"

echo
printf '%s file(s) parsed, %s failed, %s skipped\n' "$CHECKED" "$FAILED" "$SKIPPED"
if ((FAILED)); then exit 1; fi
exit 0
