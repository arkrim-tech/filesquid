# Task API-F2.19 Walkthrough: Command envelopes on the library routes

Branch `feat/api-f2.19-library-envelopes`, module `filesquid-api` ([plan row](../plan.md)), with the default UI's library callers (`apps/filesquid/ui`, the UI-1.1 pages) and the desktop host's route tests moved to the envelope in the same change. Global Task FS-2.5. Owner decision 1 of 2026-10-23 (adopted by "provide and implement recommendations for owner's decisions 1-6"). Contract **0.29.0** (main was 0.28.0).

## SRS mapping

| SRS ID and title | Covered by |
|---|---|
| [API-S-002](../srs.md) Versioned API contract owned by this crate; every mutation route takes a `CommandEnvelope` and answers a `CommandOutcome`; a conflict is HTTP 409 with a `conflict` outcome; a repeated operation ID of a command that changed something answers its first outcome with no second change; the job controls, the location-choices route and the repair route are the only mutation routes without an envelope | `integration_library_commands` (10 tests), `library_command_envelopes_API-S-002.feature` (7 scenarios), `unit_contract` |
| [API-S-019](../srs.md) Library domain commands: create-library sends expected revision `0`; a command whose expected revision is not the library's current revision is `conflict` with no change; the stored receipt is answered before the revision is checked; a command that changes nothing stores no receipt | `a_stale_expected_revision_is_a_conflict_outcome_with_no_change`, `a_repeated_operation_id_answers_its_first_answer_with_no_second_change`, `a_repeated_create_answers_the_same_library_once_written`, `a_repeated_open_and_a_refusal_store_no_receipt` |
| [API-S-025](../srs.md) Local library and repair routes: the rebuild runs as a durable job answered `pending`; a request for a library whose job is unfinished answers that job and submits none; every refusal journaled once; opaque IDs only | `a_rebuild_with_an_unfinished_job_answers_that_job_under_a_new_operation_id`, `every_library_mutation_route_refuses_a_body_without_an_envelope`, `integration_library_routes` (30 tests, now on envelopes), the API-S-025 feature |
| [API-S-034](../srs.md) Library location choices: not a library command, takes no envelope; [API-S-023](../srs.md) the repair route keeps its consent body | `the_location_choices_and_repair_routes_keep_their_bodies`, scenario "The repair route keeps its consent body" |
| [CORE-S-005](../../../core/resources/srs.md) Command envelope with schema version, operation key, expected revision and scope | `an_envelope_of_another_schema_or_scope_is_refused`, `envelope::tests` (4 unit tests) |
| [UI-S-012](../../../../ui/resources/srs.md) Every command carries a `CommandEnvelope`; a 2xx answer resolves to a `CommandOutcome` for the same operation id; HTTP 409 resolves to `conflict`; only `committed` is completion evidence | `ui/tests/unit/library-client.test.ts` (21 tests), `ui/tests/ui/library-start.test.ts`, `library-detail.test.ts`, `library.spec.ts` (production build) |
| [API-S-038](../srs.md) Library facts and rename | The rename route lands with API-F2.18 (PR #139); its envelope is added when that PR is on main (see below) |

## What changed

- **Request.** `POST /api/v1/libraries` (create), `/libraries/openings` (open), `/libraries/snapshot-copies` (copy) and `/libraries/rebuilds` (rebuild) take `{"envelope": CommandEnvelope, "input": <the former body>}`, the body `FileSquidClient.command()` sends. Schema version 1, an opaque operation ID (1 to 128 letters, digits, `.`, `_`, `-` or `:`), and the scope the route derives from its input: `location:<location_id>` for create and open (they bind a folder; the library ID may not exist yet), `library:<library_id>` for copy and rebuild. Anything else is `400 invalid_request`, journaled once.
- **Revision of a bound library.** The library routes had no revision to compare. A library this host binds (create, or a fresh open) is at revision `1`, answered as the new `LibraryView.revision`; a later command that changes the bound library (the rename of API-F2.18) advances it. Create expects `0`; open expects `0` for a library not open here, or the current revision of a library already open at that location, which is answered again and changes nothing; copy and rebuild expect the current revision. The comparison runs in `LibraryCommands` under the same lock that binds, so it is atomic with the command. A stale revision is `conflict`.
- **Answers.** `200` with the `CommandOutcome` fields (`operation_id`, `kind`, `error` null) and `result`, what each route answered before: `committed` with the library (create, open) or the copy report; `pending` with the job for a rebuild (`RebuildJobAnswer`, whose new job now carries the envelope's operation ID). Every 409 refusal (a stale revision, `root_not_empty`, `location_in_use`, an operation ID still in flight, ...) is HTTP 409 with `LibraryCommandConflict`: the `ApiError` fields beside a `conflict` outcome of the same operation ID. Other refusals keep the typed `ApiError`.
- **Receipts.** The in-memory `CommandReceipts` of API-F2.13, reached as `ApiState::command_receipts()`: one store per host process, so an operation ID is one idempotency key across the file write routes and the library routes. A create, a fresh open, a copy and a rebuild that submitted its job store their answer; a repeated operation ID answers it again before the revision is checked, with no second change. An open of an open library, a rebuild that answered an unfinished job and every refusal store none, so the ID is free for another command and a refused command's retry is evaluated again.
- **Helper.** The new crate module `envelope` holds the generic pieces (the `Command<T>` body, `check` against a route-given scope, `claim` of the operation ID, the committed, pending and conflict answers, the `Settled` claim outcomes), so the preferences and metadata routes can take the envelope in a small follow-up once the owner decides, and the file routes can move onto it. The file routes are not changed in this task (the api lane works in `files/` at the same time); their helpers in `files/command.rs` remain until that move.
- **Journal.** Event names, targets and redaction are unchanged. A refusal the route makes itself (an unreadable body, a bad envelope, a reused or in-flight operation ID, a job the store refused) carries the envelope's operation ID when one was read. The operations' own entries (`library.root.create`, `library.root.open`, `library.snapshot.copy`, `library.rebuild`) are journaled as before, without the operation ID: threading it through `LibraryOperations` and the service is left to the follow-up that moves the file routes onto the shared helper.
- **Contract 0.29.0.** New schemas `LibraryLocationCommand`, `SnapshotCopyCommand`, `RebuildLibraryCommand`, `LibraryViewOutcome`, `SnapshotCopyOutcome`, `RebuildJobOutcome` and `LibraryCommandConflict`, the `LibraryCommandConflict` response, `LibraryView.revision`, the four operations' descriptions; `ui/src/api/schema.d.ts` and `contract-version.ts` regenerated.
- **UI.** `ClientLibrarySource` sends the four commands through `FileSquidClient.command()` (`LibraryClient` now needs `command()` beside `request()`), takes the outcome's `result`, and raises a `conflict` outcome as the `ApiError` 409 `conflict` the pages already word (`libraryConflictCreate`, `libraryConflictOpen`, `libraryConflictCopy`). `LibrarySource.copy()` and `rebuild()` take the expected revision; `LibraryRegistry.revisionOf()` answers the revision the API gave when the page created or opened the library, else the initial revision `1` (`INITIAL_LIBRARY_REVISION`; UI-1.3's facts will read the current one). `rebuild()` resolves to the pending job (`parseRebuildJob`), not a report: the library page keeps "Working." and reads the repair candidates again, the recover flow goes on to the library page; the job's progress and report are UI-1.3's. The fakes, the Playwright fake of the library routes and the README follow.
- **Desktop.** No host code changes beyond the `LibraryOperations` signatures; the integration tests, the cucumber steps and the WebdriverIO spec send the envelope through a `command()` helper.
- **Docs.** API-S-025 states the envelope rules of the library routes; the plan row and the root checklist are ticked.

Outside this task, pending the owner's decision: the preferences route and the metadata set and remove routes keep their plain bodies. The rename route of API-F2.18 (PR #139) takes the envelope as soon as that PR is on main; see the end of this walkthrough.

TDD: `integration_library_commands` was written first and failed (10 of 10) before the routes changed; the route tests, the feature steps, the desktop tests and the UI tests then moved to envelopes.

## Test output

`cargo test -p filesquid-api --locked --test integration_library_commands` before the change (the routes still took plain bodies):

```text
failures:
    a_rebuild_with_an_unfinished_job_answers_that_job_under_a_new_operation_id
    a_repeated_create_answers_the_same_library_once_written
    a_repeated_open_and_a_refusal_store_no_receipt
    a_repeated_operation_id_answers_its_first_answer_with_no_second_change
    a_stale_expected_revision_is_a_conflict_outcome_with_no_change
    an_envelope_of_another_schema_or_scope_is_refused
    an_operation_id_of_another_command_is_refused
    create_open_copy_and_rebuild_answer_an_outcome_for_the_same_operation
    every_library_mutation_route_refuses_a_body_without_an_envelope
    the_location_choices_and_repair_routes_keep_their_bodies

test result: FAILED. 0 passed; 10 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s

error: test failed, to rerun pass `-p filesquid-api --test integration_library_commands`
```

`cargo test -p filesquid-api --locked --lib --test integration_library_commands --test integration_library_routes` after the change:

```text
test envelope::tests::a_checked_envelope_keeps_its_operation_id_and_expected_revision ... ok
test envelope::tests::a_replay_refuses_nothing_and_the_other_settlements_name_their_code ... ok
test envelope::tests::another_schema_scope_or_a_non_opaque_operation_id_is_refused ... ok
test envelope::tests::committed_and_pending_answers_carry_the_outcome_fields_and_the_result ... ok
test local_library::commands::tests::an_unreachable_snapshot_is_retryable_and_an_unreadable_one_is_refused ... ok
test result: ok. 77 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.60s
test a_repeated_create_answers_the_same_library_once_written ... ok
test a_rebuild_with_an_unfinished_job_answers_that_job_under_a_new_operation_id ... ok
test a_repeated_open_and_a_refusal_store_no_receipt ... ok
test a_repeated_operation_id_answers_its_first_answer_with_no_second_change ... ok
test a_stale_expected_revision_is_a_conflict_outcome_with_no_change ... ok
test an_envelope_of_another_schema_or_scope_is_refused ... ok
test an_operation_id_of_another_command_is_refused ... ok
test create_open_copy_and_rebuild_answer_an_outcome_for_the_same_operation ... ok
test every_library_mutation_route_refuses_a_body_without_an_envelope ... ok
test the_location_choices_and_repair_routes_keep_their_bodies ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
test a_copy_into_an_open_library_or_of_an_unknown_library_is_refused ... ok
test a_rebuild_of_a_library_that_is_not_open_submits_no_job ... ok
test a_rebuild_on_a_host_without_a_job_service_is_unavailable_and_journaled_once ... ok
test a_consented_repair_by_the_desktop_owner_makes_a_sha256_object_committable ... ok
test a_repair_of_a_library_that_is_not_open_is_not_found_once ... ok
test a_head_with_both_files_is_refused_with_nothing_to_repair_and_nothing_written ... ok
test a_repeated_rebuild_answers_the_unfinished_job_and_cancel_changes_no_file ... ok
test a_repair_with_a_missing_or_wrong_credential_is_denied_once ... ok
test a_repair_without_consent_is_refused_once_and_writes_nothing ... ok
test an_open_library_folder_under_a_second_location_id_is_refused_once ... ok
test create_keeps_a_library_id_the_caller_names ... ok
test a_second_folder_of_an_open_library_is_refused_once ... ok
test create_writes_only_the_declaration_and_answers_an_opaque_library_id ... ok
test every_server_caller_including_the_administrator_is_refused_the_repair ... ok
test opening_a_folder_that_is_not_a_library_is_refused_once ... ok
test open_answers_the_library_and_its_objects ... ok
test opening_a_folder_answers_the_library_its_snapshot_names ... ok
test opening_a_folder_under_another_library_id_than_its_snapshot_s_is_refused_once ... ok
test the_candidate_query_for_a_library_that_is_not_open_is_not_found ... ok
test opening_an_open_location_answers_its_library_again ... ok
test the_routes_answer_unavailable_once_until_the_host_installs_its_commands ... ok
test the_rebuild_answers_pending_with_a_library_rebuild_job_whose_result_is_the_report ... ok
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
```

`cargo test -p filesquid-api --locked --test unit_contract --test unit_rebuild_job --test unit_receipts --test features` and `cargo test -p filesquid-desktop --locked --test integration_library_routes`:

```text
CARGO_OUTPUT_PLACEHOLDER
```

`npx vitest run` (in `apps/filesquid/ui`):

```text
 ✓ tests/ui/library-start.test.ts (22 tests) 491ms
 ✓ tests/ui/library-detail.test.ts (21 tests) 571ms
 ✓ tests/ui/library-repair.test.ts (22 tests) 746ms
 ✓ tests/ui/library-route.test.ts (6 tests) 186ms
 ✓ tests/unit/library-client.test.ts (21 tests) 123ms
 ✓ tests/unit/contract.test.ts (2 tests) 386ms
   ✓ src/api/schema.d.ts matches crates/api/openapi.json  380ms
 Test Files  64 passed (64)
      Tests  1425 passed | 1 skipped (1426)
```

`npx playwright test -c playwright.preview.config.ts tests/e2e/preview/library.spec.ts` against the production build (run with a throwaway config that points Playwright at the preinstalled Chromium, as the environment requires; the config file was deleted afterwards):

```text
Running 12 tests using 1 worker
  ✓   1 [chromium] › tests/e2e/preview/library.spec.ts:58:1 › the start page offers create, open and recover in the active vocabulary (1.9s)
  ✓   2 [chromium] › tests/e2e/preview/library.spec.ts:91:1 › (1) create asks for a folder, passes the location ID on and shows the library, with no path (1.2s)
  ✓   3 [chromium] › tests/e2e/preview/library.spec.ts:117:1 › (1) open passes the location ID to the open route (1.1s)
  ✓   4 [chromium] › tests/e2e/preview/library.spec.ts:131:1 › (1) recover opens the copy, starts its rebuild job expecting the opened revision and repairs nothing (1.0s)
  ✓   5 [chromium] › tests/e2e/preview/library.spec.ts:161:1 › (1) copy asks for a destination and passes it as destination_location_id (1.0s)
  ✓   6 [chromium] › tests/e2e/preview/library.spec.ts:187:1 › (2) a cancelled folder choice sends no library request and says nothing changed (1.0s)
  ✓   7 [chromium] › tests/e2e/preview/library.spec.ts:201:1 › (3) 503 unavailable is the unavailable state with its reason and a Retry, and the choices stay (1.1s)
  ✓   8 [chromium] › tests/e2e/preview/library.spec.ts:228:1 › (4) confirm sends the consent for exactly the named objects and shows each result (950ms)
  ✓   9 [chromium] › tests/e2e/preview/library.spec.ts:265:1 › (4) cancel and Escape send nothing and return focus to the opener (1.1s)
  ✓  10 [chromium] › tests/e2e/preview/library.spec.ts:288:1 › (4) a refused repair says nothing was changed and shows no result (815ms)
  ✓  11 [chromium] › tests/e2e/preview/library.spec.ts:303:1 › (5) the Monitor's W010 note links to the library's repair offer (887ms)
  ✓  12 [chromium] › tests/e2e/preview/library.spec.ts:356:1 › colours come from tokens in light and dark (1.2s)
  12 passed (29.8s)
```

The head-commit hook run (`FULL=1 bash scripts/pre-commit.sh` with a private Cargo target, `Test scope: full`) is recorded in the pull request.

## Sync Execution Report

- [`crates/api/resources/srs.md`](../srs.md): API-S-025 gains the **Command envelopes** rules of the library routes (scope, revision, conflict, receipts, the pending rebuild).
- [`ui/README.md`](../../../../ui/README.md): the library flow's commands, revisions and the pending rebuild job.
- No PRD, product-vision or ADR change: ADR 030 and API-S-002 already require the envelope on every mutation route.

## Mocked Environment Variables

None.
