# Architecture Decision Record - FileSquid

041-owner-folder-libraries-and-home-collections.md

## Title

One library model: every library lives in an owner-chosen folder, and every file has exactly one home collection

## Status

Accepted

Accepted: 2026-10-22 by owner decision. The owner's decisions, verbatim:

- "Lets do option A with Owner-chosen folder library"
- To "With Option A on owner-chosen folder libraries, what happens to the existing catalog libraries stored inside the app's data directory?", the owner chose: "Folders only" — "Every library lives in a folder the owner chooses. The app's data directory keeps only the rebuildable catalog index per library, never the files. In-app libraries are retired; there are none with real data yet."
- To "Where should a file go when no folder or collection is chosen (upload at the library's top level, or an import with no destination)?", the owner chose: "Must choose" — "Refuse until the owner picks a collection; no top-level uploads."
- To "Where should each library's catalog index (the rebuildable SQLite database) live?", the owner chose: "App data directory" — "Index in the app's data directory, keyed by library UUID, as DE-S-009 says today. The library folder holds only OCFL, so it stays clean for ordinary tools; a moved library just rebuilds its index."
- Then: "Put it in the MVP roadmap".

This ADR amends [ADR 005](./005-sqlite-local-postgres-hosted.md) (where the local catalog lives and what it indexes) and [ADR 010](./010-ocfl-1-1-dual-profile.md) (what the collection snapshot records). The work is planned inside the FileSquid MVP, before gate Task FS-3.9 ([implementation plan](../implementation-plan.md)).

## Context / Requirement Reference

- Requirements: [PRD F-001](../prd.md#f-001-independent-local-library-and-ownership) (local library, rebuild without the operational database), [PRD F-002](../prd.md#f-002-safe-file-folder-and-device-import) (import review shows the destination), [PRD F-004](../prd.md#f-004-collections-folders-and-navigation) ("A file's bytes MUST live in exactly one collection"), [PRD F-009](../prd.md#f-009-multiple-libraries-and-filesystem-administration) (multiple libraries, relocation), [PRD F-042](../prd.md#f-042-ocfl-preservation-and-portable-recovery) (OCFL as the source of truth), vision [FE-065](../product-vision.md#independent-library) and [U-03](../product-vision.md#platform-ownership-and-access).
- Until this decision, `main` had two library models side by side:
  - **Catalog libraries** (Task DE-0.2, [DE-S-009](../../apps/filesquid/crates/desktop/resources/srs.md), [DE-S-010](../../apps/filesquid/crates/desktop/resources/srs.md)): a SQLite catalog `libraries/<library-id>.sqlite` and an OCFL storage root `libraries/<library-id>.ocfl/` both inside the app's data directory. They hold collections, folders and virtual collections, but the file service did not serve their OCFL roots.
  - **Folder libraries** (Task DE-1.1, [DE-S-018](../../apps/filesquid/crates/desktop/resources/srs.md)): libraries in folders the owner chose through the native folder dialog, registered under opaque location IDs and served by the file service. Their `CatalogFolders` and `CatalogScopes` list no folders or objects yet.
- Uploads (Task API-F1.4) and desktop imports (Task DE-1.2, [API-S-036](../../apps/filesquid/crates/api/resources/srs.md)) commit file objects into folder libraries but register no home collection. Uploaded and imported files are therefore invisible in navigation, and the browser's drop-target label "Drop to upload into {folder}" (`fileDropTarget` in the default UI's label table) promises a placement the API does not make. A request naming a `collection_id` is refused as `collection_not_supported`.
- Task DE-1.7 (desktop plan) planned indexing imports into catalog libraries only, and recorded that the owner would be asked about the two models before it starts.

## Decision

