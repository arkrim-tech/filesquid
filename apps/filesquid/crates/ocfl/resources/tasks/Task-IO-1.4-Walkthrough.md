# Task IO-1.4 Walkthrough: Version-inventory repair for heads without one (W010)

Branch `feat/io-1.4-w010-inventory-repair`. Module task [IO-1.4](../plan.md) of global Task 1.1 ([implementation plan](../../../../../../resources/implementation-plan.md)).

## OCFL-S-020: Version-inventory repair (a consented, one-time repair gives a committed head without a version inventory, W010, a byte-identical copy of the root inventory and root sidecar under the same fenced reservation as a commit; every refusal check runs before any write, also when resuming; an interrupted repair leaves the logical state unchanged and is completed by running it again; no version is published)

`ObjectWriter::repair_version_inventory(root)` implements it. Tests: `integration_version_inventory_repair` (14 tests, each on MemoryBackend and FsBackend) and `version_inventory_repair_OCFL-S-020.feature` (14 scenarios).

## OCFL-S-018: Write order and unpublished versions (a committed head without its own version inventory MUST NOT be the base of a commit until the OCFL-S-020 repair lifts the refusal)

The writer still refuses a W010 head (`CommitError::MissingVersionInventory`), and now also refuses a head whose version inventory has no sidecar, so a half-finished repair is completed before that head becomes an older version. After a repair a sha256 object commits; the repaired sha512 fixture is refused with `UnsupportedAlgorithm` and writes nothing (OCFL-S-002).

## OCFL-S-019: Inspection and copy never edit an object (the version-inventory repair is the one exception)

Opening, reading and validating the W010 fixture write nothing (`opening_reading_and_validating_never_repair`, BDD "Opening, reading and validating a W010 object never repairs it"). The engine has no code path that calls the repair.

## SRS-F-042: OCFL preservation and portable recovery

Objects written by other tools without head version inventories become editable without any change to their content or history.

## SRS-NFR-001: Preservation and transactional integrity

No refusal decided by the checks before the first write writes a byte. A failure between the two writes (a crash, a lost reservation, a backend error) can leave the version inventory without its sidecar; that leaves the logical state unchanged, the writer refuses the head until it is completed, and running the repair again completes it.

## Specification check

