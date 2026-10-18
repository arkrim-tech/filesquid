# Task docs-orchestrator-agent-definition: Walkthrough

## What changed

A new agent definition, [filesquid-orchestrator.md](../../.claude/agents/filesquid-orchestrator.md), describes the FileSquid build orchestrator. It turns the owner's orchestrator prompt into a standing definition, so later orchestrator prompts only carry the live lane session IDs, the per-chat merge budget and any new decisions.

- Frontmatter follows the three engineer role files: `name`, `description` (when to use it) and `color`. Like them, it sets no `tools` or `model` field.
- Sections: what the owner's prompt supplies, role and milestone, times, start-up state rebuild, dispatch model (cloud lanes, in-container sub-agents, concurrency, model choice and escalation), the pasteable instructions block for every agent and lane, merge procedure, cleanup and capacity, token discipline and the per-chat merge budget, and owner decisions.
- Every rule in the owner's prompt is kept. The milestone and gate task are read from the "Current milestone" section of [pr-review-loop.md](../../.agents/rules/pr-review-loop.md) instead of being hard-coded, and the plan files are linked relatively.
- The instructions block refers to the engineer role files for what they already cover (build lock and narrow cargo commands, `FULL=1`, the ADR 039 workspace-hack, merge-commit hygiene, no owner email, model names) and keeps pasted what a cloud lane needs, because a cloud lane does not load role files on its own.
- Models are named only by the dispatch tier names (`sonnet`, `haiku`, default); no model identifier appears. The shared Cargo target is written as `$HOME/.cargo-target-shared`, so the file holds no environment-specific home path.
- No `AGENTS.md` or README section lists the agent definitions, so no index was updated.

## Cubic review fixes

Round 1 (5 P2 findings, all fixed in the definition):

- Global tasks, including the milestone's gate task, now have a dispatch rule: they go to the role that owns their module slices, to a role-less sub-agent when they have none, and to the owner when they need hardware no lane has.
- Docs and tooling agents without an engineer role are named in the instructions block, with the role files' process rules they still keep.
- The Cubic loop in the block carries the bounds of pr-review-loop.md: 10 rounds, three returns of one finding, 3 re-triggers per head commit (after 15 minutes), and the 24-poll cap.
- The polling rule now says it replaces the rule's subscription and fallback check-in by owner decision, and keeps the capped re-trigger path.
- Open PRs are listed in bounded 100-item pages (at most 10), with a warning when the cap is reached.

TDD-ENFORCE: Not applicable - documentation-only change (an agent definition and this walkthrough).

## Verification

The change scope is `docs`, so an ordinary `bash scripts/pre-commit.sh` is the required run ([local-verification.md](../../.agents/rules/local-verification.md#change-aware-test-scope)).

Run at 2026-10-18T23:26:49Z (UTC). Summary line: `All Pre-Commit Hooks Passed! Test scope: docs (22s)`.

Complete, unabridged hook output of the first run:

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
  duration_ms: 10.145923
  type: 'test'
  ...
# Subtest: ADR markdown hrefs reject absolute destinations
ok 2 - ADR markdown hrefs reject absolute destinations
  ---
  duration_ms: 0.426578
  type: 'test'
  ...
# Subtest: FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
ok 3 - FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
  ---
  duration_ms: 1.382011
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
# duration_ms 101.15404
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
checked 206 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 165 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 MVP queue and merge driver tests...
...............................................
----------------------------------------------------------------------
Ran 47 tests in 6.673s

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
✅ All Pre-Commit Hooks Passed! Test scope: docs (22s)
========================================
```

### Cubic round 1 fix commit

The hook was run again on the round 1 fix commit, after its walkthrough edit was staged. Summary line: `All Pre-Commit Hooks Passed! Test scope: docs (20s)`.

Complete, unabridged hook output:

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
  duration_ms: 15.729493
  type: 'test'
  ...
# Subtest: ADR markdown hrefs reject absolute destinations
ok 2 - ADR markdown hrefs reject absolute destinations
  ---
  duration_ms: 0.307174
  type: 'test'
  ...
# Subtest: FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
ok 3 - FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
  ---
  duration_ms: 1.383348
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
# duration_ms 104.241524
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
checked 206 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 165 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 MVP queue and merge driver tests...
...............................................
----------------------------------------------------------------------
Ran 47 tests in 5.647s

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
✅ All Pre-Commit Hooks Passed! Test scope: docs (20s)
========================================
```
