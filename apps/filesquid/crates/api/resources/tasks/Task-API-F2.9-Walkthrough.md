# Task API-F2.9 Walkthrough: PROV-O records, history routes and backfill

Module task API-F2.9 of the `filesquid-api` [plan](../plan.md). Branch `feat/api-f2.9-history`, built on `feat/prov-o-history` (the `filesquid-prov` crate of PV-0.1 and PV-0.2). It follows ADR 040 Decisions 3 to 9 (`resources/architecture/040-prov-o-history.md`, added by the requirements branch) and the owner decisions on PROV-O: no `prov:Person`; the installation UUID is the agent `urn:uuid:<installation-uuid>` and the OCFL `user` address; a commit's journal entries go in the same OCFL commit at `provenance/<version>.journal.jsonl`.

## API-S-028: PROV-O record per commit and journal linkage

> Every commit the API makes or drives MUST stage `provenance/<version>.jsonld` through the version-bound file hook (OCFL-S-024) built by `filesquid-prov`, set the version `user` and the `filesquid.operation.v1` message, and emit an `ocfl.commit` journal entry. The journal entry IDs of a commit MUST be assigned before it, and repair consent, outcome and follow-up commit MUST share one repair run ID.

Parents: PRD F-067 PROV-O history of every version and change, SRS-F-067, PRD F-061 Activity journal and live health monitoring, ADR 040.

## API-S-029: History routes and backfill

> `GET /api/v1/files/{id}/history` and `GET /api/v1/history` (filters: object version, operation, agent, from, to; cursor paging) MUST answer `HistoryEntry` items, and `GET /api/v1/files/{id}/versions/{version}/prov` the raw JSON-LD record, all under route action `history.read`; any history index MUST rebuild from the OCFL records. A backfill job MUST register in the process registry, stop at its cap or on cancel, resume by rescanning the storage root and skipping versions that have a record, and commit one provenance-only version per object. The OCFL records are the checkpoint: no separate progress file is kept, an object whose versions all have records is skipped, and a run that stopped part way continues with the versions still missing one.

Parents: PRD F-067, SRS-F-067, PRD F-065 File version history and retention, ADR 040.

## API-S-032: Durable journal

> Every committed operation MUST stage its journal entries as `provenance/<version>.journal.jsonl` beside its PROV-O record in the same commit (API-S-028).

Parents: PRD F-061, SRS-F-061, PRD F-001, PRD F-042, [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md). Only the per-commit part is in this task; the sealing service and the journal rebuild are out of scope (owner decision).

PRD F-046 (privacy on export and sharing) is served by the redacted export below.

## What changed

- **Dependency**: `filesquid-api` depends on `filesquid-prov` with the `ocfl` feature. The application-layer isolation guard (API-S-006) allows it: `ALLOWED_MODULES` in [`tests/support/isolation.rs`](../../tests/support/isolation.rs) and the examples of [`application_layer_isolation_API-S-006.feature`](../../tests/features/application_layer_isolation_API-S-006.feature). The server's TC-005 isolation steps are unbound (skipped), so they needed no change.
- **`api::history`** ([`src/history/`](../../src/history/mod.rs)):
  - `ProvenanceContext { installation, host }`.
  - `version_user` builds the OCFL `user` with name `FileSquid installation` and address `urn:uuid:<uuid>`.
  - `CommitOperation` holds the operation ID, kind, start time, label, moves and actor. `commit_meta` builds the `VersionMeta` with the installation user and the `filesquid.operation.v1` message. Any commit path can use it, including the metadata store's commit options.
  - `CommitRecorder` is a `VersionBoundFiles`. For the reserved version it returns two files:
    - `provenance/<version>.jsonld`, built with `filesquid_prov::ocfl::bound_record` and naming the journal entry ID allocated before the commit;
    - `provenance/<version>.journal.jsonl`: the redacted `ocfl.commit` entry carrying that ID, the operation ID and `object_ref` `<object-id>@<version>`.

    A rebase calls it again, so both files name the published version. `journal_entries(&receipt)` returns what the caller appends after the commit: the carried entry, plus an `ocfl.lease.takeover` warning when the reservation took over a lease.
