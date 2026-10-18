# Task UI-0.5 Walkthrough

## SRS-NFR-006: Accessibility and usability

Plan row: [UI-0.5 Accessibility contracts in the shell](../plan.md), part of global [Task FS-2.1](../../../../../resources/implementation-plan.md#task-fs-2.1). UI-F2.1 remains, so the global box stays open. Requirements: [SRS-NFR-006](../../../../../resources/srs.md#srs-nfr-006-accessibility-and-usability) and [PRD NFR-006](../../../../../resources/prd.md#nfr-006-accessibility-and-usability), [PRD NFR-007](../../../../../resources/prd.md#nfr-007-compatibility-rendering-and-localization) (locale and RTL hooks), [PRD RC-005](../../../../../resources/prd.md#rc-005-functional-and-accessible-ui-quality), [SRS-F-055](../../../../../resources/srs.md#srs-f-055-default-ui-and-panel-composition), new [UI-S-016](../srs.md) (accessibility contracts), [ADR 031](../../../../../resources/architecture/031-vanilla-typescript-ui-and-white-label-theming.md). Scenarios: [`accessibility_contracts_NFR-006.feature`](../../tests/features/accessibility_contracts_NFR-006.feature).

PRD NFR-006 in full: web interfaces MUST target WCAG 2.2 AA; desktop MUST support keyboard-only tasks, native screen readers, visible focus, scalable text and high-contrast/reduced-motion preferences. Drag-only workflows MUST have alternatives. Undo and Redo for the organization commands of F-004 MUST be reachable from the keyboard and the Edit menu on every host. Release testing MUST include import, search, edit, permission review, recovery and export on each supported platform; built UI MUST mount with rigorous DOM/state assertions and no SSR/hydration/browser errors. SRS-NFR-006 applies it to every host and every distribution.

## Changes

- **UI-S-016** ([module SRS](../srs.md)): the granular contract this task implements (keyboard, skip control, landmarks and names, focus ring and 3:1 focus contrast, focus recovery, motion, contrast preferences, text scaling and reflow, locale). [SRS-NFR-006](../../../../../resources/srs.md#srs-nfr-006-accessibility-and-usability) and the ImageSquid SRS (inherited row) point to it.
- **Shared accessibility stylesheet** ([`a11y-style.ts`](../../src/components/a11y-style.ts), `A11Y_STYLE`): first in every component's shadow root. `:focus-visible` draws `2px solid var(--fs-primary)` offset `2px`; `tabindex="-1"` focus targets draw none; `prefers-reduced-motion: reduce` sets every transition and animation duration to `0s`; `prefers-contrast: more` sets borders to `--fs-on-surface-variant`; `forced-colors: active` draws focus in `Highlight`.
- **`<fs-app-shell>`** ([`fs-app-shell.ts`](../../src/components/fs-app-shell.ts)): a skip control (`skipToContent` label) first in the Tab order that focuses `main` without changing the hash; `nav` named by the new `navigationMain` label (it was the hard-coded English "Main"); `main tabindex="-1"`; public `focusMain()`; focus recovery to `main` when a re-render removes the focused element. Layout: grid host without the fixed `calc(100vh - 57px)`, header that wraps, `2rem` logo, `15rem` navigation that stacks above `main` below `40rem`, logical properties only.
- **`<fs-theme-control>`** ([`fs-theme-control.ts`](../../src/components/fs-theme-control.ts)): colour changes take `--fs-motion-duration`; focus ring offset now `2px`; under forced colours the chosen option is `Highlight` / `HighlightText`; a re-render keeps focus on the chosen radio.
- **`<fs-monitor>`** ([`fs-monitor.ts`](../../src/components/fs-monitor.ts)): tables named by their section heading inside focusable, named scroll regions (`role="region"`, `tabindex="0"`), so a 320 px window scrolls the table, not the page; focus recovery to the token field, the alert, or the first section heading after sign-in; logical properties (`text-align: start`, `border-block-end`, `margin-block-end`); wrapping forms.
- **Route focus** ([`routes.ts`](../../src/app/routes.ts)): after a `hashchange` (not on first apply) focus moves to the shell's `main` (`RouteHost.focusMain`).
- **Helpers** ([`src/a11y/`](../../src/a11y/)), exported from `@filesquid/ui`: `accessibilityViolations` and `accessibleName` (positive tabindex, unnamed controls, missing `alt`, unnamed landmarks, through open shadow roots); `deepActiveElement`, `focusElement`, `replaceKeepingFocus`, `recoverFocus`; `Distribution.locale`, `DEFAULT_LOCALE`, `resolveLocale` (refuses a `dir` other than `ltr` or `rtl`, or a blank language), `applyLocale`; `prefersReducedMotion` and `REDUCED_MOTION_QUERY` for script-driven motion (the hover card of UI-S-006).
- **Tokens and contrast** ([`tokens.css`](../../src/theme/tokens.css), [`tokens.ts`](../../src/theme/tokens.ts), [`contrast.ts`](../../src/theme/contrast.ts)): theme-independent `--fs-motion-duration: 150ms` (required of every distribution); `NON_TEXT_CONTRAST = 3` and `FOCUS_CONTRAST_PAIRS` (`--fs-primary` on `--fs-surface`, `--fs-surface-container`, `--fs-primary-container`), checked by `contrastFailures` alongside the nine text pairs.
- **Both entry points** apply the distribution's locale to `<html lang dir>` before mounting.
- **ImageSquid**: `skipToContent`, `navigationMain` labels and `--fs-motion-duration` added; its UI test runs `accessibilityViolations`; its e2e checks Tab order, names and the focus ring in its own colours in light and dark.
- **Docs**: [UI README](../../README.md) section "Accessibility contracts" and the motion token row; plan row and [checklist](../../../../../resources/implementation-plan-checklist.md) ticked; feature file written with the feature-writer skill.

## Decisions and assumptions for the owner and the platform engineer

1. **No axe-core.** No ADR approves an accessibility-audit package, so none was added. The audit is a small in-package DOM check plus Playwright's ARIA snapshot (Chromium's own accessible names). Adding axe-core later needs an ADR 017 amendment.
2. **Forced colours use system colour keywords.** `Highlight` and `HighlightText` are the only non-token colours, allowed only inside `@media (forced-colors: active)` blocks and checked by `component-styles.test.ts`. That is how forced-colours mode works; tokens cannot express it.
3. **Skip control is a button.** A `#main` link would change the hash, which is the router, so the skip control is a `<button>` that focuses `main`.
4. **Text scaling is tested as root font size and narrow viewport.** Playwright cannot change the browser's default font size; 200 % `html` font size covers `rem` scaling, and a 640 px viewport at 200 % stands in for 200 % zoom on a 1280 px window.
5. **No API change.** Nothing here needs a new route or field.
6. **Not in this task**: Undo and Redo for F-004 organization commands (they arrive with those commands; UI-F2.1 and later) and drag alternatives (no drag exists yet). The helpers here are what those tasks use.

## SRS traceability (UI-S-016)

| Constraint | Tests |
|---|---|
| Native controls only; no positive tabindex | `a11y-audit.test.ts` (19); `accessibility.test.ts` "passes the accessibility audit", "names the navigation ..."; ImageSquid `distribution.test.ts` "inherits the FileSquid accessibility contracts" |
| Tab order skip → theme → nav | e2e "Tab order and a 2px primary focus ring in the light/dark theme"; ImageSquid e2e "inherits the keyboard, naming and focus contracts in light/dark" |
| Skip control: first, hidden until focused, focuses `main`, hash unchanged | `accessibility.test.ts` "starts with a skip control ...", "the skip control moves focus to main without changing the hash"; e2e "the skip control appears on focus ..." |
| Landmarks, one `h1`, names from the label table, named table regions | e2e ARIA snapshot "landmarks, headings and control names come from the label table"; `accessibility.test.ts` "names the navigation ...", "passes the audit when loaded and names each table ..."; e2e "opening the Monitor ..." (region and table by name) |
| Focus ring 2px solid primary, offset 2px, light and dark | `component-styles.test.ts` "the shared stylesheet draws a 2px primary focus ring ..."; e2e Tab-order tests compare the computed outline with the computed `--fs-primary` in both themes (FileSquid and ImageSquid) |
| Primary at 3:1 on surface, surface-container, primary-container | `theme.test.ts` "checks the focus ring (primary) at 3:1 ...", "reports a focus ring below 3:1 on the sidebar ...", "accepts a focus pair between 3:1 and 4.5:1 ..."; ImageSquid `theme.test.ts` "meet WCAG 2.2 AA" (now includes the focus pairs) |
| Focus recovery: route → main; token form → field; alert → alert; loaded → first heading; not on first load | `focus.test.ts` (16); `accessibility.test.ts` "moves focus to main when a re-render ...", "does not take focus on first render", "focuses main after a hashchange but not on the first apply", "moves focus to the first section heading after a keyboard sign-in", "puts focus back on the new token field ...", "moves focus to the alert ...", "does not take focus when it loads in the background", "keeps focus where it is outside the Monitor when a poll fails", "moves focus to the alert when a poll fails while focus is in the replaced dashboard"; `theme-control.test.ts` "keeps focus on the chosen radio when reconfigured while focused"; e2e "opening the Monitor from the keyboard ...", "a keyboard sign-in to the Monitor keeps focus in the Monitor" |
| `--fs-motion-duration` 150ms, durations from it, 0s under reduce, `prefersReducedMotion` | `theme.test.ts` "requires a theme-independent motion duration of 150ms"; `component-styles.test.ts` "takes motion durations from --fs-motion-duration", "stops motion under prefers-reduced-motion"; `preferences.test.ts` (3 motion tests); e2e "reduced motion stops every transition" (every element of every shadow root, Monitor included) |
| Borders on-surface-variant under more contrast; Highlight under forced colours; system colours only there | `component-styles.test.ts` (shared blocks, "uses system colours only inside forced-colors blocks"); e2e "more contrast strengthens borders; forced colours keep the chosen option" |
| `rem` sizes, no fixed pixel heights, no horizontal scroll at 320 px, at 200 % and at both together, nav stacks below 40rem | `component-styles.test.ts` "fixes no text container to a pixel height" (`height` and `block-size` forms); e2e "no horizontal scroll at 320 px wide and 100 % / 200 % text", "... 640 px ... 200 %", "... 1280 px ... 200 %" (title size doubles, not clipped, Monitor tables scroll inside their region) |
| Locale and RTL: `Distribution.locale`, `applyLocale`, refused `dir`, logical properties | `preferences.test.ts` (7 locale tests); `component-styles.test.ts` "uses logical properties only"; e2e "landmarks ..." (`lang="en"`, `dir="ltr"`), "a right-to-left locale mirrors the header and navigation" |
| Audit follows the accname rules (name-required roles, `alt` only on images, descendant image `alt`, input `value`, no content name for `select`/`textarea`, `:disabled` and `inert` skipped) | `a11y-audit.test.ts` "accessibleName and the audit follow the accname rules" (8), "role tokens and content text" (4: first role token as the effective role, content names from text nodes only) |
| Slotted light-DOM content gets the shared rules; the copy lives only while a shell is connected | `accessibility.test.ts` "keeps one document-level copy while any shell is connected and removes it after the last", "installs its own style even when an unrelated element already has the id", "keeps the document-level style helpers internal"; e2e "light-DOM content slotted into the shell gets the same focus ring" |
| Locale tag trimmed and canonical; a malformed tag or direction throws at startup naming the distribution | `preferences.test.ts` "trims the language tag ...", "refuses a malformed language tag" |
| Distributions inherit (shared stylesheet, exported audit) | `component-styles.test.ts` "includes the shared accessibility stylesheet" (3, asserting the rendering expression); `a11y-audit.test.ts` "is exported ..."; ImageSquid UI and e2e tests above |
| UI-S-011, UI-S-013: built bundle, custom elements defined, no console errors | every e2e test asserts no console or page errors against `npm run build` and `filesquid-server` |

Coverage: no Vitest coverage reporter is approved in [ADR 017](../../../../../resources/architecture/017-test-runners-and-reporting.md). Every new exported function has a direct unit test: `accessibilityViolations` and `accessibleName` in `a11y-audit.test.ts`; `deepActiveElement`, `focusElement`, `replaceKeepingFocus` and `recoverFocus` in `focus.test.ts`; `resolveLocale`, `applyLocale` and `prefersReducedMotion` in `preferences.test.ts`; `effectiveRole` in `a11y-audit.test.ts`; the internal `retainDocumentA11yStyle` and `releaseDocumentA11yStyle` through the shell in `accessibility.test.ts`; the focus contrast pairs in `theme.test.ts`.

## Review round 1 (Cubic, 25 findings)

All 25 were fixed in code or documentation; none was disputed.

- **Slotted content:** `<fs-app-shell>` installs one document-level copy of `A11Y_STYLE`, so light-DOM content a distribution slots in gets the focus ring, motion and contrast rules. UI-S-016 now says so.
- **Audit (6 findings, one cause):** `accessibleName` follows the accname order: `alt` only on `img` and image inputs; the `value` (or the default "Submit"/"Reset") of button, submit and reset inputs; content names, including descendant image `alt`, only for roles that take their name from content (never `select`, `textarea`, slider or textbox). The audit checks every name-required widget role (switch, combobox, slider, menuitemcheckbox, spinbutton, textbox, ...). It skips `:disabled` controls, including those inside a disabled fieldset, and anything inside `inert` or `hidden`. Results are grouped by rule across all shadow roots, as documented.
- **320 px at 200 % text:** added to the feature file and the e2e matrix. It failed at first, so the layout was fixed, not the requirement: the shell grid column can shrink (`minmax(0, 1fr)`), the theme control wraps, and the Monitor's term and value lists wrap. UI-S-016 and the README say "at both together".
- **Focus:** the theme control's re-render now targets the new fieldset's chosen radio. `focusElement` leaves natively focusable elements (`summary`, editing hosts, `iframe`, media with controls, `area[href]`) alone. A failed Monitor poll keeps focus where it is unless focus was inside the replaced content; this is tested both ways and stated in UI-S-016.
- **Locale:** `resolveLocale` trims the tag, returns its canonical form (`Intl.getCanonicalLocales`) and refuses a malformed one.
- **Docs and checks:**
  - The ImageSquid SRS row is now a verbatim copy with MUST. The heading drops its date and the copy dates move to a sentence, with anchors updated in the ImageSquid PRD and plan. The front matter gains a 2026-10-18 revision note, and the RC-005 coverage row cites UI-S-016.
  - The feature file's Source paths are fixed.
  - `NON_TEXT_CONTRAST` cites only 1.4.11 at AA (2.4.13 is AAA).
  - The system-colour check is case-insensitive.
  - The pixel-height check covers the `block-size` forms.
  - The motion check flags any literal time beside the token.
  - The stylesheet check asserts the rendering expression.
  - The hash reset moved to `afterEach`.
  - `recoverFocus` has direct tests.

## Review round 2 (Cubic, 8 findings)

All 8 were fixed; none was disputed.

- **Role tokens:** the audit takes the first token of a space-separated `role` list as the effective role (`effectiveRole`). It uses that role in control discovery, landmark discovery and the content-name rule.
- **Comments:** content names come from text nodes only.
- **Document style:**
  - The helpers are now internal (`retainDocumentA11yStyle` / `releaseDocumentA11yStyle`, not exported from `@filesquid/ui`).
  - They look only for their own `style#fs-a11y-style` in `doc.head`.
  - They are ref-counted per document: the shell retains on connect and releases on disconnect, and the last release removes the style.
- **Docs:**
  - UI-S-016 and its ImageSquid copy say that a malformed locale tag or direction throws at startup with an error naming the distribution (tests assert the name), and that the shell removes the document copy when the last shell leaves.
  - The ImageSquid PRD's NFR-006 and RC-005 surface lists cite UI-S-016.
  - Test counts in this walkthrough are updated.
- **Existing e2e race (found while re-running):** `app.spec.ts` "design tokens switch between light and dark" failed once in a full run because the theme store's OS-change listener can run after the test sets `data-theme` and undo it. The test now waits for the theme control to report `data-resolved="dark"` before the override. It passed 10 of 10 repeats and the full suite afterwards.

## Test output

Lint and types, both packages: `npx prettier --write .`, `npx eslint . --fix` and `npx tsc --noEmit` exit 0.

### FileSquid Vitest (`npm test` in `apps/filesquid/ui`)

```text
> @filesquid/ui@0.1.0 pretest
> rm -rf tests/allure-results


> @filesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 <repo>/apps/filesquid/ui

 ✓ tests/unit/component-styles.test.ts (42 tests) 31ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 35ms
 ✓ tests/unit/client-io.test.ts (22 tests) 78ms
 ✓ tests/unit/theme.test.ts (26 tests) 19ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 607ms
 ✓ tests/ui/monitor.test.ts (14 tests) 264ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 109ms
 ✓ tests/ui/app-shell.test.ts (15 tests) 352ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 24ms
 ✓ tests/unit/commands.test.ts (45 tests) 20ms
 ✓ tests/unit/focus.test.ts (16 tests) 69ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 68ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 68ms
 ✓ tests/unit/admin-client.test.ts (6 tests) 14ms
 ✓ tests/unit/client.test.ts (8 tests) 12ms
 ✓ tests/unit/contrast.test.ts (13 tests) 21ms
 ✓ tests/unit/preferences.test.ts (10 tests) 23ms
 ✓ tests/unit/distribution.test.ts (3 tests) 6ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 24ms
 ✓ tests/unit/contract.test.ts (1 test) 114ms

 Test Files  20 passed (20)
      Tests  328 passed (328)
   Start at  07:57:39
   Duration  7.70s (transform 1.22s, setup 0ms, import 2.88s, tests 1.96s, environment 12.10s)

JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

### ImageSquid Vitest (`npm test` in `apps/imagesquid`)

```text
> @imagesquid/ui@0.1.0 pretest
> rm -rf tests/allure-results


> @imagesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 <repo>/apps/imagesquid

 ✓ tests/unit/theme.test.ts (9 tests) 9ms
 ✓ tests/ui/distribution.test.ts (4 tests) 118ms

 Test Files  2 passed (2)
      Tests  13 passed (13)
   Start at  07:58:17
   Duration  1.22s (transform 482ms, setup 0ms, import 564ms, tests 128ms, environment 573ms)

JUNIT report written to <repo>/apps/imagesquid/tests/allure-results/vitest-junit.xml
```

### FileSquid Playwright (`npm run test:e2e`, built UI and `filesquid-server`)

```text
Running 27 tests using 2 workers

  ✓   2 [chromium] › tests/e2e/app.spec.ts:44:1 › shell mounts with FileSquid brand, internal vocabulary and live API health (313ms)
  ✓   1 [chromium] › tests/e2e/accessibility.spec.ts:88:1 › landmarks, headings and control names come from the label table (403ms)
  ✓   3 [chromium] › tests/e2e/app.spec.ts:68:1 › design tokens switch between light and dark (282ms)
  ✓   4 [chromium] › tests/e2e/accessibility.spec.ts:121:3 › Tab order and a 2px primary focus ring in the light theme (284ms)
  ✓   5 [chromium] › tests/e2e/app.spec.ts:85:1 › client-side routes fall back to the distribution index (196ms)
  ✓   6 [chromium] › tests/e2e/accessibility.spec.ts:121:3 › Tab order and a 2px primary focus ring in the dark theme (217ms)
  ✓   7 [chromium] › tests/e2e/app.spec.ts:91:1 › without a token the shell names the missing credentials from the label table (172ms)
  ✓   8 [chromium] › tests/e2e/accessibility.spec.ts:146:1 › light-DOM content slotted into the shell gets the same focus ring (286ms)
  ✓   9 [chromium] › tests/e2e/app.spec.ts:103:1 › Monitor asks for the admin token, then shows live health, processes and the journal (371ms)
  ✓  10 [chromium] › tests/e2e/accessibility.spec.ts:166:1 › the skip control appears on focus and moves focus to main without routing (264ms)
  ✓  11 [chromium] › tests/e2e/app.spec.ts:136:1 › a wrong token is named as invalid credentials (399ms)
  ✓  12 [chromium] › tests/e2e/accessibility.spec.ts:184:1 › opening the Monitor from the keyboard moves focus to main (371ms)
  ✓  13 [chromium] › tests/e2e/app.spec.ts:147:1 › the signed-in token stays in memory: not in storage, URL or DOM, gone on reload (525ms)
  ✓  15 [chromium] › tests/e2e/app.spec.ts:178:1 › the desktop launch token authorises the shell, the Monitor and the event channel (185ms)
  ✓  14 [chromium] › tests/e2e/accessibility.spec.ts:202:1 › a keyboard sign-in to the Monitor keeps focus in the Monitor (755ms)
  ✓  16 [chromium] › tests/e2e/app.spec.ts:263:3 › inside the desktop webview (custom URI scheme adapter) › requests use the custom scheme and events use Tauri, not same-origin HTTP or WebSocket (332ms)
  ✓  17 [chromium] › tests/e2e/accessibility.spec.ts:219:1 › reduced motion stops every transition (355ms)
  ✓  18 [chromium] › tests/e2e/theme.spec.ts:91:1 › official FileSquid tokens apply in light and dark and meet WCAG 2.2 AA (311ms)
  ✓  20 [chromium] › tests/e2e/theme.spec.ts:115:1 › the official mark fills the brand slot, with its dark-surface variant in dark (248ms)
  ✓  19 [chromium] › tests/e2e/accessibility.spec.ts:258:1 › more contrast strengthens borders; forced colours keep the chosen option (619ms)
  ✓  21 [chromium] › tests/e2e/theme.spec.ts:141:1 › the theme follows the OS by default and the control's choice persists across a reload (458ms)
  ✓  22 [chromium] › tests/e2e/accessibility.spec.ts:301:3 › no horizontal scroll at 320 px wide and 100 % text (244ms)
  ✓  23 [chromium] › tests/e2e/theme.spec.ts:185:1 › the selected theme option and the API status meet AA as rendered (456ms)
  ✓  24 [chromium] › tests/e2e/accessibility.spec.ts:301:3 › no horizontal scroll at 320 px wide and 200 % text (448ms)
  ✓  25 [chromium] › tests/e2e/accessibility.spec.ts:301:3 › no horizontal scroll at 1280 px wide and 200 % text (232ms)
  ✓  26 [chromium] › tests/e2e/accessibility.spec.ts:301:3 › no horizontal scroll at 640 px wide and 200 % text (231ms)
  ✓  27 [chromium] › tests/e2e/accessibility.spec.ts:324:1 › a right-to-left locale mirrors the header and navigation (160ms)

  27 passed (24.7s)
```

### ImageSquid Playwright (`npm run test:e2e`)

```text
Running 7 tests using 1 worker

  ✓  1 [chromium] › tests/e2e/app.spec.ts:22:1 › ImageSquid brand, album vocabulary and FileSquid API health (189ms)
  ✓  2 [chromium] › tests/e2e/app.spec.ts:79:1 › ImageSquid tokens override FileSquid defaults in light and dark (IS-S-008) (178ms)
  ✓  3 [chromium] › tests/e2e/app.spec.ts:104:1 › ImageSquid's theme control keeps a dark choice across a reload (312ms)
  ✓  4 [chromium] › tests/e2e/app.spec.ts:124:1 › ImageSquid names missing credentials in its own words (169ms)
  ✓  5 [chromium] › tests/e2e/app.spec.ts:133:1 › ImageSquid reaches the FileSquid Monitor with its own vocabulary (319ms)
  ✓  6 [chromium] › tests/e2e/app.spec.ts:154:3 › ImageSquid inherits the keyboard, naming and focus contracts in light (UI-S-016) (243ms)
  ✓  7 [chromium] › tests/e2e/app.spec.ts:154:3 › ImageSquid inherits the keyboard, naming and focus contracts in dark (UI-S-016) (209ms)

  7 passed (6.7s)
```
