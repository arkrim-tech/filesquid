# Task DE-1.7 Walkthrough: Owner-folder library index, home collections and retirement of in-app catalog roots

Branch: `feat/de-1.7-owner-folder-library-index` (global [Task 1.2](../../../../../../resources/implementation-plan.md#task-1.2)). API contract `0.27.0` (one minor above `0.26.0` on `main`, from API-F2.17). Binding decisions: [ADR 041](../../../../../../resources/architecture/041-owner-folder-libraries-and-home-collections.md).

## What changed

- **`filesquid-api`**:
  - [`canonical/collection_path.rs`](../../../api/src/canonical/collection_path.rs) (new): the collection logical-path rule (NFC, at most 1,024 bytes, segments of at most 255 bytes, no `\`, NUL, control characters, `.` or `..`, not in the reserved folders), the API-S-005 safe segment, and the numbering rule of ADR 041 Decision 12: a folder takes `-N` at its end (`Holidays-1`), a file before its first dot (`photo-1.jpg`, `archive-1.tar.gz`), a leading dot stays in the first part (`.env-1`, `.config-1.json`); at most 9,999 numbers, then `path_suffix_exhausted`; a numbered segment over 255 bytes is `name_too_long`.
  - The snapshot's file entity (`SnapshotFile`) gains `path` (its logical path in its home collection) and `object_id` (its file object); both or neither, checked on read with fixed reasons. The earlier form is still read and counted as unregistered. `LibraryHead` gains the library's own `retention_days` (at most 3,650), kept in the snapshot so a rebuild restores it.
  - [`canonical/unregistered.rs`](../../../api/src/canonical/unregistered.rs) (new): counts the file objects of a folder the snapshot does not register; a rebuild reports them and never places them in an invented collection (ADR 041 Decision 6).
  - Deleting a collection that holds files plans `CatalogChange::DeleteCollectionFiles` then `RemoveCollection` in one operation: each of its files not already deleted is marked deleted in place (`deleted_at`, `purge_after` from the library's retention, else the 30-day installation default), keeping its home collection and path; an already deleted file keeps its times.
  - `POST /api/v1/imports` requires `collection_id` and accepts an optional `folder`: no destination is `400 invalid_request` (`destination_required`), an unknown or foreign collection `404 not_found` (`collection_not_found`), a folder outside the path rule `400 invalid_request` (`invalid_path`), all before any dialog and journaled once. `collection_not_supported` is retired.
  - Opening a folder answers the library its snapshot names (a copied or moved folder keeps its ID); a second folder of an open library is `library_already_open`, a request naming another library `library_mismatch`.
  - [`files/scope_listing.rs`](../../../api/src/files/scope_listing.rs) (new): `GET /api/v1/files/listing?node=` answers a collection or folder node from the host's index (its subfolders, then its registered files under their names), paged by an opaque cursor; listing entries carry the file's `object_id`.
  - `openapi.json` 0.27.0; `ui/src/api/schema.d.ts` regenerated.
- **`filesquid-native`** (`desktop` feature):
  - Catalog schema v3 ([`schema.rs`](../../../native/src/catalog/schema.rs)): the library's `retention_days` and folder identity; each file's `path`, `object_id`, folder key and name, with a unique `(home, path)` index and a partial folder index; v2 catalogs migrate forward.
  - The in-app roots `libraries/<library-id>.ocfl/` are retired (`OCFL_ROOT_SUFFIX` removed, no migration). [`canonical.rs`](../../../native/src/catalog/canonical.rs) commits the snapshot into the owner's folder through the same backend and fenced leases as the file routes; the catalog directory `libraries/` keeps only `<library-id>.sqlite` and `<library-id>.work/` staging; the rest of the data directory holds the operational store and the file service's staging and leases (`files/<digest>/`, `locations/<digest>/`), never a library's storage root, originals or metadata records. A `CreateLibrary` commit to the catalog is refused (`library_needs_folder`).
  - [`attach.rs`](../../../native/src/catalog/attach.rs) (new): attaching a folder creates the library entity in a new folder (default name "Library"), or rebuilds the index when it is missing, built from another copy of the folder, or behind the snapshot head, in the journaled `library.reconcile` whose entry counts unregistered file objects.
  - [`register.rs`](../../../native/src/catalog/register.rs) and [`placement.rs`](../../../native/src/catalog/placement.rs) (new): registering a file publishes its path and object in the snapshot under the import's operation ID, then indexes it (`library.files.register`); folder and scope queries over the index.
  - Import ([`import/placing.rs`](../../../native/src/import/placing.rs), [`import/index.rs`](../../../native/src/import/index.rs), new): the plan keeps the destination collection and folder; each picked source gets its logical path (numbered when taken, a deleted file's path counting as taken) once, when the manifest is made, so its files land together; a source with all numbers taken or a name over 255 bytes is quarantined with that code before a byte is read. Each file is registered after its object's commit; the import's pending intent stays until the job finishes, so a crash between the two resumes and registers once.
- **`filesquid-desktop`**: create and open attach the folder to the catalog; `CatalogFolders` and `CatalogScopes` answer from the index; the import starter keeps the destination in the plan.
- **Docs**: root README catalog and commit sections; API-S-025 (open by snapshot, `library_mismatch`).

## Decisions taken while implementing

- Folder libraries' snapshot commits and file commits share one backend and one set of leases (the `HostRoot` of DE-1.1), so one folder never has two lease sets.
- Unregistered file objects are reported as a count in the `library.reconcile` entry (`unregistered:N`), never named.
- The desktop end-to-end tests (`integration_imports`, `integration_library_routes`, the BDD scenarios) were written after the lower layers were implemented and passed at once; the API and native tests were written first and failed first (raw output below).

## Known limits

- Uploads, copies and moves still register no home collection; that is API-F2.14. Restore, retention settings and the expiry job are API-F2.15.
- The index answers listings; the UI still shows them through the existing navigation (UI-F2.17).

## Review rounds

| Round | Findings | What changed | Evidence |
|---|---|---|---|
| Cubic 1 (36: 7 P1, 26 P2, 3 P3) | A collection removed without deleting its files; a file below a file; an earlier-form row answered as registered; a replayed import refused by a changed destination; collection readability without its ancestors; a committed file left unregistered after the retries; an unreadable snapshot opened under another ID; a stale index served for an empty folder | Removal refused unless its files are deleted first (`COLLECTION_HOLDS_FILES`, checked before publishing); ancestors count as taken; only a placed row is `AlreadyRegistered`; `ImportStarter::replay` answered before the destination check; `check_readable`; the job suspends and keeps its intent; `snapshot_unreadable`; an index of another folder is emptied | `removing_a_collection_that_still_holds_files_is_refused_before_anything_is_published`, `a_file_s_path_is_taken_below_it_and_no_file_sits_below_a_file`, `a_file_of_the_earlier_form_is_placed_when_it_is_registered`, `a_repeated_operation_answers_its_job_before_the_destination_is_checked_again`, `a_file_the_index_keeps_refusing_to_register_suspends_the_job_and_keeps_its_intent`, `opening_a_folder_whose_snapshot_cannot_be_read_is_refused_once`, `an_empty_folder_attached_with_an_index_of_another_folder_starts_afresh` |
| | Cursors not bound to their listing; reserved names placed; refused registrations not journaled; two picked folders of one name merged; unsafe display names; catalog failures answered `unavailable`; the attach blocking the executor and losing its error class; staging left by an unusable folder; the library bound ignoring files on disk; deleted files in `Unfiled`; retention outside 1 to 3,650 days; `touched` and the empty-collection marker | Cursor context (SHA-256 of scope and folder); reserved names numbered (`provenance-1`), paths inside them `invalid_path`; a `failed` `library.files.register` entry with its code; roots reserved within a plan; `safe_name`; `internal` for `Failed`/`Conflict`; `spawn_blocking` and `conflict`/`unavailable`/`internal`; the root declared before staging and index; catalog files counted; `Unfiled` excludes deleted files; range checks; library key only; marker only when a file is deleted | `a_cursor_of_another_folder_is_refused`, `an_item_named_like_a_reserved_folder_is_numbered_beside_it`, `a_path_inside_a_reserved_folder_is_invalid`, `a_refused_registration_is_journaled_as_failed_with_its_code`, `two_picked_folders_of_one_name_land_beside_each_other`, `an_indexed_name_is_shown_in_its_safe_form`, `a_catalog_failure_a_retry_cannot_fix_is_internal_not_unavailable`, `a_folder_that_cannot_be_a_library_leaves_nothing_in_the_catalog_directory`, `no_index_is_created_beyond_the_catalog_s_limit_of_files`, `files_deleted_with_their_collection_are_deleted_not_unfiled`, `a_retention_period_outside_one_to_3650_days_is_refused`, `deleting_a_collection_without_files_only_removes_it` |
| | Tests and docs: weak data-directory and snapshot steps, a test name claiming files, a narrow redaction check, a 64-library reattach, an `object_id` sample, a privacy assertion, a broken doc link, plan total, README and walkthrough claims | Each fixed as named; the reference-catalog parity test now deletes the collection's files with it and compares the deleted view | `features` (48 scenarios), `integration_library_routes`, `unit_contract`, `unit_catalog` |

## Test results

### Failing first

The first native build against the new API types, before the catalog implemented them:

```text
$ cargo test -p filesquid-native --features desktop --locked
error[E0063]: missing field `retention_days` in initializer of `LibraryHead`
   --> crates/native/src/catalog/project.rs:176:70
    |
176 |         rows::library(conn)?.map(|(record, revision, created_order)| LibraryHead {
    |                                                                      ^^^^^^^^^^^ missing `retention_days`

error[E0063]: missing fields `object_id` and `path` in initializer of `SnapshotFile`
   --> crates/native/src/catalog/project.rs:224:61
    |
224 |         rows::keyed_file(conn, library_id, id)?.map(|keyed| SnapshotFile {
    |                                                             ^^^^^^^^^^^^ missing `object_id` and `path`

error[E0063]: missing field `retention_days` in initializer of `LibraryState`
   --> crates/native/src/catalog/mod.rs:820:71
    |
820 |             rows::library(&library.conn)?.map(|(record, revision, _)| LibraryState {
    |                                                                       ^^^^^^^^^^^^ missing `retention_days`

error[E0004]: non-exhaustive patterns: `&CatalogChange::DeleteCollectionFiles { .. }` not covered
   --> crates/native/src/catalog/write.rs:334:11
    |
334 |     match change {
    |           ^^^^^^ pattern `&CatalogChange::DeleteCollectionFiles { .. }` not covered
    |
note: `CatalogChange` defined here
   --> crates/api/src/library/catalog.rs:111:1
    |
111 | pub enum CatalogChange {
    | ^^^^^^^^^^^^^^^^^^^^^^
...
124 |     DeleteCollectionFiles {
    |     --------------------- not covered
    = note: the matched value is of type `&CatalogChange`
help: ensure that all possible cases are being handled by adding a match arm with a wildcard pattern or an explicit pattern as shown
    |
366 ~         CatalogChange::PurgeFile(id) => exec(tx, "DELETE FROM files WHERE id = ?1", id.as_str()),
367 ~         &CatalogChange::DeleteCollectionFiles { .. } => todo!(),
    |

Some errors have detailed explanations: E0004, E0063.
For more information about an error, try `rustc --explain E0004`.
error: could not compile `filesquid-native` (lib) due to 4 previous errors
warning: build failed, waiting for other jobs to finish...
```

### Passing

Run from `apps/filesquid` with the shared build lock (`<target>` is the shared Cargo target directory). Raw output, compile lines and passing BDD step marks left out:

```text
$ cargo test -p filesquid-api --locked --test unit_collection_path --test integration_scope_listing --test unit_canonical --test unit_imports_route --test unit_library
$ cargo test -p filesquid-native --features desktop --locked --test unit_folder_libraries --test unit_import_job
$ cargo test -p filesquid-desktop --locked --test integration_imports --test integration_library_routes --test integration_library_catalog
$ cargo test -p filesquid-desktop --features test-webdriver --locked --test features
     Running tests/integration/scope_listing.rs (<target>/debug/deps/integration_scope_listing-d6d898d5d52cb31b)

running 4 tests
test a_collection_node_lists_its_folders_then_its_registered_files ... ok
test a_folder_node_lists_the_files_beneath_it_under_their_numbered_names ... ok
test a_scope_listing_pages_with_an_opaque_cursor ... ok
test a_scope_listing_names_no_filesystem_path ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/unit/canonical.rs (<target>/debug/deps/unit_canonical-8c92ffff27472509)

running 40 tests
test a_copy_record_keeps_what_the_owner_sees_and_is_bound_to_the_new_object ... ok
test a_file_entity_of_the_earlier_form_is_read_and_reported_unregistered ... ok
test a_file_entity_records_its_logical_path_and_object ... ok
test a_file_entity_with_a_path_outside_the_rule_is_refused_with_a_fixed_reason ... ok
test a_commit_journal_a_replay_could_not_read_is_refused_before_publishing ... ok
test a_head_without_its_version_inventory_is_refused_and_nothing_is_written ... ok
test a_committed_snapshot_reads_back_entity_for_entity ... ok
test a_file_original_is_its_own_object_with_its_metadata_record ... ok
test a_provenance_only_message_names_no_receipt ... ok
test a_record_that_does_not_cite_the_bytes_commits_nothing ... ok
test a_recorded_kind_is_written_and_read_back_by_both_readers ... ok
test a_referenced_import_entry_cannot_describe_a_stored_original ... ok
test a_long_extension_stays_in_the_record_but_not_in_the_stored_name ... ok
test a_root_without_a_snapshot_has_no_commit_journal ... ok
test a_sharing_name_drops_an_extension_the_stored_name_does_not_keep ... ok
test a_storage_root_is_created_only_in_an_empty_folder ... ok
test a_stale_draft_left_in_staging_is_never_published ... ok
test a_later_version_changes_only_the_entities_it_touches ... ok
test an_entity_whose_identifier_cannot_name_a_path_is_a_decode_error ... ok
test an_entity_that_does_not_match_its_path_is_refused_on_read ... ok
test an_original_name_or_media_type_that_is_not_valid_is_refused ... ok
test an_unreadable_operation_message_is_refused_on_read ... ok
test an_owner_name_round_trips_through_its_metadata_record ... ok
test an_upload_whose_declared_size_differs_builds_no_record ... ok
test canonical_failures_keep_their_meaning_at_the_catalog_port ... ok
test catalog_operations_map_to_history_kinds ... ok
test each_change_touches_the_library_and_its_own_entity ... ok
test a_new_upload_of_the_same_file_appends_one_import_entry ... ok
test every_entity_has_one_logical_path_and_back ... ok
test identifiers_that_cannot_name_a_path_segment_are_refused ... ok
test sharing_names_a_file_by_its_file_id_unless_the_policy_selects_the_original_name ... ok
test the_head_record_committed_again_must_cite_the_bytes ... ok
test the_library_entity_keeps_its_own_retention_period ... ok
test history_files_ride_the_snapshot_version_and_are_not_entities ... ok
test the_metadata_record_is_the_metadata_modules_file_record ... ok
test the_operation_message_carries_its_receipt ... ok
test the_stored_name_is_the_file_identifier_and_its_extension_only ... ok
test the_old_four_line_record_is_refused_on_read ... ok
test file_objects_the_snapshot_does_not_register_are_counted ... ok
test an_entity_file_larger_than_the_bound_is_refused_before_it_is_read ... ok

test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s

     Running tests/unit/collection_path.rs (<target>/debug/deps/unit_collection_path-976c69ba34db13f9)

running 15 tests
test a_folder_takes_the_number_at_its_end ... ok
test a_destination_folder_is_a_collection_path_outside_the_reserved_folders ... ok
test a_dotfile_keeps_its_leading_dot_in_its_first_part ... ok
test a_file_takes_the_number_before_its_first_dot ... ok
test a_free_path_is_answered_as_it_is ... ok
test a_name_over_255_bytes_after_the_safe_transformation_is_name_too_long ... ok
test a_numbered_segment_over_255_bytes_is_name_too_long ... ok
test a_store_failure_stops_the_search ... ok
test a_taken_path_gets_the_first_free_number_beside_it ... ok
test collection_paths_follow_the_profile_rule ... ok
test join_puts_a_path_beneath_its_destination_folder ... ok
test a_safe_segment_is_the_api_s_005_name_in_nfc ... ok
test the_caps_are_the_owner_s ... ok
test the_9999th_number_is_still_tried ... ok
test the_search_stops_after_9999_numbers ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/unit/imports_route.rs (<target>/debug/deps/unit_imports_route-31f8a222c45c2880)

running 8 tests
test a_started_import_answers_its_job_and_journals_it_once ... ok
test a_cancelled_dialog_answers_cancelled_without_a_job ... ok
test an_import_without_a_destination_is_refused_before_any_dialog ... ok
test an_unknown_or_foreign_collection_is_not_found_before_any_dialog ... ok
test a_destination_folder_outside_the_path_rule_is_refused_before_any_dialog ... ok
test a_wrong_scope_no_starter_or_no_token_asks_nothing ... ok
test the_destination_reaches_the_host ... ok
test refusals_are_typed_and_journaled_once_with_their_code ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/unit/library.rs (<target>/debug/deps/unit_library-3336809e28cfe0a6)

running 36 tests
test a_failed_commit_leaves_the_memory_catalog_unchanged ... ok
test a_mixed_delete_batch_answers_the_times_of_the_files_it_newly_deletes ... ok
test a_later_create_library_change_cannot_replace_a_library ... ok
test a_library_s_own_retention_sets_the_purge_after_time ... ok
test a_repeated_operation_id_returns_the_first_outcome ... ok
test a_no_op_stores_no_receipt_journal_entry_or_version ... ok
test a_replayed_delete_answers_its_first_deletion ... ok
test a_stale_expected_revision_is_a_conflict_without_change ... ok
test a_retried_no_op_is_planned_again_against_current_state ... ok
test a_no_op_planned_on_a_revision_that_moved_on_is_a_conflict ... ok
test adding_a_present_member_is_a_no_op_that_commits_nothing ... ok
test an_empty_raw_commit_stores_nothing_in_the_memory_catalog ... ok
test catalog_failures_are_retryable_and_journaled_for_queries_too ... ok
test catalog_failures_are_journaled_exactly_once_with_the_journal_layer_installed ... ok
test commands_round_trip_as_versioned_json ... ok
test deleting_a_collection_deletes_its_files_in_place ... ok
test create_library_needs_the_new_library_revision ... ok
test create_collection_needs_a_folder_of_its_library ... ok
test deleting_a_collection_keeps_the_times_of_files_already_deleted ... ok
test deletion_is_recoverable_for_thirty_days_and_restore_keeps_hidden ... ok
test deleting_a_virtual_collection_retains_its_members ... ok
test errors_map_to_api_and_port_errors ... ok
test deleting_a_collection_whose_files_are_all_deleted_answers_no_new_deletion ... ok
test every_commit_carries_one_redacted_journal_entry ... ok
test create_library_mints_an_id_and_starts_at_revision_one ... ok
test names_must_be_non_empty_and_bounded ... ok
test minted_identifiers_are_canonical_uuid_v4 ... ok
test permanent_deletion_needs_deleted_files_and_an_exact_confirmation ... ok
test references_to_another_library_are_not_found ... ok
test membership_batches_keep_their_order_after_the_last_member ... ok
test refused_commands_are_journaled_as_failed ... ok
test renaming_to_the_same_title_commits_nothing ... ok
test uuid_check_rejects_other_shapes ... ok
test virtual_collection_membership_leaves_the_home_collection_alone ... ok
test rule_based_virtual_collections_refuse_hand_picked_members ... ok
test file_batches_hold_one_to_one_thousand_distinct_files ... ok

test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/unit/folder_libraries.rs (<target>/debug/deps/unit_folder_libraries-6c2e4d866ce41306)

running 14 tests
test a_create_library_command_is_refused_and_creates_nothing ... ok
test a_file_object_without_a_snapshot_entry_is_counted_and_left_unregistered ... ok
test a_registered_file_names_its_path_and_object_in_the_snapshot_and_the_index ... ok
test a_registration_whose_numbers_are_all_taken_is_refused_and_changes_nothing ... ok
test a_library_folder_copied_elsewhere_reopens_with_its_collections_folders_and_files ... ok
test a_retired_in_app_root_is_neither_opened_nor_served ... ok
test an_index_write_for_a_library_that_is_not_attached_is_refused ... ok
test a_version_2_catalog_is_migrated_to_version_3_and_rebuilt_from_its_folder ... ok
test an_index_built_from_another_copy_of_the_folder_is_rebuilt ... ok
test no_library_root_or_original_is_ever_created_in_the_catalog_directory ... ok
test a_taken_path_is_numbered_beside_it_and_a_deleted_file_s_path_counts ... ok
test deleting_a_collection_deletes_its_files_in_place_in_one_version ... ok
test the_library_s_own_retention_survives_a_rebuild_and_sets_purge_after ... ok
test folders_and_scope_entries_come_from_the_index_of_ordinary_files ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s

     Running tests/unit/import_job.rs (<target>/debug/deps/unit_import_job-916e540f90aadade)

running 18 tests
test a_file_committed_before_a_crash_is_answered_not_imported_twice ... ok
test a_deleted_file_s_path_counts_as_taken ... ok
test a_crash_between_a_file_s_object_and_its_registration_resumes_and_registers_once ... ok
test a_file_that_keeps_failing_is_quarantined_after_five_attempts_and_the_rest_go_on ... ok
test a_folder_is_listed_in_one_stable_order_with_its_subfolders ... ok
test a_numbered_name_over_255_bytes_is_quarantined_as_name_too_long ... ok
test a_plan_and_its_manifest_survive_a_restart_and_one_operation_has_one_job ... ok
test a_file_too_large_for_this_build_is_quarantined_without_retries ... ok
test a_folder_is_imported_as_one_job_with_a_checkpoint_per_file ... ok
test a_plan_without_a_destination_fails_its_job_as_destination_required ... ok
test a_source_whose_numbers_are_all_taken_is_quarantined_as_path_suffix_exhausted ... ok
test checkpoints_backoff_and_reserve_follow_their_rules ... ok
test a_folder_imported_again_is_numbered_beside_the_first ... ok
test picked_files_are_imported_each_with_itself_as_its_root ... ok
test what_cannot_be_listed_is_kept_with_its_code ... ok
test a_queued_job_without_a_plan_is_failed_not_left_pending ... ok
test a_quit_suspends_between_files_and_the_resume_imports_only_the_rest ... ok
test the_worker_runs_every_unfinished_job_beyond_one_queue_full ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.03s

     Running tests/integration/imports.rs (<target>/debug/deps/integration_imports-053c259c56e9e71c)

running 5 tests
test a_cancelled_dialog_or_an_unopened_library_makes_no_job ... ok
test an_import_without_a_destination_is_refused_before_any_dialog ... ok
test a_picked_folder_is_imported_as_one_job_and_a_repeat_answers_the_same_job ... ok
test a_job_the_stopped_worker_could_not_take_runs_at_the_next_launch ... ok
test imported_files_appear_under_their_collection_and_folder ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s

     Running tests/integration/library_catalog.rs (<target>/debug/deps/integration_library_catalog-2c598f23e2e90d7e)

running 5 tests
test a_catalog_directory_that_cannot_be_opened_is_journaled_without_its_path ... ok
test the_installation_uuid_is_created_once_and_journaled ... ok
test a_catalog_commit_carries_its_history_in_the_same_version ... ok
test catalog_commits_raise_journal_entry_events ... ok
test the_host_keeps_one_catalog_file_per_library_and_journals_its_commits ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/integration/library_routes.rs (<target>/debug/deps/integration_library_routes-cbefee7c09ebded3)

running 10 tests
test a_cancelled_or_unusable_pick_registers_nothing ... ok
test create_in_a_chosen_folder_answers_an_opaque_library_and_serves_its_files ... ok
test the_data_directory_keeps_only_each_library_s_index_never_its_files ... ok
test a_library_folder_copied_elsewhere_reopens_with_its_collections_and_registered_files ... ok
test the_backfill_route_runs_over_the_folders_the_owner_opened ... ok
test the_navigation_lists_the_installations_repository_and_its_libraries ... ok
test an_opened_folder_feeds_the_history_and_copies_and_rebuilds_without_paths ... ok
test the_owner_picks_a_folder_in_the_hosts_dialog_and_the_app_sees_only_its_id ... ok
test the_owners_preferences_are_kept_in_the_operational_store ... ok
test the_owners_consented_repair_is_kept_in_the_persistent_ledger ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.19s

     Running tests/features.rs (<target>/debug/deps/features-6df9d25a864353e9)
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
Feature: Desktop libraries in folders the owner chose
  Scenario: A library created in an empty chosen folder gets an opaque ID
   ✔> Given the desktop host has started its library routes
   ✔  Given the owner chose an empty folder
   ✔  When the app creates a library in that folder
   ✔  Then the answer should name a new library without any folder path
   ✔  And the desktop should serve that library's files
  Scenario: A completed snapshot is copied into another empty folder
   ✔> Given the desktop host has started its library routes
   ✔  Given the owner opened a folder that holds a library
   ✔  And the owner chose an empty folder
   ✔  When the app copies the library's snapshot into that folder
   ✔  Then the copy should succeed with no invalid object
   ✔  And the chosen folder should hold an OCFL storage root
  Scenario: The data directory keeps only the library's index, never its files
   ✔> Given the desktop host has started its library routes
   ✔  Given the owner chose an empty folder
   ✔  When the app creates a library in that folder
   ✔  Then the data directory should hold only that library's index
   ✔  And the chosen folder should hold the library's collection snapshot
  Scenario: A library folder copied elsewhere reopens with its collections
   ✔> Given the desktop host has started its library routes
   ✔  Given a library folder holding the collection "Holidays" was copied to a new place
   ✔  When the app opens the copied folder without naming its library
   ✔  Then the answer should name the library the copied folder holds
   ✔  And the library should list the collection "Holidays"
  Scenario: A second folder of a library that is already open is refused
   ✔> Given the desktop host has started its library routes
   ✔  Given the owner opened a folder that holds a library
   ✔  And a copy of that folder was chosen
   ✔  When the app opens the copied folder without naming its library
   ✔  Then the request should be refused as a conflict
  Scenario: A folder that already holds a file is not used for a new library
   ✔> Given the desktop host has started its library routes
   ✔  Given the owner chose a folder that holds a file
   ✔  When the app creates a library in that folder
   ✔  Then the request should be refused as a conflict
   ✔  And the folder should still hold only that file
  Scenario: A snapshot is never copied into a folder inside the library
   ✔> Given the desktop host has started its library routes
   ✔  Given the owner opened a folder that holds a library
   ✔  And the owner chose a folder inside that library
   ✔  When the app copies the library's snapshot into that folder
   ✔  Then the request should be refused as a conflict
  Scenario: A repair without the owner's consent changes nothing
   ✔> Given the desktop host has started its library routes
   ✔  Given the owner opened a folder that holds a library whose head lacks its version inventory
   ✔  When the app asks to repair it without the owner's consent
   ✔  Then the request should be refused as a conflict
   ✔  And the head should still lack its version inventory
  Scenario: An unknown folder is refused and journaled once without a path
   ✔> Given the desktop host has started its library routes
   ✔  When the app creates a library in a folder the host never registered
   ✔  Then the request should be refused as not found
   ✔  And the journal should record the refusal once without any folder path
  Scenario: The owner's consented repair is kept after the app restarts
   ✔> Given the desktop host has started its library routes
48 scenarios (48 passed)
206 steps (206 passed)
```

The full suites of `filesquid-api`, `filesquid-native` (all features), `filesquid-desktop`, `filesquid-server` and `filesquid-cli`, workspace clippy with `-D warnings` (and the desktop crate with `--features test-webdriver`) and the UI contract test pass. Full pre-commit run on the head commit:

```text
$ FULL=1 bash scripts/pre-commit.sh
✅ All Pre-Commit Hooks Passed! Test scope: full (466s)
```

## SRS mapping

| Requirement | Title | Evidence |
|---|---|---|
| [SRS-F-001](../../../../../../resources/srs.md#srs-f-001-independent-local-library-and-ownership) | Independent local library and ownership | `no_library_root_or_original_is_ever_created_in_the_catalog_directory`, `the_data_directory_keeps_only_each_library_s_index_never_its_files`, `a_library_folder_copied_elsewhere_reopens_with_its_collections_folders_and_files`; `desktop_libraries_F-001.feature` |
| [SRS-F-002](../../../../../../resources/srs.md#srs-f-002-safe-file-folder-and-device-import) | Safe file, folder and device import | `imported_files_appear_under_their_collection_and_folder`, `a_folder_imported_again_is_numbered_beside_the_first`, `an_import_without_a_destination_is_refused_before_any_dialog`; `desktop_imports_F-002.feature` |
| [SRS-F-004](../../../../../../resources/srs.md#srs-f-004-collections-folders-and-navigation) | Collections, folders and navigation | `a_collection_node_lists_its_folders_then_its_registered_files`, `a_folder_node_lists_the_files_beneath_it_under_their_numbered_names`, `folders_and_scope_entries_come_from_the_index_of_ordinary_files` |
| [DE-S-009](../srs.md) | Per-library SQLite catalog index (each library MUST have its own SQLite catalog index in the data directory, the library itself in an owner-chosen folder) | `a_version_2_catalog_is_migrated_to_version_3_and_rebuilt_from_its_folder`, `an_index_built_from_another_copy_of_the_folder_is_rebuilt`, `a_retired_in_app_root_is_neither_opened_nor_served`, `the_library_s_own_retention_survives_a_rebuild_and_sets_purge_after` |
| [DE-S-010](../srs.md) | Every library change follows ADR 005's sequence (intent, OCFL version, then catalog) | `a_registered_file_names_its_path_and_object_in_the_snapshot_and_the_index`, `a_crash_between_a_file_s_object_and_its_registration_resumes_and_registers_once`, `deleting_a_collection_deletes_its_files_in_place_in_one_version`, `file_objects_the_snapshot_does_not_register_are_counted` |
| [DE-S-018](../srs.md) | Library routes over owner-chosen folders | `integration_library_routes` (open by snapshot, second folder refused, copied folder reopens); `CatalogFolders` and `CatalogScopes` in `integration_imports` |
| [DE-S-019](../srs.md) | Desktop file and folder import | `the_destination_reaches_the_host`, `a_plan_without_a_destination_fails_its_job_as_destination_required`, `a_taken_path_is_numbered_beside_it_and_a_deleted_file_s_path_counts`, `a_source_whose_numbers_are_all_taken_is_quarantined_as_path_suffix_exhausted`, `a_numbered_name_over_255_bytes_is_quarantined_as_name_too_long` |
| [DE-S-006](../srs.md) | The webview MUST receive opaque handles and API responses, never raw filesystem paths or SQL | `a_scope_listing_names_no_filesystem_path`; path and name assertions on answers and journal entries in `integration_imports` and `integration_library_catalog` |
| [API-S-011](../../../api/resources/srs.md) | Paged view, copy and move (**Same logical path**) | `unit_collection_path` (15 tests: `a_taken_path_gets_the_first_free_number_beside_it`, `a_file_takes_the_number_before_its_first_dot`, `a_folder_takes_the_number_at_its_end`, `the_9999th_number_is_still_tried`, `the_search_stops_after_9999_numbers`, `a_numbered_segment_over_255_bytes_is_name_too_long`) |
| [API-S-019](../../../api/resources/srs.md) | Library domain commands and queries | `deleting_a_collection_deletes_its_files_in_place`, `deleting_a_collection_keeps_the_times_of_files_already_deleted`, `a_library_s_own_retention_sets_the_purge_after_time`; `library_domain_commands_and_queries_API-S-019.feature` |
| [API-S-021](../../../api/resources/srs.md) | Navigation listing | `integration_scope_listing` (`a_scope_listing_pages_with_an_opaque_cursor` and three more) |
| [API-S-025](../../../api/resources/srs.md) | Local library and repair routes | `integration_library_routes` open-by-snapshot tests (`library_mismatch`, `library_already_open`) |
| [API-S-026](../../../api/resources/srs.md) | The FileSquid OCFL application profile | `a_file_entity_records_its_logical_path_and_object`, `a_file_entity_of_the_earlier_form_is_read_and_reported_unregistered`, `a_file_entity_with_a_path_outside_the_rule_is_refused_with_a_fixed_reason`, `the_library_entity_keeps_its_own_retention_period`, `a_file_object_without_a_snapshot_entry_is_counted_and_left_unregistered` |
| [API-S-036](../../../api/resources/srs.md) | Import route | `unit_imports_route` (`an_import_without_a_destination_is_refused_before_any_dialog`, `an_unknown_or_foreign_collection_is_not_found_before_any_dialog`, `a_destination_folder_outside_the_path_rule_is_refused_before_any_dialog`); `unit_contract` (0.27.0) |
