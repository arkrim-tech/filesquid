# Task API-F2.18 Walkthrough: Library facts: name, free space and recent libraries

Branch `feat/api-f2.18-library-facts`, modules `filesquid-api` ([plan row](../plan.md)), `filesquid-native` and `filesquid-desktop` (the desktop's location registry), with `filesquid-server`'s answer. Part of global Task 1.2. Contract 0.30.0 (main was 0.29.0, after CO-0.11); `ui/src/api/schema.d.ts` and `contract-version.ts` regenerated with `npm run contract:types`.

## SRS mapping

| SRS ID and title | Covered by |
|---|---|
| [API-S-038](../srs.md) Library facts: a library name set at create and renamable, journaled; `GET /api/v1/libraries/{library}/facts` with the free bytes of its volume or the reason they are unknown; `GET /api/v1/libraries/recent`, readable libraries first, at most 20, newest first, never a path | `integration_library_facts` (10 tests), `library_facts_API-S-038.feature` (6 scenarios), `unit_library_host` (registry and free space), the desktop's `a_named_library_answers_its_facts_and_is_listed_as_recent_without_its_folder`, the server's `the_server_keeps_no_library_registry_so_the_library_facts_are_unavailable` |
| [API-S-019](../srs.md) Library domain commands and queries: names and titles of 1 to 1,024 bytes, not blank | `a_name_that_breaks_the_rules_is_refused_before_anything_is_created` |
| [API-S-005](../srs.md) No path, storage key or location ID in an answer, error or journal entry | the redaction assertions of every test above |
| [API-S-004](../srs.md), [API-S-017](../srs.md) Each route declares its action; every refusal journaled once | `unit_auth`, `unit_contract`, the refusal tests |
| [API-S-002](../srs.md) Versioned contract and generated types | `unit_contract` (drift, samples, documented answers), the UI's contract drift test |

## What changed

- **Name.** `LocationRequest` (create and open) takes an optional `name`: kept when given; a library first bound without one is named "Untitled library", so every library has a name. A blank name, one over 1,024 bytes or one with a control character is refused as `400 invalid_request` with the code `invalid_name`, journaled once, before anything is bound or written.
- **Rename.** `POST /api/v1/libraries/renames` (`library.manage`) takes a plain body `{library_id, name}` (owner decision 1 of 2026-10-23: the envelope arrives with API-F2.19) and answers the facts. It is journaled as `library.rename` with the library ID only, never the name; a rename to the current name changes nothing and is not journaled; a library this host never opened is `404` (`library_not_found`).
- **Facts.** `GET /api/v1/libraries/{library}/facts` (`library.read`) answers `{library_id, name, free_bytes, free_bytes_reason}`. A library the caller may not read is refused exactly as an unknown one (`404 not_found`), journaled once as `library.facts`. Readability is the navigation access the host installed (API-S-021), else this release's host access, so the server administrator, who owns no library, reads the facts of a library it may read.
- **Recent.** `GET /api/v1/libraries/recent` (`library.read`) keeps the readable libraries first, then lists at most 20 of them newest first with `library_id`, `name`, `last_opened` (RFC 3339 UTC), `open` and the free bytes; never a path or location ID.
- **Host port.** `LibraryRegistry` (remember, rename, entry, entries, free space). `RegistryBook` holds the per-library entries in memory, at most 256, forgetting the least recently opened; `MemoryLibraryRegistry` is the book with an injected free-space reader. `LibraryCommands::with_registry` remembers each library it creates or opens, with the service's clock; without a registry the three routes answer `503 unavailable`.
- **Desktop.** `HostLocations` (DE-1.1's location registry) implements the port with a `RegistryBook` for the host's lifetime and answers free bytes with `fs4::available_space` on Linux and macOS (`fs4` is already a dependency of `filesquid-native`, so the lockfile is unchanged); any other build answers `volume_unreadable` (owner decision 5 of 2026-10-23; DE-F3.3 adds the Win32 read). `DesktopLibraries` installs it.
- **Server.** `filesquid-server` installs no library commands, so the facts, recent and rename routes answer `503 unavailable` there, as the other library routes do.
- **Dependency line with DE-1.8** (orchestrator, 2026-10-23): this task defines the API side and the desktop implementation of the port over the existing location registry, keeping each library's name and last opened time for the host's lifetime; DE-1.8 later adds what its row still claims, keeping the registry across launches among it. Recorded in both rows' Status.

TDD: `integration_library_facts` was written before the implementation and did not compile against the old API (`unresolved imports filesquid_api::local_library::FreeSpace, LibraryRegistry, FACTS_OPERATION, RENAME_OPERATION, UNTITLED_LIBRARY`; `no method named with_registry`).

Known limits: the registry lives for the host's lifetime, so the recent list starts empty at each launch until DE-1.8 keeps it; a library's name lives in the registry and in the library entity of the folder's snapshot, which the host keeps in step (Cubic round 3): a new library's entity takes the bound name, a reopened, moved or copied folder names the registry, and a rename retitles the entity in a snapshot version journaled as `library.entity.rename` (the library ID only), so the navigation title follows the name (API-S-038).

## Test output

`cargo test` of `filesquid-api` (`integration_library_facts`, `unit_contract`), `filesquid-native` (`unit_library_host`, feature `desktop`), `filesquid-desktop` (`integration_library_routes`) and `filesquid-server` (`integration_host`):

```text
     Running tests/integration/library_facts.rs (<target>/debug/deps/integration_library_facts-5ae061bbd5d57a20)

running 10 tests
test a_library_this_host_remembers_but_has_not_opened_now_is_listed_as_not_open ... ok
test a_library_is_created_with_its_name_and_the_facts_answer_it ... ok
test a_library_created_without_a_name_is_untitled_and_open_may_name_it ... ok
test a_rename_of_an_unknown_library_or_to_a_bad_name_is_refused_once ... ok
test a_name_that_breaks_the_rules_is_refused_before_anything_is_created ... ok
test an_unknown_or_unreadable_library_is_not_found_and_journaled_once ... ok
test a_rename_is_journaled_with_the_library_only_and_a_same_name_changes_nothing ... ok
test free_bytes_are_null_with_their_reason_when_the_volume_cannot_be_read ... ok
test without_a_registry_the_facts_and_recent_routes_are_unavailable ... ok
test recent_lists_at_most_20_readable_libraries_newest_first_without_paths ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

     Running tests/unit/contract.rs (<target>/debug/deps/unit_contract-27eeb267071a4e20)

running 28 tests
test error_code_all_lists_every_variant ... ok
test contract_is_openapi_3_1_with_the_crate_api_version ... ok
test error_codes_match_the_rust_enum_with_status_and_retryability ... ok
test authentication_before_routing_is_documented_as_a_wire_convention ... ok
test command_envelope_carries_idempotency_key_expected_revision_and_scope ... ok
test errors_carry_code_and_retryability ... ok
test event_topics_in_the_contract_match_the_rust_topics ... ok
test every_component_schema_has_a_rust_sample ... ok
test every_component_schema_matches_its_serialized_rust_type ... ok
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
test events_from_every_publisher_match_their_documented_payloads ... ok
test every_documented_operation_answers_a_documented_status_and_body ... ok
test router_fallbacks_answer_as_documented ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.86s

     Running tests/unit/library_host.rs (<target>/debug/deps/unit_library_host-79dec963283aa766)

running 14 tests
test a_file_or_a_missing_folder_is_refused_without_its_path ... ok
test a_folder_that_overlaps_the_data_directory_is_refused_and_prepares_nothing ... ok
test a_host_chosen_id_must_be_well_formed_and_unique ... ok
test a_registered_folder_gets_an_opaque_id_without_its_path ... ok
test a_refused_id_prepares_no_storage ... ok
test a_symbolic_link_is_refused_as_a_location ... ok
test free_space_is_read_on_linux_and_macos_and_unreadable_for_an_unknown_location ... ok
test one_folder_chosen_twice_keeps_its_id_and_a_second_host_id_shares_its_leases ... ok
test nested_and_unknown_locations_overlap_and_siblings_do_not ... ok
test the_registry_keeps_each_librarys_name_and_last_opened_time_without_its_folder ... ok
test concurrent_choices_of_one_folder_get_one_id ... ok
test the_stores_refuse_a_symbolic_link_in_place_of_the_operational_store ... ok
test the_ledger_journals_consents_and_receipts_and_keeps_them_across_a_restart ... ok
test the_stores_open_through_a_linked_folder_above_the_operational_store ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests/integration/library_routes.rs (<target>/debug/deps/integration_library_routes-be26db170b910b92)

running 9 tests
test a_cancelled_or_unusable_pick_registers_nothing ... ok
test create_in_a_chosen_folder_answers_an_opaque_library_and_serves_its_files ... ok
test a_named_library_answers_its_facts_and_is_listed_as_recent_without_its_folder ... ok
test the_navigation_lists_the_installations_repository_and_its_libraries ... ok
test the_owner_picks_a_folder_in_the_hosts_dialog_and_the_app_sees_only_its_id ... ok
test the_owners_preferences_are_kept_in_the_operational_store ... ok
test the_backfill_route_runs_over_the_folders_the_owner_opened ... ok
test an_opened_folder_feeds_the_history_and_copies_and_rebuilds_without_paths ... ok
test the_owners_consented_repair_is_kept_in_the_persistent_ledger ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s

     Running tests/integration/host.rs (<target>/debug/deps/integration_host-fa2ced0391f5b9de)

running 12 tests
test a_non_unicode_variable_is_refused_without_echoing_it ... ok
test an_unusable_operator_name_refuses_to_start_without_echoing_the_value ... ok
test the_binary_refuses_a_non_unicode_operator_name ... ok
test the_binary_prints_the_warning_even_with_logging_off ... ok
test a_named_server_describes_itself_and_writes_no_warning ... ok
test the_operator_name_is_read_trimmed_from_the_environment ... ok
test the_operator_unnamed_entry_carries_nothing_private ... ok
test an_unnamed_server_starts_with_one_warning_on_stdout_and_in_the_journal ... ok
test without_a_journal_layer_the_warning_is_journaled_directly_once ... ok
test the_server_offers_no_folder_dialog_or_backfill_launcher_yet ... ok
test the_server_keeps_no_library_registry_so_the_library_facts_are_unavailable ... ok
test the_warning_is_written_at_every_start ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

BDD, `cargo test -p filesquid-api --locked --test features` (the new feature, then the whole run):

```text
Feature: Library facts: name, free space and recent libraries
  Scenario: A library created with a name answers its name and free space
   ✔  Given the owner has created the library "Family archive"
   ✔  When the owner reads the library's facts
   ✔  Then the facts should name it "Family archive" with 1000000 free bytes
  Scenario: A rename is journaled with the library only
   ✔  Given the owner has created the library "Family archive"
   ✔  When the owner renames the library to "Family archive 1990s"
   ✔  Then the facts should name it "Family archive 1990s" with 1000000 free bytes
   ✔  And the journal should record one library rename naming the library and not the name
  Scenario: A volume that cannot be read answers null free bytes with the reason
   ✔  Given the owner has created the library "Remote"
   ✔  And the library's volume is not local
   ✔  When the owner reads the library's facts
   ✔  Then the free bytes should be null because "not_local"
  Scenario: The recent list keeps readable libraries first, then the 20 newest
   ✔  Given the owner has opened 22 libraries one after another
   ✔  And the caller may not read the newest one
   ✔  When the caller lists the recent libraries
   ✔  Then 20 libraries should be listed, newest readable first, with no path or location ID
  Scenario: A name that breaks the rules creates nothing
   ✔  When the owner creates a library named with 1025 bytes
   ✔  Then the creation should be refused with "invalid_name" and nothing written
  Scenario: An unreadable library is not found
   ✔  Given the owner has created the library "Hidden"
   ✔  And the caller may not read that library
   ✔  When the owner reads the library's facts
   ✔  Then the facts should be refused as not found
```

```text
[Summary]
28 features
380 scenarios (278 passed, 102 skipped)
1340 steps (1238 passed, 102 skipped)
```

The head-commit hook run (`FULL=1 bash scripts/pre-commit.sh`, `Test scope: full`) is recorded in the pull request.

## Cubic round 3 (the whole PR after the merge of main)

The P1 finding: the name only reached the host registry, so the navigation title stayed "Library". Now the name is kept in step with the library entity of the folder's snapshot (`SqliteCatalog::library_name` and `rename_library`, journaled as `library.entity.rename` with the library ID only; the desktop's `align_name` after every attach and its retitle after every rename). The feature gained the blank, control-character and 1,024-byte name cases and uses one actor in the unreadable-library scenario. Run from `apps/filesquid/` with the shared target:

```text
     Running tests/unit/folder_libraries.rs (<target>/debug/deps/unit_folder_libraries-1d70d3f4ec7a989a)

running 23 tests
test a_create_library_command_is_refused_and_creates_nothing ... ok
test a_folder_that_cannot_be_a_library_leaves_nothing_in_the_catalog_directory ... ok
test a_file_s_path_is_taken_below_it_and_no_file_sits_below_a_file ... ok
test a_file_object_without_a_snapshot_entry_is_counted_and_left_unregistered ... ok
test a_file_of_the_earlier_form_is_placed_when_it_is_registered ... ok
test a_library_folder_copied_elsewhere_reopens_with_its_collections_folders_and_files ... ok
test a_retired_in_app_root_is_neither_opened_nor_served ... ok
test a_refused_registration_is_journaled_as_failed_with_its_code ... ok
test a_registration_whose_numbers_are_all_taken_is_refused_and_changes_nothing ... ok
test a_registered_file_names_its_path_and_object_in_the_snapshot_and_the_index ... ok
test an_empty_folder_attached_with_an_index_of_another_folder_starts_afresh ... ok
test an_index_write_for_a_library_that_is_not_attached_is_refused ... ok
test a_version_2_catalog_is_migrated_to_version_3_and_rebuilt_from_its_folder ... ok
test an_index_built_from_another_copy_of_the_folder_is_rebuilt ... ok
test a_taken_path_is_numbered_beside_it_and_a_deleted_file_s_path_counts ... ok
test no_index_is_created_beyond_the_catalog_s_limit_of_files ... ok
test files_deleted_with_their_collection_are_deleted_not_unfiled ... ok
test no_library_root_or_original_is_ever_created_in_the_catalog_directory ... ok
test removing_a_collection_that_still_holds_files_is_refused_before_anything_is_published ... ok
test renaming_a_library_retitles_its_entity_in_one_journaled_version_and_a_same_name_rename_publishes_nothing ... ok
test deleting_a_collection_deletes_its_files_in_place_in_one_version ... ok
test the_library_s_own_retention_survives_a_rebuild_and_sets_purge_after ... ok
test folders_and_scope_entries_come_from_the_index_of_ordinary_files ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s
```

```text
     Running tests/integration/library_routes.rs (<target>/debug/deps/integration_library_routes-be26db170b910b92)

running 11 tests
test a_cancelled_or_unusable_pick_registers_nothing ... ok
test create_in_a_chosen_folder_answers_an_opaque_library_and_serves_its_files ... ok
test a_named_library_answers_its_facts_and_is_listed_as_recent_without_its_folder ... ok
test a_library_folder_copied_elsewhere_reopens_with_its_collections ... ok
test the_data_directory_keeps_only_each_library_s_index_never_its_files ... ok
test the_navigation_lists_the_installations_repository_and_its_libraries ... ok
test an_opened_folder_feeds_the_history_and_copies_and_rebuilds_without_paths ... ok
test the_owners_preferences_are_kept_in_the_operational_store ... ok
test the_owner_picks_a_folder_in_the_hosts_dialog_and_the_app_sees_only_its_id ... ok
test the_backfill_route_runs_over_the_folders_the_owner_opened ... ok
test the_owners_consented_repair_is_kept_in_the_persistent_ledger ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
```

```text
Feature: Library facts: name, free space and recent libraries
  Scenario: A library created with a name answers its name and free space
   ✔  Given the owner has created the library "Family archive"
   ✔  When the owner reads the library's facts
   ✔  Then the facts should name it "Family archive" with 1000000 free bytes
  Scenario: A rename is journaled with the library only
   ✔  Given the owner has created the library "Family archive"
   ✔  When the owner renames the library to "Family archive 1990s"
   ✔  Then the facts should name it "Family archive 1990s" with 1000000 free bytes
   ✔  And the journal should record one library rename naming the library and not the name
  Scenario: A volume that cannot be read answers null free bytes with the reason
   ✔  Given the owner has created the library "Remote"
   ✔  And the library's volume is not local
   ✔  When the owner reads the library's facts
   ✔  Then the free bytes should be null because "not_local"
  Scenario: The recent list keeps readable libraries first, then the 20 newest
   ✔  Given the owner has opened 22 libraries one after another
   ✔  And the caller may not read the newest one
   ✔  When the caller lists the recent libraries
   ✔  Then 20 libraries should be listed, newest readable first, with no path or location ID
  Scenario: A name of exactly 1,024 bytes is the longest the rules allow
   ✔  When the owner creates a library named with 1024 bytes
   ✔  Then the library should be created with its 1024-byte name
  Scenario Outline: A name that breaks the rules creates nothing
   ✔  When the owner creates a library named with 1025 bytes
   ✔  Then the creation should be refused with "invalid_name" and nothing written
  Scenario Outline: A name that breaks the rules creates nothing
   ✔  When the owner creates a library with a blank name
   ✔  Then the creation should be refused with "invalid_name" and nothing written
  Scenario Outline: A name that breaks the rules creates nothing
   ✔  When the owner creates a library with a control character in its name
   ✔  Then the creation should be refused with "invalid_name" and nothing written
  Scenario: An unreadable library is not found
   ✔  Given the owner has created the library "Hidden"
   ✔  And the caller may not read that library
   ✔  When the caller reads the library's facts
   ✔  Then the facts should be refused as not found

```

```text
[Summary]
28 features
388 scenarios (286 passed, 102 skipped)
1377 steps (1275 passed, 102 skipped)
```

```text
     Running tests/integration/library_facts.rs (<target>/debug/deps/integration_library_facts-5ae061bbd5d57a20)

running 12 tests
test a_library_this_host_remembers_but_has_not_opened_now_is_listed_as_not_open ... ok
test a_caller_who_may_not_read_the_repository_reads_no_library_facts ... ok
test a_library_created_without_a_name_is_untitled_and_open_may_name_it ... ok
test a_library_is_created_with_its_name_and_the_facts_answer_it ... ok
test a_name_given_on_open_that_replaces_another_is_journaled_as_a_rename ... ok
test a_rename_of_an_unknown_library_or_to_a_bad_name_is_refused_once ... ok
test a_name_that_breaks_the_rules_is_refused_before_anything_is_created ... ok
test a_rename_is_journaled_with_the_library_only_and_a_same_name_changes_nothing ... ok
test an_unknown_or_unreadable_library_is_not_found_and_journaled_once ... ok
test without_a_registry_the_facts_and_recent_routes_are_unavailable ... ok
test free_bytes_are_null_with_their_reason_when_the_volume_cannot_be_read ... ok
test recent_lists_at_most_20_readable_libraries_newest_first_without_paths ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
```

## Mocked Environment Variables

None. The tests use real filesystem OCFL roots under the system temporary directory (`TMPDIR`) and remove them.
