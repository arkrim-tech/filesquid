# Task docs-de-s-014-adr-033-040-api-gap-rows Walkthrough: the owner's decisions of 2026-11-01 and 2026-11-02

Branch `docs/de-s-014-interval-adr-033-040-api-gap-rows`, a documentation task with no app directory. It records the owner's decisions of 2026-11-01 and 2026-11-02 in the documents that own them: two ADR amendments (ADR 033 and ADR 040, drafted for the owner's approval), the API, desktop and UI SRS, the implementation plan, the API and UI module plans, the checklist, three UI walkthroughs and a Conflict Artifact, and the BDD scenarios.

TDD-ENFORCE: Not applicable - documentation, plan and ADR changes only; the new Gherkin scenarios have no step definitions yet (the rows that build them add those).

**ADR changes need the owner's approval.** This PR amends ADR 033 and ADR 040, so the owner merges it; merging approves the two amendments quoted below.

## Owner decisions, verbatim

- A, DE-S-014: "RE: DE-S-014 - "at launch and then every hour" is the default, this should be a configurable setting" and "open a small docs PR to record de-s-014 and add it to MVP".
- B, ADR 033 and ADR 040: "ADR 033 and ADR 040 - draft them for my approval to include in the same small docs PR".
- C, the location row: "RE: 5 - the desktop user can see folder name and volume so they can better manage their own private desktop".
- D, the API gaps the built default UI recorded: "make sure each API row issues gets a fix in MVP".
- E, the remaining recommendations: "I accept your recommenations on the remaining issues".

## What changed, decision by decision

