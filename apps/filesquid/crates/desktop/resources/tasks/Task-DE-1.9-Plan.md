# Task DE-1.9 Plan: Desktop wiring of the job routes and import job controls

Branch `feat/de-1.9-desktop-job-wiring`. Module row [DE-1.9](../plan.md) of global Task FS-1.4 ([implementation plan](../../../../../../resources/implementation-plan.md)).

## Already on main (not redone here)

Compared with `git log -- apps/filesquid/crates/desktop` and the [API-F2.17 walkthrough](../../../api/resources/tasks/Task-API-F2.17-Walkthrough.md) (PR #132):

| Part of the row | State on main | Where |
|---|---|---|
| Install this launch's `Jobs` (SQLite job store of DE-0.4) in `ApiState`, so the job routes answer instead of 503 | Done by API-F2.17 | desktop `install_job_routes` (`src/lib.rs`), C17 of that walkthrough |
| Every stored job change publishes `job.update` on the desktop event bus | Done by API-F2.17 (`open_jobs(..).with_events(events)` in `prepare_backfill`) | `src/lib.rs` |
| History backfill keeps its own launcher slot (`install_history_backfill`), not a job runner | Done by DE-0.10 / FS-2.8 | `install_backfill_route` |
| IO-1.7 `ChangeModel::stage_file` | Merged in PR #142 | `crates/ocfl/src/change/model.rs` |

## Implemented here

1. **Kind class:** `import` classified as `JobClass::Transfer` on the launch's job service (`filesquid_native::workers::history_backfill::open_jobs`), so API-S-037's default cap of 4 transfers holds.
2. **Import runner:** `ImportWorker` implements `filesquid_api::jobs::JobRunner`; `resume` queues the job on the worker (`queue_full`/`unavailable` answer `JobError::Unavailable`, the job stays pending). The desktop installs it with `ApiState::install_job_runner("import", ..)` (`install_import_runner`) before the router is served.
3. **Controls at the checkpoint:** the import job (`run_import_job`) reads `RunningJob::requested()` after every per-file checkpoint and every lease renewal between attempts; when a control stands it halts only through `RunningJob::halt_if_requested()`. Pause: pending, paused, checkpoint = index of the next manifest entry after the file in hand. Cancel: cancelled after the file in hand, no further file read, plan finished and the import's intent cleared. Resume that cleared the pause first: `Halt::Continued`, the run goes on. A control seen between attempts of one file stops the attempts with the checkpoint at that file.
4. **Replay:** the launch-time replay and a queued run skip a paused job (read before `Jobs::start`), so no `job.refused`/`import.job` entry is written for it; a job found finished (for example cancelled while pending) has its plan finished and intent cleared.
5. **Journal:** controls are journaled only by the API (`job.paused`, `job.resumed`, `job.cancel_requested`, `job.cancelled`); the worker adds none.
6. **Streamed stage:** `filesquid-api`'s one import commit (`FileService::import_original`) takes a `StorageBackend` source and key instead of bytes and stages the original with `ChangeModel::stage_file` (smallest adapter change in `filesquid-api`, no contract change); `filesquid-native` reads the source through a one-file read-only backend (`import::source::SourceFile`, entity tag from size, modification time and inode, no whole-file hash). `MAX_IN_MEMORY_BYTES`, `ImportFailure::TooLarge` and `too_large_for_this_build` are removed; `source_changed` is a retryable per-file failure under the 5-attempt rule.

## Constraints copied from the SRS

- DE-S-020: job service, `import` runner and kind classes (`import` as a transfer) installed before the router is served; the routes answer on `filesquid://` with the server's meaning and never `unavailable` once started; a resume of an import job hands it to the import runner.
- DE-S-020: requested control read at every per-file checkpoint and lease renewal; halt only through the API's atomic check; pause suspends with its checkpoint after the file in hand; cancel ends `cancelled` after the file in hand with no further file read; resume continues the same manifest without reading a finished file again; a resume that cleared the pause before the checkpoint keeps the run going; a paused job is not started by the launch-time replay.
- DE-S-020 / API-S-037: every control that changes the job journaled once by the API; the worker journals none of them.
- DE-S-020 / DE-S-006: no answer, `job.update` event, error or journal entry carries a source path or a source file or folder name.
- API-S-037: every stored change (submission, start, checkpoint, control, suspension, end) publishes one `job.update`; a lease renewal publishes none; the listing and event carry `result: null`; job view carries no source identity, checkpoint, holder or fence. Default caps: transfers 4, CPU-heavy 2 (CORE-S-006). Resume answers `scheduled`.
- DE-S-019 / OCFL-S-026: each source read at most twice (hash, then copy while hashing); chunks of at most 8 MiB (`MAX_CHUNK_BYTES`), parts of at most 64 MiB; differing digests fail the file with `source_changed` under the per-file attempt rule (at most 5 attempts, backoff capped at 60 s plus jitter); a resumed job reads no finished file again; the 512 MiB `too_large_for_this_build` refusal is dropped; staging cap 10 GiB and reserve max(2 GiB, 5 %) unchanged.
- DE-S-019: the stated size is checked (`size_mismatch`) against the size the source had when staged.
- NFR-003: an import of a file above 512 MiB stays within bounded memory.

## Tests (written first)

- `filesquid-native` `unit_import_file`: a 513 MiB sparse file imports through `stage_file` (no `too_large_for_this_build`), digest and size cited; a source changing between reads fails with `source_changed`, retryable; size mismatch still refused. `unit_import_large` (own binary): peak resident memory (Linux `VmHWM`) grows by less than 256 MiB while a 600 MiB file imports.
- `filesquid-native` `unit_import_job`: the too-large test replaced by a large-file import; `unit_jobs`: `import` is a transfer (fifth running import refused `slots_full`).
- `filesquid-desktop` `integration_import_controls` (new), through `filesquid://` with a gated index that holds the worker on the file in hand: job routes list and answer the import (no 503); pause stops after the file in hand, keeps its checkpoint, is not started by the relaunch replay, resume finishes the same manifest without reading the finished file (it is removed from disk first); cancel ends `cancelled` with no further file read; resume before the checkpoint keeps the run going; each control journaled exactly once; the ordered `job.update` sequence asserted exactly with no lag; redaction of names and paths in answers, events and journal.
- BDD: `desktop_imports_F-002.feature` scenarios for pause/resume and cancel.

## Traceability

[DE-S-020](../srs.md), [DE-S-019](../srs.md), [DE-S-014](../srs.md), [DE-S-006](../srs.md), [OCFL-S-026](../../../ocfl/resources/srs.md), [API-S-033](../../../api/resources/srs.md), [API-S-037](../../../api/resources/srs.md), [NFR-003](../../../../../../resources/prd.md#nfr-003-bounded-processing-and-migration-endurance), [F-002](../../../../../../resources/prd.md#f-002-safe-file-folder-and-device-import), [F-057](../../../../../../resources/prd.md#f-057-desktop-and-server-hosts).

## Out of scope

The per-file items and retry routes of API-S-037 (`/items`, `/retry`, `/retry-failed`) are API work (API-F2.16), not this row.
