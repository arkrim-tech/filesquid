# Task CL-1.1 Walkthrough: Local library create/open/recover commands

Module task [CL-1.1](../plan.md) of global [Task 1.1](../../../../../../resources/implementation-plan.md#task-1.1). Branch `feat/cl-1.1-library-create-open-recover`, from `main` at `89d546a`. Dependencies CL-0.2 and IO-1.4 were complete ([IO-1.4 walkthrough](../../../ocfl/resources/tasks/Task-IO-1.4-Walkthrough.md)).

**Status: done.** `filesquid-api` gains the `local_library` module: the meaning of creating and opening a library on an owner-chosen OCFL storage root, copying a completed snapshot, rebuilding from files, the version-inventory repair candidate query, and the owner-consented, journaled repair command of API-S-023 over the IO-1.4 engine repair. The host injects the storage root as a `filesquid-ocfl` `StorageBackend`, the reservations as `VersionReservations` and the repair ledger as `RepairLedger`; the crate does no storage I/O itself and does not link `filesquid-native`. As in CL-0.2, no HTTP route or contract path is added (a contract change also changes the UI's generated types); the routes DE-1.1 and UI-1.1 call still need a plan row.

## API-S-023: Version-inventory repair, consented and journaled

The version-inventory repair of OCFL-S-020 MUST be reachable only through an explicit API command that names the library and the objects to repair, MUST be refused for any caller other than the desktop owner or the owner of that library on a server, MUST run only after that owner has consented in the UI to a request that names the objects and says that one version inventory and its sidecar, copies of the root inventory, will be added to each object's latest version directory and that nothing else changes, MUST journal the consent and each object's outcome as `ocfl.repair.version_inventory` (opaque object ID, version, outcome, error code; no paths or content), the outcome in the same transaction that records the repair receipt, and MUST NOT run as a side effect of opening, browsing, validating, inspecting, copying or committing ([module SRS](../srs.md)).

Parents: [F-042 OCFL preservation and portable recovery](../../../../../../resources/prd.md#f-042-ocfl-preservation-and-portable-recovery), [SRS-F-042 OCFL preservation and portable recovery](../../../../../../resources/srs.md#srs-f-042-ocfl-preservation-and-portable-recovery), [F-061 Activity journal and live health monitoring](../../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring), [NFR-004 Security and privacy](../../../../../../resources/prd.md#nfr-004-security-and-privacy), OCFL-S-020.

| Constraint | Gate | State |
|---|---|---|
| C1 An explicit command names the library and 1 to 1,000 distinct objects | `object_lists_are_bounded_and_distinct` | Met |
| C2 Refused for anyone but the desktop owner (the server administrator owns no library; server library owners arrive with API-F1.9 as a new `Principal`, which fails to compile in `authorise_repair` until decided) | `the_command_is_refused_for_another_caller`, scenario "refused for another caller" | Met |
| C3 Runs only after a confirmed consent naming exactly this library, these objects and statement `version_inventory_repair.v1`; nothing is written otherwise | `the_command_is_refused_without_consent_and_writes_nothing`, `the_consent_statement_names_what_changes`, scenario "refused without consent" | Met |
| C4 The consent is recorded in the ledger (`started`) before any repair; an unrecordable consent repairs nothing | `consent_and_one_outcome_row_per_object_are_journaled_without_paths`, `an_unrecordable_consent_repairs_nothing` | Met |
| C5 One outcome row per object (library, object ID, version, outcome, error code), recorded with its receipt in one ledger call, no paths or content | `consent_and_one_outcome_row_per_object_are_journaled_without_paths` | Met |
| C6 Every refusal is journaled; an outcome the ledger cannot take goes through the bounded journal queue | `the_command_is_refused_without_consent_and_writes_nothing` (4 refusals, 4 rows), `an_unrecordable_outcome_still_reaches_the_journal` | Met |
| C7 A head with both its version inventory and sidecar is refused with `nothing_to_repair`, not a no-op success; an interrupted repair is resumed | `a_head_that_already_has_both_files_is_refused_with_nothing_to_repair`, `a_second_repair_is_refused_with_nothing_to_repair`, `an_interrupted_repair_is_resumed` | Met |
| C8 A repaired sha256 object is committable; a sha512 object stays closed (the candidate says so) | `a_consented_repair_by_the_owner_makes_a_sha256_object_committable`, `candidates_say_whether_the_repair_opens_the_object_to_commits`; the sha512 refusal itself is the engine's (IO-1.4 tests) | Met |
| C9 W010 commits stay refused until repaired; open, copy, rebuild and the candidate query never reach the repair | `a_consented_repair_by_the_owner_makes_a_sha256_object_committable` (commit refused first), `open_copy_and_rebuild_never_reach_the_repair` | Met |
| C10 The candidate query checks each head for its version inventory and sidecar, independent of API-F2.7 | `the_candidate_query_lists_heads_without_their_version_inventory` | Met |
| C11 Engine refusals map to stable codes; held reservations are retryable | `a_held_reservation_is_a_retryable_refusal`, `an_unknown_object_is_refused_per_object` | Met |

## Secondary SRS mapping

- **SRS-F-001: Independent local library and ownership.** `create_root` makes an empty owner-chosen folder an OCFL 1.1 storage root with no account or connection and writes only `0=ocfl_1.1`, never over a file (`create_writes_only_the_storage_root_declaration`, `create_refuses_a_folder_that_is_not_empty`, `create_twice_is_refused`); `open_root` refuses a folder that is not a storage root or holds Git metadata, by code only (`open_refuses_*`). Direct inspection and copy never edit the library: open, rebuild, the candidate query and copy write nothing to the source (`open_reports_the_objects_and_writes_nothing`, `copy_produces_a_checked_snapshot_and_never_writes_the_source`, `rebuild_reconstructs_from_files_and_repairs_nothing`).
- **SRS-F-042: OCFL preservation and portable recovery.** The snapshot copy holds only committed files (declarations, inventories and sidecars, manifest content), each written with `IfAbsent` into an empty destination (`copy_refuses_a_destination_that_is_not_empty`), streams files above one put through a multipart upload (`copy_streams_a_file_larger_than_one_put`), refuses a head being published as retryable `snapshot_incomplete` (`copy_refuses_a_snapshot_with_an_unpublished_head`), and is validated with its findings reported (`copy_reports_an_external_edit_found_in_the_copy`). The rebuild reads every object from its files alone and reports validator errors and W010 heads instead of adopting or repairing them (`rebuild_reports_an_external_edit_instead_of_adopting_it`).
- **API-S-005 / API-S-006.** Answers and journal entries carry opaque IDs and codes only (object IDs that are not opaque are redacted by the journal); `filesquid-api` links `filesquid-ocfl` with default features off and does no storage I/O (`unit_isolation` passes).
- **Logging (ADR 033).** Create, copy and rebuild journal their outcome through the bounded queue; open and the candidate query journal their failures, with the caller as actor.

## What changed

- `crates/api/src/local_library/`: `root` (create, open), `snapshot` (completed snapshot copy), `rebuild` (rebuild from files), `repair` (request, consent, candidates, engine call and error codes), `ledger` (`RepairLedger` port, `MemoryRepairLedger`), `error` (`LocalLibraryError`), `service` (`LocalLibraryService`).
- `crates/api/Cargo.toml`: `filesquid-ocfl` (path, default features off); test targets `unit_local_library` and `unit_version_inventory_repair`.
- `crates/api/src/events.rs`: `fetch_update` renamed to `try_update`. The rustc 1.99 toolchain deprecates `fetch_update`, which failed `cargo clippy -D warnings` for `filesquid-api` on `main` as well; separate commit.
- Tests: `tests/unit/local_library.rs`, `tests/unit/version_inventory_repair.rs`, `tests/support/ocfl_fixture.rs`, `tests/support/local_library_steps.rs`, `tests/features/local_library_and_version_inventory_repair_API-S-023.feature`.
- Docs: module SRS API-S-006 (the `filesquid-ocfl` link without its filesystem backend), the CL-1.1 plan row, product SRS test lists for SRS-F-001 and SRS-F-042, root README test command, checklist box and MVP progress.

## For the desktop host and the UI (DE-1.1, UI-1.1)

- The desktop host opens the chosen folder as an `FsBackend`, refuses a copy destination inside the library folder (a backend cannot see its parent folders), and implements `RepairLedger` in SQLite so the outcome entry and the receipt land in one transaction with the activity journal.
- `LocalLibraryService::repair_candidates` feeds the UI-1.1 offer; the consent dialog sends `RepairConsent` with `statement_id = "version_inventory_repair.v1"` and exactly the objects shown; cancelling sends nothing. Per-object answers carry `repaired` or `refused` with an error code and `retryable`.

## Tests

TDD: the tests were written first. Before the module existed, `cargo test -p filesquid-api --test unit_local_library --test unit_version_inventory_repair` failed to compile:

```text
error[E0432]: unresolved import `filesquid_api::local_library`
error[E0433]: cannot find `local_library` in `filesquid_api`
error: could not compile `filesquid-api` (test "unit_local_library") due to 2 previous errors
error: could not compile `filesquid-api` (test "unit_version_inventory_repair") due to 5 previous errors
```

After the implementation, `cargo test -p filesquid-api --locked --test unit_local_library --test unit_version_inventory_repair`:

```text
     Running tests/unit/local_library.rs
running 14 tests
test copy_refuses_a_destination_that_is_not_empty ... ok
test copy_refuses_a_snapshot_with_an_unpublished_head ... ok
test copy_reports_an_external_edit_found_in_the_copy ... ok
test create_twice_is_refused ... ok
test create_writes_only_the_storage_root_declaration ... ok
test errors_carry_stable_codes_and_api_errors ... ok
test open_refuses_a_folder_that_is_not_a_storage_root ... ok
test open_refuses_git_metadata_in_the_root ... ok
test create_refuses_a_folder_that_is_not_empty ... ok
test copy_produces_a_checked_snapshot_and_never_writes_the_source ... ok
test rebuild_reports_an_external_edit_instead_of_adopting_it ... ok
test open_reports_the_objects_and_writes_nothing ... ok
test rebuild_reconstructs_from_files_and_repairs_nothing ... ok
test copy_streams_a_file_larger_than_one_put ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.87s
     Running tests/unit/version_inventory_repair.rs
running 16 tests
test a_head_that_already_has_both_files_is_refused_with_nothing_to_repair ... ok
test a_held_reservation_is_a_retryable_refusal ... ok
test an_interrupted_repair_is_resumed ... ok
test a_second_repair_is_refused_with_nothing_to_repair ... ok
test an_unknown_object_is_refused_per_object ... ok
test a_consented_repair_by_the_owner_makes_a_sha256_object_committable ... ok
test an_unrecordable_consent_repairs_nothing ... ok
test an_unrecordable_outcome_still_reaches_the_journal ... ok
test candidates_say_whether_the_repair_opens_the_object_to_commits ... ok
test object_lists_are_bounded_and_distinct ... ok
test the_command_is_refused_for_another_caller ... ok
test open_copy_and_rebuild_never_reach_the_repair ... ok
test consent_and_one_outcome_row_per_object_are_journaled_without_paths ... ok
test the_consent_statement_names_what_changes ... ok
test the_command_is_refused_without_consent_and_writes_nothing ... ok
test the_candidate_query_lists_heads_without_their_version_inventory ... ok
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

`cargo test -p filesquid-api --locked --test features` (this task's feature, then the harness summary; the skipped scenarios belong to features whose steps land with later tasks):

```text
Feature: Local library create, open, copy, rebuild and version-inventory repair
  Scenario: Create a library in an empty folder without an account or a connection
   ✔  Given an empty folder chosen by the owner
   ✔  When the owner creates a library there
   ✔  Then the folder should hold only the OCFL 1.1 storage-root declaration
   ✔  And the creation should be journaled as succeeded
  Scenario: A folder that already holds files is refused
   ✔  Given a folder chosen by the owner that already holds a file
   ✔  When the owner creates a library there
   ✔  Then the request should be refused with "root_not_empty"
   ✔  And nothing should be written to the folder
  Scenario: Copying a completed snapshot leaves the library untouched
   ✔  Given a library with a complete object and an object whose head has no version inventory
   ✔  When the owner copies a completed snapshot to an empty folder
   ✔  Then the copy should validate without errors and open as a library of 2 objects
   ✔  And nothing should be written to the library
   ✔  And the object without a version inventory should still lack it
  Scenario: Rebuilding from files reports problems and repairs nothing
   ✔  Given a library with a complete object and an object whose head has no version inventory
   ✔  When the owner rebuilds the library from its files
   ✔  Then the rebuild should list 2 objects
   ✔  And the object without a version inventory should be flagged as a repair candidate
   ✔  And nothing should be written to the library
  Scenario: The candidate query lists the objects to offer
   ✔  Given a library with a complete object and an object whose head has no version inventory
   ✔  When the owner asks which objects need a version-inventory repair
   ✔  Then only the object without a version inventory should be listed
  Scenario: A consented repair by the owner makes the object committable
   ✔  Given a library with an object whose head has no version inventory
   ✔  And a new version of that object is refused until it is repaired
   ✔  When the owner consents and repairs that object
   ✔  Then the object should be repaired
   ✔  And a new version of that object should be accepted
   ✔  And the consent and one outcome per object should be journaled without paths
  Scenario: The repair is refused without consent
   ✔  Given a library with an object whose head has no version inventory
   ✔  When the repair is requested without the owner's consent
   ✔  Then the request should be refused with "consent_required"
   ✔  And nothing should be written to the library
   ✔  And the refusal should be journaled
  Scenario: The repair is refused for another caller
   ✔  Given a library with an object whose head has no version inventory
   ✔  When the server administrator requests the repair with consent
   ✔  Then the request should be refused with "forbidden"
   ✔  And nothing should be written to the library
   ✔  And the refusal should be journaled
  Scenario: A head that already has its version inventory is not repaired again
   ✔  Given a library with a complete object
   ✔  When the owner consents and repairs that object
   ✔  Then the object should be refused with "nothing_to_repair"
   ✔  And nothing should be written to the library
211 scenarios (105 passed, 106 skipped)
488 steps (382 passed, 106 skipped)
```

## Mocked Environment Variables

None.
