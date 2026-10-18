# Task ADR-037 Walkthrough: C2PA moves from the MVP to release 2

Documentation-only task on branch `docs/c2pa-to-release-2`. TDD-ENFORCE: Not applicable - documentation update.

## C-025: Release sequence and C-056: FileSquid MVP scope

Owner decision (2026-10-18): take C2PA out of the FileSquid MVP roadmap and ship it in release 2, to reach the MVP sooner. The MVP shows basic file management on OCFL; the C2PA narrative comes with the ImageSquid release.

Reading used for "release 2": the repository already calls the ImageSquid release "release 2" (phases I1–I7). C2PA therefore also leaves release 1 (phase F3R) and joins the ImageSquid release as FileSquid features behind the API. A new phase C (Content Credentials) sits before phase I1.

## Changes

- [ADR 037](../architecture/037-c2pa-in-release-2.md) (new): no C2PA in the MVP or release 1; the `c2pa` module, its routes and the Content Credentials panel ship with release 2; files imported earlier get backfilled records that state their own creation time and do not claim import time. ADRs [029](../architecture/029-filesquid-unified-product-and-distributions.md) and [035](../architecture/035-macos-first-desktop-mvp.md) and the [ADR index](../architecture/README.md) note the amendment.
- [Product vision](../product-vision.md): C-025 (release 1 ships `ocfl` and `metadata`; `c2pa` ships with ImageSquid; backfilled records), C-056 (MVP scope without C2PA or the Content Credentials panel), distribution table.
- [PRD](../prd.md): executive summary, F-050 release statement, the MVP, FileSquid release and ImageSquid release gate rows, the F-043 prerequisite row and a new F-043 acceptance criterion for backfilled records.
- [SRS](../srs.md): SRS-F-050 release statement.
- [Implementation plan](../implementation-plan.md):
  - New phase C (Content Credentials, release 2) before phase I1, holding Tasks 0.17, 0.18, FS-3.12 and 2.6 (moved, IDs unchanged) and new Tasks FS-C.1 (API-F1.5 and UI-F2.5), FS-C.2 (CP-F3.1) and FS-C.3 (Content Credentials gate, RC-009).
  - MVP: Task FS-1.3 drops API-F1.5 (52 → 42 hours); Task FS-2.2 drops UI-F2.5 (56 → 44 hours); Task FS-3.9 no longer claims RC-009.
  - Release 1: Task FS-3.3 drops CP-F3.1 (24 → 18 hours); Task FS-3.5 drops Task 2.6, FS-3.12 and RC-009.
  - Release 2: Task 1.5 waits on FS-C.1, Task 1.7's RC-009 rows on FS-C.3, Task 2.2 on Task 2.6, Task 8.2 on FS-3.12.
  - Added the missing "Phase F1: FileSquid API" heading (the summary timeline already counted it); module dispatch register, release sequence, dependencies, critical path and summary timeline updated.
- [Checklist](../implementation-plan-checklist.md): C2PA rows moved to a new phase C section; renumbered; counts and anchors updated.
- Module plans: [c2pa](../../apps/filesquid/crates/c2pa/resources/plan.md) (all rows in phase C), [api](../../apps/filesquid/crates/api/resources/plan.md) (API-F1.5 status, backfill job, 10 → 14 hours; `generator:` search operator reports unsupported until phase C), [ui](../../apps/filesquid/ui/resources/plan.md) (UI-F2.5 status). ImageSquid [plan](../../apps/imagesquid/resources/plan.md) and [PRD](../../apps/imagesquid/resources/prd.md) name FileSquid Task FS-C.1 and the C2PA release.
- [v2 mockups README](../ui-mockups/v2/README.md): the Content Credentials panel is a release-2 design.
- [README](../../README.md): C2PA bullet and release column.
- BDD: new backfill scenario in [`importtime_c2pa_records_and_action_disclosure_F-043.feature`](../../apps/filesquid/ui/tests/features/importtime_c2pa_records_and_action_disclosure_F-043.feature).

## Time estimates

| Scope | Before | After |
|---|---|---|
| MVP (phases F0–F3) | 856 hours (the plan said 864; F0 was over-counted by 8) | 758 hours |
| Release 1 (phase F3R) | 424 hours | 338 hours |
| Phase C (release 2) | none | 217 hours |
| All FileSquid phases | 2,560 hours | 2,593 hours |

