# Task UI-F2.18 Walkthrough: Conform the built surfaces and the shell to the active v3 mockups

Branch `feat/ui-f2.18-v3-tokens-and-shell`. Plan row: [Task UI-F2.18](../plan.md). Active suite: [v3 README](../../../../../resources/ui-mockups/v3/README.md). The owner's answers of 2026-10-22 and 2026-10-23 recorded there bind this task and were applied as recorded, not asked again.

## SRS mapping

| SRS ID and title | What was built | Proof |
|---|---|---|
| [SRS-UI-007](../../../../../resources/srs.md#srs-ui-007-light-and-dark-themes) Light and dark themes | The guide's values in `src/theme/tokens.css`; the exemption list `FILESQUID_CONTRAST_EXEMPTIONS` applied only to the default; focus and control-outline pairs at 3:1 with no exception | `tests/unit/theme.test.ts`, `tests/e2e/theme.spec.ts`, ImageSquid `tests/unit/theme.test.ts` |
| UI-S-005 A distribution's label table MUST define every key of the default table, and its token file every default token | Approved v3 strings in both label tables; every v3 token in the ImageSquid token file with its own AA values | `tests/unit/theme.test.ts` (both packages) |
| UI-S-013 UI tests run against production artifacts with strict DOM assertions | Preview specs of both packages and the new `tests/e2e/preview/mockup-fidelity.spec.ts` | Playwright output below |
| UI-S-015 Brand slots, theme control and contrast contract | Brand at the top of the sidebar; theme control only in Settings, Appearance; contrast rule with the listed pairs at exact values | `tests/ui/settings.test.ts`, `tests/ui/app-shell.test.ts`, `tests/unit/theme.test.ts` |
| UI-S-016 Accessibility contracts | `--fs-focus-ring` ring, focused-input border, Tab order through the new sidebar, reflow at 320 px and 200 % text | `tests/e2e/accessibility.spec.ts`, `tests/unit/component-styles.test.ts` |
| UI-S-017 Navigation tree in sections by kind with Recently deleted last | Eyebrow headings, 16 px kind icons, chevron twisties, arrow move buttons, active tint with the 2.5 px brand bar; the single Recently deleted row after a 1 px divider | `tests/ui/nav-tree.test.ts`, `tests/ui/app-shell.test.ts`, fidelity spec |
| UI-S-024 Visual constants of the active mockup suite | Colour, type, spacing, radius tokens and the shared `CONTROL_STYLE` | fidelity spec, `tests/unit/theme.test.ts` |
| UI-S-025 Shell composition | 240 px sidebar, 60 px header, 40 px footer, 320 px panel region, window-height frame with `main` scrolling, `frameless` welcome | `tests/ui/app-shell.test.ts`, fidelity spec |
| UI-S-026 Browser views, menus and dialogs | Rename form and refusals (title, 36 px input, 2 px error border), the centred view states with icons | `tests/ui/file-browser-writes.test.ts`, `tests/ui/states.test.ts`, fidelity spec |
| UI-S-027 Library lifecycle screens | Welcome (01), folder-choice answers (03), library page (41), repair consent (42) and results (43) | `tests/ui/library-*.test.ts`, `tests/e2e/preview/library.spec.ts`, fidelity spec |
| UI-S-031 Preview and file panel composition | Unsupported treatment: badge, 22 px heading, the two help lines | `tests/ui/file-preview.test.ts`, `tests/e2e/preview/file-browser.spec.ts` |
| UI-S-032 Monitor and Logs composition | Page head (title, owner-only line, Live chip, Download JSON Lines), card sections, eyebrows, 22 px status word, stalled state and finding codes in the error colour | `tests/ui/monitor.test.ts`, `tests/ui/ocfl-findings.test.ts`, fidelity spec |
| UI-S-033 Settings page composition | Appearance, Privacy and About with "API: {status} · contract {version}"; host popover 372 px on the overlay surface | `tests/ui/settings.test.ts`, `tests/e2e/preview/host-indicator.spec.ts`, fidelity spec |

## What changed

- **Tokens.** `src/theme/tokens.css` and `src/theme/tokens.ts` carry every colour role of UI-S-024 in light and dark, the type sizes, the 4 to 96 px spacing scale and the radii. Two type sizes the mockups draw but the token list did not name were added: `--fs-font-size-state` (15 px view-state titles, mockup 56) and `--fs-font-size-card` (21 px welcome card titles, UI-S-027); the UI SRS records both. ImageSquid's `tokens.css` defines every token with its own values: Arkrim blue primary and focus ring, a cool grey sidebar (`--fs-surface-card` `#f4f5f7`), a control outline `#7d8490`, every text pair at 4.5:1 and no exemption.
- **Shell.** `<fs-app-shell>` is the UI-S-025 frame. The header holds the search slot, the host badge and the Monitor button and nothing else (owner answers 7 and 12). The frame is the window's height and `main` scrolls, so the sidebar, header and footer keep their mockup boxes on long pages; below 40rem the page flows as before.
- **Settings and host popover.** Built by the previous session; verified and covered here.
- **Built surfaces.** Nav tree, view states (icons per mockup 56), welcome, library page, repair page and dialog, Monitor and findings, the rename form and the breadcrumb take the v3 look through tokens and `CONTROL_STYLE`.
- **Contract version.** `npm run contract:types` now also writes `src/api/contract-version.ts` (`tests/support/write-contract-version.mjs`), so the version Settings shows moves with the generated types; the drift test checks it.
- **Repair consent wording.** `repairConsent` keeps its current string. The owner decided on 2026-10-23 to change it in a separate fix task (`fix/repair-consent-wording`) that changes the API's expected statement, the label, mockup 42 and UI-S-027 together, so the UI and the API never disagree.

## Review fixes (Cubic round 1: 25 P2, 13 P3)

- Contrast: hovered primary buttons keep their tested colours (white on `#0f8a39` or `#4aca74` fell below 4.5:1); disabled buttons keep their disabled colours on hover; a focused invalid input keeps a focus outline; the theme control's options and every input take `--fs-outline-control`.
- Contrast data: `CONTRAST_PAIRS` gains error text on `--fs-surface-card`; the exemption list now holds only pairs the test checks. Error, muted and placeholder text is never drawn on `--fs-surface-overlay`, because in dark it would be below 4.5:1 there (`#ef5350` 4.01:1, `#6a6a6a` 2.58:1) with no exemption; so the rename form sits on `--fs-surface-card`, and the Settings API alert shows its words in the body colour with the alert icon in the error colour. A unit test fixes the exact allowlist and recomputes each listed ratio.
- Icons keep a 1.6 CSS px stroke at every size (`vector-effect: non-scaling-stroke`).
- The file browser frame uses the shared `CONTROL_STYLE` buttons and inputs instead of its own copy, and marks only the refused field with `aria-invalid` (the name, or the folder).
- The Settings About logo follows the theme (dark-surface variant in dark); only the header badge tells the shell which host is shown, so an inline statement cannot hide the Logs row; the no-telemetry line is shown only while hosted backup and sync are both off, so it is true as worded; the popover's Privacy settings divider renders.
- The preview badge uses the 10 px badge size; a failed journal entry colours only its outcome.
- Tests: field names and reason text of the Monitor fixtures follow the contract; the breadcrumb's current link, the per-state icons, every tree row's icons, the single Recently deleted row, the library page columns, the one primary button, the host re-read on refresh, the generated contract version in ImageSquid and its dark control outline, and ImageSquid's Tab order to the host badge and Monitor are asserted; the contrast probe of `theme.spec.ts` refuses a missing token.
- UI-S-015 names the type-size and radius tokens it requires.
- Kept, with the reason on the thread: the plan box (the surfaces left open belong to named later tasks or undelivered API rows, listed below); the required approved strings (UI-S-024 Strings and owner answer 13 require both label tables to define them); `hostDesktopNoTelemetry` wording (UI-S-033's wording; UI-S-027 quotes a different sentence for the same key, recorded below for the owner).

## Review fixes (Cubic round 2: 1 P2, 1 P3)

- The Monitor fixture of the fidelity spec uses the API's `process_stalled` reason code.
- The token test asserts that light error text on `--fs-surface-card` is reported without the exemptions (4.23:1) and that dark error text there passes.

## Not built here (needs a later task or an API route)

- Recent libraries on the welcome screen, the library page's storage used and free space rows and STORAGE BY COLLECTION, and the shell's storage figures: they need the library facts of API-S-038, not delivered. The shell's storage block and `deletedCount` wait for a view to supply them.
- The rebuild as a durable job (04): API-S-025 and API-S-037 jobs, not delivered.
- Storage unavailable (34), the library menu (08) and the preview bar's Download, Show in Finder, zoom and filmstrip (22, 23): UI-F2.17, UI-F2.8 and UI-F2.15.
- The rename form keeps the file browser's inline form (styled as mockup 14) rather than a modal dialog, so API-F2.13 can move the file browser's callers without a conflicting rewrite.
- No API route, field or event was assumed or stubbed.
- For the owner: UI-S-027 quotes `hostDesktopNoTelemetry` as "FileSquid sends nothing off this computer. There is no account and no telemetry." while UI-S-033 and the label table read "FileSquid sends nothing off this computer. No account, no telemetry." The label table keeps the UI-S-033 wording until the owner picks one.

## How it was tested

1. The tests updated by the previous session and the first new tests of this session ran red against the unfinished implementation (the red runs below), then green. Some look tests (tree, library pages, Monitor, findings, rename form) were written together with their stylesheet changes; they assert the computed constants of the fidelity spec as well.
2. `npx prettier --write .`, `npx eslint .`, `npx tsc --noEmit` in `apps/filesquid/ui` and `apps/imagesquid`: clean.
3. `npm test` in both packages.
4. Playwright, both suites of both packages. The installed Playwright expects a newer Chromium build than this machine has, so the suites ran through temporary configs that point `executablePath` at the machine's Chromium (`/opt/pw-browsers/chromium`); the configs were deleted before committing. Chromium here reports a 1.5 px border as 1 px even at 2x. The fidelity spec therefore checks only that those borders render as this browser renders 1.5 px, which a 1 px border would also pass; the authored `1.5px` of each such border is covered by the component tests (`tests/unit/component-styles.test.ts`, the `CONTROL_STYLE` and stylesheet assertions in `tests/ui/`), not by the fidelity spec. No other mocked environment values were used.

## Raw test output

### Red runs (start of this session, before the implementation was finished)

`npm test` in `apps/filesquid/ui`:

```text
 FAIL  tests/ui/app-shell.test.ts > <fs-app-shell> > shows the storage block only once a summary is given, worded from the label table
 FAIL  tests/ui/host-indicator.test.ts > <fs-host-indicator> structure > is a defined custom element: a named group with a badge button and a closed popover
 FAIL  tests/ui/host-indicator.test.ts > <fs-host-indicator> follows the host > follows a new label table when configured again while connected
 FAIL  tests/ui/nav-tree.test.ts > <fs-nav-tree> structure > puts the shared accessibility stylesheet first
 FAIL  tests/ui/states.test.ts > <fs-states> > shows { kind: 'error' } in the alert region
 FAIL  tests/ui/states.test.ts > <fs-states> > puts the shared accessibility stylesheet first
 FAIL  tests/unit/component-styles.test.ts > component stylesheets > 'fs-states.ts' takes spacing, type size and radius from tokens
 FAIL  tests/unit/component-styles.test.ts > UI-S-016 accessibility contracts in component stylesheets > the shared stylesheet draws a 2px primary focus ring offset by 2px
 FAIL  tests/unit/component-styles.test.ts > UI-S-016 accessibility contracts in component stylesheets > fs-app-shell.ts includes the shared accessibility stylesheet
 FAIL  tests/unit/component-styles.test.ts > UI-S-016 accessibility contracts in component stylesheets > fs-file-preview.ts includes the shared accessibility stylesheet
 FAIL  tests/unit/component-styles.test.ts > UI-S-016 accessibility contracts in component stylesheets > fs-host-indicator.ts includes the shared accessibility stylesheet
 FAIL  tests/unit/component-styles.test.ts > UI-S-016 accessibility contracts in component stylesheets > fs-states.ts includes the shared accessibility stylesheet
 Test Files  5 failed | 51 passed (56)
      Tests  12 failed | 1257 passed | 1 skipped (1270)
```

`npm test` in `apps/imagesquid`:

```text
 FAIL  tests/ui/distribution.test.ts > renders ImageSquid brand and vocabulary in the shared shell
 FAIL  tests/ui/distribution.test.ts > keeps the official mark in the dark theme and shows the theme control
 FAIL  tests/unit/theme.test.ts > ImageSquid label table > uses repo, album, gallery and exhibit for the internal nouns
 FAIL  tests/unit/theme.test.ts > ImageSquid design tokens > define every required token for light and dark
 FAIL  tests/unit/theme.test.ts > ImageSquid design tokens > meet WCAG 2.2 AA for primary actions and body text in light and dark
 Test Files  2 failed | 2 passed (4)
      Tests  5 failed | 31 passed | 1 skipped (37)
```

Preview suite (`apps/filesquid/ui`, before the specs and surfaces were updated):

```text
  17 failed
    [chromium] › tests/e2e/preview/file-browser.spec.ts:554:1 › motion stops under reduced motion and focus is the 2px primary ring
    [chromium] › tests/e2e/preview/host-indicator.spec.ts:271:1 › Enter opens the popover with the full statement; Escape closes it and returns focus
    [chromium] › tests/e2e/preview/host-indicator.spec.ts:296:1 › Privacy settings opens the Settings page, whose Privacy section has the full statement
    [chromium] › tests/e2e/preview/host-indicator.spec.ts:331:1 › computed token values are exact in light and dark
    [chromium] › tests/e2e/preview/host-indicator.spec.ts:378:1 › the badge is reachable by keyboard and has a visible focus outline
    [chromium] › tests/e2e/preview/library.spec.ts:45:1 › the start page offers create, open and recover in the active vocabulary
    [chromium] › tests/e2e/preview/library.spec.ts:74:1 › (1) create asks for a folder, passes the location ID on and shows the library, with no path
    [chromium] › tests/e2e/preview/library.spec.ts:96:1 › (1) open passes the location ID to the open route
    [chromium] › tests/e2e/preview/library.spec.ts:110:1 › (1) recover opens the copy, rebuilds from files and reports without repairing
    [chromium] › tests/e2e/preview/library.spec.ts:166:1 › (2) a cancelled folder choice sends no library request and says nothing changed
    [chromium] › tests/e2e/preview/library.spec.ts:180:1 › (3) 503 unavailable is the unavailable state with its reason and a Retry, and the choices stay
    [chromium] › tests/e2e/preview/library.spec.ts:335:1 › colours come from tokens in light and dark
    [chromium] › tests/e2e/preview/navigation.spec.ts:194:1 › computed token values of the tree and breadcrumb are exact in light and dark
    [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:289:1 › error and warning codes take their colours from tokens in light and dark
    [chromium] › tests/e2e/preview/production-build.spec.ts:79:1 › navigation, API status and theme legend words come from the active label table
    [chromium] › tests/e2e/preview/production-build.spec.ts:101:1 › computed token values are exact in light and dark
    [chromium] › tests/e2e/preview/production-build.spec.ts:117:1 › a refused health call shows the unavailable alert, not a console error from the app
  47 passed (56.0s)
```

First run of the new mockup fidelity spec:

```text
  ✘   1 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:201:5 › light theme › the shell frame (UI-S-025): sidebar, header, footer, controls and strings (495ms)
  ✘   3 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:323:5 › light theme › welcome (01, UI-S-027): title, lead, cards, privacy box and footer (498ms)
  ✘   4 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:379:5 › light theme › folder-choice unavailable (03) and the view states treatment (56) (588ms)
    Error: height 1669.78125 is within 2 px of 900
  10 failed
  6 passed (25.6s)
```

### npm test (apps/filesquid/ui)

```text
 ✓ tests/ui/host-indicator.test.ts (40 tests) 523ms
 ✓ tests/ui/nav-tree.test.ts (30 tests) 1403ms
 ✓ tests/unit/theme.test.ts (35 tests) 72ms
 ✓ tests/ui/file-browser-writes.test.ts (46 tests) 2224ms
 ✓ tests/ui/file-browser.test.ts (37 tests) 2579ms
 ✓ tests/ui/app-shell.test.ts (23 tests) 958ms
 ✓ tests/ui/ocfl-findings.test.ts (22 tests) 712ms
 ✓ tests/ui/library-start.test.ts (22 tests) 434ms
 ✓ tests/unit/files-source.test.ts (31 tests) 80ms
 ✓ tests/ui/library-repair.test.ts (22 tests) 553ms
 ✓ tests/unit/files-memory-source.test.ts (34 tests) 73ms
 ✓ tests/unit/component-styles.test.ts (193 tests) 165ms
 ✓ tests/ui/monitor.test.ts (22 tests) 6491ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 1224ms
 ✓ tests/ui/library-detail.test.ts (20 tests) 475ms
 ✓ tests/unit/files-ops.test.ts (54 tests) 234ms
 ✓ tests/ui/file-preview.test.ts (16 tests) 186ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 78ms
 ✓ tests/ui/navigation-view.test.ts (20 tests) 283ms
 ✓ tests/unit/client-io.test.ts (22 tests) 121ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 41ms
 ✓ tests/ui/settings.test.ts (10 tests) 284ms
 ✓ tests/ui/nav-tree-live.test.ts (11 tests) 528ms
 ✓ tests/unit/media-handle.test.ts (14 tests) 132ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 55ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 128ms
 ✓ tests/unit/library-client.test.ts (18 tests) 70ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 35ms
 ✓ tests/unit/navigation-client.test.ts (13 tests) 23ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 49ms
 ✓ tests/unit/files-model.test.ts (58 tests) 66ms
 ✓ tests/ui/states.test.ts (26 tests) 182ms
 ✓ tests/unit/commands.test.ts (45 tests) 51ms
 ✓ tests/unit/host-descriptor.test.ts (43 tests) 72ms
 ✓ tests/unit/admin-client.test.ts (10 tests) 79ms
 ✓ tests/unit/files-window.test.ts (19 tests) 34ms
 ✓ tests/unit/focus.test.ts (16 tests) 176ms
 ✓ tests/unit/library-errors.test.ts (15 tests) 19ms
 ✓ tests/unit/navigation-contract.test.ts (30 tests) 23ms
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 1462ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 31ms
 ✓ tests/ui/library-route.test.ts (6 tests) 124ms
 ✓ tests/ui/files-route.test.ts (6 tests) 545ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 95ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 137ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 16ms
 ✓ tests/unit/library-model.test.ts (7 tests) 19ms
 ✓ tests/unit/client.test.ts (8 tests) 17ms
 ✓ tests/unit/host-client.test.ts (4 tests) 13ms
 ✓ tests/unit/preferences.test.ts (10 tests) 43ms
 ✓ tests/unit/view-state.test.ts (5 tests) 16ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 167ms
 ✓ tests/unit/contrast.test.ts (13 tests) 16ms
 ✓ tests/unit/distribution.test.ts (3 tests) 7ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 27ms
 ✓ tests/unit/contract.test.ts (2 tests) 347ms
 Test Files  56 passed (56)
      Tests  1290 passed | 1 skipped (1291)
   Duration  31.77s (transform 4.52s, setup 2.86s, import 10.50s, tests 24.00s, environment 42.01s)
```

### npm test (apps/imagesquid)

```text
 ✓ tests/unit/test-reporting.test.ts (12 tests) 15ms
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 949ms
 ✓ tests/unit/theme.test.ts (12 tests) 32ms
 ✓ tests/ui/distribution.test.ts (5 tests) 193ms
 Test Files  4 passed (4)
      Tests  38 passed | 1 skipped (39)
   Duration  3.08s (transform 1.90s, setup 502ms, import 2.12s, tests 1.19s, environment 2.40s)
```

### Playwright, built UI served by `filesquid-server` (apps/filesquid/ui)

```text
  ✓   1 [chromium] › tests/e2e/accessibility.spec.ts:67:1 › landmarks, headings and control names come from the label table (487ms)
  ✓   2 [chromium] › tests/e2e/app.spec.ts:35:1 › shell mounts with FileSquid brand, internal vocabulary and the API status in Settings only (597ms)
  ✓   3 [chromium] › tests/e2e/accessibility.spec.ts:112:3 › Tab order and a 2px --fs-focus-ring focus ring in the light theme (530ms)
  ✓   4 [chromium] › tests/e2e/app.spec.ts:63:1 › design tokens switch between light and dark (394ms)
  ✓   6 [chromium] › tests/e2e/app.spec.ts:81:1 › client-side routes fall back to the distribution index (248ms)
  ✓   5 [chromium] › tests/e2e/accessibility.spec.ts:112:3 › Tab order and a 2px --fs-focus-ring focus ring in the dark theme (594ms)
  ✓   7 [chromium] › tests/e2e/app.spec.ts:87:1 › without a token Settings, About names the missing credentials from the label table (304ms)
  ✓   8 [chromium] › tests/e2e/accessibility.spec.ts:150:1 › light-DOM content slotted into the shell gets the same focus ring (441ms)
  ✓   9 [chromium] › tests/e2e/app.spec.ts:99:1 › Monitor asks for the admin token, then shows live health, processes and the journal (550ms)
  ✓  10 [chromium] › tests/e2e/accessibility.spec.ts:178:1 › the skip control appears on focus and moves focus to main without routing (401ms)
  ✓  11 [chromium] › tests/e2e/app.spec.ts:135:1 › a wrong token is named as invalid credentials (482ms)
  ✓  12 [chromium] › tests/e2e/accessibility.spec.ts:196:1 › opening the Monitor from the keyboard moves focus to main (448ms)
  ✓  13 [chromium] › tests/e2e/app.spec.ts:146:1 › the signed-in token stays in memory: not in storage, URL or DOM, gone on reload (829ms)
  ✓  14 [chromium] › tests/e2e/accessibility.spec.ts:214:1 › a keyboard sign-in to the Monitor keeps focus in the Monitor (888ms)
  ✓  15 [chromium] › tests/e2e/app.spec.ts:177:1 › the desktop launch token authorises the shell, the Monitor and the event channel (373ms)
  ✓  17 [chromium] › tests/e2e/app.spec.ts:270:3 › inside the desktop webview (custom URI scheme adapter) › requests use the custom scheme and events use Tauri, not same-origin HTTP or WebSocket (401ms)
  ✓  16 [chromium] › tests/e2e/accessibility.spec.ts:231:1 › reduced motion stops every transition (516ms)
  ✓  18 [chromium] › tests/e2e/accessibility.spec.ts:273:1 › more contrast strengthens borders; forced colours keep the chosen option (370ms)
  ✓  19 [chromium] › tests/e2e/navigation.spec.ts:189:1 › the elements are defined and worded from the label table inside the shell (434ms)
  ✓  20 [chromium] › tests/e2e/accessibility.spec.ts:317:3 › no horizontal scroll at 320 px wide and 100 % text (369ms)
  ✓  21 [chromium] › tests/e2e/navigation.spec.ts:247:1 › the tree follows the ARIA treeview keys and reads children as nodes open (432ms)
  ✓  22 [chromium] › tests/e2e/accessibility.spec.ts:317:3 › no horizontal scroll at 320 px wide and 200 % text (448ms)
  ✓  23 [chromium] › tests/e2e/navigation.spec.ts:283:1 › pins, section order and library collapse are kept on the device across a reload (631ms)
  ✓  24 [chromium] › tests/e2e/accessibility.spec.ts:317:3 › no horizontal scroll at 1280 px wide and 200 % text (421ms)
  ✓  25 [chromium] › tests/e2e/navigation.spec.ts:327:1 › refused device storage keeps the choices for this page and says why (398ms)
  ✓  26 [chromium] › tests/e2e/accessibility.spec.ts:317:3 › no horizontal scroll at 640 px wide and 200 % text (383ms)
  ✓  27 [chromium] › tests/e2e/navigation.spec.ts:352:1 › the tree shows the error and offline states and reads again on Retry (389ms)
  ✓  28 [chromium] › tests/e2e/accessibility.spec.ts:341:1 › a right-to-left locale mirrors the header and navigation (340ms)
  ✓  29 [chromium] › tests/e2e/navigation.spec.ts:374:3 › the five states are distinct and use token colours in the light theme (307ms)
  ✓  30 [chromium] › tests/e2e/ocfl-findings.spec.ts:37:1 › the findings section reads the real findings route with the token (457ms)
  ✓  31 [chromium] › tests/e2e/navigation.spec.ts:434:3 › tree controls draw the 2px --fs-focus-ring ring in the light theme (415ms)
  ✓  32 [chromium] › tests/e2e/ocfl-findings.spec.ts:68:1 › Validate now starts runs through the real run route and says nothing was open (424ms)
  ✓  33 [chromium] › tests/e2e/navigation.spec.ts:374:3 › the five states are distinct and use token colours in the dark theme (351ms)
  ✓  34 [chromium] › tests/e2e/theme.spec.ts:96:1 › official FileSquid tokens apply in light and dark and meet WCAG 2.2 AA (409ms)
  ✓  35 [chromium] › tests/e2e/navigation.spec.ts:434:3 › tree controls draw the 2px --fs-focus-ring ring in the dark theme (326ms)
  ✓  36 [chromium] › tests/e2e/theme.spec.ts:122:1 › the official mark fills the brand slot, with its dark-surface variant in dark (354ms)
  ✓  37 [chromium] › tests/e2e/navigation.spec.ts:460:1 › tree motion follows the token, stops under reduced motion, and forced colours mark the current item (461ms)
  ✓  39 [chromium] › tests/e2e/navigation.spec.ts:501:3 › the tree and breadcrumb reflow at 320 px and 100 % text (411ms)
  ✓  38 [chromium] › tests/e2e/theme.spec.ts:147:1 › the theme follows the OS by default and the control's choice persists across a reload (603ms)
  ✓  40 [chromium] › tests/e2e/navigation.spec.ts:501:3 › the tree and breadcrumb reflow at 320 px and 200 % text (534ms)
  ✓  41 [chromium] › tests/e2e/theme.spec.ts:192:1 › the chosen theme option and the Settings API status are drawn as UI-S-015 lists them (839ms)
  ✓  42 [chromium] › tests/e2e/navigation.spec.ts:501:3 › the tree and breadcrumb reflow at 1280 px and 200 % text (336ms)
  42 passed (43.5s)
```

### Playwright, production build (`vite build` plus `vite preview`, apps/filesquid/ui)

```text
  ✓   2 [chromium] › tests/e2e/preview/host-indicator.spec.ts:96:1 › the indicator is a defined custom element in the shell header, named from the label table (530ms)
  ✓   1 [chromium] › tests/e2e/preview/file-browser.spec.ts:96:1 › the browser is a defined element and lists display names in the list layout from the label table (729ms)
  ✓   3 [chromium] › tests/e2e/preview/host-indicator.spec.ts:110:1 › the desktop host says only you can read your files, and shows the not-enrolled state (478ms)
  ✓   4 [chromium] › tests/e2e/preview/file-browser.spec.ts:140:1 › a file row shows size, type hint, modified date and version (540ms)
  ✓   5 [chromium] › tests/e2e/preview/host-indicator.spec.ts:124:1 › a server names its operator, who can read the files, and states the at-rest duty (377ms)
  ✓   6 [chromium] › tests/e2e/preview/file-browser.spec.ts:152:1 › computed token values are exact in the browser in light and dark (640ms)
  ✓   7 [chromium] › tests/e2e/preview/host-indicator.spec.ts:137:1 › a server without an operator name uses the unnamed-operator label (442ms)
  ✓   9 [chromium] › tests/e2e/preview/host-indicator.spec.ts:149:1 › the browser host says files are decrypted only in this browser (385ms)
  ✓   8 [chromium] › tests/e2e/preview/file-browser.spec.ts:175:1 › opening a folder updates the breadcrumb and the address, and Back returns (798ms)
  ✓  10 [chromium] › tests/e2e/preview/host-indicator.spec.ts:162:1 › enrolled backup and sync are shown apart from the host, as ciphertext only (501ms)
  ✓  12 [chromium] › tests/e2e/preview/host-indicator.spec.ts:187:1 › a host.change event updates the indicator without a reload or another read (366ms)
  ✓  11 [chromium] › tests/e2e/preview/file-browser.spec.ts:196:1 › an image previews through a content handle that is used only as the media src (744ms)
  ✓  13 [chromium] › tests/e2e/preview/host-indicator.spec.ts:209:1 › events.resync makes the indicator read the descriptor again (493ms)
  ✓  14 [chromium] › tests/e2e/preview/file-browser.spec.ts:231:1 › a handle that fails is replaced once; a second failure shows the error and mints no third (645ms)
  ✓  16 [chromium] › tests/e2e/preview/file-browser.spec.ts:247:1 › a video resumes at the same position and paused state after one new handle (633ms)
  ✓  15 [chromium] › tests/e2e/preview/host-indicator.spec.ts:222:1 › loading shows no host until the descriptor arrives (1.2s)
  ✓  17 [chromium] › tests/e2e/preview/file-browser.spec.ts:283:1 › audio plays natively and is named by the display name (626ms)
  ✓  18 [chromium] › tests/e2e/preview/host-indicator.spec.ts:234:1 › a failed read shows the error state and never a guessed host, then Retry recovers (545ms)
  ✓  20 [chromium] › tests/e2e/preview/host-indicator.spec.ts:256:1 › the header shows a compact badge with the short host label, not the statement (448ms)
  ✓  19 [chromium] › tests/e2e/preview/file-browser.spec.ts:296:1 › text previews through one ranged read of the first 64 KiB and says it is cut (572ms)
  ✓  21 [chromium] › tests/e2e/preview/host-indicator.spec.ts:271:1 › Enter opens the popover with the full statement; Escape closes it and returns focus (530ms)
  ✓  22 [chromium] › tests/e2e/preview/file-browser.spec.ts:312:1 › a format it cannot show says so and shows no viewer (609ms)
  ✓  23 [chromium] › tests/e2e/preview/host-indicator.spec.ts:297:1 › Privacy settings opens the Settings page, whose Privacy section has the full statement (686ms)
  ✓  24 [chromium] › tests/e2e/preview/file-browser.spec.ts:342:1 › the viewer steps through files, closes with Escape and returns focus to the row (637ms)
  ✓  26 [chromium] › tests/e2e/preview/file-browser.spec.ts:355:1 › an upload is one PUT of the client's facts that creates the file's own object (655ms)
  ✓  25 [chromium] › tests/e2e/preview/host-indicator.spec.ts:333:1 › computed token values are exact in light and dark (UI-S-024, mockup 40) (846ms)
  ✓  27 [chromium] › tests/e2e/preview/file-browser.spec.ts:404:1 › a rename posts to the renames route, never to moves, and shows the new display name (685ms)
  ✓  28 [chromium] › tests/e2e/preview/host-indicator.spec.ts:378:1 › the badge is reachable by keyboard and has a visible focus outline (929ms)
  ✓  29 [chromium] › tests/e2e/preview/file-browser.spec.ts:433:1 › a refused rename shows the label for its reason and keeps the display name (650ms)
  ✓  30 [chromium] › tests/e2e/preview/library.spec.ts:47:1 › the start page offers create, open and recover in the active vocabulary (406ms)
  ✓  32 [chromium] › tests/e2e/preview/library.spec.ts:80:1 › (1) create asks for a folder, passes the location ID on and shows the library, with no path (627ms)
  ✓  31 [chromium] › tests/e2e/preview/file-browser.spec.ts:452:1 › writes are single calls: new folder, move, remove and Undo (1.3s)
  ✓  33 [chromium] › tests/e2e/preview/library.spec.ts:102:1 › (1) open passes the location ID to the open route (753ms)
  ✓  34 [chromium] › tests/e2e/preview/file-browser.spec.ts:499:1 › a refused write shows the label for its reason and changes nothing (566ms)
  ✓  35 [chromium] › tests/e2e/preview/library.spec.ts:116:1 › (1) recover opens the copy, rebuilds from files and reports without repairing (611ms)
  ✓  36 [chromium] › tests/e2e/preview/file-browser.spec.ts:514:1 › a long folder is virtualised, pages in on demand and stays keyboard reachable (833ms)
  ✓  37 [chromium] › tests/e2e/preview/library.spec.ts:153:1 › (1) copy asks for a destination and passes it as destination_location_id (502ms)
  ✓  39 [chromium] › tests/e2e/preview/library.spec.ts:176:1 › (2) a cancelled folder choice sends no library request and says nothing changed (528ms)
  ✓  38 [chromium] › tests/e2e/preview/file-browser.spec.ts:554:1 › the layout does not scroll sideways at 320 px, at 200 % text size, or both, in list and grid (689ms)
  ✓  40 [chromium] › tests/e2e/preview/library.spec.ts:190:1 › (3) 503 unavailable is the unavailable state with its reason and a Retry, and the choices stay (687ms)
  ✓  41 [chromium] › tests/e2e/preview/file-browser.spec.ts:571:1 › motion stops under reduced motion and focus is the 2px --fs-focus-ring ring (592ms)
  ✓  42 [chromium] › tests/e2e/preview/library.spec.ts:217:1 › (4) confirm sends the consent for exactly the named objects and shows each result (484ms)
  ✓  43 [chromium] › tests/e2e/preview/file-browser.spec.ts:598:1 › a failed listing shows the error state with Try again, and retrying loads it (536ms)
  ✓  44 [chromium] › tests/e2e/preview/library.spec.ts:254:1 › (4) cancel and Escape send nothing and return focus to the opener (626ms)
  ✓  45 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:219:5 › light theme › the shell frame (UI-S-025): sidebar, header, footer, controls and strings (717ms)
  ✓  46 [chromium] › tests/e2e/preview/library.spec.ts:277:1 › (4) a refused repair says nothing was changed and shows no result (445ms)
  ✓  47 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:303:5 › light theme › the navigation tree as built (55, UI-S-017): eyebrows, 30 px rows, 16 px icons (546ms)
  ✓  48 [chromium] › tests/e2e/preview/library.spec.ts:292:1 › (5) the Monitor's W010 note links to the library's repair offer (537ms)
  ✓  49 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:341:5 › light theme › welcome (01, UI-S-027): title, lead, cards, privacy box and footer (618ms)
  ✓  50 [chromium] › tests/e2e/preview/library.spec.ts:345:1 › colours come from tokens in light and dark (886ms)
  ✓  51 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:397:5 › light theme › folder-choice unavailable (03) and the view states treatment (56) (636ms)
  ✓  52 [chromium] › tests/e2e/preview/navigation.spec.ts:62:1 › the live tree fills the shell's navigation slot, worded from the label table (452ms)
  ✓  54 [chromium] › tests/e2e/preview/navigation.spec.ts:83:1 › a level is read page by page through next_cursor until it is null (540ms)
  ✓  53 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:432:5 › light theme › library page (41, UI-S-027): facts rows, actions and the W010 note (966ms)
  ✓  55 [chromium] › tests/e2e/preview/navigation.spec.ts:121:1 › a destination opens with a breadcrumb filled from ancestors (574ms)
  ✓  56 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:482:5 › light theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (829ms)
  ✓  57 [chromium] › tests/e2e/preview/navigation.spec.ts:142:1 › a deep link to a destination shows its breadcrumb after a reload (557ms)
  ✓  59 [chromium] › tests/e2e/preview/navigation.spec.ts:154:1 › navigation.change reads only the named level again, without a reload (552ms)
  ✓  58 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:527:5 › light theme › Monitor (31) and its OCFL validation section (54) (831ms)
  ✓  60 [chromium] › tests/e2e/preview/navigation.spec.ts:177:1 › a 503 shows the unavailable state with Retry, never an empty tree (556ms)
  ✓  61 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:585:5 › light theme › host popover (40) and Settings (32): 372 px overlay, Appearance and About (768ms)
  ✓  62 [chromium] › tests/e2e/preview/navigation.spec.ts:194:1 › computed token values of the tree and breadcrumb are exact in light and dark (749ms)
  ✓  63 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:219:5 › dark theme › the shell frame (UI-S-025): sidebar, header, footer, controls and strings (801ms)
  ✓  64 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:174:1 › (1) an E023 finding shows with its code beside the degraded health reason (695ms)
  ✓  65 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:303:5 › dark theme › the navigation tree as built (55, UI-S-017): eyebrows, 30 px rows, 16 px icons (556ms)
  ✓  66 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:200:1 › (2) the finding disappears after a clean revalidation's health.change, without a reload (708ms)
  ✓  67 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:341:5 › dark theme › welcome (01, UI-S-027): title, lead, cards, privacy box and footer (621ms)
  ✓  68 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:220:1 › (3) a W010 warning is listed apart from errors and health stays ok (539ms)
  ✓  69 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:397:5 › dark theme › folder-choice unavailable (03) and the view states treatment (56) (695ms)
  ✓  70 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:238:1 › (4) no location is rendered when the API withholds it (468ms)
  ✓  71 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:432:5 › dark theme › library page (41, UI-S-027): facts rows, actions and the W010 note (763ms)
  ✓  72 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:250:1 › (5) the last and next run times come from the findings route; Validate now starts a run (618ms)
  ✓  74 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:289:1 › error and warning codes take their colours from tokens in light and dark (583ms)
  ✓  73 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:482:5 › dark theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (779ms)
  ✓  75 [chromium] › tests/e2e/preview/production-build.spec.ts:63:1 › the built page is static, mounts the shell and defines every custom element (502ms)
  ✓  76 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:527:5 › dark theme › Monitor (31) and its OCFL validation section (54) (659ms)
  ✓  77 [chromium] › tests/e2e/preview/production-build.spec.ts:80:1 › sidebar, header and Settings words come from the active label table (UI-S-025, UI-S-033) (488ms)
  ✓  79 [chromium] › tests/e2e/preview/production-build.spec.ts:124:1 › computed token values are exact in light and dark (UI-S-024, CA-1) (405ms)
  ✓  78 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:585:5 › dark theme › host popover (40) and Settings (32): 372 px overlay, Appearance and About (694ms)
  ✓  80 [chromium] › tests/e2e/preview/production-build.spec.ts:146:1 › a refused health call shows the unavailable alert in Settings, About, not a console error (398ms)
  80 passed (37.9s)
```

### Playwright, ImageSquid on `filesquid-server`

```text
  ✓  1 [chromium] › tests/e2e/app.spec.ts:14:1 › ImageSquid brand, album vocabulary and the API status in Settings only (381ms)
  ✓  2 [chromium] › tests/e2e/app.spec.ts:64:1 › ImageSquid tokens override FileSquid defaults in light and dark (IS-S-008) (243ms)
  ✓  3 [chromium] › tests/e2e/app.spec.ts:89:1 › ImageSquid's theme control keeps a dark choice across a reload (394ms)
  ✓  4 [chromium] › tests/e2e/app.spec.ts:110:1 › ImageSquid names missing credentials in its own words (319ms)
  ✓  5 [chromium] › tests/e2e/app.spec.ts:119:1 › ImageSquid reaches the FileSquid Monitor with its own vocabulary (390ms)
  ✓  6 [chromium] › tests/e2e/app.spec.ts:137:3 › ImageSquid inherits the keyboard, naming and focus contracts in light (UI-S-016) (324ms)
  ✓  7 [chromium] › tests/e2e/app.spec.ts:137:3 › ImageSquid inherits the keyboard, naming and focus contracts in dark (UI-S-016) (355ms)
  ✓  8 [chromium] › tests/e2e/app.spec.ts:185:3 › the navigation tree and view states speak ImageSquid in light (UI-F2.1a) (268ms)
  ✓  9 [chromium] › tests/e2e/app.spec.ts:185:3 › the navigation tree and view states speak ImageSquid in dark (UI-F2.1a) (251ms)
  9 passed (9.1s)
```

### Playwright, ImageSquid production build

```text
  ✓  1 [chromium] › tests/e2e/preview/production-build.spec.ts:57:1 › the built page is static, mounts the shell and defines every custom element (440ms)
  ✓  2 [chromium] › tests/e2e/preview/production-build.spec.ts:74:1 › sidebar, header and Settings words come from the active label table (UI-S-025, UI-S-033) (410ms)
  ✓  3 [chromium] › tests/e2e/preview/production-build.spec.ts:112:1 › computed ImageSquid token values are exact in light and dark (301ms)
  ✓  4 [chromium] › tests/e2e/preview/production-build.spec.ts:133:1 › a refused health call shows the unavailable alert in Settings, About (246ms)
  ✓  5 [chromium] › tests/e2e/preview/production-build.spec.ts:146:1 › the library flow runs in ImageSquid with its own label table (UI-1.1) (344ms)
  5 passed (8.9s)
```

### Pre-commit hook (ordinary run, `bash scripts/pre-commit.sh`)

```text
✅ Branch Validation passed.
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
✅ filesquid-server uniquely bound to 8083
✅ Traceability Validation passed: 96 PRD requirements, 84 resolved SRS references.
✅ Workspace Boundary validation passed.
✅ Logging rule validation passed (no relevant staged files).
⏭️  Skipped: Rust formatting in ./apps/filesquid (no Rust file changed; test scope: npm:apps/filesquid/ui,apps/imagesquid)
✅ ./apps/filesquid/ui is properly formatted.
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
# skipped 0
✅ ADR structure (ISO 42010) passed.
✅ PRD allocation passed.
✅ Plan traceability passed.
✅ Relative links and anchors passed.
✅ FileSquid and ImageSquid document boundary passed.
✅ MVP queue and merge driver tests passed.
✅ MVP progress bar passed.
✅ Task cleanup passed.
✅ Documentation validation passed.
✅ Change scope validation passed (168 assertions).
ℹ️  Test scope: npm:apps/filesquid/ui,apps/imagesquid
⏭️  Skipped: Clippy (no Rust crate changed)
⏭️  Skipped: Workspace tests (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-core (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-ocfl (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-metadata (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-c2pa (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-prov (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-api (no Rust crate changed)
⏭️  Skipped: Module crate alone: filesquid-native (no Rust crate changed)
⏭️  Skipped: filesquid-metadata all-features tests and wasm32 builds (no Rust crate changed)
⏭️  Skipped: filesquid-prov all-features tests and wasm32 builds (no Rust crate changed)
⏭️  Skipped: Rustdoc (no Rust crate changed)
⏭️  Skipped: Clippy (desktop, test-instrumented) (no Rust crate changed)
⏭️  Skipped: Desktop tests (test-instrumented) (no Rust crate changed)
✅ ESLint passed (7s).
✅ TypeScript passed (5s).
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 819ms
      Tests  1290 passed | 1 skipped (1291)
✅ Vitest passed (28s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
✅ ESLint passed (2s).
✅ TypeScript passed (3s).
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 727ms
      Tests  38 passed | 1 skipped (39)
✅ Vitest passed (3s).
ℹ️  Playwright e2e skipped for apps/imagesquid (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: npm:apps/filesquid/ui,apps/imagesquid).
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui,apps/imagesquid (75s)
```
