#!/usr/bin/env bash
# =============================================================================
# tests/bin-helpers.sh -- the small .local/bin helpers no other suite covers
#
# sddm-theme writes the login screen's config through sudo; gclone, proj and
# dev drive git, fzf and tmux; hexstrike-mcp is the stdio bridge two MCP
# clients start; cheat documents keybinds that can drift from the configs;
# colorblocks, colorbars and pipes paint the terminal.
#
# Each runs for real, against stand-ins on a PATH that holds nothing else,
# with a HOME, a working directory and a filesystem root made for the case.
# No case can reach the real /etc, a real tmux server, sudo or the network.
#
# Usage: tests/bin-helpers.sh
# =============================================================================
set -euo pipefail

SUITE_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(cd -- "$SUITE_DIR/.." && pwd)"
BIN="$REPO_ROOT/.local/bin"
BASH_BIN="${BASH:-$(command -v bash)}"

WORK="$(mktemp -d "${TMPDIR:-/tmp}/bin-helpers-test.XXXXXX")"
# shellcheck disable=SC2329  # invoked from the trap below
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT INT TERM

# The real tools the scripts call, and nothing else.
REAL="$WORK/real"
mkdir -p "$REAL"
for t in basename dirname tr sort sed find mkdir tee rm cat sleep; do
  ln -s "$(command -v "$t")" "$REAL/$t"
done

# Stand-ins. Each logs "<name> <args>" to $CALLS, then runs its body.
CALLS="$WORK/calls"
FAKES="$WORK/fakes"
mkdir -p "$FAKES"
fake() {  # fake <name> <body line>...
  local name=$1
  shift
  {
    printf '#!%s\n' "$BASH_BIN"
    # shellcheck disable=SC2016  # $* expands when the stand-in runs
    printf 'echo "%s $*" >>"%s"\n' "$name" "$CALLS"
    printf '%s\n' "$@"
  } >"$FAKES/$name"
  chmod +x "$FAKES/$name"
}
# shellcheck disable=SC2016  # the stand-ins' $vars expand when they run
{
  fake sudo 'exec "$@"'
  # Picks $FZF_PICK, or the first line offered; $FZF_RC stands for Esc (130).
  fake fzf \
    "cat >\"$WORK/fzf-stdin\"" \
    '[[ -n "${FZF_RC:-}" ]] && exit "$FZF_RC"' \
    "IFS= read -r first <\"$WORK/fzf-stdin\" || true" \
    'printf "%s\n" "${FZF_PICK:-$first}"'
  # Clones by creating the directory git would, unless $GIT_RC says it failed.
  fake git \
    '[[ "${GIT_RC:-0}" == 0 ]] || exit "$GIT_RC"' \
    'mkdir -p "$(basename "${*: -1}" .git)"'
  # Every session exists only if $TMUX_HAS says so.
  fake tmux '[[ "$1" == has-session ]] && exit "${TMUX_HAS:-1}"' 'exit 0'
  fake tput 'case "$1" in cols) echo 120 ;; lines) echo 40 ;; esac'
  fake fake-shell 'echo "shell in $PWD" >>'"\"$CALLS\""
}

CASES=0
FAILED=0
CURRENT=""
PROBLEMS=()
RC=0
OUT=""
FHOME=""
FROOT=""
CWD=""
CASE_PATH=""

start_case() {
  CASES=$((CASES + 1))
  CURRENT="$1"
  PROBLEMS=()
  RC=0
  OUT=""
  FHOME="$WORK/home.$CASES"
  FROOT="$WORK/root.$CASES"
  CWD="$WORK/cwd.$CASES"
  mkdir -p "$FHOME" "$FROOT" "$CWD"
  CASE_PATH="$FAKES:$REAL"
  rm -f "$CALLS"
}
note() { PROBLEMS+=("$1"); }
calls() { cat "$CALLS" 2>/dev/null || true; }

