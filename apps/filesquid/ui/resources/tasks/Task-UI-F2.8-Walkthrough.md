# Task UI-F2.8 Walkthrough: Drive-style views, sorting, selection and preferences

Branch `feat/ui-f2.8-drive-views`. Module row of global Task FS-2.5. Plan row: [Task UI-F2.8](../plan.md). Mockups (active suite v3): [05](../../../../../resources/ui-mockups/v3/05-all-files-list.svg), [05 dark](../../../../../resources/ui-mockups/v3/05-all-files-list-dark.svg), [06](../../../../../resources/ui-mockups/v3/06-all-files-grid.svg), [10](../../../../../resources/ui-mockups/v3/10-new-and-item-menus.svg), [11](../../../../../resources/ui-mockups/v3/11-selection-bulk-actions.svg), [12](../../../../../resources/ui-mockups/v3/12-zip-download.svg), [13](../../../../../resources/ui-mockups/v3/13-move-copy-picker.svg), [14](../../../../../resources/ui-mockups/v3/14-rename-and-delete.svg), [15](../../../../../resources/ui-mockups/v3/15-recent-view.svg), [32](../../../../../resources/ui-mockups/v3/32-settings.svg), [33](../../../../../resources/ui-mockups/v3/33-state-loading.svg), [44](../../../../../resources/ui-mockups/v3/44-rename-refusals.svg), [56](../../../../../resources/ui-mockups/v3/56-view-states.svg).

## SRS mapping

