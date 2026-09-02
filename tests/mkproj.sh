#!/usr/bin/env bash
# =============================================================================
# tests/mkproj.sh -- exercise mkproj's refusal to destroy what it did not create
#
# The subject is .local/bin/mkproj. Almost everything it does is boilerplate
# nobody will miss if it drifts; one thing is not. Commit 33246b9 added
# `[[ -e "$DIR" ]] && refuse` because `mkproj <name> rust` used to `rm -rf` a
# project that already existed. So every case below is really asking the same
# question in a different spelling: can this thing be talked into writing to,
# or deleting, a path the caller did not ask it to create?
#
# The answer depends entirely on NAME, because NAME is unvalidated and lands in
# three places with three different notions of what a path is: the guard's
# `[[ -e "$BASE/$NAME" ]]` (a stat, which fails on a component that does not
# exist yet and so never resolves the `..` behind it), `mkdir -p "$BASE/$NAME"`
# (which materialises that component), and the rust branch's
# `rm -rf "$NAME"` (which resolves the lot). The traversal cases are the gap
# between the first and the third.
#
# Hermetic by construction:
#   * Every run happens under one `mktemp -d`, with PROJ_DIR and HOME pointed
#     inside it. `run_mkproj` re-checks that for every single invocation and
#     aborts the suite rather than run a case that could reach a real
#     ~/Projects -- the failure mode here is deleted data, not a red line.
#   * The subject is started with `env -i` on a curated PATH, so nothing it
#     finds is whatever happens to be installed on the runner.
#   * GIT_AUTHOR_* / GIT_COMMITTER_* are exported and passed through: mkproj
#     ends in `git commit`, and under `set -euo pipefail` a missing identity
#     turns every happy path into a non-zero exit that has nothing to do with
#     the behaviour under test.
#   * cargo, go, npm and python3 are never invoked. The rust cases run against
#     a shim `cargo` (see FAKE_CARGO below) because the hazard they pin is in
#     the lines *around* cargo, not in cargo; the types that genuinely need a
#     toolchain are skipped out loud at the end of the run.
#
# Usage: tests/mkproj.sh [path-to-mkproj]
#        MKPROJ_BIN=... tests/mkproj.sh
# =============================================================================
set -euo pipefail

# ---------------------------------------------------------------------------
# Locate the subject relative to this file, so the suite runs from any cwd.
# ---------------------------------------------------------------------------
SUITE_DIR="$(cd -- "$(dirname -- "$0")" && pwd)"
REPO_ROOT="$(cd -- "$SUITE_DIR/.." && pwd)"
MKPROJ="${1:-${MKPROJ_BIN:-$REPO_ROOT/.local/bin/mkproj}}"

if [[ ! -r "$MKPROJ" ]]; then
  printf 'cannot read subject: %s\n' "$MKPROJ" >&2
  exit 2
fi

# Absolute path to this shell: the subject is started with `env -i` and a
# curated PATH, so its shebang lookup cannot be relied on.
BASH_BIN="${BASH:-$(command -v bash)}"

# The tools mkproj itself reaches for on the paths this suite exercises.
# Anything not listed here is deliberately absent from the PATH it is handed.
declare -A REAL=()
for _t in git mkdir rm chmod cat; do
  if ! _p="$(command -v "$_t")"; then
    printf 'harness needs %s on PATH\n' "$_t" >&2
    exit 2
  fi
  REAL["$_t"]="$_p"
done
unset _t _p

# mkproj's last act is `git commit`. Under `set -euo pipefail` an unconfigured
# identity makes that exit non-zero, so a runner without ~/.gitconfig would
# fail every happy path for the wrong reason.
export GIT_AUTHOR_NAME="mkproj suite"
export GIT_AUTHOR_EMAIL="mkproj-suite@invalid"
export GIT_COMMITTER_NAME="mkproj suite"
export GIT_COMMITTER_EMAIL="mkproj-suite@invalid"

