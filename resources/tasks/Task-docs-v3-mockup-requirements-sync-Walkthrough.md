# Task docs-v3-mockup-requirements-sync: Walkthrough

## UI-S-024 to UI-S-033: the composition of the active v3 mockup suite

Documentation-only task. Output: the v3 suite made the active mockup suite; ten new UI SRS rows (UI-S-024 Visual constants of the active mockup suite, UI-S-025 Shell composition, UI-S-026 Browser views, menus and dialogs, UI-S-027 Library lifecycle screens, UI-S-028 Destination, drop target, import and jobs, UI-S-029 Recently deleted, collection deletion, restore, duplicate names, write refusals and their settings, UI-S-030 Search composition, UI-S-031 Preview and file panel composition, UI-S-032 Monitor and Logs composition, UI-S-033 Settings page composition); cascades into UI-S-004, UI-S-006, UI-S-007, UI-S-009, UI-S-021, UI-S-022, SRS-UI-007, SRS-F-055, SRS-F-062 and the SRS mockup coverage section; two PRD corrections (F-055, F-062); every UI plan prompt that builds a v3 surface; the new Task UI-F2.18; the API-F2.14, API-F2.15 and DE-F3.1 prompts.

TDD-ENFORCE: Not applicable - requirements, plans and one Gherkin feature only; no code changed.

## Owner request

> "Okay, now that we've updated our UI-mockups you need to check that there haven't been any changes in the v3 set that requires updates to SRS.md or PRD.md. Also make sure any plan prompt descriptions are detailed enough to implement the UI mockups with 100% accuracy."

## Owner decision during the task

> "yes, make v3 the active suite in that PR"

Applied in its own commit (`docs: make v3 the active UI mockup suite`): the [mockups README](../ui-mockups/README.md) names [v3](../ui-mockups/v3/README.md) as the active suite and marks [v1](../ui-mockups/v1/README.md) and [v2](../ui-mockups/v2/README.md) archived; the v3 README's status and its question 1 record the decision. The new SRS rows therefore cite v3 files as requirement sources.

The other 15 questions of the v3 README stay open. Where a gap depends on one, the table says "blocked on owner question N" and the requirement stands as it was, or a Conflict Artifact is written. Question 3 (one Recently deleted per library instead of a bottom Deleted files item) is settled by [ADR 041](../architecture/041-owner-folder-libraries-and-home-collections.md) Decision 8 ("There is one Trash"), so it is applied.

**Design guide (part of question 14).** The task instruction says the SRS follows the [PDF guide](../assets/FileSquid-design-guide-v1.pdf) where it and v2 differ, and the owner's v3 request said to follow the PDF. UI-S-024 therefore applies guide rows 1 to 6, 9 and 10 to every screen, the swept ones included, and says that this is part of question 14, which the owner can reverse. Rows 7 (warning colour) and 8 (logo size) are left as the mockups draw them, blocked on question 14; row 11 (contrast) is Conflict Artifact CA-1 (question 15).

**Proposed strings (question 13).** Strings the mockups use that have no label yet are recorded with a proposed key marked "(new)" and their mockup wording, and UI-S-024 says that the wording is owner question 13.

## Gap analysis

Classes: **(a)** SRS missing a requirement the mockup shows; **(b)** SRS contradicts the mockup; **(c)** PRD missing or contradicting; **(d)** mockup surface with no PRD ID; **(e)** mockup contradicts a PRD row or an owner decision (Conflict Artifact, nothing changed); **blocked** on an open owner question (requirement unchanged); **phased** (the MVP draws a subset; the full requirement stays for later tasks).

