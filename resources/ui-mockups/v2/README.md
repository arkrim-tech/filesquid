# UI mockups v2: FileSquid MVP (macOS desktop)

Draft mockups of every screen a person uses in the **FileSquid MVP**: the early-access default distribution on the macOS desktop host ([C-056](../../product-vision.md#release-sequence), [ADR 035](../../architecture/035-macos-first-desktop-mvp.md)). The scope comes from phases F0 to F3 of the [implementation plan](../../implementation-plan.md), up to the gate [Task FS-3.9](../../implementation-plan.md#phase-f3-filesquid-mvp-macos-desktop). The styling follows the [FileSquid Design Guide](../../assets/FileSquid-Design-Guide.html), and the layouts build on the [v1 drafts](../v1/README.md).

**Status: draft, not the active suite.** The [mockups README](../README.md) names the active suite. Until the owner approves v2 there, these files MUST NOT be read as SRS requirements.

## How v2 differs from v1

- **MVP features only.** v1 drew the whole first consumer release. v2 shows only controls whose function an MVP task delivers. The [Left out](#left-out-because-the-mvp-does-not-deliver-them) list names what was removed and why.
- **macOS window.** Every screen is a 1440 × 900 macOS window with the title bar and window buttons, because the MVP ships only as a macOS DMG.
- **Logo.** The mark comes from the coloured [`FileSquid-logo.svg`](../../assets/FileSquid-logo.svg): a green head and red, yellow and blue shapes in both modes, with a black outline and wordmark on light surfaces and a white one on dark surfaces.
- **Early access.** The sidebar, the welcome screen and About carry the "Early access" label that C-056 and DE-F3.1 require.
- **Desktop privacy boundary.** Every browsing screen shows "This device · only you can read these files" ([UI-S-004](../../../apps/filesquid/ui/resources/srs.md)). The welcome screen and About state it in full.

## Design constants used

- Brand green `#16AC49` in both modes, for the one primary action per section, active states and focus. The tri-colour (`#FD1E23`, `#FCD805`, `#007AF9`) appears only in file-type badges, never as a status colour.
- Light: page `#FFFFFF`, raised `#F5F5F5`, borders `#E0E0E0` and `#CCCCCC`, text `#000000` / `#111111` / `#555555` / `#777777`, active tint `#F0FAF4` with `#0f8a39`, error `#E53935`.
- Dark: page `#141414`, card `#1C1C1C`, raised `#242424`, overlay `#2C2C2C`, borders `#2A2A2A` and `#333333`, text `#F5F5F5` / `#E0E0E0` / `#A0A0A0` / `#6A6A6A`, active tint `#0D2B18` with `#4ACA74`, error `#EF5350`.
- Flat fills only: no gradients, glows or shadows. Dark mode uses surface steps for elevation.
- Georgia serif headings, system sans UI text, monospace for identifiers, digests, paths and namespace prefixes. Sentence case everywhere.
- Sidebar 240 px, right panel 320 px, 8 px spacing scale, buttons with a 7 px radius, inputs with a 6 px radius, line icons with round caps.

## Library lifecycle

| File | Surface | States shown | Plan task | PRD |
|---|---|---|---|---|
| [01-welcome.svg](./01-welcome.svg) | First run: create, open or recover a library | No library open; privacy statement, early access, no telemetry | [UI-1.1](../../../apps/filesquid/ui/resources/plan.md), Task 1.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership), [F-057](../../prd.md#f-057-desktop-and-server-hosts) |
| [02-create-library.svg](./02-create-library.svg) | Create library: name and the folder that becomes the OCFL storage root, with the on-disk layout | Form filled | UI-1.1, CL-1.1, DE-1.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership), [F-042](../../prd.md#f-042-ocfl-preservation-and-portable-recovery) |
| [03-open-library.svg](./03-open-library.svg) | Open library: recent libraries and a chosen folder checked as a valid OCFL 1.1 storage root | One recent library on a disconnected disk | UI-1.1, DE-1.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership) |
| [04-recover-library.svg](./04-recover-library.svg) | Recover from a copy: validate every object and rebuild the catalog from OCFL alone | Durable job running; one digest mismatch reported, nothing repaired | UI-1.1, CL-1.1, IO-1.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership), [F-042](../../prd.md#f-042-ocfl-preservation-and-portable-recovery) |
| [08-library-menu.svg](./08-library-menu.svg) | Library actions: show storage root, copy, rebuild catalog, create or open another, close | Menu open | UI-1.1, CL-1.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership) |
| [09-copy-library.svg](./09-copy-library.svg) | Copy library: a checked OCFL snapshot readable with ordinary tools | Destination chosen | CL-1.1, DE-1.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership), [F-042](../../prd.md#f-042-ocfl-preservation-and-portable-recovery) |

## Browsing and organising

| File | Surface | States shown | Plan task | PRD |
|---|---|---|---|---|
| [05-all-files-list.svg](./05-all-files-list.svg), [05-all-files-list-dark.svg](./05-all-files-list-dark.svg) | Folder in the list layout (default) with the full shell | Populated folder, row hover; light and dark | [UI-F2.1](../../../apps/filesquid/ui/resources/plan.md), UI-F2.2, UI-F2.8 | [F-055](../../prd.md#f-055-default-ui-and-panel-composition), [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) |
| [06-all-files-grid.svg](./06-all-files-grid.svg) | Grid layout and the sort menu (five sort keys, direction, folder placement) | Sort menu open | UI-F2.8, API-F2.2 | [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) |
| [07-empty-library.svg](./07-empty-library.svg) | Empty library with import actions and a drop area | New library | UI-F2.1, UI-1.2 | [F-002](../../prd.md#f-002-safe-file-folder-and-device-import), [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |
| [10-new-and-item-menus.svg](./10-new-and-item-menus.svg) | New menu (import files, import folder, new folder) and the per-item menu | Both menus open | UI-F2.2, UI-F2.8 | [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) |
| [11-selection-bulk-actions.svg](./11-selection-bulk-actions.svg) | Bulk action bar stating the effective scope | 4 files and 1 folder selected | UI-F2.8 | [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) |
| [12-zip-download.svg](./12-zip-download.svg) | Streamed ZIP download with caps, outgoing metadata policy and manifest of omitted files | Writing the archive | UI-F2.8, API-F2.2 | [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences), [F-046](../../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports) |
| [13-move-copy-picker.svg](./13-move-copy-picker.svg) | Move or copy folder picker | Move tab, destination chosen | UI-F2.2, UI-F2.8, API-F2.2 | [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences), [F-042](../../prd.md#f-042-ocfl-preservation-and-portable-recovery) |
| [14-rename-and-delete.svg](./14-rename-and-delete.svg) | Inline rename and the delete toast with Undo | Rename pending; two files deleted | UI-F2.2 | [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |
| [15-recent-view.svg](./15-recent-view.svg) | Recent: files the owner opened or changed, grouped by day | Fixed order, stated in place of a sort control | UI-F2.8, API-F2.2 | [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) |
| [16-deleted-files.svg](./16-deleted-files.svg) | Deleted files with Restore | Six deleted items | UI-F2.2, UI-F2.8 | [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) |

## Import

| File | Surface | States shown | Plan task | PRD |
|---|---|---|---|---|
| [17-import-drag-drop.svg](./17-import-drag-drop.svg) | Drag-and-drop target naming the destination folder | 12 files dragged from Finder | [UI-1.2](../../../apps/filesquid/ui/resources/plan.md), DE-1.2 | [F-002](../../prd.md#f-002-safe-file-folder-and-device-import) |
| [18-import-review.svg](./18-import-review.svg) | Import review: proposed and already-imported items, destination, counts, space and staging limit | Camera card with 18 duplicates skipped | UI-1.2, DE-1.2 | [F-002](../../prd.md#f-002-safe-file-folder-and-device-import), [F-043](../../prd.md#f-043-import-time-c2pa-records-and-action-disclosure) |
| [19-import-progress.svg](./19-import-progress.svg) | Import jobs panel | Committed, verifying SHA-256, copying, waiting, failed with retry; a paused job resumed from its checkpoint | UI-1.2, DE-0.4, API-F1.6 | [F-002](../../prd.md#f-002-safe-file-folder-and-device-import) |

## Search

| File | Surface | States shown | Plan task | PRD |
|---|---|---|---|---|
| [20-search-suggestions.svg](./20-search-suggestions.svg) | Search with Type, Location and Modified chips, suggestions, matching files and operators | Typing, two chips applied | [UI-F2.9](../../../apps/filesquid/ui/resources/plan.md), API-F2.3 | [F-063](../../prd.md#f-063-file-search-query-language-and-filter-chips) |
| [21-search-results.svg](./21-search-results.svg) | Full results with scope and summary chips | `sharedwith:` reported as unsupported on the desktop | UI-F2.9, API-F2.3 | [F-063](../../prd.md#f-063-file-search-query-language-and-filter-chips) |

## Preview

| File | Surface | States shown | Plan task | PRD |
|---|---|---|---|---|
| [22-preview-image.svg](./22-preview-image.svg), [22-preview-image-dark.svg](./22-preview-image-dark.svg) | Image preview with zoom, previous and next, filmstrip and Details panel | Full size loading; light and dark | UI-F2.2, DE-0.5, API-F1.3 | [F-003](../../prd.md#f-003-browsing-selection-and-viewing), [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |
| [23-preview-video.svg](./23-preview-video.svg) | Video playing from ranged reads | Playing, partly buffered | UI-F2.2, API-F1.3 | [F-003](../../prd.md#f-003-browsing-selection-and-viewing) |
| [24-preview-unsupported.svg](./24-preview-unsupported.svg) | A format FileSquid cannot display | Download and Show in Finder offered | UI-F2.2 | [F-003](../../prd.md#f-003-browsing-selection-and-viewing) |

## Panels

| File | Surface | States shown | Plan task | PRD |
|---|---|---|---|---|
| [25-panel-details.svg](./25-panel-details.svg) | Details: storage facts from the OCFL module, and each resource of a RAW+JPEG file | Image selected | [UI-F2.3](../../../apps/filesquid/ui/resources/plan.md) | [F-051](../../prd.md#f-051-ocfl-module), [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |
| [26-panel-versions.svg](./26-panel-versions.svg) | Versions: labels, current version, restore, download, edit label; no delete control | 7 versions, menu open | UI-F2.3, UI-F2.11, API-F2.5 | [F-065](../../prd.md#f-065-file-version-history-and-retention) |
| [27-version-restore.svg](./27-version-restore.svg) | Restore a version as a new version with an optional label | v5 restored as v8 | UI-F2.11, API-F2.5 | [F-065](../../prd.md#f-065-file-version-history-and-retention) |
| [28-panel-metadata.svg](./28-panel-metadata.svg) | Metadata: populated `dcterms:` and `exif:` properties with their source | Owner-entered and extracted values | [UI-F2.4](../../../apps/filesquid/ui/resources/plan.md), API-F1.4 | [F-046](../../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports), [F-052](../../prd.md#f-052-metadata-module) |
| [29-metadata-add-property.svg](./29-metadata-add-property.svg) | Add property: `/terms/` first, then a Schema.org suggestion, registered namespaces, or a custom namespace key | Searching "righ" | UI-F2.4, MD-0.7, API-F1.4 | [F-046](../../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports), [F-052](../../prd.md#f-052-metadata-module) |
| [30-panel-credentials.svg](./30-panel-credentials.svg) | Content Credentials: unsigned import-time record, history, validation codes of a prior credential | Prepared record, not signed; camera signer not in a trust list | [UI-F2.5](../../../apps/filesquid/ui/resources/plan.md), API-F1.5 | [F-043](../../prd.md#f-043-import-time-c2pa-records-and-action-disclosure), [F-054](../../prd.md#f-054-c2pa-module) |

## Monitor, settings and states

| File | Surface | States shown | Plan task | PRD |
|---|---|---|---|---|
| [31-monitor.svg](./31-monitor.svg), [31-monitor-dark.svg](./31-monitor-dark.svg) | Monitor: health, throughput, processes and jobs, queues, activity journal with search and JSON Lines export | One stalled worker; light and dark | [UI-F2.7](../../../apps/filesquid/ui/resources/plan.md), API-F2.1 | [F-061](../../prd.md#f-061-activity-journal-and-live-health-monitoring) |
| [32-settings.svg](./32-settings.svg) | Settings: appearance, browsing preferences, About with the early-access label and privacy statement | System theme selected | UI-0.2, UI-F2.8, DE-F3.1 | [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences), [F-037](../../prd.md#f-037-setup-updates-and-diagnostics), [SRS-UI-007](../../srs.md#srs-ui-007-light-and-dark-themes) |
| [33-state-loading.svg](./33-state-loading.svg) | Folder loading placeholders | First page loading | UI-F2.1 | [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |
| [34-state-storage-unavailable.svg](./34-state-storage-unavailable.svg) | Storage root unavailable (disconnected disk) | Error with one change still pending | UI-F2.1, UI-0.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership), [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |

## Left out because the MVP does not deliver them

C-056 and ADR 035 move these after the MVP, or no MVP task delivers them. v2 draws no control for them.

| Removed from v1 | Why |
|---|---|
| Server sign-in, accounts, groups, Personal library, Shares view, Sharing panel, share buttons, shared and link badges | Desktop-only MVP: no `filesquid-server`, accounts or sharing ([ADR 035](../../architecture/035-macos-first-desktop-mvp.md)); UI-F2.6 and UI-F2.12 come after it. |
| Home view, Activity panel, comments, activity stream | F-064 activity stream and comments move to Task FS-2.7. Home depends on activity, favorites and shares. |
| Favorites, Tags view, Manage tags, `tag:` and `is:favorite` operators | Favorites and keyword views come with CL-1.4 in phase I1. `dcterms:subject` can still be edited in the Metadata panel. |
| Virtual collections, virtual exhibits | F-006 rules are in phase I1; virtual exhibits (UI-F2.13) follow the MVP. |
| People chip, `owner:` and the other server-only operators | One owner on the desktop; server operators need sharing records. `sharedwith:` is shown as unsupported in mockup 21. |
| Signing, embedding and "Scan collection" in Content Credentials | C2PA signing and embedding move to Task FS-3.12; collection scans follow the MVP. The MVP records an unsigned import-time record. |
| Managed or referenced storage choice, default library, watched folders | The MVP imports by copying into the library (F-002 managed import). Referenced storage and watched folders have no MVP task. |
| Permanent delete and retention expiry | The MVP offers remove and restore only; nothing in the API deletes history. |
| Hover card, start-view setting, help button | Optional hover card skipped; the start view chooses between Home and the personal library, neither of which exists on the MVP desktop; no help surface is planned. |
| Backup and sync status | Encrypted backup and sync to `*.filesquid.tech` is phase F4. |

## Notes for the SRS writer

- Product name, labels and colours come from the default label table and tokens. "Import" is the desktop label for the New menu's upload actions.
- Version chips in the list (`v7`) show the latest OCFL version, as UI-F2.2 lists the version column.
- The mockups use sample names and data. They are not real records.
