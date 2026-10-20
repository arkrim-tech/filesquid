# Task API-F2.2 Walkthrough: View operation, preferences, storage use, copy, move and ZIP download (part 1)

Branch `feat/api-f2.2-views-copy-move-zip`, module `filesquid-api` ([plan row](../plan.md)), part of global Task FS-2.5. This is part 1, cut with the orchestrator on 2026-10-20: the plan box stays unticked, and the remaining items are listed below and in the plan row's status line. Holistic UI check: the default UI's view controls (UI-F2.8) read and save these preferences, the navigation tree (UI-F2.1a) moves its device-kept choices here once under UI-S-017, the file view of UI-F2.1b lists a collection or folder node directly, and a "Download" action can save a whole file of any size on `filesquid-server`.

## SRS mapping

| SRS ID and title | Covered by |
|---|---|
| [API-S-011](../srs.md) View operation, per-user UI preferences, storage use, copy, move and ZIP download (part 1: preferences, storage use per library, whole-file download, reserved folders, listing by node) | `integration_drive` (17 tests), `preferences_downloads_listings_API-S-011.feature` (7 scenarios), `unit_contract`, `unit_auth` |
| [SRS-F-062](../../../../../../resources/srs.md#srs-f-062-drive-style-views-sorting-selection-and-layout-preferences) Drive-style views, sorting, selection and layout preferences | start view, sort key and direction, folder placement, layout and hover card stored per user: `saved_preferences_are_restored_and_journaled_without_their_values`, `preferences_belong_to_the_user_who_saved_them` |
| [UI-S-017](../../../../ui/resources/srs.md) Navigation preferences | version 1, at most 100 pins, 16 section ids of 1 to 64 characters and 1,000 collapsed handles, node handles only: `every_bound_of_the_preferences_is_enforced`, `the_largest_navigation_choices_are_kept` |
| [API-S-021](../srs.md) Navigation listing and node handles | readability through the navigation service, one `not_found` for unknown, malformed or unreadable nodes: `a_node_the_caller_may_not_read_is_not_found`, `unknown_malformed_and_unmapped_nodes_are_not_found` |
| [API-S-008](../srs.md) File operation routes | reserved `provenance` and `.filesquid` folders left out of listings and closed to writes: `listings_leave_out_the_reserved_folders`, `files_cannot_be_written_into_the_provenance_folder` |
| [API-S-027](../srs.md) Display names in download answers | `Content-Disposition: attachment` with the display name: `a_file_larger_than_one_answer_downloads_whole_in_ranged_pieces` |
| [NFR-003](../../../../../../resources/srs.md#srs-nfr-003-bounded-processing-and-migration-endurance) Bounded processing | the download streams 8 MiB ranged reads, at most `size / 8 MiB + 1` of them, never the whole file in memory |
| [API-S-002](../srs.md) Versioned contract and generated types | contract 0.15.0, `unit_contract` drift and sample tests, regenerated `ui/src/api/schema.d.ts` |

## What changed

- **Preferences** (`src/preferences/`): `GET` and `PUT /api/v1/preferences` under the new action `preferences.manage`, keyed by the principal's actor name, so each caller reaches only their own. `Preferences` holds the start view, sort key and direction, folder placement, layout, hover card and `NavigationPreferences` (version 1, pins as `{node, kind}`, section order, collapsed handles). Every node must be a handle `NodeRef::from_handle` accepts, so no name, title, path or token can be stored in its place. A save is journaled as `preferences.update` with the actor only. The host injects a `PreferenceStore` with `ApiState::install_preferences`; `MemoryPreferenceStore` is the in-memory store. The routes answer `unavailable` until a store is installed.
- **Storage use per library** is the existing `GET /api/v1/files/library-history-size` of API-F2.5. API-S-011 now names it as the storage-use route.
- **Whole-file download** (`src/files/download.rs`): `GET /api/v1/files/download` answers `200` with `Content-Length`, the media type, `nosniff` and an attachment disposition naming the display name. The body is a stream of ranged reads of at most 8 MiB, each through a freshly opened object, bounded by the file's size. A missing file is refused before the answer starts; a failure mid-stream is journaled once as `files.download` and ends the stream early. Through the desktop scheme every answer stays capped at 16 MiB (DE-S-005), so the desktop UI keeps saving larger files with ranged reads; the contract says so.
- **Reserved folders** (`src/files/labels.rs`, `service.rs`): `RESERVED_FOLDERS` are `.filesquid` and `provenance`. The root listing leaves them out, so a page may hold fewer entries than asked while a cursor remains; a prefix inside them is `not_found`; uploads, moves, removals, restores and new folders there are refused.
- **Listing by node** (`src/files/nodes.rs`): `GET /api/v1/files/listing` takes either `library_id` and `object_id` (with an optional `prefix`) or one `node`. A collection node lists the root of its scope's object and a folder node lists its path. Readability is checked through `NavigationService::check_readable` on a blocking thread. Library, virtual collection and repository nodes are `invalid_request`. The host maps a folder scope to its object through the new `ScopeObjects` port (`FileService::install_scope_objects`).
- **Contract 0.15.0**: the download, preferences and node-listing operations, the `Preferences`, `NavigationPreferences`, `PinnedDestination`, `StartView`, `SortKey`, `SortDirection`, `FolderPlacement` and `Layout` schemas, and the `PreferencesUnavailable` response. `library_id` and `object_id` are optional on the listing.
- **File-write contract** ([note](./Task-API-F2.2-Conflict.md)): the file-write routes answer plain bodies and `FileCommit`, while UI-S-012 and the contract convention require `CommandEnvelope` and `CommandOutcome`. The owner decided on 2026-10-20 to follow the SRS in a separate contract-wide API task; this PR keeps the plain bodies and adds no write route.

## Remaining (box unticked)

- (5) Copy with a new FileId and source provenance: waits on API-F1.4's record-codec change (owner decision 2026-10-20: `FileRecord` is the only codec), which writes the new FileId's record.
- (6) Listing by library and virtual-collection node in the canonical per-file model: waits on the same change.
- (7) The view operation's per-file rows (sort keys `modified_by_me` and `opened_by_me` from the caller's journal events, view-local filter): waits on the same change.
- (8) The streamed ZIP download with the outgoing metadata policy per member: waits on an F-046 outgoing-policy engine.
- (10) Storage use per collection: waits on (6)'s mapping.

## Test output

```text
$ cargo test -p filesquid-api --locked --test integration_drive --test unit_contract --test unit_auth --test integration_files --test integration_navigation --test integration_versions --lib
     Running unittests src/lib.rs (/home/user/.cargo-target-shared/debug/deps/filesquid_api-3ee00c2894e5572f)
test auth::token::tests::equal_slices_compare_equal_and_every_difference_is_found ... ok
test admin::tests::a_complete_export_has_no_continuation_cursor_header ... ok
test admin::tests::an_export_that_hits_the_page_cap_returns_a_continuation_cursor ... ok
test content::encoding::tests::only_43_base64url_characters_are_well_formed ... ok
test auth::layer::tests::without_a_monitor_a_refusal_goes_to_stdout_unfiltered ... ok
test auth::token::tests::secrets_compare_by_digest_whatever_their_lengths ... ok
test content::encoding::tests::thirty_two_bytes_become_43_characters_matching_rfc_4648 ... ok
test content::store::tests::a_clock_that_moves_back_ends_the_handle ... ok
test content::store::tests::a_minted_handle_is_accepted_until_it_idles_out ... ok
test content::store::tests::a_handle_no_longer_allowed_is_refused_and_removed ... ok
test content::store::tests::ending_a_credential_ends_only_its_handles ... ok
test files::labels::tests::a_file_with_an_invalid_label_is_never_written ... ok
test files::labels::tests::an_override_equal_to_the_commit_label_is_dropped ... ok
test files::labels::tests::the_file_round_trips ... ok
test content::store::tests::the_store_keeps_digests_and_debug_shows_no_handle ... ok
test files::labels::tests::the_reserved_folder_is_the_folder_and_its_contents_only ... ok
test files::media::tests::extensions_map_without_case_and_unknown_ones_fall_back ... ok
test files::names::tests::a_non_ascii_name_with_spaces_gets_a_fallback_and_an_exact_utf8_form ... ok
test files::names::tests::long_names_are_cut_at_a_character_boundary ... ok
test files::names::tests::separators_controls_and_quotes_never_reach_the_header ... ok
test files::range::tests::plans_cap_every_answer_and_find_unsatisfiable_ranges ... ok
test files::range::tests::repeated_range_fields_are_several_ranges ... ok
test files::range::tests::single_byte_ranges_parse ... ok
test history::redact::tests::a_path_keeps_its_token_across_records ... ok
test history::redact::tests::an_entity_id_loses_only_its_path ... ok
test history::redact::tests::a_path_without_a_numbered_identifier_refuses_the_record ... ok
test history::redact::tests::non_objects_and_deep_documents_are_refused ... ok
test history::source::tests::a_storage_failure_is_unavailable_and_a_bad_record_is_corrupt ... ok
test files::labels::tests::the_thousand_and_first_labelled_version_is_refused ... ok
test history::routes::tests::an_export_past_its_byte_budget_stops_at_a_whole_record_and_is_flagged ... ok
test journal_writer::tests::a_rejected_write_is_one_json_line_per_record_on_stdout ... ok
test ocfl_validation::run::tests::repeated_identifiers_get_distinct_keys ... ok
test ocfl_validation::service::tests::an_old_run_lock_never_frees_a_reopened_library ... ok
test ocfl_validation::service::tests::closing_a_library_cancels_its_run ... ok
test ocfl_validation::service::tests::reopening_an_open_library_keeps_its_locks ... ok
test routes::tests::the_liveness_probe_is_the_one_public_path ... ok
test ocfl_validation::service::tests::shutdown_waits_for_takeover_checks_and_refuses_new_ones ... ok
test routes::tests::a_route_other_than_the_liveness_probe_cannot_be_public - should panic ... ok
test files::labels::tests::the_largest_allowed_labels_file_stays_within_its_read_bound ... ok
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
     Running tests/integration/drive.rs (/home/user/.cargo-target-shared/debug/deps/integration_drive-6eb2583c57ee0c00)
test a_listing_names_exactly_one_form ... ok
test a_collection_node_lists_the_root_of_its_files ... ok
test a_folder_node_lists_that_folder_with_paging ... ok
test a_user_who_saved_nothing_gets_the_default_preferences ... ok
test a_missing_file_is_refused_before_the_download_starts ... ok
test an_earlier_version_downloads_as_it_was ... ok
test a_node_the_caller_may_not_read_is_not_found ... ok
test files_cannot_be_written_into_the_provenance_folder ... ok
test every_bound_of_the_preferences_is_enforced ... ok
test preferences_are_unavailable_until_the_host_keeps_them ... ok
test library_virtual_collection_and_repository_nodes_do_not_list_as_folders ... ok
test preferences_belong_to_the_user_who_saved_them ... ok
test saved_preferences_are_restored_and_journaled_without_their_values ... ok
test listings_leave_out_the_reserved_folders ... ok
test the_largest_navigation_choices_are_kept ... ok
test unknown_malformed_and_unmapped_nodes_are_not_found ... ok
test a_file_larger_than_one_answer_downloads_whole_in_ranged_pieces ... ok
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.63s
     Running tests/integration/files.rs (/home/user/.cargo-target-shared/debug/deps/integration_files-52312e27faebda19)
test a_hostile_original_name_never_reaches_a_header_as_a_path ... ok
test a_file_length_is_read_once_then_cached ... ok
test a_limit_outside_1_to_1000_or_a_malformed_cursor_is_refused ... ok
test a_file_added_beneath_a_pending_folder_persists_it ... ok
test a_move_of_a_missing_path_is_not_found_and_publishes_nothing ... ok
test a_move_to_the_same_path_is_refused_without_a_version ... ok
test a_malformed_or_multi_range_header_is_ignored ... ok
test a_prefix_lists_one_folder_level ... ok
test a_read_without_a_range_answers_the_whole_small_file ... ok
test a_read_pins_the_version_it_names ... ok
test a_move_renames_a_file_or_a_whole_folder_in_one_version ... ok
test a_request_body_with_an_unknown_field_is_invalid ... ok
test a_restore_of_the_current_version_is_refused_without_a_version ... ok
test a_remove_and_a_restore_each_publish_one_version ... ok
test an_upload_above_the_cap_is_refused ... ok
test an_unsatisfiable_range_answers_416_with_the_length ... ok
test an_unknown_library_object_version_or_folder_is_not_found_and_journaled ... ok
test an_upload_that_changes_nothing_is_refused_without_a_version ... ok
test an_upload_commits_exactly_one_version_and_is_journaled ... ok
test creating_a_folder_commits_nothing_and_stays_pending ... ok
test details_answer_storage_facts_without_any_storage_path ... ok
test file_routes_need_the_token_and_refusals_are_journaled ... ok
test head_describes_the_file_without_reading_its_content ... ok
test head_with_a_range_describes_the_get_for_that_range ... ok
test listings_and_details_show_the_original_name_not_the_stored_one ... ok
test history_lists_the_versions_that_changed_a_file ... ok
test no_answer_or_journal_entry_carries_a_filesystem_path ... ok
test pages_follow_the_opaque_cursor_without_a_repeat_or_gap ... ok
test reads_and_downloads_are_named_by_the_original_name ... ok
test single_ranges_answer_206_with_their_content_range ... ok
test the_root_lists_folders_first_then_files ... ok
test versions_list_every_version_and_the_head ... ok
test the_read_routes_refuse_an_invalid_path_before_any_lookup ... ok
test a_listing_holds_the_default_page_of_200 ... ok
test answers_are_capped_at_16_mib ... ok
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.09s
     Running tests/integration/navigation.rs (/home/user/.cargo-target-shared/debug/deps/integration_navigation-49379a51bb48bb22)
test a_cursor_is_refused_at_another_parent_of_the_same_kind ... ok
test a_caller_who_may_not_read_the_repository_gets_not_found_and_no_all_event ... ok
test a_change_reaches_each_subscriber_only_with_the_parents_it_may_read ... ok
test a_filtered_subscription_delivers_the_filtered_event ... ok
test a_cursor_of_the_wrong_level_is_refused ... ok
test a_lag_hint_reaches_a_subscriber_whose_filter_drops_everything ... ok
test a_folder_too_deep_to_name_is_left_out_and_does_not_count ... ok
test a_folder_implied_only_by_hidden_or_deleted_files_is_not_listed ... ok
test a_library_lists_collections_then_virtual_collections_then_unfiled_folders ... ok
test a_collection_lists_the_folders_its_files_imply_and_never_a_file ... ok
test a_node_under_an_unreadable_ancestor_is_not_found ... ok
test an_all_change_never_decodes_as_false ... ok
test a_library_with_no_readable_child_reports_none ... ok
test a_page_through_the_router_carries_an_opaque_cursor_that_continues_the_listing ... ok
test a_virtual_collection_has_no_children_here ... ok
test more_than_64_changed_parents_become_all ... ok
test an_unusable_page_request_is_refused_as_invalid_request_and_journaled_once ... ok
test one_call_fills_the_breadcrumb_for_a_deep_folder ... ok
test only_readable_children_are_listed_and_counted ... ok
test node_handles_are_opaque_and_at_most_512_bytes ... ok
test paging_across_1001_siblings_repeats_nothing_and_leaves_no_gap ... ok
test the_default_page_holds_200_and_the_largest_1000 ... ok
test the_repository_lists_its_libraries_with_the_repository_as_ancestor ... ok
test the_root_level_is_the_hosts_one_repository ... ok
test the_topic_is_documented ... ok
test the_route_is_authenticated_and_declares_its_action ... ok
test without_an_installed_service_the_route_answers_unavailable ... ok
test unknown_unreadable_and_malformed_nodes_get_the_same_answer ... ok
test renaming_a_collection_publishes_one_change_naming_its_library ... ok
test has_children_looks_past_thousands_of_unreadable_siblings ... ok
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.07s
     Running tests/integration/versions.rs (/home/user/.cargo-target-shared/debug/deps/integration_versions-67839843b690b1bb)
test a_deleted_version_has_no_size ... ok
test a_later_label_commits_its_prov_o_record ... ok
test a_file_history_carries_size_digest_label_and_version_facts ... ok
test a_label_that_changes_nothing_is_refused_without_a_version ... ok
test a_restore_commits_its_prov_o_record_and_journals_once ... ok
test an_earlier_version_is_served_with_ranged_reads ... ok
test a_later_label_is_committed_as_application_data_and_shown ... ok
test history_size_counts_each_kept_content_of_a_file_once ... ok
test a_later_label_overrides_and_clears_a_commit_time_label ... ok
test library_history_size_counts_every_kept_content_once ... ok
test labels_are_bounded_and_checked ... ok
test restore_takes_a_label_not_a_free_message ... ok
test restore_reinstates_chosen_files_in_exactly_one_version_with_a_label ... ok
test the_contract_offers_no_version_deletion_expiry_or_thinning ... ok
test the_application_data_folder_is_reserved ... ok
test restore_of_every_file_of_a_version_is_a_revert ... ok
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
     Running tests/unit/auth.rs (/home/user/.cargo-target-shared/debug/deps/unit_auth-da25a00edfd2788a)
test a_host_route_cannot_be_made_public_through_the_guard ... ok
test a_present_but_unusable_authorization_header_is_not_a_missing_one ... ok
test a_not_permitted_denial_names_the_principal_and_the_action ... ok
test a_websocket_upgrade_may_carry_the_token_as_a_subprotocol ... ok
test action_names_are_unique_dotted_identifiers ... ok
test a_non_bearer_authorization_header_is_refused_as_invalid_credentials ... ok
test an_empty_bearer_token_is_presented_and_then_rejected ... ok
test an_error_without_a_reason_omits_the_member ... ok
test an_unusable_header_never_falls_back_to_the_subprotocol ... ok
test an_unusable_header_on_a_websocket_upgrade_is_refused_even_with_a_valid_subprotocol ... ok
test debug_output_never_shows_a_secret ... ok
test denials_map_reasons_to_codes_and_are_not_retryable ... ok
test an_authoriser_can_be_installed_only_once ... ok
test a_principal_the_policy_does_not_permit_is_forbidden ... ok
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
test unknown_paths_and_wrong_methods_authenticate_before_routing ... ok
test a_denial_never_echoes_the_presented_credential ... ok
test every_route_denies_a_call_without_the_per_launch_token ... ok
test without_an_installed_authoriser_every_route_is_denied ... ok
test every_route_denies_a_wrong_token ... ok
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
     Running tests/unit/contract.rs (/home/user/.cargo-target-shared/debug/deps/unit_contract-92c2b94d12c6c0e2)
test error_code_all_lists_every_variant ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test errors_carry_code_and_retryability ... ok
test every_component_schema_has_a_rust_sample ... ok
test event_topics_in_the_contract_match_the_rust_topics ... ok
test every_documented_operation_has_a_unique_operation_id ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
test every_error_response_carries_the_api_error_schema ... ok
test every_event_topic_documents_its_payload_schema ... ok
test every_operation_documents_both_denials ... ok
test every_limit_parameter_is_the_shared_page_size ... ok
test every_operation_requires_the_bearer_scheme_and_none_makes_it_optional ... ok
test every_operation_names_the_action_the_router_authorises ... ok
test health_schema_matches_the_core_payload_and_requires_every_field ... ok
test host_routes_are_the_event_channel_only ... ok
test query_envelope_and_page_size_parameter_are_bounded_like_core ... ok
test routed_and_documented_paths_agree_both_ways ... ok
test router_fallbacks_are_documented_as_wire_conventions ... ok
test router_serves_its_own_contract ... ok
test the_checker_accepts_only_truly_unconstrained_objects ... ok
test the_contract_uses_only_checked_schema_keywords ... ok
test the_only_public_operation_is_the_liveness_probe ... ok
test events_from_every_publisher_match_their_documented_payloads ... ok
test every_documented_operation_answers_a_documented_status_and_body ... ok
test router_fallbacks_answer_as_documented ... ok
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.88s
```

```text
$ cargo test -p filesquid-api --locked --test integration_drive --test features
Feature: Preferences, whole-file downloads and drive listings through the API
  Scenario: A user who saved nothing gets the default preferences
   ✔  Given a drive host that keeps preferences
   ✔  When the user reads their preferences
   ✔  Then the preferences should show the "list" layout and 0 pinned destinations
  Scenario: Saved preferences come back and the journal keeps none of their values
   ✔  Given a drive host that keeps preferences
   ✔  When the user saves the "grid" layout with one pinned collection
   ✔  Then the preferences should show the "grid" layout and 1 pinned destinations
   ✔  And the save should be journaled once without its values
  Scenario: A navigation choice over its bound is refused
   ✔  Given a drive host that keeps preferences
   ✔  When the user saves 101 pinned destinations
   ✔  Then the drive answer should be the error "invalid_request"
   ✔  And no preferences should be kept for the user
  Scenario: A file larger than one content answer downloads whole
   ✔  Given a drive file "films/reel.bin" larger than one content answer
   ✔  When the user streams "films/reel.bin" whole
   ✔  Then the stream should carry every byte as an attachment named "reel.bin"
  Scenario: Listings leave out the folders FileSquid keeps for itself
   ✔  Given a drive object holding "a.txt", "provenance/v1.jsonld" and ".filesquid/labels.json"
   ✔  When the user lists the drive object's root folder
   ✔  Then the drive listing should show only "a.txt"
   ✔  And listing inside "provenance" should not be found
  Scenario: A folder node lists its folder of the collection's object
   ✔  Given the "Photos" collection is served as one OCFL object
   ✔  When the user lists the folder node "2025"
   ✔  Then the drive listing should show "2025/summer" and "2025/winter"
  Scenario: A library node does not list as a folder
   ✔  Given the "Photos" collection is served as one OCFL object
   ✔  When the user lists the library node
   ✔  Then the drive answer should be the error "invalid_request"
(the other features' output is left out)
[Summary]
21 features
318 scenarios (207 passed, 111 skipped)
1037 steps (926 passed, 111 skipped)
```

The head-commit hook run (`FULL=1 bash scripts/pre-commit.sh`, `Test scope: full`) is recorded in the pull request.

## Mocked Environment Variables

None. The tests use real filesystem OCFL roots under the system temporary directory (`TMPDIR`) and remove them.
