# Task pr-review-loop - Walkthrough

## Summary

Recorded the owner's instruction (2026-10-17) that every agent pull request runs a Cubic review loop until no Cubic comments remain, and that agents working towards the MVP may then merge their own PR into `main`.

| File | Change |
|---|---|
| [`.agents/rules/pr-review-loop.md`](../../.agents/rules/pr-review-loop.md) | New rule: open PR → wait for Cubic (`cubic-dev-ai[bot]`) → fix or answer every thread → push → repeat; merge conditions; 10-round cap (AGENTS.md §22); security disagreements go to the owner |
| [`AGENTS.md`](../../AGENTS.md) | §1 PR Review Mandate names Cubic, the loop, the merge permission and the rule |
| [`.agents/rules/task-workflow.md`](../../.agents/rules/task-workflow.md) | Push on completion is followed by PR, Cubic loop and merge; a PR merge is the only way agent work reaches `main` |
| [`resources/implementation-plan.md`](../implementation-plan.md) | "Branch naming" and "Operator loop" no longer say "Do not open a pull request unless asked" |

TDD-ENFORCE: Not applicable - documentation-only change.

## Validation

`bash scripts/pre-commit.sh` exited 0. Gates 0 to 8 and the end of gate 9, raw output (the 3,700 lines of Cargo and Vitest output in between are omitted):

```text
========================================
Running Pre-Commit Hooks...
========================================
✅ Branch Validation passed.
Starting Port Isolation Validation...
🔍 Validating ./apps/filesquid/ui (Vite)...
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
🔍 Validating ./apps/imagesquid (Vite)...
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
🔍 Validating filesquid-server (default bind)...
✅ filesquid-server uniquely bound to 8083

🚀 VALIDATION PASSED: All modules maintain strict port isolation.
Starting Document Sync Validation...
✅ Document Sync Validation passed.
Starting PRD/SRS Traceability Validation...
✅ Traceability Validation passed: 95 PRD requirements, 81 resolved SRS references.
Starting Portable Documentation Validation...
✅ Portable Documentation validation passed.
Starting Workspace Boundary Validation...
✅ Workspace Boundary validation passed.
Starting logging rule validation...
✅ Logging rule validation passed (no relevant staged files).
Starting Format Validation...
🔍 Validating Rust formatting in ./apps/filesquid...
✅ ./apps/filesquid is properly formatted.
🔍 Validating Prettier formatting in ./apps/filesquid/ui...
✅ ./apps/filesquid/ui is properly formatted.
🔍 Validating Prettier formatting in ./apps/imagesquid...
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
Starting Documentation Validation...
🔍 ADR structure (ISO 42010)...
TAP version 13
# Subtest: stack ADRs exist with ISO 42010 sections and accepted vocabulary
ok 1 - stack ADRs exist with ISO 42010 sections and accepted vocabulary
  ---
  duration_ms: 8.497566
  type: 'test'
  ...
# Subtest: ADR markdown hrefs reject absolute destinations
ok 2 - ADR markdown hrefs reject absolute destinations
  ---
  duration_ms: 0.282701
  type: 'test'
  ...
# Subtest: FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
ok 3 - FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
  ---
  duration_ms: 1.257591
  type: 'test'
  ...
1..3
# tests 3
# suites 0
# pass 3
# fail 0
# cancelled 0
# skipped 0
# todo 0
# duration_ms 91.745104
✅ ADR structure (ISO 42010) passed.
🔍 PRD allocation...
FileSquid: 258 vision IDs, 258 allocated, 129 allocation rows
ImageSquid: 29 vision IDs, 29 allocated, 19 allocation rows
OK: every vision ID is allocated
✅ PRD allocation passed.
🔍 Plan traceability...
OK: every requirement is accounted for in a plan task
✅ Plan traceability passed.
🔍 Relative links and anchors...
checked 142 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 146 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 Task cleanup...
✅ Task cleanup passed.
✅ Documentation validation passed.
[... Clippy, cargo test, per-crate tests, rustdoc, ESLint, tsc and Vitest output omitted ...]
JUNIT report written to apps/imagesquid/tests/allure-results/vitest-junit.xml
✅ Vitest passed.
ℹ️  Playwright e2e skipped for apps/imagesquid (set FILESQUID_E2E=1 to run).
✅ Lint and test validation passed.
========================================
✅ All Pre-Commit Hooks Passed!
========================================
```

## Mocked Environment Variables

None. `npm ci` was run in `apps/filesquid/ui` and `apps/imagesquid` because the gate needs `node_modules`.
