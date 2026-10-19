# Conflict Artifact: Task API-F2.7, the engine does not report reservation takeovers

Raised 2026-10-19 by Task [API-F2.7](../plan.md) (global [Task 1.1](../../../../../../resources/implementation-plan.md#task-1.1)) under [sync-logic.md](../../../../../../.agents/rules/sync-logic.md).

**Status: open, owner decision needed.** API-F2.7 is built so that the takeover trigger works as soon as the engine reports a takeover: hosts pass a takeover flag to `OcflValidation::validate_after_commit`, and acceptance criterion (3) is tested at that seam. The flag has no source in `filesquid-ocfl` today, so no host can detect a takeover commit until the resolution below lands.

## 1. Conflict description

The API-F2.7 plan row says "for an object after any commit on it that took over an expired or lost reservation (IO-0.6 reports the takeover)". In the code on `main`, IO-0.6 does not report it to the caller:

- `CommitReceipt` ([`write/commit.rs`](../../../ocfl/src/write/commit.rs)) carries the object ID, version, `created`, inventory digest and written paths. Nothing says the reservation was taken over.
- `Reservation` ([`write/reservation.rs`](../../../ocfl/src/write/reservation.rs)) carries the object, version, holder and token only.
- `LeaseStore` ([`write/lease.rs`](../../../ocfl/src/write/lease.rs), `log_take`) writes only a `tracing::warn!` line ("took over an expired version reservation"). That line goes to stdout, where `filesquid-api` cannot read it.

`filesquid-api` cannot detect a takeover on its own without reading the lease store's private record format. That would duplicate engine meaning outside the engine and break module independence ([ADR 027](../../../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md), AGENTS.md §15). Validating after every commit would satisfy the trigger, but it re-reads every content file of the object on each commit, which the requirement does not ask for.

## 2. PRD / SRS reference

[API-S-022](../srs.md): the host "MUST run the `filesquid-ocfl` validator over every object of every library it has open: on a schedule …, for an object after any commit on it that took over an expired or lost reservation, and on request."

[OCFL-S-018](../../../ocfl/resources/srs.md) "Known gap": a writer whose reservation was taken over can leave a stray file under `vN/`, which the validator reports as E023.

## 3. ADR reference

[ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) puts health and the Monitor behind `filesquid-api`, which reads module results through their public APIs. [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md) and [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md) make `filesquid-ocfl` the only owner of the write path and its reservations.

## 4. Recommended resolution

**Option A (recommended).** The modules engineer adds the takeover to the engine's public result as a small IO-0.6 follow-up in `filesquid-ocfl`. For example, `CommitReceipt::took_over: bool`, set when the reservation replaced one that was held, not released, and had expired or been reassigned. `VersionReservations::reserve` would report the same, so other stores can too. Hosts then call `OcflValidation::validate_after_commit(library, object_id, receipt.took_over)` after each commit. API-F2.7 needs no change for this.

**Option B.** Validate every committed object after each commit, ignoring the takeover. No engine change, but every commit reads all of its object's content once more.

**Option C.** Leave the trigger to the scheduled and on-request runs only, and change API-S-022 and the plan row accordingly.

Artifacts to update for option A: `crates/ocfl/src/write/{commit,reservation,lease}.rs` and their tests (modules engineer), the [ocfl plan](../../../ocfl/resources/plan.md) (a new IO row), and the host commit paths (API-F1.3 for the file routes, DE-1.1).

## 5. Impact assessment

Without a resolution, a stray `vN/` file left behind by a taken-over writer (E023) is found only by the next scheduled run (default every 7 days) or an on-request run, not right after the takeover commit. Health still turns `degraded` once that run happens, so the gap is a delay, not a silent loss. API-F2.7 criteria (1), (2) and (4) to (9) do not depend on this. Criterion (3) is met at the API seam but cannot fire end to end until the engine reports takeovers.
