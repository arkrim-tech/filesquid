# Task IO-0.9 Walkthrough: Enumerate committed logical state

Module plan row: [IO-0.9](../plan.md). Global task: [Task 0.4](../../../../../../resources/implementation-plan.md#task-0.4) (stays open: IO-0.10 remains).

## OCFL-S-004: Logical state MUST be inventory-driven at a caller-pinned version; stream logical files; enumerate committed logical state

## OCFL-S-003: Default public API is generic OCFL 1.1 (no FileSquid application-layer types)

## OCFL-S-005: Default dependencies MUST exclude Arrow, Parquet, DuckDB and Iceberg

## OCFL-S-010: Read and write conformance (a) — every good fixture resolved through inventory `state` and `manifest`

Sources: [OCFL-S-004, OCFL-S-003, OCFL-S-005, OCFL-S-007, OCFL-S-010](../srs.md), [T-039](../../../../../../resources/product-vision.md#canonical-ocfl-state-and-evidence), [PRD F-051](../../../../../../resources/prd.md#f-051-ocfl-module), [PRD F-042](../../../../../../resources/prd.md#f-042-ocfl-preservation-and-portable-recovery), [PRD F-001](../../../../../../resources/prd.md#f-001-independent-local-library-and-ownership), [PRD RC-010](../../../../../../resources/prd.md#rc-010-complete-ocfl-preservation-and-independent-recovery), [SRS-F-051](../../../../../../resources/srs.md#srs-f-051-ocfl-module), [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md) engine core item 6, [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md) "Read and browse", [ADR 005](../../../../../../resources/architecture/005-sqlite-local-postgres-hosted.md).

## What changed

- **New public API** (`src/read/enumerate.rs`, re-exported from `read` and the crate root):
  - `StorageRoot::enumerate(versions: Versions) -> CommittedState<'_, 'a, B>`.
  - `CommittedState::next_event(&mut self).await -> Option<Result<CommittedEvent, EnumerateError>>`: a pull cursor; each call does one step (open one object, resolve one version, or yield one file), so there is no inner loop and the cursor holds at most one object's inventory and one version's state. It reads no content bytes.
  - `Versions::{Head, All}` (default `Head`): each object's committed `head`, or every committed version the inventory lists, from the first through `head`, oldest first (zero-padded names such as `v001` are kept as written).
  - `CommittedEvent::{Object(CommittedObject), Version(CommittedVersion), File(CommittedFile)}`. `CommittedObject` has `root`, `id`, `head`, `digest_algorithm`, `unpublished_head`; `CommittedVersion` has `object_id`, `version`, `files` (count); `CommittedFile` has the T-039 fields `object_id`, `version`, `path`, `digest` and the content locator `content_key` (a `StorageKey` from the storage root), plus `digest_algorithm`, so each file event stands alone (objects may use sha256 or sha512).
  - `EnumerateError { root, source: ReadError }` with `code()`.
  - Order is fixed: objects in object-root order (as `StorageRoot::objects()`), versions oldest first, files in logical-path order.
- **Built on the IO-0.4 reader, no second resolver.** Each object is opened with `Object::open` (Git and hierarchy guards, declaration, root inventory and sidecar, head version inventory) and each version is resolved with `Object::state` (inventory `state` and `manifest` only). `LogicalState` gains a crate-private `into_files()` so files move into events without a copy.
- **A broken object does not stop the enumeration (engine decision, now in OCFL-S-004).** An object that cannot be opened or resolved yields one `EnumerateError` naming its discovered root (with its OCFL code where one applies), logged with `tracing::warn!`, and the next call moves on to the next object. A catalog rebuild therefore sees every readable object and a list of the broken ones, instead of nothing. A storage root that cannot be opened (for example a `.git` in an object root) is still refused by `StorageRoot::open` before any enumeration starts.
- **No new dependency.** Default features unchanged; no Arrow, Parquet, DuckDB, Iceberg, SQLite or PostgreSQL. The `wasm32-unknown-unknown` build with default features off still passes (BDD "Engine builds for the browser").
- **Docs.** Crate rustdoc (`src/lib.rs`, `src/read/mod.rs`), crate README section "Enumerate committed logical state", OCFL-S-004 detail, component diagram, root README "Run tests".

## TDD

The integration test, its support module and the BDD steps were written first. The first run did not compile, because the API did not exist yet:

```text
error[E0432]: unresolved imports `filesquid_ocfl::read::CommittedEvent`, `filesquid_ocfl::read::EnumerateError`, `filesquid_ocfl::read::Versions`
error[E0432]: unresolved imports `filesquid_ocfl::read::CommittedEvent`, `filesquid_ocfl::read::Versions`
error[E0599]: `filesquid_ocfl::StorageRoot<'_, B>` is not an iterator
error: could not compile `filesquid-ocfl` (test "integration_enumerate_state") due to 3 previous errors
```

After `src/read/enumerate.rs` was added, all 14 integration tests and the 6 new BDD scenarios passed on the first run.

## Constraint gates

| Constraint | Gate |
|---|---|
| C1 events carry object id, version, logical path, digest, content locator | `head_events_name_every_field_of_each_committed_file_in_memory` and `_on_a_filesystem`; BDD "Each committed file at head is enumerated with its locator" |
| C2 no SQLite, PostgreSQL or catalog types | BDD F-051 "Committed logical state is enumerated without a database or catalog types" (no database crate in `Cargo.toml`, no database or catalog name in the enumerator's code); existing F-051 "Core API is generic OCFL 1.1" |
| C3 resolved through `state` and `manifest`; earlier-version bytes resolve; unlisted files absent | `every_committed_version_is_enumerated_oldest_first` (`c.txt` at `obj/v1/content/b.txt`), `files_the_inventory_does_not_list_are_not_enumerated`; fixture oracle tests; BDD "Files the inventory does not list are not enumerated" |
| C4 unpublished `vN` not enumerated; E058, E060, E064 refusals surface | `a_version_above_the_committed_head_is_not_enumerated`, `a_root_inventory_its_sidecar_does_not_commit_reports_an_unpublished_head`, `an_object_whose_sidecar_or_head_inventory_is_wrong_is_refused_with_its_code`, `bad_fixtures_among_good_ones_are_reported_and_the_good_ones_still_enumerate` (E007, E058, E060, E063, E064); BDD "A version above the committed head is not enumerated" |
| C5 guards run on storage-root and object open | `a_git_directory_in_an_object_root_stops_the_storage_root_from_opening`; `Object::open` per object (existing IO-0.4 guard tests) |
| C6 from the storage root alone | every test opens only `StorageRoot::open(&backend)` and enumerates |
| C7 bounded memory and loops | one step per `next_event` call (no loop in the cursor); one object's inventory at a time; discovery bounded by `MAX_WALK_ENTRIES`; test loops capped (`MAX_EVENTS`, `MAX_TREE_DEPTH`) |
| C8 head or every version; events name the version | `Versions::Head` and `Versions::All` in the head, all-versions and fixture tests; BDD "Every committed version is enumerated oldest first" |
| C9 no new dependency; wasm32 build | `Cargo.toml` `[dependencies]` unchanged; BDD "Default dependencies exclude analytical engines", "Engine builds for the browser" |
| C10 a broken object yields one error and enumeration continues | `a_broken_object_yields_one_error_and_the_next_object_is_still_enumerated`, `bad_fixtures_among_good_ones_...`; BDD "A broken object is reported and the others are still enumerated" |
| C11 deterministic order | exact event lists in the head and all-versions tests; fixture tests compare against an oracle sorted by root, version number and logical path |
| OCFL-S-010 (a) | `every_good_and_warning_fixture_enumerates_as_its_inventory_says_in_memory` and `_on_a_filesystem`: all 12 good and 13 warning fixtures in one storage root; file rows equal an independent `serde_json` reading of each root inventory; every content key read back matches its digest |

## Test output

`cargo test -p filesquid-ocfl --locked --test integration_enumerate_state -- --nocapture --test-threads=1`:

```text
running 14 tests
test a_broken_object_yields_one_error_and_the_next_object_is_still_enumerated ... ok
test a_git_directory_in_an_object_root_stops_the_storage_root_from_opening ... ok
test a_root_inventory_its_sidecar_does_not_commit_reports_an_unpublished_head ... ok
test a_version_above_the_committed_head_is_not_enumerated ... ok
test an_empty_storage_root_enumerates_nothing ... ok
test an_object_whose_sidecar_or_head_inventory_is_wrong_is_refused_with_its_code ... ok
test bad_fixtures_among_good_ones_are_reported_and_the_good_ones_still_enumerate ... ok
test every_committed_version_is_enumerated_oldest_first ... ok
test every_good_and_warning_fixture_enumerates_as_its_inventory_says_in_memory ... in memory: 64 file events over every version, 34 at head
ok
test every_good_and_warning_fixture_enumerates_as_its_inventory_says_on_a_filesystem ... on a filesystem: 64 file events over every version
ok
test files_the_inventory_does_not_list_are_not_enumerated ... ok
test head_events_name_every_field_of_each_committed_file_in_memory ... ok
test head_events_name_every_field_of_each_committed_file_on_a_filesystem ... ok
test the_pinned_fixture_tree_is_present ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.64s
```

Full crate suite, `cargo test -p filesquid-ocfl --locked` (each target and its result):

```text
     Running unittests src/lib.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.39s
     Running tests/features.rs
112 scenarios (109 passed, 3 skipped)
485 steps (482 passed, 3 skipped)
     Running tests/integration/content_fixtures.rs
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
     Running tests/integration/enumerate_state.rs
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.64s
     Running tests/integration/fixture_conformance.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/integration/fixture_inventories.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/fixture_reads.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.74s
     Running tests/integration/fs_conformance.rs
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.31s
     Running tests/integration/hierarchy_guard.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/integration/logical_file.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.84s
     Running tests/integration/memory_conformance.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.91s
     Running tests/integration/opaque_bytes.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.14s
     Running tests/integration/read_objects.rs
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.55s
     Running tests/integration/validate_objects.rs
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s
     Running tests/integration/write_order.rs
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
     Running tests/unit/digest.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/fixity.rs
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/git_guard.rs
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
     Running tests/unit/inventory.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
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

Scenarios from that run: the 5 of the new feature, the new F-051 scenario, and the existing browser-build scenario that follows it:

```text
Feature: Enumerate committed logical state
  Scenario: Each committed file at head is enumerated with its locator
   ✔> Given an OCFL storage root in memory holding the objects "ab/obj1" with three versions and "cd/obj2" with one
   ✔  When I enumerate the committed logical state at each object's head
   ✔  Then the enumeration should be:
       | event   | object          | version | path     | content locator             |
       | object  | urn:example:one | v3      |          | ab/obj1                     |
       | version | urn:example:one | v3      |          |                             |
       | file    | urn:example:one | v3      | a.txt    | ab/obj1/v3/content/a.txt    |
       | file    | urn:example:one | v3      | c.txt    | ab/obj1/v1/content/b.txt    |
       | object  | urn:example:two | v1      |          | cd/obj2                     |
       | version | urn:example:two | v1      |          |                             |
       | file    | urn:example:two | v1      | only.txt | cd/obj2/v1/content/only.txt |
   ✔  And every enumerated locator should hold bytes that match the file's digest
  Scenario: Every committed version is enumerated oldest first
   ✔> Given an OCFL storage root in memory holding the objects "ab/obj1" with three versions and "cd/obj2" with one
   ✔  When I enumerate the committed logical state of every version
   ✔  Then the versions enumerated for "urn:example:one" should be "v1, v2, v3"
   ✔  And version "v2" of "urn:example:one" should list "a.txt, c.txt, d.txt"
  Scenario: Files the inventory does not list are not enumerated
   ✔> Given an OCFL storage root in memory holding the objects "ab/obj1" with three versions and "cd/obj2" with one
   ✔  Given a stray file "v3/content/stray.txt" in the object "ab/obj1"
   ✔  When I enumerate the committed logical state of every version
   ✔  Then no enumerated file should be "stray.txt"
  Scenario: A version above the committed head is not enumerated
   ✔> Given an OCFL storage root in memory holding the objects "ab/obj1" with three versions and "cd/obj2" with one
   ✔  Given version "v4" content and a version inventory exist in "ab/obj1" while its root head is "v3"
   ✔  When I enumerate the committed logical state of every version
   ✔  Then the versions enumerated for "urn:example:one" should be "v1, v2, v3"
  Scenario: A broken object is reported and the others are still enumerated
   ✔> Given an OCFL storage root in memory holding the objects "ab/obj1" with three versions and "cd/obj2" with one
   ✔  Given the object "b/broken" in the storage root has no root inventory
   ✔  When I enumerate the committed logical state at each object's head
   ✔  Then the enumeration should report "b/broken" with OCFL code "E063"
   ✔  And the objects enumerated should be "ab/obj1, cd/obj2"
  Scenario: Committed logical state is enumerated without a database or catalog types
   ✔  When I inspect the filesquid-ocfl public API
   ✔  Then a committed logical-state enumerator should be available
   ✔  And it should need no SQLite, PostgreSQL or FileSquid catalog types
  Scenario: Engine builds for the browser
```

Local gate, `FULL=1 bash scripts/pre-commit.sh` from the repository root (run with a private `CARGO_TARGET_DIR`, so no other worktree's build could be reused):

```text
✅ All Pre-Commit Hooks Passed! Test scope: full (849s)
```

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no percentage is reported. Every constraint above has at least one test. The integration tests reach the cursor's file, next-version, end-of-object, object-opened and object-refused paths. The error path after an object has opened (a version that cannot be resolved, or a content path that is not a storage key) is defensive and is not reached: `Inventory::parse` already refuses those inventories (E050, E099, E100), so no stored object can produce it.

## Sync Execution Report

- **Module SRS:** OCFL-S-004 gains the enumerator's testable detail: the five T-039 fields per committed file, head or every version as the caller chooses, fixed order, storage root alone, no SQLite, PostgreSQL or application types, one object's inventory at a time, and a broken object reported with its root (and its OCFL code when it breaks an OCFL rule) without stopping the others. It restates T-039, ADR 023 item 6 and ADR 016; the last clause records the engine decision above.
- **Module plan:** IO-0.9 checked; Related Tests add the new feature, the integration test, its support module and this Walkthrough.
- **Repository-root implementation-plan checklist:** IO-0.9 checked; MVP progress bar redrawn (`scripts/docs/update_mvp_progress.py`). Global Task 0.4 stays open (IO-0.10).
- **Crate README:** new "Enumerate committed logical state" section. **Root README:** "Run tests" names `integration_enumerate_state` among the fixture tests.
- **Crate rustdoc:** `src/lib.rs` and `src/read/mod.rs` list the enumerator.
- **Component diagram:** the state enumerator is now BUILT (IO-0.9).
- **PRD, global SRS, product vision, ADRs:** unchanged; they already require the enumerator (T-039, SRS-F-051, ADR 023, ADR 016) and no new technology or dependency was added.
- **UI feature** `ocfl_preservation_and_portable_recovery_F-042.feature` is owned by `apps/filesquid/ui` and not edited.

## Mocked Environment Variables

None. No credential or `.env` value was mocked.

## Remaining limits and notes for callers

- **`filesquid-api` wiring (platform engineer):** catalog rebuild can call `StorageRoot::open(&backend)` then `enumerate(Versions::Head)` and consume `CommittedEvent`s; `EnumerateError`s should reach the activity journal and the Monitor health view as broken objects.
- **Snapshot of discovery:** objects added after `StorageRoot::open` are not enumerated; a writer that commits during enumeration is seen at whatever head `Object::open` reads for that object, and the event names that version.
- **No fixity here:** the enumerator does not read content bytes. A caller that needs verified bytes streams them (`Object::stream_file`, E092) or validates the object (`validate_object`).
