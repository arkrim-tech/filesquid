# Task DE-0.1 Walkthrough: Capability expansion (deny-by-default)

Module task [DE-0.1](../plan.md) of global [Task 0.5](../../../../../../resources/implementation-plan.md) (DE-0.2 to DE-0.6 and DE-0.8 remain, so the global task stays open). Branch `feat/de-0.1-capability-expansion`.

## DE-S-002: Capabilities are deny-by-default; no fs/shell/sql plugins at scaffold

> Capabilities are deny-by-default; no fs/shell/sql plugins at scaffold. The webview's Tauri capabilities MUST grant the main window only `core:event:allow-listen` and `core:event:allow-unlisten`; they MUST NOT grant any permission of the fs, shell, sql, dialog, http, process or opener plugins, `core:path` (which returns filesystem paths), the asset protocol, remote pages, webview labels or windows other than `main`. Filesystem and SQLite access stay in the Rust host behind the API authoriser. The host MUST declare an app ACL manifest so Tauri refuses every custom command that no capability grants, and the build MUST fail when a capability file, the effective Tauri configuration (base, platform file and CLI overlay merged; it MUST create no window other than `main`), the crate manifest, a linked non-core Tauri plugin (outside test-instrumented builds' WebDriver plugin), the app ACL file or a `#[tauri::command]` breaks this policy.

