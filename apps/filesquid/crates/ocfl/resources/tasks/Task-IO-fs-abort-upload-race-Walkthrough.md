# Task IO-fs-abort-upload-race: Walkthrough

## Problem

`integration_fs_conformance::fs_abort_and_upload_part_never_resurrect_an_upload` ([fs_conformance.rs](../../tests/integration/fs_conformance.rs)) failed once under heavy load during a `git commit` hook run on macOS (recorded as "Known flake, not fixed here" in `Task-fix-macos-hook-failures-Walkthrough.md` under `crates/desktop/resources/tasks/` on the `fix/macos-hook-failures` branch, not yet merged):

```text
an aborted upload came back: ["<id>"]
```

Cause, in [uploads.rs](../../src/storage/fs/uploads.rs): `abort_multipart_blocking` renames `uploads/<id>/` to `<id>.closed` and then calls `fs::remove_dir_all`. A part write (`upload_part_blocking` → `write_atomic` → `create_new_checked`) whose path lookup reached `uploads/<id>/` just before the rename still creates its temporary file inside the now closed directory. When that file appears after `remove_dir_all` listed the directory, the final `rmdir` fails with "directory not empty". The old code then called `reopen_upload_logged`, which renamed `<id>.closed` back to `<id>`, so the aborted upload came back (and the racing part could then publish into it).

Traceability: [OCFL-S-007 storage backend](../srs.md), [storage_backend_OCFL-S-007.feature](../../tests/features/storage_backend_OCFL-S-007.feature).

## What changed

- `abort_multipart_blocking` closes the upload and hands the closed directory to a new `discard_closed`, which takes the removal function as a parameter so a unit test can play the race deterministically (production passes `fs::remove_dir_all`).
- `discard_closed` never renames an aborted upload back. It retries a removal that fails with `ErrorKind::DirectoryNotEmpty`, at most `DISCARD_ATTEMPTS` (5) attempts with a 5 ms × attempt pause (AGENTS.md §22). Any other error, or the cap, ends the loop: the `<id>.closed` directory is left behind (never a valid upload ID, so the upload stays aborted), a warning is logged and the error is returned. Only part writes whose lookup was already in flight at the rename can add files, so the retries converge.
- `complete_multipart` still renames the directory back when assembling fails, so the caller can retry or abort; that path is unchanged.
- The module doc comment of `uploads.rs`, the `reopen_upload_logged` doc and the crate [README](../../README.md) (FsBackend section) now describe the abort behaviour.
- The integration test is unchanged.
- Hook harness: `scripts/docs/test_merge_mvp_progress.py` and `scripts/verify_changed_scope.sh` are taken unchanged from commit `b0c933e` (`fix/macos-hook-failures`, PR #122). In a linked worktree `git commit` exports `GIT_DIR` and `GIT_INDEX_FILE` to the hook, and without this fix their throwaway test repositories wrote to the real repository (a hook run on this branch set `core.bare = true` and a `Test` user in the shared `.git/config`; the owner repaired it).

## TDD

Two unit tests in `uploads.rs` (`#[cfg(test)] mod tests`):

- `a_part_racing_an_abort_never_brings_the_upload_back`: closes an upload with one part, then discards it with a remover that, on its first call, deletes the contents, creates `.ocfl-tmp-late-part` (the racing part write) and calls the real `fs::remove_dir`, which fails with "directory not empty"; later calls remove everything. Asserts the uploads directory is empty and a later part gets `UnknownUpload`.
- `an_abort_that_cannot_remove_its_directory_stops_and_stays_closed`: a remover that always fails with `DirectoryNotEmpty` is called exactly `DISCARD_ATTEMPTS` times, the error is returned, only `<id>.closed` remains and a later part gets `UnknownUpload`.

Red run (seam added, old reopen behaviour kept), `cargo test -p filesquid-ocfl --lib storage::fs::uploads`, reproducing the flake's message deterministically:

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.08s
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_ocfl-…)

running 1 test
test storage::fs::uploads::tests::a_part_racing_an_abort_never_brings_the_upload_back ... FAILED

failures:

---- storage::fs::uploads::tests::a_part_racing_an_abort_never_brings_the_upload_back stdout ----

thread 'storage::fs::uploads::tests::a_part_racing_an_abort_never_brings_the_upload_back' (7259787) panicked at crates/ocfl/src/storage/fs/uploads.rs:294:9:
an aborted upload came back: ["58855-1792645258922625000-0"]
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    storage::fs::uploads::tests::a_part_racing_an_abort_never_brings_the_upload_back

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 43 filtered out; finished in 0.03s

error: test failed, to rerun pass `-p filesquid-ocfl --lib`
```

Green run after the fix:

```text
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.55s
     Running unittests src/lib.rs (target/debug/deps/filesquid_ocfl-27d6ee9e39ad4d6d)

running 2 tests
test storage::fs::uploads::tests::an_abort_that_cannot_remove_its_directory_stops_and_stays_closed ... ok
test storage::fs::uploads::tests::a_part_racing_an_abort_never_brings_the_upload_back ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 43 filtered out; finished in 0.14s
```

Stress run of the original integration test after the fix: 400 runs of `fs_abort_and_upload_part_never_resurrect_an_upload --exact`, 16 copies at once (25 rounds), 0 failures.

## Full hook

On this branch alone the full hook fails on macOS for reasons already on `main` and fixed by `b0c933e` (the `MAX_MOUNTS` dead-code lint, native SQLite stores under the `/var` link, a non-UTF-8 OCFL fixture name). The full hook was therefore run, directly and not inside `git commit`, in a scratch worktree holding `origin/main` + `b0c933e` + this change:

```text
FULL=1 bash scripts/pre-commit.sh
✅ All Pre-Commit Hooks Passed! Test scope: full (717s)
```

This branch's commit ran the hook with `FILESQUID_SKIP_TESTS=1` (steps 0 to 9); it must not be merged before PR #122, or until the full hook passes on its own head.
