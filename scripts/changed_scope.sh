#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Change scope: prints which lint and test scope the working tree needs, from the diff against the merge-base with origin/main.
#
# The changed files are everything that differs between the merge-base with
# origin/main and the working tree: committed on the branch, staged, unstaged
# and untracked (not ignored) files together. Renames count as a delete plus
# an add, so both paths are classified.
#
# Prints exactly one line on stdout; the reason goes to stderr:
#   docs                    only documentation changed outside the crates and
#                           npm packages: Markdown, resources/, .agents/, and
#                           at the repository root .claude/ and docs/; plus the
#                           text checkers that pre-commit steps 0 to 9 run on
#                           every commit whatever the scope (the allowlist in
#                           DOCS_CHECKER_SCRIPTS, and scripts/docs/). Every file
#                           inside a crate or package, Markdown and resources/
#                           included, belongs to it, because its tests read
#                           files there (for example crates/api/resources/srs.md
#                           and crates/c2pa/README.md)
#   rust:<crates>           the changed crates under apps/filesquid/crates/*, plus
#                           every workspace crate that depends on them (all
#                           dependency kinds, transitively; from
#                           `cargo metadata --no-deps --format-version 1`)
#   npm:<packages>          the changed npm packages the hook tests (only
#                           apps/filesquid/ui during the MVP). A package's own
#                           top-level package.json and package-lock.json count
#                           as the package: no Rust build reads them. A change
#                           confined to apps/imagesquid, its top-level
#                           manifests included, gives npm:apps/filesquid/ui:
#                           the hook runs no ImageSquid suite (owner decision
#                           2026-10-23), and @filesquid/ui's distribution-table
#                           test reads ImageSquid's label table and token file
#   rust:<crates> npm:<packages>   both
#   full                    the workspace Cargo.toml or Cargo.lock, a
#                           rust-toolchain file, any other package.json or
#                           package-lock.json, a script that decides or runs the
#                           scope (scripts/pre-commit.sh, changed_scope.sh,
#                           verify_changed_scope.sh, verify_tests.sh,
#                           verify_formatting.sh, setup-dev-environment.sh,
#                           scripts/lib/) or any other script not in the
#                           allowlist, a generated contract file (openapi.json,
#                           schema.d.ts; the api contract test reads the
#                           latter) or any file outside the categories above
#                           changed; or FULL=1 is set; or detection failed
#                           (unknown base ref, git or cargo metadata error,
#                           unknown crate directory)
#
# It never falls back to a smaller scope: every failure prints `full`.
#
# Environment:
#   FULL=1                         print full
#   CHANGED_SCOPE_BASE=<ref>       compare with this ref instead of origin/main
#   CHANGED_SCOPE_FILES=<file>     read the changed paths from this file, one
#                                  per line, instead of git (fixture tests only)
#   CHANGED_SCOPE_METADATA=<file>  read the cargo metadata JSON from this file
#                                  instead of running cargo (fixture tests only)
# The hook strips the CHANGED_SCOPE_* overrides (scripts/lib/hook_scope.sh).
#
# Usage: ./scripts/changed_scope.sh   (from the repository root)

# An inherited CDPATH would make `cd apps/filesquid` resolve elsewhere.
unset CDPATH
WORKSPACE_DIR="apps/filesquid"
CRATES_DIR="$WORKSPACE_DIR/crates"
# The npm packages whose suites the hook runs (scripts/verify_tests.sh).
NPM_PACKAGES="apps/filesquid/ui"
# Package directories the hook does not test itself, each as <directory>=<package
# in NPM_PACKAGES whose suite guards it>. ImageSquid's own lint, typecheck, Vitest
# and Playwright suite left the hook for the MVP (owner decision 2026-10-23,
# .agents/rules/local-verification.md); @filesquid/ui's distribution-table test
# reads its label table and token file, so a change there runs that suite.
GUARDED_PACKAGES="apps/imagesquid=apps/filesquid/ui"
# Text checkers that pre-commit steps 0 to 9 run on every commit, whatever the
# scope, so the commit that changes one also runs it. An explicit list: a
# script not named here (a new one included) forces full.
DOCS_CHECKER_SCRIPTS="verify_branch.sh verify_isolated_ports.sh verify_doc_sync.sh
  verify_srs_traceability.sh verify_relative_links.sh verify_workspace_boundaries.sh
  verify_centralized_logging.sh verify_docs.sh verify_task_cleanup.sh"

