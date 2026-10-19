# Task DE-0.10 Walkthrough: Installation UUID, native journal columns, outbox events and coordinator commit linkage

Branch: `feat/prov-o-history` (global Task FS-2.8; ADR 040 Decisions 4 and 6; [ADR 021](../../../../../../resources/architecture/021-uuid-v4-identifiers.md)).

## Requirements

- DE-S-012, installation UUID: one UUID v4 per installation, generated once on first run, persisted with the host's data and stable across restarts; it is the owner agent of every PROV-O record and the OCFL version `user` address.
- DE-S-011, native journal linkage and coordinator commits: the native journal stores `operation_id` and `object_ref` and hands out IDs before a commit. Every catalog commit made with a provenance context (the desktop always configures one) carries its PROV-O record and journal entries in the version it publishes; without one, or when the IDs cannot be allocated, the version still names its operation and the journal links it through `ocfl.commit` (and `prov.record` reports the missing record).

## What changed

- `filesquid-native::installation::load_or_create`:
  - generates the installation UUID v4 from the OS random source on the first start that finds none;
  - stores it in `<data dir>/installation.json` (owner-only, written through a renamed temporary file) and in the `host_installation` table of the operational store `filesquid.sqlite`;
  - reads it back at every start and restores a missing copy from the other;
  - refuses a malformed copy or two copies that disagree. Errors name no path and no value.
- The desktop reads it at launch (`load_installation`), journals `host.installation.created` (or `host.installation.restored`, or `host.installation` `failed` with an error code), never the UUID, and keeps it as `DesktopInstallation` in Tauri's managed state for the API.
- The SQLite journal gains the `operation_id` and `object_ref` columns and indexes, added to an existing journal by an idempotent forward migration (`ALTER TABLE ... ADD COLUMN`; old rows read them as absent). It also gains `allocate_ids` (it advances the AUTOINCREMENT counter) and stores an entry with a preset ID under that ID, once.
- The catalog's journal outbox reaches the host journal through `Monitor::announcing_store`, so outbox entries raise `journal.entry` events and count in the metrics. The new test fails with the previous plain store, as recorded below.

## Catalog commit linkage (after DE-0.3 merged)

DE-0.3 reached `main` while this task was open, so the catalog's snapshot commits are now linked:

- `SqliteCatalog::with_provenance(ProvenanceContext)` (the desktop passes its installation UUID and `host_facts()`) makes every snapshot version carry the installation as its OCFL `user`, a `filesquid.operation.v1` message with the operation `kind`, and the operation's PROV-O record and journal entries (`provenance/<version>.jsonld`, `provenance/<version>.journal.jsonl`) in the same version, through `api::history::CommitRecorder`.
- The commit's journal IDs (its `ocfl.commit` entry and the operation's own entry) are allocated from the host journal before the commit; the outbox stores exactly the entries the version carries, with the same IDs.
- Reconcile replays the journal entries of every version a behind (or lost) catalog missed into its outbox; the stores keep a pre-allocated ID once (`ON CONFLICT(id) DO NOTHING`), so nothing is stored twice.
- Without a provenance context, or when the IDs cannot be allocated, the version is still published with its message; the outbox stores `ocfl.commit` and the operation's entry linked by operation ID and object version, plus a `prov.record` warning (error code only) when the IDs were unavailable.

File objects (DE-1.2) do not exist yet; they use the same recorder when they land. DE-0.8, a listed dependency, is not done.

```text
$ cargo test -p filesquid-native --features desktop --test unit_canonical_commit
test a_commit_without_journal_ids_publishes_and_warns_without_identifying_data ... ok
test a_command_publishes_one_snapshot_version_before_the_catalog_answers ... ok
test a_crash_after_the_snapshot_replays_the_commits_journal_entries_once ... ok
test a_crash_after_the_snapshot_loses_no_acknowledged_state ... ok
test a_crash_before_the_snapshot_leaves_the_work_pending_and_a_retry_completes_once ... ok
test a_library_directory_that_is_a_symbolic_link_is_refused ... ok
test a_head_without_its_version_inventory_is_refused_and_not_repaired ... ok
test a_deleted_catalog_is_rebuilt_from_the_snapshot_with_its_receipts ... ok
test a_missing_snapshot_is_published_again_in_full_by_the_next_commit ... ok
test an_unpublished_version_left_by_a_crash_is_ignored_and_replaced ... ok
test every_library_keeps_its_ocfl_root_beside_its_catalog_file ... ok
test a_snapshot_of_another_library_is_never_loaded ... ok
test an_unrecorded_catalog_still_links_its_commits ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
$ cargo test -p filesquid-desktop --test integration_library_catalog
test a_catalog_directory_that_cannot_be_opened_is_journaled_without_its_path ... ok
test catalog_commits_raise_journal_entry_events ... ok
test the_installation_uuid_is_created_once_and_journaled ... ok
test a_catalog_commit_carries_its_history_in_the_same_version ... ok
test the_host_keeps_one_catalog_file_per_library_and_journals_its_commits ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

## SRS traceability

| Requirement | Tests |
|---|---|
| DE-S-012 | `filesquid-native` `unit_installation` (8 tests) and the in-module rollback tests of `installation.rs` (2); desktop `integration_library_catalog::the_installation_uuid_is_created_once_and_journaled` |
| DE-S-011 journal columns and migration | `unit_journal::both_stores_keep_the_operation_id_and_object_reference`, `an_older_journal_gains_the_linkage_columns_on_open`, `both_stores_assign_ids_before_the_entries_are_written` |
| DE-S-011 outbox events | desktop `integration_library_catalog::catalog_commits_raise_journal_entry_events` |
| DE-S-011 commit linkage, history in the same version | desktop `integration_library_catalog::a_catalog_commit_carries_its_history_in_the_same_version`; native `unit_canonical_commit::an_unrecorded_catalog_still_links_its_commits` |
| DE-S-011 replay after a crash | native `unit_canonical_commit::a_crash_after_the_snapshot_replays_the_commits_journal_entries_once` |
| `prov.record` redaction | native `unit_canonical_commit::a_commit_without_journal_ids_publishes_and_warns_without_identifying_data` |

## TDD: failing runs

The installation module before it existed:

```text
      1 error: could not compile `filesquid-native` (test "unit_installation") due to 1 previous error
      1 error[E0432]: unresolved import `filesquid_native::installation`
