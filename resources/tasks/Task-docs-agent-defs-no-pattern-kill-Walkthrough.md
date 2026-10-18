# Task docs-agent-defs-no-pattern-kill: Walkthrough

Run at 2026-10-18T22:51:01Z (UTC).

## What changed

One bullet was added to the "Process rules" section of each engineer agent definition:

- [filesquid-modules-engineer.md](../../.claude/agents/filesquid-modules-engineer.md)
- [filesquid-platform-engineer.md](../../.claude/agents/filesquid-platform-engineer.md)
- [filesquid-ui-engineer.md](../../.claude/agents/filesquid-ui-engineer.md)

The bullet forbids stopping processes by name or pattern (`pkill -f`, `killall`, `pgrep … | xargs kill`). Agents share one machine, so a pattern kill can stop another agent's hook run. An agent stops only a process it started, by the PID it recorded. The owner specified the wording. Nothing else in the three files, frontmatter included, changed.

Why: a desktop agent ran `pkill -f scripts/pre-commit.sh`, which can stop other agents' hook runs on the shared container.

## Verification

The change scope is `docs`, so an ordinary `bash scripts/pre-commit.sh` is the required run ([local-verification.md](../../.agents/rules/local-verification.md)). Summary line: `All Pre-Commit Hooks Passed! Test scope: docs (18s)`.

Hook output, with two passages abridged: the TAP listing of the ADR structure step, and the repeated per-step "Skipped" lines of step 10, collapsed into one line. All other lines are verbatim.

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
Starting PRD/SRS Traceability Validation...
✅ Traceability Validation passed: 95 PRD requirements, 82 resolved SRS references.
Starting Portable Documentation Validation...
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
# tests 3
# suites 0
# pass 3
# fail 0
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
checked 203 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 164 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 MVP queue and merge driver tests...
Ran 47 tests in 5.400s

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
⏭️  Skipped: Clippy, workspace tests, module crates, rustdoc, desktop steps, ESLint/TypeScript/Vitest/Playwright (docs-only change; set FULL=1 to run)
✅ Lint and test validation passed (test scope: docs).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: docs (18s)
========================================
```
