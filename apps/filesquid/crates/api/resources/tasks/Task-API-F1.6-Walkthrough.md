# Task API-F1.6 Walkthrough: Job and event routes

Branch: `feat/api-f1.6-job-event-routes` (global [Task FS-1.4](../../../../../../resources/implementation-plan.md#task-fs-1.4); [ADR 007](../../../../../../resources/architecture/007-durable-jobs-and-interactive-performance.md), [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md)). API contract `0.21.0`.

## API-S-036: Job routes and job events

Also PRD [NFR-003](../../../../../../resources/prd.md#nfr-003-bounded-processing-and-migration-endurance) and [SRS-NFR-003](../../../../../../resources/srs.md#srs-nfr-003-bounded-processing-and-migration-endurance) Bounded processing and migration endurance, [F-056](../../../../../../resources/prd.md#f-056-single-filesquid-api) and [SRS-F-056](../../../../../../resources/srs.md#srs-f-056-single-filesquid-api) Single FileSquid API, [CORE-S-006](../../../core/resources/srs.md) Job checkpoint protocol, and API-S-033 Durable jobs (DE-0.4's `Jobs` and `JobStore`, reused here).

## What changed

- **Controls** (`jobs::rules`, `jobs::model`): a job carries an optional requested control, `pause` or `cancel`. `pause` holds a pending job (`start` refuses it as `job_paused`) or asks a running worker to suspend at its next checkpoint, after which it stays paused with its checkpoint. `resume` clears a pause. `cancel` finishes a pending job at once, or asks a running worker to stop; a run that suspends, or whose lease expires at recovery, under a cancel request ends `cancelled`. A finished job refuses every control (`job_finished`), a job being cancelled refuses pause and resume (`cancel_requested`), and a running job with no pause refuses resume (`job_running`). The same control twice changes nothing the second time.
- **Worker seam** (`RunningJob::requested`): the request reaches a worker at every checkpoint and lease renewal. A worker that reads it suspends; `filesquid-native`'s history backfill worker now does, and stops between libraries.
- **Runners** (`jobs::JobRunner`, `ApiState::install_job_runner`): a host installs one runner per job kind. `POST /jobs/{job}/resume` hands a pending job to it, and the runner resumes from the checkpoint. A host with no runner for the kind leaves the job pending for the next launch; the answer's `scheduled` says which.
- **Caps** (`jobs::slots`): `Jobs::with_class` marks a kind as `Transfer` or `CpuHeavy`. `start` refuses `slots_full` (retryable, journaled, the job unchanged) while the class's cap of live leases is in use. Defaults come from `filesquid-core` (`DEFAULT_ACTIVE_TRANSFERS` 4, `DEFAULT_CPU_HEAVY_WORKERS` 2); an expired lease holds no slot. The check runs under one lock, so two starts cannot both take the last slot.
- **Listing** (`JobStore::list`, `JobFilter`, `JobList`): newest first, by kind and state, at most 100, marked `truncated`. `MemoryJobStore` and `filesquid-native`'s `SqliteJobs` (indexed by update time) implement it.
- **Routes** (`jobs::routes`): `GET /api/v1/jobs`, `GET /api/v1/jobs/{job}` (`jobs.read`), and `POST /api/v1/jobs/{job}/pause`, `/resume`, `/cancel` (`jobs.manage`). They answer `JobView`, `JobPage` and `JobControlAnswer`. Errors are typed: 404 `not_found`, 409 `conflict`, 400 `invalid_request`, 503 `unavailable` before the host installs its job service. The store runs on the blocking pool.
- **Events**: the new `job.update` topic carries the `JobView` for every stored change: submit, start, checkpoint, control, suspend, end and release at recovery. A lease renewal publishes nothing. The view carries no source identity, checkpoint, lease holder or fence.
- **Journal**: `job.paused`, `job.resumed`, `job.cancel_requested`, `job.cancelled`, and `job.refused` with the rule's error code. Each names the job ID and operation ID.
- **Persistence** (`filesquid-native`): `SqliteJobs` gains a `control` column, migrated into an existing table on open, and a `jobs_newest` index.
- **Contract**: 0.21.0, with `schema.d.ts` regenerated. ADR 030 records the `job.update` topic.

## Not delivered in this task, and why

- **No generic start route.** A job is created by the route of its kind, whose answer names the job. For example, DE-1.2's `POST /api/v1/imports` ([PR #115](https://github.com/arkrim-tech/filesquid/pull/115)) creates import jobs. The job routes report and control every job, and `resume` hands work back to the kind's runner. Starting a job without its kind's input has no meaning.
- **Import jobs.** DE-1.2's `ImportWorker` (not on main yet) has only a global stop flag. To be pausable and cancellable it has to read `RunningJob::requested()` at its checkpoints, as the backfill worker now does, and register an `import` runner with `install_job_runner`. That is DE-1.2's lane. With no runner, a resumed import stays pending until the next launch.
- **Host wiring.** The desktop does not install `Jobs`, its runners or the kind classes into `ApiState` yet (DE lane). Until it does, the routes answer 503 `unavailable` there.

## SRS traceability

| Requirement | Tests |
|---|---|
| API-S-036 an interrupted job resumes without repeating completed work | `integration_job_routes::an_interrupted_job_resumes_without_repeating_completed_work`; `integration_durable_jobs::a_paused_backfill_job_waits_until_it_is_resumed` (desktop); BDD "An interrupted job resumes without repeating completed work" |
| API-S-036 pause of a pending and a running job | `a_paused_pending_job_does_not_start_until_it_is_resumed`, `a_running_job_sees_a_pause_at_its_next_checkpoint_and_stays_paused`, `a_renewal_also_brings_the_request_to_the_worker`; BDD "A paused job that waits does not start until it is resumed" |
| API-S-036 cancel, including recovery under a cancel request | `cancelling_a_pending_job_finishes_it`, `a_running_job_under_a_cancel_request_is_cancelled_when_it_suspends`, `a_crashed_run_under_a_cancel_request_is_cancelled_at_recovery` |
| API-S-036 refusals, idempotence and journal | `controls_that_do_not_apply_are_refused_and_journaled`, `the_same_control_twice_changes_nothing_the_second_time`, `the_routes_list_answer_and_refuse_jobs`; BDD "A finished job refuses every control" |
| API-S-036 runner hand-off and no runner | `an_interrupted_job_resumes_without_repeating_completed_work` (`scheduled` true), `a_job_without_a_runner_stays_pending_for_the_next_launch` |
| API-S-036 caps (CORE-S-006 defaults) | `a_classified_kind_is_held_to_its_cap`, `an_expired_lease_holds_no_slot`, `the_default_caps_are_the_core_ones`; BDD "A classified kind is held to its cap" |
| API-S-036 listing | `listing_is_newest_first_filtered_and_bounded`; `unit_jobs::a_listing_is_newest_first_filtered_and_marked_truncated` (SQLite) |
| API-S-036 job view and `job.update` events | `every_stored_change_publishes_one_job_update_and_a_renewal_none`, `the_job_view_carries_no_source_checkpoint_or_lease`; `unit_contract` (payload schema, publisher drift); BDD "Every stored change of a job is announced on the event channel" |
| API-S-004 deny by default | `the_job_routes_need_a_credential`; `unit_auth` action map (`jobs.read`, `jobs.manage`) |
| Persistence of the control | `unit_jobs::a_requested_control_survives_closing_the_store`, `a_store_made_before_controls_gains_the_column_and_keeps_its_jobs` |

## TDD: failing run before implementation

`unit_job_control` and `integration_job_routes` built against `origin/main` (`8263ccb`), errors counted by kind:

```text
      1 error: could not compile `filesquid-api` (test "integration_job_routes") due to 1 previous error
      1 error: could not compile `filesquid-api` (test "unit_job_control") due to 33 previous errors
      1 error[E0425]: cannot find value `JOB_UPDATE` in module `topic`
      1 error[E0432]: unresolved imports `filesquid_api::jobs::JobClass`, `filesquid_api::jobs::JobControl`, `filesquid_api::jobs::JobFilter`, `filesquid_api::jobs::JobLimits`, `filesquid_api::jobs::JobView`, `filesquid_api::jobs::JOB_CANCEL_REQUESTED`, `filesquid_api::jobs::JOB_PAUSED`, `filesquid_api::jobs::JOB_RESUMED`
      1 error[E0432]: unresolved imports `filesquid_api::jobs::JobControl`, `filesquid_api::jobs::JobRunner`
      7 error[E0599]: no method named `cancel` found for struct `Jobs` in the current scope
      4 error[E0599]: no method named `job` found for struct `Jobs` in the current scope
      2 error[E0599]: no method named `list` found for struct `Jobs` in the current scope
      8 error[E0599]: no method named `pause` found for struct `Jobs` in the current scope
      3 error[E0599]: no method named `requested` found for struct `RunningJob` in the current scope
      4 error[E0599]: no method named `resume` found for struct `Jobs` in the current scope
      2 error[E0599]: no method named `with_class` found for struct `Jobs` in the current scope
      1 error[E0599]: no method named `with_events` found for struct `Jobs` in the current scope
```

## Mocked Environment Variables

None.

## Raw test output

`cargo test --locked -p filesquid-api --test unit_job_control --test integration_job_routes`, `-p filesquid-native --test unit_jobs` and `-p filesquid-desktop --test integration_durable_jobs`:

```text
     Running tests/integration/job_routes.rs (<target>/debug/deps/integration_job_routes-a31de03a55d97d1a)
test the_job_routes_need_a_credential ... ok
test a_job_without_a_runner_stays_pending_for_the_next_launch ... ok
test the_routes_list_answer_and_refuse_jobs ... ok
test an_interrupted_job_resumes_without_repeating_completed_work ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/job_control.rs (<target>/debug/deps/unit_job_control-9936981c436ff1d3)
test a_renewal_also_brings_the_request_to_the_worker ... ok
test a_classified_kind_is_held_to_its_cap ... ok
test a_crashed_run_under_a_cancel_request_is_cancelled_at_recovery ... ok
test a_paused_pending_job_does_not_start_until_it_is_resumed ... ok
test an_expired_lease_holds_no_slot ... ok
test a_running_job_sees_a_pause_at_its_next_checkpoint_and_stays_paused ... ok
test cancelling_a_pending_job_finishes_it ... ok
test controls_that_do_not_apply_are_refused_and_journaled ... ok
test a_running_job_under_a_cancel_request_is_cancelled_when_it_suspends ... ok
test every_stored_change_publishes_one_job_update_and_a_renewal_none ... ok
test listing_is_newest_first_filtered_and_bounded ... ok
test the_default_caps_are_the_core_ones ... ok
test the_job_view_carries_no_source_checkpoint_or_lease ... ok
test the_same_control_twice_changes_nothing_the_second_time ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/jobs.rs (<target>/debug/deps/unit_jobs-454b471a9d3049ca)
test a_count_past_the_signed_range_is_refused ... ok
test a_refused_update_stores_nothing_and_a_known_id_is_not_inserted_twice ... ok
test a_job_and_its_checkpoint_survive_closing_the_store ... ok
test a_listing_is_newest_first_filtered_and_marked_truncated ... ok
test a_requested_control_survives_closing_the_store ... ok
test an_update_that_changes_the_id_stores_nothing ... ok
test the_job_store_shares_the_owner_only_operational_database ... ok
test a_store_made_before_controls_gains_the_column_and_keeps_its_jobs ... ok
test unfinished_jobs_are_listed_oldest_first_and_bounded ... ok
test two_connections_inserting_one_job_id_store_it_once ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test a_paused_backfill_job_waits_until_it_is_resumed ... ok
test a_job_for_another_installation_is_failed_and_a_new_one_runs ... ok
test quitting_suspends_the_backfill_job_with_its_checkpoint_kept ... ok
test a_backfill_job_completes_and_records_both_libraries_in_its_checkpoint ... ok
test a_run_left_by_a_crash_is_waited_out_and_resumed ... ok
test the_next_launch_resumes_the_job_without_rescanning_completed_libraries ... ok
test restart_recovery_reads_only_jobs_and_stays_within_the_budget ... ok
test an_unreadable_checkpoint_visits_every_library_again ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.19s
```

`cargo test --locked -p filesquid-api --test features -- --name "job|jobs|cap|finished job|announced"` (job feature excerpt and summary):

```text
Feature: Durable jobs are reported and controlled through the job routes
  Scenario: An interrupted job resumes without repeating completed work
   ✔> Given a host with a job service and a counting worker
   ✔  Given a counting job of 6 units that has done 3
   ✔  When I pause the job through the job routes
   ✔  And the worker reaches its next checkpoint
   ✔  Then the job should be paused with 4 units done
   ✔  When I resume the job through the job routes
   ✔  Then the job should complete
   ✔  And every unit should have been done exactly once
  Scenario: A paused job that waits does not start until it is resumed
   ✔> Given a host with a job service and a counting worker
   ✔  Given a counting job of 6 units that waits
   ✔  When I pause the job through the job routes
   ✔  Then the worker should not be able to start it
   ✔  When I resume the job through the job routes
   ✔  Then the job should complete
  Scenario: A classified kind is held to its cap
   ✔> Given a host with a job service and a counting worker
   ✔  Given transfers are capped at 1
   ✔  And two waiting transfer jobs
   ✔  When the first transfer starts
   ✔  Then the second transfer should be refused as "slots_full"
   ✔  And the second transfer should still wait
  Scenario: Every stored change of a job is announced on the event channel
   ✔> Given a host with a job service and a counting worker
   ✔  Given a counting job of 6 units that waits
   ✔  When I pause the job through the job routes
   ✔  Then a "job.update" event should show the job paused
   ✔  And no event should carry the job's source
  Scenario: A finished job refuses every control
   ✔> Given a host with a job service and a counting worker
   ✔  Given a counting job of 6 units that waits
   ✔  When I cancel the job through the job routes
   ✔  Then the job should be cancelled
   ✔  And pausing, resuming and cancelling it again should each be refused as a conflict
[Summary]
5 features
10 scenarios (10 passed)
52 steps (52 passed)
```

Full api feature run: `24 features, 349 scenarios (247 passed, 102 skipped), 1212 steps (1110 passed, 102 skipped)`. The skipped scenarios are the pre-existing ones that wait on later tasks.
