

## CO-0.11 closure (2026-10-23)

Owner decision 3b of 2026-10-23 ("provide and implement recommendations for owner's decisions 1-6"): `OperationKind` gains `publish`; catalog organisation is serialised `catalog.organisation`, as the F-067 feature says; the backfill kind keeps the name `backfill` (ADR 040 Decision 7), and the plan row's Description, CORE-S-010 and the one "reconstruction job" line of `prov_o_history_F-067.feature` now say backfill. Branch `fix/co-0.11-operation-kinds`.

### CORE-S-010: PROV-O history contract (the operation kinds and their stable tokens)

- `OperationKind::Publish` (`publish`), for share and publication outputs, between `scrub` and `backfill`; `OperationKind::ALL` has 22 kinds.
- `OperationKind::CatalogOrganisation` is written `catalog.organisation`.
- The former token `catalog` is still read as catalog organisation (`OperationKind::parse`, and so serde and `OperationMessage::parse`) and is never written: the API has written it in the `filesquid.operation.v1` messages of OCFL versions and in their `provenance/<version>.jsonld` records since PR #86, and committed OCFL versions are immutable, so without the alias those versions' messages would be refused as `unknown-kind` and their history entries would read as `unknown`. A message read with `catalog` is formatted again as `catalog.organisation`. The token is the public constant `FORMER_CATALOG_TOKEN`.

### API-S-002: The versioned API contract owned by the API crate

`crates/api/openapi.json` 0.24.0 to 0.26.0, one minor above main's 0.25.0 (API-F2.13) after merging origin/main (`API_VERSION`, its `unit_contract` assertion and the UI's `CONTRACT_VERSION` with it): the `OperationKind` enum lists `catalog.organisation` and `publish`. `ui/src/api/schema.d.ts` regenerated with `npm run contract:types` (one line). `filesquid-prov` keeps kinds as validated strings (`check_kind` allows `.`), so it needed no change; no API code maps an operation to `Publish` yet, which API-F2.9's share and publication outputs will do. One desktop assertion read the written token (`crates/desktop/tests/integration/library_catalog.rs`, `a_catalog_commit_carries_its_history_in_the_same_version`) and now expects `catalog.organisation`; nothing else in the workspace, the UI or ImageSquid writes or asserts the `catalog` token. Generic readers keep reading old values as they are (`filesquid-prov` stores the kind as a validated string, and the API maps a record's kind through `OperationKind::parse`, which takes the former token).

### Tests

- `unit_history`: the token table with `catalog.organisation` and `publish`; `the_former_catalog_token_is_read_as_catalog_organisation` (parse, serde and a stored message, rewritten in the current spelling); `the_publish_kind_is_written_and_parsed_back`.
- `prov_o_history_contract_F-067.feature`: the outline "Catalog organisation, publication and backfill have stable tokens" and the scenario "A version committed before the rename is read as catalog organisation".

### TDD: failing run before the change

```text
error[E0599]: no variant, associated function, or constant named `Publish` found for enum `OperationKind` in the current scope
  --> crates/core/tests/unit/history.rs:50:25
   |
50 |         (OperationKind::Publish, "publish"),
   |                         ^^^^^^^ variant, associated function, or constant not found in `OperationKind`

error[E0599]: no variant, associated function, or constant named `Publish` found for enum `OperationKind` in the current scope
  --> crates/core/tests/unit/history.rs:86:69
   |
86 |     let message = OperationMessage::new(op("op-10"), OperationKind::Publish);
   |                                                                     ^^^^^^^ variant, associated function, or constant not found in `OperationKind`

--
error: could not compile `filesquid-core` (test "unit_history") due to 2 previous errors
```

### Raw test output

`cargo test -p filesquid-core --test unit_history --locked`:

```text
    Updating crates.io index
   Compiling filesquid-core v0.1.0 (<repo>/apps/filesquid/crates/core)
    Finished `test` profile [unoptimized] target(s) in 0.97s
     Running tests/unit/history.rs (<target>/debug/deps/unit_history-b97d00bdec4538cd)

running 22 tests
test a_journal_link_names_an_operation_and_opaque_entry_ids ... ok
test a_history_entry_serializes_one_activity_for_the_api ... ok
test a_message_without_the_prefix_is_not_a_filesquid_message ... ok
test an_operation_message_formats_and_parses_back ... ok
test a_version_ref_renders_the_object_reference ... ok
test an_oversized_label_is_refused_before_it_is_committed ... ok
test an_oversized_message_is_refused ... ok
test every_operation_kind_has_a_stable_token ... ok
test installation_id_is_a_uuid_v4_rendered_as_a_urn ... ok
test an_unreadable_prefixed_message_is_refused_not_dropped ... ok
test installation_ids_that_are_not_uuid_v4_are_refused ... ok
test optional_members_are_left_out ... ok
test prov_context_is_the_prov_namespace_iri ... ok
test refusal_errors_name_no_stored_value ... ok
test the_catalog_receipt_message_is_read ... ok
test the_former_catalog_token_is_read_as_catalog_organisation ... ok
test the_publish_kind_is_written_and_parsed_back ... ok
test the_revision_is_written_as_a_decimal_string_and_read_back ... ok
test the_sidecar_operation_is_the_journal_links_only ... ok
test the_sidecar_keeps_its_gate_and_links_version_operation_and_journal ... ok
test unicode_escapes_and_surrogate_pairs_are_decoded ... ok
test version_names_beyond_eighteen_digits_are_valid_object_versions ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo test -p filesquid-core --test features --locked` (the CORE-S-010 feature and the summary):