Everything after the first sentence was added to [DE-S-002](../srs.md) in this task. Parents: [F-037 Setup, updates and diagnostics](../../../../../../resources/prd.md#f-037-setup-updates-and-diagnostics), [SRS-F-037 Setup, updates and diagnostics](../../../../../../resources/srs.md#srs-f-037-setup-updates-and-diagnostics), [ADR 002](../../../../../../resources/architecture/002-tauri2-desktop-shell.md).

### Secondary SRS mapping

- **DE-S-001: The desktop serves the FileSquid API in-process through a Tauri custom URI scheme with no TCP listener (the API health route replaces `health_ping`). Any remaining Tauri command MUST be typed and explicitly authorized by a deny-by-default Tauri capability.** Before this task Tauri ran the app's own commands for local pages without any capability check, because the app had no ACL manifest (`tauri::webview` checks app commands only when `has_app_acl_manifest`). `permissions/app-acl.toml` now declares one, so an unlisted command is refused by the ACL: in the packaged window `health_ping` went from `Command health_ping not found` (red) to a `not allowed` refusal (green). No command exists, and none may be defined in `lib.rs`. No TCP listener was added.
- **DE-S-006: The webview MUST receive opaque handles and API responses, never raw filesystem paths or SQL.** `core:path` is not granted (build-time check and packaged-window test, which also asserts no path appears in the refusal), and no fs or sql plugin can be granted or linked.
- **DE-S-003: Logs are stdout JSON.** Unchanged; no logging was added.

## Reading of the task row against ADR 030

The row predates [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md). Read literally, "add scoped filesystem and SQL capabilities as explicit permissions" would grant the webview fs and SQL Tauri permissions, which ADR 030 (no per-feature IPC), [DE-S-006](../srs.md) and [F-037](../../../../../../resources/prd.md#f-037-setup-updates-and-diagnostics) ("Native file access, workers and OS adapters MUST stay out of the untrusted webview") forbid. This task therefore keeps scoped filesystem and SQLite access in the Rust host (behind the API authoriser, in `filesquid-native` under the app data directory) and makes the webview's capability set explicit and machine-checked. The reading is recorded in the [plan row](../plan.md), [ADR 002](../../../../../../resources/architecture/002-tauri2-desktop-shell.md) and [DE-S-002](../srs.md); the owner may revise it.

## What changed

- [`src/capability_policy.rs`](../../src/capability_policy.rs) (new, pure): the policy constants (`APP_WINDOWS`, `ENABLED_CAPABILITIES`, `GRANTED_PERMISSIONS`, `FORBIDDEN_PLUGINS`, `APP_COMMANDS`, `TEST_ONLY_PLUGIN_CRATES`) plus checks that return `Violation`s: `check_capability`, `check_app_security` (which includes the windows the configuration creates), `check_plugin_dependencies` (which includes renamed and table-form dependencies), `check_linked_plugins`, `check_app_acl` and `check_command_placement` (which covers `cfg_attr`, multi-line and imported `#[command]` forms). It also has `merge_config` (JSON Merge Patch, as Tauri merges overlays). Each `Violation` displays its fix. `check_linked_plugins` reads the `DEP_<CRATE>_PERMISSION_FILES_PATH` variables that every Tauri plugin publishes to dependent build scripts; `tauri-build` finds plugins the same way. A linked plugin is therefore caught however the manifest declares it.
- [`src/capability_inputs.rs`](../../src/capability_inputs.rs) (new, I/O, not a library module): `build.rs` and the tests both include it, so they read the same inputs. It reads every file under `capabilities/` and `src/` (subdirectories included, at most eight levels) and builds the effective configuration: `tauri.conf.json`, then `tauri.<platform>.conf.json`, then the CLI's `TAURI_CONFIG`. It refuses JSON5 and TOML configuration files it cannot read, and also reads `Cargo.toml` and `permissions/app-acl.toml`.
- [`build.rs`](../../build.rs): runs the reader and panics with the list of violations, then calls `tauri_build::try_build` with `AppManifest::new().commands(APP_COMMANDS)`. It reruns when the platform file or `TAURI_CONFIG` changes. It also hands the linked-plugin keys to the tests (`FILESQUID_TAURI_PLUGIN_KEYS`), so the tests check the same plugin set. `serde_json` is a build dependency (already in `Cargo.lock`).
- [`permissions/app-acl.toml`](../../permissions/app-acl.toml) (new): the permission `no-app-commands`, which allows nothing. `tauri-build` creates the app ACL manifest only when the app has a permission, and this permission ensures there is one while `APP_COMMANDS` is empty.
- `capabilities/default.json` and `tauri.conf.json` are unchanged: they already matched the policy, which now pins them.
- Docs: [DE-S-002](../srs.md), [ADR 002](../../../../../../resources/architecture/002-tauri2-desktop-shell.md) amendment, [plan row](../plan.md), [SRS-F-037](../../../../../../resources/srs.md#srs-f-037-setup-updates-and-diagnostics) tests and traceability table, root [README](../../../../../../README.md#run-tests).

## Constraint gates

| Constraint | Test |
|---|---|
| Main window granted exactly event listen and unlisten | `the_policy_grants_event_listening_on_the_main_window_only`, `the_shipped_capability_grants_exactly_the_policy`, scenario "The main window may listen for FileSquid events and nothing else", e2e "lets the app page listen for and stop listening for FileSquid events" |
| No fs, shell, sql, dialog, http, process, opener permission | `file_shell_sql_and_dialog_plugins_are_forbidden`, `a_file_system_permission_is_refused_as_a_forbidden_plugin`, `sql_shell_and_dialog_permissions_are_refused`, `a_scoped_file_system_permission_object_is_refused`, scenario outline (4 examples), e2e refusals of `plugin:fs`, `plugin:sql`, `plugin:shell`, `plugin:dialog`, `plugin:http` |
| No such plugin crate linked; test driver optional and test builds only | `the_shipped_manifest_has_no_forbidden_tauri_plugin`, `a_file_system_plugin_dependency_is_refused`, `a_plugin_declared_as_a_dependency_table_is_refused`, `the_test_driver_plugin_must_stay_optional`, `core_plugins_and_other_variables_are_not_linked_plugins`, `a_linked_file_system_or_sql_plugin_is_refused_however_it_is_declared`, `the_test_driver_plugin_is_linked_only_in_test_builds`, and the manual build below (with `test_build` forced to false, the instrumented build fails naming `tauri-plugin-wdio-webdriver`) |
| Malformed capability input fails closed | `a_capability_without_a_permission_list_is_malformed`, `a_malformed_permission_entry_is_refused`, `a_window_list_that_is_not_a_list_of_strings_is_refused`; a capability file that is not JSON makes `build.rs` stop as unreadable |
| No `core:path` (raw paths) | `a_permission_that_returns_filesystem_paths_is_refused`, scenario "A capability that would hand the window filesystem paths is refused", e2e "refuses plugin:path\|resolve_directory" and "never hands the app page a filesystem path" |
| Event emit and other core commands not granted | `emitting_events_from_the_webview_is_not_granted`, e2e refusals of `plugin:event\|emit`, `plugin:window\|title`, `plugin:webview\|get_all_webviews`, `plugin:app\|version` |
| Explicit per-window allowlist: no wildcard, no webview labels | `a_capability_for_every_window_is_refused`, `a_capability_for_webview_labels_is_refused`, scenario "A capability for every window is refused" |
| Remote pages inherit no IPC | `a_capability_for_remote_pages_is_refused`, scenario "A capability for remote pages is refused" (navigation off app origins stays refused by DE-F1.1's guard) |
| Only the `default` capability enabled; none inline | `the_shipped_tauri_configuration_passes_the_policy`, `a_configuration_that_enables_another_capability_is_refused`, `an_inline_capability_in_the_configuration_is_refused`, `a_configuration_with_no_capability_list_is_refused`, `an_unlisted_capability_identifier_is_refused` |
| No global Tauri object, no asset protocol | `the_global_tauri_object_is_refused`, `the_asset_protocol_is_refused_in_the_configuration`, `the_asset_protocol_feature_is_refused_in_the_manifest` |
| Unlisted custom command refused by Tauri | `an_app_acl_manifest_makes_tauri_check_every_custom_command`, `the_policy_defines_no_app_command`, e2e "refuses health_ping" |
| `#[tauri::command]` never in `lib.rs`; listed when elsewhere | `a_tauri_command_in_lib_rs_is_refused`, `an_unlisted_tauri_command_in_a_submodule_is_refused`, `a_documented_mention_of_the_attribute_is_not_a_command`, `the_shipped_sources_define_no_unlisted_tauri_command`, two scenarios |
| Build fails on a widened capability | `every_shipped_capability_file_passes_the_policy`, `every_violation_names_its_fix`, `the_shipped_crate_passes_the_whole_policy_the_build_enforces`, scenario "The shipped capability set passes the deny-by-default policy" (both use the build's reader), and the manual build below |
| Effective configuration checked: platform file and CLI overlay merged, no window but `main` | `a_second_configured_window_is_refused`, `an_overlay_merges_the_way_tauri_merges_it`, `an_overlay_that_widens_the_configuration_is_refused`, `a_platform_configuration_that_widens_the_window_is_refused`, `a_cli_configuration_that_adds_a_capability_is_refused`, `the_image_squid_style_overlay_passes`, `a_configuration_file_the_policy_cannot_read_is_refused` |
| App ACL present and allowing nothing | `an_app_acl_manifest_makes_tauri_check_every_custom_command`, `a_missing_app_acl_is_refused`, `an_app_acl_without_a_permission_is_refused`, `an_app_acl_that_allows_a_command_is_refused`, `a_removed_app_acl_fails_the_policy` |
| Linked plugins seen by the tests as by the build | `the_tests_see_the_plugins_the_build_links` |
| Every command form detected | `a_renamed_plugin_dependency_is_refused`, `a_conditional_tauri_command_attribute_is_refused`, `a_multi_line_tauri_command_attribute_is_refused`, `an_imported_command_attribute_is_refused` |

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no coverage percentage is reported. Every public item of `capability_policy` is exercised by the tests above.

## Known limitation

Tauri offers no hook on its access-control refusals: a refused IPC command is answered to the page but cannot be journaled by the host. The page holds only event listen and unlisten, and every FileSquid operation goes through the API, whose refusals are journaled (`api.auth.denied`). Recorded in the [ADR 002](../../../../../../resources/architecture/002-tauri2-desktop-shell.md) amendment.

## Red: tests before the implementation

`cargo test -p filesquid-desktop --features test-webdriver --locked` (unique errors, counted):

```text
      1 error: could not compile `filesquid-desktop` (test "integration_capabilities") due to 1 previous error
      1 error[E0432]: unresolved import `filesquid_desktop_lib::capability_policy`
```

Packaged window, before `permissions/app-acl.toml` (`xvfb-run -a npx wdio run ./wdio.conf.mjs --spec ./specs/capabilities.e2e.mjs`):

```text
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:http|fetch
[WebKitGTK 605.1.15 linux #0-0]    ✖ refuses health_ping
[WebKitGTK 605.1.15 linux #0-0]    ✓ never hands the app page a filesystem path
[WebKitGTK 605.1.15 linux #0-0] 12 passing (1.1s)
[WebKitGTK 605.1.15 linux #0-0] 1 failing
Expected pattern: /not allowed/
Received string:  "WebDriverError: Command health_ping not found when running \"execute/sync\" with method \"POST\""
Spec Files:	 0 passed, 1 failed, 1 total (100% completed) in 00:00:03
```

## Green: test output

From `apps/filesquid/`: `cargo fmt --all` (no change), `cargo clippy --workspace --all-targets --locked -- -D warnings` (exit 0), `cargo clippy -p filesquid-desktop --all-targets --features test-webdriver --locked -- -D warnings` (exit 0), `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` (exit 0).

`cargo test -p filesquid-desktop --features test-webdriver --locked`:

```text
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_desktop_lib-0925f25d86780962)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (<target>/debug/deps/filesquid_desktop-cba6b2bc859dc100)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/features.rs (<target>/debug/deps/features-425cfc04506b2be6)
Feature: Desktop capabilities are deny-by-default
  Scenario: The shipped capability set passes the deny-by-default policy
  Scenario: The main window may listen for FileSquid events and nothing else
  Scenario Outline: A capability that grants file, SQL, program or dialog access is refused
  Scenario Outline: A capability that grants file, SQL, program or dialog access is refused
  Scenario Outline: A capability that grants file, SQL, program or dialog access is refused
  Scenario Outline: A capability that grants file, SQL, program or dialog access is refused
  Scenario: A capability that would hand the window filesystem paths is refused
  Scenario: A capability for every window is refused
  Scenario: A capability for remote pages is refused
  Scenario: A Tauri command defined next to the handler in lib.rs is refused
  Scenario: A Tauri command that no capability authorizes is refused
Feature: Desktop transport: per-launch token, event channel and ranged reads
20 scenarios (20 passed)
74 steps (74 passed)
     Running tests/integration/api_protocol.rs (<target>/debug/deps/integration_api_protocol-0df96af0e7868500)
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/integration/capabilities.rs (<target>/debug/deps/integration_capabilities-9ca5b31c8da030ff)
test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/event_channel.rs (<target>/debug/deps/integration_event_channel-a7f2299e8a8c2ee7)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/integration/range_fixture.rs (<target>/debug/deps/integration_range_fixture-562c0ef8cd0a473d)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.95s
```

`cargo test -p filesquid-desktop --locked --test integration_capabilities` (tail):

```text
test result: ok. 54 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Packaged window (`cargo build -p filesquid-desktop --features test-webdriver --locked`, then `xvfb-run -a npm run test:e2e` in `tests/e2e`):

```text
[WebKitGTK 605.1.15 linux #0-0] » specs/capabilities.e2e.mjs
[WebKitGTK 605.1.15 linux #0-0]    ✓ lets the app page listen for and stop listening for FileSquid events
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:event|emit
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:path|resolve_directory
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:window|title
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:webview|get_all_webviews
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:app|version
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:fs|read_text_file
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:sql|load
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:shell|execute
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:dialog|open
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses plugin:http|fetch
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses health_ping
[WebKitGTK 605.1.15 linux #0-0]    ✓ never hands the app page a filesystem path
[WebKitGTK 605.1.15 linux #0-0] 13 passing (578ms)
[WebKitGTK 605.1.15 linux #0-1] » specs/transport.e2e.mjs
[WebKitGTK 605.1.15 linux #0-1]    ✓ gives the app page a 64-hex per-launch token
[WebKitGTK 605.1.15 linux #0-1]    ✓ answers API calls with the token and refuses them without it
[WebKitGTK 605.1.15 linux #0-1]    ✓ reads the last bytes of a 100 MB file as partial content
[WebKitGTK 605.1.15 linux #0-1]    ✓ reads the whole 100 MB file through capped ranges
[WebKitGTK 605.1.15 linux #0-1]    ✓ pushes events to the app window on the event channel
[WebKitGTK 605.1.15 linux #0-1]    ✓ keeps the app window on the app's own origin
[WebKitGTK 605.1.15 linux #0-1] 6 passing (5.6s)
Spec Files:	 2 passed, 2 total (100% completed) in 00:00:10
```

The build refuses a widened capability (`fs:default` and `core:path:default` added to `capabilities/default.json`, then reverted; `cargo build -p filesquid-desktop --locked`):

```text
error: failed to run custom build command for `filesquid-desktop v0.1.0 (<repo>/apps/filesquid/crates/desktop)`
  the desktop capability policy is violated (src/capability_policy.rs):
    - permission "fs:default" belongs to a forbidden plugin; file, SQL and program access stay in the Rust host
    - permission "core:path:default" is not granted by the policy; the API serves this through the filesquid:// scheme
```

The linked-plugin check sees the real build: with `test_build` forced to `false` in `build.rs` (then reverted), `cargo build -p filesquid-desktop --features test-webdriver --locked` stops with:

```text
  the desktop capability policy is violated (src/capability_policy.rs):
    - the build links the Tauri plugin "tauri-plugin-wdio-webdriver"; only core plugins (and ["tauri-plugin-wdio-webdriver"] in test builds) may be linked
```

## Pre-commit

`FULL=1 FILESQUID_DESKTOP_E2E=1 bash scripts/pre-commit.sh` from the repository root:

```text
✅ Desktop packaged-window e2e passed (11s).
✅ All Pre-Commit Hooks Passed! Test scope: full (153s)
```

## Mocked Environment Variables

None.
