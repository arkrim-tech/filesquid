# Task API-F2.11 Walkthrough: Durable journal, per-commit entries and sealing service

Branch: `feat/api-f2.11-durable-journal-sealing` (global [Task FS-2.9](../../../../../../resources/implementation-plan.md#task-fs-2.9); [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) Decision 7). API contract `0.15.0`.

## API-S-032: Durable journal

Also PRD [F-061](../../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring) and [SRS-F-061](../../../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring) Activity journal and live health monitoring, [F-001](../../../../../../resources/prd.md#f-001-independent-local-library-and-ownership) Independent local library and ownership, [F-042](../../../../../../resources/prd.md#f-042-ocfl-preservation-and-portable-recovery) OCFL preservation and portable recovery.

## What changed

- **Per-commit entries** (constraint 1): already in place since API-F2.9. Every commit through `CommitRecorder` writes `provenance/<version>.journal.jsonl` beside its PROV-O record in the same version: the `ocfl.commit` entry and, through `with_entries`, the operation's own entries, each under its pre-allocated ID (catalog commits in `filesquid-native` and version labels in the API pass their own entry). A new test commits an operation, seals the journal and shows that the version's file and the sealed segments together hold every entry of the library, so a rebuild de-duplicating by ID recovers each exactly once.
- **Journal objects** (`journal::seal`, `object.rs`): `OcflJournalObject` commits through the `filesquid-ocfl` change model into an injected storage root (`JournalRoot`: backend, reservations, staging, profile transform). The object is `urn:filesquid:journal:<scope-id>` at `journal/`; each seal adds one version holding `segments/<first>-<through>.jsonl` (20-digit IDs), committed as the installation's OCFL `user` with a `filesquid.journal.seal.v1` message. The cursor is the highest `through` among the head's segments, so it advances with the commit and never before it. `read_sealed_entries` reads every segment back (digest checked, 16 MiB per segment) for the CL-1.9 rebuild.
- **Sealing service** (`service.rs`): `SealService::seal_all` (hourly and at app close) and `close_library` (library close) seal each open library's entries (those naming its library ID) into its object and every other entry into the installation's object, at most 50,000 entries and 16 MiB per scope per run. A run is a registered `journal.seal` process; each scope's result is journaled as `journal.seal` (`succeeded` with counts and the object reference, or `failed` with `unavailable` or `invalid`) beside the segment's `ocfl.commit`. A failed scope commits nothing, so its entries wait for the next run. A run with nothing but the seal's own bookkeeping registers, commits and journals nothing.
- **Health** (constraint 9): `Monitor::install_seal_status` takes the service; `HealthInputs.journal_unsealed_age_ms` (optional in the contract) is the age of the oldest entry a seal left unsealed, and the `journal_unsealed` reason turns health `degraded` above two sealing intervals (2 hours) until a seal succeeds.
- **Contract**: 0.15.0 (`HealthInputs.journal_unsealed_age_ms`); `schema.d.ts` regenerated.

## Not delivered in this task, and why

- **Host wiring** is [DE-0.11](../../../desktop/resources/plan.md): running the seals hourly, at app close and at library close as the desktop does, keeping the installation's object at `<data dir>/journal.ocfl/` and catching up a missed seal at start.
- **The rebuild** is [CL-1.9](../plan.md): it reads `read_commit_journal` and `read_sealed_entries` into a fresh store, de-duplicated by journal ID.
- **File routes** (`/api/v1/files/...`) still commit without `CommitRecorder`, so their versions carry no per-commit journal file yet; their entries are sealed like every other entry. Their operation IDs are [API-F2.12](../plan.md).
- **Library classification**: an entry belongs to a library when it names the library ID among its targets; an entry that names only an object of the library (some `ocfl.commit` entries) is sealed with the installation's events, and is in its commit's version as well.
- **Seal commits carry no PROV-O record**: the journal object is the journal itself; its versions name the seal run in their message and are journaled as `ocfl.commit`.

## SRS traceability

| Requirement | Tests |
|---|---|
| API-S-032 per-commit entries in the same version | `unit_journal_seal::a_committed_operation_and_the_seal_together_keep_every_entry_once`; `unit_history::the_operations_own_entries_ride_the_version_with_their_ids` |
| API-S-032 library and installation scopes (mixed events) | `a_seal_puts_library_events_in_the_library_object_and_the_rest_in_the_installation_object`; BDD "Other events are sealed every hour", "Host events are sealed on app close" |
| API-S-032 cursor advances only with the commit | `a_second_seal_seals_only_what_came_after_the_first`, `a_run_seals_at_most_its_cap_and_the_next_run_continues` |
| API-S-032 failed seal journaled, nothing lost | `a_failed_seal_is_journaled_and_the_next_seal_loses_nothing`; BDD "A failed seal drops nothing" |
| API-S-032 library close | `closing_a_library_seals_it_and_stops_sealing_into_it`; BDD "Library events are sealed on library close" |
| ADR 033 registered, journaled process | `sealing_is_a_registered_journaled_process` |
| Segments are OCFL objects in a storage root | `segments_are_ocfl_objects_in_the_storage_root` (engine validator, no stray keys) |
| No-op run | `a_run_with_only_the_seals_own_bookkeeping_commits_and_journals_nothing`; BDD "A seal with nothing but its own bookkeeping writes nothing" |
| `journal_unsealed` health reason | `an_entry_left_unsealed_for_two_intervals_turns_health_degraded`; BDD "Entries left unsealed for two sealing intervals degrade health" |
| Redaction | `segments_hold_the_redacted_entries_only`, `sealing_is_a_registered_journaled_process` |

## TDD: failing run before implementation

`unit_journal_seal` built against `origin/main` (`ffa7518`):

```text
error: could not compile `filesquid-api` (test "unit_journal_seal") due to 1 previous error
error[E0432]: unresolved import `filesquid_api::journal::seal`
```

## Mocked Environment Variables

None.

## Raw test output

`cargo test -p filesquid-api --locked --test unit_journal_seal --test unit_health_rules --test unit_history`:

```text
     Running tests/unit/health_rules.rs (<target>/debug/deps/unit_health_rules-a81b8bbfe550797a)
test all_clear_is_ok_without_reasons ... ok
test free_space_thresholds ... ok
test open_ocfl_error_findings_degrade_health_with_codes_and_object_count ... ok
test error_rate_thresholds ... ok
test stalled_processes_degrade ... ok
test queue_fill_threshold ... ok
test unreachable_store_or_read_only_data_dir_fail ... ok
test worst_condition_wins_and_all_reasons_are_listed ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/history.rs (<target>/debug/deps/unit_history-ec238ecffe53b83e)
test a_history_entry_carries_the_label_and_marks_reconstruction ... ok
test a_file_history_lists_its_object_only ... ok
test a_commit_writes_its_record_and_journal_entries_in_the_same_version ... ok
test a_history_over_several_roots_keeps_one_record_cap ... ok
test a_history_over_several_storage_roots_indexes_and_finds_every_record ... ok
test a_receipt_for_another_version_still_journals_the_commit ... ok
test a_memory_history_indexes_its_records_and_refuses_conflicts ... ok
test a_record_that_cannot_be_built_stops_the_commit ... ok
test a_redacted_record_names_no_installation_message_or_path ... ok
test a_record_stored_under_another_version_is_corrupt ... ok
test a_lookup_that_skipped_an_unreadable_object_is_unavailable ... ok
test a_storage_root_that_cannot_be_opened_flags_the_history_incomplete ... ok
test a_root_the_host_could_not_open_leaves_the_others_readable ... ok
test a_takeover_with_an_allocated_id_rides_the_version ... ok
test a_rebased_commit_regenerates_both_files_for_the_new_version ... ok
test a_takeover_adds_a_warning_after_the_commit_entry ... ok
test an_identical_record_inserted_again_is_held_once ... ok
test a_truncated_history_is_flagged_on_pages_and_the_export ... ok
test an_ocfl_history_without_a_storage_root_is_unavailable ... ok
test a_version_record_is_raw_json_ld_or_not_found ... ok
test commit_meta_sets_the_user_and_the_operation_message ... ok
test every_new_journal_event_keeps_its_opaque_fields_and_loses_paths ... ok
test an_ocfl_history_past_its_record_cap_is_flagged_truncated ... ok
test an_ocfl_history_is_rebuilt_from_the_records_in_the_storage_root ... ok
test redaction_keeps_derivations_linked_and_paths_numbered_across_records ... ok
test history_routes_answer_unavailable_before_the_host_installs_a_source ... ok
test the_export_is_a_json_ld_array_and_can_be_redacted ... ok
test the_version_user_names_the_installation_and_never_a_person ... ok
test bad_history_parameters_answer_invalid_request ... ok
test the_history_lists_everything_or_one_filter_in_cursor_pages ... ok
test the_operations_own_entries_ride_the_version_with_their_ids ... ok
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/journal_seal.rs (<target>/debug/deps/unit_journal_seal-c6c200e6bb1f195b)
test a_failed_seal_is_journaled_and_the_next_seal_loses_nothing ... ok
test a_run_with_only_the_seals_own_bookkeeping_commits_and_journals_nothing ... ok
test a_committed_operation_and_the_seal_together_keep_every_entry_once ... ok
test a_run_seals_at_most_its_cap_and_the_next_run_continues ... ok
test closing_a_library_seals_it_and_stops_sealing_into_it ... ok
test journal_objects_are_named_per_scope ... ok
test an_entry_left_unsealed_for_two_intervals_turns_health_degraded ... ok
test a_seal_puts_library_events_in_the_library_object_and_the_rest_in_the_installation_object ... ok
test sealing_is_a_registered_journaled_process ... ok
test segments_hold_the_redacted_entries_only ... ok
test a_second_seal_seals_only_what_came_after_the_first ... ok
test segments_are_ocfl_objects_in_the_storage_root ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

`cargo test -p filesquid-api --locked --test features` (the durable journal feature; whole run `323 scenarios (221 passed, 102 skipped)`; the skipped ones are the per-commit rename, rebuild and UI scenarios of CL-1.9, DE-0.11, UI-F2.15 and UI-F2.16, and other features' unbuilt operations):

```text
Feature: Durable, complete and browsable activity journal
  Scenario: A committed operation carries its journal entries in the same OCFL commit
   ✔> Given a desktop library with recorded activity
   ?  When I rename a file
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:19:5
  Scenario: Other events are sealed every hour
   ✔> Given a desktop library with recorded activity
   ✔  Given an authorisation refusal and a health change were journaled
   ✔  When the hourly seal runs
   ✔  Then the installation's journal object should gain one JSON Lines segment with those entries
   ✔  And the seal should be a registered, journaled process
  Scenario: Library events are sealed on library close
   ✔> Given a desktop library with recorded activity
   ✔  Given library entries that are not yet sealed
   ✔  When I close the library
   ✔  Then they should be sealed into the library's journal object
  Scenario: Host events are sealed on app close
   ✔> Given a desktop library with recorded activity
   ✔  Given host entries that are not yet sealed
   ✔  When I close the app
   ✔  Then they should be sealed into the installation's journal object
  Scenario: Database deleted, rebuild restores the journal
   ✔> Given a desktop library with recorded activity
   ?  Given committed operations and other entries sealed at the last hourly seal
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:40:5
  Scenario: A failed seal drops nothing
   ✔> Given a desktop library with recorded activity
   ✔  Given the library's storage root is not writable during a seal
   ✔  When the next seal runs after the storage root is writable again
   ✔  Then every entry that was not sealed should be in the new segment
  Scenario: A seal with nothing but its own bookkeeping writes nothing
   ✔> Given a desktop library with recorded activity
   ✔  Given host entries that are not yet sealed
   ✔  And the hourly seal has run
   ✔  When the hourly seal runs again with nothing new
   ✔  Then no journal version should be added and nothing should be journaled
  Scenario: Entries left unsealed for two sealing intervals degrade health
   ✔> Given a desktop library with recorded activity
   ✔  Given the library's storage root is not writable during a seal
   ✔  When two sealing intervals pass and the seal fails again
   ✔  Then health should be degraded with the reason "journal_unsealed"
  Scenario: A browser console warning reaches the journal
   ✔> Given a desktop library with recorded activity
   ✔  When the UI reports a console warning
   ✔  Then the journal should hold one "ui.console.warn" entry with the component, error code and opaque IDs
  Scenario: Browser console errors reach the journal without their text
   ✔> Given a desktop library with recorded activity
   ✔  When the UI reports a console error with a message that contains a file name
   ✔  Then the journal should hold one "ui.console.error" entry with the component, error code and opaque IDs
   ✔  And the entry should not contain the message text
  Scenario: Client events are bounded
   ✔> Given a desktop library with recorded activity
   ✔  When the UI sends 21 events in one request
   ✔  Then the request should be refused with an error code
   ✔  And a caller over the rate bound should receive status 429 with a Retry-After header
  Scenario: Rate limits delay client events but never merge them
   ✔> Given a desktop library with recorded activity
   ?  Given the UI has 150 console warnings queued
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:80:5
  Scenario: A full client queue spills to IndexedDB and loses nothing
   ✔> Given a desktop library with recorded activity
   ?  Given the host has been unreachable and the UI's in-memory queue holds 10,000 console warnings
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:87:5
  Scenario: Events queued at page close are persisted and delivered on the next open
   ✔> Given a desktop library with recorded activity
   ?  Given the UI has 2 console errors already delivered and 3 console errors the route has not yet accepted
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:94:5
  Scenario: A refused IndexedDB write after the queue is full is counted at page close
   ✔> Given a desktop library with recorded activity
   ?  Given the host has been unreachable and the UI's in-memory queue holds 10,000 console warnings
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:102:5
  Scenario: Persisted events are never delivered under another account's credential
   ✔> Given a desktop library with recorded activity
   ?  Given "ana" left 3 undelivered console errors persisted on this device for the server host
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:109:5
  Scenario: Without IndexedDB the events the UI could not keep are counted at page close
   ✔> Given a desktop library with recorded activity
   ?  Given IndexedDB is unavailable in the page
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:115:5
  Scenario: A client event resent after a lost acknowledgement adds no entry
   ✔> Given a desktop library with recorded activity
   ✔  When the UI sends a console warning twice with the same client event ID
   ✔  Then both requests should be acknowledged
   ✔  And the journal should hold one "ui.console.warn" entry for that client event ID
  Scenario: An event raised by another principal is refused
   ✔> Given a desktop library with recorded activity
   ✔  When the UI sends a console warning raised by another principal
   ✔  Then the request should be refused as not permitted and journaled as a refusal
   ✔  And no "ui.console.warn" entry should be written
  Scenario: The overflow count is bounded
   ✔> Given a desktop library with recorded activity
   ✔  When the UI sends one overflow event with the count 1000000
   ✔  Then the journal should hold one "ui.console.overflow" entry with the count 1000000
   ✔  And an overflow event with the count 0 should be refused with an error code
   ✔  And an overflow event with the count 1000001 should be refused with an error code
  Scenario: A lone overflow event naming no other principal is exempt from the rate bound
   ✔> Given a desktop library with recorded activity
   ✔  Given the caller has used up the rate bound
   ✔  When the UI sends one overflow event with the count 3
   ✔  Then the journal should hold one "ui.console.overflow" entry with the count 3
  Scenario: A lone overflow event raised by another principal is not exempt
   ✔> Given a desktop library with recorded activity
   ✔  Given the caller has used up the rate bound
   ✔  When the UI sends one overflow event raised by another principal
   ✔  Then the request should get status 429
   ✔  And no "ui.console.overflow" entry should be written
  Scenario: The journal filters by file and by operation ID
   ✔> Given a desktop library with recorded activity
   ✔  When I filter the journal by the file
   ✔  Then I should see only the entries about that file
   ✔  When I filter the journal by the operation ID of the rename
   ✔  Then I should see only the entries of that operation
  Scenario: An entry links to its PROV-O activity or is marked journal-only
   ✔> Given a desktop library with recorded activity
   ✔  When I open the journal entry of the committed rename
   ✔  Then it should link to the PROV-O record of its object version
   ✔  When I open the journal entry of the failed move
   ✔  Then it should be marked journal-only
  Scenario: Display names are shown for display and never exported
   ✔> Given a desktop library with recorded activity
   ✔  Given the host resolves display names for the owner
   ✔  When I read the journal with names and export it
   ✔  Then the page should show the file's display name
   ✔  And the export should carry opaque IDs only
  Scenario: OCFL engine warnings carry the operation and the object version
   ✔> Given a desktop library with recorded activity
   ✔  When the OCFL engine warns about a lease takeover inside an operation
   ✔  Then the journal entry should name the engine operation, the operation ID and the object version
  Scenario: Metrics snapshots are off by default
   ✔> Given a desktop library with recorded activity
   ✔  When the monitor ticks with its default settings
   ✔  Then no "metrics.snapshot" entry should be written
  Scenario: The owner browses and filters the Logs view
   ✔> Given a desktop library with recorded activity
   ?  When I open the Logs view and filter by operation "metadata.edit" and the last day
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:176:5
  Scenario: The owner exports the journal as CSV
   ✔> Given a desktop library with recorded activity
   ✔  When I export the journal filtered by library and level "warn" as CSV
   ✔  Then a CSV file with one row per matching entry and opaque IDs only should be downloaded
   ✔  And the journal should record a "journal.export" entry
  Scenario: A non-owner cannot export the journal
   ✔> Given a desktop library with recorded activity
   ✔  Given a caller without the owner credential
   ✔  When it requests the journal export
   ✔  Then the request should be refused and journaled as a refusal
  Scenario: A non-owner cannot open the Logs view
   ✔> Given a desktop library with recorded activity
   ✔  Given a caller without the owner credential
   ?  When it opens the Logs view
      Step skipped: tests/features/durable_journal_and_journal_tab_F-061.feature:193:5
```
