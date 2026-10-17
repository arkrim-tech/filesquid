# Task IO-0.1 Walkthrough: Storage backend trait

Global task: [Task 0.3](../../../../../../resources/implementation-plan.md) (first slice; IO-0.2 to IO-0.5 remain open, so the global box stays unchecked). Module plan: [plan.md](../plan.md). Branch: `feat/io-0.1-storage-backend`.

## OCFL-S-007: The engine MUST reach bytes only through an async, runtime-agnostic storage backend interface

Full requirement text ([module SRS](../srs.md)): "The engine MUST reach bytes only through an async, runtime-agnostic storage backend interface (local filesystem on native; S3-compatible on native and `wasm32`; in-memory for tests). The engine core MUST NOT call `std::fs`, `tokio::fs` or a network client directly, and MUST build for `wasm32-unknown-unknown` with default features off."

Also covered: OCFL-S-003 (default public API is generic OCFL 1.1; stand-alone crate), OCFL-S-005 (no Arrow, Parquet, DuckDB or Iceberg by default), OCFL-S-001 (Git guard kept on the path helpers); [PRD F-051: OCFL module](../../../../../../resources/prd.md#f-051-ocfl-module), [PRD F-042: OCFL preservation and portable recovery](../../../../../../resources/prd.md#f-042-ocfl-preservation-and-portable-recovery), [C-053](../../../../../../resources/product-vision.md#module-rules), [ADR 025](../../../../../../resources/architecture/025-client-side-filesquid-engines.md) Decisions 2 and 3, [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md) Decision 1, [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md), [ADR 012](../../../../../../resources/architecture/012-s3-compatible-ciphertext-storage.md) conflict guard, [ADR 027](../../../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md) (metadata store over `filesquid-ocfl`).

## What changed

- **`src/storage/` (portable):**
  - `backend.rs`: `StorageBackend`, the one async storage interface: `put`, `put_if` (compare-and-swap), `get_range`, `head`, `exists`, paged `list`, `delete` (intended for staging and recovery; the backend deletes any key it is given), `create_multipart`, `upload_part`, `complete_multipart`, `abort_multipart`. Methods return `impl Future + MaybeSend`; `MaybeSend`/`MaybeSync` are `Send`/`Sync` on native targets and empty on `wasm32`, so browser futures carry no `Send` bound. No executor dependency.
  - `drive.rs`: `complete_immediately`, which polls a backend future once with a no-op waker. The in-memory and filesystem backends never wait, so synchronous callers (path helpers, the metadata CLI, tests) use it instead of an async runtime; a future that is not ready is reported as an error.
  - `key.rs`: `StorageKey`, a `/`-separated relative path. Empty keys, a leading `/`, empty, `.` or `..` segments, `\` and NUL are rejected before any I/O.
  - `types.rs`: `ETag` (lowercase hex SHA-256 on the shipped backends), `ObjectMeta`, `Precondition` (`IfAbsent`, `IfMatch`), `ByteRange`, `ListPage`, `UploadId`, `CompletedPart`, and the bounds `MAX_CHUNK_BYTES` (8 MiB reads), `MAX_PUT_BYTES` (8 MiB), `MAX_PART_BYTES` (64 MiB), `MAX_PARTS` (10 000), `MAX_OBJECT_BYTES` (`MAX_PARTS` × `MAX_PART_BYTES` = 625 GiB, the largest object multipart can produce), `MAX_LIST_PAGE` (1 000), `MAX_WALK_DEPTH` (64), `MAX_WALK_ENTRIES` (1 000 000). The shared pure checks (precondition, part order, page cutting) live here so both backends apply them the same way.
  - `stream.rs`: `ChunkReader` (`new`, and `with_limit` for a caller-known size; each read is clamped to the limit, an object of exactly the limit ends normally and data past it is `LimitExceeded` without returning those bytes) and `MultipartWriter` (`finish` leaves the upload open on failure so the caller can `abort`).
  - `memory.rs`: `MemoryBackend`, one lock per call, true compare-and-swap. Builds on `wasm32`.
  - `fs/` (native only, `fs` feature): `FsBackend`.
    - `paths.rs`: `FsBackend::new` resolves root and staging (links, `.`, `..`; for a root that does not exist yet, the deepest existing ancestor is canonicalised and the rest applied lexically) before refusing staging at or inside the root. Every key is mapped through a confinement check: an existing component below the root that is a symbolic link is `SymlinkForbidden`; a segment starting with the reserved `.fsquid-` prefix is refused; on Windows a segment containing `:` (drive or stream), `<>"|?*` or a control character, a device name (`CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`, with or without an extension) or a trailing dot or space is refused. OCFL 1.1 content paths may contain `:`, and Unix and S3 store it as an ordinary character, so the Windows rules are not part of `StorageKey`.
    - `lock.rs`: every mutation (`put`, `put_if`, `delete`, `complete_multipart`) holds an operating-system lock (`File::try_lock`, bounded to about 10 s of 5 ms retries) on `staging/locks/<sha256 of the resolved root>.lock`, so all instances and processes that share a staging directory serialise their writes to one root.
    - `files.rs`: objects are written to a flushed `.fsquid-tmp-*` file beside the target and published by rename (`Replace`) or by hard link (`NoReplace`, used for `IfAbsent`), which the operating system refuses when the target exists, so two creators can never both win even without the shared lock. Temporary files are removed on every path.
    - `walk.rs`: listings walk only the prefix's directory, skip `.fsquid-` names, treat a prefix under a file as empty, and do not follow directory links.
    - Multipart parts are staged under `staging/uploads/<id>/`, verified against their tags on completion and removed after completion or abort. `delete` prunes the empty directories it leaves, never the root. `FsBackend::root()` returns the resolved root.
- **`src/logical_file.rs` (native, `fs`):** `write_logical_file` and `read_logical_file` keep their signatures and go through `FsBackend`: writes above 8 MiB use multipart, reads stream 8 MiB chunks. A read takes the file's tag first and checks the SHA-256 of the bytes read against it, retrying up to 3 times, so a file replaced between chunks is never returned as a mix of two versions. A missing file is still `WriteError::Io` with `ErrorKind::NotFound`. `reject_git_in_ocfl_root` checks `.git` directly on the filesystem; it does not use a backend.
- **`src/error.rs`:** `OcflError` and `WriteError`; `WriteError::Io` is labelled "logical-file I/O failed" (it covers reads too); `WriteError::Storage` carries other backend errors. `StorageError::SymlinkForbidden` is new.
- **`filesquid-metadata` store (`crates/metadata/src/store.rs`):** moved onto the backend. `load_from` and `save_to` take any `StorageBackend` and the `metadata.md` key from `metadata_key(object_prefix)`; `load` and `save` (used by the CLI) apply the Git guard and run the same code over an `FsBackend` through `complete_immediately`. The dependency on `filesquid-ocfl` stays unconditional; making it the optional `ocfl` store is MD-0.4 ([ADR 027](../../../../../../resources/architecture/027-filesquid-metadata-pluggable-stores.md)).
- **`Cargo.toml`:** `fs` feature (default on); `sha2` 0.11 and `tracing` 0.1 (both already in the workspace lockfile). Path-helper and filesystem tests declare `required-features = ["fs"]`.
- **`wasm32` gate:** the F-051 scenario "Engine builds for the browser" runs `cargo build -p filesquid-ocfl --locked --no-default-features --target wasm32-unknown-unknown` in its own target directory under Cargo's test temp dir, inside `cargo test`, so the local pre-commit hook (`scripts/verify_tests.sh`) enforces it.
- **BDD:** [`storage_backend_OCFL-S-007.feature`](../../tests/features/storage_backend_OCFL-S-007.feature) (round trip, stale compare-and-swap, create-only conflict, a multipart object read back in bounded chunks, paged listing, key escape, chunk bound; every scenario runs on both the in-memory and the local filesystem backend), plus step definitions for the F-051 scenarios "Storage goes through one async backend interface" and "Engine builds for the browser". The chunk scenarios show that each read returns a bounded chunk; they do not measure memory, and the in-memory backend holds whole objects by design.
- **Docs:** crate [README](../../README.md) (storage backends, `FsBackend` guarantees, path helpers, `wasm32` build), [metadata README](../../../metadata/README.md) (store over a backend), root README (wasm32 target prerequisite and test note), [component diagram](../component-diagram.html) (table rows and SVG badges), module plan (IO-0.1 checked, test links).

## Review round 1 (Cubic)

Fixed in this round, each with a failing test first (see the TDD section): cross-instance compare-and-swap (P0), Windows drive-prefix keys (P0), every mutation under one lock (P1), symlinked parents (P1), the chunk-reader limit boundary, the unreachable 5 TiB limit, mixed-version reads in `read_logical_file`, look-alike temporary names hidden from listings, listing under a file, aliased staging, the metadata store migration, and the documentation findings.

## Remaining limits

- **Cross-host writers.** The root lock is an advisory operating-system lock shared through the staging directory. Writers on different hosts (for example over a network share) or with different staging directories are not serialised for `IfMatch`; `IfAbsent` still has a single winner through the hard link. Exclusive writers across hosts come from the per-object OCFL lease (IO-0.6). Filesystems without hard links (for example FAT) fail `IfAbsent` writes with an I/O error rather than risk a double create.
- **Check-then-use.** Symlink and staging checks run before each operation. A process that can write inside the root could swap a directory for a link afterwards; the root MUST NOT be writable by untrusted processes.
- **Object safety.** `StorageBackend` uses return-position `impl Future` and is not `dyn`-compatible. Callers are generic over the backend; `filesquid-api` can wrap the shipped backends in an enum.

## SRS traceability

| Requirement | Tests |
|---|---|
| OCFL-S-007 one async storage interface (put, get, head, list, exists, delete, multipart, compare-and-swap) | `integration_backend_conformance::{memory,filesystem}::*` (17 cases per backend), feature "Bytes written through a backend read back unchanged" |
| OCFL-S-007 local filesystem (native) and in-memory backends | `integration_backend_conformance::filesystem::*`, `fs_backend_writes_plain_files_under_the_root`, F-051 "Storage goes through one async backend interface" |
| OCFL-S-007 engine core does not call `std::fs`, `tokio::fs` or a network client | F-051 step "the engine core should not call std::fs directly" (scans every source file except `storage/fs/` and `logical_file.rs`) |
| OCFL-S-007 builds for `wasm32-unknown-unknown` with default features off; no `Send` on `wasm32` | F-051 "Engine builds for the browser"; `cargo clippy -p filesquid-ocfl --no-default-features --target wasm32-unknown-unknown` |
| ADR 025 bounded transfers | `chunk_reader_bounds`, `single_put_is_bounded`, `multipart_rejects_bad_parts`, `parts_above_the_read_chunk_are_accepted`, `multipart_round_trip_streams_in_chunks`, `chunk_reader_stops_exactly_at_the_limit`, `chunk_reader_never_returns_bytes_past_the_limit`, `unit_storage_key::byte_ranges_are_bounded_by_the_chunk_limit`, `limits_match_the_plan_constraints`, features "Multipart objects are read back in bounded chunks", "A chunk larger than the maximum chunk size is refused" |
| ADR 012 compare-and-swap; a competing write never silently overwrites a head | `compare_and_swap`, `multipart_complete_honours_cas`, `fs_create_only_has_one_winner_across_instances_sharing_staging`, `fs_create_only_has_one_winner_across_instances_with_separate_staging`, `fs_head_swap_has_one_winner_across_instances` (8 threads, 8 instances, 5 rounds each), features "A stale compare-and-swap does not overwrite the head", "Creating a key that already exists is rejected" |
| Keys never escape the storage root | `unit_storage_key::keys_that_could_escape_the_root_are_rejected`, `fs_keys_cannot_escape_through_a_symlinked_parent`, `fs_keys_cannot_read_through_a_symlinked_file`, `fs::paths::tests::*` (Windows drive, stream and device names), `fs_reserved_internal_names_are_rejected`, feature "Keys that escape the storage root are rejected", `integration_logical_file::escaping_paths_are_rejected` |
| Bounded listing | `list_pages_in_lexical_order`, `lookalike_temporary_names_are_listed`, `listing_under_a_file_is_empty`, feature "Listing pages through keys in lexical order" |
| Resources closed on every path; staging outside the root | `fs_backend_leaves_no_temporary_files_or_staging`, `fs_staging_must_be_outside_the_storage_root`, `fs_staging_aliased_into_the_root_is_rejected`, `fs_staging_reached_through_a_symlink_into_the_root_is_rejected`, `fs_root_is_reported_canonical`, `multipart_abort_leaves_nothing`, `fs_delete_prunes_empty_directories` |
| SHA-256 entity tags | `unit_storage_key::entity_tag_is_lowercase_hex_sha256`, `put_get_head_exists`, `multipart_round_trip_streams_in_chunks` |
| Path helpers over the backend; consistent reads; Git guard kept (OCFL-S-001) | `integration_logical_file::*`, `logical_file::tests::a_read_never_mixes_two_versions`, `a_file_that_keeps_changing_is_reported`, `a_stable_file_reads_in_one_pass`, `unit_git_guard::git_directory_is_rejected` |
| Metadata store over a backend | `filesquid-metadata` `unit_store::backend_store::*` (memory backend) and the existing path tests |
| OCFL-S-003 generic API, stand-alone crate; OCFL-S-005 no analytical engines | F-051 "Core API is generic OCFL 1.1", "Default dependencies exclude analytical engines", "Stand-alone FOSS product" |

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no coverage percentage is reported. Every `StorageBackend` operation runs against both shipped backends, the path helpers and `FsBackend::new`, `FsBackend::root` and `complete_immediately` have direct tests, and the table above lists the tests per requirement. `MaybeSend`/`MaybeSync`, `UploadId::new`, `ETag::new` and the `Debug` impls have no dedicated tests.

## TDD: failing runs before implementation

First implementation, with the tests written and no `storage` module (`cargo test -p filesquid-ocfl --no-fail-fast`):

```text
error[E0432]: unresolved import `filesquid_ocfl::storage`
error[E0432]: unresolved import `filesquid_ocfl::storage`
error[E0432]: unresolved import `filesquid_ocfl::storage`
error: could not compile `filesquid-ocfl` (test "integration_backend_conformance") due to 1 previous error
error: could not compile `filesquid-ocfl` (test "unit_storage_key") due to 1 previous error
error: could not compile `filesquid-ocfl` (test "features") due to 1 previous error
```

Review round 1, with the new tests written and only the new API names added (`MAX_PART_BYTES`, `ChunkReader::with_limit`, `StorageError::SymlinkForbidden`, `complete_immediately`), before the behaviour changed (`cargo test -p filesquid-ocfl --test integration_backend_conformance`; the race tests reported 8 winners out of 8 writers):

```text
test filesystem::chunk_reader_never_returns_bytes_past_the_limit ... FAILED
test filesystem::chunk_reader_stops_exactly_at_the_limit ... FAILED
test filesystem::listing_under_a_file_is_empty ... FAILED
test filesystem::lookalike_temporary_names_are_listed ... FAILED
test fs_backend_leaves_no_temporary_files_or_staging ... FAILED
test fs_create_only_has_one_winner_across_instances_sharing_staging ... FAILED
test fs_create_only_has_one_winner_across_instances_with_separate_staging ... FAILED
test fs_keys_cannot_escape_through_a_symlinked_parent ... FAILED
test fs_keys_cannot_read_through_a_symlinked_file ... FAILED
test fs_head_swap_has_one_winner_across_instances ... FAILED
test fs_root_is_reported_canonical ... FAILED
test fs_staging_aliased_into_the_root_is_rejected ... FAILED
test fs_reserved_internal_names_are_rejected ... FAILED
test fs_staging_reached_through_a_symlink_into_the_root_is_rejected ... FAILED
test memory::chunk_reader_never_returns_bytes_past_the_limit ... FAILED
test memory::chunk_reader_stops_exactly_at_the_limit ... FAILED
test filesystem::parts_above_the_read_chunk_are_accepted ... FAILED
test memory::parts_above_the_read_chunk_are_accepted ... FAILED
test result: FAILED. 29 passed; 18 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s
```

Consistent reads, before `read_bytes` checked the tag (`cargo test -p filesquid-ocfl --lib`):

```text
test logical_file::tests::a_file_that_keeps_changing_is_reported ... FAILED
test logical_file::tests::a_read_never_mixes_two_versions ... FAILED
test result: FAILED. 10 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

Metadata store, before `load_from`, `save_to` and `metadata_key` existed (`cargo test -p filesquid-metadata`):

```text
error[E0432]: unresolved import `filesquid_ocfl::storage::complete_immediately`
error[E0432]: unresolved imports `filesquid_metadata::store::load_from`, `filesquid_metadata::store::metadata_key`, `filesquid_metadata::store::save_to`
```

## Gates

Run from `apps/filesquid/`:

- `cargo fmt --all`: clean
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: clean
- `cargo clippy -p filesquid-ocfl --no-default-features --target wasm32-unknown-unknown --locked -- -D warnings`: clean
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`: clean
- `cargo test -p filesquid-ocfl --locked`: 71 tests and 20 scenarios (76 steps) passed (raw output below); the conformance suite was also run 5 times in a row to check the race tests for flakiness, all passing
- `cargo test -p filesquid-metadata --locked`: passed, including the 3 new backend store tests
- `bash scripts/pre-commit.sh` from the repository root: passed, all steps (0 to 9), with `TMPDIR` set as described under Mocked Environment Variables. Without it, `filesquid-api`'s F-061 scenario "A stalled process degrades health live" reads less than 5 GiB free on the container's temporary volume (`DEGRADED_FREE_BYTES`) and reports `degraded`; that is an environment condition, not a change in this branch.

The nine skipped scenarios have no step definitions yet: six fixture-conformance scenarios (IO-0.4, IO-0.5, IO-0.7) and three F-051 scenarios (hosted-service linkage, Drive-style browser, sidebar slots). They were skipped before this task too. The JUnit report is written to `tests/allure-results/cucumber-junit.xml`.

## Mocked Environment Variables

- `TMPDIR` pointed at a directory on a RAM-backed `tmpfs` for the `bash scripts/pre-commit.sh` run, because the build container's disk had under 5 GiB free and `filesquid-api`'s health test reads free space from the temporary directory. No credentials or `.env` values were mocked. On a developer machine with at least 5 GiB free, no override is needed.
- `CARGO_TARGET_DIR` pointed at a Cargo target directory shared with other build sessions; this changes where artifacts go, not what is tested.

## Raw test output

`cargo test -p filesquid-ocfl --locked` (the shared Cargo target directory is shown as `$CARGO_TARGET_DIR` and the checkout as `$REPO`):

```text
   Compiling filesquid-ocfl v0.1.0 ($REPO/apps/filesquid/crates/ocfl)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 10.09s
     Running unittests src/lib.rs ($CARGO_TARGET_DIR/debug/deps/filesquid_ocfl-bc67e4259894bec6)

running 12 tests
test logical_file::tests::a_stable_file_reads_in_one_pass ... ok
test logical_file::tests::missing_git_is_ok ... ok
test storage::drive::tests::pending_futures_are_reported ... ok
test storage::drive::tests::ready_futures_return_their_output ... ok
test storage::fs::paths::tests::a_drive_segment_never_replaces_the_root ... ok
test storage::fs::paths::tests::missing_components_resolve_lexically ... ok
test storage::fs::paths::tests::windows_accepts_ordinary_names ... ok
test storage::fs::paths::tests::windows_rejects_device_names_and_trailing_dots ... ok
test logical_file::tests::a_file_that_keeps_changing_is_reported ... ok
test storage::fs::paths::tests::windows_rejects_drive_prefixes_and_streams ... ok
test logical_file::tests::a_read_never_mixes_two_versions ... ok
test storage::fs::paths::tests::windows_rules_apply_only_on_windows ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

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
  Scenario Outline: Multipart objects are read back in bounded chunks
   ✔  Given an empty in-memory storage backend
   ✔  And a 3 MiB file at "objects/a/v1/content/big.bin" uploaded in 1 MiB multipart parts
   ✔  When I stream "objects/a/v1/content/big.bin" in 256 KiB chunks
   ✔  Then every chunk should be at most 256 KiB
   ✔  And the streamed bytes should equal the uploaded bytes
  Scenario Outline: Multipart objects are read back in bounded chunks
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
  Scenario Outline: A chunk larger than the maximum chunk size is refused
   ✔  Given an empty in-memory storage backend
   ✔  When I request a read chunk one byte larger than the maximum chunk size
   ✔  Then the read should be rejected as over the limit
  Scenario Outline: A chunk larger than the maximum chunk size is refused
   ✔  Given an empty local filesystem storage backend
   ✔  When I request a read chunk one byte larger than the maximum chunk size
   ✔  Then the read should be rejected as over the limit
[Summary]
3 features
29 scenarios (20 passed, 9 skipped)
85 steps (76 passed, 9 skipped)
     Running tests/integration/backend_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_backend_conformance-e3c3310df50b956b)

running 47 tests
test filesystem::chunk_reader_bounds ... ok
test filesystem::chunk_reader_stops_exactly_at_the_limit ... ok
test filesystem::chunk_reader_never_returns_bytes_past_the_limit ... ok
test filesystem::listing_under_a_file_is_empty ... ok
test filesystem::compare_and_swap ... ok
test filesystem::delete_is_idempotent ... ok
test filesystem::lookalike_temporary_names_are_listed ... ok
test filesystem::missing_key_is_not_found ... ok
test filesystem::multipart_abort_leaves_nothing ... ok
test filesystem::list_pages_in_lexical_order ... ok
test filesystem::multipart_rejects_bad_parts ... ok
test filesystem::put_get_head_exists ... ok
test filesystem::multipart_complete_honours_cas ... ok
test filesystem::single_put_is_bounded ... ok
test filesystem::put_replaces ... ok
test fs_backend_leaves_no_temporary_files_or_staging ... ok
test fs_backend_writes_plain_files_under_the_root ... ok
test fs_create_only_has_one_winner_across_instances_sharing_staging ... ok
test fs_delete_prunes_empty_directories ... ok
test fs_create_only_has_one_winner_across_instances_with_separate_staging ... ok
test fs_keys_cannot_escape_through_a_symlinked_parent ... ok
test fs_keys_cannot_read_through_a_symlinked_file ... ok
test fs_reserved_internal_names_are_rejected ... ok
test fs_head_swap_has_one_winner_across_instances ... ok
test fs_staging_aliased_into_the_root_is_rejected ... ok
test fs_root_is_reported_canonical ... ok
test fs_staging_must_be_outside_the_storage_root ... ok
test fs_staging_reached_through_a_symlink_into_the_root_is_rejected ... ok
test memory::chunk_reader_never_returns_bytes_past_the_limit ... ok
test memory::chunk_reader_bounds ... ok
test memory::compare_and_swap ... ok
test memory::delete_is_idempotent ... ok
test memory::chunk_reader_stops_exactly_at_the_limit ... ok
test memory::listing_under_a_file_is_empty ... ok
test memory::lookalike_temporary_names_are_listed ... ok
test memory::list_pages_in_lexical_order ... ok
test memory::missing_key_is_not_found ... ok
test memory::multipart_abort_leaves_nothing ... ok
test memory::multipart_complete_honours_cas ... ok
test memory::multipart_rejects_bad_parts ... ok
test filesystem::multipart_round_trip_streams_in_chunks ... ok
test memory::put_get_head_exists ... ok
test memory::put_replaces ... ok
test memory::single_put_is_bounded ... ok
test memory::multipart_round_trip_streams_in_chunks ... ok
test filesystem::parts_above_the_read_chunk_are_accepted ... ok
test memory::parts_above_the_read_chunk_are_accepted ... ok

test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.04s

     Running tests/integration/logical_file.rs ($CARGO_TARGET_DIR/debug/deps/integration_logical_file-c065d6d6a8ffbeac)

running 5 tests
test escaping_paths_are_rejected ... ok
test git_directory_blocks_reads_and_writes ... ok
test missing_file_is_io_not_found ... ok
test write_then_read_round_trips_on_a_real_directory ... ok
test large_files_round_trip_through_bounded_chunks ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.96s

     Running tests/unit/git_guard.rs ($CARGO_TARGET_DIR/debug/deps/unit_git_guard-b2a01e876be21e6a)

running 1 test
test git_directory_is_rejected ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/unit/storage_key.rs ($CARGO_TARGET_DIR/debug/deps/unit_storage_key-831ac99f0c3ba826)

running 6 tests
test entity_tag_is_lowercase_hex_sha256 ... ok
test keys_that_could_escape_the_root_are_rejected ... ok
test limits_match_the_plan_constraints ... ok
test byte_ranges_are_bounded_by_the_chunk_limit ... ok
test relative_slash_separated_keys_are_accepted ... ok
test key_segments_are_exposed_in_order ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Doc-tests filesquid_ocfl

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```
