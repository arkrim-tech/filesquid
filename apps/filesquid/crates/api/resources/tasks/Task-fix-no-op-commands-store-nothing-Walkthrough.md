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
| N5a A no-op whose revision moved on between planning and commit is `conflict`, not `committed` | `a_no_op_planned_on_a_revision_that_moved_on_is_a_conflict` | Met |
| N6 A commit with no change stores nothing (memory and SQLite), still checks the revision, and the SQLite catalog removes its pending intent | `an_empty_raw_commit_stores_nothing_in_the_memory_catalog`, `a_commit_without_change_keeps_the_revision_and_stores_nothing` | Met |
| N7 Every other command keeps its receipt and journal behaviour | `a_repeated_operation_id_returns_the_first_outcome`, `a_replayed_delete_answers_its_first_deletion`, `every_commit_carries_one_redacted_journal_entry`, `refused_commands_are_journaled_as_failed`, `a_receipt_in_one_library_answers_a_commit_naming_another`, `an_intent_recorded_after_its_commit_is_not_stored`, retry and journal scenarios | Met |

## What changed

- [`service.rs`](../../src/library/service.rs): `LibraryService` still hands a planned command with no change to the catalog as an empty commit, so the catalog rechecks the expected revision atomically (Cubic round 1); the catalog stores nothing for it.
- [`catalog.rs`](../../src/library/catalog.rs): the `LibraryCatalog::commit` contract says a commit with no change stores nothing and answers the current revision (a host may remove its pending intent).
- [`memory.rs`](../../src/library/memory.rs): `MemoryCatalog` stores nothing for an empty commit; `journal_entries` and `operation_count` docs updated.
- [`command.rs`](../../src/library/command.rs): `CommandResult::outcome` doc.
- [`native/src/catalog/write.rs`](../../../native/src/catalog/write.rs) and [`mod.rs`](../../../native/src/catalog/mod.rs): the SQLite catalog finishes an empty commit by removing its pending intent only (`clear_unchanged_intent`): no receipt, no outbox entry, no revision bump.
- Tests: `a_no_op_is_journaled_and_replays_under_its_operation_id` is replaced by `a_no_op_stores_no_receipt_journal_entry_or_version`, `a_retried_no_op_is_planned_again_against_current_state` and `an_empty_raw_commit_stores_nothing_in_the_memory_catalog` ([`tests/unit/library.rs`](../../tests/unit/library.rs)); two API-S-019 scenarios and their steps; in `filesquid-native`, `a_commit_without_change_keeps_the_revision_but_stores_its_receipt` is replaced by `a_commit_without_change_keeps_the_revision_and_stores_nothing`, `a_command_that_changes_nothing_stores_no_receipt_or_journal_entry` is new, three raw-commit tests now commit a real change, and the DE-S-009 catalog feature gains a no-op scenario.
- Docs: module SRS row API-S-019; desktop SRS row DE-S-009; [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) "What is journaled"; `.agents/rules/logging-enforce.md` §2; the CL-0.2 and DE-0.2 walkthroughs, where they stated the old behaviour.
- No new dependency, environment variable, service or command, so the root README is unchanged.

PR #77 (DE-0.3) merged into `main` during review with DE-S-010 worded for the old behaviour ("a no-op's receipt and journal entry stay in the catalog"). After merging `origin/main`, the SQLite catalog's new DE-0.3 commit sequence (`commit_in_transaction`) finishes an empty commit with `finish_unchanged`, which only clears its intent and publishes no snapshot version; DE-S-010, the ADR 010 amendment, the DE-0.3 walkthrough's owner-decision notes and the `reconcile.rs` module doc now state the new rule. The `library` module doc in `filesquid-api` was corrected in the same commit.

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

## Cubic review round 1

Cubic raised 6 findings on `06222d6` (P1 1, P2 2, P3 3):

