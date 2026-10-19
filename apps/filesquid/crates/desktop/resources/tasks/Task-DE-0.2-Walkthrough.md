# Task DE-0.2 Walkthrough: Per-library SQLite catalog

## DE-S-009: Per-library SQLite catalog storage of the desktop host

Each library now has its own SQLite catalog file, the desktop's native storage behind the `filesquid-api` library catalog port. Command and query meaning stays in `filesquid-api` (`LibraryService`); the storage is `SqliteCatalog` in `filesquid-native`, behind its `desktop` feature (owner decision: `filesquid-native` holds host-side native I/O, SQLite storage included; only `filesquid-desktop` enables `desktop`; `filesquid-api` and the module crates do not depend on it). The desktop host opens the catalogs at launch beside its activity journal.

### SRS mapping

| Requirement | Title | Covered by |
|---|---|---|
| DE-S-009 | Per-library SQLite catalog storage of the desktop host | [`tests/unit/catalog.rs`](../../../native/tests/unit/catalog.rs), [`per_library_sqlite_catalog_F-001.feature`](../../../native/tests/catalog_features/per_library_sqlite_catalog_F-001.feature), [`tests/integration/library_catalog.rs`](../../tests/integration/library_catalog.rs) |
| SRS-F-001 | Independent local library and ownership | one file per library, state survives a restart, the catalog is a rebuildable index (`removing_a_catalog_file_loses_only_the_rebuildable_index`), consistent backup |
| PRD F-009 | Multiple libraries and filesystem administration | `each_library_has_its_own_catalog_file_named_by_its_id`, `libraries_list_by_repository_in_creation_order`, `a_receipt_in_one_library_answers_a_commit_naming_another` |
| DE-S-006 | The webview receives opaque handles and API responses, never raw filesystem paths or SQL | `an_unusable_root_fails_without_naming_its_path`, `the_schema_is_versioned_and_a_newer_catalog_is_refused`, `a_catalog_directory_that_cannot_be_opened_is_journaled_without_its_path` |
| SRS-F-061 (logging-enforce.md §2) | Activity journal: every domain operation journaled in the same transaction, nothing dropped, redacted | `every_commit_journal_entry_reaches_the_host_journal_redacted`, `entries_the_host_journal_refused_are_forwarded_at_the_next_open`, `the_host_keeps_one_catalog_file_per_library_and_journals_its_commits` |

## What changed

- **`filesquid-native` `catalog` module** (`desktop` feature): [`mod.rs`](../../../native/src/catalog/mod.rs) (`SqliteCatalog`, the `LibraryCatalog` implementation), [`schema.rs`](../../../native/src/catalog/schema.rs) (schema v1 and migration by `PRAGMA user_version`), [`write.rs`](../../../native/src/catalog/write.rs) (one transaction per commit), [`rows.rs`](../../../native/src/catalog/rows.rs) (indexed reads and paging), [`outbox.rs`](../../../native/src/catalog/outbox.rs) (transactional journal outbox), [`intent.rs`](../../../native/src/catalog/intent.rs) (pending intent).
  - One file per library, `<data dir>/libraries/<library-id>.sqlite`; only a canonical UUID v4 names a file, so no identifier can reach another path. Files are created 0600, the folder 0700, WAL mode, foreign keys on.
  - Schema v1: `library` (one row), `folders`, `collections`, `virtual_collections`, `virtual_members` (cascading from virtual collections and files, so a virtual collection's removal or a purge drops memberships while a collection's removal keeps its files unfiled), `files` (hide and delete state), `receipts`, `pending_intents`, `journal_outbox`. A newer schema is refused.
  - Commit order as the port states: a stored receipt of the operation ID in any library answers first; then the create or revision rule; then every change, the revision, the receipt, the removal of the operation's pending intent and the journal entry in one transaction. A failure rolls everything back, and a failed create removes the file it made.
  - Journal entries are written redacted into the file's outbox in the commit transaction, then forwarded to the host's activity journal store; entries the store refuses stay and are forwarded by the next commit or at the next open (bounded batches). Nothing is dropped; a crash between append and delete can forward an entry twice.
  - `index_files` and `index_folders` store files and folders as an import or a rebuild from OCFL will (DE-0.3, DE-1.1, DE-1.2), with their journal entry in the same transaction.
  - `record_intent` and `pending_intents`: the first step of the ADR 005 commit sequence that DE-0.3 builds on.
  - `backup`: a consistent copy with `VACUUM INTO`, journaled as `library.catalog.backup`; a non-empty destination is refused. A copy of the live file is not a supported backup.