# run <script> [VAR=value ...] [-- args...]
run() {
  local script=$1
  shift
  local -a vars=()
  while (($#)) && [[ "$1" != "--" ]]; do
    vars+=("$1")
    shift
  done
  (($#)) && shift
  RC=0
  OUT="$(cd "$CWD" && env -i HOME="$FHOME" PATH="$CASE_PATH" SHELL="$FAKES/fake-shell" \
    TERM=dumb "${vars[@]}" "$BASH_BIN" "$BIN/$script" "$@" 2>&1)" || RC=$?
  OUT="$(printf '%s\n' "$OUT" | sed "s/$(printf '\033')\[[0-9;]*m//g")"
}

# A PATH with every stand-in but one.
path_without() {
  local dir="$WORK/path-without.$CASES" f
  mkdir -p "$dir"
  for f in "$FAKES"/*; do
    [[ "${f##*/}" == "$1" ]] || ln -s "$f" "$dir/${f##*/}"
  done
  CASE_PATH="$dir:$REAL"
}

expect_rc() { [[ "$RC" == "$1" ]] || note "expected rc $1, got $RC"; }
expect_out() { [[ "$OUT" == *"$1"* ]] || note "output lacks: $1"; }
expect_call() { [[ "$(calls)" == *"$1"* ]] || note "no call: $1"; }
expect_no_call() { [[ "$(calls)" != *"$1"* ]] || note "unexpected call: $1"; }

end_case() {
  local p
  if ((${#PROBLEMS[@]} == 0)); then
    printf 'PASS  %s\n' "$CURRENT"
    return 0
  fi
  FAILED=$((FAILED + 1))
  printf 'FAIL  %s\n' "$CURRENT"
  for p in "${PROBLEMS[@]}"; do printf '        %s\n' "$p"; done
  printf '        rc: %s\n        calls:\n' "$RC"
  calls | sed 's/^/          | /'
  printf '        output:\n'
  printf '%s\n' "$OUT" | sed 's/^/          | /'
}

# =============================================================================
# sddm-theme
# =============================================================================
seed_themes() {
  mkdir -p "$FROOT/usr/share/sddm/themes/noctalia" "$FROOT/usr/share/sddm/themes/maya" \
    "$FROOT/usr/share/sddm/themes/junk"
  : >"$FROOT/usr/share/sddm/themes/noctalia/Main.qml"
  : >"$FROOT/usr/share/sddm/themes/maya/theme.conf"
}
DROPIN_REL="etc/sddm.conf.d/10-noctalia-theme.conf"

start_case "sddm_theme_writes_the_named_theme_into_the_drop_in_through_sudo"
seed_themes
run sddm-theme SDDM_THEME_ROOT="$FROOT" -- maya
expect_rc 0
[[ "$(cat "$FROOT/$DROPIN_REL" 2>/dev/null)" == $'[Theme]\nCurrent=maya\nCursorTheme=Bibata-Modern-Classic\nCursorSize=24' ]] ||
  note "the drop-in does not select maya with the cursor dotctl sets"
expect_call "sudo tee $FROOT/$DROPIN_REL"
end_case

start_case "sddm_theme_refuses_a_directory_sddm_cannot_load"
seed_themes
run sddm-theme SDDM_THEME_ROOT="$FROOT" -- junk
expect_rc 1
expect_out "Theme 'junk' not found"
[[ ! -e "$FROOT/$DROPIN_REL" ]] || note "wrote a drop-in for a non-theme"
end_case

# The bug its comment records: an unmatched glob offered a theme named '*'.
start_case "sddm_theme_with_no_themes_offers_none_not_a_literal_star"
mkdir -p "$FROOT/usr/share/sddm/themes"
run sddm-theme SDDM_THEME_ROOT="$FROOT" -- '*'
expect_rc 1
expect_out "No SDDM themes found"
expect_no_call "sudo"
end_case

start_case "sddm_theme_removes_the_stale_drop_in_that_shadowed_it"
seed_themes
mkdir -p "$FROOT/etc/sddm.conf.d"
printf '[Theme]\nCurrent=maya\n' >"$FROOT/etc/sddm.conf.d/theme.conf"
run sddm-theme SDDM_THEME_ROOT="$FROOT" -- noctalia
expect_rc 0
[[ ! -e "$FROOT/etc/sddm.conf.d/theme.conf" ]] || note "the stale theme.conf is still there to shadow it"
expect_out "Removed $FROOT/etc/sddm.conf.d/theme.conf"
end_case

start_case "sddm_theme_picks_with_fzf_and_escape_changes_nothing"
seed_themes
run sddm-theme SDDM_THEME_ROOT="$FROOT" FZF_RC=130
expect_rc 0
expect_call "fzf --prompt=SDDM Theme>"
[[ "$(cat "$WORK/fzf-stdin")" == $'maya\nnoctalia' ]] || note "fzf was not offered exactly the loadable themes, sorted"
[[ ! -e "$FROOT/$DROPIN_REL" ]] || note "Esc still wrote a drop-in"
end_case

start_case "sddm_theme_writes_the_drop_in_and_cursor_dotctl_owns"
# bash cannot share src/login.rs's consts, so the spellings are diffed here.
WANT_CONF="$(sed -n 's/^const SDDM_THEME_CONF: &str = "\(.*\)";$/\1/p' "$REPO_ROOT/src/login.rs")"
WANT_CURSOR="$(sed -n 's/^const CURSOR_THEME_NAME: &str = "\(.*\)";$/\1/p' "$REPO_ROOT/src/login.rs")"
grep -qF "SDDM_DROPIN=\"\$ROOT$WANT_CONF\"" "$BIN/sddm-theme" ||
  note "sddm-theme does not write dotctl's drop-in ($WANT_CONF)"
grep -qF "CursorTheme=$WANT_CURSOR" "$BIN/sddm-theme" ||
  note "sddm-theme does not set dotctl's cursor theme ($WANT_CURSOR)"
end_case

# =============================================================================
# gclone
# =============================================================================
for spec in \
  "gh:fool/grogu|git@github.com:fool/grogu.git|grogu" \
  "gl:fool/grogu|git@gitlab.com:fool/grogu.git|grogu" \
  "fool/grogu|git@github.com:fool/grogu.git|grogu" \
  "https://example.org/fool/grogu.git|https://example.org/fool/grogu.git|grogu" \
  "git@example.org:fool/grogu.git|git@example.org:fool/grogu.git|grogu"; do
  IFS='|' read -r arg url dir <<<"$spec"
  start_case "gclone_${arg//[^a-z]/_}_clones_${url//[^a-z]/_}"
  run gclone -- "$arg"
  expect_rc 0
  expect_call "git clone --recurse-submodules $url"
  expect_call "shell in $CWD/$dir"
  end_case
done

start_case "gclone_with_no_repo_says_how_to_call_it"
run gclone
[[ "$RC" != 0 ]] || note "expected a failure"
expect_out "Usage: gclone <repo>"
end_case

start_case "gclone_stops_when_the_clone_fails"
run gclone GIT_RC=128 -- fool/grogu
expect_rc 128
expect_no_call "shell in"
end_case

# =============================================================================
# proj
# =============================================================================
start_case "proj_names_the_directories_it_looks_in_when_none_exist"
run proj
expect_rc 1
expect_out "No project directories found. Create one of: $FHOME/Projects"
end_case

seed_projects() {
  mkdir -p "$FHOME/Projects/my.app/.git" "$FHOME/Projects/group/deep/.git" "$FHOME/code/tool/.git"
}

start_case "proj_offers_the_repos_one_level_down_in_every_project_dir"
seed_projects
run proj
expect_rc 0
[[ "$(cat "$WORK/fzf-stdin")" == "$FHOME/Projects/my.app"$'\n'"$FHOME/code/tool" ]] ||
  note "fzf was not offered exactly the two repos, sorted"
expect_call "shell in $FHOME/Projects/my.app"
end_case

start_case "proj_passes_its_query_to_fzf"
seed_projects
run proj -- tool
expect_call "--query=tool"
end_case

start_case "proj_inside_tmux_opens_a_session_named_after_the_repo"
seed_projects
run proj TMUX=1
expect_rc 0
expect_call "tmux new-session -ds my_app -c $FHOME/Projects/my.app"
expect_call "tmux switch-client -t my_app"
expect_no_call "shell in"
end_case

start_case "proj_inside_tmux_switches_to_an_existing_session"
seed_projects
run proj TMUX=1 TMUX_HAS=0
expect_call "tmux switch-client -t my_app"
expect_no_call "new-session"
end_case

# =============================================================================
# dev
# =============================================================================
start_case "dev_without_tmux_says_so"
path_without tmux
run dev
expect_rc 1
expect_out "tmux not installed"
end_case

start_case "dev_attaches_to_an_existing_session_without_rebuilding_it"
run dev TMUX_HAS=0 -- work "$CWD"
expect_rc 0
[[ "$(calls)" == $'tmux has-session -t=work\ntmux attach-session -t work' ]] ||
  note "expected has-session then attach, nothing else"
end_case

start_case "dev_builds_the_editor_layout_in_a_new_session"
run dev -- my.app "$CWD"
expect_rc 0
expect_call "tmux new-session -d -s my_app -c $CWD -x 120 -y 40"
expect_call "tmux split-window -h -t my_app:1 -c $CWD -l 35%"
expect_call "tmux split-window -v -t my_app:1.2 -c $CWD -l 40%"
expect_call "tmux send-keys -t my_app:1.1 nvim . Enter"
expect_call "tmux attach-session -t my_app"
end_case

# The directory's own name, with tmux's forbidden dot made an underscore.
start_case "dev_defaults_to_the_current_directory"
run dev
SESSION_NAME="${CWD##*/}"
expect_call "tmux new-session -d -s ${SESSION_NAME//./_} -c $CWD"
end_case

# =============================================================================
# hexstrike-mcp
# =============================================================================
start_case "hexstrike_mcp_runs_the_venv_bridge_against_loopback_only"
mkdir -p "$FHOME/tools/hexstrike-ai/hexstrike-env/bin"
# shellcheck disable=SC2016  # expands when the stand-in runs
printf '#!%s\necho "python3 $*" >>"%s"\ncat >"%s/py-stdin"\n' "$BASH_BIN" "$CALLS" "$WORK" \
  >"$FHOME/tools/hexstrike-ai/hexstrike-env/bin/python3"
chmod +x "$FHOME/tools/hexstrike-ai/hexstrike-env/bin/python3"
RC=0
OUT="$(printf '{"jsonrpc":"2.0"}\n' | env -i HOME="$FHOME" PATH="$CASE_PATH" \
  "$BASH_BIN" "$BIN/hexstrike-mcp" --debug 2>&1)" || RC=$?
expect_rc 0
expect_call "python3 $FHOME/tools/hexstrike-ai/hexstrike_mcp.py --server http://127.0.0.1:8888 --debug"
[[ "$(cat "$WORK/py-stdin")" == '{"jsonrpc":"2.0"}' ]] || note "the MCP client's stdin did not reach the bridge"
end_case

# =============================================================================
# cheat
# =============================================================================
start_case "every_topic_cheats_help_lists_has_its_own_sheet"
run cheat -- help
HELP="$OUT"
mapfile -t TOPICS < <(printf '%s\n' "$HELP" | sed -n 's/^  cheat \([a-z]*\) .*/\1/p')
((${#TOPICS[@]} >= 10)) || note "found only ${#TOPICS[@]} topics in the help"
for t in "${TOPICS[@]}"; do
  run cheat -- "$t"
  [[ -n "$OUT" && "$OUT" != "$HELP" ]] || note "cheat $t falls through to the help"
done
end_case

start_case "cheats_aliases_reach_the_same_sheets"
for pair in ba:blackarch pe:privesc activedirectory:ad; do
  run cheat -- "${pair%%:*}"
  short="$OUT"
  run cheat -- "${pair#*:}"
  [[ "$short" == "$OUT" ]] || note "cheat ${pair%%:*} differs from cheat ${pair#*:}"
done
end_case

start_case "the_tmux_sheet_names_only_keys_tmux_conf_leaves_bound"
TMUX_CONF="$REPO_ROOT/.config/tmux/tmux.conf"
declare -A BOUND=() UNBOUND=()
while read -r cmd a b _; do
  case "$cmd" in
    bind | bind-key)
      [[ "$a" == -r ]] && a="$b"
      [[ "$a" == -T ]] || BOUND["$a"]=1
      ;;
    unbind | unbind-key)
      a="${a#\'}"
      UNBOUND["${a%\'}"]=1
      ;;
  esac
done <"$TMUX_CONF"
# tmux's own bindings for the keys the sheet relies on.
declare -A DEFAULTS=([c]=1 [d]=1 [n]=1 [p]=1 [s]=1 ['$']=1)
run cheat -- tmux
while read -r key; do
  [[ -n "${BOUND[$key]:-}" ]] && continue
  [[ -n "${DEFAULTS[$key]:-}" && -z "${UNBOUND[$key]:-}" ]] && continue
  note "the sheet advertises C-a $key, which tmux.conf neither binds nor leaves at its default"
done < <(printf '%s\n' "$OUT" | sed -n 's/^  C-a \([^ ]*\) .*/\1/p' | tr '/' '\n')
end_case

start_case "the_nvim_sheet_names_only_keys_the_config_maps"
run cheat -- nvim
mapfile -t KEYS < <(printf '%s\n' "$OUT" | grep -o -E '<leader>[[:alnum:]]+|(^| )g[dr]( |$)' | tr -d ' ')
((${#KEYS[@]} >= 18)) || note "found only ${#KEYS[@]} keys in the sheet"
for k in "${KEYS[@]}"; do
  grep -rqF "\"$k\"" "$REPO_ROOT/.config/nvim" --include='*.lua' ||
    note "the sheet advertises $k, which no Lua file in .config/nvim maps"
done
end_case

# =============================================================================
# colorblocks, colorbars, pipes
# =============================================================================
colours_painted() { grep -o -E $'\e\\[48;5;[0-9]+m' <<<"$1" | sort -u | wc -l; }

start_case "colorblocks_paints_the_16_ansi_colours"
RC=0
OUT="$(env -i PATH="$CASE_PATH" "$BASH_BIN" "$BIN/colorblocks")" || RC=$?
expect_rc 0
(($(colours_painted "$OUT") == 16)) || note "expected 16 distinct background colours"
end_case

start_case "colorbars_paints_all_256_colours"
RC=0
OUT="$(env -i PATH="$CASE_PATH" "$BASH_BIN" "$BIN/colorbars")" || RC=$?
expect_rc 0
(($(colours_painted "$OUT") == 256)) || note "expected all 256 background colours"
end_case

start_case "pipes_draws_until_interrupted_then_restores_the_screen"
# SIGINT stands for the user's Ctrl+C, sent by timeout to a foreground child: a
# background job in a script starts with SIGINT ignored, which no trap undoes.
RC=0
timeout --preserve-status -s INT 1 env -i PATH="$CASE_PATH" TERM=dumb \
  "$BASH_BIN" "$BIN/pipes" >"$WORK/pipes-out" 2>&1 || RC=$?
expect_rc 0
[[ "$(<"$WORK/pipes-out")" == *$'\e[38;5;'* ]] || note "it drew nothing"
expect_call "tput smcup"
expect_call "tput rmcup"
expect_call "tput cnorm"
end_case

printf '\n%s case(s), %s failed\n' "$CASES" "$FAILED"
if ((FAILED)); then exit 1; fi
exit 0
