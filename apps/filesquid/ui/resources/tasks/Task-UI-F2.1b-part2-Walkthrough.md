# Task UI-F2.1b Walkthrough, part 2: live navigation data and the compact host indicator

[Task UI-F2.1b](../plan.md) (Host privacy indicator and live navigation data) is the last module row of global [Task FS-2.1](../../../../../resources/implementation-plan.md#task-fs-2.1). Part 1, the host privacy indicator, is in [Task-UI-F2.1b-part1-Walkthrough.md](./Task-UI-F2.1b-part1-Walkthrough.md). This part adds the live navigation source over `GET /api/v1/navigation` ([API-S-021](../../../crates/api/resources/srs.md), API-F1.11, contract 0.11.0), cursor paging through the generated contract types, the breadcrumb from `ancestors`, refresh on `navigation.change`, the 503 `unavailable` state, and the owner's follow-up to part 1 (option 1a, 2026-10-19): the header indicator is now a compact badge with a keyboard-reachable popover. With it UI-F2.1b and Task FS-2.1 are complete.

## SRS mapping

| ID | Title | Covered by |
|---|---|---|
| [SRS-F-004](../../../../../resources/srs.md#srs-f-004-collections-folders-and-navigation) | Collections, folders and navigation | [`albums_folders_and_navigation_F-004.feature`](../../tests/features/albums_folders_and_navigation_F-004.feature) (live-data scenarios), `tests/ui/nav-tree-live.test.ts`, `tests/ui/navigation-view.test.ts`, `tests/e2e/preview/navigation.spec.ts` |
| [API-S-021](../../../crates/api/resources/srs.md) | Navigation listing: one level per call, opaque handles of at most 512 bytes, `ancestors`, cursor paging, `not_found`, `navigation.change` | `tests/unit/navigation-contract.test.ts` (contract check of every answer and payload), `tests/unit/navigation-client.test.ts` (route, query, errors, shared event channel), `tests/ui/nav-tree-live.test.ts`, preview `navigation.spec.ts` (paging through `next_cursor`, breadcrumb from one listing's `ancestors`, change without a reload) |
| [UI-S-012](../srs.md) | The UI reaches FileSquid only through `FileSquidClient` | `FileSquidClient.navigation()` with the generated `NavigationPage`, `NavigationChange` and `listNavigation` query types; `tests/unit/navigation-client.test.ts`; the contract drift test still passes |
| [UI-S-017](../srs.md) | Navigation preferences: handles and kinds only; a pin answered `not_found` is unavailable | existing `tests/ui/nav-tree.test.ts`, plus pins renamed after a re-read level in `nav-tree-live.test.ts` |
| [UI-S-004](../srs.md) | The UI states which host it is connected to and who can read files there; a compact header indicator, the full statement in a keyboard-reachable popover and in Settings, Privacy | `tests/ui/host-indicator.test.ts` (badge, popover, Escape, outside click, focus leaving), preview `host-indicator.spec.ts` (badge text, statement hidden until opened, Enter, Space, Escape, outside click, tokens light and dark, focus ring), [`host_privacy_indicator_UI-S-018.feature`](../../tests/features/host_privacy_indicator_UI-S-018.feature) |
| [UI-S-018](../srs.md) | The host privacy indicator takes its facts only from the host descriptor | every part-1 test still passes against the badge and popover; `buildHostStatement` test for the Settings Privacy section |
| [UI-S-001](../srs.md) | Components styled only through design tokens and worded only through the label table | `tests/unit/component-styles.test.ts` (covers `fs-navigation-view.ts` and the new indicator styles) |
| [UI-S-005](../srs.md) | A distribution's label table defines every key of the default table | the 8 new keys are in `REQUIRED_LABEL_KEYS` and ImageSquid's table (`apps/imagesquid/tests/unit/theme.test.ts`) |
| [UI-S-013](../srs.md) | UI tests against production artifacts with strict DOM assertions | `tests/e2e/preview/navigation.spec.ts` and `host-indicator.spec.ts` against `vite build` plus `vite preview`; the server-backed suite against the built UI served by `filesquid-server` |
| [UI-S-016](../srs.md) | Accessibility contracts | badge is a native button in a named group, `aria-expanded`, `aria-haspopup="dialog"`, named dialog, focus returns on Escape, polite live label; server-backed `accessibility.spec.ts` (accessibility tree, Tab order with the badge, focus rings) |

## Constraint gates

| Constraint (from the Plan) | Code | Test |
|---|---|---|
| C1 repository without `node`, children with it | `ClientNavigationSource.list`, `FileSquidClient.navigation` | `navigation-client.test.ts`, preview "the live tree fills…" |
| C2 handle 1 to 512 bytes, five kinds, `null` name only for a repository, boolean `has_children`; only contract fields kept | `parseNavigationPage` | `navigation-contract.test.ts` (512 vs 513 bytes, each bad field, extra field dropped) |
| C3 breadcrumb from one listing's `ancestors` | `<fs-navigation-view>` reads `list(node, null, 1)` | `navigation-view.test.ts`, preview breadcrumb test (`limit=1` on the wire) |
| C4 follow `next_cursor` until `null`; at most 1,000 nodes a page; `MAX_PAGES` 50 | `readLevel` (UI-F2.1a), contract bound | preview paging test (5 collections in pages of 2: 3 calls per read), contract bound test |
| C5 `not_found` is a gone destination and an unavailable pin | `FsNavigationView.fail`, `readPins` | `navigation-view.test.ts`, `nav-tree.test.ts` |
| C6 a handle over 512 bytes is refused | `parseNode` via `isHandle` | `navigation-contract.test.ts` |
| C7 `parents` re-read only those levels; `all`, `events.resync`, reconnect, the repository read all | `FsNavTree.onChange`, `levelsTouched`, `ClientNavigationSource.onEvent` | `nav-tree-live.test.ts`, `navigation-client.test.ts`, preview change test (only `l1` read, same page) |
| C8 503 is the unavailable state with Retry, never an empty tree | `failureState` (status 503), `<fs-states>` `unavailable` | `view-state.test.ts`, `states.test.ts`, `nav-tree-live.test.ts`, `navigation-view.test.ts`, preview 503 test, server-backed `app.spec.ts` (the real server answers 503) |
| C9 compact badge: icon and short label from `host_type`; statement not permanent | `<fs-host-indicator>` badge | `host-indicator.test.ts`, preview badge test |
| C10 popover: click, Enter, Space; Escape and outside click close; focus to the badge; named dialog; polite live label | `<fs-host-indicator>` popover | `host-indicator.test.ts`, preview keyboard test |
| C11 part-1 rules unchanged | the same read, event and validation code | all 27 part-1 indicator tests pass unchanged except the structure test |
| C12 statement exported for Settings, Privacy | `buildHostStatement`, `HOST_STATEMENT_STYLE` | `host-indicator.test.ts` |

## What changed

- **Client** ([`src/api/client.ts`](../../src/api/client.ts)): `navigation({ node, cursor, limit })` and the contract types `NavigationPageBody`, `NavigationChangeBody`, `NavigationQuery`. `NavigationKind` and `NavigationNode` in [`navigation/model.ts`](../../src/navigation/model.ts) are now the generated contract types.
- **Contract check** ([`navigation/contract.ts`](../../src/navigation/contract.ts)): `parseNavigationPage`, `parseNavigationChange`, `MAX_PAGE_NODES`, `MAX_CHANGE_PARENTS`.
- **Live source** ([`navigation/client-source.ts`](../../src/navigation/client-source.ts)): `ClientNavigationSource` with one shared event channel and `reconnect()` (used after a token is entered on the server host).
- **Tree** ([`fs-nav-tree.ts`](../../src/components/fs-nav-tree.ts)): targeted refresh of the levels a change names; a gone level closes; pins take new names.
- **Destination view and route** ([`fs-navigation-view.ts`](../../src/components/fs-navigation-view.ts), [`app/routes.ts`](../../src/app/routes.ts)): `<fs-navigation-view>`, `bindNavigationRoute`, `parseNavigationHash`, `NAVIGATION_ROUTE_PREFIX`.
- **States** ([`view-state.ts`](../../src/components/view-state.ts), [`fs-states.ts`](../../src/components/fs-states.ts)): the `unavailable` state for HTTP 503, an alert with Retry. The file browser and Monitor get it too through `failureState`.
- **Host indicator** ([`fs-host-indicator.ts`](../../src/components/fs-host-indicator.ts), [`host-statement.ts`](../../src/components/host-statement.ts)): compact badge and popover; the statement builder is shared with the future Settings Privacy section. The shell places the badge beside the theme control.
- **Default distribution** ([`app/main.ts`](../../src/app/main.ts)): slots the live tree into the shell and binds the navigation route.
- **Labels** (default and ImageSquid): `navigationGone`, `navigationFilesPending`, `stateUnavailable`, `hostBadgeDesktop`, `hostBadgeServer`, `hostBadgeBrowser`, `hostBadgeLoading`, `hostBadgeUnknown`.
- **Tests updated for the new shell**: the preview suites answer the navigation route (`tests/e2e/support/navigation-api.ts`); the server-backed suites select the API status line by `p.api` (the tree's states element adds live regions), expect the tree's unavailable state, the badge in the Tab order and a third event listener; `trackErrors` excuses only the browser's own network line for a 503 from `/api/v1/navigation` (the real hosts answer that until they install the navigation service). The file-browser failure test answers 500 instead of 503 so it still proves the error state (a 503 is now the unavailable state).
- **Docs**: package [README](../../README.md), [plan.md](../plan.md) status, UI-F2.8 description (Settings Privacy section), Gherkin in [`albums_folders_and_navigation_F-004.feature`](../../tests/features/albums_folders_and_navigation_F-004.feature) and [`host_privacy_indicator_UI-S-018.feature`](../../tests/features/host_privacy_indicator_UI-S-018.feature).

## Decisions and gaps for the platform engineer

- **The file browser cannot open from a navigation node yet.** `<fs-file-browser>` lists one OCFL object (`library_id`, `object_id`, `prefix`, API-F1.3). A collection or folder node spans the files of many objects, and the handle MUST NOT be parsed, so no `FileLocation` can be derived from it. Opening the browser from the tree and breadcrumb needs an API addition: a file listing by navigation node (the library-level file views of API-S-019, for example `GET /api/v1/files/listing?node=<handle>` answering `FolderPage`). Until then `<fs-navigation-view>` shows the breadcrumb and says the destination's files cannot be listed here yet, and `#/files/...` stays.
- **Both real hosts answer 503.** Neither `filesquid-desktop` (until DE-1.1) nor `filesquid-server` installs the navigation service yet, so the shipped tree shows the unavailable state on both. The server-backed Playwright suite proves this against the real server.
- **Settings, Privacy.** No Settings page exists yet; UI-F2.8 mounts `buildHostStatement` there and adds the popover's "Privacy settings" link (the plan's UI-F2.8 description now says so).
- **Events.** The tree, the destination view and the indicator each follow the event channel; `ClientNavigationSource` shares one channel between the tree and the view, so a page opens one channel for navigation, one for the indicator and one for the Monitor when it is shown.

## How it was tested

TDD: the new unit, jsdom and Playwright tests were written first; the first run failed (modules missing, elements undefined):

```
 Test Files  8 failed (8)
      Tests  34 failed | 68 passed (102)
```

Commands in `apps/filesquid/ui` and in `apps/imagesquid`: `npx prettier --write .`, `npx eslint . --fix`, `npx tsc --noEmit`, `npm test`, and `FILESQUID_E2E=1` Playwright with both configs (`playwright.preview.config.ts`: `vite build` plus `vite preview`; `playwright.config.ts`: the built UI served by `filesquid-server`). The sandbox's Chromium build was used through a temporary config that set `executablePath` and was deleted before committing. Head-commit hook: see the pull request.

## Raw test output

### Lint and types (apps/filesquid/ui and apps/imagesquid)

```
eslint: clean
tsc: clean
prettier: All matched files use Prettier code style!
```

### npm test (apps/filesquid/ui)

```
 ✓ tests/ui/host-indicator.test.ts (38 tests) 407ms
 ✓ tests/ui/file-browser-writes.test.ts (36 tests) 1441ms
 ✓ tests/ui/file-browser.test.ts (36 tests) 1577ms
 ✓ tests/unit/component-styles.test.ts (138 tests) 62ms
 ✓ tests/ui/nav-tree.test.ts (26 tests) 756ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 415ms
 ✓ tests/ui/file-preview.test.ts (16 tests) 174ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 63ms
 ✓ tests/unit/files-source.test.ts (22 tests) 54ms
 ✓ tests/unit/client-io.test.ts (22 tests) 50ms
 ✓ tests/unit/theme.test.ts (26 tests) 25ms
 ✓ tests/ui/monitor.test.ts (14 tests) 205ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 45ms
 ✓ tests/ui/app-shell.test.ts (18 tests) 319ms
 ✓ tests/ui/navigation-view.test.ts (16 tests) 225ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 39ms
 ✓ tests/unit/media-handle.test.ts (14 tests) 74ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 129ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 31ms
 ✓ tests/unit/navigation-client.test.ts (13 tests) 32ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 26ms
 ✓ tests/ui/nav-tree-live.test.ts (9 tests) 214ms
 ✓ tests/unit/files-model.test.ts (58 tests) 51ms
 ✓ tests/unit/files-ops.test.ts (23 tests) 86ms
 ✓ tests/unit/commands.test.ts (45 tests) 40ms
 ✓ tests/unit/host-descriptor.test.ts (43 tests) 36ms
 ✓ tests/unit/files-memory-source.test.ts (14 tests) 32ms
 ✓ tests/unit/files-window.test.ts (19 tests) 18ms
 ✓ tests/unit/focus.test.ts (16 tests) 65ms
 ✓ tests/ui/states.test.ts (17 tests) 97ms
 ✓ tests/unit/navigation-contract.test.ts (30 tests) 27ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 14ms
 ✓ tests/unit/test-scripts.test.ts (10 tests) 895ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 78ms
 ✓ tests/ui/files-route.test.ts (6 tests) 434ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 50ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 13ms
 ✓ tests/unit/admin-client.test.ts (6 tests) 18ms
 ✓ tests/unit/client.test.ts (8 tests) 23ms
 ✓ tests/unit/preferences.test.ts (10 tests) 24ms
 ✓ tests/unit/host-client.test.ts (4 tests) 19ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 79ms
 ✓ tests/unit/view-state.test.ts (5 tests) 9ms
 ✓ tests/unit/contrast.test.ts (13 tests) 16ms
 ✓ tests/unit/distribution.test.ts (3 tests) 6ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 35ms
 ✓ tests/unit/contract.test.ts (1 test) 144ms
 Test Files  47 passed (47)
      Tests  975 passed (975)
   Duration  15.92s (transform 2.66s, setup 1.93s, import 5.19s, tests 8.67s, environment 22.86s)
```

### Playwright, production build (`vite build` plus `vite preview`, apps/filesquid/ui)

The new and changed specs (the full run passed all 42 tests, including `file-browser.spec.ts` and `production-build.spec.ts`):

```
✓   1 [chromium] › tests/e2e/preview/host-indicator.spec.ts:96:1 › the indicator is a defined custom element in the shell header, named from the label table (385ms)
✓   3 [chromium] › tests/e2e/preview/host-indicator.spec.ts:110:1 › the desktop host says only you can read your files, and shows the not-enrolled state (291ms)
✓   5 [chromium] › tests/e2e/preview/host-indicator.spec.ts:124:1 › a server names its operator, who can read the files, and states the at-rest duty (292ms)
✓   7 [chromium] › tests/e2e/preview/host-indicator.spec.ts:137:1 › a server without an operator name uses the unnamed-operator label (311ms)
✓   8 [chromium] › tests/e2e/preview/host-indicator.spec.ts:149:1 › the browser host says files are decrypted only in this browser (330ms)
✓  10 [chromium] › tests/e2e/preview/host-indicator.spec.ts:162:1 › enrolled backup and sync are shown apart from the host, as ciphertext only (355ms)
✓  11 [chromium] › tests/e2e/preview/host-indicator.spec.ts:187:1 › a host.change event updates the indicator without a reload or another read (330ms)
✓  13 [chromium] › tests/e2e/preview/host-indicator.spec.ts:209:1 › events.resync makes the indicator read the descriptor again (319ms)
✓  15 [chromium] › tests/e2e/preview/host-indicator.spec.ts:222:1 › loading shows no host until the descriptor arrives (1.1s)
✓  18 [chromium] › tests/e2e/preview/host-indicator.spec.ts:234:1 › a failed read shows the error state and never a guessed host, then Retry recovers (464ms)
✓  20 [chromium] › tests/e2e/preview/host-indicator.spec.ts:256:1 › the header shows a compact badge with the short host label, not the statement (305ms)
✓  22 [chromium] › tests/e2e/preview/host-indicator.spec.ts:271:1 › Enter opens the popover with the full statement; Escape closes it and returns focus (402ms)
✓  24 [chromium] › tests/e2e/preview/host-indicator.spec.ts:296:1 › computed token values are exact in light and dark (840ms)
✓  26 [chromium] › tests/e2e/preview/host-indicator.spec.ts:343:1 › the badge is reachable by keyboard and has a visible focus outline (374ms)
✓  28 [chromium] › tests/e2e/preview/navigation.spec.ts:62:1 › the live tree fills the shell's navigation slot, worded from the label table (320ms)
✓  29 [chromium] › tests/e2e/preview/navigation.spec.ts:83:1 › a level is read page by page through next_cursor until it is null (358ms)
✓  31 [chromium] › tests/e2e/preview/navigation.spec.ts:121:1 › a destination opens with a breadcrumb filled from ancestors (452ms)
✓  33 [chromium] › tests/e2e/preview/navigation.spec.ts:142:1 › a deep link to a destination shows its breadcrumb after a reload (398ms)
✓  35 [chromium] › tests/e2e/preview/navigation.spec.ts:154:1 › navigation.change reads only the named level again, without a reload (361ms)
✓  37 [chromium] › tests/e2e/preview/navigation.spec.ts:177:1 › a 503 shows the unavailable state with Retry, never an empty tree (348ms)
✓  39 [chromium] › tests/e2e/preview/navigation.spec.ts:194:1 › computed token values of the tree and breadcrumb are exact in light and dark (592ms)

  42 passed (16.7s)
```

### Playwright, built UI served by `filesquid-server` (apps/filesquid/ui)

```
  ✓  40 [chromium] › tests/e2e/navigation.spec.ts:464:3 › the tree and breadcrumb reflow at 1280 px and 200 % text (293ms)

  40 passed (55.2s)
```

### apps/imagesquid

```
 Test Files  4 passed (4)
      Tests  37 passed (37)

  4 passed (5.7s)      (playwright.preview.config.ts)
  9 passed (7.5s)      (playwright.config.ts, filesquid-server)
```
