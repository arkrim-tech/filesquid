# Fix macOS hook failures Walkthrough: SQLite stores under a linked folder, and four macOS-only test and lint failures

Branch: `fix/macos-hook-failures`, cut from `origin/main` at `db604e0`. The fix is in `filesquid-native`, which has no `resources/` folder of its own, so this walkthrough sits with the desktop host that ships it.

## Problem

`FULL=1 bash scripts/pre-commit.sh` failed step 10 on macOS on a clean `origin/main`:

1. **Clippy (`-D warnings`)**: `const MAX_MOUNTS` in `crates/native/src/import/volume.rs` was reported as never used. Only the Linux mount table reader (`find_mount`, `#[cfg(target_os = "linux")]`) uses it.
2. **Desktop and native tests**: `integration_durable_jobs` failed all 7 tests with `create: Unavailable` (and `the job store could not be opened (internal)`). The same cause failed `integration_history_wiring`, `integration_imports`, `integration_library_catalog` and `integration_library_routes` (desktop), and `unit_canonical_commit`, `unit_catalog`, `unit_findings`, `unit_host`, `unit_import_job`, `unit_jobs` and `unit_library_host` (native): about 100 tests in all. The hook stopped at the first failure.

Once those two were fixed, the same run reached three more macOS-only failures on `origin/main`:

3. **Clippy**: unused import `AttrValue` in `crates/native/tests/unit/import_facts.rs`. Only the two Linux-only extended-attribute tests use it.
4. **`filesquid-ocfl` `integration_fs_conformance::fs_prefix_listing_skips_a_partial_name_sibling`**: the fixture writes a file whose name is not UTF-8, which APFS refuses (`Illegal byte sequence`, os error 92), so the test panicked before it checked anything.
5. **Vitest, `@filesquid/ui` and ImageSquid `tests/unit/test-scripts.test.ts`, "fails and names the path when a removal fails"**: the test made the removal fail by naming a path under a file (`afile/child`). Node 24 on macOS treats that `ENOTDIR` as a path that does not exist, which `rmSync(..., { force: true })` ignores, so the cleaner exited 0.

## Root cause of the SQLite store failures

Every SQLite store in `filesquid-native` opens its file with `SQLITE_OPEN_NOFOLLOW`. SQLite refuses a symbolic link **anywhere** in the path it is given, not only at the file: its Unix VFS resolves the full path and the open fails with `SQLITE_CANTOPEN` once any component was a link. On macOS `std::env::temp_dir()` is under `/var/folders/…`, and `/var` is a link to `/private/var`, so every store a test opened in the temporary directory failed. The catalog mapped that to `CatalogError::Unavailable`, which `LibraryService` answered as `Unavailable`; the job store mapped it to `internal`.

Proof: with `TMPDIR` set to its physical path (`/private/var/folders/…`), `cargo test -p filesquid-desktop --test integration_durable_jobs` passed 7 of 7 unchanged.

This is a product defect as well as a test one: an owner whose data folder, or a folder above it, is reached through a link (a linked home folder, a data disk linked into place) could not open the app's stores. The intent the code and [Task DE-0.2](./Task-DE-0.2-Walkthrough.md) record is narrower: "A symbolic link where a catalog file belongs is refused, never followed".

## What changed

