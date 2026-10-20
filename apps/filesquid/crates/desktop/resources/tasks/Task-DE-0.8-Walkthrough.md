# Task DE-0.8 Walkthrough: Native-processing library boundary (`filesquid-native`)

Branch: `feat/de-0.8-native-boundary` (global [Task 0.5](../../../../../../resources/implementation-plan.md#task-0.5)).

## Requirements

- **TC-005 Shared Rust graph, hosted mailbox isolation and OCFL writer**: each host links only what its privacy boundary allows.
- **DE-S-015 Native-processing library boundary** (new): SQLite storage, the OCFL coordinator and the job and media workers live in `filesquid-native`, the desktop-only parts behind its `desktop` feature; the Tauri package is a thin host; only the two hosts link `filesquid-native`, the server with default features only.
- **ADR 001 Shared Rust core with Tauri and Axum hosts**, and the **ADR 033 amendment of 2026-10-18** (`filesquid-native`'s `desktop` feature, never enabled by the server).

## What changed

- SQLite storage was already in `filesquid-native`: the journal (CL-0.1), the findings, the per-library catalogs (DE-0.2), the canonical OCFL coordinator with its leases and staging (DE-0.3) and the job store (DE-0.4).
- The remaining native processing in the host, the durable history backfill job worker of DE-0.4 (`durable_jobs.rs`: its lease-renewal thread and the per-library worker loop), moves to `filesquid-native::workers::history_backfill`, behind the `desktop` feature. The move is a file rename. It also drops a `serde` derive in favour of `serde_json::Value`, so `filesquid-native` gains no dependency. The desktop host calls it at launch exactly as before.
- Media workers do not exist yet. DE-0.5 adds them to `filesquid_native::workers`.
- New boundary rules, each also run on a synthetic violation:
  - `filesquid-desktop` `integration_native_boundary` reads `cargo metadata`. Only `filesquid-desktop` (with `desktop`) and `filesquid-server` (no features) may link `filesquid-native`, and neither host may link a SQLite driver directly.
  - The same test scans the desktop `src/`. It may not open storage directly (`rusqlite`, `Connection::open`, `FsBackend::new`, `LeaseStore::new`, `ChangeModel::new`, `StagingArea::new`), run a worker loop (`thread::scope`, `.backfill_library(`, `.renew()`), or use `std::fs` in any form, except in the build-time capability check and the test-instrumented range fixture. Any form means a path, a `use std::fs`, or `fs` inside a grouped `use std::{...}`.
  - The existing `filesquid-server` `unit_native_features` test still proves that the server's resolved feature graph never enables `desktop`.
- `filesquid-native` still builds alone, without `desktop` (`cargo check -p filesquid-native`).

## SRS traceability

| Requirement | Tests |
|---|---|
| DE-S-015 Native-processing library boundary: only the hosts link `filesquid-native`; no SQLite driver in a host | desktop `integration_native_boundary::only_the_hosts_link_filesquid_native_and_the_desktop_has_no_sqlite_driver`, `the_linking_rules_reject_a_sqlite_driver_in_a_host_and_native_in_the_api`; native feature `native_boundary_TC-005.feature` (6 scenarios: every linking entry is checked, and the SQLite driver outline covers `rusqlite`, `libsqlite3-sys` and `sqlx`) |
| DE-S-015 Native-processing library boundary: thin Tauri host | desktop `integration_native_boundary::the_desktop_host_holds_no_native_storage_or_worker_code`, `the_source_rule_rejects_storage_opens_and_worker_loops` |
| TC-005 Shared Rust graph, hosted mailbox isolation and OCFL writer; ADR 033 amendment: server default features only | server `unit_native_features` (2), native feature scenario "The server links filesquid-native with default features only" |
| DE-S-014 Durable jobs and checkpoint-on-quit: unchanged after the move | desktop `integration_durable_jobs` (7: `an_unreadable_checkpoint_visits_every_library_again` added for the checkpoint parser), `integration_history_wiring` (5) |

## TDD: failing run

The boundary test before the worker moved:

```text
test the_desktop_host_holds_no_native_storage_or_worker_code ... FAILED
["durable_jobs.rs: thread::scope", "durable_jobs.rs: .backfill_library(", "durable_jobs.rs: .renew()"]
test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

## Cubic review

- Round 1 (6 P2/P3): fixed the feature step that checked only the first linking entry, the `std::fs` rule that missed `use std::fs` and grouped imports, the SQLite scenario that covered `rusqlite` only, a stale doc phrase, and the untested checkpoint parser. One finding was answered rather than changed: DE-0.8 stays ticked although no media workers exist. There were none in the host to extract, DE-0.5 builds its worker in `filesquid_native::workers` from the start, and DE-S-015 with its boundary test keeps every worker out of the host.

- Round 2 (1 P3), fixed: this raw output was regenerated after the round-1 tests were added.

## Known gaps

- The ciphertext-only hosted service crate does not exist yet. The linking rule already refuses `filesquid-native` in any package but the two hosts, so it will cover that crate too.
- The WASM builds (ADR 025) are module crates, which the same rule covers; there is no separate WASM package yet.

## Mocked Environment Variables

None.

## Raw test output

After Cubic round 1:

```text
$ cargo test -p filesquid-desktop --test integration_native_boundary --locked
running 4 tests
test the_linking_rules_reject_a_sqlite_driver_in_a_host_and_native_in_the_api ... ok
test the_source_rule_rejects_storage_opens_and_worker_loops ... ok
test the_desktop_host_holds_no_native_storage_or_worker_code ... ok
test only_the_hosts_link_filesquid_native_and_the_desktop_has_no_sqlite_driver ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
$ cargo test -p filesquid-desktop --test integration_durable_jobs --locked
running 7 tests
test a_job_for_another_installation_is_failed_and_a_new_one_runs ... ok
test quitting_suspends_the_backfill_job_with_its_checkpoint_kept ... ok
test restart_recovery_reads_only_jobs_and_stays_within_the_budget ... ok
test a_backfill_job_completes_and_records_both_libraries_in_its_checkpoint ... ok
test the_next_launch_resumes_the_job_without_rescanning_completed_libraries ... ok
test a_run_left_by_a_crash_is_waited_out_and_resumed ... ok
test an_unreadable_checkpoint_visits_every_library_again ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.46s
$ cargo test -p filesquid-server --test unit_native_features --locked
running 2 tests
test the_rule_rejects_a_tree_with_the_desktop_feature ... ok
test the_server_links_filesquid_native_with_default_features_only ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
$ cargo test -p filesquid-native --test features --locked
Feature: Durable jobs in the host's operational store
  Scenario: A job suspended at quit resumes from its checkpoint after a restart
  Scenario: A crashed run's lease is released once it expires
  Scenario: An expired worker cannot write after another worker took the job over
  Scenario: A job whose source changed is not resumed
Feature: The native-processing library boundary
  Scenario: The desktop host links the desktop-only native processing
  Scenario: The server links filesquid-native with default features only
  Scenario: No crate but the two hosts links filesquid-native
  Scenario Outline: Only filesquid-native links a SQLite driver
  Scenario Outline: Only filesquid-native links a SQLite driver
  Scenario Outline: Only filesquid-native links a SQLite driver
Feature: Native journal store and data-directory checks
  Scenario: The SQLite journal store writes no log file
  Scenario: A host monitor opens its store in the data directory
  Scenario: A data directory that cannot be written fails health
13 scenarios (13 passed)
41 steps (41 passed)
$ cargo check -p filesquid-native --locked
    Finished `dev` profile [unoptimized] target(s) in 0.60s
```

The head-commit hook run is recorded in the pull request.
