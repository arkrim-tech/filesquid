# Task docs-ui-mockup-updates: Walkthrough

## v3 UI mockups for the FileSquid MVP

Documentation-only task (design artifacts). Output: 62 SVG mockups and an index in [`resources/ui-mockups/v3/`](../ui-mockups/v3/README.md), marked as a draft. The [mockups README](../ui-mockups/README.md) lists v3 as a draft and still names no active suite. No SRS, PRD, ADR or rule file was changed.

TDD-ENFORCE: Not applicable - documentation and design artifacts only; no code changed.

## Owner request

> "sweep through all the end-user UI functionality we've developed so far and make sure that it has a corresponding ui-mockup in the @resources/ui-mockups/v3/ directory. Make sure to use the same starting point as the v2 UIs and follow the @resources/assets/FileSquid-design-guide-v1.pdf"

## What changed

- **Sources read:** the [v2 suite](../ui-mockups/v2/README.md) and every v2 SVG; the [design guide PDF](../assets/FileSquid-design-guide-v1.pdf), all 13 pages, compared with the HTML guide v2 cites; the default UI's source and [label table](../../apps/filesquid/ui/src/theme/labels.en.json), its [SRS](../../apps/filesquid/ui/resources/srs.md), [plan](../../apps/filesquid/ui/resources/plan.md) and task walkthroughs; [ADR 041](../architecture/041-owner-folder-libraries-and-home-collections.md) and its [walkthrough](./Task-docs-library-model-owner-folders-Walkthrough.md); ADR 032 and 033; the API SRS rows the UI uses (API-S-011, 012, 019, 021, 025, 027, 033, 034, 036, 037); PRD F-002.
- **Inventory:** every end-user surface that is built (UI-1.1, UI-F2.1a, UI-F2.1b, UI-F2.2, UI-F2.7, UI-F2.14 and the rename fix), specified by an MVP plan row, or changed by the owner's 2026-10-22 decisions, listed in the v3 README's coverage table with its source and status.
- **Sweep of v2:** each library in the sidebar now ends with its Recently deleted node, and the bottom Deleted files item is gone (ADR 041 Decision 8). The footer summary uses the `fileSummary` label. A builder reproduces v2's sidebar and header byte for byte before the change, so swept files differ from v2 only there.
- **Redrawn:** the library screens without names, paths, free space or recents (01 to 04, 08, 09); the empty library with uploads disabled at the top level (07); rename through the renames route (14); Recently deleted (16); the drop target wording (17); the import review with a required destination (18); jobs over the job routes (19); search refused for an unavailable operator (21); "Remove" in the menus (10 to 12); "Try again" (25); the Versions note (26); storage unavailable without a path (34).
- **New:** the library page (41, light and dark), repair consent and results (42, 43), rename refusals (44), numbered duplicate names (45), collection deletion (46), restore into an untitled collection (47), write refusals (48), Recently deleted settings (49, light and dark) and the history confirmation (50), the drop target over a collection (51), the import destination (52), an unknown search operator (53), the Monitor's OCFL validation (54, light and dark), the built navigation tree (55), view states (56) and Recently deleted in dark (16 dark).
- **Design guide:** the PDF and the HTML guide carry the same tokens; the differences between v2 (and the built tokens) and the PDF are listed in the v3 README under "Design guide differences". New and redrawn screens follow the PDF; swept screens keep v2's values until the owner decides.

## Coverage counts

| Status | Files |
|---|---|
| Unchanged from v2 | 1 |
| Updated (sweep only: sidebar, footer label, title) | 22 |
| Updated (redrawn or text changed) | 19 |
| New | 20 |
| Retired from v2 | 0 |
| **Total** | **62** |

## How the mockups were produced

As for v2, a Python generator in the session scratch area (not committed) holds the tokens, icons, list rows, dialogs and the sidebar, header and footer builders. Each SVG was rendered with headless Chromium at 1440 × 900, checked for text leaving the canvas, reviewed as an image, fixed and rendered again.

## Questions for the owner

The v3 README lists them in full under "Questions for the owner":

1. Approve v3 as the active suite.
2. The navigation tree: built sections by kind or a tree of libraries, and where Recently deleted sits.
3. UI-S-006's "deleted files" sidebar view versus one Recently deleted node per library.
4. How the first collection of an empty library is created.
5. The import review that PRD F-002 asks for and the import route does not offer.
6. Per-file job rows that the job view does not carry.
7. The theme control in the shell header (UI-S-015) or in Settings.
8. "Import" or "Upload" on the desktop New menu.
9. Rebuild as a durable job with progress.
10. Free space per library and recent libraries.
11. A permanent delete control in Recently deleted.
12. The shell's API status line on end-user screens.
13. The proposed strings that have no label yet.
14. Applying the PDF values to every screen, the 24 px logo minimum and a warning token.
15. The built AA contrast values in mockups.
16. Where storage per collection is shown.

## Verification

```text
$ cd resources/ui-mockups/v3 && xmllint --noout *.svg && echo ALL_OK
ALL_OK
```

Render check in headless Chromium at 1440 × 900 (text boxes leaving the canvas; console errors):

```text
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
10-new-and-item-menus.svg: offcanvas=0
11-selection-bulk-actions.svg: offcanvas=0
12-zip-download.svg: offcanvas=0
13-move-copy-picker.svg: offcanvas=0
14-rename-and-delete.svg: offcanvas=0
15-recent-view.svg: offcanvas=0
16-deleted-files-dark.svg: offcanvas=0
16-deleted-files.svg: offcanvas=0
17-import-drag-drop.svg: offcanvas=0
18-import-review.svg: offcanvas=0
19-import-progress.svg: offcanvas=0
20-search-suggestions.svg: offcanvas=0
21-search-results.svg: offcanvas=0
22-preview-image-dark.svg: offcanvas=0
22-preview-image.svg: offcanvas=0
23-preview-video.svg: offcanvas=0
24-preview-unsupported.svg: offcanvas=0
25-panel-states.svg: offcanvas=0
26-panel-versions.svg: offcanvas=0
27-version-restore.svg: offcanvas=0
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
39-logs-export.svg: offcanvas=0
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
console errors: 0
```

Pre-commit hook (`bash scripts/pre-commit.sh`), summary line:

```text
✅ All Pre-Commit Hooks Passed! Test scope: docs (21s)
```

## Sync Execution Report

No upstream document was changed. v3 is a draft and not a requirement source, so the PRD, SRS, ADRs and plans are unchanged; the questions above carry what the owner needs to settle before v3 becomes the active suite.
