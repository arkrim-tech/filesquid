#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Change-scope validation: tests scripts/changed_scope.sh and the hook libraries against a temporary git repository and fixture file lists (pre-commit).
#
# Builds a throwaway git repository under TMPDIR (prefix precommit-scope-test.)
# that mirrors the FileSquid layout and crate graph, with a fake origin/main,
# and checks every scope: docs, rust:<crates> with reverse dependencies,
# npm:<packages> with file: links, mixed scopes, every full trigger, and the
# fallbacks to full (FULL=1, unknown base, not a git repository, cargo
# metadata failure, unknown crate directory, missing file list). It also
# checks that the hook ignores the fixture-only overrides, the pure scope
# parsers, the Cargo lock path, and the bounded lock wait. Takes seconds and
# needs no network or Cargo build (one case runs `cargo metadata --no-deps`).
#
# Usage: ./scripts/verify_changed_scope.sh   (from the repository root)

echo "Starting Change Scope Validation..."

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

. ./scripts/lib/cargo_shared.sh 2>/dev/null
. ./scripts/lib/hook_scope.sh

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
    metadata_package filesquid-api api filesquid-core; printf ','
    metadata_package filesquid-native native filesquid-api; printf ','
    metadata_package filesquid-ocfl ocfl; printf ','
    metadata_package filesquid-metadata metadata filesquid-ocfl; printf ','
    metadata_package filesquid-c2pa c2pa; printf ','
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
  for crate in core api native ocfl metadata c2pa cli server desktop; do
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
  fixture_git checkout -q -B build/fixture origin/main
  fixture_git reset -q --hard origin/main
  fixture_git clean -fdq
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

test_file_lists() {
  expect_list docs README.md resources/prd.md .agents/rules/rule.md docs/guide.md
  expect_list docs apps/filesquid/crates/api/resources/plan.md apps/filesquid/resources/notes.md
  expect_list docs apps/imagesquid/resources/prd.md apps/filesquid/ui/resources/tasks/Task-x-Walkthrough.md
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
    apps/filesquid/rust-toolchain apps/filesquid/ui/package.json apps/imagesquid/package-lock.json \
    apps/filesquid/crates/desktop/tests/e2e/package.json scripts/verify_tests.sh scripts/lib/cargo_shared.sh \
    apps/filesquid/crates/api/openapi.json apps/filesquid/ui/src/api/schema.d.ts docker-compose.yml \
    apps/filesquid/distribution.json apps/filesquid/crates/newcrate/src/lib.rs apps/other/main.rs; do
    expect_list full "$path"
  done
  expect_list full README.md apps/filesquid/crates/c2pa/src/lib.rs scripts/pre-commit.sh
  assert_eq "missing file list" full "$(scope_in "$FIXTURE" CHANGED_SCOPE_FILES="$WORK/none.txt")"
}

test_git_changes() {
  reset_fixture
  assert_eq "no changes" docs "$(scope_in "$FIXTURE")"
  put README.md "# Edited"; put resources/prd.md "# Edited"; put .agents/rules/rule.md "# Edited"
  assert_eq "unstaged docs" docs "$(scope_in "$FIXTURE")"
  reset_fixture; put apps/filesquid/crates/core/src/lib.rs "// staged"; fixture_git add -A
  assert_eq "staged core" "$CORE_SCOPE" "$(scope_in "$FIXTURE")"
  reset_fixture; put apps/filesquid/crates/c2pa/src/lib.rs "// unstaged"
  put apps/imagesquid/src/main.ts "// staged"; fixture_git add apps/imagesquid
  assert_eq "staged and unstaged" "rust:filesquid-c2pa npm:apps/imagesquid" "$(scope_in "$FIXTURE")"
  reset_fixture; put apps/filesquid/crates/server/src/new.rs "// untracked"
  assert_eq "untracked" "rust:filesquid-server" "$(scope_in "$FIXTURE")"
  reset_fixture; put apps/filesquid/target/debug/out "ignored"
  assert_eq "ignored file" docs "$(scope_in "$FIXTURE")"
  reset_fixture; put apps/filesquid/crates/desktop/src/lib.rs "// committed"
  fixture_git commit -qam "branch commit"
  assert_eq "committed on branch" "rust:filesquid-desktop" "$(scope_in "$FIXTURE")"
  reset_fixture; fixture_git mv apps/filesquid/crates/c2pa/src/lib.rs apps/filesquid/crates/server/src/c2pa.rs
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
  fixture_git add -A && fixture_git commit -qm "helper" && fixture_git update-ref refs/remotes/origin/main HEAD
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
  local holder status
  command -v flock >/dev/null 2>&1 || { echo "ℹ️  flock not found; lock timeout test skipped."; return; }
  mkdir -p "$WORK/locked"
  flock "$WORK/locked/target.lock" sleep 3 &
  holder=$!
  wait_until_locked "$WORK/locked/target.lock"
  (cd "$WORK/locked" && CARGO_TARGET_DIR=target FILESQUID_CARGO_LOCK_WAIT=1 with_cargo_lock true 2>/dev/null)
  status=$?
  wait "$holder"
  assert_eq "bounded lock wait times out with 75" 75 "$status"
  (cd "$WORK/locked" && CARGO_TARGET_DIR=target FILESQUID_CARGO_LOCK_WAIT=5 with_cargo_lock true)
  assert_eq "lock acquired once free" 0 "$?"
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
test_git_changes
test_fallbacks
test_hook_ignores_overrides
test_scope_parsers
test_lock_timeout
test_real_workspace

if [ "$FAILED" -ne 0 ]; then
  echo ""
  echo "🚨 VALIDATION FAILED: scripts/changed_scope.sh or a hook library broke an expectation above."
  exit 1
fi

echo "✅ Change scope validation passed ($PASSED assertions)."
exit 0
