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
# names containing spaces survive. Missing roots are skipped.
list_walkthroughs() {
  local root
  for root in "${SEARCH_ROOTS[@]}"; do
    [ -d "$root" ] || continue
    find "$root" -name "*-Walkthrough.md" -print0 2>/dev/null
  done
}

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
done < <(list_walkthroughs)

if [ $FAILED -ne 0 ]; then
  echo "You must remove the '-Plan.md' and '-ToDos.md' files once the '-Walkthrough.md' is generated."
  exit 1
fi

exit 0
