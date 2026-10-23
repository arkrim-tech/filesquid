# Task API-F2.17 Walkthrough: Rebuild from files as a durable job

Branch `feat/api-f2.17-rebuild-job`, module `filesquid-api` ([plan row](../plan.md)), dispatched by global Task 1.2. Contract **0.25.0** (main was 0.24.0). Mockup: [04-recover-library.svg](../../../../../../resources/ui-mockups/v3/04-recover-library.svg) ([UI-S-027](../../../../ui/resources/srs.md)).

Owner answer 9 of 2026-10-22, verbatim: "Yes, rebuild should be a durable job". The job controls take no `CommandEnvelope` (owner decision 2026-10-23, endorsing API-S-037's exemption).

## API-S-025: Local library and repair routes (Rebuild as a job)

`POST /api/v1/libraries/rebuilds` (`library.manage`) refuses a library that is not open, a malformed body and a host without a job service before any job is submitted (each journaled once as `library.rebuild`), then answers `{outcome: pending, job_id, job, scheduled}`: a job of kind `library.rebuild` whose source identity is `library:<id>` (stored, never answered or journaled). A library whose rebuild job is unfinished answers that job, and no second job is submitted; when 1,000 rebuild jobs are unfinished and none is the library's, the request is refused `unavailable` rather than risk a duplicate. The host's runner reads the objects in object-root order and, every batch (at least 64 objects, and at least a sixteenth of the objects reported so far, so a large report is rewritten a bounded number of times), stores in one atomic step a checkpoint holding only the root of the last object read and the report so far as the job's stored result, at most 64 MiB (`MAX_RESULT_BYTES`; past it the job fails `too_large`). Each checkpoint and each lease renewal reads the control asked of the job; once a pause or cancel is read, the run checkpoints after the object it is reading and stops there, also when the request comes during the last object; a resumed run, and a run after a crash once recovery released its lease, continue after the cursor, so no checkpointed object is read again and none is reported twice. A finished run completes the job with the report (objects read, objects not readable, each object's validation errors, and the journal restore) as its result, and the finished rebuild is journaled once as `library.rebuild`. A lease ticker renews the lease (every third of it by default) while one long object is read. Nothing is written to the library's files, so a cancel changes none of them.

## API-S-037: Job routes and job events

`JobView` gains `result`: once the job has completed, the result its kind defines (the `RebuildReport` for `library.rebuild`), answered by `GET /api/v1/jobs/{job}` and the control routes; `null` before then, after a failure or cancel, for a kind without one, and in the job listing and the `job.update` event (`JobView::summary`), so neither grows with a report (Cubic round 2; API-S-037 now says so). The view still carries no source identity, checkpoint, lease holder or fence. Pause, resume and cancel work on the rebuild job as on every job: each change journaled once (`job.paused`, `job.resumed`, `job.cancel_requested`, `job.cancelled`) under the job's operation ID; a repeated pause changes nothing and is not journaled.

## API-S-033: Durable job model

`Job.result` is stored with the job (`RunningJob::checkpoint_with_result` writes it with the checkpoint under the lease, `complete_with_result` with the end, and a job finished without a result clears a partial one); a write under an ended lease is fenced and stores neither, and a result past `MAX_RESULT_BYTES` is refused `too_large`. The SQLite job store adds a `result` column with an additive migration and keeps it across a reopen.

