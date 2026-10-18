# Task IO-0.4 Walkthrough: Read and browse

Module plan row: [IO-0.4](../plan.md). Global task: Task 0.3 in the [implementation plan](../../../../../../resources/implementation-plan.md), which stays open until IO-0.5. Branch `feat/io-0.4-read-browse`, based on `main` at `e697cb9` (PR #22 and IO-0.3 merged).

## OCFL-S-004: Logical state MUST be inventory-driven at a caller-pinned version; stream logical files; enumerate committed logical state.

Sources: [OCFL-S-004](../srs.md), [PRD F-051](../../../../../../resources/prd.md#f-051-ocfl-module), [T-039](../../../../../../resources/product-vision.md#canonical-ocfl-state-and-evidence), [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md) ("Read and browse", "Published head (readers)"), [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md) Decisions 2 to 4. The SRS row now also states the reader rules as built (see Sync).

## OCFL-S-010: Read and write conformance, part (a): every version of every `1.1/good-objects/` fixture resolves through `state` and `manifest`, and every streamed logical file matches its inventory digest.

Sources: [OCFL-S-010](../srs.md), [OCFL-S-011](../srs.md) (fixture pinning and handling). Part (b), writing from the `content` fixtures, is IO-0.10.

## OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root (the storage root, the storage-hierarchy directories above the objects, and each object root; a `.git` inside preserved object content is allowed). The guards run on open.

The IO-0.2 contract said callers MUST run the guards when they open a storage root or an object. The read API now does so on every open.

## What changed

- **`src/read/` (new public module `read`).** No `std::fs`, no executor; builds for `wasm32-unknown-unknown`.
  - `StorageRoot::open(&backend)`: runs the storage-root guard, whose one paged listing (at most `MAX_WALK_ENTRIES` keys) also yields the root's `0=` files and the outermost object roots; it then checks the `0=ocfl_1.1` declaration (E069, E080). `guard.rs` exposes that scan (`checked_layout`, `Layout::root_declaration_names`, `Layout::outermost_object_roots`), so the reader lists the root's keys once. A storage root that also declares an object is `ReadError::ObjectAtStorageRoot`.
  - `Object::open(&backend, root)`: Git guard and hierarchy check on the object root; exactly one object declaration among the files directly in the root, with exact contents (E003, E007; above 64 bytes refused before it is read); a root declared as another OCFL version is `ReadError::UnsupportedVersion` (OCFL 1.0 is out of scope, OCFL-S-011); root inventory (E063 when missing; above 64 MiB refused before it is read; every `Inventory::parse` code) and its sidecar (E058, E060, E061; above 4 KiB refused before it is read); the head version inventory, when present, must be the same bytes (E064). A sidecar mismatch is read again up to `READ_ATTEMPTS` (3) times, in case a writer published a new head between the two reads.
  - `resolve_state` / `Object::state`: the logical state of a caller-pinned version through `state` and `manifest` only. A version the root inventory does not list, an unpublished `vN` above `head` included, is `ReadError::VersionNotFound`.
  - `LogicalFileReader` (`Object::stream_file`): chunks of at most `MAX_CHUNK_BYTES` (8 MiB), hashed as they pass; at the end the digest must match (E092), and a missing content file is E092. A failed stream keeps returning its error, so it can never end as if verified.
  - `Object::copy_file`: streams into a multipart upload on any backend, in parts of at most `MAX_PART_BYTES`, and publishes only after the digest matched and the caller's precondition held; otherwise the upload is aborted.
  - `version_history` / `Object::history` and `diff_versions` / `Object::diff`: history with `created`, `message` and `user`; diff as added, deleted, modified and renamed paths in path order.
  - `ReadError` with `code()`. Format refusals name the full key, or the key and JSON pointer, of the broken file where a file is involved; version, path, storage, guard and engine-scope refusals name what they are about instead.
- **`src/error.rs`**: `OcflError` is now `Clone` (so `ReadError` can be).
- **Tests.** `tests/unit/read_state.rs` (pure resolution, history, diff); `tests/integration/read_objects.rs` (every scenario on the in-memory backend and on a real directory through `FsBackend`, plus a backend that publishes a new head during open); `tests/integration/fixture_reads.rs` (pinned fixtures); `tests/support/object_builder.rs` (builds forward-delta test objects in any backend); [`read_and_browse_OCFL-S-004.feature`](../../tests/features/read_and_browse_OCFL-S-004.feature) with steps in `tests/support/read_steps.rs`; steps for the read scenario of [`OCFL_1.1_fixture_conformance.feature`](../../tests/features/OCFL_1.1_fixture_conformance.feature).
- **`@fixtures` tag.** `OCFL_1.1_fixture_conformance.feature` is tagged `@fixtures` and runs only when `FILESQUID_OCFL_FIXTURES` names the fixtures (`tests/features.rs`), so its read scenario runs for real when they are present and the scenarios whose steps belong to IO-0.5 and IO-0.10 are not reported without them.
- **Docs.** Crate [README](../../README.md) "Read and browse"; root README "Run tests" (the fixture variable); module [SRS](../srs.md) OCFL-S-004; module [plan](../plan.md); [component diagram](../component-diagram.html); repository-root implementation-plan checklist.

## Read conformance against the pinned fixtures

The fixtures (`OCFL/fixtures` commit `d323fc207126becec066998b43408d998bf0fb1b`) were cloned read-only outside the repository and the `1.1` tree passed with `FILESQUID_OCFL_FIXTURES`. They are not committed; IO-0.5 adds them under `tests/`.

- **good-objects:** all 12 objects open; every version from `v1` to `head` resolves; all 34 logical files stream and match their inventory digests (sha512, uppercase digests and a renamed content directory included).
- **warn-objects:** all 13 objects read the same way (30 logical files).
- **bad-objects a reader meets:** 8 are refused with the code their name lists (E003 twice, E007, E058, E060, E063, E064, E092).
- **Not a read failure:** `E092_E093_content_path_does_not_exist` lists two content paths for one digest and only the second is missing, so the logical file reads correctly from the first. Reporting the missing path is structural validation (IO-0.5). The remaining bad fixtures break rules a reader does not check (extra files, version-directory inventories, extension directories) or rules one inventory decides (IO-0.3); IO-0.5 validates them all.

## Sync

- Module SRS OCFL-S-004 now states the reader rules as built: resolution through `state` and `manifest` only, the published-head rules of ADR 016 with their codes (E058, E060, E061, E064), 8 MiB chunks, E092 on a digest mismatch, no copy published unless every byte matched, and the guards on open. Nothing was added beyond ADR 016, ADR 023 and OCFL 1.1.
- Module plan IO-0.4 lists OCFL-S-001, OCFL-S-002 and OCFL-S-017 as related requirements and the new tests.
- Root README "Run tests" now names `FILESQUID_OCFL_FIXTURES` (introduced by IO-0.3, not documented there before).
- PRD, global SRS, product vision and ADRs: unchanged. F-051 and SRS-F-051 already require read and browse.

## Remaining limits

- Until IO-0.5 commits the fixtures, the pre-commit hook runs the fixture tests without them, so they report that they skipped; the run above is recorded here instead.
- `LogicalState` is one object's state at one version. The storage-root-wide enumerator of committed logical state is IO-0.9.
- A reader verifies the content it streams, not every content path: validation of the whole object is IO-0.5.
- A storage root's objects are discovered by their declarations, not through a storage layout extension (`ocfl_layout.json`), which OCFL makes optional.

## SRS traceability

| Requirement | Tests |
|---|---|
| OCFL-S-004: Logical state MUST be inventory-driven at a caller-pinned version (through `state` and `manifest`; older content paths resolve) | `unit_read_state`: `state_is_resolved_through_the_manifest_at_the_pinned_version`, `unchanged_bytes_resolve_to_the_older_version_directory`, `a_version_the_inventory_does_not_list_is_refused`, `a_state_digest_missing_from_the_manifest_is_e050`; `integration_read_objects`: `every_version_resolves_through_state_and_manifest`, `files_outside_the_inventory_are_not_logical_state`, `only_versions_and_paths_in_the_inventory_can_be_pinned`; BDD "Logical state is resolved through the inventory at a pinned version", "Files that the inventory does not list are not logical state" |
| OCFL-S-004: enumerate committed logical state (object id, version, logical path, digest, content path) | `unit_read_state`: `every_file_is_enumerated_with_its_digest_and_content_path`; `integration_read_objects`: `an_object_at_the_backend_root_opens` |
| OCFL-S-004: stream logical files (bounded chunks, digest checked, E092, copy never published on mismatch) | `integration_read_objects`: `every_logical_file_streams_in_bounded_chunks`, `content_that_does_not_match_its_digest_is_e092`, `content_missing_from_storage_is_e092`, `a_logical_file_copies_into_another_key`, `a_copy_that_fails_its_digest_is_never_published`, `sha512_objects_with_uppercase_digests_read`; BDD "Every logical file streams with bytes that match its inventory digest", "Content that does not match its digest is refused" |
| OCFL-S-004 (ADR 016 published head): only the root inventory commits; `vN` above `head` unpublished; sidecar and head version inventory checked; a head published during open is read again | `integration_read_objects`: `a_version_above_the_root_head_is_unpublished`, `inventory_and_sidecar_faults_have_their_codes`, `a_head_version_inventory_that_differs_from_the_root_is_e064`, `a_missing_head_version_inventory_is_allowed`, `a_head_published_during_open_is_read_again`, `a_head_version_inventory_removed_during_open_counts_as_missing`, `objects_with_ten_or_more_versions_open`; BDD "A version above the root head is unpublished" and the outline "An object whose root files are broken is refused when opened" |
| OCFL-S-004 (ADR 016 read and browse): version history and diff of two versions | `unit_read_state`: `history_lists_every_version_in_order_with_its_metadata`, `diff_reports_added_and_deleted_paths`, `diff_reports_renames_and_modifications`, `diff_pairs_renames_of_one_digest_in_path_order`, `diff_of_an_unknown_version_is_refused`; `integration_read_objects`: `history_and_diff_come_from_the_inventory`; BDD "Version history comes from the inventory", "Two versions are compared" |
| OCFL-S-004 (ADR 023 Decision 2): open a storage root; discover object roots | `integration_read_objects`: `a_storage_root_discovers_its_outermost_objects`, `storage_root_declaration_faults_have_their_codes`; unit `guard::tests::only_outermost_object_roots_are_discovered`; BDD "A storage root lists its outermost objects" |
| OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root (guards run on open) | `integration_read_objects`: `opening_an_object_runs_the_git_guard`, `opening_a_storage_root_runs_the_guard` |
| OCFL-S-002 / OCFL-S-017: NAMASTE, inventories and sidecars loaded with their codes and size bounds | `integration_read_objects`: `declaration_faults_have_their_codes`, `files_under_a_0_prefixed_directory_are_not_declarations`, `roots_declared_as_another_ocfl_version_are_refused`, `an_unknown_ocfl_version_is_refused_as_unsupported`, `declaration_listings_with_a_bad_continuation_are_refused`, `a_storage_root_with_too_many_0_files_is_refused`, `an_oversized_declaration_is_refused_before_it_is_read`, `an_oversized_sidecar_is_refused_before_it_is_read`, `a_file_that_grows_during_a_read_is_never_buffered_past_its_bound`, `sha512_objects_with_uppercase_digests_read` |
| OCFL-S-010 (a): every good fixture version resolves and every streamed file matches its digest | `integration_fixture_reads`: `every_version_of_every_good_fixture_reads_with_matching_digests`, `every_warning_fixture_reads_with_matching_digests`, `bad_fixtures_a_reader_meets_are_refused_with_their_codes`; BDD "Valid OCFL 1.1 fixture objects are readable at every version" (run above) |
| OCFL-S-011: fixtures pinned, test data only, never loaded by the library | fixtures read only from `FILESQUID_OCFL_FIXTURES` by tests; `unit_source_scan` (no filesystem I/O in `src/read/`) |
| OCFL-S-007: bytes only through the storage backend; `wasm32` build | `unit_source_scan`; BDD "Engine builds for the browser"; `cargo build --target wasm32-unknown-unknown` below |

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no percentage is reported. Every `ReadError` variant and every OCFL code the reader returns (E003, E007, E050, E058, E060, E061, E063, E064, E069, E080, E092) has a test that triggers it, except `ReadError::ObjectAtStorageRoot`, whose discovery rule is unit-tested in `guard.rs` (`only_outermost_object_roots_are_discovered`).

## TDD: failing run before implementation

Tests written first, with no `read` module (`cargo test -p filesquid-ocfl --locked --no-fail-fast --test unit_read_state --test integration_read_objects --test integration_fixture_reads --test features`, error lines with counts):

```text
      4 error[E0432]: unresolved import `filesquid_ocfl::read`
      4 error[E0277]: the size for values of type `[_]` cannot be known at compilation time
      1 error: could not compile `filesquid-ocfl` (test "unit_read_state") due to 1 previous error
      1 error: could not compile `filesquid-ocfl` (test "integration_read_objects") due to 1 previous error
      1 error: could not compile `filesquid-ocfl` (test "integration_fixture_reads") due to 3 previous errors
      1 error: could not compile `filesquid-ocfl` (test "features") due to 3 previous errors
```

The `[_]` errors follow from the missing types. After the module existed, the first full run passed except one fixture expectation: `E092_E093_content_path_does_not_exist` reads correctly (see above), so it was removed from the refusal list with the reason in the test. Self-review then found that a stream whose digest check failed returned `Ok(None)` on the next call; the regression assertions in `content_that_does_not_match_its_digest_is_e092` were added with the fix, which keeps the failure.

## Review round 1 (Cubic)

Cubic reviewed `3d058a4` and raised 12 findings. Each was checked against the code and the contract; all 12 were valid and are fixed.

- **P1, bounded small reads (`small.rs`).** `read_small` checked the size once and then called `read_verified`, which takes a fresh size: a writer that replaced a sidecar or inventory with a huge object between the two could make it buffer the whole new object. `read_small` now runs the verified-read loop itself (`read_once` and `is_consistent` are crate-visible in `storage/whole.rs`): every attempt checks the size against the bound before any byte is read and reads at most that many bytes, and a changed object is retried, at most `READ_ATTEMPTS` times. Test: `a_file_that_grows_during_a_read_is_never_buffered_past_its_bound` grows the sidecar to 1 MiB after the reader's first `head` and asserts the refusal is `TooLarge` and that at most 4 KiB of it was ever served.
- **Head version inventory race (`object.rs`).** `exists` then `read_small` failed if the optional `vHEAD/inventory.json` vanished in between. It is now read directly and `NotFound` means absent. Test: `a_head_version_inventory_removed_during_open_counts_as_missing`.
- **Nested `0=` paths (`declaration.rs`).** Listings are recursive, so files under a directory such as `obj/0=data/` filled the one listing page and the object was refused as E003. Declarations are now the `0=` files directly in the root, collected over a bounded number of pages with a progress check. Test: `files_under_a_0_prefixed_directory_are_not_declarations` (20 files under `obj/0=data/`).
- **OCFL 1.0 declarations.** OCFL-S-011 puts OCFL 1.0 objects out of scope and no ADR adds them (`resources/architecture/` has no 1.0 support), so a 1.0 object or storage root is now refused as `ReadError::UnsupportedVersion`. It has no OCFL code, because it is the engine's scope rather than an OCFL rule. Test: `roots_declared_as_another_ocfl_version_are_refused`.
- **Storage root walked twice (`root.rs`).** The declaration lookup listed the root before the guard listed it again. The guard's layout now records the `0=` files directly in the root, so `StorageRoot::open` uses its one scan for the guard, the declaration and discovery. `FsBackend::list` re-walks the directory for each page; that is the IO-0.1 backend's design and is shared by every listing caller, so it is out of this task's scope. Unit test: `only_outermost_object_roots_are_discovered` now also checks the declaration names.
- **Builder timestamps (`object_builder.rs`).** `created` was `2026-10-1{n}`, invalid from v10. It is now `2026-10-11T00:00:00Z` plus one second per version. Test: `objects_with_ten_or_more_versions_open` (12 versions).
- **Fixture counts (`fixture_reads.rs`).** The group reads now assert the pinned counts, 12 `good-objects` and 13 `warn-objects`.
- **Reverse diff (`read_state.rs`).** The test now asserts the exact deleted changes.
- **History scenario.** The feature now gives the expected `created`, `message`, user name and address of every version in a table, and the step compares them.
- **README.** A failed copy publishes nothing new and leaves an existing object at the destination unchanged.
- **Walkthrough.** The OCFL-S-001 heading states the guard's scope as built, and the error-location sentence is limited to format refusals.

### TDD: round 1

The new integration tests were run against the round-0 sources (the round-1 `src/` set aside, the committed `src/` restored, then put back). `roots_declared_as_another_ocfl_version_are_refused` was reduced to its object half for this run, because `ReadError::UnsupportedVersion` does not exist in round 0; it fails because the 1.0 object opens. `objects_with_ten_or_more_versions_open` passes on round 0 because its fix is in the test builder itself.

```text
test a_head_version_inventory_removed_during_open_counts_as_missing ... FAILED
test a_file_that_grows_during_a_read_is_never_buffered_past_its_bound ... FAILED
test files_under_a_0_prefixed_directory_are_not_declarations ... FAILED
test roots_declared_as_another_ocfl_version_are_refused ... FAILED
thread 'a_head_version_inventory_removed_during_open_counts_as_missing' (12678) panicked at crates/ocfl/tests/integration/read_objects.rs:661:39:
thread 'a_file_that_grows_during_a_read_is_never_buffered_past_its_bound' (12676) panicked at crates/ocfl/tests/integration/read_objects.rs:646:5:
thread 'files_under_a_0_prefixed_directory_are_not_declarations' (12693) panicked at crates/ocfl/tests/integration/read_objects.rs:685:38:
thread 'roots_declared_as_another_ocfl_version_are_refused' (12700) panicked at crates/ocfl/tests/integration/read_objects.rs:397:27:
test result: FAILED. 24 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
error: test failed, to rerun pass `-p filesquid-ocfl --test integration_read_objects`
```

### Raw test output, round 1

`cargo test -p filesquid-ocfl --locked` (summary lines):

```text
     Running unittests src/lib.rs
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.27s
     Running tests/features.rs
66 scenarios (63 passed, 3 skipped)
261 steps (258 passed, 3 skipped)
     Running tests/integration/fixture_inventories.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/fixture_reads.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/fs_conformance.rs
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.84s
     Running tests/integration/hierarchy_guard.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/integration/logical_file.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.56s
     Running tests/integration/memory_conformance.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.71s
     Running tests/integration/read_objects.rs
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     Running tests/unit/digest.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/git_guard.rs
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/unit/inventory.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
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

`cargo test -p filesquid-ocfl --locked --no-default-features` (summary lines):

```text
     Running unittests src/lib.rs
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.21s
     Running tests/integration/fixture_inventories.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/memory_conformance.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.80s
     Running tests/unit/digest.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/git_guard.rs
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/unit/inventory.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
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

With `FILESQUID_OCFL_FIXTURES` set, `integration_fixture_reads` and `integration_fixture_inventories` pass (3 tests each), and the BDD run is 72 scenarios, 64 passed and 8 skipped, as in round 0.

## Review round 2 (Cubic)

Cubic reviewed `122149a` and raised 3 findings. All were valid and are fixed.

- **P1, declaration paging (`declaration.rs`).** `direct_declaration_names` checked only that a continuation moved forward, so a faulty backend that returned an empty page with a continuation, or a continuation past the page's last key, could skip the declaration. It now checks each continuation with the storage-root guard's `next_start` (crate-visible in `guard.rs`): an empty page with a continuation, or a continuation that is not the page's last key or does not move forward, is refused as a backend fault. Test: `declaration_listings_with_a_bad_continuation_are_refused` rewrites the first declaration page both ways.
- **Unbounded root declaration candidates (`guard.rs`, `root.rs`).** Round 1 moved the storage root's declaration lookup into the guard's scan, which kept every `0=` file name in the root and so dropped the 16-name bound. The bound (`MAX_DECLARATION_ENTRIES`, now in `namaste.rs` and shared by both paths) applies again: the scan keeps at most 16 names and counts the rest, and `StorageRoot::open` refuses a root with more than 16 `0=` files as E069. Tests: `a_storage_root_with_too_many_0_files_is_refused` (21 `0=` files) and the unit test `root_declaration_names_are_bounded` (40 names, 16 kept).
- **Unknown versions (`declaration.rs`).** `0=ocfl_object_2.0` did not parse as a known declaration, so the object was refused as having none (E003). Every `0=ocfl_object_<version>` (object) or `0=ocfl_<version>` (storage root) name is now a declaration of its kind. Its version, when a `MAJOR.MINOR` number other than 1.1, is `ReadError::UnsupportedVersion`, and when not a number (`0=ocfl_object_x`) is E003 or E069. Test: `an_unknown_ocfl_version_is_refused_as_unsupported` (object 2.0, object `x`, storage root 2.0).

### TDD: round 2

The three new integration tests were run against the round-1 sources before the fix:

```text
test a_storage_root_with_too_many_0_files_is_refused ... FAILED
test an_unknown_ocfl_version_is_refused_as_unsupported ... FAILED
test declaration_listings_with_a_bad_continuation_are_refused ... FAILED
test content_that_does_not_match_its_digest_is_e092 ... ok
thread 'a_storage_root_with_too_many_0_files_is_refused' (17409) panicked at crates/ocfl/tests/integration/read_objects.rs:685:50:
too many 0= files: StorageRoot { declaration: Declaration { kind: StorageRoot, spec: V1_1 }, objects: ["obj"], .. }
thread 'an_unknown_ocfl_version_is_refused_as_unsupported' (17414) panicked at crates/ocfl/tests/integration/read_objects.rs:696:5:
Format(Invalid { code: "E003", at: "obj", reason: "an object root needs exactly one conformance declaration; found 0" })
thread 'declaration_listings_with_a_bad_continuation_are_refused' (17418) panicked at crates/ocfl/tests/integration/read_objects.rs:670:9:
Format(Invalid { code: "E003", at: "obj", reason: "an object root needs exactly one conformance declaration; found 0" })
test result: FAILED. 28 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

error: test failed, to rerun pass `-p filesquid-ocfl --test integration_read_objects`
```

The first draft of the continuation test rewrote every page, so round 1's progress check refused it for the wrong reason and the test passed; it now rewrites only the first page, so the skipped keys are what it tests.

### Raw test output, round 2

`cargo test -p filesquid-ocfl --locked` (summary lines):

```text
     Running unittests src/lib.rs
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.49s
     Running tests/features.rs
66 scenarios (63 passed, 3 skipped)
261 steps (258 passed, 3 skipped)
     Running tests/integration/fixture_inventories.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/fixture_reads.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/fs_conformance.rs
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s
     Running tests/integration/hierarchy_guard.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
     Running tests/integration/logical_file.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.57s
     Running tests/integration/memory_conformance.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.86s
     Running tests/integration/read_objects.rs
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
     Running tests/unit/digest.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/git_guard.rs
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/unit/inventory.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/unit/namaste_sidecar.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/read_state.rs
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/source_scan.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/storage_key.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`--no-default-features`, the `wasm32` build, rustdoc and clippy are clean. With `FILESQUID_OCFL_FIXTURES` set, `integration_fixture_reads` and `integration_fixture_inventories` pass and the BDD run is 72 scenarios, 64 passed and 8 skipped, as before.

## Gates

Run from `apps/filesquid/` with `CARGO_TARGET_DIR` on a shared target:

| Command | Result |
|---|---|
| `cargo fmt --all` | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo test -p filesquid-ocfl --locked` | all pass; 66 BDD scenarios: 63 passed, 3 skipped (the three `reusable_ocfl_engine_F-051.feature` scenarios owned by server and UI tasks; the `@fixtures` feature is not run without the fixtures) |
| `cargo test -p filesquid-ocfl --locked --no-default-features` | all pass |
| `FILESQUID_OCFL_FIXTURES=<1.1 tree> cargo test -p filesquid-ocfl --locked --test integration_fixture_reads -- --nocapture` | 3 passed (below) |
| `FILESQUID_OCFL_FIXTURES=<1.1 tree> cargo test -p filesquid-ocfl --locked --test features` | 72 scenarios: 64 passed, 8 skipped (the read scenario runs and passes; the 5 other fixture scenarios have IO-0.5 or IO-0.10 steps, plus the 3 above) |
| `cargo build -p filesquid-ocfl --locked --no-default-features --target wasm32-unknown-unknown` | builds |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | clean |
| `bash scripts/pre-commit.sh` (repository root) | exit 0 |

## Mocked Environment Variables

None. `FILESQUID_OCFL_FIXTURES` names real, pinned fixture data; it is not a mocked value.

## Raw test output

### `cargo test -p filesquid-ocfl --locked` (summary lines)

```text
     Running unittests src/lib.rs
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.34s
     Running tests/features.rs
66 scenarios (63 passed, 3 skipped)
261 steps (258 passed, 3 skipped)
     Running tests/integration/fixture_inventories.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/fixture_reads.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/fs_conformance.rs
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.90s
     Running tests/integration/hierarchy_guard.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     Running tests/integration/logical_file.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.62s
     Running tests/integration/memory_conformance.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.94s
     Running tests/integration/read_objects.rs
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
     Running tests/unit/digest.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/git_guard.rs
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
     Running tests/unit/inventory.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
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

### `cargo test -p filesquid-ocfl --locked --no-default-features` (summary lines)

```text
     Running unittests src/lib.rs
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.34s
     Running tests/integration/fixture_inventories.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/memory_conformance.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.74s
     Running tests/unit/digest.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/git_guard.rs
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/unit/inventory.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
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

### Pinned fixtures: `integration_fixture_reads`

```text
     Running tests/integration/fixture_reads.rs
running 3 tests
test bad_fixtures_a_reader_meets_are_refused_with_their_codes ... bad-objects/E003_E063_empty: refused with Some("E003"): E003 at "E003_E063_empty": an object root needs exactly one conformance declaration; found 0
bad-objects/E003_no_decl: refused with Some("E003"): E003 at "E003_no_decl": an object root needs exactly one conformance declaration; found 0
bad-objects/E007_bad_declaration_contents: refused with Some("E007"): E007 at "E007_bad_declaration_contents/0=ocfl_object_1.1": the contents must be "ocfl_object_1.1\n"
bad-objects/E058_no_sidecar: refused with Some("E058"): E058 at "E058_no_sidecar/inventory.json.sha512": the sidecar is missing
bad-objects/E060_E064_root_inventory_digest_mismatch: refused with Some("E060"): E060 at "E060_E064_root_inventory_digest_mismatch/inventory.json.sha512": the sidecar digest does not match the inventory
bad-objects/E063_no_inv: refused with Some("E063"): E063 at "E063_no_inv/inventory.json": the object has no inventory
bad-objects/E064_different_root_and_latest_inventories: refused with Some("E064"): E064 at "E064_different_root_and_latest_inventories/v1/inventory.json": the head version inventory differs from the root inventory
bad-objects/E092_content_file_digest_mismatch: refused with Some("E092"): E092 at "E092_content_file_digest_mismatch/v1/content/test.txt": the content file does not match its inventory digest
ok
test every_version_of_every_good_fixture_reads_with_matching_digests ... good-objects/diff_files_same_md5: 1 versions, 2 logical files read
good-objects/minimal_content_dir_called_stuff: 1 versions, 1 logical files read
good-objects/minimal_logs_directory_one_log_file: 1 versions, 1 logical files read
good-objects/minimal_mixed_digests: 1 versions, 1 logical files read
good-objects/minimal_no_content: 1 versions, 0 logical files read
good-objects/minimal_one_version_one_file: 1 versions, 1 logical files read
good-objects/minimal_uppercase_digests: 1 versions, 1 logical files read
good-objects/ocfl_object_all_fixity_digests: 1 versions, 1 logical files read
good-objects/spec-ex-full: 3 versions, 9 logical files read
good-objects/spec-ex-minimal: 1 versions, 1 logical files read
good-objects/updates_all_actions: 4 versions, 13 logical files read
good-objects/updates_three_versions_one_file: 3 versions, 3 logical files read
good-objects: 12 objects read
ok
test every_warning_fixture_reads_with_matching_digests ... warn-objects/W001_W004_W005_zero_padded_versions: 4 versions, 13 logical files read
warn-objects/W001_zero_padded_versions: 3 versions, 3 logical files read
warn-objects/W002_extra_dir_in_version_dir: 1 versions, 1 logical files read
warn-objects/W004_uses_sha256: 1 versions, 1 logical files read
warn-objects/W004_versions_diff_digests: 2 versions, 2 logical files read
warn-objects/W005_id_not_uri: 1 versions, 1 logical files read
warn-objects/W007_no_message_or_user: 1 versions, 1 logical files read
warn-objects/W007_spec-ex-diff-paths: 1 versions, 2 logical files read
warn-objects/W008_user_no_address: 1 versions, 1 logical files read
warn-objects/W009_user_address_not_uri: 1 versions, 1 logical files read
warn-objects/W010_no_version_inventory: 1 versions, 1 logical files read
warn-objects/W011_version_inv_diff_metadata: 2 versions, 2 logical files read
warn-objects/W013_unregistered_extension: 1 versions, 1 logical files read
warn-objects: 13 objects read
ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.67s
```

### Pinned fixtures: BDD read scenario and summary

```text
  Scenario: Valid OCFL 1.1 fixture objects are readable at every version
   ✔  Given the pinned OCFL 1.1 community fixtures
   ✔  When I resolve every version of every object in good-objects
   ✔  Then every streamed logical file should match its inventory digest
6 features
72 scenarios (64 passed, 8 skipped)
273 steps (265 passed, 8 skipped)
```
