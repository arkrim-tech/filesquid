# Task IO takeover flag Walkthrough: commit receipts report reservation takeovers

Module plan row: [IO-0.6](../plan.md) (Status note: follow-up). Owner decision 2026-10-19, option A of the API-F2.7 Conflict Artifact (`apps/filesquid/crates/api/resources/tasks/Task-API-F2.7-Conflict.md`, PR #74). Consumer: [API-F2.7](../../../api/resources/plan.md) post-commit validation ([API-S-022](../../../api/resources/srs.md)).

## OCFL-S-018: Write order and unpublished versions — every commit receipt reports whether its reservation took over one that was still held

## OCFL-S-023: Takeover reporting — commit and repair receipts, and every reservation store, report reservation takeovers

## OCFL-S-020: Version-inventory repair — the repair receipt reports the same

Sources: [OCFL-S-018, OCFL-S-020, OCFL-S-023](../srs.md), [API-S-022](../../../api/resources/srs.md), [ADR 007](../../../../../../resources/architecture/007-durable-jobs-and-interactive-performance.md), [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md), [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md).

## What changed

- `Reservation::took_over() -> bool` and `Reservation::with_takeover(self) -> Self` (additive; `Reservation::new` is unchanged and starts `false`). The `VersionReservations::reserve` contract now says the returned reservation reports a takeover.
- `LeaseStore` marks the reservation it grants as a takeover when it replaces a lease record that was not released (it can only replace a live record once it has expired, so this is exactly the expired-or-reassigned case). A released or absent record is not a takeover. The existing `tracing::warn!` ("took over an expired version reservation") is unchanged.
- `CommitReceipt::took_over` and `RepairReceipt::took_over` (additive public fields) copy the flag from the reservation the commit or repair published under. `ChangeModel::commit`, `revert`, `reinstate` and the view writes return the same `CommitReceipt`, so they carry it too.
- No change to `filesquid-api` (wiring follows API-F2.7 on `main`). **Compatibility:** the two methods are additive, but a new public field on `CommitReceipt` and `RepairReceipt` (neither is `#[non_exhaustive]`) is a source-breaking change for code outside the crate that builds them with a struct literal or destructures them exhaustively. The crate is unpublished (`publish = false`) and no workspace crate does either (`cargo clippy --workspace` passes), so nothing breaks today; the published API is versioned later (IO-D.1), which decides `#[non_exhaustive]` for result types.
- New module SRS row OCFL-S-023 (OCFL-S-018 points to it), crate README "Write" and its repair section, and the IO-0.6 plan row (Status note naming API-F2.7) are updated.

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

`cargo test -p filesquid-ocfl --locked --test unit_lease`:

```text
    Finished `test` profile [unoptimized] target(s) in 4.60s
     Running tests/unit/lease.rs ($CARGO_TARGET_DIR/debug/deps/unit_lease-196772f0f0361667)

running 19 tests
test a_clock_that_cannot_tell_the_time_fails_closed ... ok
test a_corrupt_lease_record_is_reported ... ok
test a_lease_within_its_safety_margin_fails_its_fence_check ... ok
test a_held_lease_refuses_every_other_reservation ... ok
test a_margin_of_half_the_ttl_or_more_is_refused ... ok
test a_new_reservation_is_not_a_takeover_unless_marked ... ok
test a_record_at_the_largest_token_is_not_wrapped ... ok
test a_released_lease_is_lost_and_its_token_keeps_growing ... ok
test a_record_too_large_to_read_back_is_not_written ... ok
test a_reservation_names_its_object_version_holder_and_first_token ... ok
test an_expired_lease_cannot_be_released ... ok
test a_reservation_reports_whether_it_took_over_an_unreleased_lease ... ok
test an_expired_lease_fails_its_fence_check_even_before_takeover ... ok
test an_expired_lease_is_taken_over_with_a_higher_token ... ok
test objects_are_reserved_independently ... ok
test rfc3339_utc_formats_unix_milliseconds ... ok
test releasing_a_lost_reservation_is_refused_and_changes_nothing ... ok
test confirming_renews_a_lease_past_half_its_ttl ... ok
test lease_records_live_under_the_store_prefix ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

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

`cargo test -p filesquid-ocfl --locked --test integration_version_inventory_repair`:

```text
    Finished `test` profile [unoptimized] target(s) in 1.49s
     Running tests/integration/version_inventory_repair.rs ($CARGO_TARGET_DIR/debug/deps/integration_version_inventory_repair-ec3ea182e8864a75)

running 15 tests
test a_head_that_already_has_its_version_inventory_is_refused ... ok
test a_head_with_an_unlisted_file_is_refused ... ok
test a_head_version_inventory_that_differs_from_the_root_is_refused ... ok
test a_lost_reservation_stops_the_repair_before_its_next_write ... ok
test a_repair_reports_whether_it_took_over_a_lapsed_reservation ... ok
test a_reservation_held_by_another_writer_refuses_the_repair ... ok
test an_interrupted_repair_on_damaged_content_is_refused ... ok
test an_interrupted_repair_is_completed_by_running_it_again ... ok
test an_interrupted_repair_whose_root_sidecar_does_not_match_is_refused ... ok
test a_repaired_sha256_object_becomes_editable ... ok
test opening_reading_and_validating_never_repair ... ok
test an_object_whose_root_sidecar_does_not_match_is_refused ... ok
test the_w010_fixture_is_repaired_and_stays_closed_to_commits_as_sha512 ... ok
test an_interrupted_repair_with_another_unlisted_file_is_refused ... ok
test an_object_with_a_validator_error_is_refused ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s

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
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s
     Running tests/integration/content_fixtures.rs ($CARGO_TARGET_DIR/debug/deps/integration_content_fixtures-2450cd9c4beef973)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.71s
     Running tests/integration/enumerate_state.rs ($CARGO_TARGET_DIR/debug/deps/integration_enumerate_state-c992207f0a36f4c4)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.74s
     Running tests/integration/fixture_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_fixture_conformance-6188b5d43aaacb5e)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
     Running tests/integration/fixture_inventories.rs ($CARGO_TARGET_DIR/debug/deps/integration_fixture_inventories-49912e26c8122bb4)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/fixture_reads.rs ($CARGO_TARGET_DIR/debug/deps/integration_fixture_reads-f992725a1bd4ec37)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.18s
     Running tests/integration/fs_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_fs_conformance-4c054563d204acfb)
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.17s
     Running tests/integration/hierarchy_guard.rs ($CARGO_TARGET_DIR/debug/deps/integration_hierarchy_guard-3efdbaf82d970047)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/integration/independent_recovery.rs ($CARGO_TARGET_DIR/debug/deps/integration_independent_recovery-cff125f4c379a44f)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.13s
     Running tests/integration/logical_file.rs ($CARGO_TARGET_DIR/debug/deps/integration_logical_file-c870615487e03f53)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.07s
     Running tests/integration/memory_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_memory_conformance-bbed81d294b9c269)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.29s
     Running tests/integration/opaque_bytes.rs ($CARGO_TARGET_DIR/debug/deps/integration_opaque_bytes-1ba5398c372ee9d8)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.71s
     Running tests/integration/read_objects.rs ($CARGO_TARGET_DIR/debug/deps/integration_read_objects-777c46e5ca575e3e)
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
     Running tests/integration/validate_objects.rs ($CARGO_TARGET_DIR/debug/deps/integration_validate_objects-f628c102e60642ab)
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s
     Running tests/integration/version_inventory_repair.rs ($CARGO_TARGET_DIR/debug/deps/integration_version_inventory_repair-ec3ea182e8864a75)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
     Running tests/integration/view_api.rs ($CARGO_TARGET_DIR/debug/deps/integration_view_api-2f58d1662313bbd6)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.70s
     Running tests/integration/write_order.rs ($CARGO_TARGET_DIR/debug/deps/integration_write_order-22dd8487357674b8)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
     Running tests/unit/digest.rs ($CARGO_TARGET_DIR/debug/deps/unit_digest-84296ab96d185c1f)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/fixity.rs ($CARGO_TARGET_DIR/debug/deps/unit_fixity-595c41ef01df1b6c)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/git_guard.rs ($CARGO_TARGET_DIR/debug/deps/unit_git_guard-dfd4d18dbb6d3248)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
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

