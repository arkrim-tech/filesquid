# Task API-F2.15 Walkthrough: Recently deleted filter, restore, retention and expiry settings, and retirement of "unfiled"

Task row: [API-F2.15](../plan.md). Global task: Task FS-2.5 ([implementation plan](../../../../../../resources/implementation-plan.md)). Decisions: [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md) Decisions 8 to 12; owner decision 2026-10-31 ("accept recommendations": every Recently deleted operation is journaled with opaque IDs only, API-S-036).

## Summary of changes

- **One Recently deleted (ADR 041 Decision 8).** The repository lists one `recently_deleted` node after its libraries (`name` null, `has_children` false; kind order library, collection, virtual_collection, recently_deleted, folder). `GET /api/v1/files/view` over it lists the deleted files of every library this host has opened that the caller may read, with no folder rows; each file row carries `deleted`: library ID and name, `deleted_at`, `purge_after` under its own library's retention, `days_left`, the deletion cause (`one_by_one`, `with_collection`), the home collection, its title (kept after that collection is deleted) and the folder. `files/listing` refuses the node; no library lists a node of its own; pins refuse it.
- **"Unfiled" retired.** `FileScope::Unfiled`, `FolderScope::Unfiled` and `is:unfiled` are gone (an unknown operator, `invalid_request`); a library node's `all` view shows only its collections and virtual collections; `is:deleted` finds files deleted one by one and with their collection, in every readable library.
- **Delete collection.** Deleting a collection deletes its undeleted files in the same command and version with the cause `with_collection`; every file of the collection keeps its title (`removed_home_title`, in the snapshot entity and the index), its object, logical path and home collection ID.
- **Restore (Decision 9).** A restore returns a file in place; when its home collection is gone it creates "untitled collection", "untitled collection 2", ... (first free title, at most `MAX_UNTITLED_TITLES` = 9,999, one per gone home and command) and re-homes the files there (`CatalogChange::RehomeFile`) in one journaled operation and one snapshot version; exhausted titles are `409 conflict` with the journaled code `restore_name_exhausted` and no change. The answer names the created collections (`restored_into`).
- **Retention (Decision 10).** `LibraryCommand::SetLibraryRetention` (`library.retention.set`, 1 to 3,650 days, `invalid_retention`) is kept in the snapshot's `library.json` and the index; the installation default lives behind the `RetentionSettings` port (`MemorySettings`; on the desktop `SqliteRetentionSettings` in `filesquid.sqlite`), 30 days by default; `purge_after` is taken at deletion time and never moved.
- **Expiry (Decision 11).** `LibraryService::expire_library` commits the expiry of the due files of one library as one `library.file.expire` operation (journal entry: library and file IDs only) with `CatalogChange::ExpireFile`, which records each file's object work in the same transaction (`pending_purges`, catalog schema v5); the host's `ObjectPurger` then deletes the object (`delete_all_version_history`) or rewrites it as a tombstone (`keep_version_history`, the default). Failed object work is journaled `library.file.expire.object` `failed` and retried by the next run. On the desktop the runs are durable `library.expiry` jobs, at launch and hourly.
- **Tombstone.** `filesquid_api::canonical::tombstone` owns the layout: `tombstone/history.json` (`filesquid.tombstone.v1`: every former version's number, created, user, message and each file's path, digest and size without bytes; the `purge` and `tombstone` events with time, actor, operation ID and PROV-O activity; the purge record) and the former PROV-O files under `tombstone/provenance/`; version 1 is the tombstone's creation, operation kind `tombstone` (added to `filesquid-core`'s `OperationKind`), with its own PROV-O record. The desktop catalog saves the payload in its work directory outside the storage root, deletes the original's stored files, creates the tombstone and cleans up, so a crash at any step is completed by the next run; a rebuild counts a tombstone as purged history, never as an unregistered file.
- **Routes.** `POST /api/v1/recently-deleted/commands` (`files.write`; delete, delete collection, restore, delete now, library retention), `GET` and `PUT /api/v1/settings/recently-deleted` (`library.read`; the PUT is admin-only, checked against the new action `settings.recently_deleted.configure`). Contract 0.36.0; `ui/src/api/schema.d.ts` regenerated, with the two kind maps of the UI given the new kind (its label `navigationRecentlyDeleted`, icon `trash`).

