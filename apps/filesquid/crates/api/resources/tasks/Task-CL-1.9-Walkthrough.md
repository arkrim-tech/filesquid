# Task CL-1.9 Walkthrough: Journal rebuild in the rebuild-from-files command

Branch: `feat/cl-1.9-journal-rebuild` (global [Task FS-2.9](../../../../../../resources/implementation-plan.md#task-fs-2.9); [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md) Decision 7). API contract `0.17.0`.

## API-S-032: Durable journal

Also [API-S-025](../srs.md) Local library and repair routes, PRD [F-061](../../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring) and [SRS-F-061](../../../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring) Activity journal and live health monitoring, [F-001](../../../../../../resources/prd.md#f-001-independent-local-library-and-ownership) Independent local library and ownership, [RC-010](../../../../../../resources/prd.md#rc-010-complete-ocfl-preservation-and-independent-recovery) Complete OCFL preservation and independent recovery.

## What changed

- **Journal rebuild** (`local_library/journal_rebuild.rs`): with a `JournalRestore` installed (`LocalLibraryService::with_journal_restore`: the host-injected journal store and the profile transform), every rebuild from files also reads the library's sealed segments one by one (`visit_sealed_segments`) and every version's per-commit journal file (`read_version_journal`). An unreadable segment or file costs only its own entries; the rebuild keeps one entry per journal ID (the sealed copy first) and appends them in ID order, in batches of 1,000, under their own IDs. The store keeps each ID once, so a second rebuild adds nothing, and new entries after the rebuild get IDs above the restored ones.
- **Report**: `RebuildReport.journal` (`JournalRebuild`): `restored`, `already_present`, `conflicting` (an ID or client event ID the store holds for another entry; the held entry is kept, never overwritten), `unreadable` (segments, objects, files or lines that cannot be read, and lines without an ID) and `last_sealed_at` (the RFC 3339 `created` time of the journal object's head). A storage or store outage stops the rebuild as `unavailable`; more than 10,000,000 distinct entries is `journal_too_large`. Without a restore, the report has no `journal` and the rebuild is as before.
- **Per-commit files of every object**: `canonical::read_object_journal` reads one object's `provenance/<version>.journal.jsonl` files, so the rebuild covers the snapshot, file objects' version labels and repaired objects' provenance-only versions; `read_commit_journal` now uses it for the snapshot.
- **Last seal**: `journal::seal::last_sealed_at`.
- **Contract**: 0.17.0 (`JournalRebuild`, `RebuildReport.journal`); `schema.d.ts` regenerated.
- **Docs**: API-S-032 states what the rebuild reports and that a conflicting ID is never overwritten; the API-F2.12 row now also requires the file-route commits to go through `CommitRecorder` (the API-F2.11 gap, folded in by the owner's 2026-10-20 "follow-ups" decision), and the API-F2.11 walkthrough points at it; DE-0.11 names `with_journal_restore` as the import path.

## Not delivered in this task, and why

- **The desktop wiring** is [DE-0.11](../../../desktop/resources/plan.md): recreating `<data dir>/filesquid.sqlite` and installing its SQLite store as the restore target. No shipped host installs a `JournalRestore` yet, so the route's report carries no `journal` until then.
- **File-route commits** carry no per-commit journal file until [API-F2.12](../plan.md); their entries come back only when they were sealed.
- **Installation (host) events** live in the installation's journal object, not in a library; a library rebuild restores the library's journal only.

## SRS traceability

| Requirement | Tests |
|---|---|
| API-S-032 every committed operation's entries are back (commits that wrote a per-commit journal file; file-route commits only when sealed, until API-F2.12) | `unit_journal_rebuild::a_rebuild_restores_every_committed_and_sealed_entry_exactly_once`, `a_committed_operations_entries_come_back_without_any_seal`; BDD "Database deleted, rebuild restores the journal" |
| API-S-032 every entry up to the last seal, exactly once | `a_rebuild_restores_every_committed_and_sealed_entry_exactly_once`, `a_second_rebuild_restores_nothing_new`; BDD "no entry twice" |
| API-S-032 bounded loss (while seals succeed, only events after the last seal that changed nothing; until API-F2.12, file-route commits after the last seal are lost too) | `a_rebuild_restores_every_committed_and_sealed_entry_exactly_once`; BDD "only entries that changed nothing after the last seal should be missing" |
| API-S-032 report of restored entries and the last seal | `a_rebuild_restores_every_committed_and_sealed_entry_exactly_once`, `a_library_without_commits_or_seals_restores_nothing` |
| A held ID is reported, never overwritten (compared field by field) | `an_id_the_fresh_store_holds_for_another_entry_is_reported_not_overwritten`, `an_entry_that_differs_in_any_field_is_conflicting` |
| One unreadable segment, version file or object costs only itself, counted once | `one_unreadable_segment_costs_only_its_own_entries`, `one_unreadable_version_file_costs_only_its_own_entries`, `an_unopenable_journal_object_is_counted_once` |
| New IDs after a rebuild | `new_entries_after_a_rebuild_get_ids_above_the_restored_ones` |
| API-S-025 rebuild unchanged without a restore; contract | `a_host_without_a_journal_restore_reports_no_journal`; `unit_contract` (samples of `JournalRebuild` and `RebuildReport` with and without `journal`) |

## TDD: failing run before implementation

`unit_journal_rebuild` built against `origin/main` (`4f45753`), errors counted:

```text
      1 error: could not compile `filesquid-api` (test "unit_journal_rebuild") due to 7 previous errors
      1 error[E0432]: unresolved import `filesquid_api::local_library::JournalRestore`
      1 error[E0599]: no method named `with_journal_restore` found for struct `LocalLibraryService` in the current scope
      5 error[E0609]: no field `journal` on type `RebuildReport`
```

## Mocked Environment Variables

None.

## Raw test output

`cargo test -p filesquid-api --locked --test unit_journal_rebuild`:

```text
test a_host_without_a_journal_restore_reports_no_journal ... ok
test a_library_without_commits_or_seals_restores_nothing ... ok
test a_committed_operations_entries_come_back_without_any_seal ... ok
test an_entry_that_differs_in_any_field_is_conflicting ... ok
test a_rebuild_restores_every_committed_and_sealed_entry_exactly_once ... ok
test an_unopenable_journal_object_is_counted_once ... ok
test one_unreadable_segment_costs_only_its_own_entries ... ok
test a_second_rebuild_restores_nothing_new ... ok
test an_id_the_fresh_store_holds_for_another_entry_is_reported_not_overwritten ... ok
test new_entries_after_a_rebuild_get_ids_above_the_restored_ones ... ok
test one_unreadable_version_file_costs_only_its_own_entries ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

`cargo test -p filesquid-api --locked --test features` (the rebuild scenario):

```text
  Scenario: Database deleted, rebuild restores the journal
   ✔> Given a desktop library with recorded activity
   ✔  Given committed operations and other entries sealed at the last hourly seal
   ✔  And entries that changed nothing written after that seal
   ✔  When the operational database is deleted and I rebuild the library from its files
   ✔  Then the journal should contain every committed operation's entries
   ✔  And every entry up to the last seal
   ✔  And no entry twice
   ✔  And only entries that changed nothing after the last seal should be missing
```

Summary of that run:

```text
[Summary]
21 features
330 scenarios (229 passed, 101 skipped)
1133 steps (1032 passed, 101 skipped)
```
