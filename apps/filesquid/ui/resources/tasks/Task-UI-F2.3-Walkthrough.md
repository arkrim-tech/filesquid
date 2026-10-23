# Task UI-F2.3 Walkthrough: Details and Versions panels

Branch `feat/ui-f2.3-details-versions-panels`. Plan row: [Task UI-F2.3](../plan.md). Mockups (active suite v3): [35-panel-file-info](../../../../../resources/ui-mockups/v3/35-panel-file-info.svg), [26-panel-versions](../../../../../resources/ui-mockups/v3/26-panel-versions.svg), [27-version-restore](../../../../../resources/ui-mockups/v3/27-version-restore.svg), [25-panel-states](../../../../../resources/ui-mockups/v3/25-panel-states.svg). Built on the tokens, shell and panel region of [UI-F2.18](./Task-UI-F2.18-Walkthrough.md).

## SRS mapping

| SRS ID and title | What was built | Proof |
|---|---|---|
| [UI-S-003](../srs.md) Panels, including the file panel's tabs, MUST plug into named sidebar slots and MUST NOT depend on each other; each file fact is shown by the panel of its owning module | `<fs-file-panel>` with `registerTab(id, label, create)` and `setVersions(create)`: each panel in its own `section[data-slot]`, created when first shown, handed only the `fileRef`; the File info tab shows storage facts only, owned by the OCFL module | `tests/ui/file-panel.test.ts`, `tests/ui/details-panel.test.ts`, `tests/unit/details-model.test.ts` |
| [UI-S-009](../srs.md) The Versions view, opened from the file panel's header, MUST list versions newest first with label, relative time and size, mark the current version, and offer restore per version; no delete, expiry or thinning control | `<fs-versions-panel>` behind the history button: rows newest first, Current chip, row menu with Restore as new version, the restore dialog; no delete, expiry or thinning control (download and edit label are UI-F2.11) | `tests/ui/versions-panel.test.ts`, `tests/unit/versions-model.test.ts`, `tests/e2e/preview/file-panel.spec.ts` |
| [UI-S-024](../srs.md) Visual constants of the active mockup suite | Every colour, size, radius and spacing through tokens, light and dark; computed styles and boxes within 2 px at 1440 × 900 | `tests/e2e/preview/file-panel.spec.ts`, `tests/unit/component-styles.test.ts` |
| [UI-S-031](../srs.md) Preview and file panel composition | File panel header (badge, 14 px 600 name with ellipsis, history, close), tabs (13 px, active 600 with a 2 px primary underline), filter, count eyebrow, 44 px rows, footer; Versions (Back, count, eyebrow, 58 px rows at a 62 px pitch, Current, menu, note); restore dialog; panel states (loading, no match, error) | `tests/e2e/preview/file-panel.spec.ts` |
| [UI-S-025](../srs.md) Shell composition | The 320 px panel region now sits under the full-width header, beside the main column and the footer (header 1200 px, footer 880 px while the panel shows) | `tests/ui/app-shell.test.ts`, `tests/e2e/preview/file-panel.spec.ts`, `tests/e2e/preview/mockup-fidelity.spec.ts` |
| [UI-S-005](../srs.md) A distribution's label table MUST define every key of the default table | 51 new `filePanel*` and `versions*` keys in both label tables | `tests/unit/theme.test.ts` (both packages) |
| [UI-S-012](../srs.md) UIs reach FileSquid only through `FileSquidClient` | `ClientHistorySource` (history, versions, history size) and the existing `FileSource.restore`, extended with the route's optional `label` | `tests/unit/history-source.test.ts` |
| [UI-S-013](../srs.md) UI tests against production artifacts with strict DOM assertions | Playwright against `vite build` plus `vite preview`, light and dark, no console errors | Playwright output below |
| [UI-S-016](../srs.md) Accessibility contracts | WAI-ARIA tabs (arrow keys, Home, End, one Tab stop), `aria-pressed` history button, menu button and menu keys, modal dialog with Tab kept inside and Escape returning focus, the 2 px `--fs-focus-ring` ring, logical properties only | `tests/ui/*.test.ts` above, `tests/e2e/preview/file-panel.spec.ts` |
| [UI-S-019](../srs.md) The stored name never reaches the DOM | The stored name and logical path are never shown; deep scans of every shadow root | `tests/ui/details-panel.test.ts`, `tests/e2e/preview/file-panel.spec.ts` |