## SRS mapping

## API-S-019: Library domain commands and queries

Delete with the collection, one Trash, restore into an untitled collection, retention command and installation settings, expiry and object work, routes. Tests: `unit_recently_deleted` (14), `integration_recently_deleted` (6), `recently_deleted_API-S-019.feature` (8 scenarios), native `unit_recently_deleted` (8) and `unit_recently_deleted_settings` (1), desktop `recently_deleted_settings_and_the_expiry_job_are_wired`.

## API-S-021: Navigation listing

One `recently_deleted` node, last, unnamed, without children, with its breadcrumb and paging. Tests: `integration_navigation` (`the_repository_lists_its_libraries_with_the_repository_as_ancestor`), `navigation_listing_API-S-021.feature`, desktop `the_navigation_lists_the_installations_repository_and_its_libraries`.

## API-S-012: Search operation and operator grammar

`is:unfiled` refused as an unknown operator; `is:deleted` across libraries. Tests: `integration_search`, `integration_recently_deleted`, `search_operation_API-S-012.feature`.

## API-S-026: FileSquid OCFL application profile

The file entity's deletion cause and removed home title; the tombstone layout; tombstones are not unregistered files. Tests: native `the_cause_and_the_removed_home_title_survive_a_rebuild`, `keep_version_history_rewrites_the_object_as_a_tombstone_and_journals_the_purge`.

## API-S-033: Durable job model

The `library.expiry` job. Tests: native `the_expiry_runs_as_a_durable_job_over_every_open_library`, desktop wiring test.

## CORE-S-010: PROV-O history contract

Operation kind `tombstone`. Test: core `unit_history`.

## DE-S-014: Durable jobs and checkpoint-on-quit

Settings in the operational store; the hourly expiry job. Tests: native `unit_recently_deleted_settings`, desktop wiring test.

## Redaction

Every new journal operation (`library.retention.set`, `library.file.expire`, `library.file.expire.object`, `file.tombstone`, `settings.recently_deleted.set`, `recently_deleted.command`) carries opaque IDs only: checked by `the_command_route_takes_only_trash_commands_in_their_own_scope`, `settings_default_survive_a_reopen_and_are_journaled`, `keep_version_history_rewrites_the_object_as_a_tombstone_and_journals_the_purge` and the feature step "the purge should be journaled once with opaque IDs only".

## Mocked Environment Variables

None.

## Test output

### filesquid-api (unit, integration, contract)

