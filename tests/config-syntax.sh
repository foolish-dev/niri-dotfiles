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
# JSON, JSONC, TOML and INI need nothing but python3's standard library, so
# they are always parsed. The rest need a tool this suite will not install and
# so gates on: `niri validate` for KDL, LuaJIT -- standalone or the copy inside
# nvim -- for Lua, `systemd-analyze verify` for units, PyYAML for YAML, and
# bash's own `-n` for the one .conf that is a shell script. Where the tool is
# missing the files are named in an explicit SKIP giving the reason, because
# the failure this file exists to prevent is a green run that quietly checked
# nothing.
#
# For the same reason the two gated-on parsers that could plausibly load and
# then check nothing -- systemd-analyze and PyYAML -- are probed against a
# deliberately broken input before they are trusted, and a probe that does not
# complain turns into a SKIP rather than a row of OKs.
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

# Hoisted out of the nvim branch below because the systemd check also needs a
# writable directory: `systemd-analyze verify --user` wants XDG_RUNTIME_DIR and
# aborts before reading anything without one.
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT

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
    elif kind == "yaml":
        # Imported here, not at the top: PyYAML is the one non-stdlib parser
        # this file touches, and a box without it must still parse JSON.
        import yaml
        with open(path, encoding="utf-8") as fh:
            list(yaml.safe_load_all(fh))   # generator; nothing is read until drained
    elif kind == "ini":
        # interpolation=None because these are not Python config files: a `%`
        # in a value is a literal (fuzzel.ini already has one in a comment) and
        # the default BasicInterpolation would raise on it. strict=True keeps
        # the duplicate-key case a failure, which is the shape of a bad
        # hand-merge.
        import configparser
        cp = configparser.ConfigParser(strict=True, interpolation=None)
        with open(path, encoding="utf-8") as fh:
            cp.read_file(fh)
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

# systemd-analyze verify is the only unit parser that is not systemd itself,
# and it ships in the same package, so any box with a user manager already has
# it. Three measured quirks shape how it is called.
#
# Its exit status is not the check. The mistakes worth catching most -- a
# typo'd directive (`Restrt=on-failure`), a line with no `=`, a value that will
# not parse (`RestartSec=banana`) -- are all reported and then IGNORED: verify
# prints the complaint and exits 0. Only a unit that could not load at all
# (unbalanced quoting, a [Timer] with no OnCalendar=) exits 1. So the rule here
# is "any output is a failure", which makes the exit status redundant rather
# than wrong. Everything it prints goes to stderr; stdout stays empty.
#
# It needs a writable XDG_RUNTIME_DIR under --user, or it dies with "Failed to
# initialize manager: No such device or address" before opening the file. It is
# handed this suite's own scratch dir so the check does not depend on the
# caller having a session -- verified under `env -u XDG_RUNTIME_DIR`. It does
# not need root, and it starts nothing.
#
# It also stats every Exec*= target and calls a missing one an error. Those
# paths are %h/.local/bin/... and %h/tools/..., which exist on a deployed
# machine and never on a CI runner, so that one message is dropped: unfiltered
# it would paint the job red on every push forever, and a check that is always
# red is a check nobody reads. What survives the filter is the parse. The SKIP
# under "Not covered" says out loud that Exec targets go unchecked.
UNIT_TOOL=""
if command -v systemd-analyze >/dev/null 2>&1; then
  # Probed, not assumed. A verify that cannot run prints its own error and
  # would be caught by the "any output" rule, but a verify that silently
  # stopped diagnosing would hand back a row of OKs, which is the one outcome
  # this suite is built to prevent.
  cat > "$SCRATCH/probe.service" <<'UNIT'
[Unit]
Description=probe: this file is expected to draw a complaint

[Service]
Type=simple
ExecStart=/bin/true
Restrt=on-failure
UNIT
  if XDG_RUNTIME_DIR="$SCRATCH" systemd-analyze verify --user "$SCRATCH/probe.service" 2>&1 \
       | grep -q "Unknown key"; then
    UNIT_TOOL=systemd-analyze
  fi
fi

unit_check() {  # unit_check <path>
  local out
  present "$1" || return 0
  out="$(XDG_RUNTIME_DIR="$SCRATCH" systemd-analyze verify --user "$1" 2>&1 \
         | grep -v 'is not executable: No such file or directory' || true)"
  if [[ -z "$out" ]]; then
    ok "$1"
  else
    bad "$1" "$out"
  fi
}

