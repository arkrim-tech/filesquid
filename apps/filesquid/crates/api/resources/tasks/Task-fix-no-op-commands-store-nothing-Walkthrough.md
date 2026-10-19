# Task fix-no-op-commands-store-nothing Walkthrough: a no-op command stores nothing

Follow-up fix to [CL-0.2](./Task-CL-0.2-Walkthrough.md) and [DE-0.2](../../../desktop/resources/tasks/Task-DE-0.2-Walkthrough.md). Branch `fix/no-op-commands-store-nothing`, from `main` at `8683e04`.

**Owner decision (2026-10-19), given directly by the owner as an explicit exception to the no-exemptions journal rule:** a command or save that changes nothing gets no receipt, no new OCFL version and no journal entry.

## API-S-019: Library domain commands and queries

Amended requirement ([module SRS](../srs.md)): a command that changes nothing (for example a rename to the same title or adding a present member) MUST answer `committed` at the current revision with no new revision, and MUST store nothing: no operation-ID receipt, no journal entry and no OCFL version. Because it has no receipt, a retried command that changed nothing MUST be planned again against the current state (there is no idempotent replay for it): it answers `committed` again when it still changes nothing, and otherwise meets the revision and change rules like a new command. A repeated operation ID of a command that changed something still answers the first outcome. Every committed command that changes something still hands the catalog exactly one journal entry and its receipt, in the same transaction as the change, and is one OCFL version (API-S-008).

