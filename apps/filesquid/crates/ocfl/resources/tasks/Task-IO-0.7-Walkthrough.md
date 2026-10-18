# Task IO-0.7 Walkthrough: Generic object payload

Module plan row: [IO-0.7](../plan.md). Global task: [Task 0.4](../../../../../../resources/implementation-plan.md#task-0.4) (stays open: IO-0.8 to IO-0.10).

## OCFL-S-010: Read and write conformance (b) — every `1.1/content/` fixture written one version per version directory, valid in the engine and in an independent community validator, logical state equal to the source

## OCFL-S-019: Inspection and copy never edit an object

## OCFL-S-003: Default public API is generic OCFL 1.1 (no FileSquid application-layer types)

Sources: [OCFL-S-010, OCFL-S-003, OCFL-S-011, OCFL-S-019](../srs.md) (OCFL-S-019 is a new row, see Sync), [PRD F-051](../../../../../../resources/prd.md#f-051-ocfl-module), [PRD F-042](../../../../../../resources/prd.md#f-042-ocfl-preservation-and-portable-recovery), [T-036](../../../../../../resources/product-vision.md#canonical-ocfl-state-and-evidence), [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md) "Community OCFL programs", [ADR 023](../../../../../../resources/architecture/023-filesquid-ocfl-engine-and-parquet.md).

## What changed

- **Write conformance (OCFL-S-010 (b)).** `tests/integration/content_fixtures.rs` (new test target `integration_content_fixtures`) and `tests/support/content_fixtures.rs` write each of the 7 pinned `content` fixtures through `ObjectWriter`, one version per version directory (13 versions), from caller-supplied logical files only. A `vN/` directory is that version's whole state; `README.md` and `vN_inventory.json` are not content. `spec-ex-full`'s `vN_inventory.json` files supply the object id (`ark:/12345/bcd987`) and each version's `created`, `message` and `user` as caller metadata. Each object is written on the in-memory backend and in a real directory, validated by the engine (zero errors), and read back: every version's logical paths and streamed bytes equal the source directory. The writer needed no change: the IO-0.6 write path already met OCFL-S-010 (b).
- **Community validators (test oracles).** `tests/support/community_validator.rs` runs `ocfl-validate.py`/`ocfl-validate` (ocfl-py) and `rocfl` when found on `PATH`, or the program in `FILESQUID_OCFL_COMMUNITY_VALIDATOR`. With none, the check prints `SKIPPED community OCFL validation ...` with install hints; `FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR=1` makes that a failure. They are run as separate processes from tests only: no crate dependency, no runtime dependency, never the writer (ADR 016, ADR 023, PRD F-051).
- **Copy guard (OCFL-S-019, T-036, ADR 016).** Before this task `Object::copy_file` accepted any destination, "this one included", so a copy could replace `inventory.json`, a sidecar or `vN/content/...` of a completed object, or add a file to it. New `src/read/copy_guard.rs`: before it writes, a copy checks every directory above the destination key (the backend root included, at most 64 levels) for an OCFL object or storage-root declaration of any version (a file named `0=ocfl_object_<version>` or `0=ocfl_<version>` directly in that directory) on the destination backend, and refuses with the new `ReadError::CopyIntoOcflRoot { key, root }` (no OCFL code; logged with `tracing::warn!`). Each level is one listing of at most 1,000 keys under `<dir>/0=ocfl_`; the filesystem backend's listing now enters only subdirectories whose keys can match the prefix, so the check never walks a large storage root. A directory with more keys under that prefix and no declaration among them is refused with `StorageError::LimitExceeded`, never guessed.
- **Generic API (OCFL-S-003).** The F-051 BDD step "FileSquid catalog, recipe, C2PA, and collection-snapshot types should be absent" now scans every file under `src/`, not only `lib.rs`, for `CollectionSnapshot`, `C2paImportRecord`, `RecipeOperation`, `FileObject`, `CatalogEntry` and `filesquid_core`.
- **BDD.** The scenario "Objects written from OCFL 1.1 content fixtures pass independent validation" in `OCFL_1.1_fixture_conformance.feature` now has steps (`tests/support/payload_steps.rs`); it was skipped before. New `read_only_copy_OCFL-S-019.feature` (5 scenarios: a copy to another backend, three copies into the object root, a copy into a storage root).
- **Fixtures.** Unchanged (`git status` shows nothing under `tests/fixtures/`), still `exclude`d from the package (OCFL-S-011).
- **No new dependency.** `serde_json` (already a dependency) parses the fixture fragments in tests. The `wasm32-unknown-unknown` build with default features off still passes.

## TDD

The tests were written first. Before `ReadError::CopyIntoOcflRoot` existed, the suite did not compile:

```text
error[E0599]: no variant named `CopyIntoOcflRoot` found for enum `ReadError`
   --> crates/ocfl/tests/integration/read_objects.rs:310:41
error: could not compile `filesquid-ocfl` (test "integration_read_objects") due to 1 previous error
```

The BDD write scenario was reported as skipped (no step definitions) before this task. The content-fixture integration tests passed on the IO-0.6 writer as first run: they are conformance tests of existing behaviour, and they found no defect in it.

## Per-fixture results (OCFL-S-010 (b))

| Fixture | Versions | Content written per version | filesquid-ocfl | ocfl-py 2.1.0 | rocfl 1.7.0 | Warnings (both validators) |
|---|---|---|---|---|---|---|
| `cf1` | 1 | v1: 1 file | 0 errors | VALID | VALID | W004, W007 |
| `cf2` | 3 | 1 / 1 / 1 | 0 errors | VALID | VALID | W004, W007 |
| `cf3` | 3 | 1 / 1 / 0 (v3 equals v1) | 0 errors | VALID | VALID | W004, W007 |
| `cf4` | 1 | v1: 1 file (bytes 0..255, mixed line endings) | 0 errors | VALID | VALID | W004, W007 |
| `spec-ex-diff-paths` | 1 | v1: 2 files with spaces in their names | 0 errors | VALID | VALID | W004, W007 |
| `spec-ex-full` | 3 | 3 / 1 / 0 (empty files deduplicated; v3 reinstates `image.tiff`) | 0 errors | VALID | VALID | W004, W009 |
| `spec-ex-minimal` | 1 | v1: 1 file | 0 errors | VALID | VALID | W004, W007 |

No content fixture is excluded. The warnings are expected and are not errors: W004 because the engine writes `sha256` (OCFL-S-002 fixes this; OCFL recommends `sha512`), W007 because the `cf*` fixtures give no `message` or `user`, and W009 because `spec-ex-full`'s fragments give plain e-mail addresses, which the engine writes as the caller supplied them. Logical state matched the source at all 13 versions on both backends.

## Test output

Write conformance, exactly as documented in the crate README, with both validators on `PATH` and required (absolute scratch paths replaced by `<CARGO_TARGET_TMPDIR>`):

```text
$ cd apps/filesquid
$ FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR=1 cargo test -p filesquid-ocfl --locked --test integration_content_fixtures -- --nocapture
    Finished `test` profile [unoptimized] target(s) in 4.92s
     Running tests/integration/content_fixtures.rs

running 7 tests
test spec_ex_full_keeps_the_callers_version_metadata ... ok
test the_pinned_content_fixtures_are_all_written ... ok
test unchanged_and_reinstated_bytes_are_not_written_again ... ok
test written_objects_use_sha256_and_its_sidecar ... ok
test every_content_fixture_is_written_one_version_per_directory_in_memory ... ok
filesquid-ocfl cf1: VALID (1 versions)
filesquid-ocfl cf2: VALID (3 versions)
filesquid-ocfl cf3: VALID (3 versions)
filesquid-ocfl cf4: VALID (1 versions)
filesquid-ocfl spec-ex-diff-paths: VALID (1 versions)
test earlier_versions_are_never_changed_by_later_commits ... ok
filesquid-ocfl spec-ex-full: VALID (3 versions)
filesquid-ocfl spec-ex-minimal: VALID (1 versions)
ocfl-py cf1: VALID
[W004] OCFL Object root inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W007a] OCFL Object root inventory v1 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object root inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
OCFL v1.1 Object at <CARGO_TARGET_TMPDIR>/ocfl-content-<pid>-1/root/cf1 is VALID
ocfl-py cf2: VALID
[W004] OCFL Object root inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W004] OCFL Object v1 inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W004] OCFL Object v2 inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W007a] OCFL Object root inventory v1 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007a] OCFL Object root inventory v2 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007a] OCFL Object root inventory v3 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007a] OCFL Object v1 inventory v1 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007a] OCFL Object v2 inventory v1 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007a] OCFL Object v2 inventory v2 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object root inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object root inventory v2 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object root inventory v3 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object v1 inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object v2 inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object v2 inventory v2 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
OCFL v1.1 Object at <CARGO_TARGET_TMPDIR>/ocfl-content-<pid>-1/root/cf2 is VALID
ocfl-py cf3: VALID
[W004] OCFL Object root inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W004] OCFL Object v1 inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W004] OCFL Object v2 inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W007a] OCFL Object root inventory v1 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007a] OCFL Object root inventory v2 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007a] OCFL Object root inventory v3 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007a] OCFL Object v1 inventory v1 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007a] OCFL Object v2 inventory v1 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007a] OCFL Object v2 inventory v2 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object root inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object root inventory v2 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object root inventory v3 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object v1 inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object v2 inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object v2 inventory v2 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
OCFL v1.1 Object at <CARGO_TARGET_TMPDIR>/ocfl-content-<pid>-1/root/cf3 is VALID
ocfl-py cf4: VALID
[W004] OCFL Object root inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W007a] OCFL Object root inventory v1 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object root inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
OCFL v1.1 Object at <CARGO_TARGET_TMPDIR>/ocfl-content-<pid>-1/root/cf4 is VALID
ocfl-py spec-ex-diff-paths: VALID
[W004] OCFL Object root inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W007a] OCFL Object root inventory v1 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object root inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
OCFL v1.1 Object at <CARGO_TARGET_TMPDIR>/ocfl-content-<pid>-1/root/spec-ex-diff-paths is VALID
ocfl-py spec-ex-full: VALID
[W004] OCFL Object root inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W004] OCFL Object v1 inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W004] OCFL Object v2 inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W009] OCFL Object root inventory v1 version block user description SHOULD be a mailto: or person identifier URI (see https://ocfl.io/1.1/spec/#W009)
[W009] OCFL Object root inventory v2 version block user description SHOULD be a mailto: or person identifier URI (see https://ocfl.io/1.1/spec/#W009)
[W009] OCFL Object root inventory v3 version block user description SHOULD be a mailto: or person identifier URI (see https://ocfl.io/1.1/spec/#W009)
[W009] OCFL Object v1 inventory v1 version block user description SHOULD be a mailto: or person identifier URI (see https://ocfl.io/1.1/spec/#W009)
[W009] OCFL Object v2 inventory v1 version block user description SHOULD be a mailto: or person identifier URI (see https://ocfl.io/1.1/spec/#W009)
[W009] OCFL Object v2 inventory v2 version block user description SHOULD be a mailto: or person identifier URI (see https://ocfl.io/1.1/spec/#W009)
OCFL v1.1 Object at <CARGO_TARGET_TMPDIR>/ocfl-content-<pid>-1/root/spec-ex-full is VALID
ocfl-py spec-ex-minimal: VALID
[W004] OCFL Object root inventory SHOULD use sha512 but uses sha256 as the DigestAlgorithm (see https://ocfl.io/1.1/spec/#W004)
[W007a] OCFL Object root inventory v1 version block SHOULD include a message key (see https://ocfl.io/1.1/spec/#W007)
[W007b] OCFL Object root inventory v1 version block SHOULD include a user key (see https://ocfl.io/1.1/spec/#W007)
OCFL v1.1 Object at <CARGO_TARGET_TMPDIR>/ocfl-content-<pid>-1/root/spec-ex-minimal is VALID
rocfl cf1: VALID
Object urn:example:ocfl-content:cf1 is valid with warnings
  Warnings:
    1. [W004] (root) Inventory 'digestAlgorithm' should be 'sha512'. Found: sha256
    2. [W007] (root) Inventory version 'v1' is missing recommended key 'message'
    3. [W007] (root) Inventory version 'v1' is missing recommended key 'user'
rocfl cf2: VALID
Object urn:example:ocfl-content:cf2 is valid with warnings
  Warnings:
     1. [W004] (root) Inventory 'digestAlgorithm' should be 'sha512'. Found: sha256
     2. [W007] (root) Inventory version 'v1' is missing recommended key 'message'
     3. [W007] (root) Inventory version 'v1' is missing recommended key 'user'
     4. [W007] (root) Inventory version 'v2' is missing recommended key 'message'
     5. [W007] (root) Inventory version 'v2' is missing recommended key 'user'
     6. [W007] (root) Inventory version 'v3' is missing recommended key 'message'
     7. [W007] (root) Inventory version 'v3' is missing recommended key 'user'
     8. [W004] (v2) Inventory 'digestAlgorithm' should be 'sha512'. Found: sha256
     9. [W007] (v2) Inventory version 'v1' is missing recommended key 'message'
    10. [W007] (v2) Inventory version 'v1' is missing recommended key 'user'
    11. [W007] (v2) Inventory version 'v2' is missing recommended key 'message'
    12. [W007] (v2) Inventory version 'v2' is missing recommended key 'user'
    13. [W004] (v1) Inventory 'digestAlgorithm' should be 'sha512'. Found: sha256
    14. [W007] (v1) Inventory version 'v1' is missing recommended key 'message'
    15. [W007] (v1) Inventory version 'v1' is missing recommended key 'user'
rocfl cf3: VALID
Object urn:example:ocfl-content:cf3 is valid with warnings
  Warnings:
     1. [W004] (root) Inventory 'digestAlgorithm' should be 'sha512'. Found: sha256
     2. [W007] (root) Inventory version 'v1' is missing recommended key 'message'
     3. [W007] (root) Inventory version 'v1' is missing recommended key 'user'
     4. [W007] (root) Inventory version 'v2' is missing recommended key 'message'
     5. [W007] (root) Inventory version 'v2' is missing recommended key 'user'
     6. [W007] (root) Inventory version 'v3' is missing recommended key 'message'
     7. [W007] (root) Inventory version 'v3' is missing recommended key 'user'
     8. [W004] (v2) Inventory 'digestAlgorithm' should be 'sha512'. Found: sha256
     9. [W007] (v2) Inventory version 'v1' is missing recommended key 'message'
    10. [W007] (v2) Inventory version 'v1' is missing recommended key 'user'
    11. [W007] (v2) Inventory version 'v2' is missing recommended key 'message'
    12. [W007] (v2) Inventory version 'v2' is missing recommended key 'user'
    13. [W004] (v1) Inventory 'digestAlgorithm' should be 'sha512'. Found: sha256
    14. [W007] (v1) Inventory version 'v1' is missing recommended key 'message'
    15. [W007] (v1) Inventory version 'v1' is missing recommended key 'user'
rocfl cf4: VALID
Object urn:example:ocfl-content:cf4 is valid with warnings
  Warnings:
    1. [W004] (root) Inventory 'digestAlgorithm' should be 'sha512'. Found: sha256
    2. [W007] (root) Inventory version 'v1' is missing recommended key 'message'
    3. [W007] (root) Inventory version 'v1' is missing recommended key 'user'
rocfl spec-ex-diff-paths: VALID
Object urn:example:ocfl-content:spec-ex-diff-paths is valid with warnings
  Warnings:
    1. [W004] (root) Inventory 'digestAlgorithm' should be 'sha512'. Found: sha256
    2. [W007] (root) Inventory version 'v1' is missing recommended key 'message'
    3. [W007] (root) Inventory version 'v1' is missing recommended key 'user'
rocfl spec-ex-full: VALID
Object ark:/12345/bcd987 is valid with warnings
  Warnings:
    1. [W004] (root) Inventory 'digestAlgorithm' should be 'sha512'. Found: sha256
    2. [W009] (root) Inventory version v1 user 'address' should be a URI. Found: alice@example.com
    3. [W009] (root) Inventory version v2 user 'address' should be a URI. Found: bob@example.com
    4. [W009] (root) Inventory version v3 user 'address' should be a URI. Found: cecilia@example.com
    5. [W004] (v2) Inventory 'digestAlgorithm' should be 'sha512'. Found: sha256
    6. [W009] (v2) Inventory version v1 user 'address' should be a URI. Found: alice@example.com
    7. [W009] (v2) Inventory version v2 user 'address' should be a URI. Found: bob@example.com
    8. [W004] (v1) Inventory 'digestAlgorithm' should be 'sha512'. Found: sha256
    9. [W009] (v1) Inventory version v1 user 'address' should be a URI. Found: alice@example.com
rocfl spec-ex-minimal: VALID
Object urn:example:ocfl-content:spec-ex-minimal is valid with warnings
  Warnings:
    1. [W004] (root) Inventory 'digestAlgorithm' should be 'sha512'. Found: sha256
    2. [W007] (root) Inventory version 'v1' is missing recommended key 'message'
    3. [W007] (root) Inventory version 'v1' is missing recommended key 'user'
test every_content_fixture_written_to_a_directory_passes_the_engine_and_community_validators ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.59s

```

Full crate run with both validators required (`FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR=1 cargo test -p filesquid-ocfl --locked`), result lines:

```text
     Running unittests src/lib.rs
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.31s
     Running tests/features.rs
100 scenarios (97 passed, 3 skipped)
439 steps (436 passed, 3 skipped)
     Running tests/integration/content_fixtures.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.11s
     Running tests/integration/fixture_conformance.rs
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
     Running tests/integration/fixture_inventories.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/fixture_reads.rs
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.39s
     Running tests/integration/fs_conformance.rs
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.06s
     Running tests/integration/hierarchy_guard.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/integration/logical_file.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.67s
     Running tests/integration/memory_conformance.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.81s
     Running tests/integration/read_objects.rs
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
     Running tests/integration/validate_objects.rs
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s
     Running tests/integration/write_order.rs
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
     Running tests/unit/digest.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
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

The 3 skipped BDD scenarios belong to the server crate and the UI package (hosted-service manifest, Drive-style browser, sidebar slots); before this task 4 were skipped, the fourth being the content-fixture write scenario that now runs.

BDD output of the two features this task touched (from the same run):

```text
Feature: OCFL 1.1 community fixture conformance
  Scenario: Valid OCFL 1.1 fixture objects pass validation
   ✔  Given the pinned OCFL 1.1 community fixtures
   ✔  When I validate every object in good-objects
   ✔  Then every object should validate with zero errors
  Scenario: Warning OCFL 1.1 fixture objects report their warning codes
   ✔  Given the pinned OCFL 1.1 community fixtures
   ✔  When I validate every object in warn-objects
   ✔  Then every object should validate with zero errors
   ✔  And every object should report each warning code in its directory prefix
  Scenario: Invalid OCFL 1.1 fixture objects are rejected
   ✔  Given the pinned OCFL 1.1 community fixtures
   ✔  When I validate every object in bad-objects
   ✔  Then every object should be rejected as invalid
   ✔  And each error code in a directory prefix that was not reported should be listed in the Walkthrough
  Scenario: Valid OCFL 1.1 fixture objects are readable at every version
   ✔  Given the pinned OCFL 1.1 community fixtures
   ✔  When I resolve every version of every object in good-objects
   ✔  Then every streamed logical file should match its inventory digest
  Scenario: Objects written from OCFL 1.1 content fixtures pass independent validation
   ✔  Given the pinned OCFL 1.1 community fixtures
   ✔  When I write one object version per version directory of every content fixture
   ✔  Then every written object should validate with zero errors in filesquid-ocfl
   ✔  And every written object should validate with zero errors in an independent community validator
   ✔  And the logical state of every written version should equal its source content
  Scenario: Community fixtures are pinned test data only
   ✔  When I inspect the OCFL fixture pin
   ✔  Then the fixtures should be pinned to one OCFL/fixtures commit
   ✔  And the fixtures should not be packaged in or loaded by the filesquid-ocfl library or CLI
Feature: Inspection and copy never edit a completed object
  Scenario: A logical file copies out to another backend
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When I copy "b.txt" of "v1" to "export/b.txt" on another backend
   ✔  Then the copy should hold the bytes of "b.txt"
  Scenario Outline: A copy into the object root is refused
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When I copy "b.txt" of "v1" to "obj/inventory.json" in the same storage
   ✔  Then the copy should be refused as a write into an OCFL root
   ✔  And "obj/inventory.json" should be unchanged
   ✔  And the object should validate with zero errors
  Scenario Outline: A copy into the object root is refused
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When I copy "b.txt" of "v1" to "obj/v1/content/a.txt" in the same storage
   ✔  Then the copy should be refused as a write into an OCFL root
   ✔  And "obj/v1/content/a.txt" should be unchanged
   ✔  And the object should validate with zero errors
  Scenario Outline: A copy into the object root is refused
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When I copy "b.txt" of "v1" to "obj/v2/content/b.txt" in the same storage
   ✔  Then the copy should be refused as a write into an OCFL root
   ✔  And "obj/v2/content/b.txt" should be unchanged
   ✔  And the object should validate with zero errors
  Scenario: A copy into a storage root is refused
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given the storage holds an OCFL storage root declaration
   ✔  When I copy "b.txt" of "v1" to "export/b.txt" in the same storage
   ✔  Then the copy should be refused as a write into an OCFL root
   ✔  And "export/b.txt" should be unchanged
```

Without any community validator on `PATH` (default `PATH` of this machine), the same test target still runs the engine checks and prints why the community check is skipped:

```text
SKIPPED community OCFL validation (OCFL-S-010 (b)): no validator found; install ocfl-py (`pip install ocfl-py`, provides ocfl-validate.py) or rocfl (`cargo install rocfl`), or name one in FILESQUID_OCFL_COMMUNITY_VALIDATOR
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
```

`cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build -p filesquid-ocfl --locked --no-default-features --target wasm32-unknown-unknown` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` all exit 0. `bash scripts/pre-commit.sh` passes before the push.

## Review round 1 (Cubic)

Cubic's first review of `dde327c` raised nine findings. Each was fixed; the tests below were written first and failed before the fix where they test engine behaviour.

- **Copy guard, declarations of any version (P1).** The guard probed only the 1.0 and 1.1 declaration names, so a copy could land below `0=ocfl_object_2.0` or `0=ocfl_2.0`. It now refuses below any directory that directly holds a file named `0=ocfl_*` (OCFL-S-019 already says `<version>`), found with one bounded listing per level. To keep that cheap, the filesystem backend's `list` now enters only subdirectories whose keys can start with the prefix (`Visitor::prunes` in `src/storage/fs/walk.rs`). New tests `a_copy_under_an_unknown_version_declaration_is_refused`, `a_copy_beside_a_declaration_lookalike_succeeds` (memory and filesystem) and `fs_prefix_listing_skips_unrelated_subtrees`. Before the fix:

  ```text
  test a_copy_under_an_unknown_version_declaration_is_refused ... FAILED
  test fs_prefix_listing_skips_unrelated_subtrees ... FAILED
  ```

- **Fixture loader (P2).** A directory in a content fixture that is not a version directory now fails the load instead of being skipped.
- **Version metadata (P2).** `spec_ex_full_keeps_the_callers_version_metadata` asserts `created`, `message` and `user` of all three versions.
- **Copy destination (P2).** The BDD copy step asserts that the other backend holds exactly the requested destination key.
- **Source unchanged (P2).** New BDD step "the source object should be byte-identical to before the copy" compares every key and byte of the object after the happy path and after each refused copy.
- **Validator discovery (P2).** `PATH` search skips files without an execute bit and keeps searching; on Windows it also tries `PATHEXT` suffixes. New test `a_non_executable_validator_on_path_is_passed_over`. The READMEs say a bare `ocfl-validate.py` is not run on Windows.
- **Rustdoc (P3).** `Object::copy_file` names the `StorageError::LimitExceeded` refusals.
- **Whitespace (P3).** Trailing spaces removed from this file's captured output.

`FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR=1 cargo test -p filesquid-ocfl --locked` passes every target (100 scenarios: 97 passed, 3 skipped that belong to the server and UI). `integration_read_objects`:

```text
test a_copy_beside_a_declaration_lookalike_succeeds ... ok
test a_copy_into_a_storage_root_is_refused ... ok
test a_copy_into_its_own_object_root_is_refused ... ok
test a_copy_that_fails_its_digest_is_never_published ... ok
test a_head_published_during_open_is_read_again ... ok
test a_copy_into_another_object_root_is_refused ... ok
test a_head_version_inventory_removed_during_open_counts_as_missing ... ok
test a_copy_under_an_unknown_version_declaration_is_refused ... ok
test a_head_version_inventory_that_differs_from_the_root_is_e064 ... ok
test a_missing_head_version_inventory_is_allowed ... ok
test a_storage_root_with_too_many_0_files_is_refused ... ok
test a_logical_file_copies_into_another_key ... ok
test an_object_at_the_backend_root_opens ... ok
test a_storage_root_discovers_its_outermost_objects ... ok
test an_oversized_declaration_is_refused_before_it_is_read ... ok
test an_oversized_sidecar_is_refused_before_it_is_read ... ok
test an_unknown_ocfl_version_is_refused_as_unsupported ... ok
test a_version_above_the_root_head_is_unpublished ... ok
test a_file_that_grows_during_a_read_is_never_buffered_past_its_bound ... ok
test declaration_faults_have_their_codes ... ok
test declaration_listings_with_a_bad_continuation_are_refused ... ok
test content_missing_from_storage_is_e092 ... ok
test content_that_does_not_match_its_digest_is_e092 ... ok
test files_under_a_0_prefixed_directory_are_not_declarations ... ok
test every_version_resolves_through_state_and_manifest ... ok
test every_logical_file_streams_in_bounded_chunks ... ok
test objects_with_ten_or_more_versions_open ... ok
test inventory_and_sidecar_faults_have_their_codes ... ok
test opening_a_storage_root_runs_the_guard ... ok
test history_and_diff_come_from_the_inventory ... ok
test files_outside_the_inventory_are_not_logical_state ... ok
test roots_declared_as_another_ocfl_version_are_refused ... ok
test storage_root_declaration_faults_have_their_codes ... ok
test only_versions_and_paths_in_the_inventory_can_be_pinned ... ok
test opening_an_object_runs_the_git_guard ... ok
test sha512_objects_with_uppercase_digests_read ... ok
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.23s
```

`FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR=1 cargo test -p filesquid-ocfl --locked --test integration_content_fixtures -- --nocapture` (verdict lines; ocfl-py 2.1.0, rocfl 1.7.0):

```text
running 8 tests
test a_non_executable_validator_on_path_is_passed_over ... ok
test spec_ex_full_keeps_the_callers_version_metadata ... ok
test the_pinned_content_fixtures_are_all_written ... ok
test unchanged_and_reinstated_bytes_are_not_written_again ... ok
test every_content_fixture_is_written_one_version_per_directory_in_memory ... ok
test written_objects_use_sha256_and_its_sidecar ... ok
test earlier_versions_are_never_changed_by_later_commits ... ok
ocfl-py cf1: VALID
ocfl-py cf2: VALID
ocfl-py cf3: VALID
ocfl-py cf4: VALID
ocfl-py spec-ex-diff-paths: VALID
ocfl-py spec-ex-full: VALID
ocfl-py spec-ex-minimal: VALID
rocfl cf1: VALID
rocfl cf2: VALID
rocfl cf3: VALID
rocfl cf4: VALID
rocfl spec-ex-diff-paths: VALID
rocfl spec-ex-full: VALID
rocfl spec-ex-minimal: VALID
test every_content_fixture_written_to_a_directory_passes_the_engine_and_community_validators ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.54s
```

`read_only_copy_OCFL-S-019.feature`:

```text
Feature: Inspection and copy never edit a completed object
  Scenario: A logical file copies out to another backend
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When I copy "b.txt" of "v1" to "export/b.txt" on another backend
   ✔  Then the copy should hold the bytes of "b.txt"
   ✔  And the source object should be byte-identical to before the copy
  Scenario Outline: A copy into the object root is refused
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When I copy "b.txt" of "v1" to "obj/inventory.json" in the same storage
   ✔  Then the copy should be refused as a write into an OCFL root
   ✔  And "obj/inventory.json" should be unchanged
   ✔  And the source object should be byte-identical to before the copy
   ✔  And the object should validate with zero errors
  Scenario Outline: A copy into the object root is refused
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When I copy "b.txt" of "v1" to "obj/v1/content/a.txt" in the same storage
   ✔  Then the copy should be refused as a write into an OCFL root
   ✔  And "obj/v1/content/a.txt" should be unchanged
   ✔  And the source object should be byte-identical to before the copy
   ✔  And the object should validate with zero errors
  Scenario Outline: A copy into the object root is refused
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When I copy "b.txt" of "v1" to "obj/v2/content/b.txt" in the same storage
   ✔  Then the copy should be refused as a write into an OCFL root
   ✔  And "obj/v2/content/b.txt" should be unchanged
   ✔  And the source object should be byte-identical to before the copy
   ✔  And the object should validate with zero errors
  Scenario: A copy into a storage root is refused
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  Given the storage holds an OCFL storage root declaration
   ✔  When I copy "b.txt" of "v1" to "export/b.txt" in the same storage
   ✔  Then the copy should be refused as a write into an OCFL root
   ✔  And "export/b.txt" should be unchanged
```

## Review round 2 (Cubic)

Cubic's review of `595a1b8` raised four findings, all fixed:

- **Partial-name directories (P2).** `KeyCollector::prunes` entered a directory whose key was only a leading part of the prefix (`0=oc` for `0=ocfl_`). `Visitor::prunes` now receives whether the entry is a directory; a directory is entered only when `<key>/` starts with the prefix or the prefix starts with `<key>/`, and a file is kept only when its key starts with the prefix. New test `fs_prefix_listing_skips_a_partial_name_sibling` puts an entry the walk refuses (a name that is not UTF-8) inside `0=oc/`; before the fix the `0=ocfl_` listing entered it:

  ```text
  test fs_prefix_listing_skips_a_partial_name_sibling ... FAILED
  list: InvalidKey { key: "not-utf8-�", reason: "a key must be valid UTF-8" }
  ```

- **Filesystem bound in the README (P2).** The crate README and the `copy_guard.rs` module doc now say what the listing reads: the directory's entries, plus any subdirectory whose name starts with `0=ocfl_`; every other subtree is pruned unread.
- **Depth bound in the README (P3).** The README names both `StorageError::LimitExceeded` refusals: a key deeper than 64 levels (`MAX_DESTINATION_DEPTH`) and a directory with more than 1,000 keys under the declaration prefix.
- **Walk docs (P3).** The `walk.rs` module doc and the `walk` rustdoc say that a visitor can prune entries and their subtrees through `Visitor::prunes`.

After the fix:

```text
running 2 tests
test fs_prefix_listing_skips_a_partial_name_sibling ... ok
test fs_prefix_listing_skips_unrelated_subtrees ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 46 filtered out; finished in 0.05s
```

## SRS traceability

| Requirement | Tests |
|---|---|
| OCFL-S-010 (b) one version per content version directory, every fixture, none skipped | `integration_content_fixtures`: `the_pinned_content_fixtures_are_all_written`, `every_content_fixture_is_written_one_version_per_directory_in_memory`; BDD "Objects written from OCFL 1.1 content fixtures pass independent validation" |
| OCFL-S-010 (b) zero errors in the engine validator | `every_content_fixture_is_written_one_version_per_directory_in_memory`, `every_content_fixture_written_to_a_directory_passes_the_engine_and_community_validators`; BDD step "... zero errors in filesquid-ocfl" |
| OCFL-S-010 (b) zero errors in an independent community validator (ocfl-py, rocfl) | `every_content_fixture_written_to_a_directory_passes_the_engine_and_community_validators`; BDD step "... in an independent community validator" |
| OCFL-S-010 (b) logical state at each version equals the source directory | `assert_state_matches` in both write tests; BDD step "the logical state of every written version should equal its source content" |
| OCFL-S-002 written objects use sha256 and its sidecar | `written_objects_use_sha256_and_its_sidecar` |
| OCFL-S-018 only new bitstreams under `vN/`; earlier versions never changed | `unchanged_and_reinstated_bytes_are_not_written_again`, `earlier_versions_are_never_changed_by_later_commits` (memory and filesystem) |
| OCFL-S-019 a copy into its own object root is refused and changes nothing | `integration_read_objects`: `a_copy_into_its_own_object_root_is_refused` (inventory, sidecar, content, a version inventory, a new `v4/` path, the declaration); BDD "A copy into the object root is refused" (3 examples) |
| OCFL-S-019 a copy into another object root or a storage root is refused | `a_copy_into_another_object_root_is_refused`, `a_copy_into_a_storage_root_is_refused`; BDD "A copy into a storage root is refused" |
| OCFL-S-019 a root declared for any OCFL version is refused | `a_copy_under_an_unknown_version_declaration_is_refused` (`0=ocfl_object_2.0`, `0=ocfl_2.0`) |
| OCFL-S-019 a copy outside OCFL roots still works | `a_logical_file_copies_into_another_key`, `a_copy_into_a_storage_root_is_refused` (to another backend), `a_copy_beside_a_declaration_lookalike_succeeds`; BDD "A logical file copies out to another backend" (the copy is stored at its destination only) |
| OCFL-S-019 a copy never changes the source object | BDD step "the source object should be byte-identical to before the copy" (every key of the object, after the happy path and after each refusal) |
| OCFL-S-007 a prefix listing reads only what the prefix can match | `integration_fs_conformance`: `fs_prefix_listing_skips_unrelated_subtrees`, `fs_prefix_listing_skips_a_partial_name_sibling` |
| OCFL-S-003 caller-supplied logical files, no FileSquid application-layer type | every write test builds drafts from `VersionDraft` and bytes only; BDD "Core API is generic OCFL 1.1" now scans all of `src/` |
| OCFL-S-011 fixtures pinned, unchanged, test data only | BDD "Community fixtures are pinned test data only"; no file under `tests/fixtures/` changed |
| OCFL-S-007 no direct I/O in the engine; wasm32 build | `unit_source_scan` (covers the new `src/read/copy_guard.rs`); BDD "Engine builds for the browser" |

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no percentage is reported. `ReadError::CopyIntoOcflRoot` is produced by tests for an object declaration in the destination's own object, in another object and for a storage-root declaration at the backend root. The depth bound (`MAX_DESTINATION_DEPTH`, 64 levels) and backend failures during the check are not produced by a test.

## Sync Execution Report

- **Module SRS:** new row **OCFL-S-019** (inspection and copy never edit an object). It restates product vision T-036 and ADR 016 ("Direct inspection and copy MUST NOT authorize in-place inventory or signed-original edits") and the IO-0.7 plan description, which the module SRS did not yet hold, and states how a copy enforces it. No requirement beyond those sources.
- **Module plan:** IO-0.7 checked; Related Requirements add OCFL-S-003, OCFL-S-011, OCFL-S-019, T-036 and ADR 016; Related Tests list the new tests and this Walkthrough.
- **Repository-root implementation-plan checklist:** IO-0.7 checked. Global Task 0.4 stays open (IO-0.8 to IO-0.10).
- **Crate README:** "Read and browse" describes the copy refusal; "Pinned OCFL community fixtures" describes `integration_content_fixtures` and a new "Community validators (test oracles)" subsection with the install and run commands (each executed here) and both variables.
- **Root README:** "Run tests" names the write conformance test, the two optional validators, `FILESQUID_OCFL_COMMUNITY_VALIDATOR` and `FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR`.
- **Component diagram:** "Generic object payload" is a BUILT row (IO-0.7); "Opaque bytes" stays PLANNED (IO-0.8).
- **BDD:** `OCFL_1.1_fixture_conformance.feature` (write scenario now executable, header comment updated), `read_only_copy_OCFL-S-019.feature` (new), F-051 generic-API step widened. `independent_local_library_and_ownership_F-001.feature` is owned by the UI package and not edited.
- **PRD, global SRS, product vision, ADRs:** unchanged. PRD F-051 and ADR 016/023 already name `ocfl-py` and `rocfl` as test oracles and require written content fixtures to pass one; T-036 already states the copy rule. No new runtime or crate dependency, so no ADR is needed.

## Mocked Environment Variables

None. No credential or `.env` value was mocked. The validator runs used `ocfl-py` 2.1.0 (installed with pip into a virtual environment outside the repository) and `rocfl` 1.7.0 (`cargo install rocfl --locked` into a directory outside the repository), both put on `PATH` for the run.

## Remaining limits and owner decisions

- **Community validation is optional in the hook (owner decision).** `scripts/verify_tests.sh` runs `cargo test`, which runs the community check only where `ocfl-py` or `rocfl` is installed; elsewhere it is skipped with a message. PRD F-051 requires the independent check "before each release". Making it mandatory in the pre-commit hook (setting `FILESQUID_OCFL_REQUIRE_COMMUNITY_VALIDATOR=1` there) would make Python with `ocfl-py`, or `rocfl`, a development prerequisite for every contributor; that is left to the owner.
- **The copy guard checks, it does not lock.** It recognises declarations of any OCFL version at the moment of the copy. A destination that becomes an OCFL root after the check (another process declaring it during the copy) is not caught, and a directory that is an object root without any declaration (a crashed first commit before the declaration was written) is not recognised; neither is reachable through the engine's own writer, which writes the declaration first.
- **`rocfl` 1.7.0 is from 2022** (ADR 016); it validates OCFL 1.1 objects here, and `ocfl-py` 2.1.0 checks OCFL 1.1 explicitly. Either one satisfies OCFL-S-010 (b).
