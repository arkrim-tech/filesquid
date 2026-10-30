# Task API-F2.16 Walkthrough: Upload review step and per-file job progress

Branch `feat/api-f2.16-upload-review`, module `filesquid-api` ([plan row](../plan.md)); the desktop and the server compile unchanged. Part of global [Task 1.2](../../../../../../resources/implementation-plan.md#task-1.2). Contract 0.31.0 (main was 0.30.0); `ui/src/api/schema.d.ts` and `contract-version.ts` regenerated with `npm run contract:types`. Owner decisions applied: answer 5 of 2026-10-22, verbatim: "Keep the review"; answer 6, verbatim: "Yes, report progress per file"; decision 2 of 2026-10-23, verbatim: "Endorsed. The job controls of API-S-037 (pause, resume, cancel, retry of one item, retry of all failed items) take no CommandEnvelope: each names the state it asks for, takes no body, answers the job view, and repeating it changes nothing, so it needs no operation ID of its own."

## What was built

- **Review** (`src/imports/`): `POST /api/v1/imports` answers `{outcome: pending, import: planned, plan_id, plan}` before any byte is staged, with every field the owner reviews in mockup `18-import-review.svg` (files found, new and already in this library, bytes to copy, the destination, the names to be numbered, free bytes after the upload, the staging limit, the source kind and volume label, the expiry); `GET /api/v1/imports/{plan_id}/items` pages the planned files by base name (at most 1,000, cursor); `POST /api/v1/imports/{plan_id}/start` starts the job once (a repeated start answers the same job; a changed source is `409 source_changed` and the plan stays open); `DELETE /api/v1/imports/{plan_id}` discards the plan, as the host does after `PLAN_TTL_MS` (1 hour). A repeated operation ID answers its open plan, its started job or `cancelled` without a dialog, and `404 plan_not_found` once the plan expired or was discarded. Journaled once each as `import.plan`, `import.start` and `import.discard` (a refused items read as `import.plan_items`), never a path or a name.
- **Host port**: `ImportStarter::plan`, `start_plan`, `discard` and `plan_items`, each with a default, so a host that implements only `start` (the desktop until Task DE-1.8) starts the job at once and answers `started` as before; `PlanBook`, the in-memory book of plans (the replay rule, the expiry, the items paging, a cap of 256).
- **Per-file progress** (`src/jobs/items.rs`): `JobView.destination`; `GET /api/v1/jobs/{job}/items` (`jobs.read`) with display name, destination folder, state, bytes and error code; `POST /api/v1/jobs/{job}/items/{item}/retry` and `POST /api/v1/jobs/{job}/retry-failed` (`jobs.manage`) over the `JobItems` port, `job.item_retried` once per file put back, `409 job_paused` for a paused job, nothing handed to the runner when nothing changed; `MemoryJobItems`; `Jobs::with_items`, `view`, `item_page` and `retry`.
- **Server**: no starter and no per-file port, so the review and items routes answer `503 unavailable`.
- **Dependency line with DE-1.8** (recorded in both plan rows): this task defines the routes, the ports and the reference implementations; DE-1.8 implements the plan after the native dialog and the per-file progress over the native import plans and worker.

## Tests

| Requirement | Tests |
|---|---|
| [API-S-036](../srs.md) Import route — Review: the plan before any byte is staged, the plan items by base name (1,000 per page), the start once with a repeated start answering the same job, discard by `DELETE` or after 1 hour changing nothing, `source_changed`, the replay of an open plan, a started job or a cancelled dialog, `plan_not_found`, the journal (`import.plan`, `import.start`, `import.discard`) with IDs only, no path | `unit_import_review` (9), `unit_imports_route` (10, the one-step host), `features` (`upload_review_and_per_file_progress_API-S-036.feature`, 10 scenarios), `unit_contract` |
| [API-S-037](../srs.md) Job routes and job events — Per-file progress: the destination in the job view, the items page with name, folder, state, bytes and error code and never a path, retry of one or all failed files back to `queued` handed to the runner, `job_paused`, a no-op repeat, `job.item_retried` once per file, `jobs.read` for the items route | `unit_job_items` (10), `features`, `unit_contract` |
| [API-S-002](../srs.md) Versioned contract and generated types | `unit_contract` (28), the UI's contract drift test |
| [API-S-004](../srs.md), [API-S-017](../srs.md) Each route declares its action; every refusal journaled once | `unit_auth` (35), the refusal tests above |
| [API-S-014](../srs.md) No version deletion | the retention guard exempts the content-free plan discard (`tests/support/retention_contract.rs`) |

Written first: the three unit targets did not compile against the old module (no `PlanBook`, no `JobItems`), then passed once the module and the routes existed.

## Test output

`cargo test -p filesquid-api --locked` (every target; the import, job and contract ones shown) and the desktop's import tests, run from `apps/filesquid/` on the shared target:

```text
Running tests/unit/import_review.rs (<target>/debug/deps/unit_import_review-296ff14dcd239170)

running 9 tests
test a_picked_source_answers_its_plan_before_any_byte_is_staged ... ok
test a_changed_source_is_refused_as_source_changed_and_the_plan_stays_open ... ok
test a_host_without_a_starter_answers_unavailable_on_every_review_route ... ok
test a_plan_expires_after_one_hour ... ok
test the_plan_book_forgets_the_oldest_plan_at_its_cap ... ok
test delete_discards_the_plan_and_changes_nothing ... ok
test a_repeated_operation_answers_its_plan_its_job_or_cancelled_without_a_dialog ... ok
test start_starts_the_job_once_and_a_repeated_start_answers_the_same_job ... ok
test the_plan_items_route_pages_the_planned_files_by_base_name ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

```text
Running tests/unit/imports_route.rs (<target>/debug/deps/unit_imports_route-fd59050a7c717ccc)

running 10 tests
test a_catalog_failure_a_retry_cannot_fix_is_internal_not_unavailable ... ok
test a_cancelled_dialog_answers_cancelled_without_a_job ... ok
test a_repeated_operation_answers_its_job_before_the_destination_is_checked_again ... ok
test a_started_import_answers_its_job_and_journals_it_once ... ok
test an_import_without_a_destination_is_refused_before_any_dialog ... ok
test a_destination_folder_outside_the_path_rule_is_refused_before_any_dialog ... ok
test an_unknown_or_foreign_collection_is_not_found_before_any_dialog ... ok
test refusals_are_typed_and_journaled_once_with_their_code ... ok
test the_destination_reaches_the_host ... ok
test a_wrong_scope_no_starter_or_no_token_asks_nothing ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

```text
Running tests/unit/job_items.rs (<target>/debug/deps/unit_job_items-4703f4e9cfa29020)

running 10 tests
test a_paused_job_refuses_a_retry_with_job_paused_and_nothing_changes ... ok
test a_host_without_the_per_file_port_answers_unavailable ... ok
test a_retry_of_one_failed_file_queues_it_and_hands_the_pending_job_to_its_runner ... ok
test a_file_that_is_not_failed_an_unknown_item_and_a_finished_job_are_handled_as_the_rule_says ... ok
test a_running_job_s_files_are_queued_without_handing_the_job_over ... ok
test retry_failed_queues_every_failed_file_and_leaves_the_others ... ok
test the_reference_port_pages_and_requeues_as_the_rule_says ... ok
test the_items_route_needs_jobs_read ... ok
test an_upload_job_s_view_and_listing_carry_its_destination ... ok
test the_items_route_pages_the_files_with_their_state_and_never_a_path ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

```text
Running tests/unit/contract.rs (<target>/debug/deps/unit_contract-27eeb267071a4e20)

running 28 tests
test error_code_all_lists_every_variant ... ok
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test event_topics_in_the_contract_match_the_rust_topics ... ok
test errors_carry_code_and_retryability ... ok
test every_component_schema_has_a_rust_sample ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
test every_documented_operation_has_a_unique_operation_id ... ok
test every_error_response_carries_the_api_error_schema ... ok
test every_event_topic_documents_its_payload_schema ... ok
test every_limit_parameter_is_the_shared_page_size ... ok
test every_operation_documents_both_denials ... ok
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
{"error":"journal writer stopped","level":"ERROR","message":"journal write failed"}
{"actor":"system","host":"","journal_write_failed":true,"level":"info","operation":"process.cancelled","outcome":"cancelled","process_id":"import-1","target_ids":[],"time":"2026-10-30T01:32:22.767Z"}
test events_from_every_publisher_match_their_documented_payloads ... ok
test every_documented_operation_answers_a_documented_status_and_body ... ok
test router_fallbacks_answer_as_documented ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.09s
```

```text
Feature: Upload review step and per-file job progress
  Scenario: A picked folder is planned before any byte is staged
   ✔> Given a desktop host whose import dialog plans what the owner picks
   ✔  When the owner asks to upload a folder into a collection
   ✔  Then the review should show 3 files found, 2 new and 1 already in this library
   ✔  And the review should name the destination, the numbered names, the free bytes after the upload and the staging limit
   ✔  And no job should have started
   ✔  And the plan request should be journaled once with the plan and no file name
  Scenario: The planned files are listed by base name only
   ✔> Given a desktop host whose import dialog plans what the owner picks
   ✔  Given the owner has asked to upload a folder into a collection
   ✔  When the owner reads the planned files 2 at a time
   ✔  Then the files should be listed by base name with their size and whether they are new
   ✔  And no path should appear in any answer
  Scenario: Confirming the plan starts one job and a second confirmation answers the same job
   ✔> Given a desktop host whose import dialog plans what the owner picks
   ✔  Given the owner has asked to upload a folder into a collection
   ✔  When the owner confirms the upload twice
   ✔  Then one import job should have started and both answers should name it
   ✔  And each start should be journaled once with the job
  Scenario: A repeated request answers the open plan without a second dialog
   ✔> Given a desktop host whose import dialog plans what the owner picks
   ✔  Given the owner has asked to upload a folder into a collection
   ✔  When the same request is sent again
   ✔  Then the open plan should be answered again
   ✔  And the dialog should have been shown once
  Scenario: The owner discards the plan and nothing changes
   ✔> Given a desktop host whose import dialog plans what the owner picks
   ✔  Given the owner has asked to upload a folder into a collection
   ✔  When the owner discards the plan
   ✔  Then the plan should be gone and no job should have started
   ✔  And the discard should be journaled once with the plan
  Scenario: A plan that nobody confirms expires after an hour
   ✔> Given a desktop host whose import dialog plans what the owner picks
   ✔  Given the owner has asked to upload a folder into a collection
   ✔  When an hour passes
   ✔  Then the plan should be gone and no job should have started
  Scenario: A source that changed since the plan is refused and the plan stays open
   ✔> Given a desktop host whose import dialog plans what the owner picks
   ✔  Given the owner has asked to upload a folder into a collection
   ✔  And the picked files changed since the plan
   ✔  When the owner confirms the upload
   ✔  Then the upload should be refused as "source_changed"
   ✔  And the plan should still be open
  Scenario: An upload job shows its destination and every file's state
   ✔> Given a desktop host whose import dialog plans what the owner picks
   ✔  Given an upload job with a copied, a copying and a failed file
   ✔  When the owner reads the job and its files
   ✔  Then the job should name its destination collection and folder
   ✔  And the files should show their name, folder, state and bytes and never a path
  Scenario: Retrying the failed file queues it again and hands the job to its runner
   ✔> Given a desktop host whose import dialog plans what the owner picks
   ✔  Given an upload job with a copied, a copying and a failed file
   ✔  When the owner retries the failed file
   ✔  Then the file should be queued again and the job handed to its runner
   ✔  And the retry should be journaled once for that file without its name
   ✔  When the owner retries the failed file
   ✔  Then nothing should change and nothing should be handed to the runner
  Scenario: A paused job refuses a retry and nothing changes
   ✔> Given a desktop host whose import dialog plans what the owner picks
   ✔  Given an upload job with a copied, a copying and a failed file
   ✔  And the job is paused
   ✔  When the owner retries all failed files
   ✔  Then the retry should be refused as "job_paused" and the file should stay failed

[Summary]
29 features
398 scenarios (296 passed, 102 skipped)
1430 steps (1328 passed, 102 skipped)
```

```text
Running tests/integration/imports.rs (<target>/debug/deps/integration_imports-6f68d64e6d24b2ce)

running 6 tests
test a_cancelled_dialog_or_an_unopened_library_makes_no_job ... ok
test an_import_without_a_destination_is_refused_before_any_dialog ... ok
test a_picked_folder_is_imported_as_one_job_and_a_repeat_answers_the_same_job ... ok
test an_operation_id_reused_for_another_import_is_refused_not_answered_with_its_job ... ok
test imported_files_appear_under_their_collection_and_folder ... ok
test a_job_the_stopped_worker_could_not_take_runs_at_the_next_launch ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s

     Running tests/integration/library_routes.rs (<target>/debug/deps/integration_library_routes-be26db170b910b92)

running 11 tests
test a_cancelled_or_unusable_pick_registers_nothing ... ok
test create_in_a_chosen_folder_answers_an_opaque_library_and_serves_its_files ... ok
test a_named_library_answers_its_facts_and_is_listed_as_recent_without_its_folder ... ok
test a_library_folder_copied_elsewhere_reopens_with_its_collections ... ok
test the_data_directory_keeps_only_each_library_s_index_never_its_files ... ok
test an_opened_folder_feeds_the_history_and_copies_and_rebuilds_without_paths ... ok
test the_navigation_lists_the_installations_repository_and_its_libraries ... ok
test the_owners_preferences_are_kept_in_the_operational_store ... ok
test the_owner_picks_a_folder_in_the_hosts_dialog_and_the_app_sees_only_its_id ... ok
test the_backfill_route_runs_over_the_folders_the_owner_opened ... ok
test the_owners_consented_repair_is_kept_in_the_persistent_ledger ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
```

`unit_auth` (35 passed) after its route table learned the new paths (see below).

The head-commit hook run (`FULL=1 bash scripts/pre-commit.sh`, `Test scope: full`) is recorded in the pull request.

## Cubic review, round 1

Fixed on the pull request (15 findings):

- `ImportStarter::replay` has no default: every host answers a repeated operation ID itself, the one-step host included, so a retried request never opens a second dialog or submits a second job.
- The plan route claims the request's operation ID (`InFlight`, `src/imports/flight.rs`) before it asks the replay and keeps the claim until the host answered; a second request of the same ID meanwhile is `409 dialog_open` (API-S-036 **Refusals**).
- `PlanBook::start` replaces `starting`/`started`/`StartDecision`: the open-to-started step and the job submission run under the book's lock, so two starts of one plan submit one job and a discarded plan is never started; a failed submission leaves the plan open.
- `PlanBook` forgets the oldest cancelled operation first (a `VecDeque` of the order), and re-cancelling a known operation forgets no other.
- Job item names and plan item names are cut to their base name (`jobs::display_name`) before the routes answer them, whatever the host port hands over (API-S-005).
- The job routes build every job view, with its destination read from the per-file port, on a blocking thread.
- `retry_allowed` refuses any job under a pause request, running ones included (`job_paused`); the retry and items routes check the per-file port before the job, so a host without one answers `503` for every job, journaled as `job.refused` (`unavailable`). API-S-037 states both.
- The feature gained scenarios for the 1,000-file page cap, the `503` of a host without the ports, the repeated operation ID answering the started job and `cancelled`, and a successful retry of all failed files.

Two findings asked the plan start and discard routes to take a `CommandEnvelope`. The owner decided on 2026-10-30 to extend the API-S-002 exemption to both routes instead, because the plan ID is the idempotency key and the plan request's own envelope carries the operation ID. API-S-002, API-S-036, the contract descriptions (the version stays 0.31.0) and the plan row now say so, and the feature gained "A discarded plan discarded again is plan_not_found and changes nothing"; "Confirming the plan starts one job and a second confirmation answers the same job" already covers a repeated start.

```text
     Running tests/unit/import_review.rs (<target>/debug/deps/unit_import_review-e68ab1f14e83db26)
test a_picked_source_answers_its_plan_before_any_byte_is_staged ... ok
test a_changed_source_is_refused_as_source_changed_and_the_plan_stays_open ... ok
test a_host_without_a_starter_answers_unavailable_on_every_review_route ... ok
test a_second_request_of_an_operation_in_flight_is_dialog_open_and_opens_no_dialog ... ok
test a_plan_expires_after_one_hour ... ok
test the_plan_book_forgets_the_oldest_cancelled_operation_first ... ok
test a_repeated_operation_answers_its_plan_its_job_or_cancelled_without_a_dialog ... ok
test the_plan_book_submits_one_job_per_plan_and_never_restarts_a_discarded_plan ... ok
test the_plan_book_forgets_the_oldest_plan_at_its_cap ... ok
test delete_discards_the_plan_and_changes_nothing ... ok
test start_starts_the_job_once_and_a_repeated_start_answers_the_same_job ... ok
test the_plan_items_route_pages_the_planned_files_by_base_name ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/unit/job_items.rs (<target>/debug/deps/unit_job_items-666704a69762fee7)
test a_paused_job_refuses_a_retry_with_job_paused_and_nothing_changes ... ok
test a_name_a_host_hands_over_with_a_folder_is_cut_to_its_base_name ... ok
test a_running_job_asked_to_pause_refuses_a_retry_with_job_paused ... ok
test a_running_job_s_files_are_queued_without_handing_the_job_over ... ok
test a_file_that_is_not_failed_an_unknown_item_and_a_finished_job_are_handled_as_the_rule_says ... ok
test a_retry_of_one_failed_file_queues_it_and_hands_the_pending_job_to_its_runner ... ok
test a_host_without_the_per_file_port_answers_unavailable ... ok
test an_upload_job_s_view_and_listing_carry_its_destination ... ok
test the_reference_port_pages_and_requeues_as_the_rule_says ... ok
test the_items_route_needs_jobs_read ... ok
test retry_failed_queues_every_failed_file_and_leaves_the_others ... ok
test the_items_route_pages_the_files_with_their_state_and_never_a_path ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
403 scenarios (301 passed, 102 skipped)
1456 steps (1354 passed, 102 skipped)
```

## Sync Execution Report

- `apps/filesquid/crates/api/resources/plan.md`: API-F2.16 ticked, the Status with the routes, the ports and the dependency line with DE-1.8.
- `apps/filesquid/crates/desktop/resources/plan.md`: DE-1.8 Status with the dependency line (the desktop's one-step start until it implements the review and the per-file port).
- `apps/filesquid/crates/desktop/resources/tasks/Task-DE-1.2-Walkthrough.md`: link and link label of the moved import module.
- `apps/filesquid/crates/api/resources/srs.md`: API-S-036 (`dialog_open` for an operation ID in flight) and API-S-037 (`job_paused` for any pause request; `503` before the job state; base names only).
- `resources/implementation-plan-checklist.md` and `resources/assets/mvp-progress.svg`: API-F2.16 ticked, progress block regenerated.

## Mocked Environment Variables

None. The tests use in-memory stores, a scripted dialog and a clock the tests advance.
