---
title: "ImageSquid Distribution Software Requirements Specification"
parent: "./prd.md"
---

# ImageSquid distribution SRS

ImageSquid is a FileSquid distribution: vanilla TypeScript views, theme tokens, a label table, brand assets and a distribution manifest, with no Rust and no API code of its own. Parent: the [ImageSquid PRD](./prd.md). Requirements use RFC 2119. FileSquid requirements, documents and ADRs are named in plain text, not linked, so this SRS stands on its own.

Revised 2026-10-18: UI-S-016 (accessibility contracts every shared element meets) was copied from the `@filesquid/ui` SRS. Revised 2026-10-16: SRS-UI-001–006 and SRS-UI-008, the mockup coverage and the screenshot decisions moved here from the FileSquid SRS and keep their IDs. UI-S-011, UI-S-012, UI-S-013 and UI-S-016 are owned by the `@filesquid/ui` SRS and are copied below because ImageSquid MUST meet them. Revised 2026-10-16: the legacy `FE-S-` IDs were renamed so the prefix no longer collides with the vision's `FE-###` feature requirements; the inherited rows formerly FE-S-001, FE-S-002 and FE-S-004 are now UI-S-011, UI-S-012 and UI-S-013 (UI-S-001–010 were already taken in the `@filesquid/ui` SRS), and the local rows formerly FE-S-003 and FE-S-005 are IS-S-009 and IS-S-010. Revised 2026-10-16 (traceability sweep): added [PRD coverage](#prd-coverage), mapping every ImageSquid PRD requirement to the ImageSquid surfaces and distribution rows that implement it and naming the FileSquid SRS item behind each capability; no requirement text changed. Revised 2026-10-16: [Mobile photo sync and capture](#mobile-photo-sync-and-capture) (IS-S-011–IS-S-021) added from the mobile PWA requirements FR-01–FR-11, and SRS-UI-006 gains the phone main screen.

## Screenshots

The current ImageSquid screenshots are [screen1-light-mode.png](./ui-mockups/screen1-light-mode.png) (library grid with a C2PA floatover) and [screen2-light-mode.png](./ui-mockups/screen2-light-mode.png) (viewer with the Metadata / C2PA / Archive inspector). Each is a normative look-and-feel input for the surface it shows: a requirement that cites it MUST follow its composition (regions, default view, grouping, hierarchy, designed controls, and shown or withheld status text). A pixel or two of browser rendering difference is acceptable. A screenshot hash is not required. The v1–v4 mockup suites were retired by the owner on 2026-10-15. Requirements derived from them are retained and cite the [UI mockups index](./ui-mockups/README.md). The screenshots are **canonical** for the surfaces they show: where a screenshot and older PRD, vision or SRS wording disagree, the screenshot governs and the text below follows it. Requirements derived from retired mockups that the screenshots do not show remain in force; for those, SHA-256, the selected publisher and OCFL 1.1 apply as the PRD and ADRs state.

## Terminology

ImageSquid's label table maps FileSquid's internal names to ImageSquid UI labels: repository → **repo**, library → **library**, collection → **album**, virtual collection → **gallery**, virtual exhibit → **exhibit**, file → **photo**, **image** or **video** (media kind). ImageSquid UI MUST use **repo**, **library**, **album**, **gallery**, and **exhibit**, and MAY label a given file photo, image, or video. ImageSquid primary chrome MUST use **Archives** for the preservation store. When an ImageSquid collection skim shows origin labels, those labels MUST be **AI** or **RW** (Real World). The Content Credentials pin MAY remain **CR**.

## Distribution requirements

