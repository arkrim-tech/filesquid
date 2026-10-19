# Task docs-ui-mockups-metadata-tabs: Walkthrough

## Owner requirements of 2026-10-19 (FileSquid MVP): tabbed file panel, provenance, host indicator and Logs view

Documentation-only task (design artifacts). It updates the draft [v2 mockup suite](../ui-mockups/v2/README.md) for the owner's requirements of 2026-10-19. v2 is still a draft and not the active suite, so these files are not yet SRS requirements.

## What changed

### Owner requirements and where each one is drawn

| Requirement | Mockups |
|---|---|
| Files are stored as `<file-uuid>.<ext>` with `<file-uuid>.metadata.md` beside them; the UI always shows the original file name, never the UUID storage name | Every screen. The former "Content location" row (a storage path) is removed from the panel, and "saved to metadata.md" now reads "saved to this file's metadata record". [04](../ui-mockups/v2/04-recover-library.svg) says "metadata records" |
| Three file panel tabs with the same layout (revised from four, see below): header with the original name, tab strip, property/value table, per-tab filter, copy value, one empty/loading/error treatment, visible keyboard focus | [35](../ui-mockups/v2/35-panel-file-info.svg), [28](../ui-mockups/v2/28-panel-metadata.svg), [36](../ui-mockups/v2/36-panel-provenance.svg), states in [25](../ui-mockups/v2/25-panel-states.svg) |
| File info (formerly File facts): original name, extension, format and MIME type, size, SHA-256, import time, source, OCFL object id (`urn:uuid:`) and version | [35](../ui-mockups/v2/35-panel-file-info.svg); also the panel of the preview screens [22](../ui-mockups/v2/22-preview-image.svg), [22 dark](../ui-mockups/v2/22-preview-image-dark.svg), [23](../ui-mockups/v2/23-preview-video.svg), [24](../ui-mockups/v2/24-preview-unsupported.svg) |
| Description (formerly Descriptive): `dcterms:` and `schema:` properties with prefixes; add, edit, remove | [28](../ui-mockups/v2/28-panel-metadata.svg), [29](../ui-mockups/v2/29-metadata-add-property.svg) |
| Provenance (formerly PROV-O; same `prov:` content): history newest first; `prov:Activity`, `prov:startedAtTime` / `prov:endedAtTime` (UTC), `prov:wasAssociatedWith` the owner agent as the installation `urn:uuid:` and the FileSquid software agent and version, `prov:used` / generated entities (object@version, logical path, digest), `prov:wasDerivedFrom`, `prov:wasInvalidatedBy` for deletes; View in Logs; JSON-LD export | [36](../ui-mockups/v2/36-panel-provenance.svg) |
| Logs view (formerly Journal) from the sidebar: the audit log; filters (time range, library, operation, outcome, level), search by file name, operation id or object id, live indicator, detail drawer, Export; an entry of a committed operation links to its PROV-O activities with View provenance (one activity per committed object version, sharing the operation id), and failed and refused entries, which have no activity, are marked journal-only | [38](../ui-mockups/v2/38-logs-view.svg), [38 dark](../ui-mockups/v2/38-logs-view-dark.svg); the Logs item is in the sidebar of every screen that has one |
| Logs export dialog: scope, format (JSON Lines or CSV), opaque-id note, file name, Export and Cancel | [39](../ui-mockups/v2/39-logs-export.svg) |

### Design decisions

