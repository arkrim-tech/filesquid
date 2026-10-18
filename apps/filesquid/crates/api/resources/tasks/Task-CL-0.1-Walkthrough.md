# Task CL-0.1 Walkthrough: Confirm crate name, SRS, and graph guards

Module task [CL-0.1](../plan.md) of global [Task 0.10](../../../../../../resources/implementation-plan.md#task-0.10). Branch `feat/cl-0.1-graph-guards`, from `main` at `0d51b3b` (PR #26 DE-F1.1 and PR #24 UI-0.1 merged).

**Status: partly done, blocked on owner review.** The crate name, the module SRS and the graph and source guards are in place. `main` already breaks API-S-006 in code that API-F2.1 added (the SQLite journal store and the monitor's data-directory checks), and the documents do not say where that code moves for both hosts. That is raised in the [Conflict Artifact](./Task-CL-0.1-Conflict.md). The guards pin those exact deviations, so any new violation fails, and the CL-0.1 boxes in the [api plan](../plan.md) and the [checklist](../../../../../../resources/implementation-plan-checklist.md) stay unchecked.

## API-S-006: Application meaning (catalog, search, replica, the FileSquid OCFL application profile) MUST be implemented here once for every host. The crate MUST NOT link Tauri host APIs, libvips, FFmpeg or a SQLite driver and MUST NOT perform storage I/O itself; hosts inject storage backends. Cargo features MUST NOT be the only boundary.

Parents: [TC-005 Shared Rust graph, hosted mailbox isolation and OCFL writer](../../../../../../resources/prd.md#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer), [SRS-TC-005 Shared Rust graph, hosted mailbox isolation and OCFL writer](../../../../../../resources/srs.md#srs-tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer), [NFR-008 Maintainability and verified engineering](../../../../../../resources/prd.md#nfr-008-maintainability-and-verified-engineering), [ADR 001](../../../../../../resources/architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 025](../../../../../../resources/architecture/025-client-side-filesquid-engines.md) Decision 6.

| Constraint | Gate | State |
|---|---|---|
| Crate is `filesquid-api` at `crates/api`, a workspace member | `crate_is_named_filesquid_api_in_the_workspace`, feature scenario 1 | Met |
| Module SRS states the boundary | `module_srs_states_the_application_layer_boundary` | Met |
| No `tauri*` | graph guard, all three feature sets | Met |
| No `libvips*`, `ffmpeg*` | graph guard | Met |
| No SQLite driver (`rusqlite`, `libsqlite3-sys`, `sqlite`, `sqlx*`) | graph guard | **Pinned deviation**: `rusqlite`, `libsqlite3-sys` with the default and all-features sets |
| No storage I/O crate or network client (`fs4`, `reqwest`, `ureq`, `isahc`, `surf`, `curl*`, `attohttpc`) | graph guard | **Pinned deviation**: `fs4` in every feature set |
| No `std::fs`, `tokio::fs`, `fs4::`, `os::unix::fs`, `os::windows::fs` in `src/` | source guard | **Pinned deviation**: `src/journal/sqlite.rs`, `src/monitor.rs` |
| Features are not the only boundary | default, `--no-default-features` and `--all-features` graphs all checked | Met |
| `filesquid-core`, `-ocfl`, `-metadata`, `-c2pa` allowed | `rules_allow_the_modules_and_the_router`, feature outline | Met |

### Already done on `main`

- API-F0.1 created the crate as `filesquid-api`; CL-0.1's status line already records the re-scope from the former `apps/client` crate.
- API-F1.7 moved the administrator token file out of `filesquid-api` into `filesquid-server` for API-S-006 ([walkthrough](./Task-API-F1.7-Walkthrough.md)).
- No Tauri, libvips or FFmpeg crate was in the graph.

## What changed

- [`tests/support/isolation.rs`](../../tests/support/isolation.rs): the rule table, the `cargo tree` runner (normal and build edges, every target, `--locked`), the bounded `src/` walk (8 levels, 2,000 files; a cap is an error), the line scanner (ignores `//` comments, catches grouped `use std::{fs, ...}`), and the pins `pinned_graph_deviations` and `PINNED_SOURCE_DEVIATIONS`. The comparison is exact, so a pinned deviation that disappears also fails and the pin list has to be updated with the move.
- [`tests/unit/isolation.rs`](../../tests/unit/isolation.rs) (`unit_isolation`): 11 tests, including synthetic names and lines proving each rule fires.
- [`tests/features/application_layer_isolation_API-S-006.feature`](../../tests/features/application_layer_isolation_API-S-006.feature) with steps in [`tests/support/isolation_steps.rs`](../../tests/support/isolation_steps.rs), written with the feature-writer skill: 21 scenarios (crate and SRS, three feature sets, four allowed modules, eight forbidden crates, the source scan, four forbidden lines).
- [`Task-CL-0.1-Conflict.md`](./Task-CL-0.1-Conflict.md): the Conflict Artifact.
- [api plan](../plan.md): CL-0.1 status and related tests. Root `README.md`: the `unit_isolation` command, run before it was written.

## Tests

TDD: the guards were first run with empty pins. They failed on `main`:

```text
test only_the_pinned_files_perform_storage_io ... FAILED
test every_feature_set_links_only_the_pinned_deviations ... FAILED
---- only_the_pinned_files_perform_storage_io stdout ----
assertion `left == right` failed: files under src/ with direct storage I/O differ from the pinned deviations (see resources/tasks/Task-CL-0.1-Conflict.md)
  left: ["src/journal/sqlite.rs", "src/monitor.rs"]
 right: []
---- every_feature_set_links_only_the_pinned_deviations stdout ----
assertion `left == right` failed: default: forbidden runtime dependencies differ from the pinned deviations (see resources/tasks/Task-CL-0.1-Conflict.md)
  left: ["fs4", "libsqlite3-sys", "rusqlite"]
 right: []
test result: FAILED. 9 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.50s
```

With the deviations pinned, `cargo test -p filesquid-api --locked --test unit_isolation`:

```text
running 11 tests
test crate_is_named_filesquid_api_in_the_workspace ... ok
test every_rule_rejects_a_matching_name ... ok
test module_srs_states_the_application_layer_boundary ... ok
test a_forbidden_dependency_would_fail_the_graph_guard ... ok
test comments_and_lookalikes_are_not_storage_io ... ok
test rules_allow_the_modules_and_the_router ... ok
test package_names_parse_cargo_tree_lines ... ok
test rules_reject_every_named_crate ... ok
test storage_io_lines_are_detected ... ok
test only_the_pinned_files_perform_storage_io ... ok
test every_feature_set_links_only_the_pinned_deviations ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.67s
```

`cargo test -p filesquid-api --locked` (summary lines):

```text
     Running unittests src/lib.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
     Running tests/features.rs
109 scenarios (49 passed, 60 skipped)
226 steps (166 passed, 60 skipped)
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
test result: ok. 11 passed; 0 failed
     Running tests/unit/journal.rs
test result: ok. 11 passed; 0 failed
     Running tests/unit/metrics.rs
test result: ok. 1 passed; 0 failed
     Running tests/unit/registry.rs
test result: ok. 5 passed; 0 failed
     Running tests/unit/routes.rs
test result: ok. 4 passed; 0 failed
     Running tests/unit/writer.rs
test result: ok. 5 passed; 0 failed
```

All 21 new scenarios pass (passed scenarios rose from 28 before this task's steps were added to 49). The 60 skipped scenarios belong to other features whose domain operations do not exist yet. The JUnit report is written to `crates/api/tests/allure-results/cucumber-junit.xml`. `cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo doc --workspace --no-deps` pass.

Coverage: the guards are tests, not product code; no coverage tool is configured for the Rust workspace yet.

## Mocked Environment Variables

None.
