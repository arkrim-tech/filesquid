# Task CL-0.1 Walkthrough: Confirm crate name, SRS, and graph guards

Module task [CL-0.1](../plan.md) of global [Task 0.10](../../../../../../resources/implementation-plan.md#task-0.10). Branch `feat/cl-0.1-graph-guards`, from `main` at `0d51b3b` (PR #26 DE-F1.1 and PR #24 UI-0.1 merged).

**Status: done.** The guards first showed that `main` broke API-S-006, through code that API-F2.1 had added. That was raised in the [Conflict Artifact](./Task-CL-0.1-Conflict.md). The owner chose option A on 2026-10-18, and the code moved to the new native-processing crate `filesquid-native`. The graph and source guards now find no violation, and there is no pin list.

## API-S-006: Application meaning (catalog, search, replica, the FileSquid OCFL application profile) MUST be implemented here once for every host. The crate MUST NOT link Tauri host APIs, libvips, FFmpeg or a SQLite driver and MUST NOT perform storage I/O itself; hosts inject storage backends. Cargo features MUST NOT be the only boundary.

Parents: [TC-005 Shared Rust graph, hosted mailbox isolation and OCFL writer](../../../../../../resources/prd.md#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer), [SRS-TC-005 Shared Rust graph, hosted mailbox isolation and OCFL writer](../../../../../../resources/srs.md#srs-tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer), [NFR-008 Maintainability and verified engineering](../../../../../../resources/prd.md#nfr-008-maintainability-and-verified-engineering), [ADR 001](../../../../../../resources/architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 025](../../../../../../resources/architecture/025-client-side-filesquid-engines.md) Decision 6.

| Constraint | Gate | State |
|---|---|---|
| Crate is `filesquid-api` at `crates/api`, a workspace member | `crate_is_named_filesquid_api_in_the_workspace`, feature scenario 1 | Met |
| Module SRS states the boundary | `module_srs_states_the_application_layer_boundary` | Met |
| No `tauri*`, `libvips*`, `ffmpeg*` | graph guard, all three feature sets | Met |
| No SQLite driver (`rusqlite`, `libsqlite3-sys`, `sqlite`, `sqlx*`) | graph guard | Met (moved to `filesquid-native`) |
| No storage crate (`fs4`), no `filesquid-native`, no network client, no async runtime other than Tokio | graph guard | Met |
| No `std::fs`, `tokio::fs`, `fs4`, OS file-system extensions, file handles or network clients in `src/` | source guard, no file exempt | Met |
| Features are not the only boundary | default, `--no-default-features` and `--all-features` graphs all checked; the `sqlite` feature is gone | Met |
| `filesquid-core`, `-ocfl`, `-metadata`, `-c2pa` allowed | `rules_allow_the_modules_the_router_and_lookalikes`, feature outline | Met |
| Hosts inject storage backends | `Monitor::start(store, checks, config, events)`; `health_uses_the_injected_host_checks` | Met |

### Secondary SRS mapping

- **SRS-F-061: Activity journal and live health monitoring.** The Journal store and Health bullets now say that the store and the storage-dependent checks are host-injected, and that `filesquid-server` may keep its journal in the `filesquid-native` SQLite store until its PostgreSQL store exists. Covered by `filesquid-native`'s `unit_journal`, `unit_host` and `native_journal_store_F-061.feature`, and the unchanged server and desktop integration tests.

### Already done on `main`

- API-F0.1 created the crate as `filesquid-api`.
- API-F1.7 moved the administrator token file into `filesquid-server`.
- No Tauri, libvips or FFmpeg crate was in the graph.

## What changed

### Crates moved and created

- **New crate `filesquid-native`** ([`crates/native`](../../../native/src/lib.rs)), a workspace member that depends on `filesquid-api`, `rusqlite` (bundled), `fs4`, `serde_json` and `tracing`:
  - `journal_sqlite.rs`: `SqliteJournal`, moved unchanged from `crates/api/src/journal/sqlite.rs` (git rename).
  - `host_checks.rs`: `DataDirChecks`, which implements the API's `HostChecks` with the write probe and `fs4::available_space`, both moved from `crates/api/src/monitor.rs`.
  - `host_monitor.rs`: `HostMonitor`, `HOST_HEARTBEAT_TIMEOUT_MS` and `JOURNAL_FILE`, moved from `crates/api/src/monitor.rs`.
- **Tests moved with the code:**
  - `crates/api/tests/unit/journal.rs` became `crates/native/tests/unit/journal.rs` (git rename; imports only).
  - The write-probe test moved from `crates/api/tests/unit/admin.rs` to `crates/native/tests/unit/host.rs`.
  - The F-061 "no log file" step moved from the API feature to `crates/native/tests/features/native_journal_store_F-061.feature`. The API scenario keeps a comment pointing there.
- **`filesquid-api`:**
  - New `HostChecks` trait and `StaticHostChecks` (pure; for tests and hosts without a data directory).
  - `Monitor::start` takes the checks next to the store, and `MonitorConfig::new(host)` no longer takes a data directory.
  - `JournalPage::from_overfetch`, `retention_cutoff` and `retention_entry` are now public for external stores.
  - The `sqlite` feature, `rusqlite` and `fs4` are gone.
- **Hosts:** `filesquid-server` and `filesquid-desktop` depend on `filesquid-native` and import `filesquid_native::HostMonitor`. Behaviour is unchanged: same file, same probe, same journal entries.

### Guards

- **Graph guard:** [`tests/support/isolation.rs`](../../tests/support/isolation.rs) and [`tests/unit/isolation.rs`](../../tests/unit/isolation.rs) (`unit_isolation`, 13 tests). The rules also cover `filesquid-native` and the async runtimes `async-std`, `smol` (exact match, so `smol_str` stays allowed), `async-executor`, `async-global-executor`, `async-io`, `async-net`, `actix-rt` and `glommio`.
- **Source guard:** [`tests/support/source_scan.rs`](../../tests/support/source_scan.rs), copied from the `filesquid-ocfl` scanner. It removes line comments, nested block comments and string literals before matching, so a `//` inside a string hides nothing. It catches grouped `std::{...}` imports across lines and reports file, line and pattern. No file is exempt.
- **BDD:** [`application_layer_isolation_API-S-006.feature`](../../tests/features/application_layer_isolation_API-S-006.feature), 37 scenarios, all passing. A double quote inside an example line is written `QUOTE`, because a Gherkin table cannot hold `\"`.
- **Gate:** `scripts/verify_tests.sh` also runs `cargo test -p filesquid-native` alone.

### Docs

[ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) amendment and index row, [SRS-F-061](../../../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring), the SRS crate lists, [API-S-006](../srs.md) trait note, [implementation plan](../../../../../../resources/implementation-plan.md) (module table, library allocation, Task 0.5 and Task 0.10), [desktop plan](../../../desktop/resources/plan.md) (header, DE-0.8), [server plan](../../../server/resources/plan.md) (interim store, SV-F1.2), [architecture](../../../../../../resources/architecture.md), root README, this plan and the [checklist](../../../../../../resources/implementation-plan-checklist.md).

### Cubic round 2 (review on 0fd3a6d) and the main merge

- `main` (#28 UI-0.2, #29 content-handle docs, #30 IO-0.5) was merged in with a merge commit. The only conflict was the API SRS: main's API-S-004 and API-S-005 (content handles) were kept with this branch's API-S-006 note.
- **Feature gate.** `filesquid-native` has a `desktop` Cargo feature, off by default, for the desktop-only processing DE-0.8 will add. `filesquid-desktop` enables it, and `filesquid-server` uses the default features only. The server test `unit_native_features` runs `cargo tree -p filesquid-server -e features` and fails if the server enables `desktop`. This is recorded in the ADR 033 amendment, the desktop plan (DE-0.8), the server plan, the implementation plan and the architecture.
- **Second monitor.** `HostMonitor::open` refuses a second monitor for an `ApiState` before creating any file. The new `HostMonitor::start(store, checks, ...)`, which SV-F1.2 will call with its PostgreSQL store, shuts down a monitor it could not install and returns `ALREADY_INSTALLED`. Two tests cover this.
- **Write probe.** It skips a probe name that is already taken (at most `MAX_PROBE_ATTEMPTS` = 32 names) instead of reporting the directory as not writable. The collision test seeds 16 real probe names and checks they survive.
- **Log-file names.** The no-log-file checks use `is_log_file_name`, which is the same rule as `ends_with(".log")`, so a file named exactly `.log` is caught. The `.log` literal is avoided only because `verify_centralized_logging.sh` rejects it in staged sources. The feature step walks subdirectories (bounded) and cleans up its directories, and the health scenario now asserts `failing` with `data_dir_not_writable`.
- **Graph guard.** The runtime rule's message now says what the guard checks (crate names; Tokio's features are not checked). `monoio`, `compio*` and `tokio-uring` are added. The happy-path source-scan step asserts that more than 10 files were scanned.
- **Docs.** ADR 005 and its index row note the interim server SQLite journal. The Storage row and the server-runtime paragraph in the architecture name it. `HostMonitor` is named wherever the move is described. The README crate list includes `native`. The Conflict Artifact's analysis is marked historical.

## wasm32

`cargo build -p filesquid-api --target wasm32-unknown-unknown --locked` still fails, but no longer because of SQLite or `fs4`. `axum`'s default features pull in `hyper-util` and `tokio` with networking, and `mio` refuses the target ("This wasm target is unsupported by mio. If using Tokio, disable the net feature."). That belongs to API-S-010 and is not fixed here.

## Tests

TDD: the strict guards were first run on this branch with empty pins, before any pin or move:

```text
test only_the_pinned_files_perform_storage_io ... FAILED
test every_feature_set_links_only_the_pinned_deviations ... FAILED
  left: ["src/journal/sqlite.rs", "src/monitor.rs"]
  left: ["fs4", "libsqlite3-sys", "rusqlite"]
test result: FAILED. 9 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.50s
```

After the move, `cargo test -p filesquid-api -p filesquid-native --locked` exited 0:

```text
     Running tests/features.rs (filesquid-api)
154 scenarios (65 passed, 89 skipped)
302 steps (213 passed, 89 skipped)
     Running tests/unit/admin.rs
test result: ok. 6 passed; 0 failed
     Running tests/unit/auth.rs
test result: ok. 34 passed; 0 failed
     Running tests/unit/contract.rs
test result: ok. 28 passed; 0 failed
     Running tests/unit/errors.rs
test result: ok. 11 passed; 0 failed
     Running tests/unit/events.rs
test result: ok. 13 passed; 0 failed
     Running tests/unit/health.rs
test result: ok. 2 passed; 0 failed
     Running tests/unit/health_rules.rs
test result: ok. 7 passed; 0 failed
     Running tests/unit/isolation.rs
test result: ok. 13 passed; 0 failed
     Running tests/unit/metrics.rs
test result: ok. 1 passed; 0 failed
     Running tests/unit/registry.rs
test result: ok. 5 passed; 0 failed
     Running tests/unit/routes.rs
test result: ok. 4 passed; 0 failed
     Running tests/unit/writer.rs
test result: ok. 5 passed; 0 failed
     Running tests/features.rs (filesquid-native)
3 scenarios (3 passed)
11 steps (11 passed)
     Running tests/unit/host.rs
test result: ok. 6 passed; 0 failed
     Running tests/unit/journal.rs
test result: ok. 12 passed; 0 failed
```

The 89 skipped API scenarios belong to features whose domain operations do not exist yet (29 of them come from `content_handles_F-056.feature`, merged from `main` with #29). Before the move, the API had 109 scenarios with 49 passing.

`cargo test -p filesquid-server -p filesquid-desktop --locked` exited 0 with the following results:
- desktop: `integration_api_protocol` 26 and `integration_event_channel` 12 passed;
- server: unit tests 5, `integration_auth` 18, `integration_events` 2, `integration_health` 2, `integration_monitor` 3, `integration_static_ui` 3 `unit_health` 5 and `unit_native_features` 2 passed.

`cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `bash scripts/pre-commit.sh` (which also runs rustdoc and the test-instrumented desktop suite) pass.

Coverage: no coverage tool is configured for the Rust workspace yet.

## Mocked Environment Variables

None.