# Reads `cargo metadata --no-deps` JSON on stdin and crate directories
# (relative to the workspace root, e.g. crates/api) as arguments. Prints the
# comma-separated names of those crates and every workspace crate that
# depends on them. Exits non-zero on an unknown directory.
REVERSE_DEPENDENCIES_PY='
import json, os, sys
meta = json.load(sys.stdin)
root = meta["workspace_root"]
by_dir = {}
for pkg in meta["packages"]:
    rel = os.path.relpath(os.path.dirname(pkg["manifest_path"]), root)
    by_dir[rel.replace(os.sep, "/")] = pkg["name"]
members = set(by_dir.values())
dependents = {name: set() for name in members}
for pkg in meta["packages"]:
    for dep in pkg["dependencies"]:
        if dep.get("path") and dep["name"] in members:
            dependents[dep["name"]].add(pkg["name"])
unknown = [d for d in sys.argv[1:] if d not in by_dir]
if unknown:
    sys.exit("not a workspace crate directory: " + ", ".join(unknown))
scope, queue = set(), [by_dir[d] for d in sys.argv[1:]]
while queue:  # bounded: a crate is expanded at most once
    name = queue.pop()
    if name not in scope:
        scope.add(name)
        queue.extend(dependents[name] - scope)
print(",".join(sorted(scope)))
'

# Prints the scope line on stdout and the reason on stderr.
report() {
  echo "changed_scope: $1 ($2)" >&2
  printf '%s\n' "$1"
}

