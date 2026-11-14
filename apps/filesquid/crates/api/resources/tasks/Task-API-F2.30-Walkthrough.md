# Task API-F2.30 Walkthrough: Configurable expiry interval for Recently deleted

Task row: [API-F2.30](../plan.md). Global task: Task FS-2.5 ([implementation plan](../../../../../../resources/implementation-plan.md)). Owner decision 2026-11-02, verbatim: "RE: DE-S-014 - "at launch and then every hour" is the default, this should be a configurable setting".

## Summary of changes

- **Setting (`filesquid-api`).** `InstallationSettings` gains `expiry_interval_minutes` (default 60, `DEFAULT_EXPIRY_INTERVAL_MINUTES`), a whole number of minutes from 5 to 1,440 (`MIN_EXPIRY_INTERVAL_MINUTES`, `MAX_EXPIRY_INTERVAL_MINUTES`, `is_expiry_interval`). `LibraryService::set_installation_settings` refuses a value outside the bounds as `invalid_request` (reason `invalid_expiry_interval`), journaled once as `failed`, with nothing stored. A value that is not a whole number is refused by the route when it reads the body. A change is journaled within `settings.recently_deleted.set` without its values, and a change to the current values stores nothing, as before.
- **Routes.** `GET /api/v1/settings/recently-deleted` answers `expiry_interval_minutes`. `PUT` on the same path takes it as a required field of its `input`, admin only (`forbidden` otherwise). The `PUT` replaces the whole settings, so the field is required and a body without it is `invalid_request`. No client called the `PUT` before this task.
- **Schedule.** `filesquid_api::library::ExpirySchedule` is pure. It keeps the start of the host's last run and answers, for the interval read at that moment, when the next run falls due: at once before the first run, else one interval after the last start. It keeps no interval, so a changed interval applies to the next run without a restart, at once when that time has already passed. An interval outside the bounds is held to them.
- **Operational store (`filesquid-native`).** `recently_deleted_settings` gains the column `expiry_interval_minutes` (`NOT NULL DEFAULT 60`, `CHECK` 5 to 1,440). A store written before this task gains the column at open through `ALTER TABLE` and keeps its other values.
- **Desktop monitor (`filesquid-desktop`).** `ExpiryMonitor` replaces the fixed hourly wait. Every second (`EXPIRY_TICK`) it reads the stored interval, asks the schedule, and runs the expiry job when a run is due. Its clock is the time since launch, so a change of the system clock does not move a run. While the settings store cannot be read it keeps the interval it read last and logs a warning.
- **Contract.** OpenAPI 0.37.0: `expiry_interval_minutes` (integer, 5 to 1,440) on `InstallationSettings` and `RecentlyDeletedSettings`. `apps/filesquid/ui/src/api/schema.d.ts` and `contract-version.ts` regenerated with `npm run contract:types`.
- **Feature file.** The outline "The admin sets an interval within its bounds" listed 60, the default, and expected one journal entry, which contradicts the scenario "Setting the current interval stores nothing". The example is now 720.

## Open point for the owner

The bounds of 5 to 1,440 minutes are still the orchestrator's proposal in the plan row and the SRS. They are two constants and two `CHECK` clauses.

## SRS mapping

## API-S-019: Library domain commands and queries

**Routes and expiry runs:** the interval is answered by the settings `GET`, changed by the admin only, refused outside the bounds or when not a whole number with nothing stored, journaled without its values, and a changed interval takes effect for the next run without a restart.

| Constraint | Tests |
|---|---|
| Default 60, answered as `expiry_interval_minutes` | `unit_recently_deleted::the_expiry_interval_defaults_to_60_minutes_within_5_to_1440`, `integration_recently_deleted::the_admin_reads_and_sets_the_expiry_interval_within_its_bounds`, scenario "The interval is one hour by default" |
| 5 and 1,440 are stored | the same integration test, `unit_recently_deleted::the_service_stores_an_interval_within_the_bounds_and_refuses_one_outside_them`, outline "The admin sets an interval within its bounds" |
| 4, 1,441 and a fraction refused `invalid_request`, nothing stored | `integration_recently_deleted::an_expiry_interval_outside_the_bounds_or_not_whole_is_refused_with_nothing_stored`, outline "An interval outside the bounds is refused" |
| A caller who is not the admin is `forbidden` | `integration_recently_deleted::a_caller_who_is_not_the_admin_may_not_change_the_expiry_interval`, scenario "Only the admin changes the interval" |
| Journaled once without its value; the current value stores nothing | the unit and integration tests above, scenario "Setting the current interval stores nothing" |
| Next run one new interval after the last start, or at once | `unit_recently_deleted::the_schedule_is_due_at_launch_and_then_one_interval_after_the_last_start`, `a_changed_interval_moves_the_next_run_and_runs_at_once_when_already_passed`, `the_schedule_holds_an_interval_to_the_bounds`, scenarios "A changed interval takes effect for the next run without a restart" and "An interval that has already passed runs at once" |
| Contract | `unit_contract` (version 0.37.0, schema examples, documented statuses) |

