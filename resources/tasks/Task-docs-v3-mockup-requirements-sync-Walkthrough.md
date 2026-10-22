# Task docs-v3-mockup-requirements-sync: Walkthrough

## UI-S-024 to UI-S-034: the composition of the active v3 mockup suite

Documentation and design task. Output: v3 made the active mockup suite; the suite redrawn for the owner's 16 answers (two new screens, 57 and 58); eleven new UI SRS rows (UI-S-024 Visual constants of the active mockup suite, UI-S-025 Shell composition, UI-S-026 Browser views, menus and dialogs, UI-S-027 Library lifecycle screens, UI-S-028 Destination, drop target, import and jobs, UI-S-029 Recently deleted, collection deletion, restore, duplicate names, write refusals and their settings, UI-S-030 Search composition, UI-S-031 Preview and file panel composition, UI-S-032 Monitor and Logs composition, UI-S-033 Settings page composition, UI-S-034 First use); one new API SRS row (API-S-038 Library facts); cascades into UI-S-002, UI-S-004, UI-S-006, UI-S-007, UI-S-009, UI-S-015, UI-S-017, UI-S-021, UI-S-022, API-S-021, API-S-025, API-S-036, API-S-037, SRS-UI-007, SRS-F-001, SRS-F-002, SRS-F-004, SRS-F-008, SRS-F-009, SRS-F-055, SRS-F-062 and the SRS mockup coverage section; PRD F-001, F-002, F-004, F-009, F-055 and F-062; every UI plan prompt that builds a v3 surface; new plan rows UI-1.3, UI-F2.18, API-F2.16, API-F2.17, API-F2.18 and DE-1.8.

TDD-ENFORCE: Not applicable - requirements, plans, mockups and one Gherkin feature only; no code changed.

## Owner request

> "Okay, now that we've updated our UI-mockups you need to check that there haven't been any changes in the v3 set that requires updates to SRS.md or PRD.md. Also make sure any plan prompt descriptions are detailed enough to implement the UI mockups with 100% accuracy."

## Owner decisions during the task

The active suite, applied in its own commit (`docs: make v3 the active UI mockup suite`):

> "yes, make v3 the active suite in that PR"

The answers to the v3 README's "Questions for the owner", 2026-10-22, verbatim as written:

> "2. keen the navigation tree in sections by kind. Recently deleted is one of those (at the bottom)
> 3. There is only one trash. Not a separate trash per library
> 4. The first collection should be created automatically upon install or first use if it doesnt already exists. Prompt the user for a name and location.
> 5. Keep the review
> 6. Yes, report progress per file
> 7. Settings -> Appaerance
> 8. Let's use Upload and Download (as synomyns for Import and Export respectively)
> 9. Yes, rebuild should be a durable job
> 10. yes add library facts to v3
> 11. yes add one
> 12. No, dont show API info on regular UI screens, just in the Settings
> 13.  Yes, proposed strings are approved
> 14. Design guide: Yes apply PDF values to every screen. Make the logo makr 24px tall. Draw warnings in error colour
> 15. No follow the guide in all cases
> 16. Yes"

