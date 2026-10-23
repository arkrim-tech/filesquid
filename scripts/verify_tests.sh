#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Lint and test validation: runs the Cargo and npm quality gates that stand in for hosted CI, limited to the change scope (pre-commit, before every push).
#
# This repository has no hosted CI; these are the gates that would otherwise
# run there. The scope comes from scripts/changed_scope.sh (the diff between
# the merge-base with origin/main and the working tree):
#
#   full  (FULL=1, the Cargo manifest or lockfile, a toolchain, an unlisted package
#          manifest, a gate-logic script or contract file
#          changed, or detection failed) — every step below:
#     From the FileSquid Cargo workspace (apps/filesquid/):
#       1. cargo clippy --workspace --all-targets --locked -- -D warnings
#       2. cargo test --workspace --locked
#       3. cargo test -p <crate> --locked for each module crate alone (ADR 029), and
#          for the native-processing library filesquid-native, which the hosts link
#       4. cargo doc --workspace --no-deps --locked with warnings denied
#       5. Clippy and cargo test for filesquid-desktop with --features test-webdriver
#          (the test-instrumented build: range fixture and BDD features)
#       5b. filesquid-metadata feature matrix: clippy and tests with --all-features (the
#          `ocfl`, `ocfl-fs` and `cli` tests are skipped by the default features), and
#          `cargo check --lib --target wasm32-unknown-unknown` with default features off,
#          and with the portable `ocfl` feature (MD-S-009)
#       5c. filesquid-prov feature matrix: clippy and tests with --all-features (the `ocfl`
#          tests are skipped by the default features), and `cargo check --lib --target
#          wasm32-unknown-unknown` with default features and with `ocfl` (PROV-S-001, PROV-S-006)
#     In the FileSquid UI package (apps/filesquid/ui), the only npm suite the hook
#     runs during the MVP (owner decision 2026-10-23, .agents/rules/local-verification.md):
#       6. npm run lint, npm run check, npm test (its distribution-table test reads
#          apps/imagesquid's label table and token file, UI-S-005)
#       7. npm run test:e2e, only when FILESQUID_E2E=1
#     apps/imagesquid keeps its own npm scripts for running by hand; the hook does not
#     run its lint, typecheck, Vitest or Playwright suite.
#     Desktop packaged window (apps/filesquid/crates/desktop/tests/e2e):
#       8. only when FILESQUID_DESKTOP_E2E=1: build the test-instrumented binary,
#          check formatting, run WebdriverIO (under xvfb-run on Linux without a display)
#   rust:<crates>  steps 1, 2 and 4 with `-p <crate>` for each listed crate (the
#          changed crates and every crate that depends on them), step 3 for each
#          listed module crate, step 5b only when filesquid-metadata is listed, step 5c only
#          when filesquid-prov is listed, steps 5 and
#          8 only when filesquid-desktop is listed
#   npm:<packages> steps 6 and 7 for the listed packages only
#   docs  no Cargo or npm step; each skipped step is printed with the reason
#
# Every cargo command runs under the shared-target lock (scripts/lib/cargo_shared.sh).
#
# Environment:
#   FULL=1                  run the full suite whatever changed (required, when
#                           the PR scope is full or includes rust:, before a PR
#                           is first marked ready and on its final commit)
#   FILESQUID_SKIP_TESTS=1  skip this script (work-in-progress commits only;
#                           run it before pushing)
#   FILESQUID_E2E=1         also run Playwright end-to-end tests for packages in scope
#   FILESQUID_DESKTOP_E2E=1 also run the desktop packaged-window tests when the
#                           desktop crate is in scope
#   FILESQUID_CARGO_LOCK_WAIT  seconds to wait for the shared-target lock (default 1800)
#
# Usage: ./scripts/verify_tests.sh [--scope <scope>]   (from the repository root)
#   Without arguments the scope is detected. scripts/pre-commit.sh passes the
#   scope it detected once. Any other argument, or an invalid scope, runs the
#   full suite after a message.

echo "Starting Lint and Test Validation..."

if [ "$FILESQUID_SKIP_TESTS" = "1" ]; then
  echo "⚠️  WARNING: FILESQUID_SKIP_TESTS=1, lint and tests skipped. Run ./scripts/verify_tests.sh before pushing."
  exit 0
fi

. ./scripts/lib/cargo_shared.sh
. ./scripts/lib/hook_scope.sh