| Mockup | What it shows | Before this task | Class | Resolution |
|---|---|---|---|---|
| every file | Guide colours per role in light and dark, Georgia and system sans type scale, 8 px spacing scale, radii 7/6/10/8, 1.6 px line icons, flat fills, 32 % scrims, status chips with text | UI-S-015 names some tokens and the contrast pairs; no values for the other roles, sizes, radii or controls | (a) | UI-S-024 |
| every file | Guide rows 1 to 6, 9, 10 (button 14 px 600, page title 30 px, eyebrow 400, icon colour, disabled state, one green button, secondary border, meta ≥ 13 px) differ on the swept screens | not specified | (a), part of question 14 | UI-S-024 follows the guide (task instruction); flagged |
| every file | Warning colour `#8A6D00` / `#E0C25A`; logo 22 px | not specified | blocked on question 14 | recorded as drawn in UI-S-024 |
| every file | Primary `#16AC49` with white text, error `#E53935`, active text `#0f8a39`, muted `#777777` / `#6A6A6A`, placeholder `#999999`, white text on file-type badges | UI-S-015 requires 4.5:1 and keeps `#0b7a32`, `#c62828`; NFR-006 targets WCAG 2.2 AA | (e) | CA-1; UI-S-015 values kept; question 15 |
| every file | Built spacing step 12 px is not on the guide's scale | UI-S-015 names `--fs-space-1` to `--fs-space-5` without values | (a) | UI-S-024: 4, 8, 16, 24, 40, 64, 96 |
| every file | 1 px rendering tolerance | SRS-UI-007: "A theme MUST NOT be required to match mockup pixels." | (b) (mockups README forbids saying the page need not match) | SRS-UI-007 rewritten: constants required, a pixel or two of rendering difference accepted |
| 05, 05 dark | Shell: 240 px sidebar (New, filter, All files, Recent, LIBRARIES, libraries with collections and Recently deleted, storage block, Settings, Logs), 60 px header (search, host badge, Monitor), 40 px footer (summary, commit state) | UI-S-006 lists views; no composition | (a) | UI-S-025 |
| 05 and every sidebar | Recently deleted as the last node of each library; no bottom Deleted files item | UI-S-006 and PRD F-062: a deleted-files entry at the bottom | (b), (c) | UI-S-006, SRS-F-062 and PRD F-062 updated (ADR 041 Decision 8; question 3 settled) |
| 05 and every sidebar | Tree of libraries without Pinned and without sections | PRD F-004 default UI: pinned destinations, reorderable sections; UI-S-017; built tree 55 | (e) | CA-2; UI-S-017 stands; question 2 |
| 05 | Version column, file-type badges, hover-only actions button, layout toggle | UI-S-006: checkbox, name, size, modified | (a) | UI-S-006 adds the version column; UI-S-026 |
| 05 | Footer commit state "All changes committed to OCFL" and its other states | PRD F-001 asks pending changes to stay visible; no UI row | (a) | UI-S-025 |
| 05 | Storage caption "incl. 4.2 GB history · 1.4 TB free on disk" | API has no free space per library (v3 question 10) | (a), API gap | UI-S-025 shows the free-space part only when the API reports it |
| 05 | No theme control and no API status line in the header | UI-S-015 requires `<fs-theme-control>` in the header; the built shell shows "API: ok" | blocked on questions 7 and 12 | UI-S-015 unchanged |
| 05, 10, 11, 20 | No home, personal, favorites, shares or tags views; no favorite, tags or share items; no per-row share control; no People chip | UI-S-006, UI-S-007, PRD F-062, F-063 | phased | the requirements stay; UI-S-006, UI-S-026, UI-S-030 say these join when their tasks ship (v3 README, Left out) |
| 06 | Grid tiles 214 × 218, sort menu with five keys, direction, folder placement | UI-S-006 names the sort menu | (a) | UI-S-026 |
| 07 | Empty library: only Recently deleted, disabled Upload files and Import files… with the reason | UI-S-002 requires disabled controls with a reason | (a) | UI-S-028 |
| 07 | How the first collection is created | no UI control in any row | blocked on question 4 | none |
| 07, 10 | Both "Upload files" and "Import files…"; the New menu says Import | label table `fileUpload`; UI-S-006 "upload files, upload folder" | blocked on question 8 | strings recorded as drawn |
| 08 | Library menu without path or close | UI-1.1 built | (a) | UI-S-027 |
| 09, 41, 41 dark | Library page facts, storage by collection, Copy and Rebuild, copy result, W010 note | UI-1.1 built; no composition | (a); placement blocked on question 16 | UI-S-027 |
| 01, 02 | Welcome and create without name, path or recents; privacy box; footer line | UI-1.1 built; no composition | (a) | UI-S-027 |
| 03 | Folder-choice answers | UI-1.1 gates | (a) | UI-S-027 |
| 04 | Recover report only, no job | API-S-025 synchronous | (a); question 9 open, no requirement changes | UI-S-027 |
| 10 | New menu with shortcuts, item menu with Remove in the error colour | UI-S-006 lists actions | (a) | UI-S-026 |
| 11 | Selection bar with the effective scope | UI-S-006 | (a) | UI-S-026 |
| 12 | ZIP dialog with policy, limits and progress | UI-S-006, API-S-011 | (a) | UI-S-026 |
| 13 | Move or copy picker, collections and folders only | UI-S-002, UI-S-006 | (a) | UI-S-026 |
| 14, 44 | Rename dialog, remove toast with Undo, five refusals | labels exist; no composition | (a) | UI-S-026 |
| 15 | Recent grouped by day with a fixed-order note | PRD F-062 fixed order | (a) | UI-S-026 |
| 16, 16 dark | Recently deleted: Was in, Deleted, How, Expires with days left, secondary Restore | UI-S-002 lists the filter; the API's deleted view has no deletion cause or home collection title | (a), API gap | UI-S-029; API-F2.15 prompt and hours |
| 16, 47 | "1 day left" in the warning colour | not specified | blocked on question 14 (row 7) | recorded as drawn |
| 17, 51 | Drop target over a shown collection or folder, or one collection row | UI-S-002 | (a) | UI-S-028 |
| 18 | Import review with counts, destination and space | PRD F-002 requires a review; API-S-036 starts the job after the dialog | blocked on question 5 | UI-S-028 records it; build waits for the owner |
| 19 | Jobs panel with job-level rows only | PRD F-002 asks per-item errors and retry | blocked on question 6 | UI-S-028 records the job rows |
| 20 | Search chips, suggestions, files, operators | UI-S-007 | (a) | UI-S-030 |
| 21 | Operator refused as unavailable, naming it | UI-S-007 "shown as unsupported" | (a) | UI-S-030 |
| 53 | Unknown operator refused as an invalid request | API-S-012 | (a) | UI-S-030 |
| 22, 22 dark, 23, 24 | Preview bar, zoom chip, filmstrip, video controls, unsupported state | UI-S-002 preview | (a) | UI-S-031 |
| 24 | "FileSquid keeps the file and every version." | label `fileNoPreviewHelp` "The original file is stored unchanged." | blocked on question 13 | both recorded in UI-S-031 |
| 25 | Panel states loading, empty, no match, error with Try again | UI-S-022 requires the states | (a) | UI-S-031 |
| 26, 27 | Versions view and restore dialog; "Deleted files expire." | UI-S-009 (no expiry control; ADR 041 Decision 11 exception) | (a) | UI-S-031 |
| 28, 29 | Description tab with You or File source chips, Add property dialog | UI-S-022, MD-S-011 | (a) | UI-S-031 |
| 35, 36 | File info and Provenance tab rows | UI-S-022, UI-S-020 | (a) | UI-S-031 |
| 31, 31 dark | Monitor cards and journal tail | SRS-F-061 | (a) | UI-S-032 (health word 22 px as in 54; 31 draws 26 px) |
| 54, 54 dark | OCFL validation section | UI-F2.14 built; labels exist | (a) | UI-S-032 |
| 38, 38 dark, 39 | Logs view, detail panel, export dialog | UI-S-021 | (a) | UI-S-032 |
| 32 | Settings sections Appearance, Browsing, Privacy, About | UI-S-018 Privacy; UI-S-015 theme in the header | (a); theme location blocked on question 7 | UI-S-033 |
| 49, 49 dark, 50 | Recently deleted settings and the confirmation | UI-S-002 Settings | (a) | UI-S-029 |
| 45 | Numbered names listed under the answered name with a New chip | UI-S-002 Same logical path | (a) | UI-S-029; API-F2.14 prompt |
| 46, 47, 48 | Delete collection dialog, untitled collection toast, refusal cards | UI-S-002 | (a) | UI-S-029 |
| 52 | Import dialog requiring a destination | UI-S-002 | (a) | UI-S-028 |
| 33, 34 | Loading rows; storage unavailable with a pending change | UI-S-002 states | (a) | UI-S-026, UI-S-027 |
| 40 | Host badge and popover | UI-S-004, UI-S-018 | none (path updated to v3) | UI-S-004, UI-S-033 |
| 55 | Built tree with Pinned and sections | UI-S-017 | none | CA-2 relates it to 05 |
| 56 | View states, including "files cannot be listed here yet" | UI-F2.1a built; no PRD criterion for the last state | (a), (d) | UI-S-026; recorded in the SRS mockup coverage section |
| PRD F-055 | Mockup list pointing at v2 | archived suite | (c) | PRD F-055 and SRS rows point at v3 |
| PRD F-037 / 32 | "This macOS build is not signed or notarised by Apple yet." and no early-access label | owner decision 2026-10-19 already removed the label | none | DE-F3.1 prompt points at v3 32 |

