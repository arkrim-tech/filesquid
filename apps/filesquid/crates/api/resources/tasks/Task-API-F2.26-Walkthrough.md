# Task API-F2.26 Walkthrough: Streamed copies of any size

Branch `feat/api-f2.26-streamed-copies`, module `filesquid-api` ([plan row](../plan.md)), global Task FS-2.5. Owner decision 2026-10-31, verbatim: "build streamed copies"; the allowance in API-S-011 to refuse a copy over 64 MiB is removed.

## SRS mapping

| SRS ID and title | Covered by |
|---|---|
| [API-S-011](../srs.md) View operation, sort keys, storage use, copy, move and ZIP download: a copy of a file version of any size MUST stream through bounded ranged reads and writes and MUST NOT hold the file in memory, on every host, the desktop included | `integration_streamed_copies` (8 tests, an empty file, a retry sent while the copy stages and a copy dropped after its stage among them), `integration_streamed_copy_large` (600 MiB), the scenario outline "A copy of any size streams with bounded memory on a disk-backed staging area" (1, 65 and 600 MiB) and the scenario "A source that changes during a streamed copy fails with nothing left staged" of [`copy_provenance_and_streamed_copies_API-S-011.feature`](../../tests/features/copy_provenance_and_streamed_copies_API-S-011.feature), and `filesquid-desktop`'s `integration_streamed_copy` through the `filesquid://` scheme |
| [NFR-003](../../../../../../resources/prd.md#nfr-003-bounded-processing-and-migration-endurance) Bounded processing and migration endurance | `a_copy_above_512_mib_keeps_its_memory_bounded` (peak resident memory grows by at most 256 MiB while 600 MiB are copied), `a_copy_above_64_mib_streams_in_bounded_reads_and_writes` (no read above 8 MiB, no write above 64 MiB) |
| [OCFL-S-026](../../../ocfl/resources/srs.md) Streamed stage of one logical file from a backend | used unchanged through `ChangeModel::stage_file`; the `ocfl` crate is not edited |
| [API-S-026](../srs.md) FileSquid OCFL application profile: the copy's record cites the content its version holds | `a_copy_record_keeps_what_the_owner_sees_and_is_bound_to_the_new_object`, `a_small_copy_is_streamed_too_and_cites_its_content` |
| [API-S-002](../srs.md) Versioned API contract owned by this crate | `unit_contract` (`API_VERSION` 0.40.0), the UI contract drift test in the hook |

## What changed

- **Copy (`src/files/copy.rs`).** `FileService::copy` no longer reads the source into memory. It resolves the storage key of the source's original at its head (`Object::content_key`), stages that content into the new object's staging area with `ChangeModel::stage_file` (ranged reads of at most 8 MiB, at most two passes, multipart parts of at most 64 MiB), and commits it with `commit_staged_original`, the same pair the import uses. `MAX_COPY_BYTES` and its `400 invalid_request` refusal are removed.
- **Write lock.** The stage runs outside the library's write lock. The lock is taken for the commit only; under it the copy rechecks that no earlier run of the same operation committed the copy (it then answers that copy and clears its own stage) and that the source's head is still the expected revision (else `409 conflict`).
- **Failures.** Any failure clears the new object's staging area on a best-effort basis: a failure to clear is logged. A copy dropped before it settles (a cancelled request, after its source was staged) is cleared by a guard that hands the clearing to a task on the Tokio runtime (CodeRabbit round 1). The spawn is native only; on `wasm32`, where storage backends need not be `Send` (API-S-010), the guard logs and leaves the stage (CodeRabbit round 2). The crate does not build for `wasm32` today, on main as on this branch, because tokio's `net` feature pulls in mio. A copy dropped while the OCFL stage is still streaming its source can leave that stage's unfinished multipart upload in the staging area; removing it needs the `ocfl` crate (an abort when `stage_file`'s upload is dropped, or a sweep of abandoned uploads), so it is reported, not changed here. A stored original that changes between the stage's reads, or between the size read before and after the stage, is `409 conflict` with `source_changed` in the message; one that cannot be read is `409 conflict` with `source_unreadable`. Storage errors are logged, never answered, since they name a storage key.
- **Record (`src/canonical/copy.rs`).** `copy_record` takes the content's SHA-256 and size (from the stage) and not the bytes.
- **Hosts.** No host change. The copy lives in `filesquid-api`, so every host that mounts the router serves it. The desktop opens each library with a staging backend on disk under its data directory (`filesquid_native::open_fs_library_root`), so the staged bytes go there; `filesquid-server` mounts the same router and opens no file library yet (its file routes answer `not_found` until its library tasks open one with `filesquid_native::open_fs_library`, which also stages on disk), so on the server the copy is exercised through the router tests of this crate. An in-memory staging backend holds what it stages, as OCFL-S-026 documents.
- **Contract 0.40.0.** The `copyFile` description states the streaming, the lock and the refusals; no schema changed. `ui/src/api/schema.d.ts` and `contract-version.ts` are regenerated.
- **Docs.** API-S-011 states the stage, the lock and the failure rules; the README's "Run tests" names the new tests and their disk need; the plan row and the root checklist are ticked.
- **Test fixtures.** `tests/support/files_fixture.rs`: the counting backend also records its largest read, largest write, bytes written and reads of at least 1 MiB, the service under test stages through it, and a read hook now runs outside the hook lock. `tests/integration/recently_deleted.rs` opens its second library on that staging backend (one line, a type change only). `tests/support/copy_fixture.rs` is new. The desktop `ImportRig` gains `command` and `install_home_collections`.

