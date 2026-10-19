# Task docs-ui-mockups-metadata-tabs: Walkthrough

## Owner requirements of 2026-10-19 (FileSquid MVP): tabbed file panel, PROV-O, audit log and journal views

Documentation-only task (design artifacts). It updates the draft [v2 mockup suite](../ui-mockups/v2/README.md) for the owner's requirements of 2026-10-19. v2 is still a draft and not the active suite, so these files are not yet SRS requirements.

## What changed

### Owner requirements and where each one is drawn

| Requirement | Mockups |
|---|---|
| Files are stored as `<file-uuid>.<ext>` with `<file-uuid>.metadata.md` beside them; the UI always shows the original file name, never the UUID storage name | Every screen. The former "Content location" row (a storage path) is removed from the panel, and "saved to metadata.md" now reads "saved to this file's metadata record". [04](../ui-mockups/v2/04-recover-library.svg) says "metadata records" |
| Four file panel tabs with the same layout: header with the original name, tab strip, property/value table, per-tab filter, copy value, one empty/loading/error treatment, visible keyboard focus | [35](../ui-mockups/v2/35-panel-file-facts.svg), [28](../ui-mockups/v2/28-panel-metadata.svg), [36](../ui-mockups/v2/36-panel-prov-o.svg), [37](../ui-mockups/v2/37-panel-audit-log.svg), states in [25](../ui-mockups/v2/25-panel-states.svg) |
| File facts: original name, extension, format and MIME type, size, SHA-256, import time, source, OCFL object id (`urn:uuid:`) and version | [35](../ui-mockups/v2/35-panel-file-facts.svg); also the panel of the preview screens [22](../ui-mockups/v2/22-preview-image.svg), [22 dark](../ui-mockups/v2/22-preview-image-dark.svg), [23](../ui-mockups/v2/23-preview-video.svg), [24](../ui-mockups/v2/24-preview-unsupported.svg) |
| Descriptive: `dcterms:` and `schema:` properties with prefixes; add, edit, remove | [28](../ui-mockups/v2/28-panel-metadata.svg), [29](../ui-mockups/v2/29-metadata-add-property.svg) |
| PROV-O: history newest first; `prov:Activity`, `prov:startedAtTime` / `prov:endedAtTime` (UTC), `prov:wasAssociatedWith` the owner agent as the installation `urn:uuid:` and the FileSquid software agent and version, `prov:used` / generated entities (object@version, logical path, digest), `prov:wasDerivedFrom`, `prov:wasInvalidatedBy` for deletes; View journal entries; JSON-LD export | [36](../ui-mockups/v2/36-panel-prov-o.svg) |
| Audit log: this file's journal entries newest first, local time, outcome as colour and text, actor, operation id; filters; row detail; link to the PROV-O activity | [37](../ui-mockups/v2/37-panel-audit-log.svg) |
| Journal view from the sidebar: filters (time range, library, operation, outcome, level), search by operation id or object id, live indicator, detail drawer, Export | [38](../ui-mockups/v2/38-journal-view.svg), [38 dark](../ui-mockups/v2/38-journal-view-dark.svg); the Journal item is added to the sidebar of every screen that has one |
| Journal export dialog: scope, format (JSON Lines or CSV), opaque-id note, file name, Export and Cancel | [39](../ui-mockups/v2/39-journal-export.svg) |

### Design decisions

- **Versions.** Five tabs do not fit the 320 px panel at 13 px. The four owner tabs fill the tab strip, and Versions opens from a history button in the panel header; Back returns to the open tab ([26](../ui-mockups/v2/26-panel-versions.svg), [27](../ui-mockups/v2/27-version-restore.svg)). The owner should confirm this placement.
- **Mockup 25.** The former Details panel (`25-panel-details.svg`) became the File facts tab, drawn in `35-panel-file-facts.svg`. Number 25 now holds the new `25-panel-states.svg`, the shared loading, empty, no-match and error treatment of the four tabs (version control records it as a rename because both files carry the number 25).
- **Journal in the sidebar.** The item sits above Deleted files in the sidebar's bottom group, so the library tree keeps its place on every screen.
- **Names in the journal.** ADR 033 stores opaque ids only. The Journal view and the Audit log show original file names by looking the object id up in the library when a row is shown; the detail drawer says so, and exports keep the opaque ids.
- **Technical rows.** Technical values read from the file (for example a video's duration) are listed in File facts below the storage facts.
- **List and preview names.** Every list, grid, search, preview and dialog screen already showed original file names (for example `facade-study.jpg`, `IMG_3990.jpg`); no screen showed a UUID storage name.

## How the mockups were produced

As for v2, a Python generator in the session scratch area (not committed) holds the tokens of the v2 README, a text-width estimate, the icons and the panel components. It rebuilt the side panel of mockups 22 to 29 and drew 25 and 35 to 39, reusing each screen's existing window, sidebar and list markup. A second script added the Journal item to every sidebar. Each changed SVG was rendered with headless Chromium at 1440 × 900 and checked for overlap, clipping and token use, then fixed and rendered again.

## Verification

```text
$ cd resources/ui-mockups/v2 && xmllint --noout *.svg && echo ALL_OK
ALL_OK
```

Pre-commit hook on the head commit after merging `origin/main` (`bash scripts/pre-commit.sh`, complete raw output, absolute paths shortened to `./`; docs scope, so no Cargo or npm step runs):

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
✅ Traceability Validation passed: 95 PRD requirements, 83 resolved SRS references.
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
TAP version 13
# Subtest: stack ADRs exist with ISO 42010 sections and accepted vocabulary
ok 1 - stack ADRs exist with ISO 42010 sections and accepted vocabulary
  ---
  duration_ms: 11.375157
  type: 'test'
  ...
# Subtest: ADR markdown hrefs reject absolute destinations
ok 2 - ADR markdown hrefs reject absolute destinations
  ---
  duration_ms: 0.397882
  type: 'test'
  ...
# Subtest: FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
ok 3 - FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
  ---
  duration_ms: 1.400796
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
# duration_ms 107.795293
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
checked 224 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 165 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 MVP queue and merge driver tests...
...............................................
----------------------------------------------------------------------
Ran 47 tests in 7.910s

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
✅ All Pre-Commit Hooks Passed! Test scope: docs (25s)
========================================
```

## Requirement gaps for the SRS writer

These are recorded in the [v2 README notes](../ui-mockups/v2/README.md#notes-for-the-srs-writer) and must be resolved before v2 becomes the active suite:

- [F-055](../prd.md#f-055-default-ui-and-panel-composition) and the MVP release row name Details, Versions and Metadata panels, not the four tabs.
- [F-052](../prd.md#f-052-metadata-module), [ADR 027](../architecture/027-filesquid-metadata-pluggable-stores.md) and [ADR 038](../architecture/038-import-file-record.md) describe `metadata.md` and `file-record.md` as logical files, not `<file-uuid>.<ext>` with `<file-uuid>.metadata.md`.
- A per-file PROV-O view and JSON-LD export have no MVP task; the PROV-O contract is in [F-043](../prd.md#f-043-import-time-c2pa-records-and-action-disclosure).
- [ADR 033](../architecture/033-activity-journal-and-live-health-monitoring.md) has no journal filter by library, object id or file, and exports JSON Lines only (the dialog offers CSV too).

## Sync Execution Report

No PRD, SRS, ADR or plan text was changed: the task was scoped to the mockups and their READMEs, and v2 is a draft that the SRS does not yet cite. The gaps above are listed for the owner. `apps/filesquid/resources/ui-mockups/README.md` and the root `resources/ui-mockups/README.md` list no screens, so they were not changed.