```text
     Running tests/integration/navigation.rs (/root/.cargo-target-shared/debug/deps/integration_navigation-4958add55f8cf455)
test a_change_reaches_each_subscriber_only_with_the_parents_it_may_read ... ok
test a_caller_who_may_not_read_the_repository_gets_not_found_and_no_all_event ... ok
test a_cursor_is_refused_at_another_parent_of_the_same_kind ... ok
test a_filtered_subscription_delivers_the_filtered_event ... ok
test a_folder_too_deep_to_name_is_left_out_and_does_not_count ... ok
test a_cursor_of_the_wrong_level_is_refused ... ok
test a_lag_hint_reaches_a_subscriber_whose_filter_drops_everything ... ok
test a_folder_implied_only_by_hidden_or_deleted_files_is_not_listed ... ok
test a_library_with_no_readable_child_reports_none ... ok
test a_library_lists_collections_then_virtual_collections_and_no_recently_deleted_node ... ok
test a_node_under_an_unreadable_ancestor_is_not_found ... ok
test an_all_change_never_decodes_as_false ... ok
test a_virtual_collection_has_no_children_here ... ok
test a_collection_lists_the_folders_its_files_imply_and_never_a_file ... ok
test a_page_through_the_router_carries_an_opaque_cursor_that_continues_the_listing ... ok
test more_than_64_changed_parents_become_all ... ok
test node_handles_are_opaque_and_at_most_512_bytes ... ok
test an_unusable_page_request_is_refused_as_invalid_request_and_journaled_once ... ok
test only_readable_children_are_listed_and_counted ... ok
test one_call_fills_the_breadcrumb_for_a_deep_folder ... ok
test paging_across_1001_siblings_repeats_nothing_and_leaves_no_gap ... ok
test the_repository_lists_its_libraries_with_the_repository_as_ancestor ... ok
test the_root_level_is_the_hosts_one_repository ... ok
test the_route_is_authenticated_and_declares_its_action ... ok
test the_topic_is_documented ... ok
test the_default_page_holds_200_and_the_largest_1000 ... ok
test unknown_unreadable_and_malformed_nodes_get_the_same_answer ... ok
test without_an_installed_service_the_route_answers_unavailable ... ok
test renaming_a_collection_publishes_one_change_naming_its_library ... ok
test has_children_looks_past_thousands_of_unreadable_siblings ... ok
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.85s
     Running tests/integration/recently_deleted.rs (/root/.cargo-target-shared/debug/deps/integration_recently_deleted-308f9557ef7f2454)
test the_command_route_takes_only_trash_commands_in_their_own_scope ... ok
test a_restore_whose_home_is_gone_names_the_untitled_collection_it_created ... ok
test a_library_the_caller_may_not_read_is_left_out_of_recently_deleted ... ok
test the_settings_answer_the_default_the_expiry_setting_and_each_library_s_period ... ok
test the_node_is_refused_by_the_folder_listing_and_found_by_is_deleted ... ok
test recently_deleted_lists_the_deleted_files_of_every_library_with_their_own_retention ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/unit/contract.rs (/root/.cargo-target-shared/debug/deps/unit_contract-9d2af10e8a3ee2c6)
test error_code_all_lists_every_variant ... ok
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
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
test events_from_every_publisher_match_their_documented_payloads ... ok
test every_documented_operation_answers_a_documented_status_and_body ... ok
test router_fallbacks_answer_as_documented ... ok
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.58s
     Running tests/unit/recently_deleted.rs (/root/.cargo-target-shared/debug/deps/unit_recently_deleted-5f9f3cd0325b5077)
test a_library_retention_sets_purge_after_and_never_moves_a_deleted_file ... ok
test a_restore_into_a_home_collection_that_exists_returns_the_file_in_place ... ok
test a_failed_object_purge_is_journaled_as_failed_and_retried_at_the_next_run ... ok
test a_file_deleted_on_its_own_is_deleted_one_by_one_under_the_default_retention ... ok
test a_run_with_nothing_due_changes_and_journals_nothing ... ok
test a_retention_outside_one_to_3650_days_is_refused ... ok
test a_restore_whose_home_is_gone_creates_an_untitled_collection_once_per_home ... ok
test delete_all_version_history_asks_for_the_object_s_deletion ... ok
test deleting_a_collection_deletes_its_files_with_it_and_keeps_its_title ... ok
test installation_settings_default_to_30_days_and_keep_version_history ... ok
test expiry_removes_due_files_from_every_view_and_records_their_object_work ... ok
test the_installation_default_applies_to_a_library_without_its_own_period ... ok
test the_first_free_title_is_used ... ok
test a_restore_is_refused_when_every_untitled_title_is_taken ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

### filesquid-api BDD (`recently_deleted_API-S-019.feature`)

```text
Feature: One Recently deleted, restore, retention and expiry
   ✔  Given a library with a collection "Site visits" holding 2 files
   ✔  When the owner deletes the first file on its own
   ✔  And the owner deletes the collection "Site visits"
   ✔  Then the first file should be deleted one by one
   ✔  And the second file should be deleted with its collection
   ✔  And every deleted file should remember the collection title "Site visits"
   ✔  Given a library with a collection "Site visits" holding 1 file
   ✔  And a collection titled "untitled collection" already exists
   ✔  And the owner deletes the collection "Site visits"
   ✔  When the owner restores the file
   ✔  Then the file should be restored into a new collection titled "untitled collection 2"
   ✔  And it should keep its place in its object
   ✔  Given a library with a collection "Site visits" holding 1 file
   ✔  And the owner deletes the collection "Site visits"
   ✔  And every untitled collection title up to 9999 is taken
   ✔  When the owner restores the file
   ✔  Then the command should be refused as "conflict" with the code "restore_name_exhausted"
   ✔  And the file should still be deleted
   ✔  Given a library with a collection "Trips" holding 2 files
   ✔  And the owner deleted the first file a day earlier
   ✔  When the owner sets the library's retention to 90 days
   ✔  And the owner deletes the second file on its own
   ✔  Then the first file should keep its earlier deletion times
   ✔  And the second file should be purged 90 days after deletion
   ✔  Given a library with a collection "Trips" holding 1 file
   ✔  When the owner sets the library's retention to 0 days
   ✔  Then the command should be refused as "invalid_request" with the code "invalid_retention"
   ✔  Given a library with a collection "Trips" holding 1 file
   ✔  When the owner sets the library's retention to 3651 days
   ✔  Then the command should be refused as "invalid_request" with the code "invalid_retention"
   ✔  Given a library with a collection "Trips" holding 2 files
   ✔  And the owner has deleted every file
   ✔  When the expiry runs 31 days later under "keep_version_history"
   ✔  Then no file should be listed in the deleted view
   ✔  And each expired file's object should become a tombstone
   ✔  And the purge should be journaled once with opaque IDs only
   ✔  Given a library with a collection "Trips" holding 1 file
   ✔  And the owner has deleted every file
   ✔  And the next object purge fails
   ✔  When the expiry runs 31 days later under "delete_all_version_history"
