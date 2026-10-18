# Task IO-0.10 Walkthrough: Forward-delta change model

Module plan row: [IO-0.10](../plan.md). Global task: [Task 0.4](../../../../../../resources/implementation-plan.md#task-0.4) (completed by this slice: IO-0.6 to IO-0.10 are all checked).

## OCFL-S-014: Delta computation — classify a working state against a base version by digest (added, changed, renamed, deleted, reinstated) and write only bytes the manifest lacks

## OCFL-S-015: Change-model API — stage, status, diff, commit, log, checkout, revert and reinstate; one version per commit; staging outside the storage root

## OCFL-S-016: Concurrent commits — rebase onto a moved head when no staged path changed, otherwise report a conflict without publishing

## OCFL-S-018: Write order and unpublished versions — a head without its version inventory (W010) stays refused as a commit base; the writer-fencing gap stays named

## OCFL-S-020: Version-inventory repair — the change model never repairs implicitly

## OCFL-S-010: Read and write conformance (b) — committed objects validate in the engine and in an independent community validator

Sources: [OCFL-S-010, OCFL-S-014, OCFL-S-015, OCFL-S-016, OCFL-S-018, OCFL-S-020](../srs.md), [PRD F-051](../../../../../../resources/prd.md#f-051-ocfl-module), [PRD NFR-001](../../../../../../resources/prd.md#nfr-001-preservation-and-transactional-integrity), [SRS-F-051](../../../../../../resources/srs.md#srs-f-051-ocfl-module), [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md) "Change model (public API)", [ADR 025](../../../../../../resources/architecture/025-client-side-filesquid-engines.md), [ADR 012](../../../../../../resources/architecture/012-s3-compatible-ciphertext-storage.md).

## What changed

- **New public module `change`** (`src/change/`, re-exported from the crate root as `ChangeModel`, `ChangeError`, `PathChange`, `StagingArea`, `VersionMeta`):
  - `compute_delta(inventory, base, working) -> Vec<PathChange>` (pure). `PathChange::{Added, Changed, Renamed, Deleted, Reinstated}`; `Added` and `Changed` carry `new_bytes` (the digest is not in the `manifest`). A deleted path and a new path with the same digest are a rename (several pair in path order); a new path whose content the `manifest` holds but the base state does not is reinstated; a new path that copies content still in the base is added with `new_bytes: false`. Digests compare without case. Changes are ordered by path (the new path of a rename).
  - `StagingArea::new(&backend, prefix)`: the staging area on any caller backend, under a caller prefix (format below).
  - `ChangeModel::new(&storage, &reservations, holder, root, object_id, staging)`: `stage_bytes`, `stage_remove`, `stage_rename`, `stage_tree` (every file under a prefix of any backend, for example a directory through `FsBackend`, becomes the working state), `reset`, `status` (`Status { object_id, base, head, changes }`), `diff(from, to)`, `log` (newest first), `checkout(version, &dest, prefix)`, `commit(meta)`, `revert(version, meta)`, `reinstate(version, paths, meta)`. `VersionMeta { created, message, user }`.
  - `ChangeError::{Conflict, NothingToCommit, PathNotFound, InvalidPath, StagingCorrupt, StagingMismatch, WrongObject, SourceChanged, TooManyRebases, Commit, Read, Storage}`; `MAX_REBASES = 16`.
- **Built on the IO-0.6 writer, no second write path.** Every commit, revert and reinstate builds a `VersionDraft` (manifest digests are kept, other content is read from staging blobs) and calls `ObjectWriter::commit` with the change set's base as `expected_head`, so the fenced `vN` reservation, write order and head compare-and-swap of OCFL-S-018 apply unchanged. The writer itself was not changed; three internal modules became `pub(crate)` (`write::base`, `read::copy_guard`, `inventory::{json, paths}`) so the change model reuses the base reader, the OCFL-root guard, the strict JSON reader and the logical-path rules.
- **Rebase or conflict.** Before each attempt the model reads the committed head. If it is not the base, every version after the base is compared with the base on each staged path; any difference is `ChangeError::Conflict { base, head, paths }` and nothing is published (the staging area is kept); otherwise the base moves to the head. `CommitError::HeadMoved` from the writer (the head moved between the model's read and the reservation) repeats the check. At most `MAX_REBASES` attempts, then `TooManyRebases`.
- **W010 stays refused.** A head without its version inventory surfaces as `ChangeError::Commit(CommitError::MissingVersionInventory)` with nothing written; the change model never calls the repair (owner decision binding this task; OCFL-S-020).
- **Engine decisions** (recorded in OCFL-S-015 and OCFL-S-016): a change set that leaves the logical state unchanged is refused (`NothingToCommit`), for commit, revert and reinstate alike; the rebase loop is bounded.
- **No new dependency.** The `wasm32-unknown-unknown` build with default features off passes (below).

## Staging area format (OCFL-S-015; ADR 016 "specified at first implementation")

The staging area is engine-private working data under a caller prefix `P` on a caller backend. It is never inside an OCFL object or storage root, never an OCFL extension, and never read when an object is read, validated, copied or recovered.

| Key | Content |
|---|---|
| `P/index.json` | The staging index (below). Absent: nothing staged. |
| `P/blobs/<sha256>` | Staged bytes whose digest the object's `manifest` lacked when staged, named by their lowercase sha256 digest, so identical bytes are staged once. |

`index.json` is one UTF-8 JSON object, format `filesquid-ocfl-staging` version 1:

```json
{
  "type": "filesquid-ocfl-staging",
  "version": 1,
  "objectId": "urn:example:obj",
  "base": "v2",
  "paths": {
    "a.txt": { "sha256": "<64 lowercase hex digits>" },
    "old.txt": { "removed": true }
  }
}
```

- `base` is the committed head the changes were staged against (`null` for an object without a version). It is recorded at the first stage and moves only when a commit rebases.
- `paths` lists staged logical paths only: each holds the content with that sha256 digest, or is removed. Every other path keeps its content from `base`. Staging the content a path already has in `base` (or removing a path `base` lacks) unstages it.
- Reading is strict and bounded: at most 64 MiB, the engine's strict JSON reader (no repeated keys, nesting at most 8), no unknown keys, exact `type` and `version`, every path passing E052 and E053, every digest 64 lowercase hex digits. Anything else is `ChangeError::StagingCorrupt`; an index for another object is `StagingMismatch`.
- Before the first write of each key the area refuses a key below a directory holding an OCFL object or storage-root declaration (`ReadError::CopyIntoOcflRoot`, the OCFL-S-019 copy guard). `checkout` uses the same guard.
- A successful `commit` deletes the blobs and the index. `reset` does the same without committing. Revert and reinstate do not use the staging area.
- One area serves one object and one writer; it takes no lock of its own.

## TDD

1. Tests were written first: `tests/integration/change_model.rs` (17 tests), `tests/support/change_rig.rs`, `tests/features/change_model_OCFL-S-014.feature` (8 scenarios) with `tests/support/change_steps.rs`, and the `integration_change_model` target in `Cargo.toml`.
2. Red run before any implementation (the `change` module did not exist):

```text
error[E0432]: unresolved import `filesquid_ocfl::change`
  --> crates/ocfl/tests/integration/change_model.rs:12:21
   |
12 | use filesquid_ocfl::change::{compute_delta, ChangeError, PathChange, StagingArea};
   |                     ^^^^^^ could not find `change` in `filesquid_ocfl`

error[E0432]: unresolved import `filesquid_ocfl::ChangeModel`
  --> crates/ocfl/tests/integration/change_model.rs:16:5
   |
16 | use filesquid_ocfl::ChangeModel;
   |     ^^^^^^^^^^^^^^^^-----------
   |                     |
   |                     no `ChangeModel` in the root

error[E0432]: unresolved import `filesquid_ocfl::change`
 --> crates/ocfl/tests/integration/../support/change_rig.rs:9:21
  |
9 | use filesquid_ocfl::change::{
  |                     ^^^^^^ could not find `change` in `filesquid_ocfl`

For more information about this error, try `rustc --explain E0432`.
error: could not compile `filesquid-ocfl` (test "integration_change_model") due to 3 previous errors
```

3. Then `src/change/` was implemented; the unit tests in `src/change/delta.rs` and `src/change/index.rs` were added for edge cases (several renames of one digest, case-insensitive digests, index round trip and refusals).

## Constraint gates

| Constraint | Test |
|---|---|
| OCFL-S-014 classify by digest: added, changed, renamed, deleted, reinstated | `a_commit_writes_only_bytes_the_manifest_lacks`, `a_reinstated_path_copies_no_bytes`, `a_working_directory_is_staged_as_one_change_set`, `compute_delta_classifies_a_working_state_against_a_base`; unit `several_paths_of_one_digest_pair_in_path_order`; BDD "A staged change set is classified…", "Reinstating deleted content copies no bytes" |
| OCFL-S-014 write only bytes the manifest lacks | `a_commit_writes_only_bytes_the_manifest_lacks` (receipt and `v2/content` listing hold only `a.txt` and `d.txt`; the copy `e.txt` and the rename write nothing) |
| OCFL-S-014 renames, deletes, reinstatements change `state` only | same, plus `a_reinstated_path_copies_no_bytes` (`c.txt` at `v3` resolves to `v1/content/c.txt`) |
| OCFL-S-015 public, documented API | rustdoc on every item; `log_and_diff_read_the_inventory`, `checkout_writes_a_version_to_a_caller_location` |
| OCFL-S-015 one version per commit; empty change set refused | every commit test asserts the version; `an_empty_change_set_publishes_nothing` |
| OCFL-S-015 revert and reinstate add a version, earlier versions untouched | `revert_commits_a_new_version_without_touching_earlier_ones`, `reinstate_commits_a_new_version_with_the_chosen_paths` (every key under `obj/v*` keeps its entity tag), `reinstate_refuses_a_path_the_version_does_not_hold`; BDD "Revert and reinstate each add a version and keep history" |
| OCFL-S-015 staging and checkout outside the storage root; no extension; reads need no staging | `staging_and_checkout_inside_an_ocfl_root_are_refused`, `staging_is_never_inside_the_object_and_reads_need_none_of_it`, `a_damaged_or_foreign_staging_index_is_refused`; BDD "Staging inside the object root is refused" |
| OCFL-S-016 rebase when no staged path changed | `a_moved_head_is_rebased_onto` (memory and filesystem); BDD "A moved head is rebased onto…" |
| OCFL-S-016 conflict otherwise, nothing published | `a_moved_head_that_touched_a_staged_path_reports_a_conflict` (whole object root unchanged, staging kept); BDD "A moved head that changed a staged path is a conflict" |
| OCFL-S-016 every commit through reservation and head CAS; bounded rebase | by construction (`ObjectWriter::commit` is the only call that writes; `MAX_REBASES`) |
| OCFL-S-018 / OCFL-S-020 W010 head refused, not repaired | `a_head_without_its_version_inventory_stays_refused_and_unrepaired`; BDD "A head without its version inventory stays refused and is not repaired" |
| OCFL-S-010 (b) engine and community validator | `committed_objects_pass_the_engine_and_community_validators` (4 versions: change, rename, delete, reinstate, revert); `rig.assert_valid()` in every scenario that commits |
| wasm32 build with default features off | `cargo build … --target wasm32-unknown-unknown` below; BDD "Engine builds for the browser" |

## Test output

`cargo test -p filesquid-ocfl --locked --test integration_change_model -- --nocapture --test-threads=1`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.17s
     Running tests/integration/change_model.rs ($CARGO_TARGET_DIR/debug/deps/integration_change_model-97a7db5939f6743a)

running 17 tests
test a_commit_writes_only_bytes_the_manifest_lacks ... ok
test a_damaged_or_foreign_staging_index_is_refused ... ok
test a_head_without_its_version_inventory_stays_refused_and_unrepaired ... ok
test a_moved_head_is_rebased_onto ... ok
test a_moved_head_that_touched_a_staged_path_reports_a_conflict ... ok
test a_reinstated_path_copies_no_bytes ... ok
test a_working_directory_is_staged_as_one_change_set ... ok
test an_empty_change_set_publishes_nothing ... ok
test checkout_writes_a_version_to_a_caller_location ... ok
test committed_objects_pass_the_engine_and_community_validators ... filesquid-ocfl change-model object: VALID (4 versions)
ocfl-py change-model object: VALID
[W004] OCFL Object root inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W004] OCFL Object v1 inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W004] OCFL Object v2 inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W004] OCFL Object v3 inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W007b] OCFL Object root inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object root inventory v2 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object root inventory v3 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object root inventory v4 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object v1 inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object v2 inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object v2 inventory v2 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object v3 inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object v3 inventory v2 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object v3 inventory v3 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
OCFL v1.1 Object at $CARGO_TARGET_DIR/tmp/ocfl-change-5548-5/root/obj is VALID
ok
test compute_delta_classifies_a_working_state_against_a_base ... ok
test log_and_diff_read_the_inventory ... ok
test reinstate_commits_a_new_version_with_the_chosen_paths ... ok
test reinstate_refuses_a_path_the_version_does_not_hold ... ok
test revert_commits_a_new_version_without_touching_earlier_ones ... ok
test staging_and_checkout_inside_an_ocfl_root_are_refused ... ok
test staging_is_never_inside_the_object_and_reads_need_none_of_it ... ok

