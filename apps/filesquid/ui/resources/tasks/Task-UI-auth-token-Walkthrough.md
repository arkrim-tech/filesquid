# Task UI-auth-token Walkthrough

## UI-S-014: FileSquidClient authenticates to the deny-by-default FileSquid API

Owner-approved fix after [API-F1.7](../../../crates/api/resources/plan.md) made every route except `GET /api/v1/health/live` require a credential. Not a plan row; no plan box is ticked. Requirements: [UI-S-014](../srs.md) (new), parents [API-S-004](../../../crates/api/resources/srs.md), [API-S-015](../../../crates/api/resources/srs.md), [SRS-F-056](../../../../../resources/srs.md#srs-f-056-single-filesquid-api), [ADR 030](../../../../../resources/architecture/030-filesquid-api-and-transports.md) Decision 5. Scenarios: [`client_authorisation_UI-S-014.feature`](../../tests/features/client_authorisation_UI-S-014.feature).

## Changes

- **`FileSquidClient`** ([`client.ts`](../../src/api/client.ts)): new `token` option (a string, or a function asked on every call) replaces the admin-only `adminToken`; `Authorization: Bearer <token>` goes on every request, not only `/api/v1/admin/*`. The event channel WebSocket offers `["filesquid.events.v1", "filesquid.bearer.<token>"]` (`eventProtocols`). A token outside the RFC 7230 `token` characters is refused before anything is sent (`ApiError` status 0, `invalid_credentials`), so it can never reach a `fetch` or `WebSocket` exception message.
- **`ApiError`** now carries the contract's `code`, `retryable` and `reason` (`DenialReason`), parsed from the response body and checked against the contract's enum. Its message names path, status and reason only: never the token and never the server's free text. New exported types `ErrorCode`, `DenialReason`, `ApiErrorBody` come from the generated `schema.d.ts`, which already matched `openapi.json` 0.5.0 (`npm run contract:types` produced no diff).
- **Token sources** ([`host.ts`](../../src/api/host.ts)): `createHostClient` prefers `window.__FILESQUID_LAUNCH_TOKEN__` (`launchToken`, `LAUNCH_TOKEN_GLOBAL`) and otherwise asks the supplied function. [`routes.ts`](../../src/app/routes.ts): `memoryTokenStore(onSet?)` replaces `sessionTokenStore`; the token is held in a closure only (no Web Storage, no URL).
- **Views**: [`denial.ts`](../../src/components/denial.ts) maps a reason to the new label keys `authMissingCredentials`, `authInvalidCredentials`, `authNotPermitted` (added to the FileSquid and ImageSquid label tables). `<fs-app-shell>` shows the reason label (`data-reason`) and gains `refreshHealth()`. `<fs-monitor>` shows the reason above its token form (missing: `role=status`; invalid or not permitted: `role=alert`), clears the field after submit, and treats a refused subscription like any other denial. The default apps pass the token store to the Monitor only outside the desktop webview and reload the shell's API status after sign-in.
- **E2E fixtures**: the e2e administrator token is now 64 hex characters (API-S-016 made the server refuse the old value), and Playwright waits on the public `/api/v1/health/live`.
- **Plan citations** (no box ticked): [plan](../plan.md) rows UI-F0.1 (owner of `FileSquidClient`) and UI-F2.6 (sign-in, which replaces the Monitor's interim token form) now cite UI-S-014, as the plan-traceability gate requires.
- **Docs**: new [`@filesquid/ui` README](../../README.md) (supplying the token on desktop and server, error reasons), [UI SRS](../srs.md) UI-S-014, root [README](../../../../../README.md) and [architecture](../../../../../resources/architecture.md) frontend diagram and client paragraph.

TDD: the tests above were written first and failed before the implementation (28 failing, excerpt):

```text
     × is sent on every route, admin and non-admin alike 16ms
     × accepts an operator-supplied token string as a constructor option 1ms
     × reads the token source on every request, so a token entered later is used 1ms
     ✓ is omitted when no token is known, so the API answers missing_credentials 1ms
     × reads the desktop per-launch token from the window global 1ms
     × prefers the launch token on the desktop and the supplied token elsewhere 2ms
     × keeps a token entered at run time in memory only, never in Web Storage 1ms
     × notifies the listener after a token is set, without passing the token 0ms
     × offers filesquid.events.v1 and filesquid.bearer.<token>, events first 1ms
     × offers only filesquid.events.v1 without a token 0ms
     × refuses an unusable token before opening a socket, without echoing it 1ms
     × HTTP 401 unauthorized carries reason missing_credentials in the typed error 3ms
     × HTTP 403 forbidden carries reason invalid_credentials in the typed error 1ms
     × HTTP 403 forbidden carries reason not_permitted in the typed error 2ms
     × leaves reason undefined for errors that are not denials 1ms
     × ignores a reason outside the contract's DenialReason enum 1ms
     ✓ never puts the token into the error, even when the server echoes it 1ms
     × refuses an unusable HTTP token before sending (status 0, invalid_credentials) 1ms
     × accepts RFC 7230 token characters and rejects anything else 1ms
     ✓ is registered as a custom element 3ms
     ✓ renders the distribution's product name, logo and labels in its shadow DOM 31ms
     ✓ shows the API health status once the client answers 12ms
     ✓ shows an alert when the API is unreachable 6ms
     × shows HTTP 401 unauthorized (missing_credentials) with its own label 15ms
     × shows HTTP 403 forbidden (invalid_credentials) with its own label 15ms
     × shows HTTP 403 forbidden (not_permitted) with its own label 6ms
     × reloads the API status on refreshHealth (after a token is entered) 7ms
     ✓ is registered and shows health with its reasons 42ms
     ✓ lists processes with state, progress, rate and ETA 15ms
     ✓ shows storage and queue facts from the health checks and metrics 16ms
     ✓ renders journal entries as text, never as markup 16ms
     ✓ applies live process, health and journal events without reloading 16ms
     ✓ asks for the admin token on 401 and retries with it 12ms
     × shows missing_credentials distinctly above the token form 10ms
     × shows invalid_credentials distinctly above the token form 3ms
     × shows not_permitted distinctly above the token form 3ms
     × shows the reason label, not a token form, without a token store (desktop) 4ms
     × shows a refused subscription as a denial instead of throwing 3ms
     × clears the typed token from the form after submitting 5ms
     ✓ keeps journal entries published while the first page loads, each once 11ms
     ✓ sets nothing up when removed before the first load finishes 32ms
     × send the token as a bearer header when one is set 12ms
     ✓ send no authorization header when no token is known 1ms
     ✓ encode journal filters as query parameters 1ms
     ✓ return the JSON Lines export as text 1ms
     ✓ follow the continuation cursor of a truncated export 1ms
     ✓ raise ApiError with 401 and 403 so the UI can ask for the token 2ms
 ✓ tests/unit/client.test.ts (8 tests) 10ms
 ✓ tests/unit/theme.test.ts (6 tests) 8ms
     ✓ mounts <fs-monitor> on #/monitor and removes it on other hashes 21ms
```

## Review round 1 (Cubic, 17 findings)

All 17 were valid and are fixed in one commit:

- **Shell race:** `<fs-app-shell>` numbers its health reads and discards an answer from an older read, so a slow pre-sign-in 401 cannot overwrite the signed-in status. Test: `app-shell.test.ts` "ignores a stale pre-sign-in answer", which fails without the fix.
- **Empty token:** an explicitly supplied empty token is unusable (status 0, `invalid_credentials`); only `undefined` means no token. `memoryTokenStore` returns undefined for an empty entry, so the signed-out flow still sends no credential and reads `missing_credentials`. Both are tested.
- **Error codes:** `errorBody` keeps a `code` only when it is in the contract's `ErrorCode` enum. The run-time lists are tied to the generated unions by `CONTRACT_LISTS_COMPLETE`, which fails `tsc` if a contract value is missing.
- **Non-JSON warning:** it now logs only the status and the parser error's kind, because V8's `SyntaxError` message quotes the body. A test checks that no body text is logged.
- **Tests:** stubs and spies are restored in `afterEach`, and the wrong-token e2e test now asserts no unexpected console errors.
- **Docs:** UI-S-014 says "when a token is known" and covers empty tokens. `DenialText` is exported. Mermaid edge labels no longer contain semicolons, and the architecture doc says WebSocket upgrades accept the header for clients that can set it. The README gives the per-platform token rules (Linux/macOS file vs Windows variable), the 64-hex format and blank-as-unset. The plan's UI-F0.1 Related Tests links the feature file, and the UI-F2.6 parenthetical is gone. The feature file asserts status 0, and the `routes.ts`, `fs-monitor.ts` and `client.ts` doc comments match the code.

## SRS traceability

| Constraint | Tests |
|---|---|
| Bearer header on every HTTP request | `client-auth.test.ts` "bearer header" (4), `admin-client.test.ts`; e2e "desktop launch token authorises the shell, the Monitor and the event channel" |
| Desktop launch token; operator token option | `client-auth.test.ts` "token sources"; e2e launch-token tests (FileSquid, ImageSquid) |
| Memory only; not in storage, URL, DOM or errors | `client-auth.test.ts` "keeps a token ... never in Web Storage", "never puts the token into the error"; `monitor.test.ts` "clears the typed token"; e2e "the signed-in token stays in memory" |
| WebSocket subprotocols | `client-auth.test.ts` "event channel subprotocols" (3); e2e upgrade succeeds with no console error |
| Typed reasons, shown distinctly | `client-auth.test.ts` "denial reasons" (5); `app-shell.test.ts` (3 reasons + refresh); `monitor.test.ts` (3 reasons, desktop alert, refused subscription); e2e missing and invalid credentials |
| Unusable token refused before sending | `client-auth.test.ts` "refuses an unusable ... token", `isUsableToken` |
| Contract types current | `contract.test.ts` |

Coverage: no coverage reporter is configured for the UI packages (none is approved in [ADR 017](../../../../../resources/architecture/017-test-runners-and-reporting.md) for Vitest); every new exported function and branch above has a direct test.

## Lint, format and type checks

```text
.prettierrc.json 31ms (unchanged)
eslint.config.js 16ms (unchanged)
index.html 35ms (unchanged)
package-lock.json 72ms (unchanged)
package.json 7ms (unchanged)
playwright.config.ts 75ms (unchanged)
README.md 68ms
src/api/client.ts 82ms (unchanged)
src/api/host.ts 8ms (unchanged)
src/app/main.ts 7ms (unchanged)
src/app/routes.ts 10ms (unchanged)
src/components/denial.ts 4ms (unchanged)
src/components/fs-app-shell.ts 20ms (unchanged)
src/components/fs-monitor.ts 121ms (unchanged)
src/distribution.ts 3ms (unchanged)
src/index.ts 4ms (unchanged)
src/theme/index.ts 1ms (unchanged)
src/theme/labels.en.json 2ms (unchanged)
src/theme/labels.ts 5ms (unchanged)
src/theme/tokens.css 36ms (unchanged)
src/theme/tokens.ts 7ms (unchanged)
tests/e2e/admin-token.ts 1ms (unchanged)
tests/e2e/app.spec.ts 25ms (unchanged)
tests/ui/app-shell.test.ts 15ms (unchanged)
tests/ui/monitor-route.test.ts 6ms (unchanged)
tests/ui/monitor.test.ts 30ms (unchanged)
tests/unit/admin-client.test.ts 11ms (unchanged)
tests/unit/client-auth.test.ts 37ms (unchanged)
tests/unit/client.test.ts 16ms (unchanged)
tests/unit/contract.test.ts 4ms (unchanged)
tests/unit/theme.test.ts 5ms (unchanged)
tsconfig.json 9ms (unchanged)
vite.config.ts 2ms (unchanged)
vitest.config.ts 2ms (unchanged)
Checking formatting...
All matched files use Prettier code style!
UI lint/tsc OK
```

```text
.prettierrc.json 34ms (unchanged)
distribution.json 5ms (unchanged)
eslint.config.js 15ms (unchanged)
index.html 47ms (unchanged)
package-lock.json 106ms (unchanged)
package.json 9ms (unchanged)
playwright.config.ts 73ms (unchanged)
src/distribution.ts 7ms (unchanged)
src/main.ts 16ms (unchanged)
src/theme/labels.en.json 14ms (unchanged)
src/theme/tokens.css 39ms (unchanged)
tauri.imagesquid.json 7ms (unchanged)
tests/e2e/admin-token.ts 7ms (unchanged)
tests/e2e/app.spec.ts 37ms (unchanged)
tests/ui/distribution.test.ts 12ms (unchanged)
tests/unit/theme.test.ts 9ms (unchanged)
tsconfig.json 1ms (unchanged)
vite.config.ts 4ms (unchanged)
vitest.config.ts 3ms (unchanged)
Checking formatting...
All matched files use Prettier code style!
IS lint/tsc OK
```

## Vitest: `@filesquid/ui` (`npm test`)

```text
> @filesquid/ui@0.1.0 pretest
> rm -rf tests/allure-results
> @filesquid/ui@0.1.0 test
> vitest run
 RUN  v4.1.12 apps/filesquid/ui
 ✓ tests/unit/client-auth.test.ts (23 tests) 28ms
 ✓ tests/ui/app-shell.test.ts (9 tests) 92ms
 ✓ tests/ui/monitor.test.ts (14 tests) 193ms
 ✓ tests/unit/client.test.ts (8 tests) 12ms
 ✓ tests/unit/admin-client.test.ts (6 tests) 14ms
 ✓ tests/unit/theme.test.ts (6 tests) 7ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 27ms
 ✓ tests/unit/contract.test.ts (1 test) 58ms
 Test Files  8 passed (8)
      Tests  69 passed (69)
   Start at  04:39:48
   Duration  2.73s (transform 471ms, setup 0ms, import 1.25s, tests 429ms, environment 3.82s)
JUNIT report written to tests/allure-results/vitest-junit.xml
```

## Vitest: `@imagesquid/ui` (`npm test`)

```text
> @imagesquid/ui@0.1.0 pretest
> rm -rf tests/allure-results
> @imagesquid/ui@0.1.0 test
> vitest run
 RUN  v4.1.12 apps/imagesquid
 ✓ tests/unit/theme.test.ts (4 tests) 7ms
 ✓ tests/ui/distribution.test.ts (1 test) 34ms
 Test Files  2 passed (2)
      Tests  5 passed (5)
   Start at  04:23:50
   Duration  1.16s (transform 281ms, setup 0ms, import 336ms, tests 41ms, environment 698ms)
JUNIT report written to tests/allure-results/vitest-junit.xml
```

## Playwright: `@filesquid/ui` (built UI + real `filesquid-server`, `npx playwright test`)

```text
Running 8 tests using 1 worker
  ✓  1 [chromium] › tests/e2e/app.spec.ts:44:1 › shell mounts with FileSquid brand, internal vocabulary and live API health (274ms)
  ✓  2 [chromium] › tests/e2e/app.spec.ts:68:1 › design tokens switch between light and dark (141ms)
  ✓  3 [chromium] › tests/e2e/app.spec.ts:79:1 › client-side routes fall back to the distribution index (152ms)
  ✓  4 [chromium] › tests/e2e/app.spec.ts:85:1 › without a token the shell names the missing credentials from the label table (172ms)
  ✓  5 [chromium] › tests/e2e/app.spec.ts:97:1 › Monitor asks for the admin token, then shows live health, processes and the journal (260ms)
  ✓  6 [chromium] › tests/e2e/app.spec.ts:130:1 › a wrong token is named as invalid credentials (236ms)
  ✓  7 [chromium] › tests/e2e/app.spec.ts:141:1 › the signed-in token stays in memory: not in storage, URL or DOM, gone on reload (402ms)
  ✓  8 [chromium] › tests/e2e/app.spec.ts:172:1 › the desktop launch token authorises the shell, the Monitor and the event channel (201ms)
  8 passed (7.0s)
```

## Playwright: `@imagesquid/ui` (built UI + real `filesquid-server`)

```text
Running 4 tests using 1 worker
  ✓  1 [chromium] › tests/e2e/app.spec.ts:22:1 › ImageSquid brand, album vocabulary and FileSquid API health (169ms)
  ✓  2 [chromium] › tests/e2e/app.spec.ts:44:1 › ImageSquid tokens override FileSquid defaults in light and dark (144ms)
  ✓  3 [chromium] › tests/e2e/app.spec.ts:63:1 › ImageSquid names missing credentials in its own words (163ms)
  ✓  4 [chromium] › tests/e2e/app.spec.ts:72:1 › ImageSquid reaches the FileSquid Monitor with its own vocabulary (332ms)
  4 passed (4.7s)
exit 0
```

## Mocked Environment Variables

- `FILESQUID_ADMIN_TOKEN` for the e2e servers is the test-only value in `tests/e2e/admin-token.ts` (`"e2e0"` repeated 16 times, 64 hex characters). It is not a secret and is never used outside the e2e runs.
- The desktop launch token is simulated in the browser e2e tests by defining `window.__FILESQUID_LAUNCH_TOKEN__` with an init script, as `filesquid-desktop` does for its own pages.
