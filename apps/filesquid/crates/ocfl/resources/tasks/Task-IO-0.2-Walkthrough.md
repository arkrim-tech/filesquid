# Task IO-0.2 Walkthrough: Git guard

Module plan row: [IO-0.2](../plan.md). Global task: Task 0.3 in the [implementation plan](../../../../../../resources/implementation-plan.md). Branch `feat/io-0.2-git-guard`, based on `main` at `fe31229` (IO-0.1 merged).

## OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root. The guard covers the storage root, the storage-hierarchy directories between the storage root and the object roots, and each object root. A `.git` entry inside an object's preserved content (for example an archived Git repository under `v1/content/`) is content and MUST be allowed (owner decision, 2026-10-18). Hard links, soft links and empty directories MUST NOT be used under the storage hierarchy.

Sources: [OCFL-S-001](../srs.md), [PRD F-042](../../../../../../resources/prd.md#f-042-ocfl-preservation-and-portable-recovery), [ADR 016](../../../../../../resources/architecture/016-rust-ocfl-library.md) (Layout constraints).

## What changed

- **Engine guard (`src/guard.rs`, every target including `wasm32`).** Reaches storage only through `StorageBackend`, so the OCFL-S-007 source scan still passes and the engine stays executor-free.
  - `check_storage_root(&backend)`: runs `backend.check_hierarchy("")`, then pages through every key (`MAX_LIST_PAGE` keys per page, at most `MAX_WALK_ENTRIES / MAX_LIST_PAGE` pages; the cap is an error, never a pass). It refuses a `.git` entry in the storage root, in a storage-hierarchy directory above the objects, or in an object root. An object root is a directory holding an `0=ocfl_object_*` declaration that is not inside another object. A `.git` inside an object's content (an archived Git repository) is preserved content and passes.
  - `reject_git_in_root(&backend, root)`: checks one storage or object root for a `.git` file or any key under `.git/`.
  - Pure `Layout` helpers hold the decision logic, with inline unit tests.
- **Backend hook.** New provided trait method `StorageBackend::check_hierarchy(prefix)`. The default (object stores, `MemoryBackend`) validates the prefix only: object stores have no links and no directories. Adding a provided method does not break other implementors.
- **Filesystem audit (`src/storage/fs/audit.rs`).** `FsBackend::check_hierarchy` takes the root write lock (so it never sees the transient second link of a create-only publish or parent directories made for a file not yet in place) and walks the directory. It refuses soft links (`SymlinkForbidden`), regular files with more than one link (`HardLinkForbidden`, Unix and macOS), empty directories below the root (`EmptyDirectoryForbidden`; the root itself may be empty) and other non-regular entries such as FIFOs (the existing not-a-regular-file `Io` error).
- **Shared walker (`src/storage/fs/walk.rs`).** The bounded walk behind listings became one walker with a `Visitor`; listing (`KeyCollector`) and the audit (`Audit`) share the same traversal, reserved-name skip and `MAX_WALK_DEPTH` / `MAX_WALK_ENTRIES` bounds. Listing behaviour is unchanged.
- **Reused IO-0.1 checks, not duplicated.** The audit reuses `confined_path` (refuses a symlinked prefix component), the reserved-name rule and `internal.rs` (`not_regular_entry`, which the old `not_regular` now calls, and a new `link_count` next to the existing file-identity helpers).
- **Path helper.** `reject_git_in_ocfl_root(path)` now uses `symlink_metadata`: it refuses a `.git` directory, a `.git` file (work-tree `gitdir:` pointer) and a `.git` link, dangling or not. Inspection errors are returned instead of being read as "absent".
- **Errors.** `OcflError::GitDirectoryForbidden { root }` (was a unit variant) and new `OcflError::Storage(StorageError)`. `StorageError` gains `HardLinkForbidden { key }` and `EmptyDirectoryForbidden { key }`. `WriteError` maps `OcflError` by hand so storage failures met by the guard keep the existing `WriteError` mapping. `filesquid-metadata` needed no change: it only propagates these errors.
- **Tests.** `tests/unit/git_guard.rs` now runs without the `fs` feature (its path-helper tests are gated on `fs`); new `tests/integration/hierarchy_guard.rs` on real directories; new [`git_guard_OCFL-S-001.feature`](../../tests/features/git_guard_OCFL-S-001.feature) with steps in `tests/features.rs`. `tests/integration/logical_file.rs` follows the new variant shape.
- **Docs.** Module [SRS](../srs.md) OCFL-S-001 now also carries ADR 016's link and empty-directory rule; crate [README](../../README.md) "Git guard" section; [component diagram](../component-diagram.html); module [plan](../plan.md) (IO-0.2 checked, test links).

## Review round 1 (Cubic)

Each finding got a failing test first where it can be tested on Linux (see "TDD: round 1").

- The audit skipped `.fsquid-*` names. The walker now asks its visitor which names to skip: the listing still skips the reserved prefix, and the audit sees every entry (`a_link_with_a_reserved_name_is_rejected`).
- An existing root that is not a directory passed as clean. The audit now refuses a start that exists but is not a directory, and a missing root still passes (`a_root_that_is_a_regular_file_is_rejected`).
- The scan bounded pages instead of keys. It now counts keys (at most `MAX_WALK_ENTRIES`), caps requests at the same number, and refuses a continuation that does not move forward (`short_pages_do_not_shrink_the_key_bound`, `a_continuation_that_makes_no_progress_is_refused`).
- `reject_git_in_root` accepted `/` and `.`. It now validates the prefix first (`reject_git_in_root_refuses_invalid_prefixes`).
- The FIFO test now skips when `mkfifo` cannot run.
- The soft-link and hard-link scenarios are tagged `@unix`. The harness leaves `@unix` scenarios out on other platforms instead of reporting undefined steps. This cannot fail on Linux, so it has no red run.
- The crate docs say that the guard runs only when a caller invokes it, and name the expected call sites (IO-0.4, IO-0.6 onward). The component diagram says hard links pass on Windows. The error message says "a .git entry". The traceability rows carry the full OCFL-S-001 title.

## Remaining limits

- Hard links are detected only where the standard library reports link counts (Unix, macOS included). On Windows `std` exposes no stable link count, so a hard link passes there. Closing this needs a reviewed Windows API dependency.
- `check_storage_root` pages through the whole root with `list`. `FsBackend::list` walks the tree again for each page, so a root with N keys costs about N squared / 1,000 directory entries. Object discovery (IO-0.4) needs the same listing and is the place to add a cheaper cursor.
- The guard checks a root when it is called. It is not yet wired into object open or `vN` publication; IO-0.4 (read) and IO-0.6 onward (write) call it.
- The audit runs before it returns, under the write lock; a process that ignores that lock (for example a manual copy) can still add a link afterwards. The root and staging MUST NOT be writable by untrusted processes, as IO-0.1 already states.

## SRS traceability

| Requirement | Tests |
|---|---|
| OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root (storage root) | `unit_git_guard`: `git_in_the_storage_root_is_rejected`, `a_git_file_in_the_storage_root_is_rejected`; BDD outline rows `.git/HEAD` (both backends) |
| OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root (object root) | `unit_git_guard`: `git_in_an_object_root_is_rejected`, `reject_git_in_root_checks_one_root`, `reject_git_in_root_refuses_invalid_prefixes`; `integration_hierarchy_guard`: `a_populated_git_directory_is_rejected`, `an_empty_git_directory_is_rejected`; BDD "An empty .git directory is refused" and outline rows `ab/obj-1/.git/config` |
| OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root (storage-hierarchy directories) | `unit_git_guard`: `git_in_a_storage_hierarchy_directory_is_rejected`; BDD outline row `ab/.git` |
| OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root (`.git` inside preserved object content is allowed, owner decision 2026-10-18) | `unit_git_guard`: `git_inside_object_content_is_preserved_content`, `a_declaration_inside_content_is_not_an_object_root`; BDD "A Git repository preserved as object content is allowed" |
| OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root; soft links MUST NOT be used under the storage hierarchy | `integration_hierarchy_guard`: `unix::a_symlinked_file_is_rejected`, `unix::a_symlinked_directory_is_rejected`, `unix::a_dangling_symlink_is_rejected`, `unix::a_symlinked_prefix_is_rejected`, `unix::a_link_with_a_reserved_name_is_rejected`; `unit_git_guard`: `path_helper::dangling_git_link_is_rejected`; BDD "A soft link in the storage hierarchy is refused" |
| OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root; hard links MUST NOT be used under the storage hierarchy | `integration_hierarchy_guard`: `unix::a_hard_link_is_rejected`; BDD "A hard link in the storage hierarchy is refused" |
| OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root; empty directories MUST NOT be used under the storage hierarchy | `integration_hierarchy_guard`: `an_empty_directory_is_rejected`, `deletes_leave_no_empty_directories`; BDD "An empty directory in the storage hierarchy is refused" |
| OCFL-S-001: Git MUST NOT be the OCFL library; `.git` MUST NOT appear in an OCFL object or storage root; the storage hierarchy holds plain files only (other non-regular entries, a root that is not a directory) | `integration_hierarchy_guard`: `unix::a_fifo_is_rejected` (skipped when `mkfifo` is unavailable), `a_root_that_is_a_regular_file_is_rejected` |
| Clean roots pass | `unit_git_guard`: `a_clean_storage_root_passes`, `an_empty_storage_root_passes`, `short_pages_do_not_shrink_the_key_bound`, `a_continuation_that_makes_no_progress_is_refused`, `object_stores_have_no_links_or_empty_directories`; `integration_hierarchy_guard`: `a_root_written_through_the_backend_passes`, `a_root_that_was_never_written_passes`, `a_hierarchy_check_below_a_prefix_skips_the_rest`; BDD "A plain storage root passes the guard" (both backends) |
| OCFL-S-007: engine core has no direct I/O; builds for `wasm32` | `unit_source_scan`; BDD "Storage goes through one async backend interface" and "Engine builds for the browser"; `cargo build -p filesquid-ocfl --no-default-features --target wasm32-unknown-unknown` |

## Coverage

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no percentage is reported. Every public function added here (`check_storage_root`, `reject_git_in_root`, `StorageBackend::check_hierarchy` on both shipped backends, `reject_git_in_ocfl_root`) and every new error variant has a direct test. The one-million-key bound of `check_storage_root` is not reached by a test; short pages and a stalled continuation are.

## TDD: failing runs before implementation

Tests written first, before `guard.rs`, `check_hierarchy` and the new error variants existed (`cargo test -p filesquid-ocfl --locked --test unit_git_guard --test integration_hierarchy_guard --test features`, error lines):

```text
error[E0432]: unresolved imports `filesquid_ocfl::check_storage_root`, `filesquid_ocfl::reject_git_in_root`
error[E0432]: unresolved import `filesquid_ocfl::check_storage_root`
error[E0559]: variant `OcflError::GitDirectoryForbidden` has no field named `root`
error[E0599]: no method named `check_hierarchy` found for reference `&FsBackend` in the current scope
error[E0599]: no variant, associated function, or constant named `Storage` found for enum `OcflError` in the current scope
error[E0599]: no method named `check_hierarchy` found for struct `MemoryBackend` in the current scope
error[E0599]: no variant named `EmptyDirectoryForbidden` found for enum `filesquid_ocfl::storage::StorageError`
error[E0559]: variant `OcflError::GitDirectoryForbidden` has no field named `root`
error[E0599]: no variant, associated function, or constant named `Storage` found for enum `filesquid_ocfl::OcflError` in the current scope
error[E0599]: no variant named `EmptyDirectoryForbidden` found for enum `filesquid_ocfl::storage::StorageError`
error[E0559]: variant `filesquid_ocfl::OcflError::GitDirectoryForbidden` has no field named `root`
error[E0599]: no method named `check_hierarchy` found for struct `FsBackend` in the current scope
error[E0432]: unresolved import `filesquid_ocfl::check_storage_root`
error[E0599]: no variant named `HardLinkForbidden` found for enum `filesquid_ocfl::storage::StorageError`
error[E0599]: no variant, associated function, or constant named `Storage` found for enum `filesquid_ocfl::OcflError` in the current scope
error: could not compile `filesquid-ocfl` (test "unit_git_guard") due to 5 previous errors
error[E0599]: no method named `check_hierarchy` found for struct `FsBackend` in the current scope
error: could not compile `filesquid-ocfl` (test "integration_hierarchy_guard") due to 10 previous errors
error[E0559]: variant `OcflError::GitDirectoryForbidden` has no field named `root`
error[E0599]: no variant named `EmptyDirectoryForbidden` found for enum `StorageError`
error[E0599]: no variant, associated function, or constant named `Storage` found for enum `OcflError` in the current scope
error[E0599]: no variant, associated function, or constant named `Storage` found for enum `OcflError` in the current scope
error[E0599]: no variant named `HardLinkForbidden` found for enum `StorageError`
error[E0599]: no variant, associated function, or constant named `Storage` found for enum `OcflError` in the current scope
error: could not compile `filesquid-ocfl` (test "features") due to 7 previous errors
```

## TDD: round 1

Regression tests for the Cubic findings, written before the fixes (`cargo test -p filesquid-ocfl --locked --test integration_hierarchy_guard`, then `--test unit_git_guard`):

```text
test a_root_that_is_a_regular_file_is_rejected ... FAILED
test unix::a_link_with_a_reserved_name_is_rejected ... FAILED
thread 'a_root_that_is_a_regular_file_is_rejected' (30944) panicked at crates/ocfl/tests/integration/hierarchy_guard.rs:140:5:
thread 'unix::a_link_with_a_reserved_name_is_rejected' (30954) panicked at crates/ocfl/tests/integration/hierarchy_guard.rs:201:9:
test result: FAILED. 13 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
error: test failed, to rerun pass `-p filesquid-ocfl --test integration_hierarchy_guard`
test reject_git_in_root_refuses_invalid_prefixes ... FAILED
test a_continuation_that_makes_no_progress_is_refused ... FAILED
test short_pages_do_not_shrink_the_key_bound ... FAILED
thread 'reject_git_in_root_refuses_invalid_prefixes' (30990) panicked at crates/ocfl/tests/unit/git_guard.rs:124:9:
thread 'a_continuation_that_makes_no_progress_is_refused' (30975) panicked at crates/ocfl/tests/unit/git_guard.rs:237:5:
thread 'short_pages_do_not_shrink_the_key_bound' (30991) panicked at crates/ocfl/tests/unit/git_guard.rs:230:5:
test result: FAILED. 15 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
error: test failed, to rerun pass `-p filesquid-ocfl --test unit_git_guard`
```

## Gates

Run from `apps/filesquid/` with `CARGO_TARGET_DIR` on a shared target:

| Command | Result |
|---|---|
| `cargo fmt --all` | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo test -p filesquid-ocfl --locked` | all pass (counts below) |
| `cargo test -p filesquid-ocfl --locked --no-default-features` | all pass (`unit_git_guard` 11 tests without the `fs` path-helper tests) |
| `cargo build -p filesquid-ocfl --locked --no-default-features --target wasm32-unknown-unknown` | builds |
| `cargo test -p filesquid-metadata --locked` | all pass |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | clean |
| `bash scripts/pre-commit.sh` (repository root) | exit 0 |

## Mocked Environment Variables

None. The tests use scratch directories under Cargo's per-target temporary directory and remove them on drop.

## Raw test output

### `cargo test -p filesquid-ocfl --locked --no-fail-fast` (summary lines)

```text
     Running unittests src/lib.rs
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.25s
     Running tests/features.rs
42 scenarios (33 passed, 9 skipped)
     Running tests/integration/fs_conformance.rs
test result: ok. 46 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s
     Running tests/integration/hierarchy_guard.rs
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/integration/logical_file.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.63s
     Running tests/integration/memory_conformance.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.90s
     Running tests/unit/git_guard.rs
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
     Running tests/unit/source_scan.rs
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/storage_key.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### `cargo test -p filesquid-ocfl --locked --test unit_git_guard --test integration_hierarchy_guard`

```text
running 15 tests
test a_root_that_was_never_written_passes ... ok
test a_root_that_is_a_regular_file_is_rejected ... ok
test a_populated_git_directory_is_rejected ... ok
test a_hierarchy_check_below_a_prefix_skips_the_rest ... ok
test a_root_written_through_the_backend_passes ... ok
test an_empty_directory_is_rejected ... ok
test deletes_leave_no_empty_directories ... ok
test an_empty_git_directory_is_rejected ... ok
test unix::a_dangling_symlink_is_rejected ... ok
test unix::a_hard_link_is_rejected ... ok
test unix::a_link_with_a_reserved_name_is_rejected ... ok
test unix::a_fifo_is_rejected ... ok
test unix::a_symlinked_directory_is_rejected ... ok
test unix::a_symlinked_file_is_rejected ... ok
test unix::a_symlinked_prefix_is_rejected ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s


running 18 tests
test a_continuation_that_makes_no_progress_is_refused ... ok
test a_git_file_in_the_storage_root_is_rejected ... ok
test a_clean_storage_root_passes ... ok
test a_declaration_inside_content_is_not_an_object_root ... ok
test an_empty_storage_root_passes ... ok
test git_in_a_storage_hierarchy_directory_is_rejected ... ok
test git_in_an_object_root_is_rejected ... ok
test git_in_the_storage_root_is_rejected ... ok
test object_stores_have_no_links_or_empty_directories ... ok
test git_inside_object_content_is_preserved_content ... ok
test path_helper::a_root_without_git_passes ... ok
test reject_git_in_root_checks_one_root ... ok
test reject_git_in_root_refuses_an_escaping_root ... ok
test path_helper::git_directory_is_rejected ... ok
test reject_git_in_root_refuses_invalid_prefixes ... ok
test path_helper::git_file_is_rejected ... ok
test path_helper::dangling_git_link_is_rejected ... ok
test short_pages_do_not_shrink_the_key_bound ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

### Cucumber: `git_guard_OCFL-S-001.feature`

```text
Feature: Git guard and plain storage hierarchy
  Scenario Outline: A plain storage root passes the guard
   ✔  Given an empty in-memory storage backend
   ✔  And an OCFL storage root with the object "ab/obj-1"
   ✔  When I run the storage root guard
   ✔  Then the guard should pass
  Scenario Outline: A plain storage root passes the guard
   ✔  Given an empty local filesystem storage backend
   ✔  And an OCFL storage root with the object "ab/obj-1"
   ✔  When I run the storage root guard
   ✔  Then the guard should pass
  Scenario Outline: Git metadata in a storage root or object root is refused
   ✔  Given an empty in-memory storage backend
   ✔  And an OCFL storage root with the object "ab/obj-1"
   ✔  And a stored file at ".git/HEAD"
   ✔  When I run the storage root guard
   ✔  Then the guard should refuse a .git directory in ""
  Scenario Outline: Git metadata in a storage root or object root is refused
   ✔  Given an empty in-memory storage backend
   ✔  And an OCFL storage root with the object "ab/obj-1"
   ✔  And a stored file at "ab/obj-1/.git/config"
   ✔  When I run the storage root guard
   ✔  Then the guard should refuse a .git directory in "ab/obj-1"
  Scenario Outline: Git metadata in a storage root or object root is refused
   ✔  Given an empty local filesystem storage backend
   ✔  And an OCFL storage root with the object "ab/obj-1"
   ✔  And a stored file at ".git/HEAD"
   ✔  When I run the storage root guard
   ✔  Then the guard should refuse a .git directory in ""
  Scenario Outline: Git metadata in a storage root or object root is refused
   ✔  Given an empty local filesystem storage backend
   ✔  And an OCFL storage root with the object "ab/obj-1"
   ✔  And a stored file at "ab/obj-1/.git/config"
   ✔  When I run the storage root guard
   ✔  Then the guard should refuse a .git directory in "ab/obj-1"
  Scenario Outline: Git metadata in a storage root or object root is refused
   ✔  Given an empty local filesystem storage backend
   ✔  And an OCFL storage root with the object "ab/obj-1"
   ✔  And a stored file at "ab/.git"
   ✔  When I run the storage root guard
   ✔  Then the guard should refuse a .git directory in "ab"
  Scenario Outline: A Git repository preserved as object content is allowed
   ✔  Given an empty in-memory storage backend
   ✔  And an OCFL storage root with the object "ab/obj-1"
   ✔  And a stored file at "ab/obj-1/v1/content/repo/.git/HEAD"
   ✔  When I run the storage root guard
   ✔  Then the guard should pass
  Scenario Outline: A Git repository preserved as object content is allowed
   ✔  Given an empty local filesystem storage backend
   ✔  And an OCFL storage root with the object "ab/obj-1"
   ✔  And a stored file at "ab/obj-1/v1/content/repo/.git/HEAD"
   ✔  When I run the storage root guard
   ✔  Then the guard should pass
  Scenario: An empty .git directory is refused
   ✔  Given an empty local filesystem storage backend
   ✔  And an OCFL storage root with the object "ab/obj-1"
   ✔  And an empty directory at "ab/obj-1/.git"
   ✔  When I run the storage root guard
   ✔  Then the guard should refuse an empty directory at "ab/obj-1/.git"
  Scenario: An empty directory in the storage hierarchy is refused
   ✔  Given an empty local filesystem storage backend
   ✔  And an OCFL storage root with the object "ab/obj-1"
   ✔  And an empty directory at "ab/obj-1/v1/content/empty"
   ✔  When I run the storage root guard
   ✔  Then the guard should refuse an empty directory at "ab/obj-1/v1/content/empty"
  Scenario: A soft link in the storage hierarchy is refused
   ✔  Given an empty local filesystem storage backend
   ✔  And an OCFL storage root with the object "ab/obj-1"
   ✔  And a soft link at "ab/obj-1/v1/content/link.txt" to a file outside the root
   ✔  When I run the storage root guard
   ✔  Then the guard should refuse a soft link at "ab/obj-1/v1/content/link.txt"
  Scenario: A hard link in the storage hierarchy is refused
   ✔  Given an empty local filesystem storage backend
   ✔  And an OCFL storage root with the object "ab/obj-1"
   ✔  And the file "ab/obj-1/inventory.json" is hard linked from outside the root
   ✔  When I run the storage root guard
   ✔  Then the guard should refuse a hard link at "ab/obj-1/inventory.json"
```
