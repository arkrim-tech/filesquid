# Task CL-0.2 Walkthrough: Library domain commands and queries

Module task [CL-0.2](../plan.md) of global [Task 0.10](../../../../../../resources/implementation-plan.md#task-0.10). Branch `feat/cl-0.2-domain-commands`, from `main` at `23a9662` (PR #27, CL-0.1). Dependencies CL-0.1 and global Task 0.2 were complete.

**Status: done.** `filesquid-api` gains the `library` module: the versioned library commands and queries, their rules, the `LibraryCatalog` port that hosts inject, and the in-memory `MemoryCatalog` reference store. The module does no storage I/O, so the API-S-006 guards stay green with no pins. No HTTP route or contract path is added: those arrive with API-F1.3 and API-F2.2, and a contract change now would also change the UI's generated types.

## API-S-019: Library domain commands and queries

The application layer MUST define versioned commands to create a library; to create, rename and delete collections and virtual collections; to add members to a hand-picked virtual collection and remove them; and to hide, unhide, delete, restore and permanently delete files. It MUST also define versioned queries for opening a library and for paged listing. It MUST store through a catalog the host injects, with no filesystem path or SQL ([module SRS](../srs.md)).

Parents: [F-001 Independent local library and ownership](../../../../../../resources/prd.md#f-001-independent-local-library-and-ownership), [F-049 Product component terminology and item type](../../../../../../resources/prd.md#f-049-product-component-terminology-and-item-type), [F-038 Open interfaces and product exit](../../../../../../resources/prd.md#f-038-open-interfaces-and-product-exit), [F-004 Collections, folders and navigation](../../../../../../resources/prd.md#f-004-collections-folders-and-navigation), [F-008 Deletion, hidden files and duplicate review](../../../../../../resources/prd.md#f-008-deletion-hidden-files-and-duplicate-review).

| Constraint | Gate | State |
|---|---|---|
| C1 New IDs are UUID v4 from the OS CSPRNG, lowercase 8-4-4-4-12 | `minted_identifiers_are_canonical_uuid_v4`, `uuid_check_rejects_other_shapes`, scenario 1 | Met |
| C2 A reference outside the named library is `not_found` | `references_to_another_library_are_not_found`, `listing_a_scope_of_another_library_is_not_found` | Met |
| C3 Create-library sends revision `0`; a stale revision is `conflict` with no change | `create_library_needs_the_new_library_revision`, `a_stale_expected_revision_is_a_conflict_without_change`, stale-revision scenario | Met |
| C4 A repeated operation ID answers the first outcome, with no second change | `a_repeated_operation_id_returns_the_first_outcome`, retry scenario | Met |
| C5 A file batch names 1 to 1,000 distinct files | `file_batches_hold_one_to_one_thousand_distinct_files` | Met |
| C6 Names are non-blank, at most 1,024 bytes | `names_must_be_non_empty_and_bounded` | Met |
| C7 Deleting a collection or virtual collection keeps its files | `deleting_a_collection_retains_its_files_as_unfiled`, `deleting_a_virtual_collection_retains_its_members`, scenario 2 | Met |
| C8 Removing a virtual-collection member keeps its collection and other memberships | `virtual_collection_membership_leaves_the_home_collection_alone`, scenario 3 | Met |
| C9 A rule-based virtual collection refuses hand-picked members | `rule_based_virtual_collections_refuse_hand_picked_members`, scenario 4 | Met |
| C10 Deleted files stay recoverable for 30 days, with `deleted_at` and `purge_after` given; restoring a file keeps it hidden if it was hidden | `deletion_is_recoverable_for_thirty_days_and_restore_keeps_hidden`, scenario 5, `deletion_policy_query_reports_thirty_days` | Met |
| C11 Permanent deletion applies only to deleted files, with a confirmation that names the library and the exact file count | `permanent_deletion_needs_deleted_files_and_an_exact_confirmation`, scenario 6 | Met |
| C12 The ordinary view leaves out hidden and deleted files; the hidden and deleted views show them | `ordinary_listing_excludes_hidden_and_deleted_files`, scenario 5 | Met |
| C13 Pages hold 1 to 1,000 items in a stable order; the cursor is opaque; a malformed cursor is refused | `paging_is_stable_with_no_repeats_or_gaps`, `the_last_page_has_no_cursor`, `cursors_are_opaque_and_malformed_ones_are_refused`, `listing_libraries_pages_by_repository`, paging scenario | Met |
| C14 Each commit carries one journal entry, written in the same transaction, holding no name or title | `every_commit_carries_one_redacted_journal_entry`, journal scenario | Met |
| C15 Refused commands, and catalog failures behind queries, are journaled as `failed` through the bounded queue | `refused_commands_are_journaled_as_failed`, `catalog_failures_are_retryable_and_journaled_for_queries_too` | Met |
| C16 No storage I/O or forbidden crate | `unit_isolation` (13 passed), API-S-006 feature | Met |
| C17 Errors map to `ApiError` and `PortError` | `errors_map_to_api_and_port_errors` | Met |
| C18 A command that changes nothing commits nothing | `adding_a_present_member_is_a_no_op_that_commits_nothing` | Met |
| API-S-005: no path or SQL in answers | `answers_carry_no_filesystem_paths`, `commands_round_trip_as_versioned_json` | Met |

### Secondary SRS mapping

- **SRS-F-001: Independent local library and ownership.** Creating and opening a library needs no account or connection (scenario 1). The new feature is listed under SRS-F-001's tests.
- **SRS-F-049: Product component terminology and item type.** Commands and answers use the internal nouns library, collection, virtual collection and file, and the `filesquid-core` types. The new feature is listed under SRS-F-049's tests.
- **SRS-F-038: Open interfaces and product exit.** Every operation is a versioned `filesquid-core` envelope behind `filesquid-api`, with no filesystem access.
- **API-S-008.** The `LibraryCatalog` contract requires the host to commit each `CatalogCommit` as one OCFL version through the `filesquid-ocfl` change model. The reference `MemoryCatalog` keeps everything in memory.

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

After the implementation, `cargo test -p filesquid-api --locked --test unit_library --test unit_library_queries` gave:

```text
     Running tests/unit/library.rs
test a_repeated_operation_id_returns_the_first_outcome ... ok
test a_stale_expected_revision_is_a_conflict_without_change ... ok
test create_library_mints_an_id_and_starts_at_revision_one ... ok
test create_library_needs_the_new_library_revision ... ok
test deleting_a_collection_retains_its_files_as_unfiled ... ok
test deleting_a_virtual_collection_retains_its_members ... ok
test deletion_is_recoverable_for_thirty_days_and_restore_keeps_hidden ... ok
test errors_map_to_api_and_port_errors ... ok
test every_commit_carries_one_redacted_journal_entry ... ok
test file_batches_hold_one_to_one_thousand_distinct_files ... ok
test minted_identifiers_are_canonical_uuid_v4 ... ok
test names_must_be_non_empty_and_bounded ... ok
test permanent_deletion_needs_deleted_files_and_an_exact_confirmation ... ok
test references_to_another_library_are_not_found ... ok
test rule_based_virtual_collections_refuse_hand_picked_members ... ok
test refused_commands_are_journaled_as_failed ... ok
test virtual_collection_membership_leaves_the_home_collection_alone ... ok
test adding_a_present_member_is_a_no_op_that_commits_nothing ... ok
test uuid_check_rejects_other_shapes ... ok
test commands_round_trip_as_versioned_json ... ok
test catalog_failures_are_retryable_and_journaled_for_queries_too ... ok
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/unit/library_queries.rs
test cursors_are_opaque_and_malformed_ones_are_refused ... ok
test deletion_policy_query_reports_thirty_days ... ok
test listing_a_scope_of_another_library_is_not_found ... ok
test open_library_answers_its_record_and_revision ... ok
test opening_an_unknown_library_is_not_found ... ok
test answers_carry_no_filesystem_paths ... ok
test listing_libraries_pages_by_repository ... ok
test ordinary_listing_excludes_hidden_and_deleted_files ... ok
test collections_and_virtual_collections_list_by_library ... ok
test paging_is_stable_with_no_repeats_or_gaps ... ok
test the_last_page_has_no_cursor ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
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

`cargo test -p filesquid-api --locked` passed: 168 tests across 15 unit targets, and `164 scenarios (75 passed, 89 skipped)` and `346 steps (257 passed, 89 skipped)` in the harness. The skipped scenarios belong to later tasks. `cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo doc --workspace --no-deps` are clean.

Coverage: no coverage tool is configured for the Rust workspace yet.

Integration and performance tests: this task does no I/O. The catalog is injected, and the only implementation is in memory. SQLite and PostgreSQL integration tests come with the host catalogs (DE-0.8, SV-F1.2).

## Mocked Environment Variables

None.
