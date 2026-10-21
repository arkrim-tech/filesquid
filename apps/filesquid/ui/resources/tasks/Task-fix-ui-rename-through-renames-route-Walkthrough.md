# Task fix-ui-rename-through-renames-route Walkthrough

## UI-S-002: Drive-style browser rename through `POST /api/v1/files/renames`

A fix, not a plan row; no plan box is ticked. Since PR #111 (API-F1.13, contract 0.20.0) `POST /api/v1/files/renames` sets only a file's display name, as a new version of its metadata record, and `POST /api/v1/files/moves` refuses (400) a move of a file object's stored original (`original/` of `files/<file-id>`). The default UI still renamed through the moves route, so renaming a file was refused. Requirements: [UI-S-002](../srs.md) (rename or move), [UI-S-019](../srs.md) (display names only), [API-S-027](../../../crates/api/resources/srs.md) (rename changes only the display name, as a new version of the metadata record). Scenarios: [`file_browser_UI-S-002.feature`](../../tests/features/file_browser_UI-S-002.feature).

## Changes

- **`FileSource.rename(library, fileId, displayName)`** ([`source.ts`](../../src/files/source.ts)): `ClientFileSource` sends one `POST /api/v1/files/renames` with the body typed by the generated `RenameRequest` and answers the generated `MetadataCommit` ([`model.ts`](../../src/files/model.ts) re-exports both from [`schema.d.ts`](../../src/api/schema.d.ts)). No route shape is hand-written. `move` stays for real moves (a folder's rename, a file in an object that is not a file's own).
- **Which route** ([`ops.ts`](../../src/files/ops.ts)): `fileIdOf(objectId)` reads the file id from `urn:uuid:<file-id>`; `renameFileId(row, objectId)` answers it only for a file row whose path is that file's own stored original, `original/<file-id>` with an optional dot and 1 to 16 ASCII letters or digits (`isOriginalPath`), in such an object. A collection snapshot is named `urn:uuid:` too, so a path alone under `original/` does not qualify. Everything else keeps the moves route.
- **Refusals** (`renameFailureText`): the route's codes, read from `openapi.json` (400 `invalid_request`, 404 `not_found`, 409 `conflict`, 503 `unavailable`; 401 and 403 are the denial labels), each have a label in the active table: `fileRenameInvalid`, `fileRenameNotFound`, `fileRenameConflict`, `fileRenameUnavailable`. Success is `fileRenamed`; the form title is `fileRenameOnlyTitle`; `fileDisplayNameTooLong` words the 1,024-byte limit. These seven new keys are required keys ([`labels.ts`](../../src/theme/labels.ts)) and are in the [FileSquid table](../../src/theme/labels.en.json) and in ImageSquid's own table.
- **Browser** ([`file-browser-writes.ts`](../../src/components/file-browser-writes.ts)): for such a row the form is titled "Rename ...", offers no folder field (the original never moves), checks the name against the route's limit of 1,024 bytes (`MAX_DISPLAY_NAME_BYTES`; a file or folder name keeps 255) and Save makes one rename call. The answer (`MetadataCommit`) carries a version and no name, so the listing is reloaded and the status shows the display name that listing returns, but only when the listing of the place the rename was asked in has loaded; otherwise it keeps the name typed. `fs-files-change` announces operation `rename`. A failed call shows its label and changes nothing.
- **`MemoryFileSource`** ([`memory-source.ts`](../../src/files/memory-source.ts)) answers as the routes now do: `rename` changes only the display name in a new version (400 for an empty name, one over 1,024 bytes or with `/` or NUL; 404 for a file the object does not hold; 409 for the current name), every listing and detail shows the API-S-005 safe form of a stored name, and `move` refuses a move from or to `original` or below it in a file's own object with 400 (an object counts as the file's own only when it holds that file's `original/<file-id>`).
- **Fixtures and mocks**: [`file-browser-fakes.ts`](../../tests/support/file-browser-fakes.ts) gains `fileObjectFixture()`; the Playwright fake ([`files-api.ts`](../../tests/e2e/support/files-api.ts)) answers `renames` from the source.
- **ImageSquid** (`apps/imagesquid`): it makes no file-route call (no source, no `move`, no `rename`; it consumes `@filesquid/ui` by `file:../filesquid/ui`), so no code changed. Its label table gains the seven new keys, because the keys are required of every distribution; its tests and preview e2e pass.
- **Docs**: [README](../../README.md) writes list, [feature](../../tests/features/file_browser_UI-S-002.feature) (the old rename scenario now covers moves; scenarios cover a file renamed by a move in a plain object, a rename through the renames route, and its refusal).

## SRS check

UI-S-002 and UI-S-006 list "rename or move" and "rename, move or copy" as separate actions and name no route, so no UI SRS text describes a rename as a move and the SRS is unchanged. The only text that described a rename as a move was the README writes list and the old Gherkin scenario, both updated.

## Findings for the platform engineer

- `FolderEntry` and `FileDetails` carry no `file_id`. The UI takes it from the shown object's identifier `urn:uuid:<file-id>` (API-S-026). If a library-level listing (a later UI task) lists files from many objects, the listing needs to carry each row's `file_id` and object, or the UI cannot rename from it.
- `MetadataCommit` carries no display name, so the UI rereads the listing after the call.

## TDD: failing first

The Vitest tests were written before the implementation and failed against it (16 failing across the four test files; excerpt):

