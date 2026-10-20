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
| 3 `cancelled` sends nothing further | `FsLibraryStart.run`, `FsLibraryDetail.copy` | "cancel: changes nothing…" (start and detail); preview (2) |
| 4 503 is the unavailable state with its reason and Retry; choices stay | `libraryFailure` (`choose`), `ViewState` `unavailable.reason`, `<fs-states>` | "503: shows the unavailable state…", "Retry asks for the folder again…"; preview (3) |
| 5 no path; ID limits | no path field in any type; `parseLocationChoice` (128 bytes), `parseLibraryHash` (128 bytes), `consentRequest` (1,024 bytes) | "never renders a filesystem path"; model and client tests; preview (1) checks every request and the body text |
| 6 409, 404, 403 messages | `libraryFailure` | `library-errors.test.ts`; start and repair UI tests |
| 7 an already open library is answered again | `open` handles a plain answer | "answers an already open library again without an error" |
| Recover: open, then rebuild; report; nothing repaired | `FsLibraryStart.complete`, `rebuildReport` | "recover: opens the chosen copy, rebuilds…", preview recover test (no `/repairs` request) |
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
- **Tests:** Vitest unit and UI tests, 12 new FileSquid preview tests with a contract-shaped fake (`tests/e2e/support/library-api.ts`, which also enforces the API's consent rules), one ImageSquid preview test, and ten Gherkin scenarios in the F-001 feature. Existing tests changed for the new link: `app-shell.test.ts`, the ImageSquid distribution tests, the server-backed `accessibility.spec.ts` and `app.spec.ts` of both packages (Tab counts and lists), `production-build.spec.ts`, and the W010 preview test.
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
- The server-backed Playwright suites (`tests/e2e/*.spec.ts`, which start `filesquid-server` through Cargo) were updated for the new link but not run here, because UI-only work runs no Cargo. The static preview suites were run.

## How it was tested

Tests were written first and run red (the first block below), then the code was written. Prettier, ESLint, `tsc --noEmit`, Vitest and the preview Playwright suite pass in `apps/filesquid/ui` and `apps/imagesquid`. Playwright browsers were installed with `npx playwright install chromium` because the installed build did not match the pinned Playwright.

## Raw test output

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

### Prettier, ESLint and tsc (apps/filesquid/ui)

```
Checking formatting...
All matched files use Prettier code style!
eslint exit 0
tsc exit 0
```

### Vitest (apps/filesquid/ui)

```

> @filesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @filesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 /home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the error state and no host when the descriptor cannot be read
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at /home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/tests/ui/host-indicator.test.ts:328:46
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the denied state for an authorisation denial
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 401 (missing_credentials)
    at /home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/tests/ui/host-indicator.test.ts:337:20
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 401,
  path: '/api/v1/host',
  code: 'unauthorized',
  reason: 'missing_credentials',
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > shows the offline state with Retry when the read fails while the browser is offline
FileSquid host descriptor could not be read TypeError: Failed to fetch
    at /home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/tests/ui/host-indicator.test.ts:356:29
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> loading and error > reads again when Retry is pressed and recovers
FileSquid host descriptor could not be read ApiError: FileSquid API /api/v1/host failed with HTTP 500
    at /home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/tests/ui/host-indicator.test.ts:368:29
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 500,
  path: '/api/v1/host',
  code: undefined,
  reason: undefined,
  retryable: false
}

stderr | tests/ui/host-indicator.test.ts > <fs-host-indicator> follows the host > reads again when a host.change payload breaks the contract
FileSquid host.change payload broke the contract; reading the descriptor

 ✓ tests/ui/host-indicator.test.ts (40 tests) 318ms
 ✓ tests/ui/file-browser.test.ts (37 tests) 1382ms
 ✓ tests/ui/file-browser-writes.test.ts (36 tests) 1459ms
 ✓ tests/ui/nav-tree.test.ts (26 tests) 767ms
stderr | tests/ui/ocfl-findings.test.ts > <fs-ocfl-findings> > words a refused start from the label table
OCFL validation could not start ApiError: FileSquid API /api/v1/admin/ocfl/validation-runs failed with HTTP 403 (not_permitted)
    at /home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:275:7
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 403,
  path: '/api/v1/admin/ocfl/validation-runs',
  code: undefined,
  reason: 'not_permitted',
  retryable: false
}

stderr | tests/ui/ocfl-findings.test.ts > <fs-ocfl-findings> > shows the unavailable state with Retry when the findings route answers 503
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
    at /home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/tests/ui/ocfl-findings.test.ts:289:29
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:302:11
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:1903:26
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2326:20
    at new Promise (<anonymous>)
    at runWithCancel (file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2323:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2305:20
    at new Promise (<anonymous>)
    at runWithTimeout (file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2272:10)
    at file:///home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/@vitest/runner/dist/chunk-artifact.js:2956:64 {
  status: 503,
  path: '/api/v1/admin/ocfl/findings',
  code: undefined,
  reason: undefined,
  retryable: false
}

 ✓ tests/ui/ocfl-findings.test.ts (21 tests) 420ms
 ✓ tests/unit/files-source.test.ts (26 tests) 67ms
 ✓ tests/unit/component-styles.test.ts (181 tests) 77ms
 ✓ tests/ui/accessibility.test.ts (19 tests) 510ms
 ✓ tests/ui/library-repair.test.ts (19 tests) 319ms
 ✓ tests/ui/navigation-view.test.ts (20 tests) 185ms
stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows the error state with Try again when the details cannot be read, and retries
FileSquid could not preview a file Error

stderr | tests/ui/file-preview.test.ts > <fs-file-preview> > shows permission denied with the reason's label
FileSquid could not preview a file ApiError

 ✓ tests/ui/file-preview.test.ts (16 tests) 171ms
 ✓ tests/unit/client-auth.test.ts (23 tests) 39ms
 ✓ tests/unit/client-io.test.ts (22 tests) 47ms
stderr | tests/ui/monitor.test.ts > <fs-monitor> > keeps the other sections up when the findings route is unavailable
OCFL findings could not be read ApiError: FileSquid API /api/v1/admin/ocfl/findings failed with HTTP 503
    at Object.fake.client.adminOcflFindings (/home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/tests/ui/monitor.test.ts:431:22)
    at FsOcflFindings.read (/home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:153:41)
    at FsOcflFindings.load (/home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:140:23)
    at FsOcflFindings.refresh (/home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:128:31)
    at FsOcflFindings.connectedCallback (/home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/src/components/fs-ocfl-findings.ts:109:15)
    at FsOcflFindings.invokeTheCallbackFunction (/home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/generated/Function.js:19:26)
    at invokeCEReactions (/home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/helpers/custom-elements.js:190:31)
    at ceReactionsPostSteps (/home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/helpers/custom-elements.js:54:3)
    at ShadowRoot.replaceChildren (/home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/node_modules/jsdom/lib/jsdom/living/generated/DocumentFragment.js:204:9)
    at replaceKeepingFocus (/home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/src/a11y/focus.ts:52:8) {
  status: 503,
  path: '/api/v1/admin/ocfl/findings',
  code: undefined,
  reason: undefined,
  retryable: false
}

 ✓ tests/ui/monitor.test.ts (21 tests) 4360ms
     ✓ shows only the newest 200 rows after a long stream during a journal re-read  3921ms
 ✓ tests/ui/app-shell.test.ts (19 tests) 395ms
 ✓ tests/unit/files-memory-source.test.ts (29 tests) 90ms
 ✓ tests/ui/library-detail.test.ts (17 tests) 392ms
 ✓ tests/unit/theme.test.ts (26 tests) 28ms
 ✓ tests/unit/files-ops.test.ts (47 tests) 117ms
 ✓ tests/ui/library-start.test.ts (15 tests) 213ms
 ✓ tests/unit/navigation-store.test.ts (19 tests) 29ms
Not implemented: HTMLMediaElement's pause() method
 ✓ tests/unit/media-handle.test.ts (14 tests) 87ms
 ✓ tests/ui/nav-tree-live.test.ts (11 tests) 254ms
 ✓ tests/unit/a11y-audit.test.ts (19 tests) 88ms
 ✓ tests/unit/navigation-model.test.ts (30 tests) 35ms
 ✓ tests/unit/navigation-source.test.ts (18 tests) 35ms
 ✓ tests/unit/navigation-client.test.ts (13 tests) 20ms
 ✓ tests/unit/theme-store.test.ts (23 tests) 39ms
 ✓ tests/unit/files-model.test.ts (58 tests) 51ms
 ✓ tests/unit/commands.test.ts (45 tests) 28ms
 ✓ tests/unit/library-client.test.ts (17 tests) 28ms
 ✓ tests/unit/host-descriptor.test.ts (43 tests) 27ms
 ✓ tests/unit/admin-client.test.ts (10 tests) 24ms
 ✓ tests/unit/files-window.test.ts (19 tests) 17ms
 ✓ tests/unit/focus.test.ts (16 tests) 68ms
 ✓ tests/ui/states.test.ts (18 tests) 95ms
 ✓ tests/ui/settings.test.ts (5 tests) 123ms
 ✓ tests/unit/library-errors.test.ts (15 tests) 17ms
 ✓ tests/unit/navigation-contract.test.ts (30 tests) 18ms
 ✓ tests/unit/test-scripts.test.ts (10 tests) 771ms
 ✓ tests/unit/test-reporting.test.ts (12 tests) 13ms
 ✓ tests/ui/library-route.test.ts (6 tests) 107ms
 ✓ tests/unit/host-adapter.test.ts (13 tests) 73ms
 ✓ tests/ui/theme-control.test.ts (8 tests) 55ms
 ✓ tests/ui/files-route.test.ts (6 tests) 293ms
 ✓ tests/unit/nav-tree-keys.test.ts (8 tests) 10ms
 ✓ tests/unit/client.test.ts (8 tests) 20ms
 ✓ tests/unit/library-model.test.ts (7 tests) 14ms
 ✓ tests/unit/preferences.test.ts (10 tests) 16ms
 ✓ tests/unit/view-state.test.ts (5 tests) 9ms
 ✓ tests/ui/breadcrumb.test.ts (6 tests) 70ms
 ✓ tests/unit/host-client.test.ts (4 tests) 17ms
 ✓ tests/unit/contrast.test.ts (13 tests) 11ms
 ✓ tests/unit/distribution.test.ts (3 tests) 6ms
 ✓ tests/ui/monitor-route.test.ts (2 tests) 24ms
 ✓ tests/unit/contract.test.ts (1 test) 175ms

 Test Files  56 passed (56)
      Tests  1206 passed (1206)
   Start at  23:48:14
   Duration  19.94s (transform 2.83s, setup 1.95s, import 6.34s, tests 14.12s, environment 27.20s)

JUNIT report written to /home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/filesquid/ui/tests/allure-results/vitest-junit.xml
vitest exit 0
```

### Playwright, production build plus vite preview (apps/filesquid/ui)

```

Running 62 tests using 2 workers

  ✓   1 [chromium] › tests/e2e/preview/host-indicator.spec.ts:96:1 › the indicator is a defined custom element in the shell header, named from the label table (272ms)
  ✓   2 [chromium] › tests/e2e/preview/file-browser.spec.ts:96:1 › the browser is a defined element and lists display names in the list layout from the label table (358ms)
  ✓   3 [chromium] › tests/e2e/preview/host-indicator.spec.ts:110:1 › the desktop host says only you can read your files, and shows the not-enrolled state (216ms)
  ✓   4 [chromium] › tests/e2e/preview/file-browser.spec.ts:140:1 › a file row shows size, type hint, modified date and version (284ms)
  ✓   5 [chromium] › tests/e2e/preview/host-indicator.spec.ts:124:1 › a server names its operator, who can read the files, and states the at-rest duty (195ms)
  ✓   7 [chromium] › tests/e2e/preview/host-indicator.spec.ts:137:1 › a server without an operator name uses the unnamed-operator label (160ms)
  ✓   6 [chromium] › tests/e2e/preview/file-browser.spec.ts:152:1 › computed token values are exact in the browser in light and dark (360ms)
  ✓   8 [chromium] › tests/e2e/preview/host-indicator.spec.ts:149:1 › the browser host says files are decrypted only in this browser (201ms)
  ✓  10 [chromium] › tests/e2e/preview/host-indicator.spec.ts:162:1 › enrolled backup and sync are shown apart from the host, as ciphertext only (220ms)
  ✓  11 [chromium] › tests/e2e/preview/host-indicator.spec.ts:187:1 › a host.change event updates the indicator without a reload or another read (169ms)
  ✓   9 [chromium] › tests/e2e/preview/file-browser.spec.ts:175:1 › opening a folder updates the breadcrumb and the address, and Back returns (438ms)
  ✓  12 [chromium] › tests/e2e/preview/host-indicator.spec.ts:209:1 › events.resync makes the indicator read the descriptor again (204ms)
  ✓  13 [chromium] › tests/e2e/preview/file-browser.spec.ts:196:1 › an image previews through a content handle that is used only as the media src (400ms)
  ✓  15 [chromium] › tests/e2e/preview/file-browser.spec.ts:231:1 › a handle that fails is replaced once; a second failure shows the error and mints no third (269ms)
  ✓  16 [chromium] › tests/e2e/preview/file-browser.spec.ts:247:1 › a video resumes at the same position and paused state after one new handle (289ms)
  ✓  14 [chromium] › tests/e2e/preview/host-indicator.spec.ts:222:1 › loading shows no host until the descriptor arrives (968ms)
  ✓  17 [chromium] › tests/e2e/preview/file-browser.spec.ts:283:1 › audio plays natively and is named by the display name (252ms)
  ✓  18 [chromium] › tests/e2e/preview/host-indicator.spec.ts:234:1 › a failed read shows the error state and never a guessed host, then Retry recovers (272ms)
  ✓  19 [chromium] › tests/e2e/preview/file-browser.spec.ts:296:1 › text previews through one ranged read of the first 64 KiB and says it is cut (280ms)
  ✓  20 [chromium] › tests/e2e/preview/host-indicator.spec.ts:256:1 › the header shows a compact badge with the short host label, not the statement (202ms)
  ✓  21 [chromium] › tests/e2e/preview/file-browser.spec.ts:312:1 › a format it cannot show says so and shows no viewer (332ms)
  ✓  22 [chromium] › tests/e2e/preview/host-indicator.spec.ts:271:1 › Enter opens the popover with the full statement; Escape closes it and returns focus (287ms)
  ✓  23 [chromium] › tests/e2e/preview/file-browser.spec.ts:325:1 › the viewer steps through files, closes with Escape and returns focus to the row (329ms)
  ✓  24 [chromium] › tests/e2e/preview/host-indicator.spec.ts:296:1 › Privacy settings opens the Settings page, whose Privacy section has the full statement (341ms)
  ✓  25 [chromium] › tests/e2e/preview/file-browser.spec.ts:338:1 › an upload is one PUT of the client's facts that creates the file's own object (254ms)
  ✓  26 [chromium] › tests/e2e/preview/host-indicator.spec.ts:331:1 › computed token values are exact in light and dark (652ms)
  ✓  27 [chromium] › tests/e2e/preview/file-browser.spec.ts:370:1 › writes are single calls: new folder, rename, remove and Undo (552ms)
  ✓  28 [chromium] › tests/e2e/preview/host-indicator.spec.ts:378:1 › the badge is reachable by keyboard and has a visible focus outline (216ms)
  ✓  29 [chromium] › tests/e2e/preview/file-browser.spec.ts:417:1 › a refused write shows the label for its reason and changes nothing (289ms)
  ✓  30 [chromium] › tests/e2e/preview/library.spec.ts:45:1 › the start page offers create, open and recover in the active vocabulary (187ms)
  ✓  32 [chromium] › tests/e2e/preview/library.spec.ts:74:1 › (1) create asks for a folder, passes the location ID on and shows the library, with no path (272ms)
  ✓  31 [chromium] › tests/e2e/preview/file-browser.spec.ts:432:1 › a long folder is virtualised, pages in on demand and stays keyboard reachable (431ms)
  ✓  33 [chromium] › tests/e2e/preview/library.spec.ts:96:1 › (1) open passes the location ID to the open route (237ms)
  ✓  34 [chromium] › tests/e2e/preview/file-browser.spec.ts:472:1 › the layout does not scroll sideways at 320 px, at 200 % text size, or both, in list and grid (364ms)
  ✓  35 [chromium] › tests/e2e/preview/library.spec.ts:110:1 › (1) recover opens the copy, rebuilds from files and reports without repairing (288ms)
  ✓  36 [chromium] › tests/e2e/preview/file-browser.spec.ts:489:1 › motion stops under reduced motion and focus is the 2px primary ring (297ms)
  ✓  37 [chromium] › tests/e2e/preview/library.spec.ts:143:1 › (1) copy asks for a destination and passes it as destination_location_id (258ms)
  ✓  38 [chromium] › tests/e2e/preview/file-browser.spec.ts:514:1 › a failed listing shows the error state with Try again, and retrying loads it (269ms)
  ✓  39 [chromium] › tests/e2e/preview/library.spec.ts:166:1 › (2) a cancelled folder choice sends no library request and says nothing changed (246ms)
  ✓  40 [chromium] › tests/e2e/preview/navigation.spec.ts:62:1 › the live tree fills the shell's navigation slot, worded from the label table (196ms)
  ✓  41 [chromium] › tests/e2e/preview/library.spec.ts:180:1 › (3) 503 unavailable is the unavailable state with its reason and a Retry, and the choices stay (297ms)
  ✓  42 [chromium] › tests/e2e/preview/navigation.spec.ts:83:1 › a level is read page by page through next_cursor until it is null (205ms)
  ✓  43 [chromium] › tests/e2e/preview/library.spec.ts:207:1 › (4) confirm sends the consent for exactly the named objects and shows each result (340ms)
  ✓  44 [chromium] › tests/e2e/preview/navigation.spec.ts:121:1 › a destination opens with a breadcrumb filled from ancestors (317ms)
  ✓  46 [chromium] › tests/e2e/preview/navigation.spec.ts:142:1 › a deep link to a destination shows its breadcrumb after a reload (279ms)
  ✓  45 [chromium] › tests/e2e/preview/library.spec.ts:244:1 › (4) cancel and Escape send nothing and return focus to the opener (377ms)
  ✓  47 [chromium] › tests/e2e/preview/navigation.spec.ts:154:1 › navigation.change reads only the named level again, without a reload (197ms)
  ✓  48 [chromium] › tests/e2e/preview/library.spec.ts:267:1 › (4) a refused repair says nothing was changed and shows no result (272ms)
  ✓  49 [chromium] › tests/e2e/preview/navigation.spec.ts:177:1 › a 503 shows the unavailable state with Retry, never an empty tree (252ms)
  ✓  50 [chromium] › tests/e2e/preview/library.spec.ts:282:1 › (5) the Monitor's W010 note links to the library's repair offer (245ms)
  ✓  52 [chromium] › tests/e2e/preview/library.spec.ts:335:1 › colours come from tokens in light and dark (527ms)
  ✓  51 [chromium] › tests/e2e/preview/navigation.spec.ts:194:1 › computed token values of the tree and breadcrumb are exact in light and dark (746ms)
  ✓  53 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:174:1 › (1) an E023 finding shows with its code beside the degraded health reason (207ms)
  ✓  54 [chromium] › tests/e2e/preview/production-build.spec.ts:62:1 › the built page is static, mounts the shell and defines every custom element (235ms)
  ✓  55 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:200:1 › (2) the finding disappears after a clean revalidation's health.change, without a reload (249ms)
  ✓  56 [chromium] › tests/e2e/preview/production-build.spec.ts:79:1 › navigation, API status and theme legend words come from the active label table (190ms)
  ✓  57 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:220:1 › (3) a W010 warning is listed apart from errors and health stays ok (225ms)
  ✓  58 [chromium] › tests/e2e/preview/production-build.spec.ts:101:1 › computed token values are exact in light and dark (193ms)
  ✓  59 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:238:1 › (4) no location is rendered when the API withholds it (266ms)
  ✓  60 [chromium] › tests/e2e/preview/production-build.spec.ts:117:1 › a refused health call shows the unavailable alert, not a console error from the app (218ms)
  ✓  61 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:250:1 › (5) the last and next run times come from the findings route; Validate now starts a run (265ms)
  ✓  62 [chromium] › tests/e2e/preview/ocfl-findings.spec.ts:289:1 › error and warning codes take their colours from tokens in light and dark (189ms)

  62 passed (16.8s)
pw exit 0
```

### Vitest and Playwright preview (apps/imagesquid)

```

> @imagesquid/ui@0.1.0 pretest
> node tests/support/clean-results.mjs tests/allure-results


> @imagesquid/ui@0.1.0 test
> vitest run


 RUN  v4.1.12 /home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/imagesquid

 ✓ tests/unit/test-reporting.test.ts (12 tests) 26ms
 ✓ tests/unit/theme.test.ts (10 tests) 13ms
 ✓ tests/unit/test-scripts.test.ts (10 tests) 683ms
 ✓ tests/ui/distribution.test.ts (5 tests) 122ms

 Test Files  4 passed (4)
      Tests  37 passed (37)
   Start at  23:49:00
   Duration  1.69s (transform 882ms, setup 238ms, import 970ms, tests 844ms, environment 1.73s)

JUNIT report written to /home/user/filesquid/.claude/worktrees/agent-a2375308b5fb6e7d4/apps/imagesquid/tests/allure-results/vitest-junit.xml

vitest exit 0
Running 5 tests using 1 worker

  ✓  1 [chromium] › tests/e2e/preview/production-build.spec.ts:57:1 › the built page is static, mounts the shell and defines every custom element (217ms)
  ✓  2 [chromium] › tests/e2e/preview/production-build.spec.ts:74:1 › navigation, API status and theme legend words come from the active label table (151ms)
  ✓  3 [chromium] › tests/e2e/preview/production-build.spec.ts:98:1 › computed ImageSquid token values are exact in light and dark (151ms)
  ✓  4 [chromium] › tests/e2e/preview/production-build.spec.ts:114:1 › a refused health call shows the unavailable alert (129ms)
  ✓  5 [chromium] › tests/e2e/preview/production-build.spec.ts:127:1 › the library flow runs in ImageSquid with its own label table (UI-1.1) (195ms)

  5 passed (5.6s)
exit 0
```
