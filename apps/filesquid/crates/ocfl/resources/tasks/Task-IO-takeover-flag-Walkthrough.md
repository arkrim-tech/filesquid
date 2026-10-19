# Task IO takeover flag Walkthrough: commit receipts report reservation takeovers

Module plan row: [IO-0.6](../plan.md) (Status note: follow-up). Owner decision 2026-10-19, option A of the API-F2.7 Conflict Artifact (`apps/filesquid/crates/api/resources/tasks/Task-API-F2.7-Conflict.md`, PR #74). Consumer: [API-F2.7](../../../api/resources/plan.md) post-commit validation ([API-S-022](../../../api/resources/srs.md)).

## OCFL-S-018: Write order and unpublished versions — every commit receipt reports whether its reservation took over one that was still held

## OCFL-S-020: Version-inventory repair — the repair receipt reports the same

Sources: [OCFL-S-018, OCFL-S-020](../srs.md), [API-S-022](../../../api/resources/srs.md), [ADR 007](../../../../../../resources/architecture/007-durable-jobs-and-interactive-performance.md), [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md), [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md).

## What changed

- `Reservation::took_over() -> bool` and `Reservation::with_takeover(self) -> Self` (additive; `Reservation::new` is unchanged and starts `false`). The `VersionReservations::reserve` contract now says the returned reservation reports a takeover.
- `LeaseStore` marks the reservation it grants as a takeover when it replaces a lease record that was not released (it can only replace a live record once it has expired, so this is exactly the expired-or-reassigned case). A released or absent record is not a takeover. The existing `tracing::warn!` ("took over an expired version reservation") is unchanged.
- `CommitReceipt::took_over` and `RepairReceipt::took_over` (additive public fields) copy the flag from the reservation the commit or repair published under. `ChangeModel::commit`, `revert`, `reinstate` and the view writes return the same `CommitReceipt`, so they carry it too.
- No change to `filesquid-api` (wiring follows API-F2.7 on `main`); the IO-0.10 and IO-1.2 public APIs keep their shape (one added field and two added methods). Inside the workspace no crate builds `filesquid_ocfl::write::CommitReceipt` or `RepairReceipt` by struct literal (`cargo clippy --workspace` passes).
- Module SRS OCFL-S-018, crate README "Write", and the IO-0.6 plan row (Status note naming API-F2.7) are updated.

## TDD

Tests first: `tests/unit/lease.rs` (2 tests), `tests/integration/write_order.rs` (new `a_commit_reports_whether_it_took_over_a_lapsed_reservation` on both backends, and every crash-recovery test now asserts the retry reports the takeover), and two steps in `write_order_OCFL-S-018.feature` (no takeover on a normal commit; a takeover on each crash-recovery example). Red run:

```text
error[E0609]: no field `took_over` on type `CommitReceipt`
   --> crates/ocfl/tests/integration/write_order.rs:309:21
    |
309 |     assert!(receipt.took_over, "the retry took over the crashed lease");
    |                     ^^^^^^^^^ unknown field
    |
    = note: available fields are: `object_id`, `version`, `created`, `inventory_digest`, `written`

error[E0599]: no method named `took_over` found for struct `filesquid_ocfl::write::Reservation` in the current scope
   --> crates/ocfl/tests/unit/lease.rs:248:20
    |
248 |     assert!(!first.took_over(), "nothing to take over");
    |                    ^^^^^^^^^
    |
help: there is a method `token` with a similar name
    |
248 -     assert!(!first.took_over(), "nothing to take over");
248 +     assert!(!first.token(), "nothing to take over");
    |

error[E0599]: no method named `took_over` found for struct `filesquid_ocfl::write::Reservation` in the current scope
   --> crates/ocfl/tests/unit/lease.rs:251:21
    |
251 |     assert!(!second.took_over(), "a released lease is free, not taken over");
    |                     ^^^^^^^^^
    |
help: there is a method `token` with a similar name
    |
251 -     assert!(!second.took_over(), "a released lease is free, not taken over");
251 +     assert!(!second.token(), "a released lease is free, not taken over");
    |

error[E0599]: no method named `took_over` found for struct `filesquid_ocfl::write::Reservation` in the current scope
   --> crates/ocfl/tests/unit/lease.rs:254:19
    |
254 |     assert!(third.took_over(), "an unreleased, expired lease was taken over");
    |                   ^^^^^^^^^
    |
help: there is a method `token` with a similar name
    |
254 -     assert!(third.took_over(), "an unreleased, expired lease was taken over");
254 +     assert!(third.token(), "an unreleased, expired lease was taken over");
    |

error[E0599]: no method named `took_over` found for struct `filesquid_ocfl::write::Reservation` in the current scope
   --> crates/ocfl/tests/unit/lease.rs:260:20
    |
260 |     assert!(!plain.took_over());
    |                    ^^^^^^^^^
    |
help: there is a method `token` with a similar name
    |
260 -     assert!(!plain.took_over());
260 +     assert!(!plain.token());
    |

error[E0599]: no method named `with_takeover` found for struct `filesquid_ocfl::write::Reservation` in the current scope
   --> crates/ocfl/tests/unit/lease.rs:261:76
    |
261 |     let marked = Reservation::new("urn:example:a", v("v1"), "writer-a", 2).with_takeover();
    |                                                                            ^^^^^^^^^^^^^ method not found in `filesquid_ocfl::write::Reservation`

error[E0609]: no field `took_over` on type `CommitReceipt`
   --> crates/ocfl/tests/integration/write_order.rs:676:20
    |
676 |     assert!(!first.took_over, "a first reservation takes nothing over");
    |                    ^^^^^^^^^ unknown field
    |
    = note: available fields are: `object_id`, `version`, `created`, `inventory_digest`, `written`

error[E0609]: no field `took_over` on type `CommitReceipt`
   --> crates/ocfl/tests/integration/write_order.rs:680:21
    |
680 |     assert!(!second.took_over, "a released lease is not taken over");
    |                     ^^^^^^^^^ unknown field
    |
    = note: available fields are: `object_id`, `version`, `created`, `inventory_digest`, `written`

error[E0599]: no method named `took_over` found for struct `filesquid_ocfl::write::Reservation` in the current scope
   --> crates/ocfl/tests/integration/write_order.rs:682:19
    |
682 |     assert!(!held.took_over());
    |                   ^^^^^^^^^
    |
help: there is a method `token` with a similar name
    |
682 -     assert!(!held.took_over());
682 +     assert!(!held.token());
    |

error[E0609]: no field `took_over` on type `CommitReceipt`
   --> crates/ocfl/tests/integration/write_order.rs:687:19
    |
687 |     assert!(third.took_over, "the stalled writer's lease was taken over");
    |                   ^^^^^^^^^ unknown field
    |
    = note: available fields are: `object_id`, `version`, `created`, `inventory_digest`, `written`

For more information about this error, try `rustc --explain E0599`.
error: could not compile `filesquid-ocfl` (test "unit_lease") due to 5 previous errors
warning: build failed, waiting for other jobs to finish...
Some errors have detailed explanations: E0599, E0609.
For more information about an error, try `rustc --explain E0599`.
error: could not compile `filesquid-ocfl` (test "integration_write_order") due to 5 previous errors
```

## Test output

`cargo test -p filesquid-ocfl --locked --test unit_lease takeover`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.23s
     Running tests/unit/lease.rs ($CARGO_TARGET_DIR/debug/deps/unit_lease-196772f0f0361667)

running 2 tests
test a_new_reservation_is_not_a_takeover_unless_marked ... ok
test an_expired_lease_fails_its_fence_check_even_before_takeover ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.00s

```

`cargo test -p filesquid-ocfl --locked --test integration_write_order -- took_over crash`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.22s
     Running tests/integration/write_order.rs ($CARGO_TARGET_DIR/debug/deps/integration_write_order-22dd8487357674b8)

running 6 tests
test a_crash_before_the_first_root_sidecar_publishes_nothing ... ok
test a_commit_reports_whether_it_took_over_a_lapsed_reservation ... ok
test a_crash_before_the_root_sidecar_reads_as_unpublished ... ok
test a_crash_before_the_root_inventory_leaves_vn_unpublished ... ok
test a_crash_between_content_and_inventories_leaves_vn_unpublished ... ok
test a_crash_while_writing_content_leaves_vn_unpublished ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 19 filtered out; finished in 0.15s

```

`cargo test -p filesquid-ocfl --locked --test features -- --input "write_order_*.feature"`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.27s
     Running tests/features.rs ($CARGO_TARGET_DIR/debug/deps/features-9009fa7eceb51bdd)
Feature: Local write order and unpublished versions
  Scenario: A new version is published in write order
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When writer "writer-a" commits "v2" adding "d.txt"
   ✔  Then the commit receipt should report no reservation takeover
   ✔  And the reservation should come before every "v2" byte
   ✔  And the writes should end with the version inventory, the root inventory and the root sidecar
   ✔  And the object head should be "v2"
   ✔  And the object should validate with zero errors
  Scenario: Unchanged and renamed files reuse their stored bytes
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When writer "writer-a" commits "v2" renaming "b.txt" to "c.txt"
   ✔  Then no content should be written under "v2"
   ✔  And "c.txt" should resolve to the content path "v1/content/b.txt"
  Scenario Outline: A crash leaves the new version unpublished until it is recovered
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given writer "writer-a" crashed while writing "obj/v2/content/d.txt" during the commit of "v2"
   ✔  When I open the written object
   ✔  Then the object head should be "v1"
   ✔  And pinning "v2" should be refused as an unknown version
   ✔  When the crashed writer's lease lapses and writer "writer-b" retries the commit
   ✔  Then the object head should be "v2"
   ✔  And the commit receipt should report a reservation takeover
   ✔  And the history should list "v1, v2"
   ✔  And the object should validate with zero errors
  Scenario Outline: A crash leaves the new version unpublished until it is recovered
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given writer "writer-a" crashed while writing "obj/v2/inventory.json.sha256" during the commit of "v2"
   ✔  When I open the written object
   ✔  Then the object head should be "v1"
   ✔  And pinning "v2" should be refused as an unknown version
   ✔  When the crashed writer's lease lapses and writer "writer-b" retries the commit
   ✔  Then the object head should be "v2"
   ✔  And the commit receipt should report a reservation takeover
   ✔  And the history should list "v1, v2"
   ✔  And the object should validate with zero errors
  Scenario Outline: A crash leaves the new version unpublished until it is recovered
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given writer "writer-a" crashed while writing "obj/inventory.json" during the commit of "v2"
   ✔  When I open the written object
   ✔  Then the object head should be "v1"
   ✔  And pinning "v2" should be refused as an unknown version
   ✔  When the crashed writer's lease lapses and writer "writer-b" retries the commit
   ✔  Then the object head should be "v2"
   ✔  And the commit receipt should report a reservation takeover
   ✔  And the history should list "v1, v2"
   ✔  And the object should validate with zero errors
  Scenario Outline: A crash leaves the new version unpublished until it is recovered
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given writer "writer-a" crashed while writing "obj/inventory.json.sha256" during the commit of "v2"
   ✔  When I open the written object
   ✔  Then the object head should be "v1"
   ✔  And pinning "v2" should be refused as an unknown version
   ✔  When the crashed writer's lease lapses and writer "writer-b" retries the commit
   ✔  Then the object head should be "v2"
   ✔  And the commit receipt should report a reservation takeover
   ✔  And the history should list "v1, v2"
   ✔  And the object should validate with zero errors
  Scenario: A second writer cannot reserve a version that is held
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given writer "writer-b" holds the reservation for "v2"
   ✔  When writer "writer-a" commits "v2" adding "d.txt"
   ✔  Then the commit should be refused because the reservation is held
   ✔  And no byte should be written under "v2"
  Scenario: A stale lease holder cannot write content after reassignment
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given the lease passes to writer "writer-b" after "obj/v2/content/d.txt" is written
   ✔  When writer "writer-a" commits "v2" adding "d.txt" and "e.txt"
   ✔  Then the commit should be refused because the reservation was lost
   ✔  And "obj/v2/content/e.txt" should not be stored
   ✔  And the object head should be "v1"
  Scenario: A stale lease holder cannot publish the head after reassignment
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given the lease passes to writer "writer-b" after "obj/v2/inventory.json.sha256" is written
   ✔  When writer "writer-a" commits "v2" adding "d.txt"
   ✔  Then the commit should be refused because the reservation was lost
   ✔  And the root inventory should be unchanged
   ✔  And the object head should be "v1"
  Scenario: A moved head is refused without publishing
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given writer "writer-b" has committed "v2" adding "d.txt"
   ✔  When writer "writer-a" commits on top of "v1" adding "e.txt"
   ✔  Then the commit should be refused because the head moved to "v2"
   ✔  And the object head should be "v2"
[Summary]
1 feature
10 scenarios (10 passed)
73 steps (73 passed)
```

`cargo test -p filesquid-ocfl --locked` (summary lines of the whole crate):

```text
     Running unittests src/lib.rs ($CARGO_TARGET_DIR/debug/deps/filesquid_ocfl-336d9c2a83f05f02)
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.80s
     Running tests/features.rs ($CARGO_TARGET_DIR/debug/deps/features-9009fa7eceb51bdd)
   ✔  Then Arrow, Parquet, DuckDB, and Iceberg should be absent from default features
[Summary]
15 features
151 scenarios (148 passed, 3 skipped)
706 steps (703 passed, 3 skipped)
     Running tests/integration/change_model.rs ($CARGO_TARGET_DIR/debug/deps/integration_change_model-97a7db5939f6743a)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.78s
     Running tests/integration/content_fixtures.rs ($CARGO_TARGET_DIR/debug/deps/integration_content_fixtures-2450cd9c4beef973)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.75s
     Running tests/integration/enumerate_state.rs ($CARGO_TARGET_DIR/debug/deps/integration_enumerate_state-c992207f0a36f4c4)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.27s
     Running tests/integration/fixture_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_fixture_conformance-6188b5d43aaacb5e)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
     Running tests/integration/fixture_inventories.rs ($CARGO_TARGET_DIR/debug/deps/integration_fixture_inventories-49912e26c8122bb4)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/fixture_reads.rs ($CARGO_TARGET_DIR/debug/deps/integration_fixture_reads-f992725a1bd4ec37)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.31s
     Running tests/integration/fs_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_fs_conformance-4c054563d204acfb)
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.27s
     Running tests/integration/hierarchy_guard.rs ($CARGO_TARGET_DIR/debug/deps/integration_hierarchy_guard-3efdbaf82d970047)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/integration/independent_recovery.rs ($CARGO_TARGET_DIR/debug/deps/integration_independent_recovery-cff125f4c379a44f)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.15s
     Running tests/integration/logical_file.rs ($CARGO_TARGET_DIR/debug/deps/integration_logical_file-c870615487e03f53)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.33s
     Running tests/integration/memory_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_memory_conformance-bbed81d294b9c269)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.23s
     Running tests/integration/opaque_bytes.rs ($CARGO_TARGET_DIR/debug/deps/integration_opaque_bytes-1ba5398c372ee9d8)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.19s
     Running tests/integration/read_objects.rs ($CARGO_TARGET_DIR/debug/deps/integration_read_objects-777c46e5ca575e3e)
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
     Running tests/integration/validate_objects.rs ($CARGO_TARGET_DIR/debug/deps/integration_validate_objects-f628c102e60642ab)
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s
     Running tests/integration/version_inventory_repair.rs ($CARGO_TARGET_DIR/debug/deps/integration_version_inventory_repair-ec3ea182e8864a75)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
     Running tests/integration/view_api.rs ($CARGO_TARGET_DIR/debug/deps/integration_view_api-2f58d1662313bbd6)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.79s
     Running tests/integration/write_order.rs ($CARGO_TARGET_DIR/debug/deps/integration_write_order-22dd8487357674b8)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.31s
     Running tests/unit/digest.rs ($CARGO_TARGET_DIR/debug/deps/unit_digest-84296ab96d185c1f)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/fixity.rs ($CARGO_TARGET_DIR/debug/deps/unit_fixity-595c41ef01df1b6c)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/git_guard.rs ($CARGO_TARGET_DIR/debug/deps/unit_git_guard-dfd4d18dbb6d3248)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/unit/inventory.rs ($CARGO_TARGET_DIR/debug/deps/unit_inventory-73e97bdd05e073f7)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/unit/lease.rs ($CARGO_TARGET_DIR/debug/deps/unit_lease-196772f0f0361667)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/namaste_sidecar.rs ($CARGO_TARGET_DIR/debug/deps/unit_namaste_sidecar-f7f54c2c35c2fac9)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/read_state.rs ($CARGO_TARGET_DIR/debug/deps/unit_read_state-5af78938f9ad7799)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/source_scan.rs ($CARGO_TARGET_DIR/debug/deps/unit_source_scan-b499895a04c6e23e)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/storage_key.rs ($CARGO_TARGET_DIR/debug/deps/unit_storage_key-f318c2486ba91397)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo build -p filesquid-ocfl --locked --no-default-features --target wasm32-unknown-unknown`:

```text
    Finished `dev` profile [unoptimized] target(s) in 4.93s
```

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool. Both `LeaseStore` outcomes (fresh or released record: no takeover; unreleased expired record: takeover) and both receipts' sources are tested; the repair receipt copies the same flag and is covered by the existing repair tests for its `false` case.

## Sync Execution Report

- **Module SRS:** OCFL-S-018 requires every commit receipt to report a takeover and every reservation store to report it for each reservation it grants.
- **Module plan:** Status note on IO-0.6 naming API-F2.7 as the consumer and this Walkthrough. No new task row, so the checklist and MVP progress are unchanged.
- **Crate README:** "Write" describes `took_over`.
- **PRD, root SRS, ADRs:** no change.

## Mocked Environment Variables

None.

## Notes for callers

- Hosts call API-F2.7's `OcflValidation::validate_after_commit(library, object_id, receipt.took_over)` after each commit once both changes are on `main` (API-F1.3 file routes, DE-1.1).
- A `VersionReservations` implementation over another store (for example a PostgreSQL generation) SHOULD mark its takeovers with `Reservation::with_takeover`.
- When a change-model commit rebases, only the last attempt's receipt is returned; an attempt refused for a moved head publishes nothing, and the writer that moved the head got its own receipt.
