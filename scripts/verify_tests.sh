#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Lint and test validation: runs the Cargo and npm quality gates that stand in for hosted CI (pre-commit, before every push).
#
# This repository has no hosted CI; these are the gates that would otherwise
# run there. From the FileSquid Cargo workspace (apps/filesquid/):
#   1. cargo clippy --workspace --all-targets --locked -- -D warnings
#   2. cargo test --workspace --locked
#   3. cargo test -p <crate> --locked for each module crate alone (ADR 029)
#   4. cargo doc --workspace --no-deps --locked with warnings denied
# In each UI package (apps/filesquid/ui, apps/imagesquid):
#   5. npm run lint, npm run check, npm test
#   6. npm run test:e2e, only when FILESQUID_E2E=1 (builds the UI and runs
#      filesquid-server; slow)
#
# Environment:
#   FILESQUID_SKIP_TESTS=1  skip this script (work-in-progress commits only;
#                           run it before pushing)
#   FILESQUID_E2E=1         also run Playwright end-to-end tests
#
# Usage: ./scripts/verify_tests.sh   (from the repository root)

echo "Starting Lint and Test Validation..."

if [ "$FILESQUID_SKIP_TESTS" = "1" ]; then
  echo "⚠️  WARNING: FILESQUID_SKIP_TESTS=1, lint and tests skipped. Run ./scripts/verify_tests.sh before pushing."
  exit 0
fi

WORKSPACE="apps/filesquid"
MODULE_CRATES="filesquid-core filesquid-ocfl filesquid-metadata filesquid-c2pa filesquid-api"
UI_PACKAGES="apps/filesquid/ui apps/imagesquid"
FAILED=0

run_step() {
  # $1 = label, $2 = directory, remaining = command. Streams output so a
  # failure shows its cause; records failure instead of exiting.
  local label="$1" dir="$2"; shift 2
  echo "🔍 $label ($dir)..."
  if (cd "$dir" && "$@"); then
    echo "✅ $label passed."
  else
    echo "❌ $label failed."
    FAILED=1
  fi
}

run_step "Clippy" "$WORKSPACE" cargo clippy --workspace --all-targets --locked -- -D warnings
run_step "Workspace tests" "$WORKSPACE" cargo test --workspace --locked
for CRATE in $MODULE_CRATES; do
  run_step "Module crate alone: $CRATE" "$WORKSPACE" cargo test -p "$CRATE" --locked
done
run_step "Rustdoc" "$WORKSPACE" env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked

for PKG in $UI_PACKAGES; do
  if [ ! -d "$PKG/node_modules" ]; then
    echo "❌ ERROR ($PKG): node_modules missing. Run 'npm ci' in $PKG first."
    FAILED=1
    continue
  fi
  run_step "ESLint" "$PKG" npm run lint
  run_step "TypeScript" "$PKG" npm run check
  run_step "Vitest" "$PKG" npm test
  if [ "$FILESQUID_E2E" = "1" ]; then
    run_step "Playwright e2e" "$PKG" npm run test:e2e
  else
    echo "ℹ️  Playwright e2e skipped for $PKG (set FILESQUID_E2E=1 to run)."
  fi
done

if [ "$FAILED" -ne 0 ]; then
  echo ""
  echo "🚨 VALIDATION FAILED: fix the lint or test failures listed above before committing."
  exit 1
fi

echo "✅ Lint and test validation passed."
exit 0