# PyYAML, probed the same way and for the same reason. It is not stdlib, and
# this suite installs nothing, so a runner without it gets a SKIP naming the
# files rather than a silent pass.
YAML_TOOL=""
if python3 -c 'import yaml' >/dev/null 2>&1 \
   && ! printf 'a:\n b: [1,\n' | python3 -c 'import sys,yaml; yaml.safe_load(sys.stdin)' >/dev/null 2>&1; then
  YAML_TOOL=pyyaml
fi

# `bash -n` compiles without executing, which is the whole requirement: this
# file is sourced by neofetch, and it sets colours and calls out to other
# programs.
conf_bash_check() {  # conf_bash_check <path>
  local err
  present "$1" || return 0
  if err="$(bash -n "$1" 2>&1)"; then
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
echo "==> systemd units"
if [[ -n "$UNIT_TOOL" ]]; then
  while IFS= read -r -d '' f; do
    case "$f" in
      # verify checks a unit against systemd's schema for the manager it is
      # told about, and --user is the right manager for exactly these: %h and
      # graphical-session.target mean nothing to the system one.
      .config/systemd/user/*) unit_check "$f" ;;
      *) skip "$f" "not a --user unit; verify would be run in the wrong manager" ;;
    esac
  done < <(git ls-files -z '*.service' '*.timer' '*.socket' '*.path' '*.target')
else
  skip "$(git ls-files '*.service' '*.timer' '*.socket' '*.path' '*.target' | wc -l) unit file(s)" \
    "systemd-analyze verify absent or not diagnosing; it is the only unit parser here"
fi

echo
echo "==> YAML"
# Worth stating where the value of this actually lands. A malformed
# .github/workflows/ci.yml fails silently -- GitHub simply does not run it --
# but it cannot be caught here on a runner either, because the job that would
# report it is the job that did not start. This check earns its keep before the
# push, and on dependabot.yml and lazygit's config.yml, which gate nothing and
# so would otherwise go unread until something quietly stopped working.
if [[ -n "$YAML_TOOL" ]]; then
  # safe_load_all, not safe_load: multi-document files are legal YAML, and
  # safe_load raises ComposerError on one, which would read as a syntax error
  # in a valid file. Syntax only -- PyYAML accepts duplicate mapping keys that
  # GitHub Actions rejects, so a green line here is not a promise the workflow
  # is well-formed.
  while IFS= read -r -d '' f; do check yaml "$f"; done < <(git ls-files -z '*.yml' '*.yaml')
else
  skip "$(git ls-files '*.yml' '*.yaml' | wc -l) yml file(s)" \
    "python3 has no stdlib YAML and PyYAML is not importable here"
fi

echo
echo "==> INI and .conf"
# One .conf extension, five formats behind it. Each is dispatched by name and
# the ones with no offline parser are named individually rather than summed,
# so the reason travels with the file.
while IFS= read -r -d '' f; do
  case "$f" in
    .config/wal/templates/*)
      # Same as the pywal JSON above: {color0} placeholders, not their format
      # until grogu renders them.
      skip "$f" "pywal template, not a conf file until rendered" ;;
    .config/neofetch/*)
      # Not INI at all -- neofetch sources it as shell.
      conf_bash_check "$f" ;;
    .config/kitty/*)
      skip "$f" "kitty's own space-separated dialect; its parser needs a running kitty" ;;
    .config/tmux/*)
      skip "$f" "tmux's own dialect; parsing it means starting a server" ;;
    *)
      # gtk-3.0/gtk-4.0 settings.ini, qt5ct/qt6ct .conf and fuzzel.ini are all
      # section-and-key INI, and all five parse under configparser (checked).
      # Shape only: it does not know that GTK reads a `gtk-theme-name` or that
      # fuzzel rejects an unknown key.
      check ini "$f" ;;
  esac
done < <(git ls-files -z '*.ini' '*.conf')

echo
# Said out loud rather than left to be inferred from what is absent.
echo "==> Not covered"
skip "Exec*= targets in the systemd units" \
  "verify's missing-binary error is dropped; those paths are outside the repo"
skip "schemas, everywhere" \
  "every check above is syntax: a file can parse and still say the wrong thing"

echo
printf '%s file(s) parsed, %s failed, %s skipped\n' "$CHECKED" "$FAILED" "$SKIPPED"
if ((FAILED)); then exit 1; fi
exit 0
