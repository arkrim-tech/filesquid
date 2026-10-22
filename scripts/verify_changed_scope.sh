#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Change-scope validation: tests scripts/changed_scope.sh and the hook libraries against a temporary git repository and fixture file lists (pre-commit).
#
# Builds a throwaway git repository under TMPDIR (prefix precommit-scope-test.)
# that mirrors the FileSquid layout and crate graph, with a fake origin/main,
# and checks every scope: docs (with the always-run checker scripts and the
# root .claude/ and docs/), rust:<crates> with reverse dependencies,
# npm:<packages> with file: links and the packages' own manifests, mixed
# scopes, every full trigger (gate-logic and unknown scripts included), and the
# fallbacks to full (FULL=1, unknown base, not a git repository, cargo
# metadata failure, unknown crate directory, missing file list). It also
# checks that the hook ignores the fixture-only overrides, the scope grammar
# and --scope, the pure scope parsers, the Cargo lock path and target
# fallbacks, that the default lock file is gitignored, and the bounded lock
# wait (including that a command's own exit 75 is not a timeout). Takes seconds and
# needs no network or Cargo build (one case runs `cargo metadata --no-deps`).
#
# Usage: ./scripts/verify_changed_scope.sh   (from the repository root)

echo "Starting Change Scope Validation..."

# Inside a git hook, git exports repository variables such as GIT_DIR and
# GIT_INDEX_FILE (absolute paths in a linked worktree). Inherited, they would
# point the fixture repository's commands, reset --hard included, at the real
# repository, so drop them before any fixture is created.
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

# The cases set their own environment; the caller's (for example FULL=1 on a
# full hook run) must not leak into them.
unset FULL CHANGED_SCOPE_FILES CHANGED_SCOPE_BASE CHANGED_SCOPE_METADATA FILESQUID_CARGO_LOCK_WAIT

REPO_ROOT="$(pwd)"
HELPER="$REPO_ROOT/scripts/changed_scope.sh"
WORK=$(mktemp -d "${TMPDIR:-/tmp}/precommit-scope-test.XXXXXX") || { echo "❌ cannot create a temporary directory"; exit 1; }
trap 'rm -rf "$WORK"' EXIT
FIXTURE="$WORK/repo"
METADATA="$WORK/metadata.json"
FAILED=0
PASSED=0

. ./scripts/lib/cargo_shared.sh || { echo "❌ cannot load scripts/lib/cargo_shared.sh"; exit 1; }
. ./scripts/lib/hook_scope.sh || { echo "❌ cannot load scripts/lib/hook_scope.sh"; exit 1; }

# Records one assertion. $1 = label, $2 = expected, $3 = actual.
assert_eq() {
  if [ "$2" = "$3" ]; then
    PASSED=$((PASSED + 1))
  else
    echo "❌ $1: expected '$2', got '$3'"
    FAILED=1
  fi
}

# Runs git quietly in the fixture repository with a neutral identity and no hooks.
fixture_git() {
  git -C "$FIXTURE" -c user.name=scope-test -c user.email=scope-test@example.invalid \
    -c commit.gpgsign=false -c core.hooksPath=/dev/null "$@" >/dev/null 2>&1
}

# Runs fixture_git "$@" and records a failure when the git command fails, so a
# broken fixture step cannot let a later assertion pass for the wrong reason.
fixture_must() {
  if fixture_git "$@"; then
    PASSED=$((PASSED + 1))
  else
    echo "❌ fixture step failed: git $*"
    FAILED=1
  fi
}

# Runs the helper in directory $1 with only the given overrides ("$@" after $1).
scope_in() {
  local dir="$1"; shift
  (cd "$dir" && env -u FULL -u CHANGED_SCOPE_FILES -u CHANGED_SCOPE_BASE \
    CHANGED_SCOPE_METADATA="$METADATA" "$@" "$HELPER" 2>/dev/null)
}

# Writes file $1 (relative to the fixture) with content $2, creating directories.
put() {
  mkdir -p "$(dirname "$FIXTURE/$1")" && printf '%s\n' "$2" >"$FIXTURE/$1"
}

