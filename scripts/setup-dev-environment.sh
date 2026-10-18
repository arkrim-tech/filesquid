#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Development environment setup: installs the pinned toolchain and dependencies and pre-warms the Cargo target, safely on every new session.
#
# Idempotent and bounded: every step can run again without harm, and every
# install, download or build command (rustup, cargo, npm, pip) runs under a
# time limit when `timeout` (or `gtimeout`) exists; quick local checks such as
# `pip show` and `python3 -m venv` are not wrapped. Without either program the steps run without a time limit after a
# warning (macOS without coreutils, for example); the lock wait stays bounded. Every step logs to stdout with an RFC 3339 UTC timestamp. A failed
# required step stops the script with a non-zero exit naming the step; a
# failed optional step prints a warning and the script continues.
#
# Steps:
#   1. required  Rust toolchain pinned in apps/filesquid/rust-toolchain.toml
#                (rustup 1.28 or newer), its rustfmt and clippy components
#                (added explicitly), and the wasm32-unknown-unknown target the
#                ocfl and c2pa tests build
#   2. required  cargo fetch --locked in apps/filesquid
#   3. required  npm ci in apps/filesquid/ui, then apps/imagesquid (which links it)
#   4. optional  community OCFL validator ocfl-py==2.1.0 with pip, when python3
#                exists (into the user site, or a private virtualenv when the
#                system Python is externally managed); rocfl is not installed
#   5. optional  pre-warm the Cargo target: cargo build --workspace --tests --locked,
#                under the same shared-target lock and Cargo environment as the
#                pre-commit hook (scripts/lib/cargo_shared.sh)
#
# Environment:
#   CARGO_TARGET_DIR               honoured as in the hook (Cargo's default otherwise)
#   CARGO_INCREMENTAL, CARGO_PROFILE_DEV_DEBUG   default 0, as in the hook
#   FILESQUID_SETUP_STEP_TIMEOUT   seconds per install step (default 1800)
#   FILESQUID_SETUP_BUILD_TIMEOUT  seconds for the pre-warm build (default 3600)
#   FILESQUID_CARGO_LOCK_WAIT      seconds to wait for the shared-target lock (default 1800)
#
# Usage: bash scripts/setup-dev-environment.sh   (from the repository root)

OCFL_PY_VERSION="2.1.0"
WORKSPACE="apps/filesquid"
UI_PACKAGES="apps/filesquid/ui apps/imagesquid"

cd "$(dirname "$0")/.." || { echo "❌ cannot enter the repository root"; exit 1; }
. ./scripts/lib/cargo_shared.sh

# Prints a log line with an RFC 3339 UTC timestamp.
log() {
  echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*"
}

# Prints a positive integer from $1, or the default $2. Pure.
seconds_or_default() {
  case "$1" in
    '' | *[!0-9]* | 0) echo "$2" ;;
    *) echo "$1" ;;
  esac
}

STEP_TIMEOUT=$(seconds_or_default "${FILESQUID_SETUP_STEP_TIMEOUT:-}" 1800)
BUILD_TIMEOUT=$(seconds_or_default "${FILESQUID_SETUP_BUILD_TIMEOUT:-}" 3600)

# Prints the `timeout <seconds>` prefix for $1 seconds, or nothing (with a
# warning on stderr) when no timeout program exists.
timeout_prefix() {
  if command -v timeout >/dev/null 2>&1; then
    echo "timeout $1"
  elif command -v gtimeout >/dev/null 2>&1; then
    echo "gtimeout $1"
  else
    echo "⚠️  WARNING: no timeout program found; this step runs without a time limit." >&2
  fi
}

# Runs a required step: $1 = name, rest = command. Exits non-zero on failure.
required_step() {
  local name="$1" started=$SECONDS; shift
  log "▶ $name"
  if "$@"; then
    log "✅ $name ($((SECONDS - started))s)"
  else
    log "❌ setup failed at required step: $name ($((SECONDS - started))s)"
    exit 1
  fi
}

# Runs an optional step: $1 = name, rest = command. Warns and continues on failure.
optional_step() {
  local name="$1" started=$SECONDS; shift
  log "▶ $name (optional)"
  if "$@"; then
    log "✅ $name ($((SECONDS - started))s)"
  else
    log "⚠️  optional step failed, continuing: $name ($((SECONDS - started))s)"
  fi
}

# Succeeds when the installed rustup is older than 1.28, which cannot install
# a toolchain from rust-toolchain.toml without arguments.
rustup_is_older_than_1_28() {
  local version major minor
  version=$(rustup --version 2>/dev/null | sed -n 's/^rustup \([0-9]*\)\.\([0-9]*\).*/\1 \2/p')
  [ -n "$version" ] || return 1
  read -r major minor <<<"$version"
  [ "$major" -lt 1 ] || { [ "$major" -eq 1 ] && [ "$minor" -lt 28 ]; }
}