WORKSPACE="apps/filesquid"
MODULE_CRATES="filesquid-core filesquid-ocfl filesquid-metadata filesquid-c2pa filesquid-prov filesquid-api filesquid-native"
DESKTOP_CRATE="filesquid-desktop"
# The npm suites the hook runs, from scripts/lib/hook_scope.sh.
UI_PACKAGES="$HOOK_NPM_PACKAGES"
# Packages whose own suites the hook does not run during the MVP (owner decision
# 2026-10-23): the guarded directories of HOOK_GUARDED_PACKAGES.
UNHOOKED_PACKAGES=$(for pair in $HOOK_GUARDED_PACKAGES; do printf '%s ' "${pair%%=*}"; done)
FAILED=0

run_step() {
  # $1 = label, $2 = directory, remaining = command. Streams output so a
  # failure shows its cause; records failure instead of exiting.
  local label="$1" dir="$2" started=$SECONDS; shift 2
  echo "🔍 $label ($dir)..."
  if (cd "$dir" && "$@"); then
    echo "✅ $label passed ($((SECONDS - started))s)."
  else
    echo "❌ $label failed ($((SECONDS - started))s)."
    FAILED=1
  fi
}

skip_step() {
  # $1 = label, $2 = reason. Prints a step that the scope left out.
  echo "⏭️  Skipped: $1 ($2)"
}

rustdoc_locked() {
  # cargo doc with warnings denied, under the shared-target lock.
  RUSTDOCFLAGS="-D warnings" cargo_locked doc "$@"
}

contains_word() {
  # Succeeds when word $2 is in the space-separated list $1. Pure.
  case " $1 " in *" $2 "*) return 0 ;; *) return 1 ;; esac
}

run_desktop_instrumented() {
  # Clippy and tests of the test-instrumented desktop build (feature test-webdriver).
  run_step "Clippy (desktop, test-instrumented)" "$WORKSPACE" cargo_locked clippy -p "$DESKTOP_CRATE" --all-targets --features test-webdriver --locked -- -D warnings
  run_step "Desktop tests (test-instrumented)" "$WORKSPACE" cargo_locked test -p "$DESKTOP_CRATE" --features test-webdriver --locked
}

skip_desktop_instrumented() {
  # $1 = reason.
  skip_step "Clippy (desktop, test-instrumented)" "$1"
  skip_step "Desktop tests (test-instrumented)" "$1"
}

wasm_target_ready() {
  # Succeeds when the wasm32-unknown-unknown target is installed for the
  # workspace toolchain; otherwise prints the fix and records a failure.
  local wasm=wasm32-unknown-unknown installed
  # Query from $WORKSPACE so the workspace rust-toolchain.toml picks the toolchain cargo uses.
  if ! installed=$(cd "$WORKSPACE" && rustup target list --installed); then
    echo "❌ ERROR: 'rustup target list --installed' failed in $WORKSPACE (see the message above)."
    FAILED=1
    return 1
  fi
  if ! echo "$installed" | grep -qx "$wasm"; then
    echo "❌ ERROR: Rust target $wasm is not installed. Run 'rustup target add $wasm'."
    FAILED=1
    return 1
  fi
}

run_metadata_features() {
  # filesquid-metadata beyond its default features: every feature on (these tests
  # are skipped by a default build), then the wasm32 build checks of MD-S-009.
  local wasm=wasm32-unknown-unknown
  run_step "Clippy (filesquid-metadata, all features)" "$WORKSPACE" cargo_locked clippy -p filesquid-metadata --all-targets --all-features --locked -- -D warnings
  run_step "Tests (filesquid-metadata, all features)" "$WORKSPACE" cargo_locked test -p filesquid-metadata --all-features --locked
  wasm_target_ready || return
  run_step "wasm32 build (filesquid-metadata, default features off)" "$WORKSPACE" cargo_locked check -p filesquid-metadata --lib --no-default-features --target "$wasm" --locked
  run_step "wasm32 build (filesquid-metadata, ocfl feature)" "$WORKSPACE" cargo_locked check -p filesquid-metadata --lib --no-default-features --features ocfl --target "$wasm" --locked
}

skip_metadata_features() {
  # $1 = reason.
  skip_step "filesquid-metadata all-features tests and wasm32 builds" "$1"
}

