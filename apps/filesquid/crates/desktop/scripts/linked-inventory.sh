#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Linked inventory of a FileSquid desktop build, for the ADR 011 licence and patent review
# (Task DE-F3.1, ADR 035 Decision 5, ADR 006).
#
# Usage: linked-inventory.sh [options] <subject>
#
#   <subject>             a built binary, a macOS .app bundle, or a mounted DMG volume
#   --runtime-lib <path>  a library the media workers load at run time (libvips); repeatable.
#                         Without it the script looks for libvips 8 where the workers do.
#   --without-vips        state that no libvips is expected, so its absence is not an error
#   --no-crates           leave out the Rust crates section (it needs cargo and the workspace)
#
# Prints tab-separated rows to stdout, one section after another:
#
#   file     <path inside the subject>                     every executable and library scanned
#   lib      <link|runtime> <owner> <path>                 every dynamic library reached
#   vips     <key> <value>                                 the libvips found and its build options
#   module   <path>                                        every libvips loader module
#   crate    <name> <version> <licence>                    every Rust crate linked into the binary
#   finding  <kind> <detail>                               what the review must not miss
#
# `link` libraries are reached from the subject's own files. `runtime` libraries are reached
# only from a run-time loaded library. The owner is `bundled` (inside the subject), `system`
# (part of macOS), a package name and version (dpkg, rpm or Homebrew), or `-`.
#
# Reads files and runs ldd (Linux) or otool (macOS), and cargo for the crates section. It
# writes only to a temporary directory, which it removes on every exit path.
#
# Exit status: 0 complete inventory; 1 usage error, unreadable subject, or an incomplete
# inventory (a library that could not be found or inspected, a --runtime-lib that is not a
# readable file, the Rust crates not listed, or the walk limit reached); 2 ImageMagick is in the inventory, bundled in the subject or reached from it
# (ADR 006 forbids it; the inventory is still printed); 3 libvips was not found, so the
# inventory is incomplete. Each case also prints a `finding` row.

set -u

# The most files and libraries one run walks (AGENTS.md section 22). Past it the run stops
# with a `finding` row and exit status 1, because the inventory would be incomplete.
MAX_ITEMS=5000

# The most symbolic links followed to reach one file.
MAX_LINKS=40

SUBJECT=""
WITHOUT_VIPS=0
WITH_CRATES=1
RUNTIME_LIBS=""
HERE=$(cd "$(dirname "$0")" && pwd -P)
WORKSPACE=$(cd "$HERE/../../.." && pwd -P)
PLATFORM=$(uname -s)

usage() {
  sed -n '9,16p' "$0" | sed 's/^# \{0,1\}//' >&2
}

# Parse the command line into the globals above. Exits 1 on a usage error.
parse_arguments() {
  while [ $# -gt 0 ]; do
    case "$1" in
      --runtime-lib)
        [ $# -ge 2 ] || { usage; exit 1; }
        RUNTIME_LIBS="${RUNTIME_LIBS}$2
"
        shift 2
        ;;
      --without-vips) WITHOUT_VIPS=1; shift ;;
      --no-crates) WITH_CRATES=0; shift ;;
      -h | --help) usage; exit 0 ;;
      -*) echo "unknown option: $1" >&2; usage; exit 1 ;;
      *)
        [ -z "$SUBJECT" ] || { echo "only one subject may be given" >&2; exit 1; }
        SUBJECT="$1"
        shift
        ;;
    esac
  done
  [ -n "$SUBJECT" ] || { usage; exit 1; }
  [ -e "$SUBJECT" ] || { echo "subject not found: $SUBJECT" >&2; exit 1; }
}

# Whether file $1 is an executable or library of this platform (ELF or Mach-O). Reads the file.
is_object_file() {
  local magic
  magic=$(head -c 4 "$1" 2>/dev/null | od -An -tx1 | tr -d ' \n')
  case "$magic" in
    7f454c46) return 0 ;;                                  # ELF
    cffaedfe | cefaedfe | feedfacf | feedface | cafebabe) return 0 ;; # Mach-O, thin or universal
    *) return 1 ;;
  esac
}