# Prints one cargo-metadata package entry: $1 = name, $2 = crate dir, rest = path deps.
metadata_package() {
  local name="$1" dir="$2" deps="" dep; shift 2
  for dep in "$@"; do
    deps="$deps${deps:+,}{\"name\":\"$dep\",\"path\":\"$FIXTURE/apps/filesquid/crates/${dep#filesquid-}\"}"
  done
  printf '{"name":"%s","manifest_path":"%s/apps/filesquid/crates/%s/Cargo.toml","dependencies":[%s]}' \
    "$name" "$FIXTURE" "$dir" "$deps"
}

# Writes the cargo metadata fixture mirroring the FileSquid crate graph.
write_metadata_fixture() {
  {
    printf '{"workspace_root":"%s/apps/filesquid","packages":[' "$FIXTURE"
    metadata_package filesquid-core core; printf ','
    metadata_package filesquid-api api filesquid-core filesquid-prov; printf ','
    metadata_package filesquid-native native filesquid-api; printf ','
    metadata_package filesquid-ocfl ocfl; printf ','
    metadata_package filesquid-metadata metadata filesquid-ocfl; printf ','
    metadata_package filesquid-c2pa c2pa; printf ','
    metadata_package filesquid-prov prov; printf ','
    metadata_package filesquid-cli cli filesquid-metadata; printf ','
    metadata_package filesquid-server server filesquid-api filesquid-native; printf ','
    metadata_package filesquid-desktop desktop filesquid-api filesquid-core filesquid-metadata filesquid-native filesquid-ocfl
    printf ']}\n'
  } >"$METADATA"
}

# Creates the fixture repository, commits it and points origin/main at it.
make_fixture_repo() {
  local crate
  mkdir -p "$FIXTURE" && git init -q "$FIXTURE" || return 1
  for crate in core api native ocfl metadata c2pa prov cli server desktop; do
    put "apps/filesquid/crates/$crate/src/lib.rs" "// $crate"
  done
  put apps/filesquid/Cargo.toml "[workspace]"
  put apps/filesquid/Cargo.lock "# lock"
  put apps/filesquid/rust-toolchain.toml "[toolchain]"
  put apps/filesquid/crates/api/openapi.json "{}"
  put apps/filesquid/ui/package.json '{"name":"@filesquid/ui"}'
  put apps/filesquid/ui/src/main.ts "export {};"
  put apps/imagesquid/package.json '{"name":"@imagesquid/ui","dependencies":{"@filesquid/ui":"file:../filesquid/ui"}}'
  put apps/imagesquid/src/main.ts "export {};"
  put README.md "# Fixture"
  put resources/prd.md "# PRD"
  put .agents/rules/rule.md "# Rule"
  put .gitignore "target/"
  fixture_git add -A && fixture_git commit -q -m init || return 1
  fixture_git update-ref refs/remotes/origin/main HEAD && fixture_git checkout -q -b build/fixture
}

# Returns the fixture to origin/main with a clean working tree.
reset_fixture() {
  fixture_must checkout -q -B build/fixture origin/main
  fixture_must reset -q --hard origin/main
  fixture_must clean -fdq
}

# Checks the helper against fixture file lists. $1 = expected, rest = paths.
expect_list() {
  local expected="$1" list="$WORK/files.txt"; shift
  printf '%s\n' "$@" >"$list"
  assert_eq "files [$*]" "$expected" "$(scope_in "$FIXTURE" CHANGED_SCOPE_FILES="$list")"
}

CORE_SCOPE="rust:filesquid-api,filesquid-core,filesquid-desktop,filesquid-native,filesquid-server"
OCFL_SCOPE="rust:filesquid-cli,filesquid-desktop,filesquid-metadata,filesquid-ocfl"
UI_SCOPE="npm:apps/filesquid/ui,apps/imagesquid"
API_SCOPE="rust:filesquid-api,filesquid-desktop,filesquid-native,filesquid-server"
PROV_SCOPE="rust:filesquid-api,filesquid-desktop,filesquid-native,filesquid-prov,filesquid-server"

