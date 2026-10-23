# FileSquid capability register (F-040)

The item-level register that [PRD F-040](./prd.md#f-040-compatibility-and-parity-register) and [SRS-F-040](./srs.md#srs-f-040-compatibility-and-parity-register) require: every Photos-baseline item, every F, NFR and TC requirement of the [PRD](./prd.md), the drive-parity table and the unavailable-by-decision entries, each with a status, a one-line reason, the task or pull request that delivers it and the evidence that exists today. It is maintained by hand at every gate task ([RC-008](./prd.md#rc-008-complete-parity-and-documentation-traceability)); the types that a running FileSquid uses to publish the same facts live in `filesquid-core` (CO-0.9, [CORE-S-009](../apps/filesquid/crates/core/resources/srs.md), [`capability_register_F-040.feature`](../apps/filesquid/crates/core/tests/features/capability_register_F-040.feature)).

**Snapshot:** `main` at the merge-base of [Task 0.8](./implementation-plan.md#task-0.8) (2026-10-23). The ticks in the [implementation plan checklist](./implementation-plan-checklist.md) are the source for every "delivered" statement; a row is never marked beyond what a ticked task's walkthrough shows.

**This register makes no parity claim.** No product or version has been compared against ([F-040](./prd.md#f-040-compatibility-and-parity-register) requires that a parity claim name them); the photo-manager parity register, with the products and versions it compares against, belongs to the ImageSquid distribution (ImageSquid C-036, C-037) and is pending its parity-audit gate (g). FileSquid has not been measured against NFR-002 or NFR-003 and makes no [FileSquid U-11](./product-vision.md#platform-ownership-and-access) claim; the proposed budgets are in [foundation evidence](./foundation-evidence.md).

## How to read a row

| Column | Meaning |
|---|---|
| Status | **supported**: the outcome exists on `main`, is reachable through the FileSquid API or the `fsquid` CLI and is covered by tests the pre-commit hook runs. **limited**: part of the outcome exists; the row says which part. **unsupported**: nothing of the outcome exists on `main`, or the owner decided against it. |
| F-040 state | The PRD vocabulary: **equivalent**, **adapter equivalent**, **limited**, **preservation-only** (the bytes are kept and exportable, nothing interprets or edits them; never editable parity), **unavailable** (pending a task, or by decision). |
| Platform and prerequisite | Where the outcome has been exercised and what it needs. Today every supported row was exercised on the desktop host and `filesquid-server` **built from source** on Linux (the hook's container) and macOS (developer machines), on Chromium through Playwright and on WebKitGTK through the desktop WebDriver suite. No packaged installer exists (Task FS-3.1), Windows is untested, and there is no browser host. |
| Delivered by | The implementation-plan task (ticked = done; open = pending) or the merged pull request. |
| Evidence | Walkthrough, feature file or README section for a supported or limited row; "none" for an unsupported row. |

Unsupported rows share one honest state unless the row says otherwise: once a file is imported, its bytes are **preserved** and **exportable** byte-exact as an OCFL object ([F-042](./prd.md#f-042-ocfl-preservation-and-portable-recovery), Task 1.1, Task FS-1.2); it is **viewable** only as a download, or in the default UI's preview when the browser engine renders the format; it is **not editable** in FileSquid; the **safe alternative** is the external tool the user already has, until the delivering task lands. Declaring a required capability's whole supported set empty cannot satisfy it; such rows are unavailable, not "supported with no formats".

Internal names are used throughout (repository, library, collection, virtual collection, exhibit); the item type is **file** ([F-049](./prd.md#f-049-product-component-terminology-and-item-type)). A distribution shows the same rows in its own words.

## A. Photos baseline (F-002 to F-028)

The Photos baseline is the photo-manager feature inventory the PRD carries as F-002 to F-028. Every child item below is an acceptance item of its F block. Family rows are summaries only; a family is never more supported than its least supported mandatory child.

### F-002: Safe file, folder and device import

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | limited | limited | Desktop and server from source | Single-file upload through the API with an import record exists; folder import, device adapters, watched folders and per-file job progress do not | [Task 1.2](./implementation-plan.md#task-1.2) | below |
| Upload a file through the API into a library | supported | equivalent | Both hosts; any file type | `POST` upload commits the bytes as an OCFL object with the stored name `<file-uuid>.<ext>` (the extension is kept only when it is 1 to 16 ASCII letters or digits; otherwise the stored original is `<file-uuid>` with no suffix) and keeps the original name for display and download | [Task FS-1.2](./implementation-plan.md#task-fs-1.2), [Task FS-1.3](./implementation-plan.md#task-fs-1.3) (API-F1.13) | [Task FS-1.2 row](./implementation-plan.md#task-fs-1.2); README "Run tests" |
| Import record (`import` section of `<file-uuid>.metadata.md`) in the same OCFL version | supported | equivalent | Both hosts | Written by the metadata module on every upload; cited version checked inside the commit | [Task FS-1.3](./implementation-plan.md#task-fs-1.3) (MD-0.8, MD-0.10) | [Task FS-1.3 row](./implementation-plan.md#task-fs-1.3) |
| Every imported file automatically **tracked** (OCFL object) and **described** (metadata record) without the `c2pa` module | supported | equivalent | Both hosts | OCFL commit plus metadata record on upload | Task FS-1.2, Task FS-1.3 | same |
| Desktop import of selected or all-new files and folders, drag-and-drop, mounted camera cards | limited | limited | Desktop from source | The desktop import job worker row DE-1.2 is ticked; the review step, per-file progress, library facts and the UI are open | [Task 1.2](./implementation-plan.md#task-1.2) (DE-1.7, DE-1.8, API-F2.16, UI-1.2 open) | DE-1.2 tick in the [checklist](./implementation-plan-checklist.md) |
| Import review (proposed, previously imported, destination, counts, capacity) | unsupported | unavailable | — | Pending | Task 1.2 (API-F2.16, UI-1.2) | none |
| Managed copy with hash verification; explicit referenced import | limited | limited | Both hosts | Managed upload hashes and verifies; referenced import (record existing bytes in place) does not exist | Task 1.2; referenced mode [Task 2.1](./implementation-plan.md#task-2.1) | Task FS-1.2 row |
| Progress, pause, resume, cancel, per-item errors and retry that survive restart; progress per file in the default UI | unsupported | unavailable | — | Durable job routes exist (API-F1.6) but no import job is driven through them yet | Task 1.2 (API-F2.16, DE-1.8, UI-1.2) | none |
| Camera and phone device adapters | unsupported | unavailable | Platform adapter per OS | Pending; adapters are FileSquid modules the ImageSquid distribution enables | [Task 8.1](./implementation-plan.md#task-8.1) | none |
| Watched folders | unsupported | unavailable | — | Pending | Task 2.1 | none |
| Browser and mobile selected uploads that resume | unsupported | unavailable | Encrypted browser host | No browser host exists | [Task 1.6](./implementation-plan.md#task-1.6), [Task 1.8](./implementation-plan.md#task-1.8) | none |
| Checkpointed batches that reuse verified completed resources after interruption | unsupported | unavailable | — | Pending | Task 1.2 | none |
| Bounded background scheduling under NFR-003; incremental usability of verified items | unsupported | unavailable | — | Pending; the proposed bounds are in [foundation evidence](./foundation-evidence.md) | Task 1.2, [Task 1.3](./implementation-plan.md#task-1.3) | none |

### F-003: Browsing, selection and viewing

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | limited | limited | Desktop and server from source | Folder listing and a file browser with preview exist; derivatives, date views, caches and prefetch do not | [Task FS-2.2](./implementation-plan.md#task-fs-2.2), [Task FS-2.5](./implementation-plan.md#task-fs-2.5), [Task 1.3](./implementation-plan.md#task-1.3) | below |
| Paged, bounded browse by collection and over all files with stable keys and ordering | limited | limited | Both hosts | Folder listing through the API (FS-1.2); the view operation with sort keys, and the year, month and day views, are open | Task FS-1.2; Task FS-2.5 (API-F2.2) | Task FS-1.2 row |
| Browse by virtual collection | unsupported | unavailable | — | No virtual collections yet | [Task 1.4](./implementation-plan.md#task-1.4), Task FS-2.5 (API-F2.3) | none |
| Progressive thumbnails and display derivatives from the media module | unsupported | unavailable | libvips adapter ([ADR 006](./architecture/006-libvips-native-stills-not-imagemagick.md)) | No media module on `main` | Task 1.3 | none |
| Ranged playback of video, audio and motion resources | limited | limited | Both hosts; browser-decodable formats only | Originals are served by ranged reads (the packaged-window `Range` read is the first native-window test, [ADR 017](./architecture/017-test-runners-and-reporting.md)); no transcoding, no HDR detection or SDR fallback | Task FS-1.2; HDR [Task 7.4](./implementation-plan.md#task-7.4) | [ADR 017](./architecture/017-test-runners-and-reporting.md) amendment |
| HDR capability detection with an identified SDR fallback | unsupported | unavailable | Per webview | Pending | Task 7.4 | none |
| Selection as items, ranges, non-contiguous sets or the whole frozen result; batch operations over it | unsupported | unavailable | — | Pending with the view operation | Task FS-2.5 (API-F2.2, UI-F2.8) | none |
| Display exclusions as query filters | unsupported | unavailable | — | Pending with search | Task FS-2.5 (API-F2.3) | none |
| Bounded prefetch of derivatives around the current view, deduplicated | unsupported | unavailable | — | No derivatives | Task 1.3 | none |
| Versioned thumbnail, display and view caches with finite budgets | unsupported | unavailable | — | No derivatives; the proposed budgets are in foundation evidence | Task 1.3 | none |
| Default UI list and grid views, preview, adjacent-item navigation | limited | limited | Chromium (Playwright), WebKitGTK (desktop suite) | `<fs-file-browser>` with list and grid, preview, upload, new folder, rename or move, remove and restore (UI-F2.2 ticked); adjustable thumbnail size, sorting and the selection forms are open | Task FS-2.2 (UI-F2.2); Task FS-2.5 (UI-F2.8) | UI-F2.2 tick; [`@filesquid/ui` README](../apps/filesquid/ui/README.md) |
| Virtualised lists and grids keep focus, selection and accessible semantics | limited | limited | Same | The shell's keyboard, naming and focus contracts are tested (UI-S-016); virtualisation under large folders is not measured | [Task FS-2.1](./implementation-plan.md#task-fs-2.1); measurement [Task FS-3.4](./implementation-plan.md#task-fs-3.4) | [UI-0.6 walkthrough](../apps/filesquid/ui/resources/tasks/Task-UI-0.6-Walkthrough.md) |
| Honest missing, pending and failed preview states with a recovery path | limited | limited | Same | The five view states (empty, loading, error, offline, permission) are distinct and tested; per-derivative states wait for derivatives | Task FS-2.1; Task 1.3 | UI-0.6 walkthrough |

### F-004: Collections, folders and navigation

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | limited | limited | Both hosts from source | Folders (collections) are created, renamed, moved, removed and restored through the API and shown in the navigation tree; from-selection, duplicate, merge, manual order, covers and virtual collections do not exist | Task FS-1.2, Task FS-2.1; [Task 1.4](./implementation-plan.md#task-1.4) | below |
| Create empty collections, rename, move, delete | supported | equivalent | Both hosts | Folder routes of the API | Task FS-1.2 | Task FS-1.2 row |
| Create from selection, populate by drag, duplicate, merge | unsupported | unavailable | — | Pending | Task 1.4; Task FS-2.5 (UI-F2.8) | none |
| Deleting a collection marks its files recoverably deleted | limited | limited | Both hosts | Remove and restore exist; retention, expiry and the Recently deleted filter are open | Task FS-1.2; Task FS-2.5 (API-F2.15) | Task FS-1.2 row |
| Numbered names on logical-path collisions | limited | limited | Both hosts | Applied on upload and rename within the file routes; copy and move into home collections are open | Task FS-1.2; Task FS-2.5 (API-F2.14) | Task FS-1.2 row |
| Manual order, title and date sort, cover selection | unsupported | unavailable | — | Pending | Task 1.4; Task FS-2.5 (API-F2.2) | none |
| Rule-based and hand-picked virtual collections | unsupported | unavailable | — | Pending | Task 1.4 | none |
| Every organisation command through the API | limited | limited | Both hosts | Folder commands only | Task FS-1.2; Task 1.4 | Task FS-1.2 row |
| Navigation tree by kind with internal names; Recently deleted as the final row | limited | limited | Chromium, WebKitGTK | The tree, its sections, pins and states are tested (UI-F2.1a); the Recently deleted row waits for API-F2.15 | Task FS-2.1; Task FS-2.5 | UI-0.6 walkthrough (navigation specs) |
| Folder-on-top placement and sort preferences persist | unsupported | unavailable | — | Pending | Task FS-2.5 (UI-F2.8) | none |

### F-005: Favorites, metadata and keywords

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | limited | limited | Both hosts from source | Descriptive statements are read and written through the API and `fsquid`; favorites, the Keyword Manager, date and GPS correction and the default UI's metadata panel are open | Task FS-1.3; Task 1.4; Task FS-2.2 (UI-F2.4) | below |
| Favorites | unsupported | unavailable | — | Pending | Task 1.4 | none |
| Titles, captions, keywords and accessibility descriptions as `dcterms:` / `schema:` statements, single and batch | limited | limited | Both hosts; `fsquid` | Property routes (API-F1.4) and `fsquid set` write and validate statements; the default UI's `<fs-metadata-panel>` (UI-F2.4) is open, so there is no UI editing yet | Task FS-1.3; Task FS-2.2 | README "View logs" (`fsquid set dcterms:title`) |
| Keyword Manager (create, rename, delete, organise, shortcuts) | unsupported | unavailable | — | Pending; ImageSquid presents it | Task 1.4 | none |
| Information panel: filename, format, dimensions, size, camera, lens, exposure | limited | limited | Both hosts | Extraction exists in the metadata module (Task 0.15) and the record is readable through the API; no UI panel yet | Task 0.15, Task FS-1.3; Task FS-2.2 (UI-F2.3, UI-F2.4) | [Task 0.15 row](./implementation-plan.md#task-0.15) |
| Date, time, time-zone and GPS assignment, correction, copy, removal, restoration with provenance | unsupported | unavailable | — | Pending | [Task 2.2](./implementation-plan.md#task-2.2) | none |
| **Star ratings** | unsupported | unavailable **by decision** | — | Favorites and keywords are the classification model; an imported `xmp:Rating` stays in the preserved original bytes (the XMP extractor does not read it into the metadata record yet; F-046 work) and is not owner-editable | owner decision recorded in [F-040](./prd.md#f-040-compatibility-and-parity-register) | PRD F-040 |
| Private EXIF and native metadata inspection and versioned corrections (F-046) | limited | limited | Both hosts | See the F-046 rows in section B | Task FS-1.3; [Task 1.5](./implementation-plan.md#task-1.5) | section B |

### F-006: Virtual collections and metadata search

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | — | No search operation on `main` | Task FS-2.5 (API-F2.3, UI-F2.9); Task 1.4; Task 2.2 | none |
| Search over filename, `dcterms:` fields, dates and location with suggestions | unsupported | unavailable | — | Pending | Task FS-2.5 (API-F2.3) | none |
| Rule-based virtual collections with Any/All rules | unsupported | unavailable | — | Pending | Task 1.4 | none |
| Named definitions kept with collections; membership updates; saved views in the browser | unsupported | unavailable | — | Pending | Task 1.4, Task 2.2 | none |
| Quick filters (photos, videos, favorites, edited, screenshots, keywords, recently deleted) | unsupported | unavailable | — | Pending | Task FS-2.5 (API-F2.3, API-F2.15) | none |
| Namespaced field presence and value matching | unsupported | unavailable | — | Pending | Task FS-2.5 (API-F2.3) | none |
| Bounded paged search with stable ordering and invalidation | unsupported | unavailable | — | Pending; bounds proposed in foundation evidence | Task FS-2.5 | none |

### F-007: Automatic and activity collections

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | — | Pending | Task 1.4; Task 2.2 | none |
| Activity collections (imports, recently added, edited, viewed, shared, favorites, received) | unsupported | unavailable | — | Pending; the journal that will feed them exists (F-061) | Task 1.4 | none |
| Media-type groupings for every identified kind | unsupported | unavailable | — | Pending | Task 1.4 | none |
| Time-based and identity-based groupings with highlights and All | unsupported | unavailable | — | Pending | [Task 4.1](./implementation-plan.md#task-4.1) | none |
| Classification collections with corrections and progress | unsupported | unavailable | — | Pending | Task 4.1, [Task 4.2](./implementation-plan.md#task-4.2) | none |
| Third-party messaging or platform ingestion | unsupported | unavailable | Adapter-dependent | No private platform API presumed | Task 8.1 | none |
| Default UI lists activity collections and filters by media kind | unsupported | unavailable | — | Pending | Task 1.4 | none |

### F-008: Deletion, hidden files and duplicate review

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | limited | limited | Both hosts from source | Remove and restore exist; retention, expiry, hidden files and duplicate review do not | Task FS-1.2; Task FS-2.5 (API-F2.15); Task 1.4; Task 2.1 | below |
| Recoverable deletion with the 30-day installation default and per-library override | limited | limited | Both hosts | Files enter the deleted state and are restored by the file routes; retention, expiry and the expiry setting are open | Task FS-1.2; Task FS-2.5 (API-F2.15) | Task FS-1.2 row |
| Restore to the original OCFL object and logical path | supported | equivalent | Both hosts | Restore route | Task FS-1.2 | Task FS-1.2 row |
| Hidden files with local authentication for the Hidden view | unsupported | unavailable | — | Pending | Task 1.4 | none |
| Exact-byte and perceptual duplicate review with merge choices | unsupported | unavailable | — | Pending | Task 1.4, Task 2.1 | none |
| Merging preserves relationships and keeps removed items recoverable | unsupported | unavailable | — | Pending | Task 2.1 | none |
| Recovered and orphaned media review | unsupported | unavailable | — | Pending | Task 2.1 | none |
| Views identify files by display name, never stored name | supported | equivalent | Both hosts | Display and download names (API-F1.13) | Task FS-1.3 | Task FS-1.3 row |
| Recently deleted filter and the `is:deleted` query | unsupported | unavailable | — | Pending | Task FS-2.5 (API-F2.15, API-F2.3) | none |
| Expiry setting: keep version history (default) or remove the object | unsupported | unavailable | — | Pending | Task FS-2.5 (API-F2.15) | none |

### F-009: Multiple libraries and filesystem administration

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | limited | limited | Desktop from source | Owner-folder libraries are created, opened, copied and recovered; referenced originals, external volumes, network-filesystem checks and library facts are open | [Task 1.1](./implementation-plan.md#task-1.1); Task 1.2 (DE-1.7, API-F2.18); Task 2.1 | below |
| Several libraries per owner; create and open a library in an owner-chosen folder | supported | equivalent | Desktop; `fsquid` | Task 1.1 (CL-1.1, API-F1.12, DE-1.1, UI-1.1) under [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md) | Task 1.1 | [Task 1.1 row](./implementation-plan.md#task-1.1) |
| Managed and referenced modes explicit; reveal, missing-file detection, relink, consolidate | unsupported | unavailable | — | Managed only | Task 2.1 | none |
| Stable identification of external volumes; missing drives pause work | unsupported | unavailable | — | Pending | Task 2.1 | none |
| Live catalog refused on unsupported network filesystems | unsupported | unavailable | — | Pending | Task 2.1 | none |
| Catalog repair, schema migration, thumbnail rebuild, recovery with a verified pre-operation backup | limited | limited | Desktop; `fsquid` | Rebuild-from-files recovers the catalog, the PROV-O index and the journal from OCFL (CL-1.1, CL-1.9); the rebuild as a durable job and thumbnail rebuild are open | Task 1.1; Task 1.2 (API-F2.17); Task 1.3 | Task 1.1 row |
| Library name, free space and recently opened libraries, without a path | unsupported | unavailable | — | Pending | Task 1.2 (API-F2.18, UI-1.3) | none |
| Documented canonical local layout; app-closed filesystem copy | supported | equivalent | Desktop; any OS file tools | ADR 041 layout; ordinary copy and recover tested (IO-1.1, IO-1.4) | Task 1.1 | Task 1.1 row; README "Run the application" |

### F-010: Export and portable backup

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | limited | limited | Both hosts from source | Original single-file download with the original name exists; ZIP, rendered export, batch naming, portable backup and scrubbed exports are open | Task FS-1.3; Task FS-2.5 (API-F2.2); Task 1.5 | below |
| Original export reproduces verified source bytes | supported | equivalent | Both hosts | Download route returns the stored bytes under the display name | Task FS-1.2, Task FS-1.3 (API-F1.13) | Task FS-1.3 row |
| Rendered export: JPEG, PNG, TIFF 16-bit, HEIC where licensed, with controls | unsupported | unavailable | libvips adapter; HEIC licence review ([ADR 011](./architecture/011-native-media-adapters-and-ffmpeg-license.md)) | No render worker | [Task 3.1](./implementation-plan.md#task-3.1) | none |
| Batch export naming (FileId, source, title, sequential, prefixes) | unsupported | unavailable | — | Pending | Task 1.5 | none |
| Video export up to 4K; drag-to-file-manager copies | unsupported | unavailable | FFmpeg worker (ADR 011) | Pending | Task 7.4 | none |
| Streamed ZIP download of a selection | unsupported | unavailable | — | Pending | Task FS-2.5 (API-F2.2) | none |
| Portable backup with catalog, recipe and relationship snapshot and manifest | unsupported | unavailable | — | Pending; the OCFL copy of a library is already a complete portable copy (F-042) | Task 1.5; [Task 5.2](./implementation-plan.md#task-5.2) | none |
| Sharing and publication exports apply the F-046 scrubbing policy and outgoing name policy | limited | preservation-only | Metadata module | Scrub classes including `OriginalName` exist in the metadata module (MD-0.10); no sharing route applies them yet | Task FS-1.3; Task 1.5; [Task FS-2.7](./implementation-plan.md#task-fs-2.7) | Task FS-1.3 row |
| PROV-O activity for every export, ZIP and share output | unsupported | unavailable | — | Pending | [Task FS-2.8](./implementation-plan.md#task-fs-2.8) | none |

### F-011: Non-destructive editing workflow

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | — | No recipe model or render worker on `main`; originals are preserved, so nothing is lost by waiting | Task 3.1 | none |
| Versioned recipes: undo, redo, revert, before/after, duplicate versions, per-adjustment toggles | unsupported | unavailable | — | Pending | Task 3.1 | none |
| Copy a recipe to many files with incompatibility reports | unsupported | unavailable | — | Pending | Task 3.1 | none |
| Retained recipe and engine versions; no silent appearance change | unsupported | unavailable | — | Pending | Task 3.1 | none |
| Every renderer checked against one reference corpus | unsupported | unavailable | — | Pending | Task 3.1 | none |
| Edits participate in F-043 action recording | unsupported | unavailable | `c2pa` module (release 2) | Pending | [Task 2.6](./implementation-plan.md#task-2.6) | none |
| Default UI Versions panel shows edit versions and reverts | unsupported | unavailable | — | The Versions panel (UI-F2.3) is open; OCFL version restore is a separate item ([F-065](./prd.md#f-065-file-version-history-and-retention)) | Task FS-2.2 (UI-F2.3); Task 3.1 | none |

### F-012: Composition and tonal/color editing

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | — | Pending | Task 3.1 | none |
| Crop, rotate, flip, straighten, perspective, auto-enhance, light, colour, black and white, curves, levels | unsupported | unavailable | — | Pending | Task 3.1 | none |
| Selective colour with at least six sampled colours | unsupported | unavailable | — | Pending | Task 3.1 | none |
| Live histograms; identical preview and export | unsupported | unavailable | — | Pending | Task 3.1 | none |
| Every parameter readable and writable through the API | unsupported | unavailable | — | Pending | Task 3.1 | none |
| Default UI shows whether a file has a recipe | unsupported | unavailable | — | Pending | Task 3.1 | none |

### F-013: Detail, filters, retouch and RAW

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | RAW decoder adapter (ADR 011) | Pending; RAW originals are preserved byte-exact once imported | Task 3.1 | none |
| Definition, noise reduction, sharpening, vignette, adjustable filters | unsupported | unavailable | — | Pending | Task 3.1 | none |
| Retouch steps with history; red-eye on stills, refused on video | unsupported | unavailable | — | Pending | Task 3.1 | none |
| RAW and ProRAW preserved, developed and exportable unchanged; RAW+JPEG pairs related | unsupported | preservation-only | RAW decoder | Bytes preserved; no development; pairing waits for the media module | Task 1.3, Task 3.1 | none |
| Deliberate RAW reprocessing with comparison; published camera support | unsupported | unavailable | — | Pending; the supported-camera set is undeclared | Task 3.1 | none |
| Default UI shows RAW+JPEG pairs as one item | unsupported | unavailable | — | Pending | Task 1.3; Task FS-2.2 (UI-F2.3) | none |

### F-014: AI-assisted cleanup and styles

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | F-045 connection | Pending | [Task 7.1](./implementation-plan.md#task-7.1) | none |
| Cleanup with region, preview, cancel, reversible apply, failure states | unsupported | unavailable | — | Pending | Task 7.1 | none |
| **Extend** through a supported F-045 generation task | unsupported | unavailable | — | Pending | Task 7.1, Task 4.2 | none |
| Reversible face obscuring; concealing exports omit source layers | unsupported | unavailable | — | Pending | Task 7.1 | none |
| Styles distinguished from capture-aware interpretation | unsupported | unavailable | — | Pending | Task 7.1 | none |
| AI outputs retain input, version and model provenance | unsupported | unavailable | — | Pending | Task 7.1, Task FS-2.8 | none |
| Machine-readable AI-manipulation mark; default UI shows it with its provenance | unsupported | unavailable | — | Pending | Task 7.1, [Task 8.4](./implementation-plan.md#task-8.4) | none |

### F-015: Portrait, depth, Cinematic and spatial media

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | preservation-only | — | Opaque capture resources are preserved as uploaded bytes; nothing interprets them. Unsupported formats are registered here, as F-015 requires | [Task 7.2](./implementation-plan.md#task-7.2) | none |
| Depth, focus and spatial resources preserved; flattened rendition linked to its source | unsupported | preservation-only | — | Bytes preserved; no linking of companion resources | Task 1.3, Task 7.2 | none |
| Portrait effect, depth blur, refocus, lighting, Cinematic focus edits | unsupported | unavailable | Source data accessible | Pending | Task 7.2 | none |
| Spatial media prerequisites declared; ordinary fallback | unsupported | unavailable | — | Pending | Task 7.2 | none |
| Per-format import, edit and export fixtures | unsupported | unavailable | — | No fixtures; no format in the supported set yet | Task 7.2 | none |
| Default UI previews the fallback rendition and lists preserved resources | unsupported | unavailable | — | Pending | Task 7.2 | none |

### F-016: Live Photos, bursts and motion images

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | preservation-only | — | Still and motion components are preserved as separate uploaded files; no pairing | Task 2.2, [Task 7.3](./implementation-plan.md#task-7.3) | none |
| One logical record for still and motion with independent checksums; partial state when a component is missing | unsupported | unavailable | — | Pending | Task 1.3, Task 2.2 | none |
| Key frame, trim, loop, bounce, long exposure, Live and still duplication, GIF export, revert | unsupported | unavailable | — | Pending | Task 7.3 | none |
| Burst review with recoverable deletion of discarded frames | unsupported | unavailable | — | Pending | Task 2.2 | none |
| Default UI shows a pair or burst as one item and plays the motion resource | unsupported | unavailable | — | Pending | Task 2.2, Task 7.3 | none |

### F-017: Video and audio editing

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | preservation-only | Isolated LGPL FFmpeg worker (ADR 011) | Video and audio originals are preserved and play in the browser preview when the engine decodes the format; no editing | Task 7.4 | none |
| Non-destructive video edits (trim, poster frame, tone, colour, curves, crop, rotate, filters) | unsupported | unavailable | — | Pending | Task 7.4 | none |
| 10-bit HDR preserve, play and export; slow-motion ranges; disclosed output properties | unsupported | unavailable | — | Pending | Task 7.4 | none |
| Audio mute, enhancement and mix equivalents | unsupported | unavailable | — | Pending | Task 7.4 | none |
| API refuses still-only operations on video | unsupported | unavailable | — | No still operations exist to refuse | Task 3.1, Task 7.4 | none |
| Default UI plays video and audio with HDR fallback and shows output properties | limited | limited | Browser-decodable formats | Preview plays what the webview decodes through ranged reads; no HDR handling, no technical metadata panel | Task FS-2.2; Task 7.4 | UI-F2.2 tick |

### F-018: Markup and companion annotation

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | — | Pending | [Task 7.5](./implementation-plan.md#task-7.5) | none |
| Annotation recipe of objects over a still; flattening on export | unsupported | unavailable | — | Pending | Task 7.5 | none |
| Companion device pairing for annotation edits | unsupported | unavailable | — | Pending | Task 7.5 | none |
| Accessibility descriptions stored independently; included by policy | limited | limited | Both hosts | A description is a `dcterms:` statement today (F-005); the export policy is open | Task FS-1.3; Task 7.5 | Task FS-1.3 row |
| Default UI shows annotation presence and edits the accessibility description | unsupported | unavailable | — | Pending | Task 7.5; Task FS-2.2 (UI-F2.4) | none |

### F-019: People, pets and groups

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | F-045 connection or local model | Pending; no recognition code exists, and none of the prohibited uses (emotion recognition, untargeted scraping, remote biometric identification) will be implemented | Task 4.1 | none |
| Detect and group faces in stills and video, and pets; multiple identities per file; status through the API | unsupported | unavailable | — | Pending | Task 4.1 | none |
| Name, rename, add regions, confirm, reject, merge, propagate | unsupported | unavailable | — | Pending | Task 4.1 | none |
| Hide an identity distinct from deleting files; automatic and custom groups | unsupported | unavailable | — | Pending | Task 4.1 | none |
| Explicit enable, disable, reset, rebuild; protected identity sync under the key model | unsupported | unavailable | F-029 | Pending | Task 4.1, [Task 5.1](./implementation-plan.md#task-5.1) | none |
| Household organisation without a deployer-style notice; professional-use notice support | unsupported | unavailable | — | Pending | Task 4.1, Task 8.4 | none |
| Default UI shows identities as inferences with correction state | unsupported | unavailable | — | Pending (the default UI MAY omit identity views) | Task 4.1 | none |

### F-020: Semantic search, OCR and visual understanding

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | F-045 connection or local model | Pending | Task 4.1, Task 4.2 | none |
| Objects, scenes, activities, people, locations, OCR text, natural-language and video-moment search | unsupported | unavailable | — | Pending | Task 4.1 | none |
| OCR results with source regions; user-initiated external actions | unsupported | unavailable | — | Pending | Task 4.1 | none |
| Visual lookup with declared subject classes; landmark matching with its own switch | unsupported | unavailable | — | Pending | Task 4.1 | none |
| Subject extraction cutouts | unsupported | unavailable | — | Pending | Task 4.1 | none |
| Consented labelled corpus with thresholds before acceptance | unsupported | unavailable | — | Pending; no corpus declared | Task 4.1 | none |
| Reviewable index rebuilds, model changes and misclassifications; inferences never shown as certain | unsupported | unavailable | — | Pending | Task 4.1 | none |
| Default UI search accepts natural-language and OCR queries; shows index states | unsupported | unavailable | — | Pending | Task 4.1; Task FS-2.5 (UI-F2.9) | none |

### F-021: Places, Trips and curation controls

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | — | Pending | Task 4.1 | none |
| Recorded versus inferred locations distinguished; unknown dates and zones respected | limited | limited | Metadata module | Recorded GPS and capture dates are extracted and preserved (Task 0.15); no inference, no map, no search | Task 0.15; Task 4.1 | Task 0.15 row |
| Trips and curated time groups with highlights, All and corrections | unsupported | unavailable | — | Pending | Task 4.1 | none |
| Map and lookup disclosure; offline metadata use without a network map | unsupported | unavailable | — | Pending | Task 4.1 | none |
| Analysis progress, opt-out and reindexing | unsupported | unavailable | — | Pending | Task 4.1 | none |
| Default UI shows the labelled location and allows location search | unsupported | unavailable | — | Pending | Task 4.1; Task FS-2.2 (UI-F2.4) | none |

### F-022: Memories and storytelling

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | — | Pending; no emotion recognition will be added | [Task 6.1](./implementation-plan.md#task-6.1) | none |
| Automatic themed memories; pinning and sharing | unsupported | unavailable | — | Pending | Task 6.1 | none |
| Editable memory definitions (title, subtitle, cover, duration, order, look, soundtrack); favorite flag and creation origin | unsupported | unavailable | — | Pending | Task 6.1 | none |
| Render a memory to a movie; music rights govern export | unsupported | unavailable | FFmpeg worker; no commercial music service | Pending | Task 6.1 | none |
| Featured-content controls and resets | unsupported | unavailable | — | Pending | Task 6.1 | none |
| Deleting a memory keeps sources; excluded content never reintroduced | unsupported | unavailable | — | Pending | Task 6.1 | none |
| Interactive AI assistance discloses itself | unsupported | unavailable | — | Pending | Task 6.1, Task 8.4 | none |
| Rendered memory appears as a derived file with provenance | unsupported | unavailable | — | Pending | Task 6.1, Task FS-2.8 | none |

### F-023: Slideshows, printing and photo projects

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | — | Pending | Task 6.1 | none |
| Instant and saved slideshow definitions | unsupported | unavailable | — | Pending | Task 6.1 | none |
| Print-ready PDF for single and multiple files, contact sheets, layouts | unsupported | unavailable | — | Pending | Task 6.1 | none |
| Project adapters (providers, create, reopen, revise, print ordering) | unsupported | unavailable | Installed provider | Pending | Task 6.1 | none |
| Legacy project conversion with honest reports | unsupported | unavailable | — | Pending | Task 6.1 | none |
| Prepared slides with bounded prefetch | unsupported | unavailable | — | Pending; the paint budget is proposed in foundation evidence | Task 6.1 | none |
| Default UI prints or saves a PDF of selected files | unsupported | unavailable | — | Pending | Task 6.1 | none |

### F-024: System sharing and extension interfaces

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | Platform adapter per OS | Pending | Task 8.1; Task FS-2.7 | none |
| Share sheet, email, messaging, nearby transfer (AirDrop where offered) | unsupported | unavailable | Per OS | Pending; a platform adapter will be the equivalence route (adapter equivalent once it ships), never an Apple-affiliation claim | Task 8.1 | none |
| Share control menu with the enabled destinations | unsupported | unavailable | — | Pending with sharing | Task FS-2.7, Task 8.1 | none |
| External-editor handoff that ingests the result as a new version | unsupported | unavailable | — | Pending | Task 8.1 | none |
| Extension declarations, isolation and provenance | unsupported | unavailable | — | Pending | Task 8.1 | none |
| Photo picker, wallpaper and companion-input integrations with published platform support | unsupported | unavailable | Per OS | Pending | Task 8.1 | none |

### F-025: Migration selection, inventory and pilot

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | — | No migration module; free of subscription by requirement | [Task 2.3](./implementation-plan.md#task-2.3) | none |
| Wizard: source, route, destination, scope, estimates | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Preflight: missing archives, unavailable originals, unsupported fields, authorisation, hidden choices | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Durable manifest before transfer | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Documented fallback identity and sidecar matching | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Representative pilot with a fidelity report | unsupported | unavailable | — | Pending | Task 2.3 | none |

### F-026: Migration preservation and reconstruction

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Exact originals, linked edited renditions, raw sidecars, opaque adjustment data, pairs | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Dates, zones, GPS, names, captions, keywords with mapping provenance | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Folders, albums, favorites, hidden states across archive parts | unsupported | unavailable | — | Pending | Task 2.3 | none |
| People labels, memories, sharing and comments retained as source data | unsupported | unavailable | — | Pending; unavailable recognition is reported, never reconstructed (C-037) | Task 2.3 | none |
| New destination authorisation for imported sharing; distinct identities under deduplication | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Existing C2PA credentials byte-exact; import record and PROV-O import activity per migrated file | unsupported | unavailable | `c2pa` module for the credential check | Pending; the import record itself exists for API uploads (F-002) | Task 2.3, Task FS-2.8, Task 2.6 | none |

### F-027: Migration execution, recovery and reconciliation

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Extract, transfer, verify, register, release with independent resource recovery | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Resume reconciles source version, destination state and checkpoints | unsupported | unavailable | — | Pending; the job checkpoint contract exists in `filesquid-core` (Task 0.2) | Task 2.3 | [Task 0.2 row](./implementation-plan.md#task-0.2) |
| Pause, resume, cancel, bounded retries, expiry, throttling, full disks | unsupported | unavailable | — | Pending; the proposed bounds are in foundation evidence | Task 2.3 | none |
| Crash reconciliation; partial data never a verified original | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Progress by stage; readable and machine-readable reports for every inventoried item | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Follow-up scans without propagating deletions | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Completion includes F-042 durability and F-043 provenance; fair scheduling | unsupported | unavailable | — | Pending | Task 2.3, Task 2.6 | none |

### F-028: Supported migration routes

| Item | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| Family summary | unsupported | unavailable | — | No route is implemented; no route claims Apple adjustment history, source sharing permissions, faces or dynamic memories | Task 2.3, [Task 2.5](./implementation-plan.md#task-2.5) | none |
| SRC-01 Apple export folders | unsupported | unavailable | Export folder | Pending | Task 2.3 | none |
| SRC-02 macOS PhotoKit (authorised libraries, bounded iCloud extraction) | unsupported | unavailable | macOS; PhotoKit permission per version ([TC-003](./prd.md#tc-003-platform-and-provider-limits)) | Pending; a macOS-bound adapter (adapter equivalent once it ships), not a portable route | Task 2.5 | none |
| SRC-03/04 Google Takeout archives and extracted folders | unsupported | unavailable | Takeout parts | Pending | Task 2.3 | none |
| SRC-07/08 Windows folders and hydrated OneDrive folders | unsupported | unavailable | Placeholder detection (TC-003) | Pending | Task 2.3 | none |
| SRC-05 Google Picker, SRC-09 Graph/OneDrive direct, SRC-06 provider transfer | unsupported | unavailable | Official contracts revalidated first | Optional; never a whole-library Google route | Task 2.3 (M-C, only if selected) | none |
| Per-route published source, export and OS versions and preservation matrix | unsupported | unavailable | — | Pending | Task 2.3, Task 2.5, Task 8.1 (M-D audit) | none |
| Outcome, preservation explanation and recovery path for every inventoried item | unsupported | unavailable | — | Pending | Task 2.3 | none |
| Default UI offers the folder and archive routes through its import flow | unsupported | unavailable | — | Pending | Task 2.3 | none |

## B. FileSquid platform, hosts, services and later features

One row per remaining F ID. Children are listed where [F-040](./prd.md#f-040-compatibility-and-parity-register) names them (F-041, F-044, F-045, F-046, F-047). F-050 to F-060 are recorded against the FileSquid phases of the [implementation plan](./implementation-plan.md).

| Requirement | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| [F-001](./prd.md#f-001-independent-local-library-and-ownership) Independent local library and ownership | supported | equivalent | Desktop from source; `fsquid` | Create, open, copy and recover an owner-folder library with no account and no network | Task 1.1 (phase F2) | Task 1.1 row |
| [F-029](./prd.md#f-029-client-encryption-and-key-custody) Client encryption and key custody | unsupported | unavailable | — | Crypto contracts exist in `filesquid-core` (Task 0.2); no encryption, keys or custody | [Task 5.1](./implementation-plan.md#task-5.1) (phase F4) | none |
| [F-030](./prd.md#f-030-personal-synchronization-and-availability) Personal synchronization and availability | unsupported | unavailable | `*.filesquid.tech` | Pending | Task 5.2 (phase F4) | none |
| [F-031](./prd.md#f-031-encrypted-backup-and-verified-restore) Encrypted backup and verified restore | unsupported | unavailable | `*.filesquid.tech` | Pending | Task 5.2 (phase F4) | none |
| [F-032](./prd.md#f-032-accounts-entitlements-billing-and-cancellation) Accounts, entitlements, billing and cancellation | unsupported | unavailable | Payment processor (later work, T-034) | Pending | [Task 5.3](./implementation-plan.md#task-5.3) (phase F4) | none |
| [F-033](./prd.md#f-033-shared-albums-and-hosted-galleries) Shared albums and hosted galleries | unsupported | unavailable | — | Pending | [Task 5.6](./implementation-plan.md#task-5.6) (phase I5) | none |
| [F-034](./prd.md#f-034-family-accounts-and-shared-libraries) Family accounts and shared libraries | unsupported | unavailable | `filesquid-server` accounts | Pending; the deny-by-default authorisation layer exists (API-F1.7) but no accounts or groups | [Task FS-3.10](./implementation-plan.md#task-fs-3.10), [Task FS-2.3](./implementation-plan.md#task-fs-2.3), Task 5.6 | none |
| [F-035](./prd.md#f-035-optional-service-extensions) Optional service extensions | unsupported | unavailable | — | Pending | [Task 8.3](./implementation-plan.md#task-8.3) (phase I7) | none |
| [F-036](./prd.md#f-036-browser-and-mobile-web-experience) Browser and mobile-web experience | unsupported | unavailable | Encrypted browser host | The static UI is served by `filesquid-server`; no encrypted browser host, no installable mobile web, no camera capture | [Task FS-5.1](./implementation-plan.md#task-fs-5.1), Task 1.6, Task 1.8 (phase F5) | none |
| [F-037](./prd.md#f-037-setup-updates-and-diagnostics) Setup, updates and diagnostics | limited | limited | From source only | The desktop host starts without an account and journals its refusals; no installer, updater or diagnostics bundle | [Task FS-3.1](./implementation-plan.md#task-fs-3.1), [Task FS-3.11](./implementation-plan.md#task-fs-3.11), [Task 8.5](./implementation-plan.md#task-8.5) | README "Run the application" |
| [F-038](./prd.md#f-038-open-interfaces-and-product-exit) Open interfaces and product exit | limited | limited | Both hosts | OpenAPI 3.1 contract with generated TypeScript types and a drift test (FS-1.1); OCFL on disk is readable without FileSquid; published packages and contract versioning are open | [Task FS-1.1](./implementation-plan.md#task-fs-1.1); [Task FS-3.7](./implementation-plan.md#task-fs-3.7), [Task FS-3.3](./implementation-plan.md#task-fs-3.3) | Task FS-1.1 row |
| [F-039](./prd.md#f-039-consent-notifications-and-activity-privacy) Consent, notifications and activity privacy | unsupported | unavailable | — | Pending | Task FS-2.7, Task 1.6, Task 4.1 | none |
| [F-040](./prd.md#f-040-compatibility-and-parity-register) Compatibility and parity register | limited | limited | — | This register and the `filesquid-core` types exist; the photo-manager parity register with named products and versions, the fixture matrix and the performance-profile evidence are pending | Task 0.8 (this register); Task 8.1 (M-D audit); ImageSquid gate (g) | this document; CO-0.9 |
| [F-041](./prd.md#f-041-source-aware-web-clipping-and-http-evidence) Source-aware web clipping and HTTP evidence | unsupported | unavailable | Recorder implementation (later work, T-034) | Pending | [Task 2.4](./implementation-plan.md#task-2.4), [Task 0.12](./implementation-plan.md#task-0.12) spike (phase I2) | none |
| F-041 child: recorder field and body completeness | unsupported | unavailable | — | Undeclared until the recorder spike passes complete controlled exchanges | Task 0.12, Task 2.4 | none |
| [F-042](./prd.md#f-042-ocfl-preservation-and-portable-recovery) OCFL preservation and portable recovery | limited | limited | Both hosts; local filesystem storage | Canonical local preservation is supported: valid OCFL 1.1 objects, app-closed copy and reconstruction without the operational database (Tasks 0.3, 0.4, 1.1); S3 storage and hosted ciphertext preservation are open | Tasks 0.3, 0.4, 1.1; [Task 5.4](./implementation-plan.md#task-5.4) hosted | Task 1.1 row |
| [F-043](./prd.md#f-043-c2pa-records-and-action-disclosure) C2PA records and action disclosure | unsupported | unavailable | `c2pa` module (release 2, [ADR 037](./architecture/037-c2pa-in-release-2.md)) | The module parses manifest stores and checks hard bindings for JPEG and PNG (CP-0.1 to CP-0.3); no host links it and no record is minted | [Task 0.18](./implementation-plan.md#task-0.18), [Task FS-3.12](./implementation-plan.md#task-fs-3.12), Task 2.6, [Task FS-C.1](./implementation-plan.md#task-fs-c.1) (phase C) | none |
| [F-044](./prd.md#f-044-paid-c2pa-publishing-and-watermarking) Paid C2PA publishing and watermarking | unsupported | unavailable | Paid provider; F-032 | Pending | [Task 8.2](./implementation-plan.md#task-8.2), Task 0.12 spike | none |
| F-044 child: in-app surface, publisher, watermark, format and destination coverage | unsupported | unavailable | — | Undeclared until the publisher spike | Task 0.12, Task 8.2 | none |
| [F-045](./prd.md#f-045-byo-ai-connections-and-per-task-model-choice) BYO AI connections and per-task model choice | unsupported | unavailable | User-owned endpoint | Pending | Task 4.2 (phase I4) | none |
| F-045 child: model and task contracts | unsupported | unavailable | — | No task contract declared | Task 4.2 | none |
| [F-046](./prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports) Media metadata inspection, correction and privacy exports | limited | limited | Both hosts; `fsquid` | See children | Task FS-1.3; Task 1.5; Task 2.2 | below |
| F-046 child: extraction profile, EXIF tags, ID3 frames and the `dc:`/`dcterms:` properties of embedded XMP | supported | equivalent | Metadata module (`extract.rs`); a file whose EXIF block or ID3 tag the readers parse, or whose first `<x:xmpmeta>` packet lies within the first 16 MiB (`MAX_XMP_SCAN_BYTES`) | OCFL-sourced extraction mapped onto `dcterms:` (Task 0.15); the supported set is nonempty | Task 0.15 | Task 0.15 row |
| F-046 child: extraction profile, IPTC-IIM, maker-specific native fields, XMP properties outside `dc:`/`dcterms:` and XMP packets beyond the first 16 MiB | unsupported | unavailable | Per-format reader | No reader for them yet; the bytes stay preserved in the original | [Task 1.5](./implementation-plan.md#task-1.5), Task 2.2 | none |
| F-046 child: `dcterms:` and `schema:` statements, single and batch, with the namespace registry | supported | equivalent | Both hosts; `fsquid` | Property routes and CLI; registered namespaces (MD-0.7) | Task FS-1.3, Task 0.14 | Task FS-1.3 row |
| F-046 child: native metadata write-back into media files | unsupported | unavailable | Per-format writer | Pending | Task 2.2 | none |
| F-046 child: scrubbing profiles (location, identity, `OriginalName`) applied on sharing routes | limited | preservation-only | Metadata module | Scrub classes exist; no sharing route applies them | Task FS-1.3 (MD-0.10); Task 1.5, Task FS-2.7 | Task FS-1.3 row |
| F-046 child: JSON sidecar interchange | supported | equivalent | `fsquid` | Task 0.16 | Task 0.16 | [Task 0.16 row](./implementation-plan.md#task-0.16) |
| [F-047](./prd.md#f-047-institutional-dam-and-iiif-interoperability) Institutional DAM and IIIF interoperability | unsupported | unavailable, **explicitly pending** | Partner and funding gates (IIIF-2, IIIF-3) | Separate institutional delivery gate; never blocks consumer parity (RC-008) | [Task 9.1](./implementation-plan.md#task-9.1), [Task 9.2](./implementation-plan.md#task-9.2), [Task 9.3](./implementation-plan.md#task-9.3) (phase L) | none |
| F-047 child: institutional mappings and consumers | unsupported | unavailable, pending | — | Undeclared until a partner profile is confirmed | Task 9.1 | none |
| [F-048](./prd.md#f-048-later-hosted-private-oss-inference) Later hosted private OSS inference | unsupported | unavailable, **explicitly pending** | Verified paid encrypted preservation first | Optional, later; never blocks consumer parity | [Task 9.4](./implementation-plan.md#task-9.4) (phase L) | none |
| [F-049](./prd.md#f-049-product-component-terminology-and-item-type) Product component terminology and item type | supported | equivalent | Both UIs | Internal names and the ImageSquid label table are tested in the built pages; item type is file | Task 0.1, Task FS-2.1 | UI-0.6 walkthrough |
| [F-050](./prd.md#f-050-filesquid-platform-and-distributions) FileSquid platform and distributions | limited | limited | — | One Cargo workspace, one API, two distributions as UI layers (phase F0, FS-0.0); publication, extraction and the release gates are open | Task FS-0.0 (F0); [Task FS-3.5](./implementation-plan.md#task-fs-3.5), [Task FS-3.8](./implementation-plan.md#task-fs-3.8) (F3R), [Task 9.5](./implementation-plan.md#task-9.5) (L) | Task FS-0.0 row |
| [F-051](./prd.md#f-051-ocfl-module) OCFL module | limited | limited | Stand-alone crate; local filesystem | Read, validate, write, change model, git guard, each building and testing alone (phase F0, Tasks 0.3, 0.4); S3 storage and publication to crates.io are open | Tasks 0.3, 0.4 (F0); Task FS-3.3 (F3R), [Task 10.4](./implementation-plan.md#task-10.4) | Task 0.3 and 0.4 rows |
| [F-052](./prd.md#f-052-metadata-module) Metadata module | limited | limited | Stand-alone crate; WASM build | Register, CLI, CSV, WASM, OCFL-sourced extraction, JSON sidecar (phase F0, Tasks 0.13 to 0.16); publication to crates.io is open | Tasks 0.13, 0.14, 0.15, 0.16 (F0); Task FS-3.3 publication | their rows |
| [F-053](./prd.md#f-053-virtual-exhibit) Virtual exhibit | unsupported | unavailable | — | Pending | [Task FS-2.6](./implementation-plan.md#task-fs-2.6) (F3R) | none |
| [F-054](./prd.md#f-054-c2pa-module) C2PA module | limited | limited | Stand-alone crate, unlinked | Scaffold, manifest-store parsing, hard bindings and the JPEG and PNG handlers (CP-0.1 to CP-0.3); signature and trust validation (CP-0.4, CP-0.5), the builder, signing, embedding, scans and publication are open | [Task 0.17](./implementation-plan.md#task-0.17), Task 0.18, Task FS-3.12, Task 2.6, [Task FS-C.2](./implementation-plan.md#task-fs-c.2) (C) | [Task 0.17 row](./implementation-plan.md#task-0.17) |
| [F-055](./prd.md#f-055-default-ui-and-panel-composition) Default UI and panel composition | limited | limited | Chromium, WebKitGTK | Shell, tokens, accessibility contracts, navigation and the file browser (phase F2); Details, Versions and Metadata panels, the three-tab file panel and the Logs view are open | Task FS-2.1 (F2); Task FS-2.2, [Task FS-2.9](./implementation-plan.md#task-fs-2.9), [Task 1.9](./implementation-plan.md#task-1.9) | UI-0.6 walkthrough |
| [F-056](./prd.md#f-056-single-filesquid-api) Single FileSquid API | limited | limited | Both hosts | OpenAPI 3.1 contract with generated types, transport-agnostic router, custom URI scheme and HTTP transports, event channel (phase F1); every feature that exists is reachable through it, but the published, versioned contract is open | Task FS-1.1 (F1); Task FS-3.7 versioning | Task FS-1.1 row |
| [F-057](./prd.md#f-057-desktop-and-server-hosts) Desktop and server hosts | limited | limited | From source; macOS and Linux | Desktop host with the per-launch token and no TCP listener; `filesquid-server` serves the API and static UI on `127.0.0.1:8083` with an admin token; accounts, storage root, installers and the browser host are open | Task FS-0.0 (F0), Task FS-1.1 (F1); Task FS-3.1 (F3), Task FS-3.10, Task FS-3.11 (F3R), Task FS-5.1 (F5) | README "Run the application" |
| [F-058](./prd.md#f-058-white-label-distributions) White-label distributions | limited | limited | Chromium | ImageSquid overrides tokens and labels over `@filesquid/ui` by local path (phase F0, F2); the distribution manifest, module switches and extraction are open | Task FS-0.0, Task FS-2.1; [Task FS-1.5](./implementation-plan.md#task-fs-1.5), Task FS-3.8 (F3R) | UI-0.6 walkthrough (ImageSquid suites) |
| [F-059](./prd.md#f-059-hosted-encrypted-backup-and-sync-service) Hosted encrypted backup and sync service | unsupported | unavailable | Region, provider, payment, audit (later work, T-034) | No hosted service exists | [Task FS-4.1](./implementation-plan.md#task-fs-4.1), [Task 0.6](./implementation-plan.md#task-0.6), Task 5.4, [Task FS-4.2](./implementation-plan.md#task-fs-4.2) (phase F4) | none |
| [F-060](./prd.md#f-060-archival-curation-jobs-and-datasets) Archival curation jobs and datasets | unsupported | unavailable, pending | Arkrim distribution | Pending; separate institutional gate | [Task AR-1.2](./implementation-plan.md#task-ar-1.2), [Task AR-1.3](./implementation-plan.md#task-ar-1.3), [Task AR-1.4](./implementation-plan.md#task-ar-1.4) (phase A) | none |
| [F-061](./prd.md#f-061-activity-journal-and-live-health-monitoring) Activity journal and live health monitoring | limited | limited | Both hosts; `fsquid` | Journal, process registry, health, admin routes, metrics, Monitor dashboard, client events, CSV and JSON Lines export, CLI journaling (Task FS-2.4 and the ticked rows API-F2.10, API-F2.11, CLI-0.2 and CL-1.9 of FS-2.9); host journal sealing (DE-0.11), the Logs view (UI-F2.16) and OTLP export are open | [Task FS-2.4](./implementation-plan.md#task-fs-2.4) (F2); Task FS-2.9; [Task FS-3.6](./implementation-plan.md#task-fs-3.6) | [`activity_journal_and_live_health_monitoring_F-061.feature`](../apps/filesquid/crates/api/tests/features/activity_journal_and_live_health_monitoring_F-061.feature); README "View logs" |
| [F-062](./prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) Drive-style views, sorting, selection and layout preferences | limited | limited | Chromium, WebKitGTK | List and grid exist; sort keys, selection, preferences, storage use and the sidebar views are open | Task FS-2.2; Task FS-2.5 (API-F2.2, UI-F2.8, UI-F2.18) | UI-F2.2 tick |
| [F-063](./prd.md#f-063-file-search-query-language-and-filter-chips) File search query language and filter chips | unsupported | unavailable | — | Pending | Task FS-2.5 (API-F2.3, UI-F2.9) | none |
| [F-064](./prd.md#f-064-file-activity-comments-and-user-activity-stream) File activity, comments and user activity stream | unsupported | unavailable | — | Pending; the journal that feeds it exists | Task FS-2.7 (F3R) | none |
| [F-065](./prd.md#f-065-file-version-history-and-retention) File version history and retention | limited | limited | Both hosts | Version listing, labels, restore and history size with no thinning through the API (API-F2.5 ticked); the Versions panel is open | Task FS-2.5 (API-F2.5 done, UI-F2.11 open) | API-F2.5 tick |
| [F-066](./prd.md#f-066-file-and-folder-sharing-roles-links-and-shares-view) File and folder sharing roles, links and shares view | unsupported | unavailable | `filesquid-server` accounts | Pending | Task FS-2.7, Task FS-2.3 (F3R) | none |
| [F-067](./prd.md#f-067-prov-o-history-of-every-version-and-change) PROV-O history of every version and change | limited | limited | Stand-alone `prov` crate | The `prov` crate's model and serialisation (PV-0.1 ticked); commit-path records, the API and the Provenance tab are open | Task FS-2.8 (F2), Task FS-2.9 | PV-0.1 tick |

## C. Non-functional requirements and technical constraints

| Requirement | Status | F-040 state | Platform and prerequisite | Reason | Delivered by | Evidence |
|---|---|---|---|---|---|---|
| [NFR-001](./prd.md#nfr-001-preservation-and-transactional-integrity) Preservation and transactional integrity | limited | limited | Local filesystem | OCFL commits are atomic and verified, journal entries ride the same transaction; concurrent-writer rebase and hosted serialisation are open | Tasks 0.4, 0.5, 1.1; Task 1.2; Task 5.4 | Task 0.4 row |
| [NFR-002](./prd.md#nfr-002-benchmark-envelope-and-responsiveness) Benchmark envelope and responsiveness | unsupported | unavailable (**proposed budgets, not measured**) | Declared 4-core / 16 GB / SSD desktop; 4 GB mobile profile | No measurement exists; budgets recorded as proposed in [foundation evidence](./foundation-evidence.md) | Task FS-3.4 (F3), [Task FS-3.13](./implementation-plan.md#task-fs-3.13) (F3R), [Task 10.1](./implementation-plan.md#task-10.1) | none |
| [NFR-003](./prd.md#nfr-003-bounded-processing-and-migration-endurance) Bounded processing and migration endurance | limited | limited (**proposed bounds, not measured**) | — | Job and checkpoint contracts and the journal's bounded backpressured queue exist; the staging, concurrency, retry, memory and soak bounds are proposed and unmeasured | Tasks 0.2, 0.5, FS-2.4; Task 1.2, Task FS-3.4, Task 10.1 | [`job_checkpoint_protocol_NFR-003.feature`](../apps/filesquid/crates/core/tests/features/job_checkpoint_protocol_NFR-003.feature) |
| [NFR-004](./prd.md#nfr-004-security-and-privacy) Security and privacy | limited | limited | Both hosts | Deny-by-default authorisation on every protected route (the liveness probe `GET /api/v1/health/live` and the static UI are the only public ones), redacted journal, no telemetry, no TCP listener on desktop; encryption, accounts and the independent crypto audit are open | Task FS-1.4 (API-F1.7 ticked), Task FS-2.4; Task 5.1, [Task 5.5](./implementation-plan.md#task-5.5) | README "Run tests" (API-S-004) |
| [NFR-005](./prd.md#nfr-005-recovery-and-service-reliability) Recovery and service reliability | limited | limited | Desktop | Library recovery from OCFL alone; service reliability, updater recovery and hosted restore are open | Task 1.1; Task 8.5, Task 5.2, Task 5.5 | Task 1.1 row |
| [NFR-006](./prd.md#nfr-006-accessibility-and-usability) Accessibility and usability | limited | limited | Chromium, WebKitGTK | WCAG 2.2 AA tokens, keyboard, focus, reduced motion, forced colours, 320 px and 200 % text, RTL tested on the shell and tree; later surfaces are tested as they land | Task FS-2.1; every UI task | UI-0.6 walkthrough |
| [NFR-007](./prd.md#nfr-007-compatibility-rendering-and-localization) Compatibility, rendering and localization | unsupported | unavailable | Codec, ICC/HDR and locale fixtures | No rendering engine; locale support beyond the label table and RTL mirroring is undeclared | Task 0.12, Task 3.1, Task 7.2, Task 8.1 | none |
| [NFR-008](./prd.md#nfr-008-maintainability-and-verified-engineering) Maintainability and verified engineering | limited | limited | Every workspace | Lint and format gates, unit, integration, BDD and built-production UI tests with Allure/JUnit under each app's `tests/allure-results/`, cleared per run, raw walkthrough evidence; load and performance tests do not exist yet (NFR-002) | Task 0.8 (UI-0.6), every task's hook run; [Task FS-3.2](./implementation-plan.md#task-fs-3.2), [Task 10.2](./implementation-plan.md#task-10.2) | [foundation evidence](./foundation-evidence.md); [`maintainability_and_verified_engineering_NFR-008.feature`](../apps/filesquid/crates/server/tests/features/maintainability_and_verified_engineering_NFR-008.feature) |
| [NFR-009](./prd.md#nfr-009-observability-and-capacity-economics) Observability and capacity economics | limited | limited | Both hosts | Journal, metrics and Monitor exist; capacity economics of the hosted service are open | Task FS-2.4; Task 0.6, Task 5.5, Task FS-3.2 | README "View logs" |
| [NFR-010](./prd.md#nfr-010-accurate-public-claims) Accurate public claims | supported | equivalent | — | No performance, parity, paid-service or compatibility claim is made beyond what this register and foundation evidence record | Task 0.8; every gate | this document |
| [TC-001](./prd.md#tc-001-immutable-originals-and-explicit-identity) Immutable originals and explicit identity | supported | equivalent | Local filesystem | Immutable OCFL versions, UUID v4 identifiers, SHA-256 digests | Tasks 0.1, 0.4, 1.1 | Task 0.4 row |
| [TC-002](./prd.md#tc-002-endpoint-processing-and-hosted-encryption) Endpoint processing and hosted encryption | unsupported | unavailable | Hosted service | No hosted service; the dependency-graph rule (no `filesquid-native` in WASM or the hosted service) is kept by the workspace tests | Task 0.6, Task 5.1, Task FS-4.1 | none |
| [TC-003](./prd.md#tc-003-platform-and-provider-limits) Platform and provider limits | unsupported | unavailable | Per route and per version | No PhotoKit, Google, Microsoft or browser-lifetime route exists, so none is verified; the register will carry each route's version matrix when it lands | Task 2.5, Task 2.3, Task 1.6, Task 8.1 | none |
| [TC-004](./prd.md#tc-004-decision-status-and-repository-alignment) Decision status and repository alignment | supported | equivalent | — | ADRs reviewed before implementation; the architecture-versus-scaffold record is in [foundation evidence](./foundation-evidence.md); synthesis from codebase reality is Task 10.3 | Task 0.8; [Task 10.3](./implementation-plan.md#task-10.3) | foundation evidence |
| [TC-005](./prd.md#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) Shared Rust graph, hosted mailbox isolation and OCFL writer | limited | limited | Cargo workspace | One module graph with each module crate building alone (hook step "Module crate alone"); the hosted mailbox and browser commit protocol are open | Tasks 0.1, 0.5, 0.10; Task 0.6, [Task 0.11](./implementation-plan.md#task-0.11) | README "Run tests" |

The release criteria RC-001 to RC-014 are gates, not capabilities; each is cited by the gate task that will show it passed ([release order](./prd.md#release-order-and-dependency-gates)). None has passed.

## D. Drive-parity table

Each documented difference from mainstream cloud drives and self-hosted file-sharing platforms, with its reason and owning F-ID ([SRS-F-040](./srs.md#srs-f-040-compatibility-and-parity-register)). These are decisions, not gaps: a row is unavailable **by decision** and stays so.

| Difference | Reason | Owning requirement |
|---|---|---|
| No version thinning, deletion or free-space cap on version history | Every version is an OCFL version; history is the archive, not a cache | [F-065](./prd.md#f-065-file-version-history-and-retention) |
| No per-folder end-to-end encryption and no administrator switch for it | Encryption is per library on the owner's device; an administrator cannot turn it on or off | [F-029](./prd.md#f-029-client-encryption-and-key-custody) |
| No login-password-derived keys, no manual re-share after key changes, no master key | Owner-held keys with recovery that stays with the owner; the hosted service never holds a key | F-029, [F-034](./prd.md#f-034-family-accounts-and-shared-libraries) |
| No operator-key encryption at rest and no administrator recovery key on self-run servers | A self-run operator can read what it stores and the UI says so; pretending otherwise would be a false claim | [F-057](./prd.md#f-057-desktop-and-server-hosts) |
| Thumbnails, previews, indexes and trash inside the encryption boundary | Derivatives and indexes reveal content, so they are protected with it | F-029, [F-042](./prd.md#f-042-ocfl-preservation-and-portable-recovery) |
| No default AI overview | AI runs only through the owner's own connections, per task, when asked | [F-063](./prd.md#f-063-file-search-query-language-and-filter-chips), [F-045](./prd.md#f-045-byo-ai-connections-and-per-task-model-choice) |
| No activity feed; publication to the user's own AT Protocol account only per explicit request | Activity is private; nothing is published without an explicit, scoped request ([ADR 034](./architecture/034-at-protocol-publishing-connection.md)) | [F-064](./prd.md#f-064-file-activity-comments-and-user-activity-stream) |
| Empty folders are not persisted; copy creates a new file | OCFL objects hold files; a folder is a logical path, and a copy is a new object with its own history | [F-062](./prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) |
| No comment emoji or smart picker; no `sharedwith:external` | Comments are plain application data; sharing roles are explicit, not inferred from an "external" class | F-063, F-064 |
| No document editors, calendar, reminders or federated mounts | FileSquid is a file archive with a drive-style view, not a groupware suite | [F-050](./prd.md#f-050-filesquid-platform-and-distributions) |
| No star ratings | Favorites and keywords are the classification model (section A, F-005) | [F-040](./prd.md#f-040-compatibility-and-parity-register) |

## E. Fixture coverage

[F-040](./prd.md#f-040-compatibility-and-parity-register) requires compatibility fixtures for webviews, browsers, cameras and codecs, ICC/HDR paths, input methods and languages. What exists today:

| Fixture family | Covered today | Not yet covered | Delivered by |
|---|---|---|---|
| Desktop webviews | WebKitGTK on Linux through the desktop WebDriver suite (Xvfb); macOS WKWebView by developers running the desktop from source | Windows WebView2; packaged builds on every OS | Task FS-3.1, Task FS-3.11, Task 8.1 |
| Browsers | Chromium through Playwright against the built production pages (FileSquid and ImageSquid) | Firefox, Safari, mobile browsers | Task 8.1; Task FS-5.1 |
| Cameras and codecs | None (no media module); any file type is preserved byte-exact | Every camera and codec claim | Task 1.3, Task 3.1, Task 7.4, Task 8.1 |
| ICC and HDR paths | None | Every rendering claim | Task 3.1, Task 7.2, Task 7.4 |
| Input methods | Keyboard contracts (Tab order, focus ring, skip control, ARIA tree keys) | Touch, pen, IME composition | Task FS-2.1 (done), Task 8.1 |
| Languages | Internal-name and ImageSquid label tables; right-to-left mirroring of the header and navigation | Translated label tables; locale-specific formats | Task 8.1 |

Apple services named in the PRD (iCloud, PhotoKit, AirDrop, Photos projects) map to FileSquid or ImageSquid services and platform adapters; no row implies Apple affiliation or proprietary compatibility.

## Maintenance

- A task that delivers an item moves its row to supported or limited in the same pull request, with the walkthrough as evidence ([pr-documentation-update.md](../.agents/rules/pr-documentation-update.md)).
- A gate task (FS-3.9, FS-3.5, FS-4.2, FS-C.3, 1.7, 8.1) re-reads every row it gates and records stakeholder acceptance of each remaining gap.
- The ImageSquid parity-audit gate (g) adds the photo-manager parity register with the products and versions compared against; until then no parity claim exists.
