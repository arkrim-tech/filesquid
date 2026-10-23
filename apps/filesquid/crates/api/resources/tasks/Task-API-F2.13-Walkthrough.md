# Task API-F2.13 Walkthrough: Command envelopes and operation-ID idempotency on the file write routes

Branch `feat/api-f2.13-command-envelopes`, module `filesquid-api` ([plan row](../plan.md)) with the default UI's client and file-browser callers (`apps/filesquid/ui`), part of global Task FS-2.5. Contract 0.25.0 (main was 0.24.0).

## SRS mapping

| SRS ID and title | Covered by |
|---|---|
| [API-S-002](../srs.md) Versioned API contract owned by this crate; every mutation route, the file write routes included, takes a `CommandEnvelope` and answers a `CommandOutcome`; a conflict is HTTP 409 with a `conflict` outcome; a repeated operation ID answers its first outcome with no second change | `integration_file_commands` (11 tests), `file_command_envelopes_API-S-002.feature` (7 scenarios), `unit_contract` (drift, samples, documented answers), regenerated `ui/src/api/schema.d.ts` with the UI drift test |
| [API-S-019](../srs.md) Library domain commands and queries: a stale expected revision is a `conflict` with no change; the stored receipt is answered before the revision is checked; a command that changes nothing stores no receipt | `a_stale_expected_revision_is_a_conflict_outcome_with_no_version`, `a_repeated_operation_is_answered_before_its_revision_is_checked`, `a_refused_command_stores_no_receipt_so_its_retry_is_evaluated_again`, `creating_a_folder_answers_committed_and_stores_no_receipt` |
| [UI-S-012](../../../../ui/resources/srs.md) The UI reaches FileSquid only through `FileSquidClient`; every command and upload carries a `CommandEnvelope`; a 2xx answer resolves to a `CommandOutcome` for the same operation id; HTTP 409 resolves to `conflict` | `ui/tests/unit/files-source.test.ts`, `client-io.test.ts`, `commands.test.ts`, `files-memory-source.test.ts`, `ui/tests/ui/file-browser-writes.test.ts`, Playwright `tests/e2e/preview/file-browser.spec.ts` against the production build, `host_adapter_UI-S-012.feature` |
| [CORE-S-005](../../../core/resources/srs.md) Versioned command envelope with operation/idempotency key and expected revision | `an_envelope_of_another_schema_or_scope_is_refused` |
| [API-S-008](../srs.md) One write path: each write publishes exactly one OCFL version or none | every route test above counts versions and journal successes |
| [API-S-017](../srs.md) Every refusal journaled once | `every_write_route_refuses_a_body_without_an_envelope`, the refusal scenario |

## What changed