# ---------------------------------------------------------------------------
# Scratch space. Cleaned on every exit path, including a failing one.
# ---------------------------------------------------------------------------
WORK="$(mktemp -d "${TMPDIR:-/tmp}/mkproj-test.XXXXXX")"
# shellcheck disable=SC2329  # invoked from the trap below
cleanup() {
  rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

# A HOME that is not the runner's. Also keeps `git init` deterministic: no
# system config, a pinned default branch, no user hooks or templates.
FAKEHOME="$WORK/home"
mkdir -p "$FAKEHOME"
cat >"$FAKEHOME/.gitconfig" <<'GC'
[init]
	defaultBranch = main
[user]
	name = mkproj suite
	email = mkproj-suite@invalid
[commit]
	gpgsign = false
GC

# ---------------------------------------------------------------------------
# The one rule this suite must not get wrong.
#
# mkproj creates and -- on the rust path -- removes directories, and its own
# default search order ends at $HOME/Projects. Nothing here may be handed a
# base or a HOME outside $WORK, so every value bound for either is checked at
# the moment it is used rather than trusted because of how it was built.
# ---------------------------------------------------------------------------
assert_confined() {
  local label="$1" path="$2"
  case "$path" in
    *..*)
      printf 'REFUSING TO RUN: %s=%s contains ".."\n' "$label" "$path" >&2
      exit 2
      ;;
  esac
  case "$path" in
    "$WORK"/*) ;;
    *)
      printf 'REFUSING TO RUN: %s=%s is outside this suite temp dir %s\n' \
        "$label" "$path" "$WORK" >&2
      exit 2
      ;;
  esac
}

# ---------------------------------------------------------------------------
# The shim `cargo`.
#
# The rust branch is where the destructive line lives, and it is reached only
# when `command -v cargo` succeeds -- so on a bare runner with no rust the
# hazard cases would silently pass by taking the skeleton branch instead. This
# shim exists to make `command -v cargo` true and to record what the subject
# passed it; it emulates `cargo init` (in place, or into a named path),
# preserving any README.md already there, which is what cargo 1.98 does.
# ---------------------------------------------------------------------------
FAKE_CARGO="$WORK/fake-cargo"
printf '#!%s\n' "$BASH_BIN" >"$FAKE_CARGO"
cat >>"$FAKE_CARGO" <<'FAKE_CARGO_EOF'
# Stand-in for cargo: no rust toolchain is needed or invoked by this suite.
set -euo pipefail
log="${FAKE_CARGO_LOG:?shim cargo needs FAKE_CARGO_LOG}"
for a in "$@"; do printf 'arg:%s\n' "$a" >>"$log"; done

if [[ "${1:-}" != "init" ]]; then
  printf 'fake cargo: only `init` is emulated, got: %s\n' "$*" >&2
  exit 2
fi
shift

target="."
for a in "$@"; do
  case "$a" in
    -*)
      # Real cargo does the same thing with a name that looks like an option.
      printf "error: unexpected argument '%s' found\n" "$a" >&2
      exit 1
      ;;
    *) target="$a" ;;
  esac
done

mkdir -p "$target/src"
here="$(cd -- "$target" && pwd)"
pkg="${here##*/}"
if [[ ! -d "$target/.git" ]]; then git -C "$target" init -q; fi
cat >"$target/Cargo.toml" <<TOML
[package]
name = "$pkg"
version = "0.1.0"
edition = "2024"

[dependencies]
TOML
cat >"$target/src/main.rs" <<'RS'
fn main() {
    println!("Hello, world!");
}
RS
printf '/target\n' >"$target/.gitignore"
printf 'Creating binary (application) package\n'
FAKE_CARGO_EOF
chmod +x "$FAKE_CARGO"

# ---------------------------------------------------------------------------
# Per-case fixtures.
# ---------------------------------------------------------------------------
ARENA=""
BASE=""
BIN=""
CARGO_LOG=""

# arena_fixture -- a throwaway world for one case.
#
#   arena/Projects/            <- PROJ_DIR, the configured base
#   arena/Projects/existing-dir/CANARY
#   arena/Projects/existing-file
#   arena/sibling/{CANARY,README.md,deep/CANARY}   <- next to the base
#   arena/Documents/CANARY                         <- stand-in for ~/Documents
#   arena/abs-target/CANARY                        <- named by absolute path
#
# Everything outside Projects/ is something mkproj was never asked to touch.
arena_fixture() {
  ARENA="$WORK/arena.$CASES"
  BASE="$ARENA/Projects"
  mkdir -p "$BASE/existing-dir" "$ARENA/sibling/deep" "$ARENA/Documents" "$ARENA/abs-target"
  printf 'CANARY\n' >"$BASE/existing-dir/CANARY"
  printf 'CANARY\n' >"$BASE/existing-file"
  printf 'CANARY\n' >"$ARENA/sibling/CANARY"
  printf 'CANARY\n' >"$ARENA/sibling/deep/CANARY"
  printf 'IMPORTANT NOTES DO NOT LOSE\n' >"$ARENA/sibling/README.md"
  printf 'CANARY\n' >"$ARENA/Documents/CANARY"
  printf 'CANARY\n' >"$ARENA/abs-target/CANARY"
}