- **Repair follow-up** ([`src/local_library/provenance.rs`](../../src/local_library/provenance.rs), `LocalLibraryService::with_provenance(context, ids)`):
  - After each repaired object, the service commits one provenance-only version through `ChangeModel::commit_bound_only`. It uses an in-memory staging area and the run's reservation holder. The record has kind `repair`, and its operation ID is the repair run ID that the consent and outcome rows already carry.
  - The commit's entries are appended through the journal queue under their pre-allocated IDs.
  - `ObjectRepair` gains two optional fields, omitted from the JSON when unset: `provenance_version` and `provenance_error_code`.
  - Without a provenance context, the answer says `provenance_not_configured` and a `prov.record` warning is journaled. When the record cannot be written, the answer says `journal_ids_unavailable` or `provenance_commit_failed` and a failed `prov.record` entry is journaled. The repair itself stands.
- **Backfill job** ([`src/history/backfill.rs`](../../src/history/backfill.rs)):
  - `Backfill::new(context, ids, journal, registry).run(root, reservations, &cancel)` walks every object of the storage root.
  - For each object with versions that have no record, it stages the reconstructed records with `stage_reconstructed` and commits them in one version that carries the run's own record (kind `backfill`).
  - **Bounded:** at most `with_max_versions` records per run (default 1000), and `reached_cap` says when to run again.
  - **Resumable and idempotent:** what is recorded is read back from OCFL, so a rerun commits nothing.
  - **Cancellable** between objects.
  - **Registered** in the `ProcessRegistry`, with progress reported per object.
  - **Journaled:** `prov.backfill` started, failed per object, and succeeded or cancelled; `ocfl.commit` for each commit. A failure on one object does not stop the run.
- **History sources** ([`src/history/source.rs`](../../src/history/source.rs)):
  - A host injects a `HistorySource` with `ApiState::install_history`. The routes answer `503 unavailable` (retryable) until it does.
  - `MemoryHistory` holds parsed records.
  - `OcflHistory` reads every record of a storage root through `filesquid-prov`. It is bounded to 100 000 objects and 100 000 records, and it rebuilds the `HistoryIndex` from the records on every call. It drives the engine with `complete_immediately`, so it serves the filesystem and in-memory backends.
  - `entry_of` maps an activity to the core `HistoryEntry`. A reconstructed record has `started_at: null`, and the label is read from the version message.
- **Routes** ([`src/history/routes.rs`](../../src/history/routes.rs)). All of them use the new `Action::ReadHistory` (`history.read`), which `HostPolicy` grants to the owner and the administrator.
  - `GET /api/v1/history`: everything, or one filter dimension (`object`, `operation`, `agent`, or `from`/`to`). More than one dimension is `invalid_request`. Pages of 1 to 1000 entries (default 100) with an opaque `cursor`.
  - `GET /api/v1/files/{id}/history`: the object `urn:uuid:<id>`. An ID that is not opaque is `invalid_request`.
  - `GET /api/v1/files/{id}/history/export`: every record of the object as one `application/ld+json` array, as an attachment. With `redact=true` it is redacted.
  - `GET /api/v1/files/{id}/versions/{version}/prov`: the JSON-LD record, or `not_found`.
- **Redaction (PRD F-046)** ([`src/history/redact.rs`](../../src/history/redact.rs)): `Redactor` and `redact_record` remove agent identities, version messages and identity-bearing logical paths from a record:
  - The installation agent becomes `urn:filesquid:agent:redacted:installation`.
  - `fsq:message` is dropped.
  - Every `fsq:path` becomes `redacted-<n>`, and the entity IDs use the same token. The numbering is consistent across the records of one export, so derivations stay linked.
  - References are rewritten. Object IDs, versions, operation IDs, digests, times and journal IDs are kept.
