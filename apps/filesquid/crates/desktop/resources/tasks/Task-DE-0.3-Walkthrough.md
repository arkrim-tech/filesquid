# Task DE-0.3 Walkthrough: Intent → OCFL commit → receipt reconcile

## DE-S-010: Intent, OCFL snapshot commit and receipt reconcile on the desktop

Every library change on the desktop is now canonical OCFL before it is acknowledged, in the order of [ADR 005](../../../../../../resources/architecture/005-sqlite-local-postgres-hosted.md): record intent, publish one snapshot version through the `filesquid-ocfl` change model, then update the SQLite catalog from the published receipt. At open, each catalog is reconciled with (or rebuilt from) its snapshot head.

### SRS mapping

| Requirement | Title | Covered by |
|---|---|---|
| DE-S-010 | Intent, OCFL snapshot commit and receipt reconcile on the desktop | [`tests/unit/canonical_commit.rs`](../../../native/tests/unit/canonical_commit.rs), [`intent_ocfl_commit_reconcile_NFR-001.feature`](../../../native/tests/catalog_features/intent_ocfl_commit_reconcile_NFR-001.feature) |
| API-S-026 | The FileSquid OCFL application profile (snapshot and file-object layout, entity encoding, operation messages, commits through the change model) | [`api/tests/unit/canonical.rs`](../../../api/tests/unit/canonical.rs) |
| API-S-008 | Every content or organisation change commits an OCFL version through the change model; no second write path | both suites: every write goes through `ChangeModel` |
| NFR-001 | Preservation and transactional integrity | `a_crash_before_the_snapshot_leaves_the_work_pending_and_a_retry_completes_once`, `a_crash_after_the_snapshot_loses_no_acknowledged_state`, `an_unpublished_version_left_by_a_crash_is_ignored_and_replaced` |
| F-042 | OCFL preservation and portable recovery | `a_deleted_catalog_is_rebuilt_from_the_snapshot_with_its_receipts`, `removing_a_catalog_file_loses_only_the_rebuildable_index` |
| OCFL-S-018 / OCFL-S-020 | Writer fencing and W010 refusal; repair only with consent | `a_head_without_its_version_inventory_is_refused_and_not_repaired`; fencing is the engine's lease, unchanged, and its named gap stays in OCFL-S-018 |
| DE-S-004 | Desktop links the API, which links `filesquid_ocfl`, and injects the local filesystem backend | `filesquid-native` (behind `desktop`) opens `FsBackend` roots and drives the API's commits |

## What changed

- **`filesquid-api` `canonical` module (new, application profile, no filesystem I/O):** the layout ([`layout.rs`](../../../api/src/canonical/layout.rs)): one collection-snapshot object per library at `snapshot/` with one logical file per entity, and one file object per file at `files/<id>` with `original/<name>`; entity encoding ([`entity.rs`](../../../api/src/canonical/entity.rs)); the operation message that carries a receipt ([`message.rs`](../../../api/src/canonical/message.rs)); the profile transform seam with the owner-readable identity ([`transform.rs`](../../../api/src/canonical/transform.rs)); commits and reads through the change model, generic over the host's backend ([`store.rs`](../../../api/src/canonical/store.rs)). `local_library::root::create_root` is now crate-visible so the storage root is declared by the API's own create rule.
- **`filesquid-native` catalog (behind `desktop`):**
  - [`canonical.rs`](../../../native/src/catalog/canonical.rs) opens each library's OCFL root `<id>.ocfl/` and working directory `<id>.work/` (staging, leases, backend scratch; outside the root), owner-only.
  - [`write.rs`](../../../native/src/catalog/write.rs): a commit records its intent in its own transaction, applies its changes in an open transaction, publishes the touched entities ([`project.rs`](../../../native/src/catalog/project.rs)) as one version between the probe's two stops, then stores revision, snapshot version, receipt, intent removal and journal entry and commits. A failure known to publish nothing discards the intent; a crash or a failure after publication keeps it.
  - [`reconcile.rs`](../../../native/src/catalog/reconcile.rs): at open, a catalog that does not reflect its snapshot head (or is missing; libraries are found from catalog files and OCFL roots) is rebuilt from it, receipts are restored from the version messages, published intents are cleared, journaled as `library.reconcile`.
  - Schema v2 adds `library.snapshot_version` (migration tested from v1).
  - `index_files` and `index_folders` publish their entities the same way.
  - `CommitProbe` (default `NoProbe`) lets crash-injection tests stop a commit just before or just after publication.