## DE-S-014: Durable jobs and checkpoint-on-quit

The interval is kept in the operational store, and the host's expiry monitor runs at launch, then at the stored interval, taking a changed interval for its next run without a relaunch.

| Constraint | Tests |
|---|---|
| Kept in the operational store, surviving a reopen; bounds enforced by the store | native `unit_recently_deleted_settings::the_expiry_interval_is_stored_within_its_bounds_and_survives_a_reopen` |
| A store from before this task gains the column at 60 | native `unit_recently_deleted_settings::a_store_from_before_the_expiry_interval_gains_it_at_60_minutes` |
| Runs at launch, then one stored interval after the last start; a changed interval without a relaunch; at once when already passed | desktop `integration_library_routes::recently_deleted_settings_and_the_expiry_job_are_wired`, [`desktop_expiry_interval_F-008.feature`](../../../desktop/tests/features/desktop_expiry_interval_F-008.feature) (7 scenarios, all bound; the desktop harness fails on skipped steps) |

## Test-first evidence

The API and desktop tests were run before the implementation and failed to compile against the missing items:

```text
error[E0432]: unresolved imports `filesquid_api::library::is_expiry_interval`, `filesquid_api::library::ExpirySchedule`, `filesquid_api::library::DEFAULT_EXPIRY_INTERVAL_MINUTES`, `filesquid_api::library::MAX_EXPIRY_INTERVAL_MINUTES`, `filesquid_api::library::MIN_EXPIRY_INTERVAL_MINUTES`
error[E0560]: struct `InstallationSettings` has no field named `expiry_interval_minutes`
error[E0432]: unresolved import `filesquid_desktop_lib::recently_deleted::ExpiryMonitor`
error[E0599]: no method named `interval_minutes` found for struct `ExpiryTask` in the current scope
```

The native tests were written before the store change but first run after it, so no failing run of them was recorded.

## Test output