test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.76s

```

`cargo test -p filesquid-ocfl --locked --test features -- --input "change_model_*.feature"`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.16s
     Running tests/features.rs ($CARGO_TARGET_DIR/debug/deps/features-9009fa7eceb51bdd)
Feature: Forward-delta change model
  Scenario: A staged change set is classified and stores only new bytes
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  When writer "writer-a" stages new bytes for "a.txt", renames "b.txt" to "docs/b.txt" and deletes "c.txt"
   ✔  Then the status should list "changed a.txt", "deleted c.txt" and "renamed b.txt -> docs/b.txt"
   ✔  When writer "writer-a" commits the staged changes
   ✔  Then the object head should be "v2" through the change model
   ✔  And the commit should have written only "v2/content/a.txt"
   ✔  And the change-model object should validate with zero errors
  Scenario: Reinstating deleted content copies no bytes
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given writer "writer-a" committed "v2" deleting "c.txt"
   ✔  When writer "writer-a" stages the original bytes of "c.txt"
   ✔  Then the status should list "reinstated c.txt"
   ✔  When writer "writer-a" commits the staged changes
   ✔  Then the commit should have written nothing
   ✔  And "c.txt" at "v3" should point at "v1/content/c.txt"
  Scenario: Revert and reinstate each add a version and keep history
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given writer "writer-a" committed "v2" deleting "c.txt"
   ✔  When writer "writer-a" reverts to "v1"
   ✔  Then the object head should be "v3" through the change model
   ✔  And the state of "v3" should equal the state of "v1"
   ✔  And no file of "v1" or "v2" should have changed
   ✔  And the commit should have written nothing
  Scenario: A moved head is rebased onto when no staged path changed
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given writer "writer-a" staged new bytes for "a.txt"
   ✔  And writer "writer-b" committed "v2" with new bytes for "b.txt"
   ✔  When writer "writer-a" commits the staged changes
   ✔  Then the object head should be "v3" through the change model
   ✔  And "v3" should hold the changes of both writers
  Scenario: A moved head that changed a staged path is a conflict
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given writer "writer-a" staged new bytes for "a.txt"
   ✔  And writer "writer-b" committed "v2" with new bytes for "a.txt"
   ✔  When writer "writer-a" commits the staged changes
   ✔  Then the commit should be refused as a conflict on "a.txt"
   ✔  And the object head should be "v2" through the change model
  Scenario: An empty change set publishes nothing
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  When writer "writer-a" commits the staged changes
   ✔  Then the commit should be refused as nothing to commit
   ✔  And the object head should be "v1" through the change model
  Scenario: A head without its version inventory stays refused and is not repaired
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  Given the head version inventory of "v1" was removed
   ✔  And writer "writer-a" staged new bytes for "a.txt"
   ✔  When writer "writer-a" commits the staged changes
   ✔  Then the commit should be refused for the missing version inventory
   ✔  And the head version inventory of "v1" should still be missing
  Scenario: Staging inside the object root is refused
   ✔> Given an object committed at "v1" with "a.txt", "b.txt" and "c.txt" through the change model
   ✔  When a writer stages a file in a staging area inside the object root
   ✔  Then staging should be refused as inside an OCFL root
[Summary]
1 feature
8 scenarios (8 passed)
46 steps (46 passed)
```

