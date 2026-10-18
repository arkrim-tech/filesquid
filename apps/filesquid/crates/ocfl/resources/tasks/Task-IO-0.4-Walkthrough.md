# Task IO-0.4 Walkthrough: Read and browse

Module plan row: [IO-0.4](../plan.md). Global task: Task 0.3 in the [implementation plan](../../../../../../resources/implementation-plan.md), which stays open until IO-0.5. Branch `feat/io-0.4-read-browse`, based on `main` at `e697cb9` (PR #22 and IO-0.3 merged).

## OCFL-S-004: Logical state MUST be inventory-driven at a caller-pinned version; stream logical files; enumerate committed logical state.

Sources: [OCFL-S-004](../srs.md), [PRD F-051](../../../../../../resources/prd.md#f-051-ocfl-module), [T-039](../../../../../../resources/product-vision.md#canonical-ocfl-state-and-evidence), [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md) ("Read and browse", "Published head (readers)"), [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md) Decisions 2 to 4. The SRS row now also states the reader rules as built (see Sync).

## OCFL-S-010: Read and write conformance, part (a): every version of every `1.1/good-objects/` fixture resolves through `state` and `manifest`, and every streamed logical file matches its inventory digest.

Sources: [OCFL-S-010](../srs.md), [OCFL-S-011](../srs.md) (fixture pinning and handling). Part (b), writing from the `content` fixtures, is IO-0.10.

## OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root (guards run on open).

The IO-0.2 contract said callers MUST run the guards when they open a storage root or an object. The read API now does so on every open.

## What changed

- **`src/read/` (new public module `read`).** No `std::fs`, no executor; builds for `wasm32-unknown-unknown`.
  - `StorageRoot::open(&backend)`: checks the `0=ocfl_1.1` declaration (E069, E080), runs the storage-root guard and discovers the outermost object roots in the same paged listing (at most `MAX_WALK_ENTRIES` keys). `guard.rs` now exposes that one scan (`checked_layout`, `Layout::outermost_object_roots`), so opening a root walks the keys once. A storage root that also declares an object is `ReadError::ObjectAtStorageRoot`.
  - `Object::open(&backend, root)`: Git guard and hierarchy check on the object root; exactly one object declaration with exact contents (E003, E007; above 64 bytes refused before it is read); root inventory (E063 when missing; above 64 MiB refused before it is read; every `Inventory::parse` code) and its sidecar (E058, E060, E061; above 4 KiB refused before it is read); the head version inventory, when present, must be the same bytes (E064). A sidecar mismatch is read again up to `READ_ATTEMPTS` (3) times, in case a writer published a new head between the two reads.
  - `resolve_state` / `Object::state`: the logical state of a caller-pinned version through `state` and `manifest` only. A version the root inventory does not list, an unpublished `vN` above `head` included, is `ReadError::VersionNotFound`.
  - `LogicalFileReader` (`Object::stream_file`): chunks of at most `MAX_CHUNK_BYTES` (8 MiB), hashed as they pass; at the end the digest must match (E092), and a missing content file is E092. A failed stream keeps returning its error, so it can never end as if verified.
  - `Object::copy_file`: streams into a multipart upload on any backend, in parts of at most `MAX_PART_BYTES`, and publishes only after the digest matched and the caller's precondition held; otherwise the upload is aborted.
  - `version_history` / `Object::history` and `diff_versions` / `Object::diff`: history with `created`, `message` and `user`; diff as added, deleted, modified and renamed paths in path order.
  - `ReadError` with `code()`; every refusal names the full key or JSON pointer of the broken file.
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
| OCFL-S-004 (ADR 016 published head): only the root inventory commits; `vN` above `head` unpublished; sidecar and head version inventory checked; a head published during open is read again | `integration_read_objects`: `a_version_above_the_root_head_is_unpublished`, `inventory_and_sidecar_faults_have_their_codes`, `a_head_version_inventory_that_differs_from_the_root_is_e064`, `a_missing_head_version_inventory_is_allowed`, `a_head_published_during_open_is_read_again`; BDD "A version above the root head is unpublished" and the outline "An object whose root files are broken is refused when opened" |
| OCFL-S-004 (ADR 016 read and browse): version history and diff of two versions | `unit_read_state`: `history_lists_every_version_in_order_with_its_metadata`, `diff_reports_added_and_deleted_paths`, `diff_reports_renames_and_modifications`, `diff_pairs_renames_of_one_digest_in_path_order`, `diff_of_an_unknown_version_is_refused`; `integration_read_objects`: `history_and_diff_come_from_the_inventory`; BDD "Version history comes from the inventory", "Two versions are compared" |
| OCFL-S-004 (ADR 023 Decision 2): open a storage root; discover object roots | `integration_read_objects`: `a_storage_root_discovers_its_outermost_objects`, `storage_root_declaration_faults_have_their_codes`; unit `guard::tests::only_outermost_object_roots_are_discovered`; BDD "A storage root lists its outermost objects" |
| OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root (guards run on open) | `integration_read_objects`: `opening_an_object_runs_the_git_guard`, `opening_a_storage_root_runs_the_guard` |
| OCFL-S-002 / OCFL-S-017: NAMASTE, inventories and sidecars loaded with their codes and size bounds | `integration_read_objects`: `declaration_faults_have_their_codes`, `an_oversized_declaration_is_refused_before_it_is_read`, `an_oversized_sidecar_is_refused_before_it_is_read`, `sha512_objects_with_uppercase_digests_read` |
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
