# Task API-F2.9 Walkthrough: PROV-O records, history routes and backfill

Module task API-F2.9 of the `filesquid-api` [plan](../plan.md). Branches `feat/api-f2.9-history` (first part) and `feat/api-f2.9-prov-records-history-routes` (second part, below), built on `feat/prov-o-history` (the `filesquid-prov` crate of PV-0.1 and PV-0.2). It follows ADR 040 Decisions 3 to 9 (`resources/architecture/040-prov-o-history.md`, added by the requirements branch) and the owner decisions on PROV-O: no `prov:Person`; the installation UUID is the agent `urn:uuid:<installation-uuid>` and the OCFL `user` address; a commit's journal entries go in the same OCFL commit at `provenance/<version>.journal.jsonl`.

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

## Second part: import commits and the F-067 scenarios

Branch `feat/api-f2.9-prov-records-history-routes`. After the first part, API-F2.8 linked the file and metadata route commits ([walkthrough](./Task-API-F2.8-Walkthrough.md)), and API-F2.16 merged the import rework, whose route (API-S-036 Import route) "leaves the PROV-O record to API-S-029".

### What changed

- **Import commits** ([`src/files/import.rs`](../../src/files/import.rs), [`crates/native/src/import/`](../../../native/src/import/job.rs)):
  - `FileService::import_original` prepares its commit through `files::provenance`, like every other file write. The kind is `import` and the operation ID is the import job's. With provenance installed, the version `user` is the installation and the version carries its PROV-O record and journal entries.
  - It answers `Committed<UploadView>`. An import that an earlier run already committed answers no entries.
  - `canonical::commit_staged_original` takes a `BoundMeta`, and its bound files pass the same journal-file checks as the other commits.
  - The import write has no route entry of its own (`Write::operation` is now optional), because the job journals `import.file`.
  - The native job journals the commit's entries (`ocfl.commit`) right after the commit and before the file is registered, so a registration retried later, which replays the import with no entries, never loses them. That covers the retry only. A crash between the OCFL publication and the journal write still leaves the operational journal without those entries, because the replay answers none. With provenance installed, the version holds them in `provenance/<version>.journal.jsonl`, from which the journal rebuild (CL-1.9) restores them with the same IDs. Writing them back at library open, as the catalog reconcile does for snapshot commits (ADR 040 Decision 6, DE-S-011), is not done for import commits yet. Its `import.file` entry now names the file's object version (`object_ref`).
- **API-S-031 for file-route commits (API-F2.12):** `integration_engine_warnings` shows that an OCFL engine warning raised during a move (a lease takeover) is journaled with the move's operation ID. The test fails when the commit's activity span is removed. It runs in its own binary, because `tracing` caches callsite interest for the whole process. See the [API-F2.12 walkthrough](./Task-API-F2.12-Walkthrough.md).
- **`prov_o_history_F-067.feature` bound** ([`tests/support/prov_steps.rs`](../../tests/support/prov_steps.rs)) for what the router can drive:
  - one record in the version of each upload, copy, rename, move, delete, metadata edit, reinstate, revert and label commit, with exactly one activity of its kind naming the operation ID, the object version and the commit's journal entry IDs;
  - the entities and relations of a metadata edit;
  - the journal join without timestamps;
  - no version or record for a no-op save, a refused call or a single-file download;
  - an ordinary copy of the storage root read with `serde_json` alone.
- **Also bound:** the F-061 scenario "A committed operation carries its journal entries in the same OCFL commit".

### Not delivered, and why

- **The history object.** Exports, scrubbed exports, ZIP downloads and share or publication outputs have no route yet; the ZIP download is part of API-F2.2, which is open. No document fixes the history object's OCFL identifier or its path in the storage root. This is an owner decision to take before the first producer lands.
- **Scenarios left unbound in F-067:**
  - Operations only the desktop catalog commits (import through the catalog, hide, unhide, purge, restore from the recently deleted, catalog organisation) and the restart scenario; the desktop harness runs these.
  - The repair follow-up, the rebase and the backfill, which `prov_o_history_records_routes_and_backfill_API-S-029.feature` and the unit tests cover.
  - The crash reconciliation at library open (DE-S-011).
  - Sharing with a preview, which has no route.
- **Per-file read permission on the history routes** (API-S-029) still arrives with server accounts. The desktop owner reads every file.
- **Import entries after a crash.** An import commit's journal entries lost to a crash between publication and the journal write come back through the journal rebuild (CL-1.9), not at library open: the open-time reconcile of ADR 040 Decision 6 covers the catalog's snapshot commits only.