# Logs why `rustup toolchain install` failed: an old rustup when that is the
# detected cause, otherwise rustup's own output above (network, timeout, ...).
explain_toolchain_failure() {
  if rustup_is_older_than_1_28; then
    log "rustup $(rustup --version 2>/dev/null | awk '{print $2}') is older than 1.28 and cannot read rust-toolchain.toml here; run 'rustup self update'"
  else
    log "rustup toolchain install failed; see rustup's output above for the cause (for example network or time limit)"
  fi
}

# Installs the toolchain named by rust-toolchain.toml, its rustfmt and clippy
# components (added explicitly, since some rustup versions skip them), and the
# wasm32 target. Any failure fails the step.
install_rust_toolchain() {
  command -v rustup >/dev/null 2>&1 || { log "rustup not found; install it from https://rustup.rs/"; return 1; }
  # shellcheck disable=SC2046 # the prefix is one or two words
  (cd "$WORKSPACE" && $(timeout_prefix "$STEP_TIMEOUT") rustup toolchain install) || { explain_toolchain_failure; return 1; }
  # shellcheck disable=SC2046
  (cd "$WORKSPACE" && $(timeout_prefix "$STEP_TIMEOUT") rustup component add rustfmt clippy \
    && $(timeout_prefix "$STEP_TIMEOUT") rustup target add wasm32-unknown-unknown)
}

# Downloads every locked Cargo dependency.
fetch_cargo_dependencies() {
  # shellcheck disable=SC2046
  (cd "$WORKSPACE" && $(timeout_prefix "$STEP_TIMEOUT") cargo fetch --locked)
}

# Runs npm ci in package directory $1.
install_npm_package() {
  command -v npm >/dev/null 2>&1 || { log "npm not found; install Node.js (see README prerequisites)"; return 1; }
  # shellcheck disable=SC2046
  (cd "$1" && $(timeout_prefix "$STEP_TIMEOUT") npm ci --no-audit --no-fund)
}

# Prints the ocfl-py version importable by python3, or nothing.
installed_ocfl_py_version() {
  python3 -m pip show ocfl-py 2>/dev/null | sed -n 's/^Version: //p'
}

# Installs ocfl-py at the pinned version into a private virtualenv and links
# its validator into ~/.local/bin (used when the system Python refuses --user).
install_ocfl_py_venv() {
  local venv="${XDG_DATA_HOME:-$HOME/.local/share}/filesquid/ocfl-py-$OCFL_PY_VERSION"
  # shellcheck disable=SC2046
  [ -x "$venv/bin/ocfl-validate.py" ] || { python3 -m venv "$venv" \
    && $(timeout_prefix "$STEP_TIMEOUT") "$venv/bin/python" -m pip install --quiet "ocfl-py==$OCFL_PY_VERSION"; } || return 1
  mkdir -p "$HOME/.local/bin" && ln -sf "$venv/bin/ocfl-validate.py" "$HOME/.local/bin/ocfl-validate.py"
}

# Installs the community OCFL validator ocfl-py at the pinned version.
install_ocfl_py() {
  command -v python3 >/dev/null 2>&1 || { log "python3 not found; ocfl-py skipped (the OCFL tests then skip the community check)"; return 0; }
  if [ "$(installed_ocfl_py_version)" = "$OCFL_PY_VERSION" ]; then
    log "ocfl-py $OCFL_PY_VERSION already installed"
    return 0
  fi
  # shellcheck disable=SC2046
  $(timeout_prefix "$STEP_TIMEOUT") python3 -m pip install --user --quiet "ocfl-py==$OCFL_PY_VERSION" || install_ocfl_py_venv || return 1
  command -v ocfl-validate.py >/dev/null 2>&1 || log "⚠️  ocfl-validate.py is installed but not on PATH; add the pip user bin directory or ~/.local/bin to PATH"
}

# Builds the workspace and its tests into the shared target, under the lock.
prewarm_cargo_target() {
  log "CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-<Cargo default>} CARGO_INCREMENTAL=$CARGO_INCREMENTAL CARGO_PROFILE_DEV_DEBUG=$CARGO_PROFILE_DEV_DEBUG"
  # shellcheck disable=SC2046
  (cd "$WORKSPACE" && with_cargo_lock $(timeout_prefix "$BUILD_TIMEOUT") cargo build --workspace --tests --locked)
}

main() {
  local pkg started=$SECONDS
  log "FileSquid development environment setup"
  required_step "Rust toolchain (rust-toolchain.toml) and wasm32 target" install_rust_toolchain
  required_step "cargo fetch --locked" fetch_cargo_dependencies
  for pkg in $UI_PACKAGES; do
    required_step "npm ci in $pkg" install_npm_package "$pkg"
  done
  optional_step "ocfl-py $OCFL_PY_VERSION (community OCFL validator)" install_ocfl_py
  optional_step "Pre-warm Cargo target (cargo build --workspace --tests --locked)" prewarm_cargo_target
  log "✅ Setup complete ($((SECONDS - started))s)"
}

main