- **Request.** The JSON write routes (`folders`, `renames`, `moves`, `copies`, `removals`, `restorations`, `labels`) take `{"envelope": CommandEnvelope, "input": <the former body>}`, the body `FileSquidClient.command()` sends. `PUT /files/upload` takes `multipart/form-data` with exactly the parts `envelope` (JSON), `input` (JSON: the former query fields) and `file` (at most 64 MiB); the query string is gone. The desktop scheme caps answers, not requests, at 16 MiB (DE-S-005), so no stage-then-commit pair is needed. axum's `multipart` feature (multer 3.1) is enabled for it.
- **Envelope checks.** Schema version 1, an operation ID the journal keeps opaque (1 to 128 letters, digits, `.`, `_`, `-`, `:`), and scope `library:<input.library_id>`; anything else is `400 invalid_request`.
- **Expected revision.** The head version of the object the command changes, compared under the library's write lock: `object_id`'s head for folders, moves, removals, restorations and labels; the file object's head for renames and an upload with `file_id`; `0` for an upload of a new file; the source file object's head for a copy (the copy's object is new). A stale one is a `conflict` with no change.
- **Answers.** `200` with the `CommandOutcome` fields (`operation_id`, `kind` `committed`, `error` null) and `result`, the view each route answered before. A conflict (stale revision, a write that would change nothing, a concurrent change, an operation ID still in flight) is HTTP 409 with `FileCommandConflict`: the `ApiError` fields beside a `conflict` outcome of the same operation ID. Other refusals keep their typed `ApiError`.
- **Receipts** (`files::CommandReceipts` in `ApiState`). A command claims its operation ID first: a stored receipt of the same route, library and input (for an upload, its facts and the SHA-256 of its bytes) answers the first answer again before anything else, with no second version and no second journal entry; the same ID with another route, library or input is `invalid_request`; an ID in flight is a `conflict`. Only a command that published a version stores its answer, before its journal entries are written, so a request cancelled meanwhile still leaves its retry a replay; creating a folder (which writes nothing) and every refusal store none, so their retry is evaluated again. The claim is an RAII guard released on every path. At most 10,000 receipts per host process, oldest forgotten first.
- **Journal.** Event names, targets and redaction are unchanged; each route entry now carries the envelope's operation ID, and so does each refusal of a request whose envelope was read and checked. A refusal of a missing or malformed envelope has no operation ID to carry and is journaled without one. Copy and rename entries now also carry their `<object-id>@vN` reference. Restore and label commits with provenance installed keep the operation IDs their recorder allocates until API-F2.12 threads the envelope's into the commit.
- **Contract 0.25.0.** New schemas `CreateFolderCommand`, `MoveCommand`, `RemoveCommand`, `RestoreCommand`, `LabelCommand`, `RenameCommand`, `CopyCommand`, `UploadInput`, `FileCommitOutcome`, `UploadCommitOutcome`, `FileCopyOutcome`, `MetadataCommitOutcome`, `FolderAnswerOutcome` and `FileCommandConflict`, and the `CommandConflict` response; the contract test accepts a 409 that carries every `ApiError` field beside its outcome fields.
- **UI.** `FileSquidClient.command()` and `upload()` keep the route's `result`; `upload()` takes `method` (`PUT` by default) and an `input` part typed as the contract's `UploadInput`. The file browser sends each uploaded file under one operation id and its Retry resends a failed file under the same id, so an upload whose answer was lost is answered again rather than made twice; moving to another object clears the remembered head until its listing loads. `putBytes()` and `uploadFile()` are removed. `ClientFileSource` sends every write as a command with a new operation id and `options.revision`; `WritesHost.revision()` gives the file browser's listed head; Undo expects the head the removal published. A `conflict` rejects as `ApiError` 409 so the views word it as before. `MemoryFileSource` and the Playwright fake apply the same rules.

TDD: `integration_file_commands` was written first and failed (10 of 11) before the routes changed; the other route tests then moved to envelopes.

Known limit: receipts live in memory, so after a host restart a replayed object write meets the moved head and is a `conflict`, but a replayed new upload or copy would run again. API-F2.12 records the operation ID in each version's message, from which a durable receipt store can be rebuilt.

## Test output

`cargo test -p filesquid-api --locked --test integration_file_commands --test unit_contract`:

```text
     Running tests/integration/file_commands.rs (<target>/debug/deps/integration_file_commands-e03b02b407220fc5)

running 11 tests
test a_repeated_operation_is_answered_before_its_revision_is_checked ... ok
test a_refused_command_stores_no_receipt_so_its_retry_is_evaluated_again ... ok
test a_new_upload_names_revision_zero_and_a_copy_its_source_head ... ok
test an_envelope_of_another_schema_or_scope_is_refused ... ok
test an_operation_id_of_another_command_is_refused ... ok
test an_upload_without_its_file_or_input_part_is_refused ... ok
test creating_a_folder_answers_committed_and_stores_no_receipt ... ok
test a_repeated_operation_answers_its_first_outcome_with_no_second_version ... ok
test a_stale_expected_revision_is_a_conflict_outcome_with_no_version ... ok
test every_write_route_refuses_a_body_without_an_envelope ... ok
test every_write_route_answers_a_committed_outcome_for_the_same_operation ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s

     Running tests/unit/contract.rs (<target>/debug/deps/unit_contract-27eeb267071a4e20)

running 28 tests
test error_code_all_lists_every_variant ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test event_topics_in_the_contract_match_the_rust_topics ... ok
test errors_carry_code_and_retryability ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
test every_component_schema_has_a_rust_sample ... ok
test every_documented_operation_has_a_unique_operation_id ... ok
test every_event_topic_documents_its_payload_schema ... ok
test every_error_response_carries_the_api_error_schema ... ok
test every_limit_parameter_is_the_shared_page_size ... ok
test every_operation_documents_both_denials ... ok
test every_operation_names_the_action_the_router_authorises ... ok
test health_schema_matches_the_core_payload_and_requires_every_field ... ok
test every_operation_requires_the_bearer_scheme_and_none_makes_it_optional ... ok
test host_routes_are_the_event_channel_only ... ok
test query_envelope_and_page_size_parameter_are_bounded_like_core ... ok
test routed_and_documented_paths_agree_both_ways ... ok
test router_fallbacks_are_documented_as_wire_conventions ... ok
test router_serves_its_own_contract ... ok
test the_checker_accepts_only_truly_unconstrained_objects ... ok
test the_contract_uses_only_checked_schema_keywords ... ok
test the_only_public_operation_is_the_liveness_probe ... ok
{"error":"journal writer stopped","level":"ERROR","message":"journal write failed"}
{"actor":"system","host":"","journal_write_failed":true,"level":"info","operation":"process.cancelled","outcome":"cancelled","process_id":"import-1","target_ids":[],"time":"2026-10-23T17:45:29.610Z"}
test events_from_every_publisher_match_their_documented_payloads ... ok
test every_documented_operation_answers_a_documented_status_and_body ... ok
test router_fallbacks_answer_as_documented ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.99s
```

BDD, `cargo test -p filesquid-api --locked --test features` (the new feature, then the whole run):

```text
Feature: Command envelopes on the file write routes
  Scenario: A move carries an envelope and answers a committed outcome for the same operation
   ✔> Given the owner's library holds an object at its first version
   ✔  When the owner moves "a.txt" to "c.txt" as operation "op-move-1" expecting "v1"
   ✔  Then the answer should be a committed outcome of operation "op-move-1"
   ✔  And its result should name version "v2"
   ✔  And the journal should record one "files.move" success under operation "op-move-1"
  Scenario: An upload is a multipart command with its envelope, input and file
   ✔> Given the owner's library holds an object at its first version
   ✔  When the owner uploads "notes.txt" as operation "op-upload-1" expecting "0"
   ✔  Then the answer should be a committed outcome of operation "op-upload-1"
   ✔  And its result should name a new file at version "v1"
  Scenario: A repeated operation answers its first outcome with no second version
   ✔> Given the owner's library holds an object at its first version
   ✔  Given the owner moved "a.txt" to "c.txt" as operation "op-move-2" expecting "v1"
   ✔  When the owner moves "a.txt" to "c.txt" as operation "op-move-2" expecting "v1"
   ✔  Then the answer should be the first answer of operation "op-move-2"
   ✔  And the object's head should be "v2"
   ✔  And the journal should record one "files.move" success under operation "op-move-2"
  Scenario: A stale expected revision is a conflict outcome and changes nothing
   ✔> Given the owner's library holds an object at its first version
   ✔  When the owner removes "b.txt" as operation "op-remove-1" expecting "v7"
   ✔  Then the answer should be HTTP 409 with a conflict outcome of operation "op-remove-1"
   ✔  And the object's head should be "v1"
  Scenario: Creating a folder stores no receipt, so its retry is evaluated again
   ✔> Given the owner's library holds an object at its first version
   ✔  Given the owner created the folder "albums" as operation "op-folder-1" expecting "v1"
   ✔  And a file "albums/x.txt" is then committed beneath that folder
   ✔  When the owner creates the folder "albums" as operation "op-folder-1" expecting "v2"
   ✔  Then the answer should be a committed outcome of operation "op-folder-1"
   ✔  And its result should say the folder is no longer pending
  Scenario: A write without an envelope is refused and journaled once
   ✔> Given the owner's library holds an object at its first version
   ✔  When the owner sends a removal of "b.txt" without an envelope
   ✔  Then the command should be refused with "invalid_request"
   ✔  And the object's head should be "v1"
   ✔  And the journal should record one "files.remove" refusal
  Scenario: An operation ID of another command is refused
   ✔> Given the owner's library holds an object at its first version
   ✔  Given the owner moved "a.txt" to "c.txt" as operation "op-move-3" expecting "v1"
   ✔  When the owner removes "b.txt" as operation "op-move-3" expecting "v2"
   ✔  Then the command should be refused with "invalid_request"
   ✔  And the object's head should be "v2"
```

```text
[Summary]
27 features
374 scenarios (272 passed, 102 skipped)
1319 steps (1217 passed, 102 skipped)
```

UI, `npx vitest run` in `apps/filesquid/ui` (the changed files, then the totals):

```text
 ✓ tests/ui/file-browser-writes.test.ts (45 tests) 2311ms
 ✓ tests/unit/files-source.test.ts (29 tests) 77ms
 ✓ tests/unit/files-memory-source.test.ts (35 tests) 76ms
 ✓ tests/unit/client-io.test.ts (23 tests) 73ms
 ✓ tests/unit/commands.test.ts (46 tests) 49ms
 Test Files  56 passed (56)
      Tests  1240 passed | 1 skipped (1241)
```

Playwright against the production build, `tests/e2e/preview/file-browser.spec.ts` (the preinstalled Chromium, through a scratch config that sets its `executablePath`):

```text
Running 20 tests using 1 worker
  ✓   1 [chromium] › tests/e2e/preview/file-browser.spec.ts:96:1 › the browser is a defined element and lists display names in the list layout from the label table (877ms)
  ✓   2 [chromium] › tests/e2e/preview/file-browser.spec.ts:140:1 › a file row shows size, type hint, modified date and version (552ms)
  ✓   3 [chromium] › tests/e2e/preview/file-browser.spec.ts:152:1 › computed token values are exact in the browser in light and dark (783ms)
  ✓   4 [chromium] › tests/e2e/preview/file-browser.spec.ts:175:1 › opening a folder updates the breadcrumb and the address, and Back returns (870ms)
  ✓   5 [chromium] › tests/e2e/preview/file-browser.spec.ts:196:1 › an image previews through a content handle that is used only as the media src (785ms)
  ✓   6 [chromium] › tests/e2e/preview/file-browser.spec.ts:231:1 › a handle that fails is replaced once; a second failure shows the error and mints no third (630ms)
  ✓   7 [chromium] › tests/e2e/preview/file-browser.spec.ts:247:1 › a video resumes at the same position and paused state after one new handle (902ms)
  ✓   8 [chromium] › tests/e2e/preview/file-browser.spec.ts:283:1 › audio plays natively and is named by the display name (742ms)
  ✓   9 [chromium] › tests/e2e/preview/file-browser.spec.ts:296:1 › text previews through one ranged read of the first 64 KiB and says it is cut (655ms)
  ✓  10 [chromium] › tests/e2e/preview/file-browser.spec.ts:312:1 › a format it cannot show says so and shows no viewer (672ms)
  ✓  11 [chromium] › tests/e2e/preview/file-browser.spec.ts:325:1 › the viewer steps through files, closes with Escape and returns focus to the row (726ms)
  ✓  12 [chromium] › tests/e2e/preview/file-browser.spec.ts:338:1 › an upload is one PUT of the client's facts that creates the file's own object (710ms)
  ✓  13 [chromium] › tests/e2e/preview/file-browser.spec.ts:387:1 › a rename posts to the renames route, never to moves, and shows the new display name (801ms)
  ✓  14 [chromium] › tests/e2e/preview/file-browser.spec.ts:416:1 › a refused rename shows the label for its reason and keeps the display name (754ms)
  ✓  15 [chromium] › tests/e2e/preview/file-browser.spec.ts:435:1 › writes are single calls: new folder, move, remove and Undo (1.2s)
  ✓  16 [chromium] › tests/e2e/preview/file-browser.spec.ts:482:1 › a refused write shows the label for its reason and changes nothing (693ms)
  ✓  17 [chromium] › tests/e2e/preview/file-browser.spec.ts:497:1 › a long folder is virtualised, pages in on demand and stays keyboard reachable (975ms)
  ✓  18 [chromium] › tests/e2e/preview/file-browser.spec.ts:537:1 › the layout does not scroll sideways at 320 px, at 200 % text size, or both, in list and grid (740ms)
  ✓  19 [chromium] › tests/e2e/preview/file-browser.spec.ts:554:1 › motion stops under reduced motion and focus is the 2px primary ring (727ms)
  ✓  20 [chromium] › tests/e2e/preview/file-browser.spec.ts:579:1 › a failed listing shows the error state with Try again, and retrying loads it (657ms)
  20 passed (27.5s)
```

The head-commit hook run (`FULL=1 bash scripts/pre-commit.sh`, `Test scope: full`) is recorded in the pull request.

## Mocked Environment Variables

None. The tests use real filesystem OCFL roots under the system temporary directory (`TMPDIR`) and remove them.
