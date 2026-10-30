# Task DE-1.8 Plan: Host side of the upload review, per-file progress, rebuild job and library facts

Branch `feat/de-1.8-host-upload-review-facts`. Module row [DE-1.8](../plan.md) of global Task 1.2. Dependencies merged: DE-1.7, API-F2.16 (review routes, `ImportStarter::plan`/`start_plan`/`discard`/`plan_items`, `PlanBook`, `JobItems`), API-F2.17 (`RebuildRunner`), API-F2.18 (`LibraryRegistry` over `HostLocations`, free bytes on Linux and macOS).

## Already on main (not redone)

| Part | Done by |
|---|---|
| Free bytes of a registered folder's volume (Linux, macOS; `volume_unreadable` elsewhere) | API-F2.18, `HostLocations::free_space` |
| Library name and last opened time for the host's lifetime | API-F2.18, `RegistryBook` in `HostLocations` |
| Rebuild as a `library.rebuild` job with a checkpoint per object batch (`REBUILD_BATCH` 64) and the runner installed | API-F2.17, desktop `install_job_routes` |
| Job controls, the `import` runner, streamed stage | DE-1.9 |

## Implemented here

### A. Per-file progress (API-S-037 **Per-file progress**)

- `filesquid-native` `SqliteJobItems` (operational store, `import_items` table): the `JobItems` port: each import job's destination and files in manifest order (item ID = the minted file ID, base name, destination folder, state, bytes done of total, error code). Installed on the launch's `Jobs` (`with_items`).
- The import job records each file: `queued` when the manifest is kept, `copying` (bytes done updated at most once a second from the stage's reads, two reads per file), `copied`, `skipped` (already in the library), or `failed` with its code (quarantine).
- Retry: the API's `retry` puts failed files back to `queued` and hands the job to the `import` runner; a run processes the manifest from its checkpoint, then every `queued` file before it (a retry pass), so a retried file is imported alone.
- A run that reaches the end with a failed file leaves the job pending (not completed), so its files can be retried; the launch-time replay does not start a job that has no queued file left and has reached its end. Cancel ends it.

### B. The review (API-S-036 **Review**)

- `DesktopImports` implements `replay`, `plan`, `start_plan`, `discard` and `plan_items` over the API's `PlanBook`: after the native dialog the host lists the sources and hashes each file (no byte staged), then answers `planned` with the facts: files found, new, already in the library (by SHA-256 against the host's index of committed digests), bytes to copy, names numbered, free bytes after (from the registry's free space of the library's folder; `null` when unreadable), the staging limit (10 GiB), the source volume label, and the plan items by base name.
- The plan's fingerprint is a digest of every listed file's path, size and modification time; `start_plan` lists again and refuses `source_changed` when it differs. Only a confirmed plan submits the job, with its plan kept as before; files already in the library are `skipped`.
- `filesquid-native` `SqliteImportDigests` (`import_digests` table): the digest of every committed import per library, written after each commit from the digest the API committed (the API's `import_original` answers it beside the view; no contract change).

### C. Rebuild across launches (API-S-025, DE-S-014)

- `RebuildRunner::with_stop` (API adapter): a set stop flag makes the run checkpoint after the object in hand and give its lease back (`suspend`), the quit of DE-S-014.
- The desktop installs the stop flag of the app's exit and waits, bounded, for running rebuilds at exit; at launch, when the jobs were recovered, it hands every unfinished `library.rebuild` job that is not paused to the runner.

### D. The registry across launches (API-S-038, DE-S-018)

- `filesquid-native` `SqliteLibraryRegistry` (`library_registry` table in the operational store): each library's name, location ID, folder and last opened time. `HostLocations` writes through on `remember` and `rename` and, at launch, re-registers each remembered folder under its location ID and reloads the book, so the recent libraries and the facts survive a restart. The folder path never leaves the host (DE-S-006).

## Constraints copied from the SRS

- API-S-036: the plan before any byte is staged; plan items by base name, at most 1,000 per page; start once, a repeated start answers the same job; discard by `DELETE` or after 1 hour (`PLAN_TTL_MS` 3,600,000) changing nothing; `source_changed` when the source changed; the replay of an open plan, a started job or a cancelled dialog; at most 256 remembered plans.
- API-S-037: item states `queued`, `copying` (bytes done of total), `copied`, `skipped`, `failed` with its code; at most 1,000 items per page; display name only, never a path; retry of a paused job refused `job_paused`; a retry that puts nothing back changes nothing.
- API-S-038 / owner decision 5 (2026-10-23): free bytes on Linux and macOS; elsewhere `null` with `volume_unreadable`, never invented, no new crate.
- DE-S-019: staging cap 10 GiB, reserve max(2 GiB, 5 %); 5 attempts with backoff capped at 60 s; journal `import.file` without paths or names.
- DE-S-014: a running job is suspended at quit with its checkpoint and resumed at the next launch; a launch whose job recovery failed schedules no job.
- DE-S-006: no path or name in any answer, event, error or journal entry beyond the display names API-S-036/037 define.

## Tests (written first)

Native unit: items store (page, retry, states); digest index; the job records item states, skips present files, a retry pass imports only the retried file, a run with a failed file stays pending; registry store round trip. Desktop integration through `filesquid://`: a plan after the scripted dialog with nothing staged; start only after confirmation; discard; `source_changed`; a file's failure with its code retried alone; a rebuild suspended at exit and resumed at the next launch from its checkpoint; recent libraries and free bytes after a restart with no path. BDD scenarios in the desktop features.

## Traceability

[DE-S-018](../srs.md), [DE-S-019](../srs.md), [DE-S-014](../srs.md), [DE-S-006](../srs.md), [API-S-025](../../../api/resources/srs.md), [API-S-036](../../../api/resources/srs.md), [API-S-037](../../../api/resources/srs.md), [API-S-038](../../../api/resources/srs.md), [F-002](../../../../../../resources/prd.md#f-002-safe-file-folder-and-device-import), [F-009](../../../../../../resources/prd.md#f-009-multiple-libraries-and-filesystem-administration).