run_prov_features() {
  # filesquid-prov beyond its default features: every feature on (the `ocfl` tests are
  # skipped by the default build), then the wasm32 build checks with default features and
  # with `ocfl` (PROV-S-001, PROV-S-006).
  run_step "Clippy (filesquid-prov, all features)" "$WORKSPACE" cargo_locked clippy -p filesquid-prov --all-targets --all-features --locked -- -D warnings
  run_step "Tests (filesquid-prov, all features)" "$WORKSPACE" cargo_locked test -p filesquid-prov --all-features --locked
  wasm_target_ready || return
  run_step "wasm32 build (filesquid-prov)" "$WORKSPACE" cargo_locked check -p filesquid-prov --lib --target wasm32-unknown-unknown --locked
  run_step "wasm32 build (filesquid-prov, ocfl feature)" "$WORKSPACE" cargo_locked check -p filesquid-prov --lib --features ocfl --target wasm32-unknown-unknown --locked
}

skip_prov_features() {
  # $1 = reason.
  skip_step "filesquid-prov all-features tests and wasm32 builds" "$1"
}

run_rust_full() {
  # Today's full Rust gate over the whole workspace.
  local crate
  run_step "Clippy" "$WORKSPACE" cargo_locked clippy --workspace --all-targets --locked -- -D warnings
  run_step "Workspace tests" "$WORKSPACE" cargo_locked test --workspace --locked
  for crate in $MODULE_CRATES; do
    run_step "Module crate alone: $crate" "$WORKSPACE" cargo_locked test -p "$crate" --locked
  done
  run_metadata_features
  run_prov_features
  run_step "Rustdoc" "$WORKSPACE" rustdoc_locked --workspace --no-deps --locked
  run_desktop_instrumented
}

run_module_crates_alone() {
  # $1 = space-separated crates in scope. Tests each listed module crate alone;
  # with a single crate in scope the scoped test run already was that run.
  local crates="$1" crate count
  count=$(echo $crates | wc -w | tr -d ' ')
  for crate in $MODULE_CRATES; do
    if ! contains_word "$crates" "$crate"; then
      skip_step "Module crate alone: $crate" "not in the change scope"
    elif [ "$count" -eq 1 ]; then
      skip_step "Module crate alone: $crate" "identical to the scoped test run above"
    else
      run_step "Module crate alone: $crate" "$WORKSPACE" cargo_locked test -p "$crate" --locked
    fi
  done
}

run_rust_scoped() {
  # $1 = comma-separated crates: the changed crates and their dependents.
  local crates package_flags="" crate
  crates=$(echo "$1" | tr ',' ' ')
  for crate in $crates; do package_flags="$package_flags -p $crate"; done
  echo "ℹ️  Rust scope: $crates"
  # shellcheck disable=SC2086 # one -p flag per crate
  run_step "Clippy (scoped)" "$WORKSPACE" cargo_locked clippy $package_flags --all-targets --locked -- -D warnings
  # shellcheck disable=SC2086
  run_step "Scoped tests" "$WORKSPACE" cargo_locked test $package_flags --locked
  run_module_crates_alone "$crates"
  if contains_word "$crates" "filesquid-metadata"; then
    run_metadata_features
  else
    skip_metadata_features "filesquid-metadata is not in the change scope"
  fi
  if contains_word "$crates" "filesquid-prov"; then
    run_prov_features
  else
    skip_prov_features "filesquid-prov is not in the change scope"
  fi
  # shellcheck disable=SC2086
  run_step "Rustdoc (scoped)" "$WORKSPACE" rustdoc_locked $package_flags --no-deps --locked
  if contains_word "$crates" "$DESKTOP_CRATE"; then
    run_desktop_instrumented
  else
    skip_desktop_instrumented "$DESKTOP_CRATE is not in the change scope"
  fi
}

skip_rust() {
  # $1 = reason. Prints every Rust step as skipped.
  local crate
  skip_step "Clippy" "$1"
  skip_step "Workspace tests" "$1"
  for crate in $MODULE_CRATES; do skip_step "Module crate alone: $crate" "$1"; done
  skip_metadata_features "$1"
  skip_prov_features "$1"
  skip_step "Rustdoc" "$1"
  skip_desktop_instrumented "$1"
}