# bin_fixture [with-cargo] -- the PATH the subject is given.
bin_fixture() {
  local t
  BIN="$WORK/bin.$CASES"
  CARGO_LOG="$WORK/cargo.$CASES.log"
  mkdir -p "$BIN"
  : >"$CARGO_LOG"
  for t in "${!REAL[@]}"; do
    ln -sf "${REAL["$t"]}" "$BIN/$t"
  done
  if [[ "${1:-}" == "with-cargo" ]]; then
    ln -sf "$FAKE_CARGO" "$BIN/cargo"
  fi
}

# snapshot <dir> -- every path under dir plus the content of each regular
# file. "Untouched" has to mean contents too: the empty-type traversal
# truncates a README in place without removing or adding a single path.
#
# A .git directory is listed but not descended into: its objects are binary
# and its exact contents are not the guarantee -- "a repository appeared over
# somebody's files" is, and the directory entry alone says that.
snapshot() {
  local root="$1" p
  while IFS= read -r p; do
    if [[ -L "$p" ]]; then
      printf 'link %s -> %s\n' "${p#"$root"}" "$(readlink "$p")"
    elif [[ -d "$p" ]]; then
      printf 'dir  %s\n' "${p#"$root"}"
    elif [[ -f "$p" ]]; then
      printf 'file %s :: %s\n' "${p#"$root"}" "$(cat "$p")"
    else
      printf 'othr %s\n' "${p#"$root"}"
    fi
  done < <(find "$root" -mindepth 1 \( -name .git -prune -print \) -o -print |
    LC_ALL=C sort)
}

# ---------------------------------------------------------------------------
# Case machinery. A failing case records why and keeps going: the point is the
# whole picture, not the first thing that broke.
# ---------------------------------------------------------------------------
CASES=0
FAILED=0
SKIPPED=0
CURRENT=""
RAN=0
RC=0
OUT=""
ERR=""
PROBLEMS=()

start_case() {
  CASES=$((CASES + 1))
  CURRENT="$1"
  PROBLEMS=()
  RAN=0
  RC=0
  OUT=""
  ERR=""
  ARENA=""
  BASE=""
  BIN=""
  CARGO_LOG=""
}

note() { PROBLEMS+=("$1"); }

# run_mkproj [VAR=VAL ...] -- [args to mkproj ...]
#
# `env -i` on purpose: PROJ_DIR, HOME and PATH are the only three things that
# steer this script, and a case that forgot to set one would otherwise inherit
# the developer's.
run_mkproj() {
  local envs=() args=() e
  while (($#)); do
    if [[ "$1" == "--" ]]; then
      shift
      args=("$@")
      break
    fi
    envs+=("$1")
    shift
  done

  for e in ${envs[@]+"${envs[@]}"}; do
    case "$e" in
      PROJ_DIR=*) assert_confined PROJ_DIR "${e#PROJ_DIR=}" ;;
      HOME=*) assert_confined HOME "${e#HOME=}" ;;
    esac
  done

  local errf="$WORK/err.$CASES"
  RAN=1
  RC=0
  OUT="$(env -i \
    PATH="$BIN" \
    HOME="$FAKEHOME" \
    FAKE_CARGO_LOG="$CARGO_LOG" \
    GIT_CONFIG_NOSYSTEM=1 \
    GIT_AUTHOR_NAME="$GIT_AUTHOR_NAME" \
    GIT_AUTHOR_EMAIL="$GIT_AUTHOR_EMAIL" \
    GIT_COMMITTER_NAME="$GIT_COMMITTER_NAME" \
    GIT_COMMITTER_EMAIL="$GIT_COMMITTER_EMAIL" \
    ${envs[@]+"${envs[@]}"} \
    "$BASH_BIN" "$MKPROJ" ${args[@]+"${args[@]}"} 2>"$errf")" || RC=$?
  ERR="$(<"$errf")"
}

