# Task DE-F1.1 Walkthrough: Per-launch token, event channel and ranged reads

Module task [DE-F1.1](../plan.md) of global [Task FS-1.1](../../../../../../resources/implementation-plan.md#task-fs-1.1), which this task completes (API-F1.1 and API-F1.2 were done; SV-F1.1 moved to Task FS-3.10). Branch `feat/de-f1.1-launch-token`. The per-launch token, its authoriser, the app-origin init script and the CORS preflight landed with [API-F1.7](../../../api/resources/tasks/Task-API-F1.7-Walkthrough.md); this task binds the event channel, serves ranged reads through the scheme, makes host failures typed, and proves both in a packaged window.

## DE-S-005: Every API request through the scheme MUST carry a per-launch token, and the event channel MUST sit behind the same client interface as the server's

> Every API request through the scheme MUST carry a per-launch token, and the event channel MUST sit behind the same client interface as the server's. The event channel MUST use one counted event-bus subscription, forward its resync hints, and reach only the app's own webviews (navigation to any other origin is refused; only the blank documents `about:blank` and `about:srcdoc` are also allowed). A satisfiable `Range` request MUST pass through the scheme with its `206` status and `Content-Range`; an unsatisfiable one MUST keep its `416` status and `Content-Range: bytes */<length>`. The handler MUST buffer at most 16 MiB of one response, answering a larger one with a typed `internal` error.

The second and third sentences were added to [DE-S-005](../srs.md) in this task. Parents: [F-057 Desktop and server hosts](../../../../../../resources/prd.md#f-057-desktop-and-server-hosts), [SRS-F-057 Desktop and server hosts](../../../../../../resources/srs.md#srs-f-057-desktop-and-server-hosts), [F-056 Single FileSquid API](../../../../../../resources/prd.md#f-056-single-filesquid-api), FileSquid [U-17, U-18](../../../../../../resources/product-vision.md#one-api-hosts-and-distributions), [T-049, T-050](../../../../../../resources/product-vision.md#api-and-hosts), [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md).

### Secondary SRS mapping

- **DE-S-001: The desktop serves the FileSquid API in-process through a Tauri custom URI scheme with no TCP listener (the API health route replaces `health_ping`). Any remaining Tauri command MUST be typed and explicitly authorized by a deny-by-default Tauri capability.** Kept: the event channel uses Tauri events, not a loopback socket, and no command was added. The embedded WebDriver server (which listens on `127.0.0.1`) exists only behind the `test-webdriver` feature; `with_test_driver` is a no-op in every other build.
- **DE-S-002: Capabilities are deny-by-default; no fs/shell/sql plugins at scaffold.** Kept: `capabilities/default.json` is unchanged (event listen and unlisten for the main window only); `tauri-plugin-wdio-webdriver` declares no permissions and is never in a shipping build.
- **DE-S-003: Logs are stdout JSON.** Kept: the new events go through `tracing` (stdout JSON, and the journal layer for warnings and errors). No log file.
- **DE-S-006: The webview MUST receive opaque handles and API responses, never raw filesystem paths or SQL.** Kept: host error bodies are `ApiError` with fixed messages; the detail goes to stdout only.

## What changed

- **Event channel** ([`src/event_channel.rs`](../../src/event_channel.rs)): `open_subscription` takes one counted `EventBus::subscription()`. If the bus refuses it, the refusal is journaled as `desktop.events.subscription` / `unavailable`, and the forwarder's process fails. `pump` forwards every event, `events.resync` included, to an `EventSink` in order. It heartbeats its `event-forwarder` process every 5 s (stall timeout 30,000 ms) and ends when the bus closes or the stop flag is set at app exit. A failed delivery is journaled as `desktop.events.forward` / `emit_failed` without its detail, and forwarding goes on. `TauriSink` emits `filesquid-event`, the name `@filesquid/ui` already listens to, so the UI did not change. `is_app_url` and `navigation_allowed` keep every webview on the app's origins (the token script's list, plus `about:blank` and `about:srcdoc`). Tauri events reach every listening webview, so this makes every listener an app page. A refusal is journaled as `desktop.navigation.refused` / `foreign_origin`, never with the address.
- **Ranged reads and host errors** ([`src/api_protocol.rs`](../../src/api_protocol.rs)): the preflight now allows `range`, and app-origin answers expose `content-range` and `accept-ranges` (plus the journal cursor header, as before). `into_scheme_response` buffers at most `MAX_SCHEME_RESPONSE_BYTES` (16 MiB) with `http_body_util::Limited`; a longer body becomes a typed `internal` `ApiError` (journaled as `desktop.scheme.response`). An unroutable request address becomes `invalid_request` (`desktop.scheme.request`). `api_error_response` builds the JSON `ApiError` answer.
- **Host wiring** ([`src/lib.rs`](../../src/lib.rs)): the `filesquid-app-pages` plugin carries the token init script and the navigation hook. `start_monitor` registers the forwarder process, `forward_events` starts the pump, and `stop_monitor` stops it and completes its process before the journal shuts down. The old uncounted `subscribe()` forwarder, which dropped lag silently, is gone.
- **Test-instrumented build** (`--features test-webdriver`): `tauri-plugin-wdio-webdriver` 1.5.0 (optional dependency), and [`src/test_fixture.rs`](../../src/test_fixture.rs), a 104,857,600-byte deterministic file at `/__filesquid_test__/range-fixture`. The fixture needs the per-launch token, answers one `Range` per request with `206`, caps each answer at 8 MiB and answers `416 bytes */104857600` past the end. Default builds answer that path `404` (tested).
- **Packaged-window suite** ([`tests/e2e/`](../../tests/e2e/specs/transport.e2e.mjs)): WebdriverIO 9.32.0 with `@wdio/tauri-service` 1.5.0, embedded provider. JUnit report in `tests/allure-results/wdio/`, cleared before each run.
- **Gates** ([`scripts/verify_tests.sh`](../../../../../../scripts/verify_tests.sh)): Clippy and `cargo test` for the desktop crate with `--features test-webdriver` on every run. `FILESQUID_DESKTOP_E2E=1` builds the binary, checks the suite's formatting and runs it (with `xvfb-run` on Linux without a display).
- **Docs**: [ADR 030](../../../../../../resources/architecture/030-filesquid-api-and-transports.md) (desktop binding, 16 MiB cap, typed host errors, S1 (a) partly measured), [ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) (first native-window tests and their test-only dependencies), [DE-S-005](../srs.md), the [SRS-F-057](../../../../../../resources/srs.md#srs-f-057-desktop-and-server-hosts) test list, and the root [README](../../../../../../README.md#run-tests).

## Constraint gates

| Constraint | Test |
|---|---|
| Token required on every non-preflight API request through the scheme except the public liveness probe `GET /api/v1/health/live` (fixture too) | `a_call_without_the_per_launch_token_is_refused`, `a_ranged_read_without_the_per_launch_token_is_refused`, e2e "answers API calls with the token and refuses them without it" |
| One counted subscription | `the_forwarder_counts_against_the_subscriber_cap` |
| Resync hints forwarded (lag past 256, events over 65,536 bytes) | `a_webview_that_fell_behind_is_told_to_reread`, `an_oversized_event_reaches_the_webview_as_a_resync_hint` |
| Refusal at 64 subscriptions journaled | `a_refused_subscription_is_journaled_as_an_error` |
| Only the app's own webviews | `only_the_apps_own_pages_are_app_urls`, `navigation_to_a_foreign_origin_is_refused_and_journaled_without_the_address`, e2e "keeps the app window on the app's own origin" (it fails when the guard is disabled: `Expected: "tauri://localhost"`, `Received: "https://example.com"`) |
| Event name `filesquid-event`; event reaches the window | `the_event_name_is_the_one_the_ui_listens_to`, e2e "pushes events to the app window on the event channel" |
| `event-forwarder` process, 5 s heartbeat, 30,000 ms timeout, ends at exit | `the_forwarder_is_a_registered_process_with_a_heartbeat`, `the_forwarder_stops_when_the_app_exits` |
| `range` allowed, `content-range` and `accept-ranges` exposed | `app_origins_may_preflight_the_range_header`, `app_origin_responses_expose_the_range_headers`, `windows_style_scheme_urls_allow_and_expose_the_range_headers` (`http://filesquid.localhost` from `http://tauri.localhost`) |
| `206` and `Content-Range` pass through unchanged | `a_partial_response_passes_through_the_scheme_unchanged` |
| 16 MiB buffer cap, typed `internal` beyond | `the_scheme_buffers_at_most_sixteen_mebibytes`, `a_response_over_the_buffer_cap_is_a_typed_internal_error` |
| Host errors are `ApiError` bodies | `host_errors_carry_the_api_error_body` |
| 100 MB file, 8 MiB ranges, `416` past the end | `integration_range_fixture` (9 tests), e2e "reads the last bytes of a 100 MB file as partial content" and "reads the whole 100 MB file through capped ranges" |
| Redaction of every new event type | `an_oversized_response_is_journaled_without_its_detail` (`desktop.scheme.response`), `a_refused_subscription_is_journaled_as_an_error` (`desktop.events.subscription`), `a_failed_delivery_is_journaled_without_its_detail_and_forwarding_goes_on` (`desktop.events.forward`), `navigation_to_a_foreign_origin_is_refused_and_journaled_without_the_address` (`desktop.navigation.refused`). `desktop.scheme.request` uses the same whitelisted fields (`operation`, `error_code`) and carries no address. |
| No fixture in default builds | `default_builds_serve_no_test_fixture` |

[ADR 017](../../../../../../resources/architecture/017-test-runners-and-reporting.md) defines no Rust line-coverage tool, so no coverage percentage is reported. Every new public item in `event_channel`, `api_protocol` and `test_fixture` is exercised by the tests above.

## Open item: media elements and the per-launch token

S1 item (a) is only partly closed. `fetch` with `Authorization` reads 100 MB in ranges in a real WebKitGTK window. `<img>` and `<video>` cannot send an `Authorization` header, though, and DE-S-005 and ADR 030 Decision 5 require the token on every request. How a media element presents the token is an open owner decision, recorded in ADR 030's S1 table. It blocks the API-F1.3 acceptance gate, not this task. WKWebView and WebView2 runs are also still open.

## Red: tests before the implementation

`cargo test -p filesquid-desktop --features test-webdriver` before the implementation (unique errors, counted; the tests referred to an API that did not exist yet):

```text
      1 error: could not compile `filesquid-desktop` (test "features") due to 3 previous errors
      1 error: could not compile `filesquid-desktop` (test "integration_api_protocol") due to 1 previous error
      1 error[E0432]: unresolved import `filesquid_desktop_lib::api_protocol::into_scheme_response`
      1 error[E0432]: unresolved import `filesquid_desktop_lib::event_channel`
      1 error[E0432]: unresolved import `filesquid_desktop_lib::test_fixture`
      1 error[E0432]: unresolved imports `filesquid_desktop_lib::api_protocol::api_error_response`, `filesquid_desktop_lib::api_protocol::into_scheme_response`, `filesquid_desktop_lib::api_protocol::MAX_SCHEME_RESPONSE_BYTES`
```

## Green: test output

From `apps/filesquid/`: `cargo fmt --all --check` (no output), `cargo clippy --workspace --all-targets --locked -- -D warnings` (exit 0), `cargo clippy -p filesquid-desktop --all-targets --features test-webdriver -- -D warnings` (exit 0), `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` (exit 0).

`cargo test -p filesquid-desktop --locked` (default build):

```text
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_desktop_lib-52778f5c8718ff5b)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/main.rs (<target>/debug/deps/filesquid_desktop-631830d4c05ebbac)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/integration/api_protocol.rs (<target>/debug/deps/integration_api_protocol-2a2ef9f0b6b87f3e)
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/integration/event_channel.rs (<target>/debug/deps/integration_event_channel-4faa31f6022eee8e)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`cargo test -p filesquid-desktop --features test-webdriver --locked` (test-instrumented build; the JSON line is the stdout record of a refused call printed before any monitor exists):

```text
     Running unittests src/lib.rs (<target>/debug/deps/filesquid_desktop_lib-d5d4aa6dcccb428a)

running 3 tests
test api_protocol::tests::the_exposed_headers_name_the_journal_cursor_header ... ok
test test_fixture::tests::ranges_are_parsed_and_capped ... ok
test test_fixture::tests::malformed_or_unsatisfiable_ranges_are_refused ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src/main.rs (<target>/debug/deps/filesquid_desktop-8c6886ce8d9914e9)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/features.rs (<target>/debug/deps/features-27868679ccc9f319)
Feature: Desktop transport: per-launch token, event channel and ranged reads
  Scenario: An app page reads the end of a 100 MB file as partial content
   ✔> Given the desktop host has started with a new per-launch token
{"actor":"anonymous","error_code":"missing_credentials","host":"","journal_unavailable":true,"level":"warn","operation":"api.auth.denied","outcome":"failed","target_ids":["no_route"],"time":"2026-10-18T05:26:01.382Z"}
   ✔  When an app page reads the last 100 bytes of the 100 MB test file with the token
   ✔  Then the read should succeed as partial content
   ✔  And the answer should name the range "bytes 104857500-104857599/104857600"
   ✔  And the answer should carry exactly the requested bytes
  Scenario: An app window receives a published event
   ✔> Given the desktop host has started with a new per-launch token
   ✔  Given an app window subscribed to the event channel
   ✔  When a process update is published
   ✔  Then the window should receive a "process.update" event
  Scenario: An open-ended range is capped so memory stays bounded
   ✔> Given the desktop host has started with a new per-launch token
   ✔  When an app page reads the 100 MB test file from its first byte with the token
   ✔  Then the read should succeed as partial content
   ✔  And the answer should name the range "bytes 0-8388607/104857600"
  Scenario: The whole 100 MB file can be read in capped ranges
   ✔> Given the desktop host has started with a new per-launch token
   ✔  When an app page reads the whole 100 MB test file range by range with the token
   ✔  Then it should have read 104857600 bytes in 13 reads
  Scenario: A window that fell behind is told to reread
   ✔> Given the desktop host has started with a new per-launch token
   ✔  Given an app window subscribed to the event channel that is not reading
   ✔  When one more event than the buffer holds is published
   ✔  Then the window should first receive an "events.resync" hint for 1 missed event
  Scenario: A response larger than the scheme buffer is refused, not buffered
   ✔> Given the desktop host has started with a new per-launch token
   ✔  When the host answers with a body one byte over its buffer cap
   ✔  Then the answer should be the typed error "internal"
  Scenario: A read without the per-launch token is refused
   ✔> Given the desktop host has started with a new per-launch token
   ✔  When a page reads the last 100 bytes of the 100 MB test file without the token
   ✔  Then the answer should be the typed error "unauthorized"
  Scenario: A range past the end of the file is refused
   ✔> Given the desktop host has started with a new per-launch token
   ✔  When an app page reads past the end of the 100 MB test file with the token
   ✔  Then the answer should be "range not satisfiable" naming the length 104857600
  Scenario: An app window cannot be navigated to a foreign site
   ✔> Given the desktop host has started with a new per-launch token
   ✔  When an app window is asked to navigate to "https://example.com/"
   ✔  Then the navigation should be refused
   ✔  And the activity journal should record "desktop.navigation.refused" without the address
[Summary]
1 feature
9 scenarios (9 passed)
33 steps (33 passed)
     Running tests/integration/api_protocol.rs (<target>/debug/deps/integration_api_protocol-40663914700666e0)

running 25 tests
test a_partial_response_passes_through_the_scheme_unchanged ... ok
test a_response_over_the_buffer_cap_is_a_typed_internal_error ... ok
test a_call_without_the_per_launch_token_is_refused ... ok
test app_origin_responses_expose_the_journal_cursor_header ... ok
test an_oversized_response_is_journaled_without_its_detail ... ok
test a_token_from_another_launch_is_refused ... ok
test app_origin_responses_expose_the_range_headers ... ok
test app_origins_may_preflight_the_authorization_header ... ok
test app_origins_receive_cors_headers ... ok
test app_origins_may_preflight_the_range_header ... ok
test dev_server_origins_are_trusted_only_in_debug_builds ... ok
test each_launch_installs_a_new_token_once ... ok
test foreign_origins_get_no_preflight_grant ... ok
test foreign_origins_receive_no_cors_headers ... ok
test health_is_served_through_the_custom_scheme ... ok
test host_errors_carry_the_api_error_body ... ok
test release_builds_trust_only_the_apps_own_scheme_origins ... ok
test scheme_name_is_filesquid ... ok
test the_init_script_hands_the_token_only_to_app_origins ... ok
test the_scheme_buffers_at_most_sixteen_mebibytes ... ok
test windows_style_scheme_urls_are_served_too ... ok
test windows_style_scheme_urls_allow_and_expose_the_range_headers ... ok
test unknown_paths_are_not_found ... ok
test the_desktop_owner_reaches_admin_routes_with_the_per_launch_token ... ok
test the_liveness_probe_needs_no_token ... ok

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/integration/event_channel.rs (<target>/debug/deps/integration_event_channel-97ad443b17ace1fe)

running 12 tests
test a_failed_delivery_is_journaled_without_its_detail_and_forwarding_goes_on ... ok
test app_webviews_may_open_blank_documents ... ok
test an_oversized_event_reaches_the_webview_as_a_resync_hint ... ok
test a_webview_that_fell_behind_is_told_to_reread ... ok
test a_refused_subscription_is_journaled_as_an_error ... ok
test navigation_to_a_foreign_origin_is_refused_and_journaled_without_the_address ... ok
test published_events_reach_the_webview_in_order_until_the_bus_closes ... ok
test the_event_name_is_the_one_the_ui_listens_to ... ok
test only_the_apps_own_pages_are_app_urls ... ok
test the_forwarder_counts_against_the_subscriber_cap ... ok
test the_forwarder_is_a_registered_process_with_a_heartbeat ... ok
test the_forwarder_stops_when_the_app_exits ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests/integration/range_fixture.rs (<target>/debug/deps/integration_range_fixture-dd7bda2eb135fa1c)

running 9 tests
test a_suffix_range_reads_the_end_of_the_file ... ok
test an_unsatisfiable_range_is_refused_with_the_file_length ... ok
test app_pages_can_read_the_range_headers ... ok
test the_fixture_is_a_100_mb_file_read_in_ranges_under_the_scheme_cap ... ok
test the_last_bytes_of_a_100_mb_file_are_read_as_partial_content ... ok
test a_read_without_a_range_is_an_invalid_request ... ok
test a_ranged_read_without_the_per_launch_token_is_refused ... ok
test an_open_ended_range_is_capped_so_memory_stays_bounded ... ok
test the_whole_100_mb_file_is_read_through_capped_ranges ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.15s

   Doc-tests filesquid_desktop_lib

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Packaged window: `cargo build -p filesquid-desktop --features test-webdriver --locked`, then in `apps/filesquid/crates/desktop/tests/e2e`: `FILESQUID_DESKTOP_BIN=<copy of the built binary> xvfb-run -a npm run test:e2e` (Linux, WebKitGTK 2.52.6; the spec's file URL is shortened to `<file URL>`, and Cargo's target directory to `<target>` above, to keep machine paths out of the repository):

```text
> pretest:e2e
> node -e "require('fs').rmSync('../allure-results/wdio', { recursive: true, force: true })"


> test:e2e
> wdio run ./wdio.conf.mjs


Execution of 1 workers started at 2026-10-18T05:26:41.202Z

[0-0] RUNNING in undefined - <file URL>/specs/transport.e2e.mjs
[0-0] PASSED in undefined - <file URL>/specs/transport.e2e.mjs

 "spec" Reporter:
------------------------------------------------------------------
[WebKitGTK 605.1.15 linux #0-0] Running: WebKitGTK (v605.1.15) on linux
[WebKitGTK 605.1.15 linux #0-0] Session ID: 2d7ee692-67d4-4371-866d-3dde3e8d1880
[WebKitGTK 605.1.15 linux #0-0]
[WebKitGTK 605.1.15 linux #0-0] » specs/transport.e2e.mjs
[WebKitGTK 605.1.15 linux #0-0] filesquid-desktop transport in a packaged window
[WebKitGTK 605.1.15 linux #0-0]    ✓ gives the app page a 64-hex per-launch token
[WebKitGTK 605.1.15 linux #0-0]    ✓ answers API calls with the token and refuses them without it
[WebKitGTK 605.1.15 linux #0-0]    ✓ reads the last bytes of a 100 MB file as partial content
[WebKitGTK 605.1.15 linux #0-0]    ✓ reads the whole 100 MB file through capped ranges
[WebKitGTK 605.1.15 linux #0-0]    ✓ pushes events to the app window on the event channel
[WebKitGTK 605.1.15 linux #0-0]    ✓ keeps the app window on the app's own origin
[WebKitGTK 605.1.15 linux #0-0]
[WebKitGTK 605.1.15 linux #0-0] 6 passing (5.5s)


Spec Files:	 1 passed, 1 total (100% completed) in 00:00:07
```

## Review round 1

Cubic reviewed head `5bf3b03` and opened 16 threads. Each was checked against the code; all were valid and are fixed in one commit:

- **Journal capture:** `tests/support/journal.rs` now follows `next_cursor` (`all_entries`), so `journaled` returns every entry, not only the first page of 100.
- **Whole-file read:** `the_whole_100_mb_file_is_read_through_capped_ranges` and the packaged-window spec now check every byte of every range against the fixture pattern, and each answer's `Content-Range`, not only the ends.
- **Launch journaling in the BDD background:** the step "the desktop host has started" now goes through the real launch path. The new `open_host_monitor` is what `start_monitor` calls: it opens the SQLite journal in a scratch data directory, installs the monitor and journals `api.auth.token_issued`. A new Background step asserts exactly one such entry, with target `owner` and without the secret. The scratch directory is removed when the scenario ends.
- **Binary path for the packaged-window run** (`wdio.conf.mjs`, `verify_tests.sh`, README):
  - A relative `CARGO_TARGET_DIR` is resolved against `apps/filesquid`, where Cargo ran.
  - The chosen binary is copied into a run-specific temporary directory before launch, so a concurrent Cargo build in a shared target cannot replace it. The directory is removed in `onComplete`.
  - `FILESQUID_DESKTOP_BIN` still overrides the binary.
- **DE-S-005:** the requirement now names the `about:blank`/`about:srcdoc` exceptions. It requires `206` with `Content-Range` only for satisfiable ranges, and `416` with `Content-Range: bytes */<length>` otherwise. The quote at the top of this walkthrough follows.
- **Engines:** the e2e package now requires node `>=22.12.0` and npm `>=11.0.0`.
- **ADRs:** ADR 017's amendment carries an RFC 3339 UTC timestamp. ADR 030's working recommendation marks `tauri::ipc::Channel` as superseded on the desktop by S1 (b)'s Tauri events.
- **Checklist:** the MVP summary reads 9 of 26 done.
- **README:** names both scheme origins, shows plain `npm run test:e2e` by default with the headless-Linux `xvfb-run` form as a comment, and describes the binary resolution and copy.
- **This walkthrough:** the token gate now says "every non-preflight API request except the public liveness probe".

`cargo test -p filesquid-desktop --features test-webdriver --locked --test features --test integration_range_fixture` (the stdout lines of refused calls are left out):

```text
     Running tests/features.rs (<target>/debug/deps/features-27868679ccc9f319)
Feature: Desktop transport: per-launch token, event channel and ranged reads
  Scenario: An app page reads the end of a 100 MB file as partial content
   ✔> Given the desktop host has started with a new per-launch token
   ✔> Then the activity journal should record one token issue for the owner without the secret
   ✔  When an app page reads the last 100 bytes of the 100 MB test file with the token
   ✔  Then the read should succeed as partial content
   ✔  And the answer should name the range "bytes 104857500-104857599/104857600"
   ✔  And the answer should carry exactly the requested bytes
  Scenario: An app window receives a published event
   ✔> Given the desktop host has started with a new per-launch token
   ✔> Then the activity journal should record one token issue for the owner without the secret
   ✔  Given an app window subscribed to the event channel
   ✔  When a process update is published
   ✔  Then the window should receive a "process.update" event
  Scenario: An open-ended range is capped so memory stays bounded
   ✔> Given the desktop host has started with a new per-launch token
   ✔> Then the activity journal should record one token issue for the owner without the secret
   ✔  When an app page reads the 100 MB test file from its first byte with the token
   ✔  Then the read should succeed as partial content
   ✔  And the answer should name the range "bytes 0-8388607/104857600"
  Scenario: The whole 100 MB file can be read in capped ranges
   ✔> Given the desktop host has started with a new per-launch token
   ✔> Then the activity journal should record one token issue for the owner without the secret
   ✔  When an app page reads the whole 100 MB test file range by range with the token
   ✔  Then it should have read 104857600 bytes in 13 reads
  Scenario: A window that fell behind is told to reread
   ✔> Given the desktop host has started with a new per-launch token
   ✔> Then the activity journal should record one token issue for the owner without the secret
   ✔  Given an app window subscribed to the event channel that is not reading
   ✔  When one more event than the buffer holds is published
   ✔  Then the window should first receive an "events.resync" hint for 1 missed event
  Scenario: A response larger than the scheme buffer is refused, not buffered
   ✔> Given the desktop host has started with a new per-launch token
   ✔> Then the activity journal should record one token issue for the owner without the secret
   ✔  When the host answers with a body one byte over its buffer cap
   ✔  Then the answer should be the typed error "internal"
  Scenario: A read without the per-launch token is refused
   ✔> Given the desktop host has started with a new per-launch token
   ✔> Then the activity journal should record one token issue for the owner without the secret
   ✔  When a page reads the last 100 bytes of the 100 MB test file without the token
   ✔  Then the answer should be the typed error "unauthorized"
  Scenario: A range past the end of the file is refused
   ✔> Given the desktop host has started with a new per-launch token
   ✔> Then the activity journal should record one token issue for the owner without the secret
   ✔  When an app page reads past the end of the 100 MB test file with the token
   ✔  Then the answer should be "range not satisfiable" naming the length 104857600
  Scenario: An app window cannot be navigated to a foreign site
   ✔> Given the desktop host has started with a new per-launch token
   ✔> Then the activity journal should record one token issue for the owner without the secret
   ✔  When an app window is asked to navigate to "https://example.com/"
   ✔  Then the navigation should be refused
   ✔  And the activity journal should record "desktop.navigation.refused" without the address
[Summary]
1 feature
9 scenarios (9 passed)
42 steps (42 passed)
     Running tests/integration/range_fixture.rs (<target>/debug/deps/integration_range_fixture-dd7bda2eb135fa1c)

running 9 tests
test a_suffix_range_reads_the_end_of_the_file ... ok
test a_read_without_a_range_is_an_invalid_request ... ok
test an_unsatisfiable_range_is_refused_with_the_file_length ... ok
test app_pages_can_read_the_range_headers ... ok
test the_fixture_is_a_100_mb_file_read_in_ranges_under_the_scheme_cap ... ok
test the_last_bytes_of_a_100_mb_file_are_read_as_partial_content ... ok
test a_ranged_read_without_the_per_launch_token_is_refused ... ok
test an_open_ended_range_is_capped_so_memory_stays_bounded ... ok
test the_whole_100_mb_file_is_read_through_capped_ranges ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.65s
```

Packaged window, round 1 run with `FILESQUID_DESKTOP_BIN` unset (default binary resolution: `CARGO_TARGET_DIR/debug/filesquid-desktop`, copied by `wdio.conf.mjs` to a run directory), `xvfb-run -a npm run test:e2e`:

```text
[WebKitGTK 605.1.15 linux #0-0] Running: WebKitGTK (v605.1.15) on linux
[WebKitGTK 605.1.15 linux #0-0] Session ID: 551cd371-031d-4c55-a58f-2c07d290e2cd
[WebKitGTK 605.1.15 linux #0-0]
[WebKitGTK 605.1.15 linux #0-0] » specs/transport.e2e.mjs
[WebKitGTK 605.1.15 linux #0-0] filesquid-desktop transport in a packaged window
[WebKitGTK 605.1.15 linux #0-0]    ✓ gives the app page a 64-hex per-launch token
[WebKitGTK 605.1.15 linux #0-0]    ✓ answers API calls with the token and refuses them without it
[WebKitGTK 605.1.15 linux #0-0]    ✓ reads the last bytes of a 100 MB file as partial content
[WebKitGTK 605.1.15 linux #0-0]    ✓ reads the whole 100 MB file through capped ranges
[WebKitGTK 605.1.15 linux #0-0]    ✓ pushes events to the app window on the event channel
[WebKitGTK 605.1.15 linux #0-0]    ✓ keeps the app window on the app's own origin
[WebKitGTK 605.1.15 linux #0-0]
[WebKitGTK 605.1.15 linux #0-0] 6 passing (5.4s)
Spec Files:	 1 passed, 1 total (100% completed) in 00:00:07
```

## Review round 2

Cubic reviewed head `f4a529c` and opened 2 threads. Both were valid:

- **`onComplete` could delete a directory it did not create.** `wdio.conf.mjs` now records ownership in `ownsRun`, which is true only in the process that called `mkdtemp`, the launcher. `onComplete` removes the run directory only when `ownsRun` is true. A path inherited by a worker, or supplied by a caller in `FILESQUID_DESKTOP_E2E_RUN`, is never removed; a short comment says why. Checked by hand:
  - After a normal run, the owned directory is gone (0 left in `TMPDIR`).
  - With a caller-supplied `FILESQUID_DESKTOP_E2E_RUN` holding a marker file, calling `onComplete` left the directory and the marker in place.
- **Which runs set `FILESQUID_DESKTOP_BIN`.** The transcript labels now name each invocation:
  - The initial green run set `FILESQUID_DESKTOP_BIN`.
  - The round 1 and round 2 runs left it unset.
  - The Mocked Environment Variables section below now says the same.

Packaged window, round 2 run with `FILESQUID_DESKTOP_BIN` and `FILESQUID_DESKTOP_E2E_RUN` unset (`env -u FILESQUID_DESKTOP_BIN -u FILESQUID_DESKTOP_E2E_RUN xvfb-run -a npm run test:e2e`):

```text
[WebKitGTK 605.1.15 linux #0-0] Running: WebKitGTK (v605.1.15) on linux
[WebKitGTK 605.1.15 linux #0-0] Session ID: 9b6f5254-e88e-4219-8a5a-ecca2a5c3e8b
[WebKitGTK 605.1.15 linux #0-0]
[WebKitGTK 605.1.15 linux #0-0] » specs/transport.e2e.mjs
[WebKitGTK 605.1.15 linux #0-0] filesquid-desktop transport in a packaged window
[WebKitGTK 605.1.15 linux #0-0]    ✓ gives the app page a 64-hex per-launch token
[WebKitGTK 605.1.15 linux #0-0]    ✓ answers API calls with the token and refuses them without it
[WebKitGTK 605.1.15 linux #0-0]    ✓ reads the last bytes of a 100 MB file as partial content
[WebKitGTK 605.1.15 linux #0-0]    ✓ reads the whole 100 MB file through capped ranges
[WebKitGTK 605.1.15 linux #0-0]    ✓ pushes events to the app window on the event channel
[WebKitGTK 605.1.15 linux #0-0]    ✓ keeps the app window on the app's own origin
[WebKitGTK 605.1.15 linux #0-0]
[WebKitGTK 605.1.15 linux #0-0] 6 passing (5s)
Spec Files:	 1 passed, 1 total (100% completed) in 00:00:06
```

## Mocked Environment Variables

None. Only the initial green packaged-window run (section "Green: test output") set `FILESQUID_DESKTOP_BIN`, to a copy of the test-instrumented binary: other sessions share the Cargo target directory and could have replaced `debug/filesquid-desktop` with a default build mid-run. Since review round 1, `wdio.conf.mjs` makes that copy itself, so the round 1 and round 2 runs left `FILESQUID_DESKTOP_BIN` unset. `XDG_DATA_HOME` is set to a fresh temporary directory by `wdio.conf.mjs` itself.
