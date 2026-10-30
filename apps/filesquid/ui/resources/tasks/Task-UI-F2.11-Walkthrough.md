# Task UI-F2.11 Walkthrough: Versions panel labels, download and history size

Branch `feat/ui-f2.11-versions-panel-labels`. Plan row: [Task UI-F2.11](../plan.md). Mockups (active suite v3): [26-panel-versions](../../../../../resources/ui-mockups/v3/26-panel-versions.svg), [27-version-restore](../../../../../resources/ui-mockups/v3/27-version-restore.svg). Built on the Versions panel of [Task UI-F2.3](./Task-UI-F2.3-Walkthrough.md). It belongs to global Task FS-2.5, whose other rows stay open, so only the module and checklist rows of UI-F2.11 are ticked.

## SRS mapping

| SRS ID and title | What was built | Proof |
|---|---|---|
| [UI-S-009](../srs.md) The Versions view, opened from the file panel's header, MUST list versions newest first with label, relative time and size, mark the current version, and offer restore, download and edit label per version; no delete, expiry or thinning control MAY appear | Every row has a menu (`versionActions`): Restore as new version, Download this version, Edit label; the current version's menu holds Download and Edit label (it is the content shown), a removal's only Edit label; no action deletes, expires or thins | `tests/unit/versions-model.test.ts`, `tests/ui/versions-panel.test.ts`, `tests/ui/version-label.test.ts`, `tests/e2e/preview/versions-panel.spec.ts` |
| [UI-S-031](../srs.md) Preview and file panel composition (Versions, mockups 26 and 27; File info, mockup 35) | The three-item menu at the mockup's box (232 px wide, 34 px items, 8 px and 5 px radii); Edit label dialog built like the restore dialog; Download this version; the History size row in File info; footer note unchanged ("Deleted files expire."; the view does not say "Nothing expires", ADR 041 Decision 11) | `tests/e2e/preview/file-panel.spec.ts`, `tests/e2e/preview/versions-panel.spec.ts` |
| [UI-S-024](../srs.md) Visual constants of the active mockup suite | Every colour through its token in light and dark: overlay dialog and menu, outline-strong borders, card and primary buttons, heading 22 px 700 Georgia dialog title, 13 px meta and help text, 14 px 600 buttons at 36 and 38 px, 36 px input with the 2 px `--fs-focus-ring` border, error text and the error-outlined alert (1.5 px), 16 px 1.6 px line icons in `--fs-icon`, the radii of buttons 7, inputs 6, dialogs 10, menus 8, items 5; no literal colour or size in the stylesheets | `tests/unit/component-styles.test.ts`, `tests/e2e/preview/versions-panel.spec.ts`, `tests/e2e/preview/file-panel.spec.ts` |
| [UI-S-012](../srs.md) UIs reach FileSquid only through `FileSquidClient` | `FileSource.setVersionLabel` (a command over `PUT /files/labels`) and `ClientDownloadSource` (`GET /files/details`, `GET /files/download`, ranged `GET /files/content`); the panel never fetches; restore stays the command of API-F2.13 | `tests/unit/version-label-source.test.ts`, `tests/unit/download-source.test.ts` |
| [UI-S-005](../srs.md) A distribution's label table MUST define every key of the default table | 13 new keys in `REQUIRED_LABEL_KEYS`, the default table and ImageSquid's table (owner decision 7) | `tests/unit/theme.test.ts`, `tests/unit/distribution-tables.test.ts` |
| [UI-S-013](../srs.md) UI tests against production artifacts with strict DOM assertions | Playwright against `vite build` plus `vite preview`, 1440 × 900, light and dark: custom elements defined, labels from the active table, computed styles, boxes within 2 px, no console errors | Playwright output below |
| [UI-S-016](../srs.md) Accessibility contracts | Menu keys as UI-F2.3; a modal Edit label dialog that keeps Tab inside, returns focus to the menu button on Escape and, after a refused save, back to the field; a polite status for saved, removed and downloading; a visible `role="alert"` for a failed download; `aria-busy` on the menu button while it runs | `tests/ui/version-label.test.ts`, `tests/e2e/preview/versions-panel.spec.ts` |

