# Task CLI-0.2 Walkthrough: Journal every `fsquid` command

Branch: `feat/cli-0.2-journal-every-command` (global [Task FS-2.9](../../../../../../resources/implementation-plan.md#task-fs-2.9); [ADR 036](../../../../../../resources/architecture/036-single-fsquid-cli.md) Decision 8, owner decision 2026-10-20). No API contract change.

## API-S-031: Journal query and export

The `fsquid` CLI journals through the same path as the hosts ([API-S-031](../../../api/resources/srs.md)). Also PRD [F-061](../../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring) and [SRS-F-061](../../../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring) Activity journal and live health monitoring, [F-050](../../../../../../resources/prd.md#f-050-filesquid-platform-and-distributions) and [SRS-F-050](../../../../../../resources/srs.md#srs-f-050-filesquid-platform-and-distributions) FileSquid platform and distributions.

## DE-S-015: Native-processing library boundary

[DE-S-015](../../../desktop/resources/srs.md) now admits `filesquid-cli` as the third package that may link `filesquid-native`, with default features only (never `desktop`) and no SQLite driver of its own (PRD [TC-005](../../../../../../resources/prd.md#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) Shared Rust graph, hosted mailbox isolation and OCFL writer).

## What changed

- **Dependencies** (`Cargo.toml`): `filesquid-api` (journal entry types, `JournalStore`), `filesquid-native` with default features (`SqliteJournal`, `JOURNAL_FILE`), `dirs` 7.0.0 (already in the lockfile through Tauri, whose `app_data_dir` is `dirs::data_dir()` joined with the app identifier; checked in the Tauri 2.12.4 source) and `serde_json`. The lockfile gains no new crate.
- **Journal** (`src/journal.rs`): the data directory is `FILESQUID_DATA_DIR` when set and not empty, otherwise `dirs::data_dir()/tech.filesquid.desktop`, the desktop's own. `open_journal` creates the directory and opens `filesquid.sqlite` in WAL mode before any command runs. `run_journaled` routes as before and writes one redacted entry per command that changes state or fails: `cli.<module>.<command>`, actor `cli`, host `fsquid`, outcome, error code (`command_error` for an error the module reports, `command_failed` for a non-zero exit) and the OCFL object identifier and version it committed as `object_ref`. A command line that names no module command is one failed `cli.route` entry (`module_required`, `unknown_command`). A success that changes nothing writes none; help needs no journal.
- **What "changed state" means**: the committed heads of the OCFL objects a command names (`--object`, default `.`, and `--registry` for metadata when it is another directory), read from their root inventories before and after it runs. An inventory that exists but cannot be read (I/O, over 64 MiB, no `id` and `head`) counts as a change, so no change goes unjournaled. A module command that writes a file outside its objects whenever it succeeds (metadata `export`) is listed in `Module::file_writers` and always journaled on success. The metadata module needed no change.
- **Refusals and failed writes**: a journal that cannot be opened refuses the command before it reads or writes anything else: exit 3, stderr names `FILESQUID_DATA_DIR`, stdout one JSON line (the redacted entry, `error_code: journal_unavailable`) marked `"journal_unavailable": true`. A journal write that fails prints the redacted entry marked `"journal_write_failed": true` and exits 4 whenever the command changed state or succeeded; a failed command that changed nothing keeps its own code.
- **Native boundary** (DE-0.8 follow-up): `NATIVE_LINKERS` in `desktop/tests/integration/native_boundary.rs` admits `("filesquid-cli", &[])`, a new test asserts the CLI links `filesquid-native` with no feature, and the synthetic case rejects a CLI with `desktop` or a direct `rusqlite`. `native_boundary_TC-005.feature` has a CLI scenario and a three-package rule, with the matching native Cucumber step (the two-package step it replaces is removed).
- **Docs**: DE-S-015; the root README `## View logs` (the "planned" bullet replaced by the working behaviour, and the commands below, run as written); the CLI README; the CLI plan header.

## Order of report and journal write

The metadata module prints its own success line (for example `set dcterms:title: committed v1`) before it returns, so the CLI writes the entry after that line and before it exits. Success is reported by the exit status: a failed write turns exit 0 into exit 4 with the `journal_write_failed` line, so a script never sees a successful exit for an unjournaled change. Writing the entry before the module's own line would need a change to the module's command runner, outside this task's crates.

## SRS traceability

| Requirement | Tests |
|---|---|
| API-S-031 a commit leaves one `cli.<module>.<command>` entry, opaque IDs only | `unit_journal::a_command_that_commits_leaves_one_redacted_entry`; BDD "A command that commits is journaled once with opaque IDs only" |
| API-S-031 a failed command leaves one failed entry | `a_failed_command_leaves_one_failed_entry`, `a_refused_command_leaves_one_failed_entry_with_its_error_code` |
| API-S-031 `fsquid validate` and unknown commands leave one failed `cli.route` entry | `validate_without_its_module_leaves_one_failed_route_entry`, `an_unknown_command_leaves_one_failed_route_entry`; BDD "A command line that names no module command is journaled as a failed route" |
| API-S-031 a no-op leaves none | `a_command_that_changes_nothing_leaves_no_entry`; BDD "A command that changes nothing is not journaled" |
| API-S-031 a file written outside the objects is a change | `a_successful_export_writes_a_file_and_is_journaled`; BDD "A successful export writes a file outside the object and is journaled" |
| API-S-031 an unreadable inventory counts as a change; one object named once | `an_inventory_that_cannot_be_read_counts_as_unobserved`, `a_registry_that_is_the_object_is_named_once` |
| API-S-031 a failed write after a commit exits non-zero with `journal_write_failed` | `a_journal_write_that_fails_after_a_commit_exits_non_zero_with_the_entry`, `a_failed_command_whose_entry_cannot_be_written_keeps_its_own_code` |
| API-S-031 `FILESQUID_DATA_DIR` selects the journal | `integration_binary::filesquid_data_dir_selects_the_journal` |
| API-S-031 WAL mode beside a running host | `a_command_runs_while_another_connection_holds_the_journal_in_wal_mode`; BDD "fsquid writes beside a running desktop host" |
| API-S-031 an unopenable journal refuses (exit 3), changes nothing, prints `journal_unavailable` | `an_unopenable_journal_refuses_the_command_and_changes_nothing`; BDD "A journal that cannot be opened refuses the command" |
| DE-S-015 the CLI links `filesquid-native` with default features only | `integration_native_boundary::the_cli_links_filesquid_native_with_default_features_only`, `the_linking_rules_reject_a_sqlite_driver_in_a_host_and_native_in_the_api`; native BDD "The fsquid CLI links filesquid-native with default features only", "No crate but the two hosts and the CLI links filesquid-native" |

## TDD: failing run before implementation

The new tests built against `origin/main` (`71c4262`) with this branch's manifest, errors counted:

```text
      1 error: could not compile `filesquid-cli` (test "unit_journal") due to 1 previous error
      1 error[E0432]: unresolved import `filesquid_cli::journal`
```

## Commands run for the README

From `apps/filesquid`, with `filesquid-server` running on the default `filesquid-data`:

```text
$ FILESQUID_DATA_DIR=filesquid-data cargo run -p filesquid-cli --bin fsquid -- set dcterms:title Holiday --id urn:uuid:7d2a1b0c-3e4f-4a5b-8c6d-9e0f1a2b3c4d --object holiday-object
set dcterms:title: committed v1
$ FILESQUID_DATA_DIR=filesquid-data cargo run -p filesquid-cli --bin fsquid -- validate dcterms:title
`validate` needs its module name: run fsquid metadata validate
$ curl -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:8083/api/v1/admin/journal?operation=cli.metadata.set"
{"entries":[{"id":6,"time":"2026-10-20T23:23:31.780Z","host":"fsquid","actor":"cli","operation":"cli.metadata.set","outcome":"succeeded","level":"info","target_ids":[],"object_ref":"urn:uuid:7d2a1b0c-3e4f-4a5b-8c6d-9e0f1a2b3c4d@v1","provenance":{"journal_only":true}}],"next_cursor":null}
```

## Mocked Environment Variables

None. The tests set `FILESQUID_DATA_DIR` to scratch directories under the system temporary directory.

## Raw test output

`cargo test -p filesquid-cli --locked`:

```text
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
13 scenarios (13 passed)
test an_unopenable_journal_refuses_the_command_and_changes_nothing ... ok
test filesquid_data_dir_selects_the_journal ... ok
test a_command_runs_while_another_connection_holds_the_journal_in_wal_mode ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test a_failed_command_whose_entry_cannot_be_written_keeps_its_own_code ... ok
test a_command_that_changes_nothing_leaves_no_entry ... ok
test a_refused_command_leaves_one_failed_entry_with_its_error_code ... ok
test a_failed_command_leaves_one_failed_entry ... ok
test a_command_that_commits_leaves_one_redacted_entry ... ok
test an_inventory_that_cannot_be_read_counts_as_unobserved ... ok
test an_unknown_command_leaves_one_failed_route_entry ... ok
test validate_without_its_module_leaves_one_failed_route_entry ... ok
test a_successful_export_writes_a_file_and_is_journaled ... ok
test a_journal_write_that_fails_after_a_commit_exits_non_zero_with_the_entry ... ok
test a_registry_that_is_the_object_is_named_once ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.22s
test bare_module_name_shows_module_help ... ok
test module_group_strips_the_module_name ... ok
test help_forms_resolve_to_help ... ok
test module_names_are_not_command_names ... ok
test shared_command_names_are_qualifier_only ... ok
test unique_command_runs_at_top_level ... ok
test unknown_command_is_an_error ... ok
test usage_names_fsquid_and_both_forms ... ok
test fsquid_is_the_only_cli_binary_in_the_workspace ... ok
test validate_with_module_runs ... ok
test validate_without_module_needs_the_module ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo test -p filesquid-cli --locked --test features`:

```text
Feature: Every fsquid command that changes something or fails is journaled
  Scenario: A command that commits is journaled once with opaque IDs only
   ✔> Given a fresh FileSquid data directory
   ✔  When I set a title on a new object with fsquid
   ✔  Then the journal should hold one "cli.metadata.set" entry that "succeeded"
   ✔  And that entry should name the object and its version but no path or value
  Scenario: A command that changes nothing is not journaled
   ✔> Given a fresh FileSquid data directory
   ✔  When I set a title on a new object with fsquid
   ✔  And I set the same title again with fsquid
   ✔  And I read the title with fsquid
   ✔  Then the journal should hold 1 entry
  Scenario: A successful export writes a file outside the object and is journaled
   ✔> Given a fresh FileSquid data directory
   ✔  When I set a title on a new object with fsquid
   ✔  And I export the object's metadata as CSV with fsquid
   ✔  Then the journal should hold 2 entries
   ✔  And the last entry should be a "cli.metadata.export" entry that "succeeded"
  Scenario: fsquid writes beside a running desktop host
   ✔> Given a fresh FileSquid data directory
   ✔  Given another connection holds the journal open
   ✔  When I set a title on a new object with fsquid
   ✔  Then that connection should see the "cli.metadata.set" entry
  Scenario: A command line that names no module command is journaled as a failed route
   ✔> Given a fresh FileSquid data directory
   ✔  When I run the fsquid binary with "validate dcterms:title"
   ✔  Then the journal should hold one "cli.route" entry that "failed" with "module_required"
  Scenario: A journal that cannot be opened refuses the command
   ✔> Given a fresh FileSquid data directory
   ✔  Given a data directory that cannot hold a journal
   ✔  When I set a title on a new object with fsquid
   ✔  Then fsquid should refuse with one journal_unavailable line naming "cli.metadata.set"
   ✔  And no object should have been written
Feature: One command-line tool named fsquid
  Scenario: A command that only one module defines runs without the module name
   ✔  When I run fsquid with "get dcterms:title"
   ✔  Then the metadata module should handle it
  Scenario: Any command also runs with its module name
   ✔  When I run fsquid with "metadata get dcterms:title"
   ✔  Then the metadata module should handle it
  Scenario: validate always needs the module name
   ✔  When I run fsquid with "validate dcterms:title"
   ✔  Then fsquid should refuse and suggest "fsquid metadata validate"
  Scenario: validate runs with the module name
   ✔  When I run fsquid with "metadata validate dcterms:title"
   ✔  Then the command should succeed
  Scenario: A module name alone with help lists its commands
   ✔  When I run fsquid with "metadata help"
   ✔  Then the metadata module should handle it
   ✔  And the command should succeed
   ✔  And the routed help should list extract, get, set, export, import, and validate
  Scenario: An unknown command is reported
   ✔  When I run fsquid with "mods"
   ✔  Then fsquid should report an unknown command
  Scenario: fsquid is the only CLI binary
   ✔  When I list the CLI binaries the workspace builds
   ✔  Then the only CLI binary should be fsquid
[Summary]
2 features
13 scenarios (13 passed)
42 steps (42 passed)
```

`cargo test -p filesquid-desktop --locked --test integration_native_boundary`:

```text
test the_linking_rules_reject_a_sqlite_driver_in_a_host_and_native_in_the_api ... ok
test the_source_rule_rejects_storage_opens_and_worker_loops ... ok
test the_desktop_host_holds_no_native_storage_or_worker_code ... ok
test only_the_hosts_and_the_cli_link_filesquid_native_and_the_desktop_has_no_sqlite_driver ... ok
test the_cli_links_filesquid_native_with_default_features_only ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
```
