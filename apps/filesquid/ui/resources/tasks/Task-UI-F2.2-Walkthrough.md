# Task UI-F2.2 Walkthrough: `<fs-file-browser>`

[Task UI-F2.2](../plan.md) is the UI slice of global [Task FS-2.2](../../../../../resources/implementation-plan.md#task-fs-2.2) (Drive-style browser and sidebar panels). It ships the browser, its preview area, the writes, the panel and preview-renderer slots, the content-handle media previews and the `#/files/...` route. UI-F2.3, UI-F2.4 and the rest of FS-2.2 remain, so the global box stays open.

Holistic review first ([holistic-ui.md](../../../../../.agents/rules/holistic-ui.md)): UI-F2.3, UI-F2.4 and UI-F2.15 mount panels in slots that receive the selected file and never depend on one another (UI-F2.15 puts three tabs inside one file panel with a Versions button), UI-F2.8 adds sorting, checkbox selection, menus, a folder picker and preferences "extending UI-F2.2", and UI-F2.9 reuses the row model for search results. So the browser owns no tab chrome, hands panels a `FileRef` through a `fileRef` property, builds its rows from one pure model (`FileRow`) with data-driven columns, keeps "the open file" separate from a later checkbox selection, and routes every write through a `FileSource` method that a menu or picker can call later.

## SRS mapping

| ID | Title | Covered by |
|---|---|---|
| [UI-S-002](../srs.md) | The default UI provides a Drive-style browser with navigation, breadcrumb, list and grid views, preview, upload, create folder, rename or move, remove and restore, and sidebar panel slots | [`file_browser_UI-S-002.feature`](../../tests/features/file_browser_UI-S-002.feature), `tests/ui/file-browser.test.ts`, `tests/ui/file-browser-writes.test.ts`, `tests/ui/files-route.test.ts`, `tests/e2e/preview/file-browser.spec.ts` |
| [UI-S-019](../srs.md) | Every component shows a file's display name from the API and never its stored name; downloads use the API's download name | `file-browser.test.ts` and `file-browser-writes.test.ts` scan every shadow root's text and attributes for the stored names after listing, folder navigation, preview, rename, remove and Undo; `files-model.test.ts` (the row carries the stored path only in `key` and `path`); the Playwright spec scans the built page the same way |
| [UI-S-014](../srs.md) | Client authorisation: a content handle is the only credential the UI may put in a URL, only as a media `src`; one new handle on a media error, resuming position and paused state | `tests/unit/files-source.test.ts` (`mintContentHandle`: authenticated POST, answer checked, no quote of the handle in an error), `tests/unit/media-handle.test.ts`, `tests/ui/file-preview.test.ts`, Playwright (real PNG, WebM and WAV decoded through handles; a video resumed at 1 s paused; a second failure shows the error with exactly two mints; handle absent from address, storage, cookie, history, links and text) |
| [UI-S-016](../srs.md) | Accessibility contracts | shared stylesheet first in each new shadow root (`component-styles.test.ts`), named `table`, `toolbar`, `region` and `complementary` landmarks and the audit in `file-browser.test.ts`, roving Tab stop and list keys, focus recovery, 320 px and 200 % text without sideways scroll, reduced motion, the 2px focus ring (Playwright) |
| [UI-S-001](../srs.md) | Components styled only through design tokens and worded only through the label table | `component-styles.test.ts` covers `fs-file-browser.ts`, `fs-file-preview.ts` and the three helper stylesheets; 62 new keys are in `REQUIRED_LABEL_KEYS`, both `labels.en.json` files and every assertion reads the table |
| [UI-S-005](../srs.md) | A distribution's label table defines every key of the default table | the 62 new keys are in `apps/imagesquid/src/theme/labels.en.json` (its tests pass) |
| [UI-S-013](../srs.md) | UI tests against production artifacts with strict DOM assertions | `tests/e2e/preview/file-browser.spec.ts` (17 tests) against `vite build` plus `vite preview` |
| [UI-S-006](../srs.md) | Drive-style views, sorting, selection and preferences (forward) | not built here by design: the row model, column table, `FileSource` writes and `fs-file-open` event are the hooks UI-F2.8 extends |
| [UI-S-022](../srs.md) | File panel tabs (forward) | `registerPanel` and the `fileRef` contract; each tab will be its own element mounted in a slot |
| [SRS-F-055](../../../../../resources/srs.md#srs-f-055-default-ui-and-panel-composition) / [PRD F-055](../../../../../resources/prd.md#f-055-default-ui-and-panel-composition) | Default UI and panel composition | `composable_filesquid_ui_F-055.feature` (three new scenarios: panels in named slots receiving only the file, renderer slot order, the browser mounting alone) and the tests above |

Constraint gates (the Plan's C1 to C22, each coded and tested):

| Gate | Where it is proved |
|---|---|
| C1 display name everywhere | `file-browser.test.ts` "never stored names, anywhere", `file-browser-writes.test.ts` "never shows a stored name after any write", Playwright first test |
| C2 list default, grid toggle with `aria-pressed` | `file-browser.test.ts` layout tests, Playwright |
| C3 columns name, size, type, modified, version | `file-browser.test.ts`, Playwright "a file row shows size, type hint, modified date and version" |
| C4 text preview `Range: bytes=0-65535`, truncated note | `file-preview.test.ts`, Playwright (`api.ranges` equals `["bytes=0-65535"]`) |
| C5 upload PUT, 64 MiB (67,108,864 bytes) refused before sending, exactly that accepted | `file-browser-writes.test.ts`, `files-ops.test.ts`, `files-model.test.ts` |
| C6 new folder POST, pending marker | `file-browser-writes.test.ts` (incl. opening a pending folder as empty), Playwright |
| C7 rename or move POST, name rules (1 to 255 bytes, no `.` `..` `/` `\` or control) | `files-model.test.ts`, `files-ops.test.ts`, `file-browser-writes.test.ts` |
| C8 remove POST, Undo restores from the version before, no delete forever | `file-browser-writes.test.ts`, Playwright (`"version":"v3"` in the restore body) |
| C9 committed version shown, listing refreshed | status messages assert the version from `FileCommit`; listing re-read after each commit |
| C10 panel slots and the `FileRef` contract | `file-browser.test.ts` (order, only the file, replace under the same id, cleared on close) |
| C11 preview-renderer slot order and fallback | `file-browser.test.ts`, `file-preview.test.ts` |
| C12 virtual window, 5 overscan rows, `3rem`/`9rem`, stable keys, aria row counts | `files-window.test.ts`, `file-browser.test.ts` (node reuse, focused row kept), Playwright (fewer than 60 rows of 1,000) |
| C13 pages of 200 on demand, 500-page cap with a note | `file-browser.test.ts` (page 2 on scroll, "Showing the first 200 items only" with `maxPages` 2) |
| C14, C15 handles only as media `src`; one re-mint, resume position and paused state | `files-source.test.ts`, `media-handle.test.ts`, `file-preview.test.ts`, Playwright |
| C16 words from the label table | all assertions read `labels.en.json`; sizes use `sizeB` to `sizeTB` |
| C17 tokens only, light and dark | `component-styles.test.ts`, Playwright computed colours (upload border = `--fs-primary`, row text = `--fs-on-surface`, table = `--fs-surface`) in both schemes |
| C18 accessibility | see UI-S-016 above |
| C19 five states with Retry | `file-browser.test.ts`, Playwright (error and Retry) |
| C20 no handle or token in the console | `media-handle.test.ts` spies every console level; Playwright `trackErrors` |
| C21 light and dark computed values | Playwright |
| C22 mounts alone; no direct module call | `file-browser.test.ts` first test; the element talks only to `FileSource` |

## What changed

- **`<fs-file-browser>`** ([`fs-file-browser.ts`](../../src/components/fs-file-browser.ts)) with four helpers kept to one job each: [`file-browser-frame.ts`](../../src/components/file-browser-frame.ts) (static DOM and stylesheet), [`file-list.ts`](../../src/components/file-list.ts) (the virtualised, keyed, roving-Tab-stop list and grid), [`file-row-view.ts`](../../src/components/file-row-view.ts) (row, tile and header DOM; `COLUMNS` is data) and [`file-browser-writes.ts`](../../src/components/file-browser-writes.ts) (upload, new folder, rename or move, remove, Undo, and the two live regions).
- **`<fs-file-preview>`** ([`fs-file-preview.ts`](../../src/components/fs-file-preview.ts)) and the renderer registry [`files/preview.ts`](../../src/files/preview.ts): built-in image, video, audio (content handles) and text (one ranged read), unsupported fallback, custom renderers first.
- **Files model and sources** ([`src/files/`](../../src/files/)): `model.ts` (types from the generated contract, limits, `FileRow`, names, sizes, dates, the location route, preview kinds), `window.ts`, `keys.ts`, `source.ts` (`FileSource`, `ClientFileSource`), `memory-source.ts` (fixture answering like the file routes, used by tests), `ops.ts` (batch upload, name rules, failure words), `media-handle.ts` (single re-mint controller).
- **`FileSquidClient`** ([`api/client.ts`](../../src/api/client.ts)): `mintContentHandle()` (authenticated `POST /api/v1/content-handles`; the answer is checked: 43 base64url characters and exactly the content route; returns the URL on the active host base; neither handle nor URL is ever in an error or log) and `putBytes()` (raw `PUT`, as the upload route takes).
- **Route** ([`app/routes.ts`](../../src/app/routes.ts), [`app/main.ts`](../../src/app/main.ts)): `bindFilesRoute` shows the browser for `#/files/<library>/<object>/<folder>/...`; entering a folder writes the address with `pushState`; Back follows.
- **Labels** (default and ImageSquid): 62 `size*` and `file*` keys; ImageSquid words three of them for photos.
- **Existing tests updated**: `component-styles.test.ts` lists the two new elements, `production-build.spec.ts` defines them.
- **Docs**: package [README](../../README.md) section "File browser", [plan.md](../plan.md) box and status, the root checklist box, two feature files.

## Decisions and gaps for the platform engineer

- **Rename vs API-S-027.** The only rename the file routes offer is `POST /files/moves`, which changes the logical path. API-S-027 says a rename changes only the display name, committed as a new version. The browser therefore renames by move (typing the name already shown keeps the stored last segment, so a move to another folder never renames), and shows whatever `display_name` the API returns afterwards. When a file's display name comes from its metadata record, a move will not change it. A rename route (or a `display_name` field on the move) is needed.
- **Scope of a listing.** The routes list one OCFL object's logical tree (`library_id`, `object_id`, folder prefix). The library-level file views of API-S-019 (all files, a collection, deleted files) and the navigation node handles of API-S-021 (API-F1.11) do not exist yet; they feed the same `FileLocation` when they do. Until then the browser is reached by `#/files/<library>/<object>`.
- **Deleted files and Undo.** The listing answers the head version only, so Undo works only right after a removal (it restores from the version before the commit). The Deleted files view of UI-F2.8 needs a way to list removed paths.
- **Download.** No whole-file download route exists (the ranged read is capped at 16 MiB per answer), so the Download and Show-in-Finder buttons of the unsupported-format mockup are left to UI-F2.8 together with the streamed ZIP.
- **Upload contract.** `FileSquidClient.upload()` (multipart with a `CommandEnvelope`, UI-S-012) does not match `PUT /files/upload` (octet-stream, identifying fields in the query), so the browser uses the new `putBytes()`. The platform engineer should confirm which of the two the SRS means for file uploads.
- **No file events.** The browser reloads after its own writes only; it does not follow other writers. A file-change event on the event channel would let it refresh (UI-F2.1b adds `navigation.change`).
- **Server-backed Playwright suite not run.** `npm run test:e2e` builds `filesquid-server` with Cargo and this task builds no Rust; its code is unchanged by this task. The static preview suite was run (see below). On this machine the installed Playwright (1.64.1) expects a different Chromium build than `/opt/pw-browsers` holds, so a local, uncommitted config pointing `executablePath` at the installed headless shell ran the same specs.

## How it was tested

TDD: the unit tests for the pure rules and the sources, and the UI tests for the preview and the browser, were written before any implementation. The first runs failed:

```
 FAIL  tests/unit/media-handle.test.ts [ tests/unit/media-handle.test.ts ]
Error: Failed to resolve import "../../src/files/media-handle" from "tests/unit/media-handle.test.ts". Does the file exist?
 Test Files  5 failed (5)
      Tests  no tests
```

and, for the browser element tests (raw head and tail in "Raw test output" below), 58 of 58 failed with `browser.configure is not a function`. The route binder test was written together with `bindFilesRoute`, and the Playwright spec after the element worked in jsdom; both ran green on their first full run apart from locator ambiguities in the spec (a strict-mode clash between the browser's `.status` region and the `<fs-states>` region inside it, and a video the fake served without byte ranges), which were fixed in the test support.

Layers:

- **Unit** (`tests/unit/files-*.test.ts`, `media-handle.test.ts`): rows and names, size and date text, location route, preview kinds, virtual window and keys, `ClientFileSource` calls (exact query and body of each route), the client's `mintContentHandle` and `putBytes`, the memory source, batch upload rules, the single re-mint controller.
- **jsdom UI** (`tests/ui/file-*.test.ts`, `files-route.test.ts`): listing, layouts, navigation, viewer, panel and renderer slots, writes, states, virtual window, keyboard, accessibility audit.
- **Production build** (`tests/e2e/preview/file-browser.spec.ts`): `vite build` plus `vite preview`; the file routes answered by the test from an object that answers as the API does; real media decoded through handles; strict DOM assertions as above.
- **Gherkin**: [`file_browser_UI-S-002.feature`](../../tests/features/file_browser_UI-S-002.feature) written with the feature-writer skill (happy paths, edges, errors, interactions) and three scenarios added to `composable_filesquid_ui_F-055.feature`; the existing content-handle scenarios of `client_authorisation_UI-S-014.feature` are the ones the media tests implement.
- No coverage tool is configured in this package, so no coverage figure is reported.

Mocked environment variables: none.

## Raw test output

### Red run of the browser element tests (apps/filesquid/ui)

```

 RUN  v4.1.12 /home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui

 ❯ tests/ui/file-browser-writes.test.ts (25 tests | 25 failed) 52ms
     × uploads each chosen file with one PUT into the current folder and reports the committed version 13ms
     × refuses a file over 64 MiB before sending it and says which 2ms
     × accepts a file of exactly 64 MiB 1ms
     × names the files that failed, keeps the ones that committed and offers to try again 1ms
     × shows a drop target for dragged files and uploads what is dropped 2ms
     × ignores a drag that carries no files 4ms
     × strips path separators from the name it uploads under 1ms
     × opens a named form with focus in the field, creates the folder and shows it pending 1ms
     × says when the folder already exists 2ms
     × refuses an unusable name in the form without calling the API 1ms
     × cancels with Escape or Cancel and returns focus to the control that opened it 1ms
     × shows a refusal with its label and keeps the form open 1ms
     × renames a file in place with one move that keeps its folder 1ms
     × moves a file to another folder by naming the folder 1ms
     × renames a folder, moving everything below it 4ms
     × refuses an unusable name or folder segment without calling the API 1ms
     × does nothing when no row is chosen 1ms
     × opens from F2 on a row 1ms
...
(793 lines omitted)
...
     67|     source,
 ❯ tests/ui/file-browser.test.ts:465:27

⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯⎯[58/58]⎯


 Test Files  2 failed (2)
      Tests  58 failed (58)
   Start at  02:31:24
   Duration  1.89s (transform 1.40s, setup 168ms, import 1.48s, tests 127ms, environment 1.34s)

JUNIT report written to /home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

### Lint, format and types (apps/filesquid/ui)

```
Checking formatting...
All matched files use Prettier code style!
prettier exit 0
eslint exit 0
tsc exit 0
```

### npm test (apps/filesquid/ui)

```

> @filesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @filesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 /home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui

 ✓ tests/ui/nav-tree.test.ts (26 tests) 937ms
 ✓ tests/ui/file-browser-writes.test.ts (27 tests) 1218ms
 ✓ tests/ui/file-browser.test.ts (33 tests) 1569ms
     ✓ is a defined custom element that mounts alone, without panels or a distribution  343ms
 ✓ tests/unit/component-styles.test.ts (126 tests) 71ms
stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the error state and no host when the descriptor cannot be read
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at /home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/tests/ui/host-indicator.test.ts:205:46
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the denied state for an authorisation denial
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 401 (missing_credentials)
    at /home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/tests/ui/host-indicator.test.ts:214:20
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 401,
  path: '/api/v1/host',
  code: 'unauthorized',
  reason: 'missing_credentials',
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the offline state with Retry when the read fails while the browser is offline
FileSquid host descriptor could not be read TypeError: Failed to fetch
    at /home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/tests/ui/host-indicator.test.ts:233:29
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > reads again when Retry is pressed and recovers
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at /home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/tests/ui/host-indicator.test.ts:245:29
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> follows the host > reads again when a host.change payload breaks the contract
FileSquid host.change payload broke the contract; reading the descriptor

 ✓ tests/ui/host-indicator.test.ts (27 tests) 277ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 522ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 69ms
 ✓ tests/unit/client-io.test.ts (22 tests) 66ms
 ✓ tests/unit/theme.test.ts (26 tests) 32ms
 ✓ tests/unit/files-source.test.ts (19 tests) 37ms
 ✓ tests/ui/monitor.test.ts (14 tests) 232ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 39ms
stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows the error state with Try again when the details cannot be read, and retries
FileSquid could not preview a file Error

stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows permission denied with the reason's label
FileSquid could not preview a file ApiError

 ✓ tests/ui/file-preview.test.ts (13 tests) 153ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 37ms
 ✓ tests/ui/app-shell.test.ts (18 tests) 444ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 111ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 41ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 33ms
 ✓ tests/unit/media-handle.test.ts (11 tests) 82ms
 ✓ tests/unit/files-model.test.ts (52 tests) 48ms
 ✓ tests/unit/commands.test.ts (45 tests) 37ms
 ✓ tests/unit/host-descriptor.test.ts (43 tests) 42ms
 ✓ tests/unit/files-memory-source.test.ts (13 tests) 33ms
 ✓ tests/unit/files-ops.test.ts (20 tests) 91ms
 ✓ tests/unit/focus.test.ts (16 tests) 89ms
 ✓ tests/unit/files-window.test.ts (17 tests) 23ms
 ✓ tests/ui/states.test.ts (16 tests) 105ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 15ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 76ms
 ✓ tests/unit/test-scripts.test.ts (10 tests) 982ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 99ms
 ✓ tests/unit/admin-client.test.ts (6 tests) 19ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 13ms
 ✓ tests/unit/client.test.ts (8 tests) 21ms
 ✓ tests/unit/preferences.test.ts (10 tests) 19ms
 ✓ tests/ui/files-route.test.ts (5 tests) 444ms
     ✓ shows the browser at the folder the hash names, now and after a hash change  311ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 74ms
 ✓ tests/unit/view-state.test.ts (4 tests) 10ms
 ✓ tests/unit/contrast.test.ts (13 tests) 26ms
 ✓ tests/unit/host-client.test.ts (4 tests) 14ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 28ms
 ✓ tests/unit/distribution.test.ts (3 tests) 8ms
 ✓ tests/unit/contract.test.ts (1 test) 153ms

 Test Files  43 passed (43)
      Tests  848 passed (848)
   Start at  02:48:42
   Duration  16.71s (transform 2.80s, setup 1.87s, import 5.15s, tests 8.44s, environment 24.44s)

JUNIT report written to /home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

### Playwright on the production build (apps/filesquid/ui, `vite build` plus `vite preview`)

```

Running 33 tests using 2 workers

  ✓   1 [chromium] › tests/e2e/preview/host-indicator.spec.ts:86:1 › the indicator is a defined custom element in the shell header, named from the label table (306ms)
  ✓   2 [chromium] › tests/e2e/preview/file-browser.spec.ts:88:1 › the browser is a defined element and lists display names in the list layout from the label table (399ms)
  ✓   3 [chromium] › tests/e2e/preview/host-indicator.spec.ts:100:1 › the desktop host says only you can read your files, and shows the not-enrolled state (246ms)
  ✓   4 [chromium] › tests/e2e/preview/file-browser.spec.ts:130:1 › a file row shows size, type hint, modified date and version (317ms)
  ✓   5 [chromium] › tests/e2e/preview/host-indicator.spec.ts:114:1 › a server names its operator, who can read the files, and states the at-rest duty (226ms)
  ✓   7 [chromium] › tests/e2e/preview/host-indicator.spec.ts:127:1 › a server without an operator name uses the unnamed-operator label (212ms)
  ✓   6 [chromium] › tests/e2e/preview/file-browser.spec.ts:142:1 › computed token values are exact in the browser in light and dark (455ms)
  ✓   8 [chromium] › tests/e2e/preview/host-indicator.spec.ts:139:1 › the browser host says files are decrypted only in this browser (235ms)
  ✓  10 [chromium] › tests/e2e/preview/host-indicator.spec.ts:152:1 › enrolled backup and sync are shown apart from the host, as ciphertext only (239ms)
  ✓   9 [chromium] › tests/e2e/preview/file-browser.spec.ts:165:1 › opening a folder updates the breadcrumb and the address, and Back returns (509ms)
  ✓  11 [chromium] › tests/e2e/preview/host-indicator.spec.ts:177:1 › a host.change event updates the indicator without a reload or another read (257ms)
  ✓  13 [chromium] › tests/e2e/preview/host-indicator.spec.ts:199:1 › events.resync makes the indicator read the descriptor again (233ms)
  ✓  12 [chromium] › tests/e2e/preview/file-browser.spec.ts:186:1 › an image previews through a content handle that is used only as the media src (409ms)
  ✓  15 [chromium] › tests/e2e/preview/file-browser.spec.ts:220:1 › a handle that fails is replaced once; a second failure shows the error and mints no third (321ms)
  ✓  16 [chromium] › tests/e2e/preview/file-browser.spec.ts:236:1 › a video resumes at the same position and paused state after one new handle (445ms)
  ✓  14 [chromium] › tests/e2e/preview/host-indicator.spec.ts:212:1 › loading shows no host until the descriptor arrives (1.0s)
  ✓  17 [chromium] › tests/e2e/preview/file-browser.spec.ts:272:1 › audio plays natively and is named by the display name (447ms)
  ✓  18 [chromium] › tests/e2e/preview/host-indicator.spec.ts:224:1 › a failed read shows the error state and never a guessed host, then Retry recovers (327ms)
  ✓  20 [chromium] › tests/e2e/preview/host-indicator.spec.ts:242:1 › computed token values are exact in light and dark (275ms)
  ✓  19 [chromium] › tests/e2e/preview/file-browser.spec.ts:285:1 › text previews through one ranged read of the first 64 KiB and says it is cut (390ms)
  ✓  21 [chromium] › tests/e2e/preview/host-indicator.spec.ts:267:1 › the indicator is reachable by keyboard and has a visible focus outline (284ms)
  ✓  22 [chromium] › tests/e2e/preview/file-browser.spec.ts:299:1 › a format it cannot show says so and shows no viewer (330ms)
  ✓  23 [chromium] › tests/e2e/preview/production-build.spec.ts:58:1 › the built page is static, mounts the shell and defines every custom element (263ms)
  ✓  24 [chromium] › tests/e2e/preview/file-browser.spec.ts:312:1 › the viewer steps through files, closes with Escape and returns focus to the row (429ms)
  ✓  25 [chromium] › tests/e2e/preview/production-build.spec.ts:75:1 › navigation, API status and theme legend words come from the active label table (263ms)
  ✓  27 [chromium] › tests/e2e/preview/production-build.spec.ts:93:1 › computed token values are exact in light and dark (270ms)
  ✓  28 [chromium] › tests/e2e/preview/production-build.spec.ts:109:1 › a refused health call shows the unavailable alert, not a console error from the app (239ms)
  ✓  26 [chromium] › tests/e2e/preview/file-browser.spec.ts:325:1 › writes are single calls: upload, new folder, rename, remove and Undo (785ms)
  ✓  29 [chromium] › tests/e2e/preview/file-browser.spec.ts:382:1 › a refused write shows the label for its reason and changes nothing (250ms)
  ✓  30 [chromium] › tests/e2e/preview/file-browser.spec.ts:397:1 › a long folder is virtualised, pages in on demand and stays keyboard reachable (437ms)
  ✓  31 [chromium] › tests/e2e/preview/file-browser.spec.ts:437:1 › the layout does not scroll sideways at 320 px, at 200 % text size, or both, in list and grid (280ms)
  ✓  32 [chromium] › tests/e2e/preview/file-browser.spec.ts:454:1 › motion stops under reduced motion and focus is the 2px primary ring (303ms)
  ✓  33 [chromium] › tests/e2e/preview/file-browser.spec.ts:479:1 › a failed listing shows the error state with Try again, and retrying loads it (252ms)

  33 passed (15.1s)
exit 0
```

### Lint, format and types (apps/imagesquid)

```
Checking formatting...
All matched files use Prettier code style!
prettier exit 0
eslint exit 0
tsc exit 0
```

### npm test (apps/imagesquid)

```

> @imagesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @imagesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 /home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/imagesquid

 ✓ tests/unit/test-reporting.test.ts (12 tests) 25ms
 ✓ tests/unit/theme.test.ts (10 tests) 20ms
 ✓ tests/ui/distribution.test.ts (5 tests) 206ms
 ✓ tests/unit/test-scripts.test.ts (10 tests) 992ms

 Test Files  4 passed (4)
      Tests  37 passed (37)
   Start at  02:49:29
   Duration  2.08s (transform 1.03s, setup 262ms, import 1.16s, tests 1.24s, environment 2.21s)

JUNIT report written to /home/user/filesquid/.claude/worktrees/agent-a85044533a2959408/apps/imagesquid/tests/allure-results/vitest-junit.xml
```

### Playwright on the production build (apps/imagesquid)

```

Running 4 tests using 1 worker

  ✓  1 [chromium] › tests/e2e/preview/production-build.spec.ts:54:1 › the built page is static, mounts the shell and defines every custom element (264ms)
  ✓  2 [chromium] › tests/e2e/preview/production-build.spec.ts:71:1 › navigation, API status and theme legend words come from the active label table (156ms)
  ✓  3 [chromium] › tests/e2e/preview/production-build.spec.ts:89:1 › computed ImageSquid token values are exact in light and dark (167ms)
  ✓  4 [chromium] › tests/e2e/preview/production-build.spec.ts:105:1 › a refused health call shows the unavailable alert (145ms)

  4 passed (6.8s)
exit 0
```

