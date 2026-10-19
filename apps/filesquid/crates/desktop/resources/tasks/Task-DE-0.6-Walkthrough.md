# Task DE-0.6 Walkthrough: Stdout JSON logging

## DE-S-003: Desktop logs are stdout JSON and every host-level refusal is journaled

The desktop already wrote `tracing` JSON to stdout (DE-0.1, DE-0.9). This task pins and proves the stream's contract and closes one silent failure.

### SRS mapping

| Requirement | Title | Covered by |
|---|---|---|
| DE-S-003 | Logs are stdout JSON (one JSON object per line, RFC 3339 UTC millisecond timestamp, level, target, fields; `log` records joined; no log file or Tauri log plugin; install failure reported) | `every_event_and_log_record_is_one_json_line_and_a_second_install_is_refused`, `the_timestamp_format_is_rfc3339_utc_with_milliseconds`, `the_tauri_log_plugin_is_refused_so_no_log_file_target_can_exist` |
| NFR-009 | Observability and capacity economics: structured, unbuffered, redacted stdout events with UTC RFC 3339 time, severity and outcome; no cloud telemetry or observability cluster for local use | the same tests; no telemetry server is added |
| ADR 014 | Stdout observability: JSON objects, RFC 3339 UTC timestamps, no log files | the same tests; `scripts/verify_centralized_logging.sh` |

## What changed

- [`src/stdout_log.rs`](../../src/stdout_log.rs) (new): `UtcMillis`, the stdout layer's timer, writes RFC 3339 UTC with milliseconds through the API's `format_rfc3339_ms`, the activity journal's own time format, so a stdout line and its journal entry carry the same kind of time. `is_rfc3339_utc_ms` checks that exact form.
- [`src/lib.rs`](../../src/lib.rs): the stdout layer uses `UtcMillis`. `install_tracing` (public, used by the host's launch) installs the stack globally, which also routes `log` records (Tauri's own) into it, and answers an error instead of the former `.try_init().ok()`. When installation fails, the host writes a redacted `journal_unavailable` line for `desktop.logging` (error code `logging_unavailable`) to stdout and the detail to stderr, and keeps running.
- Tests: [`tests/integration/stdout_logging.rs`](../../tests/integration/stdout_logging.rs) (new; one global install per test binary) and a refusal test for `tauri-plugin-log` in [`tests/integration/capabilities.rs`](../../tests/integration/capabilities.rs). `log` is a dev-dependency only, to emit a record through the facade Tauri uses.
- DE-S-003 states the stream's contract; README `## View logs` names the desktop's timestamp format and the test command.

TDD: the new tests were written first and failed to compile (`unresolved import filesquid_desktop_lib::install_tracing`, `unresolved import filesquid_desktop_lib::stdout_log`).

## Test output

`cargo test -p filesquid-desktop --locked --test integration_stdout_logging --test integration_capabilities`, from `apps/filesquid/`:

```text
    Finished `test` profile [unoptimized] target(s) in 0.47s
     Running tests/integration/capabilities.rs (/home/user/.cargo-target-shared/debug/deps/integration_capabilities-1cb9f81612a65426)
running 55 tests
test a_capability_for_webview_labels_is_refused ... ok
test a_capability_for_remote_pages_is_refused ... ok
test a_capability_without_a_permission_list_is_malformed ... ok
test a_capability_for_every_window_is_refused ... ok
test a_conditional_tauri_command_attribute_is_refused ... ok
test a_configuration_that_enables_another_capability_is_refused ... ok
test a_cli_configuration_that_adds_a_capability_is_refused ... ok
test a_documented_mention_of_the_attribute_is_not_a_command ... ok
test a_configuration_with_no_capability_list_is_refused ... ok
test a_file_system_plugin_dependency_is_refused ... ok
test a_linked_file_system_or_sql_plugin_is_refused_however_it_is_declared ... ok
test a_configuration_file_the_policy_cannot_read_is_refused ... ok
test a_malformed_permission_entry_is_refused ... ok
test a_file_system_permission_is_refused_as_a_forbidden_plugin ... ok
test a_missing_app_acl_is_refused ... ok
test a_permission_that_returns_filesystem_paths_is_refused ... ok
test a_plugin_declared_as_a_dependency_table_is_refused ... ok
test a_multi_line_tauri_command_attribute_is_refused ... ok
test a_removed_app_acl_fails_the_policy ... ok
test a_second_configured_window_is_refused ... ok
test a_renamed_plugin_dependency_is_refused ... ok
test a_scoped_file_system_permission_object_is_refused ... ok
test a_tauri_command_in_lib_rs_is_refused ... ok
test a_window_list_that_is_not_a_list_of_strings_is_refused ... ok
test an_app_acl_that_allows_a_command_is_refused ... ok
test an_app_acl_manifest_makes_tauri_check_every_custom_command ... ok
test an_app_acl_without_a_permission_is_refused ... ok
test an_imported_command_attribute_is_refused ... ok
test an_inline_capability_in_the_configuration_is_refused ... ok
test a_platform_configuration_that_widens_the_window_is_refused ... ok
test an_overlay_merges_the_way_tauri_merges_it ... ok
test an_unlisted_capability_identifier_is_refused ... ok
test an_overlay_that_widens_the_configuration_is_refused ... ok
test an_unlisted_tauri_command_in_a_submodule_is_refused ... ok
test core_plugins_and_other_variables_are_not_linked_plugins ... ok
test emitting_events_from_the_webview_is_not_granted ... ok
test file_shell_sql_and_dialog_plugins_are_forbidden ... ok
test every_violation_names_its_fix ... ok
test every_shipped_capability_file_passes_the_policy ... ok
test sql_shell_and_dialog_permissions_are_refused ... ok
test the_asset_protocol_feature_is_refused_in_the_manifest ... ok
test the_asset_protocol_is_refused_in_the_configuration ... ok
test the_global_tauri_object_is_refused ... ok
test the_policy_defines_no_app_command ... ok
test the_policy_grants_event_listening_on_the_main_window_only ... ok
test the_shipped_capability_grants_exactly_the_policy ... ok
test the_shipped_manifest_has_no_forbidden_tauri_plugin ... ok
test the_shipped_tauri_configuration_passes_the_policy ... ok
test the_tauri_log_plugin_is_refused_so_no_log_file_target_can_exist ... ok
test the_test_driver_plugin_is_linked_only_in_test_builds ... ok
test the_test_driver_plugin_must_stay_optional ... ok
test the_tests_see_the_plugins_the_build_links ... ok
test the_image_squid_style_overlay_passes ... ok
test the_shipped_sources_define_no_unlisted_tauri_command ... ok
test the_shipped_crate_passes_the_whole_policy_the_build_enforces ... ok
test result: ok. 55 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/stdout_logging.rs (/home/user/.cargo-target-shared/debug/deps/integration_stdout_logging-91e8e4ecbca17631)
running 4 tests
test a_flushing_writer_flushes_every_record_it_writes ... ok
test a_logging_stack_that_cannot_be_installed_is_reported_on_stdout_and_to_the_journal ... ok
test the_timestamp_format_is_rfc3339_utc_with_milliseconds ... ok
test every_event_and_log_record_is_one_json_line_and_a_second_install_is_refused ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Review fixes (Cubic round 1)

- Every stdout record is flushed when written: the stdout layer writes through `stdout_log::Flushing` (writers flush on drop), the IPC observer's line sink and the `journal_unavailable` writer flush after each line, because Rust promises line buffering only when stdout is a terminal (`a_flushing_writer_flushes_every_record_it_writes`).
- A logging stack that cannot be installed is reported by `report_logging_failure` (tested) and, once the journal opens, journaled as `desktop.logging` / `logging_unavailable` (`logging_failure_entry`). The loss it causes (no `tracing` events for that launch) is documented on `init_tracing`.
- README `## View logs` separates the `tracing`/`log` record shape from the journal-entry shape of refusal and `journal_unavailable` lines.
- The framing test requires a trailing newline and no empty line.

## Review fixes (Cubic round 2)

- The API journal writer's fallback (entries the journal store rejected, marked `journal_write_failed`, after one `journal write failed` error line) writes and flushes each line through one helper instead of `println!`; README `## View logs` documents that shape.

## Mocked Environment Variables

None.
