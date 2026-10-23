# Task PV-0.2 Walkthrough: `ocfl` feature: records in OCFL objects and reconstruction

Branch: `feat/prov-o-history` (global Task FS-2.8; ADR 040 Decisions 3 and 7).

## PROV-S-006: `ocfl` feature

Also OCFL-S-024 (the record is written through the change-model hook) and PROV-S-001 (the feature is optional, off by default, adds only `filesquid-ocfl` with its default features off, and still builds for `wasm32-unknown-unknown`).

## What changed

- Cargo feature `ocfl = ["dep:filesquid-ocfl"]`. The isolation test allows exactly one optional dependency, `filesquid-ocfl`.
- `ocfl::RecordHook` (a `VersionBoundFiles`) and `ocfl::bound_record` / `version_facts` build the record of the commit in hand from `OperationFacts` and the change model's `BoundContext`. The record is written at `provenance/<version>.jsonld`, named after the reserved version and its `created` time, and regenerated after a rebase.
- `read_record` streams a record bounded by 1 MiB. It looks in the head first, because a reconstructed record first appears in the version that backfilled it, and in the version itself second.
- `list_records` and `missing_records` work from the head's state.
- `reconstruct` derives a record from an inventory alone:
  - `created` is the end (and start) time;
  - the `user` address is the installation agent only when it is `urn:uuid:<uuid v4>`;
  - the operation ID and kind come from a `filesquid.operation.v1` message, otherwise `unrecorded` and `unknown`;
  - entities come from the state diff with the previous version;
  - the record is marked `reconstructed` with its own `recordedAt`, and no agent is invented.
- `stage_reconstructed` stages the missing records through the change model. It is capped per run and stages nothing for versions that already have one, so it is idempotent. The caller commits them with the run's own record. Nothing writes outside the change model.
- The pre-commit gate (`scripts/verify_tests.sh` step 5c) runs clippy and tests with `--all-features` and both wasm32 builds (default features, `--features ocfl`).

## SRS traceability

| Requirement | Tests |
|---|---|
| PROV-S-006 | `unit_ocfl` (12 tests): record per commit, rebase regeneration, listing, reconstruction without invented agents, backfill in one version and idempotent, cap per run |
| PROV-S-001 | `unit_isolation::library_dependencies_are_serde_serde_json_and_thiserror` (one optional dependency), wasm32 check with `--features ocfl` |

## TDD: failing run before implementation

```text
 apps/filesquid/Cargo.lock | 1 +
 1 file changed, 1 insertion(+)
      1 error: could not compile `filesquid-prov` (test "unit_ocfl") due to 1 previous error
      1 error[E0432]: unresolved import `filesquid_prov::ocfl`
```

## Mocked Environment Variables

None.

## Raw test output

`cargo test -p filesquid-prov --all-features --locked --test unit_ocfl --test unit_isolation`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.42s
     Running tests/unit/isolation.rs (<target>/debug/deps/unit_isolation-c337441e5dc3acaa)

running 9 tests
test a_default_filesquid_dependency_would_fail ... ok
test a_feature_naming_native_or_c2pa_would_fail ... ok
test an_optional_filesquid_dependency_in_default_would_fail ... ok
test library_dependencies_are_serde_serde_json_and_thiserror ... ok
test the_crate_is_mit_licensed_and_unpublished_for_now ... ok
test the_crate_root_denies_missing_docs ... ok
test the_manifest_keeps_every_isolation_rule ... ok
test hosts_sqlite_and_media_crates_would_fail ... ok
test the_library_does_no_filesystem_network_or_process_io ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/unit/ocfl.rs (<target>/debug/deps/unit_ocfl-1071fd6097652e4b)

running 7 tests
test a_reconstructed_record_invents_no_agent_and_reads_the_operation_message ... ok
test a_commit_writes_its_record_named_after_its_version ... ok
test backfill_is_capped_per_run ... ok
test record_paths_follow_the_version_name ... ok
test a_rebased_commit_regenerates_its_record_for_the_new_version ... ok
test records_are_listed_and_a_version_without_one_reads_as_none ... ok
test backfill_stages_every_missing_record_and_commits_them_in_one_version ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

## Review round 1

Cubic's findings on the crate are fixed: timestamps refuse over-long fractions and allow a leap second only at `23:59:60`; activity identifiers escape `:` in the operation ID so they are injective, and cursors validate the activity-ID form; `read_record` first checks that the version is committed (so a head record never answers for an uncommitted version), then still reads the head copy when the head holds the record, else the version's own; `missing_records` also counts a record held only in its own version as present; a record's agents must all be associated with its activity and every host must act on behalf of the installation; an invalidated entity cannot be of the committed version; the reader refuses entities under `provenance/`; reconstruction falls back per field on a malformed operation ID or kind and keeps the operation ID of a catalog message without a kind; `missing_records` uses a set; chained and swapped moves build; `stage_reconstructed` refuses another object's model; the isolation tests read every dependency table, dotted and quoted keys, underscore spellings and grouped `std` imports; the crate doc and README describe the `ocfl` feature.

