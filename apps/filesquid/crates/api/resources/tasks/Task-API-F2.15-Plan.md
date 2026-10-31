# Task API-F2.15 Plan: Recently deleted, restore, retention and expiry settings, retirement of "unfiled"

Source: [plan row](../plan.md), [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md) Decisions 8 to 12, [API-S-011](../srs.md), [API-S-019](../srs.md), [API-S-021](../srs.md), [API-S-026](../srs.md), [API-S-033](../srs.md), mockups 16, 46 to 50 of [v3](../../../../../../resources/ui-mockups/v3/).

## Constraints copied from the SRS and ADR

- One Recently deleted node (`recently_deleted`) at the repository level, last node, `name` null, `has_children` false; kind order library, collection, virtual_collection, recently_deleted, folder.
- It lists every deleted file of every library this host opened that the caller may read, each with library ID and name, `deleted_at`, `purge_after` under its own library's retention, deletion cause (`one_by_one`, `with_collection`), home collection title (also after that collection is deleted) and folder; no folder rows; `files/listing` refuses the node; no library lists a node of its own.
- `is:unfiled` is an unknown operator (`invalid_request`); `is:deleted` finds files deleted one by one and with their collection.
- Deleting a collection marks its undeleted files deleted in the same command and version; an already deleted file keeps `deleted_at`, `purge_after`; nothing moves; home collection ID and logical path kept.
- Restore returns to the original object and path; home gone: "untitled collection", then "untitled collection 2", ... first free title, at most 9,999 titles (`MAX_UNTITLED_TITLES`), one per gone home per command, one operation, one snapshot version; exhausted: `409 conflict`, code `restore_name_exhausted`, no change, journaled `failed`.
- Retention: installation default 30 days in the operational store, admin only; library's own period in its snapshot (`library.json`), cached in the index; 1 to 3,650 days (`invalid_retention` otherwise); `purge_after` = deletion time + library period else default; a change never moves an existing `purge_after`; every change journaled.
- Expiry setting: `keep_version_history` (default) or `delete_all_version_history`, installation-wide, admin only, journaled; readable by anyone who may read a library.
- Expiry job: durable, registered, capped (API-S-033); one `purge` operation ID per library per run, journaled before or with the deletion; PROV-O activity per committed version; failure journaled `failed` and retried next run; a library closed past `purge_after` expires on the first run after it is opened.
- delete_all: the file object, index entries and metadata record are deleted.
- keep: the object is rewritten as a tombstone with the same identifier at the same path, holding only former version facts (number, created, user, message, digests and sizes), the former PROV-O records and a purge record; its v1 is the tombstone creation (kind `tombstone`, added to `OperationKind`); its history carries the purge event; crash at each step completed by recovery; a library recovered from OCFL alone shows the tombstone as purged history, never as a file.
- Journal entries: opaque IDs only, never names or paths (owner decision 2026-10-31, API-S-036); redaction tests.
- Contract minor bump 0.35.0 to 0.36.0; regenerate `ui/src/api/schema.d.ts`.

## Approach

1. Domain (`library`): `DeletionCause`, `FileEntry.removed_home_title`, `SetLibraryRetention`, `RehomeFile`, `SetRetention`, restore planning with untitled collections, `LibraryError::Refused`, `RetentionSettings` port with `MemorySettings`.
2. Retire `FileScope::Unfiled`, `FolderScope::Unfiled`, `is:unfiled`.
3. Navigation `NodeRef::RecentlyDeleted`, view across libraries, settings and Recently deleted command routes.
4. Expiry module, native pending purges and tombstone writer with recovery, desktop scheduling and operational settings store.
5. Contract, docs, BDD features, walkthrough.

## Verification

Unit tests (`unit_recently_deleted`, navigation, search), integration through the router, native catalog and tombstone crash tests, desktop wiring tests, BDD features, `FULL=1 bash scripts/pre-commit.sh`.
