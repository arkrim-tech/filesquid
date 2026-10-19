# Task API-F2.7 Plan: OCFL validation findings in health and the Monitor

Module task [API-F2.7](../plan.md) of global Task 1.1. Branch `feat/api-f2.7-ocfl-validation-findings`. Dependencies API-F2.1, API-F1.7, CL-1.1, IO-0.5 and IO-0.6 are complete. One conflict is open: the engine does not report reservation takeovers ([Conflict Artifact](./Task-API-F2.7-Conflict.md)).

## Approach

- `filesquid-api` gains an `ocfl_validation` module:
  - `FindingsStore`, the host-injected store for findings, cursors and runs, plus `MemoryFindingsStore`.
  - The run algorithm, generic over the host's `StorageBackend`.
  - `ValidationService<B>`, which holds the open libraries, the schedule and the cap, and the one-run-per-library lock.
  - The object-safe `OcflValidation` handle installed in `ApiState`.
  - Two routes: `GET /api/v1/admin/ocfl/findings` (`monitor.read`) and `POST /api/v1/admin/ocfl/validation-runs` (new action `ocfl.validation.run`).
- The health rule `ocfl_validation_errors` (at least `degraded`, with the codes and the number of affected objects) reads the store through the monitor. Any change in findings publishes `health.change`.
- `filesquid-native` gains `SqliteFindings` in the operational database.
- Both hosts create the store and service, install them, and tick the scheduler from their monitor ticker. Hosts have no open OCFL libraries until DE-1.1, which registers them through `ValidationService::open_library`.

## SRS constraints (API-S-022 and the plan row)

- C1 The validator runs over every object of every open library through host-injected backends.
- C2 Schedule: every 7 days by default, which the desktop owner or a server administrator may change. The cap is 100,000 objects per run by default and is also changeable.
- C3 After a commit that took over a reservation, that object is validated (seam only; see the Conflict Artifact).
- C4 Runs on request through `POST /api/v1/admin/ocfl/validation-runs`, optionally from the start of the order; by default it continues from the cursor.
- C5 A run registers in the process registry with progress and heartbeat.
- C6 At most one run per library at a time; a second request is refused and journaled.
- C7 A run stops on cancel or at its cap and reports the objects it did not check.
- C8 Runs visit objects in a stable order by object ID from a persistent per-library cursor kept in the findings store. The cursor advances per object checked; reaching the end wraps and starts a new pass. An object deleted ahead of the cursor is skipped. An object created behind the cursor is checked on the next pass.
- C9 When a deleted object still has open findings, they are closed and each closure is journaled as `ocfl.validation.finding_closed` with the opaque object ID, version and code only.
- C10 The store keeps the latest findings per object: object ID, version, code, error or warning, location.
- C11 Each error finding is journaled as `ocfl.validation.finding` with the opaque object ID, version and code only.
- C12 While any error finding is open, health is at least `degraded` with reason `ocfl_validation_errors`, carrying the codes and the number of affected objects. E023 is reported like every other error code.
- C13 A finding stays open until a later validation of that object no longer reports it.
- C14 Warnings (W010 and others) are listed but do not change health.
- C15 The findings route answers the desktop owner or a server administrator only, with the open findings and, per library, the last run (start, end, outcome, checked, not checked) and the next scheduled run time.
- C16 A finding's location is shown only to a caller who may read that object: the desktop owner yes, the server administrator no until API-F1.9.
- C17 Changes to the open findings are pushed on `health.change`.
- C18 Redaction: journal rows and health reasons carry no logical path, content path, title or content.
- C19 Every refusal is journaled. `filesquid-api` does no file or SQL access and does not depend on `filesquid-native`.
- C20 `openapi.json` and the generated `schema.d.ts` are updated.

## Verification

- Unit tests in `tests/unit/ocfl_validation.rs` against MemoryBackend, plus an FsBackend case through a dev-dependency on `filesquid-ocfl` with its `fs` feature.
- A BDD feature, router tests and a native store test.
- `FULL=1 bash scripts/pre-commit.sh`.
