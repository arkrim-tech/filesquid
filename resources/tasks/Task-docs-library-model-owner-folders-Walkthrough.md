# Walkthrough: record the owner-folder library model and add it to the MVP plan

Branch: `docs/library-model-owner-folders`. Documentation only; no code, contract or test changed. TDD-ENFORCE: Not applicable - documentation update.

## Owner decision (2026-10-22, verbatim)

- "Lets do option A with Owner-chosen folder library"
- To "With Option A on owner-chosen folder libraries, what happens to the existing catalog libraries stored inside the app's data directory?", the owner chose: "Folders only" — "Every library lives in a folder the owner chooses. The app's data directory keeps only the rebuildable catalog index per library, never the files. In-app libraries are retired; there are none with real data yet."
- To "Where should a file go when no folder or collection is chosen (upload at the library's top level, or an import with no destination)?", the owner chose: "Must choose" — "Refuse until the owner picks a collection; no top-level uploads."
- To "Where should each library's catalog index (the rebuildable SQLite database) live?", the owner chose: "App data directory" — "Index in the app's data directory, keyed by library UUID, as DE-S-009 says today. The library folder holds only OCFL, so it stays clean for ordinary tools; a moved library just rebuilds its index."
- Then: "Put it in the MVP roadmap".
- On Conflict 1 below (deleting a collection that still holds files), later the same day: "RE: deletion question in #124, when users delete a collection that still holds files, those files go into a "Recently deleted" top-level directory within their library. That directory has a default 30 day retention period before files inside are expunged for good."

