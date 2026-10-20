# Task fix-ui-upload-new-contract Walkthrough

## UI-S-002: Drive-style browser upload on the upload query of contract 0.18.0

A fix, not a plan row; no plan box is ticked. Since PR #108 (API-F1.4, contract 0.18.0, owner decision "A+3a" of 2026-10-20) `PUT /api/v1/files/upload` takes the file's facts instead of a logical path and refuses any other query key, so the default UI's uploads, which still sent `object_id` and `path`, were refused with 400. Requirements: [UI-S-002](../srs.md) (Drive-style browser with upload), [UI-S-019](../srs.md) (display names only), [API-S-026](../../../crates/api/resources/srs.md) (an upload creates `files/<file-id>`, never at a client-chosen path) and [API-S-035](../../../crates/api/resources/srs.md) (the facts the client provides, and the normalisation form it MUST state for a name that is not ASCII). Scenarios: [`file_browser_UI-S-002.feature`](../../tests/features/file_browser_UI-S-002.feature).

## Changes

- **`FileSquidClient.uploadFile(query, body)`** ([`client.ts`](../../src/api/client.ts)): `PUT /api/v1/files/upload` with the generated `FileUploadQuery` (without `size`, which is always the body's size) and the `UploadCommit` answer. Undefined optional facts are not sent; nothing else is dropped.
- **`FileSource.upload(library, facts, body)`** ([`source.ts`](../../src/files/source.ts), [`model.ts`](../../src/files/model.ts)): takes the library and `UploadFacts` (`name`, `name_normalization`, `media_type`, `last_modified`, `file_id`) and answers `UploadCommit` (with `file_id`). No object or path.
- **Facts** ([`ops.ts`](../../src/files/ops.ts)): `uploadFacts(file, name)` states the name (still cleaned by `uploadName` and limited to 255 bytes), its form (`nameNormalization`: `nfc`, else `nfd`, else `unnormalised`, stated for every name), the media type without parameters when both parts are RFC 6838 restricted names (`reportedMediaType`, `isMediaType`), and the last-modified time as RFC 3339 UTC when its year has four digits (`reportedModified`). A fact the browser does not give in that form is left out, so the API records it as unavailable.
- **Browser** ([`file-browser-writes.ts`](../../src/components/file-browser-writes.ts)): unchanged behaviour: one `PUT` per file, files one at a time, one version per file, the 64 MiB, 255-byte and 1,000-file limits, retry. The commit is in the file's own object, so `<fs-file-browser>` announces it (`fs-files-change`) without moving the shown object's head.
- **`MemoryFileSource`** ([`memory-source.ts`](../../src/files/memory-source.ts)) answers as the route: a new file object `urn:uuid:<file-id>` at `v1`, the next version for a known `file_id`, `not_found` for an unknown one, `conflict` for unchanged bytes, `invalid_request` for a bad name, media type, time or a non-ASCII name without its form; `uploads` lists what was uploaded.
- **Playwright fake** ([`files-api.ts`](../../tests/e2e/support/files-api.ts)) refuses an unknown upload key (the old `object_id` and `path` among them), a missing `name` or `size`, and a declared size that is not the body's, with 400 as the API does.
- **Docs**: [UI SRS](../srs.md) UI-S-002, [plan](../plan.md) UI-F2.2 status, [README](../../README.md), [feature](../../tests/features/file_browser_UI-S-002.feature).

## TDD: failing first

The tests were written before the implementation and failed against it (46 failing):

```text
     × declares the body's size and sends only the facts that are set, as the contract's query 8ms
     × rejects with ApiError when the upload is refused 2ms
     × uploads one file as one PUT with the client's facts in the query and no path 7ms
     × names the file whose content an upload replaces with file_id 2ms
     × states the name, its normalisation form, the media type and the last-modified time 8ms
     × names "Café.txt" as nfd 0ms
     × names "éé.txt" as unnormalised 0ms
     × reports the media type "text/plain;charset=utf-8" as "text/plain" 1ms
     × reports the last-modified time 253402300800000 as undefined 4ms
     × uploads one file at a time as its own file, in order, with its facts, reporting progress 7ms
     × sends the cleaned display name, never a path 1ms
     × an upload creates the file's own object at v1 and leaves the object shown as it was 15ms
     × an upload naming a file_id is a new version of that file; unchanged bytes are a conflict 2ms
     × refuses a name that is not ASCII without its form as invalid_request without a version 2ms
     × uploads each chosen file with one PUT as its own file and reports the committed version 210ms
     × tries the failed files again in the library they were meant for, after moving away 29ms
     × strips path separators from the name it uploads under 16ms
⎯⎯⎯⎯⎯⎯ Failed Tests 46 ⎯⎯⎯⎯⎯⎯⎯
 Test Files  4 failed (4)
      Tests  46 failed | 82 passed (128)
```

## Results

`npm test` (Vitest, whole package):

```text
 Test Files  49 passed (49)
      Tests  1074 passed (1074)
   Duration  21.16s (transform 3.48s, setup 2.17s, import 6.46s, tests 15.92s, environment 28.06s)
```

Upload tests, verbose (excerpt):

```text
 ✓ tests/unit/files-source.test.ts > FileSquidClient.uploadFile > declares the body's size and sends only the facts that are set, as the contract's query 1ms
 ✓ tests/unit/files-source.test.ts > FileSquidClient.uploadFile > rejects with ApiError when the upload is refused 2ms
 ✓ tests/unit/files-source.test.ts > ClientFileSource > uploads one file as one PUT with the client's facts in the query and no path 1ms
 ✓ tests/unit/files-source.test.ts > ClientFileSource > names the file whose content an upload replaces with file_id 1ms
 ✓ tests/unit/files-ops.test.ts > uploadFacts > states the name, its normalisation form, the media type and the last-modified time 3ms
 ✓ tests/unit/files-ops.test.ts > uploadFacts > names "Café.txt" as nfd 1ms
 ✓ tests/unit/files-ops.test.ts > uploadFacts > names "éé.txt" as unnormalised 0ms
 ✓ tests/unit/files-ops.test.ts > uploadFacts > reports the media type "text/plain;charset=utf-8" as "text/plain" 0ms
 ✓ tests/unit/files-ops.test.ts > uploadFacts > reports the last-modified time 253402300800000 as undefined 0ms
 ✓ tests/unit/files-ops.test.ts > uploadFacts > leaves out a media type or time it cannot report 1ms
 ✓ tests/unit/files-ops.test.ts > uploadFiles > uploads one file at a time as its own file, in order, with its facts, reporting progress 10ms
 ✓ tests/unit/files-ops.test.ts > uploadFiles > sends the cleaned display name, never a path 1ms
 ✓ tests/unit/files-ops.test.ts > uploadFiles > refuses a name longer than 255 bytes without sending it 2ms
 ✓ tests/unit/files-memory-source.test.ts > MemoryFileSource writes > an upload creates the file's own object at v1 and leaves the object shown as it was 4ms
 ✓ tests/unit/files-memory-source.test.ts > MemoryFileSource writes > an upload naming a file_id is a new version of that file; unchanged bytes are a conflict 2ms
 ✓ tests/ui/file-browser-writes.test.ts > upload > uploads each chosen file with one PUT as its own file and reports the committed version 227ms
 ✓ tests/ui/file-browser-writes.test.ts > upload > tries the failed files again in the library they were meant for, after moving away 34ms
 ✓ tests/ui/file-browser-writes.test.ts > upload > strips path separators from the name it uploads under 14ms
 Test Files  4 passed (4)
      Tests  128 passed (128)
```

Playwright, production build served by `vite preview` (`playwright.preview.config.ts`, run with a temporary copy of that config pointing Chromium at the preinstalled browser; the copy was deleted before commit):

```text
  ✓  26 [chromium] › tests/e2e/preview/file-browser.spec.ts:338:1 › an upload is one PUT of the client's facts that creates the file's own object (369ms)
  ✓  27 [chromium] › tests/e2e/preview/file-browser.spec.ts:370:1 › writes are single calls: new folder, rename, remove and Undo (921ms)
  ✓  30 [chromium] › tests/e2e/preview/file-browser.spec.ts:417:1 › a refused write shows the label for its reason and changes nothing (478ms)

  50 passed (21.3s)
```

The server-backed suite (`playwright.config.ts`) builds and runs `filesquid-server` with Cargo and has no upload test; it was not run for this npm-only fix.

## Cubic round 1

The fixes for round 1's 15 findings: `MemoryFileSource` checks the calendar date and clock of a last-modified time as the API's `is_rfc3339_utc` does (leap day and `23:59:60` accepted), refuses a malformed `file_id` and a body over 64 MiB with `invalid_request`, and advances `created` on every upload. `ClientFileSource` always uses the library it is given. The Playwright fake refuses a missing `library_id` and an unknown `name_normalization`. The UI SRS spells out the name cleaning, the restricted-name rule and the time conversion, and the README, plan and feature state the known limitation. New tests failed first (7), then passed:

```text
     × always uploads into the library it is given, whatever the facts hold 15ms
     × refuses a date that is not in the calendar as invalid_request without a version 28ms
     × refuses a clock that is out of range as invalid_request without a version 7ms
     × refuses a leap second that is not 23:59:60 as invalid_request without a version 17ms
     × refuses a file_id that is not a UUID v4 as invalid_request without a version 8ms
     × refuses a body over 64 MiB as invalid_request without reading it 1ms
     × gives every upload a later created time, a new version of a file included 2ms
      Tests  7 failed | 95 passed (102)
```

```text
 Test Files  49 passed (49)
      Tests  1084 passed (1084)

  50 passed (21.0s)
```

## Open points for the platform engineer

- An upload now lands in its own object, not in the folder shown, and the API records no home collection or folder for it, so a new upload does not appear in the shown listing or in the navigation tree. The browser reports the committed `v1` and says nothing else; the drop target still reads "Drop to upload into {folder}". Placing an upload (a home collection or folder in the snapshot, or a navigation listing of file objects) needs an API change and an owner decision on the wording.
- The API plan's API-F1.4 status still says the UI uploads to a logical path; this fix resolves that, and the line can be updated with the next API change.