`cargo test -p filesquid-ocfl --locked --lib change`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.16s
     Running unittests src/lib.rs ($CARGO_TARGET_DIR/debug/deps/filesquid_ocfl-336d9c2a83f05f02)

running 9 tests
test change::delta::tests::an_unchanged_state_has_no_changes ... ok
test change::delta::tests::several_paths_of_one_digest_pair_in_path_order ... ok
test change::delta::tests::digests_compare_without_case ... ok
test change::delta::tests::a_change_back_to_older_content_writes_no_bytes ... ok
test change::index::tests::a_new_object_index_has_a_null_base ... ok
test change::index::tests::staging_what_the_base_holds_unstages_the_path ... ok
test change::index::tests::an_index_round_trips ... ok
test change::index::tests::unknown_keys_and_bad_digests_are_refused ... ok
test storage::whole::tests::a_change_and_change_back_during_a_read_is_not_mixed ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 34 filtered out; finished in 0.00s

```

`cargo test -p filesquid-ocfl --locked` (summary lines of the whole crate):

```text
     Running unittests src/lib.rs ($CARGO_TARGET_DIR/debug/deps/filesquid_ocfl-336d9c2a83f05f02)
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.59s
     Running tests/features.rs ($CARGO_TARGET_DIR/debug/deps/features-9009fa7eceb51bdd)
   ✔  Then Arrow, Parquet, DuckDB, and Iceberg should be absent from default features