test_file_lists() {
  expect_list docs README.md resources/prd.md .agents/rules/rule.md docs/guide.md
  expect_list docs apps/filesquid/resources/notes.md apps/README.md
  expect_list "$API_SCOPE" apps/filesquid/crates/api/resources/srs.md apps/filesquid/resources/notes.md
  expect_list "rust:filesquid-c2pa" apps/filesquid/crates/c2pa/README.md
  expect_list "$PROV_SCOPE" apps/filesquid/crates/prov/tests/features/prov_o_model_F-067.feature
  expect_list "$PROV_SCOPE" apps/filesquid/crates/prov/src/lib.rs
  expect_list "$UI_SCOPE" apps/filesquid/ui/resources/tasks/Task-x-Walkthrough.md
  expect_list "npm:apps/imagesquid" apps/imagesquid/resources/prd.md
  expect_list "$OCFL_SCOPE" apps/filesquid/crates/ocfl/tests/fixtures/README.md
  expect_list "$OCFL_SCOPE" apps/filesquid/crates/ocfl/tests/resources/case.json
  expect_list "$CORE_SCOPE" apps/filesquid/crates/core/src/lib.rs README.md
  expect_list "rust:filesquid-server" apps/filesquid/crates/server/src/main.rs
  expect_list "$UI_SCOPE" apps/filesquid/ui/src/main.ts
  expect_list "$UI_SCOPE" apps/filesquid/ui/README.md
  expect_list "npm:apps/imagesquid" apps/imagesquid/src/main.ts
  expect_list "rust:filesquid-c2pa npm:apps/imagesquid" apps/filesquid/crates/c2pa/src/lib.rs apps/imagesquid/src/main.ts
  expect_list docs ""
}

test_full_triggers() {
  local path
  for path in apps/filesquid/Cargo.toml apps/filesquid/Cargo.lock apps/filesquid/rust-toolchain.toml \
    apps/filesquid/rust-toolchain package.json package-lock.json apps/package.json \
    apps/filesquid/crates/desktop/tests/e2e/package.json apps/filesquid/crates/desktop/tests/e2e/package-lock.json \
    apps/filesquid/crates/api/openapi.json apps/filesquid/ui/src/api/schema.d.ts docker-compose.yml \
    apps/filesquid/distribution.json apps/filesquid/crates/newcrate/src/lib.rs apps/other/main.rs; do
    expect_list full "$path"
  done
  expect_list full README.md apps/filesquid/crates/c2pa/src/lib.rs scripts/pre-commit.sh
  expect_list full .claude/agents/x.md apps/filesquid/ui/package.json package.json
  assert_eq "missing file list" full "$(scope_in "$FIXTURE" CHANGED_SCOPE_FILES="$WORK/none.txt")"
}

# Text checkers that steps 0 to 9 run on every commit, whatever the scope.
DOCS_CHECKERS="scripts/verify_branch.sh scripts/verify_isolated_ports.sh scripts/verify_doc_sync.sh
  scripts/verify_srs_traceability.sh scripts/verify_relative_links.sh scripts/verify_workspace_boundaries.sh
  scripts/verify_centralized_logging.sh scripts/verify_docs.sh scripts/verify_task_cleanup.sh
  scripts/docs/check_links.py scripts/docs/validate-adrs.test.mjs scripts/docs/new_checker.py"
# Scripts that decide or run the scope; any other script also stays full.
GATE_SCRIPTS="scripts/pre-commit.sh scripts/changed_scope.sh scripts/verify_changed_scope.sh
  scripts/verify_tests.sh scripts/verify_formatting.sh scripts/lib/cargo_shared.sh scripts/lib/hook_scope.sh
  scripts/lib/new_lib.sh scripts/setup-dev-environment.sh scripts/verify_new_gate.sh scripts/tool.py
  scripts/verify_docs.sh.bak scripts/docs"

