# Task API-F2.8 Walkthrough: Journal and commit linkage for PROV-O history

Branch: `feat/prov-o-history` (global Task FS-2.8; ADR 040 Decision 6).

## API-S-028: PROV-O record per commit and journal linkage

Also API-S-017 (activity journal entry shape and redaction).

## What changed

- `JournalEntry` gains optional `operation_id` and `object_ref` (`<object-id>@vN`, which is `urn:uuid:<uuid>@vN` for FileSquid objects), with builder methods.
  - Redaction: `operation_id` passes the existing opaque check. `object_ref` passes only when it parses as a `VersionRef` whose object ID is opaque. `@` stays forbidden in every other field, so an e-mail address in `actor` is still redacted.
  - Both fields are documented in `openapi.json` (contract 0.7.0, additive), set in the contract sample, kept by the tracing whitelist and regenerated into `apps/filesquid/ui/src/api/schema.d.ts`.
- `journal::commit::ocfl_commit_entries(&CommitFacts)` journals every published commit as `ocfl.commit` (operation ID, object reference, object ID as target). A reservation that took over a still-held lease adds an `ocfl.lease.takeover` warning naming the same object version.
- `JournalStore::allocate_ids` (memory store; SQLite in `filesquid-native`) hands out IDs before a commit. `append` stores an entry carrying such an ID under it, and stores a replay only once.
- `AnnouncingJournal` (and `Monitor::announcing_store`) announces entries appended outside the writer with the same `journal.entry` events and counters. The desktop hands it to the catalog, so outbox entries are announced (DE-0.10).
- Version-inventory repair: consent, every outcome and the answer share one repair run ID as their `operation_id` (`RepairAnswer::run_id`), and an outcome names `object@version`.

## Commit paths linked so far

- Library snapshot commits (DE-0.3, merged while this task was open) set the installation `user`, write the `filesquid.operation.v1` message with the operation `kind` (`canonical::OperationRecord.kind`, mapped by `canonical::operation_kind`), and journal `ocfl.commit` plus the operation's own entry with `operation_id` and `object_ref`; with a provenance context both entries ride the version (`CommitRecorder::with_entries`). See the DE-0.10 walkthrough.
- One message format: `filesquid_core::OperationMessage` reads the catalog's message (revision as a decimal string, `created_id`, `deletion`; a message without `kind` reads as `unknown`) and writes the same shape.
- Repair follow-up versions are recorded by API-F2.9.

## Not delivered in this task, and why

- File operations (API-F1.3) and metadata routes (API-F1.4) do not exist, so no API path commits file objects or journals `metadata.edit` yet; they use `CommitRecorder` and `CommitOptions` when they land. The stand-alone `fsquid` metadata CLI keeps no host journal.

