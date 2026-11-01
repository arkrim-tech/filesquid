# Task UI-F2.16 Plan: Logs view and journal export

Branch `feat/ui-f2.16-logs-view`, dispatched by global Task FS-2.9. Plan row: [Task UI-F2.16](../plan.md).

## Traceability

- PRD: [F-061](../../../../../resources/prd.md#f-061-activity-journal-and-live-health-monitoring) Activity journal and live health monitoring, [F-067](../../../../../resources/prd.md#f-067-prov-o-history-of-every-version-and-change) PROV-O history, [NFR-006](../../../../../resources/prd.md#nfr-006-accessibility-and-usability) Accessibility and usability.
- SRS: [UI-S-021](../srs.md) Logs view, [UI-S-023](../srs.md) Client event delivery, [UI-S-024](../srs.md) constants, [UI-S-032](../srs.md) Monitor and Logs composition, [UI-S-016](../srs.md) accessibility, API-S-030 and API-S-031 ([API SRS](../../crates/api/resources/srs.md)), [ADR 033](../../../../../resources/architecture/033-activity-journal-and-live-health-monitoring.md).
- Mockups (v3): [38-logs-view](../../../../../resources/ui-mockups/v3/38-logs-view.svg), [38-logs-view-dark](../../../../../resources/ui-mockups/v3/38-logs-view-dark.svg), [39-logs-export](../../../../../resources/ui-mockups/v3/39-logs-export.svg).
- Gherkin: `tests/features/logs_view_and_client_events_UI-S-021.feature` (written with the feature-writer conventions); the API lane's [durable_journal_and_journal_tab_F-061.feature](../../crates/api/tests/features/durable_journal_and_journal_tab_F-061.feature) names the same scenarios.

## Holistic review (holistic-ui.md)

- UI-F2.7 (Monitor, done): the Monitor keeps its own journal tail. Both views read the same routes through `FileSquidClient`; the journal filter type is shared (`JournalFilters`), so the Monitor needs no change and Logs does not fork it.
- UI-F2.15 (done): the Provenance tab links `#/logs?file=<uuid>` and `#/logs?operation_id=<id>` (`logsHash`, `parseLogsHash` in `routes.ts`); the Logs route binds exactly those. The Logs entry detail is built on `PropertyTable` (its rows, copy, states), and the Logs list reuses the table's state vocabulary (`errorState`, `failureWords`, `stateButton`, `PROPERTY_TABLE_STYLE` rules for placeholder rows and states).
- UI-S-020 (Provenance tab) and UI-S-021 (Logs) meet at **View provenance** and **Show file**, which open the file panel (Provenance tab or File info) of the entry's file, so the files route gets one deep-link form (`?open=<file id>&tab=<tab>`) rather than a Logs-only scaffold.
- No later UI task replaces the Logs interaction model (UI-F2.17 and UI-F2.18 do not touch it).

## Owner decisions that bind this task

- The journal holds opaque IDs only; the view shows what the journal routes return. A display name appears only through `names=true` (API-S-031), never from a lookup the UI invents, and the export never carries names (owner 2026-10-31, "accept recommendations").
- Cubic capacity: when the review ends "neutral" with "AI review line limit reached", or Cubic is unavailable, push and hand back `BLOCKED: Cubic capacity`.

## Contract on main (no unmerged fields)

- `GET /api/v1/admin/journal` filters: `from`, `to`, `actor`, `operation`, `outcome` (`info|started|succeeded|failed|stalled|resumed|cancelled`), `level`, `process`, `library`, `object`, `file`, `operation_id`, `after`, `limit` (1 to 1,000, default 100), `order`, `names`. Page: `{ entries: JournalEntryView[], next_cursor }`.
- `JournalEntryView`: `id`, `time`, `host`, `process_id`, `actor`, `operation`, `outcome`, `level`, `target_ids`, `duration_ms`, `error_code`, `operation_id`, `object_ref`, `client_event_id`, `provenance` (`journal_only`, `operation_id`, `object_ref`, `record`, `activities`), `display_name`.
- `GET /api/v1/admin/journal/entries/{entry}?names=` one entry.
- `GET /api/v1/admin/journal/export` same filters plus `format=jsonl|csv`, at most 50,000 entries a response, `X-Journal-Next-Cursor` to continue.
- `POST /api/v1/client-events`: at most 20 events and 16 KiB; `429` with `Retry-After`; acknowledgement `{ acknowledged, journaled, principal }`.
- Not in the contract (stubbed nowhere, reported): a count of the entries a filter matches, a count of all entries, the time of the first entry, the library of an entry as a field. A refusal is the operation `api.auth.denied` with outcome `failed`.

## Constraints extracted from the SRS and the mockups (each has a gate in the ToDos)

### UI-S-021 behaviour

1. A **Logs** view for the desktop owner (server administrators later), reached from the sidebar item placed directly below Settings with the Journal item's icon; its own view, not the Monitor's tail.
2. Lists all journal entries newest first with paging.
3. Filters: time range, library, operation, outcome, level, and by file, object or operation ID (the search field).
4. One entry in detail.
5. An entry of a committed operation links to its Provenance activities (one per committed object version, sharing the operation ID); failed and refused entries and `file.download` entries are labelled journal-only.
6. Opens filtered from the Provenance tab's **View in Logs** (`#/logs?file=<uuid>`, `#/logs?operation_id=<id>`).
7. Updates live from `journal.entry` events.
8. Export control: JSON Lines or CSV, whole or filtered, through `GET /api/v1/admin/journal/export`.
9. UI-S-016 accessibility contracts. Refusal for a non-owner is a permission state with no entries.
10. Rows show the file's display name beside its object ID where the API resolves it (`names=true`).

### UI-S-032 and mockups 38 and 39 (1440 x 900; a mockup y of n is n - 28 below its title bar)

- Title "Logs" 30 px 700 Georgia (`--fs-on-surface-heading`) at x 264; `logsIntro` "Every activity on this device, newest first. Only you can open it." 13 px 400 secondary, beside the title; **Live · {count} new** chip (11 px upper-case-free, 13 px 500 `--fs-on-primary-container` on `--fs-primary-container`, radius 4, 26 px tall, a 7 px `--fs-primary` dot) and **Download…** (secondary button, 32 px tall at the mockup, 14 px 600) at the right.
- Filter row: the search field (206 px wide, 32 px tall, radius 6, control outline `--fs-outline-control` 1.5 px, placeholder "File, operation or object id" 13 px `--fs-on-surface-placeholder`) and the chips "Last 24 hours", "Library: all", "Operation: all", "Outcome: all", "Level: all" (26 px tall, radius 4, 1 px `--fs-outline-strong`, 13 px 500 body text, 10 px inner padding, 6 px between chips; the time chip has a calendar icon).
- Table: heads Time (local), Level, Operation, Outcome, Target, Operation id (13 px 600 secondary); rows 34 px (1 px `--fs-outline` rule), time and level and operation and operation id in mono 12 px (level 600: `info` secondary, `warn` and `error` in `--fs-error`), outcome chip (11 px 500, 18 px tall, radius 4: ok on the active tint with `--fs-on-primary-container`, failed and refused outlined 1 px in `--fs-error` with error text), target 13 px body (the display name; the opaque ID when none), operation id mono 12 px `--fs-on-surface-muted` shortened to `a71c…3e90`, a "journal-only" mono 11 px 500 muted tag on failed and refused rows; the selected row on `--fs-primary-container` with a 2.5 px `--fs-primary` bar at its start; an entry with no outcome or no operation ID shows "—".
- Below the rows: "Showing {shown} of {total} entries in the last 24 hours" 13 px muted and **Load older** (secondary, 28 px at the mockup); footer "Times in your time zone ({zone}) · stored as UTC · kept forever" (13 px secondary) left and "{count} entries" right, in the shell's 40 px footer, which ends where the 360 px detail panel starts.
- Detail panel 360 px wide (the shell's panel region widens from 320 px for it): "Entry detail" 13 px 500 muted; the mono 16 px 600 operation and the outcome chip; rows Time (UTC) mono, Local time, Level mono, Outcome, Actor (`urn:uuid` mono, "Owner agent · this installation"), Software agent, Library, Object "urn:uuid:… → v{n}" mono, File with "· looked up in your library", Operation id mono 11 px, Duration; links **View provenance** and **Show file** (14 px 600 `--fs-on-primary-container`, ghost); **Copy as JSON** (secondary); the raised note "Logs store opaque ids only. Names are looked up in your library when shown." (13 px secondary, radius 8, `--fs-surface-raised`).
- Download dialog 560 x 632: title "Download logs" 22 px 700 Georgia, "Entries from this device's logs" 13 px secondary; Scope radios (Current filter, Time range, All entries, each with a 13 px muted line); Format as two 248 x 58 cards (the chosen one on `--fs-primary-container` with a 1.5 px `--fs-primary` border): JSON Lines ".jsonl · one entry per line, as stored" and CSV ".csv · one row per entry, flattened"; the note on `--fs-surface-raised` "Downloads keep opaque ids. File names, titles and paths are not added, so a download is safe to share for support."; File name (mono, 36 px field with a 2 px `--fs-focus-ring` border while focused) with **Choose…** and "Saved to Downloads. Choose… opens the macOS save dialog."; Cancel and **Download** (primary, white text, 38 px, radius 7); the scrim `--fs-scrim` at `--fs-scrim-opacity`; the dialog on `--fs-surface-overlay`, radius 10, 1 px `--fs-outline-strong`.
- Colours only through tokens, light and dark (UI-S-024): surface #ffffff / #141414, card #ffffff / #1c1c1c, raised #f5f5f5 / #242424, overlay #ffffff / #2c2c2c, outline #e0e0e0 / #2a2a2a, outline-strong #cccccc / #333333, control outline #8e8e8e / #757575, heading #000000 / #f5f5f5, body #111111 / #e0e0e0, secondary #555555 / #a0a0a0, muted #777777 / #6a6a6a, placeholder #999999 / #6a6a6a, active tint #f0faf4 / #0d2b18 with #0f8a39 / #4aca74 text, error #e53935 / #ef5350, primary #16ac49 with #ffffff text, focus ring #0f8a39 / #16ac49. Georgia headings, system sans text, mono for identifiers. Spacing 4, 8, 16, 24, 40, 64, 96. Radii: buttons 7, inputs 6, dialogs 10, menus 8. Sidebar 240, header 60, footer 40, file panel 320.

### UI-S-023 client event delivery

- Forward console warnings and errors to `POST /api/v1/client-events`; each event carries a UUID v4 client event ID it keeps for life (a resend or a next-open delivery sends the original ID).
- In-memory queue of at most **10,000** events; batches of at most **20** events and **16 KiB**; at most **60 requests a minute**; a sent batch stays queued until acknowledged.
- A full memory queue spills, in order, into the page's IndexedDB store, budget **50 MiB** for the origin; no event is discarded while storage accepts writes; spilled events return to the queue oldest first as it drains.
- While storage accepts writes every queued event is also written there as queued and removed when acknowledged; at `pagehide` every event still queued or unacknowledged is persisted; the next open delivers persisted events oldest first, each its own entry, before new events.
- Every stored event carries its host and the opaque principal of the session; a stored event is sent only under a credential of that host and principal; other principals' events are held untouched.
- Retry after `429`, `401` or `403`, `5xx` or a network failure with exponential backoff from **1 s**, doubling, at most **60 s**, never sooner than `Retry-After`, for as long as the page is open.
- A `400` batch is not retried as is: each event is resent alone; one the route refuses alone is removed and replaced by one `ui.console.error` event with error code `client_event_refused` and the refused event's component.
- Rate limits only delay, never merge: each delivered event is its own entry.
- When IndexedDB is unavailable or refuses a write (quota, the 50 MiB budget): fall back to the in-memory queue, count every event that could not be kept, and at page close send one `ui.console.overflow` event (own client event ID, `count` 1 to **1,000,000**, capped, never 0, no error code or other IDs) as a keep-alive request.
- Events carry only level, a fixed component identifier, an error code and opaque IDs: never message text, URLs, file names or stack traces.
- Failing test first for the write-through: an event queued while storage accepts writes is delivered on the next open after a page that ended without `pagehide`.

## Design

### Pure modules

- `src/logs/model.ts`: time-range presets and their bounds; the filter state and its journal query; classification of the search text (a file ID, an object ID, an operation ID, an operation name, or not searchable); the row view (time, level, operation, outcome kind, target text, short operation ID, journal-only mark, refused detection); the entry detail rows; the footer and paging words; export scope, format, file name and filtered counts words; `matchesFilter` for live entries.
- `src/logs/source.ts`: `LogsSource` and `ClientLogsSource` over `FileSquidClient`: `page`, `entry`, `export`, `libraries` (for the Library chip, from `GET /api/v1/libraries/recent`), `subscribe` (journal events).
- `src/clientevents/queue.ts`: `ClientEventQueue` with an injected transport, store, clock and scheduler: no globals, so every threshold is a unit test; `src/clientevents/store.ts`: `EventStore`, `IndexedDbEventStore`, `UnavailableEventStore`; `src/clientevents/console.ts`: console capture that ignores its own output.

### Elements

- `<fs-logs>` (the list view: head, filter row, chips with menus, table, paging, states, live chip), `<fs-log-entry>` (the detail panel over `PropertyTable`), the Download dialog builder; `bindLogsRoute` mounts them for `#/logs`, puts the detail in the shell's `panel` slot and the sentences in the footer slots.
- The shell's panel region widens to 360 px while the slotted panel asks for it (`data-panel-wide`).
- The files route gains `?open=<file id>&tab=<id>`; the file browser opens the file whose own original is that ID, and the file panel selects the tab.

### Errors and states

- Loading (placeholder rows), empty ("no entries match"), no match with Clear filters, error with reason and Try again, denied (`authNotPermitted`, no entries), the live chip only while the event channel is up.
- A failed export shows an alert in the dialog and keeps the dialog open.

## Files

New: `src/logs/model.ts`, `src/logs/source.ts`, `src/clientevents/{queue,store,console}.ts`, `src/components/{fs-logs,fs-log-entry,logs-export-dialog,logs-view-parts}.ts`, `src/app/logs-route.ts`, tests under `tests/unit`, `tests/ui`, `tests/support`, `tests/e2e/preview`, `tests/e2e/support`, the feature file. Changed: `src/api/client.ts`, `src/components/fs-app-shell.ts`, `src/components/fs-file-browser.ts`, `src/components/fs-file-panel.ts`, `src/files/model.ts`, `src/app/routes.ts`, `src/app/main.ts`, `src/index.ts`, both label tables, README, plan, checklist.

## Verification plan

Vitest (units for model, source, queue, store contract, console capture, route; UI tests for the elements), Playwright against `vite build` plus `vite preview` at 1440 x 900 in light and dark (mockup boxes within 2 px, computed styles, labels from the table, no console errors, behaviours), prettier, eslint, `tsc --noEmit`, the pre-commit hook (scope `npm:apps/filesquid/ui`).