test_script_split() {
  local path
  for path in $DOCS_CHECKERS; do
    expect_list docs "$path"
  done
  for path in $GATE_SCRIPTS; do
    expect_list full "$path"
  done
  expect_list docs scripts/verify_docs.sh scripts/docs/check_links.py README.md .agents/rules/rule.md
  expect_list full scripts/verify_docs.sh scripts/verify_tests.sh
  expect_list "rust:filesquid-c2pa" scripts/verify_branch.sh apps/filesquid/crates/c2pa/src/lib.rs
}

test_npm_manifests() {
  expect_list "$UI_SCOPE" apps/filesquid/ui/package-lock.json
  expect_list "$UI_SCOPE" apps/filesquid/ui/package.json
  expect_list "npm:apps/imagesquid" apps/imagesquid/package.json
  expect_list "npm:apps/imagesquid" apps/imagesquid/package-lock.json apps/imagesquid/src/main.ts
  expect_list "rust:filesquid-c2pa $UI_SCOPE" apps/filesquid/ui/package-lock.json apps/filesquid/crates/c2pa/src/lib.rs
  expect_list full apps/filesquid/ui/package.json apps/filesquid/ui/src/api/schema.d.ts
  expect_list full apps/imagesquid/package.json apps/filesquid/Cargo.lock
}

test_docs_directories() {
  expect_list docs .claude/agents/x.md
  expect_list docs docs/x.md
  expect_list docs .claude/settings.json docs/diagram.svg
  expect_list docs .claude/agents/x.md docs/x.md scripts/verify_docs.sh resources/prd.md
  expect_list "$UI_SCOPE" .claude/agents/x.md docs/x.md apps/filesquid/ui/package-lock.json
  expect_list full .claude/agents/x.md docs/x.md scripts/changed_scope.sh
  expect_list full apps/docs/x.ts
  expect_list full apps/filesquid/.claude/settings.json
}

test_git_changes() {
  reset_fixture
  assert_eq "no changes" docs "$(scope_in "$FIXTURE")"
  put README.md "# Edited"; put resources/prd.md "# Edited"; put .agents/rules/rule.md "# Edited"
  assert_eq "unstaged docs" docs "$(scope_in "$FIXTURE")"
  reset_fixture; put apps/filesquid/crates/core/src/lib.rs "// staged"; fixture_must add -A
  assert_eq "staged core" "$CORE_SCOPE" "$(scope_in "$FIXTURE")"
  reset_fixture; put apps/filesquid/crates/c2pa/src/lib.rs "// unstaged"
  put apps/imagesquid/src/main.ts "// staged"; fixture_must add apps/imagesquid
  assert_eq "staged and unstaged" "rust:filesquid-c2pa npm:apps/imagesquid" "$(scope_in "$FIXTURE")"
  reset_fixture; put apps/filesquid/crates/server/src/new.rs "// untracked"
  assert_eq "untracked" "rust:filesquid-server" "$(scope_in "$FIXTURE")"
  reset_fixture; put apps/filesquid/target/debug/out "ignored"
  assert_eq "ignored file" docs "$(scope_in "$FIXTURE")"
  reset_fixture; put apps/filesquid/crates/desktop/src/lib.rs "// committed"
  fixture_must commit -qam "branch commit"
  assert_eq "committed change is no longer in the working tree" "" "$(git -C "$FIXTURE" status --porcelain)"
  assert_eq "committed on branch" "rust:filesquid-desktop" "$(scope_in "$FIXTURE")"
  reset_fixture; fixture_must mv apps/filesquid/crates/c2pa/src/lib.rs apps/filesquid/crates/server/src/c2pa.rs
  assert_eq "rename across crates" "rust:filesquid-c2pa,filesquid-server" "$(scope_in "$FIXTURE")"
}