"Option A" means one library model: every library has collections, folders and virtual collections ([PRD F-004](../prd.md#f-004-collections-folders-and-navigation): "A file's bytes MUST live in exactly one collection"); every file has exactly one home collection; uploads, imports and copies are registered in their home collection, so a file uploaded into a folder appears in that folder at once and "Drop to upload into {folder}" stays accurate; the OCFL collection snapshot inside the library folder is the source of truth, and the per-library SQLite catalog in the app's data directory is a rebuildable index that is rebuilt when a moved or copied library folder is opened.

## What changed

**Decision record.** No ADR governed where a library lives: [ADR 005](../architecture/005-sqlite-local-postgres-hosted.md) governs the local SQLite catalog ("each library has its own SQLite catalog file ... on the owner's disk") and [ADR 010](../architecture/010-ocfl-1-1-dual-profile.md) the OCFL local profile and collection snapshot, while the data-directory layout (`libraries/<id>.sqlite`, `libraries/<id>.ocfl`) was only in DE-S-009 and DE-S-010. A search of `resources/architecture/` for `libraries/<`, `.ocfl`, "catalog librar", "data directory" and "owner-chosen" found no other ADR naming catalog libraries (ADR 023, 035, 038 and 040 do not). So the decision is a new ADR, [ADR 041](../architecture/041-owner-folder-libraries-and-home-collections.md), with all 9 ISO 42010 items, and dated amendment notes in ADR 005 and ADR 010 and the ADR index.

**Requirements (RFC 2119, two-way traceability).**

- DE-S-009: the catalog is an index in the data directory, keyed by library UUID, for a library whose OCFL root is an owner-chosen folder; the data directory holds no OCFL root, originals or records; the index stores each file's home collection, logical path and object identifier, and is rebuilt when missing, foreign or stale.
- DE-S-010: the storage root is the owner-chosen folder (staging and leases stay in `<data dir>/libraries/<id>.work/`); reconcile at open rebuilds files with home collection, logical path and object ID and reports, never places, a file object without a snapshot entry.
- DE-S-018: an owner-chosen folder is the only storage root a library has; `CatalogFolders` and `CatalogScopes` are answered from the index.
- DE-S-019: the import job's plan keeps the destination; each imported file is registered beneath it.
- API-S-011: a **Destination** clause for upload, copy and move (`collection_id` required, `folder` optional, `destination_required` and `collection_not_found` refusals, registration in the snapshot under the operation ID, journaled, contract bump).
- API-S-026: the snapshot's file entity records home collection, logical path and file object identifier; every file has exactly one home collection; an upload registers in its destination.
- API-S-036: imports require `collection_id`; `collection_not_supported` is retired and replaced by `destination_required` before any dialog.
- UI-S-002: no top-level upload or import; upload controls only in a collection or folder; the drop target only over one; the import dialog requires a destination.
- Product SRS: bullets added to SRS-F-001 (owner-chosen folders only), SRS-F-002 (destination) and SRS-F-004 (home collection), each citing ADR 041.
- PRD F-002 and F-004 were checked and not changed: they already agree (F-004 "A file's bytes MUST live in exactly one collection"; F-002 import review shows a "destination album/new-album choice"). PRD unchanged, so the README "About this project" section needs no update.

**Recently deleted (owner decision on Conflict 1, second commit).** Existing rules were searched first: [PRD F-008](../prd.md#f-008-deletion-hidden-files-and-duplicate-review) already has a recoverable deletion that defaults to 30 days and a "Recently Deleted" view; [API-S-019](../../apps/filesquid/crates/api/resources/srs.md) already has `deleted_at`, `purge_after` (30 days by default), the deleted view and confirmed permanent deletion; ADR 040, CO-0.11 and SRS-F-067 already list `delete` and `purge` as operations with PROV-O activities; [PRD F-065](../prd.md#f-065-file-version-history-and-retention) keeps whole-object deletion as an explicit, journaled action under F-008. The collection-deletion path was aligned with that recoverable deletion, so there is no second trash:

- ADR 041 has a new Decision 8, Recently deleted is a reserved system area and not a collection, and Decision 3 now says a file is either in exactly one home collection or in Recently deleted, which records the home it came from. The rejected options (a) to (c) are listed under Alternatives Considered, and the open questions are listed under Related Decisions / Notes.
- PRD:
  - F-004: deleting a collection moves its files to Recently deleted.
  - F-006: the "unfiled" quick filter and rule become "recently deleted".
  - F-008: the purge after the retention period is recorded, journaled and has its PROV-O activity.
  - F-063: `is:unfiled` is removed, and `is:deleted` covers these files.
- Product SRS: SRS-F-004 and SRS-F-008 each gained a Recently deleted bullet, and the SRS-F-063 grammar drops `is:unfiled`.
- Module SRS:
  - API-S-012 retires `is:unfiled`.
  - API-S-019: collection deletion deletes the files in the same command, records their former home, and the expiry purge goes through the permanent-deletion command as a journaled `purge` with PROV-O.
  - API-S-021: a reserved `recently_deleted` node replaces the unfiled folders.
  - UI-S-002: the Recently deleted entry is never a destination, and the collection-deletion confirmation names it.
- Plan:
  - New [API-F2.15](../../apps/filesquid/crates/api/resources/plan.md) (12 hours, Task FS-2.5).
  - [DE-1.7](../../apps/filesquid/crates/desktop/resources/plan.md) goes from 20 to 23 hours (former-home columns, collection deletion in the catalog, scheduling the purge job).
  - [UI-F2.17](../../apps/filesquid/ui/resources/plan.md) goes from 8 to 11 hours (Recently deleted entry, confirmation).
  - API-F2.3 has a status note.
  - Task 1.2 goes from 54 to 57 hours and Task FS-2.5 from 132 to 147 hours, now also depending on FS-2.8 for API-F2.9.
  - Phase F2 goes from 546 to 564 hours, the MVP from 1,032 to 1,050, the total from 2,885 to 2,903 and the desktop plan from 606 to 609.
  - The checklist row was added and the progress was regenerated: 66%, about 692 of 1,050 hours.

**Plan (MVP, phases F0 to F3, before gate FS-3.9).**

- Desktop [DE-1.7](../../apps/filesquid/crates/desktop/resources/plan.md) widened and renamed "Owner-folder library index, home collections and retirement of in-app catalog roots" (12 to 20 hours): retire the in-app roots, index per folder library in the data directory, snapshot fields, catalog schema v3, index imports with collection and logical path, rebuild from the snapshot, `collection_id` required on imports, `CatalogFolders` and `CatalogScopes` for real, README update. Global Task 1.2.
- New [API-F2.14](../../apps/filesquid/crates/api/resources/plan.md) "Home collections for uploads, copies and moves" (10 hours), global Task FS-2.5, after API-F2.13 and DE-1.7. Contract minor bump.
- New [UI-F2.17](../../apps/filesquid/ui/resources/plan.md) "Uploads and imports always target a collection or folder" (8 hours), global Task FS-2.5, after UI-F2.8, UI-1.2 and API-F2.14. Holistic UI notes added to UI-F2.8 (its move-or-copy folder picker is the destination picker) and UI-1.2 (its import dialog has a required destination from the start), and to UI-F2.2's known limitation.
- Status notes on API-F1.4 and DE-1.2 point to the new tasks; API-F2.2's remainder "registering a copy's home collection" now points to API-F2.14.
- Global [Task 1.2](../implementation-plan.md#task-1.2) 46 to 54 hours; [Task FS-2.5](../implementation-plan.md#task-fs-2.5) 114 to 132 hours, now depending on Task 1.2; dispatch table; phase F2 520 to 546 hours; MVP 1,006 to 1,032 hours; total 2,859 to 2,885 hours; desktop plan total 598 to 606 hours.
- Checklist rows added for API-F2.14 and UI-F2.17, DE-1.7 renamed, and `python3 scripts/docs/update_mvp_progress.py` run (MVP progress 67%, about 691 of 1,032 hours).

## Files changed

- [`resources/architecture/041-owner-folder-libraries-and-home-collections.md`](../architecture/041-owner-folder-libraries-and-home-collections.md) (new)
- [`resources/architecture/005-sqlite-local-postgres-hosted.md`](../architecture/005-sqlite-local-postgres-hosted.md)
- [`resources/architecture/010-ocfl-1-1-dual-profile.md`](../architecture/010-ocfl-1-1-dual-profile.md)
- [`resources/architecture/README.md`](../architecture/README.md)
- [`resources/prd.md`](../prd.md) (second commit: F-004, F-006, F-008, F-063)
- [`resources/srs.md`](../srs.md)
- [`resources/implementation-plan.md`](../implementation-plan.md)
- [`resources/implementation-plan-checklist.md`](../implementation-plan-checklist.md)
- `resources/assets/mvp-progress.svg`
- [`apps/filesquid/crates/desktop/resources/srs.md`](../../apps/filesquid/crates/desktop/resources/srs.md)
- [`apps/filesquid/crates/desktop/resources/plan.md`](../../apps/filesquid/crates/desktop/resources/plan.md)
- [`apps/filesquid/crates/api/resources/srs.md`](../../apps/filesquid/crates/api/resources/srs.md)
- [`apps/filesquid/crates/api/resources/plan.md`](../../apps/filesquid/crates/api/resources/plan.md)
- [`apps/filesquid/ui/resources/srs.md`](../../apps/filesquid/ui/resources/srs.md)
- [`apps/filesquid/ui/resources/plan.md`](../../apps/filesquid/ui/resources/plan.md)
- this walkthrough

## Sync Execution Report

- **ADR → SRS:** ADR 041 cascaded into DE-S-009, DE-S-010, DE-S-018, DE-S-019, API-S-011, API-S-026, API-S-036 and UI-S-002, each citing ADR 041 in its parent cell; ADR 005 and ADR 010 carry dated amendment notes pointing to ADR 041.
- **SRS → product SRS:** SRS-F-001, SRS-F-002 and SRS-F-004 gained one bullet each, linking down to the module rows; ADR 041 added to their Source lists.
- **SRS → PRD:** PRD F-001, F-002, F-004 and F-009 checked for the library model; no change needed. The Recently deleted decision changed PRD F-004, F-006, F-008 and F-063 (second commit); the README "About this project" section names none of these details, so it is unchanged.
- **SRS → plan:** DE-1.7, API-F2.14 and UI-F2.17 cite every changed row in Related Requirements; global Tasks 1.2 and FS-2.5, the dispatch table, the phase table and the totals updated; the checklist and progress bar regenerated.
- **Codebase reality:** no code changed. The root README's catalog and commit sections still describe the in-app roots that the code has today; DE-1.7 updates them when it changes the code.
- **Conflicts:** one Conflict Artifact below, resolved by the owner's Recently deleted decision; open questions that decision does not settle are listed below; C-056 and UI-1.1 were checked and do not conflict.

## Conflict Artifacts

### Conflict 1: "unfiled" files versus exactly one home collection (resolved 2026-10-22)

**Resolution:** the owner chose a fourth rule, not (a), (b) or (c): "RE: deletion question in #124, when users delete a collection that still holds files, those files go into a "Recently deleted" top-level directory within their library. That directory has a default 30 day retention period before files inside are expunged for good." Recorded as ADR 041 Decision 8 and applied in the second commit (see "Recently deleted" above). "Unfiled" no longer exists as a state.

1. **Conflict Description:** ADR 041 Decision 3 says every file MUST have exactly one home collection, and the owner chose "Must choose" ("Refuse until the owner picks a collection; no top-level uploads."). Several requirements still create or list files that have no collection. Uploads and imports can no longer create them, but deleting a collection still does.
2. **PRD/SRS Reference:**
   - [API-S-019](../../apps/filesquid/crates/api/resources/srs.md): "Deleting a collection MUST retain its files, which become unfiled", and the query that lists "the unfiled files".
   - [API-S-021](../../apps/filesquid/crates/api/resources/srs.md): a library's children include "the folders implied by the logical paths of its unfiled files".
   - [PRD F-004](../prd.md#f-004-collections-folders-and-navigation): "Removing or deleting a collection MUST retain its files."
   - [PRD F-006](../prd.md#f-006-virtual-collections-and-metadata-search): the quick filter "**unfiled** (in no collection or virtual collection)".
   - [PRD F-063](../prd.md#f-063-file-search-query-language-and-filter-chips): `is:unfiled` ("files whose collection or folder was removed").
3. **ADR Reference:** [ADR 041](../architecture/041-owner-folder-libraries-and-home-collections.md) Decision 3: "Every file MUST have exactly one **home collection**." Decision 4: "There is no upload or import at a library's top level and no default collection."
4. **Recommended Resolution:** the owner picks one rule for deleting a collection that still holds files:
   - (a) refuse the deletion until its files are moved; or
   - (b) ask for a destination collection at deletion and move the files there in the same operation; or
   - (c) keep "unfiled" as a recoverable state that only collection deletion can produce, and record it in ADR 041 as the one exception.

   Artifacts to update: API-S-019, API-S-021, PRD F-004, F-006 and F-063, SRS-F-004, SRS-F-006 and SRS-F-063, API-F2.3 (search grammar) and CL-1.4 in the api plan, the UI-S-006 views, and ADR 041.
5. **Impact Assessment:** if the conflict is left open, a deleted collection leaves files that break the "exactly one home collection" rule. The rebuilt index and the navigation listing then disagree about where those files live, and DE-1.7 and API-F2.14 cannot test one invariant for every file.

### Open questions for the owner (Recently deleted)

The owner's words do not settle these, so nothing was decided for them. ADR 041 lists them, and API-F2.15, DE-1.7 and UI-F2.17 are told not to decide them:

1. **Restore:** where does a restored file go once its original collection no longer exists? Under "Must choose" the likely answer is that the owner picks a destination, but that is not decided.
2. **Retention scope:** the owner called 30 days the "default". At what scope can it be changed: per library or per installation?
3. **Single files:** does deleting a single file, not a whole collection, use the same Recently deleted area and the same 30 days? (The existing recoverable deletion of a file already defaults to 30 days and is listed in the deleted view.)
4. **"Expunged for good":** what does it mean for the OCFL object and its version history? Removing the object directory is an OCFL-level purge that also removes the history PRD F-065 keeps.
5. **Physical folder:** ADR 041 Decision 8 says files MUST move into Recently deleted. Is that area only a reserved navigation entry over the deleted state, or must it also be a physical folder in the library's storage root?
6. **Same logical path (from Cubic review):** when an upload, import, copy or move would give a second file the same logical path in one collection, is it refused, renamed or stored beside the first? API-F2.14 waits for the owner on that part.

### Checked, no conflict: C-056 and UI-1.1

The two suspected contradictions were checked and do not conflict:

- [C-056](../product-vision.md#release-sequence) requires "create, open, copy and recover a local library from OCFL alone". Owner-chosen folders meet it.
- UI-1.1 creates and opens libraries only through the location-choices route and a folder `location_id`. The contract's library routes all take an owner-chosen folder (`POST /api/v1/libraries` is "Create a library in an empty folder"). No UI flow creates an in-app library, so the in-app roots can only be created inside `filesquid-native`, which DE-1.7 retires.

## Verification

The change scope is not `docs`: the hook classifies changes under `crates/<crate>/resources/` as that crate's `rust:` scope, and the head-commit runs forced the full suite with `FULL=1`. Run from the repository root on the complete tree (environment: `CARGO_TARGET_DIR` shared target, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, a private `TMPDIR`):

```text
$ FULL=1 bash scripts/pre-commit.sh
...
ℹ️  Test scope: full
...
✅ Rustdoc passed (28s).
✅ Clippy (desktop, test-instrumented) passed (29s).
✅ Desktop tests (test-instrumented) passed (71s).
✅ Vitest passed (26s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
✅ Vitest passed (2s).
ℹ️  Playwright e2e skipped for apps/imagesquid (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: full).
✅ All Pre-Commit Hooks Passed! Test scope: full (953s)
```

Test scope: full. A commit that changes this section cannot list its own run; that run's summary line is posted in the pull request.
