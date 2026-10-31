# Task DE-0.10 Walkthrough: Installation UUID, native journal columns, outbox events and coordinator commit linkage

Branches: `feat/prov-o-history` (the first slices), then `feat/de-0.10-installation-uuid-journal-outbox` (closing the task once DE-0.3, DE-0.8, DE-1.2 and API-F2.8 were done). Global Task FS-2.8; ADR 040 Decisions 4 and 6; [ADR 021](../../../../../../resources/architecture/021-uuid-v4-identifiers.md).

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

File objects (DE-1.2) did not exist yet when this section was written; they now commit through the same recorder (see "Closing the task" below).

## Closing the task (branch `feat/de-0.10-installation-uuid-journal-outbox`)

With every dependency done, the branch audited each clause of DE-S-011 and DE-S-012 against `main` (6def085) and closed the four gaps it found. The file objects of DE-1.2 already committed through the file service's provenance (`install_file_provenance`, Task FS-2.8 host wiring): installation `user`, `filesquid.operation.v1` message of kind `import`, `provenance/v1.jsonld` and `provenance/v1.journal.jsonl`, and the `ocfl.commit` entry the import job journals.

- **Installation file schema (DE-S-012).** `installation.json` is now `{"schema": 1, "installation_id": "<uuid>", "created": "<RFC 3339 UTC>"}`. The operational store's `host_installation` table gains a `created` column through an idempotent forward migration, so a copy restored from the other keeps the original creation time. A file of another schema, a schema 1 file without `created`, an unversioned file with one, or a `created` that is not RFC 3339 UTC is refused as malformed without naming a path or the value. When both copies hold a creation time and they differ, the store is rewritten with the file's, so the copies agree. Concurrent launches that both find an earlier build's table migrate it safely. A file an earlier build wrote (no `schema`, no `created`) is still read as that installation and upgraded to schema 1; if that upgrade write fails, the launch goes on with a stdout warning, since the file still names the installation.
- **Import commits a crash left unjournaled (DE-S-011).** The import job journals a commit's `ocfl.commit` entry after the OCFL commit, through the bounded writer queue. A crash in between used to lose that entry: the resumed job's replay of the import answered the object with no entries. `FileService::import_original` now answers a replayed import with the entries its version recorded (`provenance/v1.journal.jsonl`), under the IDs the commit named, so the resumed job journals them. The store keeps a preset ID once (`ON CONFLICT DO NOTHING`), so a second replay adds nothing. A recorded journal that cannot be read in full (an unreadable file, a line that is not an entry, an entry without an ID) fails the replay instead of answering a partial journal, so the job never acknowledges the import with entries missing; the item keeps its error code for the owner's retry (Cubic round 1).
- **One announcement per stored entry.** The journal writer now stores through `append_new` and announces and counts only the entries the store stored now, as `AnnouncingJournal` already did. A replayed entry the store already held raises no second `journal.entry` event and does not count twice in the metrics.
- **BDD.** [`desktop_installation_F-067.feature`](../../tests/features/desktop_installation_F-067.feature) has six scenarios through the real launch path (`open_host_monitor`, `load_installation`, `install_file_provenance`). They cover the first launch, restoring either copy, losing both, a malformed file, and the import crash repaired with the recorded ID. Every journal check asserts opaque IDs only: no UUID value, no path, no file name (owner decision 2026-10-31, API-S-036).

The import replay changes one existing assertion. `unit_import_file::an_import_commit_is_linked_to_the_journal_and_carries_its_record` used to require that a replay answers no entries. It now requires the replay to answer exactly the entries the commit answered (same IDs) and to leave the head at `v1`.

### TDD: failing runs on this branch