# in_base <name> [type] -- the ordinary invocation: PROJ_DIR is this case's base.
in_base() {
  run_mkproj "PROJ_DIR=$BASE" -- "$@"
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

expect_out() {
  if [[ "$OUT" != *"$1"* ]]; then note "stdout does not contain: $1"; fi
}

expect_no_out() {
  if [[ "$OUT" == *"$1"* ]]; then note "stdout should not contain: $1"; fi
}

expect_dir() {
  if [[ ! -d "$1" ]]; then note "expected a directory: $1"; fi
}

expect_no_path() {
  if [[ -e "$1" || -L "$1" ]]; then note "should not exist: $1"; fi
}

expect_file_is() {
  local path="$1" want="$2" got
  if [[ ! -f "$path" ]]; then
    note "expected a regular file: $path"
    return 0
  fi
  got="$(<"$path")"
  if [[ "$got" != "$want" ]]; then
    note "$path contains [$got], expected [$want]"
  fi
}

expect_intact() {
  expect_file_is "$1" "CANARY"
}

# expect_unchanged <dir> <snapshot taken before the run>
expect_unchanged() {
  local now line
  now="$(snapshot "$1")"
  if [[ "$now" != "$2" ]]; then
    note "$1 changed (- before, + after):"
    while IFS= read -r line; do PROBLEMS+=("  - $line"); done <<<"$2"
    while IFS= read -r line; do PROBLEMS+=("  + $line"); done <<<"$now"
  fi
}

# expect_commits <dir> <n>
expect_commits() {
  local n
  if [[ ! -d "$1/.git" ]]; then
    note "no git repository at $1"
    return 0
  fi
  n="$(git -C "$1" rev-list --count HEAD 2>/dev/null || echo none)"
  if [[ "$n" != "$2" ]]; then
    note "$1 has $n commit(s), expected $2"
  fi
}

# expect_cargo_saw_no_option -- NAME must never reach cargo as an argv that
# argument parsing can mistake for a flag.
expect_cargo_saw_no_option() {
  local a
  while IFS= read -r a; do
    case "$a" in
      arg:-*) note "cargo received an option-looking argument: ${a#arg:}" ;;
    esac
  done <"$CARGO_LOG"
}

expect_cargo_not_run() {
  if [[ -s "$CARGO_LOG" ]]; then
    note "cargo was invoked: $(tr '\n' ' ' <"$CARGO_LOG")"
  fi
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
    if [[ -n "$OUT" ]]; then
      printf '        actual stdout:\n'
      while IFS= read -r line; do printf '          | %s\n' "$line"; done <<<"$OUT"
    fi
    if [[ -n "$ERR" ]]; then
      printf '        actual stderr:\n'
      while IFS= read -r line; do printf '          | %s\n' "$line"; done <<<"$ERR"
    fi
  fi
}

skip_case() {
  SKIPPED=$((SKIPPED + 1))
  printf 'SKIP  %s\n' "$1"
  printf '        %s\n' "$2"
}

# =============================================================================
# 0. The harness's own safety rail, judged before anything else runs. If this
#    is broken, every case below is running somewhere unknown.
# =============================================================================
start_case "the_suite_refuses_to_aim_a_run_at_a_base_outside_its_temp_dir"
_rc=0
(assert_confined PROJ_DIR "$HOME/Projects") >/dev/null 2>&1 || _rc=$?
if ((_rc == 0)); then
  note "assert_confined accepted \$HOME/Projects"
fi
_rc=0
(assert_confined PROJ_DIR "$WORK/../elsewhere") >/dev/null 2>&1 || _rc=$?
if ((_rc == 0)); then
  note "assert_confined accepted a path containing '..'"
fi
_rc=0
(assert_confined PROJ_DIR "$WORK/fine") >/dev/null 2>&1 || _rc=$?
if ((_rc != 0)); then
  note "assert_confined rejected a path inside its own temp dir"
fi
end_case
unset _rc

