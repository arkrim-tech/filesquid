# Task UI-1.1 Walkthrough: Library create/open/recover UI

[Task UI-1.1](../plan.md) is the last open module row of global [Task 1.1](../../../../../resources/implementation-plan.md#task-1.1) (CL-1.1, API-F1.12, DE-1.1, IO-1.1, IO-1.4 and API-F2.7 are done), so global Task 1.1 is ticked with it. `@filesquid/ui` now has the library flow: create a library, open one, recover from a copied snapshot, copy a library, rebuild its catalog from files, and the consented version-inventory repair. Every distribution gets it, and ImageSquid wires it in its own vocabulary.

Hook summary line on the head commit: `All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui,apps/imagesquid`

## SRS mapping

| ID | Title | Covered by |
|---|---|---|
| [SRS-F-001](../../../../../resources/srs.md#srs-f-001-independent-local-library-and-ownership) | Independent local library and ownership | [`independent_local_library_and_ownership_F-001.feature`](../../tests/features/independent_local_library_and_ownership_F-001.feature) (ten new scenarios); `tests/ui/library-start.test.ts`, `tests/ui/library-detail.test.ts`, `tests/e2e/preview/library.spec.ts` |
| [API-S-034](../../../crates/api/resources/srs.md) | Library location choices (the folder reaches the library routes only as an opaque location ID; `cancelled` changes nothing; `503 unavailable` from a host without a chooser) | `tests/unit/library-client.test.ts` (purposes, `location_id`, `destination_location_id`), `tests/ui/library-start.test.ts` (cancel, 503 with reason and Retry), preview criteria (1) to (3) |
| [API-S-025](../../../crates/api/resources/srs.md) | Local library and repair routes (opaque identifiers, no path in answers, refusals) | `tests/unit/library-client.test.ts` (routes and bodies from the generated types, contract checks of every answer), `tests/unit/library-errors.test.ts` (409, 404, 400, 403, 503 worded), no path in any request or in the page (preview criterion 1) |
| [API-S-023](../../../crates/api/resources/srs.md) | Version-inventory repair runs only through an explicit API command after the owner's consent | `tests/unit/library-model.test.ts` (`consentRequest`), `tests/ui/library-repair.test.ts`, preview criterion (4) (confirm, cancel, Escape, refusal) and (5) (Monitor link) |
| [UI-S-005](../srs.md) | A distribution's label table MUST define every key of the default table | 67 new keys in `REQUIRED_LABEL_KEYS` and in both tables; ImageSquid tests pass |
| [UI-S-012](../srs.md) | The UI reaches FileSquid only through `FileSquidClient` | `ClientLibrarySource` over `client.request`; no other transport |
| [UI-S-013](../srs.md) | UI tests run against production artifacts with strict DOM assertions | preview Playwright suite (below), for FileSquid and ImageSquid |
| [UI-S-016](../srs.md) | Accessibility contracts | stylesheet first, native controls, accessibility audit in each element test, modal focus handling (Cancel first, Tab kept inside, Escape, return to opener), route change focuses `main` |

## Constraint gates

| Constraint (from the Plan) | Code | Test |
|---|---|---|
| 1 purposes `create`, `open`, `copy_destination` to location-choices | `ClientLibrarySource.chooseLocation`; start and detail pages | "asks the location-choices route for purpose %s"; "asks with purpose open"; preview (1) |
| 2 `location_id` to create and open; `destination_location_id` with `library_id` to snapshot-copies; no `library_id` sent on create | `create`, `open`, `copy` | "passes the location ID as location_id…", "passes the destination as destination_location_id…"; preview (1) |
| 3 `cancelled` sends nothing further | `FsLibraryStart.choose`, `FsLibraryDetail.chooseDestination` | "cancel: changes nothing…" (start and detail); preview (2) |
| 4 503 is the unavailable state with its reason and Retry; choices stay | `libraryFailure` (`choose`), `ViewState` `unavailable.reason`, `<fs-states>` | "503: shows the unavailable state…", "Retry asks for the folder again…"; preview (3) |
| 5 no path; ID limits | no path field in any type; `parseLocationChoice` (128 bytes), `parseLibraryHash` (128 bytes), `consentRequest` (1,024 bytes) | "never renders a filesystem path"; model and client tests; preview (1) checks every request and the body text |
| 6 409, 404, 403 messages | `libraryFailure` | `library-errors.test.ts`; start and repair UI tests |
| 7 an already open library is answered again | `open` handles a plain answer | "answers an already open library again without an error" |
| Recover: open, then rebuild; report; nothing repaired | `FsLibraryStart.enter` and `FsLibraryStart.rebuild`, `rebuildReport` | "recover: opens the chosen copy, rebuilds…", preview recover test (no `/repairs` request) |
| Retry resumes from the failed step: a chosen folder is not asked for again (a second registration of an open folder is refused as in use) | `FsLibraryStart.resume` (`Progress`), `FsLibraryDetail.run` (kept destination) | "Retry after a failed open reuses the chosen folder…", "Retry after a failed rebuild of a recovered copy…", "copy: Retry after a failed copy reuses…", and the two "starts afresh" tests; Gherkin "Retry after a failed step keeps the folder already chosen" |
| Copy: report with `invalid_objects` codes | `copyReport` | "copy: asks for a destination…", "copy: says a clean copy validated…"; preview copy test |
| 8 W010 note "can be read but not changed" | `FsLibraryDetail.loadCandidates`, `libraryReadOnly` and `libraryReadOnlyPartial` | "says that files can be read but not changed (W010)…", "words a truncated candidate list as at least" |
| 9 consent dialog names the objects, one copy of root inventory and sidecar, latest version directory, nothing else | `FsLibraryRepair.openDialog`, `repairConsent` | "opens a modal dialog that names the objects…"; preview (4) |
| 10 only Confirm calls the route; Cancel and Escape send nothing | `confirm`, `closeDialog`, `keepInside` | "Cancel closes the dialog…", "Escape cancels…", "Confirm calls the repair route once…"; preview (4) |
| 11 consent body exactly as specified | `consentRequest` | `library-model.test.ts` (1 to 1,000, distinct, bytes), "Confirm calls…", preview (4) body equality and the fake route's own consent checks |
| 12 candidates, truncated, sha512, interrupted | `candidateText`, `showOffer` | "is a defined element that says…", "says when more objects need repair…" |
| 13 result per object | `outcomeText`, `resultsSection` | "shows the result per object…", "names a provenance record that was not written" |
| 14 refused repair says nothing was changed | `libraryFailure` (`repair`) | "a refused repair (%#) says nothing was changed" (403, 409, 404, 503); preview (4) |
| 15 `repairHref` | `libraryRepairHref` passed in both `main.ts` files | `library-route.test.ts`; preview (5) clicks the Monitor's link and reaches the offer; the existing W010 preview test now expects the link |
| 16 labels and tokens, light and dark | `LIBRARY_STYLE` tokens only | preview "colours come from tokens in light and dark" (heading, card, dialog, primary button, Cancel border, unavailable title) |
| 17 UI-S-016 | `pageStyle`, native `button` and `a`, `focusElement` | accessibility audit in each element test; preview Tab and Escape test |
| 18 production build, strict DOM | Playwright preview suite | 12 new tests; elements defined, labels from the table, no console errors |
| 19 ImageSquid | both label tables, `apps/imagesquid/src/main.ts` | ImageSquid Vitest and preview tests (new library test) |
| 20 no swallowed errors | `console.warn` in each `fail`, state or message shown, `TypeError` for contract breaks | "words…", console spies in UI tests |

## What changed

- **Model and source** ([`src/libraries/`](../../src/libraries)): `model.ts` (contract aliases from `schema.d.ts`, routes `#/library`, `#/library/<id>`, `#/library/<id>/repair`, `LibrarySource`, `consentRequest`), `contract.ts` (checks of every answer), `client-source.ts` (`ClientLibrarySource`), `errors.ts` (`libraryFailure`), `registry.ts` (`LibraryRegistry`, at most 64 libraries, memory only).
- **Elements** ([`src/components/`](../../src/components)): `<fs-library-start>`, `<fs-library-detail>`, `<fs-library-repair>` and the shared `library-parts.ts` (stylesheet, builders, `Feedback` live regions). `bindLibraryRoute` ([`src/app/library-route.ts`](../../src/app/library-route.ts)) mounts one page for the hash.
- **Shell and wiring:** a "Create or open a library" link (`libraryNavLink`) before Monitor in the shell's tool list; `src/app/main.ts` and `apps/imagesquid/src/main.ts` bind the route and pass `libraryRepairHref` to `bindMonitorRoute`, so the Monitor's W010 note now ends with a "Review repair" link.
- **`<fs-states>`:** the `unavailable` state may carry a reason line (used for the folder chooser's 503).
- **Labels:** 67 new keys (`library*`, `repair*`) in `labels.ts`, `labels.en.json` and ImageSquid's table. ImageSquid's table must not contain the internal noun "repository", so the Git wording is "a folder managed by Git".
- **Tests:** Vitest unit and UI tests, 12 new FileSquid preview tests with a contract-shaped fake (`tests/e2e/support/library-api.ts`, which also enforces the API's consent rules), one ImageSquid preview test, and ten Gherkin scenarios in the F-001 feature. Existing tests changed for the new link: `app-shell.test.ts`, the ImageSquid distribution tests, the server-backed `accessibility.spec.ts` and `app.spec.ts` of both packages (Tab counts and lists), `production-build.spec.ts`, and the W010 preview test. Two server-backed tests that also fail on `main` were fixed: the reduced-motion walk in `accessibility.spec.ts` skipped no unrendered slot fallback (`ul.destinations` has no computed style while the navigation tree is slotted), and ImageSquid's `app.spec.ts` matched every `role="status"` region in the shell, the Monitor's included, instead of the shell's API status `p.api`.
- **Docs:** package README section "The library flow", the plan row, the global plan row and both checklist rows.

## Holistic UI decisions

- **UI-1.2 (import).** The open library's identity is the route `#/library/<library_id>`. UI-1.2 reads it from there and uses the shared `LibrarySource`/`LibraryRegistry`; it never asks for a folder again and needs no new state.
- **UI-F2.3, UI-F2.4, UI-F2.15 (panels).** No library UI lives in a panel. The library pages are routed pages in the shell's main area, independent of `registerPanel` slots.
- **UI-F2.8 (views, New control).** Create and open stay on the start page; the New control can link to `#/library` without change. The storage-used indicator and recent libraries need API fields that do not exist (see below).
- **UI-F2.14 (Monitor).** The per-library repair route is the `repairHref` the Monitor takes, passed in both apps.
- **Recover is open plus rebuild,** not a separate route, so a recovered library is an ordinary open library with a report on its page.

## Decisions and gaps for the platform engineer

- No new route or field is assumed. Everything uses the generated types in `schema.d.ts` (contract 0.20.0 after merging main).
- The mockups show a library **name**, **free disk space** and a **recent libraries** list. The API has no library name, no per-library free-space field (the health report's `free_bytes` is host-wide) and no recent-library field, and the UI never receives a path, so none of them is shown. Adding them needs API fields and an owner decision.
- The rebuild route is synchronous. Mockup 04's progress with Pause and resume needs a job with checkpoints; until then the recover flow shows a working state and then the report.
- A 409 answer carries only the code `conflict`; the rule that refused it (`root_not_empty`, `not_a_storage_root`, `git_in_root`, `location_in_use`, and so on) is only in the journal. The UI therefore words each conflict per step and lists the likely causes. Naming the rule in the answer would let the UI say exactly why.
- The server-backed Playwright suites (`tests/e2e/*.spec.ts`, which start `filesquid-server` through Cargo) were run against a real `filesquid-server` built from this branch: 42 passed in `apps/filesquid/ui` and 9 in `apps/imagesquid` (output below). The Tab counts, navigation lists and ImageSquid Tab order this task changed hold against the real server.

## How it was tested

Tests were written first and run red (the first block below), then the code was written. Prettier, ESLint, `tsc --noEmit`, Vitest, the preview Playwright suite and the server-backed Playwright suite (`FILESQUID_E2E=1`) pass in `apps/filesquid/ui` and `apps/imagesquid`. Playwright used the Chromium build already installed for the pinned Playwright version.

## Raw test output

Each block is the tool's output as printed, with one mechanical substitution: the absolute path of the working copy is written `<repo>`, because permanent files carry no absolute paths (AGENTS.md section 8).

### Red run (before implementation; counts of distinct failures)

```
     36 TypeError: LibraryRegistry is not a constructor
     15 TypeError: page.configure is not a function
     10 TypeError: ClientLibrarySource is not a constructor
      8 TypeError: libraryFailure is not a function
      3 TypeError: parseLibraryHash is not a function
      3 TypeError: consentRequest is not a function
      1 ⎯⎯⎯⎯⎯⎯ Failed Tests 92 ⎯⎯⎯⎯⎯⎯⎯
      1 TypeError: repairHref is not a function
      1 AssertionError: expected undefined to be defined
      1 AssertionError: expected undefined to be '#/library' // Object.is equality
      1 AssertionError: expected 'This is not available right now Try a…' to be 'This is not available right now A fol…' // Object.is equality
      1  Test Files  8 failed (8)
```

### Red run for the review fixes (Retry resumes from the failed step, journal check, repair busy per library)

```
     × Retry after a failed open reuses the chosen folder and asks for none again 15ms
     × Retry after a failed rebuild of a recovered copy rebuilds the opened library only 7ms
     × copy: Retry after a failed copy reuses the chosen destination and asks for none again 32ms
      Tests  3 failed | 35 passed (38)
     × a repair still running for another library does not block this library's offer 22ms
      Tests  1 failed | 19 passed (20)
     × checks a rebuild's optional journal restore against the contract 11ms
      Tests  1 failed | 17 passed (18)
```

### Prettier, ESLint and tsc (apps/filesquid/ui)

```
Checking formatting...
All matched files use Prettier code style!
prettier exit 0
eslint exit 0
tsc exit 0
```

### Vitest (apps/filesquid/ui)

```

> @filesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @filesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 <repo>/apps/filesquid/ui

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the error state and no host when the descriptor cannot be read
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:328:46
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
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:337:20
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
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:356:29
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
    at <repo>/apps/filesquid/ui/tests/ui/host-indicator.test.ts:368:29
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

 ✓ tests/ui/host-indicator.test.ts (40 tests) 377ms
 ✓ tests/ui/file-browser-writes.test.ts (36 tests) 1269ms
 ✓ tests/ui/file-browser.test.ts (37 tests) 1444ms
 ✓ tests/ui/nav-tree.test.ts (26 tests) 771ms
stderr | tests/ui/ocfl-findings.test.ts > <fs-ocfl-findings> > words a refused start from the label table
OCFL validation could not start ApiError: FileSquid API /api/v1/admin/ocfl/validation-runs failed with HTTP 403 (not_permitted)
    at <repo>/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:275:7
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
    at <repo>/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:289:29
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

 ✓ tests/ui/ocfl-findings.test.ts (21 tests) 367ms
 ✓ tests/unit/files-source.test.ts (26 tests) 60ms
 ✓ tests/unit/component-styles.test.ts (181 tests) 124ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 443ms
 ✓ tests/ui/library-start.test.ts (19 tests) 200ms
 ✓ tests/ui/library-repair.test.ts (20 tests) 410ms
 ✓ tests/ui/library-detail.test.ts (19 tests) 279ms
stderr | tests/ui/monitor.test.ts > <fs-monitor> > keeps the other sections up when the findings route is unavailable
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
    at Object.fake.client.adminOcflFindings (<repo>/apps/filesquid/ui/tests/ui/monitor.test.ts:431:22)
    at FsOcflFindings.read (<repo>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:153:41)
    at FsOcflFindings.load (<repo>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:140:23)
    at FsOcflFindings.refresh (<repo>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:128:31)
    at FsOcflFindings.connectedCallback (<repo>/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:109:15)
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

 ✓ tests/ui/monitor.test.ts (21 tests) 4254ms
     ✓ shows only the newest 200 rows after a long stream during a journal re-read  3753ms
stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows the error state with Try again when the details cannot be read, and retries
FileSquid could not preview a file Error

stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows permission denied with the reason's label
FileSquid could not preview a file ApiError

 ✓ tests/ui/navigation-view.test.ts (20 tests) 256ms
 ✓ tests/ui/file-preview.test.ts (16 tests) 192ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 39ms
 ✓ tests/unit/client-io.test.ts (22 tests) 46ms
 ✓ tests/unit/theme.test.ts (26 tests) 31ms
 ✓ tests/ui/app-shell.test.ts (19 tests) 403ms
 ✓ tests/unit/files-memory-source.test.ts (29 tests) 46ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 37ms
 ✓ tests/unit/files-ops.test.ts (47 tests) 112ms
 ✓ tests/ui/nav-tree-live.test.ts (11 tests) 276ms
Not implemented: HTMLMediaElement's pause() method
 ✓ tests/unit/media-handle.test.ts (14 tests) 90ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 38ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 94ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 30ms
 ✓ tests/unit/navigation-client.test.ts (13 tests) 31ms
 ✓ tests/unit/library-client.test.ts (18 tests) 43ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 40ms
 ✓ tests/unit/files-model.test.ts (58 tests) 62ms
 ✓ tests/unit/commands.test.ts (45 tests) 48ms
 ✓ tests/unit/admin-client.test.ts (10 tests) 25ms
 ✓ tests/unit/host-descriptor.test.ts (43 tests) 33ms
 ✓ tests/unit/files-window.test.ts (19 tests) 23ms
 ✓ tests/unit/focus.test.ts (16 tests) 59ms
 ✓ tests/ui/states.test.ts (18 tests) 87ms
 ✓ tests/ui/settings.test.ts (5 tests) 115ms
 ✓ tests/unit/library-errors.test.ts (15 tests) 23ms
 ✓ tests/unit/navigation-contract.test.ts (30 tests) 18ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 15ms
 ✓ tests/unit/test-scripts.test.ts (10 tests) 868ms
 ✓ tests/ui/library-route.test.ts (6 tests) 71ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 69ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 68ms
 ✓ tests/ui/files-route.test.ts (6 tests) 321ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 10ms
 ✓ tests/unit/client.test.ts (8 tests) 16ms
 ✓ tests/unit/library-model.test.ts (7 tests) 17ms
 ✓ tests/unit/preferences.test.ts (10 tests) 16ms
 ✓ tests/unit/view-state.test.ts (5 tests) 9ms
 ✓ tests/unit/host-client.test.ts (4 tests) 12ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 73ms
 ✓ tests/unit/contrast.test.ts (13 tests) 12ms
 ✓ tests/unit/distribution.test.ts (3 tests) 7ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 25ms
 ✓ tests/unit/contract.test.ts (1 test) 180ms

 Test Files  56 passed (56)
      Tests  1213 passed (1213)
   Start at  00:09:01
   Duration  20.31s (transform 2.75s, setup 2.07s, import 6.34s, tests 14.08s, environment 27.89s)

JUNIT report written to <repo>/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
```

### Playwright, production build plus vite preview (apps/filesquid/ui)

```

Running 62 tests using 2 workers

  ✓   1 [chromium] › tests/e2e/preview/host-indicator.spec.ts:96:1 › the indicator is a defined custom element in the shell header, named from the label table (277ms)
  ✓   2 [chromium] › tests/e2e/preview/file-browser.spec.ts:96:1 › the browser is a defined element and lists display names in the list layout from the label table (330ms)
  ✓   3 [chromium] › tests/e2e/preview/host-indicator.spec.ts:110:1 › the desktop host says only you can read your files, and shows the not-enrolled state (192ms)
  ✓   4 [chromium] › tests/e2e/preview/file-browser.spec.ts:140:1 › a file row shows size, type hint, modified date and version (230ms)
  ✓   5 [chromium] › tests/e2e/preview/host-indicator.spec.ts:124:1 › a server names its operator, who can read the files, and states the at-rest duty (166ms)
  ✓   7 [chromium] › tests/e2e/preview/host-indicator.spec.ts:137:1 › a server without an operator name uses the unnamed-operator label (184ms)
  ✓   6 [chromium] › tests/e2e/preview/file-browser.spec.ts:152:1 › computed token values are exact in the browser in light and dark (349ms)
  ✓   8 [chromium] › tests/e2e/preview/host-indicator.spec.ts:149:1 › the browser host says files are decrypted only in this browser (193ms)
  ✓  10 [chromium] › tests/e2e/preview/host-indicator.spec.ts:162:1 › enrolled backup and sync are shown apart from the host, as ciphertext only (228ms)
  ✓   9 [chromium] › tests/e2e/preview/file-browser.spec.ts:175:1 › opening a folder updates the breadcrumb and the address, and Back returns (433ms)
  ✓  11 [chromium] › tests/e2e/preview/host-indicator.spec.ts:187:1 › a host.change event updates the indicator without a reload or another read (195ms)
  ✓  13 [chromium] › tests/e2e/preview/host-indicator.spec.ts:209:1 › events.resync makes the indicator read the descriptor again (169ms)
  ✓  12 [chromium] › tests/e2e/preview/file-browser.spec.ts:196:1 › an image previews through a content handle that is used only as the media src (361ms)
  ✓  15 [chromium] › tests/e2e/preview/file-browser.spec.ts:231:1 › a handle that fails is replaced once; a second failure shows the error and mints no third (277ms)
  ✓  16 [chromium] › tests/e2e/preview/file-browser.spec.ts:247:1 › a video resumes at the same position and paused state after one new handle (321ms)
  ✓  17 [chromium] › tests/e2e/preview/file-browser.spec.ts:283:1 › audio plays natively and is named by the display name (238ms)
  ✓  14 [chromium] › tests/e2e/preview/host-indicator.spec.ts:222:1 › loading shows no host until the descriptor arrives (985ms)
  ✓  18 [chromium] › tests/e2e/preview/file-browser.spec.ts:296:1 › text previews through one ranged read of the first 64 KiB and says it is cut (274ms)
  ✓  19 [chromium] › tests/e2e/preview/host-indicator.spec.ts:234:1 › a failed read shows the error state and never a guessed host, then Retry recovers (319ms)
  ✓  20 [chromium] › tests/e2e/preview/file-browser.spec.ts:312:1 › a format it cannot show says so and shows no viewer (252ms)
  ✓  21 [chromium] › tests/e2e/preview/host-indicator.spec.ts:256:1 › the header shows a compact badge with the short host label, not the statement (217ms)
  ✓  23 [chromium] › tests/e2e/preview/host-indicator.spec.ts:271:1 › Enter opens the popover with the full statement; Escape closes it and returns focus (226ms)
  ✓  22 [chromium] › tests/e2e/preview/file-browser.spec.ts:325:1 › the viewer steps through files, closes with Escape and returns focus to the row (346ms)
  ✓  25 [chromium] › tests/e2e/preview/file-browser.spec.ts:338:1 › an upload is one PUT of the client's facts that creates the file's own object (249ms)
  ✓  24 [chromium] › tests/e2e/preview/host-indicator.spec.ts:296:1 › Privacy settings opens the Settings page, whose Privacy section has the full statement (360ms)
  ✓  26 [chromium] › tests/e2e/preview/file-browser.spec.ts:370:1 › writes are single calls: new folder, rename, remove and Undo (550ms)
  ✓  27 [chromium] › tests/e2e/preview/host-indicator.spec.ts:331:1 › computed token values are exact in light and dark (656ms)
  ✓  28 [chromium] › tests/e2e/preview/file-browser.spec.ts:417:1 › a refused write shows the label for its reason and changes nothing (235ms)
  ✓  29 [chromium] › tests/e2e/preview/host-indicator.spec.ts:378:1 › the badge is reachable by keyboard and has a visible focus outline (214ms)
  ✓  31 [chromium] › tests/e2e/preview/library.spec.ts:45:1 › the start page offers create, open and recover in the active vocabulary (168ms)
  ✓  30 [chromium] › tests/e2e/preview/file-browser.spec.ts:432:1 › a long folder is virtualised, pages in on demand and stays keyboard reachable (384ms)
  ✓  32 [chromium] › tests/e2e/preview/library.spec.ts:74:1 › (1) create asks for a folder, passes the location ID on and shows the library, with no path (288ms)
  ✓  33 [chromium] › tests/e2e/preview/file-browser.spec.ts:472:1 › the layout does not scroll sideways at 320 px, at 200 % text size, or both, in list and grid (329ms)
  ✓  34 [chromium] › tests/e2e/preview/library.spec.ts:96:1 › (1) open passes the location ID to the open route (282ms)
  ✓  35 [chromium] › tests/e2e/preview/file-browser.spec.ts:489:1 › motion stops under reduced motion and focus is the 2px primary ring (255ms)
  ✓  36 [chromium] › tests/e2e/preview/library.spec.ts:110:1 › (1) recover opens the copy, rebuilds from files and reports without repairing (259ms)
  ✓  37 [chromium] › tests/e2e/preview/file-browser.spec.ts:514:1 › a failed listing shows the error state with Try again, and retrying loads it (249ms)
  ✓  39 [chromium] › tests/e2e/preview/navigation.spec.ts:62:1 › the live tree fills the shell's navigation slot, worded from the label table (207ms)
  ✓  38 [chromium] › tests/e2e/preview/library.spec.ts:143:1 › (1) copy asks for a destination and passes it as destination_location_id (280ms)
  ✓  40 [chromium] › tests/e2e/preview/navigation.spec.ts:83:1 › a level is read page by page through next_cursor until it is null (194ms)
  ✓  41 [chromium] › tests/e2e/preview/library.spec.ts:166:1 › (2) a cancelled folder choice sends no library request and says nothing changed (233ms)
  ✓  42 [chromium] › tests/e2e/preview/navigation.spec.ts:121:1 › a destination opens with a breadcrumb filled from ancestors (381ms)
  ✓  43 [chromium] › tests/e2e/preview/library.spec.ts:180:1 › (3) 503 unavailable is the unavailable state with its reason and a Retry, and the choices stay (326ms)
  ✓  44 [chromium] › tests/e2e/preview/navigation.spec.ts:142:1 › a deep link to a destination shows its breadcrumb after a reload (278ms)
  ✓  45 [chromium] › tests/e2e/preview/library.spec.ts:207:1 › (4) confirm sends the consent for exactly the named objects and shows each result (311ms)
  ✓  46 [chromium] › tests/e2e/preview/navigation.spec.ts:154:1 › navigation.change reads only the named level again, without a reload (192ms)
  ✓  47 [chromium] › tests/e2e/preview/library.spec.ts:244:1 › (4) cancel and Escape send nothing and return focus to the opener (334ms)
  ✓  48 [chromium] › tests/e2e/preview/navigation.spec.ts:177:1 › a 503 shows the unavailable state with Retry, never an empty tree (231ms)
  ✓  49 [chromium] › tests/e2e/preview/library.spec.ts:267:1 › (4) a refused repair says nothing was changed and shows no result (316ms)
  ✓  51 [chromium] › tests/e2e/preview/library.spec.ts:282:1 › (5) the Monitor's W010 note links to the library's repair offer (232ms)
  ✓  50 [chromium] › tests/e2e/preview/navigation.spec.ts:194:1 › computed token values of the tree and breadcrumb are exact in light and dark (697ms)
  ✓  53 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:174:1 › (1) an E023 finding shows with its code beside the degraded health reason (200ms)
  ✓  52 [chromium] › tests/e2e/preview/library.spec.ts:335:1 › colours come from tokens in light and dark (493ms)
  ✓  54 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:200:1 › (2) the finding disappears after a clean revalidation's health.change, without a reload (236ms)
  ✓  55 [chromium] › tests/e2e/preview/production-build.spec.ts:62:1 › the built page is static, mounts the shell and defines every custom element (197ms)
  ✓  56 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:220:1 › (3) a W010 warning is listed apart from errors and health stays ok (257ms)
  ✓  57 [chromium] › tests/e2e/preview/production-build.spec.ts:79:1 › navigation, API status and theme legend words come from the active label table (212ms)
  ✓  59 [chromium] › tests/e2e/preview/production-build.spec.ts:101:1 › computed token values are exact in light and dark (213ms)
  ✓  58 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:238:1 › (4) no location is rendered when the API withholds it (214ms)
  ✓  60 [chromium] › tests/e2e/preview/production-build.spec.ts:117:1 › a refused health call shows the unavailable alert, not a console error from the app (195ms)
  ✓  61 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:250:1 › (5) the last and next run times come from the findings route; Validate now starts a run (281ms)
  ✓  62 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:289:1 › error and warning codes take their colours from tokens in light and dark (179ms)

  62 passed (16.8s)
pw exit 0
```

### Playwright, production build served by the real filesquid-server (apps/filesquid/ui, `FILESQUID_E2E=1`)

```

Running 42 tests using 2 workers

  ✓   1 [chromium] › tests/e2e/accessibility.spec.ts:67:1 › landmarks, headings and control names come from the label table (259ms)
  ✓   2 [chromium] › tests/e2e/app.spec.ts:35:1 › shell mounts with FileSquid brand, internal vocabulary and live API health (301ms)
  ✓   3 [chromium] › tests/e2e/accessibility.spec.ts:106:3 › Tab order and a 2px primary focus ring in the light theme (164ms)
  ✓   4 [chromium] › tests/e2e/app.spec.ts:60:1 › design tokens switch between light and dark (170ms)
  ✓   5 [chromium] › tests/e2e/accessibility.spec.ts:106:3 › Tab order and a 2px primary focus ring in the dark theme (165ms)
  ✓   6 [chromium] › tests/e2e/app.spec.ts:77:1 › client-side routes fall back to the distribution index (153ms)
  ✓   7 [chromium] › tests/e2e/accessibility.spec.ts:133:1 › light-DOM content slotted into the shell gets the same focus ring (209ms)
  ✓   8 [chromium] › tests/e2e/app.spec.ts:83:1 › without a token the shell names the missing credentials from the label table (201ms)
  ✓   9 [chromium] › tests/e2e/accessibility.spec.ts:157:1 › the skip control appears on focus and moves focus to main without routing (231ms)
  ✓  10 [chromium] › tests/e2e/app.spec.ts:95:1 › Monitor asks for the admin token, then shows live health, processes and the journal (344ms)
  ✓  11 [chromium] › tests/e2e/accessibility.spec.ts:175:1 › opening the Monitor from the keyboard moves focus to main (262ms)
  ✓  12 [chromium] › tests/e2e/app.spec.ts:128:1 › a wrong token is named as invalid credentials (260ms)
  ✓  13 [chromium] › tests/e2e/accessibility.spec.ts:193:1 › a keyboard sign-in to the Monitor keeps focus in the Monitor (382ms)
  ✓  15 [chromium] › tests/e2e/accessibility.spec.ts:210:1 › reduced motion stops every transition (213ms)
  ✓  14 [chromium] › tests/e2e/app.spec.ts:139:1 › the signed-in token stays in memory: not in storage, URL or DOM, gone on reload (508ms)
  ✓  17 [chromium] › tests/e2e/app.spec.ts:170:1 › the desktop launch token authorises the shell, the Monitor and the event channel (224ms)
  ✓  16 [chromium] › tests/e2e/accessibility.spec.ts:251:1 › more contrast strengthens borders; forced colours keep the chosen option (529ms)
  ✓  18 [chromium] › tests/e2e/app.spec.ts:260:3 › inside the desktop webview (custom URI scheme adapter) › requests use the custom scheme and events use Tauri, not same-origin HTTP or WebSocket (212ms)
  ✓  19 [chromium] › tests/e2e/accessibility.spec.ts:294:3 › no horizontal scroll at 320 px wide and 100 % text (220ms)
  ✓  20 [chromium] › tests/e2e/navigation.spec.ts:186:1 › the elements are defined and worded from the label table inside the shell (248ms)
  ✓  21 [chromium] › tests/e2e/accessibility.spec.ts:294:3 › no horizontal scroll at 320 px wide and 200 % text (260ms)
  ✓  22 [chromium] › tests/e2e/navigation.spec.ts:232:1 › the tree follows the ARIA treeview keys and reads children as nodes open (295ms)
  ✓  23 [chromium] › tests/e2e/accessibility.spec.ts:294:3 › no horizontal scroll at 1280 px wide and 200 % text (225ms)
  ✓  25 [chromium] › tests/e2e/accessibility.spec.ts:294:3 › no horizontal scroll at 640 px wide and 200 % text (206ms)
  ✓  24 [chromium] › tests/e2e/navigation.spec.ts:268:1 › pins, section order and library collapse are kept on the device across a reload (414ms)
  ✓  26 [chromium] › tests/e2e/accessibility.spec.ts:317:1 › a right-to-left locale mirrors the header and navigation (215ms)
  ✓  27 [chromium] › tests/e2e/navigation.spec.ts:309:1 › refused device storage keeps the choices for this page and says why (253ms)
  ✓  28 [chromium] › tests/e2e/ocfl-findings.spec.ts:37:1 › the findings section reads the real findings route with the token (248ms)
  ✓  29 [chromium] › tests/e2e/navigation.spec.ts:334:1 › the tree shows the error and offline states and reads again on Retry (305ms)
  ✓  30 [chromium] › tests/e2e/ocfl-findings.spec.ts:68:1 › Validate now starts runs through the real run route and says nothing was open (199ms)
  ✓  31 [chromium] › tests/e2e/navigation.spec.ts:356:3 › the five states are distinct and use token colours in the light theme (197ms)
  ✓  32 [chromium] › tests/e2e/theme.spec.ts:62:1 › official FileSquid tokens apply in light and dark and meet WCAG 2.2 AA (192ms)
  ✓  33 [chromium] › tests/e2e/navigation.spec.ts:399:3 › tree controls draw the 2px primary focus ring in the light theme (256ms)
  ✓  34 [chromium] › tests/e2e/theme.spec.ts:86:1 › the official mark fills the brand slot, with its dark-surface variant in dark (283ms)
  ✓  35 [chromium] › tests/e2e/navigation.spec.ts:356:3 › the five states are distinct and use token colours in the dark theme (169ms)
  ✓  37 [chromium] › tests/e2e/navigation.spec.ts:399:3 › tree controls draw the 2px primary focus ring in the dark theme (218ms)
  ✓  36 [chromium] › tests/e2e/theme.spec.ts:112:1 › the theme follows the OS by default and the control's choice persists across a reload (324ms)
  ✓  39 [chromium] › tests/e2e/theme.spec.ts:156:1 › the selected theme option and the API status meet AA as rendered (215ms)
  ✓  38 [chromium] › tests/e2e/navigation.spec.ts:423:1 › tree motion follows the token, stops under reduced motion, and forced colours mark the current item (242ms)
  ✓  40 [chromium] › tests/e2e/navigation.spec.ts:464:3 › the tree and breadcrumb reflow at 320 px and 100 % text (162ms)
  ✓  41 [chromium] › tests/e2e/navigation.spec.ts:464:3 › the tree and breadcrumb reflow at 320 px and 200 % text (163ms)
  ✓  42 [chromium] › tests/e2e/navigation.spec.ts:464:3 › the tree and breadcrumb reflow at 1280 px and 200 % text (173ms)

  42 passed (12.6s)
pw exit 0
```

### Vitest and Playwright preview (apps/imagesquid)

```

> @imagesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @imagesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 <repo>/apps/imagesquid

 ✓ tests/unit/test-reporting.test.ts (12 tests) 23ms
 ✓ tests/unit/theme.test.ts (10 tests) 13ms
 ✓ tests/unit/test-scripts.test.ts (10 tests) 733ms
 ✓ tests/ui/distribution.test.ts (5 tests) 137ms

 Test Files  4 passed (4)
      Tests  37 passed (37)
   Start at  00:09:24
   Duration  1.54s (transform 772ms, setup 206ms, import 865ms, tests 907ms, environment 1.57s)

JUNIT report written to <repo>/apps/imagesquid/tests/allure-results/vitest-junit.xml

Running 5 tests using 1 worker

  ✓  1 [chromium] › tests/e2e/preview/production-build.spec.ts:57:1 › the built page is static, mounts the shell and defines every custom element (207ms)
  ✓  2 [chromium] › tests/e2e/preview/production-build.spec.ts:74:1 › navigation, API status and theme legend words come from the active label table (134ms)
  ✓  3 [chromium] › tests/e2e/preview/production-build.spec.ts:98:1 › computed ImageSquid token values are exact in light and dark (138ms)
  ✓  4 [chromium] › tests/e2e/preview/production-build.spec.ts:114:1 › a refused health call shows the unavailable alert (130ms)
  ✓  5 [chromium] › tests/e2e/preview/production-build.spec.ts:127:1 › the library flow runs in ImageSquid with its own label table (UI-1.1) (214ms)

  5 passed (6.7s)
pw exit 0
```

### Playwright, production build served by the real filesquid-server (apps/imagesquid, `FILESQUID_E2E=1`)

```

Running 9 tests using 1 worker

  ✓  1 [chromium] › tests/e2e/app.spec.ts:14:1 › ImageSquid brand, album vocabulary and FileSquid API health (162ms)
  ✓  2 [chromium] › tests/e2e/app.spec.ts:58:1 › ImageSquid tokens override FileSquid defaults in light and dark (IS-S-008) (125ms)
  ✓  3 [chromium] › tests/e2e/app.spec.ts:83:1 › ImageSquid's theme control keeps a dark choice across a reload (298ms)
  ✓  4 [chromium] › tests/e2e/app.spec.ts:103:1 › ImageSquid names missing credentials in its own words (114ms)
  ✓  5 [chromium] › tests/e2e/app.spec.ts:112:1 › ImageSquid reaches the FileSquid Monitor with its own vocabulary (260ms)
  ✓  6 [chromium] › tests/e2e/app.spec.ts:134:3 › ImageSquid inherits the keyboard, naming and focus contracts in light (UI-S-016) (258ms)
  ✓  7 [chromium] › tests/e2e/app.spec.ts:134:3 › ImageSquid inherits the keyboard, naming and focus contracts in dark (UI-S-016) (214ms)
  ✓  8 [chromium] › tests/e2e/app.spec.ts:187:3 › the navigation tree and view states speak ImageSquid in light (UI-F2.1a) (134ms)
  ✓  9 [chromium] › tests/e2e/app.spec.ts:187:3 › the navigation tree and view states speak ImageSquid in dark (UI-F2.1a) (172ms)

  9 passed (6.6s)
pw exit 0
```