```text
$ cargo test -p filesquid-native --features desktop --locked --test unit_import_file
test a_crash_before_the_commit_was_journaled_is_repaired_once_with_the_recorded_id ... FAILED
test an_import_commit_is_linked_to_the_journal_and_carries_its_record ... FAILED
test result: FAILED. 8 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
$ cargo test -p filesquid-api --lib --locked journal_writer
test journal_writer::tests::a_rejected_write_is_one_json_line_per_record_on_stdout ... ok
test journal_writer::tests::a_replayed_entry_the_store_already_holds_is_announced_once ... FAILED
test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 76 filtered out; finished in 0.09s
$ cargo test -p filesquid-native --features desktop --locked --test unit_installation
test a_file_of_another_schema_is_refused_without_naming_a_path ... FAILED
test the_file_holds_schema_1_the_uuid_and_its_creation_time ... FAILED
test result: FAILED. 10 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

### Test output on this branch

```text
$ cargo test -p filesquid-api --lib --locked journal_writer
test journal_writer::tests::a_rejected_write_is_one_json_line_per_record_on_stdout ... ok
test journal_writer::tests::a_replayed_entry_the_store_already_holds_is_announced_once ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 76 filtered out; finished in 0.00s
$ cargo test -p filesquid-native --features desktop --locked --lib installation
running 6 tests
test installation::tests::a_file_with_a_creation_time_that_is_not_utc_is_malformed ... ok
test installation::tests::a_created_store_copy_is_forgotten_so_the_next_launch_creates_again ... ok
test installation::tests::copies_with_two_creation_times_settle_on_the_files ... ok
test installation::tests::a_restored_or_foreign_store_copy_is_kept ... ok
test installation::tests::mixed_file_shapes_are_malformed ... ok
test installation::tests::a_store_table_of_an_earlier_build_gains_the_creation_time ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
$ cargo test -p filesquid-native --features desktop --locked --test unit_installation --test unit_import_file --test unit_import_job --test unit_journal
     Running tests/unit/import_file.rs
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/import_job.rs
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.46s
     Running tests/unit/installation.rs
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/journal.rs
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
$ cargo test -p filesquid-desktop --features test-webdriver --locked --test features -- --name "installation|import's commit|lost copy|Losing both|malformed installation"
Feature: The desktop's installation identity and the journal linkage of its commits
  Scenario: The first launch creates the installation UUID once
   ✔  Given a desktop data directory with no installation UUID
   ✔  When the desktop launches twice
   ✔  Then both launches should read the same UUID v4
   ✔  And the installation file should hold schema 1, the UUID and an RFC 3339 UTC creation time
   ✔  And the journal should record "host.installation.created" once
   ✔  And no journal entry should name the installation UUID or the data directory
  Scenario Outline: A lost copy is restored from the other
   ✔  Given a desktop that has launched once
   ✔  When its installation file is lost and the desktop launches again
   ✔  Then the launch should read the same UUID as before
   ✔  And the installation file should be as it was after the first launch
   ✔  And the journal should record "host.installation.restored" once
  Scenario Outline: A lost copy is restored from the other
   ✔  Given a desktop that has launched once
   ✔  When its operational store is lost and the desktop launches again
   ✔  Then the launch should read the same UUID as before
   ✔  And the installation file should be as it was after the first launch
   ✔  And the journal should record "host.installation.restored" once
  Scenario: Losing both copies makes a new installation, and journals it
   ✔  Given a desktop that has launched once
   ✔  When both copies are lost and the desktop launches again
   ✔  Then the launch should read a new UUID v4
   ✔  And the journal should record "host.installation.created" once
  Scenario: A malformed installation file is refused without naming a path
   ✔  Given a desktop data directory whose installation file holds no UUID v4
   ✔  When the desktop launches
   ✔  Then the launch should be refused with an error that names no path
   ✔  And the journal should record "host.installation" failed with the error code "malformed"
  Scenario: A crash between an import's commit and its journal entry is repaired with the recorded ID
   ✔  Given a desktop with an open library whose commits record their history
   ✔  And a file was imported but the app stopped before its commit reached the journal
   ✔  When the import is replayed twice after the restart
   ✔  Then the journal should hold that import's "ocfl.commit" entry once, under the ID its version recorded
   ✔  And that entry should name the import's operation and object version and no path