- **Versions.** The panel has three tabs (File info, Description, Provenance). Versions is a full view of its own rather than a property table, so it opens from a history button in the panel header instead of a fourth tab; Back returns to the open tab ([26](../ui-mockups/v2/26-panel-versions.svg), [27](../ui-mockups/v2/27-version-restore.svg)). The owner should confirm this placement.
- **Mockup 25.** The former Details panel (`25-panel-details.svg`) became the File info tab, drawn in `35-panel-file-info.svg`. Number 25 now holds the new `25-panel-states.svg`, the shared loading, empty, no-match and error treatment of the tabs (version control records it as a rename because both files carry the number 25).
- **Logs in the sidebar.** The item first sat above Deleted files; the owner moved it below Settings (see the revision below).
- **Names in the journal.** ADR 033 stores opaque ids only. The Logs view shows original file names by looking the object id up in the library when a row is shown; the detail drawer says so, and exports keep the opaque ids.
- **Technical rows.** Technical values read from the file (for example a video's duration) are listed in File info below the storage facts.
- **List and preview names.** Every list, grid, search, preview and dialog screen already showed original file names (for example `facade-study.jpg`, `IMG_3990.jpg`); no screen showed a UUID storage name.

### Owner decisions of 2026-10-19 (revision)

The owner revised the suite on 2026-10-19. Each change applies to every v2 screen where the element appears, light and dark:

| Decision | What changed | Mockups |
|---|---|---|
| Remove "Early access" | The sidebar badge, the welcome badge and the About badge are removed with their pills; the welcome footer reads "Version 0.1.0 · macOS" and About says "This macOS build is not signed or notarised by Apple yet." | Every screen with a sidebar, [01](../ui-mockups/v2/01-welcome.svg) to [03](../ui-mockups/v2/03-open-library.svg), [32](../ui-mockups/v2/32-settings.svg) |
| Remove "This device · only you can read these files" (owner decision 1a) | The header sentence is replaced, in the same place, by a host indicator badge: a device icon and the host label "This device". Its popover states the ADR 032 desktop statement "Your files stay on this device. Only you can read them." and, when backup or sync is on (after the MVP), "Backup copies are encrypted on this device before upload." Settings repeats the statement in a new Privacy section | Every browsing screen; new [40](../ui-mockups/v2/40-host-indicator.svg); [32](../ui-mockups/v2/32-settings.svg) |
| Remove "Saved to this file's metadata record" | The Description tab's footer bar held only this line, so the bar is removed | [28](../ui-mockups/v2/28-panel-metadata.svg), [29](../ui-mockups/v2/29-metadata-add-property.svg), [25](../ui-mockups/v2/25-panel-states.svg) |
| Remove the Audit log tab | Three tabs with the same look and behaviour: File info, Description, Provenance, left-aligned with 24 px between labels. `37-panel-audit-log.svg` is deleted; the file's entries are browsed in Logs. Mockup 25 now shows Description empty and Provenance with no match | [22](../ui-mockups/v2/22-preview-image.svg) to [29](../ui-mockups/v2/29-metadata-add-property.svg), [35](../ui-mockups/v2/35-panel-file-info.svg), [36](../ui-mockups/v2/36-panel-provenance.svg) |
| Rename tab labels | "File facts" is "File info", "Descriptive" is "Description" and "PROV-O" is "Provenance"; the Provenance tab still lists the `prov:` attributes and values | All file panel screens; files `35-panel-file-info.svg` and `36-panel-provenance.svg` |
| Journal becomes Logs, below Settings | Same icon; the item moves to the bottom of the sidebar, directly below Settings, and Deleted files, storage and Settings move up one 30 px row. The full-page view and its export dialog are titled Logs and Export logs; Logs search also matches file names | Every screen with a sidebar; files `38-logs-view.svg`, `38-logs-view-dark.svg`, `39-logs-export.svg` |
| "View in Logs" | Replaces "View journal entries" in the Provenance tab and its error state; it opens Logs filtered to that file and operation. The Logs drawer's link back reads "View provenance" | [36](../ui-mockups/v2/36-panel-provenance.svg), [25](../ui-mockups/v2/25-panel-states.svg), [38](../ui-mockups/v2/38-logs-view.svg), [39](../ui-mockups/v2/39-logs-export.svg) |

The welcome screen keeps its own privacy statement ("Who can read your files: only you, on this device."), which is a different sentence from the removed header text. The Monitor keeps its "Activity journal" section, the live tail of the same entries.

## How the mockups were produced

As for v2, a Python generator in the session scratch area (not committed) holds the tokens of the v2 README, a text-width estimate, the icons and the panel components. It rebuilt the side panel of mockups 22 to 29 and drew 25 and 35 to 39, reusing each screen's existing window, sidebar and list markup. A second script added the Journal item to every sidebar. For the revision, further scripts in the session scratch area (not committed) removed the early-access, privacy-sentence and metadata-record elements, rebuilt the tab strip with three labels, moved Logs below Settings, added the host indicator badge, redrew the Empty and No-match panes of 25 and the Privacy and About sections of 32, and drew 40. Each changed SVG was rendered with headless Chromium at 1440 × 900 and checked for overlap, clipping and token use, then fixed and rendered again.

## Verification

Revision of 2026-10-19 (three tabs, Logs below Settings, host indicator, label removals):

```text
$ cd resources/ui-mockups/v2 && xmllint --noout *.svg && echo ALL_OK
ALL_OK
```

Render check: every v2 SVG loaded in headless Chromium at 1440 × 900, screenshotted, and its text boxes tested for leaving the canvas and for overlap. The overlap count is a naive test on untransformed text boxes, so it also counts text under an open menu, a drop overlay or a dialog's scrim (for example mockups 06, 18, 26, 29 and 39) and the four translated panes of board 25; each such screen was checked visually, and no new overlap or clipping remains. No screen has text off the canvas, and there were no console errors:

```text
01-welcome.svg: offcanvas=0 overlaps=0
02-create-library.svg: offcanvas=0 overlaps=6
03-open-library.svg: offcanvas=0 overlaps=7
04-recover-library.svg: offcanvas=0 overlaps=0
05-all-files-list-dark.svg: offcanvas=0 overlaps=0
05-all-files-list.svg: offcanvas=0 overlaps=0
06-all-files-grid.svg: offcanvas=0 overlaps=1
07-empty-library.svg: offcanvas=0 overlaps=0
08-library-menu.svg: offcanvas=0 overlaps=5
09-copy-library.svg: offcanvas=0 overlaps=6
10-new-and-item-menus.svg: offcanvas=0 overlaps=7
11-selection-bulk-actions.svg: offcanvas=0 overlaps=0
12-zip-download.svg: offcanvas=0 overlaps=5
13-move-copy-picker.svg: offcanvas=0 overlaps=4
14-rename-and-delete.svg: offcanvas=0 overlaps=0
15-recent-view.svg: offcanvas=0 overlaps=0
16-deleted-files.svg: offcanvas=0 overlaps=0
17-import-drag-drop.svg: offcanvas=0 overlaps=1
18-import-review.svg: offcanvas=0 overlaps=13
19-import-progress.svg: offcanvas=0 overlaps=7
20-search-suggestions.svg: offcanvas=0 overlaps=14
21-search-results.svg: offcanvas=0 overlaps=0
22-preview-image-dark.svg: offcanvas=0 overlaps=0
22-preview-image.svg: offcanvas=0 overlaps=0
23-preview-video.svg: offcanvas=0 overlaps=0
24-preview-unsupported.svg: offcanvas=0 overlaps=0
25-panel-states.svg: offcanvas=0 overlaps=51
26-panel-versions.svg: offcanvas=0 overlaps=1
27-version-restore.svg: offcanvas=0 overlaps=1
28-panel-metadata.svg: offcanvas=0 overlaps=0
29-metadata-add-property.svg: offcanvas=0 overlaps=8
31-monitor-dark.svg: offcanvas=0 overlaps=0
31-monitor.svg: offcanvas=0 overlaps=0
32-settings.svg: offcanvas=0 overlaps=0
33-state-loading.svg: offcanvas=0 overlaps=0
34-state-storage-unavailable.svg: offcanvas=0 overlaps=0
35-panel-file-info.svg: offcanvas=0 overlaps=0
36-panel-provenance.svg: offcanvas=0 overlaps=0
38-logs-view-dark.svg: offcanvas=0 overlaps=0
38-logs-view.svg: offcanvas=0 overlaps=0
39-logs-export.svg: offcanvas=0 overlaps=22
40-host-indicator.svg: offcanvas=0 overlaps=0
console errors: 0
```

Pre-commit hook on the working tree of the revision, after merging `origin/main` (`bash scripts/pre-commit.sh`, complete raw output; docs scope, so no Cargo or npm step runs):

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
  duration_ms: 13.86387
  type: 'test'
  ...
# Subtest: ADR markdown hrefs reject absolute destinations
ok 2 - ADR markdown hrefs reject absolute destinations
  ---
  duration_ms: 0.349164
  type: 'test'
  ...
# Subtest: FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
ok 3 - FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
  ---
  duration_ms: 1.643256
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
# duration_ms 124.814065
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
checked 237 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 169 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 MVP queue and merge driver tests...
...............................................
----------------------------------------------------------------------
Ran 47 tests in 7.076s

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
✅ All Pre-Commit Hooks Passed! Test scope: docs (24s)
========================================
```

## Requirement gaps for the SRS writer

These are recorded in the [v2 README notes](../ui-mockups/v2/README.md#notes-for-the-srs-writer) and must be resolved before v2 becomes the active suite:

- [F-055](../prd.md#f-055-default-ui-and-panel-composition), the default UI SRS (UI-S-019 to UI-S-022) and the UI plan task UI-F2.15 still describe four tabs (File facts, Descriptive, PROV-O, Audit log) and a Journal tab reachable from the app header; the owner's revision shows three tabs (File info, Description, Provenance) and Logs below Settings in the sidebar.
- DE-F3.1 in the desktop plan requires the About view to say "early access" and a packaged-window test to assert that label; the owner removed the label from the UI.
- [F-052](../prd.md#f-052-metadata-module), [ADR 027](../architecture/027-filesquid-metadata-pluggable-stores.md) and [ADR 038](../architecture/038-import-file-record.md) describe `metadata.md` and `file-record.md` as logical files, not `<file-uuid>.<ext>` with `<file-uuid>.metadata.md`.
- Resolved on `main` since the first round: the per-file PROV-O history and its JSON-LD export are now [F-067](../prd.md#f-067-prov-o-history-of-every-version-and-change), delivered by API-F2.9 and UI-F2.15.
- [ADR 033](../architecture/033-activity-journal-and-live-health-monitoring.md) has no journal filter by library, object id or file, and exports JSON Lines only (the Logs export dialog offers CSV too).

## Sync Execution Report

No requirement text in the PRD, SRS, ADRs or plans was changed: the task was scoped to the mockups and their READMEs, and v2 is a draft. The one PRD edit is in [F-055](../prd.md#f-055-default-ui-and-panel-composition): its mockup file list now names the renamed files (`35-panel-file-info.svg`, `36-panel-provenance.svg`, `38-logs-view.svg`, `38-logs-view-dark.svg`, `39-logs-export.svg`), drops the deleted `37-panel-audit-log.svg`, and says the 2026-10-19 owner decisions are not yet reflected in the requirement. The same file names are cited as plain text in the default UI SRS (UI-S-022) and the UI plan (UI-F2.15); those files belong to the `apps/filesquid/ui` package, so editing them would leave the docs-only scope of this change, and they are listed with the other gaps above for the owner. `apps/filesquid/resources/ui-mockups/README.md` and the root `resources/ui-mockups/README.md` list no screens, so they were not changed.