# Classifies one path under scripts/. Pure. Prints docs for scripts/docs/ and
# for a top-level script in DOCS_CHECKER_SCRIPTS, full for anything else (the
# scripts that decide or run the scope, scripts/lib/, and unknown scripts).
classify_script() {
  local path="$1" name
  case "$path" in
    scripts/docs/*) echo docs; return ;;
  esac
  for name in $DOCS_CHECKER_SCRIPTS; do
    [ "$path" = "scripts/$name" ] && { echo docs; return; }
  done
  echo full
}

# Prints every package directory the scope knows: the tested packages in
# NPM_PACKAGES, then the guarded directories in GUARDED_PACKAGES. Pure.
package_dirs() {
  local pair
  # shellcheck disable=SC2086 # one word per package directory
  printf '%s\n' $NPM_PACKAGES
  for pair in $GUARDED_PACKAGES; do
    printf '%s\n' "${pair%%=*}"
  done
}

# Prints the package in NPM_PACKAGES whose suite covers package directory $1:
# the package itself, or its guard in GUARDED_PACKAGES. Prints nothing for any
# other directory. Pure.
suite_for_package() {
  local dir="$1" pkg pair
  for pkg in $NPM_PACKAGES; do
    [ "$dir" = "$pkg" ] && { echo "$pkg"; return; }
  done
  for pair in $GUARDED_PACKAGES; do
    [ "$dir" = "${pair%%=*}" ] && { echo "${pair#*=}"; return; }
  done
}

# Classifies one package.json or package-lock.json. Pure. Prints
# npm:<package> when it is the top-level manifest of a tested or guarded
# package directory (the package whose suite covers it), full for any other one.
classify_npm_manifest() {
  local suite
  suite=$(suite_for_package "${1%/*}")
  if [ -n "$suite" ]; then
    echo "npm:$suite"
  else
    echo full
  fi
}

# Classifies one changed path. Pure. The case order matters: full triggers
# (contract and toolchain files; npm manifests and scripts/, which are full
# unless their own rule above matches; the Cargo manifest and lockfile), then
# crates, then npm packages, then documentation. Prints one of: docs, full,
# crate:<dir relative to the workspace root>, npm:<package in NPM_PACKAGES>.
classify_path() {
  local path="$1" rest dir
  case "${path##*/}" in
    openapi.json | schema.d.ts | rust-toolchain | rust-toolchain.toml) echo full; return ;;
    package.json | package-lock.json) classify_npm_manifest "$path"; return ;;
  esac
  case "$path" in
    scripts/*) classify_script "$path"; return ;;
    "$WORKSPACE_DIR/Cargo.toml" | "$WORKSPACE_DIR/Cargo.lock") echo full; return ;;
    "$CRATES_DIR"/*/*)
      rest="${path#"$CRATES_DIR"/}"
      echo "crate:crates/${rest%%/*}"
      return ;;
  esac
  for dir in $(package_dirs); do
    case "$path" in
      "$dir"/*) echo "npm:$(suite_for_package "$dir")"; return ;;
    esac
  done
  case "$path" in
    resources/* | .agents/* | .claude/* | docs/* | "$WORKSPACE_DIR"/resources/* | *.md) echo docs ;;
    *) echo full ;;
  esac
}

# Prints the changed paths, one per line. Reads git unless
# CHANGED_SCOPE_FILES names a fixture list. Fails when the base is unknown.
read_changed_files() {
  if [ -n "${CHANGED_SCOPE_FILES:-}" ]; then
    cat "$CHANGED_SCOPE_FILES"
    return
  fi
  local base
  base=$(git merge-base "${CHANGED_SCOPE_BASE:-origin/main}" HEAD 2>/dev/null) || return 1
  git diff --name-only --no-renames "$base" -- || return 1
  git ls-files --others --exclude-standard || return 1
}

# Prints the workspace's `cargo metadata --no-deps` JSON (or the fixture).
read_cargo_metadata() {
  if [ -n "${CHANGED_SCOPE_METADATA:-}" ]; then
    cat "$CHANGED_SCOPE_METADATA"
    return
  fi
  (cd "$WORKSPACE_DIR" && cargo metadata --no-deps --format-version 1 2>/dev/null)
}

# Prints the comma-separated rust scope for crate directories "$@".
resolve_rust_scope() {
  local metadata
  metadata=$(read_cargo_metadata) || return 1
  printf '%s' "$metadata" | python3 -c "$REVERSE_DEPENDENCIES_PY" "$@"
}

# Classifies every changed path on stdin. Prints "full <path>" for the first
# path that forces full, and the unique crate:/npm: tokens. Pure.
classify_files() {
  local path class
  while IFS= read -r path; do
    [ -n "$path" ] || continue
    class=$(classify_path "$path")
    if [ "$class" = full ]; then
      echo "full $path"
      return
    fi
    [ "$class" = docs ] || echo "$class"
  done | sort -u
}

main() {
  local files classes full_path crate_dirs npm_dirs rust line=""
  if [ "${FULL:-}" = "1" ]; then report full "FULL=1 is set"; return; fi
  files=$(read_changed_files) || { report full "cannot diff against ${CHANGED_SCOPE_BASE:-origin/main}; fetch it or check the git repository"; return; }
  if [ -z "$files" ]; then report docs "no files changed since the merge-base"; return; fi
  classes=$(printf '%s\n' "$files" | classify_files)
  full_path=$(printf '%s\n' "$classes" | sed -n 's/^full //p' | head -n 1)
  if [ -n "$full_path" ]; then report full "$full_path changed"; return; fi
  if [ -z "$classes" ]; then report docs "only documentation or always-run checker scripts outside the crates and npm packages changed"; return; fi
  crate_dirs=$(printf '%s\n' "$classes" | sed -n 's/^crate://p')
  npm_dirs=$(printf '%s\n' "$classes" | sed -n 's/^npm://p')
  if [ -n "$crate_dirs" ]; then
    # shellcheck disable=SC2086 # one word per crate directory
    rust=$(resolve_rust_scope $crate_dirs) || { report full "cargo metadata or crate lookup failed for: $(echo $crate_dirs)"; return; }
    line="rust:$rust"
  fi
  if [ -n "$npm_dirs" ]; then
    # classify_files sorted and deduplicated the packages; join them with commas.
    line="${line:+$line }npm:$(printf '%s\n' "$npm_dirs" | paste -sd, -)"
  fi
  report "$line" "changed crates and npm suites: $(echo $crate_dirs $npm_dirs)"
}

main