Parents (unchanged): [F-001 Independent local library and ownership](../../../../../../resources/prd.md#f-001-independent-local-library-and-ownership), [F-049 Product component terminology and item type](../../../../../../resources/prd.md#f-049-product-component-terminology-and-item-type), [F-038 Open interfaces and product exit](../../../../../../resources/prd.md#f-038-open-interfaces-and-product-exit), [F-004 Collections, folders and navigation](../../../../../../resources/prd.md#f-004-collections-folders-and-navigation), [F-008 Deletion, hidden files and duplicate review](../../../../../../resources/prd.md#f-008-deletion-hidden-files-and-duplicate-review). The PRD states no no-op rule, so it needs no change; the product [SRS](../../../../../../resources/srs.md) only lists the API-S-019 feature under SRS-F-001 and SRS-F-049 and does not repeat the rule.

| Constraint | Gate | State |
|---|---|---|
| N1 A no-op answers `committed` at the current revision, with no new revision | `a_no_op_stores_no_receipt_journal_entry_or_version`, `adding_a_present_member_is_a_no_op_that_commits_nothing`, `renaming_to_the_same_title_commits_nothing`, scenario "A command that changes nothing stores nothing" | Met |
| N2 A no-op stores no operation-ID receipt | `a_no_op_stores_no_receipt_journal_entry_or_version`, `a_command_that_changes_nothing_stores_no_receipt_or_journal_entry` (SQLite), both no-op scenarios | Met |
| N3 A no-op stores no journal entry (neither a `succeeded` entry with the catalog nor a `failed` entry through the queue) | `a_no_op_stores_no_receipt_journal_entry_or_version`, `a_command_that_changes_nothing_stores_no_receipt_or_journal_entry`, both no-op scenarios | Met |
| N4 A no-op writes no OCFL version (no commit that changed something) | `a_no_op_stores_no_receipt_journal_entry_or_version` (`commit_count` unchanged), scenario "nothing should have been committed" | Met |
| N5 A retried no-op is planned again against current state: same answer while it still changes nothing; `conflict` once the library has moved on | `a_retried_no_op_is_planned_again_against_current_state`, scenario "A retried command that changed nothing is planned again" | Met |
| N6 A commit with no change that reaches a catalog anyway stores nothing (memory and SQLite), still checks the revision, and the SQLite catalog removes its pending intent | `an_empty_raw_commit_stores_nothing_in_the_memory_catalog`, `a_commit_without_change_keeps_the_revision_and_stores_nothing` | Met |
| N7 Every other command keeps its receipt and journal behaviour | `a_repeated_operation_id_returns_the_first_outcome`, `a_replayed_delete_answers_its_first_deletion`, `every_commit_carries_one_redacted_journal_entry`, `refused_commands_are_journaled_as_failed`, `a_receipt_in_one_library_answers_a_commit_naming_another`, `an_intent_recorded_after_its_commit_is_not_stored`, retry and journal scenarios | Met |

## What changed

- [`service.rs`](../../src/library/service.rs): `LibraryService` answers a planned command with no change through the pure `unchanged` helper (`committed` at the revision the planner just checked) and never hands it to the catalog.
- [`catalog.rs`](../../src/library/catalog.rs): the `LibraryCatalog::commit` contract says a commit with no change stores nothing and answers the current revision (a host may remove its pending intent).
- [`memory.rs`](../../src/library/memory.rs): `MemoryCatalog` stores nothing for an empty commit; `journal_entries` and `operation_count` docs updated.
- [`command.rs`](../../src/library/command.rs): `CommandResult::outcome` doc.
- [`native/src/catalog/write.rs`](../../../native/src/catalog/write.rs) and [`mod.rs`](../../../native/src/catalog/mod.rs): the SQLite catalog finishes an empty commit by removing its pending intent only (`clear_unchanged_intent`): no receipt, no outbox entry, no revision bump.
- Tests: `a_no_op_is_journaled_and_replays_under_its_operation_id` is replaced by `a_no_op_stores_no_receipt_journal_entry_or_version`, `a_retried_no_op_is_planned_again_against_current_state` and `an_empty_raw_commit_stores_nothing_in_the_memory_catalog` ([`tests/unit/library.rs`](../../tests/unit/library.rs)); two API-S-019 scenarios and their steps; in `filesquid-native`, `a_commit_without_change_keeps_the_revision_but_stores_its_receipt` is replaced by `a_commit_without_change_keeps_the_revision_and_stores_nothing`, `a_command_that_changes_nothing_stores_no_receipt_or_journal_entry` is new, three raw-commit tests now commit a real change, and the DE-S-009 catalog feature gains a no-op scenario.
- Docs: module SRS row API-S-019; desktop SRS row DE-S-009; [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) "What is journaled"; `.agents/rules/logging-enforce.md` §2; the CL-0.2 and DE-0.2 walkthroughs, where they stated the old behaviour.
- No new dependency, environment variable, service or command, so the root README is unchanged.

Open PR #77 (DE-0.3, `feat/de-0.3-intent-ocfl-commit-receipt`) words DE-S-010 for the old behaviour ("a no-op's receipt and journal entry stay in the catalog"). It had not merged when this branch was finished, so DE-S-010 must follow this rule after #77 merges.

## Tests

TDD: the tests were written first. Against the old code, `cargo test -p filesquid-api --test unit_library --locked -- no_op empty_raw_commit` failed:

```text
test adding_a_present_member_is_a_no_op_that_commits_nothing ... ok
test an_empty_raw_commit_stores_nothing_in_the_memory_catalog ... FAILED
test a_no_op_stores_no_receipt_journal_entry_or_version ... FAILED
test a_retried_no_op_is_planned_again_against_current_state ... FAILED
thread 'an_empty_raw_commit_stores_nothing_in_the_memory_catalog' panicked at crates/api/tests/unit/library.rs:821:5:
  left: Some(CommitReceipt { library_id: LibraryId("78ecb39e-c128-4c8c-8eff-36b3df0ba371"), revision: "1", created_id: None, deletion: None })
 right: None
thread 'a_no_op_stores_no_receipt_journal_entry_or_version' panicked at crates/api/tests/unit/library.rs:783:5:
  left: Some(CommitReceipt { library_id: LibraryId("e066792b-f8a6-46c6-b616-30546f3192c4"), revision: "3", created_id: None, deletion: None })
 right: None
thread 'a_retried_no_op_is_planned_again_against_current_state' panicked at crates/api/tests/unit/library.rs:804:10:
test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.10s
```

`cargo test -p filesquid-native --features desktop --test unit_catalog --locked -- without_change changes_nothing` failed:

```text
test a_commit_without_change_keeps_the_revision_and_stores_nothing ... FAILED
test a_command_that_changes_nothing_stores_no_receipt_or_journal_entry ... FAILED
thread 'a_commit_without_change_keeps_the_revision_and_stores_nothing' panicked at crates/native/tests/unit/catalog.rs:226:5:
  left: Some(CommitReceipt { library_id: LibraryId("1ff1ac52-ab52-44ec-9a7d-e63cfd8e0f47"), revision: "1", created_id: None, deletion: None })
 right: None
thread 'a_command_that_changes_nothing_stores_no_receipt_or_journal_entry' panicked at crates/native/tests/unit/catalog.rs:258:5:
  left: Some(CommitReceipt { library_id: LibraryId("3374b96a-0003-4b98-9940-c4a7495ae212"), revision: "2", created_id: None, deletion: None })
 right: None
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.08s
```

The API-S-019 scenarios failed in the `features` harness (`cargo test -p filesquid-api --test features --locked`):

```text
   ✘  And the catalog should hold no receipt for operation "op-same-title"
      Step panicked. Captured output: assertion `left == right` failed
        left: Some(CommitReceipt { library_id: LibraryId("75d1a5f9-6d9e-45a3-8221-ba6a49cad536"), revision: "2", created_id: None, deletion: None })
       right: None
   ✘  Then the command should be refused as "conflict"
      Step panicked. Captured output: refused: CommandResult { outcome: CommandOutcome { operation_id: OperationId("op-same-title"), kind: Committed, error: None }, library_id: Some(LibraryId("f13ae0b2-17cf-435e-82d3-6f77ca786e56")), revision: Some("2"), created_id: None, deletion: None }
213 scenarios (105 passed, 106 skipped, 2 failed)
497 steps (389 passed, 106 skipped, 2 failed)
```

The DE-S-009 catalog scenario was added after the implementation; its unit equivalent `a_command_that_changes_nothing_stores_no_receipt_or_journal_entry` failed first, above.

After the implementation, `cargo test -p filesquid-api --test unit_library --locked`:

```text
test a_failed_commit_leaves_the_memory_catalog_unchanged ... ok
test a_later_create_library_change_cannot_replace_a_library ... ok
test a_mixed_delete_batch_answers_the_times_of_the_files_it_newly_deletes ... ok
test a_no_op_stores_no_receipt_journal_entry_or_version ... ok
test a_repeated_operation_id_returns_the_first_outcome ... ok
test adding_a_present_member_is_a_no_op_that_commits_nothing ... ok
test a_stale_expected_revision_is_a_conflict_without_change ... ok
test a_replayed_delete_answers_its_first_deletion ... ok
test a_retried_no_op_is_planned_again_against_current_state ... ok
test an_empty_raw_commit_stores_nothing_in_the_memory_catalog ... ok
test catalog_failures_are_retryable_and_journaled_for_queries_too ... ok
test commands_round_trip_as_versioned_json ... ok
test create_collection_needs_a_folder_of_its_library ... ok
test create_library_needs_the_new_library_revision ... ok
test create_library_mints_an_id_and_starts_at_revision_one ... ok
test catalog_failures_are_journaled_exactly_once_with_the_journal_layer_installed ... ok
test deleting_a_collection_retains_its_files_as_unfiled ... ok
test errors_map_to_api_and_port_errors ... ok
test deleting_a_virtual_collection_retains_its_members ... ok
test deletion_is_recoverable_for_thirty_days_and_restore_keeps_hidden ... ok
test every_commit_carries_one_redacted_journal_entry ... ok
test membership_batches_keep_their_order_after_the_last_member ... ok
test names_must_be_non_empty_and_bounded ... ok
test minted_identifiers_are_canonical_uuid_v4 ... ok
test permanent_deletion_needs_deleted_files_and_an_exact_confirmation ... ok
test renaming_to_the_same_title_commits_nothing ... ok
test refused_commands_are_journaled_as_failed ... ok
test uuid_check_rejects_other_shapes ... ok
test rule_based_virtual_collections_refuse_hand_picked_members ... ok
test references_to_another_library_are_not_found ... ok
test virtual_collection_membership_leaves_the_home_collection_alone ... ok
test file_batches_hold_one_to_one_thousand_distinct_files ... ok
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

The `features` harness:

```text
  Scenario: A command that changes nothing stores nothing
   ✔  Given a library with a collection "Trips" holding 1 file
   ✔  When the owner renames the collection "Trips" to "Trips" as operation "op-same-title"
   ✔  Then the command should answer committed at the current revision
   ✔  And the catalog should hold no receipt for operation "op-same-title"
   ✔  And nothing should have been committed
   ✔  And no journal entry should have been stored
  Scenario: A retried command that changed nothing is planned again
   ✔  Given a library with a collection "Trips" holding 1 file
   ✔  And the owner has renamed the collection "Trips" to "Trips" as operation "op-same-title"
   ✔  When the owner renames the collection "Trips" to "Holidays"
   ✔  And the owner retries operation "op-same-title"
   ✔  Then the command should be refused as "conflict"
[Summary]
13 features
213 scenarios (107 passed, 106 skipped)
499 steps (393 passed, 106 skipped)
```

The skipped scenarios belong to later tasks, as before this change.

`cargo test -p filesquid-native --features desktop --test catalog_features --test unit_catalog --locked`:

```text
  Scenario: A command that changes nothing stores nothing
   ✔  Given an empty catalog directory
   ✔  And the owner created the library "Family archive" with the collection "Summer"
   ✔  When the owner renames the collection "Summer" to "Summer"
   ✔  Then the catalog should hold no receipt for the rename
   ✔  And the host journal should not record "library.collection.rename"
   ✔  And the library "Family archive" should be at revision 2
7 scenarios (7 passed)
34 steps (34 passed)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.40s
```

Full gate on the head commit: `FULL=1 bash scripts/pre-commit.sh` passed with the summary line in the [Gate](#gate) section below.

Coverage: no coverage tool is configured for the Rust workspace yet. Performance: no hot path changed (a no-op now does less work).

## Gate

`FULL=1 bash scripts/pre-commit.sh` from the repository root (branch change scope `rust:filesquid-api,filesquid-desktop,filesquid-native`; the full run covers it):

```text
✅ All Pre-Commit Hooks Passed! Test scope: full
```

## Mocked Environment Variables

None.
