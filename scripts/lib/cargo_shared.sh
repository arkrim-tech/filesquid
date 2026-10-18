# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Shared Cargo target: the Cargo environment and the lock that serialise cargo runs sharing one CARGO_TARGET_DIR (sourced, not run).
#
# Several agents and worktrees may share one Cargo target directory. Concurrent
# cargo runs on it corrupt each other's builds ("could not find `X`", "can't
# find crate"), so every cargo run in the pre-commit gates and the setup script
# takes an exclusive `flock` on a lock file next to the target directory.
#
#   with_cargo_lock <program...>   run an executable (not a shell function) under the lock
#   cargo_locked <cargo args...>   run `cargo <args>` under the lock
#
# Sourcing also exports CARGO_INCREMENTAL=0 and CARGO_PROFILE_DEV_DEBUG=0
# unless they are already set, so the hook and scripts/setup-dev-environment.sh
# build identical, smaller artifacts and the setup pre-warm is reused. Set
# either variable yourself to override it.
#
# Call both from the Cargo workspace directory: a relative CARGO_TARGET_DIR,
# and Cargo's default `target/`, resolve against the current directory exactly
# as Cargo resolves them.
#
# The wait is bounded (AGENTS.md section 22): FILESQUID_CARGO_LOCK_WAIT seconds,
# default 1800. On timeout the command is not run, a message names the lock
# file, and the exit status is 75. Without `flock` (for example on macOS) the
# command runs unlocked after a warning, so the gates stay platform-agnostic.
#
# Usage: . scripts/lib/cargo_shared.sh

CARGO_LOCK_TIMEOUT_EXIT=75

# Prints the lock wait in seconds: FILESQUID_CARGO_LOCK_WAIT when it is a
# positive integer, otherwise the default of 1800.
cargo_lock_wait_seconds() {
  case "${FILESQUID_CARGO_LOCK_WAIT:-}" in
    '' | *[!0-9]* | 0) echo 1800 ;;
    *) echo "$FILESQUID_CARGO_LOCK_WAIT" ;;
  esac
}

# Prints the lock file path for target directory $1 resolved against
# directory $2: "<target>.lock", a sibling of the target directory. Pure.
cargo_lock_path() {
  local target="${1%/}" cwd="$2"
  case "$target" in
    /* | [A-Za-z]:*) ;;
    *) target="$cwd/$target" ;;
  esac
  printf '%s.lock\n' "$target"
}

# Prints a one-time warning when flock is missing. Called once on source.
cargo_lock_warn_if_unavailable() {
  if ! command -v flock >/dev/null 2>&1; then
    echo "⚠️  WARNING: flock not found; cargo runs without the shared-target lock. Do not run two cargo commands on one CARGO_TARGET_DIR at once (install util-linux flock to enable the lock)." >&2
  fi
}

# Runs "$@" under an exclusive lock on the current Cargo target directory.
# Returns the command's status, or 75 when the bounded wait times out.
with_cargo_lock() {
  if ! command -v flock >/dev/null 2>&1; then
    "$@"
    return
  fi
  local lock wait status
  lock=$(cargo_lock_path "${CARGO_TARGET_DIR:-target}" "$PWD")
  wait=$(cargo_lock_wait_seconds)
  mkdir -p "$(dirname "$lock")" || return 1
  if ! flock -n "$lock" true; then
    echo "⏳ Waiting up to ${wait}s for the Cargo lock $lock (another cargo run on this target directory holds it)..." >&2
  fi
  flock -w "$wait" -E "$CARGO_LOCK_TIMEOUT_EXIT" "$lock" "$@"
  status=$?
  if [ "$status" -eq "$CARGO_LOCK_TIMEOUT_EXIT" ]; then
    echo "❌ Timed out after ${wait}s waiting for the Cargo lock $lock; the command was not run. Another cargo run on CARGO_TARGET_DIR still holds it: wait for it to finish and rerun, or raise FILESQUID_CARGO_LOCK_WAIT." >&2
  fi
  return "$status"
}

# Runs `cargo "$@"` under the shared-target lock.
cargo_locked() {
  with_cargo_lock cargo "$@"
}

# Exports the Cargo settings shared by the gates and the setup script,
# keeping any value the caller already set.
export_shared_cargo_env() {
  export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
  export CARGO_PROFILE_DEV_DEBUG="${CARGO_PROFILE_DEV_DEBUG:-0}"
}

export_shared_cargo_env
cargo_lock_warn_if_unavailable