# =============================================================================
# 1. The happy paths that need no toolchain. Three claims each: the directory
#    exists where PROJ_DIR said, the README names the project, and there is
#    exactly ONE commit -- mkproj's whole git story is `init` then one
#    `commit`, and a second commit would mean it found a repo it did not make.
# =============================================================================
start_case "an_empty_project_is_created_in_the_base_with_one_commit_and_a_readme_naming_it"
arena_fixture
bin_fixture
in_base newproj empty
expect_rc 0
expect_out "Created empty project."
expect_dir "$BASE/newproj"
expect_file_is "$BASE/newproj/README.md" "# newproj"
expect_file_is "$BASE/newproj/.gitignore" ".env"
expect_commits "$BASE/newproj" 1
expect_intact "$BASE/existing-dir/CANARY"
end_case

start_case "a_shell_project_gets_an_executable_script_named_after_it_and_one_commit"
arena_fixture
bin_fixture
in_base tool shell
expect_rc 0
expect_out "Created shell project."
expect_dir "$BASE/tool"
expect_file_is "$BASE/tool/README.md" "# tool"
if [[ ! -x "$BASE/tool/tool.sh" ]]; then
  note "expected an executable $BASE/tool/tool.sh"
fi
expect_commits "$BASE/tool" 1
end_case

start_case "a_c_project_gets_its_sources_and_one_commit"
arena_fixture
bin_fixture
in_base cproj c
expect_rc 0
expect_out "Created C project."
expect_file_is "$BASE/cproj/README.md" "# cproj"
if [[ ! -f "$BASE/cproj/src/main.c" ]]; then note "expected $BASE/cproj/src/main.c"; fi
if [[ ! -f "$BASE/cproj/Makefile" ]]; then note "expected $BASE/cproj/Makefile"; fi
expect_commits "$BASE/cproj" 1
end_case

start_case "a_name_with_a_subdirectory_nests_inside_the_base_rather_than_escaping_it"
arena_fixture
bin_fixture
in_base sub/dir empty
expect_rc 0
expect_dir "$BASE/sub/dir"
expect_file_is "$BASE/sub/dir/README.md" "# sub/dir"
expect_commits "$BASE/sub/dir" 1
end_case

start_case "a_trailing_slash_on_the_name_still_names_one_project_inside_the_base"
arena_fixture
bin_fixture
# Shell completion hands out names with a trailing slash, so this spelling has
# to keep working; what it must not do is land anywhere but $BASE/trailing.
in_base "trailing/" empty
expect_rc 0
expect_dir "$BASE/trailing"
expect_commits "$BASE/trailing" 1
expect_no_path "$ARENA/trailing"
end_case

# =============================================================================
# 2. Where the project lands. PROJ_DIR is the only knob a caller has, and the
#    $HOME search order behind it is what makes an unset PROJ_DIR dangerous --
#    so both halves are pinned, and the precedence case proves the search order
#    is not consulted at all when PROJ_DIR is set.
# =============================================================================
start_case "proj_dir_takes_precedence_over_the_home_directory_search_order"
arena_fixture
bin_fixture
# A HOME whose ~/Projects exists and is first in mkproj's own search order.
_home="$WORK/home.$CASES"
mkdir -p "$_home/Projects"
cp "$FAKEHOME/.gitconfig" "$_home/.gitconfig"
printf 'CANARY\n' >"$_home/Projects/CANARY"
_before="$(snapshot "$_home/Projects")"
run_mkproj "PROJ_DIR=$BASE" "HOME=$_home" -- elsewhere empty
expect_rc 0
expect_dir "$BASE/elsewhere"
expect_unchanged "$_home/Projects" "$_before"
end_case
unset _home _before

start_case "the_home_directory_search_order_is_consulted_only_when_proj_dir_is_unset"
arena_fixture
bin_fixture
# Only ~/code exists, so mkproj must walk past Projects and projects to it.
_home="$WORK/home.$CASES"
mkdir -p "$_home/code"
cp "$FAKEHOME/.gitconfig" "$_home/.gitconfig"
run_mkproj "HOME=$_home" -- searched empty
expect_rc 0
expect_dir "$_home/code/searched"
expect_file_is "$_home/code/searched/README.md" "# searched"
expect_no_path "$_home/Projects"
end_case
unset _home