- **Contract `0.8.0`** ([`openapi.json`](../../openapi.json), `API_VERSION`):
  - The four paths, with `x-filesquid-action: history.read`, bearer security (inherited) and both denials.
  - New responses `NotFound` and `HistoryUnavailable`.
  - New schemas `HistoryPage`, `HistoryEntry`, `OperationKind`, `VersionRef` and `JournalLink`, each with Rust samples in the contract test.
  - The contract tests fill path parameters from a sample history (`tests/support/history_fixture.rs`).
  - `apps/filesquid/ui/src/api/schema.d.ts` was regenerated with `npm run contract:types` (`openapi-typescript` 7.13.0).

## Test → requirement mapping

| Requirement | Tests |
|---|---|
| API-S-028 version metadata | `the_version_user_names_the_installation_and_never_a_person`, `commit_meta_sets_the_user_and_the_operation_message` |
| API-S-028 / API-S-032 record and journal in the same commit | `a_commit_writes_its_record_and_journal_entries_in_the_same_version`, `a_rebased_commit_regenerates_both_files_for_the_new_version`, `a_takeover_adds_a_warning_after_the_commit_entry`, `a_receipt_for_another_version_still_journals_the_commit`, `a_record_that_cannot_be_built_stops_the_commit` |
| API-S-028 redaction of every new journal event (`ocfl.commit`, `prov.record`, `prov.backfill`) | `every_new_journal_event_keeps_its_opaque_fields_and_loses_paths` |
| API-S-028 repair follow-up | `a_repair_commits_a_provenance_record_under_the_run_id`, `without_a_provenance_context_the_missing_record_is_journaled_and_answered`, `a_record_that_cannot_be_written_is_journaled_as_failed_and_the_repair_stands`, `a_refused_object_gets_no_provenance_follow_up`; BDD "A repaired object gets a provenance record under the repair run ID", "Without a provenance context the repair says that no record was written" |
| API-S-029 backfill | `a_backfill_reconstructs_every_missing_record_and_a_rerun_commits_nothing`, `a_backfill_stops_at_its_cap_and_the_next_run_continues`, `a_cancelled_backfill_commits_nothing_and_ends_its_process_cancelled`, `a_failing_object_is_journaled_and_the_run_goes_on`, `a_backfill_without_a_storage_root_fails_its_process`; BDD "A backfill reconstructs every missing record once", "A backfill stops at its cap and the next run continues" |
| API-S-029 index rebuilt from OCFL records | `an_ocfl_history_is_rebuilt_from_the_records_in_the_storage_root`, `an_ocfl_history_without_a_storage_root_is_unavailable`, `a_memory_history_indexes_its_records_and_refuses_conflicts`, `a_history_entry_carries_the_label_and_marks_reconstruction` |
| API-S-029 routes | `the_history_lists_everything_or_one_filter_in_cursor_pages`, `bad_history_parameters_answer_invalid_request`, `a_file_history_lists_its_object_only`, `a_version_record_is_raw_json_ld_or_not_found`, `history_routes_answer_unavailable_before_the_host_installs_a_source`, `action_names_are_unique_dotted_identifiers`, `every_routed_path_declares_its_action`, `every_route_answers_the_per_launch_token_holder`; BDD scenarios 5 to 11 |
| API-S-029 contract | `contract_is_openapi_3_1_with_the_crate_api_version`, `every_documented_operation_answers_a_documented_status_and_body`, `every_component_schema_matches_its_serialized_rust_type`, Vitest `contract.test.ts` |
| PRD F-046 redaction | `a_redacted_record_names_no_installation_message_or_path`, `redaction_keeps_derivations_linked_and_paths_numbered_across_records`, `the_export_is_a_json_ld_array_and_can_be_redacted`, the `redact.rs` unit tests; BDD "A redacted export names no installation and no path" |
| API-S-006 isolation | `unit_isolation` (13 tests), BDD "The FileSquid modules may be linked" (now including `filesquid-prov`) |

