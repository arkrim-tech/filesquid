# Task UI-0.6 Walkthrough: Playwright production suite and Allure

Global Task 0.8, UI half. Branch `test/ui-0.6-playwright-allure`. UI-only: no Rust crate was changed. TDD-ENFORCE applies: `tests/unit/test-reporting.test.ts` and the preview spec were written first and the reporting test failed (7 of 10 failed) before the configs existed.

## What changed

- **Allure**: `allure-vitest`, `allure-playwright` and the Node-only `allure` CLI (3.21) are dev dependencies of `@filesquid/ui` and `@imagesquid/ui`. Vitest writes Allure raw results to `tests/allure-results/vitest/` and JUnit to `vitest-junit.xml`; Playwright writes Allure raw results to `tests/allure-results/playwright/` and JUnit beside them. `npm test` clears all of `tests/allure-results/` (`pretest`); `npm run test:e2e` clears only the Playwright results (`pretest:e2e`), so an end-to-end run preserves existing unit results, but a later unit run removes the end-to-end results (the hook runs `npm test` first). Both cleanups are Node scripts (`tests/support/clean-results.mjs`), so they work on Windows too. `npm run report` builds the HTML report into the untracked `tests/allure-report/` (ignored by Git, Prettier and ESLint).
- **Production suite**: `playwright.preview.config.ts` runs `npm run build` and `vite preview --strictPort` on the fixed preview port (4173 FileSquid, 4174 ImageSquid), and `tests/e2e/preview/production-build.spec.ts` proves the static artifact with no server: scripts are content-hashed files under `/assets/` (the spec requires a hash-shaped file name) and none is inline, all six custom elements are defined, the navigation, status and theme words equal the label table, computed `--fs-primary`, `--fs-on-primary` and body background are exact in light and dark, and a refused health call shows the alert with no other console error. The server-backed suite stays in `playwright.config.ts` (it ignores `preview/`); `npm run test:e2e` (`tests/support/run-e2e.mjs`) runs both in turn, so they never hold the same port together, and fails if either fails.
- **Shared strict-DOM helper**: `tests/e2e/support/strict-dom.ts` (`trackErrors`, `activeLabels`, `label`, `expectElementsDefined`, `tokenColour`, `bodyBackground`, `rgbOf`, `setScheme`). The private copies in the existing FileSquid and ImageSquid specs were removed in its favour, so later UI tasks reuse one helper.
- **Docs**: root `README.md` and `apps/filesquid/ui/README.md` list the commands, the BDD location and the report commands (`npm run report`, `npx allure open tests/allure-report`, both verified).
- Plan row UI-0.6 and the UI-0.6 checklist box are ticked. The global Task 0.8 box stays open: its capability register, foundation evidence and documentation parts, and the server half (BE-0.6, moved to Task FS-3.10), are outside this row.

## SRS mapping

| Requirement | Evidence |
| --- | --- |
| UI-S-013 UI tests run against production artifacts (`vite build` plus `vite preview`) with strict DOM assertions: custom elements defined, labels from the active label table, computed token values in light and dark, no console errors. | [`production_test_suite_UI-S-013.feature`](../../tests/features/production_test_suite_UI-S-013.feature); `tests/e2e/preview/production-build.spec.ts` (both packages); `tests/e2e/support/strict-dom.ts`; `tests/unit/test-reporting.test.ts` |
| NFR-008 Maintainability and verified engineering (PRD) | Allure and JUnit under each app's `tests/allure-results/`, cleared before each run, plus a generated report |
| ADR 017 Test runners and reporting (cargo test, Vitest, Playwright and Allure under each app) | Same; results stay in each app's own directory |

Constraint gates: results cleared before each run (`test-reporting.test.ts`), runner results kept apart (sub-folders), report generation (`npm run report`, `tests/allure-report/index.html` exists in both packages), ports 4173 and 4174 unchanged and `--strictPort`, FileSquid tokens (light `#0b7a32` / `#ffffff` / `#ffffff`, dark `#16ac49` / `#0d2b18` / `#141414`) and ImageSquid tokens (light `#005db6` / `#ffffff`, dark `#a9c7ff` / `#003063`, dark surface `#121418`) asserted in the built page.