# =============================================================================
# 3. The usage error. `${1:?...}` fires on a null argument as well as a missing
#    one, and neither may leave anything behind.
# =============================================================================
start_case "a_missing_name_is_a_usage_error_that_creates_nothing"
arena_fixture
bin_fixture
_before="$(snapshot "$ARENA")"
run_mkproj "PROJ_DIR=$BASE" --
expect_refused
expect_err "Usage: mkproj <name>"
expect_unchanged "$ARENA" "$_before"
end_case
unset _before

start_case "an_empty_name_is_a_usage_error_that_creates_nothing"
arena_fixture
bin_fixture
_before="$(snapshot "$ARENA")"
in_base "" empty
expect_refused
expect_unchanged "$ARENA" "$_before"
end_case
unset _before

# =============================================================================
# 4. The overwrite refusal -- the reason this script has a guard at all.
#
#    Commit 33246b9 exists because `mkproj <name> rust` used to rm -rf a
#    pre-existing project. Each of these is run with the rust type and a shim
#    cargo on PATH, i.e. down the branch that once did the deleting, and each
#    asserts the canary explicitly: a refusal that still emptied the directory
#    would satisfy an rc check alone.
# =============================================================================
start_case "an_existing_directory_is_refused_and_its_contents_are_left_untouched"
arena_fixture
bin_fixture with-cargo
_before="$(snapshot "$ARENA")"
in_base existing-dir rust
expect_refused
expect_err "already exists -- refusing to overwrite"
expect_intact "$BASE/existing-dir/CANARY"
expect_unchanged "$ARENA" "$_before"
expect_cargo_not_run
end_case
unset _before

start_case "an_existing_file_with_the_projects_name_is_refused_and_its_contents_are_left_untouched"
arena_fixture
bin_fixture with-cargo
_before="$(snapshot "$ARENA")"
in_base existing-file rust
expect_refused
expect_err "already exists -- refusing to overwrite"
expect_intact "$BASE/existing-file"
expect_unchanged "$ARENA" "$_before"
end_case
unset _before

start_case "a_dangling_symlink_with_the_projects_name_is_refused_by_the_guard_and_the_link_survives"
arena_fixture
bin_fixture with-cargo
# `[[ -e ]]` follows the link and is false for a dangling one, so the plain
# guard misses this and the run dies later, in mkdir, with a message about a
# file that "exists" after the script just said it did not. The refusal must
# come from the guard and name the same reason as every other collision.
ln -s /nonexistent/nowhere "$BASE/dangling"
_before="$(snapshot "$ARENA")"
in_base dangling rust
expect_refused
expect_err "already exists -- refusing to overwrite"
if [[ ! -L "$BASE/dangling" ]]; then note "the dangling symlink did not survive"; fi
expect_unchanged "$ARENA" "$_before"
end_case
unset _before

for _pair in "a_name_of_dot_is_refused_and_changes_nothing|." \
  "a_name_of_dotdot_is_refused_and_changes_nothing|.." \
  "an_existing_sibling_named_through_dotdot_is_refused_and_left_untouched|../sibling"; do
  start_case "${_pair%%|*}"
  arena_fixture
  bin_fixture with-cargo
  _before="$(snapshot "$ARENA")"
  in_base "${_pair#*|}" rust
  expect_refused
  expect_intact "$ARENA/sibling/CANARY"
  expect_intact "$ARENA/sibling/deep/CANARY"
  expect_unchanged "$ARENA" "$_before"
  expect_cargo_not_run
  end_case
  unset _before
done
unset _pair

# =============================================================================
# 5. Traversal: the gap between the guard's stat and rm -rf's resolution.
#
#    `[[ -e "$BASE/nope/../existing-dir" ]]` is FALSE, because the stat fails
#    on `nope` -- a component that does not exist yet. mkdir -p then creates
#    it, and `cd "$BASE" && rm -rf "nope/../existing-dir"` resolves the `..`
#    against a path that now exists and deletes the real target. That is
#    precisely the pre-33246b9 behaviour, reachable through a name.
#
#    These cases pin the fix: such a name is refused outright, before anything
#    is created or removed.
# =============================================================================
start_case "a_dotdot_hidden_behind_a_missing_component_cannot_delete_a_project_inside_the_base"
arena_fixture
bin_fixture with-cargo
_before="$(snapshot "$ARENA")"
in_base "nope/../existing-dir" rust
expect_refused
expect_intact "$BASE/existing-dir/CANARY"
expect_no_path "$BASE/nope"
expect_no_out "Created Rust project."
expect_unchanged "$ARENA" "$_before"
expect_cargo_not_run
end_case
unset _before

