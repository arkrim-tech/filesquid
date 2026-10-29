# Task UI-F2.4 Walkthrough: `<fs-metadata-panel>`

Branch `feat/ui-f2.4-metadata-panel`. Plan row: [Task UI-F2.4](../plan.md). Mockups (active suite v3): [28-panel-metadata](../../../../../resources/ui-mockups/v3/28-panel-metadata.svg), [29-metadata-add-property](../../../../../resources/ui-mockups/v3/29-metadata-add-property.svg), [25-panel-states](../../../../../resources/ui-mockups/v3/25-panel-states.svg). Built on the file panel and the shared property table of [UI-F2.3](./Task-UI-F2.3-Walkthrough.md) and the tokens and shell of [UI-F2.18](./Task-UI-F2.18-Walkthrough.md). Completes global [Task FS-2.2](../../../../../resources/implementation-plan.md#task-fs-2.2).

## SRS mapping

| SRS ID and title | What was built | Proof |
|---|---|---|
| [UI-S-003](../srs.md) Panels, including the file panel's tabs, MUST plug into named sidebar slots and MUST NOT depend on each other; each file fact is shown by the panel of its owning module | `<fs-metadata-panel>` is registered as the Description tab in its own slot by the files route (`registerTab("description", …)`), receives only `fileRef`, holds no reference to another panel and shows only the facts the metadata module owns (descriptive statements) | `tests/ui/metadata-panel.test.ts`, `tests/ui/files-route.test.ts` |
| [UI-S-022](../srs.md) File panel tabs: the Description tab shows the Dublin Core `dcterms:` and Schema.org `schema:` statements, viewable and editable as MD-S-011 allows, sharing one property-table component, with copy, filter and distinct empty, loading and error states | Rows keyed by CURIE on the shared `PropertyTable` (extended with source chips, row actions, an empty state and a slot below the rows), copy, filter, no-match, loading, error and empty states | `tests/ui/metadata-panel.test.ts`, `tests/e2e/preview/metadata-panel.spec.ts` |
| [UI-S-031](../srs.md) Preview and file panel composition: Description rows keyed by the mono property in `--fs-primary` for owner values and `--fs-on-surface-variant` for extracted values with a source chip "You" or "File"; a hovered row shows edit, remove and copy; Add property (ghost) below; the Add property dialog (600 px) with the search field, the eyebrows DUBLIN CORE TERMS, SCHEMA.ORG SUGGESTION, REGISTERED NAMESPACES, "Key in a custom namespace…", "Value for {property}" and Cancel and Add; panel states of mockup 25 | Exactly that composition, each string a label key, each constant a token; the dialog's groups follow the API's add order | `tests/e2e/preview/metadata-panel.spec.ts` (boxes within 2 px, computed styles, light and dark) |
| [UI-S-024](../srs.md) Visual constants of the active mockup suite | Every colour, size, radius and spacing through tokens; no literal colour or size in the two new component stylesheets; logical properties only | `tests/unit/component-styles.test.ts`, `tests/e2e/preview/metadata-panel.spec.ts` |
| [UI-S-012](../srs.md) UIs reach FileSquid only through `FileSquidClient` | `ClientMetadataSource` over the five metadata property routes of contract 0.27.0 (API-S-035): `GET /files/metadata`, `PUT /files/metadata/statements`, `POST /files/metadata/removals`, `GET /files/metadata/terms`, `GET /files/metadata/namespaces`; the element never fetches | `tests/unit/metadata-source.test.ts` |
| [UI-S-005](../srs.md) A distribution's label table MUST define every key of the default table | 85 new keys (`filePanelDescription`, `metadata*`, and one `dcterms<Term>` definition per offered `/terms/` property) in `REQUIRED_LABEL_KEYS`, the default table and ImageSquid's table | `tests/unit/theme.test.ts`, `tests/unit/distribution-tables.test.ts` |
| [UI-S-013](../srs.md) UI tests against production artifacts with strict DOM assertions | Playwright against `vite build` plus `vite preview`, 1440 × 900, light and dark, no console errors | Playwright output below |
| [UI-S-016](../srs.md) Accessibility contracts | Description list rows, actions named by their property, a polite status region for every commit, an alert error state, a modal dialog that keeps Tab inside and returns focus on Escape, the 2 px `--fs-focus-ring` ring, no load moves focus | `tests/ui/metadata-panel.test.ts`, `tests/e2e/preview/metadata-panel.spec.ts` |
| [MD-S-011](../../../crates/metadata/resources/srs.md) A presentation-only Metadata panel over the API's metadata property routes as `<fs-metadata-panel>`, mountable without depending on another panel | The element, its source and its model; no module call, no filesystem path | the tests above |
| [API-S-035](../../../crates/api/resources/srs.md) Metadata property routes | The add order (`/terms/` first, then `schema:`, then each registered namespace) drives the dialog's groups; a 400 for an unknown name or prefix, a 404 for a statement the record lacks and a 409 for a conflict are worded; values are checked at 1 to 65,536 bytes before sending | `tests/unit/metadata-model.test.ts`, `tests/ui/metadata-panel.test.ts` |

PRD: [F-046](../../../../../resources/prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports) Media metadata inspection, correction and privacy exports, [F-055](../../../../../resources/prd.md#f-055-default-ui-and-panel-composition) Default UI and panel composition. Gherkin: [media_metadata_inspection_correction_and_privacy_e_F-046.feature](../../tests/features/media_metadata_inspection_correction_and_privacy_e_F-046.feature) and [composable_filesquid_ui_F-055.feature](../../tests/features/composable_filesquid_ui_F-055.feature) gained the scenarios of this task.

## What changed

- **`<fs-metadata-panel>`** (`src/components/fs-metadata-panel.ts`): the Description tab. Rows from `GET /files/metadata`, keyed by CURIE; an asserted value in the owner tone with the chip "You", edit and remove; an extracted value in the extracted tone with the chip "File" and copy only. Remove is one `POST /files/metadata/removals`; edit and add are one `PUT /files/metadata/statements`; each commit is announced, reported as `fs-files-change` (`metadata.set` or `metadata.remove`) and followed by a read of the new version, with focus back on the row's edit button or the Add property button. The same version already shown is not read again; another file or version drops an open dialog and ignores a late answer.
- **Add property dialog** (`src/components/metadata-dialog.ts`): mockup 29 as a modal form. As the owner types, the 52 file-scoped `/terms/` names are matched locally (`DCTERMS_FILE_TERMS`, with the definition of each from the label table) and `GET /files/metadata/terms` is asked for the typed name when it is a valid bare name, so the groups follow the API's add order; the registry (`GET /files/metadata/namespaces`, read once per library) supplies the IRI shown under a registered candidate and decides whether "Key in a custom namespace…" is offered. A chosen candidate or a valid custom key reveals "Value for {property}" and enables Add; the value is checked before sending; a refusal stays in the dialog with its reason. The edit form is the same builder with the property fixed and Save.
- **Model and source** (`src/files/metadata.ts`, `src/files/metadata-source.ts`): the contract types, the rows, the term search, the candidate groups, the value, term-name and custom-key checks, the count text; `MetadataSource` and `ClientMetadataSource` (the file id from the object's `urn:uuid:`; plain bodies as the contract defines them today).
- **`PropertyTable`** (`src/components/property-table.ts`): optional label tone (`PropertyRow.labelStyle`), row chip, row actions before copy, an empty state, an element below the rows, and an optional footer; the File info tab is unchanged.
- **Wiring**: `file-panel-route.ts` registers the Description tab when `bindFilesRoute` is given a `metadata` source, which `main.ts` now passes; `index.ts` defines and exports the element, the dialog, the source and the model; an `edit` icon.
- **Labels**: 85 keys in both tables and `REQUIRED_LABEL_KEYS`.
- **Tests**: `tests/unit/metadata-model.test.ts`, `tests/unit/metadata-source.test.ts`, `tests/ui/metadata-panel.test.ts`, the Description-tab case of `tests/ui/files-route.test.ts`, `tests/support/metadata-fakes.ts`, `tests/e2e/support/metadata-api.ts`, `tests/e2e/preview/metadata-panel.spec.ts`; `component-styles.test.ts` covers the new element; `fidelity.ts` gained the icon colour; the F2.3 keyboard test takes the first (File info) tab now that there are two.
- **Docs**: the package README's File panel section, the plan row's Status, the root checklist and the global FS-2.2 box, the MVP progress block.

## Decisions and open points for the owner

- **Scope follows the active suite, not the older row text.** The UI-F2.4 row (and the global FS-2.2 description) name `exif:` rows and a read-only Import record section in the Metadata panel. The later owner decision of 2026-10-19 (UI-S-022) and the v3 suite (UI-S-031) put technical `exif:` rows under "TECHNICAL · READ FROM THE FILE" and the import facts (`GET /files/facts`, ADR 038) in the **File info** tab, both UI-F2.15's job, and the F2.3 walkthrough already assigned the facts-route rows to UI-F2.15. So the Description tab lists descriptive statements only (`TECHNICAL_PREFIXES` excludes `exif`, a one-line constant for UI-F2.15 to build on), and neither technical rows nor the Import record are drawn here. If the owner wants the Import record in the Description tab after all, say so and it is a read-only section over `GET /files/facts` (the route exists in contract 0.27.0).
- **Term descriptions and local search.** The terms route answers the add order of one exact name, while mockup 29 draws prefix matches ("righ" → rights, rightsHolder) with definitions. The UI therefore ships the 52 file-scoped `/terms/` names (the register's 55 less the three `accrual*` terms MD-S-002 refuses on a file) with one definition per term in the label table, worded for a file. The list mirrors the metadata module's register; it is not read from the API. The Schema.org suggestion is shown exactly as the API answers it (`schema:<typed name>`), since the API offers no Schema.org vocabulary.
- **Extracted rows are read-only.** The API marks only `exif:` statements extracted today, so every Description row is asserted in practice; the UI still draws the extracted tone, the "File" chip and copy-only for any statement the contract marks `extracted`, as mockup 28 draws `dcterms:created`.
- **Remove has no confirmation dialog.** Mockup 28 draws remove as a hover action and no approved string exists for a confirmation; a removal commits one version that the Versions view can restore, and it is announced.
- **SRS wording that disagrees with UI-S-024 or the mockup** (followed the stricter or general rule, as UI-F2.3 did): the mockup's 12 px property key is 13 px (meta text at least 13 px); the mockup's 32 px Add property button is 36 px (every button 36 px); the empty state's Add property is a ghost button at 36 px.
- **No `CommandEnvelope`** on the set and remove routes, as the contract defines them (open owner decision); `ClientMetadataSource.setStatement` and `removeStatement` are their only call sites.
- **The files-route test for the Description tab** was written after the wiring as a wiring proof; the red-first run covered the element, the dialog, the model and the source.

No API route, field or event was assumed or stubbed; every route used is in `apps/filesquid/crates/api/openapi.json` on `main` (0.27.0).

## How it was tested

1. Red run: the three new test files failed to resolve their modules before the implementation (output below).
2. `npx prettier --write .`, `npx eslint . --fix` and `npx tsc --noEmit` in `apps/filesquid/ui` are clean; `apps/imagesquid` is formatted with the same Prettier.
3. `npm test` in `apps/filesquid/ui`: 67 files, 1,472 tests (1 skipped), green.
4. Playwright, production build (`vite build` plus `vite preview`) at 1440 × 900 in light and dark: the new `metadata-panel.spec.ts` (8 tests) and the whole preview suite (98 tests). The installed Playwright expects a newer Chromium than this machine has, so the suite ran through a temporary config that points `executablePath` at the machine's preinstalled Chromium; the config was deleted before committing. The server-backed suite needs a built `filesquid-server`, which this npm-only change does not build.
5. `bash scripts/pre-commit.sh` on the head commit (output below). No mocked environment values were used.

## Raw test output

### Red run (before the implementation)

```text
 FAIL  tests/ui/metadata-panel.test.ts [ tests/ui/metadata-panel.test.ts ]
Error: Failed to resolve import "../../src/files/metadata" from "tests/support/metadata-fakes.ts". Does the file exist?
 FAIL  tests/unit/metadata-model.test.ts [ tests/unit/metadata-model.test.ts ]
Error: Failed to resolve import "../../src/files/metadata" from "tests/unit/metadata-model.test.ts". Does the file exist?
 FAIL  tests/unit/metadata-source.test.ts [ tests/unit/metadata-source.test.ts ]
Error: Failed to resolve import "../../src/files/metadata-source" from "tests/unit/metadata-source.test.ts". Does the file exist?
 Test Files  3 failed (3)
      Tests  no tests
```

### npm test (apps/filesquid/ui)

```text
 ✓ tests/ui/host-indicator.test.ts (40 tests) 550ms
 ✓ tests/unit/theme.test.ts (36 tests) 109ms
 ✓ tests/ui/nav-tree.test.ts (30 tests) 1275ms
 ✓ tests/ui/file-browser-writes.test.ts (47 tests) 1799ms
 ✓ tests/ui/metadata-panel.test.ts (18 tests) 794ms
 ✓ tests/ui/app-shell.test.ts (24 tests) 720ms
 ✓ tests/ui/file-browser.test.ts (37 tests) 1608ms
 ✓ tests/unit/files-source.test.ts (29 tests) 71ms
 ✓ tests/ui/ocfl-findings.test.ts (22 tests) 435ms
 ✓ tests/ui/library-start.test.ts (22 tests) 343ms
 ✓ tests/unit/files-memory-source.test.ts (35 tests) 51ms
 ✓ tests/ui/monitor.test.ts (22 tests) 4639ms
 ✓ tests/ui/library-repair.test.ts (22 tests) 411ms
 ✓ tests/unit/files-ops.test.ts (55 tests) 209ms
 ✓ tests/unit/component-styles.test.ts (239 tests) 87ms
 ✓ tests/ui/library-detail.test.ts (20 tests) 292ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 556ms
 ✓ tests/ui/file-preview.test.ts (16 tests) 207ms
 ✓ tests/unit/client-io.test.ts (23 tests) 46ms
 ✓ tests/ui/versions-panel.test.ts (22 tests) 7863ms
 ✓ tests/ui/navigation-view.test.ts (20 tests) 242ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 47ms
 ✓ tests/ui/settings.test.ts (10 tests) 200ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 31ms
 ✓ tests/ui/details-panel.test.ts (11 tests) 272ms
 ✓ tests/ui/file-panel.test.ts (11 tests) 419ms
 ✓ tests/ui/nav-tree-live.test.ts (11 tests) 415ms
 ✓ tests/unit/media-handle.test.ts (14 tests) 84ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 36ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 111ms
 ✓ tests/ui/files-route.test.ts (11 tests) 706ms
 ✓ tests/unit/library-client.test.ts (18 tests) 44ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 29ms
 ✓ tests/unit/navigation-client.test.ts (13 tests) 25ms
 ✓ tests/unit/files-model.test.ts (60 tests) 51ms
 ✓ tests/unit/metadata-model.test.ts (13 tests) 40ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 36ms
 ✓ tests/unit/commands.test.ts (46 tests) 29ms
 ✓ tests/unit/host-descriptor.test.ts (43 tests) 28ms
 ✓ tests/ui/states.test.ts (26 tests) 159ms
 ✓ tests/unit/versions-model.test.ts (14 tests) 61ms
 ✓ tests/unit/admin-client.test.ts (10 tests) 30ms
 ✓ tests/unit/files-window.test.ts (19 tests) 24ms
 ✓ tests/unit/focus.test.ts (16 tests) 64ms
 ✓ tests/unit/memory-history.test.ts (9 tests) 17ms
 ✓ tests/unit/library-errors.test.ts (15 tests) 15ms
 ✓ tests/unit/test-scripts.test.ts (10 tests | 1 skipped) 575ms
 ✓ tests/unit/distribution-tables.test.ts (7 tests) 18ms
 ✓ tests/unit/navigation-contract.test.ts (30 tests) 23ms
 ✓ tests/unit/details-model.test.ts (7 tests) 32ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 19ms
 ✓ tests/ui/library-route.test.ts (6 tests) 97ms
 ✓ tests/unit/history-source.test.ts (6 tests) 18ms
 ✓ tests/unit/metadata-source.test.ts (6 tests) 21ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 75ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 66ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 11ms
 ✓ tests/unit/client.test.ts (8 tests) 19ms
 ✓ tests/unit/library-model.test.ts (7 tests) 15ms
 ✓ tests/unit/preferences.test.ts (10 tests) 17ms
 ✓ tests/unit/view-state.test.ts (5 tests) 10ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 69ms
 ✓ tests/unit/host-client.test.ts (4 tests) 13ms
 ✓ tests/unit/contrast.test.ts (13 tests) 15ms
 ✓ tests/unit/distribution.test.ts (3 tests) 16ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 26ms
 ✓ tests/unit/contract.test.ts (2 tests) 221ms
 Test Files  67 passed (67)
      Tests  1472 passed | 1 skipped (1473)
   Start at  02:41:59
   Duration  27.55s (transform 3.72s, setup 2.37s, import 9.05s, tests 26.66s, environment 32.92s)
JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

### Playwright, production build: metadata-panel.spec.ts (temporary config with the machine's Chromium)

```text
Running 8 tests using 1 worker
  ✓  1 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:84:5 › light theme › Description (28): tab, filter, count, rows, chips, actions and Add property (1.5s)
  ✓  2 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:205:5 › light theme › Add property (29): the dialog, the groups in the add order, one set and the new row (1.5s)
  ✓  3 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:336:5 › light theme › panel states (25): empty, loading, no match and error keep the header, tabs and filter (866ms)
  ✓  4 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:399:5 › light theme › no match and keyboard: the filter, Escape in the dialog, and the focus ring (670ms)
  ✓  5 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:84:5 › dark theme › Description (28): tab, filter, count, rows, chips, actions and Add property (1.4s)
  ✓  6 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:205:5 › dark theme › Add property (29): the dialog, the groups in the add order, one set and the new row (1.5s)
  ✓  7 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:336:5 › dark theme › panel states (25): empty, loading, no match and error keep the header, tabs and filter (904ms)
  ✓  8 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:399:5 › dark theme › no match and keyboard: the filter, Escape in the dialog, and the focus ring (694ms)
  8 passed (18.5s)
```

### Playwright, production build: whole preview suite

```text
Running 98 tests using 2 workers
  ✓   2 [chromium] › tests/e2e/preview/file-browser.spec.ts:96:1 › the browser is a defined element and lists display names in the list layout from the label table (889ms)
  ✓   3 [chromium] › tests/e2e/preview/file-browser.spec.ts:140:1 › a file row shows size, type hint, modified date and version (457ms)
  ✓   1 [chromium] › tests/e2e/preview/file-panel.spec.ts:102:5 › light theme › File info (35): header, tab, filter, count, rows and footer (1.5s)
  ✓   4 [chromium] › tests/e2e/preview/file-browser.spec.ts:152:1 › computed token values are exact in the browser in light and dark (769ms)
  ✓   5 [chromium] › tests/e2e/preview/file-panel.spec.ts:212:5 › light theme › Versions (26): Back, count, eyebrow, rows, Current, menu and note (1.4s)
  ✓   6 [chromium] › tests/e2e/preview/file-browser.spec.ts:175:1 › opening a folder updates the breadcrumb and the address, and Back returns (787ms)
  ✓   8 [chromium] › tests/e2e/preview/file-browser.spec.ts:196:1 › an image previews through a content handle that is used only as the media src (931ms)
  ✓   7 [chromium] › tests/e2e/preview/file-panel.spec.ts:337:5 › light theme › restore (27): the dialog, one call with the label, and the new current version (1.3s)
  ✓   9 [chromium] › tests/e2e/preview/file-browser.spec.ts:231:1 › a handle that fails is replaced once; a second failure shows the error and mints no third (691ms)
  ✓  11 [chromium] › tests/e2e/preview/file-browser.spec.ts:247:1 › a video resumes at the same position and paused state after one new handle (886ms)
  ✓  10 [chromium] › tests/e2e/preview/file-panel.spec.ts:460:5 › light theme › panel states (25): loading, no match and error keep the header, tabs and filter (1.3s)
  ✓  12 [chromium] › tests/e2e/preview/file-browser.spec.ts:283:1 › audio plays natively and is named by the display name (737ms)
  ✓  13 [chromium] › tests/e2e/preview/file-panel.spec.ts:537:5 › light theme › keyboard: the tab, the history button and the dialog keep focus where it belongs (1.1s)
  ✓  14 [chromium] › tests/e2e/preview/file-browser.spec.ts:296:1 › text previews through one ranged read of the first 64 KiB and says it is cut (797ms)
  ✓  16 [chromium] › tests/e2e/preview/file-browser.spec.ts:312:1 › a format it cannot show says so and shows no viewer (668ms)
  ✓  15 [chromium] › tests/e2e/preview/file-panel.spec.ts:102:5 › dark theme › File info (35): header, tab, filter, count, rows and footer (1.2s)
  ✓  17 [chromium] › tests/e2e/preview/file-browser.spec.ts:342:1 › the viewer steps through files, closes with Escape and returns focus to the row (812ms)
  ✓  19 [chromium] › tests/e2e/preview/file-browser.spec.ts:355:1 › an upload is one PUT of the client's facts that creates the file's own object (528ms)
  ✓  18 [chromium] › tests/e2e/preview/file-panel.spec.ts:212:5 › dark theme › Versions (26): Back, count, eyebrow, rows, Current, menu and note (1.4s)
  ✓  20 [chromium] › tests/e2e/preview/file-browser.spec.ts:404:1 › a rename posts to the renames route, never to moves, and shows the new display name (739ms)
  ✓  22 [chromium] › tests/e2e/preview/file-browser.spec.ts:433:1 › a refused rename shows the label for its reason and keeps the display name (569ms)
  ✓  21 [chromium] › tests/e2e/preview/file-panel.spec.ts:337:5 › dark theme › restore (27): the dialog, one call with the label, and the new current version (1.3s)
  ✓  24 [chromium] › tests/e2e/preview/file-panel.spec.ts:460:5 › dark theme › panel states (25): loading, no match and error keep the header, tabs and filter (1.1s)
  ✓  23 [chromium] › tests/e2e/preview/file-browser.spec.ts:452:1 › writes are single calls: new folder, move, remove and Undo (1.3s)
  ✓  26 [chromium] › tests/e2e/preview/file-browser.spec.ts:499:1 › a refused write shows the label for its reason and changes nothing (682ms)
  ✓  25 [chromium] › tests/e2e/preview/file-panel.spec.ts:537:5 › dark theme › keyboard: the tab, the history button and the dialog keep focus where it belongs (1.0s)
  ✓  28 [chromium] › tests/e2e/preview/host-indicator.spec.ts:96:1 › the indicator is a defined custom element in the shell header, named from the label table (574ms)
  ✓  27 [chromium] › tests/e2e/preview/file-browser.spec.ts:514:1 › a long folder is virtualised, pages in on demand and stays keyboard reachable (872ms)
  ✓  29 [chromium] › tests/e2e/preview/host-indicator.spec.ts:110:1 › the desktop host says only you can read your files, and shows the not-enrolled state (391ms)
  ✓  30 [chromium] › tests/e2e/preview/file-browser.spec.ts:554:1 › the layout does not scroll sideways at 320 px, at 200 % text size, or both, in list and grid (791ms)
  ✓  31 [chromium] › tests/e2e/preview/host-indicator.spec.ts:124:1 › a server names its operator, who can read the files, and states the at-rest duty (391ms)
  ✓  32 [chromium] › tests/e2e/preview/file-browser.spec.ts:571:1 › motion stops under reduced motion and focus is the 2px --fs-focus-ring ring (609ms)
  ✓  33 [chromium] › tests/e2e/preview/host-indicator.spec.ts:137:1 › a server without an operator name uses the unnamed-operator label (587ms)
  ✓  35 [chromium] › tests/e2e/preview/host-indicator.spec.ts:149:1 › the browser host says files are decrypted only in this browser (552ms)
  ✓  34 [chromium] › tests/e2e/preview/file-browser.spec.ts:598:1 › a failed listing shows the error state with Try again, and retrying loads it (703ms)
  ✓  36 [chromium] › tests/e2e/preview/host-indicator.spec.ts:162:1 › enrolled backup and sync are shown apart from the host, as ciphertext only (552ms)
  ✓  37 [chromium] › tests/e2e/preview/library.spec.ts:47:1 › the start page offers create, open and recover in the active vocabulary (693ms)
  ✓  38 [chromium] › tests/e2e/preview/host-indicator.spec.ts:187:1 › a host.change event updates the indicator without a reload or another read (556ms)
  ✓  39 [chromium] › tests/e2e/preview/library.spec.ts:80:1 › (1) create asks for a folder, passes the location ID on and shows the library, with no path (644ms)
  ✓  40 [chromium] › tests/e2e/preview/host-indicator.spec.ts:209:1 › events.resync makes the indicator read the descriptor again (559ms)
  ✓  41 [chromium] › tests/e2e/preview/library.spec.ts:102:1 › (1) open passes the location ID to the open route (600ms)
  ✓  43 [chromium] › tests/e2e/preview/library.spec.ts:116:1 › (1) recover opens the copy, rebuilds from files and reports without repairing (450ms)
  ✓  44 [chromium] › tests/e2e/preview/library.spec.ts:153:1 › (1) copy asks for a destination and passes it as destination_location_id (391ms)
  ✓  42 [chromium] › tests/e2e/preview/host-indicator.spec.ts:222:1 › loading shows no host until the descriptor arrives (1.3s)
  ✓  45 [chromium] › tests/e2e/preview/library.spec.ts:176:1 › (2) a cancelled folder choice sends no library request and says nothing changed (594ms)
  ✓  46 [chromium] › tests/e2e/preview/host-indicator.spec.ts:234:1 › a failed read shows the error state and never a guessed host, then Retry recovers (625ms)
  ✓  48 [chromium] › tests/e2e/preview/host-indicator.spec.ts:256:1 › the header shows a compact badge with the short host label, not the statement (524ms)
  ✓  47 [chromium] › tests/e2e/preview/library.spec.ts:190:1 › (3) 503 unavailable is the unavailable state with its reason and a Retry, and the choices stay (665ms)
  ✓  49 [chromium] › tests/e2e/preview/host-indicator.spec.ts:271:1 › Enter opens the popover with the full statement; Escape closes it and returns focus (611ms)
  ✓  50 [chromium] › tests/e2e/preview/library.spec.ts:217:1 › (4) confirm sends the consent for exactly the named objects and shows each result (657ms)
  ✓  51 [chromium] › tests/e2e/preview/host-indicator.spec.ts:297:1 › Privacy settings opens the Settings page, whose Privacy section has the full statement (756ms)
  ✓  52 [chromium] › tests/e2e/preview/library.spec.ts:254:1 › (4) cancel and Escape send nothing and return focus to the opener (648ms)
  ✓  54 [chromium] › tests/e2e/preview/library.spec.ts:277:1 › (4) a refused repair says nothing was changed and shows no result (715ms)
  ✓  53 [chromium] › tests/e2e/preview/host-indicator.spec.ts:333:1 › computed token values are exact in light and dark (UI-S-024, mockup 40) (1.0s)
  ✓  55 [chromium] › tests/e2e/preview/library.spec.ts:292:1 › (5) the Monitor's W010 note links to the library's repair offer (511ms)
  ✓  56 [chromium] › tests/e2e/preview/host-indicator.spec.ts:378:1 › the badge is reachable by keyboard and has a visible focus outline (781ms)
  ✓  57 [chromium] › tests/e2e/preview/library.spec.ts:345:1 › colours come from tokens in light and dark (1.0s)
  ✓  59 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:116:5 › light theme › the shell frame (UI-S-025): sidebar, header, footer, controls and strings (720ms)
  ✓  58 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:89:5 › light theme › Description (28): tab, filter, count, rows, chips, actions and Add property (1.7s)
  ✓  60 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:200:5 › light theme › the navigation tree as built (55, UI-S-017): eyebrows, 30 px rows, 16 px icons (559ms)
  ✓  62 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:238:5 › light theme › welcome (01, UI-S-027): title, lead, cards, privacy box and footer (713ms)
  ✓  63 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:294:5 › light theme › folder-choice unavailable (03) and the view states treatment (56) (565ms)
  ✓  61 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:224:5 › light theme › Add property (29): the dialog, the groups in the add order, one set and the new row (1.9s)
  ✓  64 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:329:5 › light theme › library page (41, UI-S-027): facts rows, actions and the W010 note (603ms)
  ✓  66 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:379:5 › light theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (710ms)
  ✓  65 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:379:5 › light theme › panel states (25): empty, loading, no match and error keep the header, tabs and filter (1.3s)
  ✓  67 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:424:5 › light theme › Monitor (31) and its OCFL validation section (54) (605ms)
  ✓  69 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:485:5 › light theme › host popover (40) and Settings (32): 372 px overlay, Appearance and About (712ms)
  ✓  68 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:447:5 › light theme › no match and keyboard: the filter, Escape in the dialog, and the focus ring (999ms)
  ✓  70 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:116:5 › dark theme › the shell frame (UI-S-025): sidebar, header, footer, controls and strings (793ms)
  ✓  72 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:200:5 › dark theme › the navigation tree as built (55, UI-S-017): eyebrows, 30 px rows, 16 px icons (567ms)
  ✓  71 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:89:5 › dark theme › Description (28): tab, filter, count, rows, chips, actions and Add property (1.6s)
  ✓  73 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:238:5 › dark theme › welcome (01, UI-S-027): title, lead, cards, privacy box and footer (535ms)
  ✓  75 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:294:5 › dark theme › folder-choice unavailable (03) and the view states treatment (56) (718ms)
  ✓  76 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:329:5 › dark theme › library page (41, UI-S-027): facts rows, actions and the W010 note (663ms)
  ✓  74 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:224:5 › dark theme › Add property (29): the dialog, the groups in the add order, one set and the new row (2.0s)
  ✓  77 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:379:5 › dark theme › repair consent (42) and results (43): dialog, scrim, chips and the refused box (660ms)
  ✓  79 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:424:5 › dark theme › Monitor (31) and its OCFL validation section (54) (827ms)
  ✓  78 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:379:5 › dark theme › panel states (25): empty, loading, no match and error keep the header, tabs and filter (1.2s)
  ✓  80 [chromium] › tests/e2e/preview/mockup-fidelity.spec.ts:485:5 › dark theme › host popover (40) and Settings (32): 372 px overlay, Appearance and About (661ms)
  ✓  82 [chromium] › tests/e2e/preview/navigation.spec.ts:62:1 › the live tree fills the shell's navigation slot, worded from the label table (428ms)
  ✓  81 [chromium] › tests/e2e/preview/metadata-panel.spec.ts:447:5 › dark theme › no match and keyboard: the filter, Escape in the dialog, and the focus ring (1.1s)
  ✓  83 [chromium] › tests/e2e/preview/navigation.spec.ts:83:1 › a level is read page by page through next_cursor until it is null (516ms)
  ✓  84 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:174:1 › (1) an E023 finding shows with its code beside the degraded health reason (615ms)
  ✓  85 [chromium] › tests/e2e/preview/navigation.spec.ts:121:1 › a destination opens with a breadcrumb filled from ancestors (643ms)
  ✓  86 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:200:1 › (2) the finding disappears after a clean revalidation's health.change, without a reload (520ms)
  ✓  87 [chromium] › tests/e2e/preview/navigation.spec.ts:142:1 › a deep link to a destination shows its breadcrumb after a reload (653ms)
  ✓  88 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:220:1 › (3) a W010 warning is listed apart from errors and health stays ok (553ms)
  ✓  89 [chromium] › tests/e2e/preview/navigation.spec.ts:154:1 › navigation.change reads only the named level again, without a reload (598ms)
  ✓  90 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:238:1 › (4) no location is rendered when the API withholds it (591ms)
  ✓  91 [chromium] › tests/e2e/preview/navigation.spec.ts:177:1 › a 503 shows the unavailable state with Retry, never an empty tree (660ms)
  ✓  92 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:250:1 › (5) the last and next run times come from the findings route; Validate now starts a run (719ms)
  ✓  94 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:289:1 › error and warning codes take their colours from tokens in light and dark (610ms)
  ✓  93 [chromium] › tests/e2e/preview/navigation.spec.ts:194:1 › computed token values of the tree and breadcrumb are exact in light and dark (965ms)
  ✓  95 [chromium] › tests/e2e/preview/production-build.spec.ts:63:1 › the built page is static, mounts the shell and defines every custom element (420ms)
  ✓  96 [chromium] › tests/e2e/preview/production-build.spec.ts:80:1 › sidebar, header and Settings words come from the active label table (UI-S-025, UI-S-033) (439ms)
  ✓  97 [chromium] › tests/e2e/preview/production-build.spec.ts:124:1 › computed token values are exact in light and dark (UI-S-024, CA-1) (391ms)
  ✓  98 [chromium] › tests/e2e/preview/production-build.spec.ts:146:1 › a refused health call shows the unavailable alert in Settings, About, not a console error (351ms)
  98 passed (50.0s)
```

### Pre-commit hook (ordinary run, `bash scripts/pre-commit.sh`, head commit)

```text
changed_scope: npm:apps/filesquid/ui (changed crates and npm suites: apps/filesquid/ui)
========================================
Running Pre-Commit Hooks... (test scope: npm:apps/filesquid/ui)
========================================
✅ Branch Validation passed.
🔍 Validating ./apps/filesquid/ui (Vite)...
✅ ./apps/filesquid/ui uniquely bound to 5173
✅ ./apps/filesquid/ui uniquely bound to 4173
🔍 Validating ./apps/imagesquid (Vite)...
✅ ./apps/imagesquid uniquely bound to 5174
✅ ./apps/imagesquid uniquely bound to 4174
🔍 Validating filesquid-server (default bind)...
✅ filesquid-server uniquely bound to 8083
✅ Traceability Validation passed: 96 PRD requirements, 84 resolved SRS references.
✅ Workspace Boundary validation passed.
✅ Logging rule validation passed (no relevant staged files).
⏭️  Skipped: Rust formatting in ./apps/filesquid (no Rust file changed; test scope: npm:apps/filesquid/ui)
🔍 Validating Prettier formatting in ./apps/filesquid/ui...
✅ ./apps/filesquid/ui is properly formatted.
🔍 Validating Prettier formatting in ./apps/imagesquid...
✅ ./apps/imagesquid is properly formatted.
✅ Format validation passed.
🔍 ADR structure (ISO 42010)...
✅ ADR structure (ISO 42010) passed.
🔍 PRD allocation...
✅ PRD allocation passed.
🔍 Plan traceability...
✅ Plan traceability passed.
🔍 Relative links and anchors...
✅ Relative links and anchors passed.
🔍 FileSquid and ImageSquid document boundary...
✅ FileSquid and ImageSquid document boundary passed.
🔍 MVP queue and merge driver tests...
✅ MVP queue and merge driver tests passed.
🔍 MVP progress bar...
✅ MVP progress bar passed.
🔍 Task cleanup...
✅ Task cleanup passed.
✅ Documentation validation passed.
✅ Change scope validation passed (188 assertions).
ℹ️  Test scope: npm:apps/filesquid/ui
🔍 ESLint (apps/filesquid/ui)...
✅ ESLint passed (4s).
🔍 TypeScript (apps/filesquid/ui)...
✅ TypeScript passed (6s).
🔍 Vitest (apps/filesquid/ui)...
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 401 (missing_credentials)
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
OCFL validation could not start ApiError: FileSquid API /api/v1/admin/ocfl/validation-runs failed with HTTP 403 (not_permitted)
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
stderr | tests/ui/details-panel.test.ts > <fs-details-panel> > hides Clear filter and forgets the filter while loading, failed or cleared
 Test Files  67 passed (67)
      Tests  1472 passed | 1 skipped (1473)
✅ Vitest passed (33s).
ℹ️  Playwright e2e skipped for apps/filesquid/ui (set FILESQUID_E2E=1 to run).
ℹ️  Desktop packaged-window e2e skipped (set FILESQUID_DESKTOP_E2E=1 to run).
✅ Lint and test validation passed (test scope: npm:apps/filesquid/ui).
========================================
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui (64s)
========================================
```
