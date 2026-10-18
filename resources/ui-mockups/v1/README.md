# UI mockups v1: FileSquid default UI

Draft mockups of the screens the FileSquid MVP (the first consumer release) needs, drawn from the [PRD](../../prd.md), the [SRS](../../srs.md) and the module SRS files with the [FileSquid Design Guide](../../assets/FileSquid-Design-Guide.html).

**Status: draft, not the active suite.** The [mockups README](../README.md) names the active suite. Until the owner approves v1 there, these files MUST NOT be read as SRS requirements.

Every mockup is an SVG at 1440 × 900 in light mode, with dark variants of the list and preview screens. Desktop screens show the "This device · only you can read these files" boundary; the welcome screen states it in full as "Who can read your files: only you, on this device." `filesquid-server` screens show the server address and "its operator can read files" ([F-057](../../prd.md#f-057-desktop-and-server-hosts)).

## Onboarding and hosts

| File | Surface | Host | States shown | PRD |
|---|---|---|---|---|
| [01-desktop-welcome.svg](./01-desktop-welcome.svg) | Welcome: create a library, open a library, recover from a copied OCFL storage root | Desktop | First run, no library; privacy boundary and no-telemetry footer | [F-001](../../prd.md#f-001-independent-local-library-and-ownership), [F-042](../../prd.md#f-042-ocfl-preservation-and-portable-recovery), [F-057](../../prd.md#f-057-desktop-and-server-hosts) |
| [02-create-library.svg](./02-create-library.svg) | Create library form: name, location, managed or referenced storage, default library, optional watched folder; on-disk OCFL layout aside | Desktop | Form filled, managed storage selected | [F-001](../../prd.md#f-001-independent-local-library-and-ownership), [F-002](../../prd.md#f-002-safe-file-folder-and-device-import), [F-009](../../prd.md#f-009-multiple-libraries-and-filesystem-administration), [F-042](../../prd.md#f-042-ocfl-preservation-and-portable-recovery) |
| [03-server-sign-in.svg](./03-server-sign-in.svg) | Sign in, with the operator's statement of who can read files | Server | Signed out | [F-057](../../prd.md#f-057-desktop-and-server-hosts), [NFR-004](../../prd.md#nfr-004-security-and-privacy) |

## Browsing

| File | Surface | Host | States shown | PRD |
|---|---|---|---|---|
| [04-home.svg](./04-home.svg) | Home: recent activity on your files, favorites, shared with you | Server | Populated; fixed newest-first order | [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences), [F-064](../../prd.md#f-064-file-activity-comments-and-user-activity-stream), [F-066](../../prd.md#f-066-file-and-folder-sharing-roles-links-and-shares-view) |
| [05-all-files-list.svg](./05-all-files-list.svg), [05-all-files-list-dark.svg](./05-all-files-list-dark.svg) | All files, list layout (default), with the full sidebar: views, libraries, virtual collections, virtual exhibits, deleted files, storage used | Desktop | Populated folder; light and dark | [F-004](../../prd.md#f-004-collections-folders-and-navigation), [F-055](../../prd.md#f-055-default-ui-and-panel-composition), [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) |
| [06-all-files-grid.svg](./06-all-files-grid.svg) | All files, grid layout with thumbnail size and the sort menu | Desktop | Sort menu open | [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) |
| [07-selection-bulk-actions.svg](./07-selection-bulk-actions.svg) | Bulk action bar stating the effective scope; ZIP download under the outgoing policy | Desktop | 5 items selected (4 files, 1 folder) | [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences), [F-010](../../prd.md#f-010-export-and-portable-backup), [F-046](../../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports) |
| [08-new-and-item-menus.svg](./08-new-and-item-menus.svg) | New menu (upload files, upload folder, new folder) and per-item menu, including reveal in file manager | Desktop | Both menus open; new folder pending until a file is added | [F-002](../../prd.md#f-002-safe-file-folder-and-device-import), [F-004](../../prd.md#f-004-collections-folders-and-navigation), [F-062](../../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) |
| [09-move-copy-picker.svg](./09-move-copy-picker.svg) | Move or copy picker: move keeps identity and records an OCFL version; copy creates new files with provenance | Desktop | 2 files selected, Move tab | [F-004](../../prd.md#f-004-collections-folders-and-navigation), [F-042](../../prd.md#f-042-ocfl-preservation-and-portable-recovery) |
| [10-drag-drop-upload.svg](./10-drag-drop-upload.svg) | Drag-and-drop upload target and upload progress | Desktop | Committed, verifying SHA-256, uploading, waiting, failed with retry | [F-002](../../prd.md#f-002-safe-file-folder-and-device-import), [F-042](../../prd.md#f-042-ocfl-preservation-and-portable-recovery) |

## Search

| File | Surface | Host | States shown | PRD |
|---|---|---|---|---|
| [11-search-suggestions.svg](./11-search-suggestions.svg) | Search box with filter chips, filter suggestions, file matches and the operator list | Desktop | Typing, with Type and Modified chips applied | [F-063](../../prd.md#f-063-file-search-query-language-and-filter-chips) |
| [12-search-results.svg](./12-search-results.svg) | Search results with scope and result summary chips | Desktop | Unsupported operator (`sharedwith:` on desktop) reported, no results dropped | [F-063](../../prd.md#f-063-file-search-query-language-and-filter-chips) |

## Preview

| File | Surface | Host | States shown | PRD |
|---|---|---|---|---|
| [13-preview-image.svg](./13-preview-image.svg), [13-preview-image-dark.svg](./13-preview-image-dark.svg) | Image preview with filmstrip and the Details panel | Desktop | Full resolution loading; light and dark | [F-003](../../prd.md#f-003-browsing-selection-and-viewing), [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |
| [14-preview-unsupported.svg](./14-preview-unsupported.svg) | Preview of a format FileSquid cannot display | Desktop | No preview; download and reveal offered | [F-003](../../prd.md#f-003-browsing-selection-and-viewing) |

## Panels

| File | Surface | Host | States shown | PRD |
|---|---|---|---|---|
| [15-panel-details.svg](./15-panel-details.svg) | Details: storage facts from the OCFL module (identifier, logical path, versions, SHA-256, fixity) | Desktop | Image selected | [F-051](../../prd.md#f-051-ocfl-module), [F-055](../../prd.md#f-055-default-ui-and-panel-composition) |
| [16-panel-versions.svg](./16-panel-versions.svg) | Versions: labelled history, restore as new version, download, edit label; no delete control | Desktop | 7 versions | [F-065](../../prd.md#f-065-file-version-history-and-retention) |
| [17-panel-metadata.svg](./17-panel-metadata.svg) | Metadata: populated `dcterms:` and `exif:` keys with prefixes, Schema.org suggestion, custom namespace key | Desktop | Owner-authored and extracted values | [F-046](../../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports), [F-052](../../prd.md#f-052-metadata-module) |
| [19-panel-activity.svg](./19-panel-activity.svg) | Activity: changes and comments, with mentions | Server | All tab; comment being written | [F-064](../../prd.md#f-064-file-activity-comments-and-user-activity-stream) |
| [20-panel-sharing.svg](./20-panel-sharing.svg) | Sharing: people and groups with roles, internal link, public link with password, expiry and outgoing policy | Server | Re-sharing turned off by the administrator | [F-066](../../prd.md#f-066-file-and-folder-sharing-roles-links-and-shares-view), [F-046](../../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports) |

## Not drawn yet

The v1 mockup task also calls for these screens, which are not in this folder yet:

- Views: activity, shares, tags, deleted files, permanent delete.
- Organisation: collection, virtual collection rules, virtual exhibit, duplicates.
- Import, export, disclosure preview, batch metadata, provenance scan.
- Libraries, recovery, settings, notifications, server admin, sharing policy, Monitor, diagnostics, and empty, loading, error and offline states.
- A dark variant of Monitor.
