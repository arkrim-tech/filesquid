# Task API-F2.17 Plan: Rebuild from files as a durable job

Branch `feat/api-f2.17-rebuild-job`. Task row: [plan.md](../plan.md) ("Task API-F2.17"). Mockup: [04-recover-library.svg](../../../../../../resources/ui-mockups/v3/04-recover-library.svg) ([UI-S-027](../../../../ui/resources/srs.md)).

## Binding requirements

- **API-S-025 Rebuild as a job** (owner answer 9, 2026-10-22, verbatim: "Yes, rebuild should be a durable job"): the rebuild from files MUST run as a durable job (API-S-033) of kind `library.rebuild`, answered `pending` with its job ID, reporting progress in objects (`done`, `total`), pausable, resumable from its checkpoint and cancellable through API-S-037, with its report (objects read, not readable, and each object with validation errors) as the job's result.
- API-S-025 (unchanged parts that still bind the route): authorised as `library.manage` before the handler runs; calls the application layer's one implementation, no file or SQL access of its own; answers carry opaque library and object IDs, never a path, storage key or OCFL content path; every refusal journaled exactly once (authorisation as `api.auth.denied`, others with their error code); route, action, schemas and error codes in the contract, generated TypeScript types match.
- **API-S-037**: a job view MUST carry, once it has finished, the `result` its kind defines (for `library.rebuild`, the API-S-025 report; `null` for a kind without one), and MUST NOT carry its source identity, checkpoint, lease holder or fencing number. Pause holds a pending job and asks a running one to suspend at its next checkpoint, keeping it; resume clears a pause and hands the job to the runner of its kind, which resumes it from its checkpoint without repeating completed work, or leaves it pending when the host has none (`scheduled`); cancel finishes a pending job (or one whose lease ended) as cancelled and asks a running one to stop at its next checkpoint. A worker that stops for a request checks that it still stands in the same atomic step as its suspension. Controls take no body and no `CommandEnvelope` (owner decision 2026-10-23, endorsing API-S-037's exemption) and are journaled under the job's own operation ID: `job.paused`, `job.cancel_requested`, `job.cancelled`, `job.resumed`, each once; a repeated pause or cancel that changes nothing is not journaled; refusals `job.refused` once with `job_finished`, `cancel_requested`, `job_running`, `not_found`. Every stored change publishes one `job.update` with the job view.
- **API-S-033**: idempotent operation ID; opaque source identity, stored, never journaled; checkpoint at most 64 KiB; exclusive lease 15 s by default with a fencing number, renewed by each checkpoint or renewal and never moved earlier; a write under an ended or taken-over lease refused `fenced`; start revalidates the source identity; a crashed run's expired lease is released at launch-time recovery (at most 1,000 unfinished jobs, never the media); a running job is a process in the registry; every state change and refusal journaled with the job ID and operation ID only, failures with an error code at error level.

## Constraints and gates

| # | Constraint | Gate |
|---|---|---|
| C1 | `POST /api/v1/libraries/rebuilds` answers `{outcome: pending, job_id, job, scheduled}`: a `library.rebuild` job, pending, handed to the host's runner | integration `the_rebuild_answers_pending_with_a_library_rebuild_job` |
| C2 | A second rebuild of the same library while its job is unfinished answers that job; no second job | unit `a_repeated_rebuild_answers_the_unfinished_job` |
| C3 | Progress counts objects: `total` is the number of objects, `done` the objects read so far | unit `progress_counts_objects` |
| C4 | Pause holds the job at a checkpoint (state pending, paused, checkpoint kept) | unit `pause_holds_the_rebuild_at_a_checkpoint` |
| C5 | Resume continues from the checkpoint and reads no completed object again | unit `resume_continues_without_reading_completed_objects_again` |
| C6 | Cancel ends the job as cancelled with no change to the library's files | unit `cancel_ends_the_rebuild_without_changing_the_files`, integration over a real root |
| C7 | A crash resumes from the checkpoint after the lease expires and recovery releases it | unit `a_crash_resumes_from_the_checkpoint` |
| C8 | The report is the job's result, in the view's `result` once finished; `null` before and for kinds without one; equal to a one-shot rebuild's report | unit `the_report_is_the_jobs_result_once_finished`, `result_is_null_for_a_kind_without_one` |
| C9 | Every control journaled once under the job's operation ID; source identity and checkpoint never journaled or shown | unit `every_control_is_journaled_once`, redaction test |
| C10 | `library_not_open` and a malformed body are refused before a job is submitted, journaled once as `library.rebuild` | integration `a_rebuild_of_a_library_that_is_not_open_submits_no_job` |
| C11 | A host without a job service answers `503 unavailable`, journaled once | integration (existing unavailable test extended) |
| C12 | Lease kept alive while one long object is read (renewal by a ticker, at most every third of the lease) | unit `a_long_object_keeps_the_lease` |
| C13 | The checkpoint holds the cursor only (at most 64 KiB); the partial report is kept as the job's stored result and written with the checkpoint in one atomic step | unit `checkpoint_and_partial_result_are_written_together` |
| C14 | `library.rebuild` outcome journaled once when the rebuild finishes (succeeded or failed with its code) | integration |
| C15 | The SQLite job store keeps the result across a reopen | native unit `the_result_survives_a_reopen` |
| C16 | Contract: version one minor above main (0.25.0), the rebuild answer schema, `JobView.result`, regenerated `schema.d.ts` | `unit_contract`, Vitest `contract.test.ts` |
| C17 | Desktop installs its job service in the API state and the rebuild runner for `library.rebuild` | desktop integration `the_rebuild_runs_as_a_job_on_the_desktop` |
| C18 | Bounded loops: passes per run, journal pages; no unbounded retry | review |

## Approach

- `jobs`: `Job.result` (stored JSON text), `RunningJob::checkpoint_with_result` and `complete_with_result`; `JobView.result` (`null` unless completed); `checkpoint` and `renew` take `&self`.
- `local_library::rebuild`: `rebuild_from(backend, after, progress)` reads the objects after the cursor in object-root order and calls a `RebuildProgress` sink per object, which may stop the run; the one-shot `rebuild` uses it.
- `local_library::rebuild_job`: `REBUILD_JOB_KIND`, the route's submit (find the unfinished job of the library, else submit one), and `RebuildRunner` (the `JobRunner` of the kind): start, renew ticker, checkpoint every batch, halt on a request, complete with the report or fail with the code.
- `LibraryOperations`: `rebuild` is replaced by `rebuild_ready` (refuses a library that is not open) and `rebuild_from`.
- `filesquid-native`: `result` column in `jobs` with an additive migration.
- `filesquid-desktop`: install the job service in the API state and the rebuild runner; forward the new library operations.

## Line between API-F2.17 and DE-1.8

API-F2.17 ships the route, the job kind, the runner contract and the API's runner, the SQLite result column, and the desktop wiring (job service and runner installed). DE-1.8 owns the desktop's launch-time resumption of an unfinished rebuild after a restart, its suspension at app exit, the batch size it tunes, and the restart test.

## Verification

`cargo test -p filesquid-api`, `-p filesquid-native --test unit_jobs`, `-p filesquid-desktop`; `npm run test` contract drift in `apps/filesquid/ui`; `FULL=1 bash scripts/pre-commit.sh`.
