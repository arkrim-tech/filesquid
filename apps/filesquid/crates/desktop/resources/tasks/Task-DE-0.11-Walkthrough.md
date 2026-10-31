# Task DE-0.11 Walkthrough: Host journal sealing and journal rebuild

Module task of global [Task FS-2.9](../../../../../../resources/implementation-plan.md#task-fs-29), on branch `feat/de-0.11-journal-sealing-rebuild`. It depends on DE-0.10, API-F2.11 (the sealing service) and CL-1.9 (the journal rebuild), which are all done.

## What changed

- **`filesquid-native::journal_objects`** (`desktop` feature): where the desktop seals.
  - The installation's journal object lives in an OCFL storage root of its own at `<data dir>/journal.ocfl/`. It is created on first use, with its leases, staging and multipart parts in `<data dir>/journal-work/`, never inside the root.
  - A library's journal object lives in the library's own storage root and uses the library's shared leases. Its staging goes under the library's folder in the data directory.
  - `restore_installation_journal` appends the installation's sealed segments back through the SQLite journal store under their own journal IDs, de-duplicated by ID. It then calls `SqliteJournal::reserve_through(id)` to move the store's ID counter past the highest ID any installation seal read. Each segment's `through` covers every entry the seal read, library entries included.
- **`filesquid-desktop::sealing`**: runs the API's `SealService`.
  - **At launch:** a store refilled before the monitor opens it (`refill_before_open`, journaled afterwards as `journal.refill`), then the seal status installed for the `journal_unsealed` health reason, a catch-up seal of whatever an earlier launch left unsealed, and the hourly loop (`SEAL_INTERVAL_MS`), which checks the app's stop flag every second.
  - **At app close:** a seal that waits at most 30 s (`CLOSE_SEAL_WAIT`).
  - **Each served library** is opened for sealing when `DesktopLibraries::serve` serves it.
  - **Every run** is a registered `journal.seal` process and journals `journal.seal` for each scope (API-S-032).
- **Library rebuild:** the desktop's `local_library_service` now carries `with_journal_restore` over the SQLite journal store (`Monitor::store`) with the `OwnerReadable` transform, so a library's rebuild from files restores its per-commit entries and sealed segments (CL-1.9).
- **Packaged-window tests:** the test-instrumented build reads `FILESQUID_E2E_DATA_DIR`, which `wdio.conf.mjs` sets to `app-data` in the run directory. The suite therefore never touches the developer's data directory on any platform, including macOS, where `XDG_DATA_HOME` does not apply. Shipping builds read no test variable.

### A defect the packaged run found

The first packaged run of the database-deleted rebuild failed. On a recreated store the AUTOINCREMENT counter restarts at 1, and the new launch journaled `host.started` before the refill ran. Its new entries took the old IDs, and the store kept them over the old entries, which the refill then skipped as duplicates. The fix has two parts:

- The refill now runs before the monitor opens the store.
- The refill then reserves the IDs through the highest `through` of the installation's segments, so library entries that a later rebuild restores keep their IDs too.

The integration test and the BDD scenario now check that *every* entry of the first launch comes back under its own ID with the same operation and time.

## Decisions and limits

- **Library close.** The desktop has no library-close step: a library stays open until the app exits. The app-close seal therefore also seals every open library (DE-S-013 updated).
- **Catch-up at launch.** The catch-up runs before any library is opened in this launch. Under the API's rule for a library this service has not opened, a library's entries left unsealed by the last launch are sealed into the installation's object. They are still restored by the refill.
- **Accepted bound (API-S-032).** Entries journaled after the last seal and before a crash, which changed nothing, are lost with the store. One case can still clash: a per-commit entry whose commit came after the installation's last seal holds an ID above the reserved range. If a new entry takes that ID, the library's rebuild reports the entry as `conflicting` and keeps the held one (CL-1.9).
- **Pre-existing failure.** `libraries.e2e.mjs` fails on `main` (a3be2dd) as well as on this branch (expected 1 object, got 2; a repair answered 409). I checked this by building and running `main`'s test-instrumented binary. This task did not change it.

## Requirements (SRS mapping)

| Requirement | Tests |
|---|---|
| DE-S-013 Journal sealing and rebuild: hourly seal as a registered process | desktop `integration_journal_sealing::the_hourly_seal_runs_without_a_call_and_ends_with_the_app`; BDD `desktop_journal_sealing_F-061.feature` "The hourly seal runs on its own and stops with the app" |
| DE-S-013 Journal sealing and rebuild: close-time seal | `the_close_seal_seals_what_is_pending`; BDD "Closing the app seals what is pending" |
| DE-S-013 Journal sealing and rebuild: missed seal caught up at the next start | `a_launch_catches_up_a_missed_seal_and_refills_a_lost_store`; BDD "A seal missed by a forced quit is caught up at the next start" |
| DE-S-013 Journal sealing and rebuild: `<data dir>/journal.ocfl/` | native `unit_journal_objects::host_events_are_sealed_into_the_installation_object_in_the_data_directory` |
| DE-S-013 Journal sealing and rebuild: database-deleted rebuild, same IDs | native `a_lost_store_is_refilled_from_the_sealed_segments_once_under_the_same_ids`, `a_data_directory_without_a_journal_object_restores_nothing`, `unit_journal::reserving_through_an_id_makes_new_ids_greater`; desktop `a_launch_catches_up_a_missed_seal_and_refills_a_lost_store`; BDD "A deleted database is recreated and refilled under the same journal IDs"; packaged window `journal-rebuild-1-close.e2e.mjs` and `journal-rebuild-2-refill.e2e.mjs` |
| DE-S-013 Journal sealing and rebuild: library sealed into its root, restored by its rebuild | `a_served_library_is_sealed_into_its_own_root_and_restored_by_its_rebuild`; BDD "A served library's events are sealed into its own folder and restored by its rebuild" |
| SRS-F-061 Activity journal and live health monitoring (opaque IDs only) | every scenario asserts that no journal entry names the data directory |

## TDD: failing runs

```text
$ cargo test -p filesquid-native --features desktop --locked --test unit_journal_objects
      1 error: could not compile `filesquid-native` (test "unit_journal_objects") due to 1 previous error
      1 error[E0432]: unresolved import `filesquid_native::journal_objects`
$ cargo test -p filesquid-desktop --locked --test integration_journal_sealing
      1 error: could not compile `filesquid-desktop` (test "integration_journal_sealing") due to 1 previous error
      1 error[E0432]: unresolved import `filesquid_desktop_lib::sealing`
```

The first packaged run of the refill spec, before the refill moved ahead of the monitor:

```text
[0-1] Error in "the journal survives a deleted operational store (refill).refills the new store from the sealed journal under the same IDs"
Expected: "2026-10-31T18:26:50.358Z"
Received: "2026-10-31T18:26:55.889Z"
```

## Test output

```text
$ cargo test -p filesquid-native --features desktop --locked --test unit_journal_objects --test unit_journal
running 22 tests
test a_client_event_id_that_is_not_opaque_is_dropped_and_its_entry_kept ... ok
test an_older_journal_gains_the_client_event_id_column_on_open ... ok
test a_preset_id_repeating_a_held_client_event_id_answers_the_holder ... ok
test append_then_query_returns_entries_in_order_with_ids ... ok
test an_older_journal_gains_the_linkage_columns_on_open ... ok
test both_stores_assign_ids_before_the_entries_are_written ... ok
test both_stores_keep_the_operation_id_and_object_reference ... ok
test both_stores_filter_by_library_object_and_operation_id ... ok
test both_stores_store_each_client_event_id_once ... ok
test entries_serialize_rfc3339_utc_time_and_lowercase_enums ... ok
test descending_queries_page_newest_first ... ok
test client_event_de_duplication_survives_a_restart ... ok
test filters_and_cursor_paging ... ok
test log_file_names_match_the_original_suffix_rule ... ok
test domain_operations_are_journaled_in_the_callers_transaction ... ok
test opaque_identifiers_are_kept ... ok
test identifiers_that_are_not_opaque_are_redacted_by_both_stores ... ok
test opening_creates_the_database_and_writes_no_log_files ... ok
test reserving_through_an_id_makes_new_ids_greater ... ok
test retention_keeps_everything_by_default_and_journals_its_own_deletions ... ok
test sqlite_journal_is_append_only ... ok
test the_sqlite_journal_and_its_wal_are_owner_only ... ok
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
running 3 tests
test a_data_directory_without_a_journal_object_restores_nothing ... ok
test host_events_are_sealed_into_the_installation_object_in_the_data_directory ... ok
test a_lost_store_is_refilled_from_the_sealed_segments_once_under_the_same_ids ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
$ cargo test -p filesquid-desktop --locked --test integration_journal_sealing
running 4 tests
test the_close_seal_seals_what_is_pending ... ok
test a_served_library_is_sealed_into_its_own_root_and_restored_by_its_rebuild ... ok
test a_launch_catches_up_a_missed_seal_and_refills_a_lost_store ... ok
test the_hourly_seal_runs_without_a_call_and_ends_with_the_app ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
$ cargo test -p filesquid-desktop --features test-webdriver --locked --test features -- --name "seal|hourly|Closing the app|deleted database|served library"
Feature: The desktop seals its activity journal and rebuilds it from files
  Scenario: The hourly seal runs on its own and stops with the app
   ✔  Given a desktop whose sealing runs on a short hourly interval
   ✔  When a host event is journaled
   ✔  Then it should be sealed into the installation's journal object without being asked
   ✔  And the hourly seal should end when the app exits
  Scenario: Closing the app seals what is pending
   ✔  Given a running desktop with its sealing started
   ✔  When a host event is journaled and the app closes
   ✔  Then the close-time seal should have sealed it into the installation's journal object
  Scenario: A seal missed by a forced quit is caught up at the next start
   ✔  Given a desktop that journaled a library creation and was forced to quit without sealing
   ✔  When the desktop starts again
   ✔  Then the library creation should be sealed into the installation's journal object
   ✔  And the journal should record one "journal.seal" run that succeeded, as a registered process
  Scenario: A deleted database is recreated and refilled under the same journal IDs
   ✔  Given a desktop that journaled a library creation and was forced to quit without sealing
   ✔  And the desktop started again, which caught up the seal
   ✔  When the operational store is deleted and the desktop starts again
   ✔  Then every entry of the first launch should be back under its own journal ID
   ✔  And the journal should record "journal.refill" succeeded once
   ✔  And no journal entry should name the data directory
  Scenario: A served library's events are sealed into its own folder and restored by its rebuild
   ✔  Given a running desktop serving a library from a folder the owner chose
   ✔  When an upload into that library is journaled and the journal is sealed
   ✔  Then the upload should be sealed into the library's own storage root
   ✔  And after the operational store is lost, the library's rebuild should restore it under its journal ID
[Summary]
1 feature
5 scenarios (5 passed)
21 steps (21 passed)
$ cargo build -p filesquid-desktop --features test-webdriver --locked
$ (cd crates/desktop/tests/e2e && xvfb-run -a npx wdio run ./wdio.conf.mjs --spec specs/journal-rebuild-1-close.e2e.mjs --spec specs/journal-rebuild-2-refill.e2e.mjs)
[0-0] PASSED in undefined - file:///specs/journal-rebuild-1-close.e2e.mjs
[0-1] PASSED in undefined - file:///specs/journal-rebuild-2-refill.e2e.mjs
[WebKitGTK 605.1.15 linux #0-0]    ✓ has journaled the app's start under a journal ID
[WebKitGTK 605.1.15 linux #0-0] 1 passing (73ms)
[WebKitGTK 605.1.15 linux #0-1]    ✓ refills the new store from the sealed journal under the same IDs
[WebKitGTK 605.1.15 linux #0-1] 1 passing (199ms)
Spec Files:	 2 passed, 2 total (100% completed) in 00:00:04  
$ (cd crates/desktop/tests/e2e && xvfb-run -a npx wdio run ./wdio.conf.mjs)   # whole packaged suite
[0-0] PASSED in undefined - file:///specs/capabilities.e2e.mjs
[0-1] PASSED in undefined - file:///specs/content-handles.e2e.mjs
[0-2] PASSED in undefined - file:///specs/journal-rebuild-1-close.e2e.mjs
[0-3] PASSED in undefined - file:///specs/journal-rebuild-2-refill.e2e.mjs
[0-4] FAILED in undefined - file:///specs/libraries.e2e.mjs
[0-5] PASSED in undefined - file:///specs/refusals.e2e.mjs
[0-6] PASSED in undefined - file:///specs/transport.e2e.mjs
Spec Files:	 6 passed, 1 failed, 7 total (100% completed) in 00:00:21  
```

`libraries.e2e.mjs` fails the same way on `main` (see "Decisions and limits").

The full-suite hook run on the head commit is recorded in the pull request (`Test scope:` line).

## Sync Execution Report

- `apps/filesquid/crates/desktop/resources/srs.md` DE-S-013 gains desktop specifics: the refill runs before the monitor opens the store, then reserves the sealed IDs and journals `journal.refill`; each library's journal object lives in its own root; the app close is each library's close; the close-time seal waits at most 30 s.
- Root `README.md`: a "Sealed journal on the desktop" item under "View logs", and `FILESQUID_E2E_DATA_DIR` and the journal-rebuild specs in the packaged-window section.
- No PRD, product-vision, ADR or API contract change: the behaviour is the one ADR 033 Decision 7, API-S-032 and DE-S-013 already require.

## Mocked Environment Variables

None. The packaged-window run sets `FILESQUID_E2E_DATA_DIR` to a folder in its own run directory; it is a test-only variable of the `test-webdriver` build.