Every question is closed in the [v3 README](../ui-mockups/v3/README.md#questions-for-the-owner), and each SRS, PRD and plan row an answer changed quotes it.

**Readings the owner should confirm.**

- **Answer 4.** "a name and location" is read as the new library's name and folder, because a location belongs to a library ([ADR 041](../architecture/041-owner-folder-libraries-and-home-collections.md) Decision 1). On first use FileSquid asks for both, creates the library there and creates its first collection, titled "untitled collection" after ADR 041 Decision 9's naming ([57](../ui-mockups/v3/57-first-use-library.svg), UI-S-034). UI-S-034 and the first-use part of UI-1.3 are blocked until the owner confirms this reading.
- **Answer 12.** It names "API info". The host badge stays in the header, because it names the host, not the API, and PRD F-057 requires the UI to make the connected host obvious; the API status moves to Settings, About.

## Gap analysis

Classes: **(a)** SRS missing a requirement the mockup shows; **(b)** SRS contradicts the mockup; **(c)** PRD missing or contradicting; **(d)** mockup surface with no PRD ID; **(e)** mockup contradicts a PRD row or an owner decision (Conflict Artifact); **phased** (the MVP draws a subset; the full requirement stays for later tasks).

| Mockup | What it shows | Before this task | Class | Resolution |
|---|---|---|---|---|
| every file | Guide colours per role, Georgia and system sans type scale, 8 px spacing scale, radii 7/6/10/8, 1.6 px line icons, flat fills, 32 % scrims | UI-S-015 names some tokens and the contrast pairs only | (a) | UI-S-024 |
| every file | Guide rows 1 to 6 and 10 on every screen, the 24 px logo, warnings in the error colour (answer 14) | not specified; v2 values on swept screens | (a) | UI-S-024; suite redrawn |
| every file | Primary `#16AC49` with white text, error `#E53935`, active text `#0f8a39` (answer 15) | UI-S-015 keeps `#0b7a32`, `#c62828` and a 4.5:1 test; PRD NFR-006 targets WCAG 2.2 AA | (e) | CA-1; UI-S-015 values and test stand until the owner resolves it |
| every file | Spacing scale 4, 8, 16, 24, 40, 64, 96 | UI-S-015 names `--fs-space-1` to `--fs-space-5` without values; built step 12 px | (a) | UI-S-024 |
| every file | 1 px rendering tolerance | SRS-UI-007: "A theme MUST NOT be required to match mockup pixels." | (b) | SRS-UI-007 rewritten |
| 05, 05 dark and every sidebar | Sectioned tree: Pinned, Collections (libraries as parents), Virtual collections, Recently deleted last (answer 2) | v3 drew a tree of libraries; PRD F-004 asks for sections | (e) before answer 2, now resolved | Every sidebar redrawn; UI-S-017, UI-S-025, PRD F-004 |
| 05 and every sidebar | One Recently deleted for every library (answer 3) | UI-S-002, UI-S-006, SRS-F-004, SRS-F-008, PRD F-004, F-062, API-S-021 and ADR 041 Decision 8: one per library | (b), (c), (e) for ADR 041 | UI and product rows, PRD and API-S-021 updated; CA-3 for ADR 041 |
| 05 | Shell regions, Version column, badges, hover actions, layout toggle, commit state | UI-S-006 lists views only | (a) | UI-S-025, UI-S-026 |
| 05 | Storage caption with history and free space (answer 10) | no free-space field in the API | (a), API gap | UI-S-025; API-S-038; API-F2.18 |
| 05 | No theme control and no API status in the header (answers 7 and 12) | UI-S-015 put `<fs-theme-control>` in the header; the built shell shows "API: ok" | (b) | UI-S-015, UI-S-025, UI-S-033; UI-F2.18 moves both |
| 05, 10, 11, 20 | No home, personal, favorites, shares, tags; no favorite, tags or share items; no People chip | UI-S-006, UI-S-007, PRD F-062, F-063 | phased | requirements stay; they join when their tasks ship |
| 06 | Grid and sort menu | UI-S-006 | (a) | UI-S-026 |
| 07 | Library with no collection; Upload files and Upload folder… disabled with the reason | UI-S-002 | (a) | UI-S-028 |
| 07, 10, 15, 17, 18, 19, 22 to 24, 31, 35, 38, 39, 52 | Upload and Download wording (answer 8) | Import and Export wording | (a), (b) | strings renamed (list below) |
| 01, 02 | Recent libraries with free space (answer 10) | not drawn; no API | (a), API gap | UI-S-027; API-S-038 |
| 57 (new) | First use: library name and folder, first collection created (answer 4) | no UI control for the first collection | (a), reading to confirm | UI-S-034 (blocked); SRS-F-001; PRD F-001 |
| 03 | Folder-choice answers | UI-1.1 | (a) | UI-S-027 |
| 04 | Rebuild as a durable job (answer 9) | API-S-025 synchronous | (a), (b) | UI-S-027; API-S-025; API-F2.17 |
| 08 | Library menu | UI-1.1 | (a) | UI-S-027 |
| 09, 41, 41 dark | Facts with free space, storage per collection (answers 10 and 16), W010 note | UI-1.1 | (a) | UI-S-027; API-S-038 |
| 10 | New menu Upload files…, Upload folder…, New folder; item menu | UI-S-006 | (a) | UI-S-026 |
| 11, 12, 13, 14, 15, 44 | Bulk bar, ZIP dialog, picker, rename and refusals, Recent | labels exist | (a) | UI-S-026 |
| 16, 16 dark | One Trash with Library, Was in, Deleted, How, Expires, Delete now and Restore (answers 3, 11) | UI-S-002 per library; no permanent-delete control | (a), (b) | UI-S-029; API-F2.15 |
| 58 (new) | Delete now confirmation (answer 11) | PRD F-008 requires a scope confirmation | (a) | UI-S-029 |
| 17, 51, 52 | Drop target and destination | UI-S-002 | (a) | UI-S-028 |
| 18 | Upload review (answer 5) | API-S-036 starts the job right after the dialog | (a), API gap | UI-S-028; API-S-036 **Review**; API-F2.16 |
| 19 | Jobs with per-file rows (answer 6) | API-S-037 job-level only | (a), API gap | UI-S-028; API-S-037 **Per-file progress**; API-F2.16 |
| 20, 21, 53 | Search composition and refusals | UI-S-007 | (a) | UI-S-030 |
| 22 to 29, 35, 36 | Preview, panel tabs, states, Versions | UI-S-022, UI-S-009 | (a) | UI-S-031 |
| 24 | "FileSquid keeps the file and every version." (answer 13) | label `fileNoPreviewHelp` reads otherwise | (b) | UI-S-031 replaces it |
| 31, 54, 38, 39 | Monitor, OCFL validation, Logs, Download logs | SRS-F-061, UI-S-021 | (a) | UI-S-032 |
| 32 | Appearance with the theme control (answer 7), About with the API status (answer 12) | UI-S-015 header; UI-S-018 Privacy | (a), (b) | UI-S-033 |
| 45 to 50 | Numbered names, collection deletion, untitled collection, refusals, settings | UI-S-002 | (a) | UI-S-029 |
| 33, 34, 56 | Loading, storage unavailable, view states | UI-S-002 states | (a), (d) for "files cannot be listed here yet" | UI-S-026, UI-S-027; PRD gap recorded in the SRS |
| 40 | Host badge and popover | UI-S-004, UI-S-018 | none | paths point at v3 |
| 55 | Sectioned tree with Recently deleted last | UI-S-017 | (a) | UI-S-017 |
| PRD F-055 | Mockup list pointing at v2 | archived suite | (c) | PRD F-055 points at v3 |

## Conflict Artifacts

### CA-1: The guide's colours fall below WCAG 2.2 AA (answer 15)

1. **Conflict Description:** The owner answered "No follow the guide in all cases". The guide's light colours fail the contrast the SRS and PRD require: white on `#16AC49` 2.99:1; `#E53935` on white 4.23:1; active text `#0f8a39` on `#F0FAF4` 4.18:1; muted `#777777` on white 4.48:1 and `#6A6A6A` on `#141414` 3.41:1; placeholder `#999999` on white 2.85:1; white text on the red (`#FD1E23`) and blue (`#007AF9`) file-type badges below 4.5:1.
2. **PRD/SRS Reference:** [PRD NFR-006](../prd.md#nfr-006-accessibility-and-usability): "Web interfaces MUST target WCAG 2.2 AA". [UI-S-015](../../apps/filesquid/ui/resources/srs.md): "its token test MUST fail when any of `on-surface`, `on-surface-variant` …, `primary` (on `surface`), `on-primary` (on `primary`), `on-primary-container` (on `primary-container`) or `error` … is below 4.5:1 in either theme". [SRS-UI-007](../srs.md#srs-ui-007-light-and-dark-themes): "Contrast of primary actions and body text on their surfaces MUST meet WCAG 2.2 AA in both themes".
3. **ADR Reference:** [ADR 031](../architecture/031-vanilla-typescript-ui-and-white-label-theming.md) Decision 3: "Components are styled only through CSS custom properties, with a light and a dark set." The token values carry the choice.
4. **Recommended Resolution:** the owner chooses one: (a) keep the guide's values and amend NFR-006, SRS-UI-007 and UI-S-015 to accept them; (b) keep WCAG 2.2 AA and use the built values (`#0b7a32` primary, `#c62828` error, `#0b7a32` active text, darker muted and placeholder values) in the product while the mockups keep the guide's; (c) amend the guide. Until then UI-S-015's values and contrast test stand, and UI-F2.18 MUST NOT change those tokens. Artifacts to update: PRD NFR-006, SRS-UI-007, UI-S-015, UI-S-024, [`tokens.css`](../../apps/filesquid/ui/src/theme/tokens.css) and the ImageSquid tokens, the guide if (c).
5. **Impact Assessment:** Unresolved, UI-F2.18 cannot apply the guide's values without failing the contrast test the hook runs, and an accessibility audit (RC-005) fails if the test is weakened silently.

### CA-2: Sidebar tree (closed by answer 2)

The tree of libraries drawn in v3 contradicted PRD F-004's sections. The owner answered "keen the navigation tree in sections by kind. Recently deleted is one of those (at the bottom)"; every sidebar is redrawn as the sectioned tree, and UI-S-017, UI-S-025 and PRD F-004 say so. Nothing remains open.

### CA-3: One Trash for every library versus ADR 041 Decision 8 (answer 3)

1. **Conflict Description:** The owner answered "There is only one trash. Not a separate trash per library". ADR 041 Decision 8 makes Recently deleted "a reserved navigation filter of each library" shown "as a reserved node of each library".
2. **PRD/SRS Reference:** updated here to one Trash: [UI-S-002](../../apps/filesquid/ui/resources/srs.md), [UI-S-006](../../apps/filesquid/ui/resources/srs.md), [UI-S-017](../../apps/filesquid/ui/resources/srs.md), [SRS-F-004](../srs.md#srs-f-004-collections-folders-and-navigation), [SRS-F-008](../srs.md#srs-f-008-deletion-hidden-files-and-duplicate-review), [PRD F-004](../prd.md#f-004-collections-folders-and-navigation), [PRD F-062](../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences); [API-S-021](../../apps/filesquid/crates/api/resources/srs.md) records the target and keeps the per-library node until this artifact is resolved.
3. **ADR Reference:** [ADR 041](../architecture/041-owner-folder-libraries-and-home-collections.md) Decision 8: "Recently deleted (Trash) is a reserved navigation filter of each library over its files in the deleted state … The navigation listing shows it as a reserved node of each library ([API-S-021])." Retention stays per library (Decision 10), which one Trash keeps: each file's purge time follows its library's period.
4. **Recommended Resolution:** amend ADR 041 Decision 8 (an owner-approved ADR change, not made here): one Recently deleted filter for the repository, the last node of the navigation tree, listing the deleted files of every library the caller may read, each with its library; then API-S-021 drops the per-library node and API-F2.15 builds the one node. Artifacts to update: ADR 041, API-S-019 and API-S-021, API-F2.15.
5. **Impact Assessment:** Unresolved, API-F2.15 would build one node per library while the UI and mockups show one Trash, and the default UI could not list a Trash across libraries.

## Mockups redrawn

All 62 files were regenerated (the sectioned sidebar, the guide's values, the 24 px logo, warnings in the error colour and the Upload/Download wording), and two are new. Content redrawn for an answer: [01](../ui-mockups/v3/01-welcome.svg) and [02](../ui-mockups/v3/02-create-library.svg) (recent libraries), [04](../ui-mockups/v3/04-recover-library.svg) (rebuild job), [09](../ui-mockups/v3/09-copy-library.svg), [41](../ui-mockups/v3/41-library-page.svg) and [41 dark](../ui-mockups/v3/41-library-page-dark.svg) (free space), [16](../ui-mockups/v3/16-deleted-files.svg), [16 dark](../ui-mockups/v3/16-deleted-files-dark.svg) and [47](../ui-mockups/v3/47-restore-untitled-collection.svg) (one Trash, Delete now), [19](../ui-mockups/v3/19-import-progress.svg) (per-file progress), [32](../ui-mockups/v3/32-settings.svg) (API status in About, 22 px section headings), [55](../ui-mockups/v3/55-navigation-tree-built.svg) (Recently deleted last), [11](../ui-mockups/v3/11-selection-bulk-actions.svg) and [12](../ui-mockups/v3/12-zip-download.svg) (bulk bar re-laid for 14 px buttons), [31](../ui-mockups/v3/31-monitor.svg) and [31 dark](../ui-mockups/v3/31-monitor-dark.svg) (health word 22 px, Download). New: [57](../ui-mockups/v3/57-first-use-library.svg) and [58](../ui-mockups/v3/58-permanent-delete-confirm.svg).

## Strings renamed (answer 8)

User-facing strings only; API routes, operation kinds (`import.item`, `job.start`, the PROV-O activity kind `import`), journal operations and label keys keep their names.

| Before | After | Mockups |
|---|---|---|
| Import files… | Upload files… | 10 |
| Import folder… | Upload folder… | 10 |
| Import files… (07's second button) | Upload folder… | 07 |
| There is no upload or import at a library's top level. | There is no upload at a library's top level. | 07 |
| Open a collection to upload or import into it. | Open a collection to upload into it. | 07 |
| Import into FileSquid | Upload into FileSquid | 52 |
| Import (source radio group) | Upload | 52 |
| Import 214 files / Import 196 files | Upload 214 files / Upload 196 files | 18 |
| Already imported · skipped | Already uploaded · skipped | 18 |
| After import: … free on this computer | After upload: … free on this computer | 18 |
| … how it was imported. | … how it was uploaded. | 18 |
| Import into {collection} › {folder} | Upload into {collection} › {folder} | 19 |
| Importing · 82 of 196 | Uploading · 82 of 196 | 19 |
| You imported it · {time} | You uploaded it · {time} | 15 |
| Imported (File info) | Uploaded | 22, 22 dark, 23, 24, 35 |
| Folder import · volume … / Files import · … | Folder upload · volume … / Files upload · … | 22 to 24, 35 |
| Import · EOS_DIGITAL → Site photos / Import · Recordings | Upload · EOS_DIGITAL → Site photos / Upload · Recordings | 31, 31 dark |
| Queues: imports 1 · … | Queues: uploads 1 · … | 31, 31 dark |
| Export (Monitor) | Download | 31, 31 dark |
| JSON Lines export · kept forever | JSON Lines download · kept forever | 31, 31 dark |
| Import · EOS_DIGITAL (Logs target) | Upload · EOS_DIGITAL | 38, 38 dark, 39 |
| Export… | Download… | 38, 38 dark, 39 |
| Export logs | Download logs | 39 |
| Export (dialog button) | Download | 39 |
| Exports keep opaque ids … an export is safe to share for support. | Downloads keep opaque ids … a download is safe to share for support. | 39 |
| `monitorExport` "Export JSON Lines" (built label) | "Download JSON Lines" | UI-F2.18 |

## Files changed

| File | Change |
|---|---|
| [`resources/ui-mockups/README.md`](../ui-mockups/README.md), [`v1/README.md`](../ui-mockups/v1/README.md), [`v2/README.md`](../ui-mockups/v2/README.md) | v3 active, v1 and v2 archived (own commit) |
| [`resources/ui-mockups/v3/`](../ui-mockups/v3/README.md) | all 62 SVGs redrawn, 57 and 58 added, the README's answers, coverage, design differences and closed questions |
| [`resources/tasks/Task-docs-ui-mockup-updates-Walkthrough.md`](./Task-docs-ui-mockup-updates-Walkthrough.md) | the #125 Cubic P3 fix, cherry-picked from `ce2145e`: the docs scope skips the Cargo and lint/test steps while the Prettier formatting check still runs |
| [`apps/filesquid/ui/resources/srs.md`](../../apps/filesquid/ui/resources/srs.md) | UI-S-024 to UI-S-034; UI-S-002, UI-S-004, UI-S-006, UI-S-007, UI-S-009, UI-S-015, UI-S-017, UI-S-021, UI-S-022 |
| [`apps/filesquid/crates/api/resources/srs.md`](../../apps/filesquid/crates/api/resources/srs.md) | API-S-021 (CA-3), API-S-025 (rebuild job), API-S-036 (review), API-S-037 (per-file progress), new API-S-038 (library facts) |
| [`resources/srs.md`](../srs.md) | SRS-UI-007, SRS-F-001, SRS-F-002, SRS-F-004, SRS-F-008, SRS-F-009, SRS-F-055, SRS-F-062, Mockup coverage |
| [`resources/prd.md`](../prd.md) | F-001, F-002, F-004, F-009, F-055, F-062 |
| [`apps/filesquid/ui/resources/plan.md`](../../apps/filesquid/ui/resources/plan.md) | mockups, constants and fidelity tests in UI-1.2, UI-F2.3, UI-F2.4, UI-F2.8, UI-F2.9, UI-F2.11, UI-F2.15, UI-F2.16, UI-F2.17, UI-F3.1; new UI-1.3 (10 h) and UI-F2.18 (20 h); UI-1.2 8 to 12 h, UI-F2.17 15 to 17 h; approved strings listed for `labels.en.json` |
| [`apps/filesquid/ui/tests/features/ui_mockup_fidelity_UI-S-024.feature`](../../apps/filesquid/ui/tests/features/ui_mockup_fidelity_UI-S-024.feature) | scenarios for UI-S-024 to UI-S-034 |
| [`apps/filesquid/crates/api/resources/plan.md`](../../apps/filesquid/crates/api/resources/plan.md) | API-F2.14 and API-F2.15 prompts; API-F2.15 25 to 28 h; new API-F2.16 (12 h), API-F2.17 (6 h), API-F2.18 (6 h) |
| [`apps/filesquid/crates/desktop/resources/plan.md`](../../apps/filesquid/crates/desktop/resources/plan.md) | DE-F3.1 points at v3 32; new DE-1.8 (10 h) |
| [`resources/implementation-plan.md`](../implementation-plan.md), [`resources/implementation-plan-checklist.md`](../implementation-plan-checklist.md), [`resources/assets/mvp-progress.svg`](../assets/mvp-progress.svg) | Task 1.2 59 to 107 h, Task FS-2.5 167 to 192 h, MVP 1,072 to 1,145 h, total 2,925 to 2,998 h; checklist rows; progress redrawn |

## Sync Execution Report

- **Mockups → SRS:** the active v3 suite and the owner's answers into the UI SRS (UI-S-024 to UI-S-034 and nine existing rows), the API SRS (API-S-021, API-S-025, API-S-036, API-S-037, API-S-038) and the product SRS (SRS-UI-007, SRS-F-001, SRS-F-002, SRS-F-004, SRS-F-008, SRS-F-009, SRS-F-055, SRS-F-062, Mockup coverage).
- **SRS → PRD:** F-001 (first use), F-002 (review, per-file progress), F-004 (sections, one Trash), F-009 (rebuild job, library facts), F-055 (mockups, Upload/Download, API information only in Settings), F-062 (one Recently deleted). The root README's "About this project" needs no change.
- **SRS → plans:** the UI, API and desktop plan rows listed above, global Tasks 1.2 and FS-2.5, the checklist and the progress bar.
- **ADRs:** none changed. ADR 041 Decision 8 conflicts with answer 3 (CA-3); NFR-006 and UI-S-015 conflict with answer 15 (CA-1).
- **For the platform engineer:** the upload review plan and confirm route (API-S-036 **Review**), per-file job items and retries (API-S-037), the rebuild as a `library.rebuild` job (API-S-025), library names, free bytes and recent libraries (API-S-038), the deleted view's deletion cause and home collection title and one Recently deleted node across libraries once CA-3 is resolved (API-F2.15), and their desktop host side (DE-1.8).

## Verification

Mockups: every file parses as XML, and a headless Chromium render at 1440 × 900 (a scratch script, not committed) reports no text off the canvas and no console error. Its button check reports seven texts whose centre falls inside a button's box: each lies under a menu or dialog (10, 25, 27, 39, 57, 58) or is a count badge (19), not a label of that button; the screenshots were checked by eye.

```text
$ cd resources/ui-mockups/v3 && xmllint --noout *.svg && echo ALL_OK
ALL_OK
01-welcome.svg: offcanvas=0
02-create-library.svg: offcanvas=0
03-open-library.svg: offcanvas=0
04-recover-library.svg: offcanvas=0
05-all-files-list-dark.svg: offcanvas=0
05-all-files-list.svg: offcanvas=0
06-all-files-grid.svg: offcanvas=0
07-empty-library.svg: offcanvas=0
08-library-menu.svg: offcanvas=0
09-copy-library.svg: offcanvas=0
10-new-and-item-menus.svg: offcanvas=0 overflow=["⌘O text[211.0,23.0] rect[16,138,208,34]"]
11-selection-bulk-actions.svg: offcanvas=0
12-zip-download.svg: offcanvas=0
13-move-copy-picker.svg: offcanvas=0
14-rename-and-delete.svg: offcanvas=0
15-recent-view.svg: offcanvas=0
16-deleted-files-dark.svg: offcanvas=0
16-deleted-files.svg: offcanvas=0
17-import-drag-drop.svg: offcanvas=0
18-import-review.svg: offcanvas=0
19-import-progress.svg: offcanvas=0 overflow=["2 text[1292.8,6.3] rect[1270,44,28,28]"]
20-search-suggestions.svg: offcanvas=0
21-search-results.svg: offcanvas=0
22-preview-image-dark.svg: offcanvas=0
22-preview-image.svg: offcanvas=0
23-preview-video.svg: offcanvas=0
24-preview-unsupported.svg: offcanvas=0
25-panel-states.svg: offcanvas=0 overflow=["api.timeout · op 77d0…1c2a text[1191.9,172.2] rect[1220,440,120,28]"]
26-panel-versions.svg: offcanvas=0
27-version-restore.svg: offcanvas=0 overflow=["11 Oct 2026 text[896.0,71.6] rect[852,596,110,38]"]
28-panel-metadata.svg: offcanvas=0
29-metadata-add-property.svg: offcanvas=0
31-monitor-dark.svg: offcanvas=0
31-monitor.svg: offcanvas=0
32-settings.svg: offcanvas=0
33-state-loading.svg: offcanvas=0
34-state-storage-unavailable.svg: offcanvas=0
35-panel-file-info.svg: offcanvas=0
36-panel-provenance.svg: offcanvas=0
38-logs-view-dark.svg: offcanvas=0
38-logs-view.svg: offcanvas=0
39-logs-export.svg: offcanvas=0 overflow=["77aa…c1d5 text[880.0,65.0] rect[882,688,90,38]"]
40-host-indicator.svg: offcanvas=0
41-library-page-dark.svg: offcanvas=0
41-library-page.svg: offcanvas=0
42-repair-consent.svg: offcanvas=0
43-repair-results.svg: offcanvas=0
44-rename-refusals.svg: offcanvas=0
45-duplicate-names.svg: offcanvas=0
46-delete-collection.svg: offcanvas=0
47-restore-untitled-collection.svg: offcanvas=0
48-write-refusals.svg: offcanvas=0
49-settings-recently-deleted-dark.svg: offcanvas=0
49-settings-recently-deleted.svg: offcanvas=0
50-delete-history-confirm.svg: offcanvas=0
51-drop-target-collection.svg: offcanvas=0
52-import-destination.svg: offcanvas=0
53-search-unknown-operator.svg: offcanvas=0
54-monitor-ocfl-validation-dark.svg: offcanvas=0
54-monitor-ocfl-validation.svg: offcanvas=0
55-navigation-tree-built.svg: offcanvas=0
56-view-states.svg: offcanvas=0
57-first-use-library.svg: offcanvas=0 overflow=["Recover text[943.9,55.9] rect[862,598,130,38]"]
58-permanent-delete-confirm.svg: offcanvas=0 overflow=["Harbour project text[700.0,96.9] rect[722,494,82,36]","One by one text[950.0,70.6] rect[812,494,180,36]"]
console errors: 0
```

The hook's output is recorded below after the final run.
