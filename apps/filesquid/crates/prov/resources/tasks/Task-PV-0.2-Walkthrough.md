# Task PV-0.2 Walkthrough: `ocfl` feature: records in OCFL objects and reconstruction

Branch: `feat/prov-o-history` (global Task FS-2.8; [ADR 040](../../../../../../resources/architecture/040-prov-o-history.md) Decisions 3 and 7).

## PROV-S-006: `ocfl` feature

Also OCFL-S-024 (the record is written through the change-model hook) and PROV-S-001 (the feature is optional, off by default, adds only `filesquid-ocfl` with its default features off, and still builds for `wasm32-unknown-unknown`).

## What changed

- Cargo feature `ocfl = ["dep:filesquid-ocfl"]`. The isolation test allows exactly one optional dependency, `filesquid-ocfl`.
- `ocfl::RecordHook` (a `VersionBoundFiles`) and `ocfl::bound_record` / `version_facts` build the record of the commit in hand from `OperationFacts` and the change model's `BoundContext`. The record is written at `provenance/<version>.jsonld`, named after the reserved version and its `created` time, and regenerated after a rebase.
- `read_record` streams a record bounded by 1 MiB. It looks in the head first, because a reconstructed record first appears in the version that backfilled it, and in the version itself second.
- `list_records` and `missing_records` work from the head's state.
- `reconstruct` derives a record from an inventory alone:
  - `created` is the end (and start) time;
  - the `user` address is the installation agent only when it is `urn:uuid:<uuid v4>`;
  - the operation ID and kind come from a `filesquid.operation.v1` message, otherwise `unrecorded` and `unknown`;
  - entities come from the state diff with the previous version;
  - the record is marked `reconstructed` with its own `recordedAt`, and no agent is invented.
- `stage_reconstructed` stages the missing records through the change model. It is capped per run and stages nothing for versions that already have one, so it is idempotent. The caller commits them with the run's own record. Nothing writes outside the change model.
- The pre-commit gate (`scripts/verify_tests.sh` step 5c) runs clippy and tests with `--all-features` and both wasm32 builds (default features, `--features ocfl`).

## SRS traceability

| Requirement | Tests |
|---|---|
| PROV-S-006 | `unit_ocfl` (7 tests): record per commit, rebase regeneration, listing, reconstruction without invented agents, backfill in one version and idempotent, cap per run |
| PROV-S-001 | `unit_isolation::library_dependencies_are_serde_serde_json_and_thiserror` (one optional dependency), wasm32 check with `--features ocfl` |

## TDD: failing run before implementation

```text
 apps/filesquid/Cargo.lock | 1 +
 1 file changed, 1 insertion(+)
      1 error: could not compile `filesquid-prov` (test "unit_ocfl") due to 1 previous error
      1 error[E0432]: unresolved import `filesquid_prov::ocfl`
```

## Mocked Environment Variables

None.

## Raw test output

`cargo test -p filesquid-prov --all-features --locked --test unit_ocfl --test unit_isolation`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.42s
     Running tests/unit/isolation.rs (<target>/debug/deps/unit_isolation-c337441e5dc3acaa)

running 9 tests
test a_default_filesquid_dependency_would_fail ... ok
test a_feature_naming_native_or_c2pa_would_fail ... ok
test an_optional_filesquid_dependency_in_default_would_fail ... ok
test library_dependencies_are_serde_serde_json_and_thiserror ... ok
test the_crate_is_mit_licensed_and_unpublished_for_now ... ok
test the_crate_root_denies_missing_docs ... ok
test the_manifest_keeps_every_isolation_rule ... ok
test hosts_sqlite_and_media_crates_would_fail ... ok
test the_library_does_no_filesystem_network_or_process_io ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/ocfl.rs (<target>/debug/deps/unit_ocfl-1071fd6097652e4b)

running 7 tests
test a_reconstructed_record_invents_no_agent_and_reads_the_operation_message ... ok
test a_commit_writes_its_record_named_after_its_version ... ok
test backfill_is_capped_per_run ... ok
test record_paths_follow_the_version_name ... ok
test a_rebased_commit_regenerates_its_record_for_the_new_version ... ok
test records_are_listed_and_a_version_without_one_reads_as_none ... ok
test backfill_stages_every_missing_record_and_commits_them_in_one_version ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```