| Decision | Recorded in | New or changed rows |
|---|---|---|
| A: expiry at launch and then at a configurable interval, default 1 hour | [API-S-019](../../apps/filesquid/crates/api/resources/srs.md) **Routes and expiry runs** (`expiry_interval_minutes`, 5 to 1,440, default 60, admin only, `invalid_request` outside the bounds, the next run due one new interval after the last run started or at once), the settings `GET` field list; [DE-S-014](../../apps/filesquid/crates/desktop/resources/srs.md) (the interval in the operational store, the monitor taking a change without a relaunch); [UI-S-029](../../apps/filesquid/ui/resources/srs.md) (the Settings field) | New [API-F2.30](../../apps/filesquid/crates/api/resources/plan.md) (2 h, Task FS-2.5, API settings and the desktop expiry monitor); [UI-F2.17](../../apps/filesquid/ui/resources/plan.md) widened 17 to 18 h for the field (its row was open, so it is extended rather than given a new row) |
| B: ADR 033, the self-scoped journal route does not wait for accounts | [ADR 033](../architecture/033-activity-journal-and-live-health-monitoring.md) Status, Decision 2 and Related Decisions / Notes; the [ADR index](../architecture/README.md) | none (the route is [API-F2.27](../../apps/filesquid/crates/api/resources/plan.md), unchanged) |
| B: ADR 040, the history object's identifier and path | [ADR 040](../architecture/040-prov-o-history.md) Status, Decision 5 and Notes; the ADR index; [API-S-028](../../apps/filesquid/crates/api/resources/srs.md) **History object** (`urn:filesquid:history:<library-id>` at `history/`, recognised by a rebuild) | [API-F2.22](../../apps/filesquid/crates/api/resources/plan.md): "confirms or replaces that proposal" becomes "implements that recorded decision" |
| C: folder and volume names for the desktop owner | [API-S-034](../../apps/filesquid/crates/api/resources/srs.md) **Folder and volume names**, [API-S-038](../../apps/filesquid/crates/api/resources/srs.md) (`folder_name`, `volume_name` in the facts), [DE-S-017](../../apps/filesquid/crates/desktop/resources/srs.md), [UI-S-034](../../apps/filesquid/ui/resources/srs.md) (the "{folder} · on {volume}" row is kept and fed by the API), [UI-S-027](../../apps/filesquid/ui/resources/srs.md) (a Location fact on the desktop library page); the [UI-1.3 Conflict Artifact](../../apps/filesquid/ui/resources/tasks/Task-UI-1.3-Conflict.md) gains the owner's resolution | New [API-F2.31](../../apps/filesquid/crates/api/resources/plan.md) (3 h, Task FS-2.5); the UI side in new [UI-F2.22](../../apps/filesquid/ui/resources/plan.md) |
| D1: no route creates a collection | API-S-019 **Create-collection route**: `POST /api/v1/libraries/collections`, action `files.write`, envelope scope `library:<id>` with the library's revision, input `{library_id, title}`, committed result `{collection_id, title}`, exactly what UI-1.3 sends, so the UI needs no change | New [API-F2.32](../../apps/filesquid/crates/api/resources/plan.md) (4 h, Task FS-2.5, with D2 and D3) |
| D2: no free-space reserve in the import plan | [API-S-036](../../apps/filesquid/crates/api/resources/srs.md) **Review** (`reserve_bytes`, the name UI-1.2's walkthrough asks for), [DE-S-019](../../apps/filesquid/crates/desktop/resources/srs.md) **Review**, [UI-S-028](../../apps/filesquid/ui/resources/srs.md) | API-F2.32; UI-F2.22 |
| D3: no collection name on a job's destination | [API-S-037](../../apps/filesquid/crates/api/resources/srs.md) **Per-file progress** (`collection_title`, `null` once the collection is gone), UI-S-028 | API-F2.32; UI-F2.22 |
| D4: no fixity time and no resources list | [API-S-027](../../apps/filesquid/crates/api/resources/srs.md) (`fixity` with `state` and `checked_at` from API-S-022's latest validation, `resources` with display name, extension, size and SHA-256, at most 100) | New [API-F2.33](../../apps/filesquid/crates/api/resources/plan.md) (7 h, Task FS-2.8, with D5 and D6); UI-F2.22 |
| D5: failed reads carry no operation ID | [API-S-009](../../apps/filesquid/crates/api/resources/srs.md) **Operation IDs on errors** (every 4xx and 5xx `ApiError` carries `operation_id`, the envelope's or one minted for the request, and its journal entry the same ID) | API-F2.33; UI-F2.22 |
| D6: history oldest first only | [API-S-029](../../apps/filesquid/crates/api/resources/srs.md) (`order` `oldest_first`, the default, or `newest_first`; a cursor of the other order is `invalid_request`) | API-F2.33; UI-F2.22 |
| E1: keep DE-0.10's quarantine of a replay whose recorded journal cannot be read | DE-S-011 did not say it (DE-0.10 built it and recorded it in its walkthrough only), so [DE-S-011](../../apps/filesquid/crates/desktop/resources/srs.md) and API-S-036 **One meaning** now state it | none (built by DE-0.10) |
| E2: ZIP downloads above the desktop's 16 MiB answer cap, option (a) | [DE-S-005](../../apps/filesquid/crates/desktop/resources/srs.md) **ZIP downloads**, [API-S-011](../../apps/filesquid/crates/api/resources/srs.md) **Saving a ZIP on the desktop** (the open design point removed) | API-F2.22: its open design point replaced by the desktop save; 19 to 22 h |
| E3: the strings UI-1.2 and UI-1.3 proposed | The [UI-1.2 walkthrough](../../apps/filesquid/ui/resources/tasks/Task-UI-1.2-Walkthrough.md) strings bullet and the [UI-1.3 walkthrough](../../apps/filesquid/ui/resources/tasks/Task-UI-1.3-Walkthrough.md) strings section record the approval of 2026-11-02 | none |

### Placement of the new rows

- A, C and D1 to D3 go to Task FS-2.5 (drive parity), which already holds API-F2.15 (the expiry runs) and the other default-UI follow-ups. Task 1.2, which built import and first use, is closed, so it is not reopened for them.
- D4 to D6 go to Task FS-2.8, which owns the history routes (API-F2.9). Task FS-2.9, which built the file panel tabs, is closing in a parallel PR (UI-F2.16), so it takes no new row.
- The desktop work of API-F2.30, API-F2.31 and API-F2.22 stays in those API rows with `-p filesquid-desktop`, as API-F2.15 did, so no desktop plan row is added.
- One UI row, UI-F2.22 (5 h, Task FS-2.5), consumes everything that changes a built surface: the location names (first use and the library page), the review's reserve, the job titles, File info's fixity and resources, the "{code} · op {id}" error state and the newest-first Provenance. D1 needs no UI change.

## New rows and hours

| Row | Hours | Global task | MVP |
|---|---|---|---|
| API-F2.30 Configurable expiry interval for Recently deleted | 2 | FS-2.5 | yes |
| API-F2.31 The owner's library folder and volume names on the desktop | 3 | FS-2.5 | yes |
| API-F2.32 Create-collection route, import reserve and job destination titles | 4 | FS-2.5 | yes |
| UI-F2.22 Show the new API facts in the built surfaces | 5 | FS-2.5 | yes |
| UI-F2.17 expiry interval field (widened 17 to 18) | 1 | FS-2.5 | yes |
| API-F2.22 desktop save dialog and streamed write (widened 19 to 22) | 3 | FS-2.5 | yes |
| API-F2.33 Fixity and resources in the file facts, operation IDs on errors and newest-first history | 7 | FS-2.8 | yes |

Global estimates: Task FS-2.5 261 to 279 h, Task FS-2.8 83 to 90 h. The MVP grows by 25 hours, from 1,260 to **1,285** estimated hours (`python3 scripts/docs/update_mvp_progress.py`: "about 1032 of 1285 estimated hours"); the Phase F2 line of the summary timeline grows from 747 to 772. The module dispatch register, the two global Descriptions and Agent Prompts, and the checklist are updated.

## Proposals the owner has not ruled on

- The expiry interval's bounds, 5 to 1,440 minutes (5 minutes to 24 hours), in whole minutes, and the rule that a changed interval counts from the start of the last run. Marked "proposed bounds, owner to confirm" in API-S-019.
- The operation ID of a refused read: one opaque ID minted per request in the operation-ID form of API-S-002, carried by the answer and its journal entry. Marked "the orchestrator's proposal, owner to confirm" in API-S-009.
- The UI words of the new rows: `libraryFactLocation` (Location, "{folder} · on {volume}"), `settingsTrashExpiryInterval` and `settingsTrashExpiryIntervalHelp` ("Check for expired files every" … "minutes", "FileSquid checks at launch and then at this interval. 5 to 1,440 minutes."), marked proposed in UI-S-027 and UI-S-029. Mockups 41 and 49 do not draw these rows; the SRS says so.
- The hours of the new rows (the orchestrator asked for about 3 hours for A; it is 2 + 1).

## ADR amendments drafted for approval

ADR 033, new Status paragraph:

> Amended 2026-11-02 (owner decision, verbatim: "ADR 033 and ADR 040 - draft them for my approval to include in the same small docs PR"; the plan change behind it was accepted on 2026-10-31, verbatim: "accept recommendations"): the self-scoped journal route of Decision 2 does not wait for accounts. Every accepted credential reads the entries whose actor it is through that route, on every host, the desktop owner included, and the route still grants none of the admin routes. See Decision 2 and Related Decisions / Notes.

ADR 033, Decision 2, the sentence that said the route "arrives with accounts (Task FS-1.4)" now reads:

> Administrators use the admin routes below; a user reads only their own entries through a self-scoped journal route, which grants none of the other admin routes and does not wait for accounts: every accepted credential reads the entries whose actor it is, so on the desktop the owner reads the owner's entries (amendment 2026-11-02).

ADR 033, new note (the ADR cites tasks in its Notes, so the task reference moves there):

> Self-scoped journal route (amendment 2026-11-02): it no longer arrives with accounts (formerly planned in Task FS-1.4). It is API-F2.27 in global Task FS-2.4 of the implementation plan, as API-S-031 **Self-scoped journal** states, and tests a caller who is not an administrator through the authoriser's other principals.

ADR 040, new Status paragraph:

> Amended 2026-11-02 (owner decision, verbatim: "ADR 033 and ADR 040 - draft them for my approval to include in the same small docs PR"; the history-object work it settles was accepted on 2026-10-31, verbatim: "accept recommendations"): Decision 5 now names the history object's identifier and its path in the library's storage root, and says when it is created.

ADR 040, added to Decision 5 after "(and its journal entries) and nothing else.":

> Its identifier is `urn:filesquid:history:<library-id>` and it lives at `history/` of the library's storage root, both derived from the library ID alone, mirroring the library's journal object `urn:filesquid:journal:<library-id>` at `journal/` (API-S-032). It is created in the same operation that creates the library; a library opened without one (created before this amendment, or copied without it) is given one, journaled once, before its first export or ZIP download. A catalog rebuild recognises it as the history object, never as an unregistered object (amendment 2026-11-02).

ADR 040, new note:

> History object identifier and path (amendment 2026-11-02): stated in API-S-028 **History object** and built by API-F2.22 in global Task FS-2.5 of the implementation plan.

Both ADRs keep their nine ISO 42010 items; only Status, Decision and Related Decisions / Notes changed.

## Conflict check (sync-logic.md)

No conflict was found, so no Conflict Artifact is needed.

- **C against ADR 032, ADR 041, ADR 030, ADR 002 and the privacy principles.** ADR 032 is about the hosted service, which never sees a library folder. ADR 041 makes every library an owner-chosen folder and says nothing against naming it to its owner. ADR 030 Decision 1 ("A folder never crosses the API as a path") and ADR 002 (the webview gets no path) hold: a folder's own name and its volume's name are not a path, and API-S-034 still forbids the path, parent folders and any name in the journal, refusals, events and logs. AGENTS.md §14 (privacy by design) holds: the names are shown only to the owner on the owner's own desktop, and every other host answers none. There is precedent: API-S-027 already answers the import source's volume name to the owner.
- **D5 against ADR 030's error body.** ADR 030 lists `code`, `message` and `retryable` on `ApiError`; `operation_id` is an added field, as API-F2.21's sub-code is. No decision is contradicted; the ADR MAY list both fields when they are built.
- **A, B, E** extend ADR 007, ADR 033, ADR 040 and ADR 041 without contradicting them; the ZIP save uses the `rfd` crate ADR 002 already allows, with no Tauri plugin and no IPC command.
- **Decoupled execution.** The new requirement text names no task or timing. Existing task references in DE-S-014 and API-S-019 (for example "(Task API-F2.15)") were left as they were; this PR adds none to an SRS.

## Sync Execution Report

- ADR to SRS: ADR 033 (Decision 2) and ADR 040 (Decision 5) are amended; API-S-028 states the history object, and API-S-031 already says the self-scoped route needs no accounts. The API SRS (API-S-009, API-S-011, API-S-019, API-S-027, API-S-028, API-S-029, API-S-034, API-S-036, API-S-037, API-S-038), the desktop SRS (DE-S-005, DE-S-011, DE-S-014, DE-S-017, DE-S-019) and the UI SRS (UI-S-027, UI-S-028, UI-S-029, UI-S-034) are amended.
- SRS to PRD: checked [PRD F-001](../prd.md#f-001-independent-local-library-and-ownership), F-008, F-061, F-062 and F-067 and the root SRS (SRS-F-001, SRS-F-008, SRS-F-061, SRS-F-062, SRS-F-067): none states the hourly schedule, forbids naming the library folder to its owner, fixes the history order or decides the desktop ZIP save, so neither changes. No requirement ID was added, so the PRD and SRS traceability links are unchanged. The root README "About this project" needs no change.
- Plans: the implementation plan (Tasks FS-2.5 and FS-2.8, the dispatch register, the summary timeline), the checklist, and the api and ui module plans; the progress bar was redrawn.
- BDD: new scenario files [`expiry_interval_API-S-019.feature`](../../apps/filesquid/crates/api/tests/features/expiry_interval_API-S-019.feature), [`create_collection_route_API-S-019.feature`](../../apps/filesquid/crates/api/tests/features/create_collection_route_API-S-019.feature), [`library_location_names_API-S-034.feature`](../../apps/filesquid/crates/api/tests/features/library_location_names_API-S-034.feature), [`import_reserve_and_job_titles_API-S-036.feature`](../../apps/filesquid/crates/api/tests/features/import_reserve_and_job_titles_API-S-036.feature) (with the replay quarantine of DE-S-011), [`fixity_and_resources_API-S-027.feature`](../../apps/filesquid/crates/api/tests/features/fixity_and_resources_API-S-027.feature), [`error_operation_ids_API-S-009.feature`](../../apps/filesquid/crates/api/tests/features/error_operation_ids_API-S-009.feature), [`history_order_API-S-029.feature`](../../apps/filesquid/crates/api/tests/features/history_order_API-S-029.feature), [`library_page_location_UI-S-027.feature`](../../apps/filesquid/ui/tests/features/library_page_location_UI-S-027.feature), [`recently_deleted_expiry_interval_UI-S-029.feature`](../../apps/filesquid/ui/tests/features/recently_deleted_expiry_interval_UI-S-029.feature) and [`file_panel_api_facts_UI-S-031.feature`](../../apps/filesquid/ui/tests/features/file_panel_api_facts_UI-S-031.feature); new scenarios in [`zip_download_and_history_object_API-S-028.feature`](../../apps/filesquid/crates/api/tests/features/zip_download_and_history_object_API-S-028.feature) (the identifier and path, the rebuild, the desktop save), [`first_use_library_UI-S-034.feature`](../../apps/filesquid/ui/tests/features/first_use_library_UI-S-034.feature) (the Location row) and [`safe_file_folder_and_device_import_F-002.feature`](../../apps/filesquid/ui/tests/features/safe_file_folder_and_device_import_F-002.feature) (the reserve and the job titles). The desktop Cucumber harness fails on skipped steps, so the desktop requirements' scenarios are in the API harness (which reports unmatched steps as skipped), and the rows that build them are told to bind any desktop scenario they add. The ADR 033 change needs no new scenario: [`self_scoped_journal_API-S-031.feature`](../../apps/filesquid/crates/api/tests/features/self_scoped_journal_API-S-031.feature) already covers a caller without accounts and the desktop owner.

## Mocked Environment Variables

None.

## Pre-commit

The PR's change scope is `rust:filesquid-api,filesquid-cli,filesquid-desktop,filesquid-native,filesquid-server npm:apps/filesquid/ui`: the API, desktop and UI SRS, the plans and the feature files live inside the crates and the UI package. So the commit ran the full suite. Excerpt of `FULL=1 bash scripts/pre-commit.sh` on `6029a5a0`, the commit with every change before this walkthrough section:

```text
✅ Clippy passed (127s).
571 scenarios (371 passed, 200 skipped)
✅ Workspace tests passed (338s).
✅ Rustdoc passed (38s).
64 scenarios (64 passed)
✅ Desktop tests (test-instrumented) passed (97s).
✅ ESLint passed (9s).
✅ TypeScript passed (10s).
✅ Vitest passed (64s).
✅ Lint and test validation passed (test scope: full).
✅ All Pre-Commit Hooks Passed! Test scope: full (1099s)
```

The 571 scenarios are the `filesquid-api` harness. The new scenarios of this PR are among the skipped ones until their rows add step definitions. The desktop harness (64 scenarios, all passed) is unchanged. The pull request records the hook runs on later commits.