run_npm_package() {
  # $1 = package directory. Lint, typecheck and unit tests (Playwright on opt-in).
  local pkg="$1"
  if [ ! -d "$pkg/node_modules" ]; then
    echo "❌ ERROR ($pkg): node_modules missing. Run 'npm ci' in $pkg first."
    FAILED=1
    return
  fi
  run_step "ESLint" "$pkg" npm run lint
  run_step "TypeScript" "$pkg" npm run check
  run_step "Vitest" "$pkg" npm test
  if [ "$FILESQUID_E2E" = "1" ]; then
    run_step "Playwright e2e" "$pkg" npm run test:e2e
  else
    echo "ℹ️  Playwright e2e skipped for $pkg (set FILESQUID_E2E=1 to run)."
  fi
}

run_npm_packages() {
  # $1 = space-separated packages in scope. Runs those, prints the rest as skipped.
  local pkg
  for pkg in $UI_PACKAGES; do
    if contains_word "$1" "$pkg"; then
      run_npm_package "$pkg"
    else
      skip_step "ESLint, TypeScript, Vitest and Playwright e2e in $pkg" "not in the change scope"
    fi
  done
}

note_unhooked_packages() {
  # Prints, once per run, the packages whose own suites the hook leaves out.
  local pkg
  for pkg in $UNHOOKED_PACKAGES; do
    skip_step "ESLint, TypeScript, Vitest and Playwright e2e in $pkg" "not run by the hook during the MVP (owner decision 2026-10-23); @filesquid/ui's distribution-table test checks its label table and token file"
  done
}

run_desktop_e2e() {
  # Packaged-window tests: build the test-instrumented binary, then drive it with WebdriverIO.
  local dir="$WORKSPACE/crates/desktop/tests/e2e"
  if [ ! -d "$dir/node_modules" ]; then
    echo "❌ ERROR ($dir): node_modules missing. Run 'npm ci' in $dir first."
    FAILED=1
    return
  fi
  # Built from $WORKSPACE, so a relative CARGO_TARGET_DIR is relative to it; wdio.conf.mjs
  # resolves it the same way and copies the binary to a run-specific directory before launch.
  run_step "Desktop test-instrumented build" "$WORKSPACE" cargo_locked build -p "$DESKTOP_CRATE" --features test-webdriver --locked
  run_step "Prettier (desktop e2e)" "$dir" npm run format:check
  if [ "$(uname -s)" = "Linux" ] && [ -z "$DISPLAY" ]; then
    run_step "Desktop packaged-window e2e" "$dir" xvfb-run -a npm run test:e2e
  else
    run_step "Desktop packaged-window e2e" "$dir" npm run test:e2e
  fi
}

run_desktop_e2e_if_requested() {
  # $1 = "yes" when the desktop crate is in scope.
  if [ "$FILESQUID_DESKTOP_E2E" != "1" ]; then
    echo "ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run)."
  elif [ "$1" != "yes" ]; then
    skip_step "Desktop packaged-window e2e" "$DESKTOP_CRATE is not in the change scope; use FULL=1 to force it"
  else
    run_desktop_e2e
  fi
}

run_full_scope() {
  run_rust_full
  run_npm_packages "$UI_PACKAGES"
  run_desktop_e2e_if_requested yes
}

run_docs_scope() {
  local reason="docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run"
  skip_rust "$reason"
  skip_step "ESLint, TypeScript, Vitest and Playwright e2e in $UI_PACKAGES" "$reason"
  skip_step "Desktop packaged-window e2e" "$reason"
}

run_partial_scope() {
  # $1 = scope line with rust: and/or npm: tokens.
  local crates packages desktop=no
  crates=$(scope_rust_crates "$1")
  packages=$(scope_npm_packages "$1" | tr ',' ' ')
  if [ -n "$crates" ]; then
    run_rust_scoped "$crates"
  else
    skip_rust "no Rust crate changed"
  fi
  contains_word "$(echo "$crates" | tr ',' ' ')" "$DESKTOP_CRATE" && desktop=yes
  run_npm_packages "$packages"
  run_desktop_e2e_if_requested "$desktop"
}

SCOPE=$(scope_or_detect "$@")
echo "ℹ️  Test scope: $SCOPE"
note_unhooked_packages
case "$SCOPE" in
  full) run_full_scope ;;
  docs) run_docs_scope ;;
  *) run_partial_scope "$SCOPE" ;;
esac

if [ "$FAILED" -ne 0 ]; then
  echo ""
  echo "🚨 VALIDATION FAILED (test scope: $SCOPE): fix the lint or test failures listed above before committing."
  exit 1
fi

echo "✅ Lint and test validation passed (test scope: $SCOPE)."
exit 0
