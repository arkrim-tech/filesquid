# Task IO-1.6 Walkthrough: Copy-guard warnings name the object

Branch `feat/io-1.6-copy-guard-object-ref`. Module task [IO-1.6](../plan.md) of global Task FS-2.9 ([implementation plan](../../../../../../resources/implementation-plan.md)).

## OCFL-S-025: Copy-guard warnings name the object (the warning a refused copy (OCFL-S-019) emits MUST name the object the copy concerns by its identifier, and its version wherever the call knows it, beside the destination and the declaring root, so a host can journal it with an object reference)

The copy guard's `tracing` warning (`refused a copy into an OCFL root`, target `filesquid_ocfl::read::copy_guard`) keeps `destination` and `root` and adds:

| Call | `object` | `version` |
|---|---|---|
| `Object::copy_file` | the source object's inventory `id` | the version read |
| `ChangeModel::checkout` | the change model's object | the version checked out |
| Staging writes (`stage_bytes`, `stage_tree`, index and blob writes, clear) | the object the change model was built for | none: the call knows no version |

`object` and `version` are the field names the API's journal layer already reads from `filesquid_ocfl` events (`crates/api/src/trace_layer.rs`), so the API can journal `<object>@<version>` as the object reference with no change in `filesquid-api`, whenever the identifier and version form a reference its journal accepts (`is_object_ref`). Tests: `integration_copy_guard_warnings` (4 tests; three run on MemoryBackend and FsBackend) and `copy_guard_warning_OCFL-S-025.feature` (3 scenarios).

## OCFL-S-019: Inspection and copy never edit an object (the refusal rules and the returned error stay as they are)

Each test also asserts the error is still `ReadError::CopyIntoOcflRoot` (with the same `key` and `root` for `copy_file`) and that nothing was written at the destination (`obj/v1/content/c.txt`, `obj/out`, `obj/staging`). The existing `read_only_copy_OCFL-S-019.feature` and `integration_change_model` refusal tests pass unchanged.

## API-S-031: Journal query, search and export (engine warnings journaled with operation and object reference fields)

The engine side of the copy guard's object reference. With a version the API journals `object_ref = <object>@<version>`; a staging refusal names the object only, which the API's journal layer records as a target ID without an object reference, because it builds the reference only from both fields.

## SRS-F-061: Activity journal and live health monitoring

A refused copy's journal entry can now point at the object it concerns rather than only a storage path.

## What changed

- `src/read/copy_guard.rs`: `CopyConcern { object, version }` argument to `refuse_copy_into_ocfl_root` and a `warn_refused` helper that emits the fields; the refusal checks are untouched.
- `src/read/object.rs`: `copy_file` passes its object `id` and version.
- `src/change/model.rs`: `checkout` passes the model's object and version; `ChangeModel::new` records the object on its staging area.
- `src/change/area.rs`: `StagingArea` keeps the object identifier it belongs to (`for_object`, crate-private) and names it in the guard's warning. The public API is unchanged.
- Tests: `tests/support/warning_capture.rs` (a `tracing`-only subscriber that records warning fields; no new dependency), `tests/integration/copy_guard_warnings.rs`, `tests/features/copy_guard_warning_OCFL-S-025.feature`, `tests/support/copy_warning_steps.rs`.
- Docs: crate README (the warning's fields), module plan row, checklist and MVP progress.

## TDD: the tests failed first

Before the implementation, `integration_copy_guard_warnings` failed on the missing fields (raw excerpt):

```text
thread 'a_refused_copy_file_names_the_source_object_and_version' (16527) panicked at crates/ocfl/tests/integration/copy_guard_warnings.rs:86:5:
assertion `left == right` failed: {"destination": "obj/v1/content/c.txt", "message": "refused a copy into an OCFL root", "root": "obj"}
  left: None
 right: Some("urn:example:obj")

failures:
    a_refused_checkout_names_the_object_and_version
    a_refused_copy_file_names_the_source_object_and_version
    a_refused_staging_write_names_the_object_without_a_version

test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

## Test output

`cargo test -p filesquid-ocfl --test integration_copy_guard_warnings --locked`:

```text
   Compiling filesquid-ocfl v0.1.0 (<repo>/apps/filesquid/crates/ocfl)
    Finished `test` profile [unoptimized] target(s) in 1.22s
     Running tests/integration/copy_guard_warnings.rs (<target>/debug/deps/integration_copy_guard_warnings-2815ebb654452722)

running 4 tests
test the_warning_comes_from_the_engine_target ... ok
test a_refused_copy_file_names_the_source_object_and_version ... ok
test a_refused_staging_write_names_the_object_without_a_version ... ok
test a_refused_checkout_names_the_object_and_version ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

`cargo test -p filesquid-ocfl --test features --locked` (the new feature and the summary; the three skipped steps are the existing `reusable_ocfl_engine_F-051.feature` scenarios owned by other crates):

```text
Feature: Copy-guard warnings name the object
  Scenario: A refused file copy names the source object and version
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When I copy "b.txt" of "v1" into "obj/v1/content/c.txt" while warnings are recorded
   ✔  Then the copy should be refused and one copy-guard warning recorded
   ✔  And the warning should name the destination "obj/v1/content/c.txt" and the root "obj"
   ✔  And the warning should name the object "urn:example:obj" at version "v1"
  Scenario: A refused checkout names the object and version
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When I check out "v1" under "obj/out" while warnings are recorded
   ✔  Then the copy should be refused and one copy-guard warning recorded
   ✔  And the warning should name the destination "obj/out/a.txt" and the root "obj"
   ✔  And the warning should name the object "urn:example:obj" at version "v1"
  Scenario: A refused staging write names the object without a version
   ✔> Given an OCFL object "obj" committed at "v1" through the engine writer
   ✔  When I stage "d.txt" in a staging area under "obj/staging" while warnings are recorded
   ✔  Then the copy should be refused and one copy-guard warning recorded
   ✔  And the warning should name a destination under "obj/staging/" and the root "obj"
   ✔  And the warning should name the object "urn:example:obj" without a version
[Summary]
17 features
158 scenarios (155 passed, 3 skipped)
741 steps (738 passed, 3 skipped)
```

`cargo test -p filesquid-ocfl --locked` (every test binary of the crate, result lines):

```text
test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.05s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.64s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.96s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.72s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.89s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.42s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.71s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.24s
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.20s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Pre-commit

The head commit's `FULL=1 bash scripts/pre-commit.sh` summary line is recorded in the pull request.

## Mocked Environment Variables

None.