- DE-0.2 tests updated where DE-0.3 changes behaviour on purpose: a removed or renamed catalog file is now rebuilt from its library's snapshot instead of losing the library; the schema version is 2; fixture operation IDs are unique across reopened fixtures.

## TDD: the tests failed first

The coordinator tests were written before the coordinator and failed to compile (`unresolved imports filesquid_native::library_ocfl_root, filesquid_native::CommitProbe`). The API's canonical tests were written right after its module skeleton; their first run failed (`E003 ... an object root needs exactly one conformance declaration`), which led to the declaration check in `read_snapshot`.

## Test output

`cargo test -p filesquid-api --locked --test unit_canonical`, `cargo test -p filesquid-native --features desktop --locked --test unit_canonical_commit --test catalog_features` and `cargo test -p filesquid-desktop --locked --test integration_library_catalog`, from `apps/filesquid/`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.23s
     Running tests/unit/canonical.rs (/home/user/.cargo-target-shared/debug/deps/unit_canonical-3d451467f1cf537e)
running 14 tests
test a_file_original_is_its_own_object ... ok
test a_head_without_its_version_inventory_is_refused_and_nothing_is_written ... ok
test a_storage_root_is_created_only_in_an_empty_folder ... ok
test a_stale_draft_left_in_staging_is_never_published ... ok
test a_committed_snapshot_reads_back_entity_for_entity ... ok
test an_entity_that_does_not_match_its_path_is_refused_on_read ... ok
test canonical_failures_keep_their_meaning_at_the_catalog_port ... ok
test each_change_touches_the_library_and_its_own_entity ... ok
test every_entity_has_one_logical_path_and_back ... ok
test identifiers_that_cannot_name_a_path_segment_are_refused ... ok
test the_operation_message_carries_its_receipt ... ok
test an_unreadable_operation_message_is_refused_on_read ... ok
test a_later_version_changes_only_the_entities_it_touches ... ok
test an_entity_file_larger_than_the_bound_is_refused_before_it_is_read ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
    Finished `test` profile [unoptimized] target(s) in 0.18s
     Running tests/catalog_features.rs (/home/user/.cargo-target-shared/debug/deps/catalog_features-4ca93b63c5ddbdf7)
Feature: Intent, OCFL snapshot commit and receipt reconcile
  Scenario: A change is published as an OCFL version before it is acknowledged
   ✔  Given an empty catalog directory
   ✔  When the owner creates the library "Family archive" with the collection "Summer"
   ✔  Then the library's OCFL snapshot should be at version "v2"
   ✔  And no intent should be pending
  Scenario: A crash before the snapshot leaves the work pending
   ✔  Given an empty catalog directory whose commits crash before the snapshot of "op-summer"
   ✔  And the owner created the library "Family archive"
   ✔  When the owner adds the collection "Summer" under operation "op-summer"
   ✔  And the host restarts
   ✔  Then the intent of "op-summer" should be pending
   ✔  And the library's OCFL snapshot should be at version "v1"
   ✔  When the owner adds the collection "Summer" under operation "op-summer"
   ✔  Then the library's OCFL snapshot should be at version "v2"
   ✔  And no intent should be pending
  Scenario: A crash after the snapshot loses no acknowledged state
   ✔  Given an empty catalog directory whose commits crash after the snapshot of "op-summer"
   ✔  And the owner created the library "Family archive"
   ✔  When the owner adds the collection "Summer" under operation "op-summer"
   ✔  And the host restarts
   ✔  Then the collection "Summer" should be listed
   ✔  And no intent should be pending
   ✔  And the library's OCFL snapshot should be at version "v2"
   ✔  And replaying "op-summer" should answer its first result without a new version
  Scenario: A lost catalog is rebuilt from the OCFL snapshot
   ✔  Given an empty catalog directory
   ✔  And the owner created the library "Family archive" with the collection "Summer"
   ✔  When the catalog file is deleted and the host restarts
   ✔  Then the collection "Summer" should be listed
   ✔  And the library "Family archive" should be at revision 2
   ✔  And replaying "op-summer" should answer its first result without a new version
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
2 features
10 scenarios (10 passed)
55 steps (55 passed)
     Running tests/unit/canonical_commit.rs (/home/user/.cargo-target-shared/debug/deps/unit_canonical_commit-b7a31ff4032fc7da)
