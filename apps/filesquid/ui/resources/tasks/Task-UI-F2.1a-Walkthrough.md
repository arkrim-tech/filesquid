# Task UI-F2.1a Walkthrough: states, and the navigation tree and breadcrumb elements

[Task UI-F2.1a](../plan.md) (States, and the navigation tree and breadcrumb elements) is the first half of the former UI-F2.1, split by the owner on 2026-10-18. It needs no new API route. The host privacy indicator and the live navigation data are Task UI-F2.1b, so the global Task FS-2.1 stays open.

The data interface and the stored-choices format follow [API-S-021](../../../crates/api/resources/srs.md) (navigation listing) and [UI-S-017](../srs.md) (navigation preferences), so UI-F2.1b can plug in a live source without changing the elements. Work began before the docs PR #35 merged, from its branch; after merging `main`, the merged text was compared and matches (the only later changes are the API's kind-order wording and the UI-S-017 scenario outline, both already met).

## SRS mapping

| ID | Title | Covered by |
|---|---|---|
| [SRS-F-004](../../../../../resources/srs.md#srs-f-004-collections-folders-and-navigation) | Collections, folders and navigation (FileSquid default UI: navigation tree, pinned destinations, reorderable sections, collapse and expand, keyboard reordering, choices kept across restarts) | `tests/unit/navigation-model.test.ts`, `tests/unit/navigation-store.test.ts`, `tests/unit/navigation-source.test.ts`, `tests/unit/nav-tree-keys.test.ts`, `tests/ui/nav-tree.test.ts`, `tests/e2e/navigation.spec.ts`, [`albums_folders_and_navigation_F-004.feature`](../../tests/features/albums_folders_and_navigation_F-004.feature) |
| [SRS-F-055](../../../../../resources/srs.md#srs-f-055-default-ui-and-panel-composition) | Default UI and panel composition (distinct empty, loading, error, offline and permission-denied states; breadcrumb) | `tests/unit/view-state.test.ts`, `tests/ui/states.test.ts`, `tests/ui/breadcrumb.test.ts`, `tests/e2e/navigation.spec.ts`, [`view_states_and_breadcrumb_F-055.feature`](../../tests/features/view_states_and_breadcrumb_F-055.feature) |
| [UI-S-001](../srs.md) | Components styled only through design tokens and worded only through the label table | `tests/unit/component-styles.test.ts` (three new elements), every UI and e2e test (labels read from the table), ImageSquid `tests/ui/distribution.test.ts` and `tests/e2e/app.spec.ts` |
| [UI-S-005](../srs.md) | A distribution's label table defines every key of the default table | ImageSquid `tests/unit/theme.test.ts` |
| [UI-S-013](../srs.md) | UI tests against production artifacts with strict DOM assertions | `tests/e2e/navigation.spec.ts` (built UI served by `filesquid-server`: elements defined, labels from the table, token colours in light and dark, no console errors) |
| [UI-S-016](../srs.md) | Accessibility contracts | `tests/unit/component-styles.test.ts`, `tests/ui/*.test.ts` (audit, focus recovery, roving `tabindex`), `tests/e2e/navigation.spec.ts` (focus ring in light and dark, reduced motion, forced colours, reflow at 320 px and 200 % text) |
| [UI-S-017](../srs.md) | Navigation preferences: pins, section order and collapse state kept on the device under `filesquid.navigation` | [`navigation_preferences_UI-S-017.feature`](../../tests/features/navigation_preferences_UI-S-017.feature), `tests/unit/navigation-store.test.ts`, `tests/unit/navigation-model.test.ts`, `tests/ui/nav-tree.test.ts`, `tests/e2e/navigation.spec.ts` |

## What changed

- **`<fs-states>`** ([`src/components/fs-states.ts`](../../src/components/fs-states.ts)): loading and empty in a persistent polite `role="status"` region; error, offline and permission denied (with the denial reason's label) in a persistent `role="alert"` region. Both regions are updated in place so screen readers announce changes. Error and offline offer Retry, which dispatches a bubbling, composed `fs-retry` event. When a change removes the focused Retry button, focus moves to the region that shows the state. [`view-state.ts`](../../src/components/view-state.ts) maps a failure to offline (browser offline), denied (`ApiError` with a denial reason) or error.
- **`<fs-nav-tree>`** ([`src/components/fs-nav-tree.ts`](../../src/components/fs-nav-tree.ts), [`nav-tree-view.ts`](../../src/components/nav-tree-view.ts), [`nav-tree-keys.ts`](../../src/components/nav-tree-keys.ts)): reads the tree level by level from an injected `NavigationSource` shaped on API-S-021 (opaque `node` handle, `kind`, `name` or `null`, `has_children`, `ancestors`, `next_cursor`, `not_found`); it never fetches by itself. It groups a library's children into Collections, Virtual collections and Folders sections after a Pinned section; with several libraries each section lists the libraries as parents that are open by default. Each section has a heading, Move up and Move down buttons (`aria-disabled` at the ends; Alt+Shift+Up/Down anywhere in the section) and a WAI-ARIA treeview (`a[role=treeitem][href]`, `aria-owns` groups, `aria-level`, `aria-expanded`, `aria-busy` while children load, `aria-current="page"`, one Tab stop, the treeview keys, Right and Left swapped in RTL), followed by a Pin toggle (`aria-pressed`) for the tree's active item. A pin the source answers `not_found` for shows as "Collection (unavailable)" with a Remove button.
- **Navigation choices** ([`src/navigation/store.ts`](../../src/navigation/store.ts), [`model.ts`](../../src/navigation/model.ts)): kept on the device in Web Storage under `filesquid.navigation` as `{ version: 1, pinned: [{ node, kind }], order, collapsed }`. Only handles, kinds and section ids are stored. Bounds: 100 pins, 1,000 collapsed nodes, 512-byte handles and 16 section ids. A value that does not parse, has another version, has another field or breaks a bound is ignored with a console message. Refused storage is handled like the theme store's (shared [`device-storage.ts`](../../src/app/device-storage.ts)): the choices apply until the page closes and the console says why.
- **Sources and loading** ([`src/navigation/source.ts`](../../src/navigation/source.ts), [`loader.ts`](../../src/navigation/loader.ts)): `MemoryNavigationSource` answers from a fixture tree exactly as the listing does (paging, ancestors, `not_found`, `invalid_request`). The loader reads whole levels (at most 50 pages each), the start of the tree, and pins through their `ancestors`.
- **`<fs-breadcrumb>`** ([`src/components/fs-breadcrumb.ts`](../../src/components/fs-breadcrumb.ts)): a `nav` named by `navigationBreadcrumb`, an ordered list of links with `aria-current="page"` on the last one, and CSS separators hidden from assistive technology. It is hidden while empty. `breadcrumbItems(ancestors, labels)` builds the trail from a listing.
- **`<fs-app-shell>`**: a `navigation` named slot in the `nav` landmark, with the static destination list as fallback, so UI-F2.1b only slots the tree in. The Monitor link now sits in its own list after the slot (the e2e aria snapshot was updated).
- **Labels** (default and ImageSquid): `folder`, `folders`, `navigationPinned`, `navigationPin`, `navigationPinItem`, `navigationUnavailable`, `navigationRemove`, `navigationRemoveItem`, `navigationMoveUp`, `navigationMoveDown`, `navigationBreadcrumb`, `stateLoading`, `stateEmpty`, `stateError`, `stateOffline`, `stateDenied`, `stateRetry`. Templates use `{name}` and `{kind}` placeholders filled by `formatLabel`. No new tokens were needed.
- **Docs**: package [README](../../README.md) section "Navigation, breadcrumb and view states"; Gherkin scenarios in [`albums_folders_and_navigation_F-004.feature`](../../tests/features/albums_folders_and_navigation_F-004.feature) and the new [`view_states_and_breadcrumb_F-055.feature`](../../tests/features/view_states_and_breadcrumb_F-055.feature).

## How it was tested

TDD: the first test run, before any implementation, failed as expected:

```
 Test Files  9 failed | 18 passed (27)
      Tests  45 failed | 327 passed (372)
```

Midway through, the docs PR #35 branch appeared with API-S-021 and UI-S-017. The model, store, view and element were realigned to them, and their tests were rewritten first against the new shapes. Commands, run inside each package: `npx prettier --write .`, `npx eslint . --fix`, `npx tsc --noEmit`, `npm test`, and `FILESQUID_E2E=1 npx playwright test` (built UI served by the real `filesquid-server`). JUnit reports are in each package's `tests/allure-results/`.

No environment variables or credentials were mocked. The e2e tests inject navigation data through an in-page fixture source, not through a stubbed API route.

## Review round 1 (Cubic, 21 findings)

Every finding was valid and fixed in one commit, with tests for each behaviour fix:

- `<fs-nav-tree>`: a pin read that fails (other than `not_found`) is retried only after the next load or store change, never in a loop. Pin and child answers that arrive after a newer load, or from a replaced source, are dropped. The Pinned section, with its Remove buttons, still shows when the repository has no library.
- `failureState`: a denial is reported as denied even while the browser is offline (tests include the 403 `invalid_credentials` case).
- Stored choices: an explicit `null` list and a stored empty string are refused with a console message. The store follows `storage` events, so one tab no longer overwrites another tab's choices.
- Breadcrumb: `breadcrumbItems` and `<fs-breadcrumb>` keep the last `MAX_DEPTH` nodes, so the current node stays last. Focus stays on a link whose `href` survives a re-render.
- `<fs-states>`: after a failed Retry (loading, then error), focus moves to the alert.
- Loader: at most 64 libraries (`MAX_LIBRARIES`) are read at start, through `Promise.allSettled`, so one failed library does not fail the tree.
- `MemoryNavigationSource`: limits that are not integers from 1 to 1,000 are refused with `invalid_request`. Cursors are opaque random tokens; one this source did not issue for that parent, or issued before `replace`, is refused.
- Tests and docs: the serialisation test now covers names, titles, logical paths, tokens and content handles. The breadcrumb scenario asserts the full trail. The plan's Related Tests list every test file this task changes.

## Raw test output

### `npm test` in `apps/filesquid/ui`

```

 RUN  v4.1.12 apps/filesquid/ui

 ✓ tests/unit/component-styles.test.ts (81 tests) 25ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 102ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 928ms
     ✓ keeps one document-level copy while any shell is connected and removes it after the last  333ms
 ✓ tests/ui/nav-tree.test.ts (26 tests) 1946ms
     ✓ moves through visible items with the arrow keys, Home and End  486ms
 ✓ tests/unit/client-io.test.ts (22 tests) 62ms
 ✓ tests/unit/theme.test.ts (26 tests) 40ms
 ✓ tests/ui/monitor.test.ts (14 tests) 297ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 30ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 318ms
 ✓ tests/ui/app-shell.test.ts (16 tests) 803ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 57ms
 ✓ tests/unit/navigation-store.test.ts (17 tests) 126ms
 ✓ tests/unit/commands.test.ts (45 tests) 13ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 125ms
 ✓ tests/unit/focus.test.ts (16 tests) 318ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 85ms
 ✓ tests/ui/states.test.ts (16 tests) 313ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 133ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 18ms
 ✓ tests/unit/client.test.ts (8 tests) 18ms
 ✓ tests/unit/admin-client.test.ts (6 tests) 27ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 26ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 177ms
 ✓ tests/unit/preferences.test.ts (10 tests) 37ms
 ✓ tests/unit/view-state.test.ts (4 tests) 9ms
 ✓ tests/unit/contrast.test.ts (13 tests) 8ms
 ✓ tests/unit/distribution.test.ts (3 tests) 4ms
 ✓ tests/unit/contract.test.ts (1 test) 79ms

 Test Files  28 passed (28)
      Tests  493 passed (493)
   Start at  16:36:33
   Duration  18.73s (transform 2.05s, setup 0ms, import 4.81s, tests 6.12s, environment 30.57s)

JUNIT report written to tests/allure-results/vitest-junit.xml
```

### `FILESQUID_E2E=1 npx playwright test` in `apps/filesquid/ui`

```
  ✓   1 [chromium] › tests/e2e/accessibility.spec.ts:88:1 › landmarks, headings and control names come from the label table (562ms)
  ✓   2 [chromium] › tests/e2e/app.spec.ts:44:1 › shell mounts with FileSquid brand, internal vocabulary and live API health (599ms)
  ✓   3 [chromium] › tests/e2e/accessibility.spec.ts:122:3 › Tab order and a 2px primary focus ring in the light theme (466ms)
  ✓   4 [chromium] › tests/e2e/app.spec.ts:68:1 › design tokens switch between light and dark (420ms)
  ✓   5 [chromium] › tests/e2e/accessibility.spec.ts:122:3 › Tab order and a 2px primary focus ring in the dark theme (471ms)
  ✓   6 [chromium] › tests/e2e/app.spec.ts:85:1 › client-side routes fall back to the distribution index (316ms)
  ✓   8 [chromium] › tests/e2e/app.spec.ts:91:1 › without a token the shell names the missing credentials from the label table (374ms)
  ✓   7 [chromium] › tests/e2e/accessibility.spec.ts:147:1 › light-DOM content slotted into the shell gets the same focus ring (418ms)
  ✓   9 [chromium] › tests/e2e/app.spec.ts:103:1 › Monitor asks for the admin token, then shows live health, processes and the journal (411ms)
  ✓  10 [chromium] › tests/e2e/accessibility.spec.ts:167:1 › the skip control appears on focus and moves focus to main without routing (419ms)
  ✓  11 [chromium] › tests/e2e/app.spec.ts:136:1 › a wrong token is named as invalid credentials (385ms)
  ✓  12 [chromium] › tests/e2e/accessibility.spec.ts:185:1 › opening the Monitor from the keyboard moves focus to main (481ms)
  ✓  13 [chromium] › tests/e2e/app.spec.ts:147:1 › the signed-in token stays in memory: not in storage, URL or DOM, gone on reload (662ms)
  ✓  14 [chromium] › tests/e2e/accessibility.spec.ts:203:1 › a keyboard sign-in to the Monitor keeps focus in the Monitor (704ms)
  ✓  15 [chromium] › tests/e2e/app.spec.ts:178:1 › the desktop launch token authorises the shell, the Monitor and the event channel (347ms)
  ✓  16 [chromium] › tests/e2e/accessibility.spec.ts:220:1 › reduced motion stops every transition (338ms)
  ✓  17 [chromium] › tests/e2e/app.spec.ts:263:3 › inside the desktop webview (custom URI scheme adapter) › requests use the custom scheme and events use Tauri, not same-origin HTTP or WebSocket (473ms)
  ✓  18 [chromium] › tests/e2e/accessibility.spec.ts:259:1 › more contrast strengthens borders; forced colours keep the chosen option (537ms)
  ✓  19 [chromium] › tests/e2e/navigation.spec.ts:204:1 › the elements are defined and worded from the label table inside the shell (277ms)
  ✓  20 [chromium] › tests/e2e/accessibility.spec.ts:302:3 › no horizontal scroll at 320 px wide and 100 % text (411ms)
  ✓  21 [chromium] › tests/e2e/navigation.spec.ts:250:1 › the tree follows the ARIA treeview keys and reads children as nodes open (575ms)
  ✓  22 [chromium] › tests/e2e/accessibility.spec.ts:302:3 › no horizontal scroll at 320 px wide and 200 % text (457ms)
  ✓  23 [chromium] › tests/e2e/navigation.spec.ts:286:1 › pins, section order and library collapse are kept on the device across a reload (593ms)
  ✓  24 [chromium] › tests/e2e/accessibility.spec.ts:302:3 › no horizontal scroll at 1280 px wide and 200 % text (401ms)
  ✓  25 [chromium] › tests/e2e/navigation.spec.ts:327:1 › refused device storage keeps the choices for this page and says why (279ms)
  ✓  26 [chromium] › tests/e2e/accessibility.spec.ts:302:3 › no horizontal scroll at 640 px wide and 200 % text (556ms)
  ✓  27 [chromium] › tests/e2e/navigation.spec.ts:352:1 › the tree shows the error and offline states and reads again on Retry (401ms)
  ✓  28 [chromium] › tests/e2e/accessibility.spec.ts:325:1 › a right-to-left locale mirrors the header and navigation (296ms)
  ✓  29 [chromium] › tests/e2e/navigation.spec.ts:374:3 › the five states are distinct and use token colours in the light theme (394ms)
  ✓  30 [chromium] › tests/e2e/theme.spec.ts:91:1 › official FileSquid tokens apply in light and dark and meet WCAG 2.2 AA (340ms)
  ✓  31 [chromium] › tests/e2e/navigation.spec.ts:417:3 › tree controls draw the 2px primary focus ring in the light theme (248ms)
  ✓  32 [chromium] › tests/e2e/theme.spec.ts:115:1 › the official mark fills the brand slot, with its dark-surface variant in dark (324ms)
  ✓  33 [chromium] › tests/e2e/navigation.spec.ts:374:3 › the five states are distinct and use token colours in the dark theme (321ms)
  ✓  35 [chromium] › tests/e2e/navigation.spec.ts:417:3 › tree controls draw the 2px primary focus ring in the dark theme (404ms)
  ✓  34 [chromium] › tests/e2e/theme.spec.ts:141:1 › the theme follows the OS by default and the control's choice persists across a reload (571ms)
  ✓  37 [chromium] › tests/e2e/theme.spec.ts:185:1 › the selected theme option and the API status meet AA as rendered (314ms)
  ✓  36 [chromium] › tests/e2e/navigation.spec.ts:440:1 › tree motion follows the token, stops under reduced motion, and forced colours mark the current item (483ms)
  ✓  38 [chromium] › tests/e2e/navigation.spec.ts:481:3 › the tree and breadcrumb reflow at 320 px and 100 % text (256ms)
  ✓  39 [chromium] › tests/e2e/navigation.spec.ts:481:3 › the tree and breadcrumb reflow at 320 px and 200 % text (217ms)
  ✓  40 [chromium] › tests/e2e/navigation.spec.ts:481:3 › the tree and breadcrumb reflow at 1280 px and 200 % text (244ms)
  40 passed (2.2m)
```

### `npm test` in `apps/imagesquid`

```

 RUN  v4.1.12 apps/imagesquid

 ✓ tests/unit/theme.test.ts (10 tests) 35ms
 ✓ tests/ui/distribution.test.ts (5 tests) 345ms

 Test Files  2 passed (2)
      Tests  15 passed (15)
   Start at  16:39:20
   Duration  2.40s (transform 772ms, setup 0ms, import 909ms, tests 381ms, environment 1.26s)

JUNIT report written to tests/allure-results/vitest-junit.xml
```

### `FILESQUID_E2E=1 npx playwright test` in `apps/imagesquid`

```
  ✓  1 [chromium] › tests/e2e/app.spec.ts:23:1 › ImageSquid brand, album vocabulary and FileSquid API health (540ms)
  ✓  2 [chromium] › tests/e2e/app.spec.ts:80:1 › ImageSquid tokens override FileSquid defaults in light and dark (IS-S-008) (224ms)
  ✓  3 [chromium] › tests/e2e/app.spec.ts:105:1 › ImageSquid's theme control keeps a dark choice across a reload (384ms)
  ✓  4 [chromium] › tests/e2e/app.spec.ts:125:1 › ImageSquid names missing credentials in its own words (188ms)
  ✓  5 [chromium] › tests/e2e/app.spec.ts:134:1 › ImageSquid reaches the FileSquid Monitor with its own vocabulary (386ms)
  ✓  6 [chromium] › tests/e2e/app.spec.ts:155:3 › ImageSquid inherits the keyboard, naming and focus contracts in light (UI-S-016) (411ms)
  ✓  7 [chromium] › tests/e2e/app.spec.ts:155:3 › ImageSquid inherits the keyboard, naming and focus contracts in dark (UI-S-016) (339ms)
  ✓  8 [chromium] › tests/e2e/app.spec.ts:202:3 › the navigation tree and view states speak ImageSquid in light (UI-F2.1a) (285ms)
  ✓  9 [chromium] › tests/e2e/app.spec.ts:202:3 › the navigation tree and view states speak ImageSquid in dark (UI-F2.1a) (330ms)
  9 passed (28.0s)
```