### TDD

The import test failed to compile before the change:

```text
1 error: could not compile `filesquid-native` (test "unit_import_file") due to 6 previous errors
      3 error[E0609]: no field `entries` on type `UploadView`
      3 error[E0609]: no field `view` on type `UploadView`
```

The engine-warning test with the commit's activity span removed (`cargo test -p filesquid-api --test integration_engine_warnings`):

```text
thread 'an_engine_warning_during_a_file_route_commit_carries_its_operation_id' (25961) panicked at crates/api/tests/integration/engine_warnings.rs:54:5:
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

The F-067 steps were bound after the code they cover existed (API-F2.8 and this task's first part). They pass, as the output below shows.

### Test output

`cargo test -p filesquid-native --features desktop --locked --test unit_import_file --test unit_import_job`:

```text
Running tests/unit/import_file.rs (<target>/debug/deps/unit_import_file-ac65c56891808d8f)
test a_size_mismatch_and_a_changed_source_are_retried_within_the_job ... ok
test a_missing_source_is_refused_without_retry ... ok
test a_size_that_is_not_the_size_read_is_refused_and_nothing_is_published ... ok
test a_source_that_cannot_be_read_while_staged_is_refused_as_source_unreadable ... ok
test a_source_that_changes_between_its_two_reads_is_refused_as_source_changed ... ok
test the_source_entity_tag_changes_when_the_file_is_rewritten ... ok
test an_import_stores_the_original_under_its_uuid_with_its_record_in_one_version ... ok
test an_import_commit_is_linked_to_the_journal_and_carries_its_record ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/import_job.rs (<target>/debug/deps/unit_import_job-8c310ba23c2c1a45)
test a_deleted_file_s_path_counts_as_taken ... ok
test a_file_committed_before_a_crash_is_answered_not_imported_twice ... ok
test a_crash_between_a_file_s_object_and_its_registration_resumes_and_registers_once ... ok
test a_file_that_keeps_failing_is_quarantined_after_five_attempts_and_the_rest_go_on ... ok
test a_folder_is_imported_as_one_job_with_a_checkpoint_per_file ... ok
test a_folder_is_listed_in_one_stable_order_with_its_subfolders ... ok
test a_folder_imported_again_is_numbered_beside_the_first ... ok
test a_plan_and_its_manifest_survive_a_restart_and_one_operation_has_one_job ... ok
test a_file_the_index_keeps_refusing_to_register_suspends_the_job_and_keeps_its_intent ... ok
test a_plan_without_a_destination_fails_its_job_as_destination_required ... ok
test a_numbered_name_over_255_bytes_is_quarantined_as_name_too_long ... ok
test a_source_whose_numbers_are_all_taken_is_quarantined_as_path_suffix_exhausted ... ok
test checkpoints_backoff_and_reserve_follow_their_rules ... ok
test a_queued_job_without_a_plan_is_failed_not_left_pending ... ok
test picked_files_are_imported_each_with_itself_as_its_root ... ok
test a_quit_suspends_between_files_and_the_resume_imports_only_the_rest ... ok
test what_cannot_be_listed_is_kept_with_its_code ... ok
test two_picked_folders_of_one_name_land_beside_each_other ... ok
test a_file_that_streams_longer_than_the_lease_keeps_the_job_s_lease_alive ... ok
test the_worker_runs_every_unfinished_job_beyond_one_queue_full ... ok
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.71s
```

`cargo test -p filesquid-api --locked --test integration_engine_warnings`:

```text
Running tests/integration/engine_warnings.rs (<target>/debug/deps/integration_engine_warnings-40656c2a64560c56)
test an_engine_warning_during_a_file_route_commit_carries_its_operation_id ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

`cargo test -p filesquid-api --locked --test features` (the F-067 feature and the run summary):

