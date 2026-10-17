# Task ADR-035 Walkthrough: macOS-first desktop MVP

Documentation-only task. TDD-ENFORCE: Not applicable - documentation update.

## C-056: FileSquid MVP scope

Owner decisions (2026-10-17): the first desktop target is macOS; Linux and Windows follow the MVP; the MVP is desktop-only (no `filesquid-server`); C2PA is limited to read, validation and unsigned import records; search, version restore and labels, and drive views, sort and ZIP download stay in; the MVP installer is an unsigned early-access DMG.

Revision (owner, 2026-10-17): libvips media workers (DE-0.5) stay in the MVP inside Task 0.5. An earlier draft had moved them out of the MVP without owner approval; that change is reverted. The owner then put the ADR 011 installer licence and patent review on the MVP path (ADR 035 decision 5; Tasks FS-3.1 and FS-3.9; DE-F3.1; PRD MVP exit gate).

## Changes

- [Product vision](../product-vision.md): new [C-056](../product-vision.md#release-sequence); FE-017, FE-075, U-34, T-011 and C-025 now say macOS first and add the MVP.
- [PRD](../prd.md): new "FileSquid MVP" row in the release gates; release order, F-050, F-057 and the platform statements updated; C-056 allocated.
- [SRS](../srs.md): SRS-F-057 now says macOS first and allows the unsigned early-access installer.
- [ADR 035](../architecture/035-macos-first-desktop-mvp.md) (new); ADRs 002, 006, 013 and 020 and the [architecture](../architecture.md) note the amendment.
- [Implementation plan](../implementation-plan.md): Phase F3 is now the MVP, closed by Task FS-3.9. The new Phase F3R holds the deferred scope and the release-1 gate (Task FS-3.5). New tasks: FS-2.7 and FS-3.9 to FS-3.13.
- Module plans: new rows DE-F3.3 and API-F1.9; DE-F3.1, DE-F3.2 and API-F1.7 re-scoped; section headers updated.
- [README](../../README.md): release column.

## Verification

`bash scripts/pre-commit.sh` exit 0. Raw output (tail):

```text
✅ Document Sync Validation passed.
Starting PRD/SRS Traceability Validation...
✅ Traceability Validation passed: 95 PRD requirements, 81 resolved SRS references.
✅ Portable Documentation validation passed.
✅ Workspace Boundary validation passed.
✅ Logging rule validation passed (no relevant staged files).
✅ ./apps/filesquid is properly formatted.
✅ ./apps/filesquid/ui is properly formatted.
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
✅ ADR structure (ISO 42010) passed.
FileSquid: 259 vision IDs, 259 allocated, 129 allocation rows
ImageSquid: 29 vision IDs, 29 allocated, 19 allocation rows
OK: every vision ID is allocated
✅ PRD allocation passed.
OK: every requirement is accounted for in a plan task
✅ Plan traceability passed.
checked 139 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
checked 146 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
✅ Clippy passed.
  Scenario: Publishing to Bluesky is explicit, policy-checked and journaled
✅ Workspace tests passed.
✅ Module crate alone: filesquid-core passed.
✅ Module crate alone: filesquid-ocfl passed.
✅ Module crate alone: filesquid-metadata passed.
  Scenario: Publishing to Bluesky is explicit, policy-checked and journaled
✅ Module crate alone: filesquid-api passed.
✅ Rustdoc passed.
✅ ESLint passed.
✅ TypeScript passed.
✅ Vitest passed.
✅ ESLint passed.
✅ TypeScript passed.
✅ Vitest passed.
✅ Lint and test validation passed.
✅ All Pre-Commit Hooks Passed!
```

## PR #5 review fixes

Cubic review of PR #5 raised fourteen findings, all fixed. Each one aligned a document with ADR 035's macOS-first, desktop-only MVP:

- **ADRs:** ADR 013 and ADR 020 separate CI build-and-test coverage (Linux and the independent web from the start) from the desktop release order (macOS first). ADR 020's installer bullet names the unsigned early-access macOS DMG for the MVP. ADR 006's loader-coverage assumption names macOS first and states that coverage is unverified on each OS until the DE-0.5 libvips worker tests pass there. The ADR index summary for ADR 020 distinguishes the MVP DMG from later signed installers.
- **PRD:** F-037 has an MVP exception for the unsigned DMG without an updater. F-040's delivery order, RC-005's initial target, the Foundation gate profile and the release-gate note on sequencing are macOS-first; the Foundation gate validates the in-process desktop transport and leaves the HTTP binding to `filesquid-server` after the MVP.
- **Architecture:** the CI/CD target stages mark signed distributables and staged update/rollback verification as post-MVP.
- **Module plans:** phase headings match the global F3/F3R split. UI-F2.10, UI-F2.12 (FS-2.7) and UI-F2.13 (FS-2.6) are marked post-MVP. UI-F3.1 measures only the packaged macOS webview, and the server-served build measurement moves to SV-F3.2 (FS-3.13). UI-F3.2, API-F3.1, CP-0.7, the publish-crate phases (FS-3.3) and SV-F3.3 (FS-3.6) sit under F3R. BE-0.6's status names FS-3.10. The desktop Summary Timeline is 558 hours (550 unchecked).

The ocfl plan's anchor link to its renamed F3R heading was updated.

```text
$ bash scripts/pre-commit.sh
✅ Branch Validation passed.
🔍 Validating ./apps/filesquid/ui (Vite)...
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
🔍 Validating ./apps/imagesquid (Vite)...
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
🔍 Validating filesquid-server (default bind)...
✅ filesquid-server uniquely bound to 8083
✅ Traceability Validation passed: 95 PRD requirements, 81 resolved SRS references.
✅ Workspace Boundary validation passed.
✅ Logging rule validation passed (no relevant staged files).
🔍 Validating Rust formatting in ./apps/filesquid...
✅ ./apps/filesquid is properly formatted.
🔍 Validating Prettier formatting in ./apps/filesquid/ui...
✅ ./apps/filesquid/ui is properly formatted.
🔍 Validating Prettier formatting in ./apps/imagesquid...
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
🔍 ADR structure (ISO 42010)...
✅ ADR structure (ISO 42010) passed.
🔍 PRD allocation...
FileSquid: 259 vision IDs, 259 allocated, 129 allocation rows
ImageSquid: 29 vision IDs, 29 allocated, 19 allocation rows
OK: every vision ID is allocated
✅ PRD allocation passed.
🔍 Plan traceability...
OK: every requirement is accounted for in a plan task
✅ Plan traceability passed.
🔍 Relative links and anchors...
checked 140 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 146 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 Task cleanup...
✅ Task cleanup passed.
✅ Documentation validation passed.
🔍 Clippy (apps/filesquid)...
✅ Clippy passed.
🔍 Workspace tests (apps/filesquid)...
✅ Workspace tests passed.
🔍 Module crate alone: filesquid-core (apps/filesquid)...
✅ Module crate alone: filesquid-core passed.
🔍 Module crate alone: filesquid-ocfl (apps/filesquid)...
✅ Module crate alone: filesquid-ocfl passed.
🔍 Module crate alone: filesquid-metadata (apps/filesquid)...
✅ Module crate alone: filesquid-metadata passed.
🔍 Module crate alone: filesquid-api (apps/filesquid)...
✅ Module crate alone: filesquid-api passed.
🔍 Rustdoc (apps/filesquid)...
✅ Rustdoc passed.
🔍 ESLint (apps/filesquid/ui)...
✅ ESLint passed.
🔍 TypeScript (apps/filesquid/ui)...
✅ TypeScript passed.
🔍 Vitest (apps/filesquid/ui)...
✅ Vitest passed.
🔍 ESLint (apps/imagesquid)...
✅ ESLint passed.
🔍 TypeScript (apps/imagesquid)...
✅ TypeScript passed.
🔍 Vitest (apps/imagesquid)...
✅ Vitest passed.
✅ Lint and test validation passed.
✅ All Pre-Commit Hooks Passed!
```

## PR #5 second review fix

Cubic flagged that moving MD-0.5 to F3R left MD-0.7 (Task FS-1.3, phase F1) depending on post-MVP work. MD-0.5 and global Task 0.16 were completed and merged in PR #4, so `main` was merged into this branch and Task 0.16 returned to F0 as a completed task, depending only on Task 0.14. JSON sidecars stay outside the MVP scope in the PRD. The Summary Timeline moves 8 hours from F3R (432 → 424) to F0 (356 → 364), so the MVP phases now total about 858 hours. Three status lines that named nonexistent phases F1R and F2R now name F3R. `bash scripts/pre-commit.sh` passed.
