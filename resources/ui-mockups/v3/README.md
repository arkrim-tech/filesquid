# UI mockups v3: FileSquid MVP (macOS desktop)

Mockups of every end-user surface of the **FileSquid MVP** that is built, specified by an MVP plan row, or changed by an owner decision up to 2026-10-22: the early-access default distribution on the macOS desktop host ([C-056](../../product-vision.md#release-sequence), [ADR 035](../../architecture/035-macos-first-desktop-mvp.md)). v3 starts from [v2](../v2/README.md): the same 1440 × 900 macOS window, logo, layout regions, numbering and README structure. The styling follows the [FileSquid Design Guide (PDF)](../../assets/FileSquid-design-guide-v1.pdf) on every screen.

**Status: active suite** by the owner's decision of 2026-10-22, verbatim: "yes, make v3 the active suite in that PR". The [mockups README](../README.md) names it as the active suite, so these files are requirement sources for the SRS. v1 and v2 are archived.

## Owner request

> "sweep through all the end-user UI functionality we've developed so far and make sure that it has a corresponding ui-mockup in the @resources/ui-mockups/v3/ directory. Make sure to use the same starting point as the v2 UIs and follow the @resources/assets/FileSquid-design-guide-v1.pdf"

## Owner answers of 2026-10-22

The owner answered every question of this README on 2026-10-22. The answers, verbatim, and what each changed in the suite:

| # | Answer (verbatim) | Change |
|---|---|---|
| 1 | "yes, make v3 the active suite in that PR" | v3 is the active suite |
| 2 | "keen the navigation tree in sections by kind. Recently deleted is one of those (at the bottom)" | Every sidebar is the sectioned tree of [55](./55-navigation-tree-built.svg): Pinned, Collections (each library a parent), Virtual collections, then Recently deleted as the last section. The tree of libraries is retired. |
| 3 | "There is only one trash. Not a separate trash per library" | One Recently deleted for all libraries: [16](./16-deleted-files.svg), [16 dark](./16-deleted-files-dark.svg) and [47](./47-restore-untitled-collection.svg) list each file's library, and its days left follow that library's retention. |
| 4 | "The first collection should be created automatically upon install or first use if it doesnt already exists. Prompt the user for a name and location." | New [57](./57-first-use-library.svg): on first use FileSquid asks for a name and a folder location, creates the library there and its first collection. The name and location are the library's (owner confirmation 2026-10-22, verbatim: "1. confirmed"). |
| 5 | "Keep the review" | [18](./18-import-review.svg) stays; the upload route gains a review step before the job starts. |
| 6 | "Yes, report progress per file" | [19](./19-import-progress.svg) shows each file with its status, error code, destination and Retry. |
| 7 | "Settings -> Appaerance" | The theme control lives in Settings, Appearance ([32](./32-settings.svg)); the header has none. |
| 8 | "Let's use Upload and Download (as synomyns for Import and Export respectively)" | User-facing Import and Export read Upload and Download on every screen (10, 07, 15, 17, 18, 19, 22 to 24, 31, 35, 38, 39, 52). API routes and code identifiers keep their names. |
| 9 | "Yes, rebuild should be a durable job" | [04](./04-recover-library.svg) shows the rebuild as a job with progress, Pause and Cancel. |
| 10 | "yes add library facts to v3" | Recent libraries with free space on [01](./01-welcome.svg) and [02](./02-create-library.svg); free space on the library page ([41](./41-library-page.svg), [41 dark](./41-library-page-dark.svg), [09](./09-copy-library.svg)). |
| 11 | "yes add one" | Delete now on each row of Recently deleted and its confirmation, new [58](./58-permanent-delete-confirm.svg). |
| 12 | "No, dont show API info on regular UI screens, just in the Settings" | No API status on browsing screens; Settings, About shows "API: ok · contract 0.21.0" ([32](./32-settings.svg)). |
| 13 | "Yes, proposed strings are approved" | The proposed strings are required label strings; the plan prompts list them for `labels.en.json`. |
| 14 | "Design guide: Yes apply PDF values to every screen. Make the logo makr 24px tall. Draw warnings in error colour" | Every screen redrawn with the guide's values (see [Design guide differences](#design-guide-differences)); the sidebar mark is 24 px tall; warnings use the error colour and no warning colour remains. |
| 15 | "No follow the guide in all cases" | Mockups keep the guide's colours (`#16AC49` with white text, `#E53935`). Where these fall below WCAG 2.2 AA, the owner kept them as a listed exception (decision on Conflict Artifact CA-1, 2026-10-22, verbatim: "a"; [SRS-UI-007](../../srs.md#srs-ui-007-light-and-dark-themes)); the focus ring uses `#0f8a39` in light so it keeps 3:1. |
| 16 | "Yes" | Storage per collection stays on the library page ([41](./41-library-page.svg)). |

## How v3 differs from v2

- **One library model ([ADR 041](../../architecture/041-owner-folder-libraries-and-home-collections.md), [#124](https://github.com/arkrim-tech/filesquid/pull/124), owner decisions of 2026-10-22).** Every library lives in a folder the owner chooses through the operating system's folder dialog, so the library screens show no path ([01](./01-welcome.svg) to [04](./04-recover-library.svg), [09](./09-copy-library.svg)). A library's children are collections; every upload names a destination collection ([07](./07-empty-library.svg), [51](./51-drop-target-collection.svg), [52](./52-import-destination.svg), [18](./18-import-review.svg)). The first library and its first collection are created on first use ([57](./57-first-use-library.svg)).
- **Sectioned tree and one Trash (owner answers 2 and 3).** Every sidebar shows the navigation tree in sections by kind (Pinned, Collections, Virtual collections) with Recently deleted as the last section, one filter over the deleted files of every library. The bottom "Deleted files" item of v2 is gone. [16](./16-deleted-files.svg) is the Recently deleted view with each file's library and days left.
- **Built screens drawn as built.** The library page, the repair consent and its results ([41](./41-library-page.svg) to [43](./43-repair-results.svg)), the location-choice answers ([03](./03-open-library.svg)), the rename form and its refusals ([14](./14-rename-and-delete.svg), [44](./44-rename-refusals.svg)), the Monitor's OCFL validation section ([54](./54-monitor-ocfl-validation.svg)), the navigation tree ([55](./55-navigation-tree-built.svg)) and the view states ([56](./56-view-states.svg)) use the words of the [label table](../../../apps/filesquid/ui/src/theme/labels.en.json).
- **Labels from the label table.** The footer summary reads "Folders: 3 · Files: 11 · Loaded size: 1.5 GB" (`fileSummary`), the item and bulk menus say "Remove" (`fileRemove`), the drop target says "Drop to upload into {folder}" (`fileDropTarget`), and the panel error button says "Try again" (`stateRetry`).
- **Upload and Download (owner answer 8).** What v2 called import and export is worded Upload and Download.
- **Honest refusals.** A search with an operator the desktop cannot answer is refused, not run without it ([21](./21-search-results.svg)); an unknown operator is an invalid request ([53](./53-search-unknown-operator.svg)); the ADR 041 codes have their own errors ([48](./48-write-refusals.svg)).
- **Jobs over the job routes.** [19](./19-import-progress.svg) shows upload jobs as the job routes report them, with progress per file (owner answer 6); [04](./04-recover-library.svg) shows the rebuild as a durable job (owner answer 9).
- **Version note.** The Versions view no longer says "Nothing expires": a deleted file's content expires at its purge time, the owner's exception to F-065 (ADR 041 Decision 11) ([26](./26-panel-versions.svg)).
- **Design guide on every screen (owner answer 14).** 14 px buttons, 30 px page titles, 400-weight eyebrows, `#111111` / `#A0A0A0` default icons, small and meta text of at least 13 px, one green button per screen besides the sidebar's New, a 24 px logo mark and warnings in the error colour. See [Design guide differences](#design-guide-differences).

## How the files were made

- **Redrawn 2026-10-22 for the owner's answers:** every file but the boards' own structure was regenerated by a script in the session scratch area (not committed): the sidebar is rebuilt as the sectioned tree, the guide's values are applied to every text, icon and button, warnings take the error colour, and Import and Export read Upload and Download. 16, 19, 04, 01, 02, 09, 41, 32 and 55 were redrawn by hand in the same primitives; 57 and 58 are new.
- Every file parses as XML (`xmllint --noout`) and renders in Chromium at 1440 × 900 with no text off the canvas and no button label wider than its button.

## Design constants used

From the PDF guide:

- **Brand green** `#16AC49` in both modes: the primary action, active states, focus and accent links. Tri-colour (`#FD1E23`, `#FCD805`, `#007AF9`) only in the logo and file-type badges, never as status.
- **Light:** page `#FFFFFF`, raised `#F5F5F5`; borders subtle `#E0E0E0`, default `#CCCCCC`, control outline `#8E8E8E` (inputs, selects, checkboxes and radio buttons, 3.28:1 on white and 3.01:1 on `#F5F5F5`); text heading `#000000`, body `#111111`, secondary `#555555`, muted `#777777`, placeholder `#999999`; active tint `#F0FAF4` with text `#0f8a39`; error `#E53935`.
- **Dark:** page `#141414`, card and sidebar `#1C1C1C`, raised `#242424`, overlay `#2C2C2C`; borders `#2A2A2A` and `#333333`, control outline `#757575` (3.03:1 or more on every dark surface); text `#F5F5F5`, `#E0E0E0`, `#A0A0A0`, `#6A6A6A` (muted and placeholder); active tint `#0D2B18` with `#4ACA74`; error `#EF5350`; code chip `#1C1C1C`.
- **Icons:** line icons, 1.6 px stroke, round caps and joins, no fill; 16, 20 and 24 px; default `#111111` light and `#A0A0A0` dark, active `#16AC49`, muted `#777777` / `#6A6A6A`, disabled `#CCCCCC` / `#3A3A3A`.
- **Type:** Georgia serif headings (H2 30–36 px 700, H3 20–24 px 600), system sans UI text (small and meta 13–14 px), eyebrows 11 px uppercase, weight 400, with 0.1em tracking, monospace for identifiers, codes and digests. Sentence case everywhere.
- **Buttons:** 14 px, weight 600, sentence case. Primary `#16AC49` with white text; secondary white (`#1C1C1C` dark) with a 1.5 px default border; ghost transparent with green text; destructive white (`#1C1C1C` dark) with error text and a 1.5 px error border.
- **Layout:** sidebar 240 px, detail panel 320 px, 8 px spacing scale (4, 8, 16, 24, 40, 64, 96), button radius 7 px, input radius 6 px, dialog radius 10 px. Flat fills only: no gradients, glows or shadows; dark mode uses the surface steps for elevation. Dialog scrims are black at 32 %.
- **Logo:** the sidebar mark is 24 px tall.
- **Inputs:** 1.5 px default border; focus a 2 px brand-green border; error a 2 px error border with the message below in the error colour.
- **Status chips** always carry text: `ok` on the active tint; warnings and errors outlined in the error colour (owner answer 14).

## Design guide differences

The PDF is a light-mode print of the HTML guide that v2 cites: the tokens, type scale, components and rules are the same in both. In the PDF the dark logo card ("Dark bg · white wordmark") prints its wordmark invisibly, a print artefact. The differences between v2 (and the built UI) and the PDF, and how v3 resolves each after the owner's answers 14 and 15:

| # | PDF guide | v2 | v3 |
|---|---|---|---|
| 1 | Button text 14–16 px, weight 600 | 13 px (12 px in rows) | 14 px on every screen |
| 2 | H2 30–36 px; "Don't shrink headings to fit more content" | Page titles 28 px | 30 px on every screen (the Recover page 34 px) |
| 3 | Eyebrow 11–12 px, weight 400 | Weight 500 | 400 on every screen |
| 4 | Default icon colour `#111111` light, `#A0A0A0` dark | Sidebar and list icons in the secondary text colour `#555555` | Default colour on every screen |
| 5 | Disabled icon colour `#CCCCCC` light, `#3A3A3A` dark | No disabled state drawn | Disabled buttons use the raised fill, a subtle border and placeholder text ([07](./07-empty-library.svg), [44](./44-rename-refusals.svg), [52](./52-import-destination.svg)) |
| 6 | "Don't use green on more than one button per screen" | Six green Restore buttons in Deleted files | Restore is a secondary button; one green button per screen besides the sidebar's New |
| 7 | No warning colour; status uses the error colour or the active tint | Warning `#8A6D00` light, `#E0C25A` dark | Warnings in the error colour (owner answer 14) |
| 8 | Logo minimum 24 px tall | Sidebar mark 22 px tall | 24 px (owner answer 14) |
| 9 | Secondary border "1.5px muted" (the guide's stylesheet uses its default border token) | 1.5 px `#CCCCCC` | Unchanged for buttons, tables, menus and dialogs, `#CCCCCC` / `#333333`; inputs, selects, checkboxes and radio buttons darkened to `#8E8E8E` / `#757575` so they reach 3:1 (WCAG 2.2 success criterion 1.4.11 (Level AA); owner decision 2026-10-23, verbatim: "1. darken them") |
| 10 | Small and meta text 13–14 px | Captions at 11–12 px (storage caption, column heads) | 13 px on every screen |
| 11 | White text on `#16AC49`; light error `#E53935` | Same | Same (owner answer 15: "No follow the guide in all cases"). The built UI's AA values (light primary `#0b7a32`, light error `#c62828`, dark on-primary `#0d2b18`, [UI-S-015](../../../apps/filesquid/ui/resources/srs.md)) give way to the guide's values: the owner chose to keep them as a listed exception to WCAG 2.2 AA (Conflict Artifact CA-1 in the [walkthrough](../../tasks/Task-docs-v3-mockup-requirements-sync-Walkthrough.md), resolved 2026-10-22, verbatim: "a"). |

## Coverage

Status after the owner's answers: **redrawn** (content changed for an answer), **updated** (the sectioned sidebar, the guide's values and the Upload/Download wording applied), **new**.

### Library lifecycle

| Surface | Mockup | Source | Status |
|---|---|---|---|
| First run: create, open or recover a library, recent libraries with free space; ADR 032 desktop statement | [01](./01-welcome.svg) | [UI-1.1](../../../apps/filesquid/ui/resources/plan.md) (built, [#114](https://github.com/arkrim-tech/filesquid/pull/114)); UI-1.3; owner answer 10 | redrawn |
| Create a library through the native folder dialog (location-choices, purpose `create`); waiting state | [02](./02-create-library.svg) | UI-1.1; API-S-034; DE-S-017; owner decision 2026-10-20 ("go with A with rfd") | redrawn |
| First use: name and folder for the first library, first collection created automatically | [57](./57-first-use-library.svg) | UI-1.3; owner answer 4 (reading confirmed: "1. confirmed") | new |
| Location-choice answers: cancelled, unavailable with reason and Try again, refused, failed step whose Try again keeps the chosen folder | [03](./03-open-library.svg) | UI-1.1 constraint gates 3, 4, 6 and "Retry resumes from the failed step"; API-S-034 | updated |
| Recover from a copy: the rebuild as a durable job with progress, Pause and Cancel, errors so far | [04](./04-recover-library.svg) | UI-1.3; API-S-025; owner answer 9 | redrawn |
| Library menu: About this library, Copy, Rebuild, Create or open a library | [08](./08-library-menu.svg) | UI-1.1 routes; `libraryFactsTitle`, `libraryCopy`, `libraryRebuild`, `libraryNavLink` | updated |
| Library page after a copy: copied counts and validation, free space | [09](./09-copy-library.svg) | UI-1.1; API-S-025 (`copy_destination`); owner answer 10 | redrawn |
| Library page: facts (ID, storage format, objects, storage used, free space), storage per collection, Copy and Rebuild, W010 note with Review repair; light and dark | [41](./41-library-page.svg), [41 dark](./41-library-page-dark.svg) | UI-1.1; API-S-011; owner answers 10 and 16 | redrawn |
| Repair consent dialog with the owner's exact statement; Cancel first | [42](./42-repair-consent.svg) | UI-1.1; API-S-023; owner decision 2026-10-18 | updated |
| Repair result per object and a refused repair request | [43](./43-repair-results.svg) | UI-1.1 | updated |
| Storage of a library unavailable; pending change kept; no path | [34](./34-state-storage-unavailable.svg) | UI-F2.1a, UI-0.1; API-S-005 | updated |

### Browsing and organising

| Surface | Mockup | Source | Status |
|---|---|---|---|
| Collection in the list layout with the full shell; light and dark | [05](./05-all-files-list.svg), [05 dark](./05-all-files-list-dark.svg) | UI-F2.2 (built, [#92](https://github.com/arkrim-tech/filesquid/pull/92)), UI-F2.8; UI-S-006, UI-S-019 | updated |
| Grid layout and the sort menu | [06](./06-all-files-grid.svg) | UI-F2.8; API-S-011 | updated |
| Empty library: no collection; upload disabled with the reason | [07](./07-empty-library.svg) | UI-S-002 **Destination**; UI-F2.17; ADR 041 Decision 4 | updated |
| New menu (Upload files…, Upload folder…, New folder) and the per-item menu (Remove) | [10](./10-new-and-item-menus.svg) | UI-F2.8; UI-S-006; owner answer 8 | updated |
| Bulk action bar stating the effective scope | [11](./11-selection-bulk-actions.svg) | UI-F2.8 | updated |
| Streamed ZIP download (no save path shown) | [12](./12-zip-download.svg) | UI-F2.8; API-S-011 | updated |
| Move or copy picker: collections and folders only, never Recently deleted | [13](./13-move-copy-picker.svg) | UI-F2.8; API-S-011 **Destination** | updated |
| Rename form and remove with Undo | [14](./14-rename-and-delete.svg) | [#119](https://github.com/arkrim-tech/filesquid/pull/119), API-S-027 | updated |
| Rename refusals | [44](./44-rename-refusals.svg) | #119 | updated |
| Recent: files opened or changed | [15](./15-recent-view.svg) | UI-F2.8; API-S-011 | updated |
| Navigation tree: Pinned, sections by kind, Recently deleted last, pin, move up and down, unavailable pin with Remove | [55](./55-navigation-tree-built.svg) | UI-F2.1a (built), UI-F2.1b ([#96](https://github.com/arkrim-tech/filesquid/pull/96)); UI-S-017; owner answers 2 and 3 | redrawn |
| View states | [56](./56-view-states.svg) | UI-F2.1a (built) | updated |
| Loading placeholders | [33](./33-state-loading.svg) | UI-F2.1a | updated |

### Destination, duplicate names and Recently deleted

| Surface | Mockup | Source | Status |
|---|---|---|---|
| Drop target only over a collection or a folder inside one, naming it | [17](./17-import-drag-drop.svg), [51](./51-drop-target-collection.svg) | UI-S-002 **Destination**; UI-F2.17; `fileDropTarget` | updated |
| Upload dialog requires a destination collection before it can start | [52](./52-import-destination.svg) | UI-1.2; UI-F2.17; API-S-036; owner answer 8 | updated |
| Upload review with the required destination; source named by volume only | [18](./18-import-review.svg) | PRD [F-002](../../prd.md#f-002-safe-file-folder-and-device-import); UI-1.2; API-S-036; owner answer 5 | updated |
| Jobs with progress per file, Pause, Resume, Cancel, Retry | [19](./19-import-progress.svg) | API-S-033, API-S-037; UI-1.2; owner answer 6 | redrawn |
| Same name numbered beside it | [45](./45-duplicate-names.svg) | ADR 041 Decision 12; API-S-011 | updated |
| Refusals: `path_suffix_exhausted`, `restore_name_exhausted`, `destination_required`, `collection_not_found` | [48](./48-write-refusals.svg) | ADR 041 Decisions 4, 9, 12 | updated |
| Recently deleted: one Trash for every library, each file's library, where it was, purge date and days left under its library's retention, Delete now and Restore; light and dark | [16](./16-deleted-files.svg), [16 dark](./16-deleted-files-dark.svg) | UI-S-002 **Recently deleted**; owner answers 3 and 11 | redrawn |
| Delete a file now: confirmation | [58](./58-permanent-delete-confirm.svg) | UI-F2.17; PRD F-008; owner answer 11 | new |
| Deleting a collection that holds files: confirmation naming Recently deleted and the purge date | [46](./46-delete-collection.svg) | UI-S-002; ADR 041 Decision 8 | updated |
| Restore into "untitled collection 2" when the collection is gone | [47](./47-restore-untitled-collection.svg) | ADR 041 Decision 9; API-S-019 | redrawn |

### Settings

| Surface | Mockup | Source | Status |
|---|---|---|---|
| Settings: Appearance with the theme control, Browsing, Privacy with the ADR 032 statement, About with the API status | [32](./32-settings.svg) | UI-0.2, UI-F2.1b, UI-F2.8, DE-F3.1; owner answers 7 and 12 | redrawn |
| Recently deleted settings: installation default, expiry setting, per-library period; light and dark | [49](./49-settings-recently-deleted.svg), [49 dark](./49-settings-recently-deleted-dark.svg) | ADR 041 Decisions 10 and 11 | updated |
| Confirmation before "delete all version history" | [50](./50-delete-history-confirm.svg) | UI-S-002 **Settings** | updated |
| Host indicator and its popover (ADR 032 statement) | [40](./40-host-indicator.svg) | UI-F2.1b (built, [#62](https://github.com/arkrim-tech/filesquid/pull/62)); UI-S-004, UI-S-018 | updated |

### Search

| Surface | Mockup | Source | Status |
|---|---|---|---|
| Search bar with Type, Location and Modified chips, suggestions and operators | [20](./20-search-suggestions.svg) | UI-F2.9; API-F2.3 ([#120](https://github.com/arkrim-tech/filesquid/pull/120)); UI-S-007 | updated |
| An operator the host cannot answer is refused as unavailable, naming it | [21](./21-search-results.svg) | API-S-012 | updated |
| An unknown operator is refused as an invalid request, naming it | [53](./53-search-unknown-operator.svg) | API-S-012 | updated |

### Preview and file panel

| Surface | Mockup | Source | Status |
|---|---|---|---|
| Image preview; light and dark | [22](./22-preview-image.svg), [22 dark](./22-preview-image-dark.svg) | UI-F2.2 (built); API-S-018 | updated |
| Video preview | [23](./23-preview-video.svg) | UI-F2.2 | updated |
| Unsupported format | [24](./24-preview-unsupported.svg) | UI-F2.2 | updated |
| Panel states | [25](./25-panel-states.svg) | UI-F2.15; UI-S-022 | updated |
| Versions view and its note | [26](./26-panel-versions.svg) | UI-F2.3, UI-F2.11; UI-S-009 | updated |
| Restore a version | [27](./27-version-restore.svg) | UI-F2.11 | updated |
| Description tab and Add property | [28](./28-panel-metadata.svg), [29](./29-metadata-add-property.svg) | UI-F2.4, UI-F2.15 | updated |
| File info tab | [35](./35-panel-file-info.svg) | UI-F2.15; API-S-027 | updated |
| Provenance tab | [36](./36-panel-provenance.svg) | UI-F2.15; UI-S-020 | updated |

### Monitor and Logs

| Surface | Mockup | Source | Status |
|---|---|---|---|
| Monitor: health, throughput, processes and jobs, queues, journal tail with Download; light and dark | [31](./31-monitor.svg), [31 dark](./31-monitor-dark.svg) | UI-F2.7 (built) | updated |
| Monitor OCFL validation; light and dark | [54](./54-monitor-ocfl-validation.svg), [54 dark](./54-monitor-ocfl-validation-dark.svg) | UI-F2.14 (built, [#100](https://github.com/arkrim-tech/filesquid/pull/100)); API-S-022 | updated |
| Logs view; light and dark | [38](./38-logs-view.svg), [38 dark](./38-logs-view-dark.svg) | UI-F2.16; UI-S-021 | updated |
| Download logs | [39](./39-logs-export.svg) | UI-F2.16; API-S-031; owner answer 8 | updated |

**Counts:** 64 files: 13 redrawn for an answer (01, 02, 04, 09, 16, 16 dark, 19, 32, 41, 41 dark, 47, 55; 31 and 31 dark also have their health word at 22 px), 49 updated, 2 new (57, 58). No screen was retired.

### Strings

The strings that had no label are approved (owner answer 13: "Yes, proposed strings are approved"). They are required label strings: the [UI SRS](../../../apps/filesquid/ui/resources/srs.md) rows UI-S-024 to UI-S-033 name them with their keys, and the [UI plan](../../../apps/filesquid/ui/resources/plan.md) prompts list them for `labels.en.json` and the ImageSquid table.

## Left out because the MVP does not deliver them

Everything in [v2's Left out table](../v2/README.md#left-out-because-the-mvp-does-not-deliver-them) still applies, except retention expiry, which ADR 041 now puts in the MVP. Also left out:

| Surface | Why |
|---|---|
| Transfer and CPU-heavy job limits (`slots_full`) | API-S-037 defines the caps, but no UI SRS row asks to show them. |
| Monitor admin-token sign-in (`monitorAdminToken`, `monitorSignIn`) | Only for `filesquid-server` until accounts arrive (UI-F2.7); the desktop MVP needs no token. |
| Server and browser host statements (`hostServer`, `hostBrowser`, service enrolment) | Not a desktop host; [40](./40-host-indicator.svg) already shows the after-MVP backup line. |
| Client-event delivery (UI-S-023) | No surface of its own; delivered events appear as Logs entries. |
| ImageSquid | It shares no surface in this suite. It composes the same library pages in its own vocabulary; its screens live in the ImageSquid distribution's own mockups folder. |

## Questions for the owner

All closed on 2026-10-22; the answers are under [Owner answers of 2026-10-22](#owner-answers-of-2026-10-22).

1. **Active suite.** Closed: "yes, make v3 the active suite in that PR".
2. **Navigation tree.** Closed: "keen the navigation tree in sections by kind. Recently deleted is one of those (at the bottom)".
3. **Deleted files view.** Closed: "There is only one trash. Not a separate trash per library".
4. **First collection.** Closed: "The first collection should be created automatically upon install or first use if it doesnt already exists. Prompt the user for a name and location." "Name and location" are the new library's name and folder (owner confirmation 2026-10-22, verbatim: "1. confirmed").
5. **Import review.** Closed: "Keep the review".
6. **Job rows.** Closed: "Yes, report progress per file".
7. **Theme control.** Closed: "Settings -> Appaerance".
8. **Upload or Import.** Closed: "Let's use Upload and Download (as synomyns for Import and Export respectively)".
9. **Recover progress.** Closed: "Yes, rebuild should be a durable job".
10. **Library facts.** Closed: "yes add library facts to v3".
11. **Permanent delete.** Closed: "yes add one".
12. **API status line.** Closed: "No, dont show API info on regular UI screens, just in the Settings".
13. **Proposed strings.** Closed: "Yes, proposed strings are approved".
14. **Design guide sweep.** Closed: "Design guide: Yes apply PDF values to every screen. Make the logo makr 24px tall. Draw warnings in error colour".
15. **Contrast values.** Closed: "No follow the guide in all cases"; the resulting Conflict Artifact CA-1 is resolved by the owner's choice of option (a), verbatim: "a".
16. **Storage by collection.** Closed: "Yes".

## Notes for the SRS writer

- Product name, labels and colours come from the default label table and tokens. Sample names and data are not real records.
- Dates in v3 assume today is 22 Oct 2026, the 30-day default retention and a 90-day period for Studio work.
- The ADR 041 readings the owner has not ruled on (one "untitled collection" per gone collection and restore command; retention bounds of 1 to 3,650 days; a deleted file's path counting as taken) are drawn as ADR 041 records them. ADR 041 Decision 8 described Recently deleted as a node of each library; owner answer 3 replaced that with one Trash, recorded as Conflict Artifact CA-3 in the [walkthrough](../../tasks/Task-docs-v3-mockup-requirements-sync-Walkthrough.md) and closed when the owner amended ADR 041 on 2026-10-23, verbatim: "2. yes amend ADR 041"; the owner approved that approach on 2026-10-22, verbatim: "3. yes". Where a guide colour fails a contrast requirement, the mockups follow the guide and the conflict is recorded the same way (approved, verbatim: "2. sounds good"); the owner then resolved that conflict, CA-1, by keeping the guide's values as a listed exception, verbatim: "a".
- The first collection's title on first use ("untitled collection", [57](./57-first-use-library.svg)) follows ADR 041 Decision 9's naming; the owner confirmed answer 4's reading ("1. confirmed").
