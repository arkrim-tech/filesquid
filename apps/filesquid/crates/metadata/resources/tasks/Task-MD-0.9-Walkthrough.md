# Task MD-0.9 Walkthrough: Commit options on the `ocfl` store

Branch: `feat/prov-o-history` (global Task FS-2.8; ADR 040 Decision 6).

## MD-S-014: Commit options

Also MD-S-008 (each edit is one new OCFL version; a no-op save commits nothing).

## What changed

- `store::ocfl::CommitOptions {user, message, bound}` and `OcflStore::commit_record_with`. These take the caller's OCFL version `user`, the version message (FileSquid passes `filesquid.operation.v1`, replacing `Edit metadata.md`) and version-bound files (IO-1.5), all committed in the same version. The method returns the `CommitReceipt`, so the caller can journal the commit (`took_over` included).
- `commit_record` is `commit_record_with` with default options, which keep the stand-alone behaviour: message `Edit metadata.md`, no `user`, no bound files.
- A no-op save commits nothing and never calls the bound-file function. A failed commit, such as a failing bound-file function, clears the change set it staged.

## Dependency note

The plan lists Task MD-0.8 (import file record) as a dependency. MD-0.8 is not done. The commit options do not depend on its content, so MD-0.9 was implemented over the current store. MD-0.8 will reuse these options.

## SRS traceability

| Requirement | Tests |
|---|---|
| MD-S-014 | `unit_store::commit_options::*` (4 tests); BDD "A caller's commit options go into the same OCFL version" |
| MD-S-008 | `unit_store` per-edit tests (unchanged, pass), `a_no_op_save_commits_nothing_and_binds_no_files` |

## TDD: failing run before implementation

```text
      1 error: could not compile `filesquid-metadata` (test "unit_store") due to 5 previous errors
      1 error[E0432]: unresolved import `filesquid_metadata::store::ocfl::CommitOptions`
      4 error[E0599]: no method named `commit_record_with` found for struct `OcflStore<'a, B, R, S, C, L>` in the current scope
```

## Mocked Environment Variables

None.

## Raw test output

`cargo test -p filesquid-metadata --all-features --locked --test unit_store`:

```text
    Finished `test` profile [unoptimized] target(s) in 5.98s
     Running tests/unit/store.rs (<target>/debug/deps/unit_store-eab858085c3416e1)

running 28 tests
test a_missing_object_or_metadata_md_is_an_empty_record ... ok
test a_csv_line_break_survives_commit_and_load ... ok
test a_committed_record_that_is_not_utf8_is_an_error ... ok
test a_head_without_its_version_inventory_stays_refused ... ok
test an_object_root_that_could_escape_is_an_invalid_path ... ok
test a_stale_change_set_of_this_writer_is_discarded_before_staging ... ok
test an_unchanged_record_publishes_nothing ... ok
test an_edit_never_modifies_an_earlier_version ... ok
test commit_options::a_failing_bound_file_function_publishes_nothing ... ok
test a_pinned_version_that_is_not_the_head_is_a_conflict ... ok
test commit_options::user_message_and_bound_files_are_committed_in_the_same_version ... ok
test commit_options::a_no_op_save_commits_nothing_and_binds_no_files ... ok
test local_object::a_git_working_tree_is_refused ... ok
test file_sha256_hashes_the_logical_file_through_filesquid_ocfl ... ok
test local_object::a_new_object_directory_needs_an_identifier ... ok
test commit_options::without_options_the_store_keeps_its_stand_alone_defaults ... ok
test each_edit_commits_exactly_one_new_version ... ok
test local_object::another_identifier_is_refused ... ok
test metadata_md_is_a_logical_file_inside_a_version ... ok
test local_object::each_save_is_a_new_version_of_a_valid_object ... ok
test reading_a_root_that_holds_another_object_is_refused ... ok
test local_object::an_edit_from_a_head_another_writer_moved_is_a_conflict ... ok
test staging_is_cleared_after_each_edit ... ok
test the_first_edit_of_a_new_object_creates_v1 ... ok
test local_object::the_digest_of_a_missing_logical_file_is_an_error ... ok
test per_edit_versions_on_a_filesystem_backend ... ok
test two_local_objects_in_one_process_stage_apart ... ok
test records_larger_than_one_put_round_trip ... ok

test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.71s
```
