# Task IO-1.1 Walkthrough: Ordinary-tool copy and independent recovery fixtures

Module plan row: [IO-1.1](../plan.md). Global task: [Task 1.1](../../../../../../resources/implementation-plan.md#task-1.1) (stays open: CL-1.1, DE-1.1, UI-1.1, API-F2.7 and UI-F2.14 remain).

## OCFL-S-022: Independent recovery

## OCFL-S-019: Inspection and copy never edit an object

## OCFL-S-020: Version-inventory repair (owner decision: a head without a version inventory stays refused, repaired only by the IO-1.4 flow)

Sources: [OCFL-S-022, OCFL-S-019, OCFL-S-020](../srs.md), [PRD F-001](../../../../../../resources/prd.md#f-001-independent-local-library-and-ownership), [PRD F-042](../../../../../../resources/prd.md#f-042-ocfl-preservation-and-portable-recovery), [PRD RC-010](../../../../../../resources/prd.md#rc-010-complete-ocfl-preservation-and-independent-recovery), [F-001 feature](../../../../ui/tests/features/independent_local_library_and_ownership_F-001.feature).

## What changed

- **Tests, fixtures and docs only.** No write-path change and no change-model API (IO-0.10 runs on another branch). The engine already behaved as required, so the new tests passed on first run; they pin that behaviour. They were not seen to fail first (no engine code was missing).
- **New requirement row OCFL-S-022** in the crate SRS: copy with ordinary tools, independent reading, no change by inspection, detection of altered originals and inventories, and W010 stays refused.
- **`tests/support/recovery.rs`**: builds a completed snapshot with the engine's writer on a filesystem root (sha256 object with two versions, photos, `.xmp` sidecars, a note; a sha512 object), `ordinary_copy` (`std::fs` only), tree snapshots, a tamper toolkit and an **independent reader** that uses only `serde_json` and `sha2` to recover every logical file, folder tree and sidecar pair.
- **`tests/integration/independent_recovery.rs`** (10 tests) and **`independent_recovery_OCFL-S-022.feature`** (8 scenarios, steps in `tests/support/recovery_steps.rs`).
- Docs: crate README "Independent recovery", root README "Run tests", plan and checklist ticks, MVP progress bar.

## Constraint gates

| Constraint | Gate |
|---|---|
| (a) copy opens, validates, enumerates like the source; content matches digests | `a_copy_made_with_ordinary_tools_opens_validates_and_enumerates_like_the_source`; BDD "The copy opens, validates and enumerates like the source" |
| (a) copy holds only OCFL files, no database, staging or Git | `the_copy_holds_only_ocfl_files_no_database_staging_or_git` |
| (b) independent reader rebuilds files, folders, sidecar pairs | `an_independent_reader_rebuilds_files_folders_and_sidecar_pairs_from_the_copy`; BDD "An independent reader rebuilds files, folders and sidecar pairs" |
| (c) inspection changes neither copy nor source | `inspecting_the_copy_changes_neither_the_copy_nor_the_source`; BDD "Inspecting the copy changes nothing" |
| (d) changed original: E092 and verified read stops | `a_changed_original_is_reported_as_a_digest_error_and_never_served`; BDD "A changed preserved original is detected" |
| (d) changed root inventory, sidecar untouched: E060, open refused | `a_changed_root_inventory_with_its_sidecar_untouched_is_refused_on_open`; BDD |
| (d) forged inventory with recomputed sidecar: E092 | `a_changed_inventory_with_a_recomputed_sidecar_is_still_detected`; BDD |
| (d) changed earlier-version inventory: E060 | `a_changed_earlier_version_inventory_is_detected`; BDD |
| W010 copy stays refused for commits, never repaired | `a_copy_without_a_head_version_inventory_stays_refused_and_is_not_repaired`; BDD "A copy without a head version inventory stays refused" |
| Community validator accepts the copy, rejects a tampered copy | `a_community_validator_accepts_the_copy_and_rejects_a_tampered_one` (passes with no assertions when no validator is on `PATH`; here `ocfl-validate.py` was installed, and a run with `FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR=1`, which fails when none is found, passed: see below) |

## Mocked Environment Variables

None.

## Community validator proof

```text
$ FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR=1 cargo test -p filesquid-ocfl --test integration_independent_recovery --locked a_community
test a_community_validator_accepts_the_copy_and_rejects_a_tampered_one ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Pre-commit on the head commit

`FULL=1 bash scripts/pre-commit.sh` after merging `origin/main` (which carries the `fetch_update` clippy fix):

```text
✅ All Pre-Commit Hooks Passed! Test scope: full (334s)
```

## Test output

`cargo test -p filesquid-ocfl --locked` (exit 0), per-target summary:

```text
     Running unittests src/lib.rs (/home/user/.cargo-target-shared/debug/deps/filesquid_ocfl-336d9c2a83f05f02)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.99s
     Running tests/features.rs (/home/user/.cargo-target-shared/debug/deps/features-9009fa7eceb51bdd)
136 scenarios (133 passed, 3 skipped)
623 steps (620 passed, 3 skipped)
     Running tests/integration/content_fixtures.rs (/home/user/.cargo-target-shared/debug/deps/integration_content_fixtures-2450cd9c4beef973)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.13s
     Running tests/integration/enumerate_state.rs (/home/user/.cargo-target-shared/debug/deps/integration_enumerate_state-c992207f0a36f4c4)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.09s
     Running tests/integration/fixture_conformance.rs (/home/user/.cargo-target-shared/debug/deps/integration_fixture_conformance-6188b5d43aaacb5e)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/integration/fixture_inventories.rs (/home/user/.cargo-target-shared/debug/deps/integration_fixture_inventories-49912e26c8122bb4)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/fixture_reads.rs (/home/user/.cargo-target-shared/debug/deps/integration_fixture_reads-f992725a1bd4ec37)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
     Running tests/integration/fs_conformance.rs (/home/user/.cargo-target-shared/debug/deps/integration_fs_conformance-4c054563d204acfb)
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.78s
     Running tests/integration/hierarchy_guard.rs (/home/user/.cargo-target-shared/debug/deps/integration_hierarchy_guard-3efdbaf82d970047)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/integration/independent_recovery.rs (/home/user/.cargo-target-shared/debug/deps/integration_independent_recovery-cff125f4c379a44f)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.98s
     Running tests/integration/logical_file.rs (/home/user/.cargo-target-shared/debug/deps/integration_logical_file-c870615487e03f53)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.27s
     Running tests/integration/memory_conformance.rs (/home/user/.cargo-target-shared/debug/deps/integration_memory_conformance-bbed81d294b9c269)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.60s
     Running tests/integration/opaque_bytes.rs (/home/user/.cargo-target-shared/debug/deps/integration_opaque_bytes-1ba5398c372ee9d8)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.20s
     Running tests/integration/read_objects.rs (/home/user/.cargo-target-shared/debug/deps/integration_read_objects-777c46e5ca575e3e)
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
     Running tests/integration/validate_objects.rs (/home/user/.cargo-target-shared/debug/deps/integration_validate_objects-f628c102e60642ab)
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.34s
     Running tests/integration/version_inventory_repair.rs (/home/user/.cargo-target-shared/debug/deps/integration_version_inventory_repair-ec3ea182e8864a75)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     Running tests/integration/write_order.rs (/home/user/.cargo-target-shared/debug/deps/integration_write_order-22dd8487357674b8)
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
     Running tests/unit/digest.rs (/home/user/.cargo-target-shared/debug/deps/unit_digest-84296ab96d185c1f)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/fixity.rs (/home/user/.cargo-target-shared/debug/deps/unit_fixity-595c41ef01df1b6c)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/git_guard.rs (/home/user/.cargo-target-shared/debug/deps/unit_git_guard-dfd4d18dbb6d3248)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/unit/inventory.rs (/home/user/.cargo-target-shared/debug/deps/unit_inventory-73e97bdd05e073f7)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/unit/lease.rs (/home/user/.cargo-target-shared/debug/deps/unit_lease-196772f0f0361667)
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/namaste_sidecar.rs (/home/user/.cargo-target-shared/debug/deps/unit_namaste_sidecar-f7f54c2c35c2fac9)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/read_state.rs (/home/user/.cargo-target-shared/debug/deps/unit_read_state-5af78938f9ad7799)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/source_scan.rs (/home/user/.cargo-target-shared/debug/deps/unit_source_scan-b499895a04c6e23e)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/storage_key.rs (/home/user/.cargo-target-shared/debug/deps/unit_storage_key-f318c2486ba91397)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The new integration target:

```text
Running tests/integration/independent_recovery.rs (/home/user/.cargo-target-shared/debug/deps/integration_independent_recovery-cff125f4c379a44f)

running 10 tests
test a_changed_earlier_version_inventory_is_detected ... ok
test a_changed_inventory_with_a_recomputed_sidecar_is_still_detected ... ok
test a_changed_root_inventory_with_its_sidecar_untouched_is_refused_on_open ... ok
test a_changed_original_is_reported_as_a_digest_error_and_never_served ... ok
test an_independent_reader_rebuilds_files_folders_and_sidecar_pairs_from_the_copy ... ok
test a_copy_without_a_head_version_inventory_stays_refused_and_is_not_repaired ... ok
test a_copy_made_with_ordinary_tools_opens_validates_and_enumerates_like_the_source ... ok
test the_copy_holds_only_ocfl_files_no_database_staging_or_git ... ok
test inspecting_the_copy_changes_neither_the_copy_nor_the_source ... ok
test a_community_validator_accepts_the_copy_and_rejects_a_tampered_one ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.98s

```

The new BDD feature:

```text
Feature: Independent recovery of a completed snapshot
  Scenario: The copy opens, validates and enumerates like the source
   ✔> Given a completed snapshot written by the engine
   ✔> And I copy the storage root with ordinary filesystem tools
   ✔  When the engine opens, validates and enumerates the copy
   ✔  Then every object of the copy should validate with zero errors
   ✔  And the copy should enumerate the same files as the source
   ✔  And the copy should hold only OCFL files
  Scenario: An independent reader rebuilds files, folders and sidecar pairs
   ✔> Given a completed snapshot written by the engine
   ✔> And I copy the storage root with ordinary filesystem tools
   ✔  When an independent reader recovers the copy with only a JSON parser and SHA-2
   ✔  Then every logical file of every version should be recovered with its original bytes
   ✔  And the folders and the sidecar pairs of "v2" should be rebuilt
  Scenario: Inspecting the copy changes nothing
   ✔> Given a completed snapshot written by the engine
   ✔> And I copy the storage root with ordinary filesystem tools
   ✔  When the engine opens, validates and enumerates the copy
   ✔  Then the copy should be byte-identical to before
   ✔  And the source should be byte-identical to before
  Scenario: A changed preserved original is detected
   ✔> Given a completed snapshot written by the engine
   ✔> And I copy the storage root with ordinary filesystem tools
   ✔  Given a stored byte of "IMG_0001.jpg" in the copy is changed
   ✔  When the engine opens, validates and enumerates the copy
   ✔  Then validation should report "E092"
   ✔  And streaming "IMG_0001.jpg" of "v1" should stop with "E092"
   ✔  And the independent reader should refuse the copy as a digest mismatch
  Scenario: A changed root inventory is refused when its sidecar is untouched
   ✔> Given a completed snapshot written by the engine
   ✔> And I copy the storage root with ordinary filesystem tools
   ✔  Given the root inventory in the copy is edited
   ✔  When the engine opens, validates and enumerates the copy
   ✔  Then validation should report "E060"
   ✔  And opening the object should be refused with "E060"
  Scenario: A changed inventory is detected even with a recomputed sidecar
   ✔> Given a completed snapshot written by the engine
   ✔> And I copy the storage root with ordinary filesystem tools
   ✔  Given the root inventory and its sidecar in the copy are forged together
   ✔  When the engine opens, validates and enumerates the copy
   ✔  Then validation should report "E092"
   ✔  And the independent reader should refuse the copy as a digest mismatch
  Scenario: A changed earlier version inventory is detected
   ✔> Given a completed snapshot written by the engine
   ✔> And I copy the storage root with ordinary filesystem tools
   ✔  Given the version 1 inventory in the copy is edited
   ✔  When the engine opens, validates and enumerates the copy
   ✔  Then validation should report "E060"
  Scenario: A copy without a head version inventory stays refused
   ✔> Given a completed snapshot written by the engine
   ✔> And I copy the storage root with ordinary filesystem tools
   ✔  Given the head version inventory is missing from the copy
   ✔  When the engine opens, validates and enumerates the copy
   ✔  Then validation should warn "W010" with no errors
   ✔  And a commit on the copy should be refused for the missing version inventory
   ✔  And the copy should be byte-identical to before
```
