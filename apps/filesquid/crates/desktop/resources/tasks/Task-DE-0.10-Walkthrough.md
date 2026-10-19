# Task DE-0.10 Walkthrough: Installation UUID, native journal columns, outbox events and coordinator commit linkage

Branch: `feat/prov-o-history` (global Task FS-2.8; ADR 040 Decisions 4 and 6; [ADR 021](../../../../../../resources/architecture/021-uuid-v4-identifiers.md)).

## DE-S-012: Installation UUID

## DE-S-011: Native journal linkage and coordinator commits

## What changed

- `filesquid-native::installation::load_or_create`:
  - generates the installation UUID v4 from the OS random source on the first start that finds none;
  - stores it in `<data dir>/installation.json` (owner-only, written through a renamed temporary file) and in the `host_installation` table of the operational store `filesquid.sqlite`;
  - reads it back at every start and restores a missing copy from the other;
  - refuses a malformed copy or two copies that disagree. Errors name no path and no value.
- The desktop reads it at launch (`load_installation`), journals `host.installation.created` (or `host.installation.restored`, or `host.installation` `failed` with an error code), never the UUID, and keeps it as `DesktopInstallation` in Tauri's managed state for the API.
- The SQLite journal gains the `operation_id` and `object_ref` columns and indexes, added to an existing journal by an idempotent forward migration (`ALTER TABLE ... ADD COLUMN`; old rows read them as absent). It also gains `allocate_ids` (it advances the AUTOINCREMENT counter) and stores an entry with a preset ID under that ID, once.
- The catalog's journal outbox reaches the host journal through `Monitor::announcing_store`, so outbox entries raise `journal.entry` events and count in the metrics. The new test fails with the previous plain store, as recorded below.

## Not delivered in this task, and why

- "Every commit of the desktop OCFL coordinator (library snapshots of DE-0.3, file objects of DE-1.2) sets the version `user`, carries the `filesquid.operation.v1` message and its PROV-O record and emits `ocfl.commit`", and "at reconcile write the journal entries a crash left unwritten from the version's PROV-O record": the desktop OCFL coordinator does not exist yet. DE-0.3 and DE-1.2 are open, and `filesquid-native` and the desktop make no OCFL commit today. The pieces the coordinator will use exist: the installation UUID, `CommitOptions`, the version-bound hook, `allocate_ids` and the per-commit `provenance/<version>.journal.jsonl` of API-F2.9.
- The plan lists DE-0.3 and DE-0.8 as dependencies; neither is done.

## SRS traceability

| Requirement | Tests |
|---|---|
| DE-S-012 | `filesquid-native` `unit_installation` (7 tests); desktop `integration_library_catalog::the_installation_uuid_is_created_once_and_journaled` |
| DE-S-011 journal columns and migration | `unit_journal::both_stores_keep_the_operation_id_and_object_reference`, `an_older_journal_gains_the_linkage_columns_on_open`, `both_stores_assign_ids_before_the_entries_are_written` |
| DE-S-011 outbox events | desktop `integration_library_catalog::catalog_commits_raise_journal_entry_events` |

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

`cargo test -p filesquid-native --locked --test unit_journal --test unit_installation`:

```text
    Finished `test` profile [unoptimized] target(s) in 8.80s
     Running tests/unit/installation.rs (<target>/debug/deps/unit_installation-ef827af72aaca499)

running 7 tests
test a_malformed_copy_is_refused_without_naming_a_path ... ok
test a_lost_operational_store_is_restored_from_the_file ... ok
test a_lost_file_is_restored_from_the_operational_store ... ok
test the_file_is_owner_only ... ok
test copies_that_disagree_are_refused ... ok
test two_installations_get_different_uuids ... ok
test the_first_start_creates_one_uuid_v4_and_later_starts_read_it ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/journal.rs (<target>/debug/deps/unit_journal-c64f3c5251ba0aca)

running 15 tests
test append_then_query_returns_entries_in_order_with_ids ... ok
test both_stores_keep_the_operation_id_and_object_reference ... ok
test both_stores_assign_ids_before_the_entries_are_written ... ok
test entries_serialize_rfc3339_utc_time_and_lowercase_enums ... ok
test descending_queries_page_newest_first ... ok
test an_older_journal_gains_the_linkage_columns_on_open ... ok
test log_file_names_match_the_original_suffix_rule ... ok
test opaque_identifiers_are_kept ... ok
test domain_operations_are_journaled_in_the_callers_transaction ... ok
test identifiers_that_are_not_opaque_are_redacted_by_both_stores ... ok
test filters_and_cursor_paging ... ok
test opening_creates_the_database_and_writes_no_log_files ... ok
test sqlite_journal_is_append_only ... ok
test retention_keeps_everything_by_default_and_journals_its_own_deletions ... ok
test the_sqlite_journal_and_its_wal_are_owner_only ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

`cargo test -p filesquid-desktop --locked --test integration_library_catalog`:

```text
    Finished `test` profile [unoptimized] target(s) in 16.34s
     Running tests/integration/library_catalog.rs (<target>/debug/deps/integration_library_catalog-ee4396ddbd7ea70b)

running 4 tests
test a_catalog_directory_that_cannot_be_opened_is_journaled_without_its_path ... ok
test the_installation_uuid_is_created_once_and_journaled ... ok
test catalog_commits_raise_journal_entry_events ... ok
test the_host_keeps_one_catalog_file_per_library_and_journals_its_commits ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```