```text
Feature: PROV-O history of every version and change
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ?  When I import it
      Step skipped: tests/features/prov_o_history_F-067.feature:19:5
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ✔  When I upload it
   ✔  Then exactly one new OCFL version should be committed
   ✔  And that version should contain its PROV-O record named after the new version under "provenance/"
   ✔  And the record should hold exactly one prov:Activity of kind "upload" with the operation ID and the object@version
   ✔  And the record should name the journal entry IDs of that commit
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ✔  When I copy it
   ✔  Then exactly one new OCFL version should be committed
   ✔  And that version should contain its PROV-O record named after the new version under "provenance/"
   ✔  And the record should hold exactly one prov:Activity of kind "copy" with the operation ID and the object@version
   ✔  And the record should name the journal entry IDs of that commit
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ?  When I unhide it
      Step skipped: tests/features/prov_o_history_F-067.feature:19:5
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ✔  When I reinstate it
   ✔  Then exactly one new OCFL version should be committed
   ✔  And that version should contain its PROV-O record named after the new version under "provenance/"
   ✔  And the record should hold exactly one prov:Activity of kind "reinstate" with the operation ID and the object@version
   ✔  And the record should name the journal entry IDs of that commit
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ✔  When I label it
   ✔  Then exactly one new OCFL version should be committed
   ✔  And that version should contain its PROV-O record named after the new version under "provenance/"
   ✔  And the record should hold exactly one prov:Activity of kind "label" with the operation ID and the object@version
   ✔  And the record should name the journal entry IDs of that commit
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ?  When I add to a collection it
      Step skipped: tests/features/prov_o_history_F-067.feature:19:5
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ✔  When I rename it
   ✔  Then exactly one new OCFL version should be committed
   ✔  And that version should contain its PROV-O record named after the new version under "provenance/"
   ✔  And the record should hold exactly one prov:Activity of kind "rename" with the operation ID and the object@version
   ✔  And the record should name the journal entry IDs of that commit
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ✔  When I move it
   ✔  Then exactly one new OCFL version should be committed
   ✔  And that version should contain its PROV-O record named after the new version under "provenance/"
   ✔  And the record should hold exactly one prov:Activity of kind "move" with the operation ID and the object@version
   ✔  And the record should name the journal entry IDs of that commit
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ?  When I hide it
      Step skipped: tests/features/prov_o_history_F-067.feature:19:5
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ✔  When I delete it
   ✔  Then exactly one new OCFL version should be committed
   ✔  And that version should contain its PROV-O record named after the new version under "provenance/"
   ✔  And the record should hold exactly one prov:Activity of kind "delete" with the operation ID and the object@version
   ✔  And the record should name the journal entry IDs of that commit
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ?  When I restore it
      Step skipped: tests/features/prov_o_history_F-067.feature:19:5
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ?  When I purge it
      Step skipped: tests/features/prov_o_history_F-067.feature:19:5
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ✔  When I edit the metadata it
   ✔  Then exactly one new OCFL version should be committed
   ✔  And that version should contain its PROV-O record named after the new version under "provenance/"
   ✔  And the record should hold exactly one prov:Activity of kind "metadata.edit" with the operation ID and the object@version
   ✔  And the record should name the journal entry IDs of that commit
  Scenario Outline: Each file-changing operation commits exactly one PROV-O record in its own version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file in the library
   ✔  When I revert it
   ✔  Then exactly one new OCFL version should be committed
   ✔  And that version should contain its PROV-O record named after the new version under "provenance/"
   ✔  And the record should hold exactly one prov:Activity of kind "revert" with the operation ID and the object@version
   ✔  And the record should name the journal entry IDs of that commit
  Scenario: Entities and relations describe the change
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file whose metadata record has one earlier version
   ✔  When I edit its metadata
   ✔  Then the new metadata state should be a prov:Entity with object ID, version, logical path and SHA-256
   ✔  And it should be prov:wasGeneratedBy the activity and prov:wasDerivedFrom the previous metadata state
  Scenario: A purge invalidates the removed state
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ?  Given a deleted file
      Step skipped: tests/features/prov_o_history_F-067.feature:50:5
  Scenario: The owner is the installation agent, not a person
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ?  When I import a file
      Step skipped: tests/features/prov_o_history_F-067.feature:55:5
  Scenario: The installation UUID is the same across restarts
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ?  Given I imported a file
      Step skipped: tests/features/prov_o_history_F-067.feature:62:5
  Scenario: Journal and PROV-O join without timestamps
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  When I rename a file
   ✔  Then the journal should hold one "ocfl.commit" entry with the operation ID and object reference of the file object and the new version
   ✔  And the PROV-O activity should list that entry's ID
  Scenario: A no-op save produces no version and no record
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a file whose metadata is unchanged
   ✔  When I save its metadata without changes
   ✔  Then no OCFL version should be committed
   ✔  And no PROV-O record should be written
  Scenario: A refused attempt stays in the journal only
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  When a caller without the per-launch token tries to rename a file
   ✔  Then the journal should record the refusal
   ✔  And no PROV-O record should be written
  Scenario Outline: Exports, scrubbed outputs and publications are recorded in the library's history object
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ?  When I export a folder as a ZIP download
      Step skipped: tests/features/prov_o_history_F-067.feature:84:5
  Scenario Outline: Exports, scrubbed outputs and publications are recorded in the library's history object
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ?  When I export a file with metadata scrubbing
      Step skipped: tests/features/prov_o_history_F-067.feature:84:5
  Scenario Outline: Exports, scrubbed outputs and publications are recorded in the library's history object
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ?  When I publish a public file to my AT Protocol account
      Step skipped: tests/features/prov_o_history_F-067.feature:84:5
  Scenario: A single-file download is journaled only
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  When I download one file
   ✔  Then the journal should record a "file.download" entry with the file ID and version
   ✔  And no PROV-O record should be written
   ✔  And no OCFL version should be committed
  Scenario: A repair is followed by a provenance-only version
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ?  Given an object whose head has no version inventory
      Step skipped: tests/features/prov_o_history_F-067.feature:101:5
  Scenario: A rebased commit regenerates its record
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ?  Given another writer committed to the same object after my commit was staged
      Step skipped: tests/features/prov_o_history_F-067.feature:106:5
  Scenario: Versions committed before the history existed are reconstructed once
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ?  Given an object with three versions and no PROV-O records
      Step skipped: tests/features/prov_o_history_F-067.feature:111:5
  Scenario: A crash between publication and the journal write leaves no dangling link
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ?  Given a commit whose OCFL version is published
      Step skipped: tests/features/prov_o_history_F-067.feature:121:5
  Scenario: The history survives an ordinary copy
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ✔  Given a library with recorded history
   ✔  When I close FileSquid and copy the storage root with ordinary file tools
   ✔  Then a JSON parser alone should read every activity, entity and agent from the copy
  Scenario: Shared PROV-O is redacted
   ✔> Given a desktop library with no c2pa module
   ✔> And the installation UUID of this FileSquid installation is known
   ?  Given a file with recorded history that includes its original file name
      Step skipped: tests/features/prov_o_history_F-067.feature:132:5
424 scenarios (338 passed, 86 skipped)
1709 steps (1623 passed, 86 skipped)
```