TDD: `a_copy_dropped_after_its_stage_leaves_nothing_staged` (CodeRabbit round 1) was written first and failed, leaving the staged blob and index, before the guard was added. `integration_streamed_copies` was written first and failed (4 of 5) against the read-whole copy. The desktop test and the step definitions were written after the service change.

Constraints checked (the gates of the deleted ToDos): no size refusal; reads at most 8 MiB and writes at most 64 MiB; memory bounded above 512 MiB; the desktop scheme; staged bytes on disk; lock for the commit only; a changed or unreadable source fails with nothing committed or staged; the copy cites the source's SHA-256 and size; the earlier copy tests pass; contract bumped and types regenerated; no change in the `ocfl` crate.

## Mocked Environment Variables

None.

## Test output

`cargo test -p filesquid-api --locked --test integration_streamed_copies` before the change (the copy still read its source whole and held the write lock from the start):

```text
test a_source_that_cannot_be_read_fails_with_nothing_staged ... FAILED
test a_source_that_changes_while_it_is_read_fails_with_nothing_staged ... FAILED
test a_copy_above_64_mib_streams_in_bounded_reads_and_writes ... FAILED
test another_write_completes_while_a_copy_stages_and_the_stale_copy_is_refused ... FAILED
thread 'a_source_that_cannot_be_read_fails_with_nothing_staged' (5778) panicked at crates/api/tests/integration/streamed_copies.rs:40:5:
thread 'a_source_that_changes_while_it_is_read_fails_with_nothing_staged' (5779) panicked at crates/api/tests/integration/streamed_copies.rs:40:5:
thread 'a_copy_above_64_mib_streams_in_bounded_reads_and_writes' (5776) panicked at crates/api/tests/integration/streamed_copies.rs:73:5:
thread 'tokio-rt-worker' (5788) panicked at crates/api/tests/integration/streamed_copies.rs:134:14:
the rename completes while the copy is staging: Timeout
thread 'another_write_completes_while_a_copy_stages_and_the_stale_copy_is_refused' (5785) panicked at crates/api/tests/integration/streamed_copies.rs:151:10:
the write lock is free while a copy is staging: Elapsed(())
test result: FAILED. 1 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 60.41s
```

After the change, `cargo test -p filesquid-api --locked --test integration_streamed_copies --test integration_streamed_copy_large --test unit_canonical --test unit_contract` (the two new binaries in full, then the two changed tests of the others):

