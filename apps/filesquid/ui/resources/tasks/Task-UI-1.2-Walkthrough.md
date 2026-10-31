# Task UI-1.2 Walkthrough: Import UI with job progress

Branch `feat/ui-1.2-import-progress`. Plan row: [Task UI-1.2](../plan.md), dispatched by global [Task 1.2](../../../../../resources/implementation-plan.md#task-1.2). Mockups (active suite v3): [52-import-destination](../../../../../resources/ui-mockups/v3/52-import-destination.svg), [18-import-review](../../../../../resources/ui-mockups/v3/18-import-review.svg), [19-import-progress](../../../../../resources/ui-mockups/v3/19-import-progress.svg), [48-write-refusals](../../../../../resources/ui-mockups/v3/48-write-refusals.svg). Built on DE-1.7, DE-1.8 and API-F2.16 (merged on `main` at `6def085`).

## SRS mapping

| SRS ID and title | What was built | Proof |
|---|---|---|
| [UI-S-002](../srs.md) The default UI MUST provide a Drive-style browser with navigation, breadcrumb, list and grid views, preview, upload, create folder, rename or move, remove and restore, and a sidebar file panel (Destination: every upload and import MUST name a destination collection, and the folder inside it when one is shown) | `<fs-import-dialog>` requires a destination collection (and optionally a folder) in the shared `<fs-destination-picker>` before its start control is enabled and sends `collection_id` and `folder`; nothing is sent before a destination is chosen; `destination_required` and `collection_not_found` are honest errors | `tests/ui/import-dialog.test.ts`, `tests/e2e/preview/import.spec.ts` |
| [UI-S-028](../srs.md) Destination, drop target, import and jobs | Mockup 52 dialog (680 × 680 px), the planning state, the mockup 18 review (860 × 720 px, four 192 × 72 px cards, Files table, destination with Change…, Space, integrity note), the mockup 19 Jobs panel (440 × 680 px, header button with count badge, chips, 4 px and 3 px bars, controls only where accepted, per-file status and error code, Retry, Retry failed, note) and the footer's "Uploading · {done} of {total}" | the same specs, `tests/ui/jobs-panel.test.ts`, `tests/unit/imports-model.test.ts`, `tests/unit/jobs-store.test.ts` |
| [UI-S-029](../srs.md) Recently deleted, collection deletion, restore, duplicate names, write refusals and their settings (Refusals, mockup 48) | The error card (icon, title, reason, Dismiss) for `collection_not_found`, `destination_required` and `path_suffix_exhausted`, worded from the answer's status and step, a file's own `error_code` and the UI's own pre-send check; no sub-code is invented (API-F2.21 adds them) | `tests/unit/imports-failure.test.ts`, the refusal tests of both UI specs |
| [UI-S-024](../srs.md) Visual constants of the active mockup suite | Every colour through its token in light and dark (overlay dialog and panel, raised cards and note boxes, outline-strong table and tree borders, control outline on radios and fields, primary, active tint, error), Georgia 22 px 700 dialog title and 20 px 700 panel title, 14 px 600 buttons, 11 px eyebrows, 13 px meta text, radii 7, 6, 8 and 10 px | computed-style assertions of `import.spec.ts` in light and dark |
| [UI-S-012](../srs.md) UIs reach FileSquid only through `FileSquidClient` | `ClientImportSource` and `ClientJobSource` over `request()` and `subscribe()`; no path, name or storage key leaves the page in a request, and every answer is checked against the contract | `tests/unit/imports-source.test.ts` |
| [UI-S-005](../srs.md) A distribution's label table MUST define every key of the default table | 86 new keys in `REQUIRED_LABEL_KEYS`, `labels.en.json` and ImageSquid's table (album and photo vocabulary) | `tests/unit/theme.test.ts`, `tests/unit/distribution-tables.test.ts` |
| [UI-S-013](../srs.md) UI tests against production artifacts with strict DOM assertions | Playwright against `vite build` plus `vite preview`, 1440 × 900, light and dark: custom elements defined, labels from the active table, computed styles, boxes within 2 px, no console errors | Playwright output below |
| [UI-S-016](../srs.md) Accessibility contracts | Modal upload dialog with Tab kept inside, Escape as Cancel, focus returned to the opener; the panel is a labelled dialog that returns focus to its button; redraws keep focus on the same control; polite status for planning and the footer; error cards are alerts | `accessibilityViolations` checks of both component test files |
| [API-S-036](../../../crates/api/resources/srs.md) Import route and [API-S-037](../../../crates/api/resources/srs.md) Job routes and job events | The plan, planned-files, start and discard routes, the job list, files and the five controls, and `job.update` events, as the contract states them; a retry of a paused job is never offered | `imports-source.test.ts`, `jobs-store.test.ts` |

PRD: [F-002](../../../../../resources/prd.md#f-002-safe-file-folder-and-device-import) Safe file, folder and device import. SRS: [SRS-F-002](../../../../../resources/srs.md#srs-f-002-safe-file-folder-and-device-import) Safe file, folder and device import. Gherkin: [safe_file_folder_and_device_import_F-002.feature](../../tests/features/safe_file_folder_and_device_import_F-002.feature) (13 new scenarios from the feature-writer skill).

## What changed

- **`src/imports/`**: `model.ts` (contract types, the `ImportSource`, `JobSource` and `DestinationDirectory` ports, and the pure rules: destination of a picker trail, numbered names, a job's phase, the controls it accepts, the files the panel lists, one file's status), `contract.ts` (parsers that keep contract fields only), `client-source.ts`, `jobs-source.ts`, `destination-source.ts`, `directory.ts`, `failure.ts`, `memory-source.ts` and `jobs-store.ts` (bounded: five pages of files per job, one read per second per job).
- **Elements**: `fs-import-dialog.ts` with `import-steps.ts`, `fs-jobs-panel.ts` with `jobs-view.ts`, `refusal-card.ts`, `import-style.ts`; wired by `src/app/import-route.ts` and `main.ts`.
- **Shared elements changed**: `<fs-destination-picker>` (import variant, `preselect`), `<fs-new-menu>` (`folders` switch, so the upload items can be on while New folder is off), `<fs-app-shell>` (`jobs` header slot, `uploadStatus` footer), `icons.ts` (upload, pause, play, retry, shield, arrow), `formatLabel` (placeholders may contain `_`, for `{error_code}`).
- **Labels**: 86 keys (`import*`, `jobs*`, `fileDestinationRequired`, `fileCollectionNotFound`, `fileSuffixExhausted`) in the default and ImageSquid tables.
- **Test support**: `strict-dom.ts` excuses the browser's network line for the Jobs button's start-up read of `GET /api/v1/jobs` where a host does not serve it yet, as it does for preferences and recent libraries.

## Decisions and open points for the owner and the platform engineer

- **Nothing here can upload end to end until the API names a collection.** Navigation node handles are opaque (API-S-021) and no route or field gives a node's `library_id` and `collection_id`, which `POST /api/v1/imports` requires. The dialog asks `GET /api/v1/navigation/destination?node=<handle>`, answering `{library_id, collection_id}` for a collection or a folder inside one. That route is **not in the contract** (I could not edit the API crate). On a host without it the dialog refuses before sending (`importDestinationUnresolved`) and the Playwright suite proves nothing is sent. UI-F2.17 hits the same gap ("a producer of `FileLocation.collection_id`").
- **Free-space reserve.** Mockup 18 says "Staging uses at most 10 GiB and keeps {reserve} free." The plan has no reserve (the host computes max(2 GiB, 5 % of capacity)), so the review says "Staging uses at most {limit}." A `reserve_bytes` field on `ImportPlan` would restore the mockup's wording. Sizes read "10 GB" and "409 GB" because the shared `formatSize` uses base 1024 with GB labels.
- **Job titles.** `JobDestination` has only `collection_id`, so a job's title names its collection only when the dialog resolved it in this session, and otherwise says "Upload into {Collection} › {folder}".
- **Retry failed is shown less often than mockup 19.** The mockup draws Retry failed on a failed (finished) job, which API-S-037 refuses. The panel offers it only where the job accepts it: on a job waiting for the owner, or a running job with two or more failed files (one failed file has its own Retry, as the mockup's running job shows).
- **Needs attention (DE-1.8, owner 2026-10-31 "accept recommendations").** A pending job whose files ended with a failed one shows the chip "needs attention", Retry failed, Retry and Cancel, no bar and no motion. Pause is not offered there (nothing is running).
- **Mockup 52 and 18 text widths.** Text-sized buttons are asserted by height and position, not width, because this machine's sans font is not the mockup's. Dialogs are centred by the window, so their regions are measured from the dialog's own corner.
- **Strings proposed here** (the approved ones are in the row's Agent Prompt): the planning sentence, the refusal sentences of the plan request by status (`importPlanGone`, `importInvalid`, `importDialogOpen`, `importSourceChanged`, `importUnavailable`, `importFailed`, `importOffline`, `importDestinationUnresolved`), `importNothingNew`, `importFilesCut`, the chips `jobsStatePausing`, `jobsStateCancelling`, `jobsStateWaiting`, `jobsStateAttention`, `jobsStateCompleted`, `jobsDetailCompleted` and the panel's empty and refusal lines. They need the owner's approval.
- **Test order.** The unit tests (model, sources, failure, store) were run red first (below). The jsdom component tests and the Playwright spec were written once the elements existed, because the mockup geometry was tuned from measurements of the rendered regions.

## Mocked Environment Variables

None. The Playwright fake host answers the contract's own shapes from the mockups' data; no credential or environment value was invented. The installed Playwright expects a newer Chromium than this machine has, so the suites ran through a temporary config that points `executablePath` at the machine's Chromium; the config was deleted before committing.

## Raw test output

### Red run (unit tests, before any implementation)

```
$ npx vitest run tests/unit/imports-model.test.ts tests/unit/imports-source.test.ts tests/unit/imports-failure.test.ts tests/unit/jobs-store.test.ts
 FAIL  tests/unit/jobs-store.test.ts [ tests/unit/jobs-store.test.ts ]
Error: Failed to resolve import "../../src/imports/jobs-store" from "tests/unit/jobs-store.test.ts". Does the file exist?
 Test Files  4 failed (4)
      Tests  no tests
```

### Unit and component tests (Vitest, jsdom)

```
$ npx eslint .          (no output: clean)
$ npx tsc --noEmit      (no output: clean)
$ npx vitest run
 Test Files  95 passed (95)
      Tests  2260 passed | 1 skipped (2261)
```

New in this task: `tests/unit/imports-model.test.ts` (17), `imports-source.test.ts` (16), `imports-failure.test.ts` (9), `jobs-store.test.ts` (12), `tests/ui/import-dialog.test.ts` (19), `tests/ui/jobs-panel.test.ts` (17) and `tests/ui/import-shell.test.ts` (6). The package has no coverage provider; coverage is shown by the SRS mapping above.

### Playwright, production build (`vite build` plus `vite preview`, 1440 × 900, light and dark)

```
$ npx playwright test (whole preview suite; the import.spec.ts runs are listed)
✓   63 [chromium] › tests/e2e/preview/import.spec.ts:127:5 › light theme › destination step (mockup 52): regions, strings and constants, nothing sent before a destination
✓   66 [chromium] › tests/e2e/preview/import.spec.ts:212:5 › light theme › choosing a folder in the tree enables the start control, which plans with the destination's IDs
✓   68 [chromium] › tests/e2e/preview/import.spec.ts:243:5 › light theme › opening from a collection's New menu starts with that collection chosen
✓   70 [chromium] › tests/e2e/preview/import.spec.ts:255:5 › light theme › planning is shown without a percentage, then the review (mockup 18): regions, strings and constants
✓   74 [chromium] › tests/e2e/preview/import.spec.ts:338:5 › light theme › Cancel discards the plan and starts nothing; the review comes before any job
✓   76 [chromium] › tests/e2e/preview/import.spec.ts:357:5 › light theme › Upload starts the job, opens the Jobs panel (mockup 19) and the footer shows the progress
✓   79 [chromium] › tests/e2e/preview/import.spec.ts:127:5 › dark theme › destination step (mockup 52): regions, strings and constants, nothing sent before a destination
✓   81 [chromium] › tests/e2e/preview/import.spec.ts:212:5 › dark theme › choosing a folder in the tree enables the start control, which plans with the destination's IDs
✓   83 [chromium] › tests/e2e/preview/import.spec.ts:243:5 › dark theme › opening from a collection's New menu starts with that collection chosen
✓   86 [chromium] › tests/e2e/preview/import.spec.ts:255:5 › dark theme › planning is shown without a percentage, then the review (mockup 18): regions, strings and constants
✓   90 [chromium] › tests/e2e/preview/import.spec.ts:338:5 › dark theme › Cancel discards the plan and starts nothing; the review comes before any job
✓   93 [chromium] › tests/e2e/preview/import.spec.ts:357:5 › dark theme › Upload starts the job, opens the Jobs panel (mockup 19) and the footer shows the progress
✓   95 [chromium] › tests/e2e/preview/import.spec.ts:443:5 › light theme, Jobs panel and refusals › the Jobs panel (mockup 19): button, panel, blocks, chips, bars, controls, files and note
✓   98 [chromium] › tests/e2e/preview/import.spec.ts:557:5 › light theme, Jobs panel and refusals › Retry puts one failed file back, Retry failed puts the rest back, Pause and Resume go through the job routes
✓  102 [chromium] › tests/e2e/preview/import.spec.ts:574:5 › light theme, Jobs panel and refusals › a pending job whose files ended with a failure is waiting for the owner: no bar, Retry failed and Cancel (DE-1.8)
✓  103 [chromium] › tests/e2e/preview/import.spec.ts:606:5 › light theme, Jobs panel and refusals › refusals (mockup 48): collection_not_found for a 404 of the plan, path_suffix_exhausted for a file, each with Dismiss
✓  105 [chromium] › tests/e2e/preview/import.spec.ts:644:5 › light theme, Jobs panel and refusals › a host that cannot name the collection yet refuses honestly and sends no plan request
✓  107 [chromium] › tests/e2e/preview/import.spec.ts:443:5 › dark theme, Jobs panel and refusals › the Jobs panel (mockup 19): button, panel, blocks, chips, bars, controls, files and note
✓  108 [chromium] › tests/e2e/preview/import.spec.ts:557:5 › dark theme, Jobs panel and refusals › Retry puts one failed file back, Retry failed puts the rest back, Pause and Resume go through the job routes
✓  111 [chromium] › tests/e2e/preview/import.spec.ts:574:5 › dark theme, Jobs panel and refusals › a pending job whose files ended with a failure is waiting for the owner: no bar, Retry failed and Cancel (DE-1.8)
✓  112 [chromium] › tests/e2e/preview/import.spec.ts:606:5 › dark theme, Jobs panel and refusals › refusals (mockup 48): collection_not_found for a 404 of the plan, path_suffix_exhausted for a file, each with Dismiss
✓  114 [chromium] › tests/e2e/preview/import.spec.ts:644:5 › dark theme, Jobs panel and refusals › a host that cannot name the collection yet refuses honestly and sends no plan request
✓  116 [chromium] › tests/e2e/preview/import.spec.ts:666:1 › the plan of a smaller upload shows its own counts and disables Upload when nothing is new

189 passed (2.1m) (the whole preview suite, 189 runs, light and dark)
```

## Pre-commit

The head-commit run is recorded in the pull request (`Test scope: npm:apps/filesquid/ui`).