| # | Constraint | Gate | State |
|---|---|---|---|
| C1 | The rebuild answers pending with a `library.rebuild` job, handed to the runner | `the_rebuild_answers_pending_with_a_library_rebuild_job_whose_result_is_the_report` (both mounts), `a_rebuild_is_a_pending_library_rebuild_job_whose_source_names_the_library`, scenario "The rebuild from files runs as a durable job whose result is the report", desktop `an_opened_folder_feeds_the_history_and_copies_and_rebuilds_without_paths` | Met |
| C2 | A repeated rebuild answers the unfinished job; past the 1,000-job read bound it is refused, never duplicated | `a_repeated_rebuild_answers_the_unfinished_job`, `a_submission_past_the_unfinished_bound_is_refused_instead_of_duplicated`, `a_repeated_rebuild_answers_the_unfinished_job_and_cancel_changes_no_file` | Met |
| C3 | Progress counts objects (`done`, `total`) | `progress_counts_objects_and_the_report_is_the_jobs_result`, `an_empty_library_completes_with_zero_of_zero_objects`, `a_rebuild_from_a_cursor_reads_no_object_at_or_before_it`, scenario step "report 2 of 2 objects done" | Met |
| C4 | Pause holds the job at a checkpoint | `pause_holds_the_rebuild_at_a_checkpoint_and_resume_reads_no_completed_object_again`, `a_paused_rebuild_is_not_started_until_it_is_resumed`, scenario "A paused rebuild waits..." | Met |
| C5 | Resume reads no completed object again | `pause_holds_the_rebuild_at_a_checkpoint_and_resume_reads_no_completed_object_again`, `a_rebuild_from_a_cursor_reads_no_object_at_or_before_it` (a rebuild without a cursor reads the object's inventory; one after it does not) | Met |
| C6 | Cancel ends it with no change to the library's files | `cancel_ends_the_rebuild_as_cancelled`, `a_cancel_asked_during_the_last_object_cancels_instead_of_completing`, `cancelling_a_pending_rebuild_reads_nothing`, `a_repeated_rebuild_answers_the_unfinished_job_and_cancel_changes_no_file`, `a_rebuild_its_progress_stops_is_not_journaled_as_finished` (no write), scenario | Met |
| C7 | A crash resumes from the checkpoint | `a_crash_resumes_from_the_checkpoint` (the run is dropped mid-object; recovery releases the expired lease; objects 0 to 3 are not read again) | Met |
| C8 | The report is the job's result in the view once finished; `null` otherwise | `the_view_carries_the_result_once_the_job_completes_and_null_otherwise`, `completing_without_a_result_clears_a_partial_one`, `the_summary_view_and_the_job_update_event_carry_no_result`, the listing assertion of the route test, `the_view_carries_no_source_checkpoint_lease_or_fence`, `result_of` in every runner test | Met |
| C9 | Every control journaled once; no source, checkpoint or report journaled | `every_control_is_journaled_once_and_no_entry_names_the_source_or_checkpoint`, scenario "journaled once for the rebuild job", `assert_redacted` in the route test | Met |
| C10 | Refusals before a job is submitted, journaled once | `a_rebuild_of_a_library_that_is_not_open_submits_no_job`, scenario "A rebuild of a library that is not open..." | Met |
| C11 | No job service: `503 unavailable`, journaled once | `a_rebuild_on_a_host_without_a_job_service_is_unavailable_and_journaled_once` | Met |
| C12 | The lease is kept while one long object is read | `a_long_object_keeps_the_lease` (the clock moves 30 s past a 15 s lease during one object) | Met |
| C13 | The checkpoint is a cursor; the partial report is written with it atomically, bounded, at a cadence that grows with the report | `a_result_past_its_bound_is_refused_and_stores_nothing`, `a_final_result_past_its_bound_is_refused_and_stores_nothing`, `the_checkpoint_holds_a_cursor_and_the_partial_report_is_stored_with_it`, `a_checkpoint_with_a_result_under_an_ended_lease_is_fenced_and_stores_nothing` | Met |
| C14 | `library.rebuild` journaled once when the rebuild finishes | route test, `a_rebuild_from_a_cursor_reads_no_object_at_or_before_it` | Met |
| C15 | The SQLite store keeps the result | `a_jobs_result_survives_closing_the_store`, `a_store_made_with_controls_but_before_results_gains_the_result_column`, `a_store_made_before_controls_gains_the_column_and_keeps_its_jobs` | Met |
| C16 | Contract 0.25.0 and regenerated `schema.d.ts` | `unit_contract` (samples of `RebuildJobAnswer`, `RebuildOutcome` and a completed rebuild's `JobView`), Vitest `contract.test.ts` | Met |
| C17 | The desktop installs its job service and the rebuild runner | desktop `an_opened_folder_feeds_the_history_and_copies_and_rebuilds_without_paths` (SQLite job store, `job.completed` in the SQLite journal); WebdriverIO spec `libraries.e2e.mjs` follows the job | Met |
| C18 | Bounded loops | `MAX_RUN_PASSES` (1,000 passes per run, then the run gives its lease back), the renewal task ends with its pass, test polls bounded by `MAX_JOB_POLLS` | Met |

## What changed

- `filesquid-api`
  - [`jobs/model.rs`](../../src/jobs/model.rs): `Job.result`. [`jobs/rules.rs`](../../src/jobs/rules.rs): `checkpoint_with_result`. [`jobs/service.rs`](../../src/jobs/service.rs): `RunningJob::checkpoint_with_result` and `complete_with_result`. [`jobs/view.rs`](../../src/jobs/view.rs): `JobView.result`, shown once completed.
  - [`local_library/rebuild.rs`](../../src/local_library/rebuild.rs): `rebuild_from` reads the objects after a cursor and tells a `RebuildProgress` about each one (`RebuildStep`, `RebuildEntry`, `RebuildEnd`); the one-shot `rebuild` uses it.
  - [`local_library/rebuild_job.rs`](../../src/local_library/rebuild_job.rs) (new): `REBUILD_JOB_KIND`, `submit_rebuild`, `rebuild_source`, `RebuildJobAnswer`, and `RebuildRunner`, the `JobRunner` of the kind.
  - [`local_library/service.rs`](../../src/local_library/service.rs), [`commands.rs`](../../src/local_library/commands.rs), [`operations.rs`](../../src/local_library/operations.rs): `LibraryOperations::rebuild` is replaced by `rebuild_ready` and `rebuild_from`.
  - [`local_library/routes.rs`](../../src/local_library/routes.rs): the rebuild route answers the job.
  - [`openapi.json`](../../openapi.json): 0.25.0, `RebuildJobAnswer`, `RebuildOutcome`, `JobView.result`; `tokio`'s `time` feature for the renewal ticker (no new crate; `Cargo.lock` unchanged).
- `filesquid-native`: [`jobs_sqlite.rs`](../../../native/src/jobs_sqlite.rs) stores `result` (additive migration of the `jobs` table).
- `filesquid-desktop`: [`lib.rs`](../../../desktop/src/lib.rs) `install_job_routes` installs this launch's job service in the API state (the job routes answered `unavailable` on the desktop before) and the rebuild runner on the app's runtime; the job service publishes `job.update` events; [`libraries.rs`](../../../desktop/src/libraries.rs) forwards the new operations.
- `apps/filesquid/ui/src/api/schema.d.ts`: regenerated only. The default UI still calls the rebuild route for a report (`fs-library-start.ts`, `fs-library-detail.ts`); following the job is UI Task UI-1.3.
- Docs: module SRS API-S-025 (one-job-per-library, refusals before submission, the cursor checkpoint and the stored partial report), desktop SRS DE-S-014 (the stored result, the job service installed for the job routes), plan row, checklist and progress bar. No new environment variable, service or command, so the root README is unchanged.

## Where the line with DE-1.8 is

This task ships the route, the `library.rebuild` kind, the runner contract (`LibraryOperations::rebuild_ready` and `rebuild_from`, `RebuildProgress`) and the API's runner, the SQLite `result` column, and the desktop wiring: the job service and the rebuild runner installed on the app's runtime. A rebuild started on the desktop runs, pauses, resumes through `POST /api/v1/jobs/{job}/resume` and cancels now. Desktop Task DE-1.8 owns: handing an unfinished rebuild job to the runner at launch after a restart (until then it waits, pending, for the owner's resume), suspending a running rebuild at app exit with a bounded wait (until then an exit is a crash: the lease expires and recovery releases it), the batch size the host tunes, and the restart test.

## Tests

TDD: the tests were written first. Against the code before the change, the new tests did not compile, because the runner, the progress port and the job result did not exist:

```text
error[E0432]: unresolved imports `filesquid_api::local_library::rebuild_source`, `filesquid_api::local_library::submit_rebuild`, `filesquid_api::local_library::RebuildEnd`, `filesquid_api::local_library::RebuildEntry`, `filesquid_api::local_library::RebuildProgress`, `filesquid_api::local_library::RebuildRunner`, `filesquid_api::local_library::RebuildStep`, `filesquid_api::local_library::REBUILD_JOB_KIND`
  --> crates/api/tests/unit/rebuild_job.rs:15:5
error[E0407]: method `rebuild_ready` is not a member of trait `LibraryOperations`
   --> crates/api/tests/unit/rebuild_job.rs:167:5
error[E0407]: method `rebuild_from` is not a member of trait `LibraryOperations`
   --> crates/api/tests/unit/rebuild_job.rs:177:5
error[E0407]: method `rebuild_ready` is not a member of trait `LibraryOperations`
   --> crates/api/tests/unit/rebuild_job.rs:591:5
error[E0407]: method `rebuild_from` is not a member of trait `LibraryOperations`
   --> crates/api/tests/unit/rebuild_job.rs:595:5
error[E0046]: not all trait items implemented, missing: `rebuild`
   --> crates/api/tests/unit/rebuild_job.rs:150:1
error[E0046]: not all trait items implemented, missing: `rebuild`
   --> crates/api/tests/unit/rebuild_job.rs:574:1
error: could not compile `filesquid-api` (test "unit_rebuild_job") due to 7 previous errors
```

```text
error[E0599]: no method named `checkpoint_with_result` found for struct `RunningJob` in the current scope
   --> crates/api/tests/unit/job_control.rs:527:9
    |
527 |     run.checkpoint_with_result("next=1", r#"{"objects":[1]}"#, 1, Some(2))
    |         ^^^^^^^^^^^^^^^^^^^^^^
    |
help: there is a method `checkpoint_text` with a similar name, but with different arguments
   --> crates/api/src/jobs/service.rs:438:5
    |
438 |     pub fn checkpoint_text(&self) -> Option<&str> {
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error[E0609]: no field `result` on type `Job`
   --> crates/api/tests/unit/job_control.rs:530:24
    |
530 |     assert_eq!(running.result.as_deref(), Some(r#"{"objects":[1]}"#));
    |                        ^^^^^^ unknown field
    |
    = note: available fields are: `id`, `kind`, `operation_id`, `source`, `state` ... and 11 others

error[E0609]: no field `result` on type `JobView`
   --> crates/api/tests/unit/job_control.rs:532:38
    |
532 |     assert_eq!(JobView::of(&running).result, None, "not before it finishes");
    |                                      ^^^^^^ unknown field
    |
    = note: available fields are: `id`, `kind`, `operation_id`, `state`, `paused` ... and 7 others

error[E0599]: no method named `complete_with_result` found for struct `RunningJob` in the current scope
   --> crates/api/tests/unit/job_control.rs:533:9
    |
533 |     run.complete_with_result(r#"{"objects":[1,2]}"#)
    |         ^^^^^^^^^^^^^^^^^^^^ method not found in `RunningJob`

error[E0609]: no field `result` on type `JobView`
   --> crates/api/tests/unit/job_control.rs:538:28
    |
538 |         JobView::of(&done).result,
    |                            ^^^^^^ unknown field
    |
    = note: available fields are: `id`, `kind`, `operation_id`, `state`, `paused` ... and 7 others

error[E0599]: no method named `checkpoint_with_result` found for struct `RunningJob` in the current scope
   --> crates/api/tests/unit/job_control.rs:551:10
    |
550 |       let refused = run
    |  ___________________-
551 | |         .checkpoint_with_result("next=1", "{}", 1, Some(2))
    | |_________-^^^^^^^^^^^^^^^^^^^^^^
    |
help: there is a method `checkpoint_text` with a similar name, but with different arguments
   --> crates/api/src/jobs/service.rs:438:5
    |
438 |     pub fn checkpoint_text(&self) -> Option<&str> {
    |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error[E0609]: no field `result` on type `Job`
   --> crates/api/tests/unit/job_control.rs:555:43
    |
555 |     assert_eq!((stored.checkpoint, stored.result), (None, None));
    |                                           ^^^^^^ unknown field
    |
    = note: available fields are: `id`, `kind`, `operation_id`, `source`, `state` ... and 11 others

Some errors have detailed explanations: E0599, E0609.
For more information about an error, try `rustc --explain E0599`.
error: could not compile `filesquid-api` (test "unit_job_control") due to 7 previous errors
```

```text
error[E0432]: unresolved imports `filesquid_api::local_library::RebuildEnd`, `filesquid_api::local_library::RebuildProgress`, `filesquid_api::local_library::RebuildStep`
error: could not compile `filesquid-api` (test "unit_local_library") due to 1 previous error
```

The first green run of `unit_rebuild_job` failed one assertion, which expected a pause to stop the run at the paused object; API-S-037 says a running job suspends "at its next checkpoint", so the test now expects the stop after the batch.

`cargo test -p filesquid-api --locked --test unit_rebuild_job`:

```text
     Running tests/unit/rebuild_job.rs (<target>/debug/deps/unit_rebuild_job-f8523ef55133abfb)
running 16 tests
test a_library_that_is_no_longer_open_fails_the_job_with_its_code ... ok
test a_cancel_asked_during_the_last_object_cancels_instead_of_completing ... ok
test a_rebuild_is_a_pending_library_rebuild_job_whose_source_names_the_library ... ok
test a_paused_rebuild_is_not_started_until_it_is_resumed ... ok
test a_repeated_rebuild_answers_the_unfinished_job ... ok
test an_empty_library_completes_with_zero_of_zero_objects ... ok
test a_long_object_keeps_the_lease ... ok
test cancelling_a_pending_rebuild_reads_nothing ... ok
test a_submission_past_the_unfinished_bound_is_refused_instead_of_duplicated ... ok
test cancel_ends_the_rebuild_as_cancelled ... ok
test progress_counts_objects_and_the_report_is_the_jobs_result ... ok
test the_checkpoint_holds_a_cursor_and_the_partial_report_is_stored_with_it ... ok
test pause_holds_the_rebuild_at_a_checkpoint_and_resume_reads_no_completed_object_again ... ok
test every_control_is_journaled_once_and_no_entry_names_the_source_or_checkpoint ... ok
test the_view_carries_no_source_checkpoint_lease_or_fence ... ok
test a_crash_resumes_from_the_checkpoint ... ok
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
```

`cargo test -p filesquid-api --locked --test unit_job_control result`:

```text
     Running tests/unit/job_control.rs (<target>/debug/deps/unit_job_control-9e2eb351d6a625a3)
running 6 tests
test a_checkpoint_with_a_result_under_an_ended_lease_is_fenced_and_stores_nothing ... ok
test completing_without_a_result_clears_a_partial_one ... ok
test the_summary_view_and_the_job_update_event_carry_no_result ... ok
test the_view_carries_the_result_once_the_job_completes_and_null_otherwise ... ok
test a_final_result_past_its_bound_is_refused_and_stores_nothing ... ok
test a_result_past_its_bound_is_refused_and_stores_nothing ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 19 filtered out; finished in 0.05s
```

`cargo test -p filesquid-api --locked --test unit_local_library a_rebuild`:

```text
     Running tests/unit/local_library.rs (<target>/debug/deps/unit_local_library-1476f93862c0072d)
running 2 tests
test a_rebuild_its_progress_stops_is_not_journaled_as_finished ... ok
test a_rebuild_from_a_cursor_reads_no_object_at_or_before_it ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.01s
```

`cargo test -p filesquid-api --locked --test integration_library_routes rebuild`:

```text
     Running tests/integration/library_routes.rs (<target>/debug/deps/integration_library_routes-fee10c62a04963dd)
running 4 tests
test a_rebuild_on_a_host_without_a_job_service_is_unavailable_and_journaled_once ... ok
test a_rebuild_of_a_library_that_is_not_open_submits_no_job ... ok
test a_repeated_rebuild_answers_the_unfinished_job_and_cancel_changes_no_file ... ok
test the_rebuild_answers_pending_with_a_library_rebuild_job_whose_result_is_the_report ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 22 filtered out; finished in 0.10s
```

`cargo test -p filesquid-native --locked --test unit_jobs`:

```text
     Running tests/unit/jobs.rs (<target>/debug/deps/unit_jobs-21f13af35310d138)
running 13 tests
test a_count_past_the_signed_range_is_refused ... ok
test a_job_and_its_checkpoint_survive_closing_the_store ... ok
test a_listing_is_newest_first_filtered_and_marked_truncated ... ok
test a_counted_update_sees_every_live_lease_of_the_kinds ... ok
test a_refused_update_stores_nothing_and_a_known_id_is_not_inserted_twice ... ok
test a_jobs_result_survives_closing_the_store ... ok
test a_requested_control_survives_closing_the_store ... ok
test an_update_that_changes_the_id_stores_nothing ... ok
test a_store_made_with_controls_but_before_results_gains_the_result_column ... ok
test a_store_made_before_controls_gains_the_column_and_keeps_its_jobs ... ok
test the_job_store_shares_the_owner_only_operational_database ... ok
test unfinished_jobs_are_listed_oldest_first_and_bounded ... ok
test two_connections_inserting_one_job_id_store_it_once ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

`cargo test -p filesquid-desktop --locked --test integration_library_routes`:

```text
     Running tests/integration/library_routes.rs (<target>/debug/deps/integration_library_routes-cbefee7c09ebded3)
running 8 tests
test a_cancelled_or_unusable_pick_registers_nothing ... ok
test create_in_a_chosen_folder_answers_an_opaque_library_and_serves_its_files ... ok
test the_navigation_lists_the_installations_repository_and_its_libraries ... ok
test the_owner_picks_a_folder_in_the_hosts_dialog_and_the_app_sees_only_its_id ... ok
test the_backfill_route_runs_over_the_folders_the_owner_opened ... ok
test the_owners_preferences_are_kept_in_the_operational_store ... ok
test an_opened_folder_feeds_the_history_and_copies_and_rebuilds_without_paths ... ok
test the_owners_consented_repair_is_kept_in_the_persistent_ledger ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```

`cargo test -p filesquid-api --locked --test unit_contract`:

```text
     Running tests/unit/contract.rs (<target>/debug/deps/unit_contract-e551d1b63735a1b3)
running 28 tests
test error_code_all_lists_every_variant ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test errors_carry_code_and_retryability ... ok
test event_topics_in_the_contract_match_the_rust_topics ... ok
test every_component_schema_has_a_rust_sample ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
test every_documented_operation_has_a_unique_operation_id ... ok
test every_error_response_carries_the_api_error_schema ... ok
test every_event_topic_documents_its_payload_schema ... ok
test every_operation_documents_both_denials ... ok
test every_limit_parameter_is_the_shared_page_size ... ok
test every_operation_names_the_action_the_router_authorises ... ok
test every_operation_requires_the_bearer_scheme_and_none_makes_it_optional ... ok
test health_schema_matches_the_core_payload_and_requires_every_field ... ok
test host_routes_are_the_event_channel_only ... ok
test query_envelope_and_page_size_parameter_are_bounded_like_core ... ok
test routed_and_documented_paths_agree_both_ways ... ok
test router_fallbacks_are_documented_as_wire_conventions ... ok
test router_serves_its_own_contract ... ok
test the_checker_accepts_only_truly_unconstrained_objects ... ok
test the_contract_uses_only_checked_schema_keywords ... ok
test the_only_public_operation_is_the_liveness_probe ... ok
test events_from_every_publisher_match_their_documented_payloads ... ok
test every_documented_operation_answers_a_documented_status_and_body ... ok
test router_fallbacks_answer_as_documented ... ok
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.03s
```

`npx vitest run tests/unit/contract.test.ts` (in `apps/filesquid/ui`, after `npm run contract:types`):

```text
 RUN  v4.1.12 <repo>/apps/filesquid/ui
 ✓ tests/unit/contract.test.ts (1 test) 206ms
 Test Files  1 passed (1)
      Tests  1 passed (1)
   Start at  18:19:14
   Duration  1.05s (transform 65ms, setup 55ms, import 648ms, tests 206ms, environment 0ms)
JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

BDD (`cargo test -p filesquid-api --locked --test features`), the API-S-025 scenarios:

```text
  Scenario: Copy a completed snapshot and rebuild from files through the API
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory
   ✔  When the owner copies a completed snapshot of the library to an empty folder through the API
   ✔  Then the copy should report 2 objects and no invalid object
   ✔  And the library folder should be unchanged
   ✔  When the owner rebuilds the library from its files through the API
   ✔  Then the rebuild should flag only the object without a version inventory
  Scenario: The candidate query offers only the object without a version inventory
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory
   ✔  When the owner asks the API which objects need a version-inventory repair
   ✔  Then only the object without a version inventory should be offered
  Scenario: A consented repair by the desktop owner makes the object committable
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory
   ✔  When the owner repairs the object without a version inventory with consent through the API
   ✔  Then the object should be answered as repaired
   ✔  And the consent and one outcome row per object should be recorded as "ocfl.repair.version_inventory"
   ✔  And the repaired object should accept a new version
  Scenario: A head that already has both files is refused with nothing_to_repair
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory
   ✔  When the owner repairs the complete object with consent through the API
   ✔  Then the object should be answered as refused with "nothing_to_repair"
   ✔  And the consent and one outcome row per object should be recorded as "ocfl.repair.version_inventory"
   ✔  And the object's outcome row should be failed with "nothing_to_repair"
   ✔  And the library folder should be unchanged
  Scenario: A repair without consent is refused once and changes nothing
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory
   ✔  When the owner asks the API to repair the object without a version inventory without consent
   ✔  Then the answer should be the error "conflict"
   ✔  And exactly one refusal "consent_required" should be journaled for "ocfl.repair.version_inventory"
   ✔  And the library folder should be unchanged
  Scenario: A repair without a credential is refused once
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory
   ✔  When a caller without a credential asks the API to repair the object without a version inventory
   ✔  Then the answer should be the error "unauthorized"
   ✔  And exactly one refusal "missing_credentials" should be journaled for "api.auth.denied"
   ✔  And the library folder should be unchanged
  Scenario: The server administrator is refused the repair
   ✔  Given the server administrator's library with a complete object and an object without a version inventory
   ✔  When the server administrator repairs the object without a version inventory with consent through the API
   ✔  Then the answer should be the error "forbidden"
   ✔  And exactly one refusal "not_permitted" should be journaled for "api.auth.denied"
   ✔  And the library folder should be unchanged
  Scenario: The server administrator can still rebuild a library
   ✔  Given the server administrator's library with a complete object and an object without a version inventory
   ✔  When the server administrator rebuilds the library from its files through the API
   ✔  Then the rebuild should flag only the object without a version inventory
  Scenario: The rebuild from files runs as a durable job whose result is the report
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory
   ✔  When the owner starts a rebuild of the library from its files through the API
   ✔  Then the answer should be a pending library.rebuild job without a result
   ✔  When the rebuild job finishes
   ✔  Then the rebuild job should report 2 of 2 objects done
   ✔  And the rebuild should flag only the object without a version inventory
   ✔  And the library folder should be unchanged
  Scenario: A paused rebuild waits, and a cancelled one ends with no change to the library
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory, on a host that runs no rebuild yet
   ✔  When the owner starts a rebuild of the library from its files through the API
   ✔  And the owner pauses the rebuild job through the API
   ✔  Then the rebuild job should be paused
   ✔  When the owner cancels the rebuild job through the API
   ✔  Then the rebuild job should be cancelled
   ✔  And the library folder should be unchanged
   ✔  And "job.paused" should be journaled once for the rebuild job
   ✔  And "job.cancelled" should be journaled once for the rebuild job
  Scenario: A rebuild of a library that is not open is refused once and starts no job
   ✔  Given the desktop owner's host with an empty folder registered
   ✔  When the owner rebuilds a library that is not open through the API
   ✔  Then the answer should be the error "not_found"
   ✔  And exactly one refusal "library_not_open" should be journaled for "library.rebuild"
...
26 features
370 scenarios (268 passed, 102 skipped)
1304 steps (1202 passed, 102 skipped)
```

`cargo test -p filesquid-api --locked`, `-p filesquid-native` and `-p filesquid-desktop` all passed. The head-commit hook run is recorded below.

## Sync Execution Report

- `apps/filesquid/crates/api/resources/srs.md` API-S-025: the rebuild job's one-per-library rule, its refusals before submission, its cursor checkpoint and its stored partial report. API-S-037: the result is answered by the job route and the control routes; the listing and the `job.update` event carry `null`.
- `apps/filesquid/crates/desktop/resources/srs.md` DE-S-014: each job row keeps its kind's result; the host installs its job service for the job routes.
- PRD, product SRS, ADRs: no change. The rebuild as a job is already in the PRD's F-001 and F-009 parents through API-S-025, and ADR 007's job model gains a stored field, not a new decision.

## Mocked Environment Variables

None.
