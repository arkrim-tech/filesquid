# UI mockups v3: FileSquid MVP (macOS desktop)

Draft mockups of every end-user surface of the **FileSquid MVP** that is built, specified by an MVP plan row, or changed by an owner decision up to 2026-10-22: the early-access default distribution on the macOS desktop host ([C-056](../../product-vision.md#release-sequence), [ADR 035](../../architecture/035-macos-first-desktop-mvp.md)). v3 starts from [v2](../v2/README.md): the same 1440 × 900 macOS window, logo, layout regions, numbering and README structure. The styling follows the [FileSquid Design Guide (PDF)](../../assets/FileSquid-design-guide-v1.pdf).

**Status: draft, not the active suite.** The [mockups README](../README.md) names the active suite. Until the owner approves v3 there, these files MUST NOT be read as SRS requirements.

## Owner request

> "sweep through all the end-user UI functionality we've developed so far and make sure that it has a corresponding ui-mockup in the @resources/ui-mockups/v3/ directory. Make sure to use the same starting point as the v2 UIs and follow the @resources/assets/FileSquid-design-guide-v1.pdf"

## How v3 differs from v2

- **One library model ([ADR 041](../../architecture/041-owner-folder-libraries-and-home-collections.md), [#124](https://github.com/arkrim-tech/filesquid/pull/124), owner decisions of 2026-10-22).** Every library lives in a folder the owner chooses through the operating system's folder dialog, so the library screens show no name field, path, free-space line or recent list ([01](./01-welcome.svg) to [04](./04-recover-library.svg), [09](./09-copy-library.svg)). A library's children are collections; every upload and import names a destination collection ([07](./07-empty-library.svg), [51](./51-drop-target-collection.svg), [52](./52-import-destination.svg), [18](./18-import-review.svg)).
- **Recently deleted (Trash) per library.** Each expanded library in the sidebar ends with its reserved Recently deleted node, a filter over the library's deleted files. The bottom "Deleted files" item of v2 is gone, because there is one Trash per library (ADR 041 Decision 8). This changes the sidebar of every screen that has one. [16](./16-deleted-files.svg) is now the Recently deleted view, with the days left per file.
- **Built screens drawn as built.** The library page, the repair consent and its results ([41](./41-library-page.svg) to [43](./43-repair-results.svg)), the location-choice answers ([03](./03-open-library.svg)), the rename form and its refusals ([14](./14-rename-and-delete.svg), [44](./44-rename-refusals.svg)), the Monitor's OCFL validation section ([54](./54-monitor-ocfl-validation.svg)), the navigation tree ([55](./55-navigation-tree-built.svg)) and the view states ([56](./56-view-states.svg)) use the words of the [label table](../../../apps/filesquid/ui/src/theme/labels.en.json).
- **Labels from the label table.** Where a v2 string has a label, v3 uses it: the footer summary reads "Folders: 3 · Files: 11 · Loaded size: 1.5 GB" (`fileSummary`), the item and bulk menus say "Remove" (`fileRemove`), the drop target says "Drop to upload into {folder}" (`fileDropTarget`), and the panel error button says "Try again" (`stateRetry`).
- **Honest refusals.** A search with an operator the desktop cannot answer is refused, not run without it ([21](./21-search-results.svg)); an unknown operator is an invalid request ([53](./53-search-unknown-operator.svg)); the ADR 041 codes have their own errors ([48](./48-write-refusals.svg)).
- **Jobs over the job routes.** [19](./19-import-progress.svg) shows import jobs as the job routes report them (state, done of total, error code, Pause, Resume, Cancel), with no per-file rows and no source path.
- **Version note.** The Versions view no longer says "Nothing expires": a deleted file's content expires at its purge time, the owner's exception to F-065 (ADR 041 Decision 11) ([26](./26-panel-versions.svg)).
- **Design guide.** New and regenerated screens follow the PDF where it and v2 differ (14 px buttons, 30 px page titles, 400-weight eyebrows, #111111 / #A0A0A0 default icons, one green button per screen); swept screens keep v2's values. See [Design guide differences](#design-guide-differences).

## How the files were made

- **Unchanged:** copied byte for byte from v2.
- **Updated (sweep):** v2's markup with only three changes: the sidebar rebuilt with each library's Recently deleted node and without the bottom Deleted files item, the footer summary worded by `fileSummary`, and the title "v3". A builder reproduces v2's sidebar and header byte for byte before the change, so nothing else moved.
- **Updated (redrawn)** and **new:** drawn with the same primitives (tokens, icons, list rows, dialogs) as hand-written SVG. No external fonts, images or scripts.
- Every file parses as XML (`xmllint --noout`) and renders in Chromium at 1440 × 900 with no text off the canvas.

## Design constants used

From the PDF guide:

- **Brand green** `#16AC49` in both modes: the one primary action per screen section, active states, focus and accent links. Tri-colour (`#FD1E23`, `#FCD805`, `#007AF9`) only in the logo and file-type badges, never as status.
- **Light:** page `#FFFFFF`, raised `#F5F5F5`; borders subtle `#E0E0E0`, default `#CCCCCC`; text heading `#000000`, body `#111111`, secondary `#555555`, muted `#777777`, placeholder `#999999`; active tint `#F0FAF4` with text `#0f8a39`; error `#E53935`.
- **Dark:** page `#141414`, card and sidebar `#1C1C1C`, raised `#242424`, overlay `#2C2C2C`; borders `#2A2A2A` and `#333333`; text `#F5F5F5`, `#E0E0E0`, `#A0A0A0`, `#6A6A6A` (muted and placeholder); active tint `#0D2B18` with `#4ACA74`; error `#EF5350`; code chip `#1C1C1C`.
- **Icons:** line icons, 1.6 px stroke, round caps and joins, no fill; 16, 20 and 24 px; default `#111111` light and `#A0A0A0` dark, active `#16AC49`, muted `#777777` / `#6A6A6A`, disabled `#CCCCCC` / `#3A3A3A`.
- **Type:** Georgia serif headings (H2 30–36 px 700, H3 20–24 px 600), system sans UI text (small 13–14 px), eyebrows 11–12 px uppercase with 0.1em tracking, monospace for identifiers, codes and digests (code chips with a token background). Sentence case everywhere.
- **Buttons:** 14–16 px, weight 600, sentence case. Primary `#16AC49` with white text; secondary white (`#1C1C1C` dark) with a 1.5 px border; ghost transparent with green text; destructive white (`#1C1C1C` dark) with error text and a 1.5 px error border.
- **Layout:** sidebar 240 px, detail panel 320 px, 8 px spacing scale (4, 8, 16, 24, 40, 64, 96), button radius 7 px, input radius 6 px, dialog radius 10 px. Flat fills only: no gradients, glows or shadows; dark mode uses the surface steps for elevation. Dialog scrims are black at 32 %.
- **Inputs:** 1.5 px default border; focus a 2 px brand-green border; error a 2 px error border with the message below in the error colour.
- **Status chips** always carry text: `ok` on the active tint, warnings outlined in `#8A6D00` / `#E0C25A` (a v2 value, see below), errors outlined in the error colour.

## Design guide differences

The PDF is a light-mode print of the HTML guide that v2 cites: the tokens, type scale, components and rules are the same in both. In the PDF the dark logo card ("Dark bg · white wordmark") prints its wordmark invisibly, a print artefact. The differences that matter are between v2 (and the built UI) and the PDF:

| # | PDF guide | v2 | v3 |
|---|---|---|---|
| 1 | Button text 14–16 px, weight 600 | 13 px (12 px in rows) | 14 px in new and redrawn screens (13 px in dense rows); swept screens keep 13 px |
| 2 | H2 30–36 px; "Don't shrink headings to fit more content" | Page titles 28 px | 30 px in new and redrawn screens |
| 3 | Eyebrow 11–12 px, weight 400 | Weight 500 | 400 in new screens; the swept sidebar keeps 500 |
| 4 | Default icon colour `#111111` light, `#A0A0A0` dark | Sidebar and list icons in the secondary text colour `#555555` | Standalone icons in new screens use the default colour; sidebar and list rows keep v2 |
| 5 | Disabled icon colour `#CCCCCC` light, `#3A3A3A` dark | No disabled state drawn | Disabled buttons use the raised fill, a subtle border and placeholder text ([07](./07-empty-library.svg), [44](./44-rename-refusals.svg), [52](./52-import-destination.svg)) |
| 6 | "Don't use green on more than one button per screen" | Six green Restore buttons in Deleted files | Restore is a secondary button in [16](./16-deleted-files.svg); other swept screens unchanged |
| 7 | No warning colour; status uses the error colour or the active tint | Warning `#8A6D00` light, `#E0C25A` dark | Kept for refused, degraded and W010 states (a question below) |
| 8 | Logo minimum 24 px tall | Sidebar mark 22 px tall | Unchanged (a question below) |
| 9 | Secondary border "1.5px muted" | 1.5 px `#CCCCCC` (the default border token, not the muted text colour) | Unchanged |
| 10 | Small and meta text 13–14 px | Captions at 11–12 px (storage caption, column heads) | 12–13 px in new screens |
| 11 | White text on `#16AC49`; light error `#E53935` | Same | Same. The built UI departs for WCAG AA contrast: light primary `#0b7a32`, light error `#c62828`, dark on-primary `#0d2b18` ([UI-S-015](../../../apps/filesquid/ui/resources/srs.md), [`tokens.css`](../../../apps/filesquid/ui/src/theme/tokens.css)) |

## Coverage

Status: **unchanged** (byte-identical to v2), **updated (sweep)** (only the sidebar, footer label and title changed), **updated** (redrawn or text changed for a decision or label), **new**.

### Library lifecycle

| Surface | Mockup | Source | Status |
|---|---|---|---|
| First run: create, open or recover a library, worded by the label table; ADR 032 desktop statement | [01](./01-welcome.svg) | [UI-1.1](../../../apps/filesquid/ui/resources/plan.md) (built, [#114](https://github.com/arkrim-tech/filesquid/pull/114)); `library*` labels | updated |
| Create a library through the native folder dialog (location-choices, purpose `create`); waiting state | [02](./02-create-library.svg) | UI-1.1; API-S-034; DE-S-017; owner decision 2026-10-20 ("go with A with rfd") | updated |
| Location-choice answers: cancelled, unavailable with reason and Try again, refused, failed step whose Try again keeps the chosen folder | [03](./03-open-library.svg) | UI-1.1 constraint gates 3, 4, 6 and "Retry resumes from the failed step"; API-S-034 | updated |
| Recover from a copy: open, rebuild, report per object; nothing repaired | [04](./04-recover-library.svg) | UI-1.1; API-S-025; `libraryRebuilt`, `libraryRebuiltInvalid` | updated |
| Library menu: About this library, Copy, Rebuild, Create or open a library | [08](./08-library-menu.svg) | UI-1.1 routes; `libraryFactsTitle`, `libraryCopy`, `libraryRebuild`, `libraryNavLink` | updated |
| Library page after a copy: copied counts and validation | [09](./09-copy-library.svg) | UI-1.1; API-S-025 (`copy_destination`); `libraryCopied`, `libraryCopyClean` | updated |
| Library page: facts (ID, storage format, objects), storage used per library and per collection, Copy and Rebuild, W010 note with Review repair; light and dark | [41](./41-library-page.svg), [41 dark](./41-library-page-dark.svg) | UI-1.1; API-S-011 (storage use per library and collection); `libraryReadOnly`, `libraryReviewRepair` | new |
| Repair consent dialog with the owner's exact statement; Cancel first | [42](./42-repair-consent.svg) | UI-1.1; API-S-023; owner decision 2026-10-18; `repairConsent`, `repairDialogTitle`, `repairConfirm` | new |
| Repair result per object and a refused repair request | [43](./43-repair-results.svg) | UI-1.1; `repairRepaired`, `repairRepairedNoProvenance`, `repairRefused`, `repairRequestRefusedConflict` | new |
| Storage of a library unavailable; pending change kept; no path | [34](./34-state-storage-unavailable.svg) | UI-F2.1a, UI-0.1; API-S-005 (no paths) | updated |
| No in-app libraries | (no screen offers one) | ADR 041 Decision 1 | — |

### Browsing and organising

| Surface | Mockup | Source | Status |
|---|---|---|---|
| Collection in the list layout with the full shell; light and dark | [05](./05-all-files-list.svg), [05 dark](./05-all-files-list-dark.svg) | UI-F2.2 (built, [#92](https://github.com/arkrim-tech/filesquid/pull/92)), UI-F2.8; UI-S-006, UI-S-019 | updated (sweep) |
| Grid layout and the sort menu (five sort keys, direction, folder placement) | [06](./06-all-files-grid.svg) | UI-F2.8; API-S-011 | updated (sweep) |
| Empty library: no collection yet; upload and import disabled with the reason | [07](./07-empty-library.svg) | UI-S-002 **Destination**; UI-F2.17; ADR 041 Decision 4 | updated |
| New menu and the per-item menu (Remove) | [10](./10-new-and-item-menus.svg) | UI-F2.8; UI-S-006; `fileRemove` | updated |
| Bulk action bar stating the effective scope | [11](./11-selection-bulk-actions.svg) | UI-F2.8 | updated |
| Streamed ZIP download (no save path shown) | [12](./12-zip-download.svg) | UI-F2.8; API-S-011 | updated |
| Move or copy picker: collections and folders only, never Recently deleted | [13](./13-move-copy-picker.svg) | UI-F2.8 (reuses the UI-1.2 destination picker); API-S-011 **Destination** | updated (sweep) |
| Rename form through the renames route (display name only) and remove with Undo | [14](./14-rename-and-delete.svg) | [#119](https://github.com/arkrim-tech/filesquid/pull/119) (built), [#111](https://github.com/arkrim-tech/filesquid/pull/111) (API-F1.13); API-S-027; `fileRenameOnlyTitle`, `fileRemoved`, `fileUndo` | updated |
| Rename refusals: too long (1,024 bytes), invalid, not found, conflict, unavailable | [44](./44-rename-refusals.svg) | #119; `fileDisplayNameTooLong`, `fileRenameInvalid`, `fileRenameNotFound`, `fileRenameConflict`, `fileRenameUnavailable` | new |
| Recent: files opened or changed | [15](./15-recent-view.svg) | UI-F2.8; API-S-011 | updated (sweep) |
| Original display names everywhere, never the stored name | every screen | UI-S-019; API-S-027; owner requirement 2026-10-19 | — |
| Navigation tree as built: Pinned, sections by kind, pin, move up and down, unavailable pin with Remove | [55](./55-navigation-tree-built.svg) | UI-F2.1a (built), UI-F2.1b ([#96](https://github.com/arkrim-tech/filesquid/pull/96)); UI-S-017; `navigation*` labels | new |
| View states: loading, empty, error, offline, denied, unavailable, gone, not listed yet | [56](./56-view-states.svg) | UI-F2.1a (built); `state*`, `navigationGone`, `navigationFilesPending`, `authNotPermitted` | new |
| Loading placeholders | [33](./33-state-loading.svg) | UI-F2.1a | updated (sweep) |

### Destination, duplicate names and Recently deleted (ADR 041)

| Surface | Mockup | Source | Status |
|---|---|---|---|
| Drop target only over a collection or a folder inside one, naming it | [17](./17-import-drag-drop.svg), [51](./51-drop-target-collection.svg) | UI-S-002 **Destination**; UI-F2.17; `fileDropTarget`; owner decision 2026-10-22 ("Must choose") | updated, new |
| Import dialog requires a destination collection before it can start | [52](./52-import-destination.svg) | UI-1.2 (status, owner decision 2026-10-22); UI-F2.17; API-S-036 | new |
| Import review with the required destination; source named by volume only | [18](./18-import-review.svg) | PRD [F-002](../../prd.md#f-002-safe-file-folder-and-device-import); UI-1.2; API-S-036 | updated |
| Jobs: state, progress, Pause, Resume, Cancel | [19](./19-import-progress.svg) | API-F1.6 ([#118](https://github.com/arkrim-tech/filesquid/pull/118)); API-S-033, API-S-037; UI-1.2 | updated |
| Same name numbered beside it: `photo.jpg` → `photo-1.jpg`, `archive.tar.gz` → `archive-1.tar.gz`, folder `directoryY` → `directoryY-1`, `.env` → `.env-1` | [45](./45-duplicate-names.svg) | ADR 041 Decision 12; API-S-011 **Same logical path**; owner decisions 2026-10-22 | new |
| Refusals: `path_suffix_exhausted` at 9,999, `restore_name_exhausted`, `destination_required`, `collection_not_found` | [48](./48-write-refusals.svg) | ADR 041 Decisions 4, 9, 12; API-S-011, API-S-019, API-S-036; UI-S-002 | new |
| Recently deleted view: files deleted one by one and with their collection, where they were, purge date and days left, Restore; light and dark | [16](./16-deleted-files.svg), [16 dark](./16-deleted-files-dark.svg) | ADR 041 Decision 8; UI-S-002 **Recently deleted**; API-S-021 (`recently_deleted`); owner decisions 2026-10-22 | updated, new |
| Deleting a collection that holds files: confirmation naming Recently deleted and the purge date | [46](./46-delete-collection.svg) | UI-S-002; ADR 041 Decision 8 | new |
| Restore into "untitled collection" (here "untitled collection 2") when the collection is gone | [47](./47-restore-untitled-collection.svg) | ADR 041 Decision 9; API-S-019; `fileRestored` | new |
| Permanent expiry: content deleted at the purge date, history kept as a tombstone by default | [16](./16-deleted-files.svg), [46](./46-delete-collection.svg), [49](./49-settings-recently-deleted.svg), [26](./26-panel-versions.svg) | ADR 041 Decisions 10 and 11 | — |

### Settings

| Surface | Mockup | Source | Status |
|---|---|---|---|
| Settings: appearance, browsing, Privacy with the ADR 032 statement, About | [32](./32-settings.svg) | UI-0.2, UI-F2.1b, UI-F2.8, DE-F3.1; [ADR 032](../../architecture/032-hosting-and-privacy-model.md); `hostDesktop` | updated (sweep) |
| Recently deleted settings: installation default 30 days, expiry setting (keep version history, the default, or delete all version history) with the admin wording, per-library period, change applies only to files deleted afterwards; light and dark | [49](./49-settings-recently-deleted.svg), [49 dark](./49-settings-recently-deleted-dark.svg) | ADR 041 Decisions 10 and 11; UI-S-002 **Settings**; API-S-019; owner decisions 2026-10-22 | new |
| Confirmation before "delete all version history" | [50](./50-delete-history-confirm.svg) | UI-S-002 **Settings** | new |
| Host indicator and its popover (ADR 032 statement) | [40](./40-host-indicator.svg) | UI-F2.1b (built, [#62](https://github.com/arkrim-tech/filesquid/pull/62)); UI-S-004, UI-S-018 | unchanged |

### Search

| Surface | Mockup | Source | Status |
|---|---|---|---|
| Search bar with Type, Location and Modified chips, suggestions and operators | [20](./20-search-suggestions.svg) | UI-F2.9 (as specified); API-F2.3 ([#120](https://github.com/arkrim-tech/filesquid/pull/120)); UI-S-007 | updated (sweep) |
| An operator the host cannot answer is refused as unavailable, naming it | [21](./21-search-results.svg) | API-S-012 | updated |
| An unknown operator is refused as an invalid request, naming it | [53](./53-search-unknown-operator.svg) | API-S-012 | new |

### Preview and file panel

| Surface | Mockup | Source | Status |
|---|---|---|---|
| Image preview; light and dark | [22](./22-preview-image.svg), [22 dark](./22-preview-image-dark.svg) | UI-F2.2 (built); API-S-018 | updated (sweep) |
| Video preview | [23](./23-preview-video.svg) | UI-F2.2 | updated (sweep) |
| Unsupported format | [24](./24-preview-unsupported.svg) | UI-F2.2; `fileNoPreview`, `fileNoPreviewHelp` | updated (sweep) |
| Panel states (Try again from `stateRetry`) | [25](./25-panel-states.svg) | UI-F2.15; UI-S-022 | updated |
| Versions view and its note (deleted files expire) | [26](./26-panel-versions.svg) | UI-F2.3, UI-F2.11; UI-S-009; ADR 041 Decision 11 | updated |
| Restore a version | [27](./27-version-restore.svg) | UI-F2.11 | updated (sweep) |
| Description tab and Add property | [28](./28-panel-metadata.svg), [29](./29-metadata-add-property.svg) | UI-F2.4, UI-F2.15 | updated (sweep) |
| File info tab | [35](./35-panel-file-info.svg) | UI-F2.15; API-S-027 | updated (sweep) |
| Provenance tab | [36](./36-panel-provenance.svg) | UI-F2.15; UI-S-020 | updated (sweep) |

### Monitor and Logs

| Surface | Mockup | Source | Status |
|---|---|---|---|
| Monitor: health, throughput, processes and jobs, queues, journal tail; light and dark | [31](./31-monitor.svg), [31 dark](./31-monitor-dark.svg) | UI-F2.7 (built) | updated (sweep) |
| Monitor OCFL validation: degraded health (E023), schedule, Validate now, runs, errors and warnings apart, location withheld, W010 note with Review repair; light and dark | [54](./54-monitor-ocfl-validation.svg), [54 dark](./54-monitor-ocfl-validation-dark.svg) | UI-F2.14 (built, [#100](https://github.com/arkrim-tech/filesquid/pull/100)); API-S-022; `findings*`, `healthOcflErrors` | new |
| Logs view; light and dark | [38](./38-logs-view.svg), [38 dark](./38-logs-view-dark.svg) | UI-F2.16; UI-S-021 | updated (sweep) |
| Logs export | [39](./39-logs-export.svg) | UI-F2.16; API-S-031 | updated (sweep) |

**Counts:** 62 files: 1 unchanged, 41 updated (22 by the sweep only, 19 redrawn or changed in text), 20 new. No v2 screen was retired: the decisions changed screens rather than removing them. Removed from screens: the library name and location fields, recent libraries, paths and free-space lines (01 to 04, 07, 09, 18, 34); "Show storage root in Finder" and "Close library" from the library menu (08); the bottom Deleted files item (every sidebar); content snippets and "not applied" search results (21); per-file import rows (19).

### Strings not yet in the label table

The label table has no entry for these; they are proposals for the owner, written in the guide's voice from the SRS or decision text: the Recently deleted node and view (16, 46, 47), its column heads and "N days left"; the empty-library and disabled-upload reasons (07); the import dialog and destination picker (18, 52); "Jobs" and the job lines (19); the search refusals (21, 53); the library page's storage lines (41); the numbered-name status (45); the collection deletion, restore and refusal texts (46 to 48); the Recently deleted settings and confirmation (49, 50); the recover report's second line (04). The welcome title "Your files. Private." comes from the guide's specimen and v2.

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

1. **Active suite.** Approve v3 as the active suite, or trim it first.
2. **Navigation tree.** The built tree (UI-F2.1a, [55](./55-navigation-tree-built.svg)) groups nodes by kind under Pinned, Collections, Virtual collections and Folders; v2 and v3 draw a tree of libraries. Which is the target, and where does a library's Recently deleted node sit in the sectioned tree?
3. **Deleted files view.** UI-S-006 still lists a "deleted files" sidebar view. v3 replaces it with each library's Recently deleted node (ADR 041: "There is one Trash"). Confirm, or keep a cross-library view as well.
4. **First collection.** How does the owner create the first collection of an empty library ([07](./07-empty-library.svg))? The API has the command (API-S-019), but no plan row or label gives the UI control.
5. **Import review.** PRD F-002 requires a review (proposed and already-imported items, destination, counts, capacity), but the import route (API-S-036) starts the job right after the native source dialog. Keep [18](./18-import-review.svg) and add an API step, or drop the review for now?
6. **Job rows.** The job view has no destination and no per-file rows, while PRD F-002 asks for per-item errors and retry. v3 shows job-level rows only ([19](./19-import-progress.svg)) and names the destination the UI chose. Should the API report per-file progress?
7. **Theme control.** UI-S-015 requires `<fs-theme-control>` in the shell header; v2 and v3 draw the theme choice in Settings, Appearance. Which?
8. **Upload or Import** (open since v2): the New menu says Import files and Import folder, while the label table has "Upload files".
9. **Recover progress.** The rebuild route is synchronous, so [04](./04-recover-library.svg) shows the report only, not v2's job with Pause. Should rebuild become a durable job?
10. **Library facts.** The API has no free space per library and no recent-library list (UI-1.1 walkthrough), so v3 shows neither. Add them?
11. **Permanent delete.** API-S-019 allows an explicit, confirmed permanent deletion of deleted files; no UI row specifies a control, so v3 draws none.
12. **API status line.** The built shell shows "API: ok (filesquid)" in its header; v2 and v3 do not draw it. Keep it on end-user screens?
13. **Proposed strings.** Approve the [strings not yet in the label table](#strings-not-yet-in-the-label-table).
14. **Design guide sweep.** Apply the PDF values (rows 1 to 6 and 10 of [Design guide differences](#design-guide-differences)) to every screen, including the swept ones? Make the logo mark 24 px tall (row 8)? Adopt a warning token in the guide, or draw warnings in the error colour (row 7)?
15. **Contrast values.** Should mockups show the built AA values (`#0b7a32`, `#c62828`, dark on-primary `#0d2b18`) instead of the guide's (row 11)?
16. **Storage by collection.** v3 shows storage per collection on the library page ([41](./41-library-page.svg)). Is that the right place?

## Notes for the SRS writer

- Product name, labels and colours come from the default label table and tokens. Sample names and data are not real records.
- Dates in v3 assume today is 22 Oct 2026 and the 30-day default retention.
- The ADR 041 readings the owner has not ruled on (one "untitled collection" per gone collection and restore command; retention bounds of 1 to 3,650 days; a deleted file's path counting as taken) are drawn as ADR 041 records them.