- P1: the service answered a no-op without the catalog, so a revision that moved on after planning (a concurrent rename to the same title) answered `committed` instead of `conflict`. The service now hands the empty plan to `LibraryCatalog::commit`, whose empty-commit path rechecks the revision atomically and stores nothing. Test first, `a_no_op_planned_on_a_revision_that_moved_on_is_a_conflict` (a catalog wrapper whose library reads lag one revision behind) failed against `06222d6`:

  ```text
  test a_no_op_planned_on_a_revision_that_moved_on_is_a_conflict ... FAILED
  the revision moved on before the commit: CommandResult { outcome: CommandOutcome { operation_id: OperationId("op-4"), kind: Committed, error: None }, library_id: Some(LibraryId("67076a06-9ce3-4f57-bbee-9d802f3d763b")), revision: Some("2"), created_id: None, deletion: None }
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 32 filtered out; finished in 0.09s
  ```

  After the fix, `cargo test -p filesquid-api --test unit_library --test features --locked` gave `test result: ok. 33 passed` and `213 scenarios (107 passed, 106 skipped)`, and the native catalog targets `7 scenarios (7 passed)` and `test result: ok. 30 passed`. API-S-019 now says a no-op meets the revision rule at commit time.
- P2 (twice): the exception in `logging-enforce.md` and ADR 033 is scoped to a command that succeeds without a change; refused and failed commands are still journaled.
- P3: the `CatalogCommit::changes` doc is unambiguous; the DE-0.2 walkthrough's SRS-F-061 row names the exception. The DE-0.2 raw test output is the evidence of that task's run and is left as recorded; the note above it names the replacement test, whose output is in this walkthrough.

## Cubic review round 2

Cubic raised 1 finding on `7b01ca0` (P2): the ADR 010 amendment now scopes the no-op exception to a command that succeeds without a change, and says refused or failed commands are still journaled; DE-S-010 says the same. TDD-ENFORCE: not applicable, documentation only.

## Merge with the PROV-O history (#84, #86)

`origin/main` gained ADR 040 (PROV-O history; its Decision 1 excludes a command that changes nothing: no version and no record), ADR 033 Decision 7, F-067 and the PROV-O implementation (`filesquid-prov`, journal `operation_id`/`object_ref`, `ocfl.commit` entries, provenance written in the same commit, `SqliteCatalog::with_provenance`). The merge commit resolves two conflicts and nothing else:

- `crates/native/src/catalog/write.rs`, `commit_in_transaction`: a commit with no change still returns early through `finish_unchanged` (only its intent is removed); every other commit publishes through the merged `publish`, which now answers the journal entries to store (the commit's own entry and `ocfl.commit`, with the PROV-O record in the version when provenance is configured and the host journal allocates the entries' IDs; when it cannot, the version is still published and a `prov.record` warning is journaled), and `finish` stores those entries with the receipt.
- `crates/desktop/resources/srs.md`: DE-S-008 takes main's wording ("import record"), DE-S-009 and DE-S-010 keep this branch's no-op sentences, and main's new DE-S-011 to DE-S-013 are kept.

