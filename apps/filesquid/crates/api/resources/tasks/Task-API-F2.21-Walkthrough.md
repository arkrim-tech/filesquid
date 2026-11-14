# Task API-F2.21 Walkthrough: View and search row facts, the Recent view and destination identifiers

Task row: [API-F2.21](../plan.md). Global task: Task FS-2.5 ([implementation plan](../../../../../../resources/implementation-plan.md)). Owner decisions of 2026-10-31, verbatim: "accept recommendations" (recommendations 2 and 14) and "well do a" (history bytes, option (a)). Contract: 0.39.0 (0.37.0 went to API-F2.30, PR #180, and 0.38.0 to API-F2.27, PR #186).

## Summary of changes

- **Row facts on file rows.** Every file row of `GET /api/v1/files/view` and `GET /api/v1/files/search` carries `library_id`, `version` (the head version of its object), and `collection_id` with `collection_title`; both are `null` for a file whose home is no collection of its library. `src/files/drive.rs` (`FileRow`, `FilePlace`, `HomeCollection`), `drive_scope.rs` (`home_collections`), `search_scope.rs`, `drive_facts.rs` (the head version).
- **Folder counts.** Every folder row (a collection or virtual collection) carries `files`, `bytes` and `count_complete`, and a collection row its `collection_id`. The count is made for the folder rows of the answered page, after paging, so it never shapes a cursor; all the folder rows of one answer together count at most 10,000 files; a row the bound left no room for, or with a file that could not be read, says `count_complete: false`. New module `src/files/folder_counts.rs`.
- **Recent.** `view=recent` lists the files the caller changed, opened or uploaded, newest first, each row with `recent` (`action`: `changed`, `opened` or `uploaded`; `at`: RFC 3339 UTC). It reads only the caller's own succeeded journal entries, at most 20 pages per library, keeps the latest action per file, and leaves out hidden and deleted files and files in a collection the caller may not read. It takes the repository node, a library node or a collection node. New module `src/files/recent.rs`. The views home, personal, shares, favorites and tags are still refused.
- **Collection IDs on navigation nodes.** `NavigationNode` carries `collection_id`: the collection of a collection node and of a folder node inside one, `null` on every other kind.
- **Destination route.** `GET /api/v1/navigation/destination?node=<handle>` (action `navigation.read`) answers `library_id`, `collection_id` and `folder` (`null` for a collection node). The node and its ancestors are resolved before its kind is looked at, so an unknown node, a malformed handle, an unreadable node, a node under an unreadable ancestor and a folder that no longer exists all get the listing's one `not_found`; a readable node of another kind is `invalid_request`. A refusal is journaled once as `navigation.destination` with its code and no node, name or path.
- **History bytes.** `GET /api/v1/files/library-history-size` answers `history_bytes` beside `bytes`: the contents no head version holds, each once, within the same bounds and `truncated` flag.
- **Sub-codes.** `ApiError` has the optional member `sub_code` (`ErrorSubCode`): `destination_required`, `collection_not_found`, `path_suffix_exhausted`, `restore_name_exhausted`, `name_too_long` and `invalid_path`. It is set from the rule code the journal already records, on the file routes (upload, copy, move, folder), the import route and the Recently deleted commands, including their 409 command answers. The change is one optional field and one builder (`ApiError::with_rule`), so the later `operation_id` member of API-F2.33 adds beside it.
- **A defect fixed on the way.** A catalog file whose object root holds no OCFL object failed the whole view with `500 internal`, although the module said such a file is left out. It is now left out and the view says `complete: false` (`object_missing` in `drive_facts.rs`).
- **UI package.** `ui/src/api/schema.d.ts` and `contract-version.ts` are regenerated. The new members are required in the contract, so the navigation parser (`src/navigation/contract.ts`) and the fixture source (`src/navigation/source.ts`) pass `collection_id` through, and eight test fixtures name the new members. Nothing else in the UI changed; using the fields is UI-F2.20 and UI-F2.17.

## Readings made where the row left a choice

Each is recorded in [API-S-011](../srs.md). Owner decision 2026-11-14, verbatim: "Accept all three (Recommended)", the option "Merge as recorded. UI-F2.20 and UI-F2.17 build on these names next."

1. **Which rows are folder rows.** The view lists a library, collection or virtual-collection node, and its folder rows are collections and virtual collections. Folders inside a collection are navigation nodes, not view rows. So the counts are on collection and virtual-collection rows, and the feature file's scenario was reworded from a folder "Trips" inside the collection to the collection's own row.
2. **Which nodes Recent takes.** The repository (the sidebar's Recent), a library and a collection.
3. **Field names.** `sub_code`, `version`, `collection_title`, `files`, `bytes`, `count_complete`, `recent.action` and `recent.at`.

## SRS mapping

## API-S-011: View operation, preferences, copy, destination and storage use

Row facts, Recent, the destination sub-codes and the storage split. Tests: `integration_row_facts` (14), `integration_home_collections` (every refusal asserts its `sub_code`), `integration_recently_deleted` (`a_restore_with_no_free_untitled_title_carries_its_sub_code`), `unit_errors`, `unit_imports_route`, the unit tests of `drive.rs`, `recent.rs` and `folder_counts.rs`, and `row_facts_recent_and_destinations_API-S-011.feature` (22 scenarios).

## API-S-012: Search operation and operator grammar

Search rows carry the same facts and counts. Tests: `integration_row_facts` (`a_search_file_row_carries_the_same_facts`, `a_search_folder_row_carries_the_same_counts`, `a_file_registered_in_no_collection_carries_null_collection_facts`), `integration_search`.

## API-S-021: Navigation listing

`collection_id` on nodes and the destination route. Tests: `integration_navigation` (8 new tests), `unit_auth` (the route's action), `unit_contract`.

## API-S-014: Version history, labels, restore and history size

`history_bytes`. Tests: `integration_versions` (`history_bytes_count_only_content_no_head_version_holds_each_once`, `library_history_size_counts_every_kept_content_once`).

## API-S-009: Errors the UI can show

The sub-code is part of the typed error and of the 409 command answers. Tests: `unit_errors`, `unit_contract` (`ErrorSubCode`, `ApiError`, `FileCommandConflict`, `LibraryCommandConflict`).

## Redaction

`no_name_title_or_path_of_these_reads_reaches_the_journal` (`integration_row_facts`), `every_node_that_cannot_be_read_gets_the_listings_one_not_found` (`integration_navigation`) and the feature's last scenario assert that no file name, title, folder or path is journaled by these routes. A successful read is not journaled.

## Test-first record

The new tests were run before the code existed: `unit_errors` and `integration_navigation` did not compile (`SubCode`, `with_rule` and `destination` were missing), and all 14 tests of `integration_row_facts` failed. The `history_bytes` test was written with its implementation.

## Coverage

The repository sets no numeric coverage threshold for the Rust crates and installs no coverage tool, so none was run. Every new public function and every branch of the new modules is exercised by the tests listed above.

## Mocked Environment Variables

None.

## Sync Execution Report

- `crates/api/openapi.json` 0.39.0 and `ui/src/api/schema.d.ts`, `ui/src/api/contract-version.ts`.
- `crates/api/resources/srs.md`: API-S-011 names the wire members and records the three readings.
- `crates/api/resources/plan.md`, `resources/implementation-plan-checklist.md` and the MVP progress bar: API-F2.21 ticked.
- No PRD, ADR or README change: no new environment variable, service, command or dependency.

## Routes and members the UI can now use

- `GET /api/v1/navigation/destination?node=<handle>` (the route `importDestinationUnresolved` waits for).
- `collection_id` on navigation nodes; `library_id`, `version`, `collection_id`, `collection_title` on file rows; `collection_id`, `files`, `bytes`, `count_complete` on folder rows; `recent` on rows of `view=recent`.
- `history_bytes` on the library history size; `sub_code` on `ApiError`.

## Test output

Run from `apps/filesquid/`: `cargo test -p filesquid-api --locked --lib --test integration_row_facts --test integration_navigation --test integration_versions --test integration_home_collections --test integration_recently_deleted --test unit_errors --test unit_contract --test unit_imports_route --test unit_auth`.

```text
     Running tests/integration/row_facts.rs
running 14 tests
test a_deleted_row_keeps_its_library_and_version ... ok
test a_file_registered_in_no_collection_carries_null_collection_facts ... ok
test a_folder_row_counts_the_files_and_bytes_beneath_it ... ok
test a_search_file_row_carries_the_same_facts ... ok
test a_search_folder_row_carries_the_same_counts ... ok
test an_unreadable_file_marks_its_folder_count_incomplete ... ok
test a_view_file_row_names_its_library_head_version_and_home_collection ... ok
test recent_is_refused_for_nodes_it_does_not_cover_and_other_views_still_wait ... ok
test no_name_title_or_path_of_these_reads_reaches_the_journal ... ok
test recent_leaves_out_failed_actions_deleted_files_and_other_collections ... ok
test the_recent_view_never_shows_another_callers_actions ... ok
test recent_pages_stably_and_narrows_by_the_view_filter ... ok
test the_recent_view_lists_the_callers_own_actions_newest_first ... ok
test a_folder_count_stopped_by_the_view_bound_says_so ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/integration/navigation.rs
running 37 tests
test a_change_reaches_each_subscriber_only_with_the_parents_it_may_read ... ok
test a_caller_who_may_not_read_the_repository_gets_not_found_and_no_all_event ... ok
test a_collection_node_resolves_to_the_destination_an_upload_names ... ok
test a_filtered_subscription_delivers_the_filtered_event ... ok
test a_cursor_is_refused_at_another_parent_of_the_same_kind ... ok
test a_cursor_of_the_wrong_level_is_refused ... ok
test a_folder_node_resolves_to_its_collection_and_folder ... ok
test a_lag_hint_reaches_a_subscriber_whose_filter_drops_everything ... ok
test a_folder_too_deep_to_name_is_left_out_and_does_not_count ... ok
test a_collection_lists_the_folders_its_files_imply_and_never_a_file ... ok
test a_folder_implied_only_by_hidden_or_deleted_files_is_not_listed ... ok
test a_library_with_no_readable_child_reports_none ... ok
test a_node_under_an_unreadable_ancestor_is_not_found ... ok
test a_library_lists_collections_then_virtual_collections_and_no_recently_deleted_node ... ok
test an_all_change_never_decodes_as_false ... ok
test a_virtual_collection_has_no_children_here ... ok
test an_unreadable_node_or_one_under_an_unreadable_ancestor_is_the_same_not_found ... ok
test a_page_through_the_router_carries_an_opaque_cursor_that_continues_the_listing ... ok
test a_node_that_is_no_collection_or_folder_is_no_destination ... ok
test an_unusable_page_request_is_refused_as_invalid_request_and_journaled_once ... ok
test more_than_64_changed_parents_become_all ... ok
test collection_and_folder_nodes_carry_their_collection_id_and_no_other_kind_does ... ok
test every_node_that_cannot_be_read_gets_the_listings_one_not_found ... ok
test node_handles_are_opaque_and_at_most_512_bytes ... ok
test only_readable_children_are_listed_and_counted ... ok
test one_call_fills_the_breadcrumb_for_a_deep_folder ... ok
test the_default_page_holds_200_and_the_largest_1000 ... ok
test paging_across_1001_siblings_repeats_nothing_and_leaves_no_gap ... ok
test the_destination_route_needs_its_node_and_takes_no_other_parameter ... ok
test the_root_level_is_the_hosts_one_repository ... ok
test the_route_is_authenticated_and_declares_its_action ... ok
test the_topic_is_documented ... ok
test the_repository_lists_its_libraries_with_the_repository_as_ancestor ... ok
test unknown_unreadable_and_malformed_nodes_get_the_same_answer ... ok
test without_an_installed_service_the_route_answers_unavailable ... ok
test renaming_a_collection_publishes_one_change_naming_its_library ... ok
test has_children_looks_past_thousands_of_unreadable_siblings ... ok
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/integration/versions.rs
running 20 tests
test a_file_history_carries_size_digest_label_and_version_facts ... ok
test a_deleted_version_has_no_size ... ok
test a_later_label_commits_its_prov_o_record ... ok
test a_label_that_changes_nothing_is_refused_without_a_version ... ok
test a_folder_restore_brings_back_earlier_files_and_keeps_later_ones ... ok
test a_later_label_is_committed_as_application_data_and_shown ... ok
test an_object_that_cannot_be_opened_marks_the_library_size_truncated ... ok
test an_earlier_version_is_served_with_ranged_reads ... ok
test a_restore_commits_its_prov_o_record_and_journals_once ... ok
test history_bytes_count_only_content_no_head_version_holds_each_once ... ok
test a_later_label_overrides_and_clears_a_commit_time_label ... ok
test library_history_size_counts_every_kept_content_once ... ok
test history_size_counts_each_kept_content_of_a_file_once ... ok
test labels_are_bounded_and_checked ... ok
test restore_takes_a_label_not_a_free_message ... ok
test the_application_data_folder_is_reserved ... ok
test the_retention_check_catches_a_removal_under_any_method ... ok
test restore_reinstates_chosen_files_in_exactly_one_version_with_a_label ... ok
test restore_of_every_file_of_a_version_is_a_revert ... ok
test the_contract_offers_no_version_deletion_expiry_or_thinning ... ok
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/unit/errors.rs
running 13 tests
test a_rule_refusal_carries_its_sub_code_beside_status_and_code ... ok
test admin_routes_before_the_monitor_starts_are_unavailable_and_retryable ... ok
test a_wrong_method_is_a_method_not_allowed_api_error ... ok
test an_error_without_a_rule_code_has_no_sub_code_member ... ok
test api_error_takes_retryability_from_its_code ... ok
test internal_errors_keep_the_detail_out_of_the_message ... ok
test api_error_renders_as_json_with_the_code_status ... ok
test a_failing_store_answers_a_redacted_internal_error_through_the_router ... ok
test missing_and_wrong_credentials_are_typed_errors ... ok
test an_unknown_api_path_is_a_not_found_api_error ... ok
test the_export_route_ignores_limit_because_it_does_not_document_it ... ok
test the_router_keeps_answering_after_a_failed_request ... ok
test invalid_and_malformed_query_parameters_are_invalid_requests ... ok
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running unittests src/lib.rs
test result: ok. 82 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/integration/home_collections.rs
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/integration/recently_deleted.rs
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/unit/auth.rs
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/unit/contract.rs
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests/unit/imports_route.rs
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The cucumber harness (`cargo test -p filesquid-api --locked --test features`) ran the 22 scenarios of this task's feature with no skipped step:

```text
Feature: Row facts, the Recent view and destination identifiers
  Scenario: A view file row names its library, head version and home collection
  Scenario: A search file row carries the same facts
  Scenario: A folder row counts the files and bytes beneath it
  Scenario: Collection and folder navigation nodes carry their collection ID
  Scenario: A collection node resolves to the destination an upload names
  Scenario: A folder node resolves to its collection and folder
  Scenario Outline: A node that cannot be read gets the listing's one not_found answer
  Scenario Outline: A node that cannot be read gets the listing's one not_found answer
  Scenario Outline: A node that cannot be read gets the listing's one not_found answer
  Scenario Outline: A node that cannot be read gets the listing's one not_found answer
  Scenario: The Recent view lists the caller's own actions newest first
  Scenario: The library's storage total names the bytes only older versions keep
  Scenario: Content shared by two older versions counts once as history
  Scenario: A file in no collection carries null collection facts
  Scenario: A folder count stopped by a bound says so
  Scenario Outline: A destination refusal carries its rule code as a sub-code
  Scenario Outline: A destination refusal carries its rule code as a sub-code
  Scenario: An upload whose numbers are all taken carries path_suffix_exhausted
  Scenario: A node that is not a collection or folder is no destination
  Scenario: A restore with no free untitled collection name carries its sub-code
  Scenario: The Recent view never shows another caller's actions
  Scenario: The path a numbered upload used is answered and kept out of the journal

      Step skipped: tests/features/zip_download_and_history_object_API-S-028.feature:87:5
[Summary]
47 features
571 scenarios (393 passed, 178 skipped)
2026 steps (1848 passed, 178 skipped)
```

The 178 skipped scenarios belong to other features whose tasks have not landed.

The UI package, from `apps/filesquid/ui/`: `npx tsc --noEmit -p .` and `npx eslint .` exit 0, and `npm test` reports:

```text
 Test Files  122 passed (122)
      Tests  2818 passed | 1 skipped (2819)
```

## Hook run

The head-commit run required for a `rust:` scope is `FULL=1 bash scripts/pre-commit.sh`. Its summary line (`Test scope: full`) is recorded in the pull request.