```text
     Running tests/integration/streamed_copies.rs (<target>/debug/deps/integration_streamed_copies-53ba7e08fad0c7a5)

running 5 tests
test a_small_copy_is_streamed_too_and_cites_its_content ... ok
test an_empty_file_is_copied_too ... ok
test a_retry_while_the_copy_stages_is_refused_and_leaves_its_stage_alone ... ok
test a_copy_dropped_after_its_stage_leaves_nothing_staged ... ok
test a_source_that_changes_while_it_is_read_fails_with_nothing_staged ... ok
test a_source_that_cannot_be_read_fails_with_nothing_staged ... ok
test another_write_completes_while_a_copy_stages_and_the_stale_copy_is_refused ... ok
test a_copy_above_64_mib_streams_in_bounded_reads_and_writes ... ok

     Running tests/integration/streamed_copy_large.rs (<target>/debug/deps/integration_streamed_copy_large-6d3f742a6097c336)

running 1 test
test a_copy_above_512_mib_keeps_its_memory_bounded ... ok

test a_copy_record_keeps_what_the_owner_sees_and_is_bound_to_the_new_object ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
```

`cargo test -p filesquid-api --locked` (every test binary passed; the feature harness's section for this task and its totals):

```text
Feature: Copy provenance and streamed copies
  Scenario: A copy's record derives it from its source
   ?  Given copy-history file "report.pdf" at version v3 in collection "Work"
      Step skipped: tests/features/copy_provenance_and_streamed_copies_API-S-011.feature:16:5
  Scenario Outline: A copy of any size streams with bounded memory on a disk-backed staging area
   ✔  Given copy-history file of 1 MiB in collection "Work" of a library whose staging area is on disk
   ✔  When copy-history caller copies it into collection "Archive"
   ✔  Then copy-history copy should have the source's SHA-256 and size
   ✔  And copy-history copy should never hold the whole file in memory, its staged bytes going to the staging area on disk
  Scenario Outline: A copy of any size streams with bounded memory on a disk-backed staging area
   ✔  Given copy-history file of 65 MiB in collection "Work" of a library whose staging area is on disk
   ✔  When copy-history caller copies it into collection "Archive"
   ✔  Then copy-history copy should have the source's SHA-256 and size
   ✔  And copy-history copy should never hold the whole file in memory, its staged bytes going to the staging area on disk
  Scenario Outline: A copy of any size streams with bounded memory on a disk-backed staging area
   ✔  Given copy-history file of 600 MiB in collection "Work" of a library whose staging area is on disk
   ✔  When copy-history caller copies it into collection "Archive"
   ✔  Then copy-history copy should have the source's SHA-256 and size
   ✔  And copy-history copy should never hold the whole file in memory, its staged bytes going to the staging area on disk
  Scenario: The history route of the copy names its source
   ?  Given copy-history copy of "report.pdf" at version v3
      Step skipped: tests/features/copy_provenance_and_streamed_copies_API-S-011.feature:35:5
  Scenario: A source that changes during a streamed copy fails with nothing left staged
   ✔  Given copy-history file of 65 MiB that changes while it is read
   ✔  When copy-history caller copies it into collection "Archive"
   ✔  Then copy-history copy should fail with nothing committed and nothing left staged
47 features
571 scenarios (375 passed, 196 skipped)
1980 steps (1784 passed, 196 skipped)
```

The two skipped scenarios of that feature are the provenance scenarios of Task API-F2.23.

`cargo test -p filesquid-desktop --locked --test integration_streamed_copy`:

```text
     Running tests/integration/streamed_copy.rs (<target>/debug/deps/integration_streamed_copy-97f3eb377cc5ddf3)
test a_copy_above_64_mib_streams_through_the_scheme ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.06s
```

## Coverage

The repository sets no numeric coverage threshold for `filesquid-api`; every branch added in `src/files/copy.rs` has a test above: the committed path (0 bytes, 1 MiB, 65 MiB, 600 MiB), the stale source at the commit, the changed source, the unreadable source, and the cleared stage after each failure.

## Hook

The head-commit run and its `Test scope:` line are in the pull request.