```text
 FAIL  tests/unit/files-source.test.ts > ClientFileSource > rename is one JSON POST to the renames route with the contract's body
TypeError: (intermediate value).rename is not a function
 ❯ tests/unit/files-source.test.ts:296:55
 FAIL  tests/unit/files-source.test.ts > ClientFileSource > rename rejects with the route's 400 refusal (invalid_request)
 FAIL  tests/unit/files-source.test.ts > ClientFileSource > rename rejects with the route's 404 refusal (not_found)
 FAIL  tests/unit/files-source.test.ts > ClientFileSource > rename rejects with the route's 409 refusal (conflict)
 FAIL  tests/unit/files-source.test.ts > ClientFileSource > rename rejects with the route's 503 refusal (unavailable)
TypeError: (intermediate value).rename is not a function
 Test Files  4 failed (4)
      Tests  16 failed | 138 passed (154)
```

## Test results (raw)

### FileSquid UI: `npx vitest run` (`npm test`)

```text
 ✓ tests/ui/file-browser-writes.test.ts (45 tests) 1661ms
 ✓ tests/ui/file-browser.test.ts (37 tests) 1724ms
 ✓ tests/unit/files-source.test.ts (31 tests) 58ms
 ✓ tests/unit/files-memory-source.test.ts (34 tests) 59ms
 ✓ tests/unit/files-ops.test.ts (54 tests) 202ms
 Test Files  56 passed (56)
      Tests  1240 passed (1240)
```

### FileSquid UI: Playwright, both suites (`FILESQUID_E2E=1 npm run test:e2e`)

Server-backed suite (`playwright.config.ts`):

```text
  42 passed (31.1s)
```

Production build served by `vite preview` (`playwright.preview.config.ts`):

```text
  ✓   1 [chromium] › tests/e2e/preview/file-browser.spec.ts:96:1 › the browser is a defined element and lists display names in the list layout from the label table (392ms)
  ✓   4 [chromium] › tests/e2e/preview/file-browser.spec.ts:140:1 › a file row shows size, type hint, modified date and version (254ms)
  ✓   6 [chromium] › tests/e2e/preview/file-browser.spec.ts:152:1 › computed token values are exact in the browser in light and dark (349ms)
  ✓   8 [chromium] › tests/e2e/preview/file-browser.spec.ts:175:1 › opening a folder updates the breadcrumb and the address, and Back returns (459ms)
  ✓  12 [chromium] › tests/e2e/preview/file-browser.spec.ts:196:1 › an image previews through a content handle that is used only as the media src (412ms)
  ✓  14 [chromium] › tests/e2e/preview/file-browser.spec.ts:231:1 › a handle that fails is replaced once; a second failure shows the error and mints no third (307ms)
  ✓  16 [chromium] › tests/e2e/preview/file-browser.spec.ts:247:1 › a video resumes at the same position and paused state after one new handle (400ms)
  ✓  17 [chromium] › tests/e2e/preview/file-browser.spec.ts:283:1 › audio plays natively and is named by the display name (297ms)
  ✓  18 [chromium] › tests/e2e/preview/file-browser.spec.ts:296:1 › text previews through one ranged read of the first 64 KiB and says it is cut (349ms)
  ✓  20 [chromium] › tests/e2e/preview/file-browser.spec.ts:312:1 › a format it cannot show says so and shows no viewer (282ms)
  ✓  23 [chromium] › tests/e2e/preview/file-browser.spec.ts:325:1 › the viewer steps through files, closes with Escape and returns focus to the row (353ms)
  ✓  25 [chromium] › tests/e2e/preview/file-browser.spec.ts:338:1 › an upload is one PUT of the client's facts that creates the file's own object (307ms)
  ✓  27 [chromium] › tests/e2e/preview/file-browser.spec.ts:387:1 › a rename posts to the renames route, never to moves, and shows the new display name (380ms)
  ✓  28 [chromium] › tests/e2e/preview/file-browser.spec.ts:416:1 › a refused rename shows the label for its reason and keeps the display name (273ms)
  ✓  30 [chromium] › tests/e2e/preview/file-browser.spec.ts:435:1 › writes are single calls: new folder, move, remove and Undo (640ms)
  ✓  33 [chromium] › tests/e2e/preview/file-browser.spec.ts:482:1 › a refused write shows the label for its reason and changes nothing (312ms)
  ✓  35 [chromium] › tests/e2e/preview/file-browser.spec.ts:497:1 › a long folder is virtualised, pages in on demand and stays keyboard reachable (482ms)
  ✓  38 [chromium] › tests/e2e/preview/file-browser.spec.ts:537:1 › the layout does not scroll sideways at 320 px, at 200 % text size, or both, in list and grid (333ms)
  ✓  40 [chromium] › tests/e2e/preview/file-browser.spec.ts:554:1 › motion stops under reduced motion and focus is the 2px primary ring (301ms)
  ✓  42 [chromium] › tests/e2e/preview/file-browser.spec.ts:579:1 › a failed listing shows the error state with Try again, and retrying loads it (297ms)
  64 passed (18.4s)
```

### ImageSquid: `npm test` and `FILESQUID_E2E=1 npm run test:e2e`

```text
 Test Files  4 passed (4)
      Tests  37 passed (37)
server-backed suite:   9 passed (9.3s)
production-build suite:   5 passed (6.7s)
```

### Format, lint, types

`npx prettier --write .`, `npx eslint . --fix` and `npx tsc --noEmit` exit 0 in both packages.

### Hook: `bash scripts/pre-commit.sh`

```text
ℹ️  Test scope: npm:apps/filesquid/ui,apps/imagesquid
✅ Vitest passed (24s).
✅ Vitest passed (2s).
✅ Lint and test validation passed (test scope: npm:apps/filesquid/ui,apps/imagesquid).
✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui,apps/imagesquid (59s)
```
