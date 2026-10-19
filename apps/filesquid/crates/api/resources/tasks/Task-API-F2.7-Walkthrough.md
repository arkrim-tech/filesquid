# Task API-F2.7 Walkthrough: OCFL validation findings in health and the Monitor

Module task [API-F2.7](../plan.md) of global [Task 1.1](../../../../../../resources/implementation-plan.md#task-1.1). Branch `feat/api-f2.7-ocfl-validation-findings`, from `main` at `24693fd` (PR #65, CL-1.1). Dependencies API-F2.1, API-F1.7, CL-1.1, IO-0.5 and IO-0.6 were complete.

**Status: done.** The takeover trigger is built and tested at its seam (`ValidationService::validate_after_commit`, `OcflValidation::on_commit`). The engine reports takeovers since PR #78 (`CommitReceipt::took_over`), which resolves the Conflict Artifact; the hosts pass it on when they first commit.

## API-S-022: OCFL validation findings MUST reach health and the Monitor

Each host that runs `filesquid-api` MUST run the `filesquid-ocfl` validator over every object of every library it has open: on a schedule, for an object after a commit that took over an expired or lost reservation, and on request. Runs are registered, at most one per library at a time, capped and cancellable, and walk a persistent per-library cursor in object-ID order. The latest findings per object are kept in the operational store. Each error finding is journaled redacted. While any error finding is open, health is at least `degraded`, with the codes and the number of affected objects. Warnings are listed only. A route lists the findings and runs to the desktop owner or a server administrator, with locations only for a caller who may read the object. Changes are pushed on `health.change` ([module SRS](../srs.md)).

Parents: [F-061 Activity journal and live health monitoring](../../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring), [SRS-F-061 Activity journal and live health monitoring](../../../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring), [F-042 OCFL preservation and portable recovery](../../../../../../resources/prd.md#f-042-ocfl-preservation-and-portable-recovery), OCFL-S-018, [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md).

| Acceptance criterion | Gate | State |
|---|---|---|
| (1) E023 degrades health with reason `ocfl_validation_errors` naming E023, journals one finding, appears in the route, and health is `ok` again once fixed and revalidated | `an_e023_object_degrades_health_until_it_is_fixed_and_revalidated`, `an_e023_object_on_a_real_directory_is_found` (FsBackend), `the_owner_sees_locations_and_the_administrator_does_not`, scenario 1 | Met |
| (2) W010 is listed as a warning, journaled once, and health stays `ok` | `a_w010_object_is_listed_as_a_warning_and_leaves_health_ok`, `a_run_over_many_objects_journals_one_entry_per_finding_with_no_loss`, scenario 2 | Met |
| (3) A takeover commit triggers validation of that object | `a_takeover_commit_triggers_validation_of_that_object`, scenario 3 (at the seam; see the Conflict Artifact) | Met; the engine reports takeovers since PR #78, and hosts wire `on_commit` when they first commit |
| (4) Only the owner or an administrator may call; locations hidden from a caller who may not read the object | `anyone_else_is_refused_and_the_refusal_is_journaled`, `locations_are_shown_only_to_a_caller_who_may_read_the_object`, scenario 4, contract and auth tests | Met |
| (5) Redaction: no logical path, content path, title or content in journal rows or health reasons | `journal_rows_and_health_reasons_carry_no_path_title_or_content`, scenario steps | Met |
| (6) A run stops at its cap and reports the unchecked objects; never twice at once per library | `a_run_stops_at_its_cap_and_reports_what_it_did_not_check`, `a_run_never_starts_twice_at_once_for_one_library`, `start_runs_refuses_an_unknown_library_and_a_busy_one`, `a_cancelled_run_stops_and_reports_the_rest` | Met |
| (7) The route reports the last run and the next scheduled run, and both change after a run | `the_findings_view_reports_the_last_run_and_the_next_scheduled_run`, `a_library_is_due_one_interval_after_it_was_opened`, `the_scheduler_tick_starts_a_scheduled_run_when_one_is_due` | Met |
| (8) Capped runs resume from the stored cursor (also after a restart) in ID order; an object deleted ahead is skipped; one created behind is checked on the next pass; the end wraps; on request continues unless asked to start over | `capped_runs_resume_from_the_cursor_in_object_id_order_and_wrap`, `an_on_request_run_continues_from_the_cursor_unless_asked_to_start_over`, `the_sqlite_store_keeps_findings_cursor_and_run_across_a_restart`, scenario 5 | Met |
| (9) Deleting an object with an open E023 closes it, journals one `ocfl.validation.finding_closed`, and health returns to `ok` | `deleting_an_object_with_an_open_e023_closes_its_finding`, scenario 6 | Met |

Other constraints: defaults of 7 days and 100,000 objects, both bounded and journaled when changed (`defaults_are_seven_days_and_one_hundred_thousand_objects`, `the_configuration_is_bounded_and_journaled`). Runs register as `ocfl.validation` with progress (`a_run_registers_in_the_process_registry_with_progress`). Findings changes are pushed on `health.change` (`a_change_in_the_open_findings_is_pushed_on_health_change`). A failing findings store fails health and the run (`a_failing_findings_store_fails_health_and_the_run`). The health rule is tested on its own (`open_ocfl_error_findings_degrade_health_with_codes_and_object_count`). `filesquid-api` still does no file or SQL access and links no `filesquid-native` (`unit_isolation`).

### Secondary SRS mapping

- **Settings.** The schedule and the cap are changed by the desktop owner or a server administrator through `PUT /api/v1/admin/ocfl/validation-settings`, journaled as `ocfl.validation.configure`.
- **SRS-F-061: Activity journal and live health monitoring.** Validation findings are a health input. Every run, refusal, finding and closure is journaled with opaque IDs and codes. The run is a registered process with a heartbeat.
- **SRS-F-042: OCFL preservation and portable recovery.** External edits to an object (the E023 stray file, a tampered file) are detected and reported, not adopted.
- **API-S-002: versioned contract.** `openapi.json` 0.7.0 adds the three routes (findings, validation runs, validation settings), the actions `ocfl.validation.run` and `ocfl.validation.configure`, and the schemas; `ui/src/api/schema.d.ts` is regenerated and the UI contract test passes.
- **API-S-006: application-layer isolation.** The findings store is injected through `FindingsStore`, like `JournalStore`. The SQLite store is `filesquid_native::SqliteFindings`.

## What changed

- `crates/api/src/ocfl_validation/`:
  - `store` and `memory`: the `FindingsStore` port and `MemoryFindingsStore`.
  - `run`: index, cursor order, the pass, journaling.
  - `service`: `ValidationService`, config, run lock, runs, takeover check, schedule.
  - `handle`: the object-safe `OcflValidation`, background runs, `tick`, `on_commit`.
  - `view`: the route types.
  - `routes`: `GET /api/v1/admin/ocfl/findings` and `POST /api/v1/admin/ocfl/validation-runs`.
  - `error`: `ValidationError`.
- Other `crates/api/src` changes:
  - `health.rs`: the inputs `ocfl_error_codes` and `ocfl_error_objects`, the rule `ocfl_validation_errors`, and the reason fields `codes` and `objects`.
  - `monitor.rs`: `install_findings` and `publish_health`. A failing findings store counts as an unreachable operational store.
  - `auth`: `Action::RunValidation` (`ocfl.validation.run`).
  - `ApiState::install_validation`.
  - Contract version 0.7.0.
- `crates/native`: `SqliteFindings` (three tables in `filesquid.sqlite`), `HostMonitor::start_validation`, and `HostMonitor::tick`, which starts the scheduled runs.
- Hosts: the desktop starts validation and keeps `DesktopValidation` for DE-1.1 to open libraries. The server starts it too; it has no OCFL library yet.
- Tests:
  - `crates/api/tests/unit/ocfl_validation.rs`, `tests/support/validation_fixture.rs` and `validation_steps.rs`.
  - `tests/features/ocfl_validation_findings_API-S-022.feature`.
  - A read hook in `ocfl_fixture.rs`.
  - Contract, auth, health-rule and BDD auth-step updates.
  - `crates/native/tests/unit/findings.rs` and a `host.rs` case.
- Docs: the module SRS (API-S-006 names `FindingsStore`), the ADR 033 amendment, the product SRS test lists for SRS-F-061, the README test commands, the plan row and checklist box, the MVP progress, and the [Conflict Artifact](./Task-API-F2.7-Conflict.md).

## For the desktop host (DE-1.1) and the UI (UI-F2.14)

- DE-1.1: after opening a library's storage root as `FsBackend`, call `DesktopValidation.0.open_library(library_id, Arc::new(backend))`, and `close_library` when it closes. After each commit, call `on_commit(library, object_root, object_id, receipt.took_over)` with the engine's commit receipt.
- UI-F2.14: `GET /api/v1/admin/ocfl/findings` (types in `schema.d.ts`: `FindingsView`), `POST /api/v1/admin/ocfl/validation-runs` (`RunRequest`, `RunAccepted`), and the health reason `ocfl_validation_errors` with `codes` and `objects`. Changes arrive on `health.change`.
- The schedule and cap: `PUT /api/v1/admin/ocfl/validation-settings` (`ValidationSettings`), reported in `FindingsView.settings`.

## Tests

TDD: the tests were written first. Before the module existed, `cargo test -p filesquid-api --test unit_ocfl_validation` failed to compile:

```text
error: could not compile `filesquid-api` (test "unit_ocfl_validation") due to 5 previous errors
error[E0432]: unresolved import `filesquid_api::ocfl_validation`
error[E0432]: unresolved import `filesquid_api::ocfl_validation`
error[E0599]: no method named `install_validation` found for struct `ApiState` in the current scope
error[E0609]: no field `codes` on type `HealthReason`
error[E0609]: no field `objects` on type `HealthReason`
```

After the implementation and Cubic rounds 1 to 4, `cargo test -p filesquid-api --locked --test unit_ocfl_validation --test unit_health_rules`:

```text
running 8 tests
test all_clear_is_ok_without_reasons ... ok
test queue_fill_threshold ... ok
test open_ocfl_error_findings_degrade_health_with_codes_and_object_count ... ok
test stalled_processes_degrade ... ok
test error_rate_thresholds ... ok
test free_space_thresholds ... ok
test unreachable_store_or_read_only_data_dir_fail ... ok
test worst_condition_wins_and_all_reasons_are_listed ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
running 36 tests
test a_failing_findings_store_fails_health_and_the_run ... ok
test a_finding_stays_open_until_a_later_validation_no_longer_reports_it ... ok
test a_change_in_the_open_findings_is_pushed_on_health_change ... ok
test a_cancelled_run_stops_and_reports_the_rest ... ok
test a_library_is_due_one_interval_after_it_was_opened ... ok
test a_findings_change_journals_a_health_transition_once ... ok
test a_run_never_starts_twice_at_once_for_one_library ... ok
test a_run_stopped_by_an_outage_reports_what_it_checked_and_publishes_health ... ok
test a_run_registers_in_the_process_registry_with_progress ... ok
test a_run_request_starts_runs_and_a_malformed_one_is_refused ... ok
test a_takeover_check_on_a_damaged_object_opens_an_error_finding ... ok
test a_run_stops_at_its_cap_and_reports_what_it_did_not_check ... ok
test a_run_whose_record_cannot_be_stored_is_reported_as_failed ... ok
test a_takeover_check_that_cannot_start_is_journaled ... ok
test a_takeover_check_reads_only_the_named_object ... ok
test a_takeover_commit_triggers_validation_of_that_object ... ok
test a_w010_object_is_listed_as_a_warning_and_leaves_health_ok ... ok
test an_object_deleted_after_the_run_indexed_it_has_its_findings_closed ... ok
test an_e023_object_degrades_health_until_it_is_fixed_and_revalidated ... ok
test anyone_else_is_refused_and_the_refusal_is_journaled ... ok
test an_on_request_run_continues_from_the_cursor_unless_asked_to_start_over ... ok
test defaults_are_seven_days_and_one_hundred_thousand_objects ... ok
test deleting_an_object_with_an_open_e023_closes_its_finding ... ok
test an_e023_object_on_a_real_directory_is_found ... ok
test capped_runs_resume_from_the_cursor_in_object_id_order_and_wrap ... ok
test journal_rows_and_health_reasons_carry_no_path_title_or_content ... ok
test locations_are_shown_only_to_a_caller_who_may_read_the_object ... ok
test start_runs_refuses_an_unknown_library_and_a_busy_one ... ok
test the_configuration_is_bounded_and_journaled ... ok
test the_findings_view_reports_the_last_run_and_the_next_scheduled_run ... ok
test the_owner_sees_locations_and_the_administrator_does_not ... ok
test the_scheduler_tick_starts_a_scheduled_run_when_one_is_due ... ok
test a_run_over_many_objects_journals_one_entry_per_finding_with_no_loss ... ok
test the_settings_route_changes_the_schedule_and_refuses_bad_values ... ok
test two_roots_that_declare_one_identifier_keep_separate_findings ... ok
test shutting_down_cancels_runs_and_refuses_new_ones ... ok
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
```

`cargo test -p filesquid-api --locked --lib ocfl_validation` (the service and run unit tests):

```text
running 5 tests
test ocfl_validation::service::tests::an_old_run_lock_never_frees_a_reopened_library ... ok
test ocfl_validation::service::tests::closing_a_library_cancels_its_run ... ok
test ocfl_validation::service::tests::reopening_an_open_library_keeps_its_locks ... ok
test ocfl_validation::run::tests::repeated_identifiers_get_distinct_keys ... ok
test ocfl_validation::service::tests::shutdown_waits_for_takeover_checks_and_refuses_new_ones ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 8 filtered out; finished in 0.06s
```

`cargo test -p filesquid-native --locked --test unit_findings --test unit_host`:

```text
running 3 tests
test the_memory_store_keeps_the_contract ... ok
test the_sqlite_store_keeps_findings_cursor_and_run_across_a_restart ... ok
test the_sqlite_store_keeps_the_contract ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
running 7 tests
test a_missing_data_dir_is_not_writable_and_has_no_free_space_reading ... ok
test an_existing_data_dir_reports_free_space ... ok
test a_second_host_monitor_is_refused_before_any_file_is_created ... ok
test the_write_probe_never_touches_an_existing_file ... ok
test host_monitor_opens_the_store_and_journals_start_and_stop ... ok
test a_second_started_monitor_is_shut_down_and_never_installed ... ok
test host_monitor_starts_ocfl_validation_once ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

`cargo test -p filesquid-api --locked --test features` (this task's feature, then the harness summary; the skipped scenarios belong to features whose steps land with later tasks):

```text
Feature: OCFL validation findings in health and the Monitor
  Scenario: A stray file under vN/ degrades health until it is removed
   ✔  Given an open library with a valid object and an object with a stray file under its head version
   ✔  When the owner requests a validation run
   ✔  Then health should be "degraded" with reason "ocfl_validation_errors" naming "E023" in 1 object
   ✔  And one "ocfl.validation.finding" entry for "E023" should be journaled without paths
   ✔  When the stray file is removed and the library is validated again
   ✔  Then one "ocfl.validation.finding_closed" entry for "E023" should be journaled without paths
   ✔  And health should be "ok"
  Scenario: An object without a version inventory is a warning only
   ✔  Given an open library with an object whose head has no version inventory
   ✔  When the owner requests a validation run
   ✔  Then the findings should list "W010" as a warning
   ✔  And one "ocfl.validation.finding" entry for "W010" should be journaled without paths
   ✔  And health should be "ok"
  Scenario: The takeover check validates the object a takeover commit wrote
   ✔  Given an open library with a valid object and an object with a stray file under its head version
   ✔  When the takeover check runs for the object with the stray file after a commit that took over an expired reservation
   ✔  Then health should be "degraded" with reason "ocfl_validation_errors" naming "E023" in 1 object
  Scenario: The administrator sees findings without locations
   ✔  Given an open library with a valid object and an object with a stray file under its head version
   ✔  When the owner requests a validation run
   ✔  Then the owner should see where the "E023" finding is
   ✔  And the server administrator should see the "E023" finding without its location
  Scenario: A capped run reports what it did not check and the next run resumes
   ✔  Given an open library with 3 valid objects and a run cap of 2
   ✔  When the owner requests a validation run
   ✔  Then the run should be "capped" with 2 objects checked and 1 not checked
   ✔  When the owner requests a validation run
   ✔  Then the cursor should have wrapped to the first object
  Scenario: Deleting an object closes its findings
   ✔  Given an open library with a valid object and an object with a stray file under its head version
   ✔  When the owner requests a validation run
   ✔  And the object with the stray file is deleted and the library is validated again
   ✔  Then one "ocfl.validation.finding_closed" entry for "E023" should be journaled without paths
   ✔  And health should be "ok"
14 features
217 scenarios (111 passed, 106 skipped)
517 steps (411 passed, 106 skipped)
```

## Cubic review

Round 1 on `b967a96`: 31 findings (P1 3, P2 14, P3 14). Fixed in the next commit:

- **Runs.**
  - A run lock releases only its own run, so a library closed and reopened during a run keeps its new lock (`an_old_run_lock_never_frees_a_reopened_library`).
  - A run stopped by an outage records what it checked and publishes health (`a_run_stopped_by_an_outage_reports_what_it_checked_and_publishes_health`).
  - Runs and takeover checks share a per-library object lock, so a stale result never overwrites a newer one.
  - A takeover check reads only the object at the committed root (`a_takeover_check_reads_only_the_named_object`), and one that cannot start is journaled (`a_takeover_check_that_cannot_start_is_journaled`).
  - Shutdown cancels runs, waits a bounded time and refuses new runs; `HostMonitor::stop` calls it (`shutting_down_cancels_runs_and_refuses_new_ones`).
  - The next due time is cached, so the ticker does not read SQLite every second.
- **Settings.** `PUT /api/v1/admin/ocfl/validation-settings` (action `ocfl.validation.configure`) changes the schedule and cap, and `FindingsView` reports them (`the_settings_route_changes_the_schedule_and_refuses_bad_values`).
- **Errors and redaction.**
  - Store errors say whether they are transient: a busy SQLite database is `unavailable`, anything else `internal`.
  - An oversized or unreadable body is a journaled `invalid_request`.
  - Stdout diagnostics name codes only.
  - `publish_health` journals a status transition as `health.changed`.
  - A second findings store on one monitor is logged.
  - `SqliteFindings` opens without following a link.
- **Stores.** `FindingChange::between` is shared by both stores; the memory store normalizes like SQLite; the store contract doc is corrected; `start_validation` checks before opening anything.
- **Docs and tests.**
  - The ADR 033 amendment, API-S-022 and the plan row say that every finding, warnings included, is journaled one by one (owner decision, 2026-10-19).
  - The API-S-006 parenthetical names the SQLite findings store, and the F-037 traceability row is fixed.
  - The Conflict Artifact quotes the ADRs.
  - The server doc says what it opens.
  - Test names and assertions are tightened (scheduler tick, configure journal, unknown status).

Answered without code: no production host opens a library yet, because the library-open lifecycle is DE-1.1; and the plan box is checked while criterion (3) waits on the engine, now stated in the plan row's Status line and the Conflict Artifact.

Round 2 on `8d4cb1f`: 6 new findings (P1 0, P2 4, P3 2), fixed in the next commit:

- Both stores normalize findings with `Finding::normalize` before they write and answer the change, so a repeat opens one finding (`unit_findings` contract).
- Health evaluation and status recording are serialized by a monitor lock, so a report never records over a newer one.
- A health transition through `publish_health` is journaled once (`a_findings_change_journals_a_health_transition_once`).
- A takeover check on an object that no longer opens validates it under its stand-in identifier (`a_takeover_check_on_a_damaged_object_opens_an_error_finding`).
- Walkthrough route and test names corrected.

Round 3: 1 new finding (P1 0, P2 0, P3 1), fixed: the Conflict Artifact on the takeover check is marked resolved (PR #78 took it over).

Round 4 on `fe2ca3f`: 15 new findings (P1 0, P2 11, P3 4). Fixed in the next commit:

- **Service.**
  - Opening a library that is already open replaces only its backend, so a run keeps its run lock and object lock (`reopening_an_open_library_keeps_its_locks`).
  - Closing a library removes it and cancels its run under one lock (`closing_a_library_cancels_its_run`).
  - A takeover check counts as going while it holds the library, refuses to start after shutdown, and shutdown waits for it (`shutdown_waits_for_takeover_checks_and_refuses_new_ones`).
- **Runs.**
  - Every root after the first that declares an object ID already indexed is keyed by its root's stand-in, so two roots never share findings (`repeated_identifiers_get_distinct_keys`, `two_roots_that_declare_one_identifier_keep_separate_findings`).
  - An object whose root is gone by the time the run reaches it has its findings closed, not a missing-declaration finding opened (`an_object_deleted_after_the_run_indexed_it_has_its_findings_closed`).
  - A run whose last-run record cannot be stored is reported and journaled as failed with the store's code (`a_run_whose_record_cannot_be_stored_is_reported_as_failed`).
  - `POST /api/v1/admin/ocfl/validation-runs` starts runs on a blocking thread, like the findings view.
- **Stores and hosts.**
  - `SqliteFindings::replace_findings` takes the write lock first (`BEGIN IMMEDIATE`), so a journal commit in between waits on the busy timeout.
  - A covering index by severity keeps the health summary the monitor reads every tick to the error rows.
  - `HostMonitor::start_validation` journals a failed start as `ocfl.validation.start` with its code, on the desktop and the server (ADR 033 amendment).
- **Tests and docs.** The store contract checks that two libraries stay apart (both stores); the stray-file scenario checks the journaled `finding_closed`; the takeover scenario says it invokes the check, since the commit wiring is pending.

Answered without code: indexing reads each object's inventory every run (the cursor is an object ID, so the order needs the IDs; the bound is the cap on validation, which reads content, and indexing reads one small file per object once per schedule); and a run that fails before indexing reports `not_checked: 0` with outcome `failed` and its error code, which already says nothing was counted.

## Mocked Environment Variables

None.
