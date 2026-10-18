# Task CL-0.2 Walkthrough: Library domain commands and queries

Module task [CL-0.2](../plan.md) of global [Task 0.10](../../../../../../resources/implementation-plan.md#task-0.10). Branch `feat/cl-0.2-domain-commands`, from `main` at `23a9662` (PR #27, CL-0.1). Dependencies CL-0.1 and global Task 0.2 were complete.

**Status: done.** `filesquid-api` gains the `library` module: the versioned library commands and queries, their rules, the `LibraryCatalog` port that hosts inject, and the in-memory `MemoryCatalog` reference store. The module does no storage I/O, so the API-S-006 guards stay green with no pins. No HTTP route or contract path is added, because a contract change would also change the UI's generated types; no row of the [module plan](../plan.md) names the library, collection, virtual-collection and membership routes yet (an open owner decision).

## API-S-019: Library domain commands and queries

The application layer MUST define versioned commands to create a library; to create, rename and delete collections and virtual collections; to add members to a hand-picked virtual collection and remove them; and to hide, unhide, delete, restore and permanently delete files. It MUST also define versioned queries for opening a library and for paged listing. It MUST store through a catalog the host injects, with no filesystem path or SQL ([module SRS](../srs.md)).

Parents: [F-001 Independent local library and ownership](../../../../../../resources/prd.md#f-001-independent-local-library-and-ownership), [F-049 Product component terminology and item type](../../../../../../resources/prd.md#f-049-product-component-terminology-and-item-type), [F-038 Open interfaces and product exit](../../../../../../resources/prd.md#f-038-open-interfaces-and-product-exit), [F-004 Collections, folders and navigation](../../../../../../resources/prd.md#f-004-collections-folders-and-navigation), [F-008 Deletion, hidden files and duplicate review](../../../../../../resources/prd.md#f-008-deletion-hidden-files-and-duplicate-review).

| Constraint | Gate | State |
|---|---|---|
| C1 New IDs are UUID v4 from the OS CSPRNG, lowercase 8-4-4-4-12 | `minted_identifiers_are_canonical_uuid_v4`, `uuid_check_rejects_other_shapes`, scenario 1 | Met |
| C2 A folder, collection, virtual collection or file outside the named library is `not_found` | `references_to_another_library_are_not_found`, `listing_a_scope_of_another_library_is_not_found`, `create_collection_needs_a_folder_of_its_library` | Met |
| C3 Create-library sends revision `0` and its commit finds the library absent; a stale revision is `conflict` with no change; a failed commit leaves the catalog unchanged | `create_library_needs_the_new_library_revision`, `a_stale_expected_revision_is_a_conflict_without_change`, `a_failed_commit_leaves_the_memory_catalog_unchanged`, `a_later_create_library_change_cannot_replace_a_library`, stale-revision scenario | Met |
| C4 A repeated operation ID answers the first outcome (including `deleted_at` and `purge_after`), with no second change; the stored receipt is answered before the revision check | `a_repeated_operation_id_returns_the_first_outcome`, `a_replayed_delete_answers_its_first_deletion`, `a_no_op_is_journaled_and_replays_under_its_operation_id`, retry scenario | Met |
| C5 A file batch names 1 to 1,000 distinct files | `file_batches_hold_one_to_one_thousand_distinct_files` (0, a repeat and 1,001 refused; exactly 1,000 accepted) | Met |
| C6 Names are non-blank, at most 1,024 bytes | `names_must_be_non_empty_and_bounded` | Met |
| C7 Deleting a collection or virtual collection keeps its files | `deleting_a_collection_retains_its_files_as_unfiled`, `deleting_a_virtual_collection_retains_its_members`, scenario 2 | Met |
| C8 Removing a virtual-collection member keeps its collection and other memberships | `virtual_collection_membership_leaves_the_home_collection_alone`, scenario 3 | Met |
| C9 A rule-based virtual collection refuses hand-picked members | `rule_based_virtual_collections_refuse_hand_picked_members`, scenario 4 | Met |
| C10 Deleted files stay recoverable for 30 days, with `deleted_at` and `purge_after` given for the files the command newly deletes (a file already deleted keeps its earlier times); restoring a file keeps it hidden if it was hidden | `deletion_is_recoverable_for_thirty_days_and_restore_keeps_hidden`, `a_mixed_delete_batch_answers_the_times_of_the_files_it_newly_deletes`, scenario 5, `deletion_policy_query_reports_thirty_days` | Met |
| C11 Permanent deletion applies only to deleted files, with a confirmation that names the library and the exact file count; a purged file and its memberships are gone | `permanent_deletion_needs_deleted_files_and_an_exact_confirmation`, scenario 6 | Met |
| C12 The ordinary view leaves out hidden and deleted files; the hidden and deleted views show them | `ordinary_listing_excludes_hidden_and_deleted_files`, scenario 5 | Met |
| C13 Pages hold 1 to 1,000 items in a stable order; the cursor is opaque; a malformed or non-canonical cursor is refused; every issued cursor decodes | `paging_is_stable_with_no_repeats_or_gaps`, `the_last_page_has_no_cursor`, `cursors_are_opaque_and_malformed_ones_are_refused`, `every_issued_cursor_decodes_and_only_canonical_text_is_accepted`, `listing_libraries_pages_by_repository`, paging scenario | Met |
| C14 Each commit, including one that changes nothing, carries one journal entry, written in the same transaction, holding no name or title | `every_commit_carries_one_redacted_journal_entry`, `a_no_op_is_journaled_and_replays_under_its_operation_id`, journal scenario | Met |
| C15 Refused commands, and catalog failures behind queries, are journaled exactly once as `failed` through the bounded queue, which `LibraryService::new` requires | `refused_commands_are_journaled_as_failed`, `catalog_failures_are_retryable_and_journaled_for_queries_too`, `catalog_failures_are_journaled_exactly_once_with_the_journal_layer_installed` | Met |
| C16 No storage I/O or forbidden crate | `unit_isolation` (13 passed), API-S-006 feature | Met |
| C17 Errors map to `ApiError` and `PortError` | `errors_map_to_api_and_port_errors` | Met |
| C18 A command that changes nothing (a present member, the same title) answers `committed` at the current revision, with no new revision and no OCFL version | `adding_a_present_member_is_a_no_op_that_commits_nothing`, `renaming_to_the_same_title_commits_nothing` | Met |
| C19 New virtual-collection members are ordered after the current last member, in the order named | `membership_batches_keep_their_order_after_the_last_member` | Met |
| API-S-005: no path or SQL in answers | `answers_carry_no_filesystem_paths`, `commands_round_trip_as_versioned_json` | Met |

### Secondary SRS mapping

- **SRS-F-001: Independent local library and ownership.** Creating and opening a library needs no account or connection (scenario 1). The new feature is listed under SRS-F-001's tests.
- **SRS-F-049: Product component terminology and item type.** Commands and answers use the internal nouns library, collection, virtual collection and file, and the `filesquid-core` types. The new feature is listed under SRS-F-049's tests.
- **SRS-F-038: Open interfaces and product exit.** Every operation is a versioned `filesquid-core` envelope behind `filesquid-api`, with no filesystem access.
- **API-S-008: Every content or organisation change MUST commit an OCFL version through the `filesquid-ocfl` change model; there MUST NOT be a second write path.** The `LibraryCatalog` contract requires the host to commit each `CatalogCommit` that changes something as one OCFL version through the `filesquid-ocfl` change model; a commit with no change stores only its journal entry and receipt and writes no version. The reference `MemoryCatalog` keeps everything in memory.

## What changed

- `crates/api/src/library/`: `ids` (UUID v4), `policy` (`FileState`, `DeletionPolicy`, `VisibilityFilter`, `PurgeConfirmation`), `command` (`LibraryCommand`, `CommandRequest`, `CommandResult`), `query` (`LibraryQuery`, `QueryRequest`, `QueryAnswer`, `Page`, `Cursor`), `catalog` (the `LibraryCatalog` port, `CatalogChange`, `CatalogCommit`), `plan` (what each command means), `service` (`LibraryService`), `memory` (`MemoryCatalog`), `error` (`LibraryError`).
- Tests: `tests/unit/library.rs`, `tests/unit/library_queries.rs`, `tests/support/library_fixture.rs`, `tests/support/library_steps.rs` and `tests/features/library_domain_commands_and_queries_API-S-019.feature`.
- Docs: module SRS row API-S-019; the CL-0.2 plan row; product SRS test lists for SRS-F-001 and SRS-F-049; root README test command; checklist box.
- No new dependency.

## Tests

TDD: the tests were written first. Before the module existed, `cargo test -p filesquid-api --locked --test unit_library` failed to compile:

```text
error[E0432]: unresolved import `filesquid_api::library`
error[E0433]: cannot find `library` in `filesquid_api`
error: could not compile `filesquid-api` (test "unit_library") due to 4 previous errors
```

After the implementation, and after Cubic review rounds 1 and 2 (below), `cargo test -p filesquid-api --locked` gave for the library targets:

```text
     Running tests/unit/library.rs

running 30 tests
test a_failed_commit_leaves_the_memory_catalog_unchanged ... ok
test a_later_create_library_change_cannot_replace_a_library ... ok
test a_mixed_delete_batch_answers_the_times_of_the_files_it_newly_deletes ... ok
test a_no_op_is_journaled_and_replays_under_its_operation_id ... ok
test a_repeated_operation_id_returns_the_first_outcome ... ok
test a_replayed_delete_answers_its_first_deletion ... ok
test a_stale_expected_revision_is_a_conflict_without_change ... ok
test catalog_failures_are_retryable_and_journaled_for_queries_too ... ok
test adding_a_present_member_is_a_no_op_that_commits_nothing ... ok
test commands_round_trip_as_versioned_json ... ok
test catalog_failures_are_journaled_exactly_once_with_the_journal_layer_installed ... ok
test create_collection_needs_a_folder_of_its_library ... ok
test create_library_needs_the_new_library_revision ... ok
test deleting_a_collection_retains_its_files_as_unfiled ... ok
test errors_map_to_api_and_port_errors ... ok
test create_library_mints_an_id_and_starts_at_revision_one ... ok
test deleting_a_virtual_collection_retains_its_members ... ok
test every_commit_carries_one_redacted_journal_entry ... ok
test deletion_is_recoverable_for_thirty_days_and_restore_keeps_hidden ... ok
test membership_batches_keep_their_order_after_the_last_member ... ok
test names_must_be_non_empty_and_bounded ... ok
test minted_identifiers_are_canonical_uuid_v4 ... ok
test permanent_deletion_needs_deleted_files_and_an_exact_confirmation ... ok
test refused_commands_are_journaled_as_failed ... ok
test references_to_another_library_are_not_found ... ok
test uuid_check_rejects_other_shapes ... ok
test rule_based_virtual_collections_refuse_hand_picked_members ... ok
test renaming_to_the_same_title_commits_nothing ... ok
test virtual_collection_membership_leaves_the_home_collection_alone ... ok
test file_batches_hold_one_to_one_thousand_distinct_files ... ok

test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/unit/library_queries.rs

running 12 tests
test answers_carry_no_filesystem_paths ... ok
test deletion_policy_query_reports_thirty_days ... ok
test cursors_are_opaque_and_malformed_ones_are_refused ... ok
test collections_and_virtual_collections_list_by_library ... ok
test listing_libraries_pages_by_repository ... ok
test open_library_answers_its_record_and_revision ... ok
test listing_a_scope_of_another_library_is_not_found ... ok
test opening_an_unknown_library_is_not_found ... ok
test ordinary_listing_excludes_hidden_and_deleted_files ... ok
test paging_is_stable_with_no_repeats_or_gaps ... ok
test the_last_page_has_no_cursor ... ok
test every_issued_cursor_decodes_and_only_canonical_text_is_accepted ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```

The `features` harness gave:

```text
Feature: Library domain commands and queries
  Scenario: Create and open a library without an account or a connection
   ✔  Given a FileSquid application layer with an empty catalog
   ✔  When the owner creates a library named "Family archive"
   ✔  Then the library should have a UUID version 4 identifier
   ✔  And opening the library should answer the name "Family archive" at revision "1"
  Scenario: Deleting a collection keeps its files
   ✔  Given a library with a collection "Trips" holding 2 files
   ✔  When the owner deletes the collection "Trips"
   ✔  Then the library should still list 2 files
   ✔  And the unfiled listing should show 2 files
  Scenario: Removing a file from a virtual collection keeps its other memberships
   ✔  Given a library with a collection "Trips" holding 1 file
   ✔  And the file is a member of the virtual collections "Best" and "Family"
   ✔  When the owner removes the file from "Best"
   ✔  Then the collection "Trips" should still list the file
   ✔  And the virtual collection "Family" should still list the file
   ✔  And the virtual collection "Best" should list no file
  Scenario: A rule-based virtual collection refuses hand-picked members
   ✔  Given a library with a collection "Trips" holding 1 file
   ✔  And a rule-based virtual collection "Recent"
   ✔  When the owner adds the file to "Recent"
   ✔  Then the command should be refused as "invalid_request"
  Scenario: Hidden and deleted files leave the ordinary listing
   ✔  Given a library with a collection "Trips" holding 3 files
   ✔  When the owner hides the first file and deletes the second
   ✔  Then the ordinary listing should show 1 file
   ✔  And the hidden view should show 1 file
   ✔  And the deleted view should show 1 file to be purged 30 days after deletion
  Scenario: Permanent deletion needs a confirmation of the exact scope
   ✔  Given a library with a collection "Trips" holding 2 files
   ✔  And the owner has deleted every file
   ✔  When the owner permanently deletes every file confirming 1 file
   ✔  Then the command should be refused as "invalid_request"
   ✔  And the deleted view should show 2 files
  Scenario: A command against a stale revision is a conflict and changes nothing
   ✔  Given a library with a collection "Trips" holding 1 file
   ✔  When the owner renames the collection "Trips" with a stale revision
   ✔  Then the command should be refused as "conflict"
   ✔  And nothing should have been committed
  Scenario: Retrying a command with the same operation ID changes nothing
   ✔  Given a library with a collection "Trips" holding 1 file
   ✔  When the owner sends the same create-collection command twice
   ✔  Then both answers should name the same collection
   ✔  And the library should list 2 collections
  Scenario: Paging visits every file once in a stable order
   ✔  Given a library with a collection "Trips" holding 7 files
   ✔  When the owner lists the library 3 files per page
   ✔  Then every file should appear exactly once in identifier order
   ✔  And a query asking for 1001 items per page should be refused
  Scenario: The journal records every change without names or titles
   ✔  Given a library with a collection "Holiday in Lisbon" holding 1 file
   ✔  When the owner renames the collection "Holiday in Lisbon" to "Secret plans"
   ✔  Then the catalog should hold one journal entry per commit
   ✔  And no journal entry should contain "Lisbon" or "Secret plans"
```

`cargo test -p filesquid-api --locked` passed: 178 tests across 15 unit targets, and `164 scenarios (75 passed, 89 skipped)` and `346 steps (257 passed, 89 skipped)` in the harness. The skipped scenarios belong to later tasks. `cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo doc --workspace --no-deps` are clean.

Coverage: no coverage tool is configured for the Rust workspace yet.

Integration and performance tests: this task does no I/O. The catalog is injected, and the only implementation is in memory. SQLite and PostgreSQL integration tests come with the host catalogs (DE-0.8, SV-F1.2).

## Cubic review round 1

Cubic raised 21 findings on PR #33. The fixes, test first:

- `MemoryCatalog::commit` applies a commit to a staged copy and publishes it only when every step succeeds, and refuses a `CreateLibrary` change that is not the first change, is repeated or names another library.
- The `LibraryCatalog::commit` documentation states the order a host follows (stored receipt first, then the create or revision rule) and the create-library rule.
- A command that changes nothing (a present member, or a rename to the same title) is still handed to the catalog with no change. The catalog stores its journal entry and receipt but keeps the revision and writes no OCFL version, so a retry answers the same outcome. The module SRS row API-S-019 now says so.
- `CatalogCommit` and `CommitReceipt` carry the deletion times, so a replayed delete answers its first `deleted_at` and `purge_after`.
- `LibraryService::new` takes the bounded journal sender, so failure journaling cannot be left out. Stdout diagnostics are INFO under the `filesquid::library` target, which the journal layer does not record, so each failure is journaled exactly once.
- `CreateCollection` checks its folder through the new `LibraryCatalog::folder` lookup (`not_found` for a missing or foreign folder).
- New members get order keys after the collection's last member (`LibraryCatalog::last_member_order`), not from the clock.
- `Cursor::encode` is fallible and bounded like `Cursor::decode`, and `decode` accepts only canonical text.
- `LibraryCommand::validate` and `file_ids` name every variant. The purge, virtual-collection delete and batch-size tests assert more. The `DeletionPolicy` and `Deletion` documentation and the plan status line are corrected.

Before the fixes, the new and changed tests did not compile against the old code (`cargo test -p filesquid-api --locked --test unit_library --test unit_library_queries`):

```text
error[E0061]: this function takes 1 argument but 2 arguments were supplied
error[E0407]: method `folder` is not a member of trait `LibraryCatalog`
error[E0407]: method `last_member_order` is not a member of trait `LibraryCatalog`
error[E0560]: struct `CatalogCommit` has no field named `deletion`
error[E0599]: no method named `expect` found for struct `filesquid_api::library::Cursor` in the current scope
error[E0599]: no method named `insert_folder` found for struct `Arc<MemoryCatalog>` in the current scope
error[E0599]: no method named `operation_count` found for struct `Arc<MemoryCatalog>` in the current scope
error: could not compile `filesquid-api` (test "unit_library") due to 8 previous errors
error: could not compile `filesquid-api` (test "unit_library_queries") due to 6 previous errors
```

## Cubic review round 2

Cubic raised 4 findings on `e1fe18d`:

- `MemoryCatalog::commit` no longer clones the whole state per commit. It checks everything that can fail first (the stored receipt, the create-library placement and the revision, in `next_revision`), then applies the changes, the revision, the journal entry and the receipt in place with infallible map writes (`apply_checked`). A commit now costs the size of its change set. TDD-ENFORCE: not applicable, a refactor of tested code; `a_failed_commit_leaves_the_memory_catalog_unchanged` and `a_later_create_library_change_cannot_replace_a_library` still pass.
- A delete answers `deleted_at` and `purge_after` of the files it newly deletes. A file in the batch that was already deleted keeps its earlier times, which the deleted view reports per file, and a batch that deletes no new file answers no times. The rustdoc of `CatalogCommit::deletion`, `CommitReceipt::deletion` and `CommandResult::deletion` and the module SRS row API-S-019 say so. The new test `a_mixed_delete_batch_answers_the_times_of_the_files_it_newly_deletes` pins this behaviour; it passed on first run because the code already behaved this way, and the fix is the documented contract.
- The test fixture's `failure_journal` pages through `next_cursor` (at most 1,000 pages of `MAX_PAGE`) instead of reading only the first page of 100.
- The C19 test is renamed `membership_batches_keep_their_order_after_the_last_member`.

## Mocked Environment Variables

None.
