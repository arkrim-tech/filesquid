# Task docs-mvp-decisions-2026-10-31 Walkthrough: the owner's MVP decisions of 2026-10-31

Branch `docs/mvp-decisions-2026-10-31`, a documentation task with no app directory. It records the owner's decisions of 2026-10-31 in the documents that own them: the PRD, the root SRS, the API, UI and desktop SRS, the implementation plan, the module plans, the checklist, three mockups and the BDD scenarios.

TDD-ENFORCE: Not applicable - documentation, mockup and plan changes only; the new Gherkin scenarios have no step definitions yet (the rows that build them add those).

## Owner decisions

- Verbatim, 2026-10-31: "accept recommendations". It accepts each orchestrator recommendation below (D2 to D15).
- Verbatim, 2026-10-31, on the open call (i), full-text search: "full-text search of filesquid drive contents is a release 1 feature".
- Verbatim, 2026-10-31, on the open call (ii), copies over 64 MiB: "build streamed copies".
- Verbatim, 2026-10-31, on the open call (iii), the "incl. {history} history" storage split: "well do a". Option (a), as the orchestrator offered it: build it in the MVP; the API also reports the bytes kept only by older versions, counting content shared between versions once, so the sidebar can show "incl. {history} history" under Storage used.
- Verbatim, 2026-10-31, on Cubic capacity (item 13): "13. leave where you left off until Cubic is re-enabled." There is no waiver of the review loop: work stops at the review step until Cubic is available again. This is recorded here only, not in the rules.

## What changed, decision by decision