```

The outbox-events test with the catalog still on the plain store:

```text
test catalog_commits_raise_journal_entry_events ... FAILED
test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

## Mocked Environment Variables

None.

## Raw test output

Current suite, after review rounds 1 and 2:

```text
$ cargo test -p filesquid-native --features desktop --test unit_installation
running 8 tests
test a_malformed_copy_is_refused_without_naming_a_path ... ok
test a_lost_file_is_restored_from_the_operational_store ... ok
test a_lost_operational_store_is_restored_from_the_file ... ok
test copies_that_disagree_are_refused ... ok
test the_file_is_owner_only ... ok
test the_first_start_creates_one_uuid_v4_and_later_starts_read_it ... ok
test two_installations_get_different_uuids ... ok
test concurrent_launches_agree_on_one_installation ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
$ cargo test -p filesquid-native --features desktop --test unit_journal
running 15 tests
test append_then_query_returns_entries_in_order_with_ids ... ok
test both_stores_assign_ids_before_the_entries_are_written ... ok
test an_older_journal_gains_the_linkage_columns_on_open ... ok
test both_stores_keep_the_operation_id_and_object_reference ... ok
test entries_serialize_rfc3339_utc_time_and_lowercase_enums ... ok
test filters_and_cursor_paging ... ok
test log_file_names_match_the_original_suffix_rule ... ok
test opaque_identifiers_are_kept ... ok
test identifiers_that_are_not_opaque_are_redacted_by_both_stores ... ok
test descending_queries_page_newest_first ... ok
test domain_operations_are_journaled_in_the_callers_transaction ... ok
test opening_creates_the_database_and_writes_no_log_files ... ok
test retention_keeps_everything_by_default_and_journals_its_own_deletions ... ok
test the_sqlite_journal_and_its_wal_are_owner_only ... ok
test sqlite_journal_is_append_only ... ok
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
$ cargo test -p filesquid-native --features desktop --test unit_canonical_commit
running 13 tests
test a_commit_without_journal_ids_publishes_and_warns_without_identifying_data ... ok
test a_command_publishes_one_snapshot_version_before_the_catalog_answers ... ok
test a_crash_after_the_snapshot_loses_no_acknowledged_state ... ok
test a_crash_after_the_snapshot_replays_the_commits_journal_entries_once ... ok
test a_head_without_its_version_inventory_is_refused_and_not_repaired ... ok
test a_crash_before_the_snapshot_leaves_the_work_pending_and_a_retry_completes_once ... ok
test a_library_directory_that_is_a_symbolic_link_is_refused ... ok
test a_deleted_catalog_is_rebuilt_from_the_snapshot_with_its_receipts ... ok
test a_missing_snapshot_is_published_again_in_full_by_the_next_commit ... ok
test an_unpublished_version_left_by_a_crash_is_ignored_and_replaced ... ok
test a_snapshot_of_another_library_is_never_loaded ... ok
test every_library_keeps_its_ocfl_root_beside_its_catalog_file ... ok
test an_unrecorded_catalog_still_links_its_commits ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
$ cargo test -p filesquid-desktop --test integration_library_catalog
running 5 tests
test a_catalog_directory_that_cannot_be_opened_is_journaled_without_its_path ... ok
test the_installation_uuid_is_created_once_and_journaled ... ok
test the_host_keeps_one_catalog_file_per_library_and_journals_its_commits ... ok
test catalog_commits_raise_journal_entry_events ... ok
test a_catalog_commit_carries_its_history_in_the_same_version ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```
