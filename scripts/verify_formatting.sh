#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Format validation: checks Rust, TypeScript and Elixir formatting under apps/ without changing files (pre-commit).
#
# Checks formatting for every stack present under apps/ without changing files:
#   - Rust: `cargo fmt --all --check` in each product-local Cargo workspace
#     (a directory under apps/ that holds a Cargo.toml), under the
#     shared-target lock (scripts/lib/cargo_shared.sh). Skipped, with the reason
#     printed, when the change scope (scripts/changed_scope.sh) is `docs` or
#     npm-only: such a change touches no .rs file, so the result cannot change.
#   - TypeScript: `prettier --check .` in each npm package (a directory under
#     apps/ or apps/*/ that holds a package.json), on every commit, because
#     Prettier also checks the package's Markdown. Requires `npm ci` to have
#     been run in that package; the check fails with that instruction otherwise.
#   - Elixir: `mix format --check-formatted` in each app with a mix.exs.
#
# Usage: ./scripts/verify_formatting.sh   (from the repository root)

echo "Starting Format Validation..."

. ./scripts/lib/cargo_shared.sh
. ./scripts/lib/hook_scope.sh

APPS_DIR="./apps"
FAILED=0
SCOPE=$(hook_scope --quiet)

check_rust_workspace() {
  # $1 = workspace directory holding Cargo.toml
  local dir="$1" output
  case "$SCOPE" in
    full | rust:*) ;;
    *)
      echo "⏭️  Skipped: Rust formatting in $dir (no Rust file changed; test scope: $SCOPE)"
      return ;;
  esac
  echo "🔍 Validating Rust formatting in $dir..."
  if output=$(cd "$dir" && cargo_locked fmt --all --check 2>&1); then
    echo "✅ $dir is properly formatted."
  else
    printf '%s\n' "$output" | head -n 40
    echo "❌ ERROR ($dir): Rust format violation. Run 'cargo fmt --all' in $dir."
    FAILED=1
  fi
}

check_npm_package() {
  # $1 = package directory holding package.json
  local dir="$1"
  echo "🔍 Validating Prettier formatting in $dir..."
  if [ ! -d "$dir/node_modules" ]; then
    echo "❌ ERROR ($dir): node_modules missing. Run 'npm ci' in $dir first."
    FAILED=1
    return
  fi
  if (cd "$dir" && npx prettier --check . >/dev/null 2>&1); then
    echo "✅ $dir is properly formatted."
  else
    echo "❌ ERROR ($dir): Prettier format violation. Run 'npx prettier --write .' in $dir."
    FAILED=1
  fi
}

check_elixir_app() {
  # $1 = app directory holding mix.exs
  local dir="$1"
  echo "🔍 Validating Elixir formatting in $dir..."
  if (cd "$dir" && mix format --check-formatted >/dev/null 2>&1); then
    echo "✅ $dir is properly formatted."
  else
    echo "❌ ERROR ($dir): Elixir format violation. Run 'mix format' in $dir."
    FAILED=1
  fi
}

for APP_PATH in "$APPS_DIR"/*; do
  [ -d "$APP_PATH" ] || continue
  [ -f "$APP_PATH/Cargo.toml" ] && check_rust_workspace "$APP_PATH"
  [ -f "$APP_PATH/package.json" ] && check_npm_package "$APP_PATH"
  [ -f "$APP_PATH/mix.exs" ] && check_elixir_app "$APP_PATH"
  for SUB_PATH in "$APP_PATH"/*; do
    [ -d "$SUB_PATH" ] && [ -f "$SUB_PATH/package.json" ] && check_npm_package "$SUB_PATH"
  done
done

if [ "$FAILED" -ne 0 ]; then
  echo ""
  echo "🚨 VALIDATION FAILED: Test-Driven Development formatting rule violated."
  echo "Format each listed workspace before committing."
  exit 1
fi

echo "✅ Format validation passed."
exit 0
