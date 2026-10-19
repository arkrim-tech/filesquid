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
- **Mockup 25.** The Details panel is replaced by the File facts tab, so `25-panel-details.svg` became `25-panel-states.svg`: the shared loading, empty, no-match and error treatment of the four tabs.
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

Pre-commit hook on the branch (`bash scripts/pre-commit.sh`; status lines of the raw output, docs scope, so no Cargo or npm step runs):

```text
✅ Branch Validation passed.
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
✅ filesquid-server uniquely bound to 8083
🚀 VALIDATION PASSED: All modules maintain strict port isolation.
✅ Document Sync Validation passed.
✅ Traceability Validation passed: 95 PRD requirements, 83 resolved SRS references.
✅ Portable Documentation validation passed.
✅ Workspace Boundary validation passed.
✅ Logging rule validation passed (no relevant staged files).
⏭️  Skipped: Rust formatting in ./apps/filesquid (no Rust file changed; test scope: docs)
✅ ./apps/filesquid/ui is properly formatted.
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
✅ ADR structure (ISO 42010) passed.
✅ PRD allocation passed.
✅ Plan traceability passed.
✅ Relative links and anchors passed.
✅ FileSquid and ImageSquid document boundary passed.
✅ MVP queue and merge driver tests passed.
✅ MVP progress bar passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
✅ Change scope validation passed (166 assertions).
ℹ️  Test scope: docs
⏭️  Skipped: ESLint, TypeScript, Vitest and Playwright e2e in apps/filesquid/ui apps/imagesquid (docs-only change: only documentation or always-run checker scripts outside the crates and npm packages changed; set FULL=1 to run)
✅ Lint and test validation passed (test scope: docs).
✅ All Pre-Commit Hooks Passed! Test scope: docs (24s)
```

## Requirement gaps for the SRS writer

These are recorded in the [v2 README notes](../ui-mockups/v2/README.md#notes-for-the-srs-writer) and must be resolved before v2 becomes the active suite:

- [F-055](../prd.md#f-055-default-ui-and-panel-composition) and the MVP release row name Details, Versions and Metadata panels, not the four tabs.
- [F-052](../prd.md#f-052-metadata-module), [ADR 027](../architecture/027-filesquid-metadata-pluggable-stores.md) and [ADR 038](../architecture/038-import-file-record.md) describe `metadata.md` and `file-record.md` as logical files, not `<file-uuid>.<ext>` with `<file-uuid>.metadata.md`.
- A per-file PROV-O view and JSON-LD export have no MVP task; the PROV-O contract is in [F-043](../prd.md#f-043-import-time-c2pa-records-and-action-disclosure).
- [ADR 033](../architecture/033-activity-journal-and-live-health-monitoring.md) has no journal filter by library, object id or file, and exports JSON Lines only (the dialog offers CSV too).

## Sync Execution Report

No PRD, SRS, ADR or plan text was changed: the task was scoped to the mockups and their READMEs, and v2 is a draft that the SRS does not yet cite. The gaps above are listed for the owner. `apps/filesquid/resources/ui-mockups/README.md` and the root `resources/ui-mockups/README.md` list no screens, so they were not changed.
