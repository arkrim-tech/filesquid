# Task IO-0.1 Walkthrough: Storage backend trait

Global task: [Task 0.3](../../../../../../resources/implementation-plan.md) (first slice; IO-0.2 to IO-0.5 remain open, so the global box stays unchecked). Module plan: [plan.md](../plan.md). Branch: `feat/io-0.1-storage-backend`.

## OCFL-S-007: The engine MUST reach bytes only through an async, runtime-agnostic storage backend interface

Full requirement text ([module SRS](../srs.md)): "The engine MUST reach bytes only through an async, runtime-agnostic storage backend interface (local filesystem on native; S3-compatible on native and `wasm32`; in-memory for tests). The engine core MUST NOT call `std::fs`, `tokio::fs` or a network client directly, and MUST build for `wasm32-unknown-unknown` with default features off."

Also covered: OCFL-S-003 (default public API is generic OCFL 1.1; stand-alone crate), OCFL-S-005 (no Arrow, Parquet, DuckDB or Iceberg by default), OCFL-S-001 (Git guard kept on the path helpers); [PRD F-051: OCFL module](../../../../../../resources/prd.md#f-051-ocfl-module), [PRD F-042: OCFL preservation and portable recovery](../../../../../../resources/prd.md#f-042-ocfl-preservation-and-portable-recovery), [C-053](../../../../../../resources/product-vision.md#module-rules), [ADR 025](../../../../../../resources/architecture/025-client-side-filesquid-engines.md) Decisions 2 and 3, [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md) Decision 1, [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md), [ADR 012](../../../../../../resources/architecture/012-s3-compatible-ciphertext-storage.md) conflict guard.

## What changed

- **`src/storage/` (new, portable):**
  - `backend.rs`: `StorageBackend`, the one async storage interface: `put`, `put_if` (compare-and-swap), `get_range`, `head`, `exists`, paged `list`, `delete` (staging and recovery only), `create_multipart`, `upload_part`, `complete_multipart`, `abort_multipart`. Methods return `impl Future + MaybeSend`; `MaybeSend`/`MaybeSync` are `Send`/`Sync` on native targets and empty on `wasm32`, so browser futures carry no `Send` bound. No executor dependency.
  - `key.rs`: `StorageKey`, a `/`-separated relative path. Empty keys, a leading `/`, empty, `.` or `..` segments, `\` and NUL are rejected before any I/O.
  - `types.rs`: `ETag` (lowercase hex SHA-256 on the shipped backends), `ObjectMeta`, `Precondition` (`IfAbsent`, `IfMatch`), `ByteRange`, `ListPage`, `UploadId`, `CompletedPart`, and the bounds `MAX_CHUNK_BYTES` (8 MiB), `MAX_PUT_BYTES` (8 MiB), `MAX_PARTS` (10 000), `MAX_LIST_PAGE` (1 000), `MAX_WALK_DEPTH` (64), `MAX_WALK_ENTRIES` (1 000 000), `MAX_OBJECT_BYTES` (5 TiB). The shared pure checks (precondition, part order, page cutting) live here so both backends apply them the same way.
  - `stream.rs`: `ChunkReader` (streams an object in chunks of at most the requested size, bounded by `MAX_OBJECT_BYTES`) and `MultipartWriter` (writes a large object part by part; `finish` leaves the upload open on failure so the caller can `abort`).
  - `memory.rs`: `MemoryBackend`, one lock per call, true compare-and-swap. Builds on `wasm32`.
  - `fs.rs`: `FsBackend` (native only, `fs` feature). Keys map to plain files. Replacing writes go through a flushed temporary sibling that is renamed into place and removed on every error path. Conditional writes are serialised in-process; cross-process exclusion is the per-object lease of IO-0.6. Multipart parts are staged in a directory that `FsBackend::new` refuses inside the storage root, verified against their entity tags on completion and removed after completion or abort. Listings walk only the prefix's directory, skip temporary files and do not follow directory links. `delete` prunes the empty directories it leaves, stopping at the root.
- **`src/logical_file.rs` (native, `fs`):** `write_logical_file`, `read_logical_file` and `reject_git_in_ocfl_root` keep their signatures, but every byte now goes through `FsBackend`: writes above 8 MiB use multipart, reads stream 8 MiB chunks. A missing file is still `WriteError::Io` with `ErrorKind::NotFound`, which `filesquid-metadata::store::load` relies on. The futures are driven by one poll with a no-op waker (the filesystem backend completes on its first poll); a `Pending` result is returned as an error.
- **`src/error.rs`:** `OcflError` and `WriteError` moved here; `WriteError::Storage` carries other backend errors.
- **`Cargo.toml`:** `fs` feature (default on); `sha2` 0.11 and `tracing` 0.1 (both already in the workspace lockfile; `sha2` is the ADR 009 SHA-256 implementation; `tracing` logs the cleanup failures that cannot be returned). Path-helper and filesystem tests declare `required-features = ["fs"]`.
- **`wasm32` gate:** the F-051 scenario "Engine builds for the browser" runs `cargo build -p filesquid-ocfl --locked --no-default-features --target wasm32-unknown-unknown` in its own target directory under Cargo's test temp dir. It runs inside `cargo test`, so `scripts/verify_tests.sh` (pre-commit step 9) enforces it. This satisfies the plan's "add that CI build" under the owner decision of 2026-10-17 (no hosted CI).
- **BDD:** new [`storage_backend_OCFL-S-007.feature`](../../tests/features/storage_backend_OCFL-S-007.feature) (round trip, stale compare-and-swap, create-only conflict, bounded streaming of a multipart file, paged listing, key escape, chunk bound; each run on the in-memory and the local filesystem backend), plus step definitions for the existing F-051 scenarios "Storage goes through one async backend interface" and "Engine builds for the browser".
- **Docs:** crate [README](../../README.md) (storage backends section, `wasm32` build), root README (wasm32 target prerequisite and the test note), [component diagram](../component-diagram.html) status rows, module plan (IO-0.1 checked, test links).

## Not done here (follow-up)

- **`filesquid-metadata` store.** The plan line says to update its store to backend calls. This task was limited to the `ocfl` crate, so `filesquid-metadata` keeps calling the unchanged `write_logical_file` / `read_logical_file` signatures, which now go through `FsBackend`. Its tests pass unchanged. Moving `filesquid-metadata` to take a `StorageBackend` directly belongs with its own `wasm32` task (MD-0.4).
- **Object safety.** `StorageBackend` uses return-position `impl Future` and is not `dyn`-compatible. Callers are generic over the backend. If `filesquid-api` needs to hold a backend chosen at run time, it can wrap the shipped backends in an enum.

## SRS traceability

| Requirement | Tests |
|---|---|
| OCFL-S-007 one async storage interface (put, get, head, list, exists, delete, multipart, compare-and-swap) | `integration_backend_conformance::{memory,filesystem}::*` (12 cases per backend), feature "Bytes written through a backend read back unchanged" |
| OCFL-S-007 local filesystem (native) and in-memory backends | `integration_backend_conformance::filesystem::*`, `fs_backend_writes_plain_files_under_the_root`, F-051 "Storage goes through one async backend interface" |
| OCFL-S-007 engine core does not call `std::fs`, `tokio::fs` or a network client | F-051 step "the engine core should not call std::fs directly" (scans every source file except `storage/fs.rs` and `logical_file.rs`) |
| OCFL-S-007 builds for `wasm32-unknown-unknown` with default features off; no `Send` on `wasm32` | F-051 "Engine builds for the browser"; `cargo clippy -p filesquid-ocfl --no-default-features --target wasm32-unknown-unknown` |
| ADR 025 bounded chunks; no whole large file in memory | `chunk_reader_bounds`, `single_put_is_bounded`, `multipart_rejects_bad_parts`, `multipart_round_trip_streams_in_chunks`, `unit_storage_key::byte_ranges_are_bounded_by_the_chunk_limit`, `limits_match_the_plan_constraints`, feature "Large files stream in bounded chunks", "A chunk larger than the maximum chunk size is refused" |
| ADR 012 compare-and-swap; a competing write never silently overwrites a head | `compare_and_swap`, `multipart_complete_honours_cas`, features "A stale compare-and-swap does not overwrite the head", "Creating a key that already exists is rejected" |
| Keys never escape the storage root | `unit_storage_key::keys_that_could_escape_the_root_are_rejected`, `list_pages_in_lexical_order` (bad prefix), feature "Keys that escape the storage root are rejected", `integration_logical_file::escaping_paths_are_rejected` |
| Bounded listing | `list_pages_in_lexical_order`, feature "Listing pages through keys in lexical order" |
| Resources closed on every path; staging outside the root | `fs_backend_leaves_no_temporary_files_or_staging`, `fs_staging_must_be_outside_the_storage_root`, `multipart_abort_leaves_nothing`, `fs_delete_prunes_empty_directories` |
| SHA-256 entity tags | `unit_storage_key::entity_tag_is_lowercase_hex_sha256`, `put_get_head_exists`, `multipart_round_trip_streams_in_chunks` |
| Path helpers replaced by backend calls; Git guard kept (OCFL-S-001) | `integration_logical_file::*`, `unit_git_guard::git_directory_is_rejected`, `cargo test -p filesquid-metadata` (unchanged, passing) |
| OCFL-S-003 generic API, stand-alone crate; OCFL-S-005 no analytical engines | F-051 "Core API is generic OCFL 1.1", "Default dependencies exclude analytical engines", "Stand-alone FOSS product" |

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no coverage percentage is reported. Every public item of `src/storage/` and `src/logical_file.rs` is exercised by the tests in the table above, and every backend operation runs against both backends.

## TDD: failing run before implementation

`cargo test -p filesquid-ocfl --no-fail-fast` with the tests written and no `storage` module:

```text
error[E0432]: unresolved import `filesquid_ocfl::storage`
error[E0432]: unresolved import `filesquid_ocfl::storage`
error[E0432]: unresolved import `filesquid_ocfl::storage`
error: could not compile `filesquid-ocfl` (test "integration_backend_conformance") due to 1 previous error
error: could not compile `filesquid-ocfl` (test "unit_storage_key") due to 1 previous error
error: could not compile `filesquid-ocfl` (test "features") due to 1 previous error
```

## Gates

Run from `apps/filesquid/`:

- `cargo fmt --all`: clean
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean
- `cargo clippy -p filesquid-ocfl --no-default-features --all-targets --locked -- -D warnings`: clean
- `cargo clippy -p filesquid-ocfl --no-default-features --target wasm32-unknown-unknown --locked -- -D warnings`: clean
- `cargo build -p filesquid-ocfl --target wasm32-unknown-unknown --no-default-features --locked`: built
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`: clean
- `cargo test -p filesquid-ocfl --locked`: 42 tests and 19 scenarios (73 steps) passed (raw output below)
- `cargo test -p filesquid-metadata --locked`: passed unchanged
- `bash scripts/pre-commit.sh` from the repository root: passed, all steps (0 to 9), with `TMPDIR` set as described under Mocked Environment Variables. The first run failed only in `filesquid-api`'s F-061 scenario "A stalled process degrades health live": that test measures free space on the temporary directory's volume, the build container had under 5 GiB free (the API's `DEGRADED_FREE_BYTES`), so health read `degraded` instead of `ok`. This is an environment condition, not a change in this branch.

The nine skipped scenarios have no step definitions yet: six fixture-conformance scenarios (IO-0.4, IO-0.5, IO-0.7) and three F-051 scenarios (hosted-service linkage, Drive-style browser, sidebar slots). They were skipped before this task too. The JUnit report is written to `tests/allure-results/cucumber-junit.xml`.

## Mocked Environment Variables

- `TMPDIR` pointed at a directory on a RAM-backed `tmpfs` for the `bash scripts/pre-commit.sh` run, because the build container's disk had under 5 GiB free and `filesquid-api`'s health test reads free space from the temporary directory. No credentials or `.env` values were mocked. On a normal developer machine with at least 5 GiB free, no override is needed.
- `CARGO_TARGET_DIR` pointed at a Cargo target directory shared with other build sessions; this changes where artifacts go, not what is tested.

## Raw test output

`cargo test -p filesquid-ocfl --locked` (the shared Cargo target directory is shown as `$CARGO_TARGET_DIR` and the checkout as `$REPO`):

```text
    Blocking waiting for file lock on build directory
   Compiling filesquid-ocfl v0.1.0 ($REPO/apps/filesquid/crates/ocfl)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.67s
     Running unittests src/lib.rs ($CARGO_TARGET_DIR/debug/deps/filesquid_ocfl-bc67e4259894bec6)

running 2 tests
test logical_file::tests::missing_git_is_ok ... ok
test logical_file::tests::pending_futures_are_reported_not_dropped_silently ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/features.rs ($CARGO_TARGET_DIR/debug/deps/features-c0f168a99832841b)
Feature: OCFL 1.1 community fixture conformance
  Scenario: Valid OCFL 1.1 fixture objects pass validation
   ?  Given the pinned OCFL 1.1 community fixtures
      Step skipped: tests/features/OCFL_1.1_fixture_conformance.feature:9:5
  Scenario: Warning OCFL 1.1 fixture objects report their warning codes
   ?  Given the pinned OCFL 1.1 community fixtures
      Step skipped: tests/features/OCFL_1.1_fixture_conformance.feature:14:5
  Scenario: Invalid OCFL 1.1 fixture objects are rejected
   ?  Given the pinned OCFL 1.1 community fixtures
      Step skipped: tests/features/OCFL_1.1_fixture_conformance.feature:20:5
  Scenario: Valid OCFL 1.1 fixture objects are readable at every version
   ?  Given the pinned OCFL 1.1 community fixtures
      Step skipped: tests/features/OCFL_1.1_fixture_conformance.feature:26:5
  Scenario: Objects written from OCFL 1.1 content fixtures pass independent validation
   ?  Given the pinned OCFL 1.1 community fixtures
      Step skipped: tests/features/OCFL_1.1_fixture_conformance.feature:31:5
  Scenario: Community fixtures are pinned test data only
   ?  When I inspect the OCFL fixture pin
      Step skipped: tests/features/OCFL_1.1_fixture_conformance.feature:38:5
Feature: OCFL module (filesquid-ocfl)
  Scenario: Core API is generic OCFL 1.1
   ✔  When I inspect the filesquid-ocfl public API
   ✔  Then FileSquid catalog, recipe, C2PA, and collection-snapshot types should be absent
  Scenario: Default dependencies exclude analytical engines
   ✔  When I inspect filesquid-ocfl library dependencies
   ✔  Then Arrow, Parquet, DuckDB, and Iceberg should be absent from default features
  Scenario: Stand-alone FOSS product
   ✔  When I inspect filesquid-ocfl product identity
   ✔  Then it should remain installable without the FileSquid app
   ✔  And it should build and publish without other FileSquid crates
   ✔  And community OCFL programs should not be the engine
  Scenario: FileSquid hosts link the engine library
   ✔  When I inspect the desktop crate
   ✔  Then desktop Cargo should depend on filesquid-ocfl
   ✔  And FileSquid should not shell out to fsquid for canonical writes
  Scenario: Storage goes through one async backend interface
   ✔  When I inspect the filesquid-ocfl public API
   ✔  Then a storage backend trait with local filesystem and in-memory backends should be available
   ✔  And the engine core should not call std::fs directly
  Scenario: Engine builds for the browser
   ✔  When I build filesquid-ocfl for wasm32-unknown-unknown with default features off
   ✔  Then the build should succeed
  Scenario: The ciphertext-only hosted service never links the engine
   ?  When I inspect the hosted backup and sync service crate
      Step skipped: tests/features/reusable_ocfl_engine_F-051.feature:37:5
  Scenario: Drive-style browser shows storage facts only
   ?  Given an OCFL object with two versions
      Step skipped: tests/features/reusable_ocfl_engine_F-051.feature:41:5
  Scenario: Browser exposes sidebar panel slots
   ?  When I inspect the filesquid-ocfl UI package dependencies
      Step skipped: tests/features/reusable_ocfl_engine_F-051.feature:48:5
Feature: Storage backend interface
  Scenario Outline: Bytes written through a backend read back unchanged
   ✔  Given an empty in-memory storage backend
   ✔  When I put "hello OCFL" at "objects/a/inventory.json"
   ✔  Then "objects/a/inventory.json" should exist with size 10
   ✔  And streaming "objects/a/inventory.json" should return "hello OCFL"
  Scenario Outline: Bytes written through a backend read back unchanged
   ✔  Given an empty local filesystem storage backend
   ✔  When I put "hello OCFL" at "objects/a/inventory.json"
   ✔  Then "objects/a/inventory.json" should exist with size 10
   ✔  And streaming "objects/a/inventory.json" should return "hello OCFL"
  Scenario Outline: A stale compare-and-swap does not overwrite the head
   ✔  Given an empty in-memory storage backend
   ✔  And "objects/a/inventory.json" holds "v1" and I remember its entity tag
   ✔  When another writer replaces "objects/a/inventory.json" with "v2"
   ✔  And I replace "objects/a/inventory.json" with "v3" only if it still matches the remembered entity tag
   ✔  Then the write should be rejected as a precondition failure
   ✔  And streaming "objects/a/inventory.json" should return "v2"
  Scenario Outline: A stale compare-and-swap does not overwrite the head
   ✔  Given an empty local filesystem storage backend
   ✔  And "objects/a/inventory.json" holds "v1" and I remember its entity tag
   ✔  When another writer replaces "objects/a/inventory.json" with "v2"
   ✔  And I replace "objects/a/inventory.json" with "v3" only if it still matches the remembered entity tag
   ✔  Then the write should be rejected as a precondition failure
   ✔  And streaming "objects/a/inventory.json" should return "v2"
  Scenario Outline: Creating a key that already exists is rejected
   ✔  Given an empty in-memory storage backend
   ✔  And "objects/a/v2/reservation" holds "writer-1" and I remember its entity tag
   ✔  When I create "objects/a/v2/reservation" with "writer-2" only if it is absent
   ✔  Then the write should be rejected as a precondition failure
   ✔  And streaming "objects/a/v2/reservation" should return "writer-1"
  Scenario Outline: Creating a key that already exists is rejected
   ✔  Given an empty local filesystem storage backend
   ✔  And "objects/a/v2/reservation" holds "writer-1" and I remember its entity tag
   ✔  When I create "objects/a/v2/reservation" with "writer-2" only if it is absent
   ✔  Then the write should be rejected as a precondition failure
   ✔  And streaming "objects/a/v2/reservation" should return "writer-1"
  Scenario Outline: Large files stream in bounded chunks
   ✔  Given an empty in-memory storage backend
   ✔  And a 3 MiB file at "objects/a/v1/content/big.bin" uploaded in 1 MiB multipart parts
   ✔  When I stream "objects/a/v1/content/big.bin" in 256 KiB chunks
   ✔  Then every chunk should be at most 256 KiB
   ✔  And the streamed bytes should equal the uploaded bytes
  Scenario Outline: Large files stream in bounded chunks
   ✔  Given an empty local filesystem storage backend
   ✔  And a 3 MiB file at "objects/a/v1/content/big.bin" uploaded in 1 MiB multipart parts
   ✔  When I stream "objects/a/v1/content/big.bin" in 256 KiB chunks
   ✔  Then every chunk should be at most 256 KiB
   ✔  And the streamed bytes should equal the uploaded bytes
  Scenario Outline: Listing pages through keys in lexical order
   ✔  Given an empty in-memory storage backend
   ✔  And 5 files under "objects/"
   ✔  When I list "objects/" with a page size of 2
   ✔  Then I should receive 3 pages holding all 5 keys in lexical order
  Scenario Outline: Listing pages through keys in lexical order
   ✔  Given an empty local filesystem storage backend
   ✔  And 5 files under "objects/"
   ✔  When I list "objects/" with a page size of 2
   ✔  Then I should receive 3 pages holding all 5 keys in lexical order
  Scenario Outline: Keys that escape the storage root are rejected
   ✔  Given an empty in-memory storage backend
   ✔  When I put "x" at "../outside.txt"
   ✔  Then the write should be rejected as an invalid key
  Scenario Outline: Keys that escape the storage root are rejected
   ✔  Given an empty local filesystem storage backend
   ✔  When I put "x" at "../outside.txt"
   ✔  Then the write should be rejected as an invalid key
  Scenario: A chunk larger than the maximum chunk size is refused
   ✔  Given an empty in-memory storage backend
   ✔  When I request a read chunk one byte larger than the maximum chunk size
   ✔  Then the read should be rejected as over the limit
[Summary]
3 features
28 scenarios (19 passed, 9 skipped)
82 steps (73 passed, 9 skipped)
     Running tests/integration/backend_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_backend_conformance-e3c3310df50b956b)

running 28 tests
test filesystem::chunk_reader_bounds ... ok
test filesystem::missing_key_is_not_found ... ok
test filesystem::delete_is_idempotent ... ok
test filesystem::compare_and_swap ... ok
test filesystem::multipart_abort_leaves_nothing ... ok
test filesystem::multipart_complete_honours_cas ... ok
test filesystem::list_pages_in_lexical_order ... ok
test filesystem::multipart_rejects_bad_parts ... ok
test filesystem::single_put_is_bounded ... ok
test filesystem::put_get_head_exists ... ok
test filesystem::put_replaces ... ok
test fs_backend_writes_plain_files_under_the_root ... ok
test fs_staging_must_be_outside_the_storage_root ... ok
test memory::chunk_reader_bounds ... ok
test memory::compare_and_swap ... ok
test memory::delete_is_idempotent ... ok
test memory::list_pages_in_lexical_order ... ok
test memory::missing_key_is_not_found ... ok
test memory::multipart_abort_leaves_nothing ... ok
test fs_delete_prunes_empty_directories ... ok
test memory::multipart_complete_honours_cas ... ok
test memory::multipart_rejects_bad_parts ... ok
test memory::put_get_head_exists ... ok
test memory::put_replaces ... ok
test fs_backend_leaves_no_temporary_files_or_staging ... ok
test memory::single_put_is_bounded ... ok
test memory::multipart_round_trip_streams_in_chunks ... ok
test filesystem::multipart_round_trip_streams_in_chunks ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.62s

     Running tests/integration/logical_file.rs ($CARGO_TARGET_DIR/debug/deps/integration_logical_file-c065d6d6a8ffbeac)

running 5 tests
test escaping_paths_are_rejected ... ok
test missing_file_is_io_not_found ... ok
test git_directory_blocks_reads_and_writes ... ok
test write_then_read_round_trips_on_a_real_directory ... ok
test large_files_round_trip_through_bounded_chunks ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.23s

     Running tests/unit/git_guard.rs ($CARGO_TARGET_DIR/debug/deps/unit_git_guard-b2a01e876be21e6a)

running 1 test
test git_directory_is_rejected ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/storage_key.rs ($CARGO_TARGET_DIR/debug/deps/unit_storage_key-831ac99f0c3ba826)

running 6 tests
test byte_ranges_are_bounded_by_the_chunk_limit ... ok
test entity_tag_is_lowercase_hex_sha256 ... ok
test key_segments_are_exposed_in_order ... ok
test keys_that_could_escape_the_root_are_rejected ... ok
test relative_slash_separated_keys_are_accepted ... ok
test limits_match_the_plan_constraints ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests filesquid_ocfl

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```
