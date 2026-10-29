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
- **Restore and API-F2.13**: the restore goes through the existing `FileSource.restore`. API-F2.13 (PR #135) merged into main during this task and made it a command; after merging main, `RestoreOptions` extends its `WriteOptions`, the Versions view sends the object's head from `GET /files/versions` as the expected revision with the optional label in the input, and a head another writer moved meanwhile is refused as `conflict` in the dialog.

No API route, field or event was assumed or stubbed.

## Review fixes (Cubic round 1: 1 P1, 24 P2, 7 P3)

- Version numbers are compared and incremented as `bigint`, exact beyond the safe integer range (P1); the count shows every version though the list keeps the newest 10,000.
- The Versions view drops an open dialog on any new reference or when it leaves the document, applies nothing from a restore answered after the file changed (it still reports the commit so the listing follows), and the file panel returns to the tabs when another file opens and hands a remounted panel the file again.
- `FsFileBrowser.reload()` refreshes the viewer only for the newest read.
- The property table hides Clear filter and empties the filter while loading, failed or cleared; the fixity rows stay within their bound with SHA-256 among them; the dialog's disabled close button looks disabled.
- `MemoryFileSource` keeps a restore's label, names a file object's file in its versions, compares only content for history (as the API compares digests) and keys contents without collisions.
- Test fakes: abort-aware gates, distinct contents, unknown versions and paths refused, command replay by operation ID in the e2e restore fake; the 503 tolerance of the states test is scoped to the details route; stronger assertions (full history-size query, value filtering, the stale answer, lazy factories, the shell's row layout).
- Docs: the README example is runnable, removed versions are described, the plan names `GET /files/versions`, the refusal scenario sets up its dialog, and this walkthrough names no machine path.

## Review fixes (Cubic round 2: 0 P1, 1 P2, 1 P3)

- `MemoryFileSource.restore` refuses a label the restorations route refuses (blank, over 512 UTF-8 bytes, or with a control character) as `invalid_request`, before the revision check, as the route validates its body before the envelope (P2). The rule lives once in `src/files/model.ts` as `validLabel` with `MAX_LABEL_BYTES`; `labelProblem` in the restore dialog applies it to the trimmed text it sends, and the dialog re-exports `MAX_LABEL_BYTES` so the package's public surface is unchanged. Unit tests: `validLabel` in `files-model.test.ts` (the 512-byte edge, the untrimmed byte count, C0, DEL and C1 characters) and the fixture refusal in `memory-history.test.ts` (three refused labels leave the head alone; a 512-byte label is kept).
- The `PropertyTable` contract comments describe reset-on-load (P3): the filter field stays in the layout through every state, its word is reset when a load starts or the file is cleared, kept only across `rows(...)` renders of the same load, and Try again resets it through the next load.
- `origin/main` was merged by its own merge commit first (PR #137, #132, #140 and #141; the branch already held #133 and #135). No conflict: the MVP progress block merged through the `mvp-progress` driver. After PR #140 the hook runs only `@filesquid/ui`'s suite, whose `distribution-tables.test.ts` reads ImageSquid's label and token tables, so the hook's scope line is now `npm:apps/filesquid/ui`.

## How it was tested

1. Tests first: the unit and UI tests of the models, the history source, the three elements, the route and the shell ran red before the implementation (below), then green. The history reads of `MemoryFileSource` and the file-route fake were added when the first full Playwright run showed the existing browser specs logging 404s for `files/versions`; their unit tests (`tests/unit/memory-history.test.ts`) were written with that change.
2. `npx prettier --write .`, `npx eslint . --fix`, `npx tsc --noEmit` in `apps/filesquid/ui` and `apps/imagesquid`: clean.
3. `npm test` in both packages.
4. Round 2 (after merging main): the three label test cases in `files-model.test.ts` and `memory-history.test.ts` ran red, then green; `npx prettier --write .`, `npx eslint .`, `npx tsc --noEmit` and `npm test` in `apps/filesquid/ui` are clean; the production-build Playwright suite (`vite build` plus `vite preview`) ran through the same kind of temporary config, deleted before committing; the ordinary hook passed (`Test scope: npm:apps/filesquid/ui`). ImageSquid's own suite is not run by the hook during the MVP (owner decision 2026-10-23) and nothing in `apps/imagesquid` changed in this round.
5. Playwright, both suites of both packages. The installed Playwright expects a newer Chromium than this machine has, so the suites ran through temporary configs that point `executablePath` at the machine's preinstalled Chromium; the server-backed suites used the already-built `filesquid-server` binary instead of `cargo run`, so no Rust build ran (npm-only scope). The configs were deleted before committing. The clock of the file panel spec is fixed with `page.clock`. No mocked environment values were used.

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
 ✓ tests/ui/host-indicator.test.ts (40 tests) 530ms
 ✓ tests/unit/theme.test.ts (36 tests) 93ms
 ✓ tests/ui/nav-tree.test.ts (30 tests) 1171ms
 ✓ tests/ui/file-browser-writes.test.ts (47 tests) 1815ms
 ✓ tests/ui/app-shell.test.ts (24 tests) 619ms
 ✓ tests/ui/file-browser.test.ts (37 tests) 1889ms
 ✓ tests/unit/files-source.test.ts (29 tests) 106ms
 ✓ tests/ui/ocfl-findings.test.ts (22 tests) 492ms
 ✓ tests/ui/library-start.test.ts (22 tests) 297ms
 ✓ tests/ui/monitor.test.ts (22 tests) 5727ms
 ✓ tests/unit/files-memory-source.test.ts (35 tests) 91ms
 ✓ tests/ui/library-repair.test.ts (22 tests) 389ms
 ✓ tests/unit/files-ops.test.ts (55 tests) 197ms
 ✓ tests/ui/library-detail.test.ts (20 tests) 474ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 602ms
 ✓ tests/unit/component-styles.test.ts (226 tests) 86ms
 ✓ tests/unit/client-io.test.ts (23 tests) 52ms
 ✓ tests/ui/file-preview.test.ts (16 tests) 225ms
 ✓ tests/ui/versions-panel.test.ts (22 tests) 9285ms
 ✓ tests/ui/navigation-view.test.ts (20 tests) 272ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 53ms
 ✓ tests/ui/settings.test.ts (10 tests) 281ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 37ms
 ✓ tests/ui/details-panel.test.ts (11 tests) 351ms
 ✓ tests/ui/file-panel.test.ts (11 tests) 477ms
 ✓ tests/unit/media-handle.test.ts (14 tests) 120ms
 ✓ tests/ui/nav-tree-live.test.ts (11 tests) 488ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 44ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 112ms
 ✓ tests/unit/library-client.test.ts (18 tests) 66ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 33ms
 ✓ tests/unit/navigation-client.test.ts (13 tests) 25ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 31ms
 ✓ tests/unit/commands.test.ts (46 tests) 37ms
 ✓ tests/ui/files-route.test.ts (10 tests) 576ms
 ✓ tests/unit/files-model.test.ts (58 tests) 58ms
 ✓ tests/unit/host-descriptor.test.ts (43 tests) 28ms
 ✓ tests/ui/states.test.ts (26 tests) 163ms
 ✓ tests/unit/versions-model.test.ts (14 tests) 56ms
 ✓ tests/unit/admin-client.test.ts (10 tests) 25ms
 ✓ tests/unit/files-window.test.ts (19 tests) 16ms
 ✓ tests/unit/focus.test.ts (16 tests) 99ms
 ✓ tests/unit/library-errors.test.ts (15 tests) 22ms
 ✓ tests/unit/navigation-contract.test.ts (30 tests) 26ms
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 708ms
 ✓ tests/unit/details-model.test.ts (7 tests) 38ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 25ms
 ✓ tests/ui/library-route.test.ts (6 tests) 102ms
 ✓ tests/unit/history-source.test.ts (6 tests) 17ms
 ✓ tests/unit/memory-history.test.ts (8 tests) 24ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 72ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 77ms
 ✓ tests/unit/client.test.ts (8 tests) 22ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 12ms
 ✓ tests/unit/library-model.test.ts (7 tests) 15ms
 ✓ tests/unit/preferences.test.ts (10 tests) 20ms
 ✓ tests/unit/host-client.test.ts (4 tests) 13ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 72ms
 ✓ tests/unit/view-state.test.ts (5 tests) 10ms
 ✓ tests/unit/contrast.test.ts (13 tests) 21ms
 ✓ tests/unit/distribution.test.ts (3 tests) 8ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 26ms
 ✓ tests/unit/contract.test.ts (2 tests) 243ms
 Test Files  63 passed (63)
      Tests  1411 passed | 1 skipped (1412)
   Duration  29.43s (transform 3.76s, setup 2.47s, import 8.95s, tests 29.16s, environment 35.36s)
```

### npm test (apps/imagesquid)

```text
 ✓ tests/unit/test-reporting.test.ts (12 tests) 31ms
 ✓ tests/unit/theme.test.ts (12 tests) 36ms
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 743ms
 ✓ tests/ui/distribution.test.ts (5 tests) 167ms
 Test Files  4 passed (4)
      Tests  38 passed | 1 skipped (39)
   Duration  2.20s (transform 1.40s, setup 264ms, import 1.53s, tests 977ms, environment 1.97s)
```

### Playwright, production build (`vite build` plus `vite preview`, apps/filesquid/ui)

```text
Running 90 tests using 2 workers
  ✓   2 [chromium] › tests/e2e/preview/file-browser.spec.ts:96:1 › the browser is a defined element and lists display names in the list layout from the label table (586ms)
  ✓   3 [chromium] › tests/e2e/preview/file-browser.spec.ts:140:1 › a file row shows size, type hint, modified date and version (403ms)
  ✓   1 [chromium] › tests/e2e/preview/file-panel.spec.ts:102:5 › light theme › File info (35): header, tab, filter, count, rows and footer (1.1s)
  ✓   4 [chromium] › tests/e2e/preview/file-browser.spec.ts:152:1 › computed token values are exact in the browser in light and dark (550ms)
  ✓   6 [chromium] › tests/e2e/preview/file-browser.spec.ts:175:1 › opening a folder updates the breadcrumb and the address, and Back returns (574ms)
  ✓   5 [chromium] › tests/e2e/preview/file-panel.spec.ts:212:5 › light theme › Versions (26): Back, count, eyebrow, rows, Current, menu and note (1.0s)
  ✓   7 [chromium] › tests/e2e/preview/file-browser.spec.ts:196:1 › an image previews through a content handle that is used only as the media src (586ms)
  ✓   9 [chromium] › tests/e2e/preview/file-browser.spec.ts:231:1 › a handle that fails is replaced once; a second failure shows the error and mints no third (492ms)
  ✓   8 [chromium] › tests/e2e/preview/file-panel.spec.ts:337:5 › light theme › restore (27): the dialog, one call with the label, and the new current version (1.1s)
  ✓  10 [chromium] › tests/e2e/preview/file-browser.spec.ts:247:1 › a video resumes at the same position and paused state after one new handle (747ms)
  ✓  11 [chromium] › tests/e2e/preview/file-panel.spec.ts:460:5 › light theme › panel states (25): loading, no match and error keep the header, tabs and filter (913ms)
  ✓  12 [chromium] › tests/e2e/preview/file-browser.spec.ts:283:1 › audio plays natively and is named by the display name (513ms)
  ✓  13 [chromium] › tests/e2e/preview/file-panel.spec.ts:537:5 › light theme › keyboard: the tab, the history button and the dialog keep focus where it belongs (733ms)
  ✓  14 [chromium] › tests/e2e/preview/file-browser.spec.ts:296:1 › text previews through one ranged read of the first 64 KiB and says it is cut (451ms)
  ✓  16 [chromium] › tests/e2e/preview/file-browser.spec.ts:312:1 › a format it cannot show says so and shows no viewer (500ms)
  ✓  15 [chromium] › tests/e2e/preview/file-panel.spec.ts:102:5 › dark theme › File info (35): header, tab, filter, count, rows and footer (777ms)
  ✓  17 [chromium] › tests/e2e/preview/file-browser.spec.ts:342:1 › the viewer steps through files, closes with Escape and returns focus to the row (483ms)
  ✓  19 [chromium] › tests/e2e/preview/file-browser.spec.ts:355:1 › an upload is one PUT of the client's facts that creates the file's own object (415ms)
  ✓  18 [chromium] › tests/e2e/preview/file-panel.spec.ts:212:5 › dark theme › Versions (26): Back, count, eyebrow, rows, Current, menu and note (1.0s)
  ✓  20 [chromium] › tests/e2e/preview/file-browser.spec.ts:404:1 › a rename posts to the renames route, never to moves, and shows the new display name (553ms)
  ✓  22 [chromium] › tests/e2e/preview/file-browser.spec.ts:433:1 › a refused rename shows the label for its reason and keeps the display name (461ms)
  ✓  21 [chromium] › tests/e2e/preview/file-panel.spec.ts:337:5 › dark theme › restore (27): the dialog, one call with the label, and the new current version (1.0s)
  ✓  23 [chromium] › tests/e2e/preview/file-browser.spec.ts:452:1 › writes are single calls: new folder, move, remove and Undo (841ms)
  ✓  24 [chromium] › tests/e2e/preview/file-panel.spec.ts:460:5 › dark theme › panel states (25): loading, no match and error keep the header, tabs and filter (806ms)
  ✓  25 [chromium] › tests/e2e/preview/file-browser.spec.ts:499:1 › a refused write shows the label for its reason and changes nothing (441ms)
  ✓  26 [chromium] › tests/e2e/preview/file-panel.spec.ts:537:5 › dark theme › keyboard: the tab, the history button and the dialog keep focus where it belongs (716ms)
  ✓  27 [chromium] › tests/e2e/preview/file-browser.spec.ts:514:1 › a long folder is virtualised, pages in on demand and stays keyboard reachable (649ms)
  ✓  28 [chromium] › tests/e2e/preview/host-indicator.spec.ts:96:1 › the indicator is a defined custom element in the shell header, named from the label table (334ms)
  ✓  29 [chromium] › tests/e2e/preview/file-browser.spec.ts:554:1 › the layout does not scroll sideways at 320 px, at 200 % text size, or both, in list and grid (462ms)
  ✓  30 [chromium] › tests/e2e/preview/host-indicator.spec.ts:110:1 › the desktop host says only you can read your files, and shows the not-enrolled state (314ms)
  ✓  31 [chromium] › tests/e2e/preview/file-browser.spec.ts:571:1 › motion stops under reduced motion and focus is the 2px --fs-focus-ring ring (447ms)
  ✓  32 [chromium] › tests/e2e/preview/host-indicator.spec.ts:124:1 › a server names its operator, who can read the files, and states the at-rest duty (329ms)
  ✓  34 [chromium] › tests/e2e/preview/host-indicator.spec.ts:137:1 › a server without an operator name uses the unnamed-operator label (303ms)
  ✓  33 [chromium] › tests/e2e/preview/file-browser.spec.ts:598:1 › a failed listing shows the error state with Try again, and retrying loads it (439ms)
  ✓  35 [chromium] › tests/e2e/preview/host-indicator.spec.ts:149:1 › the browser host says files are decrypted only in this browser (332ms)
  ✓  36 [chromium] › tests/e2e/preview/library.spec.ts:47:1 › the start page offers create, open and recover in the active vocabulary (341ms)
  ✓  37 [chromium] › tests/e2e/preview/host-indicator.spec.ts:162:1 › enrolled backup and sync are shown apart from the host, as ciphertext only (326ms)
  ✓  38 [chromium] › tests/e2e/preview/library.spec.ts:80:1 › (1) create asks for a folder, passes the location ID on and shows the library, with no path (492ms)
  ✓  39 [chromium] › tests/e2e/preview/host-indicator.spec.ts:187:1 › a host.change event updates the indicator without a reload or another read (331ms)
  ✓  40 [chromium] › tests/e2e/preview/library.spec.ts:102:1 › (1) open passes the location ID to the open route (391ms)
  ✓  41 [chromium] › tests/e2e/preview/host-indicator.spec.ts:209:1 › events.resync makes the indicator read the descriptor again (352ms)
  ✓  42 [chromium] › tests/e2e/preview/library.spec.ts:116:1 › (1) recover opens the copy, rebuilds from files and reports without repairing (434ms)
  ✓  44 [chromium] › tests/e2e/preview/library.spec.ts:153:1 › (1) copy asks for a destination and passes it as destination_location_id (364ms)
  ✓  43 [chromium] › tests/e2e/preview/host-indicator.spec.ts:222:1 › loading shows no host until the descriptor arrives (1.1s)
  ✓  45 [chromium] › tests/e2e/preview/library.spec.ts:176:1 › (2) a cancelled folder choice sends no library request and says nothing changed (327ms)
  ✓  46 [chromium] › tests/e2e/preview/host-indicator.spec.ts:234:1 › a failed read shows the error state and never a guessed host, then Retry recovers (406ms)
  ✓  47 [chromium] › tests/e2e/preview/library.spec.ts:190:1 › (3) 503 unavailable is the unavailable state with its reason and a Retry, and the choices stay (493ms)
  ✓  48 [chromium] › tests/e2e/preview/host-indicator.spec.ts:256:1 › the header shows a compact badge with the short host label, not the statement (322ms)
  ✓  49 [chromium] › tests/e2e/preview/library.spec.ts:217:1 › (4) confirm sends the consent for exactly the named objects and shows each result (525ms)
  ✓  50 [chromium] › tests/e2e/preview/host-indicator.spec.ts:271:1 › Enter opens the popover with the full statement; Escape closes it and returns focus (440ms)
  ✓  51 [chromium] › tests/e2e/preview/library.spec.ts:254:1 › (4) cancel and Escape send nothing and return focus to the opener (491ms)
  ✓  52 [chromium] › tests/e2e/preview/host-indicator.spec.ts:297:1 › Privacy settings opens the Settings page, whose Privacy section has the full statement (529ms)
  ✓  53 [chromium] › tests/e2e/preview/library.spec.ts:277:1 › (4) a refused repair says nothing was changed and shows no result (450ms)
  ✓  55 [chromium] › tests/e2e/preview/library.spec.ts:292:1 › (5) the Monitor's W010 note links to the library's repair offer (363ms)
  ✓  54 [chromium] › tests/e2e/preview/host-indicator.spec.ts:333:1 › computed token values are exact in light and dark (UI-S-024, mockup 40) (845ms)
  ✓  57 [chromium] › tests/e2e/preview/host-indicator.spec.ts:378:1 › the badge is reachable by keyboard and has a visible focus outline (471ms)
  ✓  56 [chromium] › tests/e2e/preview/library.spec.ts:345:1 › colours come from tokens in light and dark (736ms)
  ✓  59 [chromium] › tests/e2e/preview/navigation.spec.ts:62:1 › the live tree fills the shell's navigation slot, worded from the label table (344ms)
  ✓  58 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:116:5 › light theme › the shell frame (UI-S-025): sidebar, header, footer, controls and strings (604ms)
  ✓  60 [chromium] › tests/e2e/preview/navigation.spec.ts:83:1 › a level is read page by page through next_cursor until it is null (301ms)
  ✓  61 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:200:5 › light theme › the navigation tree as built (55, UI-S-017): eyebrows, 30 px rows, 16 px icons (472ms)
  ✓  62 [chromium] › tests/e2e/preview/navigation.spec.ts:121:1 › a destination opens with a breadcrumb filled from ancestors (478ms)
  ✓  63 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:238:5 › light theme › welcome (01, UI-S-027): title, lead, cards, privacy box and footer (543ms)
  ✓  64 [chromium] › tests/e2e/preview/navigation.spec.ts:142:1 › a deep link to a destination shows its breadcrumb after a reload (450ms)
  ✓  66 [chromium] › tests/e2e/preview/navigation.spec.ts:154:1 › navigation.change reads only the named level again, without a reload (383ms)
  ✓  65 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:294:5 › light theme › folder-choice unavailable (03) and the view states treatment (56) (522ms)
  ✓  67 [chromium] › tests/e2e/preview/navigation.spec.ts:177:1 › a 503 shows the unavailable state with Retry, never an empty tree (433ms)
  ✓  68 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:329:5 › light theme › library page (41, UI-S-027): facts rows, actions and the W010 note (577ms)
  ✓  69 [chromium] › tests/e2e/preview/navigation.spec.ts:194:1 › computed token values of the tree and breadcrumb are exact in light and dark (672ms)
  ✓  70 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:379:5 › light theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (533ms)
  ✓  71 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:174:1 › (1) an E023 finding shows with its code beside the degraded health reason (399ms)
  ✓  72 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:424:5 › light theme › Monitor (31) and its OCFL validation section (54) (543ms)
  ✓  73 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:200:1 › (2) the finding disappears after a clean revalidation's health.change, without a reload (451ms)
  ✓  74 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:485:5 › light theme › host popover (40) and Settings (32): 372 px overlay, Appearance and About (470ms)
  ✓  75 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:220:1 › (3) a W010 warning is listed apart from errors and health stays ok (399ms)
  ✓  77 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:238:1 › (4) no location is rendered when the API withholds it (368ms)
  ✓  76 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:116:5 › dark theme › the shell frame (UI-S-025): sidebar, header, footer, controls and strings (629ms)
  ✓  78 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:250:1 › (5) the last and next run times come from the findings route; Validate now starts a run (439ms)
  ✓  79 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:200:5 › dark theme › the navigation tree as built (55, UI-S-017): eyebrows, 30 px rows, 16 px icons (449ms)
  ✓  80 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:289:1 › error and warning codes take their colours from tokens in light and dark (437ms)
  ✓  81 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:238:5 › dark theme › welcome (01, UI-S-027): title, lead, cards, privacy box and footer (545ms)
  ✓  82 [chromium] › tests/e2e/preview/production-build.spec.ts:63:1 › the built page is static, mounts the shell and defines every custom element (344ms)
  ✓  83 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:294:5 › dark theme › folder-choice unavailable (03) and the view states treatment (56) (502ms)
  ✓  84 [chromium] › tests/e2e/preview/production-build.spec.ts:80:1 › sidebar, header and Settings words come from the active label table (UI-S-025, UI-S-033) (444ms)
  ✓  86 [chromium] › tests/e2e/preview/production-build.spec.ts:124:1 › computed token values are exact in light and dark (UI-S-024, CA-1) (390ms)
  ✓  85 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:329:5 › dark theme › library page (41, UI-S-027): facts rows, actions and the W010 note (586ms)
  ✓  87 [chromium] › tests/e2e/preview/production-build.spec.ts:146:1 › a refused health call shows the unavailable alert in Settings, About, not a console error (315ms)
  ✓  88 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:379:5 › dark theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (518ms)
  ✓  89 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:424:5 › dark theme › Monitor (31) and its OCFL validation section (54) (403ms)
  ✓  90 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:485:5 › dark theme › host popover (40) and Settings (32): 372 px overlay, Appearance and About (363ms)
  90 passed (34.5s)
```

### Playwright, built UI served by `filesquid-server` (apps/filesquid/ui)

```text
Running 42 tests using 2 workers
  ✓   1 [chromium] › tests/e2e/accessibility.spec.ts:67:1 › landmarks, headings and control names come from the label table (448ms)
  ✓   2 [chromium] › tests/e2e/app.spec.ts:35:1 › shell mounts with FileSquid brand, internal vocabulary and the API status in Settings only (544ms)
  ✓   4 [chromium] › tests/e2e/app.spec.ts:63:1 › design tokens switch between light and dark (303ms)
  ✓   3 [chromium] › tests/e2e/accessibility.spec.ts:112:3 › Tab order and a 2px --fs-focus-ring focus ring in the light theme (500ms)
  ✓   5 [chromium] › tests/e2e/app.spec.ts:81:1 › client-side routes fall back to the distribution index (237ms)
  ✓   7 [chromium] › tests/e2e/app.spec.ts:87:1 › without a token Settings, About names the missing credentials from the label table (287ms)
  ✓   6 [chromium] › tests/e2e/accessibility.spec.ts:112:3 › Tab order and a 2px --fs-focus-ring focus ring in the dark theme (482ms)
  ✓   8 [chromium] › tests/e2e/app.spec.ts:99:1 › Monitor asks for the admin token, then shows live health, processes and the journal (491ms)
  ✓   9 [chromium] › tests/e2e/accessibility.spec.ts:150:1 › light-DOM content slotted into the shell gets the same focus ring (429ms)
  ✓  10 [chromium] › tests/e2e/app.spec.ts:135:1 › a wrong token is named as invalid credentials (386ms)
  ✓  11 [chromium] › tests/e2e/accessibility.spec.ts:178:1 › the skip control appears on focus and moves focus to main without routing (378ms)
  ✓  13 [chromium] › tests/e2e/accessibility.spec.ts:196:1 › opening the Monitor from the keyboard moves focus to main (470ms)
  ✓  12 [chromium] › tests/e2e/app.spec.ts:146:1 › the signed-in token stays in memory: not in storage, URL or DOM, gone on reload (742ms)
  ✓  15 [chromium] › tests/e2e/app.spec.ts:177:1 › the desktop launch token authorises the shell, the Monitor and the event channel (356ms)
  ✓  14 [chromium] › tests/e2e/accessibility.spec.ts:214:1 › a keyboard sign-in to the Monitor keeps focus in the Monitor (675ms)
  ✓  16 [chromium] › tests/e2e/app.spec.ts:270:3 › inside the desktop webview (custom URI scheme adapter) › requests use the custom scheme and events use Tauri, not same-origin HTTP or WebSocket (428ms)
  ✓  17 [chromium] › tests/e2e/accessibility.spec.ts:231:1 › reduced motion stops every transition (437ms)
  ✓  19 [chromium] › tests/e2e/accessibility.spec.ts:273:1 › more contrast strengthens borders; forced colours keep the chosen option (287ms)
  ✓  18 [chromium] › tests/e2e/navigation.spec.ts:189:1 › the elements are defined and worded from the label table inside the shell (363ms)
  ✓  20 [chromium] › tests/e2e/accessibility.spec.ts:317:3 › no horizontal scroll at 320 px wide and 100 % text (406ms)
  ✓  21 [chromium] › tests/e2e/navigation.spec.ts:247:1 › the tree follows the ARIA treeview keys and reads children as nodes open (429ms)
  ✓  22 [chromium] › tests/e2e/accessibility.spec.ts:317:3 › no horizontal scroll at 320 px wide and 200 % text (399ms)
  ✓  23 [chromium] › tests/e2e/navigation.spec.ts:283:1 › pins, section order and library collapse are kept on the device across a reload (551ms)
  ✓  24 [chromium] › tests/e2e/accessibility.spec.ts:317:3 › no horizontal scroll at 1280 px wide and 200 % text (378ms)
  ✓  25 [chromium] › tests/e2e/navigation.spec.ts:327:1 › refused device storage keeps the choices for this page and says why (361ms)
  ✓  26 [chromium] › tests/e2e/accessibility.spec.ts:317:3 › no horizontal scroll at 640 px wide and 200 % text (372ms)
  ✓  27 [chromium] › tests/e2e/navigation.spec.ts:352:1 › the tree shows the error and offline states and reads again on Retry (420ms)
  ✓  28 [chromium] › tests/e2e/accessibility.spec.ts:341:1 › a right-to-left locale mirrors the header and navigation (250ms)
  ✓  29 [chromium] › tests/e2e/navigation.spec.ts:374:3 › the five states are distinct and use token colours in the light theme (297ms)
  ✓  30 [chromium] › tests/e2e/ocfl-findings.spec.ts:37:1 › the findings section reads the real findings route with the token (489ms)
  ✓  31 [chromium] › tests/e2e/navigation.spec.ts:443:3 › tree controls draw the 2px --fs-focus-ring ring in the light theme (345ms)
  ✓  32 [chromium] › tests/e2e/ocfl-findings.spec.ts:68:1 › Validate now starts runs through the real run route and says nothing was open (419ms)
  ✓  33 [chromium] › tests/e2e/navigation.spec.ts:374:3 › the five states are distinct and use token colours in the dark theme (375ms)
  ✓  34 [chromium] › tests/e2e/theme.spec.ts:100:1 › official FileSquid tokens apply in light and dark and meet WCAG 2.2 AA (366ms)
  ✓  35 [chromium] › tests/e2e/navigation.spec.ts:443:3 › tree controls draw the 2px --fs-focus-ring ring in the dark theme (359ms)
  ✓  36 [chromium] › tests/e2e/theme.spec.ts:126:1 › the official mark fills the brand slot, with its dark-surface variant in dark (435ms)
  ✓  37 [chromium] › tests/e2e/navigation.spec.ts:469:1 › tree motion follows the token, stops under reduced motion, and forced colours mark the current item (423ms)
  ✓  39 [chromium] › tests/e2e/navigation.spec.ts:510:3 › the tree and breadcrumb reflow at 320 px and 100 % text (428ms)
  ✓  38 [chromium] › tests/e2e/theme.spec.ts:151:1 › the theme follows the OS by default and the control's choice persists across a reload (598ms)
  ✓  40 [chromium] › tests/e2e/navigation.spec.ts:510:3 › the tree and breadcrumb reflow at 320 px and 200 % text (413ms)
  ✓  42 [chromium] › tests/e2e/navigation.spec.ts:510:3 › the tree and breadcrumb reflow at 1280 px and 200 % text (326ms)
  ✓  41 [chromium] › tests/e2e/theme.spec.ts:196:1 › the chosen theme option and the Settings API status are drawn as UI-S-015 lists them (737ms)
  42 passed (17.5s)
```

### Playwright, ImageSquid production build

```text
Running 5 tests using 1 worker
  ✓  1 [chromium] › tests/e2e/preview/production-build.spec.ts:67:1 › the built page is static, mounts the shell and defines every custom element (326ms)
  ✓  2 [chromium] › tests/e2e/preview/production-build.spec.ts:84:1 › sidebar, header and Settings words come from the active label table (UI-S-025, UI-S-033) (336ms)
  ✓  3 [chromium] › tests/e2e/preview/production-build.spec.ts:124:1 › computed ImageSquid token values are exact in light and dark (262ms)
  ✓  4 [chromium] › tests/e2e/preview/production-build.spec.ts:146:1 › a refused health call shows the unavailable alert in Settings, About (218ms)
  ✓  5 [chromium] › tests/e2e/preview/production-build.spec.ts:159:1 › the library flow runs in ImageSquid with its own label table (UI-1.1) (329ms)
  5 passed (7.5s)
```

### Playwright, ImageSquid on `filesquid-server`

```text
Running 9 tests using 1 worker
  ✓  1 [chromium] › tests/e2e/app.spec.ts:14:1 › ImageSquid brand, album vocabulary and the API status in Settings only (433ms)
  ✓  2 [chromium] › tests/e2e/app.spec.ts:64:1 › ImageSquid tokens override FileSquid defaults in light and dark (IS-S-008) (237ms)
  ✓  3 [chromium] › tests/e2e/app.spec.ts:89:1 › ImageSquid's theme control keeps a dark choice across a reload (358ms)
  ✓  4 [chromium] › tests/e2e/app.spec.ts:110:1 › ImageSquid names missing credentials in its own words (214ms)
  ✓  5 [chromium] › tests/e2e/app.spec.ts:119:1 › ImageSquid reaches the FileSquid Monitor with its own vocabulary (377ms)
  ✓  6 [chromium] › tests/e2e/app.spec.ts:137:3 › ImageSquid inherits the keyboard, naming and focus contracts in light (UI-S-016) (348ms)
  ✓  7 [chromium] › tests/e2e/app.spec.ts:137:3 › ImageSquid inherits the keyboard, naming and focus contracts in dark (UI-S-016) (391ms)
  ✓  8 [chromium] › tests/e2e/app.spec.ts:190:3 › the navigation tree and view states speak ImageSquid in light (UI-F2.1a) (234ms)
  ✓  9 [chromium] › tests/e2e/app.spec.ts:190:3 › the navigation tree and view states speak ImageSquid in dark (UI-F2.1a) (246ms)
  9 passed (8.9s)
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
✅ ESLint passed (5s).
✅ TypeScript passed (6s).
      Tests  1411 passed | 1 skipped (1412)
✅ Vitest passed (34s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
✅ ESLint passed (2s).
✅ TypeScript passed (3s).
      Tests  38 passed | 1 skipped (39)
✅ Vitest passed (3s).
ℹ️  Playwright e2e skipped for apps/imagesquid (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: npm:apps/filesquid/ui,apps/imagesquid).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui,apps/imagesquid (75s)
========================================
```

## Raw test output (Cubic round 2, after merging main)

### Red run (the three label test cases before the fix)

```text

 RUN  v4.1.12 <repo>/apps/filesquid/ui

 ❯ tests/unit/memory-history.test.ts (9 tests | 1 failed) 31ms
     ✓ lists every version of the object, oldest first, with the head 6ms
     ✓ lists only the versions that changed the file: added, removed and restored 2ms
     ✓ counts each distinct content of the history once 2ms
     ✓ keeps a restore's label on its version, in the versions and the history 1ms
     × refuses a restore label the restorations route refuses, before the revision check 12ms
     ✓ lists no version that only changed the display name, as the API compares content 2ms
     ✓ names no file for an object that is not a file object 0ms
     ✓ counts two different contents of the same size as two 1ms
     ✓ refuses a file that never existed 1ms
 ❯ tests/unit/files-model.test.ts (60 tests | 2 failed) 63ms
     ✓ match the file routes 3ms
     ✓ shows the display name of a file and never its stored name 4ms
     ✓ keeps the API order, with a folder named by its last segment 1ms
     ✓ adds pending folders after the listed folders and drops one the listing now holds 1ms
     ✓ counts folders, files and bytes 1ms
     ✓ join, split and walk 1ms
     ✓ a change affects the path itself and everything below it 0ms
     ✓ accepts an ordinary name, trimmed 1ms
     ✓ refuses "" as empty 1ms
     ✓ refuses "   " as empty 0ms
     ✓ refuses "a/b" as separator 0ms
     ✓ refuses "a\\b" as separator 0ms
     ✓ refuses "." as reserved 0ms
     ✓ refuses ".." as reserved 0ms
     ✓ refuses "a\u0000b" as control 0ms
     ✓ refuses "a\u001fb" as control 0ms
     ✓ refuses "a
b" as control 0ms
     ✓ refuses "ab" as control 0ms
     ✓ refuses "éééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééé" as long 0ms
     ✓ counts bytes, not characters: 255 bytes pass and 256 do not 0ms
     × takes 1 to 512 UTF-8 bytes, not blank, without control characters, as the route does 7ms
     × refuses a blank, an over-long and a control-character label 1ms
     ✓ steps back one OCFL version 1ms
     ✓ 0 bytes is 0 B 16ms
     ✓ 999 bytes is 999 B 0ms
     ✓ 2048 bytes is 2 KB 0ms
     ✓ 421888 bytes is 412 KB 0ms
     ✓ 6481920 bytes is 6.2 MB 0ms
     ✓ 19293798 bytes is 18.4 MB 0ms
     ✓ 1288490189 bytes is 1.2 GB 0ms
     ✓ 3298534883328 bytes is 3 TB 0ms
     ✓ 1048524 bytes is 1 MB 0ms
     ✓ 1023 bytes is 1,023 B 0ms
     ✓ takes its words from the label table 0ms
     ✓ writes the date the way the locale does 5ms
     ✓ shows an unparseable time as given instead of a wrong date 0ms
     ✓ round-trips a location with folders 3ms
     ✓ the root has no folder segments 0ms
     ✓ does not read "" 0ms
     ✓ does not read "#/monitor" 0ms
     ✓ does not read "#/files" 0ms
     ✓ does not read "#/files/only-library" 0ms
     ✓ does not read "#/files/l/o/../x" 0ms
     ✓ does not read "#/files/l/o/%E0%A4%A" 0ms
     ✓ does not read "#/files/l/o/a%2F..%2Fb" 0ms
     ✓ does not read "#/files/l/o/a%5Cb" 0ms
     ✓ image/png previews as image 0ms
     ✓ image/jpeg previews as image 0ms
     ✓ image/svg+xml previews as image 0ms
     ✓ video/mp4 previews as video 0ms
     ✓ video/webm previews as video 0ms
     ✓ audio/mpeg previews as audio 0ms
     ✓ audio/wav previews as audio 0ms
     ✓ text/plain; charset=utf-8 previews as text 0ms
     ✓ text/csv previews as text 0ms
     ✓ application/json previews as text 0ms
     ✓ application/pdf previews as undefined 0ms
     ✓ application/x-blender previews as undefined 0ms
     ✓ image/tiff previews as undefined 0ms
     ✓ application/octet-stream previews as undefined 0ms

⎯⎯⎯⎯⎯⎯⎯ Failed Tests 3 ⎯⎯⎯⎯⎯⎯⎯

 FAIL  tests/unit/files-model.test.ts > validLabel > takes 1 to 512 UTF-8 bytes, not blank, without control characters, as the route does
AssertionError: expected undefined to be 512 // Object.is equality

- Expected:
512

+ Received:
undefined

 ❯ node_modules/allure-vitest/src/matchers.ts:510:70
 ❯ runWithNestedMatcherStepsSuppressed node_modules/allure-vitest/src/matchers.ts:395:19
 ❯ node_modules/allure-vitest/src/matchers.ts:510:6
 ❯ runMatcherStep node_modules/allure-vitest/src/matchers.ts:461:19
 ❯ Proxy.<anonymous> node_modules/allure-vitest/src/matchers.ts:509:11
 ❯ tests/unit/files-model.test.ts:155:29
    153| describe("validLabel", () => {
    154|   it("takes 1 to 512 UTF-8 bytes, not blank, without control character…
    155|     expect(MAX_LABEL_BYTES).toBe(512);
       |                             ^
    156|     expect(validLabel("Back again")).toBe(true);
    157|     expect(validLabel("é".repeat(256))).toBe(true);

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[1/3]⎯

 FAIL  tests/unit/files-model.test.ts > validLabel > refuses a blank, an over-long and a control-character label
TypeError: validLabel is not a function
 ❯ tests/unit/files-model.test.ts:163:12
    161|
    162|   it("refuses a blank, an over-long and a control-character label", ()…
    163|     expect(validLabel("")).toBe(false);
       |            ^
    164|     expect(validLabel("   ")).toBe(false);
    165|     expect(validLabel(`${"é".repeat(256)}a`)).toBe(false);

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[2/3]⎯

 FAIL  tests/unit/memory-history.test.ts > MemoryFileSource history reads > refuses a restore label the restorations route refuses, before the revision check
AssertionError: expected ApiError: FileSquid API /api/v1/files fai… { …(5) } to match object { code: 'invalid_request' }
(5 matching properties omitted from actual)

- Expected
+ Received

- {
-   "code": "invalid_request",
+ ApiError {
+   "code": "conflict",
  }

 ❯ tests/unit/memory-history.test.ts:71:8
     69|       await expect(
     70|         s.restore(OBJECT, "v1", ["Docs/a.txt"], { revision: "stale", l…
     71|       ).rejects.toMatchObject({ code: "invalid_request" });
       |        ^
     72|     }
     73|     expect((await s.versions()).head).toBe("v2");

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[3/3]⎯


 Test Files  2 failed (2)
      Tests  3 failed | 66 passed (69)
   Start at  01:37:33
   Duration  1.15s (transform 511ms, setup 128ms, import 562ms, tests 95ms, environment 1.05s)

JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

### npm test (apps/filesquid/ui)

```text

> @filesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @filesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 <repo>/apps/filesquid/ui

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the error state and no host when the descriptor cannot be read
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:329:46
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the denied state for an authorisation denial
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 401 (missing_credentials)
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:338:20
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 401,
  path: '/api/v1/host',
  code: 'unauthorized',
  reason: 'missing_credentials',
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the offline state with Retry when the read fails while the browser is offline
FileSquid host descriptor could not be read TypeError: Failed to fetch
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:357:29
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > reads again when Retry is pressed and recovers
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:369:29
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> follows the host > reads again when a host.change payload breaks the contract
FileSquid host.change payload broke the contract; reading the descriptor

 ✓ tests/ui/host-indicator.test.ts (40 tests) 584ms
 ✓ tests/ui/nav-tree.test.ts (30 tests) 1128ms
 ✓ tests/unit/theme.test.ts (36 tests) 112ms
 ✓ tests/ui/file-browser-writes.test.ts (47 tests) 1749ms
 ✓ tests/ui/app-shell.test.ts (24 tests) 591ms
 ✓ tests/ui/file-browser.test.ts (37 tests) 1792ms
stderr | tests/ui/versions-panel.test.ts > <fs-versions-panel> > sends the head it read as the revision, so a head moved meanwhile is refused
FileSquid could not restore a version ApiError

stderr | tests/ui/versions-panel.test.ts > <fs-versions-panel> > keeps the dialog open with the refusal's words when the restore is refused
FileSquid could not restore a version ApiError

stderr | tests/ui/versions-panel.test.ts > <fs-versions-panel> > shows loading, then the error with Try again when the history cannot be read
FileSquid could not read the versions ApiError

 ✓ tests/unit/files-source.test.ts (29 tests) 68ms
stderr | tests/ui/ocfl-findings.test.ts > <fs-ocfl-findings> > words a refused start from the label table
OCFL validation could not start ApiError: FileSquid API /api/v1/admin/ocfl/validation-runs failed with HTTP 403 (not_permitted)
    at <repo>/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:301:7
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 403,
  path: '/api/v1/admin/ocfl/validation-runs',
  code: undefined,
  reason: 'not_permitted',
  retryable: false
}

stderr | tests/ui/ocfl-findings.test.ts > <fs-ocfl-findings> > shows the unavailable state with Retry when the findings route answers 503
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
    at <repo>/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:315:29
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 503,
  path: '/api/v1/admin/ocfl/findings',
  code: undefined,
  reason: undefined,
  retryable: false
}

 ✓ tests/ui/ocfl-findings.test.ts (22 tests) 488ms
 ✓ tests/ui/library-start.test.ts (22 tests) 323ms
stderr | tests/ui/monitor.test.ts > <fs-monitor> > keeps the other sections up when the findings route is unavailable
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
    at Object.fake.client.adminOcflFindings (<repo>/apps/filesquid/ui/tests/ui/monitor.test.ts:458:22)
    at FsOcflFindings.read (<repo>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:155:41)
    at FsOcflFindings.load (<repo>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:142:23)
    at FsOcflFindings.refresh (<repo>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:130:31)
    at FsOcflFindings.connectedCallback (<repo>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:111:15)
    at FsOcflFindings.invokeTheCallbackFunction (<repo>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/generated/Function.js:19:26)
    at invokeCEReactions (<repo>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/helpers/custom-elements.js:190:31)
    at ceReactionsPostSteps (<repo>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/helpers/custom-elements.js:54:3)
    at ShadowRoot.replaceChildren (<repo>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/generated/DocumentFragment.js:204:9)
    at replaceKeepingFocus (<repo>/apps/filesquid/ui/src/a11y/focus.ts:52:8) {
  status: 503,
  path: '/api/v1/admin/ocfl/findings',
  code: undefined,
  reason: undefined,
  retryable: false
}

 ✓ tests/ui/monitor.test.ts (22 tests) 5227ms
     ✓ shows only the newest 200 rows after a long stream during a journal re-read  4630ms
 ✓ tests/unit/files-memory-source.test.ts (35 tests) 48ms
 ✓ tests/ui/library-repair.test.ts (22 tests) 452ms
 ✓ tests/unit/files-ops.test.ts (55 tests) 208ms
 ✓ tests/ui/library-detail.test.ts (20 tests) 331ms
 ✓ tests/unit/component-styles.test.ts (226 tests) 106ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 709ms
stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows the error state with Try again when the details cannot be read, and retries
FileSquid could not preview a file Error

stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows permission denied with the reason's label
FileSquid could not preview a file ApiError

 ✓ tests/ui/file-preview.test.ts (16 tests) 210ms
 ✓ tests/unit/client-io.test.ts (23 tests) 50ms
 ✓ tests/ui/versions-panel.test.ts (22 tests) 8730ms
     ✓ counts every version even when it lists only the newest MAX_VERSION_ROWS  8037ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 52ms
 ✓ tests/ui/navigation-view.test.ts (20 tests) 265ms
 ✓ tests/ui/settings.test.ts (10 tests) 262ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 48ms
stderr | tests/ui/details-panel.test.ts > <fs-details-panel> > shows the error with its reason, Nothing was changed, the code and Try again
FileSquid could not read file details ApiError

stderr | tests/ui/details-panel.test.ts > <fs-details-panel> > words a denial with its reason and an offline failure as offline
FileSquid could not read file details ApiError

stderr | tests/ui/details-panel.test.ts > <fs-details-panel> > words a denial with its reason and an offline failure as offline
FileSquid could not read file details TypeError

stderr | tests/ui/details-panel.test.ts > <fs-details-panel> > hides Clear filter and forgets the filter while loading, failed or cleared
FileSquid could not read file details ApiError

 ✓ tests/ui/details-panel.test.ts (11 tests) 320ms
 ✓ tests/ui/file-panel.test.ts (11 tests) 472ms
 ✓ tests/ui/nav-tree-live.test.ts (11 tests) 340ms
Not implemented: HTMLMediaElement's pause() method
 ✓ tests/unit/media-handle.test.ts (14 tests) 93ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 39ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 120ms
 ✓ tests/unit/library-client.test.ts (18 tests) 43ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 51ms
 ✓ tests/unit/files-model.test.ts (60 tests) 66ms
 ✓ tests/unit/navigation-client.test.ts (13 tests) 22ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 32ms
stderr | tests/ui/files-route.test.ts > bindFilesRoute > mounts the file panel in the panel slot for the opened file and removes it on close (UI-F2.3)
FileSquid could not read the history size object

 ✓ tests/ui/files-route.test.ts (10 tests) 754ms
 ✓ tests/unit/commands.test.ts (46 tests) 27ms
 ✓ tests/ui/states.test.ts (26 tests) 154ms
 ✓ tests/unit/host-descriptor.test.ts (43 tests) 34ms
 ✓ tests/unit/versions-model.test.ts (14 tests) 69ms
 ✓ tests/unit/admin-client.test.ts (10 tests) 31ms
 ✓ tests/unit/files-window.test.ts (19 tests) 34ms
 ✓ tests/unit/focus.test.ts (16 tests) 71ms
 ✓ tests/unit/memory-history.test.ts (9 tests) 20ms
 ✓ tests/unit/library-errors.test.ts (15 tests) 16ms
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 741ms
 ✓ tests/unit/distribution-tables.test.ts (7 tests) 13ms
 ✓ tests/unit/details-model.test.ts (7 tests) 31ms
 ✓ tests/unit/navigation-contract.test.ts (30 tests) 21ms
 ✓ tests/ui/library-route.test.ts (6 tests) 108ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 15ms
 ✓ tests/unit/history-source.test.ts (6 tests) 16ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 72ms
 ✓ tests/unit/client.test.ts (8 tests) 24ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 93ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 17ms
 ✓ tests/unit/library-model.test.ts (7 tests) 15ms
 ✓ tests/unit/preferences.test.ts (10 tests) 18ms
 ✓ tests/unit/view-state.test.ts (5 tests) 10ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 81ms
 ✓ tests/unit/contrast.test.ts (13 tests) 22ms
 ✓ tests/unit/host-client.test.ts (4 tests) 14ms
 ✓ tests/unit/distribution.test.ts (3 tests) 11ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 30ms
 ✓ tests/unit/contract.test.ts (2 tests) 221ms

 Test Files  64 passed (64)
      Tests  1421 passed | 1 skipped (1422)
   Start at  01:38:47
   Duration  28.47s (transform 3.80s, setup 2.68s, import 8.79s, tests 27.91s, environment 34.32s)

JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

### Playwright, production build (`vite build` plus `vite preview`, apps/filesquid/ui, temporary config with the machine's Chromium)

```text

Running 90 tests using 2 workers

  ✓   1 [chromium] › tests/e2e/preview/file-browser.spec.ts:96:1 › the browser is a defined element and lists display names in the list layout from the label table (882ms)
  ✓   3 [chromium] › tests/e2e/preview/file-browser.spec.ts:140:1 › a file row shows size, type hint, modified date and version (426ms)
  ✓   2 [chromium] › tests/e2e/preview/file-panel.spec.ts:102:5 › light theme › File info (35): header, tab, filter, count, rows and footer (1.4s)
  ✓   4 [chromium] › tests/e2e/preview/file-browser.spec.ts:152:1 › computed token values are exact in the browser in light and dark (746ms)
  ✓   5 [chromium] › tests/e2e/preview/file-panel.spec.ts:212:5 › light theme › Versions (26): Back, count, eyebrow, rows, Current, menu and note (1.4s)
  ✓   6 [chromium] › tests/e2e/preview/file-browser.spec.ts:175:1 › opening a folder updates the breadcrumb and the address, and Back returns (768ms)
  ✓   8 [chromium] › tests/e2e/preview/file-browser.spec.ts:196:1 › an image previews through a content handle that is used only as the media src (927ms)
  ✓   7 [chromium] › tests/e2e/preview/file-panel.spec.ts:337:5 › light theme › restore (27): the dialog, one call with the label, and the new current version (1.4s)
  ✓   9 [chromium] › tests/e2e/preview/file-browser.spec.ts:231:1 › a handle that fails is replaced once; a second failure shows the error and mints no third (871ms)
  ✓  10 [chromium] › tests/e2e/preview/file-panel.spec.ts:460:5 › light theme › panel states (25): loading, no match and error keep the header, tabs and filter (1.2s)
  ✓  11 [chromium] › tests/e2e/preview/file-browser.spec.ts:247:1 › a video resumes at the same position and paused state after one new handle (789ms)
  ✓  13 [chromium] › tests/e2e/preview/file-browser.spec.ts:283:1 › audio plays natively and is named by the display name (631ms)
  ✓  12 [chromium] › tests/e2e/preview/file-panel.spec.ts:537:5 › light theme › keyboard: the tab, the history button and the dialog keep focus where it belongs (980ms)
  ✓  14 [chromium] › tests/e2e/preview/file-browser.spec.ts:296:1 › text previews through one ranged read of the first 64 KiB and says it is cut (608ms)
  ✓  15 [chromium] › tests/e2e/preview/file-panel.spec.ts:102:5 › dark theme › File info (35): header, tab, filter, count, rows and footer (866ms)
  ✓  16 [chromium] › tests/e2e/preview/file-browser.spec.ts:312:1 › a format it cannot show says so and shows no viewer (574ms)
  ✓  18 [chromium] › tests/e2e/preview/file-browser.spec.ts:342:1 › the viewer steps through files, closes with Escape and returns focus to the row (686ms)
  ✓  17 [chromium] › tests/e2e/preview/file-panel.spec.ts:212:5 › dark theme › Versions (26): Back, count, eyebrow, rows, Current, menu and note (1.0s)
  ✓  19 [chromium] › tests/e2e/preview/file-browser.spec.ts:355:1 › an upload is one PUT of the client's facts that creates the file's own object (589ms)
  ✓  21 [chromium] › tests/e2e/preview/file-browser.spec.ts:404:1 › a rename posts to the renames route, never to moves, and shows the new display name (676ms)
  ✓  20 [chromium] › tests/e2e/preview/file-panel.spec.ts:337:5 › dark theme › restore (27): the dialog, one call with the label, and the new current version (1.2s)
  ✓  22 [chromium] › tests/e2e/preview/file-browser.spec.ts:433:1 › a refused rename shows the label for its reason and keeps the display name (632ms)
  ✓  23 [chromium] › tests/e2e/preview/file-panel.spec.ts:460:5 › dark theme › panel states (25): loading, no match and error keep the header, tabs and filter (947ms)
  ✓  24 [chromium] › tests/e2e/preview/file-browser.spec.ts:452:1 › writes are single calls: new folder, move, remove and Undo (997ms)
  ✓  25 [chromium] › tests/e2e/preview/file-panel.spec.ts:537:5 › dark theme › keyboard: the tab, the history button and the dialog keep focus where it belongs (753ms)
  ✓  26 [chromium] › tests/e2e/preview/file-browser.spec.ts:499:1 › a refused write shows the label for its reason and changes nothing (533ms)
  ✓  27 [chromium] › tests/e2e/preview/host-indicator.spec.ts:96:1 › the indicator is a defined custom element in the shell header, named from the label table (509ms)
  ✓  29 [chromium] › tests/e2e/preview/host-indicator.spec.ts:110:1 › the desktop host says only you can read your files, and shows the not-enrolled state (458ms)
  ✓  28 [chromium] › tests/e2e/preview/file-browser.spec.ts:514:1 › a long folder is virtualised, pages in on demand and stays keyboard reachable (991ms)
  ✓  30 [chromium] › tests/e2e/preview/host-indicator.spec.ts:124:1 › a server names its operator, who can read the files, and states the at-rest duty (460ms)
  ✓  32 [chromium] › tests/e2e/preview/host-indicator.spec.ts:137:1 › a server without an operator name uses the unnamed-operator label (412ms)
  ✓  31 [chromium] › tests/e2e/preview/file-browser.spec.ts:554:1 › the layout does not scroll sideways at 320 px, at 200 % text size, or both, in list and grid (693ms)
  ✓  33 [chromium] › tests/e2e/preview/host-indicator.spec.ts:149:1 › the browser host says files are decrypted only in this browser (439ms)
  ✓  34 [chromium] › tests/e2e/preview/file-browser.spec.ts:571:1 › motion stops under reduced motion and focus is the 2px --fs-focus-ring ring (647ms)
  ✓  35 [chromium] › tests/e2e/preview/host-indicator.spec.ts:162:1 › enrolled backup and sync are shown apart from the host, as ciphertext only (585ms)
  ✓  36 [chromium] › tests/e2e/preview/file-browser.spec.ts:598:1 › a failed listing shows the error state with Try again, and retrying loads it (715ms)
  ✓  37 [chromium] › tests/e2e/preview/host-indicator.spec.ts:187:1 › a host.change event updates the indicator without a reload or another read (515ms)
  ✓  38 [chromium] › tests/e2e/preview/library.spec.ts:47:1 › the start page offers create, open and recover in the active vocabulary (645ms)
  ✓  39 [chromium] › tests/e2e/preview/host-indicator.spec.ts:209:1 › events.resync makes the indicator read the descriptor again (520ms)
  ✓  40 [chromium] › tests/e2e/preview/library.spec.ts:80:1 › (1) create asks for a folder, passes the location ID on and shows the library, with no path (578ms)
  ✓  42 [chromium] › tests/e2e/preview/library.spec.ts:102:1 › (1) open passes the location ID to the open route (374ms)
  ✓  41 [chromium] › tests/e2e/preview/host-indicator.spec.ts:222:1 › loading shows no host until the descriptor arrives (1.3s)
  ✓  43 [chromium] › tests/e2e/preview/library.spec.ts:116:1 › (1) recover opens the copy, rebuilds from files and reports without repairing (451ms)
  ✓  45 [chromium] › tests/e2e/preview/library.spec.ts:153:1 › (1) copy asks for a destination and passes it as destination_location_id (622ms)
  ✓  44 [chromium] › tests/e2e/preview/host-indicator.spec.ts:234:1 › a failed read shows the error state and never a guessed host, then Retry recovers (756ms)
  ✓  47 [chromium] › tests/e2e/preview/host-indicator.spec.ts:256:1 › the header shows a compact badge with the short host label, not the statement (560ms)
  ✓  46 [chromium] › tests/e2e/preview/library.spec.ts:176:1 › (2) a cancelled folder choice sends no library request and says nothing changed (619ms)
  ✓  48 [chromium] › tests/e2e/preview/host-indicator.spec.ts:271:1 › Enter opens the popover with the full statement; Escape closes it and returns focus (590ms)
  ✓  49 [chromium] › tests/e2e/preview/library.spec.ts:190:1 › (3) 503 unavailable is the unavailable state with its reason and a Retry, and the choices stay (755ms)
  ✓  50 [chromium] › tests/e2e/preview/host-indicator.spec.ts:297:1 › Privacy settings opens the Settings page, whose Privacy section has the full statement (725ms)
  ✓  51 [chromium] › tests/e2e/preview/library.spec.ts:217:1 › (4) confirm sends the consent for exactly the named objects and shows each result (649ms)
  ✓  53 [chromium] › tests/e2e/preview/library.spec.ts:254:1 › (4) cancel and Escape send nothing and return focus to the opener (614ms)
  ✓  52 [chromium] › tests/e2e/preview/host-indicator.spec.ts:333:1 › computed token values are exact in light and dark (UI-S-024, mockup 40) (1.0s)
  ✓  54 [chromium] › tests/e2e/preview/library.spec.ts:277:1 › (4) a refused repair says nothing was changed and shows no result (601ms)
  ✓  55 [chromium] › tests/e2e/preview/host-indicator.spec.ts:378:1 › the badge is reachable by keyboard and has a visible focus outline (733ms)
  ✓  56 [chromium] › tests/e2e/preview/library.spec.ts:292:1 › (5) the Monitor's W010 note links to the library's repair offer (612ms)
  ✓  57 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:116:5 › light theme › the shell frame (UI-S-025): sidebar, header, footer, controls and strings (747ms)
  ✓  58 [chromium] › tests/e2e/preview/library.spec.ts:345:1 › colours come from tokens in light and dark (1.0s)
  ✓  59 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:200:5 › light theme › the navigation tree as built (55, UI-S-017): eyebrows, 30 px rows, 16 px icons (628ms)
  ✓  60 [chromium] › tests/e2e/preview/navigation.spec.ts:62:1 › the live tree fills the shell's navigation slot, worded from the label table (441ms)
  ✓  61 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:238:5 › light theme › welcome (01, UI-S-027): title, lead, cards, privacy box and footer (721ms)
  ✓  62 [chromium] › tests/e2e/preview/navigation.spec.ts:83:1 › a level is read page by page through next_cursor until it is null (540ms)
  ✓  63 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:294:5 › light theme › folder-choice unavailable (03) and the view states treatment (56) (698ms)
  ✓  64 [chromium] › tests/e2e/preview/navigation.spec.ts:121:1 › a destination opens with a breadcrumb filled from ancestors (720ms)
  ✓  66 [chromium] › tests/e2e/preview/navigation.spec.ts:142:1 › a deep link to a destination shows its breadcrumb after a reload (619ms)
  ✓  65 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:329:5 › light theme › library page (41, UI-S-027): facts rows, actions and the W010 note (808ms)
  ✓  67 [chromium] › tests/e2e/preview/navigation.spec.ts:154:1 › navigation.change reads only the named level again, without a reload (569ms)
  ✓  68 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:379:5 › light theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (883ms)
  ✓  69 [chromium] › tests/e2e/preview/navigation.spec.ts:177:1 › a 503 shows the unavailable state with Retry, never an empty tree (539ms)
  ✓  70 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:424:5 › light theme › Monitor (31) and its OCFL validation section (54) (714ms)
  ✓  71 [chromium] › tests/e2e/preview/navigation.spec.ts:194:1 › computed token values of the tree and breadcrumb are exact in light and dark (988ms)
  ✓  72 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:485:5 › light theme › host popover (40) and Settings (32): 372 px overlay, Appearance and About (655ms)
  ✓  73 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:174:1 › (1) an E023 finding shows with its code beside the degraded health reason (638ms)
  ✓  74 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:116:5 › dark theme › the shell frame (UI-S-025): sidebar, header, footer, controls and strings (930ms)
  ✓  75 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:200:1 › (2) the finding disappears after a clean revalidation's health.change, without a reload (684ms)
  ✓  76 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:200:5 › dark theme › the navigation tree as built (55, UI-S-017): eyebrows, 30 px rows, 16 px icons (605ms)
  ✓  77 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:220:1 › (3) a W010 warning is listed apart from errors and health stays ok (544ms)
  ✓  78 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:238:5 › dark theme › welcome (01, UI-S-027): title, lead, cards, privacy box and footer (759ms)
  ✓  79 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:238:1 › (4) no location is rendered when the API withholds it (612ms)
  ✓  80 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:294:5 › dark theme › folder-choice unavailable (03) and the view states treatment (56) (683ms)
  ✓  81 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:250:1 › (5) the last and next run times come from the findings route; Validate now starts a run (869ms)
  ✓  83 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:289:1 › error and warning codes take their colours from tokens in light and dark (543ms)
  ✓  82 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:329:5 › dark theme › library page (41, UI-S-027): facts rows, actions and the W010 note (835ms)
  ✓  84 [chromium] › tests/e2e/preview/production-build.spec.ts:63:1 › the built page is static, mounts the shell and defines every custom element (530ms)
  ✓  85 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:379:5 › dark theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (799ms)
  ✓  86 [chromium] › tests/e2e/preview/production-build.spec.ts:80:1 › sidebar, header and Settings words come from the active label table (UI-S-025, UI-S-033) (575ms)
  ✓  88 [chromium] › tests/e2e/preview/production-build.spec.ts:124:1 › computed token values are exact in light and dark (UI-S-024, CA-1) (602ms)
  ✓  87 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:424:5 › dark theme › Monitor (31) and its OCFL validation section (54) (855ms)
  ✓  89 [chromium] › tests/e2e/preview/production-build.spec.ts:146:1 › a refused health call shows the unavailable alert in Settings, About, not a console error (498ms)
[WebServer] 1:40:50 AM [vite] http proxy error: /api/v1/navigation?node=l1
[WebServer] Error: connect ECONNREFUSED 127.0.0.1:8083
[WebServer]     at TCPConnectWrap.afterConnect [as oncomplete] (node:net:1637:16)
  ✓  90 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:485:5 › dark theme › host popover (40) and Settings (32): 372 px overlay, Appearance and About (629ms)

  90 passed (44.3s)
```

### Pre-commit hook (ordinary run, `bash scripts/pre-commit.sh`)

```text
changed_scope: npm:apps/filesquid/ui (changed crates and npm suites: apps/filesquid/ui)
========================================
Running Pre-Commit Hooks... (test scope: npm:apps/filesquid/ui)
========================================
✅ Branch Validation passed.
Starting Port Isolation Validation...
🔍 Validating ./apps/filesquid/ui (Vite)...
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
🔍 Validating ./apps/imagesquid (Vite)...
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
🔍 Validating filesquid-server (default bind)...
✅ filesquid-server uniquely bound to 8083

🚀 VALIDATION PASSED: All modules maintain strict port isolation.
Starting Document Sync Validation...
Starting PRD/SRS Traceability Validation...
✅ Traceability Validation passed: 96 PRD requirements, 84 resolved SRS references.
Starting Portable Documentation Validation...
Starting Workspace Boundary Validation...
✅ Workspace Boundary validation passed.
Starting logging rule validation...
✅ Logging rule validation passed (no relevant staged files).
Starting Format Validation...
⏭️  Skipped: Rust formatting in ./apps/filesquid (no Rust file changed; test scope: npm:apps/filesquid/ui)
🔍 Validating Prettier formatting in ./apps/filesquid/ui...
✅ ./apps/filesquid/ui is properly formatted.
🔍 Validating Prettier formatting in ./apps/imagesquid...
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
Starting Documentation Validation...
🔍 ADR structure (ISO 42010)...
TAP version 13
# Subtest: stack ADRs exist with ISO 42010 sections and accepted vocabulary
ok 1 - stack ADRs exist with ISO 42010 sections and accepted vocabulary
  ---
  duration_ms: 10.07168
  type: 'test'
  ...
# Subtest: ADR markdown hrefs reject absolute destinations
ok 2 - ADR markdown hrefs reject absolute destinations
  ---
  duration_ms: 0.24372
  type: 'test'
  ...
# Subtest: FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
ok 3 - FileSquid ADRs 029-032 are indexed with the FileSquid header and expected status
  ---
  duration_ms: 0.722411
  type: 'test'
  ...
1..3
# tests 3
# suites 0
# pass 3
# fail 0
# cancelled 0
# skipped 0
# todo 0
# duration_ms 75.877642
✅ ADR structure (ISO 42010) passed.
🔍 PRD allocation...
FileSquid: 263 vision IDs, 263 allocated, 133 allocation rows
ImageSquid: 29 vision IDs, 29 allocated, 19 allocation rows
OK: every vision ID is allocated
✅ PRD allocation passed.
🔍 Plan traceability...
OK: every requirement is accounted for in a plan task
✅ Plan traceability passed.
🔍 Relative links and anchors...
checked 294 Markdown files: 0 broken link(s)
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
checked 174 documents: 0 link(s) cross the FileSquid/ImageSquid boundary
✅ FileSquid and ImageSquid document boundary passed.
🔍 MVP queue and merge driver tests...
......................................................
----------------------------------------------------------------------
Ran 54 tests in 5.456s

OK
✅ MVP queue and merge driver tests passed.
🔍 MVP progress bar...
✅ MVP progress bar passed.
🔍 Task cleanup...
✅ Task cleanup passed.
✅ Documentation validation passed.
Starting Change Scope Validation...
✅ Change scope validation passed (188 assertions).
Starting Lint and Test Validation...
ℹ️  Test scope: npm:apps/filesquid/ui
⏭️  Skipped: ESLint, TypeScript, Vitest and Playwright e2e in apps/imagesquid (not run by the hook during the MVP (owner decision 2026-10-23); @filesquid/ui's distribution-table test checks its label table and token file)
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
🔍 ESLint (apps/filesquid/ui)...

> @filesquid/ui@0.1.0 lint
> eslint .

✅ ESLint passed (5s).
🔍 TypeScript (apps/filesquid/ui)...

> @filesquid/ui@0.1.0 check
> tsc --noEmit

✅ TypeScript passed (6s).
🔍 Vitest (apps/filesquid/ui)...

> @filesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @filesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 <repo>/apps/filesquid/ui

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the error state and no host when the descriptor cannot be read
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:329:46
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the denied state for an authorisation denial
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 401 (missing_credentials)
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:338:20
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 401,
  path: '/api/v1/host',
  code: 'unauthorized',
  reason: 'missing_credentials',
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the offline state with Retry when the read fails while the browser is offline
FileSquid host descriptor could not be read TypeError: Failed to fetch
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:357:29
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > reads again when Retry is pressed and recovers
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:369:29
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> follows the host > reads again when a host.change payload breaks the contract
FileSquid host.change payload broke the contract; reading the descriptor

 ✓ tests/ui/host-indicator.test.ts (40 tests) 484ms
 ✓ tests/unit/theme.test.ts (36 tests) 76ms
 ✓ tests/ui/nav-tree.test.ts (30 tests) 1138ms
 ✓ tests/ui/file-browser-writes.test.ts (47 tests) 1764ms
 ✓ tests/ui/app-shell.test.ts (24 tests) 682ms
 ✓ tests/ui/file-browser.test.ts (37 tests) 1652ms
stderr | tests/ui/versions-panel.test.ts > <fs-versions-panel> > sends the head it read as the revision, so a head moved meanwhile is refused
FileSquid could not restore a version ApiError

stderr | tests/ui/versions-panel.test.ts > <fs-versions-panel> > keeps the dialog open with the refusal's words when the restore is refused
FileSquid could not restore a version ApiError

stderr | tests/ui/versions-panel.test.ts > <fs-versions-panel> > shows loading, then the error with Try again when the history cannot be read
FileSquid could not read the versions ApiError

 ✓ tests/unit/files-source.test.ts (29 tests) 90ms
stderr | tests/ui/ocfl-findings.test.ts > <fs-ocfl-findings> > words a refused start from the label table
OCFL validation could not start ApiError: FileSquid API /api/v1/admin/ocfl/validation-runs failed with HTTP 403 (not_permitted)
    at <repo>/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:301:7
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 403,
  path: '/api/v1/admin/ocfl/validation-runs',
  code: undefined,
  reason: 'not_permitted',
  retryable: false
}

stderr | tests/ui/ocfl-findings.test.ts > <fs-ocfl-findings> > shows the unavailable state with Retry when the findings route answers 503
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
    at <repo>/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:315:29
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at <repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 503,
  path: '/api/v1/admin/ocfl/findings',
  code: undefined,
  reason: undefined,
  retryable: false
}

 ✓ tests/ui/ocfl-findings.test.ts (22 tests) 357ms
 ✓ tests/ui/library-start.test.ts (22 tests) 417ms
stderr | tests/ui/monitor.test.ts > <fs-monitor> > keeps the other sections up when the findings route is unavailable
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
    at Object.fake.client.adminOcflFindings (<repo>/apps/filesquid/ui/tests/ui/monitor.test.ts:458:22)
    at FsOcflFindings.read (<repo>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:155:41)
    at FsOcflFindings.load (<repo>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:142:23)
    at FsOcflFindings.refresh (<repo>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:130:31)
    at FsOcflFindings.connectedCallback (<repo>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:111:15)
    at FsOcflFindings.invokeTheCallbackFunction (<repo>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/generated/Function.js:19:26)
    at invokeCEReactions (<repo>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/helpers/custom-elements.js:190:31)
    at ceReactionsPostSteps (<repo>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/helpers/custom-elements.js:54:3)
    at ShadowRoot.replaceChildren (<repo>/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/generated/DocumentFragment.js:204:9)
    at replaceKeepingFocus (<repo>/apps/filesquid/ui/src/a11y/focus.ts:52:8) {
  status: 503,
  path: '/api/v1/admin/ocfl/findings',
  code: undefined,
  reason: undefined,
  retryable: false
}

 ✓ tests/ui/monitor.test.ts (22 tests) 5060ms
     ✓ shows only the newest 200 rows after a long stream during a journal re-read  4507ms
 ✓ tests/unit/files-memory-source.test.ts (35 tests) 60ms
 ✓ tests/ui/library-repair.test.ts (22 tests) 404ms
 ✓ tests/unit/files-ops.test.ts (55 tests) 203ms
 ✓ tests/ui/library-detail.test.ts (20 tests) 373ms
 ✓ tests/unit/component-styles.test.ts (226 tests) 96ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 679ms
 ✓ tests/unit/client-io.test.ts (23 tests) 53ms
stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows the error state with Try again when the details cannot be read, and retries
FileSquid could not preview a file Error

stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows permission denied with the reason's label
FileSquid could not preview a file ApiError

 ✓ tests/ui/file-preview.test.ts (16 tests) 225ms
 ✓ tests/ui/versions-panel.test.ts (22 tests) 8576ms
     ✓ counts every version even when it lists only the newest MAX_VERSION_ROWS  7678ms
 ✓ tests/ui/navigation-view.test.ts (20 tests) 229ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 52ms
 ✓ tests/ui/settings.test.ts (10 tests) 201ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 29ms
stderr | tests/ui/details-panel.test.ts > <fs-details-panel> > shows the error with its reason, Nothing was changed, the code and Try again
FileSquid could not read file details ApiError

stderr | tests/ui/details-panel.test.ts > <fs-details-panel> > words a denial with its reason and an offline failure as offline
FileSquid could not read file details ApiError

stderr | tests/ui/details-panel.test.ts > <fs-details-panel> > words a denial with its reason and an offline failure as offline
FileSquid could not read file details TypeError

stderr | tests/ui/details-panel.test.ts > <fs-details-panel> > hides Clear filter and forgets the filter while loading, failed or cleared
FileSquid could not read file details ApiError

 ✓ tests/ui/details-panel.test.ts (11 tests) 292ms
 ✓ tests/ui/nav-tree-live.test.ts (11 tests) 350ms
 ✓ tests/ui/file-panel.test.ts (11 tests) 456ms
Not implemented: HTMLMediaElement's pause() method
 ✓ tests/unit/media-handle.test.ts (14 tests) 81ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 33ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 134ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 43ms
 ✓ tests/unit/library-client.test.ts (18 tests) 42ms
 ✓ tests/unit/navigation-client.test.ts (13 tests) 24ms
 ✓ tests/unit/files-model.test.ts (60 tests) 56ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 28ms
stderr | tests/ui/files-route.test.ts > bindFilesRoute > mounts the file panel in the panel slot for the opened file and removes it on close (UI-F2.3)
FileSquid could not read the history size object

 ✓ tests/unit/commands.test.ts (46 tests) 52ms
 ✓ tests/ui/files-route.test.ts (10 tests) 625ms
 ✓ tests/unit/host-descriptor.test.ts (43 tests) 25ms
 ✓ tests/ui/states.test.ts (26 tests) 144ms
 ✓ tests/unit/versions-model.test.ts (14 tests) 50ms
 ✓ tests/unit/files-window.test.ts (19 tests) 17ms
 ✓ tests/unit/admin-client.test.ts (10 tests) 25ms
 ✓ tests/unit/focus.test.ts (16 tests) 92ms
 ✓ tests/unit/library-errors.test.ts (15 tests) 15ms
 ✓ tests/unit/memory-history.test.ts (9 tests) 21ms
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 634ms
 ✓ tests/unit/distribution-tables.test.ts (7 tests) 15ms
 ✓ tests/unit/details-model.test.ts (7 tests) 34ms
 ✓ tests/unit/navigation-contract.test.ts (30 tests) 47ms
 ✓ tests/ui/library-route.test.ts (6 tests) 98ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 22ms
 ✓ tests/unit/history-source.test.ts (6 tests) 18ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 91ms
 ✓ tests/unit/client.test.ts (8 tests) 17ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 95ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 19ms
 ✓ tests/unit/library-model.test.ts (7 tests) 15ms
 ✓ tests/unit/preferences.test.ts (10 tests) 21ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 71ms
 ✓ tests/unit/view-state.test.ts (5 tests) 9ms
 ✓ tests/unit/contrast.test.ts (13 tests) 21ms
 ✓ tests/unit/host-client.test.ts (4 tests) 13ms
 ✓ tests/unit/distribution.test.ts (3 tests) 7ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 26ms
 ✓ tests/unit/contract.test.ts (2 tests) 203ms

 Test Files  64 passed (64)
      Tests  1421 passed | 1 skipped (1422)
   Start at  01:42:25
   Duration  27.09s (transform 3.59s, setup 2.40s, import 8.55s, tests 26.98s, environment 31.96s)

JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
✅ Vitest passed (29s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: npm:apps/filesquid/ui).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui (59s)
========================================
```