Mocked Environment Variables: None.

## Red: tests before the implementation

The new test files, run against the code before this change (`cargo test -p filesquid-api --locked --test unit_history --test unit_history_jobs`; distinct errors):

```text
error: could not compile `filesquid-api` (test "unit_history") due to 4 previous errors
error: could not compile `filesquid-api` (test "unit_history_jobs") due to 11 previous errors
error[E0432]: unresolved import `filesquid_api::history`
error[E0432]: unresolved imports `filesquid_api::local_library::JOURNAL_IDS_UNAVAILABLE`, `filesquid_api::local_library::PROVENANCE_NOT_CONFIGURED`
error[E0433]: cannot find `history` in `filesquid_api`
error[E0599]: no method named `install_history` found for reference `&ApiState` in the current scope
error[E0599]: no method named `install_history` found for struct `ApiState` in the current scope
error[E0599]: no method named `with_provenance` found for struct `LocalLibraryService` in the current scope
error[E0609]: no field `provenance_error_code` on type `&ObjectRepair`
error[E0609]: no field `provenance_version` on type `&ObjectRepair`
```

## Green: test output

`cargo test -p filesquid-api --locked`, the two new unit targets:

```text
     Running tests/unit/history.rs (<target>/debug/deps/unit_history-7c960a311257f101)

running 20 tests
test a_history_entry_carries_the_label_and_marks_reconstruction ... ok
test a_memory_history_indexes_its_records_and_refuses_conflicts ... ok
test a_receipt_for_another_version_still_journals_the_commit ... ok
test a_record_that_cannot_be_built_stops_the_commit ... ok
test a_file_history_lists_its_object_only ... ok
test a_redacted_record_names_no_installation_message_or_path ... ok
test a_takeover_adds_a_warning_after_the_commit_entry ... ok
test a_commit_writes_its_record_and_journal_entries_in_the_same_version ... ok
test an_ocfl_history_without_a_storage_root_is_unavailable ... ok
test a_version_record_is_raw_json_ld_or_not_found ... ok
test commit_meta_sets_the_user_and_the_operation_message ... ok
test every_new_journal_event_keeps_its_opaque_fields_and_loses_paths ... ok
test history_routes_answer_unavailable_before_the_host_installs_a_source ... ok
test an_ocfl_history_is_rebuilt_from_the_records_in_the_storage_root ... ok
test redaction_keeps_derivations_linked_and_paths_numbered_across_records ... ok
test a_rebased_commit_regenerates_both_files_for_the_new_version ... ok
test bad_history_parameters_answer_invalid_request ... ok
test the_version_user_names_the_installation_and_never_a_person ... ok
test the_export_is_a_json_ld_array_and_can_be_redacted ... ok
test the_history_lists_everything_or_one_filter_in_cursor_pages ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

```text
     Running tests/unit/history_jobs.rs (<target>/debug/deps/unit_history_jobs-9865c4a0ab4bf994)

running 9 tests
test a_backfill_without_a_storage_root_fails_its_process ... ok
test a_cancelled_backfill_commits_nothing_and_ends_its_process_cancelled ... ok
test a_record_that_cannot_be_written_is_journaled_as_failed_and_the_repair_stands ... ok
test a_refused_object_gets_no_provenance_follow_up ... ok
test a_failing_object_is_journaled_and_the_run_goes_on ... ok
test a_repair_commits_a_provenance_record_under_the_run_id ... ok
test without_a_provenance_context_the_missing_record_is_journaled_and_answered ... ok
test a_backfill_reconstructs_every_missing_record_and_a_rerun_commits_nothing ... ok
test a_backfill_stops_at_its_cap_and_the_next_run_continues ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

The new BDD feature (`prov_o_history_records_routes_and_backfill_API-S-029.feature`):