# Print the physical path of $1, or $1 itself when it cannot be resolved. Follows at most
# MAX_LINKS symbolic links ($2 counts them), so a link loop ends with the path reached.
physical_path() {
  local dir base target links="${2:-0}"
  dir=$(cd "$(dirname "$1")" 2>/dev/null && pwd -P) || { printf '%s\n' "$1"; return; }
  base=$(basename "$1")
  if [ ! -L "$dir/$base" ] || [ "$links" -ge "$MAX_LINKS" ] || ! command -v readlink >/dev/null 2>&1; then
    printf '%s\n' "$dir/$base"
    return
  fi
  target=$(readlink "$dir/$base")
  case "$target" in
    /*) physical_path "$target" $((links + 1)) ;;
    *) physical_path "$dir/$target" $((links + 1)) ;;
  esac
}

# Print the object files of the subject, one per line: the file itself, or every object file
# under the directory.
list_subject_files() {
  if [ -f "$SUBJECT_PATH" ]; then
    printf '%s\n' "$SUBJECT_PATH"
    return
  fi
  find "$SUBJECT_PATH" -type f 2>/dev/null | sort | while IFS= read -r candidate; do
    if is_object_file "$candidate"; then
      printf '%s\n' "$candidate"
    fi
  done
}

# Print the dynamic libraries file $1 names, resolved, one per line (Linux: ldd). A path may
# hold spaces: it is everything between "=> " (or the line start) and the load address.
list_dependencies_linux() {
  ldd "$1" 2>/dev/null | awk '
    /=> not found/ { sub(/^[ \t]+/, ""); sub(/ => not found.*$/, ""); print "!" $0; next }
    {
      line = $0
      arrow = index(line, "=> ")
      if (arrow > 0) line = substr(line, arrow + 3)
      sub(/^[ \t]+/, "", line)
      sub(/ \(0x[0-9a-fA-F]+\)$/, "", line)
      if (line ~ /^\//) print line
    }'
}

# Print the run paths (LC_RPATH) of Mach-O file $1, one per line. A path may hold spaces.
list_rpaths_macos() {
  otool -l "$1" 2>/dev/null | awk '/cmd LC_RPATH/ {
    getline; getline
    sub(/^[ \t]*path /, ""); sub(/ \(offset [0-9]+\)$/, "")
    print
  }'
}

# Print the first existing file that @rpath name $1 resolves to for loader $2, trying the run
# paths of the loader and then of the main executable; nothing when none exists.
resolve_rpath_name_macos() {
  local name="$1" loader="$2" rpath candidate
  { list_rpaths_macos "$loader"; list_rpaths_macos "$MAIN_FILE"; } | while IFS= read -r rpath; do
    case "$rpath" in
      @rpath/*) continue ;;
    esac
    candidate=$(resolve_install_name_macos "$rpath/${name#@rpath/}" "$loader")
    if [ -e "$candidate" ]; then
      printf '%s\n' "$candidate"
      break
    fi
  done
}

# Print install name $1 resolved for the file $2 that names it: @executable_path,
# @loader_path and @rpath are expanded; a name that cannot be found is printed with a
# leading "!".
resolve_install_name_macos() {
  local name="$1" loader="$2" loader_dir found
  loader_dir=$(dirname "$loader")
  case "$name" in
    @executable_path/*) printf '%s\n' "$MAIN_DIR/${name#@executable_path/}" ;;
    @loader_path/*) printf '%s\n' "$loader_dir/${name#@loader_path/}" ;;
    @rpath/*)
      found=$(resolve_rpath_name_macos "$name" "$loader")
      if [ -n "$found" ]; then printf '%s\n' "$found"; else printf '!%s\n' "$name"; fi
      ;;
    *) printf '%s\n' "$name" ;;
  esac
}

# Print the dynamic libraries file $1 names, resolved, one per line (macOS: otool).
list_dependencies_macos() {
  otool -L "$1" 2>/dev/null | awk '!/:$/ {
    sub(/^[ \t]+/, ""); sub(/ \(compatibility version.*$/, "")
    if ($0 != "") print
  }' | sort -u |
    while IFS= read -r name; do
      resolve_install_name_macos "$name" "$1"
    done
}

# Print the dynamic libraries file $1 names, for this platform.
list_dependencies() {
  if [ "$PLATFORM" = "Darwin" ]; then
    list_dependencies_macos "$1"
  else
    list_dependencies_linux "$1"
  fi
}

# Print who provides library $1: bundled, system, a package with its version, or "-".
library_owner() {
  local path="$1" package
  case "$path" in
    "$SUBJECT_PATH"/*) echo "bundled"; return ;;
  esac
  if [ "$PLATFORM" = "Darwin" ]; then
    case "$path" in
      /usr/lib/* | /System/*) echo "system"; return ;;
      */Cellar/*)
        printf '%s\n' "$path" | sed 's#.*/Cellar/\([^/]*\)/\([^/]*\)/.*#homebrew:\1 \2#'
        return
        ;;
    esac
    echo "-"
    return
  fi
  if command -v dpkg-query >/dev/null 2>&1; then
    package=$(dpkg-query -S "$path" 2>/dev/null | head -n 1 | sed 's/: .*//')
    if [ -n "$package" ]; then
      printf 'dpkg:%s %s\n' "$package" "$(dpkg-query -W -f='${Version}' "$package" 2>/dev/null)"
      return
    fi
  fi
  if command -v rpm >/dev/null 2>&1 && package=$(rpm -qf "$path" 2>/dev/null); then
    printf 'rpm:%s\n' "$package"
    return
  fi
  echo "-"
}