| Local requirement | Parent |
| --- | --- |
| IS-S-001 ImageSquid MUST NOT contain backend code or call anything except the FileSquid API through `FileSquidClient`. A capability it needs that FileSquid lacks MUST be added to FileSquid as a module. | [F-058](./prd.md#f-058-white-label-distributions), FileSquid U-25, FileSquid ADR 029 |
| IS-S-002 The ImageSquid label table MUST define every key of the default `@filesquid/ui` label table, and its token file MUST define every default token, for light and dark. | FileSquid SRS-UI-007, FileSquid U-24 |
| IS-S-003 The [ImageSquid screenshots](./ui-mockups) are canonical for the surfaces they show: library grid ([screen1](./ui-mockups/screen1-light-mode.png)) and viewer with inspector ([screen2](./ui-mockups/screen2-light-mode.png)). Where a screenshot and written wording disagree, the screenshot governs. | [SRS-UI-001](#srs-ui-001-library-chrome), [SRS-UI-002](#srs-ui-002-media-inspector) |
| IS-S-004 The ImageSquid label table MUST provide `automaticGalleriesHeading` = "Collections" (sidebar heading over Favorites, Places, Videos, Screenshots, Selfies) and `searchPlaceholder` = "Search photos, collections, dates…". All other ImageSquid nouns stay repo, library, album, gallery and exhibit. | [ImageSquid U-05](./product-vision.md#user-requirements) |
| IS-S-005 Provenance skim MUST be on by default: every tile in the default All Photos grid shows an **AI** or **RW** origin chip; the **CR** chip turns skim off and on. | [SRS-UI-001](#srs-ui-001-library-chrome) |
| IS-S-006 The C2PA floatover MUST show an origin badge reading **Real** or **AI**, a confidence and verification line, rows Captured, Camera, Signed by and C2PA, and **View full details**. The smart query **Hardware Certified** selects files whose C2PA capture claim is hardware-signed. | [SRS-UI-001](#srs-ui-001-library-chrome) |
| IS-S-007 The inspector tabs MUST be **Metadata \| C2PA \| Archive** with Metadata selected by default; Archive contains the Source section. | [SRS-UI-002](#srs-ui-002-media-inspector) |
| IS-S-008 The ImageSquid token file MUST use the Arkrim role values: light primary `#005db6` / on-primary `#ffffff`; dark primary `#a9c7ff` / on-primary `#003063`. (Moved from FileSquid SRS-UI-007.) | FileSquid SRS-UI-007 (token contract), [NFR-006](./prd.md#nfr-006-accessibility-and-usability) |
| IS-S-009 Displayed name is ImageSquid; logo is the official mark. Both come from the ImageSquid distribution's brand assets and label table, not from code. | [PRD F-037](./prd.md#f-037-setup-updates-and-diagnostics), [PRD F-058](./prd.md#f-058-white-label-distributions) |
| IS-S-010 The inspector MUST compose the `@filesquid/ui` panels (Metadata, Content Credentials, Details and Versions) instead of reimplementing them, applying ImageSquid wording and theme through the label table and design tokens. | [F-055](./prd.md#f-055-default-ui-and-panel-composition), [SRS-UI-002](#srs-ui-002-media-inspector), FileSquid ADR 031 |

### Mobile photo sync and capture

Added 2026-10-16 from the owner's mobile PWA photo sync requirements; the owner's FR number is in brackets. These rows apply to the ImageSquid PWA on every browser and are surfaced by [SRS-UI-006](#srs-ui-006-mobile-web-companion). "Ingest" means a verified upload through `FileSquidClient.upload()` into the FileSquid import workflow (FileSquid F-002, F-043); the PWA holds no library of its own (IS-S-001). Browser storage holds only the sync record, remembered folder handles and photos waiting to upload; it is a cache (FileSquid F-036).

| Local requirement | Parent |
| --- | --- |
| IS-S-011 (FR-01) The mobile main screen MUST show a **Sync Now** control. Activating it MUST start ingestion from the remembered folder where one is granted ([IS-S-014](#mobile-photo-sync-and-capture)), and otherwise MUST open the picker ([IS-S-018](#mobile-photo-sync-and-capture)). While a sync runs, the control MUST show that state and MUST NOT start a second concurrent sync. | [F-036](./prd.md#f-036-browser-and-mobile-web-experience) |
| IS-S-012 (FR-02) On app open, the app MUST start a sync without user action only when [IS-S-020](#mobile-photo-sync-and-capture) reports a remembered folder with permission state `granted`. When permission is `prompt`, it MUST show a one-tap **Resume photo access** action and MUST NOT call a permission or picker API before a user gesture. On picker-only platforms it MUST NOT open the picker on launch. | [F-036](./prd.md#f-036-browser-and-mobile-web-experience), [IS-S-020](#mobile-photo-sync-and-capture) |
| IS-S-013 (FR-03) Photo access MUST use `window.showDirectoryPicker()` and the File System Access handles where available (expected on Android and desktop browsers) and `<input type="file" multiple accept="image/*,video/*">` where not (expected on iOS). The path MUST be selected by [IS-S-020](#mobile-photo-sync-and-capture), never by user-agent string. A directory walk MUST be bounded in entries and depth per sync and MUST report when it stops at a bound. | [F-036](./prd.md#f-036-browser-and-mobile-web-experience), [IS-S-020](#mobile-photo-sync-and-capture) |
| IS-S-014 (FR-04) Where directory access is available, the user MUST choose the photo folder once. The app MUST store the `FileSystemDirectoryHandle` in IndexedDB, MUST request persistent read access (`requestPermission({ mode: "read" })`, and `navigator.storage.persist()` so the stored handle is not evicted), and MUST reuse the handle on later syncs. When the handle is missing, unreadable or downgraded to `prompt` or `denied`, the app MUST say so and offer re-grant or re-selection; it MUST NOT silently fall back to scanning nothing and report success. | [F-036](./prd.md#f-036-browser-and-mobile-web-experience) |
| IS-S-015 (FR-05) Each sync MUST record, per device and remembered folder, the time it completed successfully (RFC 3339, UTC). A later sync of that folder MUST consider only files whose `lastModified` is after the recorded time. Every candidate MUST still pass FileSquid previously-imported detection, so a file already in the library is never ingested twice. The time MUST be recorded only after every candidate is verified or reported failed; a partial sync MUST NOT advance it. When the sync record is missing, the next sync MUST rescan the folder and skip already-imported files. Picker sessions ([IS-S-018](#mobile-photo-sync-and-capture)) MUST rely on previously-imported detection rather than `lastModified`, because picked files can carry the selection time. | [F-036](./prd.md#f-036-browser-and-mobile-web-experience), FileSquid F-002, FileSquid NFR-003 |
| IS-S-016 (FR-06) The mobile main screen MUST always show the last successful photo sync as a local date and time (for example "Last synced 16 Oct 2026, 14:05"), or "Not synced yet". The label MUST be distinct from the **Archives** backup status chip ([SRS-UI-001](#srs-ui-001-library-chrome)). | [F-036](./prd.md#f-036-browser-and-mobile-web-experience), [IS-S-015](#mobile-photo-sync-and-capture) |
| IS-S-017 (FR-07) After detection, the app MUST show the number of photos pending ingestion (for example "14 new photos found") before or as ingestion starts, then progress as "<done> of <total>", and per-photo failures with retry. Already-imported files MUST be counted separately, not as new. | [F-036](./prd.md#f-036-browser-and-mobile-web-experience) |
| IS-S-018 (FR-08) Where persistent folder access is unavailable, the sync screen MUST state that photos are selected each sync session. **Sync Now** MUST open the file input directly (one tap, multiple selection, photo and video types preset), and the app MUST skip already-imported photos without asking the user to deselect them. | [F-036](./prd.md#f-036-browser-and-mobile-web-experience), [IS-S-015](#mobile-photo-sync-and-capture) |
| IS-S-019 (FR-09) The app MUST offer **Take Photo**, using `<input type="file" accept="image/*" capture="environment">` or `getUserMedia()` where [IS-S-020](#mobile-photo-sync-and-capture) reports camera capture. A captured photo MUST be ingested into the current library without a further confirmation step. Until its upload is verified it MUST be kept in a bounded IndexedDB queue, shown as "Not yet in library", and retried when the app next runs online; it MUST NOT be shown as protected. The app MUST NOT claim to save the photo to the device camera roll. | [F-036](./prd.md#f-036-browser-and-mobile-web-experience), FileSquid F-002, FileSquid F-043 |
| IS-S-020 (FR-10) At startup and before each sync, the app MUST detect: directory access (`showDirectoryPicker` present), handle permission state (`queryPermission`), multi-file picker, camera capture and IndexedDB availability. The result MUST be one capability record that selects the sync controls and path; it MUST be re-evaluated when the permission state changes. | [F-036](./prd.md#f-036-browser-and-mobile-web-experience) |
| IS-S-021 (FR-11) When directory access is unavailable, the app MUST use the picker path of [IS-S-018](#mobile-photo-sync-and-capture) and MUST NOT show folder selection, folder re-grant or sync-on-launch controls. When IndexedDB is unavailable, it MUST state that the sync record and remembered folder cannot be kept. When camera capture is unavailable, **Take Photo** MUST be absent, not disabled without explanation. | [F-036](./prd.md#f-036-browser-and-mobile-web-experience), [IS-S-020](#mobile-photo-sync-and-capture) |

### Inherited from `@filesquid/ui`

UI-S-011, UI-S-012 and UI-S-013 were copied on 2026-10-16; UI-S-016 was copied on 2026-10-18.

| Requirement | Parent |
| --- | --- |
| UI-S-011 The UI is a client-rendered vanilla TypeScript application built with Vite, using standard custom elements and no UI framework. Private data MUST NOT be server-rendered. | [PRD F-036](./prd.md#f-036-browser-and-mobile-web-experience), FileSquid U-29, FileSquid ADR 031 |
| UI-S-012 The UI reaches FileSquid only through `FileSquidClient` (`request()`, `read()`, `upload()`, `subscribe()`), which selects the HTTP-plus-event-channel adapter on `filesquid-server`, the custom-scheme adapter on `filesquid-desktop`, and later the browser-host adapter. | [PRD F-037](./prd.md#f-037-setup-updates-and-diagnostics), FileSquid F-056, FileSquid ADR 030 |
| UI-S-013 UI tests run against production artifacts (`vite build` plus `vite preview`) with strict DOM assertions: custom elements defined, labels from the active label table, computed token values in light and dark, no console errors. | AGENTS.md UI Test Mandate, FileSquid ADR 017 |
| UI-S-016 Every `@filesquid/ui` custom element, and so every distribution including ImageSquid, MUST meet these accessibility contracts. **Shared rules:** every component MUST put the shared accessibility stylesheet first in its shadow root, and the shell MUST add one document-level copy of it so light-DOM content slotted into a component gets the same rules. **Keyboard:** every action MUST be reachable through native interactive elements (`a[href]`, `button`, `input`) and no element MAY have a positive `tabindex`. **Bypass:** the shell's first focusable element MUST be a skip control worded by the `skipToContent` label, visually hidden until focused, that moves focus to `main` (`tabindex="-1"`) without changing the URL hash. **Names:** the shell MUST expose `header`, a `nav` named by the `navigationMain` label and `main`, with the product name as its only `h1`; every interactive element and image MUST have a non-empty accessible name from the label table or the distribution, and each Monitor table MUST be named by its section heading inside a named, focusable scroll region. **Focus:** every element in the Tab order MUST show a `2px` solid `--fs-primary` outline offset by `2px` on `:focus-visible`, and `--fs-primary` MUST reach 3:1 against `--fs-surface`, `--fs-surface-container` and `--fs-primary-container` in both themes; when a re-render or route change removes the focused element, focus MUST move to a defined target (route change: `main`; Monitor token form: the token field; Monitor alert: the alert; Monitor loaded: its first section heading); focus MUST NOT be moved on first load, and a background load, poll or failed poll MUST NOT take focus from outside the content it replaces. **Motion:** the theme-independent token `--fs-motion-duration` (FileSquid `150ms`) MUST supply every transition and animation duration, and under `prefers-reduced-motion: reduce` every component's computed transition and animation duration MUST be `0s`. **Contrast preferences:** under `prefers-contrast: more` component borders MUST use `--fs-on-surface-variant`; under `forced-colors: active` the chosen theme option MUST use `Highlight` and `HighlightText` and focus outlines `Highlight`; system colours MAY appear only inside a `forced-colors` block. **Text scaling:** type sizes MUST be `rem` tokens and no text container MAY have a fixed pixel height; at a 320 CSS px viewport, at 200 % root font size, and at both together, the page MUST NOT scroll horizontally, and below `40rem` the navigation MUST stack above the main area. **Locale:** a distribution MAY declare `locale` (`lang`, a well-formed BCP 47 tag that is trimmed and written in canonical form; `dir` `ltr` or `rtl`; default `en`, `ltr`), applied to the root element's `lang` and `dir`, and a malformed tag or direction MUST be refused; component stylesheets MUST use logical properties only, so `dir="rtl"` mirrors the layout. The ImageSquid label table MUST supply `skipToContent` and `navigationMain`, and its token file `--fs-motion-duration`. | [PRD NFR-006](./prd.md#nfr-006-accessibility-and-usability), [PRD RC-005](./prd.md#rc-005-functional-and-accessible-ui-quality), FileSquid UI-S-016 |

## PRD coverage

Every requirement in the [ImageSquid PRD](./prd.md) is implemented by the ImageSquid elements below. The FileSquid SRS item named in plain text implements the capability behind the FileSquid API. Where an ImageSquid-owned requirement has no dedicated surface yet, its views follow the PRD's acceptance criteria and compose FileSquid custom elements (IS-S-001, IS-S-010).

| PRD item | Implemented by |
| --- | --- |
| [F-003](./prd.md#f-003-gallery-selection-and-viewing) Gallery, selection and viewing | [SRS-UI-001](#srs-ui-001-library-chrome) (grid, toolbar, selection), [SRS-UI-002](#srs-ui-002-media-inspector) (viewer, filmstrip), [IS-S-005](#distribution-requirements); FileSquid SRS-F-003 (capability through the FileSquid API) |
| [F-004](./prd.md#f-004-albums-folders-and-navigation) Albums, folders and navigation | [SRS-UI-001](#srs-ui-001-library-chrome) (Albums and Exhibits sidebar groups); FileSquid SRS-F-004 (capability through the FileSquid API) |
| [F-005](./prd.md#f-005-favorites-metadata-and-keywords) Favorites, metadata and keywords | [SRS-UI-001](#srs-ui-001-library-chrome) (favorites), [SRS-UI-002](#srs-ui-002-media-inspector) (Metadata tab); FileSquid SRS-F-005 (capability through the FileSquid API) |
| [F-006](./prd.md#f-006-smart-galleries-and-metadata-search) Smart galleries and metadata search | [SRS-UI-001](#srs-ui-001-library-chrome) (Smart Queries, search), [IS-S-006](#distribution-requirements) (Hardware Certified query); FileSquid SRS-F-006 (capability through the FileSquid API) |
| [F-007](./prd.md#f-007-automatic-and-activity-collections) Automatic and activity collections | [SRS-UI-001](#srs-ui-001-library-chrome) (Library and Collections sidebar groups); remaining media-type, activity and classification views per this PRD's acceptance criteria; no dedicated surface yet; FileSquid SRS-F-007 (capability through the FileSquid API) |
| [F-008](./prd.md#f-008-deletion-hidden-media-and-duplicate-review) Deletion, hidden media and duplicate review | No dedicated surface yet; views follow FileSquid's acceptance criteria; FileSquid SRS-F-008 (capability through the FileSquid API) |
| [F-010](./prd.md#f-010-export-and-portable-backup) Export and portable backup | [SRS-UI-004](#srs-ui-004-preservation-and-migration), [SRS-UI-002](#srs-ui-002-media-inspector) (export from the viewer); FileSquid SRS-F-010 (capability through the FileSquid API) |
| [F-011](./prd.md#f-011-non-destructive-editing-workflow) Non-destructive editing workflow | [SRS-UI-003](#srs-ui-003-non-destructive-studio); FileSquid SRS-F-011 (capability through the FileSquid API) |
| [F-012](./prd.md#f-012-composition-and-tonalcolor-editing) Composition and tonal/color editing | [SRS-UI-003](#srs-ui-003-non-destructive-studio); FileSquid SRS-F-012 (capability through the FileSquid API) |
| [F-013](./prd.md#f-013-detail-filters-retouch-and-raw) Detail, filters, retouch and RAW | [SRS-UI-003](#srs-ui-003-non-destructive-studio); FileSquid SRS-F-013 (capability through the FileSquid API) |
| [F-014](./prd.md#f-014-ai-assisted-cleanup-and-styles) AI-assisted cleanup and styles | [SRS-UI-003](#srs-ui-003-non-destructive-studio); FileSquid SRS-F-014 (capability through the FileSquid API) |
| [F-015](./prd.md#f-015-portrait-depth-cinematic-and-spatial-media) Portrait, depth, Cinematic and spatial media | [SRS-UI-003](#srs-ui-003-non-destructive-studio); FileSquid SRS-F-015 (capability through the FileSquid API) |
| [F-016](./prd.md#f-016-live-photos-bursts-and-motion-images) Live Photos, bursts and motion images | [SRS-UI-003](#srs-ui-003-non-destructive-studio); FileSquid SRS-F-016 (capability through the FileSquid API) |
| [F-017](./prd.md#f-017-video-and-audio-editing) Video and audio editing | [SRS-UI-003](#srs-ui-003-non-destructive-studio); FileSquid SRS-F-017 (capability through the FileSquid API) |
| [F-018](./prd.md#f-018-markup-and-companion-annotation) Markup and companion annotation | [SRS-UI-003](#srs-ui-003-non-destructive-studio); FileSquid SRS-F-018 (capability through the FileSquid API) |
| [F-019](./prd.md#f-019-people-pets-and-groups) People, pets and groups | Views per this PRD's acceptance criteria; no dedicated surface yet; FileSquid SRS-F-019 (capability through the FileSquid API) |
| [F-020](./prd.md#f-020-semantic-search-ocr-and-visual-understanding) Semantic search, OCR and visual understanding | [SRS-UI-001](#srs-ui-001-library-chrome) (search field); results, OCR and visual-understanding views per this PRD's acceptance criteria; no dedicated surface yet; FileSquid SRS-F-020 (capability through the FileSquid API) |
| [F-021](./prd.md#f-021-places-trips-and-curation-controls) Places, Trips and curation controls | [SRS-UI-001](#srs-ui-001-library-chrome) (Places in Collections); Trips and curation views per this PRD's acceptance criteria; no dedicated surface yet; FileSquid SRS-F-021 (capability through the FileSquid API) |
| [F-022](./prd.md#f-022-memories-and-storytelling) Memories and storytelling | Views per this PRD's acceptance criteria; no dedicated surface yet; FileSquid SRS-F-022 (capability through the FileSquid API) |
| [F-023](./prd.md#f-023-slideshows-printing-and-photo-projects) Slideshows, printing and photo projects | Views per this PRD's acceptance criteria; no dedicated surface yet; FileSquid SRS-F-023 (capability through the FileSquid API) |
| [F-024](./prd.md#f-024-system-sharing-and-extension-interfaces) System sharing and extension interfaces | [SRS-UI-001](#srs-ui-001-library-chrome), [SRS-UI-002](#srs-ui-002-media-inspector) (share controls); FileSquid SRS-F-024 (capability through the FileSquid API) |
| [F-025](./prd.md#f-025-migration-selection-inventory-and-pilot) Migration selection, inventory and pilot | [SRS-UI-004](#srs-ui-004-preservation-and-migration); FileSquid SRS-F-025 (capability through the FileSquid API) |
| [F-027](./prd.md#f-027-migration-execution-recovery-and-reconciliation) Migration execution, recovery and reconciliation | [SRS-UI-004](#srs-ui-004-preservation-and-migration); FileSquid SRS-F-027 (capability through the FileSquid API) |
| [F-028](./prd.md#f-028-supported-migration-routes) Supported migration routes | [SRS-UI-004](#srs-ui-004-preservation-and-migration), [SRS-UI-008](#srs-ui-008-unmatched-v2-controls) (migration reachable from overflow); route-specific views per this PRD's acceptance criteria; no dedicated surface yet; FileSquid SRS-F-028 (capability through the FileSquid API) |
| [F-033](./prd.md#f-033-shared-albums-and-hosted-galleries) Shared albums and hosted galleries | No dedicated surface yet; views follow FileSquid's acceptance criteria; FileSquid SRS-F-033 (capability through the FileSquid API) |
| [F-034](./prd.md#f-034-family-accounts-and-shared-libraries) Family accounts and shared libraries | [SRS-UI-001](#srs-ui-001-library-chrome) (current-library switcher); FileSquid SRS-F-034 (capability through the FileSquid API) |
| [F-036](./prd.md#f-036-browser-and-mobile-web-experience) Browser and mobile-web experience | [SRS-UI-006](#srs-ui-006-mobile-web-companion), [IS-S-011](#mobile-photo-sync-and-capture)–[IS-S-021](#mobile-photo-sync-and-capture), [UI-S-011](#inherited-from-filesquidui); FileSquid SRS-F-036 (capability through the FileSquid API) |
| [F-037](./prd.md#f-037-setup-updates-and-diagnostics) Setup, updates and diagnostics | [SRS-UI-008](#srs-ui-008-unmatched-v2-controls) (task monitor), [IS-S-009](#distribution-requirements), [UI-S-012](#inherited-from-filesquidui); FileSquid SRS-F-037 (capability through the FileSquid API) |
| [F-039](./prd.md#f-039-consent-notifications-and-activity-privacy) Consent, notifications and activity privacy | No dedicated surface yet; views follow FileSquid's acceptance criteria; FileSquid SRS-F-039 (capability through the FileSquid API) |
| [F-040](./prd.md#f-040-compatibility-and-parity-register) Compatibility and parity register | No dedicated surface yet; views follow FileSquid's acceptance criteria; FileSquid SRS-F-040 (capability through the FileSquid API) |
| [F-041](./prd.md#f-041-source-aware-web-clipping-and-http-evidence) Source-aware web clipping and HTTP evidence | [SRS-UI-002](#srs-ui-002-media-inspector) (Archive tab, Source section); FileSquid SRS-F-041 (capability through the FileSquid API) |
| [F-042](./prd.md#f-042-ocfl-preservation-and-portable-recovery) OCFL preservation and portable recovery | [SRS-UI-004](#srs-ui-004-preservation-and-migration), [SRS-UI-002](#srs-ui-002-media-inspector) (Archive tab); FileSquid SRS-F-042 (capability through the FileSquid API) |
| [F-043](./prd.md#f-043-import-time-c2pa-records-and-action-disclosure) Import-time C2PA records and action disclosure | [SRS-UI-002](#srs-ui-002-media-inspector) (C2PA tab), [SRS-UI-001](#srs-ui-001-library-chrome) (C2PA floatover), [IS-S-006](#distribution-requirements); FileSquid SRS-F-043 (capability through the FileSquid API) |
| [F-044](./prd.md#f-044-paid-c2pa-publishing-and-watermarking) Paid C2PA publishing and watermarking | [SRS-UI-005](#srs-ui-005-paid-c2pa-publication); FileSquid SRS-F-044 (capability through the FileSquid API) |
| [F-045](./prd.md#f-045-byo-ai-connections-and-per-task-model-choice) BYO AI connections and per-task model choice | No dedicated surface yet; views follow FileSquid's acceptance criteria; FileSquid SRS-F-045 (capability through the FileSquid API) |
| [F-046](./prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports) Media metadata inspection, correction and privacy exports | [SRS-UI-002](#srs-ui-002-media-inspector) (Metadata tab); FileSquid SRS-F-046 (capability through the FileSquid API) |
| [F-047](./prd.md#f-047-institutional-dam-and-iiif-interoperability) Institutional DAM and IIIF interoperability | No dedicated surface yet; views follow FileSquid's acceptance criteria; FileSquid SRS-F-047 (capability through the FileSquid API) |
| [F-049](./prd.md#f-049-product-component-terminology-and-item-type) Product component terminology and item type | [SRS-UI-001](#srs-ui-001-library-chrome), [IS-S-004](#distribution-requirements), [Terminology](#terminology); FileSquid SRS-F-049 (capability through the FileSquid API) |
| [F-050](./prd.md#f-050-filesquid-platform-and-distributions) FileSquid platform and distributions | [IS-S-001](#distribution-requirements); FileSquid SRS-F-050 (capability through the FileSquid API) |
| [F-055](./prd.md#f-055-default-ui-and-panel-composition) Default UI and panel composition | [SRS-UI-002](#srs-ui-002-media-inspector), [IS-S-010](#distribution-requirements); FileSquid SRS-F-055 (capability through the FileSquid API) |
| [F-058](./prd.md#f-058-white-label-distributions) White-label distributions | [IS-S-001](#distribution-requirements), [IS-S-002](#distribution-requirements), [IS-S-008](#distribution-requirements), [IS-S-009](#distribution-requirements), [SRS-UI-001](#srs-ui-001-library-chrome); FileSquid SRS-F-058 (capability through the FileSquid API) |
| [NFR-002](./prd.md#nfr-002-benchmark-envelope-and-responsiveness) Benchmark envelope and responsiveness | [SRS-UI-001](#srs-ui-001-library-chrome), [SRS-UI-002](#srs-ui-002-media-inspector) (grid and viewer responsiveness); FileSquid SRS-NFR-002 (capability through the FileSquid API) |
| [NFR-003](./prd.md#nfr-003-bounded-processing-and-migration-endurance) Bounded processing and migration endurance | [SRS-UI-004](#srs-ui-004-preservation-and-migration) (checkpoint, resume and progress); FileSquid SRS-NFR-003 (capability through the FileSquid API) |
| [NFR-006](./prd.md#nfr-006-accessibility-and-usability) Accessibility and usability | [IS-S-008](#distribution-requirements), [UI-S-013](#inherited-from-filesquidui), [UI-S-016](#inherited-from-filesquidui), every surface; FileSquid SRS-NFR-006 (capability through the FileSquid API) |
| [TC-003](./prd.md#tc-003-platform-and-provider-limits) Platform and provider limits | [SRS-UI-004](#srs-ui-004-preservation-and-migration), [SRS-UI-006](#srs-ui-006-mobile-web-companion) (honest capability limits); FileSquid SRS-TC-003 (capability through the FileSquid API) |
| [RC-002](./prd.md#rc-002-performance-and-resource-bounds) Performance and resource bounds | Verification still required across every ImageSquid view against this PRD's acceptance criteria; no measured result is asserted here; no dedicated surface yet; FileSquid SRS-NFR-002, FileSquid SRS-NFR-003 (capability through the FileSquid API) |
| [RC-005](./prd.md#rc-005-functional-and-accessible-ui-quality) Functional and accessible UI quality | [UI-S-013](#inherited-from-filesquidui), [UI-S-016](#inherited-from-filesquidui), every surface; verification against this PRD's acceptance criteria still required; FileSquid SRS-NFR-006 (capability through the FileSquid API) |
| [RC-009](./prd.md#rc-009-source-evidence-and-c2pa-lifecycle) Source evidence and C2PA lifecycle | [SRS-UI-002](#srs-ui-002-media-inspector) (C2PA tab, Archive Source section); scan and report views per this PRD's acceptance criteria; no dedicated surface yet; FileSquid SRS-F-041, FileSquid SRS-F-043 (capability through the FileSquid API) |
| [RC-011](./prd.md#rc-011-paid-third-party-publishing-and-watermark-delivery) Paid third-party publishing and watermark delivery | [SRS-UI-005](#srs-ui-005-paid-c2pa-publication); FileSquid SRS-F-044 (capability through the FileSquid API) |

## UI surfaces

These surfaces belong to the **ImageSquid distribution**. ImageSquid labels apply. Each surface MUST be built from ImageSquid view modules that compose FileSquid custom elements and call only the FileSquid API. The FileSquid default UI is specified in FileSquid SRS-F-055.

### SRS-UI-001: Library chrome

The ImageSquid desktop library surface MUST show the following regions and controls ([screen1](./ui-mockups/screen1-light-mode.png)).

**Layout.** A persistent left sidebar region and a main content region. A sidebar collapse control MUST sit at the top of the sidebar. Layout MUST NOT require pixel coordinates from the screenshot.

**Top bar, left to right.** The official ImageSquid logo and the product name **ImageSquid**; a current-library switcher showing the library name and a disclosure caret; a search field with a search icon; then, right-aligned, the **Archives** sync status chip (a status dot plus status text, for example "Archives Synced"), the **CR** Content Credentials chip, a theme control, and the profile avatar. The search placeholder MUST read “Search photos, collections, dates…” (ImageSquid label table).

**Sidebar groups, in this order, with uppercase group headings.**
1. **Library**: All Photos, Days, Months, Years, Recently Saved.
2. **Collections**: the automatic galleries Favorites, Places, Videos, Screenshots, Selfies. The heading text **Collections** comes from the ImageSquid label table.
3. **Albums**: the owner's albums, one row per album, each with a folder icon.
4. **Exhibits**: the owner's exhibits.
5. **Smart Queries**: saved provenance queries, including **AI Synthetic**, **Hardware Certified** (files whose C2PA capture claim is hardware-signed; the label is not a certification claim, FileSquid C-018) and **Unsigned**.

The current destination MUST be highlighted with a raised surface and primary-coloured text and icon. The sidebar footer MUST show Archives storage use as "<used> of <quota>" over the word **Archives**, with an info control.

**Toolbar row.** A segmented control **Years | Months | Days | All Photos** with All Photos selected by default; a zoom control (minus, slider, plus); then, right-aligned, **Filter** (with a dot when a filter is active), share, an inspector (info) toggle, and **Select**. Select MUST use the primary colour role on a tinted surface.

**Content.** A period heading (for example "October 2024") followed by photo and video counts in a monospace face (for example "14,290 Photos • 760 Videos"). Below it, a grid of uniform tiles in rows. A selected or focused tile MUST show a primary-coloured outline.

**Provenance skim.** Provenance skim MUST be **on by default**: the default All Photos grid shows an origin chip on every tile, as in screen1. The CR chip MUST toggle skim; when the owner turns it off, tiles MUST omit origin chips. When skim is on, each tile MUST show an origin chip at its bottom-right corner: a pill with a coloured dot and the label **AI** or **RW** (Real World) ([origin-chip-AI.svg](./assets/origin-chip-AI.svg), [origin-chip-RW.svg](./assets/origin-chip-RW.svg)).

**C2PA floatover.** When skim is on and a tile receives hover or keyboard focus, a floatover card MUST appear beside the tile. It MUST show: the file thumbnail; a status dot and the title "C2PA Provenance"; an origin badge reading **Real** or **AI** (long forms of the RW / AI chips); a confidence and verification line with an info control; rows **Captured**, **Camera**, **Signed by**, and **C2PA** (validation result and specification version); and a full-width **View full details** button that opens the inspector C2PA tab ([SRS-UI-002](#srs-ui-002-media-inspector)). Unsigned, signed, invalid and unavailable-check states MUST be distinct in the card.

**States and roles.** Empty, loading, error, offline, and permission-denied states MUST be distinct and labeled. Primary actions MUST use the primary/on-primary colour roles. A More control (or the profile menu) MUST reach the unmatched destinations in [SRS-UI-008](#srs-ui-008-unmatched-v2-controls) (local vault, archive storage root, migration, C2PA publish, task monitor); the screenshot does not show it. Primary chrome MUST NOT use OCFL as the preservation-store label.

**Source**
- [ImageSquid library screenshot (screen1)](./ui-mockups/screen1-light-mode.png)
- [UI mockups index](./ui-mockups/README.md): `library_photos_v4`, `library_provenance_gallery_v4`, `unmatched-v2.md` (mockup retired by owner 2026-10-15; requirement retained)
- [F-003](./prd.md#f-003-gallery-selection-and-viewing), [F-004](./prd.md#f-004-albums-folders-and-navigation), [F-049](./prd.md#f-049-product-component-terminology-and-item-type), [F-058](./prd.md#f-058-white-label-distributions)
- [ImageSquid U-05](./product-vision.md#user-requirements), [ImageSquid terminology](./product-vision.md#terminology)

**Tests**
- [`library_chrome_SRS-UI-001.feature`](../tests/features/library_chrome_SRS-UI-001.feature)

### SRS-UI-002: Media inspector

The ImageSquid media detail surface MUST show a viewer and an inspector ([screen2](./ui-mockups/screen2-light-mode.png)). The top bar and sidebar MUST match [SRS-UI-001](#srs-ui-001-library-chrome), with the containing album or gallery highlighted in the sidebar.

**Viewer header, left to right.** A back button labelled with the containing album or gallery name; the file name with a C2PA badge showing the manifest's specification version (for example "C2PA v2.1"); below the file name, the capture date and time in a monospace face; a centred tool group **Fit | 100% | Compare** plus a version-history control; then, right-aligned, favorite (heart), share, **Info** (inspector toggle, tinted when open), and **Edit**. Edit MUST use the primary colour role and MUST open the studio ([SRS-UI-003](#srs-ui-003-non-destructive-studio)).

**Canvas.** The file MUST be centred on a neutral surface. Overlays: at top-left, a manifest chip with a dot and the manifest class (for example "C2PA Synthetic Manifest"); at top-right, the CR Content Credentials pin; at bottom-right, pixel dimensions and the representation shown (for example "1024 × 1024 px • Raw Asset"). The representation label MUST distinguish the original from a derivative preview.

**Filmstrip.** A filmstrip MUST sit centred below the canvas. It MUST show neighbouring files of the current album or gallery, highlight the current file, and show the position as "<n> of <total>" (for example "2 of 12").

**Inspector.** The inspector MUST occupy the right-hand column. It MUST have the tabs **Metadata | C2PA | Archive**, with Metadata selected by default, and a close control.
- **Metadata tab.** A **Properties** card MUST list populated technical (`exif:`) and descriptive (`dcterms:`, `schema:`) metadata as Obsidian-style property rows (CSS grid, light and dark). Each row MUST show an icon, the prefixed key, and the value. A key or value longer than its column MUST truncate with an ellipsis. Rows MUST group `dcterms:` first, then `exif:`, then `schema:`. The card MUST end with **Add property**. Empty `/terms/` properties MUST remain hidden until the owner adds a field. Adding a descriptive field MUST search ISO 15836-2 `/terms/` labels first and, when no match exists, suggest the closest Schema.org term. Every displayed pair MUST include its prefix. Below the card, the inspector MUST show the YAML front matter of the file's canonical `metadata.md`, read-only, in a monospace face, delimited by `---` lines and grouped by namespace map (`dcterms:`, `exif:`, `schema:`). Canonical storage is Markdown with YAML front matter in OCFL (`metadata.md`).
- **C2PA tab.** It MUST show the C2PA/provenance overview, cryptographic credentials, and inspection assertions. Unsigned, signed, invalid, and unavailable-check states MUST remain distinct. The surface MUST NOT present C2PA as EU AI Act certification.
- **Archive tab.** It MUST show the file's storage facts and version history. It MAY show OCFL 1.1 as secondary power-user copy. It MUST contain a **Source** section that presents F-041 source URLs and locked HTTP-evidence access.

**Composition.** The inspector MUST be composed from FileSquid custom elements rather than reimplemented: the Metadata tab from the Metadata panel (`<fs-metadata-panel>`), the C2PA tab from the Content Credentials panel (`<fs-credentials-panel>`), and the Archive tab from the Details and Versions panels. The Source section remains an ImageSquid view module; its data MUST come from FileSquid's web-clipping module through the FileSquid API. ImageSquid supplies its wording through the label table and its theme through design tokens (FileSquid SRS-F-055, FileSquid SRS-UI-007). ImageSquid UI copy MUST use repo, library, album, gallery and exhibit, with Archives as the preservation-store label.

**Source**
- [ImageSquid viewer and inspector screenshot (screen2)](./ui-mockups/screen2-light-mode.png)
- [UI mockups index](./ui-mockups/README.md): v4 inspector light and dark, `media_detail_c2pa_inspector_v3_minimal` (mockup retired by owner 2026-10-15; requirement retained)
- [F-041](./prd.md#f-041-source-aware-web-clipping-and-http-evidence), [F-043](./prd.md#f-043-import-time-c2pa-records-and-action-disclosure), [F-046](./prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports), [F-055](./prd.md#f-055-default-ui-and-panel-composition)
- FileSquid ADR 022, FileSquid ADR 028, FileSquid ADR 031

**Tests**
- [`media_inspector_SRS-UI-002.feature`](../tests/features/media_inspector_SRS-UI-002.feature)

### SRS-UI-003: Non-destructive studio

The ImageSquid studio surface MUST keep originals unchanged, show non-destructive parameters, AI tools as explicit actions, and a commit path that records retained history. Assistive standard edits MUST NOT be labeled AI-generated. The studio MUST open from the viewer's **Edit** control ([screen2](./ui-mockups/screen2-light-mode.png)). Editing recipes and derivatives MUST be a FileSquid module reached through the FileSquid API (FileSquid SRS-F-058).

**Source**
- [UI mockups index](./ui-mockups/README.md): `non_destructive_creativity_studio_v3_minimal` (mockup retired by owner 2026-10-15; requirement retained)
- [ImageSquid viewer screenshot (screen2), Edit entry point](./ui-mockups/screen2-light-mode.png)
- [F-011](./prd.md#f-011-non-destructive-editing-workflow), [F-014](./prd.md#f-014-ai-assisted-cleanup-and-styles)

**Tests**
- [`studio_SRS-UI-003.feature`](../tests/features/studio_SRS-UI-003.feature)

### SRS-UI-004: Preservation and migration

The ImageSquid preservation surface MUST show job progress, checkpoint/resume, incoming buffer, storage allocation, and OCFL snapshot history. Processing completion, media verification, and preservation protection MUST remain separate states. The Archives storage-use footer of [SRS-UI-001](#srs-ui-001-library-chrome) MUST open this surface.

**Source**
- [UI mockups index](./ui-mockups/README.md): `preservation_migration_manager_v3_minimal` (mockup retired by owner 2026-10-15; requirement retained)
- [ImageSquid library screenshot (screen1), Archives footer](./ui-mockups/screen1-light-mode.png)
- [F-025](./prd.md#f-025-migration-selection-inventory-and-pilot), [F-042](./prd.md#f-042-ocfl-preservation-and-portable-recovery)

**Tests**
- [`preservation_SRS-UI-004.feature`](../tests/features/preservation_SRS-UI-004.feature)

### SRS-UI-005: Paid C2PA publication

The ImageSquid publication sheet MUST show scrubbing policy, human-perceivable EU-icon disclosure options, selected-publisher timestamp, price, and Sign & Publish. A C2PA manifest or watermark alone MUST NOT satisfy the visible disclosure. Originals MUST remain unchanged.

**Source**
- [UI mockups index](./ui-mockups/README.md): `paid_c2pa_publication_v3_minimal` (mockup retired by owner 2026-10-15; requirement retained)
- [F-044](./prd.md#f-044-paid-c2pa-publishing-and-watermarking)

**Tests**
- [`publication_SRS-UI-005.feature`](../tests/features/publication_SRS-UI-005.feature)

### SRS-UI-006: Mobile web companion

The ImageSquid mobile-web surface MUST provide library browse, albums, search, and capture/library entry with honest capability limits. It MUST NOT promise continuous background camera backup after browser closure.

**Main screen.** The installed PWA's main screen on a phone MUST show the **Sync Now** control ([IS-S-011](#mobile-photo-sync-and-capture)), the last-synced line ([IS-S-016](#mobile-photo-sync-and-capture)), **Take Photo** where supported ([IS-S-019](#mobile-photo-sync-and-capture)), and, during a sync, the pending count and progress ([IS-S-017](#mobile-photo-sync-and-capture)). Which sync controls appear is set by the capability record ([IS-S-020](#mobile-photo-sync-and-capture), [IS-S-021](#mobile-photo-sync-and-capture)). Sync runs only while the app is open; an interrupted sync resumes from verified photos on the next run (FileSquid C-001).

**Source**
- [UI mockups index](./ui-mockups/README.md): `mobile_web_companion_v3_minimal` (mockup retired by owner 2026-10-15; requirement retained)
- [F-036](./prd.md#f-036-browser-and-mobile-web-experience)
- [Mobile photo sync and capture](#mobile-photo-sync-and-capture) (IS-S-011–IS-S-021)

**Tests**
- [`mobile_companion_SRS-UI-006.feature`](../tests/features/mobile_companion_SRS-UI-006.feature)

### SRS-UI-008: Unmatched v2 controls

Controls listed in `unmatched-v2.md` that the current ImageSquid screenshots do not place on the primary surface MUST remain reachable from More, overflow, or profile menus.

**Source**
- [UI mockups index](./ui-mockups/README.md): `unmatched-v2.md`, v2 library light and dark (mockup retired by owner 2026-10-15; requirement retained)
- [ImageSquid library screenshot (screen1)](./ui-mockups/screen1-light-mode.png)

**Tests**
- [`overflow_v2_SRS-UI-008.feature`](../tests/features/overflow_v2_SRS-UI-008.feature)

## Mockup coverage

The owner retired the v1–v4 ImageSquid mockup suites, the v3.1 Stitch unpacks and the v2 `unmatched-v2.md` inventory on 2026-10-15. Every requirement that cited them is retained and cites the [UI mockups index](./ui-mockups/README.md) with the note "(mockup retired by owner 2026-10-15; requirement retained)". The retired Apple Photos captures were migration-source references, not ImageSquid chrome.

| Source | Status | SRS citation |
|---|---|---|
| [screen1-light-mode.png](./ui-mockups/screen1-light-mode.png) (library grid, C2PA floatover) | Active | SRS-UI-001, SRS-UI-004 (Archives footer), FileSquid SRS-UI-007 (light theme, theme control), SRS-UI-008; library, grid and gallery F-blocks |
| [screen2-light-mode.png](./ui-mockups/screen2-light-mode.png) (viewer, Metadata / C2PA / Archive inspector) | Active | SRS-UI-002, SRS-UI-003 (Edit entry); media detail, inspector and viewer F-blocks |
| Dark-mode screenshots | Not supplied | FileSquid SRS-UI-007 requires dark tokens; no dark screenshot binds composition |
| v4 light and dark suite | Retired | SRS-UI-001, SRS-UI-002, FileSquid SRS-UI-007 |
| v3 six light + six dark screens, v3.1 and v3.1-dark Stitch | Retired | SRS-UI-001–006 |
| v3 `DESIGN.md` light and dark | Retired | FileSquid SRS-UI-007 |
| v2 light and dark suites, `unmatched-v2.md` | Retired | SRS-UI-008 |
| v1 suites | Retired | Covered by SRS-UI-001–006 |
| `apple_photo_11-*` references | Parity reference | FileSquid SRS-F-040 |