test_fallbacks() {
  reset_fixture; put README.md "# Edited"
  assert_eq "FULL=1" full "$(scope_in "$FIXTURE" FULL=1)"
  assert_eq "unknown base" full "$(scope_in "$FIXTURE" CHANGED_SCOPE_BASE=origin/missing)"
  mkdir -p "$WORK/plain"
  assert_eq "not a git repository" full "$(scope_in "$WORK/plain" GIT_CEILING_DIRECTORIES="$WORK")"
  put apps/filesquid/crates/core/src/lib.rs "// changed"
  assert_eq "cargo metadata failure" full "$(scope_in "$FIXTURE" CHANGED_SCOPE_METADATA="$WORK/missing.json")"
  reset_fixture
}

test_hook_ignores_overrides() {
  local list="$WORK/docs-only.txt" scope
  reset_fixture; mkdir -p "$FIXTURE/scripts" && cp "$HELPER" "$FIXTURE/scripts/changed_scope.sh"
  fixture_must add -A
  fixture_must commit -qm "helper"
  fixture_must update-ref refs/remotes/origin/main HEAD
  put apps/imagesquid/src/main.ts "// changed"; printf 'README.md\n' >"$list"
  scope=$(cd "$FIXTURE" && CHANGED_SCOPE_FILES="$list" CHANGED_SCOPE_BASE=HEAD hook_scope --quiet)
  assert_eq "hook ignores CHANGED_SCOPE_* overrides" "npm:apps/imagesquid" "$scope"
  reset_fixture
}

test_scope_parsers() {
  assert_eq "rust crates" "a,b" "$(scope_rust_crates "rust:a,b npm:x")"
  assert_eq "npm packages" "x,y" "$(scope_npm_packages "rust:a npm:x,y")"
  assert_eq "no npm packages" "" "$(scope_npm_packages "rust:a")"
  assert_eq "lock beside absolute target" "/srv/target.lock" "$(cargo_lock_path /srv/target/ /work)"
  assert_eq "lock beside relative target" "/work/target.lock" "$(cargo_lock_path target /work)"
  assert_eq "default lock wait" 1800 "$(FILESQUID_CARGO_LOCK_WAIT=abc cargo_lock_wait_seconds)"
  assert_eq "custom lock wait" 7 "$(FILESQUID_CARGO_LOCK_WAIT=7 cargo_lock_wait_seconds)"
  assert_eq "target from CARGO_TARGET_DIR" /a "$(CARGO_TARGET_DIR=/a CARGO_BUILD_TARGET_DIR=/b cargo_target_dir)"
  assert_eq "target from CARGO_BUILD_TARGET_DIR" /b "$(env -u CARGO_TARGET_DIR CARGO_BUILD_TARGET_DIR=/b bash -c '. ./scripts/lib/cargo_shared.sh; cargo_target_dir')"
  assert_eq "default target" target "$(env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR bash -c '. ./scripts/lib/cargo_shared.sh; cargo_target_dir')"
}

# Records whether is_valid_scope accepts $2 as expected ($1 = yes or no).
expect_valid() {
  local actual=no
  is_valid_scope "$2" && actual=yes
  assert_eq "scope grammar '$2'" "$1" "$actual"
}

test_scope_grammar() {
  local scope
  expect_valid yes docs
  expect_valid yes full
  expect_valid yes "rust:filesquid-api,filesquid-server"
  expect_valid yes "npm:apps/filesquid/ui,apps/imagesquid"
  expect_valid yes "rust:filesquid-c2pa npm:apps/imagesquid"
  expect_valid no ""
  expect_valid no "rust:"
  expect_valid no "npm:"
  expect_valid no "rust:a extra"
  expect_valid no "npm:x rust:a"
  expect_valid no "rust:a,,b"
  expect_valid no "$(printf 'docs\nfull')"
  scope=$(scope_or_detect --scope "rust:" 2>/dev/null)
  assert_eq "invalid --scope runs full" full "$scope"
  assert_eq "valid --scope is used" docs "$(scope_or_detect --scope docs)"
  assert_eq "--scope=docs runs full" full "$(scope_or_detect --scope=docs 2>/dev/null)"
  assert_eq "misspelled option runs full" full "$(scope_or_detect --scop docs 2>/dev/null)"
  assert_eq "extra arguments run full" full "$(scope_or_detect --scope docs extra 2>/dev/null)"
}