# Whether macOS serves library $1 from its shared cache, so there is no file to walk.
is_macos_system_library() {
  [ "$PLATFORM" = "Darwin" ] || return 1
  case "$1" in
    /usr/lib/* | /System/*) return 0 ;;
    *) return 1 ;;
  esac
}

# Walk the dependencies of every file listed in file $1, breadth first, and append one
# `lib` row per library first reached to $ROWS, tagged with origin $2. Libraries already in
# $SEEN are skipped. Stops at MAX_ITEMS.
walk_libraries() {
  local queue="$1" origin="$2" count=0 current dependency resolved
  while [ -s "$queue" ]; do
    current=$(head -n 1 "$queue")
    tail -n +2 "$queue" > "$queue.next" && mv "$queue.next" "$queue"
    count=$((count + 1))
    if [ "$count" -gt "$MAX_ITEMS" ]; then
      printf 'finding\tlimit\tmore than %s libraries; the inventory is incomplete\n' "$MAX_ITEMS" >> "$FINDINGS"
      return
    fi
    if ! is_object_file "$current"; then
      printf 'finding\tuninspected\tnot an executable or library of this platform: %s\n' "$current" >> "$FINDINGS"
      continue
    fi
    list_dependencies "$current" | while IFS= read -r dependency; do
      case "$dependency" in
        !*)
          printf 'finding\tunresolved\t%s (named by %s)\n' "${dependency#!}" "$current" >> "$FINDINGS"
          continue
          ;;
      esac
      resolved=$(physical_path "$dependency")
      grep -Fxq "$resolved" "$SEEN" && continue
      printf '%s\n' "$resolved" >> "$SEEN"
      printf 'lib\t%s\t%s\t%s\n' "$origin" "$(library_owner "$resolved")" "$resolved" >> "$ROWS"
      if [ -f "$resolved" ] && ! is_macos_system_library "$resolved"; then
        printf '%s\n' "$resolved" >> "$queue"
      fi
    done
  done
}

# Print the libvips the media workers would load on this machine, or nothing. The names
# follow LIBRARY_NAMES in crates/native/src/workers/media/vips.rs.
find_libvips() {
  local candidate
  if [ "$PLATFORM" = "Darwin" ]; then
    for candidate in "$SUBJECT_PATH/Contents/Frameworks/libvips.42.dylib" \
      /opt/homebrew/lib/libvips.42.dylib /usr/local/lib/libvips.42.dylib; do
      if [ -e "$candidate" ]; then
        printf '%s\n' "$candidate"
        return
      fi
    done
    return
  fi
  if command -v ldconfig >/dev/null 2>&1; then
    ldconfig -p 2>/dev/null | awk '$1 == "libvips.so.42" { print $NF; exit }'
  fi
}

# Print the run-time loaded libraries to walk: the ones named on the command line, else the
# libvips found on this machine, else (with --without-vips) none.
list_runtime_libraries() {
  if [ -n "$RUNTIME_LIBS" ]; then
    printf '%s' "$RUNTIME_LIBS"
    return
  fi
  [ "$WITHOUT_VIPS" -eq 1 ] || find_libvips
}

# Print the loader modules beside library $1 (`vips-modules-<version>/` in its directory).
list_vips_modules() {
  local dir
  dir=$(dirname "$1")
  find "$dir" -maxdepth 2 -path '*/vips-modules-*' -type f 2>/dev/null | sort
}