```text
$ cargo test -p filesquid-api --locked   (excerpt: the suites this task changed; whole run exit 0)
running 19 tests
test a_changed_interval_moves_the_next_run_and_runs_at_once_when_already_passed ... ok
test a_file_deleted_on_its_own_is_deleted_one_by_one_under_the_default_retention ... ok
test a_library_retention_sets_purge_after_and_never_moves_a_deleted_file ... ok
test a_failed_object_purge_is_journaled_as_failed_and_retried_at_the_next_run ... ok
test a_retention_outside_one_to_3650_days_is_refused ... ok
test a_restore_whose_home_is_gone_creates_an_untitled_collection_once_per_home ... ok
test a_run_with_nothing_due_changes_and_journals_nothing ... ok
test delete_all_version_history_asks_for_the_object_s_deletion ... ok
test expiry_removes_due_files_from_every_view_and_records_their_object_work ... ok
test deleting_a_collection_deletes_its_files_with_it_and_keeps_its_title ... ok
test installation_settings_default_to_30_days_and_keep_version_history ... ok
test the_expiry_interval_defaults_to_60_minutes_within_5_to_1440 ... ok
test the_installation_default_applies_to_a_library_without_its_own_period ... ok
test a_restore_into_a_home_collection_that_exists_returns_the_file_in_place ... ok
test the_first_free_title_is_used ... ok
test the_schedule_holds_an_interval_to_the_bounds ... ok
test the_schedule_is_due_at_launch_and_then_one_interval_after_the_last_start ... ok
test the_service_stores_an_interval_within_the_bounds_and_refuses_one_outside_them ... ok
test a_restore_is_refused_when_every_untitled_title_is_taken ... ok
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
running 9 tests
test a_caller_who_is_not_the_admin_may_not_change_the_expiry_interval ... ok
test a_restore_whose_home_is_gone_names_the_untitled_collection_it_created ... ok
test a_library_the_caller_may_not_read_is_left_out_of_recently_deleted ... ok
test an_expiry_interval_outside_the_bounds_or_not_whole_is_refused_with_nothing_stored ... ok
test the_command_route_takes_only_trash_commands_in_their_own_scope ... ok
test the_admin_reads_and_sets_the_expiry_interval_within_its_bounds ... ok
test the_node_is_refused_by_the_folder_listing_and_found_by_is_deleted ... ok
test recently_deleted_lists_the_deleted_files_of_every_library_with_their_own_retention ... ok
test the_settings_answer_the_default_the_expiry_setting_and_each_library_s_period ... ok
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
Feature: Configurable expiry interval for Recently deleted
  Scenario: The interval is one hour by default
   ✔> Given expiry-interval installation with one open library holding a deleted file
   ✔  When expiry-interval caller reads the Recently deleted settings
   ✔  Then expiry-interval answer should carry expiry_interval_minutes 60
  Scenario Outline: The admin sets an interval within its bounds
   ✔> Given expiry-interval installation with one open library holding a deleted file
   ✔  When expiry-interval admin sets the expiry interval to 5 minutes
   ✔  Then expiry-interval settings should hold 5 minutes
   ✔  And expiry-interval change should be journaled once as settings.recently_deleted.set without its values
  Scenario Outline: The admin sets an interval within its bounds
   ✔> Given expiry-interval installation with one open library holding a deleted file
   ✔  When expiry-interval admin sets the expiry interval to 720 minutes
   ✔  Then expiry-interval settings should hold 720 minutes
   ✔  And expiry-interval change should be journaled once as settings.recently_deleted.set without its values
  Scenario Outline: The admin sets an interval within its bounds
   ✔> Given expiry-interval installation with one open library holding a deleted file
   ✔  When expiry-interval admin sets the expiry interval to 1440 minutes
   ✔  Then expiry-interval settings should hold 1440 minutes
   ✔  And expiry-interval change should be journaled once as settings.recently_deleted.set without its values
  Scenario: A changed interval takes effect for the next run without a restart
   ✔> Given expiry-interval installation with one open library holding a deleted file
   ✔  Given expiry-interval host ran expiry at launch with the default interval
   ✔  When expiry-interval admin sets the expiry interval to 15 minutes
   ✔  Then expiry-interval next run should fall due 15 minutes after the last run started
   ✔  And expiry-interval host should not need a restart
  Scenario: An interval that has already passed runs at once
   ✔> Given expiry-interval installation with one open library holding a deleted file
   ✔  Given expiry-interval last run started 30 minutes ago with the default interval
   ✔  When expiry-interval admin sets the expiry interval to 10 minutes
   ✔  Then expiry-interval next run should start at once
  Scenario: Setting the current interval stores nothing
   ✔> Given expiry-interval installation with one open library holding a deleted file
   ✔  When expiry-interval admin sets the expiry interval to 60 minutes
   ✔  Then expiry-interval store should be unchanged and nothing journaled
  Scenario Outline: An interval outside the bounds is refused
   ✔> Given expiry-interval installation with one open library holding a deleted file
   ✔  When expiry-interval admin sets the expiry interval to 4 minutes
   ✔  Then expiry-interval answer should be refused as invalid_request
   ✔  And expiry-interval settings should still hold 60 minutes
  Scenario Outline: An interval outside the bounds is refused
   ✔> Given expiry-interval installation with one open library holding a deleted file
   ✔  When expiry-interval admin sets the expiry interval to 1441 minutes
   ✔  Then expiry-interval answer should be refused as invalid_request
   ✔  And expiry-interval settings should still hold 60 minutes
  Scenario Outline: An interval outside the bounds is refused
   ✔> Given expiry-interval installation with one open library holding a deleted file
   ✔  When expiry-interval admin sets the expiry interval to 1.5 minutes
   ✔  Then expiry-interval answer should be refused as invalid_request
   ✔  And expiry-interval settings should still hold 60 minutes
  Scenario: Only the admin changes the interval
   ✔> Given expiry-interval installation with one open library holding a deleted file
   ✔  When expiry-interval caller who is not the admin sets the expiry interval to 30 minutes
   ✔  Then expiry-interval answer should be refused as forbidden
   ✔  And expiry-interval settings should still hold 60 minutes
Feature: File activity, comments and user activity stream
  Scenario: The stream is derived from the journal
571 scenarios (382 passed, 189 skipped)
2001 steps (1812 passed, 189 skipped)

$ cargo test -p filesquid-native --features desktop --test unit_recently_deleted_settings --test unit_recently_deleted --locked
running 8 tests
test a_restore_whose_home_is_gone_creates_an_untitled_collection_in_one_version ... ok
test a_deletion_stopped_part_way_is_finished_by_the_next_run ... ok
test a_library_retention_command_is_kept_in_the_snapshot_and_journaled ... ok
test delete_all_version_history_deletes_the_object ... ok
test the_cause_and_the_removed_home_title_survive_a_rebuild ... ok
test keep_version_history_rewrites_the_object_as_a_tombstone_and_journals_the_purge ... ok
test a_crash_at_each_step_of_the_tombstone_write_is_completed_by_the_next_run ... ok
test the_expiry_runs_as_a_durable_job_over_every_open_library ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.64s
running 3 tests
test settings_default_survive_a_reopen_and_are_journaled ... ok
test the_expiry_interval_is_stored_within_its_bounds_and_survives_a_reopen ... ok
test a_store_from_before_the_expiry_interval_gains_it_at_60_minutes ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

$ cargo test -p filesquid-desktop --features test-webdriver --test features --test integration_library_routes --locked   (excerpt)
Feature: The desktop's expiry monitor follows the stored expiry interval
  Scenario: Expiry runs at launch and then one stored interval after the last run started
   ✔> Given desktop-expiry host launched with its Recently deleted settings in the operational store
   ✔  When desktop-expiry monitor ticks at launch
   ✔  Then desktop-expiry monitor should have completed 1 expiry job
   ✔  And desktop-expiry monitor should not run 59 minutes after the last run started
   ✔  And desktop-expiry monitor should run 60 minutes after the last run started
   ✔  And desktop-expiry monitor should have completed 2 expiry jobs
  Scenario: A changed interval is taken for the next run without a relaunch
   ✔> Given desktop-expiry host launched with its Recently deleted settings in the operational store
   ✔  Given desktop-expiry monitor ran at launch
   ✔  When desktop-expiry owner sets the expiry interval to 15 minutes
   ✔  Then desktop-expiry next run should fall due 15 minutes after the last run started
   ✔  And desktop-expiry monitor should not run 14 minutes after the last run started
   ✔  And desktop-expiry monitor should run 15 minutes after the last run started
  Scenario: A new interval that has already passed runs at once
   ✔> Given desktop-expiry host launched with its Recently deleted settings in the operational store
   ✔  Given desktop-expiry monitor ran at launch
   ✔  And desktop-expiry 30 minutes have passed
   ✔  When desktop-expiry owner sets the expiry interval to 10 minutes
   ✔  Then desktop-expiry monitor should run at its next tick
  Scenario: The stored interval survives a relaunch
   ✔> Given desktop-expiry host launched with its Recently deleted settings in the operational store
   ✔  Given desktop-expiry owner sets the expiry interval to 5 minutes
   ✔  When desktop-expiry host is relaunched
   ✔  Then desktop-expiry settings should answer expiry_interval_minutes 5
   ✔  And desktop-expiry monitor should run at its next tick
   ✔  And desktop-expiry monitor should run 5 minutes after the last run started
  Scenario Outline: An interval outside the bounds is refused and the monitor keeps its interval
   ✔> Given desktop-expiry host launched with its Recently deleted settings in the operational store
   ✔  Given desktop-expiry monitor ran at launch
   ✔  When desktop-expiry owner sets the expiry interval to 4 minutes
   ✔  Then desktop-expiry answer should be refused as invalid_request
   ✔  And desktop-expiry next run should fall due 60 minutes after the last run started
  Scenario Outline: An interval outside the bounds is refused and the monitor keeps its interval
   ✔> Given desktop-expiry host launched with its Recently deleted settings in the operational store
   ✔  Given desktop-expiry monitor ran at launch
   ✔  When desktop-expiry owner sets the expiry interval to 1441 minutes
   ✔  Then desktop-expiry answer should be refused as invalid_request
   ✔  And desktop-expiry next run should fall due 60 minutes after the last run started
  Scenario Outline: An interval outside the bounds is refused and the monitor keeps its interval
   ✔> Given desktop-expiry host launched with its Recently deleted settings in the operational store
   ✔  Given desktop-expiry monitor ran at launch
   ✔  When desktop-expiry owner sets the expiry interval to 1.5 minutes
   ✔  Then desktop-expiry answer should be refused as invalid_request
   ✔  And desktop-expiry next run should fall due 60 minutes after the last run started
Feature: Desktop file and folder import
71 scenarios (71 passed)
335 steps (335 passed)
test recently_deleted_settings_and_the_expiry_job_are_wired ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.24s
```

Coverage: the repository sets no numeric coverage threshold; every constraint above has a unit, integration and Cucumber test.

## Mocked Environment Variables

None.

## Pre-commit hook

The head-commit run is `FULL=1 bash scripts/pre-commit.sh`. Its summary line (`Test scope: full`) is recorded in the pull request body, because the run follows the last commit.