Both ADR 033 amendments (the no-op exception in Decision 2 and main's Decision 7) and the `logging-enforce.md` exception sentence merged without conflict and stand together.

Regression test added on top of the merge, `a_recorded_no_op_commits_no_version_and_journals_nothing` in `crates/native/tests/unit/canonical_commit.rs`: in a catalog built with `with_provenance`, a no-op rename commits no version (the head stays `v2`), stores no receipt and adds no journal entry (no `ocfl.commit`); the next real rename commits `v3` with `ocfl.commit` and `library.collection.rename` linked to `@v3`. It failed against main's `commit_in_transaction`, which stored the no-op's entry and receipt:

```text
test a_recorded_no_op_commits_no_version_and_journals_nothing ... FAILED
thread 'a_recorded_no_op_commits_no_version_and_journals_nothing' (15417) panicked at crates/native/tests/unit/canonical_commit.rs:494:5:
assertion `left == right` failed
  left: Some(CommitReceipt { library_id: LibraryId("4b950672-3a16-4621-8c65-ef31e7ac85df"), revision: "2", created_id: None, deletion: None })
 right: None
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.15s
```

With the merge resolution, `cargo test -p filesquid-native --features desktop --test unit_canonical_commit --test unit_catalog --test catalog_features --locked`:

```text
     Running tests/catalog_features.rs
11 scenarios (11 passed)
61 steps (61 passed)
     Running tests/unit/canonical_commit.rs
test a_recorded_no_op_commits_no_version_and_journals_nothing ... ok
test an_unrecorded_catalog_still_links_its_commits ... ok
test a_crash_after_the_snapshot_replays_the_commits_journal_entries_once ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
     Running tests/unit/catalog.rs
test a_commit_without_change_keeps_the_revision_and_stores_nothing ... ok
test a_command_that_changes_nothing_stores_no_receipt_or_journal_entry ... ok
test every_commit_journal_entry_reaches_the_host_journal_redacted ... ok
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.43s
```

## Cubic review round 3

Cubic raised 1 finding on `602460a` (P3): the merge summary above now says the PROV-O record rides the version only when the host journal allocates the entries' IDs, as `a_commit_without_journal_ids_publishes_and_warns_without_identifying_data` shows. TDD-ENFORCE: not applicable, documentation only.

## DE-S-010: Intent, OCFL snapshot commit and receipt reconcile on the desktop (index refreshes)

**Owner decision (2026-10-19):** a catalog index refresh that changes nothing is treated like a no-op command: it writes no journal entry, no receipt and no version. Before this change `SqliteCatalog::index_files` and `index_folders` already published no version when the entities were unchanged, but still pushed the refresh's own journal entry (unlinked to any version) into the outbox.

| Constraint | Gate | State |
|---|---|---|
| I1 An index refresh that changes nothing (the same files or folders again, or none) publishes no OCFL version | `an_index_refresh_that_changes_nothing_journals_nothing` (head stays `v4`), `a_recorded_index_refresh_that_changes_nothing_journals_nothing` (head stays `v3`) | Met |
| I2 It stores no journal entry and leaves no outbox entry, with or without PROV-O history | both tests above (no new entry, `outbox_len` 0), scenario "An index refresh that changes nothing journals nothing" | Met |
| I3 A refresh that changes something still publishes a version and journals `ocfl.commit` and its own entry linked to that version, as before | both tests above (`@v5`/`@v6`, and `@v4` in the recording catalog), the same scenario's second half | Met |
| I4 A failed refresh is not treated as a no-op: it returns its error and is journaled once as `failed` with its error code, targeting only the library (redacted). A failure before the version is published rolls the catalog transaction back; one after it leaves the catalog behind its head, which reconcile brings forward at the next open (DE-S-010) | `a_failed_index_refresh_is_journaled_as_failed` (Cubic round 4), `a_refresh_of_an_unknown_library_is_journaled_as_failed` (Cubic rounds 5 and 6: no outbox, so through the host's bounded journal queue) | Met |

What changed:

- [`native/src/catalog/mod.rs`](../../../native/src/catalog/mod.rs): `write_journaled` stores no journal entry when `CanonicalRoot::commit` answers `None` (the snapshot is unchanged); it still commits the transaction, so a catalog row that differed from an already-equal snapshot is brought into line. Module and `index_files` docs state the rule.
- Cubic round 4: `write_journaled` now wraps `write_indexed` and, when the refresh fails, pushes a `failed` entry (operation, actor, time, library target and error code only) into the outbox in a transaction of its own, then forwards it; if even that push fails it logs a stdout warning and still returns the refresh's error.
- [`native/src/catalog/history.rs`](../../../native/src/catalog/history.rs): `CommitPlan::unpublished` is now only the fallback of a published commit whose version cannot be referenced; its doc says so.
- Tests: two unit tests in [`tests/unit/canonical_commit.rs`](../../../native/tests/unit/canonical_commit.rs) and the DE-S-009 catalog scenario "An index refresh that changes nothing journals nothing" with its steps.
- Docs: `.agents/rules/logging-enforce.md` §2 exception, [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) Decision 2's exception and the [ADR 010](../../../../../../resources/architecture/010-ocfl-1-1-dual-profile.md) amendment now read: a command or catalog index refresh that succeeds without a change produces no receipt, no OCFL version and no journal entry; a refused or failed one is still journaled as `failed`. Desktop SRS rows DE-S-009 and DE-S-010 say the same for `index_files` and `index_folders`.
- In a catalog that records PROV-O history the journal IDs are allocated before the engine knows whether the refresh changes anything, so a no-op refresh leaves those reserved IDs unused. `allocate_ids` promises unique increasing IDs, not a gapless sequence, so nothing reads the gap.

TDD: the tests were written first. Against the previous code, `cargo test -p filesquid-native --features desktop --test unit_canonical_commit --locked index_refresh` failed:

```text
running 2 tests
test an_index_refresh_that_changes_nothing_journals_nothing ... FAILED
test a_recorded_index_refresh_that_changes_nothing_journals_nothing ... FAILED
thread 'an_index_refresh_that_changes_nothing_journals_nothing' (8731) panicked at crates/native/tests/unit/canonical_commit.rs:578:5:
assertion failed: journaled_since(fx.journal.as_ref(), before).is_empty()
thread 'a_recorded_index_refresh_that_changes_nothing_journals_nothing' (8730) panicked at crates/native/tests/unit/canonical_commit.rs:625:5:
assertion failed: journaled_since(journal.as_ref(), before).is_empty()
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.31s
```

and the new scenario failed in `cargo test -p filesquid-native --features desktop --test catalog_features --locked` (the second `library.file.index` entry has `object_ref: None`, from the unchanged refresh):

```text
  Scenario: An index refresh that changes nothing journals nothing
   ✔  Given an empty catalog directory
   ✔  And the owner created the library "Family archive" with the collection "Summer"
   ✔  And the file "f-1" was indexed in the collection "Summer" at listing order 3
   ✔  When the file "f-1" is indexed in the collection "Summer" at listing order 3 again
   ✘  Then the host journal should record "library.file.index" 1 time
      Step panicked. Captured output: assertion `left == right` failed: [... JournalEntry { id: Some(7), ..., actor: "system", operation: "library.file.index", outcome: Succeeded, ..., object_ref: None }]
        left: 2
       right: 1
12 scenarios (11 passed, 1 failed)
66 steps (65 passed, 1 failed)
```

After the change, `cargo test -p filesquid-native --features desktop --test unit_canonical_commit --test unit_catalog --test catalog_features --locked`:

```text
     Running tests/catalog_features.rs
  Scenario: An index refresh that changes nothing journals nothing
   ✔  And the file "f-1" was indexed in the collection "Summer" at listing order 3
   ✔  When the file "f-1" is indexed in the collection "Summer" at listing order 3 again
   ✔  Then the host journal should record "library.file.index" 1 time
   ✔  When the file "f-1" is indexed in the collection "Summer" at listing order 4 again
   ✔  Then the host journal should record "library.file.index" 2 times
12 scenarios (12 passed)
68 steps (68 passed)
     Running tests/unit/canonical_commit.rs
test a_recorded_index_refresh_that_changes_nothing_journals_nothing ... ok
test an_index_refresh_that_changes_nothing_journals_nothing ... ok
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
     Running tests/unit/catalog.rs
test folders_are_indexed_and_read_back ... ok
test removing_a_catalog_file_loses_only_the_rebuildable_index ... ok
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.72s
```

## Cubic review round 4

Cubic raised 4 findings on `1ba3484` (P2 4):

- The index scenario only counted `library.file.index` entries. It now asserts that the host journal holds no entry at all after the first index, and that the snapshot stays at `v3` and then moves to `v4`.
- The index steps ignored the collection named in the step. They now check that the named collection is the library's only collection.
- The I4 row claimed a failed refresh is journaled as `failed`, but `write_journaled` journaled nothing on an error path. A failed refresh is now journaled as `failed` (see I4 and What changed). Test first: `a_failed_index_refresh_is_journaled_as_failed` failed against `1ba3484`:

  ```text
  test a_failed_index_refresh_is_journaled_as_failed ... FAILED
  thread 'a_failed_index_refresh_is_journaled_as_failed' (10372) panicked at crates/native/tests/unit/canonical_commit.rs:625:5:
  assertion `left == right` failed
    left: []
   right: [("library.file.index", Failed, Some("internal")), ("library.folder.index", Failed, Some("internal"))]
  test result: FAILED. 16 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
  ```

- The I4 row said a failed refresh stores nothing, but SQLite rollback cannot undo an OCFL version that was already published. I4 now describes that state: the catalog stays behind its head until reconcile brings it forward at the next open.

After the fix, `cargo test -p filesquid-native --features desktop --test unit_canonical_commit --test unit_catalog --test catalog_features --locked`:

```text
     Running tests/catalog_features.rs
  Scenario: An index refresh that changes nothing journals nothing
   ✔  And the file "f-1" was indexed in the collection "Summer" at listing order 3
   ✔  When the file "f-1" is indexed in the collection "Summer" at listing order 3 again
   ✔  Then the host journal should hold no entry since the first index
   ✔  When the file "f-1" is indexed in the collection "Summer" at listing order 4 again
12 scenarios (12 passed)
70 steps (70 passed)
     Running tests/unit/canonical_commit.rs
test a_failed_index_refresh_is_journaled_as_failed ... ok
test a_recorded_index_refresh_that_changes_nothing_journals_nothing ... ok
test an_index_refresh_that_changes_nothing_journals_nothing ... ok
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
     Running tests/unit/catalog.rs
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.09s
```

## Cubic review round 5

Cubic raised 2 findings on `1453cf0` (P2 2):

- DE-S-010's commit sequence (a failure leaves the catalog unchanged; the journal entry is stored only after publication) seemed to contradict the new failure journaling. DE-S-010 now scopes that sequence to the refresh's own transaction. It says the `failed` entry is written after that transaction rolls back, in a transaction of its own, and is the only thing a failed refresh adds to the catalog. TDD-ENFORCE: not applicable, documentation only.
- A refresh of a library the catalog does not hold returned before the failure journaling ran. `write_journaled` now journals a refusal that comes before any library is open (unknown library, poisoned lock) as `failed` straight to the host journal, targeting the named library. Test first: `a_refresh_of_an_unknown_library_is_journaled_as_failed` failed against `1453cf0`:

  ```text
  test a_refresh_of_an_unknown_library_is_journaled_as_failed ... FAILED
  thread 'a_refresh_of_an_unknown_library_is_journaled_as_failed' (26684) panicked at crates/native/tests/unit/canonical_commit.rs:668:5:
  assertion `left == right` failed
    left: []
   right: [("library.folder.index", Failed, Some("internal"), ["0b0f8a52-3f0e-4c4b-9a59-5b3c0e7d1a02"])]
  test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.06s
  ```

After the fix, `cargo test -p filesquid-native --features desktop --test unit_canonical_commit --test unit_catalog --test catalog_features --locked`:

```text
     Running tests/catalog_features.rs
12 scenarios (12 passed)
70 steps (70 passed)
     Running tests/unit/canonical_commit.rs
test a_failed_index_refresh_is_journaled_as_failed ... ok
test a_refresh_of_an_unknown_library_is_journaled_as_failed ... ok
test a_recorded_index_refresh_that_changes_nothing_journals_nothing ... ok
test an_index_refresh_that_changes_nothing_journals_nothing ... ok
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
     Running tests/unit/catalog.rs
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.65s
```

## Cubic review round 6

Cubic raised 1 finding on `6131d28` (P1): the refusal that comes before any library is open was appended straight to the host journal store and was lost if the store refused it. `SqliteCatalog::with_failure_journal` now takes the host's bounded journal queue (`JournalSender`, the same queue `LibraryService` journals refused commands through: producers wait and entries are never dropped), and such a refusal is sent there. The desktop host wires `host.monitor().journal()` in `open_library_catalog`, and the test fixture wires its writer. Without a queue (a catalog built for tests or tools), the entry is still appended directly, and an entry the store refuses is reported on stdout. Test first: `a_refresh_of_an_unknown_library_is_journaled_as_failed` was rewritten to open a catalog whose failure queue writes to a separate journal, and to assert that the entry arrives there and not in the catalog's store. Against `6131d28` it did not compile:

```text
error[E0599]: no method named `with_failure_journal` found for struct `SqliteCatalog` in the current scope
   --> crates/native/tests/unit/canonical_commit.rs:666:10
error: could not compile `filesquid-native` (test "unit_canonical_commit") due to 1 previous error
```

After the fix, `cargo test -p filesquid-native --features desktop --test unit_canonical_commit --test unit_catalog --test catalog_features --locked`:

```text
     Running tests/catalog_features.rs
12 scenarios (12 passed)
70 steps (70 passed)
     Running tests/unit/canonical_commit.rs
test a_failed_index_refresh_is_journaled_as_failed ... ok
test a_refresh_of_an_unknown_library_is_journaled_as_failed ... ok
test a_recorded_index_refresh_that_changes_nothing_journals_nothing ... ok
test an_index_refresh_that_changes_nothing_journals_nothing ... ok
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     Running tests/unit/catalog.rs
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.34s
```

## Gate

`FULL=1 bash scripts/pre-commit.sh` from the repository root (branch change scope `rust:filesquid-api,filesquid-desktop,filesquid-native`; the full run covers it):

```text
✅ All Pre-Commit Hooks Passed! Test scope: full
```

## Mocked Environment Variables

None.