[Summary]
13 features
136 scenarios (133 passed, 3 skipped)
622 steps (619 passed, 3 skipped)
     Running tests/integration/change_model.rs ($CARGO_TARGET_DIR/debug/deps/integration_change_model-97a7db5939f6743a)
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s
     Running tests/integration/content_fixtures.rs ($CARGO_TARGET_DIR/debug/deps/integration_content_fixtures-2450cd9c4beef973)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.12s
     Running tests/integration/enumerate_state.rs ($CARGO_TARGET_DIR/debug/deps/integration_enumerate_state-c992207f0a36f4c4)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.19s
     Running tests/integration/fixture_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_fixture_conformance-6188b5d43aaacb5e)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
     Running tests/integration/fixture_inventories.rs ($CARGO_TARGET_DIR/debug/deps/integration_fixture_inventories-49912e26c8122bb4)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/fixture_reads.rs ($CARGO_TARGET_DIR/debug/deps/integration_fixture_reads-f992725a1bd4ec37)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.28s
     Running tests/integration/fs_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_fs_conformance-4c054563d204acfb)
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.11s
     Running tests/integration/hierarchy_guard.rs ($CARGO_TARGET_DIR/debug/deps/integration_hierarchy_guard-3efdbaf82d970047)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/integration/logical_file.rs ($CARGO_TARGET_DIR/debug/deps/integration_logical_file-c870615487e03f53)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.02s
     Running tests/integration/memory_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_memory_conformance-bbed81d294b9c269)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s
     Running tests/integration/opaque_bytes.rs ($CARGO_TARGET_DIR/debug/deps/integration_opaque_bytes-1ba5398c372ee9d8)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.28s
     Running tests/integration/read_objects.rs ($CARGO_TARGET_DIR/debug/deps/integration_read_objects-777c46e5ca575e3e)
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/integration/validate_objects.rs ($CARGO_TARGET_DIR/debug/deps/integration_validate_objects-f628c102e60642ab)
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s
     Running tests/integration/version_inventory_repair.rs ($CARGO_TARGET_DIR/debug/deps/integration_version_inventory_repair-ec3ea182e8864a75)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
     Running tests/integration/write_order.rs ($CARGO_TARGET_DIR/debug/deps/integration_write_order-22dd8487357674b8)
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
     Running tests/unit/digest.rs ($CARGO_TARGET_DIR/debug/deps/unit_digest-84296ab96d185c1f)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/fixity.rs ($CARGO_TARGET_DIR/debug/deps/unit_fixity-595c41ef01df1b6c)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/git_guard.rs ($CARGO_TARGET_DIR/debug/deps/unit_git_guard-dfd4d18dbb6d3248)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/unit/inventory.rs ($CARGO_TARGET_DIR/debug/deps/unit_inventory-73e97bdd05e073f7)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/unit/lease.rs ($CARGO_TARGET_DIR/debug/deps/unit_lease-196772f0f0361667)
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
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

