# Task API-F1.11 Walkthrough: Navigation listing route

Module task [API-F1.11](../plan.md) of global [Task FS-2.1](../../../../../../resources/implementation-plan.md) (the global box stays open until [UI-F2.1b](../../../../ui/resources/plan.md) part 2 is done). Branch `feat/api-f1.11-navigation-listing`, from `main` at `bf4b4fb`. Dependencies CL-0.2, API-F1.3 and API-F1.7 were done.

**Status: done.** `GET /api/v1/navigation` (action `navigation.read`) and the `navigation.change` event are in the router and the contract (version 0.11.0, `schema.d.ts` regenerated), over the CL-0.2 catalog queries and an injected folder index. Part 2 of UI-F2.1b can consume it: the wire shape is the one the UI-F2.1a `NavigationSource` interface already has (`nodes`, `ancestors`, `next_cursor`; `node`, `kind`, `name`, `has_children`).

## API-S-021: Navigation listing: one level of the tree per call, ancestors for the breadcrumb, permission-filtered paging

Parents: [F-004 Collections, folders and navigation](../../../../../../resources/prd.md#f-004-collections-folders-and-navigation), [SRS-F-004 Collections, folders and navigation](../../../../../../resources/srs.md#srs-f-004-collections-folders-and-navigation), [F-056 Single FileSquid API](../../../../../../resources/prd.md#f-056-single-filesquid-api), [F-049 Product component terminology and item type](../../../../../../resources/prd.md#f-049-product-component-terminology-and-item-type).

| Constraint | Gate | State |
|---|---|---|
| `GET /api/v1/navigation`, action `navigation.read`, authenticated | `the_route_is_authenticated_and_declares_its_action`; `unit_auth` every-route loops; contract `x-filesquid-action` drift test | Met |
| No `node`: the host's one repository | `the_root_level_is_the_hosts_one_repository`; scenario "The root level is the host's repository" | Met |
| Repository lists libraries; library lists collections, then virtual collections, then unfiled folders; collection lists folders; folder lists subfolders; virtual collection nothing; never files | `the_repository_lists_its_libraries_*`, `a_library_lists_collections_then_virtual_collections_then_unfiled_folders`, `a_collection_lists_the_folders_its_files_imply_and_never_a_file`, `a_virtual_collection_has_no_children_here`; four scenarios | Met |
| A folder exists only when committed ordinary files imply it; hidden or deleted files imply none; a pending folder does not exist | `a_folder_implied_only_by_hidden_or_deleted_files_is_not_listed` (real OCFL root), `the_memory_index_leaves_out_hidden_and_deleted_files`; outline with `hidden` and `deleted` | Met |
| Node: opaque handle of at most 512 bytes with no path, storage key, OCFL content path, SQL or token; `kind`; `name`; `has_children` | `node_handles_are_opaque_and_at_most_512_bytes`; `every_kind_of_node_round_trips_through_its_handle`, `other_spellings_and_tampered_handles_are_refused`, `a_folder_whose_handle_would_pass_512_bytes_has_none`; scenario "Node handles are opaque" | Met. A folder nested so deep that its handle would pass 512 bytes cannot be named and is not listed; the SRS row now says so |
| Repository `name`: the configured name or `null` | `RepositoryInfo`; `the_root_level_is_the_hosts_one_repository` | Met |
| `ancestors`: repository down to and including the node | `one_call_fills_the_breadcrumb_for_a_deep_folder`; scenario "One call fills the breadcrumb" | Met |
| `limit` 1 to 1,000 (default 200), opaque cursor | `the_default_page_holds_200_and_the_largest_1000`, `a_page_through_the_router_carries_an_opaque_cursor_*` | Met |
| Stable order: fixed kind order, catalog order, folders by code-point order, no repeat or gap | `paging_across_1001_siblings_repeats_nothing_and_leaves_no_gap` (names include `É` and `a`, inserted in reverse), `the_memory_index_lists_subfolders_in_code_point_order_after_a_name`; scenario "Paging is stable across many siblings" | Met. The order never comes from a lexical or enum-derived kind sort: `Rank` is an explicit 0 to 3 |
| Malformed cursor or limit outside 1 to 1,000: `invalid_request` | `an_unusable_page_request_is_refused_as_invalid_request_and_journaled_once` (malformed cursor, 0, 1001, `abc`, `-1`, an unknown parameter), `a_cursor_of_the_wrong_level_is_refused`; outline with three examples | Met |
| Only readable nodes, filtered before paging; `has_children` only for readable children | `only_readable_children_are_listed_and_counted`, `a_library_with_no_readable_child_reports_none`; two scenarios. The scan examines at most 5000 children per call and ends the page with a continuing cursor, so a caller who can read few of many siblings still advances | Met |
| Unknown and unreadable node: the same `not_found` | `unknown_unreadable_and_malformed_nodes_get_the_same_answer` (identical bodies, also for a malformed handle), `a_node_under_an_unreadable_ancestor_is_not_found`; scenario "Unknown and unreadable nodes get the same answer" | Met |
| One meaning: libraries, collections and virtual collections from the API-S-019 queries, folders from the folder listing by logical-path prefix; no copy | `NavigationService` holds the `LibraryCatalog` and the `FolderIndex` port and nothing else; the integration tests run it over a `MemoryCatalog` and an `OcflFolders` index built on the API-F1.3 `FileService::listing` of real OCFL roots | Met |
| `navigation.change`: after a committed change that adds, removes, renames or moves a node, one event names 1 to 64 parent handles, or `all` | `renaming_a_collection_publishes_one_change_naming_its_library` (through `LibraryService`), `more_than_64_changed_parents_become_all`, `a_change_names_one_to_64_parents_or_all`; scenario "A change names the parents whose children changed" | Met for library commands (create, rename and delete of a library, collection or virtual collection publish; a hide, unhide, delete, restore or purge publishes `all`, because the folders it changes are not known). A file move or upload does not publish yet: see the hand-over below |
| A subscriber receives only the handles it may read, and no event when it may read none | `a_change_reaches_each_subscriber_only_with_the_parents_it_may_read`, `a_filtered_subscription_delivers_the_filtered_event` (`EventBus::subscription_filtered` and `NavigationService::event_filter`); scenario "Change events name only the parents a subscriber may read" | Met at the API; a host must open each client's subscription through the filter |
| Contract holds the route, parameters, node and page schemas and the topic | `openapi.json` 0.11.0; `unit_contract` drift tests (every operation, every schema sample, a real `navigation.change` publisher); `schema.d.ts` regenerated with `npm run contract:types` | Met |
| Refusals are journaled once, successes are not | `an_unusable_page_request_is_refused_as_invalid_request_and_journaled_once` (seven refusals, seven `navigation.list` entries, none with a node, name or path) | Met |

### Secondary SRS mapping

- **API-S-019: Library domain commands and queries.** Libraries, collections and virtual collections come from `LibraryCatalog::libraries`, `collections` and `virtual_collections` in the order those queries list them; `LibraryService::with_navigation` publishes after a commit.
- **API-S-005: Responses carry opaque handles, never raw filesystem paths or SQL.** Handles and cursors are encoded records, and the codec refuses any other spelling; the answer test checks the body for storage keys, content paths, SQL and the token.
- **API-S-004: Deny-by-default authorisation.** The route carries `Action::ReadNavigation`, so the router guards it; `unit_auth` covers it in its every-route loops.
- **API-S-017: Credential issues and refusals are journaled, successful calls are not.** Refusals are journaled once as `navigation.list` with their code; reads are not.
- **API-S-003: One bounded event channel.** `navigation.change` is a documented topic (`topic::ALL`, the contract's `EventTopic` and payload map); a client rereads after `events.resync`.
- **API-S-002: One versioned contract, with types generated from it.** The contract moved to 0.11.0 and `ui/src/api/schema.d.ts` was regenerated.
- **API-S-006: The application layer does no storage I/O.** `FolderIndex` and `NavigationAccess` are injected ports; this module opens no file and no database.
- **OCFL-S-012: Folder listing by logical-path prefix.** The real-root tests derive folders from `FileService::listing`.

## What changed

- `crates/api/src/navigation/`: `NodeRef` and its handle codec (`node.rs`), the opaque cursor (`cursor.rs`), the `FolderIndex` and `NavigationAccess` ports with `HostAccess` (`port.rs`), `MemoryFolderIndex` (`memory.rs`), `NavigationService` (`service.rs`), the route (`routes.rs`), `NavigationChange` and `NavigationPublisher` (`change.rs`).
- `library/announce.rs` and `LibraryService::with_navigation`: the navigation parents a committed command changed.
- `events.rs`: the `navigation.change` topic, `EventFilter` and `EventBus::subscription_filtered`.
- `auth/policy.rs`: `Action::ReadNavigation` (`navigation.read`). `lib.rs`: `ApiState::install_navigation` and `navigation()`, API version 0.11.0.
- `openapi.json` and `ui/src/api/schema.d.ts`: the route, the `NavigationNode`, `NavigationPage`, `NavigationKind` and `NavigationChange` schemas, the `node` parameter and the topic.
- Tests: `tests/integration/navigation.rs` (30), `tests/unit/navigation.rs` (10), the cucumber steps of `navigation_listing_API-S-021.feature` (17 scenarios, no longer skipped), and the existing contract, auth and event tests extended for the new route, action and topic.

## Hand-over to the hosts and the UI

- **Hosts (DE and SV tasks):** install the service with `ApiState::install_navigation(NavigationService::new(catalog, folder_index, access, RepositoryInfo))`, give `LibraryService` a `NavigationPublisher`, and open each client's event subscription with `EventBus::subscription_filtered(service.event_filter(principal))`. The folder index is the host's: folders by logical-path prefix over the ordinary files of a collection or of the unfiled files (the reference `MemoryFolderIndex` shows the contract). Until a host installs the service the route answers `503 unavailable`.
- **Not done here:** a file move, upload or restore through the API-F1.3 routes does not yet publish `navigation.change`, because the file service does not know which collection an object belongs to. `NavigationPublisher::publish` is ready for the host (or a later task) to call with the changed parents.
- **UI-F2.1b part 2:** `GET /api/v1/navigation?node=&cursor=&limit=` answers `{nodes, ancestors, next_cursor}`; refresh the named parents on `navigation.change`, the whole visible tree on `all` or `events.resync`. A page can hold fewer nodes than `limit` while `next_cursor` is present.

## Cubic round 1

Fifteen P2 findings. Fixed: a folder too deep to name is now left out of the listing and does not count toward `has_children` (it had made the listing fail with `internal`), and the SRS row says so; cursors are bound to their parent and a cursor with a non-catalog key is refused; `has_children` keeps scanning past unreadable siblings (bounded by 20 scans); the repository node is checked for readability and `all` goes only to a caller who may read it; `NavigationChange::All` can no longer say `false`; a lag hint bypasses a subscriber's filter; the catalog `Failed` detail reaches stdout; `MemoryFolderIndex` probes the sorted set instead of copying it; the `NavigationNode` contract sample is built from the Rust type; the move step says the API announces the move. Answered without a change: unfiltered subscriptions are documented as safe only while every principal reads everything (the `EventBus` rule now says a host MUST filter once one reads less), and a concurrent replay of one operation ID may publish a second event, a harmless hint that a catalog trait change would not be worth.

## Mocked Environment Variables

None. No environment variable, service or command was added.

## Test output

### Failing first (before the module existed)

```text
Compiling derive-getters v0.5.0
   Compiling pin-project-internal v1.1.13
   Compiling serde_path_to_error v0.1.20
   Compiling cucumber-codegen v0.23.0
   Compiling axum v0.8.9
   Compiling clap v4.6.7
   Compiling junit-report v0.9.1
   Compiling pin-project v1.1.13
   Compiling tracing-subscriber v0.3.23
   Compiling filesquid-prov v0.1.0 (/home/user/filesquid/apps/filesquid/crates/prov)
   Compiling ref-cast v1.0.27
   Compiling futures v0.3.34
   Compiling globwalk v0.9.1
   Compiling filesquid-core v0.1.0 (/home/user/filesquid/apps/filesquid/crates/core)
   Compiling smart-default v0.7.1
   Compiling cucumber v0.23.0
   Compiling filesquid-api v0.1.0 (/home/user/filesquid/apps/filesquid/crates/api)
error[E0432]: unresolved import `filesquid_api::navigation`
 --> crates/api/tests/unit/navigation.rs:5:20
  |
5 | use filesquid_api::navigation::{
  |                    ^^^^^^^^^^ could not find `navigation` in `filesquid_api`

For more information about this error, try `rustc --explain E0432`.
error: could not compile `filesquid-api` (test "unit_navigation") due to 1 previous error
```

### Navigation tests

```text
     Running tests/integration/navigation.rs (/home/user/.cargo-target-shared/debug/deps/integration_navigation-6774f832b01d878f)
test a_change_reaches_each_subscriber_only_with_the_parents_it_may_read ... ok
test a_caller_who_may_not_read_the_repository_gets_not_found_and_no_all_event ... ok
test a_cursor_is_refused_at_another_parent_of_the_same_kind ... ok
test a_filtered_subscription_delivers_the_filtered_event ... ok
test a_cursor_of_the_wrong_level_is_refused ... ok
test a_lag_hint_reaches_a_subscriber_whose_filter_drops_everything ... ok
test a_folder_too_deep_to_name_is_left_out_and_does_not_count ... ok
test a_collection_lists_the_folders_its_files_imply_and_never_a_file ... ok
test a_folder_implied_only_by_hidden_or_deleted_files_is_not_listed ... ok
test a_library_with_no_readable_child_reports_none ... ok
test a_node_under_an_unreadable_ancestor_is_not_found ... ok
test an_all_change_never_decodes_as_false ... ok
test a_library_lists_collections_then_virtual_collections_then_unfiled_folders ... ok
test a_virtual_collection_has_no_children_here ... ok
test an_unusable_page_request_is_refused_as_invalid_request_and_journaled_once ... ok
test a_page_through_the_router_carries_an_opaque_cursor_that_continues_the_listing ... ok
test more_than_64_changed_parents_become_all ... ok
test one_call_fills_the_breadcrumb_for_a_deep_folder ... ok
test node_handles_are_opaque_and_at_most_512_bytes ... ok
test only_readable_children_are_listed_and_counted ... ok
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
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.86s
test health_uses_the_injected_host_checks ... ok
test admin_routes_report_unavailable_before_the_monitor_starts ... ok
test the_desktop_owner_reaches_admin_routes_with_the_per_launch_token ... ok
test processes_lists_registered_processes ... ok
test admin_routes_need_the_admin_token_on_the_server ... ok
test journal_query_filters_pages_and_exports_json_lines ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test a_host_route_cannot_be_made_public_through_the_guard ... ok
test a_present_but_unusable_authorization_header_is_not_a_missing_one ... ok
test a_non_bearer_authorization_header_is_refused_as_invalid_credentials ... ok
test a_not_permitted_denial_names_the_principal_and_the_action ... ok
test action_names_are_unique_dotted_identifiers ... ok
test a_websocket_upgrade_may_carry_the_token_as_a_subprotocol ... ok
test an_empty_bearer_token_is_presented_and_then_rejected ... ok
test an_error_without_a_reason_omits_the_member ... ok
test an_unusable_header_never_falls_back_to_the_subprotocol ... ok
test a_principal_the_policy_does_not_permit_is_forbidden ... ok
test debug_output_never_shows_a_secret ... ok
test denials_map_reasons_to_codes_and_are_not_retryable ... ok
test an_unusable_header_on_a_websocket_upgrade_is_refused_even_with_a_valid_subprotocol ... ok
test an_authoriser_can_be_installed_only_once ... ok
test every_denial_is_journaled_without_the_presented_credential ... ok
test a_denial_never_echoes_the_presented_credential ... ok
test every_route_answers_the_per_launch_token_holder ... ok
test every_routed_path_declares_its_action ... ok
test host_routes_are_denied_by_default ... ok
test launch_tokens_are_unique_256_bit_hex ... ok
test only_the_liveness_route_is_public ... ok
test the_bearer_scheme_is_read_case_insensitively ... ok
test the_host_policy_grants_the_owner_and_the_administrator_every_action ... ok
test fallback_refusals_are_journaled_with_the_no_route_target ... ok
test the_liveness_route_still_refuses_an_anonymous_wrong_method ... ok
test the_subprotocol_carrier_is_read_from_any_offered_entry ... ok
test the_liveness_route_answers_without_a_token_and_says_only_that_the_process_is_up ... ok
test the_subprotocol_token_counts_only_next_to_the_events_subprotocol ... ok
test the_token_authenticator_accepts_only_the_exact_token ... ok
test the_token_issued_entry_has_no_field_that_could_carry_a_secret ... ok
test unknown_paths_and_wrong_methods_authenticate_before_routing ... ok
test every_route_denies_a_call_without_the_per_launch_token ... ok
test without_an_installed_authoriser_every_route_is_denied ... ok
test every_route_denies_a_wrong_token ... ok
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
test a_commit_journal_a_replay_could_not_read_is_refused_before_publishing ... ok
test a_head_without_its_version_inventory_is_refused_and_nothing_is_written ... ok
test a_file_original_is_its_own_object_with_its_metadata_record ... ok
test a_provenance_only_message_names_no_receipt ... ok
test a_recorded_kind_is_written_and_read_back_by_both_readers ... ok
test a_root_without_a_snapshot_has_no_commit_journal ... ok
test a_metadata_record_without_its_original_is_refused_on_read ... ok
test a_storage_root_is_created_only_in_an_empty_folder ... ok
test a_committed_snapshot_reads_back_entity_for_entity ... ok
test a_stale_draft_left_in_staging_is_never_published ... ok
test an_entity_whose_identifier_cannot_name_a_path_is_a_decode_error ... ok
test an_entity_that_does_not_match_its_path_is_refused_on_read ... ok
test an_original_name_or_media_type_that_is_not_valid_is_refused ... ok
test an_unreadable_operation_message_is_refused_on_read ... ok
test canonical_failures_keep_their_meaning_at_the_catalog_port ... ok
test catalog_operations_map_to_history_kinds ... ok
test each_change_touches_the_library_and_its_own_entity ... ok
test every_entity_has_one_logical_path_and_back ... ok
test an_owner_name_round_trips_through_its_metadata_record ... ok
test identifiers_that_cannot_name_a_path_segment_are_refused ... ok
test the_metadata_record_is_readable_front_matter ... ok
test the_operation_message_carries_its_receipt ... ok
test the_stored_name_is_the_file_identifier_and_its_extension_only ... ok
test a_later_version_changes_only_the_entities_it_touches ... ok
test history_files_ride_the_snapshot_version_and_are_not_entities ... ok
test an_entity_file_larger_than_the_bound_is_refused_before_it_is_read ... ok
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
test error_code_all_lists_every_variant ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
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
test query_envelope_and_page_size_parameter_are_bounded_like_core ... ok
test host_routes_are_the_event_channel_only ... ok
test routed_and_documented_paths_agree_both_ways ... ok
test router_fallbacks_are_documented_as_wire_conventions ... ok
test router_serves_its_own_contract ... ok
test the_checker_accepts_only_truly_unconstrained_objects ... ok
test the_contract_uses_only_checked_schema_keywords ... ok
test the_only_public_operation_is_the_liveness_probe ... ok
test every_documented_operation_answers_a_documented_status_and_body ... ok
test events_from_every_publisher_match_their_documented_payloads ... ok
test router_fallbacks_answer_as_documented ... ok
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s
test a_wrong_method_is_a_method_not_allowed_api_error ... ok
test admin_routes_before_the_monitor_starts_are_unavailable_and_retryable ... ok
test api_error_renders_as_json_with_the_code_status ... ok
test internal_errors_keep_the_detail_out_of_the_message ... ok
test an_unknown_api_path_is_a_not_found_api_error ... ok
test api_error_takes_retryability_from_its_code ... ok
test a_failing_store_answers_a_redacted_internal_error_through_the_router ... ok
test the_export_route_ignores_limit_because_it_does_not_document_it ... ok
test missing_and_wrong_credentials_are_typed_errors ... ok
test invalid_and_malformed_query_parameters_are_invalid_requests ... ok
test the_router_keeps_answering_after_a_failed_request ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test a_resync_hint_carries_only_the_missed_count ... ok
test a_closed_bus_ends_the_subscription ... ok
test a_subscriber_exactly_one_buffer_behind_loses_nothing ... ok
test an_event_at_the_size_cap_is_delivered_unchanged ... ok
test dropping_a_subscription_frees_its_slot_and_its_receiver ... ok
test concurrent_subscribers_never_exceed_the_cap ... ok
test publishing_without_subscribers_is_not_an_error ... ok
test subscribers_receive_published_events ... ok
test the_documented_limits_have_their_declared_values ... ok
test the_subscriber_cap_refuses_one_more_with_a_retryable_error ... ok
test one_event_past_the_buffer_becomes_a_resync_hint_then_the_newest_events ... ok
test an_event_one_byte_over_the_size_cap_becomes_a_resync_hint ... ok
test a_slow_subscriber_cannot_block_publishers_or_grow_memory ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test unknown_api_path_is_not_found ... ok
test health_is_ok_and_names_filesquid ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test all_clear_is_ok_without_reasons ... ok
test error_rate_thresholds ... ok
test free_space_thresholds ... ok
test queue_fill_threshold ... ok
test open_ocfl_error_findings_degrade_health_with_codes_and_object_count ... ok
test unreachable_store_or_read_only_data_dir_fail ... ok
test worst_condition_wins_and_all_reasons_are_listed ... ok
test stalled_processes_degrade ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test a_history_entry_carries_the_label_and_marks_reconstruction ... ok
test a_memory_history_indexes_its_records_and_refuses_conflicts ... ok
test a_file_history_lists_its_object_only ... ok
test a_receipt_for_another_version_still_journals_the_commit ... ok
test a_lookup_that_skipped_an_unreadable_object_is_unavailable ... ok
test a_record_that_cannot_be_built_stops_the_commit ... ok
test a_commit_writes_its_record_and_journal_entries_in_the_same_version ... ok
test a_record_stored_under_another_version_is_corrupt ... ok
test a_takeover_with_an_allocated_id_rides_the_version ... ok
test a_takeover_adds_a_warning_after_the_commit_entry ... ok
test a_redacted_record_names_no_installation_message_or_path ... ok
test an_identical_record_inserted_again_is_held_once ... ok
test a_rebased_commit_regenerates_both_files_for_the_new_version ... ok
test a_truncated_history_is_flagged_on_pages_and_the_export ... ok
test an_ocfl_history_without_a_storage_root_is_unavailable ... ok
test a_version_record_is_raw_json_ld_or_not_found ... ok
test commit_meta_sets_the_user_and_the_operation_message ... ok
test every_new_journal_event_keeps_its_opaque_fields_and_loses_paths ... ok
test an_ocfl_history_past_its_record_cap_is_flagged_truncated ... ok
test an_ocfl_history_is_rebuilt_from_the_records_in_the_storage_root ... ok
test bad_history_parameters_answer_invalid_request ... ok
test history_routes_answer_unavailable_before_the_host_installs_a_source ... ok
test redaction_keeps_derivations_linked_and_paths_numbered_across_records ... ok
test the_version_user_names_the_installation_and_never_a_person ... ok
test the_export_is_a_json_ld_array_and_can_be_redacted ... ok
test the_operations_own_entries_ride_the_version_with_their_ids ... ok
test the_history_lists_everything_or_one_filter_in_cursor_pages ... ok
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test a_backfill_without_a_storage_root_fails_its_process ... ok
test a_cancelled_backfill_commits_nothing_and_ends_its_process_cancelled ... ok
test a_failed_object_past_the_budget_does_not_reach_the_cap ... ok
test a_backfill_heartbeats_between_staging_and_commit ... ok
test a_record_that_cannot_be_written_is_journaled_as_failed_and_the_repair_stands ... ok
test a_refused_object_gets_no_provenance_follow_up ... ok
test a_failing_object_is_journaled_and_the_run_goes_on ... ok
test a_backfill_stops_at_its_cap_and_the_next_run_continues ... ok
test a_backfill_reconstructs_every_missing_record_and_a_rerun_commits_nothing ... ok
test without_a_provenance_context_the_missing_record_is_journaled_and_answered ... ok
test a_repair_commits_a_provenance_record_under_the_run_id ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test a_blank_operator_name_is_no_operator_name ... ok
test a_host_describes_itself_once_and_the_first_description_stays ... ok
test a_missing_credential_is_refused ... ok
test a_service_host_error_never_contains_the_value ... ok
test a_server_describes_itself_with_its_operator ... ok
test an_enrolled_service_reports_backup_and_sync_separately ... ok
test an_operator_name_error_never_contains_the_value ... ok
test an_operator_name_holds_1_to_256_characters ... ok
test an_operator_name_is_trimmed ... ok
test an_operator_name_with_a_control_character_is_refused ... ok
test a_wrong_credential_is_refused ... ok
test an_unnamed_server_reports_a_null_operator ... ok
test before_a_host_describes_itself_the_route_is_unavailable ... ok
test every_accepted_credential_reads_it_even_one_granted_nothing_else ... ok
test host_change_is_a_documented_topic ... ok
test host_read_is_granted_to_every_authenticated_principal_whatever_the_policy ... ok
test only_a_server_identity_can_carry_an_operator ... ok
test reading_the_descriptor_is_not_journaled ... ok
test a_refresh_without_a_change_sends_nothing ... ok
test the_browser_host_describes_itself_with_no_operator ... ok
test the_default_port_reports_exactly_not_enrolled ... ok
test the_descriptor_has_exactly_its_documented_fields ... ok
test the_desktop_describes_itself_with_no_operator_and_not_enrolled ... ok
test a_client_that_reconnects_reads_the_current_descriptor_from_the_route ... ok
test the_desktop_owner_reads_it_with_the_per_launch_token ... ok
test the_route_is_host_read ... ok
test the_service_host_is_a_bare_dns_host_name ... ok
test the_service_host_is_stored_lowercase ... ok
test the_host_type_never_follows_a_request_header_or_address ... ok
test the_answer_and_the_event_carry_nothing_private ... ok
test each_change_sends_one_host_change_with_the_whole_descriptor_to_every_subscriber ... ok
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test a_forbidden_dependency_would_fail_the_graph_guard ... ok
test a_url_string_does_not_hide_io_after_it ... ok
test comments_and_string_literals_are_not_storage_io ... ok
test crate_is_named_filesquid_api_in_the_workspace ... ok
test every_rule_rejects_a_matching_name ... ok
test multi_line_grouped_imports_are_detected ... ok
test module_srs_states_the_application_layer_boundary ... ok
test package_names_parse_cargo_tree_lines ... ok
test rules_reject_every_named_crate ... ok
test rules_allow_the_modules_the_router_and_lookalikes ... ok
test storage_io_is_detected_with_its_line ... ok
test no_source_file_is_exempt_and_none_performs_storage_io ... ok
test every_feature_set_graph_is_isolated ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s
test a_commit_is_journaled_as_ocfl_commit_with_operation_and_object_version ... ok
test a_lease_takeover_is_journaled_naming_the_object_and_version ... ok
test identifying_values_in_the_new_fields_are_redacted ... ok
test an_entry_carries_its_operation_id_and_object_reference ... ok
test both_fields_are_stored_and_published_by_the_writer ... ok
test the_trace_layer_keeps_the_operation_id_and_object_reference ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test a_later_create_library_change_cannot_replace_a_library ... ok
test a_mixed_delete_batch_answers_the_times_of_the_files_it_newly_deletes ... ok
test a_failed_commit_leaves_the_memory_catalog_unchanged ... ok
test a_replayed_delete_answers_its_first_deletion ... ok
test a_repeated_operation_id_returns_the_first_outcome ... ok
test a_no_op_planned_on_a_revision_that_moved_on_is_a_conflict ... ok
test a_no_op_stores_no_receipt_journal_entry_or_version ... ok
test a_retried_no_op_is_planned_again_against_current_state ... ok
test a_stale_expected_revision_is_a_conflict_without_change ... ok
test adding_a_present_member_is_a_no_op_that_commits_nothing ... ok
test an_empty_raw_commit_stores_nothing_in_the_memory_catalog ... ok
test catalog_failures_are_journaled_exactly_once_with_the_journal_layer_installed ... ok
test catalog_failures_are_retryable_and_journaled_for_queries_too ... ok
test commands_round_trip_as_versioned_json ... ok
test create_library_mints_an_id_and_starts_at_revision_one ... ok
test create_library_needs_the_new_library_revision ... ok
test deleting_a_collection_retains_its_files_as_unfiled ... ok
test create_collection_needs_a_folder_of_its_library ... ok
test errors_map_to_api_and_port_errors ... ok
test deleting_a_virtual_collection_retains_its_members ... ok
test deletion_is_recoverable_for_thirty_days_and_restore_keeps_hidden ... ok
test every_commit_carries_one_redacted_journal_entry ... ok
test names_must_be_non_empty_and_bounded ... ok
test minted_identifiers_are_canonical_uuid_v4 ... ok
test permanent_deletion_needs_deleted_files_and_an_exact_confirmation ... ok
test membership_batches_keep_their_order_after_the_last_member ... ok
test references_to_another_library_are_not_found ... ok
test refused_commands_are_journaled_as_failed ... ok
test renaming_to_the_same_title_commits_nothing ... ok
test rule_based_virtual_collections_refuse_hand_picked_members ... ok
test uuid_check_rejects_other_shapes ... ok
test virtual_collection_membership_leaves_the_home_collection_alone ... ok
test file_batches_hold_one_to_one_thousand_distinct_files ... ok
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test deletion_policy_query_reports_thirty_days ... ok
test cursors_are_opaque_and_malformed_ones_are_refused ... ok
test answers_carry_no_filesystem_paths ... ok
test collections_and_virtual_collections_list_by_library ... ok
test listing_a_scope_of_another_library_is_not_found ... ok
test listing_libraries_pages_by_repository ... ok
test open_library_answers_its_record_and_revision ... ok
test opening_an_unknown_library_is_not_found ... ok
test ordinary_listing_excludes_hidden_and_deleted_files ... ok
test paging_is_stable_with_no_repeats_or_gaps ... ok
test the_last_page_has_no_cursor ... ok
test every_issued_cursor_decodes_and_only_canonical_text_is_accepted ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test copy_refuses_a_destination_that_is_not_empty ... ok
test copy_refuses_a_destination_that_gains_a_file_during_the_copy ... ok
test copy_keeps_version_sidecars_of_another_algorithm ... ok
test copy_refuses_a_snapshot_with_an_unpublished_head ... ok
test copy_reports_an_external_edit_found_in_the_copy ... ok
test create_refuses_a_folder_holding_only_an_empty_directory_or_link ... ok
test copy_produces_a_checked_snapshot_and_never_writes_the_source ... ok
test create_refuses_a_folder_that_is_not_empty ... ok
test create_twice_is_refused ... ok
test errors_carry_stable_codes_and_api_errors ... ok
test create_writes_only_the_storage_root_declaration ... ok
test open_refuses_a_folder_that_is_not_a_storage_root ... ok
test open_refuses_git_metadata_in_the_root ... ok
test open_reports_the_objects_and_writes_nothing ... ok
test rebuild_reports_an_external_edit_instead_of_adopting_it ... ok
test rebuild_reconstructs_from_files_and_repairs_nothing ... ok
test copy_refuses_a_large_source_file_that_changes_while_it_is_copied ... ok
test copy_streams_a_file_larger_than_one_put ... ok
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.27s
test prometheus_text_and_json_report_the_same_values ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/navigation.rs (/home/user/.cargo-target-shared/debug/deps/unit_navigation-ef3a3447a00556a1)
test a_decoded_folder_handle_with_a_traversal_segment_is_refused ... ok
test a_folder_path_that_is_not_a_logical_path_has_no_handle ... ok
test a_folder_whose_handle_would_pass_512_bytes_has_none ... ok
test a_handle_is_the_same_for_the_same_node_and_differs_between_nodes ... ok
test identifiers_with_separators_and_names_with_unicode_survive ... ok
test every_kind_of_node_round_trips_through_its_handle ... ok
test other_spellings_and_tampered_handles_are_refused ... ok
test a_change_names_one_to_64_parents_or_all ... ok
test the_memory_index_leaves_out_hidden_and_deleted_files ... ok
test the_memory_index_lists_subfolders_in_code_point_order_after_a_name ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test a_change_in_the_open_findings_is_pushed_on_health_change ... ok
test a_cancelled_run_stops_and_reports_the_rest ... ok
test a_failing_findings_store_fails_health_and_the_run ... ok
test a_finding_stays_open_until_a_later_validation_no_longer_reports_it ... ok
test a_library_is_due_one_interval_after_it_was_opened ... ok
test a_findings_change_journals_a_health_transition_once ... ok
test a_run_never_starts_twice_at_once_for_one_library ... ok
test a_run_registers_in_the_process_registry_with_progress ... ok
test a_run_request_starts_runs_and_a_malformed_one_is_refused ... ok
test a_run_stops_at_its_cap_and_reports_what_it_did_not_check ... ok
test a_run_stopped_by_an_outage_reports_what_it_checked_and_publishes_health ... ok
test a_takeover_check_on_a_damaged_object_opens_an_error_finding ... ok
test a_takeover_check_reads_only_the_named_object ... ok
test a_run_whose_record_cannot_be_stored_is_reported_as_failed ... ok
test a_takeover_commit_triggers_validation_of_that_object ... ok
test a_takeover_check_that_cannot_start_is_journaled ... ok
test a_w010_object_is_listed_as_a_warning_and_leaves_health_ok ... ok
test an_object_deleted_after_the_run_indexed_it_has_its_findings_closed ... ok
test an_e023_object_degrades_health_until_it_is_fixed_and_revalidated ... ok
test anyone_else_is_refused_and_the_refusal_is_journaled ... ok
test an_on_request_run_continues_from_the_cursor_unless_asked_to_start_over ... ok
test defaults_are_seven_days_and_one_hundred_thousand_objects ... ok
test an_e023_object_on_a_real_directory_is_found ... ok
test deleting_an_object_with_an_open_e023_closes_its_finding ... ok
test capped_runs_resume_from_the_cursor_in_object_id_order_and_wrap ... ok
test locations_are_shown_only_to_a_caller_who_may_read_the_object ... ok
test finding_entries_carry_the_object_version_reference ... ok
test journal_rows_and_health_reasons_carry_no_path_title_or_content ... ok
test start_runs_refuses_an_unknown_library_and_a_busy_one ... ok
test the_configuration_is_bounded_and_journaled ... ok
test the_findings_view_reports_the_last_run_and_the_next_scheduled_run ... ok
test the_owner_sees_locations_and_the_administrator_does_not ... ok
test the_settings_route_changes_the_schedule_and_refuses_bad_values ... ok
test the_scheduler_tick_starts_a_scheduled_run_when_one_is_due ... ok
test two_roots_that_declare_one_identifier_keep_separate_findings ... ok
test a_run_over_many_objects_journals_one_entry_per_finding_with_no_loss ... ok
test shutting_down_cancels_runs_and_refuses_new_ones ... ok
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
test rate_and_eta_follow_progress ... ok
test a_missed_heartbeat_marks_the_process_stalled_once ... ok
test registering_and_finishing_journals_state_changes ... ok
test dropping_an_unfinished_handle_cancels_it_and_failure_is_journaled_as_error ... ok
test finished_processes_are_pruned_when_they_finish_and_gauges_follow ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test a_command_that_breaks_a_library_rule_cannot_be_pending ... ok
test a_change_is_committed_only_by_a_fenced_head_publication ... ok
test a_conflict_blocks_later_changes_until_rebased_or_discarded ... ok
test a_changed_grant_supersedes_the_old_identity_only_for_the_same_viewer_and_library ... ok
test a_cached_page_answers_with_its_coverage_and_key ... ok
test a_head_publication_names_a_revision ... ok
test a_grant_change_makes_the_old_entries_unreachable ... ok
test a_miss_is_not_cached_and_never_an_empty_page ... ok
test a_new_change_is_queued_with_its_command_unchanged ... ok
test a_page_answer_type_round_trips_as_json ... ok
test a_publishing_change_can_be_neither_discarded_nor_rebased ... ok
test a_page_stored_for_another_identity_is_never_served ... ok
test a_repeated_operation_id_is_the_same_change ... ok
test a_retry_waits_then_publishes_again_and_stalls_after_the_attempt_bound ... ok
test an_evicted_entry_says_evicted ... ok
test a_superseded_identity_loses_its_entries_and_others_keep_theirs ... ok
test an_interrupted_last_attempt_restores_as_stalled ... ok
test cache_capacity_must_be_positive_and_has_a_default ... ok
test coverage_is_complete_only_when_every_shard_is_loaded ... ok
test create_library_cannot_be_pending_because_it_names_no_library ... ok
test debug_output_never_prints_cached_answers ... ok
test every_error_has_a_stable_code_and_a_message_without_names ... ok
test grant_digest_ignores_order_and_repeats_but_not_membership ... ok
test grant_digest_bounds_the_scope_list ... ok
test equal_identities_give_equal_keys_and_every_component_changes_the_key ... ok
test not_next_is_retryable_like_pending_full ... ok
test incomplete_coverage_is_visible_on_the_answer ... ok
test keys_and_identities_carry_no_name_or_path ... ok
test only_a_committed_change_can_be_acknowledged ... ok
test only_the_oldest_unfinished_change_may_begin_publishing ... ok
test queued_at_must_be_rfc3339_utc ... ok
test query_keys_differ_by_identity_page_size_cursor_and_query ... ok
test refreshing_an_entry_makes_it_the_newest ... ok
test repeating_the_original_request_after_a_rebase_answers_the_change ... ok
test restore_refuses_a_committed_change_without_a_published_revision ... ok
test restore_refuses_a_revision_on_an_uncommitted_change ... ok
test restoring_state_validates_keys_identities_and_coverage ... ok
test storing_again_after_eviction_recovers_the_entry ... ok
test the_cache_is_bounded_evicts_oldest_first_and_remembers_it ... ok
test the_eviction_memory_is_bounded_too ... ok
test the_key_encoding_is_not_ambiguous_across_field_boundaries ... ok
test the_number_of_libraries_with_pending_changes_is_bounded ... ok
test the_snapshot_restores_the_same_ledger_and_requeues_an_interrupted_publication ... ok
test unknown_operations_are_not_found ... ok
test viewer_ids_are_bounded_and_free_of_control_characters ... ok
test an_oversized_snapshot_is_refused_before_parsing ... ok
test pending_changes_are_bounded_per_library ... ok
test restore_refuses_duplicates_malformed_input_and_over_bound_snapshots ... ok
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
test fallbacks_answer_the_same_through_every_host_uri_form ... ok
test the_health_route_answers_through_the_desktop_scheme ... ok
test the_content_denial_answers_the_same_through_every_host_uri_form ... ok
test denials_answer_the_same_through_every_host_uri_form ... ok
test every_routed_path_answers_the_same_through_every_host_uri_form ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test a_held_reservation_is_a_retryable_refusal ... ok
test a_head_that_already_has_both_files_is_refused_with_nothing_to_repair ... ok
test a_damaged_interrupted_repair_is_refused_without_writing_the_sidecar ... ok
test a_root_sidecar_that_does_not_commit_the_root_inventory_is_refused ... ok
test a_consented_repair_by_the_owner_makes_a_sha256_object_committable ... ok
test a_second_repair_is_refused_with_nothing_to_repair ... ok
test an_interrupted_repair_is_resumed ... ok
test an_unrecordable_consent_repairs_nothing ... ok
test an_unlisted_file_in_the_head_is_neither_offered_nor_repaired ... ok
test an_unknown_object_is_refused_per_object ... ok
test an_unrecordable_outcome_still_reaches_the_journal ... ok
test candidates_say_whether_the_repair_opens_the_object_to_commits ... ok
test object_lists_are_bounded_and_distinct ... ok
test consent_and_one_outcome_row_per_object_are_journaled_without_paths ... ok
test consent_and_outcomes_share_one_repair_run_id ... ok
test open_copy_and_rebuild_never_reach_the_repair ... ok
test the_consent_statement_id_is_pinned ... ok
test the_consent_statement_names_what_changes ... ok
test the_command_is_refused_without_consent_and_writes_nothing ... ok
test the_command_is_refused_for_another_caller ... ok
test the_candidate_query_lists_heads_without_their_version_inventory ... ok
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test a_replayed_entry_the_store_already_holds_is_announced_once ... ok
test an_announcing_store_publishes_and_counts_what_the_store_accepted ... ok
test live_journal_events_carry_the_redacted_entry ... ok
test trace_layer_keeps_display_formatted_whitelisted_fields ... ok
test written_entries_are_published_on_the_event_bus_and_counted ... ok
test trace_layer_journals_warnings_without_free_text_or_unlisted_fields ... ok
test a_burst_of_ten_times_capacity_loses_nothing ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Navigation scenarios (cucumber)

```text
Feature: Navigation listing
  Scenario: The root level is the host's repository
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  When the owner lists the navigation tree without a node
   ✔  Then the answer should hold exactly one node of kind "repository"
  Scenario: A library lists its collections, virtual collections and unfiled folders in kind order
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  When the owner lists the children of library "Family"
   ✔  Then the kinds should be "collection", "virtual_collection" and "folder" in that order
   ✔  And the folder should be named "inbox"
  Scenario: A collection lists the folders its files imply
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  When the owner lists the children of folder "2025" in collection "Photos"
   ✔  Then the answer should hold folders "summer" and "winter"
   ✔  And no file should be listed
  Scenario: One call fills the breadcrumb
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  When the owner lists the children of folder "2025/summer" in collection "Photos"
   ✔  Then the ancestors should be the repository, "Family", "Photos", "2025" and "summer" in that order
  Scenario: A virtual collection has no children here
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  When the owner lists the children of virtual collection "Best of 2025"
   ✔  Then the answer should be empty
   ✔  And the virtual collection should report no children
  Scenario Outline: A folder implied only by hidden or deleted files is not listed
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  Given every file under "2025/winter" in collection "Photos" is hidden
   ✔  When the owner lists the children of folder "2025" in collection "Photos"
   ✔  Then the answer should hold only folder "summer"
  Scenario Outline: A folder implied only by hidden or deleted files is not listed
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  Given every file under "2025/winter" in collection "Photos" is deleted
   ✔  When the owner lists the children of folder "2025" in collection "Photos"
   ✔  Then the answer should hold only folder "summer"
  Scenario: Paging is stable across many siblings
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  Given collection "Photos" holds 1001 top-level folders
   ✔  When the owner pages through the children of collection "Photos" 200 at a time
   ✔  Then every folder should appear exactly once in a stable order
  Scenario: Node handles are opaque
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  When the owner lists the children of folder "2025" in collection "Photos"
   ✔  Then no node handle should contain a filesystem path, storage key, OCFL content path, SQL or token
   ✔  And no node handle should be longer than 512 bytes
  Scenario: A change names the parents whose children changed
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  Given a client subscribed to the event channel
   ✔  When the owner renames collection "Photos" to "Pictures"
   ✔  Then the client should receive one "navigation.change" event naming the handle of library "Family"
  Scenario: Change events name only the parents a subscriber may read
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  Given library "Family" also holds collection "Letters"
   ✔  And subscriber "Ana" may read library "Family", collection "Photos" and collection "Letters"
   ✔  And subscriber "Ben" may read library "Family" and collection "Photos" but not collection "Letters"
   ✔  And subscriber "Cy" may read nothing in library "Family"
   ✔  When the API announces one committed move of a folder from collection "Letters" into collection "Photos"
   ✔  Then "Ana" should receive one "navigation.change" event naming the handles of collections "Letters" and "Photos"
   ✔  And "Ben" should receive one "navigation.change" event naming only the handle of collection "Photos"
   ✔  And "Cy" should receive no "navigation.change" event
  Scenario Outline: An unusable page request is refused
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  When the owner lists the children of library "Family" with a malformed cursor
   ✔  Then the request should be refused as "invalid_request"
  Scenario Outline: An unusable page request is refused
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  When the owner lists the children of library "Family" with a limit of 0
   ✔  Then the request should be refused as "invalid_request"
  Scenario Outline: An unusable page request is refused
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  When the owner lists the children of library "Family" with a limit of 1001
   ✔  Then the request should be refused as "invalid_request"
  Scenario: Unknown and unreadable nodes get the same answer
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  Given a second user who may read library "Family" and virtual collection "Best of 2025" but not collection "Photos" or the unfiled files
   ✔  When that user lists the children of collection "Photos" and of a node that does not exist
   ✔  Then both answers should be the same "not_found" error
  Scenario: Only readable children are listed and counted
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  Given a second user who may read library "Family" and virtual collection "Best of 2025" but not collection "Photos" or the unfiled files
   ✔  When that user lists the children of the repository
   ✔  Then library "Family" should report has_children true
   ✔  When that user lists the children of library "Family"
   ✔  Then the answer should hold only virtual collection "Best of 2025"
  Scenario: A node with no readable child reports none
   ✔> Given a navigation fixture with library "Family" holding collection "Photos" and virtual collection "Best of 2025"
   ✔> And collection "Photos" holds files at logical paths "2025/summer/beach.jpg" and "2025/winter/snow.jpg"
   ✔> And library "Family" holds an unfiled file at logical path "inbox/note.txt"
   ✔  Given a third user who may read library "Family" and nothing inside it
   ✔  When that user lists the children of the repository
   ✔  Then library "Family" should report has_children false
   ✔  When that user lists the children of library "Family"
   ✔  Then the answer should be empty
```

### Whole filesquid-api suite

```text
     Running unittests src/lib.rs (/home/user/.cargo-target-shared/debug/deps/filesquid_api-d6810ce49c57588a)
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/features.rs (/home/user/.cargo-target-shared/debug/deps/features-1b07fe462b71d9a1)
[Summary]
18 features
291 scenarios (180 passed, 111 skipped)
924 steps (813 passed, 111 skipped)
     Running tests/integration/content_handles.rs (/home/user/.cargo-target-shared/debug/deps/integration_content_handles-6f0a04f7f5c2006d)
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.91s
     Running tests/integration/files.rs (/home/user/.cargo-target-shared/debug/deps/integration_files-9f166ac55e7a4816)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.62s
     Running tests/integration/navigation.rs (/home/user/.cargo-target-shared/debug/deps/integration_navigation-6774f832b01d878f)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.86s
     Running tests/unit/admin.rs (/home/user/.cargo-target-shared/debug/deps/unit_admin-e871abc0e5591625)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/auth.rs (/home/user/.cargo-target-shared/debug/deps/unit_auth-2556111050a7c4ab)
test result: ok. 34 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
     Running tests/unit/canonical.rs (/home/user/.cargo-target-shared/debug/deps/unit_canonical-7316e37a5407df42)
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/unit/contract.rs (/home/user/.cargo-target-shared/debug/deps/unit_contract-841ff359eb640360)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s
     Running tests/unit/errors.rs (/home/user/.cargo-target-shared/debug/deps/unit_errors-8709912a089457b6)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/events.rs (/home/user/.cargo-target-shared/debug/deps/unit_events-dfbbf2ea98b3cf4c)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/health.rs (/home/user/.cargo-target-shared/debug/deps/unit_health-6f669ca38b593d08)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/health_rules.rs (/home/user/.cargo-target-shared/debug/deps/unit_health_rules-a81b8bbfe550797a)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/history.rs (/home/user/.cargo-target-shared/debug/deps/unit_history-ec238ecffe53b83e)
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/history_jobs.rs (/home/user/.cargo-target-shared/debug/deps/unit_history_jobs-91cef2ebc00f37bf)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/host.rs (/home/user/.cargo-target-shared/debug/deps/unit_host-e0356a63790b92b9)
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/unit/isolation.rs (/home/user/.cargo-target-shared/debug/deps/unit_isolation-16ede18384f163af)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s
     Running tests/unit/journal_linkage.rs (/home/user/.cargo-target-shared/debug/deps/unit_journal_linkage-dcacaf0623da2e35)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/library.rs (/home/user/.cargo-target-shared/debug/deps/unit_library-711b0f7f7a9af7af)
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/library_queries.rs (/home/user/.cargo-target-shared/debug/deps/unit_library_queries-23dd5001d60a1201)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/local_library.rs (/home/user/.cargo-target-shared/debug/deps/unit_local_library-ec57982ecb7d8354)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.27s
     Running tests/unit/metrics.rs (/home/user/.cargo-target-shared/debug/deps/unit_metrics-cbbcd64e2dc708ad)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/navigation.rs (/home/user/.cargo-target-shared/debug/deps/unit_navigation-ef3a3447a00556a1)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/ocfl_validation.rs (/home/user/.cargo-target-shared/debug/deps/unit_ocfl_validation-20270be21ffe0270)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/unit/registry.rs (/home/user/.cargo-target-shared/debug/deps/unit_registry-d9b0235a997bb3ed)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/unit/replica.rs (/home/user/.cargo-target-shared/debug/deps/unit_replica-289d613b3e7571c9)
test result: ok. 48 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
     Running tests/unit/routes.rs (/home/user/.cargo-target-shared/debug/deps/unit_routes-e47a9431809dd4e3)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
     Running tests/unit/version_inventory_repair.rs (/home/user/.cargo-target-shared/debug/deps/unit_version_inventory_repair-1de5ffcec3512bf9)
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/unit/writer.rs (/home/user/.cargo-target-shared/debug/deps/unit_writer-fe51a6cff7b56eb7)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