| Decision | Recorded in | New or changed rows |
|---|---|---|
| D2 and D14: row facts on view and search rows, folder counts, `view=recent` with action kinds, `collection_id` on navigation nodes and folder rows, an `ApiError` sub-code | [API-S-011](../../apps/filesquid/crates/api/resources/srs.md) **Row facts**, **Recent** and the sub-code sentence; [API-S-012](../../apps/filesquid/crates/api/resources/srs.md); [API-S-021](../../apps/filesquid/crates/api/resources/srs.md) | New [API-F2.21](../../apps/filesquid/crates/api/resources/plan.md) (17 h, Task FS-2.5) and [UI-F2.20](../../apps/filesquid/ui/resources/plan.md) (4 h, Task FS-2.5); API-F2.21 added to UI-F2.17's Dependencies; UI-F2.8 and UI-F2.9 Status updated |
| D3: Task FS-1.4's Monitor-link and journal items | [UI-S-032](../../apps/filesquid/ui/resources/srs.md), [API-S-031](../../apps/filesquid/crates/api/resources/srs.md) **Self-scoped journal**, [PRD F-061](../prd.md#f-061-activity-journal-and-live-health-monitoring), [SRS-F-061](../srs.md#srs-f-061-activity-journal-and-live-health-monitoring) | Task FS-1.4 ticked; Task FS-2.4 reopened with new [API-F2.27](../../apps/filesquid/crates/api/resources/plan.md) (5 h) and [UI-F2.21](../../apps/filesquid/ui/resources/plan.md) (3 h) |
| D4: the path used is answered and recorded in the snapshot version, not the journal | API-S-011 **Same logical path** | API-F2.14 Status |
| D5: mockup 22 media frame at 8 px | [`22-preview-image.svg`](../ui-mockups/v3/22-preview-image.svg), [`22-preview-image-dark.svg`](../ui-mockups/v3/22-preview-image-dark.svg) (corner 4 px to 8 px, with a clip path so the image keeps the corner), [UI-S-031](../../apps/filesquid/ui/resources/srs.md), the [v3 README](../ui-mockups/v3/README.md), the [UI-F3.1 walkthrough](../../apps/filesquid/ui/resources/tasks/Task-UI-F3.1-Walkthrough.md) ("10 px" corrected) | UI-F3.1 Status |
| D6: an on-screen limit line where a tree level stops at 10,000 nodes | [UI-S-017](../../apps/filesquid/ui/resources/srs.md), proposed key `navigationLevelLimit` "Showing the first {count} only. Use search to find the rest." | UI-F3.4 (16 to 17 h; Task FS-3.4 40 to 41 h) |
| D7: API-F2.14 part 2 choices accepted, except the revision rule | API-S-011 **Collection moves, renames and folders** | New [API-F2.25](../../apps/filesquid/crates/api/resources/plan.md) (3 h, Task FS-2.5) |
| D8: DE-1.8's choices as MVP behaviour | [DE-S-019](../../apps/filesquid/crates/desktop/resources/srs.md) **Host type** and **Known limits**; root [README](../../README.md) import section | DE-1.8 Status; new post-MVP [DE-F3.4](../../apps/filesquid/crates/desktop/resources/plan.md) (4 h, Task FS-3.11) |
| D9: API-F2.8's version-message rule; envelopes on the metadata write routes | [API-S-002](../../apps/filesquid/crates/api/resources/srs.md), API-S-028 **version message** | New [API-F2.24](../../apps/filesquid/crates/api/resources/plan.md) (4 h, Task FS-2.5) |
| D10 and D15: library history object; streamed ZIP download without per-member scrubbing in the owner's own ZIP | API-S-011 **ZIP download**, API-S-028 **History object**, [PRD F-062](../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences), [SRS-F-062](../srs.md#srs-f-062-drive-style-views-sorting-selection-and-layout-preferences) **Multi-download**, [UI-S-026](../../apps/filesquid/ui/resources/srs.md) ZIP dialog, mockup [`12-zip-download.svg`](../ui-mockups/v3/12-zip-download.svg) (policy box redrawn: "Metadata kept", no chip) | New [API-F2.22](../../apps/filesquid/crates/api/resources/plan.md) (19 h, Task FS-2.5), which states the open design point of saving a ZIP above the 16 MiB answer cap of DE-S-005 on the desktop without deciding it |
| D10: a copy's cross-object `prov:wasDerivedFrom` | API-S-011 copy | New [API-F2.23](../../apps/filesquid/crates/api/resources/plan.md) (4 h, Task FS-2.8) |
| D11: UI-S-030 focus border, SRS-UI-007 exemption | [UI-S-030](../../apps/filesquid/ui/resources/srs.md) (`--fs-focus-ring`), [SRS-UI-007](../srs.md#srs-ui-007-light-and-dark-themes) (search operator text and **Clear all** named) | UI-F2.9 Status: per-format type chips stay after the MVP and are not scheduled |
| D12: library revisions restart at 1 after a host restart (API-S-025) | This walkthrough only, as the orchestrator directed: the decision changes nothing, and API-S-025 already states the behaviour | none |
| (ii) "build streamed copies" | API-S-011 copy (the 64 MiB allowance removed) | New [API-F2.26](../../apps/filesquid/crates/api/resources/plan.md) (6 h, Task FS-2.5) |
| (iii) "well do a": the storage split | [API-S-014](../../apps/filesquid/crates/api/resources/srs.md) **History size** (`history_bytes`: the bytes on disk held only by versions other than each object's head version, each distinct content counted once), API-S-011 storage use, [UI-S-025](../../apps/filesquid/ui/resources/srs.md) caption | Folded into API-F2.21 (17 to 19 h) and UI-F2.20 (the caption line) |
| (i) "full-text search ... is a release 1 feature" | [PRD F-063](../prd.md#f-063-file-search-query-language-and-filter-chips) and [SRS-F-063](../srs.md#srs-f-063-file-search-query-language-and-filter-chips): "in the first release" removed, so only the plan carries the timing | New post-MVP [API-F2.29](../../apps/filesquid/crates/api/resources/plan.md) (12 h, Task FS-2.7) |

### API-F2.2 and API-F2.3 closure

Both Descriptions are narrowed to what parts 1 and 2 delivered, and their Status names where each remainder went: row facts and Recent to API-F2.21; the ZIP download and history object to API-F2.22; the copy's derivation to API-F2.23; streamed copies to API-F2.26; the server-only operators (`sharedwith:`, `to:`, `from:`, `pendingowner:`, `owner:` naming another user), the multi-user search scenarios and the home, personal and shares views to the new post-MVP [API-F2.28](../../apps/filesquid/crates/api/resources/plan.md) (14 h, Task FS-2.7; Dependencies SV-F1.4 and API-F2.4); the favorites and tags views and `is:favorite` to CL-1.4 (Task 1.4, 8 to 12 h; Task 1.4 32 to 36 h); natural-language search questions to Task 4.2 (DE-4.2, 12 to 16 h; Task 4.2 24 to 28 h); the search-suggestions route dropped (the UI builds suggestions from the search operation; not scheduled). SV-F1.4 is removed from API-F2.3's Dependencies.

- **API-F2.2 and API-F2.3 are ticked.** All three open calls are answered ((i) API-F2.29, (ii) API-F2.26, (iii) API-F2.21 and UI-F2.20) and every remainder is moved, so both boxes are ticked in the api plan and the checklist (orchestrator, 2026-10-31).

### D3: did Task FS-2.4 already deliver the two items?

No. [UI-F2.7](../../apps/filesquid/ui/resources/plan.md)'s Description lists hiding the Monitor from users without the owner or administrator role as **Deferred** to Task FS-1.4, and neither API-F2.1 nor UI-F2.7 adds a self-scoped journal route (API-S-031 had none). So Task FS-2.4 is reopened with two new rows, API-F2.27 (the route) and UI-F2.21 (the Monitor button and route offered only when the admin routes accept the caller, learned from a `403` on the admin health route with no new contract field). Neither needs accounts: on the desktop the caller is the owner, and the tests use the authoriser's other principals. Task FS-1.4 is ticked in the plan and the checklist. Tasks FS-2.5 and FS-2.8 list Task FS-2.4 as a dependency; their Dependencies now say that the two new rows gate no row there. In the checklist, Task FS-2.4 moves after the finished tasks and before Task 1.2.

### Placement of the Release 1 rows

- API-F2.29 (full-text search) and API-F2.28 (server search) go to Task FS-2.7 (Activity stream, comments and sharing), in the Release 1 section of the checklist. It is the Release 1 task that already holds the search work that needs accounts and shares (API-F2.4, SV-F1.4), so the two search remainders of API-F2.3 stay together. Task FS-3.11 (Ubuntu and Windows desktop releases) is Release 1 too but holds installer work only.
- DE-F3.4 (per-attempt staging area) goes to Task FS-3.11, the first post-MVP desktop task after gate Task FS-3.9.

## New rows and hours

| Row | Hours | Global task | MVP |
|---|---|---|---|
| API-F2.21 View and search row facts, the Recent view and destination identifiers (with the history bytes) | 19 | FS-2.5 | yes |
| API-F2.22 Library history object and streamed ZIP download | 19 | FS-2.5 | yes |
| API-F2.24 Command envelopes on the metadata write routes | 4 | FS-2.5 | yes |
| API-F2.25 Collection moves compare the expected revision | 3 | FS-2.5 | yes |
| API-F2.26 Streamed copies of any size | 6 | FS-2.5 | yes |
| UI-F2.20 Consume the row facts | 4 | FS-2.5 | yes |
| API-F2.23 A copy records its source in the provenance history | 4 | FS-2.8 | yes |
| API-F2.27 Self-scoped journal route | 5 | FS-2.4 | yes |
| UI-F2.21 Monitor link and route by role | 3 | FS-2.4 | yes |
| UI-F3.4 on-screen limit line (widened) | 1 | FS-3.4 | yes |
| API-F2.28 Server search operators, multi-user search and the home, personal and shares views | 14 | FS-2.7 | no |
| API-F2.29 Full-text search of PDF and plain text | 12 | FS-2.7 | no |
| DE-F3.4 Per-attempt staging area for imports | 4 | FS-3.11 | no |

Global estimates: Task FS-2.5 206 to 261 h, Task FS-2.4 48 to 56 h, Task FS-2.8 79 to 83 h, Task FS-3.4 40 to 41 h, Task FS-2.7 76 to 102 h, Task FS-3.11 24 to 28 h. The MVP grows by 68 hours, from 1,192 to **1,260** estimated hours (`python3 scripts/docs/update_mvp_progress.py`: "about 988 of 1260 estimated hours"). The module dispatch register and the summary timeline are updated.

## ADR changes this PR could not make (ADRs are protected)

- [ADR 033](../architecture/033-activity-journal-and-live-health-monitoring.md) Decision 2 says the self-scoped journal route "arrives with accounts (Task FS-1.4)". The route is now API-F2.27 in Task FS-2.4 and does not wait for accounts; the task reference in the ADR should be updated or removed.
- [ADR 040](../architecture/040-prov-o-history.md) Decision 5 sets one history object per library but names neither its identifier nor its storage path, nor that it is created with the library. API-F2.22 fixes them in API-S-028; the ADR MAY then record them. No ADR decision is contradicted.
- No other ADR conflicts: ADR 041 Decisions 3, 4 and 12 agree with the accepted API-F2.14 choices, and ADR 038 Decision 6 (import records scrubbed from exports and shares) is kept by stating that a ZIP holds no import record.

## Sync Execution Report

- ADR to SRS: no ADR changed. The API SRS (API-S-002, API-S-011, API-S-012, API-S-014, API-S-021, API-S-028, API-S-031), the UI SRS (UI-S-017, UI-S-025, UI-S-026, UI-S-030, UI-S-031, UI-S-032), the desktop SRS (DE-S-019) and the root SRS (SRS-UI-007, SRS-F-061, SRS-F-062, SRS-F-063) are amended.
- SRS to PRD: PRD F-061 (Monitor only for the owner and administrators, self-scoped journal), F-062 (the owner's ZIP without per-member scrubbing) and F-063 (no release timing in the full-text sentence) are amended. No requirement ID was added, so the PRD and SRS traceability links are unchanged. The root README "About this project" needs no change; its import section gained the two import limits.
- Plans: the implementation plan, the checklist and the api, ui and desktop module plans; the progress bar was redrawn.
- BDD: new scenario files [`row_facts_recent_and_destinations_API-S-011.feature`](../../apps/filesquid/crates/api/tests/features/row_facts_recent_and_destinations_API-S-011.feature), [`zip_download_and_history_object_API-S-028.feature`](../../apps/filesquid/crates/api/tests/features/zip_download_and_history_object_API-S-028.feature), [`copy_provenance_and_streamed_copies_API-S-011.feature`](../../apps/filesquid/crates/api/tests/features/copy_provenance_and_streamed_copies_API-S-011.feature), [`metadata_command_envelopes_API-S-002.feature`](../../apps/filesquid/crates/api/tests/features/metadata_command_envelopes_API-S-002.feature), [`collection_move_revision_API-S-011.feature`](../../apps/filesquid/crates/api/tests/features/collection_move_revision_API-S-011.feature), [`self_scoped_journal_API-S-031.feature`](../../apps/filesquid/crates/api/tests/features/self_scoped_journal_API-S-031.feature), [`row_facts_in_the_drive_UI-S-026.feature`](../../apps/filesquid/ui/tests/features/row_facts_in_the_drive_UI-S-026.feature) and [`monitor_by_role_UI-S-032.feature`](../../apps/filesquid/ui/tests/features/monitor_by_role_UI-S-032.feature); scenarios added to the UI-S-030, UI-S-035 and SRS-UI-007 feature files. The `filesquid-api` harness reports the new scenarios as skipped until their rows add step definitions; no desktop feature file was touched, because the desktop harness fails on skipped steps.

## Mocked Environment Variables

None.

## Pre-commit

The PR's change scope is `rust:` and `npm:` (the API, UI and desktop SRS, plans and feature files live inside the crates and the UI package), so the head commit ran the full suite. Excerpt of `FULL=1 bash scripts/pre-commit.sh` on `fa1093f2` (the review fixes after it rerun the hook as recorded in the pull request):

```text
✅ Clippy passed (37s).
✅ Workspace tests passed (507s).
494 scenarios (362 passed, 132 skipped)
✅ Rustdoc passed (83s).
✅ Desktop tests (test-instrumented) passed (198s).
✅ TypeScript passed (14s).
✅ Vitest passed (83s).
✅ Lint and test validation passed (test scope: full).
✅ All Pre-Commit Hooks Passed! Test scope: full (1063s)
```

The 494 scenarios are the `filesquid-api` harness; the new scenarios of this PR are among the skipped ones until their rows add step definitions.
