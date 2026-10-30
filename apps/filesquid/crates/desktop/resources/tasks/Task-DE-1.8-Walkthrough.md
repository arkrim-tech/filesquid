# Task DE-1.8 Walkthrough: Host side of the upload review, per-file progress, rebuild job and library facts

Branch `feat/de-1.8-host-upload-review-facts`. Module row [DE-1.8](../plan.md) of global [Task 1.2](../../../../../../resources/implementation-plan.md#task-1.2), which stays open for its UI rows (UI-1.2, UI-1.3). No contract change (no `openapi.json` or `API_VERSION` bump).

## What was already on main

| Part of the row | Done by |
|---|---|
| Free bytes of a registered folder's volume on Linux and macOS, `volume_unreadable` elsewhere | API-F2.18, `HostLocations::free_space` |
| Library name and last opened time for the host's lifetime | API-F2.18, `RegistryBook` |
| Rebuild as a `library.rebuild` job, checkpoint per object batch (64), runner installed | API-F2.17, desktop `install_job_routes` |
| Job controls, the `import` runner, the streamed stage | DE-1.9 |

## What changed

- **Review (API-S-036).** [`imports.rs`](../../src/imports.rs) implements `replay`, `plan`, `start_plan`, `discard` and `plan_items` over the API's `PlanBook`. [`native import/review.rs`](../../../native/src/import/review.rs) does the review: after the native dialog it lists the sources, makes the manifest the job will run and hashes each file once, staging nothing. It answers files found, new, already held (by SHA-256), bytes to copy, numbered names, free bytes after the copy (from the registry; `null` when unreadable) and the 10 GiB staging limit, and lists the plan items by base name. A start lists the sources again and is refused `source_changed` when the fingerprint (each entry's path, size and modification time) differs. Only a confirmed plan submits the job, keeping its plan, manifest and items, with the files already held marked `skipped`.
- **Per-file progress (API-S-037).** [`native import/items.rs`](../../../native/src/import/items.rs) is `SqliteJobItems`, the `JobItems` port over the operational store (`import_items`), installed on the launch's `Jobs`.
  - The import job records each file: `copying`, with the bytes read reported once a second from the source view's counter (two reads per file); then `copied` with its committed size, `skipped`, or `failed` with its code.
  - After the manifest, the job imports the files a retry put back, alone. A run that tried every file with one failed leaves the job pending (API-S-037 refuses a retry of a finished job), and the launch-time replay does not start such a job until a file is queued again.
- **Digests.** `filesquid-api`'s `FileService::import_original` now answers `Committed<ImportedOriginal>` (API-F2.9's journal linkage around the view, the committed SHA-256 and size), a host type that is not in the contract. The job records every committed digest per library (`import_digests`), so a later plan finds the files the library holds.
- **Rebuild across launches (API-S-025, DE-S-014).**
  - `RebuildRunner::with_stop` (an API adapter): when set, a run checkpoints after the object in hand and suspends.
  - The desktop passes the app's exit flag to it, and the exit waits at most 10 s for running rebuilds ([`rebuilds.rs`](../../src/rebuilds.rs)).
  - Libraries are opened by the owner, not at launch, so an unfinished rebuild that is not paused is handed to the runner when its library is opened, and only when the launch reconciled its jobs.
- **Registry across launches (API-S-038, DE-S-018).** [`native registry_sqlite.rs`](../../../native/src/registry_sqlite.rs) is `SqliteLibraryRegistry` (`library_registry`). `HostLocations::persistent` writes through on `remember` and `rename` and, at launch, restores each library and registers its folder again under its location ID. An entry whose folder is gone is kept, with its free bytes `volume_unreadable`. The folder never crosses the API.
- **Windows (owner decision 5, 2026-10-23).** Unchanged: free bytes are `null` with `volume_unreadable` outside Linux and macOS; no new crate.
- **Docs.** DE-S-014, DE-S-018, DE-S-019; root README import section and test commands; plan row, checklist and progress block.

