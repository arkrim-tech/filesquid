# Task DE-1.9 Walkthrough: Desktop wiring of the job routes and import job controls

Branch `feat/de-1.9-desktop-job-wiring`. Module row [DE-1.9](../plan.md) of global [Task FS-1.4](../../../../../../resources/implementation-plan.md#task-fs-1.4). With it every module row of FS-1.4 is ticked (API-F1.6, API-F1.7, DE-1.9), but FS-1.4's own box stays open: the API-F1.6 status records that its Monitor-link hiding and self-scoped journal route have no module row yet. No contract change (no `openapi.json` or `API_VERSION` bump).

## What was already on main

| Part of the row | Done by |
|---|---|
| This launch's `Jobs` (SQLite store of DE-0.4) installed in `ApiState`, so the job routes answer on `filesquid://` instead of 503 | API-F2.17 (PR #132), desktop `install_job_routes` |
| Every stored job change publishes `job.update` on the desktop event bus | API-F2.17 (`open_jobs(..).with_events(..)` in `prepare_backfill`) |
| The history backfill keeps its own launcher slot, not a job runner | DE-0.10 / FS-2.8 (`install_backfill_route`) |
| `ChangeModel::stage_file` | IO-1.7 (PR #142) |

## What changed

- **`filesquid-native`**
  - [`workers/history_backfill.rs`](../../../native/src/workers/history_backfill.rs) `open_jobs`: `import` is classed as a transfer (`JobClass::Transfer`), so at most 4 run at once (API-S-037, CORE-S-006).
  - [`import/worker.rs`](../../../native/src/import/worker.rs): `ImportWorker` implements `JobRunner`; a resumed import is queued like a new one (`queue_full`/`unavailable` answer `JobError::Unavailable`, the job stays pending). A queued or recovered job that is paused is never started (no `job.refused` or `import.job` entry); one found finished (cancelled while pending) has its plan finished and its intent cleared.
  - [`import/job.rs`](../../../native/src/import/job.rs): `run_manifest` reads `RunningJob::requested()` after every per-file checkpoint and halts only through `RunningJob::halt_if_requested()`; a control read at a lease renewal between attempts of one file stops the attempts with the checkpoint at that file. A cancelled run finishes its plan and clears its intent, except when a committed file is still unregistered: the job then suspends (or, under a cancel, ends) with the intent kept. The final attempt of a file is never halted, so a file that exhausts its attempts is quarantined. The loop is bounded by the manifest plus `MAX_CONTINUED_HALTS` (1,000). Each file is imported on a scoped thread with its own runtime while the worker thread renews the job's lease every third of its length (`renewal_interval`, 5 s for the 15 s lease), since the storage backends block.
  - [`import/source.rs`](../../../native/src/import/source.rs) (new): `SourceFile`, the read-only one-file storage view (`SOURCE_KEY`), entity tag from size, modification time and device/inode, never from the bytes; errors name the key, never the path.
  - [`import/file.rs`](../../../native/src/import/file.rs): `MAX_IN_MEMORY_BYTES`, `ImportFailure::TooLarge` and `too_large_for_this_build` removed; `commit_failure` (now public) maps `size_mismatch`, `source_changed` and `source_unreadable` to retryable per-file failures.
- **`filesquid-api`** (smallest adapter change, no route or contract change): `FileService::import_original` takes a `StorageBackend` source and key instead of bytes and stages the original with `ChangeModel::stage_file` after `reset`, in the new object's own staging area outside the library's write lock (taken for the commit only), checking the reported size against the source's size before and refusing a source whose size or entity tag changed by the end (`source_changed`); a source that cannot be read is `source_unreadable`; what a failed attempt staged is cleared. `ChangeError::SourceChanged` answers `conflict` with `source_changed` in the message. New `canonical::commit_staged_original` checks that exactly the original is staged with the cited digest, then commits it with its record; `check_new_content` checks a record against a digest and size (`check_new_original` delegates to it).
- **`filesquid-desktop`**: `install_import_runner` installs the worker as the `import` runner in `start_imports`, before the router serves.
- **Workspace `Cargo.toml`**: `[profile.dev.package.sha2] opt-level = 3`. Unoptimised SHA-256 made one 600 MiB import take over 200 s in a test build (about 40 s with it). It affects dev and test builds only.
- **Docs**: DE-S-019 (every file staged through `stage_file` into the on-disk staging area; the 512 MiB refusal no longer applies; lease renewal while one file is imported), root README import section and test commands, DE-1.9 plan and checklist ticks, progress block.

## DE-S-020: Desktop job routes

| Constraint | Test |
|---|---|
| Job service, `import` runner and kind classes installed; routes answer, not `unavailable` | `a_paused_import_stops_after_the_file_in_hand_and_resumes_without_reading_it_again` (`GET /api/v1/jobs?kind=import` 200); `import_jobs_are_transfers_so_at_most_four_run_at_once` (fifth refused `slots_full`) |
| Pause suspends after the file in hand, with its checkpoint | same test: `paused`, `done` 1 of 3, one file imported |
| Resume hands the job to the import runner and continues the same manifest without reading a finished file | same test: `scheduled: true`; the first source file is deleted before the resume and the job still completes with 3 files and no quarantine |
| Cancel ends cancelled after the file in hand, no further file read | `a_cancelled_import_ends_after_the_file_in_hand_with_no_further_file_read` (1 file, 1 object) |
| A resume that cleared the pause before the checkpoint keeps the run going | `a_resume_that_clears_the_pause_before_the_checkpoint_keeps_the_run_going` (`attempts` 1, no `job.suspended`) |
| A paused job is not started by the launch-time replay | `a_paused_import_is_not_started_by_the_next_launch_until_it_is_resumed` (still paused, `attempts` 1, no `job.refused`/`import.job` entry) |
| Each control journaled once, by the API | `job.paused`, `job.resumed`, `job.suspended`, `job.cancel_requested`, `job.cancelled` each counted exactly once |
| Complete ordered `job.update` sequence, none dropped | exact sequence asserted (10 updates for pause/resume, 5 for cancel); a `Lagged` receive fails the test |
| No source path or name in answers, events or journal | rig asserts every answer; names asserted absent from the journal and the collected events |

## DE-S-019: Desktop file and folder import (streamed stage, attempt rule)

| Constraint | Test |
|---|---|
| File above 512 MiB imported, no `too_large_for_this_build`, bounded memory | `a_600_mib_file_is_imported_within_bounded_memory` (own binary; peak `VmHWM` growth under 256 MiB on Linux; digest and size cited) |
| Source read at most twice; changed source fails `source_changed`, retried | `a_source_that_changes_between_its_two_reads_is_refused_as_source_changed` (2 passes, nothing published), `a_size_mismatch_and_a_changed_source_are_retried_within_the_job` |
| Size checked | `a_size_that_is_not_the_size_read_is_refused_and_nothing_is_published` |
| Entity tag changes with the file, view is read-only and one-key | `the_source_entity_tag_changes_when_the_file_is_rewritten` |
| Lease kept while one file takes longer than a lease | `a_file_that_streams_longer_than_the_lease_keeps_the_job_s_lease_alive` (300 ms lease, 96 MiB file, one run) |

## OCFL-S-026, API-S-033, API-S-037, NFR-003

OCFL-S-026 (streamed single-file stage) is now used by the import; API-S-033 (durable jobs, lease renewal) and API-S-037 (job routes, controls and events) are exercised on the desktop mount by the tests above; NFR-003 (bounded processing) by the 600 MiB import.

## BDD

`desktop_imports_F-002.feature` gains "A paused import stops after the file in hand and resumes where it stopped" and "A cancelled import ends after the file in hand and reads no further file" (steps in `tests/support/import_steps.rs`).

## TDD: failing run before the implementation

```text
error[E0432]: unresolved import `filesquid_native::import::source`
  --> crates/native/tests/unit/import_file.rs:21:31
error[E0432]: unresolved import `filesquid_native::import::source`
 --> crates/native/tests/unit/../support/changing_source.rs:6:31
error[E0603]: function `commit_failure` is private
  --> crates/native/tests/unit/import_file.rs:20:38
  --> crates/native/src/import/file.rs:98:1
error[E0061]: this method takes 3 arguments but 4 arguments were supplied
   --> crates/native/tests/unit/import_file.rs:196:10
   --> crates/api/src/files/import.rs:89:18
error[E0061]: this method takes 3 arguments but 4 arguments were supplied
   --> crates/native/tests/unit/import_file.rs:216:10
   --> crates/api/src/files/import.rs:89:18
error: could not compile `filesquid-native` (test "unit_import_file") due to 5 previous errors
error[E0432]: unresolved import `filesquid_desktop_lib::install_import_runner`
  --> crates/desktop/tests/integration/../support/import_rig.rs:24:62
error: could not compile `filesquid-desktop` (test "integration_import_controls") due to 1 previous error
```

## Test output

`cargo test -p filesquid-native --features desktop --locked --test unit_import_file --test unit_import_job --test unit_import_large` and `cargo test -p filesquid-desktop --locked --test integration_import_controls --test integration_durable_jobs --test integration_imports`:

```text
running 6 tests
test a_size_mismatch_and_a_changed_source_are_retried_within_the_job ... ok
test a_missing_source_is_refused_without_retry ... ok
test a_size_that_is_not_the_size_read_is_refused_and_nothing_is_published ... ok
test a_source_that_changes_between_its_two_reads_is_refused_as_source_changed ... ok
test the_source_entity_tag_changes_when_the_file_is_rewritten ... ok
test an_import_stores_the_original_under_its_uuid_with_its_record_in_one_version ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s


running 20 tests
test a_deleted_file_s_path_counts_as_taken ... ok
test a_file_committed_before_a_crash_is_answered_not_imported_twice ... ok
test a_crash_between_a_file_s_object_and_its_registration_resumes_and_registers_once ... ok
test a_file_that_keeps_failing_is_quarantined_after_five_attempts_and_the_rest_go_on ... ok
test a_folder_is_imported_as_one_job_with_a_checkpoint_per_file ... ok
test a_folder_is_listed_in_one_stable_order_with_its_subfolders ... ok
test a_folder_imported_again_is_numbered_beside_the_first ... ok
test a_plan_and_its_manifest_survive_a_restart_and_one_operation_has_one_job ... ok
test a_plan_without_a_destination_fails_its_job_as_destination_required ... ok
test a_file_the_index_keeps_refusing_to_register_suspends_the_job_and_keeps_its_intent ... ok
test a_numbered_name_over_255_bytes_is_quarantined_as_name_too_long ... ok
test a_source_whose_numbers_are_all_taken_is_quarantined_as_path_suffix_exhausted ... ok
test checkpoints_backoff_and_reserve_follow_their_rules ... ok
test a_queued_job_without_a_plan_is_failed_not_left_pending ... ok
test picked_files_are_imported_each_with_itself_as_its_root ... ok
test two_picked_folders_of_one_name_land_beside_each_other ... ok
test a_quit_suspends_between_files_and_the_resume_imports_only_the_rest ... ok
test what_cannot_be_listed_is_kept_with_its_code ... ok
test a_file_that_streams_longer_than_the_lease_keeps_the_job_s_lease_alive ... ok
test the_worker_runs_every_unfinished_job_beyond_one_queue_full ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.20s


running 1 test
test a_600_mib_file_is_imported_within_bounded_memory ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 41.35s


running 9 tests
test a_paused_backfill_job_waits_until_it_is_resumed ... ok
test a_job_for_another_installation_is_failed_and_a_new_one_runs ... ok
test import_jobs_are_transfers_so_at_most_four_run_at_once ... ok
test quitting_suspends_the_backfill_job_with_its_checkpoint_kept ... ok
test a_backfill_job_completes_and_records_both_libraries_in_its_checkpoint ... ok
test restart_recovery_reads_only_jobs_and_stays_within_the_budget ... ok
test the_next_launch_resumes_the_job_without_rescanning_completed_libraries ... ok
test a_run_left_by_a_crash_is_waited_out_and_resumed ... ok
test an_unreadable_checkpoint_visits_every_library_again ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.62s


running 4 tests
test a_cancelled_import_ends_after_the_file_in_hand_with_no_further_file_read ... ok
test a_resume_that_clears_the_pause_before_the_checkpoint_keeps_the_run_going ... ok
test a_paused_import_stops_after_the_file_in_hand_and_resumes_without_reading_it_again ... ok
test a_paused_import_is_not_started_by_the_next_launch_until_it_is_resumed ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.01s


running 6 tests
test a_cancelled_dialog_or_an_unopened_library_makes_no_job ... ok
test an_import_without_a_destination_is_refused_before_any_dialog ... ok
test a_picked_folder_is_imported_as_one_job_and_a_repeat_answers_the_same_job ... ok
test a_job_the_stopped_worker_could_not_take_runs_at_the_next_launch ... ok
test an_operation_id_reused_for_another_import_is_refused_not_answered_with_its_job ... ok
test imported_files_appear_under_their_collection_and_folder ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.45s
```

`cargo test -p filesquid-desktop --features test-webdriver --locked --test features` (import feature; whole run below):

```text
Feature: Desktop file and folder import
  Scenario: A picked folder is imported as one job, one object per file
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import a folder under the operation "op-holidays"
   ✔  Then the answer should name a started import job
   ✔  And 2 files should be imported, each as its own object
   ✔  And no journal entry should name a picked file or folder
  Scenario: Repeating an operation answers the same job without a second dialog
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import a folder under the operation "op-holidays"
   ✔  And the app asks to import a folder under the operation "op-holidays" again
   ✔  Then both answers should name the same import job
   ✔  And the dialog should have been shown once
   ✔  And 2 files should be imported, each as its own object
  Scenario: A cancelled dialog makes no job
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will close the dialog without picking anything
   ✔  When the app asks to import files under the operation "op-cancel"
   ✔  Then the answer should say the import was cancelled
   ✔  And no file should be imported
  Scenario: Imported files appear under the collection and folder the import names
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import a folder under the operation "op-trip"
   ✔  Then 2 files should be imported, each as its own object
   ✔  And the collection should list the folder "trips/Holidays" holding "Beach day.jpg" and "Notes.txt"
  Scenario: A folder imported again sits beside the first under a numbered name
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import a folder under the operation "op-first"
   ✔  And the import has finished with 2 files
   ✔  And the app asks to import a folder under the operation "op-again"
   ✔  Then the collection should list the folders "Holidays" and "Holidays-1" in "trips"
  Scenario: An import that names no destination collection is refused before any dialog
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import a folder without naming a destination collection
   ✔  Then the import should be refused as an invalid request naming "destination_required"
   ✔  And the dialog should not have been shown
   ✔  And no file should be imported
  Scenario: A library that is not open is refused and journaled once without a path
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import into a library that is not open
   ✔  Then the import should be refused as not found
   ✔  And the dialog should not have been shown
   ✔  And the journal should record one import request without any path
  Scenario: An import the host could not run now runs at the next launch
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  And the import worker has stopped for the app to quit
   ✔  When the app asks to import a folder under the operation "op-later"
   ✔  Then the import should be refused as unavailable
   ✔  And after the app restarts 2 files should be imported, each as its own object
  Scenario: A paused import stops after the file in hand and resumes where it stopped
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  And the import worker will be held on its first file
   ✔  When the app asks to import a folder under the operation "op-paused"
   ✔  And the owner pauses the import while its first file is in hand
   ✔  Then the import job should be paused with 1 of 2 files done
   ✔  And the job control "job.paused" should be journaled once
   ✔  When the owner resumes the import job
   ✔  Then the import job should be completed with 2 of 2 files done
   ✔  And 2 files should be imported, each as its own object
   ✔  And no journal entry should name a picked file or folder
  Scenario: A cancelled import ends after the file in hand and reads no further file
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  And the import worker will be held on its first file
   ✔  When the app asks to import a folder under the operation "op-stopped"
   ✔  And the owner cancels the import while its first file is in hand
   ✔  Then the import job should be cancelled with 1 of 2 files done
   ✔  And the job control "job.cancel_requested" should be journaled once
   ✔  And only the file in hand should be imported

50 scenarios (50 passed)
225 steps (225 passed)
```

## Known limits

- The heartbeat of one file is bounded at 24 hours (AGENTS.md §22). Past it the lease lapses and the run is fenced at its next checkpoint; the next run answers the committed file instead of importing it twice.

- The per-file items and retry routes of API-S-037 (`/items`, `/retry`, `/retry-failed`) are API-F2.16, not this row.
- A control read between attempts of one file stops those attempts with the checkpoint at that file, so the next run retries it from its first attempt.

## Mocked Environment Variables

None.

## Pre-commit

The head-commit run (`FULL=1 bash scripts/pre-commit.sh` with a private target, its summary line `Test scope: full`) is recorded in the pull request.
