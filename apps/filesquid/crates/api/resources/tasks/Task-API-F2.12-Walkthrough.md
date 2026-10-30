# Task API-F2.12 Walkthrough: Operation IDs on file-route commits

Module task API-F2.12 of the `filesquid-api` [plan](../plan.md), global Task FS-2.9. Delivered in two parts, both on other tasks' branches: API-F2.8 gave every file-route commit the version `user`, the `filesquid.operation.v1` message with the command envelope's operation ID, `CommitRecorder` and the activity span ([walkthrough](./Task-API-F2.8-Walkthrough.md)). API-F2.9 (branch `feat/api-f2.9-prov-records-history-routes`) added the end-to-end test below and linked the import commits ([walkthrough](./Task-API-F2.9-Walkthrough.md)).

## API-S-031: Journal query and export, and engine warnings

The journal records every engine warning with the operation ID of the activity it was raised in. The file routes' commits are no exception: their versions carry their operation's journal entries (API-S-028, API-S-032).

Also API-S-028 (PROV-O record per commit and journal linkage) and API-S-032 (Durable journal).

## What changed

- `tests/integration/engine_warnings.rs` (binary `integration_engine_warnings`):
  - A stalled writer holds the next version's reservation, so the move takes it over, and the OCFL engine warns "took over an expired version reservation" inside the move's activity span.
  - The test asserts that every `ocfl.engine.*` entry the journal layer writes carries the move's operation ID.
  - It has a binary of its own: `tracing` caches callsite interest for the whole process, so a parallel test that reaches the engine's callsites without a subscriber raced with this test's thread-local subscriber.
- The code under test is the `activity` span (`trace_layer::activity_span`, opened through `canonical::message::commit_span` with the commit's operation ID) that API-F2.8 wraps around every file-route commit in `files::provenance::commit_staged`.

## SRS traceability

| Requirement | Test |
|---|---|
| API-S-031 engine warnings carry the operation ID during a file-route commit | `integration_engine_warnings::an_engine_warning_during_a_file_route_commit_carries_its_operation_id` |
| API-S-028 file-route commits carry the envelope's operation ID, `user`, message and record | `integration_commit_linkage` (API-F2.8) |
| API-S-032 per-commit journal entries for file routes | `journal_and_commit_linkage_API-S-028.feature`, F-061 "A committed operation carries its journal entries in the same OCFL commit" |

## TDD

With the commit's activity span removed (`commit.await` instead of `commit.instrument(prepared.span()).await` in `commit_staged`), `cargo test -p filesquid-api --test integration_engine_warnings`:

```text
thread 'an_engine_warning_during_a_file_route_commit_carries_its_operation_id' (25961) panicked at crates/api/tests/integration/engine_warnings.rs:54:5:
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

## Test output

`cargo test -p filesquid-api --locked --test integration_engine_warnings`:

```text
Running tests/integration/engine_warnings.rs (<target>/debug/deps/integration_engine_warnings-40656c2a64560c56)
test an_engine_warning_during_a_file_route_commit_carries_its_operation_id ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

## Mocked Environment Variables

None.
