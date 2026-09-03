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
bad()  { printf 'FAIL  %s\n        %s\n' "$1" "$2"; CHECKED=$((CHECKED + 1)); FAILED=$((FAILED + 1)); }
skip() { printf 'SKIP  %-46s %s\n' "$1" "$2"; SKIPPED=$((SKIPPED + 1)); }

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
  if err="$(python3 -c "$PARSE" "$1" "$2" 2>&1)"; then
    ok "$2"
  else
    bad "$2" "$err"
  fi
}

echo "==> JSON"
while IFS= read -r f; do
  case "$f" in
    .config/wal/templates/*)
      # pywal templates: {color0} placeholders, not JSON until grogu renders them.
      skip "$f" "pywal template, not JSON until rendered" ;;
    .config/opencode/opencode.json)
      check jsonc "$f" ;;
    *)
      check json "$f" ;;
  esac
done < <(git ls-files '*.json')

echo
echo "==> JSONC"
while IFS= read -r f; do check jsonc "$f"; done < <(git ls-files '*.jsonc')

echo
echo "==> TOML"
while IFS= read -r f; do check toml "$f"; done < <(git ls-files '*.toml')

echo
# Said out loud rather than left to be inferred from what is absent.
echo "==> Not covered"
skip "$(git ls-files '*.kdl' | tr '\n' ' ')" "no KDL parser in any stdlib"
skip ".config/nvim/**.lua ($(git ls-files '.config/nvim/*.lua' '.config/nvim/**/*.lua' | wc -l) files)" \
  "would need a lua interpreter on the runner"

echo
printf '%s file(s) parsed, %s failed, %s skipped\n' "$CHECKED" "$FAILED" "$SKIPPED"
if ((FAILED)); then exit 1; fi
exit 0