# Print the `vips` rows for run-time library $1: where it is, and the version and build
# options its own `vips` program reports when there is one beside it or on PATH.
describe_vips() {
  local library="$1" tool
  printf 'vips\tlibrary\t%s\n' "$library"
  tool="$(dirname "$(dirname "$library")")/bin/vips"
  [ -x "$tool" ] || tool=$(command -v vips 2>/dev/null || true)
  if [ -z "$tool" ] || [ ! -x "$tool" ]; then
    printf 'vips\tbuild-options\tunknown: no vips program found to report them\n'
    return
  fi
  printf 'vips\tversion\t%s\n' "$("$tool" --version 2>/dev/null | head -n 1)"
  "$tool" --vips-config 2>/dev/null | sed '/^[[:space:]]*$/d' | while IFS= read -r line; do
    printf 'vips\tbuild-option\t%s\n' "$line"
  done
}

# Walk every run-time loaded library and its loader modules; print the `vips` and `module`
# rows. Records a finding when none is found and --without-vips was not given, and one for
# each named library that is not a readable file (missing, or a symbolic-link loop).
inventory_runtime_libraries() {
  local queue="$WORK/runtime-queue" library module found=0
  : > "$queue"
  while IFS= read -r named; do
    [ -n "$named" ] || continue
    found=1
    library=$(physical_path "$named")
    if [ ! -f "$library" ] || [ ! -r "$library" ]; then
      printf 'finding\truntime-missing\t%s\n' "$named" >> "$FINDINGS"
      continue
    fi
    case "$(basename "$library")" in libvips*) describe_vips "$library" ;; esac
    if ! grep -Fxq "$library" "$SEEN"; then
      printf '%s\n' "$library" >> "$SEEN"
      printf 'lib\truntime\t%s\t%s\n' "$(library_owner "$library")" "$library" >> "$ROWS"
    fi
    printf '%s\n' "$library" >> "$queue"
    list_vips_modules "$library" | while IFS= read -r module; do
      printf 'module\t%s\n' "$module"
      printf '%s\n' "$module" >> "$SEEN"
      printf 'lib\truntime\t%s\t%s\n' "$(library_owner "$module")" "$module" >> "$ROWS"
      printf '%s\n' "$module" >> "$queue"
    done
  done <<EOF
$(list_runtime_libraries)
EOF
  walk_libraries "$queue" runtime
  if [ "$found" -eq 0 ] && [ "$WITHOUT_VIPS" -eq 0 ]; then
    printf 'finding\tlibvips-missing\tno libvips 8 on this machine or in the subject; its libraries and loaders are not listed\n' >> "$FINDINGS"
  fi
}

# Print one `crate` row per Rust crate linked into filesquid-desktop, with its licence.
# Runs `cargo tree` in the workspace; a failure is a finding.
inventory_crates() {
  local listing="$WORK/crates"
  if ! (cd "$WORKSPACE" && cargo tree -p filesquid-desktop -e normal --locked --prefix none \
    --format '{p}|{l}') > "$listing" 2> "$WORK/crates-error"; then
    printf 'finding\tcrates-unlisted\tcargo tree failed: %s\n' "$(head -n 1 "$WORK/crates-error")" >> "$FINDINGS"
    return
  fi
  sed 's/ (\*)$//' "$listing" | awk -F'|' 'NF == 2 {
    split($1, package, " ")
    licence = ($2 == "") ? "UNDECLARED" : $2
    print "crate\t" package[1] "\t" substr(package[2], 2) "\t" licence
  }' | sort -u
}