running 10 tests
test a_crash_before_the_snapshot_leaves_the_work_pending_and_a_retry_completes_once ... ok
test a_command_publishes_one_snapshot_version_before_the_catalog_answers ... ok
test a_crash_after_the_snapshot_loses_no_acknowledged_state ... ok
test a_library_directory_that_is_a_symbolic_link_is_refused ... ok
test a_head_without_its_version_inventory_is_refused_and_not_repaired ... ok
test a_deleted_catalog_is_rebuilt_from_the_snapshot_with_its_receipts ... ok
test every_library_keeps_its_ocfl_root_beside_its_catalog_file ... ok
test an_unpublished_version_left_by_a_crash_is_ignored_and_replaced ... ok
test a_missing_snapshot_is_published_again_in_full_by_the_next_commit ... ok
test a_snapshot_of_another_library_is_never_loaded ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
    Finished `test` profile [unoptimized] target(s) in 16.38s
     Running tests/integration/library_catalog.rs (/home/user/.cargo-target-shared/debug/deps/integration_library_catalog-70df91da0dbf1111)
running 2 tests
test a_catalog_directory_that_cannot_be_opened_is_journaled_without_its_path ... ok
test the_host_keeps_one_catalog_file_per_library_and_journals_its_commits ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

## Review fixes (Cubic round 1)

- Entity reads refuse a file over 1 MiB before reading it and a snapshot of more than 1,000,000 entities before reading any; commits refuse a catalog above that bound before publishing, and projections never truncate.
- A catalog whose OCFL root lost its snapshot forgets its snapshot version at open (logged as a warning), so the next commit publishes every entity again (`a_missing_snapshot_is_published_again_in_full_by_the_next_commit`).
- Library directories that are symbolic links, or resolve outside the catalog directory, are refused before any backend opens them (`a_library_directory_that_is_a_symbolic_link_is_refused`).
- Reconcile refuses a snapshot of another library, or one holding another library's collections (`a_snapshot_of_another_library_is_never_loaded`), and journals restored receipts too.
- The API writes the storage-root declaration itself, so `local_library` stays private; a delta that changes nothing writes no version (index writes then commit without one); a prefixed operation message that cannot be read is refused instead of dropping its receipt; decode errors carry a fixed reason; reservation failures that cannot clear map to `Failed`; `<id>.json` must fit 255 bytes; `CreateLibrary` touches the library entity once; the W010 refusal is also tested in the API on the in-memory backend.
- The no-op receipt gap, the original-name contract and the transform's byte-only scope are documented in DE-S-010, API-S-026 and ADR 010.

## Pre-commit hook

`FULL=1 bash scripts/pre-commit.sh` on the merge of `origin/main` after the Cubic round 1 fixes (`48c7494`):

```text
✅ All Pre-Commit Hooks Passed! Test scope: full (344s)
```

## Owner decisions and follow-ups

- **Layout:** ADR 010 left the local layout "to be specified when the first library writer is built". This task specifies it in API-S-026 and records an amendment note in ADR 010 for the owner to confirm in this pull request.
- **Encrypted profile:** no ADR 009 cipher exists yet, so only the owner-readable identity transform is implemented; the encrypted profile uses the same `ProfileTransform` seam when the cipher lands. The owner accepted on 2026-10-19 that the encrypted profile's path mapping is deferred until the ADR 009 cipher lands.
- **Crash injection:** crashes are simulated by a probe that stops the commit at the exact point a crash would, then the catalog files on disk are reopened as a restarted process would; no process is killed.

## Mocked Environment Variables

None.
