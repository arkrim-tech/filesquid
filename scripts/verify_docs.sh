#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Peter Van Garderen
#
# SPDX-License-Identifier: MIT
#
# Runs the documentation validators under scripts/docs/ and the task-cleanup
# check. Each validator exits non-zero on failure and prints what is wrong.
#   - validate-adrs.test.mjs: every ADR has the ISO 42010 sections and header.
#   - check_prd_allocation.py: every vision requirement ID is allocated in a PRD.
#   - check_plan_traceability.py: every PRD, SRS and module SRS requirement is cited by a plan task.
#   - check_links.py: every relative Markdown link and anchor resolves.
#   - check_distribution_links.py: no link crosses the FileSquid/ImageSquid boundary.
#   - update_mvp_progress.py --check: the checklist's MVP progress bar matches its ticked boxes.
#   - verify_task_cleanup.sh: no Plan/ToDos file survives next to its Walkthrough, under apps/ and the root resources/tasks/.
#
# Usage: ./scripts/verify_docs.sh   (from the repository root)

echo "Starting Documentation Validation..."

FAILED=0

run_check() {
  # $1 = label, remaining = command. Records a failure instead of exiting so
  # every validator reports in one pass.
  local label="$1"; shift
  echo "🔍 $label..."
  if "$@"; then
    echo "✅ $label passed."
  else
    echo "❌ $label failed."
    FAILED=1
  fi
}

run_check "ADR structure (ISO 42010)" node --test scripts/docs/validate-adrs.test.mjs
run_check "PRD allocation" python3 scripts/docs/check_prd_allocation.py
run_check "Plan traceability" python3 scripts/docs/check_plan_traceability.py
run_check "Relative links and anchors" python3 scripts/docs/check_links.py
run_check "FileSquid and ImageSquid document boundary" python3 scripts/docs/check_distribution_links.py
run_check "MVP progress bar" python3 scripts/docs/update_mvp_progress.py --check
run_check "Task cleanup" ./scripts/verify_task_cleanup.sh

if [ "$FAILED" -ne 0 ]; then
  echo ""
  echo "🚨 VALIDATION FAILED: fix the documentation issues listed above before committing."
  exit 1
fi

echo "✅ Documentation validation passed."
exit 0