# Record a finding for every file of the subject, and every library or loader it reaches,
# whose file name names ImageMagick (ADR 006). The folder it is in does not count.
find_imagemagick() {
  awk -F'\t' '{
    count = split($NF, parts, "/")
    if (tolower(parts[count]) ~ /magick/) print "finding\timagemagick\t" $NF
  }' "$WORK/files" "$ROWS" >> "$FINDINGS"
}

# Print the exit status the findings call for: 2 ImageMagick, 3 libvips missing, 1 an
# incomplete inventory (walk limit, unresolved, uninspected or missing library, crates not
# listed), else 0.
exit_status() {
  if grep -q '^finding	imagemagick	' "$FINDINGS"; then echo 2; return; fi
  if grep -q '^finding	libvips-missing	' "$FINDINGS"; then echo 3; return; fi
  if grep -Eq '^finding	(limit|unresolved|uninspected|runtime-missing|crates-unlisted)	' "$FINDINGS"; then echo 1; return; fi
  echo 0
}

# Print the subject's main executable: the first file in a `Contents/MacOS` folder of an app
# bundle, the subject itself or an app inside a mounted volume, else the first object file.
find_main_executable() {
  local bundled=""
  if [ -d "$SUBJECT_PATH" ]; then
    bundled=$(find "$SUBJECT_PATH" -maxdepth 4 -path '*/Contents/MacOS/*' -type f 2>/dev/null | sort | head -n 1)
  fi
  if [ -n "$bundled" ]; then
    printf '%s\n' "$bundled"
  else
    head -n 1 "$WORK/files"
  fi
}

main() {
  parse_arguments "$@"
  SUBJECT_PATH=$(physical_path "$SUBJECT")
  WORK=$(mktemp -d "${TMPDIR:-/tmp}/filesquid-inventory.XXXXXX") || exit 1
  trap 'rm -rf "$WORK"' EXIT
  SEEN="$WORK/seen" ROWS="$WORK/rows" FINDINGS="$WORK/findings"
  : > "$SEEN"; : > "$ROWS"; : > "$FINDINGS"
  list_subject_files > "$WORK/files"
  [ -s "$WORK/files" ] || { echo "no executable or library in: $SUBJECT" >&2; exit 1; }
  MAIN_FILE=$(find_main_executable)
  MAIN_DIR=$(dirname "$MAIN_FILE")

  printf '# FileSquid linked inventory (ADR 011 review input)\n'
  printf '# subject: %s\n# platform: %s %s\n' "$(basename "$SUBJECT_PATH")" "$PLATFORM" "$(uname -m)"
  if [ -d "$SUBJECT_PATH" ]; then
    printf '# main executable: %s\n' "${MAIN_FILE#"$SUBJECT_PATH"/}"
  else
    printf '# main executable: %s\n' "$(basename "$MAIN_FILE")"
  fi
  printf '## Files scanned\n'
  sed "s#^$SUBJECT_PATH/##; s#^#file\t#" "$WORK/files"
  cat "$WORK/files" >> "$SEEN"
  cp "$WORK/files" "$WORK/link-queue"
  walk_libraries "$WORK/link-queue" link
  printf '## Run-time loaded libvips\n'
  inventory_runtime_libraries
  printf '## Dynamic libraries\n'
  sort -u "$ROWS"
  if [ "$WITH_CRATES" -eq 1 ]; then
    printf '## Rust crates linked into the binary\n'
    inventory_crates
  fi
  find_imagemagick
  printf '## Findings\n'
  if [ -s "$FINDINGS" ]; then sort -u "$FINDINGS"; else printf 'finding\tnone\t-\n'; fi
  exit "$(exit_status)"
}

main "$@"