```text
Feature: PROV-O records, history routes and backfill in filesquid-api
  Scenario: A repaired object gets a provenance record under the repair run ID
   ✔  Given a library with an object whose head has no version inventory, recording provenance
   ✔  When the owner repairs that object
   ✔  Then a new version of the object should hold a PROV-O record of kind "repair"
   ✔  And the record, the consent and the outcome should share the repair run ID
   ✔  And the new version should carry its "ocfl.commit" journal entry under the ID the journal stored
  Scenario: Without a provenance context the repair says that no record was written
   ✔  Given a library with an object whose head has no version inventory, without provenance
   ✔  When the owner repairs that object
   ✔  Then the repair answer should name the provenance error "provenance_not_configured"
   ✔  And a "prov.record" warning should be journaled under the repair run ID
  Scenario: A backfill reconstructs every missing record once
   ✔  Given a library of 2 objects committed before the history existed
   ✔  When the history backfill runs
   ✔  Then every version should have a record and the earlier ones should be marked reconstructed
   ✔  And the backfill process should end "completed" with 2 of 2 objects done
   ✔  When the history backfill runs again
   ✔  Then the second run should commit nothing
  Scenario: A backfill stops at its cap and the next run continues
   ✔  Given a library of 2 objects committed before the history existed
   ✔  When the history backfill runs with a cap of 3 records
   ✔  Then the run should report that it reached its cap
   ✔  When the history backfill runs again
   ✔  Then every version should have a record and the earlier ones should be marked reconstructed
  Scenario: The history of a file lists its activities in commit order
   ✔  Given a history holding 2 versions of one file and 1 version of another
   ✔  When the caller lists the history of the first file
   ✔  Then the answer should hold 2 entries of that file, oldest first
  Scenario: The history is paged with an opaque cursor
   ✔  Given a history holding 2 versions of one file and 1 version of another
   ✔  When the caller lists the whole history 2 entries at a time
   ✔  Then the pages should hold 2 and then 1 entries and the last page no cursor
  Scenario: The record of one version is raw JSON-LD
   ✔  Given a history holding 2 versions of one file and 1 version of another
   ✔  When the caller reads the record of version "v2" of the first file
   ✔  Then the answer should be the JSON-LD record of that version
  Scenario: A version without a record is not found
   ✔  Given a history holding 2 versions of one file and 1 version of another
   ✔  When the caller reads the record of version "v9" of the first file
   ✔  Then the response should be an "not_found" error with a message
  Scenario: Two filter dimensions at once are refused
   ✔  Given a history holding 2 versions of one file and 1 version of another
   ✔  When the caller lists the history by object and by operation at once
   ✔  Then the response should be an "invalid_request" error with a message
  Scenario: A redacted export names no installation and no path
   ✔  Given a history holding 2 versions of one file and 1 version of another
   ✔  When the caller exports the history of the first file redacted
   ✔  Then the export should be a JSON-LD array of 2 records without the installation or any path
  Scenario: The history routes wait for the host to install its history
   ✔  Given a host that has not installed its history yet
   ✔  When the caller lists the history of the first file
   ✔  Then the response should be an "unavailable" error with a message
   ✔  And the error should be retryable
```

`cargo test -p filesquid-api --locked`, every target:

```text
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_api-20413012dc7e1373)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/features.rs (<target>/debug/deps/features-fc64a968fed95c81)
[Summary]
14 features
223 scenarios (117 passed, 106 skipped)
533 steps (427 passed, 106 skipped)
     Running tests/unit/admin.rs (<target>/debug/deps/unit_admin-5fa00582e5f318a3)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/auth.rs (<target>/debug/deps/unit_auth-bb726c082c8a9c71)
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/contract.rs (<target>/debug/deps/unit_contract-00544b705de8fea3)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/unit/errors.rs (<target>/debug/deps/unit_errors-4b0a2ec1d0e48498)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/events.rs (<target>/debug/deps/unit_events-1e08bd3467dfd6ba)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/health.rs (<target>/debug/deps/unit_health-d26dd9a189ca6e98)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health_rules.rs (<target>/debug/deps/unit_health_rules-b02690d5abe62766)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/history.rs (<target>/debug/deps/unit_history-7c960a311257f101)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/history_jobs.rs (<target>/debug/deps/unit_history_jobs-9865c4a0ab4bf994)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/unit/host.rs (<target>/debug/deps/unit_host-adff7444f1cb2ca4)
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/unit/isolation.rs (<target>/debug/deps/unit_isolation-872924a634b416f9)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
     Running tests/unit/journal_linkage.rs (<target>/debug/deps/unit_journal_linkage-8bd9d91fadcc6a46)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/library.rs (<target>/debug/deps/unit_library-f5ac86f5f4a46fdb)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/library_queries.rs (<target>/debug/deps/unit_library_queries-c7e1d3361690c539)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/local_library.rs (<target>/debug/deps/unit_local_library-4fe0631602c6ed81)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.75s
     Running tests/unit/metrics.rs (<target>/debug/deps/unit_metrics-2dbe1f362e97dd2a)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/registry.rs (<target>/debug/deps/unit_registry-2be15e0146373137)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/replica.rs (<target>/debug/deps/unit_replica-26666a2018c7cb76)
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
     Running tests/unit/routes.rs (<target>/debug/deps/unit_routes-bdb34a0af0db5483)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/version_inventory_repair.rs (<target>/debug/deps/unit_version_inventory_repair-fa285727da1013f3)
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/unit/writer.rs (<target>/debug/deps/unit_writer-356eb66f76a3cee3)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo test -p filesquid-server --locked`:

```text
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_server-5df5078ce63e817b)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
     Running unittests src/main.rs (<target>/debug/deps/filesquid_server-4b4492bf722409cf)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/features.rs (<target>/debug/deps/features-5dcf7078dfcd2fce)
[Summary]
5 scenarios (5 passed)
26 steps (26 passed)
     Running tests/integration/auth.rs (<target>/debug/deps/integration_auth-01ff1003e30c00eb)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
     Running tests/integration/events.rs (<target>/debug/deps/integration_events-a3ab4e8478d43333)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/integration/health.rs (<target>/debug/deps/integration_health-c0a00ff929e3eae9)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/host.rs (<target>/debug/deps/integration_host-31279ea267062226)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/monitor.rs (<target>/debug/deps/integration_monitor-f1e78bb3c2403db7)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/static_ui.rs (<target>/debug/deps/integration_static_ui-c62e9b2e7eed8f38)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health.rs (<target>/debug/deps/unit_health-af054275fd415299)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/native_features.rs (<target>/debug/deps/unit_native_features-629c2bcffcdceac9)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo test -p filesquid-desktop --locked`:

```text
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_desktop_lib-f1f7863e3a5b33d9)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (<target>/debug/deps/filesquid_desktop-1e6a178f66a7e6e5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/api_protocol.rs (<target>/debug/deps/integration_api_protocol-8b1990d26d548ad4)
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/capabilities.rs (<target>/debug/deps/integration_capabilities-6e729d358b2b16c7)
test result: ok. 55 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/event_channel.rs (<target>/debug/deps/integration_event_channel-4b12a67c0776231c)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/integration/host_events.rs (<target>/debug/deps/integration_host_events-1518ef3c81cc06d4)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
     Running tests/integration/ipc_gate.rs (<target>/debug/deps/integration_ipc_gate-470752e7629f5d11)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/library_catalog.rs (<target>/debug/deps/integration_library_catalog-2bd671ada43da5d5)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/stdout_logging.rs (<target>/debug/deps/integration_stdout_logging-0f48aad86537c29d)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo clippy -p filesquid-api -p filesquid-desktop -p filesquid-server --all-targets --locked -- -D warnings` (no warnings):

```text
Checking filesquid-api v0.1.0 (<repo>/apps/filesquid/crates/api)
    Checking filesquid-native v0.1.0 (<repo>/apps/filesquid/crates/native)
    Checking filesquid-server v0.1.0 (<repo>/apps/filesquid/crates/server)
    Checking filesquid-desktop v0.1.0 (<repo>/apps/filesquid/crates/desktop)
    Finished `dev` profile [unoptimized] target(s) in 7.85s
```

