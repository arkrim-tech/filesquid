# Task FS-2.8 host wiring Walkthrough: PROV-O history source, repair follow-up and backfill on the desktop

Branch: `feat/fs-2.8-host-wiring` (global [Task FS-2.8](../../../../../../resources/implementation-plan.md#task-fs-28); [ADR 040](../../../../../../resources/architecture/040-prov-o-history.md) Decisions 6 and 7). Follow-up to [Task API-F2.9](../../../api/resources/tasks/Task-API-F2.9-Walkthrough.md) and [Task DE-0.10](./Task-DE-0.10-Walkthrough.md): it closes the gaps API-F2.9 recorded as "Not wired: the history source, the repair follow-up and the backfill".

## Requirements

- **API-S-028 PROV-O record in the same commit**, repair follow-up: a repair is followed by a provenance-only version whose record shares the repair run ID.
- **API-S-029 History routes and backfill**: the history routes answer from the host's records; the backfill registers in the process registry, checkpoints per object, stops at its cap or on cancel, skips versions that have a record and commits one provenance-only version per object.
- **DE-S-011 Native journal and commit linkage**: the desktop host installs the history source over its libraries' storage roots, builds the repair service with its provenance follow-up and runs the backfill at launch as a registered, journaled job that app exit cancels (sentence added to DE-S-011 by this change).
- **DE-S-012 Installation UUID**: the installation UUID the launch reads is the agent of every record the wiring writes.

## What changed

- `filesquid-api` (`history/source.rs`): the new `HistoryRoots` trait lets a host list the OCFL storage roots it serves for each read. The new `OcflRootsHistory` reads all of them as one history: one record cap and one object cap for all roots, for the index and for a single-record lookup alike, at most `MAX_HISTORY_ROOTS` (256) roots. The host lists each root as opened or failed (`RootResult`), so one library that cannot be read does not hide the others. A root that cannot be opened is logged and left out: the index is then flagged `truncated`, and a lookup that found nothing answers `unavailable`, not `None`. `OcflHistory` keeps its single-root behaviour and now shares the reading code (`Reading`, `read_one`). No route module changed.
- `filesquid-native` (`catalog/provenance.rs`, `desktop` feature):
  - `CatalogRoots` implements `HistoryRoots` over the OCFL storage roots of the libraries a `SqliteCatalog` serves (`<data dir>/libraries/<library-id>.ocfl/`). Each read lists the roots again, so a library created later is included.
  - `SqliteCatalog::backfill_history` runs the API's `Backfill` over each library, one library at a time. It holds the catalog's lock and uses the library's own version reservations (`.work/leases`), so it never races a catalog commit. It repeats a run that reached its cap, at most `MAX_BACKFILL_RUNS` (100) runs per library, and stops when the cancel flag is set. When a run committed records, the catalog tries to reconcile with the new head straight away. The provenance-only version changes no entity, and its journal entries keep their pre-allocated IDs, so they are stored once. If that reconcile fails, it is logged as a warning (which the journal layer journals) and the next open reconciles instead.
- `filesquid-desktop` (`lib.rs`):
  - `provenance_context` gives the installation UUID and `host_facts()`, used by the catalog, the repair service and the backfill.
  - `install_history_source` installs `OcflRootsHistory<CatalogRoots>` in `ApiState`, so the history routes stop answering `503`.
  - `local_library_service(host, installation, ledger)` builds the CL-1.1 service with `with_provenance`. Its journal IDs come from the host journal store. DE-1.1 calls it with the persistent repair ledger when it wires the API-F1.12 repair route.
  - `history_backfill(host, installation)` builds the job over the host's journal queue, its journal store (ID allocation) and its process registry.
  - `start_monitor` installs the history source right after the catalogs open. After the ticker starts, it runs the backfill on its own thread (`filesquid-history-backfill`). App exit sets the monitor's stop flag, which cancels the run between objects, and waits at most `BACKFILL_STOP_WAIT` (10 s) for the thread before the journal is flushed. A library whose run could not start, or a catalog that could not list its libraries, is logged as a warning with an error code only, and the journal layer journals that warning. Every run that started journals `prov.backfill` `started` and `succeeded`, `cancelled` or `failed` itself.
- Resumption: the backfill runs at every launch. A version that already has a record is skipped, so an interrupted or capped run continues where it stopped, and a complete library commits nothing.

## SRS traceability

| Requirement | Tests |
|---|---|
| API-S-029 History routes and backfill: routes from a real desktop library | desktop `integration_history_wiring::the_history_routes_answer_from_a_real_desktop_library` (503 before the source is installed, then `GET /api/v1/history`, `GET /api/v1/files/{id}/history` and `GET /api/v1/files/{id}/versions/v1/prov` through the `filesquid://` scheme handler with the per-launch token; installed once) |
| API-S-029 several storage roots, caps, unreadable root | api `unit_history::a_history_over_several_storage_roots_indexes_and_finds_every_record`, `a_storage_root_that_cannot_be_opened_flags_the_history_incomplete`, `a_history_over_several_roots_keeps_one_record_cap`, `a_root_the_host_could_not_open_leaves_the_others_readable`; desktop `integration_history_wiring::one_unreadable_library_leaves_the_others_history_served` (Cubic round 1) |
| API-S-028 PROV-O record in the same commit: repair follow-up under the run ID | desktop `integration_history_wiring::a_repair_commits_its_provenance_follow_up_under_the_repair_run_id` (a W010 snapshot head in a desktop library, repaired by the desktop-built service; record at `v3` names the run ID, `GET /api/v1/history?operation=<run>` answers one `repair` entry, one `ocfl.commit` entry carries the run ID) |
| API-S-029 backfill: registered, journaled, resumable, catalog still serves | desktop `integration_history_wiring::the_backfill_fills_the_history_of_versions_committed_without_records` (library committed without records; one run reconstructs `v1` in a `backfill` version `v2`; the second run commits nothing; both runs `Completed` in the registry; two `prov.backfill` `started`/`succeeded` pairs; one `ocfl.commit` under the run ID; a later catalog command succeeds) |
| API-S-029 backfill cancel | desktop `integration_history_wiring::a_cancelled_backfill_commits_nothing_and_leaves_the_rest_for_the_next_launch` |
| DE-S-011 Native journal and commit linkage, DE-S-012 Installation UUID: no regressions | desktop `integration_library_catalog` (5), native `unit_canonical_commit` (18), api `unit_history_jobs` (11) |

## TDD: failing runs

The API test before `HistoryRoots` and `OcflRootsHistory` existed:

```text
error[E0432]: unresolved imports `filesquid_api::history::HistoryRoots`, `filesquid_api::history::OcflRootsHistory`
error: could not compile `filesquid-api` (test "unit_history") due to 1 previous error
```

The desktop test against the desktop host before the wiring:

```text
error[E0432]: unresolved imports `filesquid_desktop_lib::history_backfill`, `filesquid_desktop_lib::install_history_source`, `filesquid_desktop_lib::local_library_service`
error: could not compile `filesquid-desktop` (test "integration_history_wiring") due to 1 previous error
```

## Plan boxes

- **API-F2.9 stays unticked.** It depends on Task API-F1.4, which is not done. Its description also covers the import commits of DE-1.2 and the admin backfill route of API-S-029, and neither exists yet.
- **DE-0.10 stays unticked.** It depends on Tasks DE-0.8 and DE-1.2, which are not done, and its file-object commits (DE-1.2) do not exist yet.
- **FS-2.8 stays unticked.** It waits on Tasks 1.1, 1.2 and FS-2.2.

## Remaining gaps

- **No backfill route.** `POST /api/v1/admin/history/backfill` (API-S-029) is still not built. The desktop runs the job at every launch instead. The route can call the same `SqliteCatalog::backfill_history`.
- **Owner-chosen storage roots.** The history source and the backfill cover the libraries of the desktop catalog. The owner-chosen roots that DE-1.1 opens through `HostFiles` are not listed yet. DE-1.1 adds them to the host's `HistoryRoots`, and passes `local_library_service` its persistent repair ledger and the root it repairs.
- **Import and file-route commits** (API-F1.3, API-F1.4, DE-1.2) still do not record provenance, because they are not built.

## Mocked Environment Variables

None.

## Raw test output

```text
$ cargo test -p filesquid-desktop --test integration_history_wiring --locked
running 5 tests
test one_unreadable_library_leaves_the_others_history_served ... ok
test the_history_routes_answer_from_a_real_desktop_library ... ok
test a_cancelled_backfill_commits_nothing_and_leaves_the_rest_for_the_next_launch ... ok
test a_repair_commits_its_provenance_follow_up_under_the_repair_run_id ... ok
test the_backfill_fills_the_history_of_versions_committed_without_records ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
$ cargo test -p filesquid-desktop --test integration_library_catalog --locked
running 5 tests
test a_catalog_directory_that_cannot_be_opened_is_journaled_without_its_path ... ok
test the_installation_uuid_is_created_once_and_journaled ... ok
test catalog_commits_raise_journal_entry_events ... ok
test the_host_keeps_one_catalog_file_per_library_and_journals_its_commits ... ok
test a_catalog_commit_carries_its_history_in_the_same_version ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
$ cargo test -p filesquid-api --test unit_history --locked
running 31 tests
test a_root_the_host_could_not_open_leaves_the_others_readable ... ok
test a_history_entry_carries_the_label_and_marks_reconstruction ... ok
test a_file_history_lists_its_object_only ... ok
test a_commit_writes_its_record_and_journal_entries_in_the_same_version ... ok
test a_memory_history_indexes_its_records_and_refuses_conflicts ... ok
test a_lookup_that_skipped_an_unreadable_object_is_unavailable ... ok
test a_receipt_for_another_version_still_journals_the_commit ... ok
test a_history_over_several_roots_keeps_one_record_cap ... ok
test a_record_stored_under_another_version_is_corrupt ... ok
test a_record_that_cannot_be_built_stops_the_commit ... ok
test a_redacted_record_names_no_installation_message_or_path ... ok
test a_history_over_several_storage_roots_indexes_and_finds_every_record ... ok
test a_takeover_with_an_allocated_id_rides_the_version ... ok
test a_takeover_adds_a_warning_after_the_commit_entry ... ok
test a_rebased_commit_regenerates_both_files_for_the_new_version ... ok
test a_storage_root_that_cannot_be_opened_flags_the_history_incomplete ... ok
test an_identical_record_inserted_again_is_held_once ... ok
test a_truncated_history_is_flagged_on_pages_and_the_export ... ok
test a_version_record_is_raw_json_ld_or_not_found ... ok
test an_ocfl_history_past_its_record_cap_is_flagged_truncated ... ok
test an_ocfl_history_without_a_storage_root_is_unavailable ... ok
test commit_meta_sets_the_user_and_the_operation_message ... ok
test every_new_journal_event_keeps_its_opaque_fields_and_loses_paths ... ok
test an_ocfl_history_is_rebuilt_from_the_records_in_the_storage_root ... ok
test redaction_keeps_derivations_linked_and_paths_numbered_across_records ... ok
test history_routes_answer_unavailable_before_the_host_installs_a_source ... ok
test bad_history_parameters_answer_invalid_request ... ok
test the_export_is_a_json_ld_array_and_can_be_redacted ... ok
test the_version_user_names_the_installation_and_never_a_person ... ok
test the_operations_own_entries_ride_the_version_with_their_ids ... ok
test the_history_lists_everything_or_one_filter_in_cursor_pages ... ok
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
$ cargo test -p filesquid-api --test unit_history_jobs --locked
running 11 tests
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
$ cargo test -p filesquid-native --features desktop --test unit_canonical_commit --locked
running 18 tests
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
```

The head-commit hook run is recorded in the pull request.
