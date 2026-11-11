# Task UI-F2.16 Walkthrough: Logs view and journal export

Branch `feat/ui-f2.16-logs-view-close-out`, the close-out of Task UI-F2.16, the last open row of global Task [FS-2.9](../../../../../resources/implementation-plan.md#task-fs-2.9). Plan row: [Task UI-F2.16](../plan.md). Mockups (active suite v3): [38-logs-view](../../../../../resources/ui-mockups/v3/38-logs-view.svg), [38-logs-view-dark](../../../../../resources/ui-mockups/v3/38-logs-view-dark.svg), [39-logs-export](../../../../../resources/ui-mockups/v3/39-logs-export.svg).

The implementation reached `main` with [PR #171](https://github.com/arkrim-tech/filesquid/pull/171) (branch `feat/ui-f2.16-logs-view`, head `ce11aac`, merged 2026-11-02). That PR did not close the task: the plan and checklist boxes stayed open, 50 of the 53 ToDos gates were unticked and there was no walkthrough. This branch checks every gate against the code and tests on `main`, adds the one missing test file, records the review findings of PR #171, and closes the row.

## SRS mapping

| SRS ID and title | Where it is tested |
| --- | --- |
| [UI-S-021](../srs.md) Logs view: the single place to browse the audit log for the desktop owner, from a sidebar item directly below Settings; all journal entries newest first with paging; filters by time range, library, operation, outcome and level, and by file, object or operation ID; one entry in detail; a committed operation linked to its Provenance activities, failed and refused entries and single-file downloads labelled journal-only; opened filtered from the Provenance tab's View in Logs; live updates from `journal.entry` events; export of the whole or the filtered journal as JSON Lines or CSV; refusal for a non-owner | `tests/ui/logs-view.test.ts`, `log-entry.test.ts`, `logs-export.test.ts`, `logs-route.test.ts`, `app-shell.test.ts`, `files-route-open.test.ts`; `tests/unit/logs-model.test.ts`, `logs-source.test.ts`, `logs-client.test.ts`, `logs-live.test.ts`, `logs-hash.test.ts`, `files-open-link.test.ts`; Playwright `tests/e2e/preview/logs-view.spec.ts` (behaviour) |
| [UI-S-023](../srs.md) Client event delivery: console warnings and errors to `POST /api/v1/client-events`, each with a client event ID kept for life; an in-memory queue of at most 10,000 events, batches of at most 20 events and 16 KiB, at most 60 requests a minute; write-through to IndexedDB, spill within 50 MiB, persistence at page close and delivery oldest first on the next open before new events, only under the host and principal that raised them; retry after 429, 401 or 403, 5xx and a network failure with backoff from 1 s doubling to 60 s, never before Retry-After; a 400 batch split into single events, a refused one replaced by `client_event_refused`; one `ui.console.overflow` count (1 to 1,000,000) at page close only when IndexedDB is unavailable or refuses a write | `tests/unit/client-event-queue.test.ts`, `client-event-store.test.ts` (added here), `console-forwarding.test.ts`, `client-events-start.test.ts`, `logs-client.test.ts`; Playwright `tests/e2e/preview/client-events.spec.ts` |
| [UI-S-032](../srs.md) Monitor and Logs composition: the Logs view of mockups 38 and 38-dark and the Download logs dialog of mockup 39 | Playwright `logs-view.spec.ts`, light and dark (boxes within 2 px, computed styles) |
| [UI-S-024](../srs.md) Visual constants of the active mockup suite | Playwright `logs-view.spec.ts` (fonts, sizes, weights, token colours in light and dark, radii, the 360 px detail panel, the 560 x 632 dialog and its 248 x 58 format cards) |
| [UI-S-016](../srs.md) Every `@filesquid/ui` custom element MUST meet these accessibility contracts | `logs-view.test.ts` and `log-entry.test.ts` (axe, named regions, row buttons and arrow keys), `logs-export.test.ts` (modal dialog, focus trap, Escape, focus return) |
| [UI-S-005](../srs.md) A distribution's label table MUST define every key of the default table | `tests/unit/distribution-tables.test.ts` (whole `npm test` run) |

PRD: [F-061](../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring) Activity journal and live health monitoring ([SRS-F-061](../../../../../resources/srs.md#srs-f-061-activity-journal-and-live-health-monitoring)), [NFR-006](../../../../../resources/prd.md#nfr-006-accessibility-and-usability) Accessibility and usability. API contract: API-S-030 and API-S-031 ([API SRS](../../../crates/api/resources/srs.md)). Gherkin: [logs_view_and_client_events_UI-S-021.feature](../../tests/features/logs_view_and_client_events_UI-S-021.feature) (feature-writer conventions; its steps are bound by the Vitest and Playwright suites named in its header) and the API lane's [durable_journal_and_journal_tab_F-061.feature](../../../crates/api/tests/features/durable_journal_and_journal_tab_F-061.feature).

## What this branch changed

- **Added** `tests/unit/client-event-store.test.ts` (14 tests): the `EventStore` contract on the in-memory store the queue tests use (host scoping, oldest first by sequence, the exclusive range, the limit and the principal filter, replacement by ID counted once, removal, the greatest sequence), the 50 MiB budget in UTF-8 bytes with a whole write over it refused and nothing changed, a replacement counted by its new size, `UnavailableEventStore` (every write refused as unavailable, nothing held), and `IndexedDbEventStore.open` falling back to it with no IndexedDB and when IndexedDB refuses to open. The real IndexedDB store runs in Chromium in `client-events.spec.ts`.
- **Closed** the row: this walkthrough, the Plan and ToDos deleted, the plan row ticked with its Status line, the checklist row and global Task FS-2.9 ticked (all nine rows are now ticked), the MVP progress block redrawn, and the root README's ADR 033 table row for the Logs view and console forwarding set to Done.
- No source file changed. TDD-ENFORCE: the new tests cover code already on `main`; they were shown to fail against a deliberately broken store (red run below) before the green run.

## Gate check of the ToDos

Every gate of `Task-UI-F2.16-ToDos.md` was checked against the code on `main` and the test that proves it, and that test was run (output below).

| Gate | Result | Evidence |
| --- | --- | --- |
| Start: branch, dependencies UI-F2.7, API-F2.10, UI-F2.15, holistic review | Met | Ticked in PR #171; the three dependencies are ticked in the plan and checklist |
| Unit `logs-model.test.ts` | Met | 40 tests: ranges, query, search classification, rows, outcome kinds, journal-only, footer words, export names and scope |
| Unit `logs-source.test.ts` | Met | 8 tests; the query strings, `names`, the entry read and the CSV export paging with one header row are in `logs-client.test.ts` (10 tests) |
| Unit `client-io.test.ts` additions (`Retry-After`, `postClientEvents`) | Met, in another file | `logs-client.test.ts` "client events (API-S-030)": Retry-After in whole seconds, none when absent, the batch posted, the keep-alive request |
| Unit `client-event-queue.test.ts` | Met | 35 tests covering every threshold the gate lists |
| Unit `client-event-store.test.ts` (store contract on the memory store; budget refusal) | **Not met on `main`: the file did not exist.** Added here | 14 tests, red and green runs below |
| Unit `console-forwarding.test.ts` | Met | 9 tests: warn and error only, fixed component, error code, never message, URL, file name or stack, own output ignored |
| UI `logs-view`, `log-entry`, `logs-export`, `logs-route`, shell panel width, files-route deep link | Met | 38, 16, 18 and 8 tests; `logs-route.test.ts` "is 320 px for a panel and 360 px while the slotted panel asks for the wide region"; `files-route-open.test.ts` 7 tests |
| Gherkin feature | Met | `tests/features/logs_view_and_client_events_UI-S-021.feature` |
| Playwright `logs-view.spec.ts`, `client-events.spec.ts` | Met | 39 passed, below |
| Implementation gates (client, sources, time ranges, search classification, `#/logs` filters, `<fs-logs>`, paging, footer, live, states, `<fs-log-entry>`, 360 px panel, Download dialog, files deep link, queue thresholds, spill, write-through, principals, 400 split, overflow, console capture, wiring and labels, the write-through test) | Met | Each by the unit and UI tests named in the SRS table; wiring in `src/app/main.ts` (`startClientEvents`, `bindLogsRoute`); labels by `distribution-tables.test.ts`; the write-through gate by "delivers an event queued while storage accepted writes after a page that ended without pagehide" |
| Mockup fidelity (head, filter row, table, Load older and footer, detail panel 360 px, dialog 560 x 632 with 248 x 58 cards, labels, tokens, no console errors) | Met | `logs-view.spec.ts` "light theme" and "dark theme", six tests each |
| Behaviour (sidebar below Settings, paging, every filter, search, View in Logs, detail and PROV-O link, journal-only, live entry, JSON Lines and CSV export, non-owner refusal, client events after 429, 401, 403, 5xx and a network failure, 400 split, spill, page-close persistence, overflow) | Met | `logs-view.spec.ts` "behaviour" (14 tests) and `client-events.spec.ts` (13 tests). "Delivered on the next open before new events" is proven by the unit test "delivers persisted events oldest first before the events raised after the page opened"; the Playwright reload test proves the persisted events are delivered oldest first with their original IDs |
| Finish: format, lint, typecheck, tests, Playwright, walkthrough, boxes, hook, PR | Met by this branch | Below |

Not checkable from the code: whether PR #171 wrote its tests before the implementation. Its red runs were never recorded because it had no walkthrough.

## Review findings of PR #171

PR #171 had one review round, by the reviewer in use before the AI PR Agent workflow (2026-11-02, 60 review threads: 1 P0, 9 P1, 44 P2, 6 P3; no ordinary PR comments). All 60 threads are resolved. 59 were answered "Fixed in 79504df3"; one was rebutted. Commit `79504df3` ("fix(ui): address Cubic review on the Logs view", 32 files) was merged with the PR and was not reviewed again. **No finding is unresolved.**

Spot checks of the fixes against `main`, each with the test that pins it:

- P0 stale acknowledgement after a credential change: "does not let a stale acknowledgement remove a parked event or name the new principal" (`client-event-queue.test.ts`).
- P1 denied state keeps the subscription: `fs-logs.ts` stops the live subscription in the denied state and hides the Live chip.
- P1 principal learned only in memory: "stores the acknowledgement principal on an event that is still queued".
- P1 refused park on a credential change: "counts an event it could not park when the credential changes".
- P1 non-monotonic sequence floor: "delivers events stored in the same millisecond, oldest first" (the floor reads `maxSeq`).
- P1 pagehide refusal not counted: "counts an event the page-close write refuses, in the keep-alive overflow".
- P2 keep-alive not asserted in Playwright: `client-events.spec.ts` records `keepalive` on each client-events fetch and asserts it.
- P2 wide panel on narrow screens: `fs-app-shell.ts` gives `aside.panel[data-wide]` `inline-size: auto; max-inline-size: 100%` in the narrow rule.
- P2 deep link beyond the first page and the wrong object: `fs-file-browser.ts` matches `urn:uuid:<file>` and loads more pages until the file is found.
- P2 overflow bound at 1,000,001: `overflowCount(1_000_001)` is asserted.
- P2 live-event race in Playwright: the spec waits for `api.connected` before emitting.

The rebutted finding (P2, `client-events.spec.ts`, "assert `queued` and `over_capacity` instead of `count`") is wrong, and the rebuttal stands: `ClientOverflowEvent` in `apps/filesquid/crates/api/openapi.json` requires `kind`, `id`, `component` and `count` (integer 1 to 1,000,000), allows only an optional `principal`, and sets `additionalProperties: false`, so `queued` and `over_capacity` are not fields of the contract.

## Global Task FS-2.9

With UI-F2.16 ticked, all nine module rows of Task FS-2.9 are ticked (API-F2.10, IO-1.6, API-F2.12, CLI-0.2, API-F2.11, CL-1.9, DE-0.11, UI-F2.15, UI-F2.16), each with its walkthrough. The global box is ticked as the orchestrator instructed. The FS-2.9 Agent Prompt also names a global walkthrough, `apps/filesquid/crates/api/resources/tasks/Task-FS-2.9-Walkthrough.md`, which does not exist on `main`. This UI branch does not write it: a file inside the `api` crate changes the hook's scope to `rust:`, and this lane builds no Rust. It is left for the owner or a documentation task on that lane.

## How it was tested

Suites run, all in `apps/filesquid/ui`:

1. `npx prettier --check .` (and `apps/imagesquid` with this package's Prettier), `npx eslint .`, `npx tsc --noEmit`: clean.
2. `npm test` (Vitest, jsdom): 123 files, 2,832 tests passed, 1 skipped (the same skip as on `main`).
3. The UI-F2.16 Vitest files alone, verbose: 16 files, 260 tests passed; then the added store test: 14 passed.
4. Playwright preview suite (`playwright.preview.config.ts`: `vite build` plus `vite preview` on port 4173, 1440 x 900, light and dark) for `tests/e2e/preview/logs-view.spec.ts` and `tests/e2e/preview/client-events.spec.ts`, with `FILESQUID_E2E=1`, under the shared Playwright lock: 39 passed. The first attempt failed before any test ran because this machine lacked the Chromium build Playwright 1.64.1 needs (`chromium_headless_shell-1248`); `npx playwright install chromium` fixed it. The `ECONNREFUSED 127.0.0.1:8083` lines in the preview server's output are the optional start-up reads (preferences, recent libraries) that the preview suite excuses; no test failed on them.
5. `bash scripts/pre-commit.sh` from the repository root, without `FILESQUID_E2E` (so its Playwright step was skipped; the specs ran in step 4): `✅ All Pre-Commit Hooks Passed! Test scope: npm:apps/filesquid/ui (118s)`. No Cargo step ran.

Not run: the server-backed Playwright suite (`playwright.config.ts`, `npm run test:e2e`), which needs a `filesquid-server` build; this lane builds no Rust. The rest of the preview suite (specs of other tasks) was not run on this branch. No coverage tool is configured in this package, so there is no coverage report.

No environment variables were mocked.

### Raw output

Red run: `npx vitest run tests/unit/client-event-store.test.ts` against a store temporarily broken in two places (`eventBytes` counting UTF-16 length, the budget set to 5 MiB), then restored. One test was renamed after this run ("…so a larger replacement can pass the budget" is now "…refusing a larger one that would pass the budget"); its body did not change:

```text
     × counts the UTF-8 bytes of the event's JSON text, not its UTF-16 length
     ✓ lists one host's events oldest first by sequence, never another host's
     ✓ reads a range of sequences exclusively, at most the limit, oldest first
     ✓ reads only the principals asked for, null being the unknown principal
     ✓ replaces an event written again under the same ID, and counts its bytes once
     ✓ removes events by ID, ignores an unknown ID, and gives their bytes back
     ✓ answers the greatest stored sequence of a host, or 0 for a host with none
     × is 50 MiB for the origin
     ✓ refuses a whole write that would pass the budget, keeping what was stored
     ✓ accepts a write again once removals make room
     ✓ counts a replacement by its new size, so a larger replacement can pass the budget
     ✓ refuses every write as unavailable and holds nothing
     ✓ is what the IndexedDB store opens as in a page with no IndexedDB
     ✓ is what the IndexedDB store opens as when IndexedDB refuses to open (a private window)
⎯⎯⎯⎯⎯⎯⎯ Failed Tests 2 ⎯⎯⎯⎯⎯⎯⎯
AssertionError: expected 216 to be 217 // Object.is equality
AssertionError: expected 5242880 to be 52428800 // Object.is equality
 Test Files  1 failed (1)
      Tests  2 failed | 12 passed (14)
```

Green run: `npx vitest run --reporter=verbose tests/unit/client-event-store.test.ts`:

```text
 ✓ tests/unit/client-event-store.test.ts > eventBytes > counts the UTF-8 bytes of the event's JSON text, not its UTF-16 length
 ✓ tests/unit/client-event-store.test.ts > the store contract (in-memory store) > lists one host's events oldest first by sequence, never another host's
 ✓ tests/unit/client-event-store.test.ts > the store contract (in-memory store) > reads a range of sequences exclusively, at most the limit, oldest first
 ✓ tests/unit/client-event-store.test.ts > the store contract (in-memory store) > reads only the principals asked for, null being the unknown principal
 ✓ tests/unit/client-event-store.test.ts > the store contract (in-memory store) > replaces an event written again under the same ID, and counts its bytes once
 ✓ tests/unit/client-event-store.test.ts > the store contract (in-memory store) > removes events by ID, ignores an unknown ID, and gives their bytes back
 ✓ tests/unit/client-event-store.test.ts > the store contract (in-memory store) > answers the greatest stored sequence of a host, or 0 for a host with none
 ✓ tests/unit/client-event-store.test.ts > the byte budget > is 50 MiB for the origin
 ✓ tests/unit/client-event-store.test.ts > the byte budget > refuses a whole write that would pass the budget, keeping what was stored
 ✓ tests/unit/client-event-store.test.ts > the byte budget > accepts a write again once removals make room
 ✓ tests/unit/client-event-store.test.ts > the byte budget > counts a replacement by its new size, refusing a larger one that would pass the budget
 ✓ tests/unit/client-event-store.test.ts > a store with no storage > refuses every write as unavailable and holds nothing
 ✓ tests/unit/client-event-store.test.ts > a store with no storage > is what the IndexedDB store opens as in a page with no IndexedDB
 ✓ tests/unit/client-event-store.test.ts > a store with no storage > is what the IndexedDB store opens as when IndexedDB refuses to open (a private window)
      Tests  14 passed (14)
```

The UI-F2.16 Vitest files: `npx vitest run --reporter=verbose` over the 16 files, passing tests counted per file, then the summary:

```text
ui/app-shell.test.ts                          27 tests
ui/files-route-open.test.ts                   7 tests
ui/log-entry.test.ts                          16 tests
ui/logs-export.test.ts                        18 tests
ui/logs-route.test.ts                         8 tests
ui/logs-view.test.ts                          38 tests
unit/client-event-queue.test.ts               35 tests
unit/client-events-start.test.ts              5 tests
unit/client-io.test.ts                        23 tests
unit/console-forwarding.test.ts               9 tests
unit/files-open-link.test.ts                  5 tests
unit/logs-client.test.ts                      10 tests
unit/logs-hash.test.ts                        5 tests
unit/logs-live.test.ts                        6 tests
unit/logs-model.test.ts                       40 tests
unit/logs-source.test.ts                      8 tests
 Test Files  16 passed (16)
      Tests  260 passed (260)
```

`npm test`:

```text
 Test Files  123 passed (123)
      Tests  2832 passed | 1 skipped (2833)
```

`flock -w 5400 /dev/shm/filesquid-ui-playwright.lock npx playwright test -c playwright.preview.config.ts tests/e2e/preview/logs-view.spec.ts tests/e2e/preview/client-events.spec.ts --reporter=list` with `FILESQUID_E2E=1` (the preview server's own lines left out):

```text
Running 39 tests using 2 workers
  ✓   1 [chromium] › tests/e2e/preview/client-events.spec.ts:112:3 › delivery › sends a console warning as one event with a level, the fixed component, a code and an ID, no text
  ✓   2 [chromium] › tests/e2e/preview/logs-view.spec.ts:105:5 › light theme › Logs (38): head, filter row and chips match the mockup
  ✓   4 [chromium] › tests/e2e/preview/logs-view.spec.ts:166:5 › light theme › Logs (38): the search field and the five chips
  ✓   5 [chromium] › tests/e2e/preview/logs-view.spec.ts:207:5 › light theme › Logs (38): the table, its rows, outcome chips, journal-only tags and the selected row
  ✓   6 [chromium] › tests/e2e/preview/logs-view.spec.ts:319:5 › light theme › Logs (38): the line under the rows, Load older and the footer
  ✓   7 [chromium] › tests/e2e/preview/logs-view.spec.ts:356:5 › light theme › Logs (38): the 360 px detail panel and its rows
  ✓   8 [chromium] › tests/e2e/preview/logs-view.spec.ts:434:5 › light theme › Download logs (39): the dialog, scope, formats, note, file name and buttons
  ✓   3 [chromium] › tests/e2e/preview/client-events.spec.ts:135:3 › delivery › retries after a 429 no sooner than Retry-After, with the same events, each journaled once
  ✓   9 [chromium] › tests/e2e/preview/logs-view.spec.ts:105:5 › dark theme › Logs (38): head, filter row and chips match the mockup
  ✓  11 [chromium] › tests/e2e/preview/logs-view.spec.ts:166:5 › dark theme › Logs (38): the search field and the five chips
  ✓  12 [chromium] › tests/e2e/preview/logs-view.spec.ts:207:5 › dark theme › Logs (38): the table, its rows, outcome chips, journal-only tags and the selected row
  ✓  13 [chromium] › tests/e2e/preview/logs-view.spec.ts:319:5 › dark theme › Logs (38): the line under the rows, Load older and the footer
  ✓  14 [chromium] › tests/e2e/preview/logs-view.spec.ts:356:5 › dark theme › Logs (38): the 360 px detail panel and its rows
  ✓  10 [chromium] › tests/e2e/preview/client-events.spec.ts:158:5 › delivery › retries after a 401 with backoff from 1 s, never merging events
  ✓  15 [chromium] › tests/e2e/preview/logs-view.spec.ts:434:5 › dark theme › Download logs (39): the dialog, scope, formats, note, file name and buttons
  ✓  17 [chromium] › tests/e2e/preview/logs-view.spec.ts:547:3 › behaviour › the sidebar shows Logs directly below Settings, current on the Logs view
  ✓  18 [chromium] › tests/e2e/preview/logs-view.spec.ts:566:3 › behaviour › lists the journal newest first and says how many are shown
  ✓  19 [chromium] › tests/e2e/preview/logs-view.spec.ts:579:3 › behaviour › pages: Load older adds the next page, with the cursor, until the end
  ✓  16 [chromium] › tests/e2e/preview/client-events.spec.ts:158:5 › delivery › retries after a 403 with backoff from 1 s, never merging events
  ✓  20 [chromium] › tests/e2e/preview/logs-view.spec.ts:593:3 › behaviour › every filter narrows the list: range, library, operation, outcome and level
  ✓  22 [chromium] › tests/e2e/preview/logs-view.spec.ts:631:3 › behaviour › searches by operation, by object ID and by operation ID
  ✓  23 [chromium] › tests/e2e/preview/logs-view.spec.ts:663:3 › behaviour › opens filtered from View in Logs: by file and by operation
  ✓  24 [chromium] › tests/e2e/preview/logs-view.spec.ts:687:3 › behaviour › the detail shows the entry and links View provenance and Show file to the file's panel
  ✓  21 [chromium] › tests/e2e/preview/client-events.spec.ts:158:5 › delivery › retries after a 503 with backoff from 1 s, never merging events
  ✓  25 [chromium] › tests/e2e/preview/logs-view.spec.ts:709:3 › behaviour › a failed entry and a refusal are journal-only and offer no View provenance
  ✓  27 [chromium] › tests/e2e/preview/logs-view.spec.ts:725:3 › behaviour › a live entry is counted and added when the chip is chosen
  ✓  28 [chromium] › tests/e2e/preview/logs-view.spec.ts:749:3 › behaviour › downloads the filtered journal as JSON Lines, oldest first, with opaque ids only
  ✓  29 [chromium] › tests/e2e/preview/logs-view.spec.ts:778:3 › behaviour › downloads a time range as CSV with one header row, though the export took several requests
  ✓  30 [chromium] › tests/e2e/preview/logs-view.spec.ts:813:3 › behaviour › shows a permission state with no entries for a caller who is not the owner
  ✓  31 [chromium] › tests/e2e/preview/logs-view.spec.ts:830:3 › behaviour › shows the empty state for an empty journal
  ✓  32 [chromium] › tests/e2e/preview/logs-view.spec.ts:838:3 › behaviour › shows no match with Clear filters, and the list returns when they are cleared
  ✓  26 [chromium] › tests/e2e/preview/client-events.spec.ts:158:5 › delivery › retries after a network failure with backoff from 1 s, never merging events
  ✓  33 [chromium] › tests/e2e/preview/client-events.spec.ts:171:3 › delivery › splits a refused batch into single events, each delivered once as its own entry
  ✓  34 [chromium] › tests/e2e/preview/client-events.spec.ts:189:3 › delivery › replaces an event the route refuses on its own with one client_event_refused event
  ✓  35 [chromium] › tests/e2e/preview/client-events.spec.ts:242:3 › storage › writes each queued event through to IndexedDB as it is queued, before any page close
  ✓  36 [chromium] › tests/e2e/preview/client-events.spec.ts:253:3 › storage › a full queue spills into IndexedDB and loses nothing
  ✓  37 [chromium] › tests/e2e/preview/client-events.spec.ts:263:3 › storage › events persisted at page close are delivered, oldest first, on the next open
  ✓  38 [chromium] › tests/e2e/preview/client-events.spec.ts:313:3 › the one gap › counts the events it could not keep when IndexedDB is unavailable, and sends it at page close
  ✓  39 [chromium] › tests/e2e/preview/client-events.spec.ts:338:3 › the one gap › counts them too when IndexedDB refuses a write
  39 passed
```
