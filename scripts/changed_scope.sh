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
#   docs                    only Markdown, resources/ or .agents/ files changed
#                           outside the crates and npm packages. Every file inside
#                           a crate or package, Markdown and resources/ included,
#                           belongs to it, because its tests read files there
#                           (for example crates/api/resources/srs.md and
#                           crates/c2pa/README.md)
#   rust:<crates>           the changed crates under apps/filesquid/crates/*, plus
#                           every workspace crate that depends on them (all
#                           dependency kinds, transitively; from
#                           `cargo metadata --no-deps --format-version 1`)
#   npm:<packages>          the changed npm packages, plus every package that
#                           links one by a `file:` dependency (ImageSquid links
#                           @filesquid/ui)
#   rust:<crates> npm:<packages>   both
#   full                    the workspace Cargo.toml or Cargo.lock, a
#                           rust-toolchain file, any package.json or
#                           package-lock.json, scripts/, a generated contract file
#                           (openapi.json, schema.d.ts) or any file outside the
#                           categories above changed; or FULL=1 is set; or
#                           detection failed (unknown base ref, git or
#                           cargo metadata error, unknown crate directory)
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

WORKSPACE_DIR="apps/filesquid"
CRATES_DIR="$WORKSPACE_DIR/crates"
NPM_PACKAGES="apps/filesquid/ui apps/imagesquid"

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

# Reads the package list in NPM_PACKAGES (environment) and changed packages as
# arguments. Prints the comma-separated changed packages plus every listed
# package that links one of them by a `file:` dependency, in list order.
NPM_DEPENDENTS_PY='
import json, os, sys
packages = os.environ["NPM_PACKAGES"].split()
dependents = {pkg: set() for pkg in packages}
for pkg in packages:
    with open(os.path.join(pkg, "package.json"), encoding="utf-8") as handle:
        manifest = json.load(handle)
    for field in ("dependencies", "devDependencies", "peerDependencies", "optionalDependencies"):
        for spec in manifest.get(field, {}).values():
            if spec.startswith("file:"):
                target = os.path.normpath(os.path.join(pkg, spec[5:])).replace(os.sep, "/")
                if target in dependents:
                    dependents[target].add(pkg)
scope, queue = set(), list(sys.argv[1:])
while queue:  # bounded: a package is expanded at most once
    name = queue.pop()
    if name not in scope:
        scope.add(name)
        queue.extend(dependents[name] - scope)
print(",".join(pkg for pkg in packages if pkg in scope))
'

# Prints the scope line on stdout and the reason on stderr.
report() {
  echo "changed_scope: $1 ($2)" >&2
  printf '%s\n' "$1"
}

# Classifies one changed path. Pure. The case order matters: full triggers,
# then crates, then npm packages, then documentation. Prints one of: docs, full,
# crate:<dir relative to the workspace root>, npm:<package directory>.
classify_path() {
  local path="$1" rest pkg
  case "${path##*/}" in
    package.json | package-lock.json | openapi.json | schema.d.ts | rust-toolchain | rust-toolchain.toml) echo full; return ;;
  esac
  case "$path" in
    scripts/* | "$WORKSPACE_DIR/Cargo.toml" | "$WORKSPACE_DIR/Cargo.lock") echo full; return ;;
    "$CRATES_DIR"/*/*)
      rest="${path#"$CRATES_DIR"/}"
      echo "crate:crates/${rest%%/*}"
      return ;;
  esac
  for pkg in $NPM_PACKAGES; do
    case "$path" in
      "$pkg"/*) echo "npm:$pkg"; return ;;
    esac
  done
  case "$path" in
    resources/* | .agents/* | "$WORKSPACE_DIR"/resources/* | *.md) echo docs ;;
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

# Prints the comma-separated npm scope for package directories "$@".
resolve_npm_scope() {
  NPM_PACKAGES="$NPM_PACKAGES" python3 -c "$NPM_DEPENDENTS_PY" "$@"
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
  local files classes full_path crate_dirs npm_dirs rust npm line=""
  if [ "${FULL:-}" = "1" ]; then report full "FULL=1 is set"; return; fi
  files=$(read_changed_files) || { report full "cannot diff against ${CHANGED_SCOPE_BASE:-origin/main}; fetch it or check the git repository"; return; }
  if [ -z "$files" ]; then report docs "no files changed since the merge-base"; return; fi
  classes=$(printf '%s\n' "$files" | classify_files)
  full_path=$(printf '%s\n' "$classes" | sed -n 's/^full //p' | head -n 1)
  if [ -n "$full_path" ]; then report full "$full_path changed"; return; fi
  if [ -z "$classes" ]; then report docs "only Markdown, resources/ or .agents/ files outside the crates and npm packages changed"; return; fi
  crate_dirs=$(printf '%s\n' "$classes" | sed -n 's/^crate://p')
  npm_dirs=$(printf '%s\n' "$classes" | sed -n 's/^npm://p')
  if [ -n "$crate_dirs" ]; then
    # shellcheck disable=SC2086 # one word per crate directory
    rust=$(resolve_rust_scope $crate_dirs) || { report full "cargo metadata or crate lookup failed for: $(echo $crate_dirs)"; return; }
    line="rust:$rust"
  fi
  if [ -n "$npm_dirs" ]; then
    # shellcheck disable=SC2086 # one word per package directory
    npm=$(resolve_npm_scope $npm_dirs) || { report full "npm package lookup failed for: $(echo $npm_dirs)"; return; }
    line="${line:+$line }npm:$npm"
  fi
  report "$line" "changed: $(echo $crate_dirs $npm_dirs)"
}

main