`cargo clippy -p filesquid-ocfl --all-targets --locked -- -D warnings`:

```text
    Finished `dev` profile [unoptimized] target(s) in 0.20s
```

`cargo build -p filesquid-ocfl --locked --no-default-features --target wasm32-unknown-unknown`:

```text
    Finished `dev` profile [unoptimized] target(s) in 5.75s
```

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no percentage is reported. Every constraint above has at least one test. Not reached by a test: `ChangeError::TooManyRebases` and the `HeadMoved` retry (they need a writer to publish between the model's head read and the reservation, which the synchronous test backends cannot interleave; the rebase check they repeat is the one the rebase and conflict tests cover), `SourceChanged` (a working file that changes between hashing and copying), and `WrongObject`.

## Sync Execution Report

- **Module SRS:** OCFL-S-015 records the engine decision that a change set leaving the state unchanged publishes no version and points to the staging format; OCFL-S-016 records that rebasing is bounded and restates the W010 refusal (OCFL-S-018) and no implicit repair (OCFL-S-020).
- **Module plan:** IO-0.10 checked; Related Tests add the new feature, integration test, step module and this Walkthrough.
- **Implementation plan:** global Task 0.4 checked (IO-0.6 to IO-0.10 done); Related Tests add the new feature.
- **Repository-root implementation-plan checklist:** IO-0.10 and Task 0.4 checked; MVP progress bar redrawn (`scripts/docs/update_mvp_progress.py`).
- **Crate README:** new "Change model" section; the "Write" section points callers to it. **Root README:** "Run tests" names `integration_change_model`.
- **Crate rustdoc:** `src/lib.rs` and `src/change/mod.rs` describe the change model.
- **PRD, root SRS, product vision, ADRs:** no change. PRD F-051, SRS-F-051 ("Change model") and ADR 016 already require exactly this API; ADR 016 leaves the staging format to the first implementation, which this Walkthrough and the crate README specify.

## Mocked Environment Variables

None. The community validator check used `ocfl-validate.py` from `ocfl-py`, found on `PATH`.

## Remaining limits and notes for callers

- `ObjectWriter::commit` stays public because the change model, the fixture writers and the write-order tests drive it, and `ObjectWriter::repair_version_inventory` lives on the same type. Callers that make versions (the view API in IO-1.2, `fsquid` in IO-D.4, `filesquid-api`) MUST use `ChangeModel` (OCFL-S-016).
- The writer-fencing gap is unchanged ([OCFL-S-018](../srs.md) "Known gap"); the change model adds no storage-side fencing. IO-5.1 carries the storage-side check for the S3-compatible backend.
- `status` reports the change set against its recorded base; when `head` differs, the commit rebases or reports a conflict.
- A staging area is single-writer: two change models MUST NOT share one prefix.
- If clearing the staging area fails after a successful commit, the commit still returns its receipt and a warning is logged; the stale index then names an older base, and its next commit reports a conflict on the paths it staged (the committed version changed them), which `reset` clears.
- Public types `filesquid-api` (platform engineer) will wire: `ChangeModel`, `StagingArea`, `VersionMeta`, `Status`, `PathChange`, `ChangeError`, `compute_delta`.