## Cubic round 1 fixes

- Takeover reporting has its own requirement, OCFL-S-023; the README and `Reservation::with_takeover` rustdoc say every reservation store MUST mark takeovers.
- README repair section lists `took_over`; repair receipts are tested in both cases.
- The compatibility note states that the new public fields are source-breaking for external struct literals and exhaustive patterns.
- The unit transcript runs the whole `unit_lease` target, so both new tests appear.

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool. Both `LeaseStore` outcomes (fresh or released record: no takeover; unreleased expired record: takeover) are tested, and both receipts are tested in both cases: `a_commit_reports_whether_it_took_over_a_lapsed_reservation` and the crash-recovery tests for commits; `the_w010_fixture_is_repaired_and_stays_closed_to_commits_as_sha512` (false) and `a_repair_reports_whether_it_took_over_a_lapsed_reservation` (true) for repairs.

## Sync Execution Report

- **Module SRS:** new OCFL-S-023 (Takeover reporting) requires commit and repair receipts and every reservation store to report takeovers; OCFL-S-018 refers to it.
- **Module plan:** Status note on IO-0.6 naming API-F2.7 as the consumer and this Walkthrough. No new task row, so the checklist and MVP progress are unchanged.
- **Crate README:** "Write" describes `took_over`.
- **PRD, root SRS, ADRs:** no change.

## Mocked Environment Variables

None.

## Notes for callers

- Hosts call API-F2.7's `OcflValidation::validate_after_commit(library, object_id, receipt.took_over)` after each commit once both changes are on `main` (API-F1.3 file routes, DE-1.1).
- A `VersionReservations` implementation over another store (for example a PostgreSQL generation) SHOULD mark its takeovers with `Reservation::with_takeover`.
- When a change-model commit rebases, only the last attempt's receipt is returned; an attempt refused for a moved head publishes nothing, and the writer that moved the head got its own receipt.