`npx vitest run tests/unit/contract.test.ts` in `apps/filesquid/ui` (after `npm run contract:types`):

```text
RUN  v4.1.12 <repo>/apps/filesquid/ui

 ✓ tests/unit/contract.test.ts (1 test) 104ms

 Test Files  1 passed (1)
      Tests  1 passed (1)
   Start at  06:05:16
   Duration  1.20s (transform 111ms, setup 90ms, import 764ms, tests 104ms, environment 0ms)

JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

## Review round 1

Fixes for the first Cubic review of the history and repair code:

- **Export memory bound.** `GET /files/{id}/history/export` writes each record straight into one body instead of building every record as JSON and then serialising a second copy. The body is capped at `MAX_EXPORT_BYTES` (64 MiB) and stops at a whole record, so the array stays valid. A partial export carries `X-History-Truncated: true`.
- **Truncated history is flagged.** `HistorySource::index` returns a `HistorySnapshot { index, truncated }`. `OcflHistory` sets `truncated` when its record or object cap leaves records out (`with_max_records` sets a lower cap). `HistoryPage` gains a required `truncated` field, and the export sets the header. `openapi.json` and `ui/src/api/schema.d.ts` are updated.
- **Version record subject check.** The version route checks that the stored record describes the requested object and version. A record stored under another version's path is `Corrupt` (500), and the index skips it.
- **Unreadable objects are not a 404.** When no object matched and one could not be opened (or lies past the object cap), the version route answers `Unavailable` (503) instead of `not_found`.
- **Storage failures are retryable.** A storage failure while reading a record maps to `Unavailable` (503). Only format, profile, build and size failures stay `Corrupt`.
- **Identical records are held once.** `MemoryHistory::insert` accepts an identical record again without storing a second copy (new `len` and `is_empty`).
- **Redaction fails closed.** An `fsq:path` the redactor cannot number (no string `@id`, or a path that is not a string) is dropped.
- **Backfill cap and heartbeat.** `reached_cap` is set only after a successful commit uses up the budget with records of that object still missing. A failed object no longer reports the cap. The run sends a heartbeat between staging an object's records and committing them.
- **Refused repairs stay journal-only.** A refused repair outcome no longer carries `object_ref`. It still names the version as a target.
- **Documentation.** The module doc names ADR 040 in plain text (the ADR is on an unmerged branch). API-S-029 above describes the rescan-and-skip resumption. The gaps below separate the wired catalog provenance from the integrations that are still unwired.

```text
$ cargo test -p filesquid-api --locked --test unit_history --test unit_history_jobs
     Running tests/unit/history.rs (<target>/debug/deps/unit_history-7c960a311257f101)