457 scenarios (371 passed, 86 skipped)
1854 steps (1768 passed, 86 skipped)
```

### filesquid-native

```text
     Running tests/unit/recently_deleted.rs (/root/.cargo-target-shared/debug/deps/unit_recently_deleted-fde77deb07daea27)
test a_restore_whose_home_is_gone_creates_an_untitled_collection_in_one_version ... ok
test a_library_retention_command_is_kept_in_the_snapshot_and_journaled ... ok
test a_deletion_stopped_part_way_is_finished_by_the_next_run ... ok
test delete_all_version_history_deletes_the_object ... ok
test keep_version_history_rewrites_the_object_as_a_tombstone_and_journals_the_purge ... ok
test the_cause_and_the_removed_home_title_survive_a_rebuild ... ok
test the_expiry_runs_as_a_durable_job_over_every_open_library ... ok
test a_crash_at_each_step_of_the_tombstone_write_is_completed_by_the_next_run ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s
     Running tests/unit/recently_deleted_settings.rs (/root/.cargo-target-shared/debug/deps/unit_recently_deleted_settings-1fb3e05c22a5096e)
test settings_default_survive_a_reopen_and_are_journaled ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### filesquid-desktop

```text
     Running tests/integration/library_routes.rs (/root/.cargo-target-shared/debug/deps/integration_library_routes-285194a4905645ad)
test a_cancelled_or_unusable_pick_registers_nothing ... ok
test a_named_library_answers_its_facts_and_is_listed_as_recent_without_its_folder ... ok
test create_in_a_chosen_folder_answers_an_opaque_library_and_serves_its_files ... ok
test a_library_folder_copied_elsewhere_reopens_with_its_collections ... ok
test an_opened_folder_feeds_the_history_and_copies_and_rebuilds_without_paths ... ok
test a_rebuild_stopped_at_exit_resumes_from_its_checkpoint_when_its_library_opens_again ... ok
test recently_deleted_settings_and_the_expiry_job_are_wired ... ok
test the_data_directory_keeps_only_each_library_s_index_never_its_files ... ok
test recent_libraries_and_their_facts_survive_a_restart_without_a_path ... ok
test the_navigation_lists_the_installations_repository_and_its_libraries ... ok
test the_owners_preferences_are_kept_in_the_operational_store ... ok
test the_owner_picks_a_folder_in_the_hosts_dialog_and_the_app_sees_only_its_id ... ok
test the_owners_consented_repair_is_kept_in_the_persistent_ledger ... ok
test the_backfill_route_runs_over_the_folders_the_owner_opened ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
```

### filesquid-core

```text
     Running tests/unit/history.rs (/root/.cargo-target-shared/debug/deps/unit_history-b97d00bdec4538cd)
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Hook run

`FULL=1 bash scripts/pre-commit.sh` on the head commit of the pull request:

```text
✅ All Pre-Commit Hooks Passed! Test scope: full
```