## SRS mapping

| Requirement | Tests |
|---|---|
| DE-S-019 Desktop file and folder import: review before any byte is staged; start after confirmation only; `source_changed`; discard; replay | `a_picked_folder_is_planned_with_nothing_staged_and_started_only_when_confirmed`, `a_discarded_plan_starts_nothing_and_a_changed_source_is_refused`; BDD "An import is reviewed before any byte is copied and starts only when confirmed", "A discarded plan imports nothing" |
| DE-S-019 / API-S-036 Import route (Review): files the library holds, by digest, are counted and skipped | `files_the_library_already_holds_are_counted_by_digest_and_skipped`; native `an_import_stores_the_original_under_its_uuid_with_its_record_in_one_version` (committed digest and size) |
| API-S-037 Job routes and job events (Per-file progress): states, codes, bytes, retry of one file, pending end | `a_failed_file_keeps_its_code_and_is_retried_alone_after_a_restart`, native `each_file_s_state_is_kept_and_a_failed_file_is_retried_alone`, `unit_import_items` (keep, retry, progress), `a_counting_source_adds_every_byte_it_reads`; BDD "A file that failed keeps its code and is retried on its own" |
| DE-S-014 Durable jobs and checkpoint-on-quit: a rebuild suspended at quit, resumed from its checkpoint | `a_rebuild_stopped_at_exit_resumes_from_its_checkpoint_when_its_library_opens_again` |
| DE-S-018 Library routes over owner-chosen folders / API-S-038 Library facts: registry across launches, free bytes, no path | `recent_libraries_and_their_facts_survive_a_restart_without_a_path`, native `a_persistent_registry_restores_names_locations_and_free_bytes_after_a_restart` |
| DE-S-006 No raw paths to the webview | every desktop answer is checked for the scratch path; names checked absent from the journal |

## TDD

The job's item-state and retry test (`each_file_s_state_is_kept_and_a_failed_file_is_retried_alone`) was written before the job's wiring. Its first run failed, together with the quit-and-resume test, on two points that drove the fixes: a retried file's size (taken when the manifest was kept) and the end state of a run with a failed file (now pending). The items store, the review, the rebuild hand-over and the persistent registry were implemented before their tests, which then passed at once. This record follows the DE-1.7 walkthrough's note.

## Test output

`cargo test -p filesquid-native --features desktop --locked --test unit_import_items --test unit_import_file --test unit_import_job --test unit_folder_libraries`, `cargo test -p filesquid-desktop --locked --test integration_import_review --test integration_library_routes --test integration_imports --test integration_import_controls` and `cargo test -p filesquid-api --locked --lib`:

```text
running 25 tests
test a_create_library_command_is_refused_and_creates_nothing ... ok
test a_folder_that_cannot_be_a_library_leaves_nothing_in_the_catalog_directory ... ok
test a_file_object_without_a_snapshot_entry_is_counted_and_left_unregistered ... ok
test a_file_of_the_earlier_form_is_placed_when_it_is_registered ... ok
test a_persistent_registry_restores_names_locations_and_free_bytes_after_a_restart ... ok
test a_file_s_path_is_taken_below_it_and_no_file_sits_below_a_file ... ok
test a_library_folder_copied_elsewhere_reopens_with_its_collections_folders_and_files ... ok
test a_refused_registration_is_journaled_as_failed_with_its_code ... ok
test a_retired_in_app_root_is_neither_opened_nor_served ... ok
test a_registered_file_names_its_path_and_object_in_the_snapshot_and_the_index ... ok
test a_registration_whose_numbers_are_all_taken_is_refused_and_changes_nothing ... ok
test an_empty_folder_attached_with_an_index_of_another_folder_starts_afresh ... ok
test an_index_write_for_a_library_that_is_not_attached_is_refused ... ok
test an_index_built_from_another_copy_of_the_folder_is_rebuilt ... ok
test a_version_2_catalog_is_migrated_to_the_current_version_and_rebuilt_from_its_folder ... ok
test a_taken_path_is_numbered_beside_it_and_a_deleted_file_s_path_counts ... ok
test no_index_is_created_beyond_the_catalog_s_limit_of_files ... ok
test no_library_root_or_original_is_ever_created_in_the_catalog_directory ... ok
test files_deleted_with_their_collection_are_deleted_not_unfiled ... ok
test deleting_a_collection_deletes_its_files_in_place_in_one_version ... ok
test removing_a_collection_that_still_holds_files_is_refused_before_anything_is_published ... ok
test renaming_a_library_retitles_its_entity_in_one_journaled_version_and_a_same_name_rename_publishes_nothing ... ok
test the_catalog_is_the_home_collections_of_uploads_and_copies ... ok
test the_library_s_own_retention_survives_a_rebuild_and_sets_purge_after ... ok
test folders_and_scope_entries_come_from_the_index_of_ordinary_files ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.31s


running 8 tests
test a_size_mismatch_and_a_changed_source_are_retried_within_the_job ... ok
test a_counting_source_adds_every_byte_it_reads ... ok
test a_missing_source_is_refused_without_retry ... ok
test a_size_that_is_not_the_size_read_is_refused_and_nothing_is_published ... ok
test a_source_that_cannot_be_read_while_staged_is_refused_as_source_unreadable ... ok
test the_source_entity_tag_changes_when_the_file_is_rewritten ... ok
test a_source_that_changes_between_its_two_reads_is_refused_as_source_changed ... ok
test an_import_stores_the_original_under_its_uuid_with_its_record_in_one_version ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s


running 4 tests
test keeping_a_manifest_again_changes_no_item ... ok
test a_manifest_is_kept_as_queued_items_in_order_with_its_destination ... ok
test a_copying_file_s_progress_is_kept_up_to_its_size_and_only_while_copying ... ok
test a_retry_puts_back_only_failed_files_and_a_repeat_changes_nothing ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s


running 21 tests
test a_deleted_file_s_path_counts_as_taken ... ok
test a_file_committed_before_a_crash_is_answered_not_imported_twice ... ok
test a_crash_between_a_file_s_object_and_its_registration_resumes_and_registers_once ... ok
test a_file_that_keeps_failing_is_quarantined_after_five_attempts_and_the_rest_go_on ... ok
test a_folder_is_imported_as_one_job_with_a_checkpoint_per_file ... ok
test a_folder_is_listed_in_one_stable_order_with_its_subfolders ... ok
test a_folder_imported_again_is_numbered_beside_the_first ... ok
test a_plan_and_its_manifest_survive_a_restart_and_one_operation_has_one_job ... ok
test a_plan_without_a_destination_fails_its_job_as_destination_required ... ok
test a_file_the_index_keeps_refusing_to_register_suspends_the_job_and_keeps_its_intent ... ok
test a_numbered_name_over_255_bytes_is_quarantined_as_name_too_long ... ok
test a_source_whose_numbers_are_all_taken_is_quarantined_as_path_suffix_exhausted ... ok
test checkpoints_backoff_and_reserve_follow_their_rules ... ok
test a_queued_job_without_a_plan_is_failed_not_left_pending ... ok
test picked_files_are_imported_each_with_itself_as_its_root ... ok
test each_file_s_state_is_kept_and_a_failed_file_is_retried_alone ... ok
test a_quit_suspends_between_files_and_the_resume_imports_only_the_rest ... ok
test what_cannot_be_listed_is_kept_with_its_code ... ok
test two_picked_folders_of_one_name_land_beside_each_other ... ok
test a_file_that_streams_longer_than_the_lease_keeps_the_job_s_lease_alive ... ok
test the_worker_runs_every_unfinished_job_beyond_one_queue_full ... ok

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.59s


running 4 tests
test a_cancelled_import_ends_after_the_file_in_hand_with_no_further_file_read ... ok
test a_resume_that_clears_the_pause_before_the_checkpoint_keeps_the_run_going ... ok
test a_paused_import_stops_after_the_file_in_hand_and_resumes_without_reading_it_again ... ok
test a_paused_import_is_not_started_by_the_next_launch_until_it_is_resumed ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.05s


running 4 tests
test a_discarded_plan_starts_nothing_and_a_changed_source_is_refused ... ok
test a_picked_folder_is_planned_with_nothing_staged_and_started_only_when_confirmed ... ok
test files_the_library_already_holds_are_counted_by_digest_and_skipped ... ok
test a_failed_file_keeps_its_code_and_is_retried_alone_after_a_restart ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.50s


running 6 tests
test a_cancelled_dialog_or_an_unopened_library_makes_no_job ... ok
test an_import_without_a_destination_is_refused_before_any_dialog ... ok
test a_picked_folder_is_imported_as_one_job_and_a_repeat_answers_the_same_job ... ok
test a_job_the_stopped_worker_could_not_take_runs_at_the_next_launch ... ok
test an_operation_id_reused_for_another_import_is_refused_not_answered_with_its_job ... ok
test imported_files_appear_under_their_collection_and_folder ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.47s


running 13 tests
test a_cancelled_or_unusable_pick_registers_nothing ... ok
test a_named_library_answers_its_facts_and_is_listed_as_recent_without_its_folder ... ok
test a_library_folder_copied_elsewhere_reopens_with_its_collections ... ok
test create_in_a_chosen_folder_answers_an_opaque_library_and_serves_its_files ... ok
test an_opened_folder_feeds_the_history_and_copies_and_rebuilds_without_paths ... ok
test a_rebuild_stopped_at_exit_resumes_from_its_checkpoint_when_its_library_opens_again ... ok
test recent_libraries_and_their_facts_survive_a_restart_without_a_path ... ok
test the_navigation_lists_the_installations_repository_and_its_libraries ... ok
test the_data_directory_keeps_only_each_library_s_index_never_its_files ... ok
test the_owners_preferences_are_kept_in_the_operational_store ... ok
test the_owner_picks_a_folder_in_the_hosts_dialog_and_the_app_sees_only_its_id ... ok
test the_backfill_route_runs_over_the_folders_the_owner_opened ... ok
test the_owners_consented_repair_is_kept_in_the_persistent_ledger ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s


running 77 tests
test auth::token::tests::equal_slices_compare_equal_and_every_difference_is_found ... ok
test admin::journal::tests::a_complete_export_has_no_continuation_cursor_header ... ok
test admin::journal::tests::an_export_that_hits_the_page_cap_returns_a_continuation_cursor ... ok
test auth::layer::tests::without_a_monitor_a_refusal_goes_to_stdout_unfiltered ... ok
test auth::token::tests::secrets_compare_by_digest_whatever_their_lengths ... ok
test client_events::limiter::tests::retry_after_counts_whole_seconds_to_the_oldest_request_leaving ... ok
test client_events::schema::tests::uuid_v4_shapes ... ok
test client_events::schema::tests::a_lone_overflow_is_recognised ... ok
test content::encoding::tests::only_43_base64url_characters_are_well_formed ... ok
test content::encoding::tests::thirty_two_bytes_become_43_characters_matching_rfc_4648 ... ok
test content::store::tests::a_clock_that_moves_back_ends_the_handle ... ok
test content::store::tests::a_handle_no_longer_allowed_is_refused_and_removed ... ok
test content::store::tests::a_minted_handle_is_accepted_until_it_idles_out ... ok
test content::store::tests::ending_a_credential_ends_only_its_handles ... ok
test content::store::tests::the_store_keeps_digests_and_debug_shows_no_handle ... ok
test envelope::tests::a_checked_envelope_keeps_its_operation_id_and_expected_revision ... ok
test envelope::tests::a_replay_refuses_nothing_and_the_other_settlements_name_their_code ... ok
test envelope::tests::another_schema_scope_or_a_non_opaque_operation_id_is_refused ... ok
test envelope::tests::committed_and_pending_answers_carry_the_outcome_fields_and_the_result ... ok
test files::drive::tests::filters_limits_and_views_are_bounded ... ok
test files::drive::tests::a_view_that_changed_between_pages_refuses_its_cursor ... ok
test files::drive::tests::a_cursor_for_other_parameters_or_a_forged_one_is_refused ... ok
test files::drive_scope::tests::each_page_asks_only_for_what_the_bound_leaves ... ok
test files::drive::tests::missing_values_sort_last_and_ties_break_by_name_then_id ... ok
test files::drive_scope::tests::only_changes_and_openings_count_and_the_latest_wins ... ok
test files::labels::tests::a_file_with_an_invalid_label_is_never_written ... ok
test files::drive::tests::folders_on_top_stay_on_top_in_both_directions_and_mixed_interleaves ... ok
test files::labels::tests::a_labels_file_naming_something_other_than_a_version_is_refused ... ok
test files::labels::tests::the_default_labels_file_is_the_current_schema_and_reads_back ... ok
test files::labels::tests::an_override_equal_to_the_commit_label_is_dropped ... ok
test files::labels::tests::the_file_round_trips ... ok
test files::labels::tests::the_reserved_folder_is_the_folder_and_its_contents_only ... ok
test files::media::tests::extensions_map_without_case_and_unknown_ones_fall_back ... ok
test client_events::limiter::tests::the_table_never_tracks_more_than_its_bound ... ok
test files::labels::tests::the_thousand_and_first_labelled_version_is_refused ... ok
test files::names::tests::a_name_without_an_ascii_letter_or_digit_falls_back_to_the_file_id ... ok
test files::names::tests::a_non_ascii_name_with_spaces_gets_a_fallback_and_an_exact_utf8_form ... ok
test files::names::tests::long_names_are_cut_at_a_character_boundary ... ok
test files::names::tests::quotes_backslashes_slashes_semicolons_and_controls_never_reach_the_fallback ... ok
test files::range::tests::plans_cap_every_answer_and_find_unsatisfiable_ranges ... ok
test files::names::tests::the_display_name_matches_the_metadata_modules_safe_display_name ... ok
test files::range::tests::single_byte_ranges_parse ... ok
test files::range::tests::repeated_range_fields_are_several_ranges ... ok
test files::search::tests::zones_are_fixed_offsets ... ok
test files::search_match::tests::capture_dates_without_a_zone_compare_by_calendar_date ... ok
test files::search_match::tests::documents_are_pdf_rtf_and_office_formats_only ... ok
test files::search::tests::relevance_is_the_default_and_any_view_key_replaces_it ... ok
test files::search_match::tests::folders_match_by_name_and_type_and_relevance_ranks_names_first ... ok
test files::search_match::tests::head_dates_use_the_callers_zone ... ok
test files::search_match::tests::free_text_reads_the_name_and_dublin_core_values ... ok
test files::search_match::tests::operators_read_their_statements_and_facts ... ok
test files::search_query::tests::folding_is_nfc_and_lower_case_without_diacritic_folding ... ok
test files::search_query::tests::operators_this_host_cannot_answer_are_unavailable ... ok
test files::search_query::tests::every_answered_operator_parses ... ok
test files::search_query::tests::queries_are_bounded ... ok
test files::search_query::tests::words_phrases_and_exclusions ... ok
test history::redact::tests::a_path_keeps_its_token_across_records ... ok
test history::redact::tests::a_path_without_a_numbered_identifier_refuses_the_record ... ok
test history::redact::tests::an_entity_id_loses_only_its_path ... ok
test history::redact::tests::non_objects_and_deep_documents_are_refused ... ok
test history::source::tests::a_storage_failure_is_unavailable_and_a_bad_record_is_corrupt ... ok
test history::routes::tests::an_export_past_its_byte_budget_stops_at_a_whole_record_and_is_flagged ... ok
test journal::seal::service::tests::a_scan_of_other_scopes_entries_only_is_idle_within_its_page_limit ... ok
test journal::seal::object::tests::segment_paths_round_trip_their_range ... ok
test journal::seal::service::tests::a_scan_stopped_by_its_page_limit_still_moves_the_cursor ... ok
test journal_writer::tests::a_rejected_write_is_one_json_line_per_record_on_stdout ... ok
test local_library::commands::tests::an_unreachable_snapshot_is_retryable_and_an_unreadable_one_is_refused ... ok
test files::search_query::tests::unknown_operators_and_bad_values_are_refused_and_never_ignored ... ok
test ocfl_validation::run::tests::repeated_identifiers_get_distinct_keys ... ok
test ocfl_validation::service::tests::an_old_run_lock_never_frees_a_reopened_library ... ok
test ocfl_validation::service::tests::closing_a_library_cancels_its_run ... ok
test ocfl_validation::service::tests::reopening_an_open_library_keeps_its_locks ... ok
test ocfl_validation::service::tests::shutdown_waits_for_takeover_checks_and_refuses_new_ones ... ok
test routes::tests::the_liveness_probe_is_the_one_public_path ... ok
test routes::tests::a_route_other_than_the_liveness_probe_cannot_be_public - should panic ... ok
test files::labels::tests::the_largest_allowed_labels_file_stays_within_its_read_bound ... ok
test journal::seal::service::tests::an_entry_larger_than_any_segment_is_an_error_not_a_panic ... ok

test result: ok. 77 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.86s
```

