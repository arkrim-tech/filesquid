# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Hook scope: runs scripts/changed_scope.sh for the pre-commit gates and validates its answer (sourced, not run).
#
#   hook_scope          print the scope; the helper's reason goes to stderr
#   hook_scope --quiet  print the scope only
#
# The fixture-only overrides CHANGED_SCOPE_FILES, CHANGED_SCOPE_METADATA and
# CHANGED_SCOPE_BASE are removed, so the gates always diff the real working
# tree against origin/main. Any failure, or an answer that is not a known
# scope, becomes `full`: the gates never run less because detection broke.
#
# Usage: . scripts/lib/hook_scope.sh   (from the repository root)

# Prints the validated scope for the current working tree.
hook_scope() {
  local scope
  if [ "${1:-}" = "--quiet" ]; then
    scope=$(env -u CHANGED_SCOPE_FILES -u CHANGED_SCOPE_METADATA -u CHANGED_SCOPE_BASE ./scripts/changed_scope.sh 2>/dev/null) || scope=full
  else
    scope=$(env -u CHANGED_SCOPE_FILES -u CHANGED_SCOPE_METADATA -u CHANGED_SCOPE_BASE ./scripts/changed_scope.sh) || scope=full
  fi
  case "$scope" in
    docs | full | rust:* | npm:*) ;;
    *) scope=full ;;
  esac
  printf '%s\n' "$scope"
}

# Prints the comma-separated crate list of scope $1 ("" when it has none). Pure.
scope_rust_crates() {
  local token
  for token in $1; do
    case "$token" in rust:*) printf '%s\n' "${token#rust:}" ;; esac
  done
}

# Prints the comma-separated npm package list of scope $1 ("" when it has none). Pure.
scope_npm_packages() {
  local token
  for token in $1; do
    case "$token" in npm:*) printf '%s\n' "${token#npm:}" ;; esac
  done
}