| SRS ID and title | What was built | Proof |
|---|---|---|
| [UI-S-006](../srs.md) `<fs-file-browser>` MUST provide the sidebar views ... list as default layout ... column-header sort with reverse and a grid sort menu; row and header checkboxes with a bulk action bar that states the effective scope; the per-item and right-click menu; shared and link badges; preview-or-download on click; an optional hover card ...; a start-view setting; and persisted per-user preferences read through the API | `<fs-drive-view>` (list default, grid, column-head sort, grid Sort menu, folder placement, selection and bulk bar, item and right-click menu, hover card, Recent, states), the sidebar's All files, Recent, view filter and storage block, the Browsing section of Settings (default layout, sort, folders, start view, hover card) and `PreferencesService`. Open items are in "What is not built" below | `tests/ui/drive-view.test.ts`, `tests/ui/drive-shell.test.ts`, `tests/unit/drive-*.test.ts`, `tests/e2e/preview/drive-view.spec.ts` |
| [UI-S-017](../srs.md) The navigation tree's pinned destinations, section order and collapse state MUST persist across restarts ... the UI MUST keep them [on the API] ... copy that value there once and then remove the key from the device | `PreferencesService` reads `GET /api/v1/preferences`, hands the navigation choices to the shared `NavigationStore` (`useRemote`, `replace`), copies a device `filesquid.navigation` value to the API once and removes the key; a host that stores none keeps the device path | `tests/unit/user-preferences.test.ts`, the UI-S-017 Playwright test, `navigation_preferences_UI-S-017.feature` |
| [UI-S-002](../srs.md) The default UI MUST provide a Drive-style browser ... rename or move, remove and restore ... **Destination** | Rename (the renames route), remove with Undo (removals and restorations), the destination picker that offers only collections and folders inside them, and the New menu whose upload items are enabled only for a collection or folder (and, until UI-F2.17 and API-F2.14, say so) | `drive-view.test.ts`, `drive-shell.test.ts`, the rename, remove and move Playwright tests |
| [UI-S-024](../srs.md) Visual constants of the active mockup suite | Every colour through a token in light and dark (rows, badges, bulk bar, menus, dialogs, toast, inputs, checkboxes), Georgia titles at 30, 22 and 20 px, 14, 13, 11 and 10 px text, 8 px spacing scale, the radii, 36 and 38 px controls, 1.5 px control borders (compared with this browser's rendering of 1.5 px), flat fills; no new token was needed | computed-style assertions in the Playwright spec, `tests/unit/component-styles.test.ts` (now also scans `drive-*.ts`, `fs-drive-view.ts`, `fs-new-menu.ts`, `settings-browsing.ts`, `fs-destination-picker.ts`) |
| [UI-S-025](../srs.md) Shell composition | The New button opens `<fs-new-menu>`; the view filter narrows only the current view (`fs-filter`); All files and Recent routes; the footer summary and commit state from the view; the storage block (bytes the open library keeps, free space) | the filter, loading, storage and Recent Playwright tests |
| [UI-S-026](../srs.md) Browser views, menus and dialogs | List (38 px header, 44 px rows, 30 x 30 px actions button on hover or focus, badge classes), grid (214 x 218 px tiles, five columns, 18 px gaps, 202 x 154 px thumbnail), Sort menu, New menu (232 px), item menu (240 px, 34 px items, Remove in the error colour), selection scope, ZIP dialog (640 x 520 px), Move or copy (600 x 640 px), Rename (560 x 250 px) and its refusals, the 440 x 44 px toast with Undo, Recent day groups, loading rows, view states | region boxes within 2 px in the Playwright spec |
| [UI-S-033](../srs.md) Settings page composition | The Browsing section between Appearance and Privacy, with the help line and the status note | the Browsing Playwright tests and `drive-shell.test.ts` |
| [UI-S-005](../srs.md) A distribution's label table MUST define every key of the default table | 95 new keys in `REQUIRED_LABEL_KEYS`, the default table and ImageSquid's table (words copied) | `tests/unit/distribution-tables.test.ts`, `theme.test.ts` |
| [UI-S-016](../srs.md) Accessibility contracts | Menus (roving focus, Escape and Tab close, focus back to the opener), modal dialogs (focus first control, Tab kept inside, Escape), the list's one Tab stop and keys (arrows, Space, F2, Delete, menu key), `aria-sort`, `aria-checked`, tree keys in the picker | `drive-view.test.ts`, `drive-shell.test.ts` |
| [UI-S-013](../srs.md) UI tests against production artifacts with strict DOM assertions | Playwright against `vite build` plus `vite preview`, 1440 x 900, light and dark: elements defined, labels from the table, computed styles, boxes, requests, no console errors | `drive-view.spec.ts` |

PRD: [F-062](../../../../../resources/prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) Drive-style views, sorting, selection and layout preferences. SRS: [SRS-F-062](../../../../../resources/srs.md#srs-f-062-drive-style-views-sorting-selection-and-layout-preferences). Feature: [`drive_style_views_and_preferences_F-062.feature`](../../tests/features/drive_style_views_and_preferences_F-062.feature) and [`navigation_preferences_UI-S-017.feature`](../../tests/features/navigation_preferences_UI-S-017.feature).

## What changed

- **Preferences** (`src/preferences/`): `model.ts` (the contract type, defaults, bounds 100, 16 and 1,000, `parsePreferences`), `source.ts` (`ClientPreferencesSource`, `MemoryPreferencesSource`), `service.ts` (`PreferencesService`: reads once, saves each change as one complete PUT with writes folded and capped, the one-time device copy, "api" and "device" modes). `NavigationStore` gained `useRemote` and `replace`.
- **Drive model** (`src/drive/`): `model.ts` (row, query, sort rules, relative dates, badge classes, scope text, ZIP caps, Recent groups), `selection.ts`, `source.ts` and `memory-source.ts` (the view operation as the API answers it), `actions.ts` (rename, remove and Undo, honest refusal of move or copy), `library.ts` (the one open library, a stop-gap).
- **Elements** (`src/components/`): `fs-drive-view.ts` with `drive-list.ts` (windowed list and grid), `drive-row-view.ts`, `drive-menu.ts`, `drive-entries.ts`, `drive-dialogs.ts`; `fs-destination-picker.ts` (shared with UI-F2.17); `fs-new-menu.ts`; `settings-browsing.ts`. The navigation view embeds the file view for a library, collection or virtual collection; the shell's filter dispatches `fs-filter`; the storage block hides the history caption when the host gives none.
- **Routes** (`src/app/`): `drive-route.ts` (`#/files`, `#/recent`, footer, filter, New menu destination, file opening), `storage-route.ts`, `main.ts` wiring.
- **Tests**: two existing Playwright specs now serve the view operation for the nodes they open, and `trackErrors` excuses the browser's network line for the optional start-up reads (preferences, recent libraries) of a host that does not serve them yet.

## Decisions to confirm

- **Merge, not overwrite** (UI-S-017 says "copy that value there once"): when the API already holds navigation choices and the device holds a value too, the two are merged (the API's pins first, then the device's pins it lacks; the API's section order unless it has none; the union of collapsed nodes, each list inside its bound), written once, and the device key is removed. Neither side loses a choice. Changed after review round 1.
- **Library lookup at the cap**: `GET /api/v1/libraries/recent` lists at most 20 libraries, so a full list proves nothing about "only one open"; the file actions and the storage block then treat the library as unknown. The stop-gap goes when the view page names its library.
- **Storage read after a burst of writes**: the storage block reads once, 400 ms after the last committed write, because each read scans the library's history on the host.
- **Whole-choices saves, last writer wins**: each save is one complete `PUT` (the contract has no merge), so when two tabs change the navigation choices the last save wins; a tab reads the API's choices again when it starts. Documented in the README and `NavigationStore.useRemote`.
- **Folder contents in a ZIP are not byte-counted**: the view contract gives a folder row neither a size nor (yet) a file count, so the 10 GiB cap is checked against the selected files' own bytes and left to the ZIP route to enforce for folder contents (the dialog says the download is not available until that route exists).
- **Download in the item menu stays disabled with the label-table reason**: the mockup draws the item, and a single-file download needs the head version, which the view rows do not name yet (item 1 below).
- **File actions without a library in the view rows**: the stop-gap `singleOpenLibrary` names the library when exactly one is open; otherwise the actions are disabled with the label-table reason.

## What is not built, and what the platform engineer must add

1. `ViewPage` or `ViewFileRow` needs `library_id` (and a head `version` for the Version chip). Without them file actions rely on the single open library and the Version column is empty.
2. Move and copy into a collection need `collection_id` and `folder` on the copy and move commands (API-F2.14). The dialog works up to the choice and then refuses with `fileMoveCopyUnavailable`, never a copy no collection would show.
3. The ZIP download has no route (API-F2.2 item 8); the dialog applies the caps and says the download is not available.
4. The views home, personal, favorites, shares and tags and Recent answer 400 (not available yet); the UI says so. Favorite, manage tags, share, Show in Finder and drag-to-folder are left out of the menus until their routes or tasks exist.
5. File counts and byte sizes inside a folder row (for "{inside} files inside") and the library's used-versus-history split for the storage caption.

## Tests

Failing first (before `fs-drive-view` existed):

```
 Test Files  1 failed (1)
      Tests  36 failed (36)
```

Prettier, ESLint and `tsc`:

```
Checking formatting...
All matched files use Prettier code style!
exit 0
eslint exit 0
tsc exit 0
```

`npm test` (Vitest):

```
 Test Files  77 passed (77)
      Tests  1747 passed | 1 skipped (1748)
   Duration  38.87s (transform 5.27s, setup 3.15s, import 12.98s, tests 37.80s, environment 46.92s)
vitest exit 0
```

Playwright against `vite build` plus `vite preview` (`FILESQUID_E2E=1`). The pinned Chromium revision is not installed in this container, so the run used a local-only config that points at the installed Chromium; the spec files and the repository's configs are unchanged. The server-backed suite (`playwright.config.ts`) needs `filesquid-server` and was not run in this lane.

```
✓    1 [chromium] › tests/e2e/preview/drive-view.spec.ts:82:5 › light theme › the list (mockup 05, UI-S-026): regions, columns and constants (969ms)
✓    5 [chromium] › tests/e2e/preview/drive-view.spec.ts:189:5 › light theme › sorting and folder placement are sent to the API and saved as preferences (523ms)
✓    7 [chromium] › tests/e2e/preview/drive-view.spec.ts:214:5 › light theme › the grid (mockup 06): tiles, Sort button and menu (643ms)
✓    9 [chromium] › tests/e2e/preview/drive-view.spec.ts:277:5 › light theme › selection (mockup 11): bulk bar, scope and the header checkbox (597ms)
✓   11 [chromium] › tests/e2e/preview/drive-view.spec.ts:316:5 › light theme › menus (mockup 10): the New menu and the item menu (590ms)
✓   14 [chromium] › tests/e2e/preview/drive-view.spec.ts:354:5 › light theme › rename (mockups 14, 44): the dialog, its refusals and the write (1.4s)
✓   18 [chromium] › tests/e2e/preview/drive-view.spec.ts:422:5 › light theme › remove (mockup 14): the toast with the version and Undo (685ms)
✓   21 [chromium] › tests/e2e/preview/drive-view.spec.ts:455:5 › light theme › move or copy (mockup 13): the dialog, the destination tree and the honest refusal (748ms)
✓   24 [chromium] › tests/e2e/preview/drive-view.spec.ts:519:5 › light theme › Download as ZIP (mockup 12): the dialog, no save path, and the caps (629ms)
✓   25 [chromium] › tests/e2e/preview/drive-view.spec.ts:82:5 › dark theme › the list (mockup 05, UI-S-026): regions, columns and constants (806ms)
✓   28 [chromium] › tests/e2e/preview/drive-view.spec.ts:189:5 › dark theme › sorting and folder placement are sent to the API and saved as preferences (530ms)
✓   30 [chromium] › tests/e2e/preview/drive-view.spec.ts:214:5 › dark theme › the grid (mockup 06): tiles, Sort button and menu (682ms)
✓   33 [chromium] › tests/e2e/preview/drive-view.spec.ts:277:5 › dark theme › selection (mockup 11): bulk bar, scope and the header checkbox (686ms)
✓   35 [chromium] › tests/e2e/preview/drive-view.spec.ts:316:5 › dark theme › menus (mockup 10): the New menu and the item menu (612ms)
✓   37 [chromium] › tests/e2e/preview/drive-view.spec.ts:354:5 › dark theme › rename (mockups 14, 44): the dialog, its refusals and the write (1.6s)
✓   39 [chromium] › tests/e2e/preview/drive-view.spec.ts:422:5 › dark theme › remove (mockup 14): the toast with the version and Undo (646ms)
✓   41 [chromium] › tests/e2e/preview/drive-view.spec.ts:455:5 › dark theme › move or copy (mockup 13): the dialog, the destination tree and the honest refusal (765ms)
✓   43 [chromium] › tests/e2e/preview/drive-view.spec.ts:519:5 › dark theme › Download as ZIP (mockup 12): the dialog, no save path, and the caps (603ms)
✓   45 [chromium] › tests/e2e/preview/drive-view.spec.ts:553:1 › the loading state (mockup 33): placeholder rows and the footer line (2.1s)
✓   49 [chromium] › tests/e2e/preview/drive-view.spec.ts:571:1 › the storage block (UI-S-025) shows the bytes the open library keeps and the free space (337ms)
✓   50 [chromium] › tests/e2e/preview/drive-view.spec.ts:584:1 › Recent (mockup 15) says the host does not serve it yet; the view stays under the shell (316ms)
✓   52 [chromium] › tests/e2e/preview/drive-view.spec.ts:598:1 › the sidebar filter narrows only the current view and is sent to the API (437ms)
✓   53 [chromium] › tests/e2e/preview/drive-view.spec.ts:610:1 › a failed view read shows the error state with Try again, which reads again (mockup 56) (412ms)
✓   56 [chromium] › tests/e2e/preview/drive-view.spec.ts:636:1 › the Browsing settings (mockup 32) save through the API and come back on a reload (667ms)
✓   59 [chromium] › tests/e2e/preview/drive-view.spec.ts:677:1 › the navigation choices on the device move to the API once and the key is removed (UI-S-017) (368ms)
✓   62 [chromium] › tests/e2e/preview/drive-view.spec.ts:703:1 › a host with no preference store keeps working and says so in Settings (309ms)
  144 passed (1.0m)
```

Re-run after review round 2 (Vitest 77 files, 1823 passed and 1 skipped; Playwright again 144 passed against `vite build` plus `vite preview`; `browsingRows` was split so no function exceeds 60 lines).

Hook, ordinary `bash scripts/pre-commit.sh` (the change scope against `origin/main` is `npm:apps/filesquid/ui`, so no Cargo step runs):

```
ℹ️  Test scope: npm:apps/filesquid/ui
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui (76s)
```

## Sync Execution Report

- PRD [F-062](../../../../../resources/prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences), SRS [SRS-F-062](../../../../../resources/srs.md#srs-f-062-drive-style-views-sorting-selection-and-layout-preferences) and the module SRS rows UI-S-006, UI-S-017, UI-S-024 to UI-S-026 and UI-S-033 already state what was built; no requirement text changed. The rows that name views the API does not serve yet (home, personal, favorites, shares, tags) keep their wording, as UI-S-006 itself says they join the sidebar when their tasks ship.
- ADR 041 (destination of move or copy) is followed unchanged; no new ADR is needed because no library, route or architecture was added.
- BDD: [`drive_style_views_and_preferences_F-062.feature`](../../tests/features/drive_style_views_and_preferences_F-062.feature) and [`navigation_preferences_UI-S-017.feature`](../../tests/features/navigation_preferences_UI-S-017.feature) carry the scenarios for the built behaviour.
- Module plan box for UI-F2.8 and its checklist row are ticked; the global Task FS-2.5 stays open (5 of 12 module rows), so its box is unchanged.

## Mocked Environment Variables

None. The Playwright fakes answer the contract's own shapes.