PRD: [F-055](../../../../../resources/prd.md#f-055-default-ui-and-panel-composition) Default UI and panel composition, [F-065](../../../../../resources/prd.md#f-065-file-version-history-and-retention) File version history and retention. Gherkin: [composable_filesquid_ui_F-055.feature](../../tests/features/composable_filesquid_ui_F-055.feature) and [file_version_history_F-065.feature](../../tests/features/file_version_history_F-065.feature) gained the scenarios of this task.

## What changed

- **`<fs-file-panel>`** (`src/components/fs-file-panel.ts`): the frame of UI-S-031. Tabs are registered panels in named slots; only the panel shown is created and handed the file, so a hidden tab reads nothing. The history button shows the Versions view in place of the tabs; its Back returns focus to the button. Close dispatches `fs-file-panel-close`.
- **`<fs-details-panel>`** (`src/components/fs-details-panel.ts`, `src/files/details.ts`): the Details panel as the File info tab. Rows: original file name, extension, MIME type, size with bytes, the SHA-256 digest (from the inventory's fixity block when the content digest is SHA-512) and the other digests, the OCFL object and "v{n} of {total} · {time} UTC", each with copy.
- **`PropertyTable`** (`src/components/property-table.ts`): the shared filter, count eyebrow, rows, loading, no-match and error states and footer that UI-F2.15 reuses for Description and Provenance.
- **`<fs-versions-panel>`** (`src/components/fs-versions-panel.ts`, `src/components/version-restore-dialog.ts`, `src/files/versions.ts`): the Versions view and the restore dialog of mockup 27. A restore is one `FileSource.restore` call with the optional label (checked first: at most 512 bytes, no control characters); on success the view announces it, dispatches `fs-files-change` and lists again; a refusal stays in the dialog.
- **History reads** (`src/files/history-source.ts`): `HistorySource` and `ClientHistorySource` over `GET /api/v1/files/history`, `/files/versions` and `/files/history-size`, in a file of their own so the command-envelope lane (API-F2.13, PR #135) editing `source.ts` does not collide. `MemoryFileSource` answers the same reads for tests and demos, and the Playwright file-route fake serves them.
- **Shell** (`src/components/fs-app-shell.ts`): the panel region moved under the header, as mockups 35 and 26 draw it.
- **Files route** (`src/app/routes.ts`, `src/app/file-panel-route.ts`, `src/app/main.ts`): with a history source, the route mounts the file panel in the shell's `panel` slot while a file is open, removes it when the viewer closes, the panel is closed or the route leaves, and reloads the browser after a restore (`FsFileBrowser.reload()` is now public).
- **Tests**: shared fidelity helpers moved from `mockup-fidelity.spec.ts` to `tests/e2e/support/fidelity.ts` so both fidelity specs use one copy.

## Decisions and open points for the owner

- **SRS wording that disagrees with UI-S-024 or the mockup** (followed the stricter or general rule; no requirement changed): UI-S-031 gives property labels as 12 px, but UI-S-024 sets meta text at 13 px or more and mockup 35 draws 13 px, so labels are 13 px; mockup 35 draws the active tab at 14 px, UI-S-031 says 13 px, so tabs are 13 px; mockup 25 draws state buttons 28 px tall with 12 px text, UI-S-024 sets every button at 36 px and 14 px 600, so they follow UI-S-024; the panel filter follows mockup 35's 32 px (as UI-F2.18 followed mockup 05's 34 px sidebar filter) rather than UI-S-024's 36 px inputs.
- **Contrast**: muted text has no listed exemption on `--fs-surface-raised` or `--fs-surface-overlay`, so a hovered row turns its muted label (and "No label") to the secondary colour, and the restore dialog's new-version line sits on the active tint as UI-S-031 says (the one surface where the active text's exemption applies) rather than as the mockup's bare text on the grey box.
- **Not in the contract** (for the platform engineer): mockup 35's RESOURCES IN THIS FILE list needs a field listing a logical file's resources (RAW+JPEG pair, motion pair, bursts) with their digests, and its Fixity "Verified {time}" row needs the time of the last fixity check; `FileDetails` has neither, so both are left out. The error state's "op …" needs an operation ID in read refusals, which `ApiError` does not carry. The content location of the UI-F2.3 description is never answered (API-S-005), and the logical path is not shown because its last segment is the stored name (UI-S-019).
- **Left for named tasks**: Download this version and Edit label (UI-F2.11); the Uploaded and Source rows and the format name, from `GET /files/facts` (UI-F2.15); View in Logs (UI-F2.16); the panel opening from a row selection while the list shows, as mockup 35 draws (UI-F2.8). The UI-F2.3 description's version `user` and `message` columns are represented as UI-S-031 composes the row (the label from the message, relative time and size).
- **Restore label and API-F2.13**: the restore keeps using `FileSource.restore`; PR #135 changes its options to `WriteOptions` with the object's head as the revision, so merging it gives `{ revision, label }` (the Versions view already reads the head).

No API route, field or event was assumed or stubbed.

## How it was tested

1. Tests first: the unit and UI tests of the models, the history source, the three elements, the route and the shell ran red before the implementation (below), then green. The history reads of `MemoryFileSource` and the file-route fake were added when the first full Playwright run showed the existing browser specs logging 404s for `files/versions`; their unit tests (`tests/unit/memory-history.test.ts`) were written with that change.
2. `npx prettier --write .`, `npx eslint . --fix`, `npx tsc --noEmit` in `apps/filesquid/ui` and `apps/imagesquid`: clean.
3. `npm test` in both packages.
4. Playwright, both suites of both packages. The installed Playwright expects a newer Chromium than this machine has, so the suites ran through temporary configs that set `executablePath` to `/opt/pw-browsers/chromium`; the server-backed suites used the already-built `filesquid-server` binary instead of `cargo run`, so no Rust build ran (npm-only scope). The configs were deleted before committing. The clock of the file panel spec is fixed with `page.clock`. No mocked environment values were used.

## Raw test output

### Red run (before the implementation)

```text
 FAIL  tests/unit/details-model.test.ts [ tests/unit/details-model.test.ts ]
 FAIL  tests/unit/history-source.test.ts [ tests/unit/history-source.test.ts ]
 FAIL  tests/unit/versions-model.test.ts [ tests/unit/versions-model.test.ts ]
 FAIL  tests/ui/app-shell.test.ts > <fs-app-shell> > puts the file panel region under the header, beside the main column and the footer (35, 26)
 FAIL  tests/ui/details-panel.test.ts > <fs-details-panel> > is a defined element with the shared stylesheet first and its tab label
 FAIL  tests/ui/details-panel.test.ts > <fs-details-panel> > shows the storage facts as rows of a label over a value, with the count and footer
 FAIL  tests/ui/details-panel.test.ts > <fs-details-panel> > never puts the stored name or the logical path in the DOM
 FAIL  tests/ui/details-panel.test.ts > <fs-details-panel> > copies the whole value and announces it
 FAIL  tests/ui/details-panel.test.ts > <fs-details-panel> > logs a refused copy and announces nothing
 FAIL  tests/ui/details-panel.test.ts > <fs-details-panel> > filters rows by label or value and shows no match with Clear filter
 FAIL  tests/ui/details-panel.test.ts > <fs-details-panel> > shows placeholder rows and the busy line while loading, with the filter disabled
 FAIL  tests/ui/details-panel.test.ts > <fs-details-panel> > shows the error with its reason, Nothing was changed, the code and Try again
 FAIL  tests/ui/details-panel.test.ts > <fs-details-panel> > words a denial with its reason and an offline failure as offline
 FAIL  tests/ui/details-panel.test.ts > <fs-details-panel> > ignores the answer for a file that is no longer shown
 FAIL  tests/ui/file-panel.test.ts > <fs-file-panel> > is a defined element with the shared stylesheet first
 FAIL  tests/ui/file-panel.test.ts > <fs-file-panel> > shows the badge, the display name, the history button and close in the header
 FAIL  tests/ui/file-panel.test.ts > <fs-file-panel> > puts each registered panel in its own named slot and tab, the first selected
 FAIL  tests/ui/file-panel.test.ts > <fs-file-panel> > hands the file to every panel and no panel a reference to another
 FAIL  tests/ui/file-panel.test.ts > <fs-file-panel> > moves between tabs with the arrow, Home and End keys and selects on click
 FAIL  tests/ui/file-panel.test.ts > <fs-file-panel> > shows the Versions view in place of the tabs from the history button, and Back returns
 FAIL  tests/ui/file-panel.test.ts > <fs-file-panel> > dispatches fs-file-panel-close from close
 FAIL  tests/ui/file-panel.test.ts > <fs-file-panel> > replaces a tab registered again under the same id
 FAIL  tests/ui/file-panel.test.ts > <fs-file-panel> > follows a new file and clears the header with none
 FAIL  tests/ui/files-route.test.ts > bindFilesRoute > mounts the file panel in the panel slot for the opened file and removes it on close (UI-F2.3)
 FAIL  tests/ui/files-route.test.ts > bindFilesRoute > removes the file panel when the viewer closes or the route leaves the files
 FAIL  tests/ui/files-route.test.ts > bindFilesRoute > reloads the listing after the panel commits a restore
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > is a defined element with the shared stylesheet first
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > shows Back, the count, the history eyebrow and the note
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > lists every version newest first with chip, label or No label, and time and size
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > marks the current version with a chip and offers no menu on it; every other row has one
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > opens the row menu with Restore as new version and no delete, expiry or thinning
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > opens the restore dialog naming the file, the source and the new version
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > restores with one call carrying the label, announces it and dispatches the change
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > restores without a label when the field is left empty
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > refuses a label with a control character or over 512 bytes before calling the API
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > keeps the dialog open with the refusal's words when the restore is refused
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > disables the buttons while the restore runs
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > closes the dialog with Escape or Cancel without a call and returns focus to the menu button
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > keeps Tab inside the dialog
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > dispatches fs-versions-close from Back
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > words the eyebrow without a size when the history size cannot be read
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > shows loading, then the error with Try again when the history cannot be read
 FAIL  tests/ui/versions-panel.test.ts > <fs-versions-panel> > marks a removal and offers no restore from it
 FAIL  tests/unit/component-styles.test.ts > component stylesheets > cover every component source
 FAIL  tests/unit/component-styles.test.ts > UI-S-016 accessibility contracts in component stylesheets > fs-details-panel.ts includes the shared accessibility stylesheet
 FAIL  tests/unit/component-styles.test.ts > UI-S-016 accessibility contracts in component stylesheets > fs-file-panel.ts includes the shared accessibility stylesheet
 FAIL  tests/unit/component-styles.test.ts > UI-S-016 accessibility contracts in component stylesheets > fs-versions-panel.ts includes the shared accessibility stylesheet
 Test Files  9 failed (9)
      Tests  44 failed | 222 passed (266)
```

### npm test (apps/filesquid/ui)

```text
 ✓ tests/ui/host-indicator.test.ts (40 tests) 1078ms
 ✓ tests/ui/nav-tree.test.ts (30 tests) 1968ms
 ✓ tests/unit/theme.test.ts (36 tests) 149ms
 ✓ tests/ui/file-browser-writes.test.ts (47 tests) 3307ms
 ✓ tests/ui/app-shell.test.ts (24 tests) 1491ms
 ✓ tests/ui/file-browser.test.ts (37 tests) 3792ms
 ✓ tests/ui/ocfl-findings.test.ts (22 tests) 512ms
 ✓ tests/ui/library-start.test.ts (22 tests) 365ms
 ✓ tests/unit/files-source.test.ts (31 tests) 161ms
 ✓ tests/ui/versions-panel.test.ts (17 tests) 961ms
 ✓ tests/ui/monitor.test.ts (22 tests) 7694ms
 ✓ tests/ui/library-repair.test.ts (22 tests) 624ms
 ✓ tests/unit/files-memory-source.test.ts (34 tests) 130ms
 ✓ tests/unit/component-styles.test.ts (226 tests) 109ms
 ✓ tests/ui/library-detail.test.ts (20 tests) 727ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 671ms
 ✓ tests/unit/files-ops.test.ts (54 tests) 194ms
 ✓ tests/ui/file-preview.test.ts (16 tests) 325ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 66ms
 ✓ tests/unit/client-io.test.ts (22 tests) 107ms
 ✓ tests/ui/navigation-view.test.ts (20 tests) 416ms
 ✓ tests/ui/settings.test.ts (10 tests) 245ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 66ms
 ✓ tests/unit/media-handle.test.ts (14 tests) 207ms
 ✓ tests/ui/nav-tree-live.test.ts (11 tests) 562ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 35ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 196ms
 ✓ tests/unit/library-client.test.ts (18 tests) 59ms
 ✓ tests/ui/details-panel.test.ts (10 tests) 534ms
 ✓ tests/ui/file-panel.test.ts (9 tests) 554ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 34ms
 ✓ tests/unit/navigation-client.test.ts (13 tests) 27ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 36ms
 ✓ tests/unit/files-model.test.ts (58 tests) 144ms
 ✓ tests/ui/files-route.test.ts (10 tests) 1025ms
 ✓ tests/unit/commands.test.ts (45 tests) 38ms
 ✓ tests/ui/states.test.ts (26 tests) 155ms
 ✓ tests/unit/host-descriptor.test.ts (43 tests) 61ms
 ✓ tests/unit/admin-client.test.ts (10 tests) 31ms
 ✓ tests/unit/files-window.test.ts (19 tests) 20ms
 ✓ tests/unit/versions-model.test.ts (13 tests) 104ms
 ✓ tests/unit/focus.test.ts (16 tests) 122ms
 ✓ tests/unit/library-errors.test.ts (15 tests) 37ms
 ✓ tests/unit/navigation-contract.test.ts (30 tests) 42ms
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 1373ms
 ✓ tests/ui/library-route.test.ts (6 tests) 149ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 16ms
 ✓ tests/unit/details-model.test.ts (7 tests) 59ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 73ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 81ms
 ✓ tests/unit/client.test.ts (8 tests) 18ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 13ms
 ✓ tests/unit/library-model.test.ts (7 tests) 23ms
 ✓ tests/unit/history-source.test.ts (6 tests) 29ms
 ✓ tests/unit/preferences.test.ts (10 tests) 34ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 159ms
 ✓ tests/unit/host-client.test.ts (4 tests) 30ms
 ✓ tests/unit/view-state.test.ts (5 tests) 14ms
 ✓ tests/unit/contrast.test.ts (13 tests) 55ms
 ✓ tests/unit/memory-history.test.ts (4 tests) 25ms
 ✓ tests/unit/distribution.test.ts (3 tests) 20ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 28ms
 ✓ tests/unit/contract.test.ts (2 tests) 421ms
 Test Files  63 passed (63)
      Tests  1396 passed | 1 skipped (1397)
   Duration  38.81s (transform 4.76s, setup 3.76s, import 11.38s, tests 31.80s, environment 50.16s)
```

### npm test (apps/imagesquid)

```text
 ✓ tests/unit/test-reporting.test.ts (12 tests) 125ms
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 1465ms
 ✓ tests/unit/theme.test.ts (12 tests) 42ms
 ✓ tests/ui/distribution.test.ts (5 tests) 253ms
 Test Files  4 passed (4)
      Tests  38 passed | 1 skipped (39)
   Duration  3.40s (transform 1.98s, setup 430ms, import 2.19s, tests 1.88s, environment 2.69s)
```

### Playwright, production build (`vite build` plus `vite preview`, apps/filesquid/ui)

```text
Running 90 tests using 2 workers
  ✓   1 [chromium] › tests/e2e/preview/file-browser.spec.ts:96:1 › the browser is a defined element and lists display names in the list layout from the label table (771ms)
  ✓   3 [chromium] › tests/e2e/preview/file-browser.spec.ts:140:1 › a file row shows size, type hint, modified date and version (413ms)
  ✓   2 [chromium] › tests/e2e/preview/file-panel.spec.ts:102:5 › light theme › File info (35): header, tab, filter, count, rows and footer (1.4s)
  ✓   4 [chromium] › tests/e2e/preview/file-browser.spec.ts:152:1 › computed token values are exact in the browser in light and dark (706ms)
  ✓   5 [chromium] › tests/e2e/preview/file-panel.spec.ts:212:5 › light theme › Versions (26): Back, count, eyebrow, rows, Current, menu and note (1.2s)
  ✓   6 [chromium] › tests/e2e/preview/file-browser.spec.ts:175:1 › opening a folder updates the breadcrumb and the address, and Back returns (775ms)
  ✓   8 [chromium] › tests/e2e/preview/file-browser.spec.ts:196:1 › an image previews through a content handle that is used only as the media src (756ms)
  ✓   7 [chromium] › tests/e2e/preview/file-panel.spec.ts:337:5 › light theme › restore (27): the dialog, one call with the label, and the new current version (1.3s)
  ✓   9 [chromium] › tests/e2e/preview/file-browser.spec.ts:231:1 › a handle that fails is replaced once; a second failure shows the error and mints no third (571ms)
  ✓  11 [chromium] › tests/e2e/preview/file-browser.spec.ts:247:1 › a video resumes at the same position and paused state after one new handle (896ms)
  ✓  10 [chromium] › tests/e2e/preview/file-panel.spec.ts:459:5 › light theme › panel states (25): loading, no match and error keep the header, tabs and filter (1.1s)
  ✓  12 [chromium] › tests/e2e/preview/file-browser.spec.ts:283:1 › audio plays natively and is named by the display name (661ms)
  ✓  13 [chromium] › tests/e2e/preview/file-panel.spec.ts:532:5 › light theme › keyboard: the tab, the history button and the dialog keep focus where it belongs (837ms)
  ✓  14 [chromium] › tests/e2e/preview/file-browser.spec.ts:296:1 › text previews through one ranged read of the first 64 KiB and says it is cut (674ms)
  ✓  16 [chromium] › tests/e2e/preview/file-browser.spec.ts:312:1 › a format it cannot show says so and shows no viewer (635ms)
  ✓  15 [chromium] › tests/e2e/preview/file-panel.spec.ts:102:5 › dark theme › File info (35): header, tab, filter, count, rows and footer (1.1s)
  ✓  17 [chromium] › tests/e2e/preview/file-browser.spec.ts:342:1 › the viewer steps through files, closes with Escape and returns focus to the row (627ms)
  ✓  19 [chromium] › tests/e2e/preview/file-browser.spec.ts:355:1 › an upload is one PUT of the client's facts that creates the file's own object (509ms)
  ✓  18 [chromium] › tests/e2e/preview/file-panel.spec.ts:212:5 › dark theme › Versions (26): Back, count, eyebrow, rows, Current, menu and note (1.2s)
  ✓  20 [chromium] › tests/e2e/preview/file-browser.spec.ts:404:1 › a rename posts to the renames route, never to moves, and shows the new display name (757ms)
  ✓  22 [chromium] › tests/e2e/preview/file-browser.spec.ts:433:1 › a refused rename shows the label for its reason and keeps the display name (630ms)
  ✓  21 [chromium] › tests/e2e/preview/file-panel.spec.ts:337:5 › dark theme › restore (27): the dialog, one call with the label, and the new current version (1.4s)
  ✓  23 [chromium] › tests/e2e/preview/file-browser.spec.ts:452:1 › writes are single calls: new folder, move, remove and Undo (1.0s)
  ✓  24 [chromium] › tests/e2e/preview/file-panel.spec.ts:459:5 › dark theme › panel states (25): loading, no match and error keep the header, tabs and filter (973ms)
  ✓  25 [chromium] › tests/e2e/preview/file-browser.spec.ts:499:1 › a refused write shows the label for its reason and changes nothing (551ms)
  ✓  26 [chromium] › tests/e2e/preview/file-panel.spec.ts:532:5 › dark theme › keyboard: the tab, the history button and the dialog keep focus where it belongs (931ms)
  ✓  27 [chromium] › tests/e2e/preview/file-browser.spec.ts:514:1 › a long folder is virtualised, pages in on demand and stays keyboard reachable (835ms)
  ✓  28 [chromium] › tests/e2e/preview/host-indicator.spec.ts:96:1 › the indicator is a defined custom element in the shell header, named from the label table (414ms)
  ✓  30 [chromium] › tests/e2e/preview/host-indicator.spec.ts:110:1 › the desktop host says only you can read your files, and shows the not-enrolled state (488ms)
  ✓  29 [chromium] › tests/e2e/preview/file-browser.spec.ts:554:1 › the layout does not scroll sideways at 320 px, at 200 % text size, or both, in list and grid (684ms)
  ✓  31 [chromium] › tests/e2e/preview/host-indicator.spec.ts:124:1 › a server names its operator, who can read the files, and states the at-rest duty (480ms)
  ✓  32 [chromium] › tests/e2e/preview/file-browser.spec.ts:571:1 › motion stops under reduced motion and focus is the 2px --fs-focus-ring ring (601ms)
  ✓  33 [chromium] › tests/e2e/preview/host-indicator.spec.ts:137:1 › a server without an operator name uses the unnamed-operator label (468ms)
  ✓  34 [chromium] › tests/e2e/preview/file-browser.spec.ts:598:1 › a failed listing shows the error state with Try again, and retrying loads it (589ms)
  ✓  35 [chromium] › tests/e2e/preview/host-indicator.spec.ts:149:1 › the browser host says files are decrypted only in this browser (479ms)
  ✓  36 [chromium] › tests/e2e/preview/library.spec.ts:47:1 › the start page offers create, open and recover in the active vocabulary (472ms)
  ✓  37 [chromium] › tests/e2e/preview/host-indicator.spec.ts:162:1 › enrolled backup and sync are shown apart from the host, as ciphertext only (531ms)
  ✓  39 [chromium] › tests/e2e/preview/host-indicator.spec.ts:187:1 › a host.change event updates the indicator without a reload or another read (419ms)
  ✓  38 [chromium] › tests/e2e/preview/library.spec.ts:80:1 › (1) create asks for a folder, passes the location ID on and shows the library, with no path (654ms)
  ✓  40 [chromium] › tests/e2e/preview/host-indicator.spec.ts:209:1 › events.resync makes the indicator read the descriptor again (440ms)
  ✓  41 [chromium] › tests/e2e/preview/library.spec.ts:102:1 › (1) open passes the location ID to the open route (670ms)
  ✓  43 [chromium] › tests/e2e/preview/library.spec.ts:116:1 › (1) recover opens the copy, rebuilds from files and reports without repairing (521ms)
  ✓  42 [chromium] › tests/e2e/preview/host-indicator.spec.ts:222:1 › loading shows no host until the descriptor arrives (1.2s)
  ✓  44 [chromium] › tests/e2e/preview/library.spec.ts:153:1 › (1) copy asks for a destination and passes it as destination_location_id (469ms)
  ✓  45 [chromium] › tests/e2e/preview/host-indicator.spec.ts:234:1 › a failed read shows the error state and never a guessed host, then Retry recovers (517ms)
  ✓  46 [chromium] › tests/e2e/preview/library.spec.ts:176:1 › (2) a cancelled folder choice sends no library request and says nothing changed (452ms)
  ✓  47 [chromium] › tests/e2e/preview/host-indicator.spec.ts:256:1 › the header shows a compact badge with the short host label, not the statement (349ms)
  ✓  48 [chromium] › tests/e2e/preview/library.spec.ts:190:1 › (3) 503 unavailable is the unavailable state with its reason and a Retry, and the choices stay (533ms)
  ✓  49 [chromium] › tests/e2e/preview/host-indicator.spec.ts:271:1 › Enter opens the popover with the full statement; Escape closes it and returns focus (444ms)
  ✓  50 [chromium] › tests/e2e/preview/library.spec.ts:217:1 › (4) confirm sends the consent for exactly the named objects and shows each result (607ms)
  ✓  51 [chromium] › tests/e2e/preview/host-indicator.spec.ts:297:1 › Privacy settings opens the Settings page, whose Privacy section has the full statement (561ms)
  ✓  52 [chromium] › tests/e2e/preview/library.spec.ts:254:1 › (4) cancel and Escape send nothing and return focus to the opener (578ms)
  ✓  53 [chromium] › tests/e2e/preview/host-indicator.spec.ts:333:1 › computed token values are exact in light and dark (UI-S-024, mockup 40) (794ms)
  ✓  54 [chromium] › tests/e2e/preview/library.spec.ts:277:1 › (4) a refused repair says nothing was changed and shows no result (491ms)
  ✓  55 [chromium] › tests/e2e/preview/host-indicator.spec.ts:378:1 › the badge is reachable by keyboard and has a visible focus outline (677ms)
  ✓  56 [chromium] › tests/e2e/preview/library.spec.ts:292:1 › (5) the Monitor's W010 note links to the library's repair offer (591ms)
  ✓  58 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:116:5 › light theme › the shell frame (UI-S-025): sidebar, header, footer, controls and strings (852ms)
  ✓  57 [chromium] › tests/e2e/preview/library.spec.ts:345:1 › colours come from tokens in light and dark (878ms)
  ✓  60 [chromium] › tests/e2e/preview/navigation.spec.ts:62:1 › the live tree fills the shell's navigation slot, worded from the label table (527ms)
  ✓  59 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:200:5 › light theme › the navigation tree as built (55, UI-S-017): eyebrows, 30 px rows, 16 px icons (624ms)
  ✓  61 [chromium] › tests/e2e/preview/navigation.spec.ts:83:1 › a level is read page by page through next_cursor until it is null (468ms)
  ✓  62 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:238:5 › light theme › welcome (01, UI-S-027): title, lead, cards, privacy box and footer (643ms)
  ✓  63 [chromium] › tests/e2e/preview/navigation.spec.ts:121:1 › a destination opens with a breadcrumb filled from ancestors (617ms)
  ✓  64 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:294:5 › light theme › folder-choice unavailable (03) and the view states treatment (56) (538ms)
  ✓  65 [chromium] › tests/e2e/preview/navigation.spec.ts:142:1 › a deep link to a destination shows its breadcrumb after a reload (622ms)
  ✓  66 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:329:5 › light theme › library page (41, UI-S-027): facts rows, actions and the W010 note (793ms)
  ✓  67 [chromium] › tests/e2e/preview/navigation.spec.ts:154:1 › navigation.change reads only the named level again, without a reload (452ms)
  ✓  69 [chromium] › tests/e2e/preview/navigation.spec.ts:177:1 › a 503 shows the unavailable state with Retry, never an empty tree (470ms)
  ✓  68 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:379:5 › light theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (661ms)
  ✓  71 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:424:5 › light theme › Monitor (31) and its OCFL validation section (54) (677ms)
  ✓  70 [chromium] › tests/e2e/preview/navigation.spec.ts:194:1 › computed token values of the tree and breadcrumb are exact in light and dark (860ms)
  ✓  73 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:174:1 › (1) an E023 finding shows with its code beside the degraded health reason (461ms)
  ✓  72 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:485:5 › light theme › host popover (40) and Settings (32): 372 px overlay, Appearance and About (656ms)
  ✓  74 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:200:1 › (2) the finding disappears after a clean revalidation's health.change, without a reload (554ms)
  ✓  75 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:116:5 › dark theme › the shell frame (UI-S-025): sidebar, header, footer, controls and strings (712ms)
  ✓  76 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:220:1 › (3) a W010 warning is listed apart from errors and health stays ok (427ms)
  ✓  77 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:200:5 › dark theme › the navigation tree as built (55, UI-S-017): eyebrows, 30 px rows, 16 px icons (600ms)
  ✓  78 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:238:1 › (4) no location is rendered when the API withholds it (460ms)
  ✓  80 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:250:1 › (5) the last and next run times come from the findings route; Validate now starts a run (488ms)
  ✓  79 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:238:5 › dark theme › welcome (01, UI-S-027): title, lead, cards, privacy box and footer (598ms)
  ✓  81 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:289:1 › error and warning codes take their colours from tokens in light and dark (495ms)
  ✓  82 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:294:5 › dark theme › folder-choice unavailable (03) and the view states treatment (56) (580ms)
  ✓  83 [chromium] › tests/e2e/preview/production-build.spec.ts:63:1 › the built page is static, mounts the shell and defines every custom element (489ms)
  ✓  84 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:329:5 › dark theme › library page (41, UI-S-027): facts rows, actions and the W010 note (647ms)
  ✓  85 [chromium] › tests/e2e/preview/production-build.spec.ts:80:1 › sidebar, header and Settings words come from the active label table (UI-S-025, UI-S-033) (511ms)
  ✓  86 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:379:5 › dark theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (609ms)
  ✓  87 [chromium] › tests/e2e/preview/production-build.spec.ts:124:1 › computed token values are exact in light and dark (UI-S-024, CA-1) (453ms)
  ✓  89 [chromium] › tests/e2e/preview/production-build.spec.ts:146:1 › a refused health call shows the unavailable alert in Settings, About, not a console error (488ms)
  ✓  88 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:424:5 › dark theme › Monitor (31) and its OCFL validation section (54) (661ms)
  ✓  90 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:485:5 › dark theme › host popover (40) and Settings (32): 372 px overlay, Appearance and About (399ms)
  90 passed (40.2s)
```

### Playwright, built UI served by `filesquid-server` (apps/filesquid/ui)

```text
Running 42 tests using 2 workers
  ✓   1 [chromium] › tests/e2e/accessibility.spec.ts:67:1 › landmarks, headings and control names come from the label table (490ms)
  ✓   2 [chromium] › tests/e2e/app.spec.ts:35:1 › shell mounts with FileSquid brand, internal vocabulary and the API status in Settings only (640ms)
  ✓   4 [chromium] › tests/e2e/app.spec.ts:63:1 › design tokens switch between light and dark (396ms)
  ✓   3 [chromium] › tests/e2e/accessibility.spec.ts:112:3 › Tab order and a 2px --fs-focus-ring focus ring in the light theme (583ms)
  ✓   5 [chromium] › tests/e2e/app.spec.ts:81:1 › client-side routes fall back to the distribution index (354ms)
  ✓   7 [chromium] › tests/e2e/app.spec.ts:87:1 › without a token Settings, About names the missing credentials from the label table (303ms)
  ✓   6 [chromium] › tests/e2e/accessibility.spec.ts:112:3 › Tab order and a 2px --fs-focus-ring focus ring in the dark theme (799ms)
  ✓   8 [chromium] › tests/e2e/app.spec.ts:99:1 › Monitor asks for the admin token, then shows live health, processes and the journal (508ms)
  ✓   9 [chromium] › tests/e2e/accessibility.spec.ts:150:1 › light-DOM content slotted into the shell gets the same focus ring (490ms)
  ✓  10 [chromium] › tests/e2e/app.spec.ts:135:1 › a wrong token is named as invalid credentials (483ms)
  ✓  11 [chromium] › tests/e2e/accessibility.spec.ts:178:1 › the skip control appears on focus and moves focus to main without routing (463ms)
  ✓  13 [chromium] › tests/e2e/accessibility.spec.ts:196:1 › opening the Monitor from the keyboard moves focus to main (561ms)
  ✓  12 [chromium] › tests/e2e/app.spec.ts:146:1 › the signed-in token stays in memory: not in storage, URL or DOM, gone on reload (830ms)
  ✓  15 [chromium] › tests/e2e/app.spec.ts:177:1 › the desktop launch token authorises the shell, the Monitor and the event channel (378ms)
  ✓  14 [chromium] › tests/e2e/accessibility.spec.ts:214:1 › a keyboard sign-in to the Monitor keeps focus in the Monitor (913ms)
  ✓  16 [chromium] › tests/e2e/app.spec.ts:270:3 › inside the desktop webview (custom URI scheme adapter) › requests use the custom scheme and events use Tauri, not same-origin HTTP or WebSocket (467ms)
  ✓  18 [chromium] › tests/e2e/navigation.spec.ts:189:1 › the elements are defined and worded from the label table inside the shell (404ms)
  ✓  17 [chromium] › tests/e2e/accessibility.spec.ts:231:1 › reduced motion stops every transition (471ms)
  ✓  20 [chromium] › tests/e2e/accessibility.spec.ts:273:1 › more contrast strengthens borders; forced colours keep the chosen option (386ms)
  ✓  19 [chromium] › tests/e2e/navigation.spec.ts:247:1 › the tree follows the ARIA treeview keys and reads children as nodes open (567ms)
  ✓  21 [chromium] › tests/e2e/accessibility.spec.ts:317:3 › no horizontal scroll at 320 px wide and 100 % text (445ms)
  ✓  22 [chromium] › tests/e2e/navigation.spec.ts:283:1 › pins, section order and library collapse are kept on the device across a reload (713ms)
  ✓  23 [chromium] › tests/e2e/accessibility.spec.ts:317:3 › no horizontal scroll at 320 px wide and 200 % text (494ms)
  ✓  24 [chromium] › tests/e2e/navigation.spec.ts:327:1 › refused device storage keeps the choices for this page and says why (367ms)
  ✓  25 [chromium] › tests/e2e/accessibility.spec.ts:317:3 › no horizontal scroll at 1280 px wide and 200 % text (546ms)
  ✓  26 [chromium] › tests/e2e/navigation.spec.ts:352:1 › the tree shows the error and offline states and reads again on Retry (520ms)
  ✓  27 [chromium] › tests/e2e/accessibility.spec.ts:317:3 › no horizontal scroll at 640 px wide and 200 % text (409ms)
  ✓  28 [chromium] › tests/e2e/navigation.spec.ts:374:3 › the five states are distinct and use token colours in the light theme (324ms)
  ✓  29 [chromium] › tests/e2e/accessibility.spec.ts:341:1 › a right-to-left locale mirrors the header and navigation (347ms)
  ✓  30 [chromium] › tests/e2e/navigation.spec.ts:443:3 › tree controls draw the 2px --fs-focus-ring ring in the light theme (405ms)
  ✓  31 [chromium] › tests/e2e/ocfl-findings.spec.ts:37:1 › the findings section reads the real findings route with the token (435ms)
  ✓  32 [chromium] › tests/e2e/navigation.spec.ts:374:3 › the five states are distinct and use token colours in the dark theme (357ms)
  ✓  33 [chromium] › tests/e2e/ocfl-findings.spec.ts:68:1 › Validate now starts runs through the real run route and says nothing was open (400ms)
  ✓  34 [chromium] › tests/e2e/navigation.spec.ts:443:3 › tree controls draw the 2px --fs-focus-ring ring in the dark theme (365ms)
  ✓  35 [chromium] › tests/e2e/theme.spec.ts:100:1 › official FileSquid tokens apply in light and dark and meet WCAG 2.2 AA (439ms)
  ✓  36 [chromium] › tests/e2e/navigation.spec.ts:469:1 › tree motion follows the token, stops under reduced motion, and forced colours mark the current item (449ms)
  ✓  37 [chromium] › tests/e2e/theme.spec.ts:126:1 › the official mark fills the brand slot, with its dark-surface variant in dark (442ms)
  ✓  38 [chromium] › tests/e2e/navigation.spec.ts:510:3 › the tree and breadcrumb reflow at 320 px and 100 % text (472ms)
  ✓  40 [chromium] › tests/e2e/navigation.spec.ts:510:3 › the tree and breadcrumb reflow at 320 px and 200 % text (412ms)
  ✓  39 [chromium] › tests/e2e/theme.spec.ts:151:1 › the theme follows the OS by default and the control's choice persists across a reload (769ms)
  ✓  41 [chromium] › tests/e2e/navigation.spec.ts:510:3 › the tree and breadcrumb reflow at 1280 px and 200 % text (409ms)
  ✓  42 [chromium] › tests/e2e/theme.spec.ts:196:1 › the chosen theme option and the Settings API status are drawn as UI-S-015 lists them (753ms)
  42 passed (20.6s)
```

### Playwright, ImageSquid production build

```text
Running 5 tests using 1 worker
  ✓  1 [chromium] › tests/e2e/preview/production-build.spec.ts:67:1 › the built page is static, mounts the shell and defines every custom element (412ms)
  ✓  2 [chromium] › tests/e2e/preview/production-build.spec.ts:84:1 › sidebar, header and Settings words come from the active label table (UI-S-025, UI-S-033) (386ms)
  ✓  3 [chromium] › tests/e2e/preview/production-build.spec.ts:124:1 › computed ImageSquid token values are exact in light and dark (293ms)
  ✓  4 [chromium] › tests/e2e/preview/production-build.spec.ts:146:1 › a refused health call shows the unavailable alert in Settings, About (292ms)
  ✓  5 [chromium] › tests/e2e/preview/production-build.spec.ts:159:1 › the library flow runs in ImageSquid with its own label table (UI-1.1) (365ms)
  5 passed (9.0s)
```

### Playwright, ImageSquid on `filesquid-server`

```text
Running 9 tests using 1 worker
  ✓  1 [chromium] › tests/e2e/app.spec.ts:14:1 › ImageSquid brand, album vocabulary and the API status in Settings only (424ms)
  ✓  2 [chromium] › tests/e2e/app.spec.ts:64:1 › ImageSquid tokens override FileSquid defaults in light and dark (IS-S-008) (248ms)
  ✓  3 [chromium] › tests/e2e/app.spec.ts:89:1 › ImageSquid's theme control keeps a dark choice across a reload (389ms)
  ✓  4 [chromium] › tests/e2e/app.spec.ts:110:1 › ImageSquid names missing credentials in its own words (233ms)
  ✓  5 [chromium] › tests/e2e/app.spec.ts:119:1 › ImageSquid reaches the FileSquid Monitor with its own vocabulary (415ms)
  ✓  6 [chromium] › tests/e2e/app.spec.ts:137:3 › ImageSquid inherits the keyboard, naming and focus contracts in light (UI-S-016) (386ms)
  ✓  7 [chromium] › tests/e2e/app.spec.ts:137:3 › ImageSquid inherits the keyboard, naming and focus contracts in dark (UI-S-016) (444ms)
  ✓  8 [chromium] › tests/e2e/app.spec.ts:190:3 › the navigation tree and view states speak ImageSquid in light (UI-F2.1a) (284ms)
  ✓  9 [chromium] › tests/e2e/app.spec.ts:190:3 › the navigation tree and view states speak ImageSquid in dark (UI-F2.1a) (291ms)
  9 passed (10.3s)
```

### Pre-commit hook (ordinary run, `bash scripts/pre-commit.sh`)

```text
========================================
========================================
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
✅ ESLint passed (6s).
✅ TypeScript passed (8s).
      Tests  1396 passed | 1 skipped (1397)
✅ Vitest passed (41s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
✅ ESLint passed (2s).
✅ TypeScript passed (7s).
      Tests  38 passed | 1 skipped (39)
✅ Vitest passed (4s).
ℹ️  Playwright e2e skipped for apps/imagesquid (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: npm:apps/filesquid/ui,apps/imagesquid).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui,apps/imagesquid (94s)
========================================
```