Before any code, the OCFL 1.1 specification (https://ocfl.io/1.1/spec/) was read for this change: §3.7 "every version directory SHOULD include an inventory file" and "the inventory file in the OCFL Object Root MUST be the same as the file in the most recent version"; §3.6 "Every occurrence of an inventory file MUST have an accompanying sidecar file"; §3.3 "Once created, the contents of a version directory are expected to be immutable" (no RFC 2119 keyword). Adding byte-identical copies of the root inventory and root sidecar to the head version directory meets §3.6 and §3.7 and breaks no MUST, so no Conflict Artifact was needed.

## What changed

- `src/write/repair.rs` (new): `ObjectWriter::repair_version_inventory` and `RepairReceipt` (object id, version, digest algorithm, inventory digest, paths written). Steps: read the root inventory and require its sidecar to match; reserve the object (`VersionReservations::reserve`, as a commit does); under the reservation re-read the root files (refuse a change), run the Git and hierarchy guards, classify the head version directory, run the full validator; write what is missing of `v<head>/inventory.json` and then `v<head>/inventory.json.<algorithm>` with `IfAbsent` and a fence check (`confirm`) before each; release the reservation unless it was lost.
- `src/write/error.rs`: `RepairError` (`Reservation`, `RootSidecarMismatch`, `RootChanged`, `Invalid`, `NotRepairable`, `NothingToRepair`, `ForeignWrite`, `Guard`, `Read`, `Storage`).
- `src/write/base.rs`: `require_head_inventory` also requires the head version sidecar, and (review round 1) reads both head files: a version inventory that is not byte-identical to the committed inventory, or a sidecar that does not match it, is refused with the new `CommitError::VersionInventoryMismatch` (test `a_head_version_inventory_that_does_not_match_is_refused`).
- `src/write/listing.rs`: `list_version_keys` returns `OcflError` so both the commit and the repair path can use it; `src/write/commit.rs`: `ObjectWriter` fields are visible to the sibling repair module.
- `src/write/mod.rs`, `src/lib.rs`: exports and module docs.
- Tests: `tests/integration/version_inventory_repair.rs`, `tests/features/version_inventory_repair_OCFL-S-020.feature`, `tests/support/repair_steps.rs`, rig helpers `try_repair`/`repair` in `tests/support/write_rig.rs`.
- Docs: crate README "Version-inventory repair (W010)"; module SRS OCFL-S-020 clarified (see below); module plan row and the root checklist ticked; MVP progress bar redrawn.

### Interpretation recorded in the SRS

OCFL-S-020 says the interrupted shape (head inventory identical, "no version sidecar or one byte-identical to the root sidecar") is resumed "by writing only what is missing", while acceptance criterion (2) requires refusing "an object whose head already has a version inventory". An engine-written object has both files byte-identical to the root files, so the two read together mean: a head with the inventory but no sidecar is resumed (sidecar only); a head with both has nothing missing and is refused as `NothingToRepair` without writing. The SRS row now says so, and also that the writer refuses a head whose version sidecar is still missing.

## How refusals and crashes are tested

- Refusals (each asserts no storage write and an unchanged key/entity-tag snapshot): an engine-written head (`NothingToRepair`), a tampered content file (`Invalid` with E092), a root sidecar that does not match (`RootSidecarMismatch`), an unlisted file in the head (`NotRepairable`), a head inventory that differs from the root inventory (`NotRepairable`), a reservation held by another writer (`Held`).
- Lost reservation: a hook hands the lease to another writer right after `v2/inventory.json` is written; the repair stops with `Lost` before the sidecar.
- Crash: the fault backend never completes the write of `v2/inventory.json.sha256`, so the writer is dropped with only the version inventory written. Afterwards the logical state is unchanged, the validator reports only E058 for the missing version sidecar, a commit is refused, and a second run writes only the sidecar; the object then validates and commits `v3`. The same partial state plus a tampered content file, a mismatching root sidecar or another unlisted file is refused without writing.

## TDD: failing run before implementation

The tests were written first; before `src/write/repair.rs` existed they did not compile:

```text
|                  ^^^^^^^^^^^ no `RepairError` in `write`

error[E0432]: unresolved imports `filesquid_ocfl::write::RepairError`, `filesquid_ocfl::write::RepairReceipt`
  --> crates/ocfl/tests/integration/../support/write_rig.rs:19:5
   |
19 |     RepairError, RepairReceipt, VersionDraft,
   |     ^^^^^^^^^^^  ^^^^^^^^^^^^^ no `RepairReceipt` in `write`
   |     |
   |     no `RepairError` in `write`

error[E0425]: cannot find type `RepairReceipt` in module `filesquid_ocfl::write`
   --> crates/ocfl/tests/integration/version_inventory_repair.rs:127:43
    |
127 |     result: Result<filesquid_ocfl::write::RepairReceipt, RepairError>,
    |                                           ^^^^^^^^^^^^^ not found in `filesquid_ocfl::write`

error[E0599]: no method named `repair_version_inventory` found for struct `ObjectWriter<'a, B, R>` in the current scope
   --> crates/ocfl/tests/integration/../support/write_rig.rs:163:37
    |
163 |         complete_immediately(writer.repair_version_inventory(ROOT))
    |                                     ^^^^^^^^^^^^^^^^^^^^^^^^ method not found in `ObjectWriter<'_, FaultBackend<B>, LeaseStore<FaultBackend<MemoryBackend>, ManualClock>>`

Some errors have detailed explanations: E0425, E0432, E0599.
For more information about an error, try `rustc --explain E0425`.
error: could not compile `filesquid-ocfl` (test "integration_version_inventory_repair") due to 4 previous errors
```

## Raw test output

`cargo test -p filesquid-ocfl --locked --test integration_version_inventory_repair`:

```text
running 14 tests
test a_head_that_already_has_its_version_inventory_is_refused ... ok
test a_head_with_an_unlisted_file_is_refused ... ok
test a_lost_reservation_stops_the_repair_before_its_next_write ... ok
test a_head_version_inventory_that_differs_from_the_root_is_refused ... ok
test a_reservation_held_by_another_writer_refuses_the_repair ... ok
test an_interrupted_repair_on_damaged_content_is_refused ... ok
test a_repaired_sha256_object_becomes_editable ... ok
test an_interrupted_repair_whose_root_sidecar_does_not_match_is_refused ... ok
test an_interrupted_repair_is_completed_by_running_it_again ... ok
test opening_reading_and_validating_never_repair ... ok
test an_object_whose_root_sidecar_does_not_match_is_refused ... ok
test an_interrupted_repair_with_another_unlisted_file_is_refused ... ok
test an_object_with_a_validator_error_is_refused ... ok
test the_w010_fixture_is_repaired_and_stays_closed_to_commits_as_sha512 ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
```

`cargo test -p filesquid-ocfl --locked --test features -- --name repair`:

```text
Feature: Version-inventory repair for heads without one
  Scenario: The pinned W010 fixture is repaired and stays closed to commits as sha512
   ✔  Given a copy of the pinned W010 fixture object
   ✔  And a commit on it is refused for the missing version inventory
   ✔  When the version-inventory repair runs
   ✔  Then the repair should have written "v1/inventory.json" and "v1/inventory.json.sha512"
   ✔  And the repaired head files should be byte-identical to the root inventory and root sidecar
   ✔  And no other file of the object should have changed
   ✔  And the repaired object should validate with zero errors and without W010
   ✔  And a commit on the repaired object should be refused as not sha256 without writing
  Scenario: A repaired sha256 object becomes editable
   ✔  Given a sha256 object committed at "v2" whose head version inventory was removed
   ✔  When the version-inventory repair runs
   ✔  Then the repair should have written "v2/inventory.json" and "v2/inventory.json.sha256"
   ✔  And the logical state should be unchanged by the repair
   ✔  And the repaired object should validate with zero errors and without W010
   ✔  And a commit of "v3" on the repaired object should succeed
  Scenario Outline: The repair refuses an object it must not touch
   ✔  Given a sha256 object committed at "v2" whose head version inventory was removed
   ✔  And the object was then changed by restoring the head version inventory
   ✔  When the version-inventory repair runs
   ✔  Then the repair should be refused as nothing to repair
   ✔  And the repair should have written nothing
  Scenario Outline: The repair refuses an object it must not touch
   ✔  Given a sha256 object committed at "v2" whose head version inventory was removed
   ✔  And the object was then changed by a content file whose digest no longer matches
   ✔  When the version-inventory repair runs
   ✔  Then the repair should be refused as invalid
   ✔  And the repair should have written nothing
  Scenario Outline: The repair refuses an object it must not touch
   ✔  Given a sha256 object committed at "v2" whose head version inventory was removed
   ✔  And the object was then changed by a root sidecar that does not match
   ✔  When the version-inventory repair runs
   ✔  Then the repair should be refused as a root sidecar mismatch
   ✔  And the repair should have written nothing
  Scenario Outline: The repair refuses an object it must not touch
   ✔  Given a sha256 object committed at "v2" whose head version inventory was removed
   ✔  And the object was then changed by an unlisted file in the head version directory
   ✔  When the version-inventory repair runs
   ✔  Then the repair should be refused as not repairable
   ✔  And the repair should have written nothing
  Scenario Outline: The repair refuses an object it must not touch
   ✔  Given a sha256 object committed at "v2" whose head version inventory was removed
   ✔  And the object was then changed by a head version inventory that differs
   ✔  When the version-inventory repair runs
   ✔  Then the repair should be refused as not repairable
   ✔  And the repair should have written nothing
  Scenario Outline: The repair refuses an object it must not touch
   ✔  Given a sha256 object committed at "v2" whose head version inventory was removed
   ✔  And the object was then changed by a reservation held by another writer
   ✔  When the version-inventory repair runs
   ✔  Then the repair should be refused as a held reservation
   ✔  And the repair should have written nothing
  Scenario: A repair whose reservation is lost writes nothing more
   ✔  Given a sha256 object committed at "v2" whose head version inventory was removed
   ✔  And the reservation passes to another writer after "obj/v2/inventory.json" is written
   ✔  When the version-inventory repair runs
   ✔  Then the repair should be refused as a lost reservation
   ✔  And "obj/v2/inventory.json.sha256" should not be in the repaired object
   ✔  And the logical state should be unchanged by the repair
  Scenario: An interrupted repair is completed by running it again
   ✔  Given a sha256 object committed at "v2" whose head version inventory was removed
   ✔  And a repair crashed before writing "obj/v2/inventory.json.sha256"
   ✔  Then the logical state should be unchanged by the repair
   ✔  And a commit of "v3" on the repaired object should be refused for the missing version inventory
   ✔  When the version-inventory repair runs again
   ✔  Then the repair should have written only "v2/inventory.json.sha256"
   ✔  And the repaired object should validate with zero errors and without W010
   ✔  And a commit of "v3" on the repaired object should succeed
  Scenario Outline: An interrupted repair on a damaged object is refused
   ✔  Given a sha256 object committed at "v2" whose head version inventory was removed
   ✔  And a repair crashed before writing "obj/v2/inventory.json.sha256"
   ✔  And the object was then changed by a content file whose digest no longer matches
   ✔  When the version-inventory repair runs again
   ✔  Then the repair should be refused as invalid
   ✔  And the repair should have written nothing
  Scenario Outline: An interrupted repair on a damaged object is refused
   ✔  Given a sha256 object committed at "v2" whose head version inventory was removed
   ✔  And a repair crashed before writing "obj/v2/inventory.json.sha256"
   ✔  And the object was then changed by a root sidecar that does not match
   ✔  When the version-inventory repair runs again
   ✔  Then the repair should be refused as a root sidecar mismatch
   ✔  And the repair should have written nothing
  Scenario Outline: An interrupted repair on a damaged object is refused
   ✔  Given a sha256 object committed at "v2" whose head version inventory was removed
   ✔  And a repair crashed before writing "obj/v2/inventory.json.sha256"
   ✔  And the object was then changed by an unlisted file in the head version directory
   ✔  When the version-inventory repair runs again
   ✔  Then the repair should be refused as not repairable
   ✔  And the repair should have written nothing
  Scenario: Opening, reading and validating a W010 object never repairs it
   ✔  Given a copy of the pinned W010 fixture object
   ✔  When the object is opened, read and validated
   ✔  Then nothing should have been written to the storage root
[Summary]
1 feature
14 scenarios (14 passed)
79 steps (79 passed)
```

`cargo test -p filesquid-ocfl --locked` (summary lines; every binary passed, 3 BDD scenarios skipped are the existing `@unix`/community-validator ones):

```text
     Running unittests src/lib.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.15s
     Running tests/features.rs
120 scenarios (117 passed, 3 skipped)
540 steps (537 passed, 3 skipped)
     Running tests/integration/content_fixtures.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.80s
     Running tests/integration/fixture_conformance.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
     Running tests/integration/fixture_inventories.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/fixture_reads.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.28s
     Running tests/integration/fs_conformance.rs
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.85s
     Running tests/integration/hierarchy_guard.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/integration/logical_file.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.61s
     Running tests/integration/memory_conformance.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.75s
     Running tests/integration/opaque_bytes.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.13s
     Running tests/integration/read_objects.rs
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
     Running tests/integration/validate_objects.rs
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s
     Running tests/integration/version_inventory_repair.rs
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
     Running tests/integration/write_order.rs
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
     Running tests/unit/digest.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/fixity.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/git_guard.rs
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/unit/inventory.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/unit/lease.rs
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/namaste_sidecar.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/read_state.rs
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/source_scan.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/storage_key.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo clippy -p filesquid-ocfl --all-targets --locked -- -D warnings` and `RUSTDOCFLAGS="-D warnings" cargo doc -p filesquid-ocfl --no-deps --locked` finished without warnings.

`FULL=1 bash scripts/pre-commit.sh` (after merging `origin/main`, which brought IO-0.9):

```text
✅ Lint and test validation passed (test scope: full).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: full (430s)
========================================
```

## SRS traceability

| Requirement | Tests |
|---|---|
| OCFL-S-020 Version-inventory repair | `integration_version_inventory_repair` (all 14); `version_inventory_repair_OCFL-S-020.feature` (all 14 scenarios) |
| OCFL-S-018 Write order and unpublished versions | `a_repaired_sha256_object_becomes_editable`, `an_interrupted_repair_is_completed_by_running_it_again`, `integration_write_order` (unchanged, still passing) |
| OCFL-S-019 Inspection and copy never edit an object | `opening_reading_and_validating_never_repair`; BDD "Opening, reading and validating a W010 object never repairs it" |
| OCFL-S-002 Inventory types (sha256 writer only) | `the_w010_fixture_is_repaired_and_stays_closed_to_commits_as_sha512` |
| PRD F-042, NFR-001 | all of the above |

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no percentage is reported. Every `RepairError` variant is produced by a test except `RootChanged` (needs a writer that ignores the reservation between the two reads), `ForeignWrite` (the same), `Guard` (the guards are shared with the commit path and tested there), `Read` and `Storage` (backend failures).

## Review round 1 (Cubic)

Cubic reviewed `8bef140` and raised 7 findings (P2: 3, P3: 4), all fixed:

- The writer checked only that the head version files exist; it now verifies the version inventory bytes against the committed inventory and the sidecar against them (`CommitError::VersionInventoryMismatch`), with a new write-order test.
- `MissingVersionInventory`'s message now covers a missing head sidecar; the BDD step for the half-repaired head names the missing sidecar.
- The interrupted-repair outline gained a differing head inventory row and a held reservation row.
- The README names the one allowed validator finding (E058 of the interrupted shape); the plan row links this Walkthrough; the NFR-001 paragraph above says which failures can leave a resumable partial repair.

## Sync Execution Report

- Module SRS [OCFL-S-020](../srs.md): two clarifying clauses (a head with both files is refused as nothing to repair; the writer refuses a head whose version sidecar is still missing). No PRD, vision, root SRS or ADR change: OCFL-S-020 and API-S-023 already describe the repair and its consent.
- Crate [README](../../README.md): new "Version-inventory repair (W010)" section; the commit bullet names the sidecar refusal.
- Module [plan](../plan.md) IO-1.4 ticked and its Related Tests updated; [checklist](../../../../../../resources/implementation-plan-checklist.md) ticked; MVP progress bar redrawn.

## For the platform engineer (CL-1.1)

`filesquid_ocfl::ObjectWriter::repair_version_inventory(root) -> Result<RepairReceipt, RepairError>` is the call the consented repair command wires up. The receipt carries what the activity journal entry needs (object id, version, digest algorithm, inventory digest, paths written). `RepairError::NothingToRepair` means the head is already fine; `Reservation(Held)` means retry later; every other refusal means the object needs attention, and `Invalid` carries the validator findings.

## Mocked Environment Variables

None.