1. **Folders only.** Every library MUST live in a folder the owner chooses (through the location-choices route of [API-S-034](../../apps/filesquid/crates/api/resources/srs.md) on the desktop). The library folder MUST hold only the library's OCFL storage root: the collection-snapshot object and one object per file, as the FileSquid OCFL application profile defines ([API-S-026](../../apps/filesquid/crates/api/resources/srs.md)). A host MUST NOT create a library's OCFL storage root, originals or metadata records in its data directory. In-app catalog libraries (`libraries/<library-id>.ocfl/` in the data directory) are retired; no library with real data exists in that form, so no migration is provided.
2. **Catalog index in the app data directory.** Each library's catalog index MUST be the SQLite file `libraries/<library-id>.sqlite` in the host's data directory, keyed by the library's UUID v4, as DE-S-009 specifies. It holds no file bytes. It MUST be rebuildable from the library folder alone. Writer staging and version leases MAY stay in the data directory beside it (`libraries/<library-id>.work/`), outside the storage root, as DE-S-018 already requires for owner-chosen folders.
3. **One library model.** Every library has collections, folders and virtual collections ([PRD F-004](../prd.md#f-004-collections-folders-and-navigation)). Every file MUST have exactly one **home collection**. A folder is a prefix of the logical paths of files inside one collection.
4. **Every write names a destination.** An upload, an import and a copy MUST name a destination collection (and MAY name a folder inside it). A move MUST name its destination collection or folder. A request without a destination MUST be refused before anything is staged or any dialog is shown. There is no upload or import at a library's top level and no default collection. Each of these operations MUST register the file in its home collection at its logical path, under the operation's ID, in the collection snapshot, and MUST be journaled like every other domain operation ([ADR 033](./033-activity-journal-and-live-health-monitoring.md)). A file uploaded into a folder therefore appears in that folder at once, and "Drop to upload into {folder}" stays accurate.
5. **The snapshot is the source of truth.** The collection snapshot inside the library folder MUST record, for each file, its home collection, its logical path inside that collection and the identifier of its file object, so the catalog index can be rebuilt from the snapshot alone (amends [ADR 010](./010-ocfl-1-1-dual-profile.md)). The per-library SQLite catalog is a rebuildable index (amends [ADR 005](./005-sqlite-local-postgres-hosted.md)).
6. **Moved and copied libraries rebuild.** Opening a library folder whose catalog index is missing, belongs to another copy, or does not match the snapshot head MUST rebuild the index from the folder, in one journaled reconcile ([DE-S-010](../../apps/filesquid/crates/desktop/resources/srs.md)). A file object that the snapshot does not register (committed by an earlier build without a home collection) MUST be reported by the rebuild and MUST NOT be placed in an invented collection.
7. **The default UI follows the rule.** Upload and import controls MUST target a folder or collection; the drop target MUST appear only over one; the import dialog MUST require a destination before it starts ([UI-S-002](../../apps/filesquid/ui/resources/srs.md)).

## Rationale

- One model removes the gap between the two library kinds: collections, folders and virtual collections work in the libraries the owner actually uses, and the file service serves exactly those libraries.
- Keeping only OCFL in the library folder keeps it clean for ordinary tools and portable: the owner can move, copy or inspect it, and FileSquid rebuilds its index ([FE-065](../product-vision.md#independent-library)). A SQLite file inside the folder would be a second, live format beside OCFL, and copying a live SQLite file is not a supported backup ([ADR 005](./005-sqlite-local-postgres-hosted.md)).
- Refusing a request without a destination keeps PRD F-004's rule that a file's bytes live in exactly one collection, without inventing a default collection that the owner never chose.
- Recording the home collection and logical path in the snapshot keeps the index rebuildable, which the commit sequence of ADR 005 already requires.

## Assumptions

- No library with real data exists in the data directory form (owner statement, 2026-10-22), so retiring it needs no migration.
- The navigation listing ([API-S-021](../../apps/filesquid/crates/api/resources/srs.md)) and the file routes can address a collection and a folder inside it with opaque handles and identifiers, without exposing a filesystem path.
- A library folder can be large; a rebuild of its index stays bounded by the existing snapshot limit (1,000,000 entities, [DE-S-010](../../apps/filesquid/crates/desktop/resources/srs.md)) and runs as a journaled reconcile at open.

## Alternatives Considered

- **Keep both models (catalog libraries and folder libraries):** two code paths for the same product concept, and uploads and imports would still need a rule for which model they land in. Rejected by the owner ("Folders only").
- **Migrate in-app catalog libraries into owner folders:** cost with no benefit, since none holds real data. Rejected by the owner.
- **A default collection for uploads and imports with no destination** (for example "Unsorted"): places files where the owner did not choose and hides them behind a name the owner never created. Rejected by the owner ("Must choose").
- **Upload into the library's top level:** a file outside every collection breaks PRD F-004's "exactly one collection". Rejected by the owner.
- **Catalog index inside the library folder:** keeps the index with the library when it moves, but puts a live SQLite file next to OCFL, where ordinary tools copy it unsafely. Rejected by the owner ("App data directory").

## Consequences / Implications

- Easier: one library model in the API, the desktop host and the UI; uploaded and imported files appear in navigation at once; the folder can be copied or moved with ordinary tools and reopened.
- Harder: the snapshot's file entity gains the logical path and the file object identifier, so the catalog schema moves to version 3 and the snapshot reader accepts both forms; the import route requires `collection_id` and refuses requests without one (replacing `collection_not_supported`); the upload, copy and move routes gain destination parameters under a minor contract bump; uploads, imports and copies commit a file object and then a snapshot version under one operation ID, so a crash between them must leave the operation pending and resumable ([ADR 005](./005-sqlite-local-postgres-hosted.md) commit sequence).
- Planned work (MVP, phases F0 to F3): desktop Task DE-1.7 widened to folder libraries and the retirement of the in-app catalog roots (global Task 1.2); API Task API-F2.14 for uploads, copies and moves (global Task FS-2.5); UI Task UI-F2.17 for the default UI (global Task FS-2.5).
- Left open for the owner (recorded as Conflict Artifacts in the [walkthrough](../tasks/Task-docs-library-model-owner-folders-Walkthrough.md)): requirements that still let a file be "unfiled" (a deleted collection's files, the `is:unfiled` operator and quick filter, the navigation listing's unfiled folders).

## Related Decisions / Notes

- [ADR 002](./002-tauri2-desktop-shell.md) and [ADR 030](./030-filesquid-api-and-transports.md): native folder choice and the location-choices route ("go with A with rfd", 2026-10-20).
- [ADR 005](./005-sqlite-local-postgres-hosted.md): local SQLite catalog; amended here for its location and role.
- [ADR 010](./010-ocfl-1-1-dual-profile.md): OCFL local profile and collection snapshot; amended here for the file entity.
- [ADR 016](./016-rust-ocfl-library.md) and [ADR 023](./023-filesquid-ocfl-engine-and-parquet.md): the enumerator that catalog rebuild consumes.
- [ADR 033](./033-activity-journal-and-live-health-monitoring.md): every upload, import, copy and move is journaled.
- [ADR 038](./038-import-file-record.md): the import record is unchanged; it records where a file came from, not its home collection.
- [ADR 040](./040-prov-o-history.md): one operation that commits a file object and the snapshot gives each object version its own activity under one operation ID.
- SRS rows: [DE-S-009, DE-S-010, DE-S-018, DE-S-019](../../apps/filesquid/crates/desktop/resources/srs.md); [API-S-011, API-S-026, API-S-036](../../apps/filesquid/crates/api/resources/srs.md); [UI-S-002](../../apps/filesquid/ui/resources/srs.md); [SRS-F-001](../srs.md#srs-f-001-independent-local-library-and-ownership), [SRS-F-002](../srs.md#srs-f-002-safe-file-folder-and-device-import), [SRS-F-004](../srs.md#srs-f-004-collections-folders-and-navigation).
