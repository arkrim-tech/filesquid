# Task UI-0.1 Walkthrough

## UI-S-012: The UI reaches FileSquid only through `FileSquidClient` (`request()`, `read()`, `upload()`, `subscribe()`), which selects the HTTP-plus-event-channel adapter on `filesquid-server`, the custom-scheme adapter on `filesquid-desktop`, and later the browser-host adapter. The desktop base URL MUST be `filesquid://localhost` on macOS, iOS and Linux and `http://filesquid.localhost` on Windows and Android; desktop events MUST arrive as the Tauri event `filesquid-event`. Every command and upload MUST carry a `CommandEnvelope` (schema version 1, non-empty operation id, expected revision and scope) and resolve to a `CommandOutcome` (`committed`, `pending`, `conflict` or `error`) for the same operation id; HTTP 409 MUST resolve to `conflict`; only `committed` is completion evidence. Without a range, `read()` MUST send no `Range` header; with one, it MUST send an inclusive `Range: bytes=<start>-<end>`, or `Range: bytes=<start>-` when no end is given (start ≥ 0, end ≥ start), and MUST refuse an invalid range before sending.

Plan row: [UI-0.1 Host adapter (HTTP vs custom URI scheme)](../plan.md), part of global [Task FS-2.1](../../../../../resources/implementation-plan.md#task-fs-2.1). UI-0.2, UI-0.5 and UI-F2.1 remain, so the global box stays open. Requirements: [UI-S-012](../srs.md), [UI-S-011](../srs.md), [UI-S-014](../srs.md), [CORE-S-005](../../../crates/core/resources/srs.md), [F-037](../../../../../resources/prd.md#f-037-setup-updates-and-diagnostics), [F-056](../../../../../resources/prd.md#f-056-single-filesquid-api), [architecture 3.2](../../../../../resources/architecture.md#32-frontend-architecture), [ADR 030](../../../../../resources/architecture/030-filesquid-api-and-transports.md). Scenarios: [`host_adapter_UI-S-012.feature`](../../tests/features/host_adapter_UI-S-012.feature).

## Already on `main` before this task

UI-F0.1 and the auth-token fix (PR #23) had delivered part of this row. This branch keeps that work and does not duplicate it:

- `detectHost`, `apiBaseUrl` (`filesquid://localhost` or `http://filesquid.localhost` on the desktop, same origin on the server) and `eventsUrl`.
- `subscribe()` on both hosts: `createHostClient` already chose Tauri events (`filesquid-event`) on the desktop and the WebSocket channel on the server. The two new unit tests that pin this behaviour passed before any implementation change (see the failing run below).
- Bearer on every request, WebSocket subprotocols, the launch token and the memory token store (UI-S-014).

## Changes (the remainder)

- **Adapter selection** ([`client.ts`](../../src/api/client.ts)): `selectHostAdapter(env)` names the adapter (`desktop-scheme` or `server-http`), its base URL and its event transport (`tauri` or `websocket`) in one pure function. [`host.ts`](../../src/api/host.ts) `createHostClient` now uses it. The encrypted browser host (UI-F5.1) adds a third kind here without touching views.
- **Client interface** (`FileSquidClient`):
  - `request(path, { method, body, signal })`: any method, JSON body with `Content-Type: application/json`, abort signal; returns parsed JSON, or `undefined` for 204. The typed admin and health methods now go through it.
  - `read(path, { range, signal })`: bytes. An inclusive range becomes `Range: bytes=start-end` (or `bytes=start-`). Returns the unread response, `partial` (206) and the parsed `Content-Range`. An invalid range is refused before sending.
  - `command(path, envelope, input, { method, signal })`: JSON body `{ envelope, input }`; returns a validated `CommandOutcome`. HTTP 409 becomes `conflict`; any other failure stays an `ApiError` with its `reason`.
  - `upload(path, file, envelope, { filename, signal })`: `multipart/form-data` with an `envelope` part (JSON) and a `file` part; the transport sets the boundary; same outcome handling as `command()`.
  - Private `send` (requires 2xx) and `transmit` (adds `Authorization`, the only place that does) replace the GET-only private `request`.
- **Command helpers** ([`commands.ts`](../../src/api/commands.ts), new; pure except `newOperationId`): `commandEnvelope` (schema version 1; non-empty operation id, expected revision and scope; a supplied operation id is kept, so a retry is idempotent), `newOperationId` (version 4 UUID from `crypto.getRandomValues`, which also works outside secure contexts), `commandOutcome` (validates the kind, a `PortError` only for `error`, and the operation id), `conflictOutcome`, `isCommitted` (only `committed` is completion evidence), `rangeHeader`, `parseContentRange`.
- [`complete-list.ts`](../../src/api/complete-list.ts): the compile-time completeness type for run-time copies of contract enums, now shared by `client.ts` and `commands.ts`.
- [`index.ts`](../../src/index.ts) exports the new helpers and types.
- **Tests**: `tests/unit/commands.test.ts` (45), `tests/unit/client-io.test.ts` (21), `tests/unit/host-adapter.test.ts` (13), and a Playwright test that runs the built bundle inside an emulated Windows desktop webview. In it every API call goes to `http://filesquid.localhost` (bridged to the real `filesquid-server`) with the launch token, none goes to the page's origin, no WebSocket opens, the Monitor listens to the Tauri event `filesquid-event`, and an emitted desktop event appears in its journal.
- **Docs**: [UI SRS](../srs.md) UI-S-012 now states the base URLs, the desktop event name, the command envelope and outcome rules and the range rule, with [CORE-S-005](../../../crates/core/resources/srs.md) as a parent. The [UI README](../../README.md) has a new "Host adapter and client interface" section. The [plan](../plan.md) row names its tests and is ticked, as is the row in the [checklist](../../../../../resources/implementation-plan-checklist.md).

## Assumptions for the platform engineer

No file route exists yet (API-F1.3). The client's side of those routes assumes:

1. **Envelope binding:** a command's JSON body is `{ "envelope": CommandEnvelope, "input": ... }`. An upload is `multipart/form-data` with an `envelope` part (`application/json`) and a `file` part. A 2xx answer carries a `CommandOutcome` (202 for `pending`), and a stale revision answers 409 `conflict`. If API-F1.3 chooses another binding, only `command()` and `upload()` change.
2. **Desktop CORS for ranged reads:** the desktop scheme handler's preflight allows only `authorization, content-type` and exposes only `X-Journal-Next-Cursor`. Cross-origin ranged reads need `range` in the allowed headers and `Content-Range` (and `Accept-Ranges`) exposed (DE-F1.1, API-F1.3).
3. **Previews:** `<img>` and `<video src>` cannot send a bearer header, and UI-S-014 forbids tokens in URLs. API-F1.3 must say how a media element is authorised (for example a short-lived, scoped content handle) before UI-F2.2 uses native previews.

## Review round 1 (Cubic, 15 findings)

All 15 findings were valid and fixed in the second commit; the new tests were written first and failed first (9 failing, listed under the second failing run below).

- `newOperationId` no longer uses `crypto.randomUUID` (secure contexts only); it builds a version 4 UUID from `crypto.getRandomValues`.
- `conflictOutcome` refuses an empty operation id; `commandOutcome` rejects an answer without a non-empty operation id.
- `parseContentRange` rejects unsafe offsets and a complete length not greater than the last byte.
- `upload` keeps a `File`'s own name when no `filename` is given.
- A 2xx command answer that is not JSON is a `TypeError` (with the parser error as `cause`), like any other invalid answer.
- The `DELETE` command test asserts the method the transport saw.
- UI-S-012 names uploads and the no-range and open-ended range cases; the UI README documents `bytes=start-` and the full error contract; the plan row cites UI-S-014 and its feature; the feature adds a server-token precondition, an iOS example and an upload scenario; this heading carries the full requirement text.

## SRS traceability

| Constraint | Tests |
|---|---|
| Server: same-origin base URL and WebSocket events | `host-adapter.test.ts` "same-origin HTTP adapter" (5), "opens the WebSocket event channel"; e2e "the desktop launch token authorises ... the event channel" |
| Desktop: `filesquid://localhost` (macOS, iOS, Linux) and `http://filesquid.localhost` (Windows, Android) | `host-adapter.test.ts` (5), "detectHost on iOS"; `client.test.ts` `apiBaseUrl` (4); e2e "inside the desktop webview" |
| Desktop events through Tauri `filesquid-event`, no WebSocket | `host-adapter.test.ts` "listens to the Tauri event"; e2e "inside the desktop webview" |
| Envelope: schema version 1, non-empty fields, a retry keeps the id; ids work without a secure context | `commands.test.ts` "commandEnvelope" (6), "newOperationId" (2) |
| Outcome kinds, `PortError` only for `error`, same non-empty operation id; non-JSON 2xx is a `TypeError` | `commands.test.ts` "commandOutcome" (13), "refuses to build a conflict outcome for an empty operation id"; `client-io.test.ts` "command" (8) |
| Only `committed` is completion | `commands.test.ts` "treats only committed as durable"; `client-io.test.ts` "returns pending for a 202 answer" |
| HTTP 409 to `conflict`; other failures stay `ApiError` with their reason | `client-io.test.ts` "maps HTTP 409" (command and upload), "raises ApiError for a denial" |
| Range `bytes=start-end`, start >= 0, end >= start; refused before sending | `commands.test.ts` "rangeHeader" (9); `client-io.test.ts` "refuses an invalid range before sending anything" |
| 206 partial with `Content-Range` (safe offsets, size > end), 200 whole | `client-io.test.ts` "read" (3); `commands.test.ts` "parseContentRange" (14) |
| UI-S-014 on every new method (Bearer, unusable token refused, no token in errors) | `client-io.test.ts` "GETs JSON ... bearer", "sends no credential without a token", "refuses an unusable token before sending, without echoing it", header checks in read and upload |
| UI-S-011, UI-S-013: built bundle, no console errors | e2e suite (9 tests) against `npm run build` and `filesquid-server` |

Coverage: no coverage reporter is configured for the UI packages (none is approved in [ADR 017](../../../../../resources/architecture/017-test-runners-and-reporting.md) for Vitest); every new exported function and branch above has a direct test.

## TDD: failing run before the implementation

```text
     × uses the same-origin HTTP adapter and WebSocket events outside the desktop (windows) 4ms
     × uses the same-origin HTTP adapter and WebSocket events outside the desktop (android) 1ms
     × uses the same-origin HTTP adapter and WebSocket events outside the desktop (macos) 0ms
     × uses the same-origin HTTP adapter and WebSocket events outside the desktop (linux) 0ms
     × uses the same-origin HTTP adapter and WebSocket events outside the desktop (other) 0ms
     × uses filesquid://localhost and Tauri events on the macos desktop 0ms
     × uses filesquid://localhost and Tauri events on the linux desktop 0ms
     × uses filesquid://localhost and Tauri events on the other desktop 0ms
     × uses http://filesquid.localhost and Tauri events on the windows desktop 1ms
     × uses http://filesquid.localhost and Tauri events on the android desktop 0ms
     ✓ listens to the Tauri event filesquid-event on the desktop and opens no WebSocket 7ms
     ✓ opens the WebSocket event channel on the server host 1ms
 FAIL  tests/unit/client-io.test.ts [ tests/unit/client-io.test.ts ]
Error: Failed to resolve import "../../src/api/commands" from "tests/unit/client-io.test.ts". Does the file exist?
 FAIL  tests/unit/commands.test.ts [ tests/unit/commands.test.ts ]
Error: Failed to resolve import "../../src/api/commands" from "tests/unit/commands.test.ts". Does the file exist?
 Test Files  3 failed (3)
      Tests  10 failed | 2 passed (12)
```

## TDD: failing run for review round 1

```text
× works without crypto.randomUUID (plain HTTP is not a secure context) 7ms
     × rejects an empty operation id, even when both sides are empty 1ms
     × refuses to build a conflict outcome for an empty operation id 0ms
     × returns undefined for "bytes 0-9/9" 3ms
     × returns undefined for "bytes 0-9/5" 1ms
     × returns undefined for "bytes 0-9007199254740992/*" 1ms
     × returns undefined for "bytes 0-1/9007199254740993" 1ms
     × rejects a 2xx answer that is not JSON with a TypeError 10ms
     × keeps a File's own name when no filename is given 3ms
⎯⎯⎯⎯⎯⎯⎯ Failed Tests 9 ⎯⎯⎯⎯⎯⎯⎯
 Test Files  2 failed | 1 passed (3)
      Tests  9 failed | 70 passed (79)
```

## Lint, format and type checks (`apps/filesquid/ui`)

```text
$ npx prettier --check .
Checking formatting...
All matched files use Prettier code style!
$ npx eslint .
(no output, exit 0)
$ npx tsc --noEmit
(no output, exit 0)
```

## Vitest: new tests (`npx vitest run --reporter=verbose` on the three new files)

```text
RUN  v4.1.12 <apps/filesquid/ui>
 ✓ tests/unit/commands.test.ts > commandEnvelope > builds a schema version 1 envelope with the caller's scope and revision 10ms
 ✓ tests/unit/commands.test.ts > commandEnvelope > generates a fresh operation id when none is given 3ms
 ✓ tests/unit/commands.test.ts > commandEnvelope > keeps a supplied operation id, so a retry is the same idempotent operation 0ms
 ✓ tests/unit/commands.test.ts > commandEnvelope > refuses an empty scope (contract minLength 1) 1ms
 ✓ tests/unit/commands.test.ts > commandEnvelope > refuses an empty expected_revision (contract minLength 1) 0ms
 ✓ tests/unit/commands.test.ts > commandEnvelope > refuses an empty operation_id (contract minLength 1) 0ms
 ✓ tests/unit/commands.test.ts > newOperationId > returns a UUID from the platform's secure random source 1ms
 ✓ tests/unit/commands.test.ts > newOperationId > works without crypto.randomUUID (plain HTTP is not a secure context) 1ms
 ✓ tests/unit/commands.test.ts > commandOutcome > accepts kind committed with a null error 1ms
 ✓ tests/unit/commands.test.ts > commandOutcome > accepts kind pending with a null error 0ms
 ✓ tests/unit/commands.test.ts > commandOutcome > accepts kind conflict with a null error 0ms
 ✓ tests/unit/commands.test.ts > commandOutcome > accepts kind error with a PortError 0ms
 ✓ tests/unit/commands.test.ts > commandOutcome > drops fields the contract does not define 0ms
 ✓ tests/unit/commands.test.ts > commandOutcome > rejects a non-object 0ms
 ✓ tests/unit/commands.test.ts > commandOutcome > rejects an unknown kind 0ms
 ✓ tests/unit/commands.test.ts > commandOutcome > rejects kind error without a PortError 0ms
 ✓ tests/unit/commands.test.ts > commandOutcome > rejects a PortError with an empty code 0ms
 ✓ tests/unit/commands.test.ts > commandOutcome > rejects a PortError without retryable 0ms
 ✓ tests/unit/commands.test.ts > commandOutcome > rejects kind committed with an error 0ms
 ✓ tests/unit/commands.test.ts > commandOutcome > rejects an empty operation id, even when both sides are empty 0ms
 ✓ tests/unit/commands.test.ts > commandOutcome > rejects an outcome for another operation id 0ms
 ✓ tests/unit/commands.test.ts > conflictOutcome and isCommitted > builds a conflict outcome for the sent operation id 0ms
 ✓ tests/unit/commands.test.ts > conflictOutcome and isCommitted > refuses to build a conflict outcome for an empty operation id 0ms
 ✓ tests/unit/commands.test.ts > conflictOutcome and isCommitted > treats only committed as durable completion evidence 0ms
 ✓ tests/unit/commands.test.ts > rangeHeader > writes an inclusive byte range 0ms
 ✓ tests/unit/commands.test.ts > rangeHeader > writes an open-ended range 0ms
 ✓ tests/unit/commands.test.ts > rangeHeader > accepts a one-byte range (end equals start) 0ms
 ✓ tests/unit/commands.test.ts > rangeHeader > refuses {"start":-1} 0ms
 ✓ tests/unit/commands.test.ts > rangeHeader > refuses {"start":1.5} 0ms
 ✓ tests/unit/commands.test.ts > rangeHeader > refuses {"start":10,"end":9} 0ms
 ✓ tests/unit/commands.test.ts > rangeHeader > refuses {"start":0,"end":2.5} 0ms
 ✓ tests/unit/commands.test.ts > rangeHeader > refuses {"start":null} 0ms
 ✓ tests/unit/commands.test.ts > rangeHeader > refuses {"start":9007199254740992} 0ms
 ✓ tests/unit/commands.test.ts > parseContentRange > reads start, end and complete length 0ms
 ✓ tests/unit/commands.test.ts > parseContentRange > leaves size undefined when the length is unknown 0ms
 ✓ tests/unit/commands.test.ts > parseContentRange > accepts a last byte one below the complete length 1ms
 ✓ tests/unit/commands.test.ts > parseContentRange > returns undefined for null 0ms
 ✓ tests/unit/commands.test.ts > parseContentRange > returns undefined for "" 0ms
 ✓ tests/unit/commands.test.ts > parseContentRange > returns undefined for "bytes */1000" 0ms
 ✓ tests/unit/commands.test.ts > parseContentRange > returns undefined for "items 0-1/2" 0ms
 ✓ tests/unit/commands.test.ts > parseContentRange > returns undefined for "bytes 9-1/20" 0ms
 ✓ tests/unit/commands.test.ts > parseContentRange > returns undefined for "bytes 0-9/9" 0ms
 ✓ tests/unit/commands.test.ts > parseContentRange > returns undefined for "bytes 0-9/5" 0ms
 ✓ tests/unit/commands.test.ts > parseContentRange > returns undefined for "bytes 0-9007199254740992/*" 0ms
 ✓ tests/unit/commands.test.ts > parseContentRange > returns undefined for "bytes 0-1/9007199254740993" 0ms
 ✓ tests/unit/host-adapter.test.ts > selectHostAdapter > uses the same-origin HTTP adapter and WebSocket events outside the desktop (windows) 6ms
 ✓ tests/unit/host-adapter.test.ts > selectHostAdapter > uses the same-origin HTTP adapter and WebSocket events outside the desktop (android) 0ms
 ✓ tests/unit/host-adapter.test.ts > selectHostAdapter > uses the same-origin HTTP adapter and WebSocket events outside the desktop (macos) 0ms
 ✓ tests/unit/host-adapter.test.ts > selectHostAdapter > uses the same-origin HTTP adapter and WebSocket events outside the desktop (linux) 0ms
 ✓ tests/unit/host-adapter.test.ts > selectHostAdapter > uses the same-origin HTTP adapter and WebSocket events outside the desktop (other) 0ms
 ✓ tests/unit/host-adapter.test.ts > selectHostAdapter > uses filesquid://localhost and Tauri events on the macos desktop 0ms
 ✓ tests/unit/host-adapter.test.ts > selectHostAdapter > uses filesquid://localhost and Tauri events on the linux desktop 0ms
 ✓ tests/unit/host-adapter.test.ts > selectHostAdapter > uses filesquid://localhost and Tauri events on the other desktop 0ms
 ✓ tests/unit/host-adapter.test.ts > selectHostAdapter > uses http://filesquid.localhost and Tauri events on the windows desktop 3ms
 ✓ tests/unit/host-adapter.test.ts > selectHostAdapter > uses http://filesquid.localhost and Tauri events on the android desktop 0ms
 ✓ tests/unit/host-adapter.test.ts > detectHost on iOS > selects filesquid://localhost inside the iOS desktop webview 0ms
 ✓ tests/unit/host-adapter.test.ts > createHostClient event transport > listens to the Tauri event filesquid-event on the desktop and opens no WebSocket 58ms
 ✓ tests/unit/host-adapter.test.ts > createHostClient event transport > opens the WebSocket event channel on the server host 1ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.request > GETs JSON under the base URL with the bearer token 6ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.request > sends a JSON body with its content type and the chosen method 1ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.request > returns undefined for 204 No Content 3ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.request > passes the abort signal to the transport 1ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.request > raises ApiError with the contract's code and reason 1ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.request > sends no credential without a token 1ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.read > sends an inclusive Range and reports a partial answer with its Content-Range 1ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.read > reads the whole representation without a Range header 0ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.read > raises ApiError for 416 Range Not Satisfiable 3ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.read > refuses an invalid range before sending anything 1ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.command > POSTs the envelope and input as JSON and returns a committed outcome 1ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.command > returns pending for a 202 answer, which is not completion 1ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.command > rejects a 2xx answer that is not JSON with a TypeError 1ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.command > returns a module error with its PortError 0ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.command > maps HTTP 409 conflict to a conflict outcome for the sent operation 1ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.command > raises ApiError for a denial instead of an outcome 0ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.command > rejects an answer for another operation id 1ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.command > refuses an unusable token before sending, without echoing it 0ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.upload > POSTs multipart form data with an envelope part and a file part 4ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.upload > keeps a File's own name when no filename is given 1ms
 ✓ tests/unit/client-io.test.ts > FileSquidClient.upload > maps HTTP 409 conflict to a conflict outcome 0ms
 Test Files  3 passed (3)
      Tests  79 passed (79)
   Start at  05:24:50
   Duration  1.48s (transform 428ms, setup 0ms, import 535ms, tests 132ms, environment 2.84s)
```

## Vitest: `@filesquid/ui` (`npm test`)

```text
> @filesquid/ui@0.1.0 pretest
> rm -rf tests/allure-results


> @filesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 <apps/filesquid/ui>

 ✓ tests/unit/client-auth.test.ts (23 tests) 75ms
 ✓ tests/unit/client-io.test.ts (21 tests) 92ms
 ✓ tests/ui/monitor.test.ts (14 tests) 451ms
 ✓ tests/unit/commands.test.ts (45 tests) 96ms
 ✓ tests/ui/app-shell.test.ts (9 tests) 144ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 66ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 24ms
 ✓ tests/unit/theme.test.ts (6 tests) 19ms
 ✓ tests/unit/admin-client.test.ts (6 tests) 14ms
 ✓ tests/unit/client.test.ts (8 tests) 12ms
 ✓ tests/unit/contract.test.ts (1 test) 66ms

 Test Files  11 passed (11)
      Tests  148 passed (148)
   Start at  05:24:53
   Duration  5.86s (transform 738ms, setup 0ms, import 1.92s, tests 1.06s, environment 9.57s)

JUNIT report written to <apps/filesquid/ui>/tests/allure-results/vitest-junit.xml
```

## Playwright: `@filesquid/ui` (built UI and real `filesquid-server`, `FILESQUID_E2E=1 npx playwright test`)

```text
Running 9 tests using 1 worker
  ✓  1 [chromium] › tests/e2e/app.spec.ts:44:1 › shell mounts with FileSquid brand, internal vocabulary and live API health (259ms)
  ✓  2 [chromium] › tests/e2e/app.spec.ts:68:1 › design tokens switch between light and dark (181ms)
  ✓  3 [chromium] › tests/e2e/app.spec.ts:79:1 › client-side routes fall back to the distribution index (143ms)
  ✓  4 [chromium] › tests/e2e/app.spec.ts:85:1 › without a token the shell names the missing credentials from the label table (163ms)
  ✓  5 [chromium] › tests/e2e/app.spec.ts:97:1 › Monitor asks for the admin token, then shows live health, processes and the journal (356ms)
  ✓  6 [chromium] › tests/e2e/app.spec.ts:130:1 › a wrong token is named as invalid credentials (260ms)
  ✓  7 [chromium] › tests/e2e/app.spec.ts:141:1 › the signed-in token stays in memory: not in storage, URL or DOM, gone on reload (489ms)
  ✓  8 [chromium] › tests/e2e/app.spec.ts:172:1 › the desktop launch token authorises the shell, the Monitor and the event channel (192ms)
  ✓  9 [chromium] › tests/e2e/app.spec.ts:257:3 › inside the desktop webview (custom URI scheme adapter) › requests use the custom scheme and events use Tauri, not same-origin HTTP or WebSocket (278ms)
  9 passed (26.6s)
```

## Playwright: `@imagesquid/ui` (built UI and real `filesquid-server`)

```text
Running 4 tests using 1 worker
  ✓  1 [chromium] › tests/e2e/app.spec.ts:22:1 › ImageSquid brand, album vocabulary and FileSquid API health (236ms)
  ✓  2 [chromium] › tests/e2e/app.spec.ts:44:1 › ImageSquid tokens override FileSquid defaults in light and dark (135ms)
  ✓  3 [chromium] › tests/e2e/app.spec.ts:63:1 › ImageSquid names missing credentials in its own words (157ms)
  ✓  4 [chromium] › tests/e2e/app.spec.ts:72:1 › ImageSquid reaches the FileSquid Monitor with its own vocabulary (290ms)
  4 passed (4.7s)
```

## Mocked Environment Variables

- `FILESQUID_ADMIN_TOKEN` for the e2e servers is the existing test-only value in `tests/e2e/admin-token.ts`. It is not a secret.
- The new e2e test emulates the desktop webview: an init script defines `__TAURI_INTERNALS__` (records IPC calls, keeps event callbacks), `__TAURI_EVENT_PLUGIN_INTERNALS__` and `window.__FILESQUID_LAUNCH_TOKEN__`; the user agent is a Windows one; `page.route` serves `http://filesquid.localhost` from the real `filesquid-server` with the CORS answers `filesquid-desktop` gives its own pages. Validation in real webviews stays with ADR 030 S1 and the desktop host tasks.
