# Task API-F2.5 Walkthrough: Version listing, labels, restore and history size

Branch `feat/api-f2.5-versions-labels-restore`, module `filesquid-api` ([plan row](../plan.md)), part of global Task FS-2.5. Holistic UI check: the Versions panel of [UI-F2.11](../../../../ui/resources/plan.md) needs each version's label, relative time and size, the current version, restore, download, edit label and the history size, and never a delete control. The routes below serve every one of these.

## SRS mapping

| SRS ID and title | Covered by |
|---|---|
| [API-S-014](../srs.md) Version listing, ranged reads, restore, labels, history size, no deletion | `integration_versions` (19 tests), `version_history_labels_restore_API-S-014.feature` (9 scenarios), `unit_contract` |
| [SRS-F-065](../../../../../../resources/srs.md#srs-f-065-file-version-history-and-retention) File version history and retention | the same tests: one version per restore, earlier versions untouched, commit-time and later labels, history size, no delete, expiry or thinning route |
| [API-S-028](../srs.md) PROV-O record per commit and journal linkage | `a_restore_commits_its_prov_o_record_and_journals_once`, `a_later_label_commits_its_prov_o_record`, the provenance scenario |
| [API-S-002](../srs.md) Versioned contract and generated types | `unit_contract` drift and sample tests, regenerated `ui/src/api/schema.d.ts` |

## What changed

- **Listing.**
  - `GET /files/history` entries carry `size` (bytes, `null` once removed) and the version `label`, beside `created`, `user`, `message` and `digest`.
  - `GET /files/versions` carries `label`.
  - Sizes come from the library's content-size cache, or from one backend `head` per content.
  - `label` and `size` are optional in the schema but always sent, so UI code written before them still type-checks.
- **Ranged reads** of any version are API-F1.3's `GET /files/content?version=`. This task adds a test.
- **Restore.** `POST /files/restorations` publishes exactly one version pointing at the earlier content (`ChangeModel::reinstate_with`). Its `filesquid.operation.v1` message has kind:
  - `revert` when the restore returns every user file of the object to that version's and the head holds no other;
  - `reinstate` otherwise.

  The request takes an optional `label` (the message's `label` member) and no longer takes a free `message`; the UI never sent one.
- **Later labels.** `PUT /files/labels` commits a `label` operation: one version holding `.filesquid/labels.json`, `{"schema":1,"labels":{"v3":"Final draft","v5":null}}`.
  - An entry overrides its version's commit-time label, and `null` clears it.
  - The label version's own message carries no `label` member, so that version is not itself labelled.
  - Caps: at most 1,000 labelled versions; labels 1 to 512 bytes, not blank, no control characters.
  - A label that would not change what the version shows is refused (`409`) with no version.
- **Why `.filesquid/`.** Neither ADR 010, ADR 038 nor ADR 040 defines an application-data location in an object. ADR 023 forbids application keys at object-root reserved paths (`extensions/`, `logs/`), and `provenance/` is the filesquid-prov/1 record area. So the labels map lives in a reserved logical folder that is recovered with the object from an ordinary copy. Uploads, moves, removals, restores and new folders in `.filesquid/` are refused. The design is recorded in [API-S-014](../srs.md).
- **History size.**
  - `GET /files/history-size` answers the versions that changed a file, its distinct contents and their bytes.
  - `GET /files/library-history-size` answers each object's distinct contents and their bytes, within 10,000 objects and 100,000 contents, with `truncated`.
- **Provenance.** Once the host calls `FileService::install_provenance(context, journal store)`, restore and label commits carry a `CommitRecorder`.
  - The version gets its PROV-O record and journal lines.
  - The route journals the recorder's entries once (`ocfl.commit` and `files.restore` or `files.label`, sharing the operation ID), instead of its own.
  - Without provenance, the commit carries the structured message with the actor as `user`.
- **No deletion.** A test checks that the contract offers no `DELETE`, expiry, thinning, prune or purge route.
- **Contract 0.13.0.**
  - `VersionRecord.label`, `FileHistoryEntry.size`, `RestoreRequest.label`.
  - New schemas: `LabelRequest`, `FileHistorySize`, `LibraryHistorySize`.
  - Three new paths.
  - `schema.d.ts` regenerated with `npm run contract:types`.

TDD: `integration_versions` was written first and failed to compile against the missing API: unresolved `LABELS_PATH` and `MAX_LABELS`, and no `install_provenance`.

Left for later rows:
- Searching labels waits for the search routes of F-063.
- Hiding `provenance/` and `.filesquid/` from file listings is routed to API-F2.2.

## Test output

`cargo test -p filesquid-api --locked --test integration_versions --test unit_contract --test unit_auth`:

```text
running 19 tests
test a_label_that_changes_nothing_is_refused_without_a_version ... ok
test a_file_history_carries_size_digest_label_and_version_facts ... ok
test a_deleted_version_has_no_size ... ok
test a_folder_restore_brings_back_earlier_files_and_keeps_later_ones ... ok
test a_restore_commits_its_prov_o_record_and_journals_once ... ok
test a_later_label_commits_its_prov_o_record ... ok
test a_later_label_is_committed_as_application_data_and_shown ... ok
test an_earlier_version_is_served_with_ranged_reads ... ok
test an_object_that_cannot_be_opened_marks_the_library_size_truncated ... ok
test a_later_label_overrides_and_clears_a_commit_time_label ... ok
test history_size_counts_each_kept_content_of_a_file_once ... ok
test labels_are_bounded_and_checked ... ok
test library_history_size_counts_every_kept_content_once ... ok
test restore_of_every_file_of_a_version_is_a_revert ... ok
test the_application_data_folder_is_reserved ... ok
test the_retention_check_catches_a_removal_under_any_method ... ok
test restore_takes_a_label_not_a_free_message ... ok
test restore_reinstates_chosen_files_in_exactly_one_version_with_a_label ... ok
test the_contract_offers_no_version_deletion_expiry_or_thinning ... ok
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
running 35 tests
test a_host_route_cannot_be_made_public_through_the_guard ... ok
test a_present_but_unusable_authorization_header_is_not_a_missing_one ... ok
test a_not_permitted_denial_names_the_principal_and_the_action ... ok
test a_websocket_upgrade_may_carry_the_token_as_a_subprotocol ... ok
test action_names_are_unique_dotted_identifiers ... ok
test a_principal_the_policy_does_not_permit_is_forbidden ... ok
test an_empty_bearer_token_is_presented_and_then_rejected ... ok
test an_error_without_a_reason_omits_the_member ... ok
test an_unusable_header_never_falls_back_to_the_subprotocol ... ok
test a_non_bearer_authorization_header_is_refused_as_invalid_credentials ... ok
test debug_output_never_shows_a_secret ... ok
test denials_map_reasons_to_codes_and_are_not_retryable ... ok
test an_unusable_header_on_a_websocket_upgrade_is_refused_even_with_a_valid_subprotocol ... ok
test an_authoriser_can_be_installed_only_once ... ok
test every_denial_is_journaled_without_the_presented_credential ... ok
test every_route_answers_the_per_launch_token_holder ... ok
test every_routed_path_declares_its_action ... ok
test fallback_refusals_are_journaled_with_the_no_route_target ... ok
test host_routes_are_denied_by_default ... ok
test launch_tokens_are_unique_256_bit_hex ... ok
test only_the_liveness_route_is_public ... ok
test the_bearer_scheme_is_read_case_insensitively ... ok
test the_host_policy_grants_the_administrator_every_action_but_the_repair ... ok
test the_host_policy_grants_the_owner_every_action ... ok
test the_liveness_route_answers_without_a_token_and_says_only_that_the_process_is_up ... ok
test the_liveness_route_still_refuses_an_anonymous_wrong_method ... ok
test the_subprotocol_carrier_is_read_from_any_offered_entry ... ok
test the_subprotocol_token_counts_only_next_to_the_events_subprotocol ... ok
test the_token_authenticator_accepts_only_the_exact_token ... ok
test the_token_issued_entry_has_no_field_that_could_carry_a_secret ... ok
test every_route_denies_a_call_without_the_per_launch_token ... ok
test unknown_paths_and_wrong_methods_authenticate_before_routing ... ok
test a_denial_never_echoes_the_presented_credential ... ok
test without_an_installed_authoriser_every_route_is_denied ... ok
test every_route_denies_a_wrong_token ... ok
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
running 28 tests
test error_code_all_lists_every_variant ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test errors_carry_code_and_retryability ... ok
test event_topics_in_the_contract_match_the_rust_topics ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
test every_component_schema_has_a_rust_sample ... ok
test every_documented_operation_has_a_unique_operation_id ... ok
test every_error_response_carries_the_api_error_schema ... ok
test every_event_topic_documents_its_payload_schema ... ok
test every_limit_parameter_is_the_shared_page_size ... ok
test every_operation_documents_both_denials ... ok
test every_operation_names_the_action_the_router_authorises ... ok
test every_operation_requires_the_bearer_scheme_and_none_makes_it_optional ... ok
test health_schema_matches_the_core_payload_and_requires_every_field ... ok
test host_routes_are_the_event_channel_only ... ok
test query_envelope_and_page_size_parameter_are_bounded_like_core ... ok
test routed_and_documented_paths_agree_both_ways ... ok
test router_fallbacks_are_documented_as_wire_conventions ... ok
test router_serves_its_own_contract ... ok
test the_checker_accepts_only_truly_unconstrained_objects ... ok
test the_contract_uses_only_checked_schema_keywords ... ok
test the_only_public_operation_is_the_liveness_probe ... ok
test every_documented_operation_answers_a_documented_status_and_body ... ok
test events_from_every_publisher_match_their_documented_payloads ... ok
test router_fallbacks_answer_as_documented ... ok
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.70s
```

`cargo test -p filesquid-api --locked --test integration_versions --test features`, the API-S-014 feature:

```text
Feature: Version history, labels and restore through the API
  Scenario: The version listing shows each version's size, digest and label
   ✔> Given the owner's file "a.txt" was saved as "alpha" and then as "bravo!"
   ✔  When the owner lists the versions of "a.txt"
   ✔  Then the listing should show 2 versions with sizes 5 and 6
   ✔  And each version should show its digest, creation time and label
  Scenario: An earlier version is read in ranges
   ✔> Given the owner's file "a.txt" was saved as "alpha" and then as "bravo!"
   ✔  When the owner reads bytes 1 to 2 of "a.txt" as it was in "v1"
   ✔  Then the answer should be the partial content "lp"
  Scenario: Restore makes exactly one new version and keeps every earlier one
   ✔> Given the owner's file "a.txt" was saved as "alpha" and then as "bravo!"
   ✔  When the owner restores "a.txt" from "v1" with the label "Original"
   ✔  Then the object should have 3 versions
   ✔  And "a.txt" should hold "alpha" again
   ✔  And version "v3" should be labelled "Original"
   ✔  And version "v2" should still hold "bravo!"
  Scenario: A label set later is kept with the file
   ✔> Given the owner's file "a.txt" was saved as "alpha" and then as "bravo!"
   ✔  When the owner labels version "v1" as "First draft"
   ✔  Then version "v1" should be labelled "First draft"
   ✔  And the object should keep its labels in its own files
  Scenario: A label can be cleared
   ✔> Given the owner's file "a.txt" was saved as "alpha" and then as "bravo!"
   ✔  Given the owner labelled version "v1" as "First draft"
   ✔  When the owner clears the label of version "v1"
   ✔  Then version "v1" should have no label
  Scenario: A label that changes nothing makes no version
   ✔> Given the owner's file "a.txt" was saved as "alpha" and then as "bravo!"
   ✔  Given the owner labelled version "v1" as "First draft"
   ✔  When the owner labels version "v1" as "First draft"
   ✔  Then the answer should be the version error "conflict"
   ✔  And the object should have 3 versions
  Scenario: History size is reported per file and per library
   ✔> Given the owner's file "a.txt" was saved as "alpha" and then as "bravo!"
   ✔  When the owner asks how much history "a.txt" keeps
   ✔  Then the file's history should keep 2 contents in 11 bytes
   ✔  And the library's history should keep 2 contents in 11 bytes
  Scenario: The API offers no way to delete, expire or thin versions
   ✔> Given the owner's file "a.txt" was saved as "alpha" and then as "bravo!"
   ✔  Then the contract should offer no version deletion, expiry or thinning
  Scenario: A restore records its PROV-O activity in the same version
   ✔> Given the owner's file "a.txt" was saved as "alpha" and then as "bravo!"
   ✔  Given the host records provenance
   ✔  When the owner restores "a.txt" from "v1" with the label "Original"
   ✔  Then version "v3" should hold its PROV-O record
   ✔  And the restore should be journaled once with its commit
```

Whole BDD run:

```text
[Summary]
20 features
311 scenarios (200 passed, 111 skipped)
1012 steps (901 passed, 111 skipped)
```

The head-commit hook run (`FULL=1 bash scripts/pre-commit.sh`, `Test scope: full`) is recorded in the pull request.

## Mocked Environment Variables

None. The tests use real filesystem OCFL roots under the system temporary directory (`TMPDIR`) and remove them.
