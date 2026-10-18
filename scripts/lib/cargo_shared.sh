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
#   with_cargo_lock <command...>   run a command (program or shell function) under the lock
#   cargo_locked <cargo args...>   run `cargo <args>` under the lock
#
# The lock covers only cargo runs started through these functions (the
# pre-commit gates and the setup script). A cargo command run by hand or by an
# IDE does not take it.
#
# The target directory is CARGO_TARGET_DIR, else CARGO_BUILD_TARGET_DIR, else
# Cargo's default `target`, resolved to its physical path (symlinks followed)
# so every spelling of one target directory maps to the same lock file. A `build.target-dir` set in a Cargo config file is
# not read; this repository sets none, so use one of the variables instead.
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
# file, and the exit status is 75. The message is printed only on a real
# timeout, so a command that itself exits 75 is never reported as one. The
# lock is held on a file descriptor of a subshell that the command does not
# inherit, so a daemon the command leaves behind cannot keep the lock. Without
# `flock` (for example on macOS) the command runs unlocked after a warning, so
# the gates stay platform-agnostic.
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

# Prints the target directory Cargo will use: CARGO_TARGET_DIR, else
# CARGO_BUILD_TARGET_DIR, else `target`. Pure over the environment.
cargo_target_dir() {
  printf '%s\n' "${CARGO_TARGET_DIR:-${CARGO_BUILD_TARGET_DIR:-target}}"
}

# Creates target directory $1 if it does not exist yet (Cargo would create it
# on its first build anyway), so its physical path can be resolved.
create_target_dir() {
  mkdir -p -- "$1"
}

# Prints the physical path of existing directory $1 (relative to the current
# directory), so a symlink and its real path share one lock. CDPATH is
# cleared so a relative name never resolves elsewhere or prints extra output.
physical_dir_path() {
  (CDPATH='' cd -- "$1" && pwd -P)
}

# Takes the lock on file descriptor 9, waiting at most $1 seconds. Prints a
# waiting notice, and on timeout a message naming lock file $2; fails then.
acquire_cargo_lock_fd9() {
  flock -n 9 && return 0
  echo "⏳ Waiting up to ${1}s for the Cargo lock $2 (another cargo run on this target directory holds it)..." >&2
  flock -w "$1" 9 && return 0
  echo "❌ Timed out after ${1}s waiting for the Cargo lock $2; the command was not run. Another cargo run on the same target directory still holds it: wait for it to finish and rerun, or raise FILESQUID_CARGO_LOCK_WAIT." >&2
  return 1
}

# Runs "$@" under an exclusive lock on the current Cargo target directory.
# Returns the command's status, or 75 when the bounded wait times out. The
# lock is released when the subshell holding descriptor 9 exits, on every path.
with_cargo_lock() {
  if ! command -v flock >/dev/null 2>&1; then
    "$@"
    return
  fi
  local lock wait target
  target=$(cargo_target_dir)
  create_target_dir "$target" || return 1
  target=$(physical_dir_path "$target") || return 1
  lock=$(cargo_lock_path "$target" "$PWD")
  wait=$(cargo_lock_wait_seconds)
  (
    acquire_cargo_lock_fd9 "$wait" "$lock" || exit "$CARGO_LOCK_TIMEOUT_EXIT"
    "$@" 9>&-
  ) 9>>"$lock"
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

# A CDPATH inherited from the environment would make `cd <relative dir>` in the
# gates resolve elsewhere and print the path; the gates never rely on it.
unset CDPATH
export_shared_cargo_env
cargo_lock_warn_if_unavailable