## Mocked environment variables

None. The preview spec answers `GET /api/v1/health` itself with `{"schema_version":1,"status":"ok","product":"filesquid"}`, so the static build is tested without a server; the server-backed specs still use the real `filesquid-server` and its `ADMIN_TOKEN` test constant (unchanged).

## Test output (raw)

Run with `TMPDIR=/dev/shm/ui-0.6`. Prettier, ESLint and `tsc --noEmit` printed nothing (clean). Each Cubic finding on the first push was fixed in a second commit; the output below is from the final head.

### `@filesquid/ui`: `npm test` (tail)

```text
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 95ms
 ✓ tests/unit/view-state.test.ts (4 tests) 14ms
 ✓ tests/unit/test-scripts.test.ts (5 tests) 249ms
 ✓ tests/unit/contrast.test.ts (13 tests) 18ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 39ms
 ✓ tests/unit/distribution.test.ts (3 tests) 8ms
 ✓ tests/unit/contract.test.ts (1 test) 66ms
 Test Files  30 passed (30)
      Tests  512 passed (512)
   Start at  19:31:56
   Duration  10.92s (transform 1.41s, setup 1.34s, import 2.91s, tests 3.71s, environment 16.96s)
JUNIT report written to apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

### `@filesquid/ui`: `FILESQUID_E2E=1 npm run test:e2e` (server-backed suite, then the vite preview suite)

```text
> @filesquid/ui@0.1.0 pretest:e2e
> node tests/support/clean-results.mjs tests/allure-results/playwright tests/allure-results/playwright-junit.xml tests/allure-results/playwright-preview-junit.xml
> @filesquid/ui@0.1.0 test:e2e
> node tests/support/run-e2e.mjs
Running 40 tests using 2 workers
  ✓   1 [chromium] › tests/e2e/app.spec.ts:35:1 › shell mounts with FileSquid brand, internal vocabulary and live API health (298ms)
  ✓   2 [chromium] › tests/e2e/accessibility.spec.ts:67:1 › landmarks, headings and control names come from the label table (277ms)
  ✓   3 [chromium] › tests/e2e/app.spec.ts:59:1 › design tokens switch between light and dark (206ms)
  ✓   4 [chromium] › tests/e2e/accessibility.spec.ts:101:3 › Tab order and a 2px primary focus ring in the light theme (204ms)
  ✓   5 [chromium] › tests/e2e/app.spec.ts:76:1 › client-side routes fall back to the distribution index (197ms)
  ✓   6 [chromium] › tests/e2e/accessibility.spec.ts:101:3 › Tab order and a 2px primary focus ring in the dark theme (205ms)
  ✓   7 [chromium] › tests/e2e/app.spec.ts:82:1 › without a token the shell names the missing credentials from the label table (161ms)
  ✓   8 [chromium] › tests/e2e/accessibility.spec.ts:126:1 › light-DOM content slotted into the shell gets the same focus ring (213ms)
  ✓  10 [chromium] › tests/e2e/accessibility.spec.ts:146:1 › the skip control appears on focus and moves focus to main without routing (197ms)
  ✓   9 [chromium] › tests/e2e/app.spec.ts:94:1 › Monitor asks for the admin token, then shows live health, processes and the journal (371ms)
  ✓  11 [chromium] › tests/e2e/accessibility.spec.ts:164:1 › opening the Monitor from the keyboard moves focus to main (243ms)
  ✓  12 [chromium] › tests/e2e/app.spec.ts:127:1 › a wrong token is named as invalid credentials (249ms)
  ✓  13 [chromium] › tests/e2e/accessibility.spec.ts:182:1 › a keyboard sign-in to the Monitor keeps focus in the Monitor (384ms)
  ✓  14 [chromium] › tests/e2e/app.spec.ts:138:1 › the signed-in token stays in memory: not in storage, URL or DOM, gone on reload (489ms)
  ✓  15 [chromium] › tests/e2e/accessibility.spec.ts:199:1 › reduced motion stops every transition (231ms)
  ✓  16 [chromium] › tests/e2e/app.spec.ts:169:1 › the desktop launch token authorises the shell, the Monitor and the event channel (237ms)
  ✓  18 [chromium] › tests/e2e/app.spec.ts:254:3 › inside the desktop webview (custom URI scheme adapter) › requests use the custom scheme and events use Tauri, not same-origin HTTP or WebSocket (197ms)
  ✓  17 [chromium] › tests/e2e/accessibility.spec.ts:238:1 › more contrast strengthens borders; forced colours keep the chosen option (515ms)
  ✓  19 [chromium] › tests/e2e/navigation.spec.ts:183:1 › the elements are defined and worded from the label table inside the shell (224ms)
  ✓  20 [chromium] › tests/e2e/accessibility.spec.ts:281:3 › no horizontal scroll at 320 px wide and 100 % text (197ms)
  ✓  21 [chromium] › tests/e2e/navigation.spec.ts:229:1 › the tree follows the ARIA treeview keys and reads children as nodes open (260ms)
  ✓  22 [chromium] › tests/e2e/accessibility.spec.ts:281:3 › no horizontal scroll at 320 px wide and 200 % text (249ms)
  ✓  24 [chromium] › tests/e2e/accessibility.spec.ts:281:3 › no horizontal scroll at 1280 px wide and 200 % text (187ms)
  ✓  23 [chromium] › tests/e2e/navigation.spec.ts:265:1 › pins, section order and library collapse are kept on the device across a reload (420ms)
  ✓  25 [chromium] › tests/e2e/accessibility.spec.ts:281:3 › no horizontal scroll at 640 px wide and 200 % text (267ms)
  ✓  26 [chromium] › tests/e2e/navigation.spec.ts:306:1 › refused device storage keeps the choices for this page and says why (228ms)
  ✓  27 [chromium] › tests/e2e/accessibility.spec.ts:304:1 › a right-to-left locale mirrors the header and navigation (186ms)
  ✓  28 [chromium] › tests/e2e/navigation.spec.ts:331:1 › the tree shows the error and offline states and reads again on Retry (266ms)
  ✓  29 [chromium] › tests/e2e/theme.spec.ts:62:1 › official FileSquid tokens apply in light and dark and meet WCAG 2.2 AA (254ms)
  ✓  30 [chromium] › tests/e2e/navigation.spec.ts:353:3 › the five states are distinct and use token colours in the light theme (196ms)
  ✓  31 [chromium] › tests/e2e/theme.spec.ts:86:1 › the official mark fills the brand slot, with its dark-surface variant in dark (239ms)
  ✓  32 [chromium] › tests/e2e/navigation.spec.ts:396:3 › tree controls draw the 2px primary focus ring in the light theme (237ms)
  ✓  34 [chromium] › tests/e2e/navigation.spec.ts:353:3 › the five states are distinct and use token colours in the dark theme (183ms)
  ✓  33 [chromium] › tests/e2e/theme.spec.ts:112:1 › the theme follows the OS by default and the control's choice persists across a reload (384ms)
  ✓  35 [chromium] › tests/e2e/navigation.spec.ts:396:3 › tree controls draw the 2px primary focus ring in the dark theme (227ms)
  ✓  36 [chromium] › tests/e2e/theme.spec.ts:156:1 › the selected theme option and the API status meet AA as rendered (255ms)
  ✓  37 [chromium] › tests/e2e/navigation.spec.ts:419:1 › tree motion follows the token, stops under reduced motion, and forced colours mark the current item (209ms)
  ✓  38 [chromium] › tests/e2e/navigation.spec.ts:460:3 › the tree and breadcrumb reflow at 320 px and 100 % text (174ms)
  ✓  39 [chromium] › tests/e2e/navigation.spec.ts:460:3 › the tree and breadcrumb reflow at 320 px and 200 % text (224ms)
  ✓  40 [chromium] › tests/e2e/navigation.spec.ts:460:3 › the tree and breadcrumb reflow at 1280 px and 200 % text (182ms)
  40 passed (11.9s)