```text
$ cargo test -p filesquid-api --test unit_canonical --test unit_history
test a_head_without_its_version_inventory_is_refused_and_nothing_is_written ... ok
test a_file_original_is_its_own_object_with_its_metadata_record ... ok
test a_recorded_kind_is_written_and_read_back_by_both_readers ... ok
test a_root_without_a_snapshot_has_no_commit_journal ... ok
test a_metadata_record_without_its_original_is_refused_on_read ... ok
test a_storage_root_is_created_only_in_an_empty_folder ... ok
test a_committed_snapshot_reads_back_entity_for_entity ... ok
test an_entity_that_does_not_match_its_path_is_refused_on_read ... ok
test an_entity_whose_identifier_cannot_name_a_path_is_a_decode_error ... ok
test an_original_name_or_media_type_that_is_not_valid_is_refused ... ok
test a_stale_draft_left_in_staging_is_never_published ... ok
test an_unreadable_operation_message_is_refused_on_read ... ok
test canonical_failures_keep_their_meaning_at_the_catalog_port ... ok
test catalog_operations_map_to_history_kinds ... ok
test each_change_touches_the_library_and_its_own_entity ... ok
test an_owner_name_round_trips_through_its_metadata_record ... ok
test every_entity_has_one_logical_path_and_back ... ok
test identifiers_that_cannot_name_a_path_segment_are_refused ... ok
test the_metadata_record_is_readable_front_matter ... ok
test the_operation_message_carries_its_receipt ... ok
test the_stored_name_is_the_file_identifier_and_its_extension_only ... ok
test a_later_version_changes_only_the_entities_it_touches ... ok
test history_files_ride_the_snapshot_version_and_are_not_entities ... ok
test an_entity_file_larger_than_the_bound_is_refused_before_it_is_read ... ok
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s
test a_history_entry_carries_the_label_and_marks_reconstruction ... ok
test a_memory_history_indexes_its_records_and_refuses_conflicts ... ok
test a_receipt_for_another_version_still_journals_the_commit ... ok
test a_record_that_cannot_be_built_stops_the_commit ... ok
test a_file_history_lists_its_object_only ... ok
test a_redacted_record_names_no_installation_message_or_path ... ok
test a_takeover_adds_a_warning_after_the_commit_entry ... ok
test a_version_record_is_raw_json_ld_or_not_found ... ok
test a_commit_writes_its_record_and_journal_entries_in_the_same_version ... ok
test an_ocfl_history_without_a_storage_root_is_unavailable ... ok
test commit_meta_sets_the_user_and_the_operation_message ... ok
test every_new_journal_event_keeps_its_opaque_fields_and_loses_paths ... ok
test history_routes_answer_unavailable_before_the_host_installs_a_source ... ok
test bad_history_parameters_answer_invalid_request ... ok
test a_rebased_commit_regenerates_both_files_for_the_new_version ... ok
test redaction_keeps_derivations_linked_and_paths_numbered_across_records ... ok
test the_export_is_a_json_ld_array_and_can_be_redacted ... ok
test the_version_user_names_the_installation_and_never_a_person ... ok
test an_ocfl_history_is_rebuilt_from_the_records_in_the_storage_root ... ok
test the_history_lists_everything_or_one_filter_in_cursor_pages ... ok
test the_operations_own_entries_ride_the_version_with_their_ids ... ok
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

## SRS traceability

| Requirement | Tests |
|---|---|
| API-S-028 fields, redaction, `ocfl.commit`, takeover | `unit_journal_linkage` (6 tests), `unit_contract` (fields documented and sampled) |
| API-S-028 IDs before the commit | `filesquid-native` `unit_journal::both_stores_assign_ids_before_the_entries_are_written` |
| API-S-028 repair run ID | `unit_version_inventory_repair::consent_and_outcomes_share_one_repair_run_id` |
| API-S-017 announced entries | `unit_writer::an_announcing_store_publishes_and_counts_what_the_store_accepted` |

## TDD: failing run before implementation

```text
      1 error: could not compile `filesquid-api` (test "unit_journal_linkage") due to 8 previous errors
      1 error[E0432]: unresolved import `filesquid_api::journal::commit`
      1 error[E0599]: no method named `object_ref` found for struct `JournalEntry` in the current scope
      2 error[E0599]: no method named `operation_id` found for struct `JournalEntry` in the current scope
      1 error[E0609]: no field `object_ref` on type `&JournalEntry`
      2 error[E0609]: no field `operation_id` on type `&JournalEntry`
```

## Mocked Environment Variables

None.

## Raw test output

`cargo test -p filesquid-api --locked --test unit_journal_linkage --test unit_writer --test unit_version_inventory_repair --test unit_contract`:

```text
    Finished `test` profile [unoptimized] target(s) in 1.70s
     Running tests/unit/contract.rs (<target>/debug/deps/unit_contract-9a02b5fcf76a9cf3)