running 26 tests
test a_history_entry_carries_the_label_and_marks_reconstruction ... ok
test a_memory_history_indexes_its_records_and_refuses_conflicts ... ok
test a_lookup_that_skipped_an_unreadable_object_is_unavailable ... ok
test a_receipt_for_another_version_still_journals_the_commit ... ok
test a_commit_writes_its_record_and_journal_entries_in_the_same_version ... ok
test a_rebased_commit_regenerates_both_files_for_the_new_version ... ok
test a_redacted_record_names_no_installation_message_or_path ... ok
test a_record_that_cannot_be_built_stops_the_commit ... ok
test a_takeover_adds_a_warning_after_the_commit_entry ... ok
test a_record_stored_under_another_version_is_corrupt ... ok
test an_identical_record_inserted_again_is_held_once ... ok
test a_file_history_lists_its_object_only ... ok
test a_truncated_history_is_flagged_on_pages_and_the_export ... ok
test an_ocfl_history_without_a_storage_root_is_unavailable ... ok
test a_version_record_is_raw_json_ld_or_not_found ... ok
test commit_meta_sets_the_user_and_the_operation_message ... ok
test every_new_journal_event_keeps_its_opaque_fields_and_loses_paths ... ok
test history_routes_answer_unavailable_before_the_host_installs_a_source ... ok
test bad_history_parameters_answer_invalid_request ... ok
test redaction_keeps_derivations_linked_and_paths_numbered_across_records ... ok
test an_ocfl_history_is_rebuilt_from_the_records_in_the_storage_root ... ok
test an_ocfl_history_past_its_record_cap_is_flagged_truncated ... ok
test the_version_user_names_the_installation_and_never_a_person ... ok
test the_export_is_a_json_ld_array_and_can_be_redacted ... ok
test the_history_lists_everything_or_one_filter_in_cursor_pages ... ok
test the_operations_own_entries_ride_the_version_with_their_ids ... ok
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/unit/history_jobs.rs (<target>/debug/deps/unit_history_jobs-9865c4a0ab4bf994)
running 11 tests
test a_backfill_without_a_storage_root_fails_its_process ... ok
test a_cancelled_backfill_commits_nothing_and_ends_its_process_cancelled ... ok
test a_failed_object_past_the_budget_does_not_reach_the_cap ... ok
test a_backfill_stops_at_its_cap_and_the_next_run_continues ... ok
test a_record_that_cannot_be_written_is_journaled_as_failed_and_the_repair_stands ... ok
test a_refused_object_gets_no_provenance_follow_up ... ok
test a_backfill_heartbeats_between_staging_and_commit ... ok
test a_failing_object_is_journaled_and_the_run_goes_on ... ok
test without_a_provenance_context_the_missing_record_is_journaled_and_answered ... ok
test a_repair_commits_a_provenance_record_under_the_run_id ... ok
test a_backfill_reconstructs_every_missing_record_and_a_rerun_commits_nothing ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

## How to run

```bash
cd apps/filesquid
cargo test -p filesquid-api --locked --test unit_history --test unit_history_jobs
cargo test -p filesquid-api --locked --test features
cd ui && npm run contract:types && npx vitest run tests/unit/contract.test.ts
```

The BDD run writes JUnit to `crates/api/tests/allure-results/cucumber-junit.xml` ([ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md)).

## Gaps

- **Wired: desktop catalog provenance.** The desktop catalog's snapshot commits already record their PROV-O history: the desktop host builds `SqliteCatalog::with_provenance` (Task DE-0.10), which uses `CommitRecorder` for every snapshot version it publishes.
- **Not wired: import and the file routes.** Import and upload (API-F1.3) and the file operations (API-F1.4) are not built yet, so their commits do not use `CommitRecorder` yet.
- **Not wired: the history source, the repair follow-up and the backfill.** No host calls `install_history`, `LocalLibraryService::with_provenance` (the repair-service provenance follow-up) or `Backfill` yet. The desktop coordinator (DE-0.3) will inject the library storage root, the installation UUID (DE-S-012) and the journal store. Until then the history routes answer `503 unavailable`.
- **No backfill route.** The admin route `POST /api/v1/admin/history/backfill` named in API-S-029 is not built; it waits on the same host injection. The job is complete and tested as a library function.
- **Backfill heartbeat granularity.** The run sends a heartbeat after each object and between staging and committing it. Staging the records of one object is a single `filesquid-prov` call (`stage_reconstructed`), so there is no heartbeat for each staged record. The per-run cap (`DEFAULT_MAX_VERSIONS`, 1000) keeps that call short.
- **The index is rebuilt on every request.** `OcflHistory` reads every record of the storage root each time (bounded). A cached index that is invalidated by commit events is left for later.
- **Read permission is per host, not per file.** The routes authorise `history.read` on the host. Per-file read permission arrives with server accounts (API-F1.9).
- **Object IDs survive redaction.** They are opaque UUIDs. Removing them would break joins inside a shared export.
- **Sealing and journal rebuild are out of scope.** The sealing service and the journal rebuild of API-S-032 were left out by owner decision.