start_case "a_dotdot_hidden_behind_a_missing_component_cannot_delete_a_tree_outside_the_base"
arena_fixture
bin_fixture with-cargo
_before="$(snapshot "$ARENA")"
in_base "nope/../../sibling" rust
expect_refused
expect_intact "$ARENA/sibling/CANARY"
expect_intact "$ARENA/sibling/deep/CANARY"
expect_file_is "$ARENA/sibling/README.md" "IMPORTANT NOTES DO NOT LOSE"
expect_unchanged "$ARENA" "$_before"
expect_cargo_not_run
end_case
unset _before

start_case "a_dotdot_hidden_behind_a_missing_component_cannot_scaffold_over_an_unrelated_home_directory"
arena_fixture
bin_fixture with-cargo
_before="$(snapshot "$ARENA")"
in_base "nope/../../Documents" rust
expect_refused
expect_intact "$ARENA/Documents/CANARY"
expect_no_path "$ARENA/Documents/Cargo.toml"
expect_unchanged "$ARENA" "$_before"
end_case
unset _before

start_case "a_traversing_name_cannot_truncate_files_outside_the_base_even_with_no_type_specific_rm"
arena_fixture
bin_fixture
# No rust, no rm anywhere on this path: the front matter alone (cd, git init,
# `echo "# $NAME" > README.md`, git add -A, git commit) is enough to destroy a
# README and commit a repo over somebody's data.
_before="$(snapshot "$ARENA")"
in_base "nope/../../sibling" empty
expect_refused
expect_file_is "$ARENA/sibling/README.md" "IMPORTANT NOTES DO NOT LOSE"
expect_intact "$ARENA/sibling/deep/CANARY"
expect_no_path "$ARENA/sibling/.git"
expect_unchanged "$ARENA" "$_before"
end_case
unset _before

start_case "a_traversing_name_cannot_write_a_readme_into_the_parent_of_the_base"
arena_fixture
bin_fixture with-cargo
# GNU rm refuses to remove a ".." directory, so this one already exits
# non-zero -- but only after the front matter has written a README.md one
# level above PROJ_DIR. The rc is not the guarantee; the untouched parent is.
_before="$(snapshot "$ARENA")"
in_base "nope/../.." rust
expect_refused
expect_no_path "$ARENA/README.md"
expect_unchanged "$ARENA" "$_before"
end_case
unset _before

start_case "a_dotdot_name_for_a_nonexistent_sibling_does_not_create_a_project_outside_the_base"
arena_fixture
bin_fixture
# Nothing is destroyed here, but the project lands one level above the
# configured base, next to whatever else lives there, while the success line
# claims the unnormalised path. PROJ_DIR means the base, not a starting point.
_before="$(snapshot "$ARENA")"
in_base "../sibling-new" empty
expect_refused
expect_no_path "$ARENA/sibling-new"
expect_unchanged "$ARENA" "$_before"
end_case
unset _before

start_case "an_absolute_name_cannot_delete_the_directory_it_names"
arena_fixture
bin_fixture with-cargo
# The guard tests "$BASE//abs/path", which cannot exist, so it always passes;
# `rm -rf "$NAME"` then hits the real one.
_before="$(snapshot "$ARENA")"
in_base "$ARENA/abs-target" rust
expect_refused
expect_intact "$ARENA/abs-target/CANARY"
expect_no_path "$ARENA/abs-target/Cargo.toml"
expect_unchanged "$ARENA" "$_before"
expect_cargo_not_run
end_case
unset _before

start_case "an_absolute_name_does_not_mirror_the_whole_path_underneath_the_base"
arena_fixture
bin_fixture
# The non-rust spelling of the same bug: no deletion, but mkdir -p builds
# $BASE/tmp/.../abs-target and the run reports success.
_before="$(snapshot "$ARENA")"
in_base "$ARENA/abs-target" empty
expect_refused
expect_no_out "Created empty project."
expect_unchanged "$ARENA" "$_before"
end_case
unset _before

