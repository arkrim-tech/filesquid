# UI mockups v2: FileSquid MVP (macOS desktop)

Draft mockups of every screen a person uses in the **FileSquid MVP**: the early-access default distribution on the macOS desktop host ([C-056](../../product-vision.md#release-sequence), [ADR 035](../../architecture/035-macos-first-desktop-mvp.md)). The scope comes from phases F0 to F3 of the [implementation plan](../../implementation-plan.md), up to the gate [Task FS-3.9](../../implementation-plan.md#phase-f3-filesquid-mvp-macos-desktop). The styling follows the [FileSquid Design Guide](../../assets/FileSquid-design-guide-v1.html), and the layouts build on the [v1 drafts](../v1/README.md).

**Status: draft, not the active suite.** The [mockups README](../README.md) names the active suite. Until the owner approves v2 there, these files MUST NOT be read as SRS requirements.

## How v2 differs from v1

- **MVP features only.** v1 drew the whole first consumer release. v2 shows only controls whose function an MVP task delivers. The [Left out](#left-out-because-the-mvp-does-not-deliver-them) list names what was removed and why.
- **macOS window.** Every screen is a 1440 × 900 macOS window with the title bar and window buttons, because the MVP ships only as a macOS DMG.
- **Logo.** The mark comes from the coloured [`FileSquid-logo.svg`](../../assets/FileSquid-logo.svg): a green head and red, yellow and blue shapes in both modes, with a black outline and wordmark on light surfaces and a white one on dark surfaces.
- **No early-access label (owner decision, 2026-10-19).** The sidebar, the welcome screen and About no longer show an "Early access" badge or label. The MVP is still the C-056 early-access build; only the label is gone from the UI (see the notes below for DE-F3.1).
- **Host indicator and privacy statement (owner decision 1a, 2026-10-19).** The header no longer shows the sentence "This device · only you can read these files". In its place every browsing screen shows a small host indicator badge: a device icon and the host label "This device" ([UI-S-004](../../../apps/filesquid/ui/resources/srs.md)). Opening the badge shows a popover with the [ADR 032](../../architecture/032-hosting-and-privacy-model.md) desktop statement "Your files stay on this device. Only you can read them." and, when backup or sync is on (after the MVP), "Backup copies are encrypted on this device before upload." ([40](./40-host-indicator.svg)). Settings repeats the statement in a Privacy section ([32](./32-settings.svg)), and the welcome screen keeps its privacy statement.
- **Original file names (owner requirement, 2026-10-19).** FileSquid stores each file as `<file-uuid>.<ext>` with its metadata record `<file-uuid>.metadata.md` beside it. Every screen shows the original file name and never the UUID storage name, and owner downloads and exports use the original name. Storage paths that would reveal the storage name (the former "Content location" row) are not shown.
- **Tabbed file panel (owner requirement and decisions, 2026-10-19).** The file side panel has three tabs with the same layout and behaviour: File info, Description and Provenance ([35](./35-panel-file-info.svg), [28](./28-panel-metadata.svg), [36](./36-panel-provenance.svg)). The former Audit log tab is removed: a file's journal entries are browsed in Logs, filtered to that file. Versions opens from the history button in the panel header ([26](./26-panel-versions.svg)). The Description tab no longer shows the "Saved to this file's metadata record" footer.
- **Logs (owner decision, 2026-10-19).** The sidebar item formerly called Journal is named Logs, keeps its icon and sits directly below Settings at the bottom of the sidebar on every screen with a sidebar. It opens the Logs view, where the owner browses the audit log: every journal entry, filterable, including by file ([38](./38-logs-view.svg), [38 dark](./38-logs-view-dark.svg)), with its export dialog ([39](./39-logs-export.svg)). "View in Logs" in the Provenance tab opens the view filtered to that file and operation.

## Design constants used

- Brand green `#16AC49` in both modes, for the one primary action per section, active states and focus. The tri-colour (`#FD1E23`, `#FCD805`, `#007AF9`) appears only in file-type badges, never as a status colour.
- Light: page `#FFFFFF`, raised `#F5F5F5`, borders `#E0E0E0` and `#CCCCCC`, text `#000000` / `#111111` / `#555555` / `#777777`, active tint `#F0FAF4` with `#0f8a39`, error `#E53935`.
- Dark: page `#141414`, card `#1C1C1C`, raised `#242424`, overlay `#2C2C2C`, borders `#2A2A2A` and `#333333`, text `#F5F5F5` / `#E0E0E0` / `#A0A0A0` / `#6A6A6A`, active tint `#0D2B18` with `#4ACA74`, error `#EF5350`.
- Flat fills only: no gradients, glows or shadows. Dark mode uses surface steps for elevation.
- Georgia serif headings, system sans UI text, monospace for identifiers, digests, paths and namespace prefixes. Sentence case everywhere.
- Sidebar 240 px, right panel 320 px, 8 px spacing scale, buttons with a 7 px radius, inputs with a 6 px radius, line icons with round caps.
- Status colours always come with text: `ok` on the active tint, `refused` outlined in the warning colour (`#8A6D00` light, `#E0C25A` dark), `failed` outlined in the error colour. Journal levels use the same colours (`info` secondary text, `warn`, `error`).
- Keyboard focus is a 2 px brand-green ring (inputs: a 2 px brand-green border), drawn in mockups 36, 39 and 40.

## File panel: one frame for three tabs

Every tab of the file side panel uses the same frame, so the three tabs look and work alike:

1. Header: the file-type badge and the **original file name**, a history button that opens Versions, and close.
2. Tab strip: File info, Description, Provenance (13 px, left-aligned with 24 px between labels; the active tab in the active colour with a 2.5 px brand underline).
3. A search and filter box for that tab (32 px tall).
4. An uppercase section label with the count, then a property/value table: each row has the property on its first line (prefixed properties in monospace) and the value below, with a copy control on every row. Rows the owner can change also show edit and remove on hover (Description).
5. A footer line naming where the values come from (File info), or the tab's actions (Provenance: View in Logs and JSON-LD). Description has no footer.
6. One treatment for loading, empty, no-match and error states, shown in [25](./25-panel-states.svg).

## Library lifecycle

| File | Surface | States shown | Plan task | PRD |
|---|---|---|---|---|
| [01-welcome.svg](./01-welcome.svg) | First run: create, open or recover a library | No library open; privacy statement, no telemetry | [UI-1.1](../../../apps/filesquid/ui/resources/plan.md), Task 1.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership), [F-057](../../prd.md#f-057-desktop-and-server-hosts) |
| [02-create-library.svg](./02-create-library.svg) | Create library: name and the folder that becomes the OCFL storage root, with the on-disk layout | Form filled | UI-1.1, CL-1.1, DE-1.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership), [F-042](../../prd.md#f-042-ocfl-preservation-and-portable-recovery) |
| [03-open-library.svg](./03-open-library.svg) | Open library: recent libraries and a chosen folder checked as a valid OCFL 1.1 storage root | One recent library on a disconnected disk | UI-1.1, DE-1.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership) |
| [04-recover-library.svg](./04-recover-library.svg) | Recover from a copy: validate every object and rebuild the catalog from OCFL alone | Durable job running; one digest mismatch reported, nothing repaired | UI-1.1, CL-1.1, IO-1.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership), [F-042](../../prd.md#f-042-ocfl-preservation-and-portable-recovery) |
| [08-library-menu.svg](./08-library-menu.svg) | Library actions: show storage root, copy, rebuild catalog, create or open another, close | Menu open | UI-1.1, CL-1.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership) |
| [09-copy-library.svg](./09-copy-library.svg) | Copy library: a checked OCFL snapshot readable with ordinary tools | Destination chosen | CL-1.1, DE-1.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership), [F-042](../../prd.md#f-042-ocfl-preservation-and-portable-recovery) |

## Browsing and organising

| File | Surface | States shown | Plan task | PRD |
|---|---|---|---|---|
| [05-all-files-list.svg](./05-all-files-list.svg), [05-all-files-list-dark.svg](./05-all-files-list-dark.svg) | Folder in the list layout (default) with the full shell | Populated folder, row hover; light and dark | [UI-F2.1a](../../../apps/filesquid/ui/resources/plan.md), UI-F2.1b, UI-F2.2, UI-F2.8 | [F-055](../../prd.md#f-055-default-ui-and-panel-composition), [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) |
| [06-all-files-grid.svg](./06-all-files-grid.svg) | Grid layout and the sort menu (five sort keys, direction, folder placement) | Sort menu open | UI-F2.8, API-F2.2 | [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) |
| [07-empty-library.svg](./07-empty-library.svg) | Empty library with import actions and a drop area | New library | UI-F2.1a, UI-F2.1b, UI-1.2 | [F-002](../../prd.md#f-002-safe-file-folder-and-device-import), [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |
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
| [18-import-review.svg](./18-import-review.svg) | Import review: proposed and already-imported items, destination, counts, space and staging limit | Camera card with 18 duplicates skipped | UI-1.2, DE-1.2 | [F-002](../../prd.md#f-002-safe-file-folder-and-device-import) |
| [19-import-progress.svg](./19-import-progress.svg) | Import jobs panel | Committed, verifying SHA-256, copying, waiting, failed with retry; a paused job resumed from its checkpoint | UI-1.2, DE-0.4, API-F1.6 | [F-002](../../prd.md#f-002-safe-file-folder-and-device-import) |

## Search

| File | Surface | States shown | Plan task | PRD |
|---|---|---|---|---|
| [20-search-suggestions.svg](./20-search-suggestions.svg) | Search with Type, Location and Modified chips, suggestions, matching files and operators | Typing, two chips applied | [UI-F2.9](../../../apps/filesquid/ui/resources/plan.md), API-F2.3 | [F-063](../../prd.md#f-063-file-search-query-language-and-filter-chips) |
| [21-search-results.svg](./21-search-results.svg) | Full results with scope and summary chips | `sharedwith:` reported as unsupported on the desktop | UI-F2.9, API-F2.3 | [F-063](../../prd.md#f-063-file-search-query-language-and-filter-chips) |

## Preview

| File | Surface | States shown | Plan task | PRD |
|---|---|---|---|---|
| [22-preview-image.svg](./22-preview-image.svg), [22-preview-image-dark.svg](./22-preview-image-dark.svg) | Image preview with zoom, previous and next, filmstrip and the file panel on File info | Full size loading; light and dark | UI-F2.2, DE-0.5, API-F1.3 | [F-003](../../prd.md#f-003-browsing-selection-and-viewing), [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |
| [23-preview-video.svg](./23-preview-video.svg) | Video playing from ranged reads, with the file panel on File info (duration read from the file) | Playing, partly buffered | UI-F2.2, API-F1.3 | [F-003](../../prd.md#f-003-browsing-selection-and-viewing) |
| [24-preview-unsupported.svg](./24-preview-unsupported.svg) | A format FileSquid cannot display, with the file panel on File info | Download and Show in Finder offered; nothing read from the format | UI-F2.2 | [F-003](../../prd.md#f-003-browsing-selection-and-viewing) |

## File panel

| File | Surface | States shown | Plan task | PRD |
|---|---|---|---|---|
| [35-panel-file-info.svg](./35-panel-file-info.svg) | File info tab: original file name, extension, format and MIME type, size, SHA-256 digest, import time (local and UTC), source, OCFL object id (`urn:uuid:`) and version, fixity; then the resources of a RAW+JPEG pair | SHA-256 row hovered with copy; read only | [UI-F2.3](../../../apps/filesquid/ui/resources/plan.md), API-F1.4 | [F-051](../../prd.md#f-051-ocfl-module), [F-002](../../prd.md#f-002-safe-file-folder-and-device-import), [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |
| [28-panel-metadata.svg](./28-panel-metadata.svg) | Description tab: populated Dublin Core (`dcterms:`) and Schema.org (`schema:`) properties with prefixes, owner-entered (You) or extracted (File), edit, remove, copy, Add property | One row hovered | [UI-F2.4](../../../apps/filesquid/ui/resources/plan.md), API-F1.4 | [F-046](../../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports), [F-052](../../prd.md#f-052-metadata-module) |
| [29-metadata-add-property.svg](./29-metadata-add-property.svg) | Add property over the Description tab: `/terms/` first, then a Schema.org suggestion, registered namespaces, or a custom namespace key | Searching "righ" | UI-F2.4, MD-0.7, API-F1.4 | [F-046](../../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports), [F-052](../../prd.md#f-052-metadata-module) |
| [36-panel-provenance.svg](./36-panel-provenance.svg) | Provenance tab (PROV-O content): the file's history newest first; each `prov:Activity` with `prov:startedAtTime` / `prov:endedAtTime` (UTC), `prov:wasAssociatedWith` the owner agent as the installation `urn:uuid:` (no person name) and the FileSquid software agent and version, `prov:used` and generated entities as object@version with logical path and digest, `prov:wasDerivedFrom`, and `prov:wasInvalidatedBy` for a remove; View in Logs (Logs filtered to this file and operation); JSON-LD export | Latest activity expanded, filter focused | [UI-F2.15](../../../apps/filesquid/ui/resources/plan.md), API-F2.9 | [F-067](../../prd.md#f-067-prov-o-history-of-every-version-and-change), [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |
| [25-panel-states.svg](./25-panel-states.svg) | The shared loading, empty, no-match and error treatment of the three tabs | File info loading, Description empty, Provenance with no match, Provenance failed with Retry and View in Logs | UI-F2.3, UI-F2.4 | [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |
| [26-panel-versions.svg](./26-panel-versions.svg) | Versions view, opened from the history button in the panel header: labels, current version, restore, download, edit label; no delete control; Back returns to the open tab | 7 versions, menu open | UI-F2.3, UI-F2.11, API-F2.5 | [F-065](../../prd.md#f-065-file-version-history-and-retention) |
| [27-version-restore.svg](./27-version-restore.svg) | Restore a version as a new version with an optional label | v5 restored as v8 | UI-F2.11, API-F2.5 | [F-065](../../prd.md#f-065-file-version-history-and-retention) |

## Monitor, settings and states

| File | Surface | States shown | Plan task | PRD |
|---|---|---|---|---|
| [31-monitor.svg](./31-monitor.svg), [31-monitor-dark.svg](./31-monitor-dark.svg) | Monitor: health, throughput, processes and jobs, queues, activity journal with search and JSON Lines export | One stalled worker; light and dark | [UI-F2.7](../../../apps/filesquid/ui/resources/plan.md), API-F2.1 | [F-061](../../prd.md#f-061-activity-journal-and-live-health-monitoring) |
| [38-logs-view.svg](./38-logs-view.svg), [38-logs-view-dark.svg](./38-logs-view-dark.svg) | Logs, opened from the sidebar item below Settings: the audit log as a full-page table of journal entries newest first with filters for time range, library, operation, outcome and level, search by file name, operation id or object id, Live indicator, entry detail drawer with View provenance, and Export | One entry selected; light and dark | UI-F2.7, API-F2.1 | [F-061](../../prd.md#f-061-activity-journal-and-live-health-monitoring) |
| [39-logs-export.svg](./39-logs-export.svg) | Export logs: scope (current filter, time range or all entries), format (JSON Lines or CSV), a note that exports keep opaque ids, destination file name, Export and Cancel | Current filter, JSON Lines | UI-F2.7, API-F2.1 | [F-061](../../prd.md#f-061-activity-journal-and-live-health-monitoring) |
| [32-settings.svg](./32-settings.svg) | Settings: appearance, browsing preferences, Privacy with the ADR 032 desktop statement and no telemetry, About with the version and the unsigned-build note | System theme selected | UI-0.2, UI-F2.8, DE-F3.1 | [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences), [F-037](../../prd.md#f-037-setup-updates-and-diagnostics), [SRS-UI-007](../../srs.md#srs-ui-007-light-and-dark-themes) |
| [40-host-indicator.svg](./40-host-indicator.svg) | Host indicator badge in the header and its popover with the ADR 032 desktop statement, and the backup line when backup or sync is on; link to Settings, Privacy | Popover open with keyboard focus; MVP and backup-on; light and dark | UI-0.1, UI-F2.1a | [F-055](../../prd.md#f-055-default-ui-and-panel-composition), [F-057](../../prd.md#f-057-desktop-and-server-hosts) |
| [33-state-loading.svg](./33-state-loading.svg) | Folder loading placeholders | First page loading | UI-F2.1a, UI-F2.1b | [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |
| [34-state-storage-unavailable.svg](./34-state-storage-unavailable.svg) | Storage root unavailable (disconnected disk) | Error with one change still pending | UI-F2.1a, UI-F2.1b, UI-0.1 | [F-001](../../prd.md#f-001-independent-local-library-and-ownership), [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |

## Left out because the MVP does not deliver them

C-056 and ADR 035 move these after the MVP, or no MVP task delivers them. v2 draws no control for them.

| Removed from v1 | Why |
|---|---|
| Server sign-in, accounts, groups, Personal library, Shares view, Sharing panel, share buttons, shared and link badges | Desktop-only MVP: no `filesquid-server`, accounts or sharing ([ADR 035](../../architecture/035-macos-first-desktop-mvp.md)); UI-F2.6 and UI-F2.12 come after it. |
| Home view, Activity panel, comments, activity stream | F-064 activity stream and comments move to Task FS-2.7. Home depends on activity, favorites and shares. |
| Favorites, Tags view, Manage tags, `tag:` and `is:favorite` operators | Favorites and keyword views come with CL-1.4 in phase I1. `dcterms:subject` can still be edited in the Description tab of the file panel. |
| Virtual collections, virtual exhibits | F-006 rules are in phase I1; virtual exhibits (UI-F2.13) follow the MVP. |
| People chip, `owner:` and the other server-only operators | One owner on the desktop; server operators need sharing records. `sharedwith:` is shown as unsupported in mockup 21. |
| Content Credentials tab, panel (former mockup 30), import-time C2PA records, signing, embedding and "Scan collection" | No C2PA in the MVP or release 1 ([ADR 037](../../architecture/037-c2pa-in-release-2.md)). C2PA ships with the ImageSquid release (release 2), when the default FileSquid sidebar regains a Credentials tab and the ImageSquid inspector shows its C2PA tab. Import review and library recovery mention the file record instead. |
| Managed or referenced storage choice, default library, watched folders | The MVP imports by copying into the library (F-002 managed import). Referenced storage and watched folders have no MVP task. |
| Permanent delete and retention expiry | The MVP offers remove and restore only; nothing in the API deletes history. |
| Hover card, start-view setting, help button | Optional hover card skipped; the start view chooses between Home and the personal library, neither of which exists on the MVP desktop; no help surface is planned. |
| Backup and sync status | Encrypted backup and sync to `*.filesquid.tech` is phase F4. |

## Notes for the SRS writer

- Product name, labels and colours come from the default label table and tokens. "Import" is the desktop label for the New menu's upload actions.
- Version chips in the list (`v7`) show the latest OCFL version, as UI-F2.2 lists the version column.
- The mockups use sample names and data. They are not real records.
- The 2026-10-19 owner requirements and decisions behind mockups 22 to 29 and 32 to 40 are not yet fully in the PRD, SRS, ADRs or plans. Before v2 becomes the active suite, the documentation needs these changes:
  - [F-055](../../prd.md#f-055-default-ui-and-panel-composition), the default UI SRS (UI-S-019 to UI-S-022) and the UI plan task UI-F2.15 describe four tabs (File facts, Descriptive, PROV-O, Audit log) and a Journal tab reachable from the app header. The mockups show three tabs (File info, Description, Provenance), no Audit log tab, and Logs as a sidebar item below Settings whose view is the audit log, filterable by file.
  - DE-F3.1 in the desktop plan says the About view says "early access" and its packaged-window test asserts the early-access label; the owner removed that label from the UI.
  - [F-052](../../prd.md#f-052-metadata-module), [ADR 027](../../architecture/027-filesquid-metadata-pluggable-stores.md) and [ADR 038](../../architecture/038-import-file-record.md) describe `metadata.md` and `file-record.md` as logical files of the OCFL object; the owner's layout is `<file-uuid>.<ext>` with `<file-uuid>.metadata.md` beside it.
  - [ADR 033](../../architecture/033-activity-journal-and-live-health-monitoring.md) lists journal filters for time range, actor, operation, outcome, level and process and a JSON Lines export. The Logs view also filters by library, by object id and by file (search by file name), and the export dialog offers CSV.
  - The journal stores opaque ids only (ADR 033). The Logs view shows original file names by looking the object id up in the library when it displays a row; exports keep the opaque ids.