test_lock_file_is_ignored() {
  local path="apps/filesquid/target.lock"
  if git check-ignore -q "$path"; then
    PASSED=$((PASSED + 1))
  else
    echo "❌ $path (the default lock beside target/) is not gitignored; it would force full scope"
    FAILED=1
  fi
}

# Waits (at most 50 x 0.1s) until lock file $1 is held by another process.
wait_until_locked() {
  local tries=0
  while [ "$tries" -lt 50 ] && flock -n "$1" true; do
    sleep 0.1
    tries=$((tries + 1))
  done
}

test_lock_timeout() {
  local holder status via_link lock
  command -v flock >/dev/null 2>&1 || { echo "ℹ️  flock not found; lock timeout test skipped."; return; }
  mkdir -p "$WORK/locked/target" && ln -s "$WORK/locked/target" "$WORK/target-link"
  lock="$(cd "$WORK/locked" && pwd -P)/target.lock"
  flock "$lock" sleep 4 &
  holder=$!
  wait_until_locked "$lock"
  (cd "$WORK/locked" && CARGO_TARGET_DIR=target FILESQUID_CARGO_LOCK_WAIT=1 with_cargo_lock true 2>/dev/null)
  status=$?
  (cd "$WORK" && CARGO_TARGET_DIR="$WORK/target-link" FILESQUID_CARGO_LOCK_WAIT=1 with_cargo_lock true 2>/dev/null)
  via_link=$?
  wait "$holder"
  assert_eq "bounded lock wait times out with 75" 75 "$status"
  assert_eq "a symlinked target shares the real target's lock" 75 "$via_link"
  (cd "$WORK/locked" && CARGO_TARGET_DIR=target FILESQUID_CARGO_LOCK_WAIT=5 with_cargo_lock true)
  assert_eq "lock acquired once free" 0 "$?"
  local message
  message=$(cd "$WORK/locked" && CARGO_TARGET_DIR=target with_cargo_lock sh -c 'exit 75' 2>&1)
  assert_eq "a command's own exit 75 is not reported as a lock timeout" "" "$message"
  test_lock_ignores_cdpath
}

test_lock_ignores_cdpath() {
  local output
  mkdir -p "$WORK/cdpath-case" "$WORK/decoy/target"
  output=$(cd "$WORK/cdpath-case" && CDPATH="$WORK/decoy" CARGO_TARGET_DIR=target with_cargo_lock echo ran)
  assert_eq "CDPATH does not redirect the lock or add output" ran "$output"
  assert_eq "lock created beside the local target" yes "$([ -f "$WORK/cdpath-case/target.lock" ] && echo yes)"
  assert_eq "no lock beside the CDPATH decoy" no "$([ -f "$WORK/decoy/target.lock" ] && echo yes || echo no)"
}

test_real_workspace() {
  local list="$WORK/real.txt" scope
  printf 'apps/filesquid/crates/api/src/lib.rs\n' >"$list"
  scope=$(env -u CHANGED_SCOPE_METADATA CHANGED_SCOPE_FILES="$list" "$HELPER" 2>/dev/null)
  case "$scope" in
    rust:*filesquid-api*filesquid-server*) PASSED=$((PASSED + 1)) ;;
    *) echo "❌ real workspace: expected api and its dependent server in scope, got '$scope'"; FAILED=1 ;;
  esac
}

if ! make_fixture_repo; then
  echo "❌ cannot build the fixture git repository in $FIXTURE"
  exit 1
fi
write_metadata_fixture
test_file_lists
test_full_triggers
test_script_split
test_npm_manifests
test_docs_directories
test_git_changes
test_fallbacks
test_hook_ignores_overrides
test_scope_parsers
test_scope_grammar
test_lock_file_is_ignored
test_lock_timeout
test_real_workspace

if [ "$FAILED" -ne 0 ]; then
  echo ""
  echo "🚨 VALIDATION FAILED: scripts/changed_scope.sh or a hook library broke an expectation above."
  exit 1
fi

echo "✅ Change scope validation passed ($PASSED assertions)."
exit 0
