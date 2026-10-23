# Task DE-1.7 ToDos

Constraint IDs refer to [Task-DE-1.7-Plan.md](./Task-DE-1.7-Plan.md).

## Failing tests first

- [ ] Native: no library root is ever created in the data directory; a `CreateLibrary` commit is refused (C2, C3)
- [ ] Native: attach creates `<id>.sqlite` (0600, WAL) and nothing else in `libraries/` besides `<id>.work/` (C4, C5)
- [ ] Native: migration v2 to v3 keeps rows and adds the new columns; a newer catalog is refused (C27, C8)
- [ ] Native: a folder copied to a new location and attached rebuilds its index with the same collections, folders and files (C9, C44)
- [ ] Native: an index of another folder of the same library is rebuilt (C9)
- [ ] Native: a file object without a snapshot entry is counted in the reconcile entry and left unregistered (C45, C24)
- [ ] Native: registering imported files publishes their path and object ID under the import's operation ID, then indexes them; a second registration of the same file changes nothing (C35, C37)
- [ ] Native: deleting a collection that holds files publishes one version marking them deleted, keeping home and path; a rebuild restores the same state; an already deleted file keeps its times (C49 to C51)
- [ ] Native: the library's retention survives a rebuild and sets `purge_after` (C53)
- [ ] Native: folder and scope queries list subfolders and files of a collection and folder, leaving out hidden and deleted ones, in code-point order (C46, C47)
- [ ] API: snapshot file entity with path and object ID round-trips; the earlier form is read and reported unregistered; a bad path is refused with a fixed reason (C21, C22, C24, C26)
- [ ] API: numbering rule (`directoryY-1`, `photo-1.jpg`, `archive-1.tar.gz`, `.env-1`, `.config-1.json`, no-dot names), cap 9,999 then `path_suffix_exhausted`, `name_too_long` over 255 bytes (C38 to C42, C23)
- [ ] API: import without `collection_id` is `400 invalid_request` journaled once as `destination_required` before any dialog; unknown collection `404 collection_not_found`; `folder` checked (C28 to C31, C34)
- [ ] API: delete collection with files plans the deletion of its non-deleted files with the library's retention (C49, C50, C53)
- [ ] API: opening a folder answers the library its snapshot names; a second folder of an open library is `library_already_open` (C10)
- [ ] API: listing by node answers the index's folders and files (C47, C48)
- [ ] Desktop: the import route requires a destination and keeps it in the plan; an import into a collection and folder registers each file (C28 to C32, C35)
- [ ] Desktop: a crash between the file version and the snapshot version resumes and registers once (C36)
- [ ] Desktop: navigation and listing routes list the imported folders and files (C46, C47)
- [ ] Desktop: imported name over 255 bytes is quarantined `name_too_long`; taken paths numbered; deleted paths count as taken (C23, C38 to C42)
- [ ] Redaction: no path in any answer; no path, file or folder name in any journal entry (C12, C34, C54)
- [ ] Run the new tests and record that they fail

## Implementation

- [ ] API canonical: `SnapshotFile.path`, `SnapshotFile.object_id`, `LibraryHead.retention_days`, decode checks (C21 to C26)
- [ ] API canonical: `collection_path` module (C22, C23, C38 to C43)
- [ ] API canonical: unregistered file objects count (C45)
- [ ] API library: `LibraryState.retention_days`, `DeleteCollectionFiles`, MemoryCatalog (C49 to C53)
- [ ] API imports: destination fields and codes (C28 to C34)
- [ ] API local library: open by the snapshot's library (C10)
- [ ] API files: listing by node from the scope page port (C47, C48)
- [ ] Native catalog: schema v3 (C27)
- [ ] Native catalog: retire `.ocfl` roots, refuse create, `open` forwards outboxes only (C2, C3, C7)
- [ ] Native catalog: `attach` with rebuild for missing, other-folder and behind indexes (C9, C10, C17, C18, C44, C45)
- [ ] Native catalog: shared leases with the file service (C6)
- [ ] Native catalog: `register_files` (C35 to C37, C14, C19)
- [ ] Native catalog: folder and scope queries (C46, C47)
- [ ] Native import: plan destination, manifest logical paths and numbering, registration, quarantine codes (C32, C35 to C42)
- [ ] Desktop: attach on create and open; `CatalogFolders`, `CatalogScopes`; import starter (C6, C46, C47, C32)
- [ ] Contract 0.25.0 and `schema.d.ts` regenerated (C33)
- [ ] README catalog and commit sections (C56)
- [ ] SRS and PRD cascade; BDD scenarios with `@feature-writer`

## Verification

- [ ] `cargo fmt --all`, clippy, tests of api, native and desktop, rustdoc
- [ ] UI contract test
- [ ] `FULL=1 bash scripts/pre-commit.sh` on the head commit
- [ ] Walkthrough with raw output; delete Plan and ToDos; tick plan and checklist; `python3 scripts/docs/update_mvp_progress.py`