- `crates/native/src/import/volume.rs`: `MAX_MOUNTS` carries the same `#[cfg(target_os = "linux")]` as its only user. No `allow` attribute.
- `crates/native/src/owner_only.rs`: new `resolve_folders(path)`, which canonicalizes the folder above `path` and keeps its last component as given.
- Each `SQLITE_OPEN_NOFOLLOW` open now resolves the folders first and then creates and opens the resolved path: `SqliteJobs::open`, `SqliteFindings::open`, `SqlitePreferences::open`, `SqliteImportPlans::open`, `SqliteRepairLedger::open`, the catalog's `open_catalog_db` and its backup `vacuum_into` (`VACUUM INTO` inherits the connection's `NOFOLLOW`).
- What is still refused: a link at the database file itself (`create_owner_only` checks it and opens with `O_NOFOLLOW`; SQLite's `NOFOLLOW` stays on), and a folder swapped for a link after the resolution (SQLite still checks the whole resolved path). The existing tests `a_catalog_file_that_is_a_symbolic_link_is_not_followed` and `the_stores_refuse_a_symbolic_link_in_place_of_the_operational_store` still pass.
- `crates/native/tests/unit/import_facts.rs`: the `AttrValue` import carries `#[cfg(target_os = "linux")]`, like the tests that use it.
- `crates/ocfl/tests/integration/fs_conformance.rs`: the entry the walk must refuse is now named `back\slash`. Every Unix filesystem stores that name, and no storage key may hold a backslash, so the walk refuses it with `StorageError::InvalidKey` just as it refused the non-UTF-8 name. Both assertions are unchanged: listing `0=` enters the folder and fails with `InvalidKey`, and listing `0=ocfl_` skips it and answers `["0=ocfl_1.1"]`.
- `apps/filesquid/ui` and `apps/imagesquid` `tests/unit/test-scripts.test.ts` (identical files): the removal now fails on a file in a read-only folder (`0o555`), which fails on Linux and macOS alike. The test also checks that the file is still there, and it restores the folder's mode in `finally` so the scratch folder can be removed. It is skipped on Windows, which ignores a read-only folder, and when run as root, which ignores the mode.
- `scripts/docs/test_merge_mvp_progress.py` and `scripts/verify_changed_scope.sh`: the owner-approved harness fix from commit `73c4a6e` on `docs/finder-library-view`, applied unchanged. Inside a git hook the two harnesses inherited `GIT_DIR` and `GIT_INDEX_FILE` and wrote to the real repository when the hook ran in a linked worktree.

## Tests

Written first and seen failing on macOS before the fix (they also fail on Linux without it, because they make their own linked folder):

- `unit_library_host::the_stores_open_through_a_linked_folder_above_the_operational_store`: the repair ledger, preferences, jobs, findings and import plans all open `link/filesquid.sqlite` where `link` is a link to `real`, and the file is created in `real`. Before the fix: `repair ledger: Failed("unable to open database file: …/link/filesquid.sqlite")`.
- `unit_catalog::a_catalog_reached_through_a_linked_folder_opens_and_commits`: a catalog opened through a linked folder creates a library, and its catalog file is in the real folder. Before the fix: panicked in the fixture's `create_library`.

After the fix:

```text
$ cargo test -p filesquid-native --features desktop --no-fail-fast --test unit_library_host --test unit_catalog
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p filesquid-native -p filesquid-desktop --all-features --no-fail-fast
passed 335 failed 0   (sum of every "test result" line)
```

## Full hook

`FULL=1 bash scripts/pre-commit.sh` on macOS (Darwin 25.6, Node 24.22), on the working tree that is this commit, after `npm ci` in both UI packages (a fresh worktree has no `node_modules`, which step 7 reports). Gate lines from the run:

```text
✅ Branch Validation passed.
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
✅ filesquid-server uniquely bound to 8083
✅ Traceability Validation passed: 96 PRD requirements, 84 resolved SRS references.
✅ Workspace Boundary validation passed.
✅ Logging rule validation passed (no relevant staged files).
⚠️  WARNING: flock not found; cargo runs without the shared-target lock. Do not run two cargo commands on one CARGO_TARGET_DIR at once (install util-linux flock to enable the lock).
✅ ./apps/filesquid is properly formatted.
✅ ./apps/filesquid/ui is properly formatted.
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
✅ ADR structure (ISO 42010) passed.
✅ PRD allocation passed.
✅ Plan traceability passed.
✅ Relative links and anchors passed.
✅ FileSquid and ImageSquid document boundary passed.
✅ MVP queue and merge driver tests passed.
✅ MVP progress bar passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
⚠️  WARNING: flock not found; cargo runs without the shared-target lock. Do not run two cargo commands on one CARGO_TARGET_DIR at once (install util-linux flock to enable the lock).
⚠️  WARNING: flock not found; cargo runs without the shared-target lock. Do not run two cargo commands on one CARGO_TARGET_DIR at once (install util-linux flock to enable the lock).
⚠️  WARNING: flock not found; cargo runs without the shared-target lock. Do not run two cargo commands on one CARGO_TARGET_DIR at once (install util-linux flock to enable the lock).
ℹ️  flock not found; lock timeout test skipped.
✅ Change scope validation passed (161 assertions).
⚠️  WARNING: flock not found; cargo runs without the shared-target lock. Do not run two cargo commands on one CARGO_TARGET_DIR at once (install util-linux flock to enable the lock).
ℹ️  Test scope: full
✅ Clippy passed (12s).
✅ Workspace tests passed (250s).
✅ Module crate alone: filesquid-core passed (0s).
✅ Module crate alone: filesquid-ocfl passed (40s).
✅ Module crate alone: filesquid-metadata passed (0s).
✅ Module crate alone: filesquid-c2pa passed (4s).
✅ Module crate alone: filesquid-prov passed (3s).
✅ Module crate alone: filesquid-api passed (90s).
✅ Module crate alone: filesquid-native passed (3s).
✅ Clippy (filesquid-metadata, all features) passed (0s).
✅ Tests (filesquid-metadata, all features) passed (8s).
✅ wasm32 build (filesquid-metadata, default features off) passed (0s).
✅ wasm32 build (filesquid-metadata, ocfl feature) passed (0s).
✅ Clippy (filesquid-prov, all features) passed (1s).
✅ Tests (filesquid-prov, all features) passed (1s).
✅ wasm32 build (filesquid-prov) passed (0s).
✅ wasm32 build (filesquid-prov, ocfl feature) passed (0s).
✅ Rustdoc passed (6s).
✅ Clippy (desktop, test-instrumented) passed (6s).
✅ Desktop tests (test-instrumented) passed (42s).
✅ ESLint passed (3s).
✅ TypeScript passed (3s).
✅ Vitest passed (23s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
✅ ESLint passed (2s).
✅ TypeScript passed (2s).
✅ Vitest passed (3s).
ℹ️  Playwright e2e skipped for apps/imagesquid (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: full).
✅ All Pre-Commit Hooks Passed! Test scope: full (518s)
```

## Known flake, not fixed here

The hook run inside the first `git commit` failed once in `filesquid-ocfl` `integration_fs_conformance::fs_abort_and_upload_part_never_resurrect_an_upload` (`an aborted upload came back: ["…-129"]`). It passed in both standalone full runs above, 40 of 40 times alone, and 392 of 392 times with up to 32 copies at once. A likely cause is in `crates/ocfl/src/storage/fs/uploads.rs`: a part write whose path lookup reached the upload folder just before `abort_multipart` renamed it to `<id>.closed` can still create its temporary file there. `remove_dir_all` then fails with "directory not empty", and `abort_multipart` renames the folder back (`reopen_upload_logged`). The fix belongs in the OCFL crate with a test that reproduces the race, so it is left as a follow-up.

## Mocked Environment Variables

None.
