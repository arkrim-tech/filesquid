# Task docs-port-conflicts-agents Walkthrough: agents never run the port-clearing one-liner

Documentation-only task on branch `docs/port-conflicts-agents`. TDD-ENFORCE: Not applicable - documentation update. No crate, script or npm package changes.

## Owner request (2026-10-18)

Cubic flagged (P1) on PR #60 that [port-conflicts.md](../../.agents/rules/port-conflicts.md) still prescribed the unsafe port-clearing one-liner, which on a shared machine can kill another agent's process. Owner decision: keep the hint as advice for the human, and forbid agents from running it.

## Changes

- [port-conflicts.md](../../.agents/rules/port-conflicts.md): the one-liner is a hint the agent gives the human to run on their own machine. Agents MUST NOT run it. An agent whose own server holds the port stops it by the PID it recorded at start; if another process holds the port, the agent reports it to the human with the hint or uses a different free port where the task allows, and never kills it. Links to the Process rules in [filesquid-platform-engineer.md](../../.claude/agents/filesquid-platform-engineer.md).

## Hook output (UTC)

Run: `bash scripts/pre-commit.sh` on the staged change, `TMPDIR=/dev/shm/docs-port-conflicts`.

```text
changed_scope: docs (only documentation or always-run checker scripts outside the crates and npm packages changed)
========================================
Running Pre-Commit Hooks... (test scope: docs)
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
✅ Traceability Validation passed: 95 PRD requirements, 82 resolved SRS references.
Starting Portable Documentation Validation...
✅ Portable Documentation validation passed.
Starting Workspace Boundary Validation...
✅ Workspace Boundary validation passed.
Starting logging rule validation...
✅ Logging rule validation passed (no relevant staged files).
Starting Format Validation...
⏭️  Skipped: Rust formatting in ./apps/filesquid (no Rust file changed; test scope: docs)
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
  duration_ms: 10.470134
  type: 'test'
  ...
# Subtest: ADR markdown hrefs reject absolute destinations
ok 2 - ADR markdown hrefs reject absolute destinations
  ---
  duration_ms: 0.390064
  type: 'test'
  ...
# Subtest: FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
ok 3 - FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
  ---
  duration_ms: 1.40445
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
# duration_ms 105.708968
✅ ADR structure (ISO 42010) passed.
🔍 PRD allocation...
FileSquid: 260 vision IDs, 260 allocated, 130 allocation rows
ImageSquid: 29 vision IDs, 29 allocated, 19 allocation rows
OK: every vision ID is allocated
✅ PRD allocation passed.
🔍 Plan traceability...
OK: every requirement is accounted for in a plan task
✅ Plan traceability passed.
🔍 Relative links and anchors...
checked 205 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 164 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 MVP queue and merge driver tests...
...............................................
----------------------------------------------------------------------
Ran 47 tests in 5.464s

OK
✅ MVP queue and merge driver tests passed.
🔍 MVP progress bar...
✅ MVP progress bar passed.
🔍 Task cleanup...
✅ Task cleanup passed.
✅ Documentation validation passed.
Starting Change Scope Validation...
✅ Change scope validation passed (166 assertions).
Starting Lint and Test Validation...
ℹ️  Test scope: docs
⏭️  Skipped: Clippy (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Workspace tests (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-core (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-ocfl (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-metadata (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-c2pa (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-api (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-native (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: filesquid-metadata all-features tests and wasm32 builds (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Rustdoc (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Clippy (desktop, test-instrumented) (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Desktop tests (test-instrumented) (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: ESLint, TypeScript, Vitest and Playwright e2e in apps/filesquid/ui apps/imagesquid (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Desktop packaged-window e2e (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
✅ Lint and test validation passed (test scope: docs).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: docs (18s)
========================================
```
