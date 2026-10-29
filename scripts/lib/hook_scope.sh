# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Hook scope: runs scripts/changed_scope.sh for the pre-commit gates and validates its answer (sourced, not run).
#
#   hook_scope [--quiet]          detect and print the validated scope; the
#                                 helper's reason goes to stderr unless --quiet
#   scope_or_detect [--scope S]   with no arguments, detect the scope; with
#                                 exactly `--scope S` and a valid S, print S (how
#                                 scripts/pre-commit.sh hands its one detection
#                                 to the gates); with anything else (an invalid
#                                 S, `--scope=S`, a misspelling, extra
#                                 arguments) print a message and `full`
#   is_valid_scope S              succeed when S follows the scope grammar
#   normalise_scope S             map each npm package in valid scope S to the
#                                 package whose suite the hook runs for it
#                                 (HOOK_NPM_PACKAGES, HOOK_GUARDED_PACKAGES);
#                                 print `full` when S names an unknown package
#
# The fixture-only overrides CHANGED_SCOPE_FILES, CHANGED_SCOPE_METADATA and
# CHANGED_SCOPE_BASE are removed, so the gates always diff the real working
# tree against origin/main. Any failure, and any answer outside the grammar
# (docs | full | rust:<crates> | npm:<packages> | rust:<crates> npm:<packages>,
# with non-empty comma-separated lists and no other tokens or lines), becomes
# `full`: the gates never run less because detection broke. A valid scope that
# names a guarded package (apps/imagesquid, whose own suite the hook does not
# run during the MVP; owner decision 2026-10-23) is rewritten to the package
# whose suite guards it, so `--scope npm:apps/imagesquid` cannot skip the npm
# checks; a scope naming any other package becomes `full`.
#
# Usage: . scripts/lib/hook_scope.sh   (from the repository root)

SCOPE_RUST_TOKEN='rust:[a-z0-9_-]+(,[a-z0-9_-]+)*'
SCOPE_NPM_TOKEN='npm:[A-Za-z0-9_./-]+(,[A-Za-z0-9_./-]+)*'
SCOPE_GRAMMAR="^(docs|full|${SCOPE_RUST_TOKEN}|${SCOPE_NPM_TOKEN}|${SCOPE_RUST_TOKEN} ${SCOPE_NPM_TOKEN})\$"
# The npm packages whose suites the hook runs, and each guarded package as
# <directory>=<package whose suite guards it>. scripts/changed_scope.sh keeps
# the same two lists (NPM_PACKAGES, GUARDED_PACKAGES); scripts/verify_changed_scope.sh
# fails when they differ.
HOOK_NPM_PACKAGES="apps/filesquid/ui"
HOOK_GUARDED_PACKAGES="apps/imagesquid=apps/filesquid/ui"

# Succeeds when $1 is exactly one valid scope line. Pure.
is_valid_scope() {
  case "$1" in *$'\n'*) return 1 ;; esac
  [[ "$1" =~ $SCOPE_GRAMMAR ]]
}

# Prints the package whose suite the hook runs for package directory $1: the
# package itself or its guard. Prints nothing for an unknown package. Pure.
hook_suite_for_package() {
  local pkg pair
  for pkg in $HOOK_NPM_PACKAGES; do
    [ "$1" = "$pkg" ] && { echo "$pkg"; return; }
  done
  for pair in $HOOK_GUARDED_PACKAGES; do
    [ "$1" = "${pair%%=*}" ] && { echo "${pair#*=}"; return; }
  done
}

# Prints valid scope $1 with its npm packages mapped to their suites (sorted,
# without duplicates), or `full` when a package is unknown. Pure.
normalise_scope() {
  local token pkg suite suites="" out=""
  for token in $1; do
    case "$token" in
      npm:*)
        for pkg in $(echo "${token#npm:}" | tr ',' ' '); do
          suite=$(hook_suite_for_package "$pkg")
          if [ -z "$suite" ]; then
            echo "changed_scope: full (unknown npm package '$pkg' in scope '$1')" >&2
            echo full
            return
          fi
          suites="$suites $suite"
        done
        token="npm:$(printf '%s\n' $suites | sort -u | paste -sd, -)" ;;
    esac
    out="${out:+$out }$token"
  done
  printf '%s\n' "$out"
}

# Prints the validated scope for the current working tree.
hook_scope() {
  local scope
  if [ "${1:-}" = "--quiet" ]; then
    scope=$(env -u CHANGED_SCOPE_FILES -u CHANGED_SCOPE_METADATA -u CHANGED_SCOPE_BASE ./scripts/changed_scope.sh 2>/dev/null) || scope=full
  else
    scope=$(env -u CHANGED_SCOPE_FILES -u CHANGED_SCOPE_METADATA -u CHANGED_SCOPE_BASE ./scripts/changed_scope.sh) || scope=full
  fi
  is_valid_scope "$scope" || scope=full
  normalise_scope "$scope"
}

# Prints the detected scope when called with no arguments, the given scope
# for exactly `--scope <valid scope>`, and `full` (with a message) otherwise.
scope_or_detect() {
  if [ "$#" -eq 0 ]; then
    hook_scope
  elif [ "$#" -eq 2 ] && [ "$1" = "--scope" ] && is_valid_scope "$2"; then
    normalise_scope "$2"
  else
    echo "changed_scope: full (unrecognised arguments '$*'; expected none or --scope <scope>)" >&2
    echo full
  fi
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