`cargo test -p filesquid-desktop --features test-webdriver --locked --test features` (import feature; whole run below):

```text
Feature: Desktop file and folder import
  Scenario: A picked folder is imported as one job, one object per file
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import a folder under the operation "op-holidays"
   ✔  Then the answer should name a started import job
   ✔  And 2 files should be imported, each as its own object
   ✔  And no journal entry should name a picked file or folder
  Scenario: Repeating an operation answers the same job without a second dialog
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import a folder under the operation "op-holidays"
   ✔  And the app asks to import a folder under the operation "op-holidays" again
   ✔  Then both answers should name the same import job
   ✔  And the dialog should have been shown once
   ✔  And 2 files should be imported, each as its own object
  Scenario: A cancelled dialog makes no job
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will close the dialog without picking anything
   ✔  When the app asks to import files under the operation "op-cancel"
   ✔  Then the answer should say the import was cancelled
   ✔  And no file should be imported
  Scenario: Imported files appear under the collection and folder the import names
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import a folder under the operation "op-trip"
   ✔  Then 2 files should be imported, each as its own object
   ✔  And the collection should list the folder "trips/Holidays" holding "Beach day.jpg" and "Notes.txt"
  Scenario: A folder imported again sits beside the first under a numbered name
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import a folder under the operation "op-first"
   ✔  And the import has finished with 2 files
   ✔  And the owner edits both picked files
   ✔  And the app asks to import a folder under the operation "op-again"
   ✔  Then the collection should list the folders "Holidays" and "Holidays-1" in "trips"
  Scenario: An import that names no destination collection is refused before any dialog
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import a folder without naming a destination collection
   ✔  Then the import should be refused as an invalid request naming "destination_required"
   ✔  And the dialog should not have been shown
   ✔  And no file should be imported
  Scenario: A library that is not open is refused and journaled once without a path
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to import into a library that is not open
   ✔  Then the import should be refused as not found
   ✔  And the dialog should not have been shown
   ✔  And the journal should record one import request without any path
  Scenario: An import the host could not run now runs at the next launch
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  And the import worker has stopped for the app to quit
   ✔  When the app asks to import a folder under the operation "op-later"
   ✔  Then the import should be refused as unavailable
   ✔  And after the app restarts 2 files should be imported, each as its own object
  Scenario: A paused import stops after the file in hand and resumes where it stopped
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  And the import worker will be held on its first file
   ✔  When the app asks to import a folder under the operation "op-paused"
   ✔  And the owner pauses the import while its first file is in hand
   ✔  Then the import job should be paused with 1 of 2 files done
   ✔  And the job control "job.paused" should be journaled once
   ✔  When the owner resumes the import job
   ✔  Then the import job should be completed with 2 of 2 files done
   ✔  And 2 files should be imported, each as its own object
   ✔  And no journal entry should name a picked file or folder
  Scenario: A cancelled import ends after the file in hand and reads no further file
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  And the import worker will be held on its first file
   ✔  When the app asks to import a folder under the operation "op-stopped"
   ✔  And the owner cancels the import while its first file is in hand
   ✔  Then the import job should be cancelled with 1 of 2 files done
   ✔  And the job control "job.cancel_requested" should be journaled once
   ✔  And only the file in hand should be imported
  Scenario: An import is reviewed before any byte is copied and starts only when confirmed
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to review a folder import under the operation "op-review"
   ✔  Then the plan should count 2 new files and 9 bytes to copy with nothing staged
   ✔  When the owner confirms the plan
   ✔  Then 2 files should be imported, each as its own object
  Scenario: A discarded plan imports nothing
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  When the app asks to review a folder import under the operation "op-dropped"
   ✔  And the owner discards the plan
   ✔  Then confirming the plan should be refused as not found
   ✔  And no file should be imported
  Scenario: A file that failed keeps its code and is retried on its own
   ✔> Given the desktop host has started its import route over an open library
   ✔  Given the owner will pick a folder of 2 files
   ✔  And the import worker will be held on its first file
   ✔  When the app asks to review a folder import under the operation "op-failing"
   ✔  And the owner confirms the plan
   ✔  And the second picked file disappears while the first is in hand
   ✔  Then the second file should have failed as "source_missing" with the job waiting for a retry
   ✔  When the second picked file comes back and the owner retries it
   ✔  Then the import job should be completed with 2 of 2 files done

53 scenarios (53 passed)
247 steps (247 passed)
```