`cargo test -p filesquid-prov --all-features`:

```text
test a_move_onto_a_kept_base_path_or_onto_itself_is_refused ... ok
test an_empty_diff_still_records_one_activity ... ok
test activity_is_associated_with_each_agent ... ok
test added_path_is_generated_without_derivation ... ok
test chained_moves_are_accepted_and_derive_from_each_source ... ok
test ended_time_is_the_created_time_and_started_is_kept ... ok
test exactly_one_activity_named_after_operation_object_and_version ... ok
test changed_path_is_generated_and_derived_from_its_previous_state ... ok
test inconsistent_moves_and_bases_are_refused ... ok
test generated_entities_are_attributed_to_the_installation_agent ... ok
test installation_is_a_plain_agent_and_host_acts_on_its_behalf ... ok
test installation_uuid_is_rendered_lowercase ... ok
test inputs_are_used_and_unchanged_paths_are_absent ... ok
test invalid_facts_are_refused_with_fixed_codes ... ok
test journal_entries_are_sorted_and_deduplicated ... ok
test moved_path_derives_from_the_old_paths_base_entity ... ok
test ocfl_user_address_names_the_installation_but_its_name_is_never_recorded ... ok
test reconstruction_is_marked_with_its_own_recording_time ... ok
test record_paths_are_not_described_by_their_own_record ... ok
test refusal_messages_name_no_stored_value ... ok
test ocfl_user_without_an_installation_urn_leaves_the_agent_unrecorded ... ok
test swapped_moves_are_accepted ... ok
test unobserved_agents_are_stated_as_unrecorded ... ok
test removed_path_is_invalidated_by_the_activity ... ok
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test a_leap_second_is_accepted_only_at_the_end_of_a_utc_day ... ok
test activity_identifier_form ... ok
test a_long_all_digit_fraction_is_refused_without_panicking ... ok
test activity_identifiers_are_injective_when_parts_hold_colons ... ok
test every_other_byte_is_upper_case_percent_encoded_utf8 ... ok
test installation_ids_are_uuid_v4_rendered_as_urn_uuid ... ok
test sha256_digests_are_64_hex_characters_in_lower_case ... ok
test timestamps_must_be_rfc3339_utc ... ok
test entity_identifier_form_encodes_the_logical_path ... ok
test timestamps_order_by_instant_not_by_text ... ok
test unreserved_characters_stay_as_they_are ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test a_cursor_after_an_activity_identifier_longer_than_one_part_round_trips ... ok
test a_cursor_after_an_operation_with_a_colon_round_trips ... ok
test activities_of_an_agent ... ok
test a_full_last_page_has_no_next_cursor ... ok
test activities_of_an_object_are_ordered_by_end_time ... ok
test cursors_travel_as_tokens ... ok
test page_size_must_be_between_one_and_a_thousand ... ok
test activities_of_an_entity_include_its_generation_and_its_use ... ok
test equal_end_times_are_ordered_by_activity_id ... ok
test every_page_size_yields_the_same_sequence_without_gaps ... ok
test records_are_kept_and_conflicting_duplicates_refused ... ok
test time_range_includes_from_and_excludes_to ... ok
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test a_default_filesquid_dependency_would_fail ... ok
test an_optional_dependency_table_is_read_as_optional ... ok
test a_feature_naming_native_or_c2pa_would_fail ... ok
test dotted_and_quoted_dependency_keys_would_fail ... ok
test dependency_tables_of_every_form_are_checked ... ok
test an_optional_filesquid_dependency_in_default_would_fail ... ok
test grouped_std_imports_of_io_modules_would_fail ... ok
test hosts_sqlite_and_media_crates_would_fail ... ok
test library_dependencies_are_serde_serde_json_and_thiserror ... ok
test the_crate_is_mit_licensed_and_unpublished_for_now ... ok
test the_crate_root_denies_missing_docs ... ok
test the_manifest_keeps_every_isolation_rule ... ok
test underscore_spellings_of_never_and_forbidden_packages_would_fail ... ok
test the_library_does_no_filesystem_network_or_process_io ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test a_catalog_message_without_a_kind_keeps_its_operation_id ... ok
test a_reconstructed_record_invents_no_agent_and_reads_the_operation_message ... ok
test a_commit_writes_its_record_named_after_its_version ... ok
test backfill_refuses_an_object_its_change_model_does_not_commit_to ... ok
test a_rebased_commit_regenerates_its_record_for_the_new_version ... ok
test a_record_path_in_the_head_never_answers_for_an_uncommitted_version ... ok
test malformed_operation_fields_fall_back_to_the_placeholders ... ok
test record_paths_follow_the_version_name ... ok
test backfill_is_capped_per_run ... ok
test records_are_listed_and_a_version_without_one_reads_as_none ... ok
test backfill_stages_every_missing_record_and_commits_them_in_one_version ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test a_host_acting_for_an_unrecorded_installation_is_read ... ok
test a_forged_entity_identifier_breaks_the_profile ... ok
test a_host_that_does_not_act_for_the_installation_breaks_the_profile ... ok
test a_wrong_context_breaks_the_profile ... ok
test a_person_agent_is_never_accepted ... ok
test an_agent_the_activity_is_not_associated_with_breaks_the_profile ... ok
test a_record_above_one_mebibyte_is_refused ... ok
test an_entity_of_a_record_path_breaks_the_profile ... ok
test bounds_are_one_mebibyte_and_sixteen_levels ... ok
test an_invalidated_entity_of_the_committed_version_breaks_the_profile ... ok
test dangling_references_break_the_profile ... ok
test malformed_json_and_invalid_utf8_are_refused ... ok
test nesting_of_exactly_sixteen_is_not_a_depth_refusal ... ok
test nesting_deeper_than_sixteen_is_refused ... ok
test exactly_one_activity_is_required ... ok
test non_utc_times_break_the_profile ... ok
test repeated_keys_are_refused_at_every_level ... ok
test unknown_or_missing_profiles_are_refused ... ok
test unknown_members_and_wrong_types_break_the_profile ... ok
test a_record_of_exactly_one_mebibyte_is_read ... ok
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test context_and_profile_are_embedded ... ok
test graph_lists_activity_then_agents_then_entities_by_identifier ... ok
test equal_inputs_give_equal_bytes ... ok
test no_person_and_no_display_name_anywhere ... ok
test member_and_node_order_do_not_change_the_parsed_record ... ok
test serialization_matches_the_profile_byte_for_byte ... ok
test reserved_tool_agent_kind_round_trips ... ok
test parse_round_trip_returns_the_same_record ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test crates/prov/src/ids.rs - ids::activity_id (line 54) ... ok
test crates/prov/src/ids.rs - ids::percent_encode (line 32) ... ok
test crates/prov/src/lib.rs - (line 34) ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
```

