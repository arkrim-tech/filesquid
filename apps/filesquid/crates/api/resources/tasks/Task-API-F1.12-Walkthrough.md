# Task API-F1.12 Walkthrough: Library and repair routes

Branch `feat/api-f1.12-library-repair-routes`, module `filesquid-api` ([plan row](../plan.md#phase-f2-library-and-repair-routes-global-task-11)), part of global Task 1.1.

## SRS mapping

| SRS ID and title | Covered by |
|---|---|
| [API-S-025](../srs.md) Local library and repair routes | `integration_library_routes` (22 tests, desktop and server mounts), `library_and_repair_routes_API-S-025.feature` (10 scenarios), `unit_contract`, `unit_auth` |
| [API-S-023](../srs.md) Version-inventory repair, consented and journaled | repair, consent, `nothing_to_repair` and one-row-per-object tests and scenarios through the route |
| [API-S-002](../srs.md) Versioned contract and generated types | `unit_contract` drift and sample tests; `ui/src/api/schema.d.ts` regenerated, UI `tests/unit/contract.test.ts` |
| [API-S-003](../srs.md) One router, mounted unchanged by every host | every route test runs on the desktop owner's and the server administrator's mount |
| [API-S-004](../srs.md) Deny-by-default authorisation | missing and wrong credential, server administrator `not_permitted`; `every_routed_path_declares_its_action` |
| [API-S-005](../srs.md) No paths, storage keys or content paths in answers | `assert_redacted` on every answer and the whole journal |
| [API-S-006](../srs.md) Application-layer isolation | routes call `LocalLibraryService` only; storage injected through `LibraryLocations`; `unit_isolation` keeps `filesquid-native` out of the graph |
| [API-S-017](../srs.md) Journal of refusals | `assert_one_refusal`: exactly one failed entry per refused call |
| [OCFL-S-020](../../../ocfl/resources/srs.md) Version-inventory repair | a repaired sha256 object accepts a new version |

## What changed

- **Routes** (`src/local_library/routes.rs`): `POST /api/v1/libraries` (create), `POST /api/v1/libraries/openings`, `POST /api/v1/libraries/snapshot-copies`, `POST /api/v1/libraries/rebuilds` (action `library.manage`), `GET /api/v1/libraries/repair-candidates` (`library.read`) and `POST /api/v1/libraries/repairs` (`library.repair`).
- **Authorisation**: three new `Action`s; `HostPolicy` grants the server administrator every action except `library.repair`, so every server caller is refused the repair with `not_permitted`, journaled once as `api.auth.denied`, before any storage is touched.
- **Host port** (`src/local_library/locations.rs`): a folder the owner chose never crosses the API as a path. The host registers it under an opaque location ID and resolves it through `LibraryLocations` to a `LibraryRoot` (storage backend and version reservations); `overlaps` lets the host keep a snapshot copy out of the library it copies (OCFL-S-019).
- **Commands** (`src/local_library/commands.rs`, `operations.rs`): `LibraryCommands` binds library IDs to locations on create and open (at most `MAX_OPEN_LIBRARIES` = 64; IDs at most 128 bytes), calls `LocalLibraryService` for every operation, and journals once the refusals it makes before the service runs (`location_not_found`, `library_not_open`, `location_in_use`, `library_already_open`, `too_many_libraries`, `destination_overlaps_library`, `invalid_location`, `invalid_library`). The object-safe `LibraryOperations` is installed with `ApiState::install_libraries`; until then the routes answer `unavailable`, journaled once.
- **CL-1.1 service**: unchanged in meaning. Its operation names are public, a `NotFound` error variant was added, a `journal_refusal` helper journals the pre-service refusals, and `RepairRequest` and `RepairConsent` refuse unknown fields. The `with_provenance` constructor is untouched.
- **Contract 0.12.0** (0.11.0 went to API-F1.11, which merged first): six paths, sixteen schemas, three responses (`LibraryNotFound`, `LibraryConflict`, `LibraryUnavailable`, which list the refusal codes); `schema.d.ts` regenerated with `npm run contract:types`.
- **Docs**: README "Run tests" paragraph for the new target (command run first); plan row ticked with its Status; checklist ticked; MVP progress redrawn.

TDD: the integration tests were written first and failed to compile against the missing API (`unresolved imports ... LibraryCommands, LibraryLocations, LibraryRoot, ... CREATE_OPERATION`); the contract and auth tests then failed on the undocumented paths and the action list before the contract and tests were updated.

## Test output

`cargo test -p filesquid-api --locked --test integration_library_routes --test unit_contract --test unit_auth`:

```text
running 22 tests
test a_copy_into_an_open_library_or_of_an_unknown_library_is_refused ... ok
test a_malformed_body_is_invalid_and_journaled_once ... ok
test a_repair_of_a_library_that_is_not_open_is_not_found_once ... ok
test a_head_with_both_files_is_refused_with_nothing_to_repair_and_nothing_written ... ok
test a_repair_with_a_missing_or_wrong_credential_is_denied_once ... ok
test an_unknown_location_is_not_found_and_journaled_once ... ok
test a_repair_without_consent_is_refused_once_and_writes_nothing ... ok
test a_consented_repair_by_the_desktop_owner_makes_a_sha256_object_committable ... ok
test create_keeps_a_library_id_the_caller_names ... ok
test create_in_a_folder_that_holds_a_file_is_refused_and_writes_nothing ... ok
test create_writes_only_the_declaration_and_answers_an_opaque_library_id ... ok
test every_server_caller_including_the_administrator_is_refused_the_repair ... ok
test consent_and_one_outcome_row_per_object_are_recorded_without_paths ... ok
test a_completed_snapshot_is_copied_to_an_empty_location_and_validates ... ok
test opening_a_folder_that_is_not_a_library_is_refused_once ... ok
test opening_an_open_location_answers_its_library_again ... ok
test open_answers_the_library_and_its_objects ... ok
test the_candidate_query_for_a_library_that_is_not_open_is_not_found ... ok
test the_routes_answer_unavailable_once_until_the_host_installs_its_commands ... ok
test rebuild_reports_every_object_and_flags_the_one_without_a_version_inventory ... ok
test the_candidate_query_lists_only_the_object_without_a_version_inventory ... ok
test open_libraries_are_bounded ... ok
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
running 35 tests
test a_host_route_cannot_be_made_public_through_the_guard ... ok
test a_non_bearer_authorization_header_is_refused_as_invalid_credentials ... ok
test a_principal_the_policy_does_not_permit_is_forbidden ... ok
test a_present_but_unusable_authorization_header_is_not_a_missing_one ... ok
test action_names_are_unique_dotted_identifiers ... ok
test a_websocket_upgrade_may_carry_the_token_as_a_subprotocol ... ok
test a_not_permitted_denial_names_the_principal_and_the_action ... ok
test an_empty_bearer_token_is_presented_and_then_rejected ... ok
test an_error_without_a_reason_omits_the_member ... ok
test an_authoriser_can_be_installed_only_once ... ok
test an_unusable_header_never_falls_back_to_the_subprotocol ... ok
test debug_output_never_shows_a_secret ... ok
test an_unusable_header_on_a_websocket_upgrade_is_refused_even_with_a_valid_subprotocol ... ok
test denials_map_reasons_to_codes_and_are_not_retryable ... ok
test every_denial_is_journaled_without_the_presented_credential ... ok
test every_route_answers_the_per_launch_token_holder ... ok
test every_routed_path_declares_its_action ... ok
test a_denial_never_echoes_the_presented_credential ... ok
test host_routes_are_denied_by_default ... ok
test launch_tokens_are_unique_256_bit_hex ... ok
test only_the_liveness_route_is_public ... ok
test the_bearer_scheme_is_read_case_insensitively ... ok
test the_host_policy_grants_the_administrator_every_action_but_the_repair ... ok
test the_host_policy_grants_the_owner_every_action ... ok
test fallback_refusals_are_journaled_with_the_no_route_target ... ok
test every_route_denies_a_call_without_the_per_launch_token ... ok
test the_subprotocol_carrier_is_read_from_any_offered_entry ... ok
test the_liveness_route_still_refuses_an_anonymous_wrong_method ... ok
test the_subprotocol_token_counts_only_next_to_the_events_subprotocol ... ok
test the_token_authenticator_accepts_only_the_exact_token ... ok
test the_token_issued_entry_has_no_field_that_could_carry_a_secret ... ok
test the_liveness_route_answers_without_a_token_and_says_only_that_the_process_is_up ... ok
test unknown_paths_and_wrong_methods_authenticate_before_routing ... ok
test without_an_installed_authoriser_every_route_is_denied ... ok
test every_route_denies_a_wrong_token ... ok
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
running 28 tests
test error_code_all_lists_every_variant ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test errors_carry_code_and_retryability ... ok
test event_topics_in_the_contract_match_the_rust_topics ... ok
test every_component_schema_has_a_rust_sample ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
test every_documented_operation_has_a_unique_operation_id ... ok
test every_event_topic_documents_its_payload_schema ... ok
test every_error_response_carries_the_api_error_schema ... ok
test every_operation_documents_both_denials ... ok
test every_limit_parameter_is_the_shared_page_size ... ok
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
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.53s
```

`cargo test -p filesquid-api --locked --test features`, the API-S-025 feature:

```text
Feature: Library and repair routes
  Scenario: Create a library in an empty folder through the API
   ✔  Given the desktop owner's host with an empty folder registered
   ✔  When the owner creates a library in that folder through the API
   ✔  Then the answer should name an opaque library ID and no filesystem path
   ✔  And the registered folder should hold only the OCFL 1.1 storage-root declaration
  Scenario: A folder that already holds a file is refused once
   ✔  Given the desktop owner's host with a registered folder that holds a file
   ✔  When the owner creates a library in that folder through the API
   ✔  Then the answer should be the error "conflict"
   ✔  And exactly one refusal "root_not_empty" should be journaled for "library.root.create"
   ✔  And the registered folder should still hold only that file
  Scenario: Copy a completed snapshot and rebuild from files through the API
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory
   ✔  When the owner copies a completed snapshot of the library to an empty folder through the API
   ✔  Then the copy should report 2 objects and no invalid object
   ✔  And the library folder should be unchanged
   ✔  When the owner rebuilds the library from its files through the API
   ✔  Then the rebuild should flag only the object without a version inventory
  Scenario: The candidate query offers only the object without a version inventory
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory
   ✔  When the owner asks the API which objects need a version-inventory repair
   ✔  Then only the object without a version inventory should be offered
  Scenario: A consented repair by the desktop owner makes the object committable
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory
   ✔  When the owner repairs the object without a version inventory with consent through the API
   ✔  Then the object should be answered as repaired
   ✔  And the consent and one outcome row per object should be recorded as "ocfl.repair.version_inventory"
   ✔  And the repaired object should accept a new version
  Scenario: A head that already has both files is refused with nothing_to_repair
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory
   ✔  When the owner repairs the complete object with consent through the API
   ✔  Then the object should be answered as refused with "nothing_to_repair"
   ✔  And the library folder should be unchanged
  Scenario: A repair without consent is refused once and changes nothing
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory
   ✔  When the owner asks the API to repair the object without a version inventory without consent
   ✔  Then the answer should be the error "conflict"
   ✔  And exactly one refusal "consent_required" should be journaled for "ocfl.repair.version_inventory"
   ✔  And the library folder should be unchanged
  Scenario: A repair without a credential is refused once
   ✔  Given the desktop owner's library with a complete object and an object without a version inventory
   ✔  When a caller without a credential asks the API to repair the object without a version inventory
   ✔  Then the answer should be the error "unauthorized"
   ✔  And exactly one refusal "missing_credentials" should be journaled for "api.auth.denied"
   ✔  And the library folder should be unchanged
  Scenario: Every server caller, the administrator included, is refused the repair
   ✔  Given the server administrator's library with a complete object and an object without a version inventory
   ✔  When the server administrator repairs the object without a version inventory with consent through the API
   ✔  Then the answer should be the error "forbidden"
   ✔  And exactly one refusal "not_permitted" should be journaled for "api.auth.denied"
   ✔  And the library folder should be unchanged
  Scenario: The server administrator can still rebuild a library
   ✔  Given the server administrator's library with a complete object and an object without a version inventory
   ✔  When the server administrator rebuilds the library from its files through the API
   ✔  Then the rebuild should flag only the object without a version inventory
```

Whole BDD run:

```text
[Summary]
19 features
301 scenarios (173 passed, 128 skipped)
880 steps (752 passed, 128 skipped)
```

The head-commit hook run (`FULL=1 bash scripts/pre-commit.sh`, `Test scope: full`) is recorded in the pull request.

## Mocked Environment Variables

None. The tests use real filesystem OCFL roots under the system temporary directory (`TMPDIR`) and remove them.
