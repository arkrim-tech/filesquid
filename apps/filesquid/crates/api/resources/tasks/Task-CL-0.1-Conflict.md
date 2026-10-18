# Conflict Artifact: Task CL-0.1, the activity-journal store and host checks inside `filesquid-api`

Raised 2026-10-18 by Task [CL-0.1](../plan.md) (global [Task 0.10](../../../../../../resources/implementation-plan.md#task-0.10)) under [sync-logic.md](../../../../../../.agents/rules/sync-logic.md).

**Status: resolved 2026-10-18. Decision: option A** (owner). `filesquid-native` (name confirmed by the owner the same day) was created at `apps/filesquid/crates/native` with the SQLite journal store, the data-directory checks and `HostMonitor`. `filesquid-api` keeps the `JournalStore` trait, the in-memory store, the monitor, the health rules and the admin routes, gains the `HostChecks` trait, and no longer has `rusqlite`, `fs4` or the `sqlite` feature. Both hosts link `filesquid-native`. The [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) amendment records that `filesquid-server` MAY use its SQLite journal store until the server's PostgreSQL operational store lands. The CL-0.1 guards now find no violation (see [Task-CL-0.1-Walkthrough.md](./Task-CL-0.1-Walkthrough.md)).

**Correction:** sections 1 and 4 below named BE-0.3 as the server's PostgreSQL task. BE-0.3 is the ciphertext-only hosted service's schema; `filesquid-server`'s PostgreSQL operational store is [SV-F1.2](../../../server/resources/plan.md), which the amendment and the plans now name.

## 1. Conflict description

CL-0.1 must add isolation tests that fail when `filesquid-api` depends on a SQLite driver or calls `std::fs` directly. On `main` the crate already does both, through code that completed tasks put there:

| Deviation on `main` | Where | Added by |
|---|---|---|
| `rusqlite` (bundled `libsqlite3-sys`), behind the default `sqlite` feature | [`Cargo.toml`](../../Cargo.toml), `src/journal/sqlite.rs` (now [`crates/native/src/journal_sqlite.rs`](../../../native/src/journal_sqlite.rs)) (`SqliteJournal`, opens the database and sets `0o600` with `std::fs` and `std::os::unix::fs`) | API-F2.1 (activity journal) |
| `fs4` and `std::fs` | [`src/monitor.rs`](../../src/monitor.rs): `fs4::available_space`, the write probe `probe_write`, and `HostMonitor::open`, which creates the data directory and opens `<data_dir>/filesquid.sqlite` | API-F2.1 (health host checks) |

Both hosts call `HostMonitor::open`: [`filesquid-server`](../../../server/src/lib.rs) and [`filesquid-desktop`](../../../desktop/src/lib.rs).

The requirements agree that this code must leave `filesquid-api`, but none of the documents says where it goes for **both** hosts today:

- The documented home for SQLite storage is the native-processing library `filesquid-native`. It does not exist yet ([DE-0.8](../../../desktop/resources/plan.md), in global [Task 0.5](../../../../../../resources/implementation-plan.md#task-0.5), which is not done) and is documented as desktop-only, "and the server host where an ADR allows". No ADR allows it for the server.
- [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) and [SRS-F-061](../../../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring) put the server journal in PostgreSQL. `filesquid-server` uses SQLite today; its PostgreSQL schema is [BE-0.3](../../../server/resources/plan.md), not done, and the server is post-MVP ([ADR 035](../../../../../../resources/architecture/035-macos-first-desktop-mvp.md)).
- CL-0.1 depends only on global Task 0.1, so the plan schedules the guard before the code it guards against has anywhere to move.

Moving the store now means choosing one of the resolutions below, which is an owner decision, not a reading of the documents.

## 2. PRD / SRS reference

- [API-S-006](../srs.md): "The crate MUST NOT link Tauri host APIs, libvips, FFmpeg or a SQLite driver and MUST NOT perform storage I/O itself; hosts inject storage backends. Cargo features MUST NOT be the only boundary."
- [SRS-F-061](../../../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring), Journal store: "Each host that runs `filesquid-api` ... MUST keep an append-only `activity_journal` table in its operational store (SQLite in WAL mode on desktop, PostgreSQL on servers)". Health: "host checks (storage roots reachable and writable, free space against thresholds, operational store reachable ...)".
- [PRD TC-005](../../../../../../resources/prd.md#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer): "Cargo features MUST NOT be the only privacy boundary."
- [api plan, CL-0.1](../plan.md): "Add isolation tests that fail if it depends on tauri, libvips, ffmpeg-sys or a SQLite driver (rusqlite/sqlx-sqlite), or calls `std::fs` directly."
- [Implementation plan, library allocation](../../../../../../resources/implementation-plan.md): "Native I/O (SQLite storage, the local-filesystem OCFL backend, libvips, FFmpeg, OS adapters) belongs to native processing"; `filesquid-native` is linked by the "Desktop host (and the server host where an ADR allows)".

## 3. ADR reference

- [ADR 001](../../../../../../resources/architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md), library allocation table: "Native processing | Local filesystem backend, SQLite, libvips, FFmpeg, OS adapters | Tauri only".
- [ADR 025](../../../../../../resources/architecture/025-client-side-filesquid-engines.md) Decision 6: "`filesquid-api` still MUST NOT implement storage I/O itself (no `std::fs`, no network client); it only receives backends from its host."
- [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) Decision 2: the journal lives "in its own operational store (SQLite on desktop, PostgreSQL on `filesquid-server`)".

## 4. Recommended resolution

Recommended: **option A**.

- **A. Pull the storage boundary of DE-0.8 forward.** Keep the `JournalStore` trait, the in-memory store, the monitor, the health rules and the admin routes in `filesquid-api`. Add a host-checks trait (data directory writable, free bytes) that `Monitor` receives, like the store. Create `filesquid-native` now with `SqliteJournal`, the data-directory checks and `HostMonitor::open`, and drop `rusqlite`, `fs4` and the `sqlite` feature from `filesquid-api`. Record in an ADR 033 amendment (or a new ADR) that `filesquid-server` MAY link `filesquid-native`'s SQLite journal store until BE-0.3 delivers PostgreSQL. The CL-0.1 pins then become empty.
- **B. Move the code into each host.** No new crate, but the SQLite store and checks would exist twice, against "Application meaning ... MUST be implemented here once" and the no-redundancy rule of ADR 001.
- **C. Amend API-S-006** to allow an optional host-store feature. Not recommended: PRD TC-005 and API-S-006 both say Cargo features MUST NOT be the only boundary.

Artifacts to update for A: [API-S-006](../srs.md) (unchanged text, add the trait note), [api plan](../plan.md) (CL-0.1 status and a follow-up row), [desktop plan](../../../desktop/resources/plan.md) (DE-0.8 split or moved earlier), [server plan](../../../server/resources/plan.md) (interim SQLite store until BE-0.3), [implementation plan](../../../../../../resources/implementation-plan.md) (Task 0.10 and Task 0.5 dependencies, library allocation row for the server), [implementation-plan-checklist.md](../../../../../../resources/implementation-plan-checklist.md), [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) (amendment) or a new ADR, [SRS-F-061](../../../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring) (interim server store), and the pins in [`tests/support/isolation.rs`](../../tests/support/isolation.rs).

## 5. Impact assessment

- If unresolved, `filesquid-api` is not expected to build for `wasm32-unknown-unknown` with its default features ([API-S-010](../srs.md)): `rusqlite` with bundled SQLite and `fs4` are native-only. This blocks the browser host. The `sqlite` Cargo feature gates only the journal store and `HostMonitor::open`; `fs4` is an unconditional dependency and the `std::fs` write probe and `fs4::available_space` in `monitor.rs` are not feature-gated, so they are linked in every feature set, including `--no-default-features`. Turning the feature off would therefore not even restore the boundary, and a feature-only boundary is what TC-005 forbids.
- CL-0.1 stays open, and global Task 0.10 cannot close. CL-0.2 and CL-0.3 can proceed: the guards already stop any new forbidden crate or storage call.
- No runtime behaviour changes until the move: the guards only pin today's graph and source.