```text
Feature: PROV-O history contract types
  Scenario: The version message of a commit round-trips
   ✔  When I format the operation message for operation "op-7" of kind "metadata.edit" with label "Before cleanup"
   ✔  Then the message should start with "filesquid.operation.v1 "
   ✔  And parsing it should give operation "op-7", kind "metadata.edit" and label "Before cleanup"
  Scenario Outline: Catalog organisation, publication and backfill have stable tokens
   ✔  When I format the operation message for operation "op-8" of kind "catalog.organisation" with label "Release"
   ✔  Then parsing it should give operation "op-8", kind "catalog.organisation" and label "Release"
  Scenario Outline: Catalog organisation, publication and backfill have stable tokens
   ✔  When I format the operation message for operation "op-8" of kind "publish" with label "Release"
   ✔  Then parsing it should give operation "op-8", kind "publish" and label "Release"
  Scenario Outline: Catalog organisation, publication and backfill have stable tokens
   ✔  When I format the operation message for operation "op-8" of kind "backfill" with label "Release"
   ✔  Then parsing it should give operation "op-8", kind "backfill" and label "Release"
  Scenario: A version committed before the rename is read as catalog organisation
   ✔  When I parse the version message "filesquid.operation.v1 {\"operation_id\":\"op-9\",\"kind\":\"catalog\"}"
   ✔  Then it should be read as kind "catalog.organisation" and written back as "catalog.organisation"
  Scenario: A message another tool wrote is not a FileSquid message
   ✔  When I parse the version message "Edit metadata.md"
   ✔  Then it should not be a FileSquid operation message
  Scenario: An unreadable FileSquid message is refused, not dropped
   ✔  When I parse the version message "filesquid.operation.v1 {\"kind\":\"import\"}"
   ✔  Then the message should be refused as unreadable
  Scenario: The installation agent is a UUID v4 URN
   ✔  When I parse the installation ID "6F1D2C3B-4A5E-4F60-8A7B-9C0D1E2F3A4B"
   ✔  Then its agent identifier should be "urn:uuid:6f1d2c3b-4a5e-4f60-8a7b-9c0d1e2f3a4b"
  Scenario: The PROV namespace is the W3C namespace IRI
   ✔  Then the PROV-O context should be "http://www.w3.org/ns/prov#" and the profile "filesquid-prov/1"
[Summary]
12 features
72 scenarios (68 passed, 4 skipped)
229 steps (225 passed, 4 skipped)
```

`cargo test -p filesquid-api --test unit_contract --locked`:

```text
 Downloading crates ...
  Downloaded multer v3.1.0
  Downloaded spin v0.9.9
  Downloaded multiversion_no_op v1.0.0
  Downloaded core_detect v1.0.0
  Downloaded simdutf8 v0.1.5
  Downloaded encoding_rs v0.8.42
   Compiling rustversion v1.0.23
   Compiling multer v3.1.0
   Compiling multiversion_no_op v1.0.0
   Compiling simdutf8 v0.1.5
   Compiling core_detect v1.0.0
   Compiling spin v0.9.9
   Compiling encoding_rs v0.8.42
   Compiling axum v0.8.9
   Compiling filesquid-api v0.1.0 (<repo>/apps/filesquid/crates/api)
    Finished `test` profile [unoptimized] target(s) in 24.50s
     Running tests/unit/contract.rs (<target>/debug/deps/unit_contract-27eeb267071a4e20)

running 28 tests
test error_code_all_lists_every_variant ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test errors_carry_code_and_retryability ... ok
test event_topics_in_the_contract_match_the_rust_topics ... ok
test every_component_schema_has_a_rust_sample ... ok
test every_documented_operation_has_a_unique_operation_id ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
test every_event_topic_documents_its_payload_schema ... ok
test every_error_response_carries_the_api_error_schema ... ok
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
{"error":"journal writer stopped","level":"ERROR","message":"journal write failed"}
{"actor":"system","host":"","journal_write_failed":true,"level":"info","operation":"process.cancelled","outcome":"cancelled","process_id":"import-1","target_ids":[],"time":"2026-10-23T19:01:59.501Z"}
test events_from_every_publisher_match_their_documented_payloads ... ok
test every_documented_operation_answers_a_documented_status_and_body ... ok
test router_fallbacks_answer_as_documented ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.67s
```