Running 4 tests using 1 worker
  ✓  1 [chromium] › tests/e2e/preview/production-build.spec.ts:38:1 › the built page is static, mounts the shell and defines every custom element (353ms)
  ✓  2 [chromium] › tests/e2e/preview/production-build.spec.ts:55:1 › navigation, API status and theme legend words come from the active label table (182ms)
  ✓  3 [chromium] › tests/e2e/preview/production-build.spec.ts:73:1 › computed token values are exact in light and dark (201ms)
  ✓  4 [chromium] › tests/e2e/preview/production-build.spec.ts:89:1 › a refused health call shows the unavailable alert, not a console error from the app (182ms)
  4 passed (8.2s)
```

### `@imagesquid/ui`: `npm test` (tail)

```text
> @imagesquid/ui@0.1.0 test
> vitest run
 RUN  v4.1.12 apps/imagesquid
 ✓ tests/unit/theme.test.ts (10 tests) 31ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 15ms
 ✓ tests/ui/distribution.test.ts (5 tests) 192ms
 ✓ tests/unit/test-scripts.test.ts (5 tests) 266ms
 Test Files  4 passed (4)
      Tests  32 passed (32)
   Start at  19:31:31
   Duration  1.82s (transform 707ms, setup 258ms, import 747ms, tests 505ms, environment 2.22s)
