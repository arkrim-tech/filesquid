# Task DE-0.9 Walkthrough: Journal every refusal in the desktop shell

Owner-requested fix (2026-10-18), follow-up to [DE-0.1](../plan.md) (PR #47). Module task [DE-0.9](../plan.md) of global [Task 0.5](../../../../../../resources/implementation-plan.md) (the global task stays open). Branch `fix/de-ipc-refusal-journal`.

## DE-S-003: Logs are stdout JSON

> Logs are stdout JSON. Every host-level refusal MUST be journaled: each IPC call the host or Tauri refuses (including calls on Tauri's `postMessage` path and invoke-key mismatches, which the host reads from Tauri's own trace), each refused navigation, new window, permission request and download, each IPC call that fails, the opening and closing of each app window, a window that cannot be built, and a panic on any host thread (flushed to the journal before the panic continues). Entries MUST name an operation, outcome, error code, the actor `webview:<label>` where a page acted, and at most an opaque target (a command id such as `plugin:fs.read_text_file`, a window label or a permission kind); they MUST NOT hold a payload, address, file name, path, panic message, invoke key or token, and Tauri's message that names the invoke key MUST NOT reach stdout.

Everything after the first sentence was added to [DE-S-003](../srs.md) in this task. Parents: [F-061 Activity journal and live health monitoring](../../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring), [SRS-F-061 Activity journal and live health monitoring](../../../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring), [ADR 014](../../../../../../resources/architecture/014-stdout-observability.md), [ADR 033](../../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md).

### Secondary SRS mapping

- **DE-S-002: Capabilities are deny-by-default; no fs/shell/sql plugins at scaffold.** Extended in this task: the host registers Tauri's `ipc` URI scheme itself and refuses, before Tauri sees it, every IPC call whose method is not `POST`, whose page origin is not an app origin, whose invoke key is wrong or missing, or whose command is not `plugin:event|listen` or `plugin:event|unlisten`; it hands allowed calls to Tauri's own access control and journals Tauri's refusal of one, and a call Tauri never answers. It builds its windows itself and refuses every new-window request, every permission request and every download other than an app page's `blob:` URL. The capability policy, the app ACL manifest and the `build.rs` check are unchanged and still hold (`integration_capabilities`, 54 tests; scenario "The shipped capability set passes the deny-by-default policy").
- **DE-S-005: Every API request through the scheme MUST carry a per-launch token …** Unchanged. Navigation refusals stay journaled (`desktop.navigation.refused`) and now carry `outcome: failed` like every other refusal.
- **DE-S-006: The webview MUST receive opaque handles and API responses, never raw filesystem paths or SQL.** Unchanged (owner decision of 2026-10-18: file and SQL access stay in the Rust host). The refusal answer names only the command the page sent.

## Investigation and chosen approach

Read in the pinned sources (`tauri` 2.12.4, `tauri-runtime-wry` 2.12.2, `wry` 0.57.0, `tauri-plugin-wdio-webdriver` 1.5):

| Option | Finding | Used |
|---|---|---|
| Hook on Tauri's access-control refusal | None. `Webview::on_message` refuses through the responder it is given; plugins see only calls that passed | no |
| Custom invoke handler (`invoke_handler`) | Runs only after access control passes | no |
| Own the `ipc` URI scheme | `manager/webview.rs` registers Tauri's handler only `if !registered_scheme_protocols.contains("ipc")`; `Webview::on_message`, `InvokeRequest`, `InvokeResponse`, `CallbackFn` and `AppHandle::invoke_key` are public | **yes**: `src/ipc_gate.rs` |
| `postMessage` path | `pending.ipc_handler` is set on every webview and cannot be replaced; `ipc-protocol.js` falls back to it when `fetch` fails, and a page script can call `window.ipc.postMessage` directly | covered by the observer |
| Tauri's `tracing` feature | Spans `ipc::request::handle` (`cmd`) and `ipc::request::response` (`response = InvokeError(...)` on refusal) on the `postMessage` path; the invoke-key check emits an ERROR on `tauri::webview` naming the key; an unreadable message emits `ipc.request.error` | **yes**: `src/ipc_observer.rs` |
| `invoke_system` (replace Tauri's IPC script) | Would remove the `postMessage` fallback, but vendors Tauri's JavaScript and still leaves `window.ipc.postMessage` | no |
| New windows, permissions, downloads | No handler on config-created windows; `WebviewWindowBuilder` offers `on_new_window`, `on_permission_request`, `on_download` | **yes**: `src/app_window.rs` builds `main` |
| Restructure so the webview has no Tauri IPC | The UI listens for events through `@tauri-apps/api` (UI-engineer scope), and the `postMessage` handler exists regardless | no |

No option needs a Tauri patch. The `ipc` gate uses public API and is exact; the observer covers what the gate cannot see. Both depend on Tauri internals that a Tauri upgrade could change (the `ipc` registration rule; the span, field and message names), and both are pinned by tests: the gate and the invoke-key event by integration tests on the real Tauri code (mock runtime), the `postMessage` spans by the packaged-window test that forces Tauri's fallback.

## What changed

- [`src/ipc_gate.rs`](../../src/ipc_gate.rs) (new): pure `decide` (method, origin, invoke key compared in constant time, callback headers, content type, allowlist `ALLOWED_COMMANDS`), `command_id` (opaque journal id), `refusal_response`, and the I/O `handle`/`forward` with a `ReplyGuard` that journals Tauri's error answers (`acl_denied`, or `desktop.ipc.failed`) and answers a dropped call (`no_response`). Answers keep Tauri's wire format.
- [`src/ipc_observer.rs`](../../src/ipc_observer.rs) (new): the `TauriIpcObserver` layer and the filters `observed` / `outside_observed`; the stdout and journal layers skip Tauri's IPC trace and webview errors.
- [`src/app_window.rs`](../../src/app_window.rs) (new): `host_built_windows` (every configured window `create: false`, overlays included), `create_main_window` with the new-window, permission and download guards, lifecycle journaling.
- [`src/panic_journal.rs`](../../src/panic_journal.rs) (new): process-wide panic hook that journals `desktop.panic` and flushes the journal.
- [`src/lib.rs`](../../src/lib.rs): `host_subscriber` (the tracing stack, testable with any writer), the `ipc` scheme registration, the panic hook, `HostJournal` (attaches observer and panic flush when the journal opens), `open_main_window` (journals and flushes on failure), `run` marks windows host-built. [`src/event_channel.rs`](../../src/event_channel.rs): navigation refusals carry `outcome: failed`.
- `Cargo.toml`: `tauri` gains the `tracing` feature; dev-dependency `tauri` with `test` (mock runtime). `Cargo.lock` gains three `tracing` edges, no new package.
- Docs: [ADR 002](../../../../../../resources/architecture/002-tauri2-desktop-shell.md) dated amendment replacing the gap note, [DE-S-002 and DE-S-003](../srs.md), root [SRS](../../../../../../resources/srs.md) (SRS-F-037, SRS-F-061 tests and traceability table), [plan row](../plan.md), [implementation plan](../../../../../../resources/implementation-plan.md) Task 0.5, [checklist](../../../../../../resources/implementation-plan-checklist.md) and MVP progress, root [README](../../../../../../README.md#view-logs).

## New journal event types and their redaction tests

| Operation | Codes | Redaction test |
|---|---|---|
| `desktop.ipc.refused` (gate) | `command_not_allowed`, `invalid_invoke_key`, `foreign_origin`, `malformed_request`, `method_not_allowed`, `unknown_webview` | `a_refused_command_is_journaled_without_its_payload_or_the_invoke_key`, `a_wrong_invoke_key_is_journaled_without_either_key`, `a_call_from_a_foreign_origin_is_journaled_without_the_address`, e2e "journals a command the host's IPC gate refuses, without its payload" |
| `desktop.ipc.refused` (Tauri via gate) | `acl_denied`, `no_response` | `tauris_own_access_control_refusal_of_a_forwarded_call_is_journaled`, `a_call_tauri_never_answers_is_journaled_and_answered` |
| `desktop.ipc.refused` (observer) | `acl_denied`, `invalid_invoke_key`, `malformed_request` | `a_refusal_tauri_reports_on_its_post_message_path_is_journaled_with_the_command`, `tauris_invoke_key_refusal_is_journaled_once_and_the_key_stays_out_of_the_logs`, `an_unreadable_post_message_call_is_journaled_without_its_text`, e2e postMessage cases |
| `desktop.ipc.failed` | `command_failed` | same redaction path as `acl_denied` (`journal_tauri_error`); no reachable trigger while only event commands are forwarded |
| `tauri::webview.error` | `webview_error` | `other_tauri_webview_errors_are_journaled_without_their_text` |
| `desktop.window.refused` | `new_window_denied` | `a_new_window_is_refused_and_journaled_without_its_address`, e2e "refuses and journals a new window" |
| `desktop.permission.refused` | `permission_denied` (target: kind) | `a_permission_request_is_refused_and_journaled_by_kind_only` |
| `desktop.download`, `desktop.download.refused` | `download_denied` | `a_download_is_journaled_without_its_address_or_file_name` |
| `desktop.window.opened`, `desktop.window.closed`, `desktop.window.create` | `window_create_failed` | `the_main_window_is_built_by_the_host_and_its_opening_and_closing_journaled`, `a_main_window_that_cannot_be_built_is_journaled` |
| `desktop.panic` | `panic` | `a_panic_is_journaled_without_its_message`, `the_panic_hook_flushes_the_journal_it_is_given`, `a_panic_on_the_journal_writer_thread_is_never_queued_and_reaches_stdout_redacted` |

## Desktop host audit (events that bypassed the journal)

| Event | Before | Now |
|---|---|---|
| IPC refused by Tauri's access control | page only | journaled (`acl_denied`) |
| IPC with a wrong invoke key | `eprintln!` of both keys to stderr | journaled; key kept off stdout and the journal |
| `postMessage` call Tauri cannot parse | `console.error` in the page | journaled (`malformed_request`) |
| New window, permission request, download | platform default, not journaled | refused (downloads: app `blob:` allowed) and journaled |
| Window open/close, window build failure | not journaled | journaled; the journal is flushed before exit on failure |
| Panic on a host thread | stderr only | journaled and flushed |
| Tauri's own `log::` records | already journaled (tracing-subscriber's `tracing-log` installs `LogTracer`) as `<target>.warn/error`, no text | unchanged |
| Host start/stop, event forwarder, monitor tick, scheme host failures | already journaled | unchanged |

Remaining, out of scope for this branch: a failure before the journal opens (the per-launch token cannot be generated, the data directory or journal cannot be opened, Tauri cannot build the app before `setup`) can only go to stdout, as the API's own pre-monitor refusals do; a panic on the journal writer's own thread is printed to stdout as a JSON line marked `journal_unavailable` and not queued (that thread is the queue's only consumer); refusals read from Tauri's trace carry the actor `webview` because Tauri's trace does not name the webview; the previous panic hook still prints the panic message to stderr.

## Constraint gates

| Constraint | Test |
|---|---|
| Allowlist equals `GRANTED_PERMISSIONS` | `the_ipc_allowlist_is_exactly_the_granted_event_permissions` |
| Refused before Tauri sees it | `every_refusal_reason_is_decided_before_tauri_sees_the_call`, scenario outline (4 commands) |
| Allowed calls still work | `an_allowed_call_with_the_invoke_key_is_forwarded_to_tauri`, scenario "An app page may listen …", e2e "still lets the app page listen for FileSquid events", `capabilities.e2e.mjs` and `transport.e2e.mjs` (event channel) |
| Page told "not allowed" in Tauri's format | `a_refusal_is_answered_in_tauris_wire_format_and_says_not_allowed`, the 11 refusals in `capabilities.e2e.mjs` |
| Opaque, distinct codes; opaque command ids | `refusal_codes_are_opaque_and_distinct`, `a_command_name_becomes_an_opaque_journal_id_or_unrecognised` |
| Ordinary events still reach stdout and the journal | `ordinary_host_events_still_reach_stdout_and_the_journal` |
| Windows built by the host, overlays included | `the_host_builds_every_configured_window_itself` |
| Only app `blob:` downloads | `only_app_page_blob_downloads_are_allowed` |
| Desktop ACL build check still holds | `integration_capabilities` (54 tests), `build.rs` on every build |

## Test output

### Red (before the implementation)

Integration tests, against the new test files with no implementation:

```text
      1 error: could not compile `filesquid-desktop` (test "integration_host_events") due to 5 previous errors; 1 warning emitted
      1 error: could not compile `filesquid-desktop` (test "integration_ipc_gate") due to 2 previous errors; 1 warning emitted
      1 error[E0432]: unresolved import `filesquid_desktop_lib::app_window`
      1 error[E0432]: unresolved import `filesquid_desktop_lib::host_subscriber`
      1 error[E0432]: unresolved import `filesquid_desktop_lib::ipc_gate`
      1 error[E0432]: unresolved import `filesquid_desktop_lib::ipc_observer`
      1 error[E0432]: unresolved import `filesquid_desktop_lib::panic_journal`
      2 error[E0599]: no method named `get_webview` found for struct `tauri::App<R>` in the current scope
```

Packaged-window suite `refusals.e2e.mjs` against the `origin/main` binary (test-instrumented build of `2039aae`):

```text
Error: no desktop.ipc.refused entry was journaled
Error: no desktop.ipc.refused entry was journaled
Error: no desktop.window.refused entry was journaled
Error: no desktop.ipc.refused entry was journaled
[WebKitGTK 605.1.15 linux #0-0]    ✖ journals a command the host's IPC gate refuses, without its payload
[WebKitGTK 605.1.15 linux #0-0]    ✓ still lets the app page listen for FileSquid events
[WebKitGTK 605.1.15 linux #0-0]    ✖ journals a forged call on Tauri's postMessage path without either key
[WebKitGTK 605.1.15 linux #0-0]    ✖ refuses and journals a new window
[WebKitGTK 605.1.15 linux #0-0]    ✓ refuses and journals a navigation off the app's origins
[WebKitGTK 605.1.15 linux #0-0]    ✖ journals Tauri's own refusal on the postMessage fallback, with the real key
[WebKitGTK 605.1.15 linux #0-0] 2 passing (1m 0.3s)
[WebKitGTK 605.1.15 linux #0-0] 4 failing
[WebKitGTK 605.1.15 linux #0-0] no desktop.ipc.refused entry was journaled
[WebKitGTK 605.1.15 linux #0-0] Error: no desktop.ipc.refused entry was journaled
[WebKitGTK 605.1.15 linux #0-0] no desktop.ipc.refused entry was journaled
[WebKitGTK 605.1.15 linux #0-0] Error: no desktop.ipc.refused entry was journaled
[WebKitGTK 605.1.15 linux #0-0] no desktop.window.refused entry was journaled
[WebKitGTK 605.1.15 linux #0-0] Error: no desktop.window.refused entry was journaled
[WebKitGTK 605.1.15 linux #0-0] no desktop.ipc.refused entry was journaled
[WebKitGTK 605.1.15 linux #0-0] Error: no desktop.ipc.refused entry was journaled
```

### Green

```text
     Running tests/integration/host_events.rs (/home/user/.cargo-target-shared/debug/deps/integration_host_events-c928e7eeb50972e0)
test a_new_window_is_refused_and_journaled_without_its_address ... ok
test a_download_is_journaled_without_its_address_or_file_name ... ok
test a_main_window_that_cannot_be_built_is_journaled ... ok
test a_permission_request_is_refused_and_journaled_by_kind_only ... ok
test a_successful_answer_on_the_post_message_path_is_not_a_refusal ... ok
test a_refusal_tauri_reports_on_its_post_message_path_is_journaled_with_the_command ... ok
test only_app_page_blob_downloads_are_allowed ... ok
test ordinary_host_events_still_reach_stdout_and_the_journal ... ok
test other_tauri_webview_errors_are_journaled_without_their_text ... ok
test the_host_builds_every_configured_window_itself ... ok
test tauris_invoke_key_refusal_is_journaled_once_and_the_key_stays_out_of_the_logs ... ok
test the_main_window_is_built_by_the_host_and_its_opening_and_closing_journaled ... ok
test an_unreadable_post_message_call_is_journaled_without_its_text ... ok
test a_panic_is_journaled_without_its_message ... ok
test the_panic_hook_flushes_the_journal_it_is_given ... ok
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
     Running tests/integration/ipc_gate.rs (/home/user/.cargo-target-shared/debug/deps/integration_ipc_gate-086deb2210080f17)
test a_command_name_becomes_an_opaque_journal_id_or_unrecognised ... ok
test a_cors_preflight_is_answered_without_a_check ... ok
test a_call_for_an_unknown_webview_is_refused_and_journaled ... ok
test a_call_from_a_foreign_origin_is_journaled_without_the_address ... ok
test a_refusal_is_answered_in_tauris_wire_format_and_says_not_allowed ... ok
test a_call_tauri_never_answers_is_journaled_and_answered ... ok
test an_allowed_call_with_the_invoke_key_is_forwarded_to_tauri ... ok
test a_malformed_json_body_is_refused_and_journaled ... ok
test a_wrong_invoke_key_is_journaled_without_either_key ... ok
test refusal_codes_are_opaque_and_distinct ... ok
test a_refused_command_is_journaled_without_its_payload_or_the_invoke_key ... ok
test every_refusal_reason_is_decided_before_tauri_sees_the_call ... ok
test the_failed_operation_names_errors_that_are_not_refusals ... ok
test the_gate_owns_the_scheme_tauri_sends_every_invoke_through ... ok
test the_ipc_allowlist_is_exactly_the_granted_event_permissions ... ok
test tauris_own_access_control_refusal_of_a_forwarded_call_is_journaled ... ok
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

BDD (`cargo test -p filesquid-desktop --features test-webdriver --test features --locked`), new feature and totals:

```text
Feature: Every refusal in the desktop shell is journaled
  Scenario: An app page may listen for FileSquid events through the host's IPC gate
   ✔  When an app page asks to listen for FileSquid events with the invoke key
   ✔  Then the host should hand the call to Tauri
  Scenario Outline: A command outside the policy is refused and journaled without its payload
   ✔  When an app page invokes "plugin:fs|read_text_file" with a private path in its payload
   ✔  Then the page should be told the command is not allowed
   ✔  And the activity journal should record the IPC refusal "command_not_allowed" for "plugin:fs.read_text_file"
   ✔  And no journal entry should contain the payload or the invoke key
  Scenario Outline: A command outside the policy is refused and journaled without its payload
   ✔  When an app page invokes "plugin:sql|load" with a private path in its payload
   ✔  Then the page should be told the command is not allowed
   ✔  And the activity journal should record the IPC refusal "command_not_allowed" for "plugin:sql.load"
   ✔  And no journal entry should contain the payload or the invoke key
  Scenario Outline: A command outside the policy is refused and journaled without its payload
   ✔  When an app page invokes "plugin:event|emit" with a private path in its payload
   ✔  Then the page should be told the command is not allowed
   ✔  And the activity journal should record the IPC refusal "command_not_allowed" for "plugin:event.emit"
   ✔  And no journal entry should contain the payload or the invoke key
  Scenario Outline: A command outside the policy is refused and journaled without its payload
   ✔  When an app page invokes "health_ping" with a private path in its payload
   ✔  Then the page should be told the command is not allowed
   ✔  And the activity journal should record the IPC refusal "command_not_allowed" for "health_ping"
   ✔  And no journal entry should contain the payload or the invoke key
  Scenario: A call with a forged invoke key is refused and journaled without either key
   ✔  When a page invokes "plugin:event|listen" with a forged invoke key
   ✔  Then the page should be told the command is not allowed
   ✔  And the activity journal should record the IPC refusal "invalid_invoke_key" for "plugin:event.listen"
   ✔  And no journal entry should contain the payload or the invoke key
  Scenario: Tauri's own access-control refusal of a forwarded call is journaled
   ✔  Given a window whose capabilities grant nothing
   ✔  When an app page asks to listen for FileSquid events with the invoke key
   ✔  Then the page should be told the command is not allowed
   ✔  And the activity journal should record the IPC refusal "acl_denied" for "plugin:event.listen"
  Scenario: A page that opens a new window is refused and journaled
   ✔  When a page asks to open a new window
   ✔  Then the activity journal should record "desktop.window.refused" with the code "new_window_denied"
  Scenario: A page that asks for the camera is refused and journaled by kind only
   ✔  When a page asks for the camera
   ✔  Then the activity journal should record "desktop.permission.refused" with the code "permission_denied"
  Scenario: A download from outside the app is refused and journaled without its address
   ✔  When a page starts a download from "https://example.com/holiday-photos.zip"
   ✔  Then the download should be refused
   ✔  And the activity journal should record "desktop.download.refused" with the code "download_denied"
   ✔  And no journal entry should contain "holiday-photos"
Feature: Desktop transport: per-launch token, event channel and ranged reads
30 scenarios (30 passed)
108 steps (108 passed)
```

Packaged window (`xvfb-run -a npm run test:e2e` in `tests/e2e`):

```text
[WebKitGTK 605.1.15 linux #0-1]
[WebKitGTK 605.1.15 linux #0-1] » specs/refusals.e2e.mjs
[WebKitGTK 605.1.15 linux #0-1] filesquid-desktop journals every refusal in a packaged window
[WebKitGTK 605.1.15 linux #0-1]    ✓ journals a command the host's IPC gate refuses, without its payload
[WebKitGTK 605.1.15 linux #0-1]    ✓ still lets the app page listen for FileSquid events
[WebKitGTK 605.1.15 linux #0-1]    ✓ journals a forged call on Tauri's postMessage path without either key
[WebKitGTK 605.1.15 linux #0-1]    ✓ refuses and journals a new window
[WebKitGTK 605.1.15 linux #0-1]    ✓ refuses and journals a navigation off the app's origins
[WebKitGTK 605.1.15 linux #0-1]    ✓ journals Tauri's own refusal on the postMessage fallback, with the real key
[WebKitGTK 605.1.15 linux #0-1]
[WebKitGTK 605.1.15 linux #0-1] 6 passing (757ms)
------------------------------------------------------------------
[WebKitGTK 605.1.15 linux #0-2] Running: WebKitGTK (v605.1.15) on linux
[WebKitGTK 605.1.15 linux #0-2] Session ID: 85375b76-015f-4209-84d2-c28a4c89f1cc
[WebKitGTK 605.1.15 linux #0-2]
[WebKitGTK 605.1.15 linux #0-2] » specs/transport.e2e.mjs
[WebKitGTK 605.1.15 linux #0-2] filesquid-desktop transport in a packaged window
[WebKitGTK 605.1.15 linux #0-2]    ✓ gives the app page a 64-hex per-launch token
[WebKitGTK 605.1.15 linux #0-2]    ✓ answers API calls with the token and refuses them without it
[WebKitGTK 605.1.15 linux #0-2]    ✓ reads the last bytes of a 100 MB file as partial content
[WebKitGTK 605.1.15 linux #0-2]    ✓ reads the whole 100 MB file through capped ranges
[WebKitGTK 605.1.15 linux #0-2]    ✓ pushes events to the app window on the event channel
[WebKitGTK 605.1.15 linux #0-2]    ✓ keeps the app window on the app's own origin
[WebKitGTK 605.1.15 linux #0-2]
[WebKitGTK 605.1.15 linux #0-2] 6 passing (4.7s)


Spec Files:	 3 passed, 3 total (100% completed) in 00:00:10  

✅ Desktop packaged-window e2e passed (14s).
```

### Full gate

```text
$ FULL=1 FILESQUID_DESKTOP_E2E=1 bash scripts/pre-commit.sh
✅ Desktop packaged-window e2e passed (14s).
✅ All Pre-Commit Hooks Passed! Test scope: full (172s)
```

## Mocked Environment Variables

None.

## Cubic round 1 fixes

- A panic on the journal writer's thread no longer queues its record (it could block on a full queue whose only consumer is that thread); it prints a redacted stdout line instead.
- The observer prints through the host's stdout writer (`with_stdout`), so the redaction tests now read the observer's own lines; `tauri::webview.error` carries the code `webview_error`.
- Navigation refusals carry the actor `webview:<label>`; a future download event kind is refused and journaled.
- The BDD permission scenario asserts the `camera` target; the packaged-window redaction check pages through the whole journal.
- DE-S-002, DE-S-003, ADR 002, the plan row, README and the root SRS note now state the `postMessage` path, the `webview` actor of trace-read refusals and the panic exceptions exactly.
