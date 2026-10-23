# Task DE-1.7 Plan: Owner-folder library index, home collections and retirement of in-app catalog roots

Branch: `feat/de-1.7-owner-folder-library-index`. Plan row: [desktop plan, Task DE-1.7](../plan.md#phases-f2-i1-and-f5-library-import-gallery-organize-and-setup-global-tasks-1116). Global task: [Task 1.2](../../../../../../resources/implementation-plan.md#task-1.2). Binding decisions: [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md) (Decisions 1 to 8, 10 and 12; restore, the settings routes and the expiry job are API-F2.15's).

## Traceability

- PRD: [F-001](../../../../../../resources/prd.md#f-001-independent-local-library-and-ownership), [F-002](../../../../../../resources/prd.md#f-002-safe-file-folder-and-device-import), [F-004](../../../../../../resources/prd.md#f-004-collections-folders-and-navigation), [F-008](../../../../../../resources/prd.md#f-008-deletion-hidden-files-and-duplicate-review), [F-009](../../../../../../resources/prd.md#f-009-multiple-libraries-and-filesystem-administration).
- SRS: [SRS-F-001](../../../../../../resources/srs.md#srs-f-001-independent-local-library-and-ownership), [SRS-F-004](../../../../../../resources/srs.md#srs-f-004-collections-folders-and-navigation); desktop [DE-S-009, DE-S-010, DE-S-018, DE-S-019](../srs.md); API [API-S-011, API-S-019, API-S-021, API-S-026, API-S-036](../../../api/resources/srs.md).
- Features: [`desktop_imports_F-002.feature`](../../tests/features/desktop_imports_F-002.feature), [`desktop_libraries_F-001.feature`](../../tests/features/desktop_libraries_F-001.feature), [`navigation_listing_API-S-021.feature`](../../../api/tests/features/navigation_listing_API-S-021.feature), [`library_domain_commands_and_queries_API-S-019.feature`](../../../api/tests/features/library_domain_commands_and_queries_API-S-019.feature), and native [`per_library_sqlite_catalog_F-001.feature`](../../../native/tests/catalog_features/per_library_sqlite_catalog_F-001.feature).

## Constraints extracted from the SRS rows and ADR 041

### Library model (ADR 041 Decisions 1, 2, 6; DE-S-009, DE-S-018)

- C1. Every library MUST live in a folder the owner chose; the library folder holds only its OCFL storage root (snapshot object, one object per file, journal and history objects).
- C2. The data directory MUST NOT hold a library's OCFL storage root, originals or metadata records; `libraries/<library-id>.ocfl/` is retired (`OCFL_ROOT_SUFFIX` removed), with no migration (owner statement 2026-10-22: no in-app library holds real data).
- C3. The host MUST NOT create a library anywhere but in an owner-chosen folder: the catalog refuses a `CreateLibrary` commit; a library is created by the library routes (API-S-025) in a chosen folder.
- C4. Each library's catalog index is `<data dir>/libraries/<library-id>.sqlite`, named only by the canonical UUID v4, owner-only (file 0600, folder 0700), WAL mode; it holds no file bytes.
- C5. Staging and leases MAY stay in `<data dir>/libraries/<library-id>.work/` (and the location's leases under `<data dir>/locations/<digest>/`), outside the storage root. A work directory that is a symbolic link or resolves outside the catalog directory MUST be refused.
- C6. Each folder library's snapshot commits and file commits share one set of leases (DE-S-018: one folder registered twice shares one backend and one set of leases).
- C7. The operational store (`filesquid.sqlite`, journal and jobs) stays in the data directory as before.
- C8. The catalog stores the library row, folders, collections, virtual collections, memberships, files with home collection, logical path, file object identifier and hide and delete state, the library's own retention cached from its snapshot, receipts and pending intent, under `PRAGMA user_version`, migrated forward and refused when newer than the host.
- C9. Opening a folder whose index is missing, belongs to another copy (another folder) or does not match the snapshot head MUST rebuild the index from the folder in one journaled `library.reconcile`.
- C10. A copy keeps its library UUID: while one folder of a library is open, opening another folder with the same UUID MUST be refused (`library_already_open`); two folders never share one index. Opening a folder answers the library its snapshot names.
- C11. The installation's default retention (30 days) and expiry setting live in the operational store, never in a catalog index; storing and changing them is API-F2.15's (not this task). This task uses the 30-day default.
- C12. Errors MUST NOT name a path; the webview MUST NOT receive a path or SQL (DE-S-006).
- C13. A consistent backup uses `VACUUM INTO` and is journaled (kept as is).

### Commit sequence (DE-S-010)

- C14. Intent (operation ID, operation, expected revision) durable in the catalog before any OCFL byte; changes applied in an open transaction; the touched entities published as one snapshot version; only then the revision, snapshot version, receipt, journal entry and intent removal committed.
- C15. A failure known to publish nothing discards the intent and leaves catalog and head unchanged; a crash before the version is published leaves the intent pending; a retry under the same operation ID completes once.
- C16. Before publishing, a snapshot above 1,000,000 entities MUST be refused.
- C17. A snapshot that names another library or holds an entity of another library MUST NOT be loaded.
- C18. A catalog whose OCFL root has lost its snapshot MUST publish every entity again at its next commit.
- C19. An index write that changes nothing publishes no version and no journal entry; a failed one is journaled `failed` with its error code (outbox, or the bounded queue when the library is not open).
- C20. A W010 head MUST be refused and never repaired implicitly.

### Snapshot (API-S-026, ADR 041 Decision 5)

- C21. `files/<id>.json` gains the file's collection logical path and its file object identifier; the library entity gains its own retention period when set.
- C22. Collection logical path: a JSON value (not an OCFL logical path), 1 to 1,024 UTF-8 bytes in NFC, `/`-separated segments of 1 to 255 bytes without `\`, NUL or control characters, not `.` or `..`; the last segment is the file's name after the API-S-005 safe transformation.
- C23. A name still longer than 255 bytes after the safe transformation MUST be refused as `invalid_request` (`name_too_long`); an import quarantines that file with the same code.
- C24. A reader MUST accept a file entity written before these fields existed and report it as unregistered.
- C25. A deleted file's entity keeps its home collection identifier (even after the collection is deleted), its logical path and object identifier, and adds `deleted_at` and `purge_after`.
- C26. Reads keep refusing an entity file above 1 MiB, verifying digests, refusing more than 1,000,000 entities; decode errors carry a fixed reason, never a stored value.

### Catalog schema v3 (DE-1.7 row)

- C27. A forward migration v2 to v3 adds the file's logical path and object identifier (and the derived columns that answer folder listings), the library's retention cache and the identity of the folder the index was built from; a catalog newer than the host is refused as before.

### Import destination (API-S-036, DE-S-019, ADR 041 Decision 4)

- C28. `POST /api/v1/imports` requires `collection_id` and accepts an optional `folder` (a collection logical folder path, not the reserved `provenance` or `.filesquid` folders).
- C29. A request without `collection_id` MUST be refused as `400 invalid_request` and journaled once with `destination_required`, before any dialog.
- C30. An unknown collection, or one of another library, MUST be refused as `404 not_found` (`collection_not_found`) before any dialog.
- C31. `collection_not_supported` is retired.
- C32. The destination is kept in the job's plan (DE-S-019), keyed by the job and the operation ID.
- C33. Minor contract bump (one minor above main's version at the last merge of `origin/main`; 0.24.0 today, so 0.25.0), regenerated `schema.d.ts`.
- C34. No request, answer, error or journal entry carries a path or a file or folder name.

### Index imports (DE-S-019 **Commit**, API-S-036 **One meaning**)

- C35. Each imported file is registered in the destination collection at the logical path of its manifest entry beneath the destination folder (a picked folder keeps its structure), in the snapshot under the import's operation ID, then in the index.
- C36. A crash between the file object's version and the snapshot version leaves the operation pending and resumable: the intent of the import's operation is recorded before the file is committed and stays pending until the job finishes; a resumed run answers the committed object and registers it once.
- C37. A file already registered under its file ID is not registered again (no second number, no second version).

### Same logical path (API-S-011, ADR 041 Decision 12)

- C38. A picked folder or file whose path is already taken in the collection, a deleted file's path included, gets a hyphen and the first free number from 1, beside the existing one; never refused, never overwritten.
- C39. Folders: the number at the end (`directoryY-1`, `directoryY-2`).
- C40. Files: the number after the first part of the name, before the first dot (`photo-1.jpg`, `archive-1.tar.gz`); a name without a dot takes it at its end; a name that starts with a dot keeps that dot in its first part (`.env-1`, `.config-1.json`).
- C41. At most 9,999 numbers (`-1` to `-9999`); then the file is quarantined and journaled as `path_suffix_exhausted`.
- C42. A numbered segment over 255 bytes is `name_too_long`.
- C43. Stored names `<file-uuid>.<ext>` never collide; numbering concerns logical paths only.

### Rebuild from the snapshot (DE-S-010)

- C44. The rebuild restores every file entity with home collection, logical path and object identifier, receipts from the version messages, and clears intents of published operations.
- C45. A file object the snapshot does not register is counted in the reconcile's journal entry and left unregistered, never placed in an invented collection.

### Navigation and listing (DE-S-018, API-S-021, API-S-011)

- C46. `CatalogFolders` answers the subfolders of a collection or folder node from the index; a folder exists only when a committed logical path implies it; a folder implied only by hidden or deleted files MUST NOT be listed; folders by name in Unicode code-point order; at most `limit` (1 to 1,000).
- C47. `CatalogScopes` answers what `GET /api/v1/files/listing?node=` lists for a collection or folder node from the index: its subfolders, then its registered ordinary files, each with its file object, so every registered file appears under its collection and folder; no filesystem path in the answer.
- C48. The listing leaves out the reserved `provenance` and `.filesquid` folders; a malformed cursor or a limit outside 1 to 1,000 is `invalid_request`.

### Deletion as a state (API-S-019, ADR 041 Decision 8)

- C49. Deleting a collection that holds files publishes ONE snapshot version (one operation) that removes the collection and records each file that is not already deleted as deleted, with `deleted_at` and `purge_after` = deletion time + the library's retention when set, else 30 days; each keeps its home collection identifier and logical path; nothing is moved.
- C50. A file already deleted keeps its earlier `deleted_at` and `purge_after`.
- C51. Only then is the index updated, in one catalog transaction; a rebuild from that snapshot restores the same deleted state.
- C52. A file deleted on its own is stored in the same deleted state (existing `DeleteFiles`).
- C53. The library's own retention period (1 to 3,650 days when set) is kept in the snapshot, cached in the index and survives a rebuild.

### Journal and redaction (ADR 033, DE-S-006)

- C54. No answer carries a filesystem path; no journal entry carries a path or a file or folder name (redaction tests).
- C55. Every domain operation (import registration, collection deletion, reconcile) is journaled with its version; refusals journaled once.

### README

- C56. Root README's catalog and commit sections (Tasks DE-0.2 and DE-0.3) no longer describe `libraries/<library-id>.ocfl/` in the data directory.

## Technical approach

1. **API canonical** (`crates/api/src/canonical`): `SnapshotFile` gains `path` and `object_id` (both optional for the earlier form), validated on decode; `LibraryHead` gains `retention_days`. New pure module `collection_path` with the path rule (C22), the safe segment (C23), the numbering rule (C38 to C42) and a bounded `free_path` search. `unregistered_objects(backend, snapshot)` counts file objects the snapshot does not register (C45).
2. **API library** (`crates/api/src/library`): `LibraryState` carries the library's retention; `DeleteCollection` plans `CatalogChange::DeleteCollectionFiles` (deletion with the library's retention) before `RemoveCollection`; `MemoryCatalog` applies it too; the receipt carries the deletion when files may be newly deleted.
3. **API imports** (`crates/api/src/imports.rs`): `collection_id` required (`destination_required`), optional `folder` (checked as a collection logical folder path), `collection_not_found` through the navigation service's catalog, `collection_not_supported` removed.
4. **API local library**: open answers the library its snapshot names (C10).
5. **API files listing by node**: `ScopeObjects` gains a scope page (subfolders and files with their objects) answered by the host index; the listing route builds the page from it, reading each file's head for its facts.
6. **Native catalog**: schema v3; `open` serves no library and only forwards outboxes; `attach(LibraryFolder)` binds a folder library (creating its library entity when the folder has no snapshot, rebuilding when the index is missing, of another folder or behind); `register_files` for imports; folder and scope queries; `CreateLibrary` refused; `.ocfl` roots retired.
7. **Native import**: plan keeps `collection_id` and `folder`; manifest keeps each file's logical path (numbered per picked source at manifest time); each committed file is registered; `name_too_long` and `path_suffix_exhausted` quarantine; intent kept pending until the job finishes.
8. **Desktop**: attaches each bound library to the catalog; `CatalogFolders` and `CatalogScopes` over the catalog index; import starter keeps the destination.
9. **Contract** 0.25.0 and `schema.d.ts`; README; SRS/PRD cascade; BDD scenarios.

## Verification

- TDD: failing tests first in `crates/api/tests/unit`, `crates/native/tests/unit`, `crates/desktop/tests/integration`, BDD features; run them failing, then implement.
- `cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test -p filesquid-api -p filesquid-native -p filesquid-desktop --locked`, `cargo doc --workspace --no-deps`; `npm run contract:types` and the Vitest contract test in `apps/filesquid/ui`.
- Head commit: `FULL=1 bash scripts/pre-commit.sh` (scope includes `rust:`), summary line in the walkthrough.
