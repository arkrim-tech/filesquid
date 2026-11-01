# Task UI-F2.16 ToDos: Logs view and journal export

Each line is a gate: coded and tested before the task is complete. See [the Plan](./Task-UI-F2.16-Plan.md).

## Start

- [x] Branch `feat/ui-f2.16-logs-view` from `origin/main` (82af8a92); rules, plan row, SRS, ADR 033, mockups 38 and 39 read
- [x] Dependencies UI-F2.7, API-F2.10 and UI-F2.15 are merged
- [x] Holistic-UI review of UI-F2.7, UI-S-020 and UI-S-021 (Plan)

## Tests first (red run recorded in the walkthrough)

- [ ] Unit: `logs-model.test.ts` (ranges, query, search classification, row view, outcome kinds, journal-only, footer words, export names and scope)
- [ ] Unit: `logs-source.test.ts` (query strings, `names`, entry, CSV export paging without repeated header, libraries, subscribe)
- [ ] Unit: `client-io.test.ts` additions (`Retry-After`, `postClientEvents`)
- [ ] Unit: `client-event-queue.test.ts` (batches of 20 and 16 KiB, 60 requests a minute, backoff 1 s doubling to 60 s, `Retry-After`, 400 split, refused replacement, 10,000 cap, spill, refill, write-through, pagehide persistence, principal hold, overflow count 1 to 1,000,000, no merge, once-only IDs)
- [ ] Unit: `client-event-store.test.ts` (the store contract on the memory store; budget refusal)
- [ ] Unit: `console-forwarding.test.ts` (levels, component, no message text, own output ignored)
- [ ] UI: `logs-view.test.ts`, `log-entry.test.ts`, `logs-export.test.ts`, `logs-route.test.ts`, shell panel width, files-route deep link
- [ ] Gherkin: `logs_view_and_client_events_UI-S-021.feature`
- [ ] Playwright (preview): `logs-view.spec.ts`, `client-events.spec.ts`

## Implementation

- [ ] `JournalFilters` gains library, object, file, operation_id, names; `adminJournalPage`, `adminJournalEntry`, CSV-safe export, `postClientEvents`, `ApiError.retryAfterMs`
- [ ] `LogsSource` and `ClientLogsSource`
- [ ] Time ranges: last hour, last 24 hours (default), last 7 days, last 30 days, all time
- [ ] Search classification: `urn:uuid:` object, `<object>@vN` object, UUID (operation ID, else file), dotted operation name; anything else is not searchable (hint, no query)
- [ ] `#/logs?file=` and `#/logs?operation_id=` bound through `parseLogsHash`, a removable filter chip, the search field shows the ID
- [ ] `<fs-logs>`: head (title, intro, Live chip with count, Download…), filter row, five chips with menus, table (6 columns), rows 34 px, selected row tint and 2.5 px bar, outcome chips, journal-only tag, `—` for none, short operation ID
- [ ] Paging: newest first, Load older, "Showing {shown} of {total}" worded only as far as the API says (no invented total)
- [ ] Footer: time zone sentence and entries count in the shell footer slots
- [ ] Live: `journal.entry` events matching the filter add a "Live · {count} new" count; clicking the chip shows them; `events.resync` reads again
- [ ] States: loading, empty, no match, error with Try again, denied for a non-owner with no entries
- [ ] `<fs-log-entry>` over `PropertyTable`: every row of mockup 38, View provenance (committed operations only), Show file, Copy as JSON, the opaque-ids note, journal-only wording
- [ ] Panel slot widens to 360 px (`data-panel-wide`), file panel stays 320 px
- [ ] Download dialog (mockup 39): scope radios, format cards, note, file name, Choose…, Cancel, Download; focus trap, Escape, busy state, failure alert; export through the route for the scope (current filter, time range, all)
- [ ] Files route deep link `?open=<file id>&tab=<tab>`; browser opens the file, panel selects the tab
- [ ] `ClientEventQueue` thresholds: memory 10,000; batch 20 and 16 KiB; 60 requests a minute; backoff from 1 s doubling to 60 s; never before `Retry-After`
- [ ] Spill to IndexedDB within 50 MiB; refill oldest first
- [ ] Write-through while storage accepts; remove on acknowledgement; persist at `pagehide`; deliver on next open oldest first before new events
- [ ] Host and principal stored with each event; foreign principals' events held untouched
- [ ] 400 split into single events; a refused one replaced by `client_event_refused` with the component
- [ ] Overflow: count of events not kept, 1 to 1,000,000, one `ui.console.overflow` keep-alive at page close, own client event ID, no error code or IDs
- [ ] Console capture: warn and error only, fixed component, error code, opaque IDs, never message text, URLs, file names or stacks
- [ ] Wired in `main.ts` for the default distribution; labels in both tables; no new token needed (colours from existing tokens)
- [ ] Failing-first test: an event queued while storage accepts writes is delivered on the next open after a page that ended without `pagehide`

## Mockup fidelity (Playwright, 1440 x 900, light and dark, within 2 px)

- [ ] Title, intro, Live chip, Download… button boxes and styles
- [ ] Filter field and five chips boxes and styles
- [ ] Table head and row boxes, fonts, colours, outcome chips, journal-only tag, selected row
- [ ] Load older and the "Showing" line; footer sentences
- [ ] Detail panel 360 px and every row; View provenance, Show file, Copy as JSON, note
- [ ] Download dialog 560 x 632, scope, format cards (248 x 58), note, file name, buttons
- [ ] Every string against the active label table; every colour token in light and dark; no console errors

## Behaviour (Playwright against the production build)

- [ ] Sidebar item below Settings, current on `#/logs`, desktop host only
- [ ] Paging, every filter (range, library, operation, outcome, level), search by operation, object and file ID
- [ ] Opening filtered from View in Logs (file and operation)
- [ ] Detail view, PROV-O link and journal-only labels, a live entry arriving
- [ ] JSON Lines and CSV export (saved bytes); refusal for a non-owner
- [ ] Client events after 429, 401 or 403, 5xx and a network failure (Retry-After and backoff, one entry per event); a 400 batch split with `client_event_refused`; a full queue spilling to IndexedDB; events persisted at page close and delivered first on the next open; IndexedDB unavailable or refusing with the overflow count at page close

## Finish

- [ ] `npx prettier --write .` in `apps/filesquid/ui` and in `apps/imagesquid` (with the UI package's Prettier), `npx eslint . --fix`, `npx tsc --noEmit`, `npm test`
- [ ] Playwright preview suite under the shared lock
- [ ] Walkthrough with the SRS IDs and titles and the raw output; Plan and ToDos deleted
- [ ] Plan row, checklist boxes, `update_mvp_progress.py`, README, ImageSquid labels
- [ ] `bash scripts/pre-commit.sh` passes (scope npm), commit, merge `origin/main`, push, PR, Cubic loop