- **Desktop host** ([`src/lib.rs`](../../src/lib.rs)): `open_library_catalog` opens the catalogs in `<app data>/libraries/` at launch, forwarding to the host journal store, and keeps them in Tauri state (`DesktopCatalog`) for the API's library routes. A failure is journaled as `library.catalog.open` `failed` with its error code only, the monitor is stopped (flushing the journal), and the launch fails.
- **`filesquid-api` `events.rs`**: Rust 1.99.1 (today's `stable`, which `rust-toolchain.toml` follows) deprecated `AtomicUsize::fetch_update`, so `clippy -D warnings` failed on `main`. The subscriber-slot claim now uses a bounded compare-and-swap loop (at most 1,024 attempts), which keeps the cap check and the increment one atomic step; the existing event-bus tests pass. The orchestrator is landing the same fix separately; when it merges, merging `origin/main` takes `main`'s version.
- `crates/native/tests/features.rs` now replaces only its own JUnit report instead of deleting `tests/allure-results/`, so the catalog harness's report survives.

## TDD: the tests failed first

```text
      1 error: could not compile `filesquid-native` (test "unit_catalog") due to 4 previous errors; 1 warning emitted
      2 error[E0432]: unresolved import `filesquid_core`
      1 error[E0432]: unresolved import `filesquid_native::SqliteCatalog`
      1 error[E0432]: unresolved imports `filesquid_native::PendingIntent`, `filesquid_native::SqliteCatalog`, `filesquid_native::CATALOG_DIR`, `filesquid_native::CATALOG_FILE_EXTENSION`, `filesquid_native::CATALOG_SCHEMA_VERSION`
```

## Test output

`cargo test -p filesquid-native --features desktop --locked --test unit_catalog --test catalog_features` and `cargo test -p filesquid-desktop --locked --test integration_library_catalog`, from `apps/filesquid/`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.19s
     Running tests/catalog_features.rs (/home/user/.cargo-target-shared/debug/deps/catalog_features-a4032c5136c3373a)
Feature: Per-library SQLite catalog
  Scenario: Each library keeps its own catalog file
   ✔  Given an empty catalog directory
   ✔  When the owner creates the libraries "Family archive" and "Work"
   ✔  Then the catalog directory should hold one catalog file per library
   ✔  And no catalog file should be named after a library name
  Scenario: Organisation survives a restart
   ✔  Given an empty catalog directory
   ✔  And the owner created the library "Family archive" with the collection "Summer"
   ✔  When the catalog is closed and opened again
   ✔  Then the library "Family archive" should be at revision 2
   ✔  And the collection "Summer" should still be listed
  Scenario: A retried command is answered once
   ✔  Given an empty catalog directory
   ✔  And the owner created the library "Family archive" with the collection "Summer"
   ✔  When the same collection command is retried after a restart
   ✔  Then the retry should answer the first result
   ✔  And the library "Family archive" should be at revision 2
  Scenario: Journal entries survive an unreachable host journal
   ✔  Given an empty catalog directory whose host journal is unreachable
   ✔  When the owner creates the library "Family archive"
   ✔  And the host journal is reachable again when the catalog reopens
   ✔  Then the host journal should record "library.create"
  Scenario: A command that cannot be stored changes nothing
   ✔  Given an empty catalog directory
   ✔  And the owner created the library "Family archive" with the collection "Summer"
   ✔  When a commit with a membership of a missing file is applied
   ✔  Then the commit should fail
   ✔  And the library "Family archive" should be at revision 2
  Scenario: A catalog is an index that a backup copies consistently
   ✔  Given an empty catalog directory
   ✔  And the owner created the library "Family archive" with the collection "Summer"
   ✔  When the owner backs up the catalog of "Family archive"
   ✔  Then the backup should open as a catalog at revision 2
   ✔  And the host journal should record "library.catalog.backup"
[Summary]
1 feature
6 scenarios (6 passed)
28 steps (28 passed)
     Running tests/unit/catalog.rs (/home/user/.cargo-target-shared/debug/deps/unit_catalog-6c982b023271d0b5)
running 29 tests
test a_catalog_file_that_is_a_symbolic_link_is_not_followed ... ok
test a_failed_create_leaves_no_catalog_file ... ok
test a_commit_without_change_keeps_the_revision_but_stores_its_receipt ... ok
test a_library_id_that_is_not_a_uuid_is_refused_without_a_file ... ok
test a_failed_commit_leaves_the_catalog_unchanged ... ok
test a_consistent_backup_holds_the_committed_library_and_is_journaled ... ok
test a_repeated_operation_id_answers_its_first_receipt_even_after_reopen ... ok
test an_unusable_root_fails_without_naming_its_path ... ok
test a_renamed_catalog_file_is_not_served_under_another_library ... ok
test catalog_files_are_owner_only ... ok
test a_receipt_in_one_library_answers_a_commit_naming_another ... ok
test an_intent_recorded_after_its_commit_is_not_stored ... ok
test creating_an_existing_library_conflicts ... ok
test committed_state_survives_a_reopen ... ok
test deleting_a_collection_keeps_its_files_as_unfiled ... ok
test deleting_a_virtual_collection_or_purging_a_file_removes_its_memberships ... ok
test every_commit_journal_entry_reaches_the_host_journal_redacted ... ok
test entries_the_host_journal_refused_are_forwarded_at_the_next_open ... ok
test each_library_has_its_own_catalog_file_named_by_its_id ... ok
test folders_are_indexed_and_read_back ... ok
test listings_page_by_order_then_id_and_hide_hidden_and_deleted_files ... ok
test pending_intent_is_recorded_and_cleared_by_its_commit ... ok
test removing_a_catalog_file_loses_only_the_rebuildable_index ... ok
test the_catalog_directory_name_is_stable ... ok
test libraries_list_by_repository_in_creation_order ... ok
test revisions_start_at_one_advance_on_change_and_stale_ones_conflict ... ok
test the_schema_is_versioned_and_a_newer_catalog_is_refused ... ok
test the_sqlite_catalog_answers_like_the_api_reference_catalog ... ok
test no_more_than_the_library_limit_can_be_created ... ok
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.17s
    Finished `test` profile [unoptimized] target(s) in 13.37s
     Running tests/integration/library_catalog.rs (/home/user/.cargo-target-shared/debug/deps/integration_library_catalog-f5487c7ef362ea6f)
running 2 tests
test a_catalog_directory_that_cannot_be_opened_is_journaled_without_its_path ... ok
test the_host_keeps_one_catalog_file_per_library_and_journals_its_commits ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

## Review fixes (Cubic round 1)

- A symbolic link where a catalog file belongs is refused, never followed: `create_owner_only` (now in the neutral `owner_only` module) checks for a link and opens with `O_NOFOLLOW`, and SQLite opens catalogs with `SQLITE_OPEN_NOFOLLOW` (`a_catalog_file_that_is_a_symbolic_link_is_not_followed`).
- A catalog file whose library row names another library than its file name is not served (`a_renamed_catalog_file_is_not_served_under_another_library`).
- Creating a library past `MAX_LIBRARIES` is refused, so the next launch can always open the directory (`no_more_than_the_library_limit_can_be_created`).
- An intent recorded after its operation committed is not stored (`an_intent_recorded_after_its_commit_is_not_stored`).
- The BDD feature folder is tracked (`.gitignore` exception for `tests/catalog_features/`), and test fixtures remove their temporary directories.
- The subscriber-slot claim uses a bounded compare-and-swap, so a refusal never comes from a transient over-count.

## Owner decision (resolved 2026-10-18)

- This task followed the `rusqlite` precedent of `filesquid-native` (CL-0.1) for the per-library catalogs rather than adding a second SQLite driver. The owner confirmed it on 2026-10-18: [ADR 005](../../../../../../resources/architecture/005-sqlite-local-postgres-hosted.md) now selects `rusqlite` with the bundled SQLite for local SQLite stores and keeps SQLx for the planned PostgreSQL store.

## Mocked Environment Variables

None.
