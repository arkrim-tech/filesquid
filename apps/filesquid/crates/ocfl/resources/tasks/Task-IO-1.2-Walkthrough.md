# Task IO-1.2 Walkthrough: UI view API

Module plan row: [IO-1.2](../plan.md). Global task: [Task FS-1.2](../../../../../../resources/implementation-plan.md#task-fs-1.2) (stays open: API-F1.3 routes this API).

## OCFL-S-012: Serializable UI view API returning storage facts only (folder listing with paging, file details, version history, size, fixity, ranged reads, version-producing writes)

## OCFL-S-016: Concurrent commits — the view API's version-producing writes use the change model; no second write path

## OCFL-S-018: Write order and unpublished versions — a head without its version inventory (W010) stays refused

## OCFL-S-020: Version-inventory repair — never run implicitly by a view write

Sources: [OCFL-S-012, OCFL-S-015, OCFL-S-016, OCFL-S-018, OCFL-S-020](../srs.md), [PRD F-055](../../../../../../resources/prd.md#f-055-default-ui-and-panel-composition), [C-055](../../../../../../resources/product-vision.md#module-rules), [ADR 028](../../../../../../resources/architecture/028-filesquid-ui-packages-and-panel-composition.md), [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md), [ADR 025](../../../../../../resources/architecture/025-client-side-filesquid-engines.md).

## What changed

- **New public module `view`** (`src/view/`):
  - `list_folder(&object, &version, prefix, cursor, limit) -> Result<FolderPage, ViewError>`: implied folders, then files, directly below a logical-path prefix (`""` for the root), in name order; at most `MAX_PAGE_ENTRIES` (1,000) per page; opaque cursor (`d:<name>` / `f:<name>`, treat as opaque). `Entry::{Folder(FolderEntry { name, path }), File(FileEntry { name, path, digest, size, modified_version, modified, type_hint })}`. Sizes come from the backend (`head`) for the page's files only. `modified_version` is the version that last set the file's content (the earliest version from which the path has held its digest without a break) and `modified` that version's `created`.
  - `file_details(&object, &version, path) -> FileDetails { object_id, version, path, name, digest, digest_algorithm, size, content_path, fixity, last_change: VersionInfo, type_hint }`.
  - `file_history(&object, path) -> Vec<FileHistoryEntry { version: VersionInfo, change: FileChange::{Added, Changed, Deleted}, digest }>` (only versions that changed the path); `object_versions(&object) -> Vec<VersionInfo>`.
  - `read_range(&object, &version, path, start, len) -> Vec<u8>` with `len` 1 to `MAX_RANGE_BYTES` (8 MiB).
  - `check_fixity(&object, &version, path) -> FixityReport { path, version, checks: Vec<FixityCheck { algorithm, expected, actual, ok }>, ok }`: one streamed read, the manifest digest first, then each fixity value for an algorithm the engine computes; a mismatch is reported, not an error.
  - Writes: `add_file(&model, path, bytes, meta)`, `move_path(&model, from, to, meta)`, `remove_path(&model, path, meta)` (a file, or a folder with every file below it) and `restore(&model, &version, paths, meta)` (through `ChangeModel::reinstate`). Each returns the `CommitReceipt` of exactly one version.
  - JSON forms: `FolderPage::to_json`, `FileDetails::to_json`, `FixityReport::to_json`, `version_json`, `versions_json`, `history_json` (camelCase keys). The crate keeps serde without `derive`, as the inventory writer does.
  - `ViewError::{InvalidRequest, InvalidCursor, FolderNotFound, PathNotFound, StagingNotEmpty, Read, Change, Storage}`.
- **One write path.** Writes stage in the given `ChangeModel`'s staging area and call `ChangeModel::commit` (or `reinstate`). A write is refused while the staging area holds other changes (`StagingNotEmpty`), clears an index with no changes first, and resets the staging area after any failure, so a refused write leaves it empty. `StagingNotCleared` after a publication is treated as success (the area is reset and the receipt returned).
- **Empty folders** are never written: a prefix with no file below it is `FolderNotFound`, and adding a file beneath a new prefix creates the folder in the listing.
- **IO-0.10 public API unchanged.** Only crate-internal visibility changed: `ChangeModel::committed` is `pub(crate)`, `change::state_digests` is re-exported `pub(crate)`, and `Object::backend()` is a new `pub(crate)` accessor. No new dependency; `wasm32-unknown-unknown` with default features off builds.
- **Engine decisions** (recorded in OCFL-S-012): page at most 1,000 entries, range at most 8 MiB, a view write never commits changes staged by anyone else, a created folder is not persisted until a file is added beneath it.

## TDD

Tests were written first (`tests/integration/view_api.rs`, `tests/features/view_api_OCFL-S-012.feature`, `tests/support/view_steps.rs`). Red run before the `view` module existed:

```text
error[E0432]: unresolved import `filesquid_ocfl::view`
  --> crates/ocfl/tests/integration/view_api.rs:13:21
   |
13 | use filesquid_ocfl::view::{
   |                     ^^^^ could not find `view` in `filesquid_ocfl`

For more information about this error, try `rustc --explain E0432`.
error: could not compile `filesquid-ocfl` (test "integration_view_api") due to 1 previous error
```

TDD-ENFORCE (UI tests): the Agent Prompt asks for UI tests against built production artifacts, but this crate has no UI. The browser of IO-1.3 is superseded by `@filesquid/ui` (UI-F2.2, UI-F2.3), whose Playwright tests run against its production build; this task tests the engine API those UIs consume through `filesquid-api`.

## Constraint gates

| Constraint | Test |
|---|---|
| Listing by prefix at a pinned version, implied folders, folders first | `a_folder_lists_implied_folders_and_files_at_a_pinned_version` (memory and filesystem); BDD "The root lists implied folders before files" |
| Cursor paging, page bound 1,000 | `listing_pages_with_an_opaque_cursor`; BDD "A listing is paged with an opaque cursor" |
| Missing folder, file as folder | `listing_a_missing_folder_or_a_file_is_refused` |
| File details: storage facts only, size via backend, content location, fixity, last change | `file_details_carry_storage_facts_only` (exact JSON key set) |
| Per-file version history; object versions | `a_file_history_lists_the_versions_that_changed_it` |
| Ranged reads, bound 8 MiB | `ranged_reads_return_the_requested_bytes_within_the_bound`; BDD "Ranged reads are bounded" |
| Fixity check | `a_fixity_check_reports_every_digest_it_verifies` (tampered content reported) |
| Add, move (file and folder), remove (folder), restore: one version each through the change model | `add_move_and_remove_commit_one_version_each_through_the_change_model` (memory and filesystem), `a_file_history_lists_the_versions_that_changed_it` (remove, restore); BDD "Moving a folder commits one version and writes no bytes" |
| Empty folder stays UI-pending | `an_empty_folder_is_persisted_only_by_a_file_beneath_it`; BDD "Adding a file beneath a new folder persists the folder" |
| W010 head refused, not repaired; staging left empty | `a_write_refused_by_the_change_model_leaves_staging_clean`; BDD "A view write on a head without its version inventory…" |
| Only its own change | `a_write_is_refused_while_other_changes_are_staged` |
| Serializable | `view_results_serialize_to_json` |
| Engine and community validators | `add_move_and_remove…` (`assert_valid`), `objects_written_through_the_view_api_pass_the_community_validator` |
| wasm32 | build below; BDD "Engine builds for the browser" |

## Test output

`cargo test -p filesquid-ocfl --locked --test integration_view_api -- --nocapture --test-threads=1`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.21s
     Running tests/integration/view_api.rs ($CARGO_TARGET_DIR/debug/deps/integration_view_api-2f58d1662313bbd6)

running 13 tests
test a_file_history_lists_the_versions_that_changed_it ... ok
test a_fixity_check_reports_every_digest_it_verifies ... ok
test a_folder_lists_implied_folders_and_files_at_a_pinned_version ... ok
test a_write_is_refused_while_other_changes_are_staged ... ok
test a_write_refused_by_the_change_model_leaves_staging_clean ... ok
test add_move_and_remove_commit_one_version_each_through_the_change_model ... ok
test an_empty_folder_is_persisted_only_by_a_file_beneath_it ... ok
test file_details_carry_storage_facts_only ... ok
test listing_a_missing_folder_or_a_file_is_refused ... ok
test listing_pages_with_an_opaque_cursor ... ok
test objects_written_through_the_view_api_pass_the_community_validator ... ocfl-py view-api object: VALID
ok
test ranged_reads_return_the_requested_bytes_within_the_bound ... ok
test view_results_serialize_to_json ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.77s

```

`cargo test -p filesquid-ocfl --locked --test features -- --input "view_api_*.feature"`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.17s
     Running tests/features.rs ($CARGO_TARGET_DIR/debug/deps/features-9009fa7eceb51bdd)
Feature: UI view API
  Scenario: The root lists implied folders before files
   ✔> Given an object whose "v1" holds "readme.md" and "docs/a.txt" and whose "v2" changes "docs/a.txt"
   ✔  When the root folder of "v2" is listed
   ✔  Then the listing should show "docs/" then "readme.md"
   ✔  And "docs/a.txt" should show its size, digest and the version that last changed it
  Scenario: Adding a file beneath a new folder persists the folder
   ✔> Given an object whose "v1" holds "readme.md" and "docs/a.txt" and whose "v2" changes "docs/a.txt"
   ✔  When a file "pending/first.txt" is added through the view API
   ✔  Then the object head should be "v3" through the view API
   ✔  And the root folder of "v3" should show "pending/"
  Scenario: Moving a folder commits one version and writes no bytes
   ✔> Given an object whose "v1" holds "readme.md" and "docs/a.txt" and whose "v2" changes "docs/a.txt"
   ✔  When the folder "docs" is moved to "archive/docs" through the view API
   ✔  Then the object head should be "v3" through the view API
   ✔  And the view write should have written nothing
   ✔  And "archive/docs/a.txt" should exist at "v3"
  Scenario: A listing is paged with an opaque cursor
   ✔> Given an object whose "v1" holds "readme.md" and "docs/a.txt" and whose "v2" changes "docs/a.txt"
   ✔  When the root folder of "v2" is listed one entry per page
   ✔  Then the first page should show "docs/" and give a cursor
   ✔  And the next page should show "readme.md" and give no cursor
  Scenario: A view write on a head without its version inventory is refused and not repaired
   ✔> Given an object whose "v1" holds "readme.md" and "docs/a.txt" and whose "v2" changes "docs/a.txt"
   ✔  Given the head version inventory was removed
   ✔  When a file "x.txt" is added through the view API
   ✔  Then the view write should be refused for the missing version inventory
   ✔  And the staging area should be empty
  Scenario: Ranged reads are bounded
   ✔> Given an object whose "v1" holds "readme.md" and "docs/a.txt" and whose "v2" changes "docs/a.txt"
   ✔  When a range larger than the read bound is requested
   ✔  Then the read should be refused as an invalid request
[Summary]
1 feature
6 scenarios (6 passed)
25 steps (25 passed)
```

`cargo test -p filesquid-ocfl --locked` (summary lines of the whole crate):

```text
     Running unittests src/lib.rs ($CARGO_TARGET_DIR/debug/deps/filesquid_ocfl-336d9c2a83f05f02)
test result: ok. 43 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.15s
     Running tests/features.rs ($CARGO_TARGET_DIR/debug/deps/features-9009fa7eceb51bdd)
   ✔  Then Arrow, Parquet, DuckDB, and Iceberg should be absent from default features
[Summary]
15 features
151 scenarios (148 passed, 3 skipped)
701 steps (698 passed, 3 skipped)
     Running tests/integration/change_model.rs ($CARGO_TARGET_DIR/debug/deps/integration_change_model-97a7db5939f6743a)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.49s
     Running tests/integration/content_fixtures.rs ($CARGO_TARGET_DIR/debug/deps/integration_content_fixtures-2450cd9c4beef973)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.85s
     Running tests/integration/enumerate_state.rs ($CARGO_TARGET_DIR/debug/deps/integration_enumerate_state-c992207f0a36f4c4)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.51s
     Running tests/integration/fixture_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_fixture_conformance-6188b5d43aaacb5e)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     Running tests/integration/fixture_inventories.rs ($CARGO_TARGET_DIR/debug/deps/integration_fixture_inventories-49912e26c8122bb4)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/fixture_reads.rs ($CARGO_TARGET_DIR/debug/deps/integration_fixture_reads-f992725a1bd4ec37)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.21s
     Running tests/integration/fs_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_fs_conformance-4c054563d204acfb)
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.93s
     Running tests/integration/hierarchy_guard.rs ($CARGO_TARGET_DIR/debug/deps/integration_hierarchy_guard-3efdbaf82d970047)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/integration/independent_recovery.rs ($CARGO_TARGET_DIR/debug/deps/integration_independent_recovery-cff125f4c379a44f)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
     Running tests/integration/logical_file.rs ($CARGO_TARGET_DIR/debug/deps/integration_logical_file-c870615487e03f53)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.69s
     Running tests/integration/memory_conformance.rs ($CARGO_TARGET_DIR/debug/deps/integration_memory_conformance-bbed81d294b9c269)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.80s
     Running tests/integration/opaque_bytes.rs ($CARGO_TARGET_DIR/debug/deps/integration_opaque_bytes-1ba5398c372ee9d8)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.39s
     Running tests/integration/read_objects.rs ($CARGO_TARGET_DIR/debug/deps/integration_read_objects-777c46e5ca575e3e)
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
     Running tests/integration/validate_objects.rs ($CARGO_TARGET_DIR/debug/deps/integration_validate_objects-f628c102e60642ab)
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
     Running tests/integration/version_inventory_repair.rs ($CARGO_TARGET_DIR/debug/deps/integration_version_inventory_repair-ec3ea182e8864a75)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
     Running tests/integration/view_api.rs ($CARGO_TARGET_DIR/debug/deps/integration_view_api-2f58d1662313bbd6)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.58s
     Running tests/integration/write_order.rs ($CARGO_TARGET_DIR/debug/deps/integration_write_order-22dd8487357674b8)
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/unit/digest.rs ($CARGO_TARGET_DIR/debug/deps/unit_digest-84296ab96d185c1f)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/fixity.rs ($CARGO_TARGET_DIR/debug/deps/unit_fixity-595c41ef01df1b6c)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/git_guard.rs ($CARGO_TARGET_DIR/debug/deps/unit_git_guard-dfd4d18dbb6d3248)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     Running tests/unit/inventory.rs ($CARGO_TARGET_DIR/debug/deps/unit_inventory-73e97bdd05e073f7)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
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
    Finished `dev` profile [unoptimized] target(s) in 7.23s
```

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no percentage is reported. Every constraint above has a test. Not reached: the bounded-chunk limit of `check_fixity` (a file above the largest object a backend stores) and the `StagingNotCleared` branch of a view write (it needs the staging index to change during a commit, which `tests/integration/change_model.rs` covers for the change model itself).

## Sync Execution Report

- **Module SRS:** OCFL-S-012 gains the page and range bounds, the one-version-per-write rule through the change model, that a view write never commits changes staged by anyone else, and that a created folder is not persisted until a file is added beneath it.
- **Module plan:** IO-1.2 checked; Related Tests add the feature, the integration test, its steps and this Walkthrough.
- **Repository-root checklist:** IO-1.2 checked; MVP progress redrawn. Global Task FS-1.2 stays open (API-F1.3).
- **Crate README:** new "UI view API" section. **Root README:** "Run tests" names `integration_view_api`.
- **Crate rustdoc:** `src/lib.rs` and `src/view/mod.rs`.
- **PRD, root SRS, product vision, ADRs:** no change; F-055, C-055, ADR 028 and ADR 030 already require this API.

## Mocked Environment Variables

None. The community check used `ocfl-validate.py` (ocfl-py) on `PATH`.

## Notes for callers

- `filesquid-api` (API-F1.3, platform engineer) wires: `view::{list_folder, file_details, file_history, object_versions, read_range, check_fixity, add_file, move_path, remove_path, restore}`, their result types and JSON forms, and `ViewError`. API-S-005 applies at the API: `content_path` and `FolderPage` paths are engine facts that the API maps to opaque handles before they reach a UI.
- Give view writes a staging area of their own (one `ChangeModel` prefix per writer); the writes refuse an area with other staged changes.
- The cursor is only valid for the same object, version and prefix.