## Conflict Artifacts

### CA-1: Mockup colours below WCAG 2.2 AA

1. **Conflict Description:** The v3 mockups and the guide draw text and controls whose contrast is below WCAG 2.2 AA: white on `#16AC49` 2.99:1; `#E53935` on white 4.23:1; active text `#0f8a39` on `#F0FAF4` 4.18:1; muted `#777777` on white 4.48:1 and `#6A6A6A` on `#141414` 3.41:1; placeholder `#999999` on white 2.85:1; white text on the red and blue file-type badges.
2. **PRD/SRS Reference:** [PRD NFR-006](../prd.md#nfr-006-accessibility-and-usability): "Web interfaces MUST target WCAG 2.2 AA". [UI-S-015](../../apps/filesquid/ui/resources/srs.md): "its token test MUST fail when any of `on-surface`, `on-surface-variant` …, `primary` (on `surface`), `on-primary` (on `primary`), `on-primary-container` (on `primary-container`) or `error` … is below 4.5:1 in either theme".
3. **ADR Reference:** [ADR 031](../architecture/031-vanilla-typescript-ui-and-white-label-theming.md) Decision 3: "Components are styled only through CSS custom properties, with a light and a dark set." The token values, not the components, carry the fix.
4. **Recommended Resolution:** keep the built AA values for primary, on-primary, on-primary-container and error (UI-S-024 does so); decide whether the muted and placeholder roles join the contrast test with darker values (for example `#6f6f6f` and `#767676` on white) or stay as the guide draws them for non-essential text; redraw the mockups with the built values (owner question 15). Artifacts to update: [UI SRS](../../apps/filesquid/ui/resources/srs.md) UI-S-015 and UI-S-024, [`tokens.css`](../../apps/filesquid/ui/src/theme/tokens.css) and the ImageSquid tokens (by UI-F2.18), the v3 mockups, the guide if the owner changes it.
5. **Impact Assessment:** Unresolved, UI-F2.18 adds the muted and placeholder tokens with values that fail AA for small text, and a later accessibility audit (RC-005) fails; or the implementation departs from the mockups without an owner decision.

### CA-2: Sidebar tree of libraries versus the sectioned navigation tree

1. **Conflict Description:** Every browsing mockup (05 and the sidebar of 06 to 54) draws the sidebar as a tree of libraries, each with its collections and Recently deleted, with no Pinned section and no sections by kind. The built tree (mockup 55, UI-F2.1a) has Pinned and sections by kind that the owner can reorder and collapse.
2. **PRD/SRS Reference:** [PRD F-004](../prd.md#f-004-collections-folders-and-navigation), FileSquid default UI: "The default UI MUST show collections, virtual collections and nested folders in its navigation tree under the internal names, with pinned destinations, reorderable sections, collapse and expand, and keyboard equivalents for reordering." [UI-S-017](../../apps/filesquid/ui/resources/srs.md): pinned destinations, section order and collapse state persist.
3. **ADR Reference:** [ADR 041](../architecture/041-owner-folder-libraries-and-home-collections.md) Decision 8: "The navigation listing shows it as a reserved node of each library ([API-S-021])." The ADR places Recently deleted per library but does not say where it sits in a sectioned tree.
4. **Recommended Resolution:** the owner chooses (question 2): keep the sections of F-004 and draw Recently deleted inside each library's group, or change F-004 and UI-S-017 to the tree of libraries with pins as a separate group. Artifacts to update: PRD F-004, UI-S-017, UI-S-025, the v3 mockups 05 and 55, UI-F2.8 and UI-F2.18 prompts.
5. **Impact Assessment:** Unresolved, UI-F2.8 cannot build the sidebar without discarding either the built tree or the mockup, and the holistic-ui rule forbids building one and replacing it.

## Files changed

| File | Change |
|---|---|
| [`resources/ui-mockups/README.md`](../ui-mockups/README.md), [`v3/README.md`](../ui-mockups/v3/README.md), [`v2/README.md`](../ui-mockups/v2/README.md), [`v1/README.md`](../ui-mockups/v1/README.md) | v3 active, v1 and v2 archived (own commit) |
| [`resources/tasks/Task-docs-ui-mockup-updates-Walkthrough.md`](./Task-docs-ui-mockup-updates-Walkthrough.md) | the #125 Cubic P3 fix, cherry-picked from `ce2145e`: the docs scope skips the Cargo and lint/test steps while the Prettier formatting check still runs |
| [`apps/filesquid/ui/resources/srs.md`](../../apps/filesquid/ui/resources/srs.md) | UI-S-024 to UI-S-033 added; UI-S-004, UI-S-006, UI-S-007, UI-S-009, UI-S-021, UI-S-022 and the preamble point at v3 |
| [`resources/srs.md`](../srs.md) | SRS-UI-007 pixel sentence, SRS-F-055 composition bullet, SRS-F-062 sidebar, Mockup coverage section with the v3 map and the PRD gap |
| [`resources/prd.md`](../prd.md) | F-055 mockups point at v3; F-062 sidebar: Recently deleted per library |
| [`apps/filesquid/ui/resources/plan.md`](../../apps/filesquid/ui/resources/plan.md) | mockups, constants, composition rows and fidelity tests in UI-1.2, UI-F2.3, UI-F2.4, UI-F2.8, UI-F2.9, UI-F2.11, UI-F2.15, UI-F2.16, UI-F2.17, UI-F3.1; new UI-F2.18 (16 hours) |
| [`apps/filesquid/ui/tests/features/ui_mockup_fidelity_UI-S-024.feature`](../../apps/filesquid/ui/tests/features/ui_mockup_fidelity_UI-S-024.feature) | scenarios for UI-S-024 to UI-S-033 |
| [`apps/filesquid/crates/api/resources/plan.md`](../../apps/filesquid/crates/api/resources/plan.md) | API-F2.14 answers the numbered path and the codes of 45 and 48; API-F2.15 answers each deleted file's cause and home collection title for 16 (25 to 26 hours) |
| [`apps/filesquid/crates/desktop/resources/plan.md`](../../apps/filesquid/crates/desktop/resources/plan.md) | DE-F3.1 points at v3 mockup 32 |
| [`resources/implementation-plan.md`](../implementation-plan.md), [`resources/implementation-plan-checklist.md`](../implementation-plan-checklist.md), [`resources/assets/mvp-progress.svg`](../assets/mvp-progress.svg) | Task FS-2.5 dispatches UI-F2.18 (167 to 184 hours); MVP 1,072 to 1,089 hours, total 2,925 to 2,942; progress redrawn |

## Sync Execution Report

- **Mockups → SRS:** the active v3 suite into the UI SRS (UI-S-024 to UI-S-033 and seven existing rows) and the product SRS (SRS-UI-007, SRS-F-055, SRS-F-062, Mockup coverage).
- **SRS → PRD:** F-055 (mockup references) and F-062 (deleted-files entry per library, ADR 041 Decision 8). No other PRD row changed; the root README's "About this project" needs no change.
- **SRS → plans:** the UI plan prompts listed above, the new UI-F2.18, API-F2.14, API-F2.15, DE-F3.1, global Task FS-2.5, the checklist and the progress bar.
- **ADRs:** none changed; no new ADR is needed (no architectural change).
- **Not changed, blocked on the owner:** questions 2 (CA-2), 4, 5, 6, 7, 8, 9, 10, 11, 12, 13 (wording), 14 (rows 7, 8; rows 1 to 6, 9, 10 applied per the task instruction), 15 (CA-1), 16.
- **Platform gaps for the platform engineer:** free space of a library's storage root readable by the owner (sidebar caption of 05); the deleted view's per-file deletion cause and home collection title, also after the collection is deleted (16; added to the API-F2.15 prompt); an import planning step that answers counts, already-imported items and space before the job starts (18, question 5).

## Verification

The hook's output is recorded below after the final run.