PRD: [F-065](../../../../../resources/prd.md#f-065-file-version-history-and-retention) File version history and retention. SRS: [SRS-F-065](../../../../../resources/srs.md#srs-f-065-file-version-history-and-retention) File version history and retention. Gherkin: [file_version_history_F-065.feature](../../tests/features/file_version_history_F-065.feature) (scenarios only; step definitions arrive with Task FS-2.5).

## What changed

- **Row menu** (`src/components/fs-versions-panel.ts`, `src/files/versions.ts`): `versionActions(row)` decides the items; the panel builds them with icons (`history`, new `download`, `edit`). Choosing one closes the menu and runs it.
- **Edit label** (`src/components/version-label-dialog.ts`, `fs-versions-panel.ts`): a modal dialog sharing the restore dialog's head, footer and styles (`dialogHead`, `dialogActions`, `DialogParts`, now exported from `version-restore-dialog.ts`; the restore button part is `submit`). `labelChange` decides: invalid (over 512 bytes, control character), unchanged (sends nothing, as the route refuses it 409), or set (blank clears with `null`). One `PUT /files/labels` command with the head as expected revision; on success `fs-files-change` (`operation: "label"`), a polite announcement, and the list reloads.
- **`FileSource.setVersionLabel`** (`src/files/source.ts`): the command over `PUT`; `MemoryFileSource` answers like the route (400 invalid, 409 stale head, 409 unchanged, one new version, the label set on the named version).
- **Download this version** (`src/files/download-source.ts`, `src/components/save-file.ts`): `ClientDownloadSource.readVersionFile` reads the size from `GET /files/details`, then one `GET /files/download` up to the desktop scheme's 16 MiB answer cap (the API journals it as `file.download`), or ranged `GET /files/content` reads of at most 8 MiB, at most 4,096 of them, above it; a version beyond the bound is refused 413 before any read. `saveFileToDevice` clicks a temporary link and revokes its object URL on every path. The file keeps its display name.
- **History size** (`src/files/details.ts`, `fs-details-panel.ts`): a "History size" row after Size, "{size} · {n} versions", from `GET /files/history-size`; a failed read leaves the row out and is logged, as the Versions eyebrow leaves the size out.
- **Wiring**: `file-panel-route.ts` (`panelSource` now takes a download source), `routes.ts` (`download` option, the panel mounts when both `history` and `download` are given), `main.ts` (`ClientDownloadSource`), `index.ts` exports.
- **Focus fix found by the Playwright run**: disabling the focused Save or Restore button while a write runs dropped focus to the page, so after a refused write Escape no longer reached the dialog. `setBusy(false)` now returns focus to the field, for the restore dialog too (unit tests focus the button first, so they fail without the fix).
- **Labels**: `versionsDownloadAction`, `versionsEditLabelAction`, `versionsEditLabelTitle`, `versionsEditLabelField`, `versionsEditLabelHelp`, `versionsEditLabelSave`, `versionsLabelSaved`, `versionsLabelCleared`, `versionsDownloading`, `versionsDownloadFailed`, `versionsDownloadTooLarge`, `filePanelHistorySize`, `filePanelHistorySizeValue`, in the default table, `REQUIRED_LABEL_KEYS` and ImageSquid's table.
- **Docs**: UI-S-031 (menu, download, edit label, history size row), the plan row's Status, the F-065 Gherkin, the checklist and the MVP progress block.

## Decisions and open points for the owner

- **Storage used in the sidebar (not this row).** The plan row asked for the history size "in storage used". The shell's block is wired by a UI task that reads the library facts route (`GET /libraries/{library}/facts`, API-S-038, in contract 0.30.0 since PR #139 merged) together with `GET /files/library-history-size`; the owner assigns it (UI-1.3, or UI-F2.18's block). Nothing is missing in the API and nothing was stubbed here: the block shows no figure it has not read, as UI-F2.18 left it. The plan row's Description, its Status, the F-065 Gherkin scenario and UI-S-031 now say so.
- **Current version has a menu button (deviation from mockup 26).** Mockup 26 draws no menu button on the current row, but UI-S-009, SRS-F-065 and PRD F-065 ask for download and edit label per version, and the current version is the one whose label ("Issued for tender") most needs editing. It offers Download and Edit label but not Restore. The UI-S-031 text now says so; the owner may prefer to redraw mockup 26.
- **Strings proposed here.** Owner answer 13 approved the strings the plan rows and UI-S-025 to UI-S-034 give. "Download this version" and "Edit label" are in mockup 26. The Edit label dialog, the announcements, the download alert and the History size row are worded here (listed above) and need the owner's nod.
- **History size row against mockup 35.** Mockup 35 counts 10 properties and draws no history row; the plan row asks for the size "in details", so the row follows Size and the count eyebrow reads one higher.
- **Large files are held as a Blob.** A download above the desktop cap is assembled from ranged reads into one Blob before saving (bounded at 32 GiB of reads, 4,096 calls). Streaming to disk needs the File System Access API, which is not available on every target; the bound and the 413 message are the safe path until the owner decides.
- **Download journal outcomes.** The download route journals three things: a download that starts is `file.download` (journal-only, no PROV-O record); a refusal before it starts, and an error that ends the stream early after it started, are each journaled once as `files.download` (`DOWNLOAD_OPERATION`, API-S-017). Ranged content reads are not journaled at all, so a version above 16 MiB downloaded on the desktop leaves none of these entries. If the owner wants one, the platform engineer needs a journaled route that works above the desktop cap.
- **Restore and Edit label sit behind the same expected revision**, so a label edit moves the object's head by one version (the labels record) and the next restore expects the new head; the list reloads after each.

No API route, field or event was assumed or stubbed; every route used is in `apps/filesquid/crates/api/openapi.json` on `main` (0.29.0 after the merge): `GET /files/history`, `/files/versions`, `/files/history-size`, `/files/details`, `/files/download`, `/files/content`, `POST /files/restorations`, `PUT /files/labels`.

## Review fixes (Cubic round 1: 1 P1, 16 P2, 9 P3)

