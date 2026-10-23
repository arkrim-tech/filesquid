# Task 0.8 Walkthrough: Test reporting, capability register, and foundation evidence

Global [Task 0.8](../implementation-plan.md#task-0.8), documentation remainder. Branch `docs/task-0.8-foundation-evidence`. Owner decision 2026-10-23, decision 6 (the owner adopted the orchestrator's recommendations, verbatim: "provide and implement recommendations for owner's decisions 1-6"): the remaining global work is documentation only, with no code changes. The module slice [UI-0.6](../../apps/filesquid/ui/resources/plan.md) (Playwright production suite and Allure) was already done and ticked ([walkthrough](../../apps/filesquid/ui/resources/tasks/Task-UI-0.6-Walkthrough.md)). The server half (BE-0.6) moved to [Task FS-3.10](../implementation-plan.md#task-fs-3.10) ([ADR 035](../architecture/035-macos-first-desktop-mvp.md)). No file inside a crate or npm package changed, so the change scope is `docs`.

## What changed

- **[`resources/capability-register.md`](../capability-register.md)** (new): the item-level F-040 register. Section A has one row for each Photos-baseline acceptance item of F-002 to F-028, under a family summary that is never more supported than its least supported mandatory child. Section B has one row for each remaining F ID, with child rows for F-041, F-044, F-045, F-046 and F-047, F-047 and F-048 **explicitly pending**, and F-050 to F-060 recorded against the FileSquid phases (F0, F1, F2, F3, F3R, F4, F5, C, A, L). Section C covers NFR-001 to NFR-010 and TC-001 to TC-005. Section D is the drive-parity table, with the eleven differences, their reasons and their owning F-IDs, plus the no-star-ratings entry. Section E covers the fixtures. Every row gives a supported, limited or unsupported status, the PRD's F-040 state, the platform and prerequisite, a one-line reason, the delivering task and the evidence. Every unsupported row shares one stated honest state: the file is preserved and exportable as OCFL, viewable as a download, not editable, and the user keeps the external tool they already have as the alternative. The register makes no parity claim, because the photo-manager parity register with named products and versions belongs to ImageSquid's parity-audit gate. Row counts: 17 supported, 53 limited, 200 unsupported.
- **[`resources/foundation-evidence.md`](../foundation-evidence.md)** (new) has seven sections:
  1. Test reporting per workspace (NFR-008, ADR 017).
  2. The NFR-002 test profiles and budgets B-1 to B-8.
  3. The NFR-003 bounds P-1 to P-9.
  4. The RC-002 verification list and the 100-session, 20-transfer pilot, recorded as a proposed test and not a capacity promise.
  5. The performance-profile declarations: macOS desktop first, then `filesquid-server`, with Linux and Windows desktop and the browser host mandatory at maturity. All are undeclared, with evidence `none`.
  6. The TC-003 limits, each unverified until its route lands.
  7. The TC-004 architecture-versus-scaffold record, with each target part labelled hypothetical until it ships.

  Every budget is marked **proposed, not measured**, with measured `none` and its measuring task. The document makes no FileSquid U-11 claim.
- **[`README.md`](../../README.md)**: the About section now links the register and the foundation evidence. The View logs section gains "Journal retention, export and the optional OTLP profile". It maps the ADR 033 pieces to Task FS-2.4 (done), the ticked and open rows of Task FS-2.9, and Task FS-3.6 (open). It states retention (keep everything, no setting yet), export, the OTLP exporter (off by default, store-and-forward, not built yet, never bundled into the desktop app) and the reference Compose `observability` profile, with the two Compose commands that were run below.
- **`scripts/docs/validate-adrs.mjs`** was checked and not changed. It reads `resources/architecture/` (`const adrDir = join(repoRoot, "resources/architecture");`, line 15) and passes.
- **Plans:** Task 0.8 is ticked in [the implementation plan](../implementation-plan.md#task-0.8), with a Status line linking the two documents and this walkthrough. Box 19 is ticked in [the checklist](../implementation-plan-checklist.md), and `python3 scripts/docs/update_mvp_progress.py` redrew the progress block and bar.
- **Plan and ToDos:** `Task-0.8-Plan.md` and `Task-0.8-ToDos.md` were deleted.

Corrections made to the earlier draft register while checking it against the checklist and the code:

- C2PA rows now cite CP-0.1 to CP-0.3, because hard bindings and the JPEG and PNG handlers are ticked.
- The F-061 open items now name DE-0.11 and UI-F2.16.
- The F-046 extraction child is split. EXIF, ID3 and embedded XMP are supported (`extract.rs`). IPTC-IIM and maker fields are unsupported.
- F-042, F-051, F-052, F-056 and NFR-008 moved from supported to limited, so no aggregate row hides an open child (S3 storage, crates.io publication, the published contract, load tests).

No scenario changed. The register and the budgets are documents. The existing F-040, NFR-002, NFR-003 and NFR-008 features already state the register rules that `filesquid-core` implements (CO-0.9).

## SRS mapping

| Requirement (ID and title) | Where it is met |
|---|---|
| SRS-F-040 Compatibility and parity register | [Capability register](../capability-register.md) sections A to E: an item-level row for each Photos-baseline item and each F, NFR and TC ID; supported, limited and unsupported statuses; the drive-parity table; the no-star-ratings entry; internal names, with the item type **file** |
| SRS-NFR-002 Benchmark envelope and responsiveness | [Foundation evidence](../foundation-evidence.md) section 2: the profiles and budgets B-1 to B-8, each marked proposed with measured `none` |
| SRS-NFR-003 Bounded processing and migration endurance | Foundation evidence section 3: the bounds P-1 to P-9, each marked proposed with measured `none` |
| SRS-NFR-008 Maintainability and verified engineering | Foundation evidence section 1: the test-reporting table per workspace (runners, `tests/allure-results/`, clearing, report command); the register's NFR-008 row is limited because no load tests exist yet |
| SRS-TC-003 Platform and provider limits | Foundation evidence section 6; register row TC-003 |
| SRS-TC-004 Decision status and repository alignment | Foundation evidence section 7: architecture versus scaffold, Git not the OCFL library, SHA-256 digest, the later work under T-034, and the ADR validator check |
| SRS-F-061 Activity journal and live health monitoring (README part of ADR 033) | README "Journal retention, export and the optional OTLP profile" |
| PRD RC-002 Performance and resource bounds | Foundation evidence section 4 (proposed verification and pilot) |
| PRD RC-008 Complete parity and documentation traceability | Register: every F, NFR and TC ID, with F-047, F-048 and F-060 pending; no aggregate percentage |

## Mocked environment variables

None.

## Command output (raw)

Run from the repository root with `TMPDIR=/dev/shm/task-0.8`. Absolute paths are replaced by `<repo>`.

### `node scripts/docs/validate-adrs.mjs`

```text
ADR validation passed (34 required files).
```

### `docker compose config --services`

```text
postgres
filesquid-server
```

### `docker compose --profile observability config --services`

```text
victorialogs
victoriametrics
postgres
filesquid-server
```

The Docker daemon is not available in this environment (`docker info` exits 1). Starting the profile stays with Task FS-3.6, and the README does not show a start command.

### `python3 scripts/docs/update_mvp_progress.py` and `--check`

```text
Wrote resources/implementation-plan-checklist.md
Wrote resources/assets/mvp-progress.svg
```

`python3 scripts/docs/update_mvp_progress.py --check` printed nothing and exited 0.

### `bash scripts/pre-commit.sh` (the working tree committed as `b72a7ebe`)

Later Cubic fix commits each passed the same ordinary run (`Test scope: docs`); the final head's run is recorded in the pull request.

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
✅ Traceability Validation passed: 96 PRD requirements, 84 resolved SRS references.
Starting Portable Documentation Validation...
grep: resources/tasks/Task-0.8-Plan.md: No such file or directory
grep: resources/tasks/Task-0.8-ToDos.md: No such file or directory
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
  duration_ms: 12.625295
  type: 'test'
  ...
# Subtest: ADR markdown hrefs reject absolute destinations
ok 2 - ADR markdown hrefs reject absolute destinations
  ---
  duration_ms: 0.338192
  type: 'test'
  ...
# Subtest: FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
ok 3 - FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
  ---
  duration_ms: 1.230514
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
# duration_ms 91.121829
✅ ADR structure (ISO 42010) passed.
🔍 PRD allocation...
FileSquid: 263 vision IDs, 263 allocated, 133 allocation rows
ImageSquid: 29 vision IDs, 29 allocated, 19 allocation rows
OK: every vision ID is allocated
✅ PRD allocation passed.
🔍 Plan traceability...
OK: every requirement is accounted for in a plan task
✅ Plan traceability passed.
🔍 Relative links and anchors...
checked 286 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 173 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 MVP queue and merge driver tests...
......................................................
----------------------------------------------------------------------
Ran 54 tests in 6.763s

OK
✅ MVP queue and merge driver tests passed.
🔍 MVP progress bar...
✅ MVP progress bar passed.
🔍 Task cleanup...
✅ Task cleanup passed.
✅ Documentation validation passed.
Starting Change Scope Validation...
✅ Change scope validation passed (168 assertions).
Starting Lint and Test Validation...
ℹ️  Test scope: docs
⏭️  Skipped: Clippy (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Workspace tests (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-core (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-ocfl (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-metadata (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-c2pa (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-prov (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-api (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Module crate alone: filesquid-native (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: filesquid-metadata all-features tests and wasm32 builds (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: filesquid-prov all-features tests and wasm32 builds (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Rustdoc (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Clippy (desktop, test-instrumented) (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Desktop tests (test-instrumented) (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: ESLint, TypeScript, Vitest and Playwright e2e in apps/filesquid/ui apps/imagesquid (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
⏭️  Skipped: Desktop packaged-window e2e (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
✅ Lint and test validation passed (test scope: docs).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: docs (25s)
========================================
```
