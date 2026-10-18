#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Task cleanup validation: fails when a task's Plan or ToDos document still exists next to its Walkthrough.
#
# This script ensures that whenever a Walkthrough document exists for a task,
# the corresponding Plan and ToDos documents are removed to prevent clutter.
# Walkthroughs are searched under apps/ (module and UI tasks) and under the
# repository-root resources/tasks/ (documentation-only tasks).

FAILED=0

# Walkthrough documents live under apps/ and under the repository-root resources/tasks/.
SEARCH_ROOTS=(apps resources/tasks)

# Prints every Walkthrough path under the search roots, NUL-separated so file
# names containing spaces survive. Missing roots are skipped. Returns non-zero
# when a scan fails, so the gate fails closed instead of checking fewer files.
list_walkthroughs() {
  local root
  for root in "${SEARCH_ROOTS[@]}"; do
    [ -d "$root" ] || continue
    find "$root" -name "*-Walkthrough.md" -print0 || return 1
  done
}

# Scan into a temporary file (removed on exit) so a failed scan is detected;
# process substitution would hide the scan's exit status.
LIST=$(mktemp "${TMPDIR:-/tmp}/task-cleanup.XXXXXX") || { echo "❌ ERROR: cannot create a temporary file"; exit 1; }
trap 'rm -f "$LIST"' EXIT
if ! list_walkthroughs > "$LIST"; then
  echo "❌ ERROR: Task cleanup scan failed: could not search for Walkthrough documents under: ${SEARCH_ROOTS[*]}"
  exit 1
fi

while IFS= read -r -d '' WT; do
  # Extract the base path and the task identifier (e.g., "apps/app_abc/resources/tasks/Task-1.1")
  BASE_NAME="${WT%-Walkthrough.md}"
  
  PLAN_FILE="${BASE_NAME}-Plan.md"
  TODOS_FILE="${BASE_NAME}-ToDos.md"

  if [ -f "$PLAN_FILE" ]; then
    echo "❌ ERROR: Task cleanup failure. Found Walkthrough but Plan document still exists: $PLAN_FILE"
    FAILED=1
  fi

  if [ -f "$TODOS_FILE" ]; then
    echo "❌ ERROR: Task cleanup failure. Found Walkthrough but ToDos document still exists: $TODOS_FILE"
    FAILED=1
  fi
done < "$LIST"

if [ $FAILED -ne 0 ]; then
  echo "You must remove the '-Plan.md' and '-ToDos.md' files once the '-Walkthrough.md' is generated."
  exit 1
fi

exit 0