## FS-2.8 test gaps (2026-10-23)

Owner decision 3a of 2026-10-23: the Agent Prompt's tests against `FsBackend` as well as `MemoryBackend` are added on branch `test/fs-2.8-fsbackend-rebase-tests`. No defect was found; no library code changed. TDD-ENFORCE: not applicable, these are tests of code delivered in PR #86.

### PROV-S-006: `ocfl` feature (records in OCFL objects and reconstruction)

`tests/unit/ocfl.rs` builds its rig over any backend (`Rig::memory`, `Rig::fs`), and every test that stores an object runs on both: a record committed in its own version, a rebased commit regenerating its record, listing, reconstruction with and without a version `user` and structured `filesquid.operation.v1` messages (including malformed and catalog messages), backfill and its cap, the stray record path, the wrong-object refusal and the record a revert removed. `FsBackend` comes from a test-only dev-dependency on `filesquid-ocfl` with its default `fs` feature; the library's own dependency stays optional and without default features (`unit_isolation` reads only the library's dependency tables and still passes), and `Cargo.lock` is unchanged.

### Raw test output

`cargo test -p filesquid-prov --features ocfl --test unit_ocfl --locked`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.25s
     Running tests/unit/ocfl.rs (<target>/debug/deps/unit_ocfl-d400942d0cb9a2c6)

running 12 tests
test a_catalog_message_without_a_kind_keeps_its_operation_id ... ok
test a_commit_writes_its_record_named_after_its_version ... ok
test a_reconstructed_record_invents_no_agent_and_reads_the_operation_message ... ok
test a_rebased_commit_regenerates_its_record_for_the_new_version ... ok
test a_record_path_in_the_head_never_answers_for_an_uncommitted_version ... ok
test a_record_a_revert_removed_from_the_head_is_not_reconstructed ... ok
test backfill_refuses_an_object_its_change_model_does_not_commit_to ... ok
test record_paths_follow_the_version_name ... ok
test backfill_is_capped_per_run ... ok
test malformed_operation_fields_fall_back_to_the_placeholders ... ok
test records_are_listed_and_a_version_without_one_reads_as_none ... ok
test backfill_stages_every_missing_record_and_commits_them_in_one_version ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.17s
```