running 28 tests
test error_code_all_lists_every_variant ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test errors_carry_code_and_retryability ... ok
test event_topics_in_the_contract_match_the_rust_topics ... ok
test every_component_schema_has_a_rust_sample ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
test every_documented_operation_has_a_unique_operation_id ... ok
test every_error_response_carries_the_api_error_schema ... ok
test every_event_topic_documents_its_payload_schema ... ok
test every_limit_parameter_is_the_shared_page_size ... ok
test every_operation_names_the_action_the_router_authorises ... ok
test every_operation_documents_both_denials ... ok
test every_operation_requires_the_bearer_scheme_and_none_makes_it_optional ... ok
test host_routes_are_the_event_channel_only ... ok
test health_schema_matches_the_core_payload_and_requires_every_field ... ok
test query_envelope_and_page_size_parameter_are_bounded_like_core ... ok
test routed_and_documented_paths_agree_both_ways ... ok
test router_fallbacks_are_documented_as_wire_conventions ... ok
test router_serves_its_own_contract ... ok
test the_checker_accepts_only_truly_unconstrained_objects ... ok
test the_contract_uses_only_checked_schema_keywords ... ok
test every_documented_operation_answers_a_documented_status_and_body ... ok
test the_only_public_operation_is_the_liveness_probe ... ok
test router_fallbacks_answer_as_documented ... ok
{"error":"journal writer stopped","level":"ERROR","message":"journal write failed"}
{"actor":"system","host":"","journal_write_failed":true,"level":"info","operation":"process.cancelled","outcome":"cancelled","process_id":"import-1","target_ids":[],"time":"2026-10-19T05:38:46.563Z"}
test events_from_every_publisher_match_their_documented_payloads ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s

     Running tests/unit/journal_linkage.rs (<target>/debug/deps/unit_journal_linkage-45194b54519bfaf2)

running 6 tests
test a_commit_is_journaled_as_ocfl_commit_with_operation_and_object_version ... ok
test a_lease_takeover_is_journaled_naming_the_object_and_version ... ok
test an_entry_carries_its_operation_id_and_object_reference ... ok
test identifying_values_in_the_new_fields_are_redacted ... ok
test the_trace_layer_keeps_the_operation_id_and_object_reference ... ok
test both_fields_are_stored_and_published_by_the_writer ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/version_inventory_repair.rs (<target>/debug/deps/unit_version_inventory_repair-45530adf0c59e26b)

running 21 tests
test a_damaged_interrupted_repair_is_refused_without_writing_the_sidecar ... ok
test a_held_reservation_is_a_retryable_refusal ... ok
test a_consented_repair_by_the_owner_makes_a_sha256_object_committable ... ok
test a_head_that_already_has_both_files_is_refused_with_nothing_to_repair ... ok
test a_root_sidecar_that_does_not_commit_the_root_inventory_is_refused ... ok
test an_interrupted_repair_is_resumed ... ok
test an_unlisted_file_in_the_head_is_neither_offered_nor_repaired ... ok
test an_unrecordable_consent_repairs_nothing ... ok
test an_unknown_object_is_refused_per_object ... ok
test a_second_repair_is_refused_with_nothing_to_repair ... ok
test candidates_say_whether_the_repair_opens_the_object_to_commits ... ok
test an_unrecordable_outcome_still_reaches_the_journal ... ok
test object_lists_are_bounded_and_distinct ... ok
test open_copy_and_rebuild_never_reach_the_repair ... ok
test the_candidate_query_lists_heads_without_their_version_inventory ... ok
test the_command_is_refused_for_another_caller ... ok
test the_consent_statement_id_is_pinned ... ok
test consent_and_one_outcome_row_per_object_are_journaled_without_paths ... ok
test consent_and_outcomes_share_one_repair_run_id ... ok
test the_consent_statement_names_what_changes ... ok
test the_command_is_refused_without_consent_and_writes_nothing ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/unit/writer.rs (<target>/debug/deps/unit_writer-0c201d9f7665ebdc)

running 6 tests
test an_announcing_store_publishes_and_counts_what_the_store_accepted ... ok
test live_journal_events_carry_the_redacted_entry ... ok
test trace_layer_journals_warnings_without_free_text_or_unlisted_fields ... ok
test a_burst_of_ten_times_capacity_loses_nothing ... ok
test trace_layer_keeps_display_formatted_whitelisted_fields ... ok
test written_entries_are_published_on_the_event_bus_and_counted ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`npx vitest run tests/unit/contract.test.ts` in `apps/filesquid/ui` (the regenerated contract types):

```text
 Test Files  1 passed (1)
      Tests  1 passed (1)
   Start at  05:38:47
   Duration  1.14s (transform 62ms, setup 98ms, import 717ms, tests 94ms, environment 0ms)

JUNIT report written to /home/user/filesquid/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```