All 26 findings were valid and fixed (see each thread's reply for its commit).

- **P1, ranged download integrity:** each ranged answer must be a 206 whose `Content-Range` is the range asked for and whose body has that length, else a `TypeError` (nothing appended, nothing saved). Tests: a whole 200, a 206 without or with another or a shorter `Content-Range`, and a short body; the assembled bytes of a large download are compared with the source; the abort test checks the signal reaches the read. The production build now downloads a version above 16 MiB through three ranged reads and saves the whole file (new Playwright test).
- **Scope of Storage used (P2, 2):** reconciled the plan Description, Status, the Gherkin scenario and the owner-decision text with the sidebar block being a later UI task that wires the library facts route (contract 0.30.0), without stubbing anything.
- **Files route types (P2):** the history and download sources are one `filePanel` option, so a panel with only one of them cannot be typed; README example updated.
- **Edit label (P2, P3):** an untouched stored label with surrounding spaces sends nothing; the help line is the field's `aria-describedby`; focus returns to the row's menu button after Download; an aborted download no longer clears the busy mark of the one that replaced it; the details panel does not warn about a cancelled history-size read.
- **Fixtures (P2, P3):** the unit fake lists the label commit among the object's versions; the Playwright fake replays a repeated operation ID, refuses the same ID with another input, and serves ranged content; `MemoryFileSource` enforces the 1,000 labelled versions and its header names the new roles; the shared `node` helper is imported, not copied.
- **Docs (P2, P3):** UI-S-031 states the 16 MiB, 8 MiB and 4,096-read bounds and the `Content-Range` check; the walkthrough lists the three download journal outcomes (`file.download`, and `files.download` for a refusal or an error after the stream started) and the corrected test counts; the README names `GET files/history-size` and the `label` operation of `fs-files-change`.
- Merge: `origin/main` was merged by its own merge commit first (PR #139 library facts, contract 0.30.0; no conflict).

## How it was tested

1. Red run (first pass, then the review fixes, output below): `tests/unit/version-label-source.test.ts`, `tests/unit/download-source.test.ts` and the History size cases of `tests/unit/details-model.test.ts` failed before the sources existed; then the new `tests/ui/version-label.test.ts` and the changed menu cases of `tests/ui/versions-panel.test.ts` failed before the panel changed; the two focus assertions failed before `setBusy` returned focus (output below).
2. `npx prettier --write .`, `npx eslint . --fix` and `npx tsc --noEmit` in `apps/filesquid/ui` are clean; `apps/imagesquid` is formatted with the same Prettier.
3. `npm test` in `apps/filesquid/ui`: 70 files, 1553 tests pass, 1 skipped (the distribution-tables test reads ImageSquid's table and token file).
4. Playwright, production build (`vite build` plus `vite preview`) at 1440 × 900 in light and dark: the new `versions-panel.spec.ts` (8 tests, 16 runs in light and dark) and the whole preview suite (116 runs). The installed Playwright expects a newer Chromium than this machine has, so the suite ran through a temporary config that points `executablePath` at the machine's preinstalled Chromium; the config was deleted before committing. The server-backed suite needs a built `filesquid-server`, which this npm-only change does not build.
5. `bash scripts/pre-commit.sh` on the head commit (summary line below).

## Mocked Environment Variables

None. The Playwright fakes answer routes from the mockups' data; no credential or environment value was invented.

## Raw test output

### Red runs

```
$ npx vitest run tests/unit/version-label-source.test.ts tests/unit/download-source.test.ts tests/unit/details-model.test.ts
TypeError: s.setVersionLabel is not a function
 ❯ tests/unit/version-label-source.test.ts:123:9
 Test Files  3 failed (3)
      Tests  10 failed | 8 passed (18)

$ npx vitest run tests/ui/version-label.test.ts tests/ui/versions-panel.test.ts
     × reads the file of that version and saves it under the display name
     × offers it on the current version too
     × announces the download while it runs and marks the menu button busy
     × shows a visible alert when the download fails and nothing is saved
     × says a version too large to download here is too large
     × opens a dialog for the version with its label in the field and one filled primary button
     × opens with an empty field for a version without a label
     × saves the trimmed label with one call carrying the head, announces it and dispatches the change
     × clears the label when the field is emptied
     × closes without a call when the label is unchanged, as the route would refuse it
     × refuses a label with a control character or over 512 bytes before calling the API
     × keeps the dialog open with the refusal's words when the head moved meanwhile
     × disables the buttons while the label is saved
     × closes with Escape or Cancel without a call and returns focus to the menu button
     × keeps Tab inside the dialog
     × restores afterwards against the new head a label edit published
     × marks the current version with a chip; every row has a menu button, the current one included
     × opens the row menu with Restore as new version, Download this version and Edit label, and no delete, expiry or thinning

$ npx vitest run tests/ui/version-label.test.ts tests/ui/versions-panel.test.ts   (focus assertions, before setBusy returned focus)
     × keeps the dialog open with the refusal's words when the head moved meanwhile 39ms
     × keeps the dialog open with the refusal's words when the restore is refused 32ms
      Tests  2 failed | 39 passed (41)

$ npx vitest run tests/unit tests/ui   (Cubic round 1 fixes, before the code changed)
     × returns focus to the row's menu button after choosing Download
     × keeps the menu button busy for the download that replaced an aborted one
     × names the help line as the description of the field
     × sends nothing for a stored label with surrounding spaces the field still holds
     × does not warn about the history size when switching file cancels its read
     × leaves a stored label with surrounding spaces alone when the field is untouched
     × refuses a label past the 1,000 labelled versions an object may have, as conflict
     × refuses a whole 200 answer that ignored the Range instead of saving a corrupted file
     × refuses a 206 without a Content-Range instead of saving a corrupted file
     × refuses a 206 for another range instead of saving a corrupted file
     × refuses a 206 for a shorter range instead of saving a corrupted file
     × refuses a 206 whose body is not the length its Content-Range names
 Test Files  5 failed | 65 passed (70)
      Tests  12 failed | 1541 passed | 1 skipped (1554)
```

### Format, lint and typecheck (`apps/filesquid/ui`)

```
$ npx prettier --check .
Checking formatting...
All matched files use Prettier code style!
$ npx eslint .
eslint exit: 0
$ npx tsc --noEmit
tsc done
```

### `npm test` (Vitest)

```

> @filesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @filesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 <repo>/apps/filesquid/ui

stderr | tests/ui/metadata-panel.test.ts > <fs-metadata-panel> > shows placeholder rows while loading and the error state with Try again
FileSquid could not read file statements ApiError

stderr | tests/ui/metadata-panel.test.ts > <fs-metadata-panel> > shows a file that is not in its own object as an error without a code
FileSquid could not read file statements TypeError

 ✓ tests/ui/metadata-panel.test.ts (24 tests) 966ms
 ✓ tests/ui/nav-tree.test.ts (30 tests) 1181ms
 ✓ tests/unit/theme.test.ts (36 tests) 54ms
 ✓ tests/ui/file-browser-writes.test.ts (47 tests) 1706ms
stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the error state and no host when the descriptor cannot be read
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:329:46
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the denied state for an authorisation denial
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 401 (missing_credentials)
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:338:20
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 401,
  path: '/api/v1/host',
  code: 'unauthorized',
  reason: 'missing_credentials',
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the offline state with Retry when the read fails while the browser is offline
FileSquid host descriptor could not be read TypeError: Failed to fetch
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:357:29
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > reads again when Retry is pressed and recovers
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:369:29
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> follows the host > reads again when a host.change payload breaks the contract
FileSquid host.change payload broke the contract; reading the descriptor

 ✓ tests/ui/host-indicator.test.ts (40 tests) 425ms
 ✓ tests/ui/app-shell.test.ts (24 tests) 672ms
 ✓ tests/ui/file-browser.test.ts (37 tests) 1563ms
stderr | tests/ui/versions-panel.test.ts > <fs-versions-panel> > sends the head it read as the revision, so a head moved meanwhile is refused
FileSquid could not restore a version ApiError

stderr | tests/ui/versions-panel.test.ts > <fs-versions-panel> > keeps the dialog open with the refusal's words when the restore is refused
FileSquid could not restore a version ApiError

stderr | tests/ui/versions-panel.test.ts > <fs-versions-panel> > shows loading, then the error with Try again when the history cannot be read
FileSquid could not read the versions ApiError

stderr | tests/ui/version-label.test.ts > Edit label > keeps the dialog open with the refusal's words when the head moved meanwhile
FileSquid could not set a version label ApiError

 ✓ tests/ui/version-label.test.ts (23 tests) 753ms
 ✓ tests/unit/files-source.test.ts (29 tests) 67ms
stderr | tests/ui/ocfl-findings.test.ts > <fs-ocfl-findings> > words a refused start from the label table
OCFL validation could not start ApiError: FileSquid API /api/v1/admin/ocfl/validation-runs failed with HTTP 403 (not_permitted)
    at <repo>/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:301:7
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 403,
  path: '/api/v1/admin/ocfl/validation-runs',
  code: undefined,
  reason: 'not_permitted',
  retryable: false
}

stderr | tests/ui/ocfl-findings.test.ts > <fs-ocfl-findings> > shows the unavailable state with Retry when the findings route answers 503
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
    at <repo>/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:315:29
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file://<repo>/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 503,
  path: '/api/v1/admin/ocfl/findings',
  code: undefined,
  reason: undefined,
  retryable: false
}

 ✓ tests/ui/ocfl-findings.test.ts (22 tests) 378ms
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

 ✓ tests/ui/monitor.test.ts (22 tests) 4842ms
     ✓ shows only the newest 200 rows after a long stream during a journal re-read  4316ms
 ✓ tests/unit/files-memory-source.test.ts (35 tests) 59ms
 ✓ tests/ui/library-start.test.ts (22 tests) 263ms
 ✓ tests/unit/files-ops.test.ts (55 tests) 125ms
 ✓ tests/ui/library-repair.test.ts (22 tests) 412ms
 ✓ tests/unit/component-styles.test.ts (251 tests) 89ms
 ✓ tests/ui/library-detail.test.ts (20 tests) 311ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 600ms
stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows the error state with Try again when the details cannot be read, and retries
FileSquid could not preview a file Error

stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows permission denied with the reason's label
FileSquid could not preview a file ApiError

 ✓ tests/ui/file-preview.test.ts (16 tests) 225ms
 ✓ tests/ui/versions-panel.test.ts (22 tests) 8432ms
     ✓ counts every version even when it lists only the newest MAX_VERSION_ROWS  7661ms
 ✓ tests/unit/client-io.test.ts (23 tests) 41ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 39ms
 ✓ tests/ui/navigation-view.test.ts (20 tests) 245ms
stderr | tests/ui/details-panel.test.ts > <fs-details-panel> > shows the error with its reason, Nothing was changed, the code and Try again
FileSquid could not read file details ApiError

stderr | tests/ui/details-panel.test.ts > <fs-details-panel> > words a denial with its reason and an offline failure as offline
FileSquid could not read file details ApiError

stderr | tests/ui/details-panel.test.ts > <fs-details-panel> > words a denial with its reason and an offline failure as offline
FileSquid could not read file details TypeError

stderr | tests/ui/details-panel.test.ts > <fs-details-panel> > hides Clear filter and forgets the filter while loading, failed or cleared
FileSquid could not read file details ApiError

 ✓ tests/ui/details-panel.test.ts (14 tests) 379ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 39ms
 ✓ tests/ui/settings.test.ts (10 tests) 247ms
 ✓ tests/ui/file-panel.test.ts (11 tests) 567ms
 ✓ tests/ui/nav-tree-live.test.ts (11 tests) 515ms
stderr | tests/ui/files-route.test.ts > bindFilesRoute > mounts the file panel in the panel slot for the opened file and removes it on close (UI-F2.3)
FileSquid could not read the history size object

 ✓ tests/ui/files-route.test.ts (11 tests) 817ms
     ✓ shows the browser at the folder the hash names, now and after a hash change  385ms
Not implemented: HTMLMediaElement's pause() method
 ✓ tests/unit/media-handle.test.ts (14 tests) 94ms
 ✓ tests/unit/metadata-model.test.ts (15 tests) 42ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 21ms
 ✓ tests/unit/versions-model.test.ts (24 tests) 62ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 97ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 29ms
 ✓ tests/unit/library-client.test.ts (18 tests) 45ms
 ✓ tests/unit/files-model.test.ts (60 tests) 60ms
 ✓ tests/unit/navigation-client.test.ts (13 tests) 35ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 29ms
 ✓ tests/unit/commands.test.ts (46 tests) 25ms
 ✓ tests/unit/download-source.test.ts (12 tests) 951ms
     ✓ reads a version above the desktop cap with ranged reads of at most 8 MiB  705ms
 ✓ tests/ui/states.test.ts (26 tests) 136ms
 ✓ tests/unit/host-descriptor.test.ts (43 tests) 33ms
 ✓ tests/unit/admin-client.test.ts (10 tests) 21ms
 ✓ tests/unit/version-label-source.test.ts (10 tests) 44ms
 ✓ tests/unit/files-window.test.ts (19 tests) 17ms
 ✓ tests/unit/focus.test.ts (16 tests) 61ms
 ✓ tests/unit/details-model.test.ts (9 tests) 32ms
 ✓ tests/unit/metadata-source.test.ts (7 tests) 13ms
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 655ms
 ✓ tests/unit/memory-history.test.ts (9 tests) 16ms
 ✓ tests/unit/library-errors.test.ts (15 tests) 19ms
 ✓ tests/unit/distribution-tables.test.ts (7 tests) 14ms
 ✓ tests/unit/navigation-contract.test.ts (30 tests) 27ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 14ms
 ✓ tests/ui/library-route.test.ts (6 tests) 79ms
 ✓ tests/unit/history-source.test.ts (6 tests) 19ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 70ms
 ✓ tests/unit/client.test.ts (8 tests) 20ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 72ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 10ms
 ✓ tests/unit/library-model.test.ts (7 tests) 14ms
 ✓ tests/unit/preferences.test.ts (10 tests) 15ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 68ms
 ✓ tests/unit/view-state.test.ts (5 tests) 9ms
 ✓ tests/unit/contrast.test.ts (13 tests) 11ms
 ✓ tests/unit/host-client.test.ts (4 tests) 13ms
 ✓ tests/unit/distribution.test.ts (3 tests) 8ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 31ms
 ✓ tests/unit/contract.test.ts (2 tests) 217ms

 Test Files  70 passed (70)
      Tests  1553 passed | 1 skipped (1554)
   Start at  00:57:19
   Duration  29.09s (transform 4.17s, setup 2.69s, import 10.13s, tests 29.26s, environment 33.23s)

JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

### Playwright (`vite build` plus `vite preview`, 1440 × 900, light and dark)

```

Running 116 tests using 2 workers

  ✓    1 [chromium] › tests/e2e/preview/file-browser.spec.ts:96:1 › the browser is a defined element and lists display names in the list layout from the label table (764ms)
  ✓    3 [chromium] › tests/e2e/preview/file-browser.spec.ts:140:1 › a file row shows size, type hint, modified date and version (521ms)
  ✓    2 [chromium] › tests/e2e/preview/file-panel.spec.ts:102:5 › light theme › File info (35): header, tab, filter, count, rows and footer (1.4s)
  ✓    4 [chromium] › tests/e2e/preview/file-browser.spec.ts:152:1 › computed token values are exact in the browser in light and dark (707ms)
  ✓    5 [chromium] › tests/e2e/preview/file-panel.spec.ts:217:5 › light theme › Versions (26): Back, count, eyebrow, rows, Current, menu and note (1.3s)
  ✓    6 [chromium] › tests/e2e/preview/file-browser.spec.ts:175:1 › opening a folder updates the breadcrumb and the address, and Back returns (722ms)
  ✓    8 [chromium] › tests/e2e/preview/file-browser.spec.ts:196:1 › an image previews through a content handle that is used only as the media src (767ms)
  ✓    7 [chromium] › tests/e2e/preview/file-panel.spec.ts:349:5 › light theme › restore (27): the dialog, one call with the label, and the new current version (1.3s)
  ✓    9 [chromium] › tests/e2e/preview/file-browser.spec.ts:231:1 › a handle that fails is replaced once; a second failure shows the error and mints no third (642ms)
  ✓   10 [chromium] › tests/e2e/preview/file-panel.spec.ts:472:5 › light theme › panel states (25): loading, no match and error keep the header, tabs and filter (1.1s)
  ✓   11 [chromium] › tests/e2e/preview/file-browser.spec.ts:247:1 › a video resumes at the same position and paused state after one new handle (936ms)
  ✓   13 [chromium] › tests/e2e/preview/file-browser.spec.ts:283:1 › audio plays natively and is named by the display name (657ms)
  ✓   12 [chromium] › tests/e2e/preview/file-panel.spec.ts:549:5 › light theme › keyboard: the tab, the history button and the dialog keep focus where it belongs (985ms)
  ✓   14 [chromium] › tests/e2e/preview/file-browser.spec.ts:296:1 › text previews through one ranged read of the first 64 KiB and says it is cut (684ms)
  ✓   15 [chromium] › tests/e2e/preview/file-panel.spec.ts:102:5 › dark theme › File info (35): header, tab, filter, count, rows and footer (1.0s)
  ✓   16 [chromium] › tests/e2e/preview/file-browser.spec.ts:312:1 › a format it cannot show says so and shows no viewer (633ms)
  ✓   18 [chromium] › tests/e2e/preview/file-browser.spec.ts:342:1 › the viewer steps through files, closes with Escape and returns focus to the row (749ms)
  ✓   19 [chromium] › tests/e2e/preview/file-browser.spec.ts:355:1 › an upload is one PUT of the client's facts that creates the file's own object (490ms)
  ✓   17 [chromium] › tests/e2e/preview/file-panel.spec.ts:217:5 › dark theme › Versions (26): Back, count, eyebrow, rows, Current, menu and note (1.5s)
  ✓   20 [chromium] › tests/e2e/preview/file-browser.spec.ts:404:1 › a rename posts to the renames route, never to moves, and shows the new display name (791ms)
  ✓   22 [chromium] › tests/e2e/preview/file-browser.spec.ts:433:1 › a refused rename shows the label for its reason and keeps the display name (565ms)
  ✓   21 [chromium] › tests/e2e/preview/file-panel.spec.ts:349:5 › dark theme › restore (27): the dialog, one call with the label, and the new current version (1.3s)
  ✓   23 [chromium] › tests/e2e/preview/file-browser.spec.ts:452:1 › writes are single calls: new folder, move, remove and Undo (1.0s)
  ✓   24 [chromium] › tests/e2e/preview/file-panel.spec.ts:472:5 › dark theme › panel states (25): loading, no match and error keep the header, tabs and filter (1.0s)
  ✓   25 [chromium] › tests/e2e/preview/file-browser.spec.ts:499:1 › a refused write shows the label for its reason and changes nothing (593ms)
  ✓   26 [chromium] › tests/e2e/preview/file-panel.spec.ts:549:5 › dark theme › keyboard: the tab, the history button and the dialog keep focus where it belongs (982ms)
  ✓   27 [chromium] › tests/e2e/preview/file-browser.spec.ts:514:1 › a long folder is virtualised, pages in on demand and stays keyboard reachable (734ms)
  ✓   28 [chromium] › tests/e2e/preview/host-indicator.spec.ts:96:1 › the indicator is a defined custom element in the shell header, named from the label table (364ms)
  ✓   29 [chromium] › tests/e2e/preview/file-browser.spec.ts:554:1 › the layout does not scroll sideways at 320 px, at 200 % text size, or both, in list and grid (596ms)
  ✓   30 [chromium] › tests/e2e/preview/host-indicator.spec.ts:110:1 › the desktop host says only you can read your files, and shows the not-enrolled state (597ms)
  ✓   32 [chromium] › tests/e2e/preview/host-indicator.spec.ts:124:1 › a server names its operator, who can read the files, and states the at-rest duty (468ms)
  ✓   31 [chromium] › tests/e2e/preview/file-browser.spec.ts:571:1 › motion stops under reduced motion and focus is the 2px --fs-focus-ring ring (687ms)
  ✓   33 [chromium] › tests/e2e/preview/host-indicator.spec.ts:137:1 › a server without an operator name uses the unnamed-operator label (447ms)
  ✓   34 [chromium] › tests/e2e/preview/file-browser.spec.ts:598:1 › a failed listing shows the error state with Try again, and retrying loads it (716ms)
  ✓   35 [chromium] › tests/e2e/preview/host-indicator.spec.ts:149:1 › the browser host says files are decrypted only in this browser (453ms)
  ✓   36 [chromium] › tests/e2e/preview/library.spec.ts:47:1 › the start page offers create, open and recover in the active vocabulary (597ms)
  ✓   37 [chromium] › tests/e2e/preview/host-indicator.spec.ts:162:1 › enrolled backup and sync are shown apart from the host, as ciphertext only (451ms)
  ✓   39 [chromium] › tests/e2e/preview/host-indicator.spec.ts:187:1 › a host.change event updates the indicator without a reload or another read (489ms)
  ✓   38 [chromium] › tests/e2e/preview/library.spec.ts:80:1 › (1) create asks for a folder, passes the location ID on and shows the library, with no path (681ms)
  ✓   40 [chromium] › tests/e2e/preview/host-indicator.spec.ts:209:1 › events.resync makes the indicator read the descriptor again (521ms)
[WebServer] 12:55:19 AM [vite] http proxy error: /api/v1/navigation?node=l1
[WebServer] Error: connect ECONNREFUSED 127.0.0.1:8083
[WebServer]     at TCPConnectWrap.afterConnect [as oncomplete] (node:net:1637:16)
  ✓   41 [chromium] › tests/e2e/preview/library.spec.ts:102:1 › (1) open passes the location ID to the open route (723ms)
  ✓   43 [chromium] › tests/e2e/preview/library.spec.ts:116:1 › (1) recover opens the copy, rebuilds from files and reports without repairing (506ms)
  ✓   42 [chromium] › tests/e2e/preview/host-indicator.spec.ts:222:1 › loading shows no host until the descriptor arrives (1.3s)
  ✓   44 [chromium] › tests/e2e/preview/library.spec.ts:153:1 › (1) copy asks for a destination and passes it as destination_location_id (682ms)
  ✓   45 [chromium] › tests/e2e/preview/host-indicator.spec.ts:234:1 › a failed read shows the error state and never a guessed host, then Retry recovers (586ms)
  ✓   46 [chromium] › tests/e2e/preview/library.spec.ts:176:1 › (2) a cancelled folder choice sends no library request and says nothing changed (614ms)
  ✓   47 [chromium] › tests/e2e/preview/host-indicator.spec.ts:256:1 › the header shows a compact badge with the short host label, not the statement (438ms)
  ✓   48 [chromium] › tests/e2e/preview/library.spec.ts:190:1 › (3) 503 unavailable is the unavailable state with its reason and a Retry, and the choices stay (772ms)
  ✓   49 [chromium] › tests/e2e/preview/host-indicator.spec.ts:271:1 › Enter opens the popover with the full statement; Escape closes it and returns focus (706ms)
  ✓   50 [chromium] › tests/e2e/preview/library.spec.ts:217:1 › (4) confirm sends the consent for exactly the named objects and shows each result (651ms)
  ✓   51 [chromium] › tests/e2e/preview/host-indicator.spec.ts:297:1 › Privacy settings opens the Settings page, whose Privacy section has the full statement (755ms)
  ✓   52 [chromium] › tests/e2e/preview/library.spec.ts:254:1 › (4) cancel and Escape send nothing and return focus to the opener (632ms)
  ✓   53 [chromium] › tests/e2e/preview/host-indicator.spec.ts:333:1 › computed token values are exact in light and dark (UI-S-024, mockup 40) (874ms)
  ✓   54 [chromium] › tests/e2e/preview/library.spec.ts:277:1 › (4) a refused repair says nothing was changed and shows no result (617ms)
  ✓   55 [chromium] › tests/e2e/preview/host-indicator.spec.ts:378:1 › the badge is reachable by keyboard and has a visible focus outline (693ms)
  ✓   56 [chromium] › tests/e2e/preview/library.spec.ts:292:1 › (5) the Monitor's W010 note links to the library's repair offer (619ms)
  ✓   58 [chromium] › tests/e2e/preview/library.spec.ts:345:1 › colours come from tokens in light and dark (1.0s)
  ✓   57 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:89:5 › light theme › Description (28): tab, filter, count, rows, chips, actions and Add property (1.6s)
  ✓   59 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:116:5 › light theme › the shell frame (UI-S-025): sidebar, header, footer, controls and strings (786ms)
  ✓   61 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:200:5 › light theme › the navigation tree as built (55, UI-S-017): eyebrows, 30 px rows, 16 px icons (598ms)
  ✓   62 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:238:5 › light theme › welcome (01, UI-S-027): title, lead, cards, privacy box and footer (618ms)
  ✓   60 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:224:5 › light theme › Add property (29): the dialog, the groups in the add order, one set and the new row (2.0s)
  ✓   63 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:294:5 › light theme › folder-choice unavailable (03) and the view states treatment (56) (770ms)
  ✓   65 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:329:5 › light theme › library page (41, UI-S-027): facts rows, actions and the W010 note (719ms)
  ✓   64 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:379:5 › light theme › edit and remove (28): one PUT, one POST, the rows and the announcements (1.2s)
  ✓   66 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:379:5 › light theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (820ms)
  ✓   67 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:435:5 › light theme › panel states (25): empty, loading, no match and error keep the header, tabs and filter (1.2s)
  ✓   68 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:424:5 › light theme › Monitor (31) and its OCFL validation section (54) (711ms)
  ✓   70 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:485:5 › light theme › host popover (40) and Settings (32): 372 px overlay, Appearance and About (712ms)
  ✓   69 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:503:5 › light theme › no match and keyboard: the filter, Escape in the dialog, and the focus ring (996ms)
  ✓   71 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:116:5 › dark theme › the shell frame (UI-S-025): sidebar, header, footer, controls and strings (871ms)
  ✓   73 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:200:5 › dark theme › the navigation tree as built (55, UI-S-017): eyebrows, 30 px rows, 16 px icons (585ms)
  ✓   72 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:89:5 › dark theme › Description (28): tab, filter, count, rows, chips, actions and Add property (1.9s)
  ✓   74 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:238:5 › dark theme › welcome (01, UI-S-027): title, lead, cards, privacy box and footer (544ms)
  ✓   76 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:294:5 › dark theme › folder-choice unavailable (03) and the view states treatment (56) (702ms)
  ✓   77 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:329:5 › dark theme › library page (41, UI-S-027): facts rows, actions and the W010 note (638ms)
  ✓   75 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:224:5 › dark theme › Add property (29): the dialog, the groups in the add order, one set and the new row (1.9s)
  ✓   78 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:379:5 › dark theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (604ms)
  ✓   80 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:424:5 › dark theme › Monitor (31) and its OCFL validation section (54) (817ms)
  ✓   79 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:379:5 › dark theme › edit and remove (28): one PUT, one POST, the rows and the announcements (1.3s)
  ✓   81 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:485:5 › dark theme › host popover (40) and Settings (32): 372 px overlay, Appearance and About (805ms)
  ✓   83 [chromium] › tests/e2e/preview/navigation.spec.ts:62:1 › the live tree fills the shell's navigation slot, worded from the label table (443ms)
  ✓   82 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:435:5 › dark theme › panel states (25): empty, loading, no match and error keep the header, tabs and filter (1.3s)
  ✓   84 [chromium] › tests/e2e/preview/navigation.spec.ts:83:1 › a level is read page by page through next_cursor until it is null (462ms)
  ✓   86 [chromium] › tests/e2e/preview/navigation.spec.ts:121:1 › a destination opens with a breadcrumb filled from ancestors (615ms)
  ✓   85 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:503:5 › dark theme › no match and keyboard: the filter, Escape in the dialog, and the focus ring (1.0s)
  ✓   87 [chromium] › tests/e2e/preview/navigation.spec.ts:142:1 › a deep link to a destination shows its breadcrumb after a reload (651ms)
  ✓   88 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:174:1 › (1) an E023 finding shows with its code beside the degraded health reason (622ms)
  ✓   89 [chromium] › tests/e2e/preview/navigation.spec.ts:154:1 › navigation.change reads only the named level again, without a reload (647ms)
  ✓   90 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:200:1 › (2) the finding disappears after a clean revalidation's health.change, without a reload (620ms)
  ✓   91 [chromium] › tests/e2e/preview/navigation.spec.ts:177:1 › a 503 shows the unavailable state with Retry, never an empty tree (595ms)
  ✓   92 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:220:1 › (3) a W010 warning is listed apart from errors and health stays ok (598ms)
  ✓   94 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:238:1 › (4) no location is rendered when the API withholds it (584ms)
  ✓   93 [chromium] › tests/e2e/preview/navigation.spec.ts:194:1 › computed token values of the tree and breadcrumb are exact in light and dark (922ms)
  ✓   95 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:250:1 › (5) the last and next run times come from the findings route; Validate now starts a run (671ms)
  ✓   96 [chromium] › tests/e2e/preview/production-build.spec.ts:63:1 › the built page is static, mounts the shell and defines every custom element (533ms)
  ✓   97 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:289:1 › error and warning codes take their colours from tokens in light and dark (657ms)
  ✓   98 [chromium] › tests/e2e/preview/production-build.spec.ts:80:1 › sidebar, header and Settings words come from the active label table (UI-S-025, UI-S-033) (654ms)
  ✓  100 [chromium] › tests/e2e/preview/production-build.spec.ts:124:1 › computed token values are exact in light and dark (UI-S-024, CA-1) (498ms)
  ✓  101 [chromium] › tests/e2e/preview/production-build.spec.ts:146:1 › a refused health call shows the unavailable alert in Settings, About, not a console error (398ms)
  ✓   99 [chromium] › tests/e2e/preview/versions-panel.spec.ts:78:5 › light theme › menu: the current version offers Download and Edit label, with 16 px icons in the icon colour (1.0s)
  ✓  102 [chromium] › tests/e2e/preview/versions-panel.spec.ts:110:5 › light theme › menu: a version offers no delete, expiry or thinning (658ms)
  ✓  103 [chromium] › tests/e2e/preview/versions-panel.spec.ts:123:5 › light theme › Edit label: the dialog, one call with the head as revision, and the new label in the row (1.1s)
  ✓  104 [chromium] › tests/e2e/preview/versions-panel.spec.ts:230:5 › light theme › Edit label: an unchanged label sends nothing, and a head moved meanwhile is refused in the dialog (923ms)
  ✓  105 [chromium] › tests/e2e/preview/versions-panel.spec.ts:261:5 › light theme › Download this version: saves that version's file under the display name (634ms)
  ✓  106 [chromium] › tests/e2e/preview/versions-panel.spec.ts:278:5 › light theme › Download this version: a version above the desktop cap is read in ranged 8 MiB reads and saved whole (1.3s)
  ✓  107 [chromium] › tests/e2e/preview/versions-panel.spec.ts:300:5 › light theme › Download this version: a refused download shows a visible alert and saves nothing (699ms)
  ✓  108 [chromium] › tests/e2e/preview/versions-panel.spec.ts:331:5 › light theme › File info: the history size row follows the size, from the history-size route (481ms)
  ✓  109 [chromium] › tests/e2e/preview/versions-panel.spec.ts:78:5 › dark theme › menu: the current version offers Download and Edit label, with 16 px icons in the icon colour (709ms)
  ✓  110 [chromium] › tests/e2e/preview/versions-panel.spec.ts:110:5 › dark theme › menu: a version offers no delete, expiry or thinning (651ms)
  ✓  111 [chromium] › tests/e2e/preview/versions-panel.spec.ts:123:5 › dark theme › Edit label: the dialog, one call with the head as revision, and the new label in the row (1.2s)
  ✓  112 [chromium] › tests/e2e/preview/versions-panel.spec.ts:230:5 › dark theme › Edit label: an unchanged label sends nothing, and a head moved meanwhile is refused in the dialog (913ms)
  ✓  113 [chromium] › tests/e2e/preview/versions-panel.spec.ts:261:5 › dark theme › Download this version: saves that version's file under the display name (622ms)
  ✓  114 [chromium] › tests/e2e/preview/versions-panel.spec.ts:278:5 › dark theme › Download this version: a version above the desktop cap is read in ranged 8 MiB reads and saved whole (1.2s)
  ✓  115 [chromium] › tests/e2e/preview/versions-panel.spec.ts:300:5 › dark theme › Download this version: a refused download shows a visible alert and saves nothing (725ms)
  ✓  116 [chromium] › tests/e2e/preview/versions-panel.spec.ts:331:5 › dark theme › File info: the history size row follows the size, from the history-size route (490ms)

  116 passed (1.0m)
```

### Pre-commit hook (`bash scripts/pre-commit.sh`, step lines only)

```
========================================
========================================
✅ Branch Validation passed.
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
✅ filesquid-server uniquely bound to 8083
✅ Document Sync Validation passed.
✅ Traceability Validation passed: 96 PRD requirements, 84 resolved SRS references.
✅ Portable Documentation validation passed.
✅ Workspace Boundary validation passed.
✅ Logging rule validation passed.
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
✅ Change scope validation passed (188 assertions).
ℹ️  Test scope: npm:apps/filesquid/ui
✅ ESLint passed (5s).
✅ TypeScript passed (6s).
 Test Files  70 passed (70)
      Tests  1553 passed | 1 skipped (1554)
✅ Vitest passed (32s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: npm:apps/filesquid/ui).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui (64s)
========================================
```