[Summary]
1 feature
6 scenarios (6 passed)
29 steps (29 passed)
```

The full-suite hook run on the head commit is recorded in the pull request (`Test scope:` line).

## Sync Execution Report

- `apps/filesquid/crates/api/resources/srs.md` API-S-036: a replayed import answers the journal entries its commit recorded, under the same IDs (DE-S-011).
- `apps/filesquid/crates/desktop/resources/srs.md` DE-S-011: for a file-object import the resumed job's replay writes them; the writer announces and counts a replayed entry once.
- DE-S-012 already specified the schema 1 file; the code now matches it. No PRD, product-vision or ADR change: the behaviour is the one ADR 040 Decision 6 and SRS-F-067 already require.

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

DE-S-011 is "Native journal and commit linkage"; DE-S-012 is "Installation UUID"; SRS-F-067 is "PROV-O history of every version and change".

| Requirement | Tests |
|---|---|
| DE-S-012 Installation UUID | `filesquid-native` `unit_installation` (12 tests, including `the_file_holds_schema_1_the_uuid_and_its_creation_time`, `a_restored_copy_keeps_the_creation_time`, `a_file_of_another_schema_is_refused_without_naming_a_path`, `a_file_from_an_earlier_build_is_still_read`) and the in-module tests of `installation.rs` (6, including the store migration, mixed file shapes and two creation times); desktop `integration_library_catalog::the_installation_uuid_is_created_once_and_journaled`; BDD `desktop_installation_F-067.feature` scenarios 1 to 5 |
| DE-S-011 Native journal and commit linkage: import commit repaired after a crash | native `unit_import_file::a_crash_before_the_commit_was_journaled_is_repaired_once_with_the_recorded_id`, `an_import_commit_is_linked_to_the_journal_and_carries_its_record`, `a_replay_whose_recorded_journal_is_unreadable_fails`; BDD `desktop_installation_F-067.feature` scenario 6 |
| DE-S-011 Native journal and commit linkage: one announcement per stored entry | api `journal_writer::tests::a_replayed_entry_the_store_already_holds_is_announced_once` |
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

The suites the earlier branches recorded, re-run on this branch after Cubic round 2:

```text
$ cargo test -p filesquid-native --features desktop --locked --test unit_installation
running 12 tests
test a_file_of_another_schema_is_refused_without_naming_a_path ... ok
test a_malformed_copy_is_refused_without_naming_a_path ... ok
test a_file_from_an_earlier_build_is_still_read ... ok
test a_lost_file_is_restored_from_the_operational_store ... ok
test a_lost_operational_store_is_restored_from_the_file ... ok
test a_restored_copy_keeps_the_creation_time ... ok
test the_file_holds_schema_1_the_uuid_and_its_creation_time ... ok
test the_file_is_owner_only ... ok
test copies_that_disagree_are_refused ... ok
test two_installations_get_different_uuids ... ok
test concurrent_launches_agree_on_one_installation ... ok
test the_first_start_creates_one_uuid_v4_and_later_starts_read_it ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
$ cargo test -p filesquid-native --features desktop --locked --test unit_journal
running 21 tests
test a_preset_id_repeating_a_held_client_event_id_answers_the_holder ... ok
test a_client_event_id_that_is_not_opaque_is_dropped_and_its_entry_kept ... ok
test an_older_journal_gains_the_client_event_id_column_on_open ... ok
test append_then_query_returns_entries_in_order_with_ids ... ok
test both_stores_keep_the_operation_id_and_object_reference ... ok
test both_stores_assign_ids_before_the_entries_are_written ... ok
test both_stores_filter_by_library_object_and_operation_id ... ok
test an_older_journal_gains_the_linkage_columns_on_open ... ok
test both_stores_store_each_client_event_id_once ... ok
test entries_serialize_rfc3339_utc_time_and_lowercase_enums ... ok
test descending_queries_page_newest_first ... ok
test client_event_de_duplication_survives_a_restart ... ok
test log_file_names_match_the_original_suffix_rule ... ok
test opaque_identifiers_are_kept ... ok
test domain_operations_are_journaled_in_the_callers_transaction ... ok
test identifiers_that_are_not_opaque_are_redacted_by_both_stores ... ok
test opening_creates_the_database_and_writes_no_log_files ... ok
test filters_and_cursor_paging ... ok
test retention_keeps_everything_by_default_and_journals_its_own_deletions ... ok
test the_sqlite_journal_and_its_wal_are_owner_only ... ok
test sqlite_journal_is_append_only ... ok
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
$ cargo test -p filesquid-native --features desktop --locked --test unit_canonical_commit
running 18 tests
test a_commit_without_journal_ids_publishes_and_warns_without_identifying_data ... ok
test a_crash_after_the_snapshot_loses_no_acknowledged_state ... ok
test a_crash_after_the_snapshot_replays_the_commits_journal_entries_once ... ok
test a_command_publishes_one_snapshot_version_before_the_catalog_answers ... ok
test a_crash_before_the_snapshot_leaves_the_work_pending_and_a_retry_completes_once ... ok
test a_library_directory_that_is_a_symbolic_link_is_refused ... ok
test a_head_without_its_version_inventory_is_refused_and_not_repaired ... ok
test a_failed_index_refresh_is_journaled_as_failed ... ok
test a_missing_snapshot_is_published_again_in_full_by_the_next_commit ... ok
test a_refresh_of_an_unknown_library_is_journaled_as_failed ... ok
test a_deleted_catalog_is_rebuilt_from_the_snapshot_with_its_receipts ... ok
test a_recorded_no_op_commits_no_version_and_journals_nothing ... ok
test a_recorded_index_refresh_that_changes_nothing_journals_nothing ... ok
test an_unpublished_version_left_by_a_crash_is_ignored_and_replaced ... ok
test a_snapshot_of_another_library_is_never_loaded ... ok
test an_unrecorded_catalog_still_links_its_commits ... ok
test every_library_keeps_its_ocfl_root_in_its_own_folder_and_only_its_index_here ... ok
test an_index_refresh_that_changes_nothing_journals_nothing ... ok
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
$ cargo test -p filesquid-desktop --locked --test integration_library_catalog
running 5 tests
test a_catalog_directory_that_cannot_be_opened_is_journaled_without_its_path ... ok
test the_installation_uuid_is_created_once_and_journaled ... ok
test the_host_keeps_one_catalog_file_per_library_and_journals_its_commits ... ok
test a_catalog_commit_carries_its_history_in_the_same_version ... ok
test catalog_commits_raise_journal_entry_events ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```