## Gaps

- **Wired: desktop catalog provenance.** The desktop catalog's snapshot commits already record their PROV-O history: the desktop host builds `SqliteCatalog::with_provenance` (Task DE-0.10), which uses `CommitRecorder` for every snapshot version it publishes.
- **Wired: import and the file routes.** The file and metadata routes since API-F2.8, and the import commit since the second part above.
- **Wired: the history source, the repair follow-up and the backfill** ([host wiring walkthrough](../../../desktop/resources/tasks/Task-FS-2.8-host-wiring-Walkthrough.md)). The desktop host installs `OcflRootsHistory` over the storage roots of every library its catalog serves, so the history routes answer from a real desktop library. It builds `LocalLibraryService::with_provenance` through `local_library_service`, injecting the installation UUID (DE-S-012) and the journal store. It runs `Backfill` over each library at every launch, as a registered, journaled job that app exit cancels. Still open: the owner-chosen roots that DE-1.1 opens are not listed by the history source yet, and DE-1.1 supplies the persistent repair ledger when it wires the repair route.
- **Backfill route.** `POST /api/v1/admin/history/backfill` was added with DE-1.1 (see the plan's API-F2.9 row).
- **Backfill heartbeat granularity.** The run sends a heartbeat after each object and between staging and committing it. Staging the records of one object is a single `filesquid-prov` call (`stage_reconstructed`), so there is no heartbeat for each staged record. The per-run cap (`DEFAULT_MAX_VERSIONS`, 1000) keeps that call short.
- **The index is rebuilt on every request.** `OcflHistory` reads every record of the storage root each time (bounded). A cached index that is invalidated by commit events is left for later.
- **Read permission is per host, not per file.** The routes authorise `history.read` on the host. Per-file read permission arrives with server accounts (API-F1.9).
- **Object IDs survive redaction.** They are opaque UUIDs. Removing them would break joins inside a shared export.
- **Sealing and journal rebuild are out of scope.** The sealing service and the journal rebuild of API-S-032 were left out by owner decision.