118 hours leave the MVP, of which 46 (CP-0.1 to CP-0.3; CP-0.3 merged on main while this PR was open) were already done, so open MVP work falls by about 72 hours. Phase C adds 13 hours: the record backfill (+4 on API-F1.5), the `generator:` operator (+1) and the Content Credentials gate (8). The import file record (round 2) adds 20 hours back to the MVP: MD-0.8 (8), DE-1.2 (+6), API-F1.4 (+4, including the upload's file record), UI-F2.4 (+2).

## Sync Execution Report

ADR 037 → vision C-025, C-056 → PRD (summary, F-050, F-043, release gates) → SRS-F-050 → implementation plan, checklist and module plans → ImageSquid plan and PRD → README About. `pr-review-loop.md` "Current milestone" is owner-only and was not edited; its scope (phases F0–F3) now excludes the C2PA tasks automatically.

## Round 2: owner follow-up (2026-10-18)

1. **Import file record instead of a C2PA record.** [ADR 038](../architecture/038-import-file-record.md) (new): every import commits an append-only `file-record.md` in the file's OCFL object, owned by `filesquid-metadata`, with as many source-system and host facts as are available (name, paths, timestamps, ownership, permissions, ACL, flags, link target, content type, every extended attribute with quarantine, where-from and Finder tags decoded, volume, OS, host, account, FileSquid version, import method and job); unavailable facts are listed with a reason; bounded; identity fields scrubbed on export and share. Cascaded into vision C-055, C-056 and C-025, PRD F-002 (new criteria) and F-043 (backfill cites the file record), [architecture](../architecture.md), new module SRS rows MD-S-013 and DE-S-008, new module task MD-0.8, and DE-1.2, API-F1.4 and UI-F2.4; global Tasks FS-1.3, FS-2.2, 1.2 and the MVP gate FS-3.9; the checklist; new F-002 BDD scenarios.
2. **`generator:` removed from the MVP search.** Dropped from API-F2.3 and its BDD examples; PRD F-063, SRS-F-063 and API-S-012 make it part of the grammar only where the `c2pa` module is enabled (otherwise an unknown operator); it returns with API-F1.5 in Task FS-C.1, with BDD scenarios for both cases.
3. **Credentials tab removed from every mockup.** v1 (13, 14, 15, 16, 17, 19, 20: the Activity and Sharing pills move up into the freed slot) and v2 (22–29). The credentials panel mockups v1/18 and v2/30 were deleted. v2 settings, import review and library recovery no longer mention Content Credentials; import review and recovery mention the file record. Both mockup READMEs updated.
4. **C2PA with ImageSquid through the one API.** ADR 037, the vision (C-025), the plan's release sequence and Task FS-C.1 say that `filesquid-c2pa` is added to FileSquid in release 2 and reaches the ImageSquid distribution and UI only through `filesquid-api`; the `<fs-credentials-panel>` is the component the ImageSquid inspector composes. Owner decision (2026-10-18): in release 2 the default FileSquid sidebar also gets the Credentials tab; recorded in ADR 037, Task FS-C.1, UI-F2.5 and the v2 mockups README.

## Verification

An excerpt of the `bash scripts/pre-commit.sh` output (the gate summary lines, copied verbatim from the full run) is appended below; the full output is several thousand lines of test logs.

`bash scripts/pre-commit.sh` exit 0. Excerpt, gate summary lines:

```text
========================================
========================================
✅ Branch Validation passed.
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
✅ filesquid-server uniquely bound to 8083
🚀 VALIDATION PASSED: All modules maintain strict port isolation.
✅ Document Sync Validation passed.
✅ Traceability Validation passed: 95 PRD requirements, 81 resolved SRS references.
✅ Portable Documentation validation passed.
✅ Workspace Boundary validation passed.
✅ Logging rule validation passed (no relevant staged files).
✅ ./apps/filesquid is properly formatted.
✅ ./apps/filesquid/ui is properly formatted.
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
✅ ADR structure (ISO 42010) passed.
FileSquid: 260 vision IDs, 260 allocated, 130 allocation rows
ImageSquid: 29 vision IDs, 29 allocated, 19 allocation rows
OK: every vision ID is allocated
✅ PRD allocation passed.
OK: every requirement is accounted for in a plan task
✅ Plan traceability passed.
checked 160 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
checked 154 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
✅ Clippy passed.
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
77 scenarios (17 passed, 60 skipped)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
32 scenarios (22 passed, 10 skipped)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.73s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.89s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
7 scenarios (7 passed)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
63 scenarios (59 passed, 4 skipped)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
18 scenarios (15 passed, 3 skipped)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.09s
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.15s
42 scenarios (33 passed, 9 skipped)
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.80s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.38s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.78s
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
✅ Workspace tests passed.
63 scenarios (59 passed, 4 skipped)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
✅ Module crate alone: filesquid-core passed.
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.14s
42 scenarios (33 passed, 9 skipped)
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.74s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.56s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
✅ Module crate alone: filesquid-ocfl passed.
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
18 scenarios (15 passed, 3 skipped)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.17s
✅ Module crate alone: filesquid-metadata passed.
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
32 scenarios (22 passed, 10 skipped)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.74s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
✅ Module crate alone: filesquid-c2pa passed.
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
77 scenarios (17 passed, 60 skipped)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
✅ Module crate alone: filesquid-api passed.
✅ Rustdoc passed.
✅ ESLint passed.
✅ TypeScript passed.
✅ Vitest passed.
✅ ESLint passed.
✅ TypeScript passed.
✅ Vitest passed.
✅ Lint and test validation passed.
========================================
✅ All Pre-Commit Hooks Passed!
========================================
```
