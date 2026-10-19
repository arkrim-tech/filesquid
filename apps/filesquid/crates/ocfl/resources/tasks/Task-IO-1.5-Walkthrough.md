# Task IO-1.5 Walkthrough: Version-bound logical files in the change model

Branch: `feat/prov-o-history` (global Task FS-2.8, PROV-O history; [ADR 040](../../../../../../resources/architecture/040-prov-o-history.md) Decision 3).

## OCFL-S-024: Version-bound logical files

Also OCFL-S-015 (change-model API), OCFL-S-016 (concurrent commits: a rebased commit binds its files to the new version) and OCFL-S-018 (write order: the function runs after the `vN` reservation, before any `vN` byte).

## What changed

- `write::VersionBoundFiles` (implemented for closures) and `write::BoundContext {object_id, version, created, base, base_state, state}`: the caller's function returns logical files (path → bytes) for the reserved version. At most `MAX_BOUND_FILES` (64) files, each at most `MAX_PUT_BYTES`; a path the version already holds is refused.
- `ObjectWriter::commit_bound` calls the function after `reserve` and before any write, digests the bytes, adds them to the version's state and the manifest, and uploads new content as inline uploads (`UploadSource::Inline`, written `IfAbsent` after a fence check). Bytes the manifest already holds are not written again. A failing function (`CommitError::BoundFiles`) publishes nothing and releases the reservation. `ObjectWriter::commit` is `commit_bound` without a function.
- `ChangeModel::commit_with`, `revert_with` and `reinstate_with` hand the function to every attempt of the rebase loop, so a rebased commit calls it again with the new version name. `ChangeModel::commit_bound_only` publishes a version whose only change is the bound files (provenance-only versions for exports, repairs and backfill); an empty file set is refused and staged changes are left for a later commit.
- `VersionMeta` `user` and `message` pass through unchanged (tested).
- The engine gives the files no meaning; it knows nothing about PROV-O.

## SRS traceability

| Requirement | Tests |
|---|---|
| OCFL-S-024 | `integration_bound_files`: `bound_files_carry_the_reserved_version_and_created_time`, `the_function_runs_under_the_version_reservation`, `a_failing_function_publishes_nothing_and_keeps_the_staging_area`, `a_bound_file_may_not_replace_a_staged_one`, `too_many_bound_files_are_refused`, `bound_bytes_the_object_holds_are_not_written_again`, `revert_and_reinstate_also_carry_bound_files`, `version_user_and_message_pass_through_unchanged`, `a_bound_only_commit_*` (3); BDD `version_bound_files_OCFL-S-024.feature` (4 scenarios) |
| OCFL-S-016 | `a_rebased_commit_binds_its_files_to_the_new_version`; BDD "A rebased commit binds its record to the new version" |
| OCFL-S-018 | `the_function_runs_under_the_version_reservation` (a second reservation of the same version is refused inside the function) |

## TDD

The tests were written before the code. They use the new API, so before the implementation they did not compile (`no method named commit_with` / `commit_bound_only`). The `commit_bound_only` tests were run against the code before that method existed and failed to compile with `E0599`.

## Coverage

ADR 017 defines no Rust line-coverage tool. Every public item added is exercised by the tests above.

## Mocked Environment Variables

None.

## Raw test output

`cargo test -p filesquid-ocfl --locked --test integration_bound_files`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.19s
     Running tests/integration/bound_files.rs (<target>/debug/deps/integration_bound_files-158813c74eba4c27)

running 12 tests
test a_bound_file_may_not_replace_a_staged_one ... ok
test a_bound_only_commit_with_no_files_publishes_nothing ... ok
test a_bound_only_commit_adds_a_version_holding_only_the_bound_files ... ok
test a_failing_function_publishes_nothing_and_keeps_the_staging_area ... ok
test a_bound_only_commit_leaves_staged_changes_alone ... ok
test a_rebased_commit_binds_its_files_to_the_new_version ... ok
test bound_bytes_the_object_holds_are_not_written_again ... ok
test too_many_bound_files_are_refused ... ok
test the_function_runs_under_the_version_reservation ... ok
test bound_files_carry_the_reserved_version_and_created_time ... ok
test version_user_and_message_pass_through_unchanged ... ok
test revert_and_reinstate_also_carry_bound_files ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

`cargo test -p filesquid-ocfl --locked --test features` (the new feature, then the totals; the 3 skipped scenarios belong to earlier features):

```text
Feature: Version-bound logical files
  Scenario: A commit carries a record named after its reserved version
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given writer "writer-a" staged new bytes for "a.txt"
   ✔  When writer "writer-a" commits the staged changes with a version-bound record
   ✔  Then the object head should be "v2" through the change model
   ✔  And "v2" should hold the record "record/v2.txt" naming "v2" and its created time
  Scenario: A record-only commit adds one version holding only the record
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  When writer "writer-a" commits only a version-bound record
   ✔  Then the object head should be "v2" through the change model
   ✔  And "v2" should hold the record "record/v2.txt" naming "v2" and its created time
  Scenario: A rebased commit binds its record to the new version
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given writer "writer-a" staged new bytes for "a.txt"
   ✔  And writer "writer-b" committed "v2" with new bytes for "b.txt"
   ✔  When writer "writer-a" commits the staged changes with a version-bound record
   ✔  Then the object head should be "v3" through the change model
   ✔  And "v3" should hold the record "record/v3.txt" naming "v3" and its created time
  Scenario: A failing record function publishes nothing
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given writer "writer-a" staged new bytes for "a.txt"
   ✔  When writer "writer-a" commits the staged changes with a failing record function
   ✔  Then the commit should be refused for its version-bound files
   ✔  And the object head should be "v1" through the change model
Feature: Version-inventory repair for heads without one
  Scenario: The pinned W010 fixture is repaired and stays closed to commits as sha512
   ✔  Given a copy of the pinned W010 fixture object
   ✔  And a commit on it is refused for the missing version inventory
   ✔  When the version-inventory repair runs
   ✔  Then the repair should have written "v1/inventory.json" and "v1/inventory.json.sha512"
155 scenarios (152 passed, 3 skipped)
726 steps (723 passed, 3 skipped)
```