## Decisions to confirm

- **An import job that ends with a failed file stays pending** (the orchestrator accepted this on 2026-10-30, pending the owner's confirmation). API-S-037 refuses a retry of a finished job, so a run that tried every file with one failed gives its lease back with the job `pending` and its checkpoint at the end, not `completed`. Its failed files stay retryable through `POST /api/v1/jobs/{job}/items/{item}/retry` and `/retry-failed`, the owner can cancel it through `/cancel`, and the launch-time replay does not start it until a retry has queued a file again. DE-S-019 states the rule.
- **What the UI must show (UI-1.2, UI-1.3).** A pending import job whose items include a `failed` one, with no `queued` or `copying` item and `done` equal to `total`, is waiting for the owner, not running. It needs a "needs attention" state with **Retry** (per failed file and for all failed files) and **Cancel**, never a spinner or a progress animation. It reads from the job view (`state`, `done`, `total`) and the job items route (`state`, `error_code`).
- **The import commit answers the committed digest** (`ImportedOriginal { view, sha256, size }`, a host type outside the contract; `UploadView` stays a contract schema). API-F2.9 (#157, merged first) made the same answer `Committed<UploadView>`; after merging it this branch answers `Committed<ImportedOriginal>` (orchestrator decision, 2026-10-30): the native job reads `committed.view.view`, `.view.sha256`, `.view.size` and journals `.entries`.

## Known limits

- The review hashes every picked file once, in the request that answers the plan; a very large pick takes as long as that read.
- A file's numbering in the review is read from its manifest's logical path, so a picked name that only the path rule rewrites counts as numbered.
- An import job with a failed file stays pending until its files are retried or it is cancelled.

## Mocked Environment Variables

None.

## Pre-commit

The head-commit run (`FULL=1 bash scripts/pre-commit.sh` with a private target, summary line `Test scope: full`) is recorded in the pull request.
