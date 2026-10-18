# Task UI-F2.1b Walkthrough, part 1: host privacy indicator

[Task UI-F2.1b](../plan.md) (Host privacy indicator and live navigation data) closes only when both parts are done. This walkthrough covers **part 1 only**, the host privacy indicator, which the plan lets land as soon as API-F1.10 is done (it is merged). **Part 2 remains**: the live navigation source (`GET /api/v1/navigation`, API-S-021, waiting for API-F1.11), cursor paging through the generated contract types, the breadcrumb from `ancestors`, and refresh on `navigation.change`. The Task UI-F2.1b box in [plan.md](../plan.md) and in the repository checklist stays unticked, and part 2 adds its own walkthrough.

Built so part 2 needs no rework: the indicator depends only on a two-method `HostClient` (`host()`, `subscribe(listener, onOpen?)`), the pure wording rule `describeHost` is separate from the element, and `FileSquidClient` gained `host()` beside the existing routes. Part 2 adds `navigation()` the same way and slots `<fs-nav-tree>` into the shell's existing `navigation` slot; nothing of the indicator changes.

## SRS mapping

| ID | Title | Covered by |
|---|---|---|
| [SRS-F-057](../../../../../resources/srs.md#srs-f-057-desktop-and-server-hosts) | Desktop and server hosts (each host states its privacy boundary in the UI) | [`host_privacy_indicator_UI-S-018.feature`](../../tests/features/host_privacy_indicator_UI-S-018.feature), `tests/unit/host-descriptor.test.ts`, `tests/ui/host-indicator.test.ts`, `tests/e2e/preview/host-indicator.spec.ts` |
| [SRS-F-059](../../../../../resources/srs.md#srs-f-059-hosted-encrypted-backup-and-sync-service) | Hosted encrypted backup and sync service (shown apart from the host, ciphertext only) | the same tests (enrolled backup and sync apart, not-enrolled state, sync never shown as backup) |
| [UI-S-004](../srs.md) | The UI states which host it is connected to and who can read files there | `tests/ui/host-indicator.test.ts` (desktop, server with and without operator, browser), `tests/e2e/preview/host-indicator.spec.ts` |
| [UI-S-018](../srs.md) | The host privacy indicator takes its facts only from the host descriptor | `tests/unit/host-client.test.ts` (`GET /api/v1/host`, `onOpen`), `tests/ui/host-indicator.test.ts` (start read, `host.change`, `events.resync`, reconnect, loading, error, contract check), `tests/ui/app-shell.test.ts` (in the header, re-read after sign-in), preview e2e (`host.change` without reload, resync) |
| [UI-S-001](../srs.md) | Components styled only through design tokens and worded only through the label table | `tests/unit/component-styles.test.ts` (covers the new element), every statement asserted against `labels.en.json` |
| [UI-S-005](../srs.md) | A distribution's label table defines every key of the default table | the 15 new `host*` keys are in `labels.ts` `REQUIRED_LABEL_KEYS` and in ImageSquid's table (`apps/imagesquid/tests/unit/theme.test.ts`) |
| [UI-S-013](../srs.md) | UI tests against production artifacts with strict DOM assertions | `tests/e2e/preview/host-indicator.spec.ts` against `vite build` plus `vite preview`: element defined, labels from the table, computed tokens in light and dark, no console errors |
| [UI-S-016](../srs.md) | Accessibility contracts | indicator is one named, focusable `role="group"` with a polite live region and the shared focus ring (unit audit and preview e2e keyboard test) |

Constraint gates (from UI-S-018 and ADR 032 Decision 6): `GET /api/v1/host` read at start; update on every `host.change` (whole descriptor); read again after `events.resync` or a reconnect; loading state until it arrives; error state when unreadable; never a guessed host (a payload that breaks the contract is rejected); `server` shows `operator_name` or the unnamed-operator label plus the at-rest statement; not enrolled shows neither backup nor sync on; enrolled shows Backup and Sync each On or Off, `service_host` and the ciphertext statement, never sync as backup; desktop with backup or sync on adds the on-device-encryption sentence; visible on every view (shell header); keyboard and screen reader reachable. Each is asserted in the tests listed above.

## What changed

- **`<fs-host-indicator>`** ([`src/components/fs-host-indicator.ts`](../../src/components/fs-host-indicator.ts)): takes `{ labels, client }`; named focusable group, polite live region, `<fs-states>` for loading, error, offline and denied (Retry re-reads and does not bubble to other views), host statements first and the hosted service apart. Newest read wins; a slow older answer is dropped. Exposes `data-state` and `data-host-type` for tests and styling hooks.
- **Host model** ([`src/host/descriptor.ts`](../../src/host/descriptor.ts)): `parseHostDescriptor` (contract check: schema version, host type, operator name 1 to 256 characters without controls, service flags agreeing with `enrolled`) and `describeHost` (pure wording from the label table; the operator name is inserted literally and rendered as text).
- **Client** ([`src/api/client.ts`](../../src/api/client.ts)): `host()` and an optional `onOpen` on `subscribe()`, called each time the WebSocket opens.
- **`<fs-app-shell>`**: mounts the indicator in the header (own row); `ShellClient` now extends `HostClient`; `refreshHealth()` also refreshes the indicator, so a token entered later reopens the event channel and reads the descriptor again.
- **Labels** (default and ImageSquid): `hostIndicator`, `hostDesktop`, `hostDesktopBackup`, `hostServer`, `hostOperatorUnnamed`, `hostServerAtRest`, `hostBrowser`, `hostServiceTitle`, `hostNotEnrolled`, `hostBackup`, `hostSync`, `hostOn`, `hostOff`, `hostServiceAt`, `hostServiceCiphertext`. The Decision 6 sentences are verbatim; the at-rest line is new wording for the SRS requirement.
- **Existing tests updated**: shell fakes gain the host half (`tests/support/host-fakes.ts`); the static preview suites (FileSquid and ImageSquid) answer `/api/v1/host` and the event WebSocket; the server-backed desktop-webview test expects two Tauri listeners (indicator and Monitor) and emits to every listener.
- **Docs**: package [README](../../README.md), [plan.md](../plan.md) status line, two new scenarios in [`host_privacy_indicator_UI-S-018.feature`](../../tests/features/host_privacy_indicator_UI-S-018.feature).

## Decisions and gaps for the platform engineer

- The browser client does not reconnect the event channel by itself. The indicator reads again on every `onOpen`, so it is correct whenever a reconnect exists (unit-tested with a socket that opens twice); the reconnect itself is a client or platform task. The reopen read also runs on the first open, so there are two descriptor reads at start on a server host (it closes the window in which an event sent before the channel opened would be lost).
- On a `server` host with no token the descriptor read is refused (`host.read` needs an accepted credential), so the indicator shows the denied state until the Monitor sign-in supplies a token; `refreshHealth()` then re-reads. No API change is needed.
- The server-backed Playwright suite (`playwright.config.ts`, needs `filesquid-server` built by Cargo) was updated for the new listener and `/api/v1/host` call but not run in this task, because this task builds no Rust.

## How it was tested

TDD: the new unit and UI tests were written first and the first run failed (module missing):

```
 Test Files  3 failed (3)
      Tests  27 failed | 1 passed (28)
```

The shell integration tests were added after the element passed. Commands, per package: `npx prettier --write .`, `npx eslint . --fix`, `npx tsc --noEmit`, `npm test`, and `FILESQUID_E2E=1 npx playwright test -c playwright.preview.config.ts` (the production build served by `vite preview`; the sandbox's Chromium build was linked into a temporary Playwright browser path). Head-commit hook: see the pull request.

## Raw test output

### Lint and types (apps/filesquid/ui)

```
eslint: clean
tsc: clean
```

### npm test (apps/filesquid/ui)

```
> @filesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @filesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 apps/filesquid/ui

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the error state and no host when the descriptor cannot be read
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at apps/filesquid/ui/tests/ui/host-indicator.test.ts:205:46
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the denied state for an authorisation denial
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 401 (missing_credentials)
    at apps/filesquid/ui/tests/ui/host-indicator.test.ts:214:20
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 401,
  path: '/api/v1/host',
  code: 'unauthorized',
  reason: 'missing_credentials',
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > reads again when Retry is pressed and recovers
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at apps/filesquid/ui/tests/ui/host-indicator.test.ts:232:29
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> follows the host > reads again when a host.change payload breaks the contract
FileSquid host.change payload broke the contract; reading the descriptor

 ✓ tests/ui/host-indicator.test.ts (24 tests) 184ms
 ✓ tests/unit/component-styles.test.ts (88 tests) 55ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 588ms
 ✓ tests/ui/nav-tree.test.ts (26 tests) 854ms
 ✓ tests/unit/client-io.test.ts (22 tests) 58ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 56ms
 ✓ tests/unit/theme.test.ts (26 tests) 28ms
 ✓ tests/ui/monitor.test.ts (14 tests) 220ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 50ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 33ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 133ms
 ✓ tests/ui/app-shell.test.ts (18 tests) 415ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 34ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 30ms
 ✓ tests/unit/commands.test.ts (45 tests) 43ms
 ✓ tests/unit/focus.test.ts (16 tests) 74ms
 ✓ tests/unit/host-descriptor.test.ts (25 tests) 20ms
 ✓ tests/ui/states.test.ts (16 tests) 114ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 16ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 74ms
 ✓ tests/unit/test-scripts.test.ts (10 tests) 898ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 78ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 24ms
 ✓ tests/unit/admin-client.test.ts (6 tests) 20ms
 ✓ tests/unit/client.test.ts (8 tests) 17ms
 ✓ tests/unit/preferences.test.ts (10 tests) 18ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 96ms
 ✓ tests/unit/host-client.test.ts (4 tests) 16ms
 ✓ tests/unit/view-state.test.ts (4 tests) 15ms
 ✓ tests/unit/contrast.test.ts (13 tests) 28ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 34ms
 ✓ tests/unit/distribution.test.ts (3 tests) 10ms
 ✓ tests/unit/contract.test.ts (1 test) 70ms

 Test Files  33 passed (33)
      Tests  579 passed (579)
   Start at  23:23:06
   Duration  11.20s (transform 1.97s, setup 1.45s, import 3.44s, tests 4.40s, environment 16.77s)

JUNIT report written to apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

### Playwright on the production build (apps/filesquid/ui)

```
Running 16 tests using 2 workers

  ✓   1 [chromium] › tests/e2e/preview/host-indicator.spec.ts:86:1 › the indicator is a defined custom element in the shell header, named from the label table (367ms)
  ✓   2 [chromium] › tests/e2e/preview/production-build.spec.ts:56:1 › the built page is static, mounts the shell and defines every custom element (444ms)
  ✓   3 [chromium] › tests/e2e/preview/host-indicator.spec.ts:100:1 › the desktop host says only you can read your files, and shows the not-enrolled state (258ms)
  ✓   4 [chromium] › tests/e2e/preview/production-build.spec.ts:73:1 › navigation, API status and theme legend words come from the active label table (243ms)
  ✓   5 [chromium] › tests/e2e/preview/host-indicator.spec.ts:114:1 › a server names its operator, who can read the files, and states the at-rest duty (208ms)
  ✓   6 [chromium] › tests/e2e/preview/production-build.spec.ts:91:1 › computed token values are exact in light and dark (216ms)
  ✓   7 [chromium] › tests/e2e/preview/host-indicator.spec.ts:127:1 › a server without an operator name uses the unnamed-operator label (198ms)
  ✓   8 [chromium] › tests/e2e/preview/production-build.spec.ts:107:1 › a refused health call shows the unavailable alert, not a console error from the app (187ms)
  ✓   9 [chromium] › tests/e2e/preview/host-indicator.spec.ts:137:1 › the browser host says files are decrypted only in this browser (192ms)
  ✓  10 [chromium] › tests/e2e/preview/host-indicator.spec.ts:150:1 › enrolled backup and sync are shown apart from the host, as ciphertext only (161ms)
  ✓  11 [chromium] › tests/e2e/preview/host-indicator.spec.ts:175:1 › a host.change event updates the indicator without a reload or another read (148ms)
  ✓  12 [chromium] › tests/e2e/preview/host-indicator.spec.ts:197:1 › events.resync makes the indicator read the descriptor again (188ms)
  ✓  13 [chromium] › tests/e2e/preview/host-indicator.spec.ts:210:1 › loading shows no host until the descriptor arrives (960ms)
  ✓  14 [chromium] › tests/e2e/preview/host-indicator.spec.ts:222:1 › a failed read shows the error state and never a guessed host, then Retry recovers (231ms)
  ✓  15 [chromium] › tests/e2e/preview/host-indicator.spec.ts:240:1 › computed token values are exact in light and dark (179ms)
  ✓  16 [chromium] › tests/e2e/preview/host-indicator.spec.ts:265:1 › the indicator is reachable by keyboard and has a visible focus outline (211ms)

  16 passed (9.4s)
```

### npm test (apps/imagesquid)

```
> @imagesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @imagesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 apps/imagesquid

 ✓ tests/unit/test-reporting.test.ts (12 tests) 17ms
 ✓ tests/unit/theme.test.ts (10 tests) 21ms
 ✓ tests/ui/distribution.test.ts (5 tests) 201ms
 ✓ tests/unit/test-scripts.test.ts (10 tests) 898ms

 Test Files  4 passed (4)
      Tests  37 passed (37)
   Start at  23:22:18
   Duration  1.83s (transform 628ms, setup 248ms, import 685ms, tests 1.14s, environment 1.99s)

JUNIT report written to apps/imagesquid/tests/allure-results/vitest-junit.xml
```

### Playwright on the production build (apps/imagesquid)

```
Running 4 tests using 1 worker

  ✓  1 [chromium] › tests/e2e/preview/production-build.spec.ts:54:1 › the built page is static, mounts the shell and defines every custom element (236ms)
  ✓  2 [chromium] › tests/e2e/preview/production-build.spec.ts:71:1 › navigation, API status and theme legend words come from the active label table (176ms)
  ✓  3 [chromium] › tests/e2e/preview/production-build.spec.ts:89:1 › computed ImageSquid token values are exact in light and dark (179ms)
  ✓  4 [chromium] › tests/e2e/preview/production-build.spec.ts:105:1 › a refused health call shows the unavailable alert (150ms)

  4 passed (5.7s)
```