JUNIT report written to apps/imagesquid/tests/allure-results/vitest-junit.xml
```

### `@imagesquid/ui`: `FILESQUID_E2E=1 npm run test:e2e`

```text
> @imagesquid/ui@0.1.0 pretest:e2e
> node tests/support/clean-results.mjs tests/allure-results/playwright tests/allure-results/playwright-junit.xml tests/allure-results/playwright-preview-junit.xml
> @imagesquid/ui@0.1.0 test:e2e
> node tests/support/run-e2e.mjs
Running 9 tests using 1 worker
  ✓  1 [chromium] › tests/e2e/app.spec.ts:14:1 › ImageSquid brand, album vocabulary and FileSquid API health (289ms)
  ✓  2 [chromium] › tests/e2e/app.spec.ts:56:1 › ImageSquid tokens override FileSquid defaults in light and dark (IS-S-008) (234ms)
  ✓  3 [chromium] › tests/e2e/app.spec.ts:81:1 › ImageSquid's theme control keeps a dark choice across a reload (322ms)
  ✓  4 [chromium] › tests/e2e/app.spec.ts:101:1 › ImageSquid names missing credentials in its own words (201ms)
  ✓  5 [chromium] › tests/e2e/app.spec.ts:110:1 › ImageSquid reaches the FileSquid Monitor with its own vocabulary (368ms)
  ✓  6 [chromium] › tests/e2e/app.spec.ts:131:3 › ImageSquid inherits the keyboard, naming and focus contracts in light (UI-S-016) (262ms)
  ✓  7 [chromium] › tests/e2e/app.spec.ts:131:3 › ImageSquid inherits the keyboard, naming and focus contracts in dark (UI-S-016) (349ms)
  ✓  8 [chromium] › tests/e2e/app.spec.ts:178:3 › the navigation tree and view states speak ImageSquid in light (UI-F2.1a) (264ms)
  ✓  9 [chromium] › tests/e2e/app.spec.ts:178:3 › the navigation tree and view states speak ImageSquid in dark (UI-F2.1a) (293ms)
  9 passed (9.1s)
Running 4 tests using 1 worker
  ✓  1 [chromium] › tests/e2e/preview/production-build.spec.ts:36:1 › the built page is static, mounts the shell and defines every custom element (206ms)
  ✓  2 [chromium] › tests/e2e/preview/production-build.spec.ts:53:1 › navigation, API status and theme legend words come from the active label table (136ms)
  ✓  3 [chromium] › tests/e2e/preview/production-build.spec.ts:71:1 › computed ImageSquid token values are exact in light and dark (145ms)
  ✓  4 [chromium] › tests/e2e/preview/production-build.spec.ts:87:1 › a refused health call shows the unavailable alert (126ms)
  4 passed (6.7s)
```

## Pre-commit hook

`FULL=1 FILESQUID_E2E=1 bash scripts/pre-commit.sh` on the final head:

```text
ℹ️  Test scope: full
✅ Playwright e2e passed
✅ Lint and test validation passed (test scope: full).
✅ All Pre-Commit Hooks Passed! Test scope: full
```