# =============================================================================
# 6. NAME as argv. On the rust path it is handed to `rm` and to `cargo`, both
#    of which parse leading dashes as options. Nothing destructive was found
#    down this road, but a run that prints "Created Rust project." having
#    created nothing of the sort is a lie the next case in a script will
#    believe.
# =============================================================================
start_case "a_name_starting_with_a_dash_is_refused_before_it_reaches_rm_or_cargo"
arena_fixture
bin_fixture with-cargo
_before="$(snapshot "$ARENA")"
in_base "-rf" rust
expect_refused
expect_err "may not start with '-'"
expect_no_path "$BASE/-rf"
expect_unchanged "$ARENA" "$_before"
expect_cargo_not_run
end_case
unset _before

start_case "the_name_double_dash_help_is_refused_instead_of_reporting_a_project_it_did_not_create"
arena_fixture
bin_fixture with-cargo
_before="$(snapshot "$ARENA")"
in_base "--help" rust
expect_refused
expect_no_out "Created Rust project."
expect_unchanged "$ARENA" "$_before"
expect_cargo_not_run
end_case
unset _before

start_case "shell_metacharacters_in_a_name_are_treated_as_data_and_never_executed"
arena_fixture
bin_fixture
# `touch` is deliberately absent from the PATH the subject is handed, so the
# payload writes its canary with a redirection and the `:` builtin alone --
# an injection that shelled out could not fail this case even if it fired.
_canary="$WORK/injected.$CASES"
_name="x\$(: >$_canary)\`: >$_canary\`;: >$_canary"
in_base "$_name" empty
if [[ -e "$_canary" ]]; then
  note "injection succeeded: $_canary was created"
fi
# Whatever it decides about the name, it must not have run it.
end_case
unset _canary _name

# =============================================================================
# 7. The rust branch itself, against the shim cargo.
#
#    `cd "$BASE" && rm -rf "$NAME"` deletes the README.md the script wrote two
#    lines earlier, so today's rust projects ship without the README every
#    other type gets. cargo init run in place inside the already-created
#    directory keeps it, reuses the existing repo, and writes the same files --
#    so the rm has nothing to do but lose data.
# =============================================================================
start_case "a_rust_project_keeps_the_readme_the_script_just_wrote"
arena_fixture
bin_fixture with-cargo
in_base rustproj rust
expect_rc 0
expect_out "Created Rust project."
expect_file_is "$BASE/rustproj/README.md" "# rustproj"
if [[ ! -f "$BASE/rustproj/Cargo.toml" ]]; then note "expected $BASE/rustproj/Cargo.toml"; fi
if [[ ! -f "$BASE/rustproj/src/main.rs" ]]; then note "expected $BASE/rustproj/src/main.rs"; fi
expect_commits "$BASE/rustproj" 1
expect_cargo_saw_no_option
end_case

start_case "a_rust_project_without_cargo_falls_back_to_a_skeleton_and_still_commits"
arena_fixture
bin_fixture
in_base rustless rust
expect_rc 0
expect_out "cargo not found; creating skeleton."
expect_file_is "$BASE/rustless/README.md" "# rustless"
if [[ ! -f "$BASE/rustless/src/main.rs" ]]; then note "expected $BASE/rustless/src/main.rs"; fi
expect_commits "$BASE/rustless" 1
end_case

# =============================================================================
# Types this suite does not cover, said out loud. A silent skip is a lie about
# coverage: these branches are untested here, and nothing below should be read
# as evidence that they work.
# =============================================================================
skip_case "the_python_type_is_not_exercised" \
  "it runs 'python3 -m venv .venv', which needs a python3 with ensurepip and takes seconds per run"
skip_case "the_go_type_is_not_exercised" \
  "it runs 'go mod init', and no go toolchain is assumed on the runner"
skip_case "the_node_type_is_not_exercised" \
  "it runs 'npm init -y', and no node toolchain is assumed on the runner"

cat <<'NOTE'

note: the rust cases above ran against a shim cargo placed on PATH -- no real
cargo is ever invoked. The shim exists so that mkproj's `command -v cargo`
test is true and the rust branch (the one that removes directories) is the
branch actually taken.
NOTE

# ---------------------------------------------------------------------------
printf '\n%s case(s), %s failed, %s skipped\n' "$CASES" "$FAILED" "$SKIPPED"
if ((FAILED)); then exit 1; fi
exit 0
