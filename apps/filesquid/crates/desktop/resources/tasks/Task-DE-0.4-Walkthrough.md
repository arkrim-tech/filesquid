# Task DE-0.4 Walkthrough: Durable jobs and checkpoint-on-quit

Branch: `feat/de-0.4-durable-jobs` (global [Task 0.5](../../../../../../resources/implementation-plan.md#task-0.5); [ADR 007](../../../../../../resources/architecture/007-durable-jobs-and-interactive-performance.md)).

## Requirements

- **NFR-003 Bounded processing and migration endurance**: restart reaches a resumable job state within 30 s without rescanning completed media; queues have finite limits.
- **DE-S-014 Durable jobs and checkpoint-on-quit** (new): jobs in the SQLite operational store; launch-time reconciliation from the job rows alone; quitting or closing the last window checkpoints and suspends a running job; a crashed run's lease is waited out, bounded, then resumed; the history backfill runs as such a job.
- **API-S-033 Durable job model** (new): one job model in `filesquid-api` over a host-injected store; idempotent operation ID; opaque source identity, revalidated on start; checkpoints of at most 64 KiB; an exclusive lease with a fencing number, refusing writes from an expired or replaced worker (CORE-S-006); bounded recovery; registry process; every state change and refusal journaled with IDs and an error code only.
- **CORE-S-006** (existing): an expired lease MUST NOT commit after replacement. `RESTART_RESUMABLE_WITHIN_SECS` (30) is used by the restart tests.

## What changed

- `filesquid-api::jobs` (new):
  - `Job`, `JobSpec`, `JobState`, `Lease` and `JobError`.
  - The pure transitions in `jobs::rules`: `acquire`, `checkpoint`, `suspend`, `finish` and `expire`.
  - The `JobStore` trait, whose stores do only `insert`, `get`, one atomic `update` and a bounded `unfinished` list, and the reference store `MemoryJobStore`.
  - The `Jobs` service with `submit`, `recover` and `start`, which revalidates the source and fails the job with `source_changed` when it differs, and `RunningJob` with `checkpoint`, `complete`, `fail`, `cancel` and `suspend`.
  - Journal operations: `job.created`, `job.started` (`started`, or `resumed` when the job has a checkpoint), `job.suspended`, `job.completed`, `job.failed`, `job.cancelled`, `job.recovered` and `job.refused`. Each entry carries the job ID as its target, the operation ID, the process ID and an error code. The source identity and the checkpoint are never journaled.
  - Bounds: lease 15 s (`JOB_LEASE_TTL_MS`), checkpoint 64 KiB (`MAX_CHECKPOINT_BYTES`), recovery reads at most 1,000 jobs (`MAX_RECOVERED_JOBS`).
- `filesquid-native::SqliteJobs` (new): a `jobs` table in `<data dir>/filesquid.sqlite` (owner-only, WAL, opened without following a link). Each update is a `BEGIN IMMEDIATE` transaction, and the store holds no rules.
- `filesquid-native` catalog: `SqliteCatalog::library_ids` and a public `SqliteCatalog::backfill_library`, so a job can checkpoint per library.
- `filesquid-desktop::durable_jobs` (new):
  - `open_jobs` opens the store with a holder named for this launch.
  - `recover_jobs` is the bounded reconciliation at launch.
  - `run_backfill_job` resumes the unfinished backfill job, or submits one. It revalidates the source (the installation), and a job of another installation is failed and replaced. It waits out a crashed run's lease, bounded by the lease length. It skips the libraries the checkpoint lists, checkpoints after each completed library, and on `stop` writes its checkpoint and suspends. A library that could not run also leaves the job suspended, so the next launch retries it.
- `filesquid-desktop` launch (`start_monitor`): opens the job store and recovers jobs right after the catalogs, keeps `DesktopJobs` in Tauri's managed state, and runs the backfill thread through `run_backfill_job`. App exit, which Tauri raises when the last window closes, sets the stop flag, and the job writes its checkpoint and suspends before the journal is flushed (waited at most 10 s, as before).

## SRS traceability

| Requirement | Tests |
|---|---|
| API-S-033 operation ID, journal and redaction | api `unit_jobs::a_submitted_job_is_pending_with_its_operation_id_and_journaled`, `job_journal_entries_carry_no_checkpoint_or_source_text` |
| API-S-033, CORE-S-006 exclusive lease and fencing | api `unit_jobs::a_lease_is_exclusive_until_it_expires`, `an_expired_worker_cannot_checkpoint_or_finish_after_a_takeover`; native feature "An expired worker cannot write after another worker took the job over" |
| API-S-033 source revalidation | api `unit_jobs::a_resume_whose_source_changed_fails_the_job_and_reuses_nothing`; native feature "A job whose source changed is not resumed"; desktop `integration_durable_jobs::a_job_for_another_installation_is_failed_and_a_new_one_runs` |
| API-S-033, DE-S-014 checkpoint-on-quit | api `unit_jobs::suspending_at_quit_keeps_the_checkpoint_and_the_next_launch_resumes_it`; native feature "A job suspended at quit resumes from its checkpoint after a restart"; desktop `integration_durable_jobs::quitting_suspends_the_backfill_job_with_its_checkpoint_kept` |
| NFR-003, DE-S-014 resume within 30 s without rescanning completed media | desktop `integration_durable_jobs::the_next_launch_resumes_the_job_without_rescanning_completed_libraries` (the completed library keeps its missing record, so it was not visited), `restart_recovery_reads_only_jobs_and_stays_within_the_budget` (500 jobs), `a_run_left_by_a_crash_is_waited_out_and_resumed` |
| API-S-033 crashed-run recovery and bounds | api `unit_jobs::recovery_releases_expired_leases_and_lists_every_unfinished_job_oldest_first`, `recovery_reads_a_bounded_number_of_jobs`, `a_checkpoint_past_its_size_cap_is_refused_and_the_old_one_kept`; native feature "A crashed run's lease is released once it expires" |
| API-S-033 registry process | api `unit_jobs::a_running_job_is_a_registered_process_with_progress` |
| DE-S-014 SQLite persistence | native `unit_jobs` (4: survives reopen, refused update stores nothing, oldest-first bounded list, owner-only file) |
| DE-S-014 backfill as a durable job | desktop `integration_durable_jobs::a_backfill_job_completes_and_records_both_libraries_in_its_checkpoint`; `integration_history_wiring` (5) unchanged |

## TDD: failing runs

```text
$ cargo test -p filesquid-api --test unit_jobs --locked
error[E0432]: unresolved import `filesquid_api::jobs`
error: could not compile `filesquid-api` (test "unit_jobs") due to 1 previous error
$ cargo test -p filesquid-desktop --test integration_durable_jobs --locked
error[E0432]: unresolved import `filesquid_desktop_lib::durable_jobs`
error: could not compile `filesquid-desktop` (test "integration_durable_jobs") due to 1 previous error
```

`quitting_suspends_the_backfill_job_with_its_checkpoint_kept` then failed on the first implementation: a quit before any library had completed left no checkpoint. The quit path now writes its checkpoint before it suspends. The native `unit_jobs` tests were written before `SqliteJobs` existed, but that compile failure was not captured.

## Known gaps

- **Per-resource attempt caps** (5 attempts, 60 s backoff, NFR-003) apply to the resources of import and upload (DE-1.2). A job's `attempts` counts its leases, and a suspended job is resumed at every launch without quarantine.
- **Job routes** (start, pause, resume, cancel over the API) are Task API-F1.6. It can build on `Jobs` and `JobStore`.
- **Keep-running setting.** ADR 007 makes keep-running-after-the-last-window a product setting; until it exists, the default (checkpoint and resume on next launch) applies.
- **Permissions on resume.** The desktop owner is the only principal, so resume revalidates the source identity only. Server accounts (API-F1.9) add a permission check.

## Mocked Environment Variables

None.

## Raw test output

```text
$ cargo test -p filesquid-api --test unit_jobs --locked
running 10 tests
test a_checkpoint_past_its_size_cap_is_refused_and_the_old_one_kept ... ok
test a_running_job_is_a_registered_process_with_progress ... ok
test a_resume_whose_source_changed_fails_the_job_and_reuses_nothing ... ok
test a_lease_is_exclusive_until_it_expires ... ok
test a_submitted_job_is_pending_with_its_operation_id_and_journaled ... ok
test an_expired_worker_cannot_checkpoint_or_finish_after_a_takeover ... ok
test suspending_at_quit_keeps_the_checkpoint_and_the_next_launch_resumes_it ... ok
test job_journal_entries_carry_no_checkpoint_or_source_text ... ok
test recovery_releases_expired_leases_and_lists_every_unfinished_job_oldest_first ... ok
test recovery_reads_a_bounded_number_of_jobs ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
$ cargo test -p filesquid-native --test unit_jobs --locked
running 4 tests
test the_job_store_shares_the_owner_only_operational_database ... ok
test a_refused_update_stores_nothing_and_a_known_id_is_not_inserted_twice ... ok
test unfinished_jobs_are_listed_oldest_first_and_bounded ... ok
test a_job_and_its_checkpoint_survive_closing_the_store ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
$ cargo test -p filesquid-desktop --test integration_durable_jobs --locked
running 6 tests
test quitting_suspends_the_backfill_job_with_its_checkpoint_kept ... ok
test a_job_for_another_installation_is_failed_and_a_new_one_runs ... ok
test a_backfill_job_completes_and_records_both_libraries_in_its_checkpoint ... ok
test the_next_launch_resumes_the_job_without_rescanning_completed_libraries ... ok
test restart_recovery_reads_only_jobs_and_stays_within_the_budget ... ok
test a_run_left_by_a_crash_is_waited_out_and_resumed ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s
$ cargo test -p filesquid-desktop --test integration_history_wiring --locked
running 5 tests
test one_unreadable_library_leaves_the_others_history_served ... ok
test a_cancelled_backfill_commits_nothing_and_leaves_the_rest_for_the_next_launch ... ok
test a_repair_commits_its_provenance_follow_up_under_the_repair_run_id ... ok
test the_history_routes_answer_from_a_real_desktop_library ... ok
test the_backfill_fills_the_history_of_versions_committed_without_records ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
$ cargo test -p filesquid-native --test features --locked
Feature: Durable jobs in the host's operational store
  Scenario: A job suspended at quit resumes from its checkpoint after a restart
  Scenario: A crashed run's lease is released once it expires
  Scenario: An expired worker cannot write after another worker took the job over
  Scenario: A job whose source changed is not resumed
Feature: Native journal store and data-directory checks
  Scenario: The SQLite journal store writes no log file
  Scenario: A host monitor opens its store in the data directory
  Scenario: A data directory that cannot be written fails health
7 scenarios (7 passed)
29 steps (29 passed)
```

The head-commit hook run is recorded in the pull request.
