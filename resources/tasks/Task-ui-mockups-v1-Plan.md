# Task ui-mockups-v1: Plan

Branch: `ui-mocksups-v1`. Documentation-only task (design artifacts). Output: SVG mockups in [`resources/ui-mockups/v1/`](../ui-mockups/v1/).

## Objective

Identify every screen the FileSquid MVP (the first consumer release) needs, from the [PRD](../prd.md) and [SRS](../srs.md) plus the module SRS files, and draw one SVG mockup per screen using the [FileSquid Design Guide](../assets/FileSquid-design-guide-v1.html).

## MVP scope used

The [FileSquid release row](../prd.md#release-order-and-dependency-gates) assigns: F-049–F-058 (F-050, F-051, F-052, F-054, F-055–F-058), F-001, F-002 (files, folders, cards, selected uploads, watched folders), F-004, F-005, F-006, F-008, F-009, F-010 (original export), F-037, F-038, F-039, F-040, F-042 (local OCFL create/version/copy/export/recover), F-043 (initial records, free scans and reports), F-046 (inspection, `dcterms:`/`schema:` authoring, outgoing policy), F-053, and server users, groups, group-owned libraries and collection permissions. The default-UI items F-061–F-066 are mandatory for the FileSquid release ([F-061](../prd.md#f-061-activity-journal-and-live-health-monitoring), [UI-S-006–010](../../apps/filesquid/ui/resources/srs.md)). Out of scope: hosted backup and sync (F-059, F-029–F-034), the encrypted browser host and mobile web (F-036), ImageSquid photo views, migration routes, AI, paid C2PA publishing (F-044), Arkrim (F-047, F-060).

## Design constants (from the design guide)

- Brand green `#16AC49` in both modes: primary button, active state, focus, accent link. One green primary action per screen section.
- Light: page `#FFFFFF`, raised `#F5F5F5`, borders `#E0E0E0` (subtle) and `#CCCCCC` (inputs, tables), heading `#000000`, body `#111111`, secondary `#555555`, muted `#777777`, placeholder `#999999`, active tint `#F0FAF4` with text `#0f8a39`, error `#E53935`.
- Dark: page `#141414`, card `#1C1C1C`, raised `#242424`, overlay `#2C2C2C`, borders `#2A2A2A` and `#333333`, heading `#F5F5F5`, body `#E0E0E0`, secondary `#A0A0A0`, muted `#6A6A6A`, active tint `#0D2B18` with text `#4ACA74`, error `#EF5350`. No pure black page, no white body text.
- Flat fills only: no gradients, glows or drop shadows. Dark mode uses surface steps for elevation.
- Tri-colour `#FD1E23` (documents), `#FCD805` (data), `#007AF9` (media) only in the logo and file-type badges, never as status colours.
- Type: headings Georgia serif 700; UI system sans; technical terms and identifiers (OCFL, C2PA, `dcterms:`, digests) in monospace chips. Sentence case everywhere, never all-caps buttons; eyebrows 11px uppercase tracked muted.
- Layout: product sidebar 240px; right detail panel 320px max; 8px spacing scale (4, 8, 16, 24, 40, 64).
- Components: buttons 40px tall (36px compact), radius 7px, 600 weight; primary green/white, secondary card with 1.5px border, ghost green text, destructive error outline. Tags radius 4px, 12px. File badges monospace 11px bold, radius 4px. Inputs 38px, 1.5px border, radius 6px; focus green, error red with message. Sidebar active item: tint background, green text, 2.5px green left border.
- Icons: line icons, 1.5–2px stroke, round caps, no fill; default `#111111` light, `#A0A0A0` dark; active green; muted `#777777`/`#6A6A6A`.
- Voice: plain, active, British spelling, no "seamless", "powerful", "simply", "just", no exclamation marks, no "Please" in errors.

## Product constraints that shape the screens

- FileSquid default UI uses internal names only: repository, library, collection, virtual collection, virtual exhibit, file ([F-049](../prd.md#f-049-product-component-terminology-and-item-type)).
- Each host states its privacy boundary and the UI shows which host it is connected to ([F-057](../prd.md#f-057-desktop-and-server-hosts), UI-S-004).
- Sidebar: view filter at top; home, all files, personal library, recent, favorites, shares, tags; deleted files, storage-used indicator and settings at the bottom ([F-062](../prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences)).
- One New control (upload files, upload folder, new folder); list is the default layout with checkbox, name, size, modified, per-row share and actions, footer summary; bulk bar states the effective scope; per-item menu: favorite, details, manage tags, rename, move or copy, download, share, delete, plus reveal in file manager on desktop.
- Panels: Details, Versions, Metadata, Content Credentials, Activity, Sharing in named slots ([F-055](../prd.md#f-055-default-ui-and-panel-composition)). Versions shows no delete control ([F-065](../prd.md#f-065-file-version-history-and-retention)). Metadata lists populated keys only with prefixes ([F-046](../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports)). Credentials distinguishes prepared/unsigned, signed, failed and unavailable ([F-043](../prd.md#f-043-import-time-c2pa-records-and-action-disclosure)).
- Search chips Type, People, Modified, Location; scope and result summary as chips; unsupported operators reported ([F-063](../prd.md#f-063-file-search-query-language-and-filter-chips)).
- Outgoing policy: scrubbing on by default with empty retained-field list; FileId outgoing filename with explicit "Use original filename" ([F-046](../prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports)).
- Recoverable deletion 30 days with expiry shown; permanent deletion needs explicit scope confirmation ([F-008](../prd.md#f-008-deletion-hidden-files-and-duplicate-review)).
- Monitor dashboard only for the desktop owner or server administrators ([F-061](../prd.md#f-061-activity-journal-and-live-health-monitoring)).

## Approach

1. Build a small SVG generator in the session scratch area (not committed) with shared tokens, icons and components so every mockup is consistent.
2. Draw each screen at 1440 × 900 (desktop window or browser window for `filesquid-server`), light mode, plus dark variants of three key screens.
3. Write `resources/ui-mockups/v1/README.md` (not yet created) indexing each mockup with its surface, host, states and PRD IDs.
4. Do not mark v1 as the active suite in [`resources/ui-mockups/README.md`](../ui-mockups/README.md) without owner approval, because an active suite becomes a hard SRS requirement source.

## Verification

- Every SVG parses as XML (`xmllint --noout`).
- Render every SVG to PNG with Chromium and inspect for overlap, clipping and token misuse.
- Run the repository pre-commit hook before committing.
