# Software Requirements Specification: FileSquid

Status: Draft. This SRS specifies **FileSquid**, the platform this repository delivers (one Rust backend, one FileSquid API, the `ocfl`, `metadata` and `c2pa` modules, desktop and multi-user server hosts, and paid ciphertext-only backup and sync at `*.filesquid.tech`), and its white-label UI **distributions**: the FileSquid default UI, **ImageSquid** (photo albums) and, later, **Arkrim** (digital archives). It derives from [prd.md](./prd.md), [architecture.md](./architecture.md#1-introduction), [ADRs 001–032](./architecture) and the [FileSquid vision](./product-vision.md). Scaffolded crates and packages do not yet implement these requirements.

Requirements use RFC 2119. This SRS specifies FileSquid and the contracts every distribution implements. A distribution's own UI surfaces and screenshots are specified in that distribution's SRS.

## Terminology

System, data, storage, API, and traceability text MUST use the internal names **repository**, **library**, **collection**, **virtual collection**, **virtual exhibit**, and **file**. The item type is **file**. A collection belongs to exactly one library. A virtual collection belongs to exactly one library. A virtual exhibit belongs to a user on a repository and MAY span public collections across libraries and repositories.

Each distribution supplies a label table keyed by the internal names ([SRS-F-058](#srs-f-058-white-label-distributions)):

| Internal name | FileSquid default UI | ImageSquid UI |
|---|---|---|
| repository | repository | repo |
| library | library | library |
| collection | collection | album |
| virtual collection | virtual collection | gallery |
| virtual exhibit | virtual exhibit | exhibit |
| file | file | photo, image or video (media kind) |

The FileSquid default UI MUST use the internal names and MUST NOT inherit the ImageSquid labels. ImageSquid UI MUST use **repo**, **library**, **album**, **gallery**, and **exhibit**, and MAY label a given file photo, image, or video. ImageSquid primary chrome MUST use **Archives** for the preservation store. When an ImageSquid collection skim shows origin labels, those labels MUST be **AI** or **RW** (Real World). The Content Credentials pin MAY remain **CR**.

## Current verified stack (scaffold)

- **FileSquid Cargo workspace** at `apps/filesquid/` (product-local; the repository root has no `Cargo.toml`). Rust 1.99.1, an exact stable release pinned in `rust-toolchain.toml` (`channel = "1.99.1"`; bumped only in its own `build/` pull request, [ADR 013](./architecture/013-cargo-svelte-workspaces.md)), edition 2021, MIT license. Crates under `apps/filesquid/crates/`:
  - `filesquid-core` (`crates/core`): shared vocabulary and contracts (opaque IDs, repository, library, collection, virtual collection, virtual exhibit, file, jobs, crypto envelopes).
  - `filesquid-ocfl` (`crates/ocfl`): the OCFL module.
  - `filesquid-metadata` (`crates/metadata`): the metadata module.
  - `filesquid-prov` (`crates/prov`): the PROV-O history module. Planned; documents only ([ADR 040](./architecture/040-prov-o-history.md)).
  - `filesquid-api` (`crates/api`): the single FileSquid API (routes, contract, auth, events). Being added on this branch.
  - `filesquid-server` (`crates/server`): Axum 0.8 host. Bind address from `FILESQUID_BIND`, default `127.0.0.1:8083`. Serves a UI directory from `FILESQUID_UI_DIR`.
  - `filesquid-desktop` (`crates/desktop`): Tauri 2.11.5 host. The API is served in-process through a custom URI scheme. No TCP port is opened.
  - `filesquid-c2pa` (`crates/c2pa`): the C2PA module. Planned; documents and Gherkin only.
- **UIs:** vanilla TypeScript (no UI framework) compiled with Vite 8, using standard custom elements:
  - `@filesquid/ui` at `apps/filesquid/ui`: FileSquid API client, custom elements, default distribution UI. Dev port `5173`, preview port `4173`.
  - `@imagesquid/ui` at `apps/imagesquid`: the ImageSquid distribution (views, theme, labels, brand, distribution manifest). No Rust. Dev port `5174`, preview port `4174`.
- **Build tooling decided, not yet installed:** a cargo-hakari workspace-hack crate (`filesquid-workspace-hack`, dev-dependency only) for one feature set in test and Clippy builds, with a pre-commit drift gate and a `cargo check -p <crate> --lib --locked` stand-alone check per module crate; adopted only if its measurement gate passes ([ADR 039](./architecture/039-cargo-hakari-workspace-hack.md)). It does not change any crate's runtime, WASM or published dependency graph.
- Svelte is removed ([ADR 003](./architecture/003-svelte-client-rendered-ui.md) superseded by [ADR 031](./architecture/031-vanilla-typescript-ui-and-white-label-theming.md)).
- Standards in use: OCFL 1.1; C2PA records per PRD F-043/F-044; ISO 15836-2 `/terms/`; RFC 4122 / RFC 9562 UUID version 4.

## UI surfaces

The FileSquid default UI is specified in [SRS-F-055](#srs-f-055-default-ui-and-panel-composition). Each distribution specifies its own surfaces in its own SRS. [SRS-UI-007](#srs-ui-007-light-and-dark-themes) is the design-token contract every distribution implements.

### SRS-UI-007: Light and dark themes

This block is the **design-token contract** that every distribution (FileSquid default, ImageSquid, Arkrim, third parties) MUST implement.
- Tokens MUST be CSS custom properties with the `--fs-` prefix and the Arkrim token role names (for example `--fs-surface`, `--fs-on-surface`, `--fs-primary`, `--fs-on-primary`).
- Each distribution MUST define every required token for both a light and a dark theme. A missing token in either theme MUST fail the token test.
- FileSquid custom elements MUST style colour, type, spacing and radius only through tokens. Component stylesheets MUST NOT contain literal colour values.
- The theme MUST follow the operating-system colour scheme by default. A theme control in Settings, Appearance MUST switch light and dark, and the choice MUST persist across launches (owner answer 7, verbatim: "Settings -> Appaerance").
- Each distribution sets its own values under these token names.
- Text and primary actions on their surfaces MUST meet WCAG 2.2 AA (4.5:1) in both themes, in every distribution, with one owner exception for the FileSquid default distribution (owner decision 2026-10-22 on Conflict Artifact CA-1, verbatim: "a", keeping the [FileSquid Design Guide](./assets/FileSquid-design-guide-v1.pdf) values of answer 15, verbatim: "No follow the guide in all cases"; [PRD NFR-006](./prd.md#nfr-006-accessibility-and-usability)). Only the pairs below are exempt, each only at exactly the listed values and with its measured ratio; the token test MUST accept a listed pair only when both its colours equal the listed values, and every other pair MUST meet 4.5:1:
  - light: `--fs-on-primary` `#ffffff` on `--fs-primary` `#16ac49`, 2.99:1; `--fs-primary` `#16ac49` as text (links, ghost buttons) on `#ffffff` (`--fs-surface`, `--fs-surface-card`, `--fs-surface-overlay`), 2.99:1; `--fs-on-primary-container` `#0f8a39` on `--fs-primary-container` `#f0faf4`, 4.18:1; `--fs-error` `#e53935` on `#ffffff` (the same three surfaces), 4.23:1, and on `#f5f5f5` (`--fs-surface-container`, `--fs-surface-raised`), 3.88:1; `--fs-on-surface-muted` `#777777` on `#ffffff` (the same three surfaces), 4.48:1; `--fs-on-surface-placeholder` `#999999` on `#ffffff` (the same three surfaces), 2.85:1;
  - dark: `--fs-on-primary` `#ffffff` on `--fs-primary` `#16ac49`, 2.99:1; `--fs-on-surface-muted` and `--fs-on-surface-placeholder` `#6a6a6a` on `--fs-surface` `#141414`, 3.41:1, and on `--fs-surface-card` `#1c1c1c`, 3.15:1;
  - both themes: `#ffffff` text on `--fs-badge-document` `#fd1e23`, 3.90:1, and on `--fs-badge-media` `#007af9`, 4.08:1.
- The exception covers text contrast only. Focus indicators MUST reach 3:1 (WCAG 2.2 success criterion 1.4.11 (Level AA)) in both themes with no exception; because the guide's `#16ac49` is 2.99:1 on `#ffffff`, the focus ring and the focused input border MUST use `--fs-focus-ring` (owner decision 2026-10-23, verbatim: "3. keep"; `#0f8a39` light, 4.45:1 on `#ffffff`, 4.09:1 on `#f5f5f5`, 4.18:1 on `#f0faf4`; `#16ac49` dark, at least 4.68:1 on every dark surface) ([UI-S-016](../apps/filesquid/ui/resources/srs.md)). Every other WCAG 2.2 AA criterion, the labels, keyboard operation and the other contracts of NFR-006 stay required. Text of a disabled control is outside WCAG 2.2 success criterion 1.4.3 (Level AA) and is not a contrast pair.
- The boundaries that identify user-interface components (text inputs, search fields, selects, checkboxes and radio buttons) MUST reach 3:1 against every surface they sit on (WCAG 2.2 success criterion 1.4.11 (Level AA)), in light and dark, with no exception (owner decision 2026-10-23, verbatim: "1. darken them", resolving Conflict Artifact CA-4 of the [v3 sync walkthrough](./tasks/Task-docs-v3-mockup-requirements-sync-Walkthrough.md)). The FileSquid default draws them in `--fs-outline-control` `#8e8e8e` light (3.28:1 on `#ffffff`, 3.01:1 on `#f5f5f5`) and `#757575` dark (4.00:1 on `#141414`, 3.70:1 on `#1c1c1c`, 3.37:1 on `#242424`, 3.03:1 on `#2c2c2c`), the lightest neutral greys that reach 3:1 on every surface they sit on. The guide's `#cccccc` / `#333333` (`--fs-outline-strong`) stays for dividers, tables, menus, dialogs, cards and the borders of text-labelled buttons, which success criterion 1.4.11 does not require to be identified by a boundary.
- A theme MUST render the colour, type, spacing and radius constants of the active mockup suite ([UI-S-024](../apps/filesquid/ui/resources/srs.md)); a pixel or two of browser rendering difference is acceptable, and no screenshot hash or one-pixel coordinate match is required ([mockups README](./ui-mockups/README.md)).
- The theme control, the brand slots, the measured contrast pairs and the FileSquid default values are specified in [UI-S-015](../apps/filesquid/ui/resources/srs.md); the ImageSquid primary values in the ImageSquid SRS (IS-S-008).

**Source**
- [NFR-006](./prd.md#nfr-006-accessibility-and-usability), [F-058](./prd.md#f-058-white-label-distributions)
- [FileSquid U-24](./product-vision.md#one-api-hosts-and-distributions), [T-051](./product-vision.md#api-and-hosts)
- [ADR 031](./architecture/031-vanilla-typescript-ui-and-white-label-theming.md)

**Tests**
- [`theme_tokens_SRS-UI-007.feature`](../apps/filesquid/ui/tests/features/theme_tokens_SRS-UI-007.feature)

## Functional requirements (PRD F-IDs)

Functional requirements apply to FileSquid. Each block names its delivery: the **FileSquid platform** or **FileSquid modules** that distribution views present. A distribution's views are specified in that distribution's SRS. Every backend capability is a FileSquid module behind the [FileSquid API](#srs-f-056-single-filesquid-api); a distribution adds views only.

### SRS-F-001: Independent local library and ownership

The system MUST implement [PRD F-001](./prd.md#f-001-independent-local-library-and-ownership) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.
- The API MUST expose versioned routes to create a library on an owner-chosen OCFL storage root, open it, copy a completed snapshot and rebuild the catalog from files, with opaque identifiers and no filesystem paths in answers, and with every refused call journaled ([API-S-025](../apps/filesquid/crates/api/resources/srs.md)). A folder MUST reach those routes only as an opaque location ID; on the desktop the owner chooses it in a native folder dialog that the host shows when the UI calls the location-choices route, and the webview never receives its path ([API-S-034](../apps/filesquid/crates/api/resources/srs.md), [DE-S-017](../apps/filesquid/crates/desktop/resources/srs.md)).
- **Owner-chosen folders only.** Every library MUST live in a folder the owner chooses, which holds only its OCFL storage root; the host's data directory MUST hold no library files or metadata, only each library's rebuildable catalog index keyed by the library's UUID and its writer staging and version leases, and opening a moved or copied library folder MUST rebuild that index from the folder ([DE-S-009](../apps/filesquid/crates/desktop/resources/srs.md), [DE-S-010](../apps/filesquid/crates/desktop/resources/srs.md), [DE-S-018](../apps/filesquid/crates/desktop/resources/srs.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md)).
- **First use and rebuild.** On first use FileSquid MUST create the first library and its first collection (owner answer 4, verbatim: "The first collection should be created automatically upon install or first use if it doesnt already exists. Prompt the user for a name and location."; "a name and location" are the library's name and folder, owner confirmation 2026-10-22, verbatim: "1. confirmed"; [UI-S-034](../apps/filesquid/ui/resources/srs.md)). The rebuild from files MUST run as a durable job with progress, pause, resume and cancel (owner answer 9, verbatim: "Yes, rebuild should be a durable job"; [API-S-025](../apps/filesquid/crates/api/resources/srs.md)).
- **Original names without FileSquid.** An app-closed copy of a library MUST let ordinary tools map every stored original `<file-uuid>.<ext>` to the owner's original file name through the `<file-uuid>.metadata.md` beside it ([MD-S-008](../apps/filesquid/crates/metadata/resources/srs.md)).

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-001](./prd.md#f-001-independent-local-library-and-ownership)
- [architecture.md](./architecture.md#41-data-architecture)
- [ADR 005](./architecture/005-sqlite-local-postgres-hosted.md), [ADR 010](./architecture/010-ocfl-1-1-dual-profile.md), [ADR 016](./architecture/016-rust-ocfl-library.md), [ADR 021](./architecture/021-uuid-v4-identifiers.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md)

**Tests**
- [`independent_local_library_and_ownership_F-001.feature`](../apps/filesquid/ui/tests/features/independent_local_library_and_ownership_F-001.feature)
- [`library_domain_commands_and_queries_API-S-019.feature`](../apps/filesquid/crates/api/tests/features/library_domain_commands_and_queries_API-S-019.feature)
- [`local_library_and_version_inventory_repair_API-S-023.feature`](../apps/filesquid/crates/api/tests/features/local_library_and_version_inventory_repair_API-S-023.feature)

### SRS-F-002: Safe file, folder and device import

The system MUST implement [PRD F-002](./prd.md#f-002-safe-file-folder-and-device-import) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.
- **Stored name.** The import MUST store the original bytes under `<file-uuid>.<ext>`: the file's UUID v4 FileId in lowercase hyphenated form plus its original extension when that extension is 1 to 16 ASCII letters or digits, kept as given (otherwise no extension). The owner's file name MUST NOT appear in any logical path; the object layout is the FileSquid OCFL application profile ([API-S-026](../apps/filesquid/crates/api/resources/srs.md)).
- **Metadata record.** In the same OCFL version and the same directory the import MUST commit `<file-uuid>.metadata.md`, the file's only metadata record: the exact original file name with its Unicode normalisation form, the display name (initially the original name), the extension, the format and MIME type, the append-only `import` section of ADR 038 facts and the extracted `dcterms:` / `schema:` statements ([MD-S-008](../apps/filesquid/crates/metadata/resources/srs.md), [MD-S-013](../apps/filesquid/crates/metadata/resources/srs.md)). There is no separate `file-record.md`.
- **Tracked and described, no C2PA.** Import completion MUST require the verified content, its metadata record and the import's PROV-O record in that version ([SRS-F-067](#srs-f-067-prov-o-history-of-every-version-and-change)); no import step MAY require the `c2pa` module.
- **Rename.** A rename MUST change only the display name, committed as a new version of the metadata record; it MUST NOT rename the stored original or change the recorded original file name.
- **Review and progress per file.** After the owner picks the source and before the job starts, the upload route MUST answer a review plan (files found, new, already in the library, bytes to copy, destination, free space after the upload, staging limit) that the UI shows and the owner confirms (owner answer 5, verbatim: "Keep the review"); the job MUST report progress per file, with each file's state, error code and a retry (owner answer 6, verbatim: "Yes, report progress per file") ([API-S-036](../apps/filesquid/crates/api/resources/srs.md), [API-S-037](../apps/filesquid/crates/api/resources/srs.md), [UI-S-028](../apps/filesquid/ui/resources/srs.md)). User-facing words are Upload and Download for import and export (owner answer 8, verbatim: "Let's use Upload and Download (as synomyns for Import and Export respectively)").
- **Destination.** Every import and upload MUST name a destination collection (and MAY name a folder inside it) and MUST register each file there as its home collection; a request without one MUST be refused before any dialog or staged byte, with no default collection and no top-level upload ([API-S-036](../apps/filesquid/crates/api/resources/srs.md), [UI-S-002](../apps/filesquid/ui/resources/srs.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md)).

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-002](./prd.md#f-002-safe-file-folder-and-device-import)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 006](./architecture/006-libvips-native-stills-not-imagemagick.md), [ADR 011](./architecture/011-native-media-adapters-and-ffmpeg-license.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md)

**Tests**
- [`safe_file_folder_and_device_import_F-002.feature`](../apps/filesquid/ui/tests/features/safe_file_folder_and_device_import_F-002.feature)

### SRS-F-003: Browsing, selection and viewing

The system MUST implement [PRD F-003](./prd.md#f-003-browsing-selection-and-viewing) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-003](./prd.md#f-003-browsing-selection-and-viewing)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-004: Collections, folders and navigation

The system MUST implement [PRD F-004](./prd.md#f-004-collections-folders-and-navigation) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.
- The API MUST expose a navigation listing that answers one level of the navigation tree (the host's repository, its libraries, their collections and virtual collections, and the folders implied by committed logical paths) with opaque node handles and the ancestors a breadcrumb needs, filtered by the caller's permissions before paging ([API-S-021](../apps/filesquid/crates/api/resources/srs.md)).
- The default UI's pinned destinations, section order and collapse state MUST persist across restarts: in the API's per-user UI preferences on a host that serves them, otherwise on the device in Web Storage as a versioned value of opaque node handles, node kinds, section order and collapse state, without names, titles, paths, tokens or content handles ([UI-S-017](../apps/filesquid/ui/resources/srs.md)).
- **Recently deleted.** Deleting a collection that still holds files MUST mark each of them recoverably deleted as [SRS-F-008](#srs-f-008-deletion-hidden-files-and-duplicate-review) states, in the same command and version; nothing is moved, and each file keeps its OCFL object, logical path and home collection, and the one Recently deleted filter, over the deleted files of every library this host has opened that the caller may read, lists it (owner answer 3, 2026-10-22, verbatim: "There is only one trash. Not a separate trash per library"; owner decisions 2026-10-22, verbatim: "RE: deletion question in #124, when users delete a collection that still holds files, those files go into a "Recently deleted" top-level directory within their library. That directory has a default 30 day retention period before files inside are expunged for good."; then ""Recently deleted" aka Trash can will be a navigation filter over collection items with the deleted state. They are not moved. They are completely deleted from library and filesystem after recently deleted retention period expires."; [API-S-019](../apps/filesquid/crates/api/resources/srs.md), [API-S-021](../apps/filesquid/crates/api/resources/srs.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md) Decision 8). No file is ever "unfiled".
- **Home collection.** Every file MUST have exactly one home collection, and deletion is a state: a deleted file keeps the identifier of its home collection even after that collection is deleted. Uploads, imports, copies and moves MUST name their destination and MUST register the file with its home collection and logical path in the collection snapshot, from which the catalog index is rebuilt ([API-S-011](../apps/filesquid/crates/api/resources/srs.md), [API-S-026](../apps/filesquid/crates/api/resources/srs.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md)).
- **Same logical path.** When an upload, import, copy, move, rename or folder creation would give a new item a logical path that already exists in its collection, a recoverably deleted file's path included, the API MUST add a hyphen and the first free number from 1 so the item sits beside the existing one (`directoryY-1`, `directoryY-2`; a file takes the number after the first part of its name, before the first dot (`photo.jpg` becomes `photo-1.jpg`, then `photo-2.jpg`; `archive.tar.gz` becomes `archive-1.tar.gz`), a name without a dot takes it at its end, and a name that starts with a dot keeps that dot in its first part so a numbered copy stays a dotfile (`.env` becomes `.env-1`, `.config.json` becomes `.config-1.json`)), and MUST NOT refuse or overwrite; at most 9,999 numbers are tried, and when all are taken the request MUST be refused as `conflict` (`path_suffix_exhausted`) and journaled; counting deleted files' paths as taken is the orchestrator's reading (owner decisions 2026-10-22, verbatim: "Filesquid should simply add an incremental number to the new file path so its sits next to conflicting filepath. Eg if filepathX/directoryY already exists and the user wants to create a new directory for a new import at the pre-existing filepathX/directoryY then FileSquid would name the new directory filepathX/directoryY-1, and then filepathX/directoryY-2 and so on."; "Caps: 9999 for both"; "no, put the number of the first part of the filename, before the first dot"; "yes, the edge is correct especially because the leading dot has operational implications"; [API-S-011](../apps/filesquid/crates/api/resources/srs.md), [API-S-036](../apps/filesquid/crates/api/resources/srs.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md) Decision 12).

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-004](./prd.md#f-004-collections-folders-and-navigation)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md)

**Tests**
- [`albums_folders_and_navigation_F-004.feature`](../apps/filesquid/ui/tests/features/albums_folders_and_navigation_F-004.feature)
- [`collections_and_virtual_collections_F-004.feature`](../apps/filesquid/crates/core/tests/features/collections_and_virtual_collections_F-004.feature)
- [`navigation_listing_API-S-021.feature`](../apps/filesquid/crates/api/tests/features/navigation_listing_API-S-021.feature)
- [`navigation_preferences_UI-S-017.feature`](../apps/filesquid/ui/tests/features/navigation_preferences_UI-S-017.feature)

### SRS-F-005: Favorites, metadata and keywords

The system MUST implement [PRD F-005](./prd.md#f-005-favorites-metadata-and-keywords) with the acceptance criteria stated there. Title, caption and keyword edits MUST write `dcterms:title`, `dcterms:description` and `dcterms:subject` (or confirmed `schema:keywords` overflow). UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-005](./prd.md#f-005-favorites-metadata-and-keywords)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)
- [ADR 022](./architecture/022-dublin-core-descriptive-metadata.md)

**Tests**
- [`favorites_metadata_and_keywords_F-005.feature`](../apps/filesquid/ui/tests/features/favorites_metadata_and_keywords_F-005.feature)

### SRS-F-006: Virtual collections and metadata search

The system MUST implement [PRD F-006](./prd.md#f-006-virtual-collections-and-metadata-search) with the acceptance criteria stated there. Search and saved-query predicates MUST address namespaced keys (`dcterms:creator`, `schema:certification`, and other populated descriptive fields) from the F-046 matrix. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-006](./prd.md#f-006-virtual-collections-and-metadata-search)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)
- [ADR 022](./architecture/022-dublin-core-descriptive-metadata.md)

**Tests**
- [`smart_albums_and_metadata_search_F-006.feature`](../apps/filesquid/ui/tests/features/smart_albums_and_metadata_search_F-006.feature)

### SRS-F-007: Automatic and activity collections

The system MUST implement [PRD F-007](./prd.md#f-007-automatic-and-activity-collections) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-007](./prd.md#f-007-automatic-and-activity-collections)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-008: Deletion, hidden files and duplicate review

The system MUST implement [PRD F-008](./prd.md#f-008-deletion-hidden-files-and-duplicate-review) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.
- Hidden, deleted and duplicate-review views MUST list files by display name ([UI-S-019](../apps/filesquid/ui/resources/srs.md)); hide, unhide, delete, restore and permanent delete MUST each record a PROV-O activity ([SRS-F-067](#srs-f-067-prov-o-history-of-every-version-and-change)).
- **Recently deleted.** FileSquid MUST offer one reserved Recently deleted (Trash) navigation filter over the deleted files of every library this host has opened that the caller may read, the final row of the navigation tree, a single row over the deleted files of every library this host has opened that the caller may read after a 1 px divider, with no section eyebrow (owner answers 2026-10-22, verbatim: "There is only one trash. Not a separate trash per library" and "keen the navigation tree in sections by kind. Recently deleted is one of those (at the bottom)"; ADR 041 Decision 8 as amended 2026-10-23, owner decision 2026-10-23, verbatim: "2. yes amend ADR 041"), which is not a collection, a folder or a storage location and MUST list every deleted file, whether deleted one by one or with its collection, under one retention, expiry and restore rule; nothing is moved (owner decisions 2026-10-22, verbatim: "RE: deletion question in #124, when users delete a collection that still holds files, those files go into a "Recently deleted" top-level directory within their library. That directory has a default 30 day retention period before files inside are expunged for good."; then ""Recently deleted" aka Trash can will be a navigation filter over collection items with the deleted state. They are not moved. They are completely deleted from library and filesystem after recently deleted retention period expires."; then "3. yes, single-file deletes use the same Trash"; [API-S-019](../apps/filesquid/crates/api/resources/srs.md), [API-S-021](../apps/filesquid/crates/api/resources/srs.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md) Decision 8).
- **Restore.** Restore MUST return a file to its original OCFL object and logical path, in its home collection when it still exists; otherwise the host MUST create a collection titled "untitled collection", or "untitled collection 2", "untitled collection 3" and so on with the first free number, and restore the file into it, with the collection and the file's state in one journaled operation and snapshot version with its PROV-O activity. At most 9,999 titles are tried (up to "untitled collection 9999"), and when all are taken the restore MUST be refused as `conflict` (`restore_name_exhausted`) and journaled. This is the one exception to "no default collection" (owner decisions 2026-10-22, verbatim: "Into its original OCFL object and relative filepath inside FileSquid. If its original collection is gone create a new one "untitled collection" and put the restored file inside. If the name "untitled collection" is already taken then just add numberical increments as name suffix, eg "untitled collection 2, untitled collection 3, etc)"; "Caps: 9999 for both"; [API-S-019](../apps/filesquid/crates/api/resources/srs.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md) Decision 9).
- **Retention.** The installation-wide default retention MUST be 30 days, kept in the host's operational store; each library MAY set its own period, kept in its collection snapshot, which overrides the default for that library. A period MUST be 1 to 3,650 days (proposed, owner to confirm); a change MUST be journaled and applies only to files deleted afterwards (owner decision 2026-10-22, verbatim: "The 30 days defaut retention policy for any files moved to the "Recently deleted" trashcan applies across the whole installation and be configured per library"; [API-S-019](../apps/filesquid/crates/api/resources/srs.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md) Decision 10).
- **Expiry.** At the end of a file's retention, one admin setting MUST govern what the host removes: **keep version history** (the default) under **keep version history** the OCFL object stays in the library under the same identifier at the same path as a **tombstone**: ALL its content other than the version history (the file's bytes, its metadata record and any derivatives) is fully deleted, and to stay valid OCFL 1.1 the host deletes the original object and recreates it as the tombstone, which holds the facts of every former version (number, `created`, `user`, `message`, digests and sizes, without bytes), the PROV-O records and a purge record, and whose version history records both the original object's deletion and the tombstone's creation as events, each with its time, actor, operation kind and PROV-O activity, linked to the journal; a crash part-way MUST NOT leave a half-deleted object, so OCFL recovery and repair complete or report an interrupted tombstone write; the file's index entries are removed or marked purged, and a library recovered from OCFL alone shows a tombstone as purged history, never as a file (owner decisions 2026-10-22, verbatim: "yes, for deletion with version history kept, use the tombstone model where the OCFL object remains in the library but ALL of its contents other than version history is fully deleted"; then "fine on your proposed method to keep tombstone a valid OCFL but then make sure that the original deletion and tombstone creation are also an event in the version history"); **delete all version history** deletes the OCFL object, its index entries, the file and its metadata. Every expiry MUST be a recorded `purge` operation, journaled before or with the deletion and with its PROV-O activity in the library's journal and history objects, never silent (owner decisions 2026-10-22, verbatim: "Okay, keep the version history by default but provide an admin config option to delete the version history as well"; then "Okay, if the "delete all version history" setting is enabled then the OCFL and any index entries are deleted when the retention period expires, along with the files and metadata, etc.. If the "keep version history" setting is enabled then keep those source OCFL objects in the library, with only version history, no other content."; [API-S-019](../apps/filesquid/crates/api/resources/srs.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md) Decision 11).

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-008](./prd.md#f-008-deletion-hidden-files-and-duplicate-review)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md)

**Tests**
- [`deletion_hidden_media_and_duplicate_review_F-008.feature`](../apps/filesquid/ui/tests/features/deletion_hidden_media_and_duplicate_review_F-008.feature)

### SRS-F-009: Multiple libraries and filesystem administration

The system MUST implement [PRD F-009](./prd.md#f-009-multiple-libraries-and-filesystem-administration) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.
- **Library facts.** The API MUST answer each library's name, the free space of the disk that holds it, and a list of recently opened libraries, with no path ([API-S-038](../apps/filesquid/crates/api/resources/srs.md)); the default UI shows them on the start screen, the library page and the sidebar (owner answer 10, verbatim: "yes add library facts to v3"; [UI-S-025](../apps/filesquid/ui/resources/srs.md), [UI-S-027](../apps/filesquid/ui/resources/srs.md)).

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-009](./prd.md#f-009-multiple-libraries-and-filesystem-administration)
- [architecture.md](./architecture.md#41-data-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`multiple_libraries_and_filesystem_administration_F-009.feature`](../apps/filesquid/ui/tests/features/multiple_libraries_and_filesystem_administration_F-009.feature)

### SRS-F-010: Export and portable backup

The system MUST implement [PRD F-010](./prd.md#f-010-export-and-portable-backup) with the acceptance criteria stated there. Batch export MUST include FileId (RFC 4122 / RFC 9562 UUID version 4) as a naming scheme. When F-046 privacy/scrubbing is enabled, the default batch-export name MUST be FileId plus the real media extension; source and original filenames remain available through an explicit **Use original filename** choice. Owner archival/original export remains a distinct preservation action: bytes stay exact, original names MAY appear in the package, and original names MUST appear in the machine-readable manifest. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.
- **Download name.** Owner downloads and owner exports MUST default to the display name, made collision-safe within one export by a numeric suffix before the extension; content and download routes MUST send `Content-Disposition` with an RFC 6266 `filename*` parameter (RFC 5987 `UTF-8''` percent-encoding) and a safe ASCII `filename` fallback ([API-S-027](../apps/filesquid/crates/api/resources/srs.md)). Batch naming schemes (FileId, source, title, sequential, prefixes) remain options.
- **Export history.** Every export, scrubbed export, ZIP download and share or publication output MUST commit its PROV-O activity to the library's history object; a single-file download MUST be journaled only and MUST NOT create a PROV-O record ([SRS-F-067](#srs-f-067-prov-o-history-of-every-version-and-change)).

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-010](./prd.md#f-010-export-and-portable-backup)
- [ADR 021](./architecture/021-uuid-v4-identifiers.md)
- [architecture.md](./architecture.md#41-data-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`export_and_portable_backup_F-010.feature`](../apps/filesquid/ui/tests/features/export_and_portable_backup_F-010.feature)

### SRS-F-011: Non-destructive editing workflow

The system MUST implement [PRD F-011](./prd.md#f-011-non-destructive-editing-workflow) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-011](./prd.md#f-011-non-destructive-editing-workflow)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 006](./architecture/006-libvips-native-stills-not-imagemagick.md), [ADR 015](./architecture/015-prov-o-jsonld-and-c2pa-sdk.md)

**Tests**
- [`immutable_originals_and_recipes_TC-001.feature`](../apps/filesquid/crates/core/tests/features/immutable_originals_and_recipes_TC-001.feature)

### SRS-F-012: Composition and tonal/color editing

The system MUST implement [PRD F-012](./prd.md#f-012-composition-and-tonalcolor-editing) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-012](./prd.md#f-012-composition-and-tonalcolor-editing)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-013: Detail, filters, retouch and RAW

The system MUST implement [PRD F-013](./prd.md#f-013-detail-filters-retouch-and-raw) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-013](./prd.md#f-013-detail-filters-retouch-and-raw)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-014: AI-assisted cleanup and styles

The system MUST implement [PRD F-014](./prd.md#f-014-ai-assisted-cleanup-and-styles) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-014](./prd.md#f-014-ai-assisted-cleanup-and-styles)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 015](./architecture/015-prov-o-jsonld-and-c2pa-sdk.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-015: Portrait, depth, Cinematic and spatial media

The system MUST implement [PRD F-015](./prd.md#f-015-portrait-depth-cinematic-and-spatial-media) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-015](./prd.md#f-015-portrait-depth-cinematic-and-spatial-media)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-016: Live Photos, bursts and motion images

The system MUST implement [PRD F-016](./prd.md#f-016-live-photos-bursts-and-motion-images) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-016](./prd.md#f-016-live-photos-bursts-and-motion-images)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-017: Video and audio editing

The system MUST implement [PRD F-017](./prd.md#f-017-video-and-audio-editing) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-017](./prd.md#f-017-video-and-audio-editing)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-018: Markup and companion annotation

The system MUST implement [PRD F-018](./prd.md#f-018-markup-and-companion-annotation) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-018](./prd.md#f-018-markup-and-companion-annotation)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-019: People, pets and groups

The system MUST implement [PRD F-019](./prd.md#f-019-people-pets-and-groups) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-019](./prd.md#f-019-people-pets-and-groups)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-020: Semantic search, OCR and visual understanding

The system MUST implement [PRD F-020](./prd.md#f-020-semantic-search-ocr-and-visual-understanding) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-020](./prd.md#f-020-semantic-search-ocr-and-visual-understanding)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-021: Places, trips and curation controls

The system MUST implement [PRD F-021](./prd.md#f-021-places-trips-and-curation-controls) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-021](./prd.md#f-021-places-trips-and-curation-controls)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-022: Memories and storytelling

The system MUST implement [PRD F-022](./prd.md#f-022-memories-and-storytelling) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-022](./prd.md#f-022-memories-and-storytelling)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-023: Slideshows, printing and photo projects

The system MUST implement [PRD F-023](./prd.md#f-023-slideshows-printing-and-photo-projects) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-023](./prd.md#f-023-slideshows-printing-and-photo-projects)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-024: System sharing and extension interfaces

The system MUST implement [PRD F-024](./prd.md#f-024-system-sharing-and-extension-interfaces) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-024](./prd.md#f-024-system-sharing-and-extension-interfaces)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`system_sharing_and_extension_interfaces_F-024.feature`](../apps/filesquid/ui/tests/features/system_sharing_and_extension_interfaces_F-024.feature)

### SRS-F-025: Migration selection, inventory and pilot

The system MUST implement [PRD F-025](./prd.md#f-025-migration-selection-inventory-and-pilot) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-025](./prd.md#f-025-migration-selection-inventory-and-pilot)
- [architecture.md](./architecture.md#33-backend-architecture)
- [ADR 007](./architecture/007-durable-jobs-and-interactive-performance.md)

**Tests**
- [`migration_selection_inventory_and_pilot_F-025.feature`](../apps/filesquid/ui/tests/features/migration_selection_inventory_and_pilot_F-025.feature)

### SRS-F-026: Migration preservation and reconstruction

The system MUST implement [PRD F-026](./prd.md#f-026-migration-preservation-and-reconstruction) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-026](./prd.md#f-026-migration-preservation-and-reconstruction)
- [architecture.md](./architecture.md#42-database-decisions)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`migration_preservation_and_reconstruction_F-026.feature`](../apps/filesquid/ui/tests/features/migration_preservation_and_reconstruction_F-026.feature)

### SRS-F-027: Migration execution, recovery and reconciliation

The system MUST implement [PRD F-027](./prd.md#f-027-migration-execution-recovery-and-reconciliation) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-027](./prd.md#f-027-migration-execution-recovery-and-reconciliation)
- [architecture.md](./architecture.md#33-backend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`migration_execution_recovery_and_reconciliation_F-027.feature`](../apps/filesquid/ui/tests/features/migration_execution_recovery_and_reconciliation_F-027.feature)

### SRS-F-028: Supported migration routes

The system MUST implement [PRD F-028](./prd.md#f-028-supported-migration-routes) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid modules reached through the [FileSquid API](#srs-f-056-single-filesquid-api); distribution views present it.

**Source**
- [F-028](./prd.md#f-028-supported-migration-routes)
- [architecture.md](./architecture.md#33-backend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- Backend scenarios are written by `@feature-writer` in the implementing FileSquid module; distribution view scenarios live with the distribution.

### SRS-F-029: Client encryption and key custody

The system MUST implement [PRD F-029](./prd.md#f-029-client-encryption-and-key-custody) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.
- Recovery material MUST include a human-readable recovery phrase from a published word list, shown once and re-displayable after re-authentication, usable to add a device when no authorised device is available. Enabling encryption, adding a device and disabling encryption MUST each require an explicit confirmation stating what becomes inaccessible without the phrase and whether the step is reversible. The UI MUST show encryption status for the connected host and a lock indicator on encrypted hosted content ([PRD F-029](./prd.md#f-029-client-encryption-and-key-custody), N e2ee).
- A FileSquid node (a Drive) MAY authorise several devices to upload. Each uploading device MUST have its Ed25519 device public key on that node's whitelist, and a key that is not listed MUST be refused. The uploader MUST encrypt before any byte crosses a network. A password MUST NOT place a key on the whitelist. Revoking a key removes it from the whitelist and MUST stop later uploads, and the key MUST NOT be able to delete recovery points ([PRD F-029](./prd.md#f-029-client-encryption-and-key-custody)).

**Delivered by:** FileSquid platform (modules and hosts).

**Source**
- [F-029](./prd.md#f-029-client-encryption-and-key-custody)
- [architecture.md](./architecture.md#62-security-measures)
- [ADR 004](./architecture/004-axum-hosted-ciphertext-mailbox.md), [ADR 009](./architecture/009-cryptography-webcrypto-compatible-suite.md)

**Tests**
- [`client_encryption_and_key_custody_F-029.feature`](../apps/filesquid/crates/server/tests/features/client_encryption_and_key_custody_F-029.feature)
- [`cryptographic_envelopes_F-029.feature`](../apps/filesquid/crates/core/tests/features/cryptographic_envelopes_F-029.feature)

### SRS-F-030: Personal synchronization and availability

The system MUST implement [PRD F-030](./prd.md#f-030-personal-synchronization-and-availability) with the acceptance criteria stated there. Devices whose keys are on one node's whitelist MUST sync that node's live working set with Automerge. A device that is not on the whitelist MUST NOT join. Automerge is the live working set. It is not a recovery point and not a second OCFL engine. The ciphertext-only mailbox MUST NOT read the Automerge document. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-030](./prd.md#f-030-personal-synchronization-and-availability)
- [architecture.md](./architecture.md#33-backend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`personal_synchronization_and_availability_F-030.feature`](../apps/filesquid/ui/tests/features/personal_synchronization_and_availability_F-030.feature)

### SRS-F-031: Encrypted backup and verified restore

The system MUST implement [PRD F-031](./prd.md#f-031-encrypted-backup-and-verified-restore) with the acceptance criteria stated there. The owner MUST be able to commit and push the current state to OCFL as the current snapshot backup package on demand, or on a configurable schedule whose only settings are 1 hour and 24 hours. A scheduled or on-demand package is a recovery point. The live Automerge head is not that package. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-031](./prd.md#f-031-encrypted-backup-and-verified-restore)
- [architecture.md](./architecture.md#62-security-measures)
- [ADR 012](./architecture/012-s3-compatible-ciphertext-storage.md)

**Tests**
- [`encrypted_backup_and_verified_restore_F-031.feature`](../apps/filesquid/ui/tests/features/encrypted_backup_and_verified_restore_F-031.feature)

### SRS-F-032: Accounts, entitlements, billing and cancellation

The system MUST implement [PRD F-032](./prd.md#f-032-accounts-entitlements-billing-and-cancellation) with the acceptance criteria stated there. The FileSquid paid core MUST be a subscription account for encrypted backup and/or encrypted account sync, hosted at `*.filesquid.tech`, at personal-to-institutional scale on one account model ([C-031](./product-vision.md#platforms-originals-and-service-limits), [SRS-F-059](#srs-f-059-hosted-encrypted-backup-and-sync-service)). A distribution MAY sell it under its own brand. OCFL collection batch jobs including FAIR reporting MUST be a later paid layer, delivered as FileSquid modules ([C-051](./product-vision.md#platforms-originals-and-service-limits), [SRS-F-060](#srs-f-060-archival-curation-jobs-and-datasets)). UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-032](./prd.md#f-032-accounts-entitlements-billing-and-cancellation)
- [C-031](./product-vision.md#platforms-originals-and-service-limits)
- [C-051](./product-vision.md#platforms-originals-and-service-limits)
- [architecture.md](./architecture.md#33-backend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`accounts_entitlements_billing_and_cancellation_F-032.feature`](../apps/filesquid/ui/tests/features/accounts_entitlements_billing_and_cancellation_F-032.feature)

### SRS-F-033: Shared albums and hosted galleries

The system MUST implement [PRD F-033](./prd.md#f-033-shared-albums-and-hosted-galleries) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-033](./prd.md#f-033-shared-albums-and-hosted-galleries)
- [architecture.md](./architecture.md#33-backend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`shared_albums_and_hosted_galleries_F-033.feature`](../apps/filesquid/ui/tests/features/shared_albums_and_hosted_galleries_F-033.feature)

### SRS-F-034: Family accounts and shared libraries

The system MUST implement [PRD F-034](./prd.md#f-034-family-accounts-and-shared-libraries) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-034](./prd.md#f-034-family-accounts-and-shared-libraries)
- [architecture.md](./architecture.md#33-backend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`family_accounts_and_shared_libraries_F-034.feature`](../apps/filesquid/ui/tests/features/family_accounts_and_shared_libraries_F-034.feature)

### SRS-F-035: Optional service extensions

The system MUST implement [PRD F-035](./prd.md#f-035-optional-service-extensions) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform (modules and hosts).

**Source**
- [F-035](./prd.md#f-035-optional-service-extensions)
- [architecture.md](./architecture.md#33-backend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`optional_service_extensions_F-035.feature`](../apps/filesquid/crates/server/tests/features/optional_service_extensions_F-035.feature)

### SRS-F-036: Browser and mobile-web experience

The system MUST implement [PRD F-036](./prd.md#f-036-browser-and-mobile-web-experience) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-036](./prd.md#f-036-browser-and-mobile-web-experience)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 003](./architecture/003-svelte-client-rendered-ui.md) (superseded by [ADR 031](./architecture/031-vanilla-typescript-ui-and-white-label-theming.md)), [ADR 008](./architecture/008-browser-wasm-scope.md), [ADR 025](./architecture/025-client-side-filesquid-engines.md) (amended by [ADR 032](./architecture/032-hosting-and-privacy-model.md))

**Tests**
- [`browser_and_mobileweb_experience_F-036.feature`](../apps/filesquid/ui/tests/features/browser_and_mobileweb_experience_F-036.feature)
- [`replica_cache_identity_and_pending_changes_API-S-024.feature`](../apps/filesquid/crates/api/tests/features/replica_cache_identity_and_pending_changes_API-S-024.feature)

### SRS-F-037: Setup, updates and diagnostics

The system MUST implement [PRD F-037](./prd.md#f-037-setup-updates-and-diagnostics) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-037](./prd.md#f-037-setup-updates-and-diagnostics)
- [architecture.md](./architecture.md#51-infrastructure-architecture)
- [ADR 002](./architecture/002-tauri2-desktop-shell.md)

**Tests**
- [`setup_updates_and_diagnostics_F-037.feature`](../apps/filesquid/ui/tests/features/setup_updates_and_diagnostics_F-037.feature)
- [`shared_health_status_CORE-S-004.feature`](../apps/filesquid/crates/core/tests/features/shared_health_status_CORE-S-004.feature)
- [`desktop_capabilities_F-037.feature`](../apps/filesquid/crates/desktop/tests/features/desktop_capabilities_F-037.feature)
- [`desktop_refusals_F-061.feature`](../apps/filesquid/crates/desktop/tests/features/desktop_refusals_F-061.feature) (every desktop shell refusal journaled)

### SRS-F-038: Open interfaces and product exit

The system MUST implement [PRD F-038](./prd.md#f-038-open-interfaces-and-product-exit) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform (modules and hosts).

**Source**
- [F-038](./prd.md#f-038-open-interfaces-and-product-exit)
- [architecture.md](./architecture.md#14-architectural-principles)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`open_interfaces_and_product_exit_F-038.feature`](../apps/filesquid/crates/server/tests/features/open_interfaces_and_product_exit_F-038.feature)
- [`command_query_ports_F-038.feature`](../apps/filesquid/crates/core/tests/features/command_query_ports_F-038.feature)

### SRS-F-039: Consent, notifications and activity privacy

The system MUST implement [PRD F-039](./prd.md#f-039-consent-notifications-and-activity-privacy) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.
- Notification preferences MUST be a per-user matrix of activity type (grouped: files, sharing, comments, versions, jobs) by channel (in-app, mail, push), with mail frequency `immediate`, `hourly`, `daily` or `weekly` and an independent daily summary switch. Defaults MUST follow the privacy rules of this block. Public-link downloads and uploads to a folder shared by link MUST be notifiable events ([SRS-F-066](#srs-f-066-file-and-folder-sharing-roles-links-and-shares-view); N activity-app).

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-039](./prd.md#f-039-consent-notifications-and-activity-privacy)
- [architecture.md](./architecture.md#62-security-measures)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`consent_notifications_and_activity_privacy_F-039.feature`](../apps/filesquid/ui/tests/features/consent_notifications_and_activity_privacy_F-039.feature)

### SRS-F-040: Compatibility and parity register

The system MUST implement [PRD F-040](./prd.md#f-040-compatibility-and-parity-register) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.
- The register MUST include the **drive-parity** table required by [PRD F-040](./prd.md#f-040-compatibility-and-parity-register): one row per documented difference from mainstream cloud drives and self-hosted file-sharing platforms (version retention, encryption granularity and keys, administrator recovery, encryption boundary of derivatives and indexes, AI overview, feeds, empty folders, copy semantics, editors, calendar, reminders, federation), each with its reason and owning F-ID.

**Delivered by:** FileSquid platform (modules and hosts).

**Source**
- [F-040](./prd.md#f-040-compatibility-and-parity-register)
- [architecture.md](./architecture.md#32-frontend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`compatibility_and_parity_register_F-040.feature`](../apps/filesquid/crates/server/tests/features/compatibility_and_parity_register_F-040.feature)
- [`capability_register_F-040.feature`](../apps/filesquid/crates/core/tests/features/capability_register_F-040.feature)

### SRS-F-041: Source-aware web clipping and HTTP evidence

The system MUST implement [PRD F-041](./prd.md#f-041-source-aware-web-clipping-and-http-evidence) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-041](./prd.md#f-041-source-aware-web-clipping-and-http-evidence)
- [architecture.md](./architecture.md#41-data-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`sourceaware_web_clipping_and_http_evidence_F-041.feature`](../apps/filesquid/ui/tests/features/sourceaware_web_clipping_and_http_evidence_F-041.feature)

### SRS-F-042: OCFL preservation and portable recovery

The system MUST implement [PRD F-042](./prd.md#f-042-ocfl-preservation-and-portable-recovery) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.
- The API MUST expose the version-inventory repair-candidate query and the owner-consented repair command as versioned routes, refusing every caller but the library's owner and refusing an object with nothing to repair ([API-S-025](../apps/filesquid/crates/api/resources/srs.md), [API-S-023](../apps/filesquid/crates/api/resources/srs.md)).

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-042](./prd.md#f-042-ocfl-preservation-and-portable-recovery)
- [architecture.md](./architecture.md#42-database-decisions)
- [ADR 010](./architecture/010-ocfl-1-1-dual-profile.md), [ADR 016](./architecture/016-rust-ocfl-library.md)

**Tests**
- [`ocfl_preservation_and_portable_recovery_F-042.feature`](../apps/filesquid/ui/tests/features/ocfl_preservation_and_portable_recovery_F-042.feature)
- [`local_library_and_version_inventory_repair_API-S-023.feature`](../apps/filesquid/crates/api/tests/features/local_library_and_version_inventory_repair_API-S-023.feature)

### SRS-F-043: C2PA records and action disclosure

The system MUST implement [PRD F-043](./prd.md#f-043-c2pa-records-and-action-disclosure) with the acceptance criteria stated there, wherever the `c2pa` module is enabled. A build without the module MUST NOT create or claim C2PA records; import tracking and PROV-O history do not depend on it ([SRS-F-067](#srs-f-067-prov-o-history-of-every-version-and-change)). UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-043](./prd.md#f-043-c2pa-records-and-action-disclosure)
- [architecture.md](./architecture.md#41-data-architecture)
- [ADR 015](./architecture/015-prov-o-jsonld-and-c2pa-sdk.md)
- [ADR 026](./architecture/026-filesquid-provenance-engine.md)

**Tests**
- [`importtime_c2pa_records_and_action_disclosure_F-043.feature`](../apps/filesquid/ui/tests/features/importtime_c2pa_records_and_action_disclosure_F-043.feature)
- [`c2pa_and_provo_records_F-043.feature`](../apps/filesquid/crates/core/tests/features/c2pa_and_provo_records_F-043.feature)

### SRS-F-044: Paid C2PA publishing and watermarking

The system MUST implement [PRD F-044](./prd.md#f-044-paid-c2pa-publishing-and-watermarking) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-044](./prd.md#f-044-paid-c2pa-publishing-and-watermarking)
- [architecture.md](./architecture.md#33-backend-architecture)
- [ADR 015](./architecture/015-prov-o-jsonld-and-c2pa-sdk.md)

**Tests**
- [`paid_c2pa_publishing_and_watermarking_F-044.feature`](../apps/filesquid/ui/tests/features/paid_c2pa_publishing_and_watermarking_F-044.feature)

### SRS-F-045: BYO AI connections and per-task model choice

The system MUST implement [PRD F-045](./prd.md#f-045-byo-ai-connections-and-per-task-model-choice) with the acceptance criteria stated there, including standing AI Connections disclosure, the inspectable **How AI Connections work** explanation, the per-job remote overlay, and the later `docs/` AI Connections manual page. Canonical English copy, the explanation fact list, overlay fields, and manuals wording live in F-045; this section MUST NOT restate them. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**. There is no dedicated v3 mockup for the AI Connections settings surface; those states are specified by F-045 / [FE-067](./product-vision.md#ai-the-user-chooses).

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-045](./prd.md#f-045-byo-ai-connections-and-per-task-model-choice)
- [FE-067](./product-vision.md#ai-the-user-chooses)
- [architecture.md](./architecture.md#33-backend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`byo_ai_connections_and_pertask_model_choice_F-045.feature`](../apps/filesquid/ui/tests/features/byo_ai_connections_and_pertask_model_choice_F-045.feature)

### SRS-F-046: Media metadata inspection, correction and privacy exports

The system MUST implement [PRD F-046](./prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports) with the acceptance criteria stated there. On upload into a node, EXIF and any other embedded metadata, and any sidecar, MUST be moved into that file's `<file-uuid>.metadata.md` and removed from the stored content bytes. The stored file is the content after that strip. Extracted source values MUST stay distinct from later owner edits. FileSquid MUST NOT keep a second sidecar. An upload MUST add a PROV-O record and a C2PA proof on the ProofMode model, signed by the uploading device key. The proof records the upload and cites the stripped metadata. It MUST NOT claim FileSquid witnessed the original capture. For sharing and publication routes, the privacy-on default outgoing filename MUST be FileId (UUID version 4) plus the real media extension. Owners MUST have an explicit **Use original filename** control, previewed with the outgoing disclosure, in the same class of choice as disabling scrubbing or retaining GPS.
- **Outgoing names.** A sharing or publication output MUST default to FileId plus the real media extension (C-033); a saved policy MAY select the original name. This is the only default that does not use the display name ([API-S-027](../apps/filesquid/crates/api/resources/srs.md)). The same choice governs the original file name inside a shared or published metadata record: it MUST be withheld by default and kept only when the saved policy selects the original name ([MD-S-013](../apps/filesquid/crates/metadata/resources/srs.md), owner decision 2026-10-20).
- **Outgoing history.** PROV-O records MUST be excluded from shared and published outputs by default; when selected, agent identities and identity-bearing logical paths MUST be scrubbed, and the preview MUST say so ([SRS-F-067](#srs-f-067-prov-o-history-of-every-version-and-change)).

Owner-authored descriptive metadata MUST use ISO 15836-2 `/terms/` properties (`dcterms:`), confirmed Schema.org overflow, or terms in a registered extension namespace ([SRS-F-052](#srs-f-052-metadata-module)). The inspector MUST list populated keys only. Create, read, update and delete MUST match the EXIF technical editor. Unmatched descriptive intent MUST suggest the closest Schema.org term (`schema:`). Every displayed pair MUST show its prefix (`exif:`, `dcterms:`, `schema:` or the registered extension prefix). Unknown `dcterms:` local names and undeclared prefixes MUST be rejected. Collection-domain `accrual*` terms MUST NOT be authored on a file.

UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-046](./prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports)
- [ADR 021](./architecture/021-uuid-v4-identifiers.md)
- [ADR 022](./architecture/022-dublin-core-descriptive-metadata.md)
- [DCMI Metadata Terms](https://www.dublincore.org/specifications/dublin-core/dcmi-terms/)
- [architecture.md](./architecture.md#41-data-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`media_metadata_inspection_correction_and_privacy_e_F-046.feature`](../apps/filesquid/ui/tests/features/media_metadata_inspection_correction_and_privacy_e_F-046.feature)

### SRS-F-047: Institutional DAM and IIIF interoperability

The system MUST implement [PRD F-047](./prd.md#f-047-institutional-dam-and-iiif-interoperability) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform (modules and hosts).

**Source**
- [F-047](./prd.md#f-047-institutional-dam-and-iiif-interoperability)
- [architecture.md](./architecture.md#33-backend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`institutional_dam_and_iiif_interoperability_F-047.feature`](../apps/filesquid/crates/server/tests/features/institutional_dam_and_iiif_interoperability_F-047.feature)

### SRS-F-048: Later hosted private OSS inference

The system MUST implement [PRD F-048](./prd.md#f-048-later-hosted-private-oss-inference) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform (modules and hosts).

**Source**
- [F-048](./prd.md#f-048-later-hosted-private-oss-inference)
- [architecture.md](./architecture.md#33-backend-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`later_hosted_private_oss_inference_F-048.feature`](../apps/filesquid/crates/server/tests/features/later_hosted_private_oss_inference_F-048.feature)

### SRS-F-049: Product component terminology and item type

The system MUST implement [PRD F-049](./prd.md#f-049-product-component-terminology-and-item-type) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table: the FileSquid default UI MUST use the internal names, and the ImageSquid distribution MUST use repo, library, album, gallery and exhibit. The item type in data and APIs MUST be **file**.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-049](./prd.md#f-049-product-component-terminology-and-item-type)
- [architecture.md](./architecture.md#41-data-architecture)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)

**Tests**
- [`product_component_terminology_and_item_type_F-049.feature`](../apps/filesquid/ui/tests/features/product_component_terminology_and_item_type_F-049.feature)
- [`opaque_identifiers_CORE-S-001.feature`](../apps/filesquid/crates/core/tests/features/opaque_identifiers_CORE-S-001.feature)
- [`item_type_is_file_CORE-S-002.feature`](../apps/filesquid/crates/core/tests/features/item_type_is_file_CORE-S-002.feature)
- [`library_domain_commands_and_queries_API-S-019.feature`](../apps/filesquid/crates/api/tests/features/library_domain_commands_and_queries_API-S-019.feature)

### SRS-F-050: FileSquid platform and distributions

The system MUST implement [PRD F-050](./prd.md#f-050-filesquid-platform-and-distributions) with the acceptance criteria stated there.
- FileSquid MUST be one product with the `ocfl` and `metadata` modules (crates `filesquid-ocfl`, `filesquid-metadata`) and, from the ImageSquid release, the `c2pa` module (crate `filesquid-c2pa`), built on `filesquid-core` and exposed through `filesquid-api`.
- After the FileSquid MVP, encrypted backup and sync at `*.filesquid.tech` MUST be the next release and the primary product. Day-to-day file work MUST stay in the Drive. OCFL compliance MUST stay behind that Drive. Ubuntu (GNOME) and Windows Tauri desktop releases start when the MVP closes and run beside that backup release. They MUST NOT wait on it, and the backup release MUST NOT wait on them. The rest of the first consumer release MUST follow that backup release: the default distribution, the `ocfl` and `metadata` modules, and the `filesquid-server` host, including accounts, groups, sharing, signing and published crates. Encrypted backup and sync MUST precede the ImageSquid release. ImageSquid MUST be the second consumer release, delivered as a FileSquid distribution, and the `c2pa` module MUST ship with it ([C-025](./product-vision.md#release-sequence)).
- After the ImageSquid release, the owner MUST choose the order of the Arkrim distribution, IIIF/GLAM interoperability and ProofMode integration. Hosted private OSS inference ([SRS-F-048](#srs-f-048-later-hosted-private-oss-inference)) MUST remain a separate later optional hosted service after verified encrypted preservation ([C-052](./product-vision.md#release-sequence)).
- Every distribution MUST reuse FileSquid through the FileSquid API. A distribution MUST NOT introduce a third application language or a backend of its own ([C-026](./product-vision.md#release-sequence)).
- FileSquid MUST upload, store and give access to any file type. Unknown types MUST remain exact-byte preservable with disclosed interpretation limits. FileSquid MUST NOT promise a working viewer, editor or metadata mapper for every format, and a FileSquid release MUST NOT wait for one ([C-027](./product-vision.md#release-sequence), [FileSquid U-15](./product-vision.md#platform-ownership-and-access)).
- The FileSquid default UI MUST use the internal names (repository, library, collection, virtual collection, virtual exhibit, file) ([SRS-F-049](#srs-f-049-product-component-terminology-and-item-type)).
- Every FileSquid command-line tool MUST be invoked as `fsquid` ([C-057](./product-vision.md#module-rules), [ADR 036](./architecture/036-single-fsquid-cli.md)). The `filesquid-cli` crate (`apps/filesquid/crates/cli/`) MUST build the only `fsquid` binary, with every module's commands, by routing to each module library's command runner; no other workspace crate except the `filesquid-desktop` and `filesquid-server` hosts MAY declare a binary. Routing: `fsquid <command>` when exactly one module defines the command and it is not qualifier-only; `fsquid <module> <command>` always; `fsquid <module>` prints that module's help; `validate` is qualifier-only and `fsquid validate` MUST fail with exit code 2 and name the grouped forms. A test MUST fail when two modules define the same command name that is not qualifier-only, when a module name equals a command name, or when another CLI binary name is declared.
- No crate or package under `apps/filesquid/` MAY import code from a distribution directory (for example `apps/imagesquid/`). `apps/filesquid/` MUST stay extractable to its own repository. Each distribution MUST be extractable to its own repository and MUST consume FileSquid only through the published API and UI package.

**Source**
- [F-050](./prd.md#f-050-filesquid-platform-and-distributions)
- [C-025](./product-vision.md#release-sequence), [C-026](./product-vision.md#release-sequence), [C-027](./product-vision.md#release-sequence), [C-052](./product-vision.md#release-sequence)
- [FileSquid U-15](./product-vision.md#platform-ownership-and-access), [FE-063](./product-vision.md#institutional-interoperability)
- [architecture.md](./architecture.md#12-scope)
- [ADR 029](./architecture/029-filesquid-unified-product-and-distributions.md)

**Tests**
- [`later_filesquid_records_platform_F-050.feature`](../apps/filesquid/crates/server/tests/features/later_filesquid_records_platform_F-050.feature)
- [`portable_contracts_isolation_CORE-S-003.feature`](../apps/filesquid/crates/core/tests/features/portable_contracts_isolation_CORE-S-003.feature)
- [`single_fsquid_cli_F-050.feature`](../apps/filesquid/crates/cli/tests/features/single_fsquid_cli_F-050.feature)

### SRS-F-051: OCFL module

The system MUST implement [PRD F-051](./prd.md#f-051-ocfl-module) with the acceptance criteria stated there. The module is the crate `filesquid-ocfl`; its own SRS is [apps/filesquid/crates/ocfl/resources/srs.md](../apps/filesquid/crates/ocfl/resources/srs.md).

**Engine API**
- The public API MUST be generic OCFL 1.1: storage-root open, object discovery, NAMASTE/inventory/digest-sidecar load, SHA-256, structural validation, forward-delta write, and OCFL community extensions where adopted. It MUST NOT contain FileSquid application types (catalog, recipe, C2PA, collection-snapshot, SQLite) ([C-044](./product-vision.md#module-rules)).
- FileSquid's file-object layout, collection snapshots, dual-profile encryption, intent recording and catalog rebuild MUST be callers of that API in FileSquid's application layer (`filesquid-api` and later modules) ([C-044](./product-vision.md#module-rules), [ADR 010](./architecture/010-ocfl-1-1-dual-profile.md)).
- Logical state MUST be resolved through inventory `state` and `manifest` at a caller-pinned object version. Unchanged bytes stored in older version directories MUST appear in that state. Directory-name inference MUST NOT be the resolver ([T-039](./product-vision.md#canonical-ocfl-state-and-evidence)).
- The engine MUST stream logical files in bounded chunks. No API MAY need a whole object in memory. It MUST enumerate committed logical state (object id, version, logical path, digest, content locator).
- The engine MUST store opaque caller-supplied bytes and MUST NOT require plaintext.

**Storage and concurrency**
- All byte access MUST go through one async, runtime-agnostic storage backend interface with local filesystem (native), S3-compatible (native and `wasm32`) and in-memory backends.
- A write MUST reserve `vN` exclusively before any content (local lease or fenced S3 reservation), MUST write the root inventory sidecar last, MUST treat an incomplete `vN` as unpublished, and MUST reconcile cleanly after a crash. S3 heads MUST use compare-and-swap.
- A second writer MUST NOT upload into the same `vN` prefix. A writer that fails to reserve, or whose base `head` is no longer current, MUST rebase or report a conflict and MUST NOT publish.
- Where only the writer checks its fencing token, a writer that stalls past its safety margin can still make one late write; this known gap is specified in OCFL-S-018 of the [module SRS](../apps/filesquid/crates/ocfl/resources/srs.md). Before a library can be written by more than one host process or by more than one user, the storage, or the service that performs or authorises each write in the same atomic step, MUST refuse writes and deletes whose fencing token is no longer current. Until then every validator error, E023 included, MUST reach health and the Monitor ([SRS-F-061](#srs-f-061-activity-journal-and-live-health-monitoring)).
- A committed head without its own version inventory (W010) MUST NOT be the base of a commit. A one-time repair that copies the root inventory and its sidecar into the head version directory MUST run under the object's exclusive, fenced reservation, MUST be requested only by the desktop owner or the owner of that library on a server, MUST run only after that owner consents and MUST be journaled (OCFL-S-020 of the module SRS, [API-S-023](../apps/filesquid/crates/api/resources/srs.md)). A repaired sha256 object becomes editable; a repaired sha512 object is still refused for commits.
- A `.git` directory MUST NOT appear in an OCFL object root or storage root. Git MUST NOT be the engine.

**Change model**
- The library MUST expose stage, status, diff (staged against `head`, and between any two versions), commit, log, checkout, revert and reinstate as a documented public API. Each commit MUST create exactly one new OCFL version.
- Given a working state and a base version, the engine MUST compute the change set by digest (added, changed, renamed, deleted, reinstated) and MUST write only bytes whose digest is not already in the object's manifest.
- Revert and reinstate MUST create a new version and MUST NOT modify earlier versions. Staging and checkout MUST NOT write inside the storage root.
- Native builds, WebAssembly builds and every version-producing API operation MUST use this change model.
- The OCFL commands of `fsquid` (stage, status, diff, commit, log, checkout, revert, reinstate, list, cat, and `fsquid ocfl validate`) MUST use only the public API.

**Placement and dependencies**
- The crate MUST run natively in `filesquid-desktop` and `filesquid-server`, and MUST build for `wasm32-unknown-unknown` for the encrypted browser host. On the ciphertext-only `*.filesquid.tech` service it MUST NOT process private content. Browser storage MUST NOT be a canonical root ([C-053](./product-vision.md#module-rules)).
- Default features MUST exclude Apache Arrow, Apache Parquet, DuckDB, Iceberg, Axum, Tauri, libvips, FFmpeg and SQLite. A `parquet` feature (default off) MAY project through the enumerator to a caller-chosen location. Analytical outputs MUST NOT be written into OCFL reserved paths ([C-045](./product-vision.md#module-rules), [C-046](./product-vision.md#module-rules), [T-041](./product-vision.md#canonical-ocfl-state-and-evidence)).
- The crate MUST NOT take a default dependency on another module crate. It MUST build and test alone (`cargo test -p filesquid-ocfl`) ([T-052](./product-vision.md#api-and-hosts)).

**Independence and release**
- `filesquid-ocfl` MUST stay installable and usable on its own as a FOSS library under the MIT license, with its commands in the `fsquid` CLI, without the FileSquid app. It MUST be independently publishable to crates.io without other FileSquid crates, and callers outside the workspace MUST pin published versions ([C-047](./product-vision.md#module-rules), [T-042](./product-vision.md#canonical-ocfl-state-and-evidence)).
- FileSquid hosts MUST link the crate. They MUST NOT shell out to `fsquid` for canonical OCFL I/O ([T-043](./product-vision.md#canonical-ocfl-state-and-evidence)).
- Each FileSquid packaged release MUST declare the exact `filesquid-ocfl` version in its lockfile and release notes ([T-040](./product-vision.md#canonical-ocfl-state-and-evidence)).

**Conformance**
- The engine MUST pass the pinned OCFL 1.1 community fixture suite as specified in OCFL-S-009 (validator conformance), OCFL-S-010 (read and write conformance) and OCFL-S-011 (fixture pinning and handling) of the [module SRS](../apps/filesquid/crates/ocfl/resources/srs.md). Community fixtures and validators (`rocfl`, `ocfl-py`) are test oracles only. They MUST NOT be the writer, a bundled sidecar or a server dependency.

**Source**
- [F-051](./prd.md#f-051-ocfl-module)
- [FileSquid U-40–U-53](./product-vision.md#ocfl-module), [FE-069](./product-vision.md#independent-library)
- [C-044](./product-vision.md#module-rules), [C-045](./product-vision.md#module-rules), [C-046](./product-vision.md#module-rules), [C-047](./product-vision.md#module-rules), [C-053](./product-vision.md#module-rules)
- [T-039–T-043](./product-vision.md#canonical-ocfl-state-and-evidence), [T-052](./product-vision.md#api-and-hosts)
- [ADR 016](./architecture/016-rust-ocfl-library.md), [ADR 023](./architecture/023-filesquid-ocfl-engine-and-parquet.md), [ADR 025](./architecture/025-client-side-filesquid-engines.md) (amended by [ADR 032](./architecture/032-hosting-and-privacy-model.md)), [ADR 029](./architecture/029-filesquid-unified-product-and-distributions.md)
- [architecture.md](./architecture.md#42-database-decisions)

**Tests**
- [`reusable_ocfl_engine_F-051.feature`](../apps/filesquid/crates/ocfl/tests/features/reusable_ocfl_engine_F-051.feature)
- [`OCFL_1.1_fixture_conformance.feature`](../apps/filesquid/crates/ocfl/tests/features/OCFL_1.1_fixture_conformance.feature)
- [`ocfl_preservation_and_portable_recovery_F-042.feature`](../apps/filesquid/ui/tests/features/ocfl_preservation_and_portable_recovery_F-042.feature)

### SRS-F-052: Metadata module

The system MUST implement [PRD F-052](./prd.md#f-052-metadata-module) with the acceptance criteria stated there. The module is the crate `filesquid-metadata`; its own SRS is [apps/filesquid/crates/metadata/resources/srs.md](../apps/filesquid/crates/metadata/resources/srs.md).

**Extraction and authoring**
- The module MUST extract supported embedded metadata (EXIF, ID3, XMP and other supported packets; IPTC when supported) from bytes or a reader, without a subprocess or filesystem path. Unknown formats MUST return an explicit unsupported result. Descriptive facts MUST map to `dcterms:` or `schema:`; native observations keep their prefix ([T-044](./product-vision.md#canonical-ocfl-state-and-evidence)).
- It MUST create, read, update and delete namespaced statements `{namespace, term, value}` (`exif:`, `dcterms:`, `schema:` and registered extension namespaces) without FileSquid catalog, recipe, C2PA, collection-snapshot or UI types.
- Owner-authored descriptive statements MUST use the closed ISO 15836-2 `/terms/` register, confirmed Schema.org overflow, or a registered extension namespace. Unknown `dcterms:` local names MUST be rejected. The fifteen `dc:` names MUST map to `dcterms:` on write. Collection-domain `accrual*` terms MUST NOT be authored on a file ([T-038](./product-vision.md#descriptive-metadata)).
- The module MUST keep a namespace registry of extension namespaces, each with a prefix, an IRI and an optional term list (for example RiC-O), registered by an owner, a distribution or a module; an owner MAY declare a custom namespace whose keys are free key:value pairs. A registration MUST NOT redefine `dcterms:`, `dc:`, `schema:` or `exif:`. An extension prefix MUST be unique within a repository and map to exactly one IRI; re-registering an existing prefix with a different IRI MUST be rejected. A namespace with a term list MUST reject unknown terms. A statement whose prefix is not registered MUST be rejected with an error naming the prefix. Extension statements MUST NOT be mapped onto `dcterms:`, MUST round-trip unchanged through `metadata.md`, CSV, JSON and XMP, and MUST obey the outgoing disclosure policy ([SRS-F-046](#srs-f-046-media-metadata-inspection-correction-and-privacy-exports)). Registrations MUST be stored in canonical OCFL state so a rebuilt catalog recovers them ([C-043](./product-vision.md#provenance-metadata-and-disclosure), [FileSquid U-55](./product-vision.md#metadata-module)).
- Schema.org overflow MUST be suggested as the closest term and stored only after confirmation. Close matches MUST NOT silently alias ([C-043](./product-vision.md#provenance-metadata-and-disclosure)).

**Records and stores**
- The canonical record MUST be `metadata.md`: YAML front matter grouped by namespace map (`dcterms:`, `exif:`, `schema:`, and one map per registered extension namespace) plus an optional Markdown body ([T-046](./product-vision.md#canonical-ocfl-state-and-evidence)).
- Persistence MUST go through a pluggable store interface: `sidecar` (native only), `ocfl` (feature; native and WebAssembly; writes `metadata.md` through `filesquid-ocfl` so each edit is a new OCFL version), `sqlite` (feature, native only), or a caller's own store. The core MUST NOT depend on a particular store ([ADR 027](./architecture/027-filesquid-metadata-pluggable-stores.md)).
- FileSquid, and therefore every distribution, MUST use the `ocfl` store as canonical. The crate MUST NOT reimplement OCFL layout or `vN` publication. FileSquid MUST NOT keep a parallel metadata extract or write path, and MUST NOT shell out to `fsquid` for canonical writes ([C-048](./product-vision.md#module-rules)).
- An optional SQLite index MUST be rebuildable from the canonical store and MUST NOT be the only copy, except when the caller explicitly chooses the `sqlite` store. Other indexes (PostgreSQL, Elasticsearch) MUST also be rebuildable from `metadata.md`.

**Interchange and CLI**
- CSV import/export MUST use one row per file, `prefix:term` headers and RFC 4180 quoting, and MUST reject unknown `dcterms:` columns and columns whose prefix is not registered.
- XMP sidecar read/write MUST ship with CSV.
- JSON sidecar import/export MUST ship with CSV and XMP: one UTF-8 JSON document per OCFL object that binds statements to the OCFL object identifier and the SHA-256 digest of the described logical file, exposed as `export json` and `import json`. Import MUST apply the `/terms/` register checks and MUST reject a sidecar whose digest does not match the target file. A JSON sidecar MUST NOT replace `metadata.md` as canonical.
- Interchange MUST NOT include MODS or SQL ([C-049](./product-vision.md#module-rules)).
- The crate's commands `extract`, `get`, `set`, `export`, `import`, `namespace` and `fsquid metadata validate` MUST be available in the native `fsquid` CLI, usable without the FileSquid app; the crate ships no binary of its own ([T-045](./product-vision.md#canonical-ocfl-state-and-evidence)).

**Properties API and panel**
- The module MUST provide a properties API: populated key/value rows with prefixes, and an Add property search across `/terms/`, then Schema.org, then registered extension namespaces, with an option to add a key in a declared custom namespace. FileSquid MUST expose it through the FileSquid API and present it in the Metadata panel ([SRS-F-055](#srs-f-055-default-ui-and-panel-composition), [SRS-F-046](#srs-f-046-media-metadata-inspection-correction-and-privacy-exports)).

**Placement, dependencies and independence**
- The crate MUST run natively in `filesquid-desktop` and `filesquid-server`, and MUST build for `wasm32-unknown-unknown`. Extract adapters MUST read bytes supplied by the store, not filesystem paths. The `fsquid` command logic and file-path helpers are native-only. On the ciphertext-only `*.filesquid.tech` service it MUST NOT be linked or process private content ([C-053](./product-vision.md#module-rules)).
- Default dependencies MUST exclude Axum, Tauri, SQLite, distributions and other module crates (except the optional `ocfl` store). Format extract adapters belong to this crate and MUST NOT pull native media processing. The crate MUST build and test alone ([T-052](./product-vision.md#api-and-hosts)).
- `filesquid-metadata` MUST stay installable as a FOSS library under the MIT license, with its commands in the `fsquid` CLI, without the FileSquid app, and MUST be independently publishable to crates.io.

**Source**
- [F-052](./prd.md#f-052-metadata-module)
- [FileSquid U-54–U-66](./product-vision.md#metadata-module), [FE-068](./product-vision.md#metadata-ownership-and-sharing-privacy), [FE-070](./product-vision.md#independent-library)
- [C-043](./product-vision.md#provenance-metadata-and-disclosure), [C-048](./product-vision.md#module-rules), [C-049](./product-vision.md#module-rules), [C-053](./product-vision.md#module-rules)
- [T-038](./product-vision.md#descriptive-metadata), [T-044–T-046](./product-vision.md#canonical-ocfl-state-and-evidence), [T-052](./product-vision.md#api-and-hosts)
- [ADR 022](./architecture/022-dublin-core-descriptive-metadata.md), [ADR 024](./architecture/024-filesquid-metadata-engine.md), [ADR 025](./architecture/025-client-side-filesquid-engines.md) (amended by [ADR 032](./architecture/032-hosting-and-privacy-model.md)), [ADR 027](./architecture/027-filesquid-metadata-pluggable-stores.md), [ADR 029](./architecture/029-filesquid-unified-product-and-distributions.md)
- [architecture.md](./architecture.md#31-logical-architecture)

**Tests**
- [`reusable_metadata_engine_F-052.feature`](../apps/filesquid/crates/metadata/tests/features/reusable_metadata_engine_F-052.feature)
- [`media_metadata_inspection_correction_and_privacy_e_F-046.feature`](../apps/filesquid/ui/tests/features/media_metadata_inspection_correction_and_privacy_e_F-046.feature)

### SRS-F-053: Virtual exhibit

The system MUST implement [PRD F-053](./prd.md#f-053-virtual-exhibit) with the acceptance criteria stated there. ImageSquid UI copy MUST use **exhibit** for a virtual exhibit. A virtual exhibit belongs to a user on a repository, MAY span public collections across libraries and repositories, and MUST NOT store file bytes. A cited ImageSquid screenshot binds the composition of the ImageSquid surface it shows; a pixel-exact match or screenshot hash MUST NOT be required.

**Delivered by:** FileSquid platform. Every distribution presents it through its own views and label table.

**Source**
- [F-053](./prd.md#f-053-virtual-exhibit)
- [architecture.md](./architecture.md#41-data-architecture)
- [ADR 021](./architecture/021-uuid-v4-identifiers.md)

**Tests**
- [`virtual_exhibit_F-053.feature`](../apps/filesquid/ui/tests/features/virtual_exhibit_F-053.feature)
- [`collections_and_virtual_collections_F-004.feature`](../apps/filesquid/crates/core/tests/features/collections_and_virtual_collections_F-004.feature)

### SRS-F-054: C2PA module

The system MUST implement [PRD F-054](./prd.md#f-054-c2pa-module) with the acceptance criteria stated there. The module is the crate `filesquid-c2pa` (planned); its own SRS is [apps/filesquid/crates/c2pa/resources/srs.md](../apps/filesquid/crates/c2pa/resources/srs.md).

**Read and validate**
- The module MUST read manifest stores embedded in assets, in sidecar `.c2pa` files and by remote reference, parsing JUMBF boxes and CBOR claims and assertions. Remote fetches MUST go through a caller-supplied fetcher.
- It MUST validate hard bindings (data hash, BMFF hash, box hash), COSE_Sign1 claim signatures, certificate chains against caller-supplied trust lists, RFC 3161 timestamps, available revocation status and ingredient chains. It MUST report results per manifest and ingredient with the C2PA specification's validation status codes, and MUST report untrusted or unchecked items rather than hide them.
- The crate MUST ship no implicit trust store.

**Mint, edit, embed**
- It MUST mint manifests (actions, ingredients, hard bindings, AI-generation and training/mining assertions, custom assertions) and sign them through a caller-supplied `Signer` (local keys, hardware modules or remote services). It MUST NOT operate or imply a certificate authority.
- Edits MUST create new manifests that reference the prior asset as a parent ingredient, including update manifests and assertion redaction as the specification allows. A signed manifest MUST NOT change in place.
- It MUST embed manifests through a format-handler interface, starting with JPEG and PNG and extending to HEIF/AVIF, BMFF (MP4/MOV), WebP and TIFF/DNG, or write a sidecar. Unsupported formats MUST be reported as unsupported, not silently skipped.

**Scan and report**
- It MUST provide bounded, resumable collection scans and disclosure-safe reports. A free scan MUST check disclosable AI-action history against the selected share or publication route. Scans MUST NOT auto-publish.
- An optional, default-off feature MUST project C2PA actions and ingredients to the PROV-O JSON-LD profile ([ADR 015](./architecture/015-prov-o-jsonld-and-c2pa-sdk.md)).

**Implementation rules**
- C2PA logic MUST be implemented from scratch. Cryptographic primitives MUST come from established crates behind a crypto-backend trait and MUST NOT be hand-rolled.
- The official `c2pa` crate, `c2patool` and C2PA conformance files MUST verify output and verdicts in CI. They MUST NOT be the implementation or a runtime dependency.
- The core MUST read and write through caller-supplied async byte sources and sinks. It has no server, filesystem or network access of its own.
- The module MUST pin a C2PA specification version.

**Placement, dependencies and independence**
- The crate MUST run natively in `filesquid-desktop` and `filesquid-server`, and MUST build for `wasm32-unknown-unknown`. On the ciphertext-only `*.filesquid.tech` service it MUST NOT be linked or process private content ([C-053](./product-vision.md#module-rules), [C-054](./product-vision.md#module-rules)).
- Default dependencies MUST exclude `filesquid-ocfl`, other FileSquid crates, distributions, Axum, Tauri, SQLite, libvips and FFmpeg. The crate MUST build and test alone ([T-052](./product-vision.md#api-and-hosts)).
- Its commands `read`, `verify`, `sign`, `edit`, `embed` and `report` MUST be available in the native `fsquid` CLI; the crate ships no binary of its own. It MUST stay usable without the FileSquid app under the MIT license and MUST be independently publishable to crates.io.
- FileSquid MUST expose credential summaries and validation reports through the FileSquid API and present them in the Content Credentials panel ([SRS-F-055](#srs-f-055-default-ui-and-panel-composition)).

**Source**
- [F-054](./prd.md#f-054-c2pa-module)
- [FileSquid U-67–U-78](./product-vision.md#c2pa-module), [FE-072](./product-vision.md#independent-library)
- [C-053](./product-vision.md#module-rules), [C-054](./product-vision.md#module-rules), [T-052](./product-vision.md#api-and-hosts)
- [ADR 015](./architecture/015-prov-o-jsonld-and-c2pa-sdk.md), [ADR 025](./architecture/025-client-side-filesquid-engines.md) (amended by [ADR 032](./architecture/032-hosting-and-privacy-model.md)), [ADR 026](./architecture/026-filesquid-provenance-engine.md), [ADR 029](./architecture/029-filesquid-unified-product-and-distributions.md)

**Tests**
- [`reusable_provenance_engine_F-054.feature`](../apps/filesquid/crates/c2pa/tests/features/reusable_provenance_engine_F-054.feature)
- [`c2pa_and_provo_records_F-043.feature`](../apps/filesquid/crates/core/tests/features/c2pa_and_provo_records_F-043.feature)

### SRS-F-055: Default UI and panel composition

The system MUST implement [PRD F-055](./prd.md#f-055-default-ui-and-panel-composition) with the acceptance criteria stated there.
- **Display name.** Every surface MUST show the display name served by the API and MUST NOT show a stored name or metadata-record file name ([UI-S-019](../apps/filesquid/ui/resources/srs.md), [API-S-027](../apps/filesquid/crates/api/resources/srs.md)). The file panel MUST offer three tabs of one look and behaviour: File info, Description (the Metadata panel) and Provenance ([UI-S-022](../apps/filesquid/ui/resources/srs.md), [UI-S-020](../apps/filesquid/ui/resources/srs.md)).

**File-fact ownership**
- File facts MUST have one owner. `filesquid-ocfl` MUST own storage facts only (object identifier, logical path and implied folders, versions, version `created`, `user` and `message`, digests and fixity, size, content location). An extension-based file-type label is a display hint only.
- Every other file fact (embedded EXIF/ID3/IPTC/XMP, format identification, original creation or capture dates, Dublin Core and Schema.org description, titles, captions, keywords, rights) MUST belong to `filesquid-metadata`. Content Credentials MUST belong to `filesquid-c2pa`.
- Application meaning (collections, people, favorites, search indexes, recipes) MUST belong to FileSquid's application layer and be presented by distributions, never by a module ([C-055](./product-vision.md#module-rules)).

**View operations**
- The FileSquid API MUST expose serializable view operations: folder listing by logical-path prefix at a pinned version with paging, file details, per-file version history, object versions, size, fixity, ranged reads, and version-producing writes (storage facts); the properties API (metadata); credential summary and validation report (C2PA) ([SRS-F-056](#srs-f-056-single-filesquid-api)).

**Components**
- `@filesquid/ui` (`apps/filesquid/ui`) MUST provide the components as standard custom elements written in vanilla TypeScript, with no UI framework. Components MUST contain presentation only and MUST reach data only through the FileSquid API client ([T-051](./product-vision.md#api-and-hosts)).
- The Drive-style file browser (`<fs-file-browser>`) MUST provide navigation, breadcrumb, list and grid views, preview, and upload, new folder, rename or move, remove and restore. Every operation that changes content or organisation MUST produce a new OCFL version. A new empty folder MAY show as pending in the UI until a file is added beneath it; it MUST NOT be persisted as an empty folder ([FileSquid U-01](./product-vision.md#platform-ownership-and-access)).
- The browser MUST provide named sidebar panel slots. FileSquid MUST ship the Details, Versions, Metadata (`<fs-metadata-panel>`), Content Credentials (`<fs-credentials-panel>`), Activity (`<fs-activity-panel>`) and Sharing (`<fs-sharing-panel>`) panels. In the default UI the Details and Metadata panels appear as the file panel's File info and Description tabs and Versions as a view opened from its header ([UI-S-022](../apps/filesquid/ui/resources/srs.md)). No panel MAY depend on another panel. A distribution registers the panels it wants.
- Components MUST use the design tokens of [SRS-UI-007](#srs-ui-007-light-and-dark-themes) (light and dark), MUST meet the accessibility contracts of [SRS-NFR-006](#srs-nfr-006-accessibility-and-usability), and MUST take every visible label from the distribution's replaceable label table. A missing token or label MUST fail a test.

**Default distribution and ImageSquid composition**
- The FileSquid default distribution MUST compose the file browser and the six panels (Details, Versions, Metadata, Content Credentials, Activity, Sharing) over the FileSquid API, with the internal names, the Details and Metadata panels as the File info and Description tabs of the one file panel. It MUST run unchanged on `filesquid-desktop` and `filesquid-server`, and on the encrypted browser host when that host ships.
- A distribution MUST compose these panels instead of reimplementing them.
- In the default UI the sidebar is one file panel whose named tab slots hold File info, Description (the Metadata panel) and Provenance ([UI-S-022](../apps/filesquid/ui/resources/srs.md)); the Versions view ([SRS-F-065](#srs-f-065-file-version-history-and-retention)) opens from its header; Content Credentials, Activity ([SRS-F-064](#srs-f-064-file-activity-comments-and-user-activity-stream)) and Sharing ([SRS-F-066](#srs-f-066-file-and-folder-sharing-roles-links-and-shares-view)) join as further named slots when their features ship. Views, sort keys, selection, layout preferences and item actions are specified in [SRS-F-062](#srs-f-062-drive-style-views-sorting-selection-and-layout-preferences); the search bar in [SRS-F-063](#srs-f-063-file-search-query-language-and-filter-chips).
- **Composition.** The default UI MUST match the active mockup suite ([v3](./ui-mockups/v3/README.md)): its visual constants, shell regions, views, dialogs, strings and states are recorded in [UI-S-024 to UI-S-034](../apps/filesquid/ui/resources/srs.md) (see [Mockup coverage](#mockup-coverage)).

**Source**
- [F-055](./prd.md#f-055-default-ui-and-panel-composition)
- [FE-073](./product-vision.md#independent-library), [FileSquid U-01](./product-vision.md#platform-ownership-and-access), [U-24](./product-vision.md#one-api-hosts-and-distributions), [U-65](./product-vision.md#metadata-module), [U-77](./product-vision.md#c2pa-module)
- [C-055](./product-vision.md#module-rules), [T-051](./product-vision.md#api-and-hosts)
- [ADR 028](./architecture/028-filesquid-ui-packages-and-panel-composition.md) (superseded by [ADR 031](./architecture/031-vanilla-typescript-ui-and-white-label-theming.md)), [ADR 031](./architecture/031-vanilla-typescript-ui-and-white-label-theming.md)

**Tests**
- [`composable_filesquid_ui_F-055.feature`](../apps/filesquid/ui/tests/features/composable_filesquid_ui_F-055.feature)
- [`reusable_ocfl_engine_F-051.feature`](../apps/filesquid/crates/ocfl/tests/features/reusable_ocfl_engine_F-051.feature)

### SRS-F-056: Single FileSquid API

The system MUST implement [PRD F-056](./prd.md#f-056-single-filesquid-api) with the acceptance criteria stated there.
- Every FileSquid feature MUST be reachable through one versioned FileSquid API, served by every host. A UI MUST NOT call a module, the filesystem or a database directly ([FE-074](./product-vision.md#platform-api-and-hosts), [FileSquid U-16](./product-vision.md#one-api-hosts-and-distributions)).
- `filesquid-api` MUST own the API contract: versioned operation schemas and errors in one machine-readable contract document checked into the repository (OpenAPI 3.1 is the candidate format; [ADR 030](./architecture/030-filesquid-api-and-transports.md) decides). TypeScript types MUST be generated from it. A test MUST fail when the contract and the generated types drift ([T-048](./product-vision.md#api-and-hosts)).
- API operations and payloads MUST use the internal names (repository, library, collection, virtual collection, virtual exhibit, file). The item type MUST be **file** ([FileSquid U-04](./product-vision.md#platform-ownership-and-access)).
- The API MUST return opaque handles, never raw filesystem paths or SQL ([FileSquid U-18](./product-vision.md#one-api-hosts-and-distributions)).
- Every API call MUST be authorised before files, SQL or keys are touched. Access MUST be deny-by-default. The only exception is the public liveness probe `GET /api/v1/health/live`, which says only that the process is up ([API-S-015](../apps/filesquid/crates/api/resources/srs.md)).
- File content MUST be readable with byte ranges, for previews and downloads. Uploads MUST stream in bounded chunks.
- Media elements, which cannot send an `Authorization` header, MUST read file bytes through short-lived, read-only content handles for one file version, minted by an authenticated client under the same authorisation as a read of that file. A content handle is not a token: tokens MUST NOT appear in URLs, and a handle MUST expire on its own, end with the credential that minted it, and never be logged ([API-S-018](../apps/filesquid/crates/api/resources/srs.md), [ADR 030](./architecture/030-filesquid-api-and-transports.md) Decision 5).
- The API MUST provide an event channel for job progress and change notifications ([FileSquid U-17](./product-vision.md#one-api-hosts-and-distributions)).
- HTTP and WebSocket transports MUST be evaluated against preview and ranged reads, uploads, push events, the no-TCP-port desktop rule and Service Worker interception for the encrypted browser host. The outcome MUST be recorded in [ADR 030](./architecture/030-filesquid-api-and-transports.md) before the API contract is declared stable ([T-050](./product-vision.md#api-and-hosts)).
- The TypeScript client in `@filesquid/ui` MUST offer one interface (`request()`, `read()`, `upload()`, `subscribe()`) that hides which transport answered.
- A failed request MUST return an error the UI can show. A host MUST NOT crash on a failed request ([FileSquid U-26](./product-vision.md#one-api-hosts-and-distributions)).
- Every module capability, including later modules, MUST be exposed through this API and MUST be switchable per distribution ([FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)).
- A distribution MUST NOT add an API of its own ([FE-077](./product-vision.md#platform-api-and-hosts)).

**Source**
- [F-056](./prd.md#f-056-single-filesquid-api)
- [FE-074](./product-vision.md#platform-api-and-hosts)
- [FileSquid U-04](./product-vision.md#platform-ownership-and-access), [U-16, U-17, U-18, U-25, U-26](./product-vision.md#one-api-hosts-and-distributions)
- [T-048](./product-vision.md#api-and-hosts), [T-050](./product-vision.md#api-and-hosts)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md), [ADR 030](./architecture/030-filesquid-api-and-transports.md)

**Tests**
- [`command_query_ports_F-038.feature`](../apps/filesquid/crates/core/tests/features/command_query_ports_F-038.feature)
- [`health.feature`](../apps/filesquid/crates/server/tests/features/health.feature)
- [`versioned_api_contract_F-056.feature`](../apps/filesquid/crates/api/tests/features/versioned_api_contract_F-056.feature) (versioned contract, typed errors); contract drift is covered by [`contract.rs`](../apps/filesquid/crates/api/tests/unit/contract.rs) and the Vitest [`contract.test.ts`](../apps/filesquid/ui/tests/unit/contract.test.ts)
- [`event_channel_F-056.feature`](../apps/filesquid/crates/api/tests/features/event_channel_F-056.feature) (transport-agnostic routes, bounded event channel, resync hints)
- [`deny_by_default_authorisation_F-056.feature`](../apps/filesquid/crates/api/tests/features/deny_by_default_authorisation_F-056.feature) (every route except the public liveness probe, every fallback and every host route refuses a call without an accepted credential; typed, journaled denials); every route is also covered by [`auth.rs`](../apps/filesquid/crates/api/tests/unit/auth.rs), the server's [`auth.rs`](../apps/filesquid/crates/server/tests/integration/auth.rs) and the desktop's [`api_protocol.rs`](../apps/filesquid/crates/desktop/tests/integration/api_protocol.rs)
- [`content_handles_F-056.feature`](../apps/filesquid/crates/api/tests/features/content_handles_F-056.feature) (content handles for media elements); every API-S-018 clause is also covered through the router by [`content_handles.rs`](../apps/filesquid/crates/api/tests/integration/content_handles.rs); the desktop scheme's pass-through of content answers is covered by the desktop's [`api_protocol.rs`](../apps/filesquid/crates/desktop/tests/integration/api_protocol.rs), and [`content-handles.e2e.mjs`](../apps/filesquid/crates/desktop/tests/e2e/specs/content-handles.e2e.mjs) is smoke coverage in a packaged desktop window (minting, ranged and `HEAD` reads, the denial, `<img>` and, outside WebKitGTK, `<video>` playback)
- [`file_operation_routes_API-S-008.feature`](../apps/filesquid/crates/api/tests/features/file_operation_routes_API-S-008.feature) (file operation routes: listing, ranged reads, upload, pending folders, move, remove and restore, one OCFL version per write); also [`files.rs`](../apps/filesquid/crates/api/tests/integration/files.rs) against a real OCFL root

### SRS-F-057: Desktop and server hosts

The system MUST implement [PRD F-057](./prd.md#f-057-desktop-and-server-hosts) with the acceptance criteria stated there. Both hosts MUST link `filesquid-api`. One host binary MUST serve any distribution. The MVP desktop is macOS. Ubuntu (GNOME) and Windows Tauri desktop releases start when the MVP closes and run beside encrypted backup. `filesquid-server` follows the backup release. Desktop and `filesquid-server` delivery and validation MUST come before the encrypted browser host ([FE-017](./product-vision.md#cross-platform-access)). Both hosts MUST write unbuffered structured logs to stdout ([SRS-NFR-009](#srs-nfr-009-observability-and-capacity-economics)).

**Desktop host (`filesquid-desktop`, Tauri 2)**
- The host MUST serve the API in-process through a Tauri custom URI scheme (`filesquid://localhost/api/...`; `http://filesquid.localhost/api/...` on Windows). It MUST NOT open a TCP listener unless [ADR 030](./architecture/030-filesquid-api-and-transports.md) records otherwise ([T-049](./product-vision.md#api-and-hosts)).
- Native file access, workers and OS adapters MUST stay out of the webview.
- Native decode, FFmpeg and libvips MUST run in out-of-process workers shipped in the installer, so a decoder crash cannot take down the catalog or UI ([FE-064](./product-vision.md#desktop-host-isolation)).
- Installers MUST be per OS (macOS for the MVP, then Ubuntu on GNOME and Windows) and MUST bundle the native libraries. An unsigned, not-notarized macOS installer is permitted only for the early-access MVP ([C-056](./product-vision.md#release-sequence)) and MUST be labelled as such. The product MUST NOT require ImageMagick or FFmpeg on PATH.

**Server host (`filesquid-server`, Axum)**
- The host MUST serve the API and the selected distribution's UI static files from one process. The bind address MUST come from `FILESQUID_BIND` (default `127.0.0.1:8083`). The UI directory MUST come from `FILESQUID_UI_DIR`. The host MUST NOT server-render pages ([FileSquid U-19](./product-vision.md#one-api-hosts-and-distributions)).
- Storage MUST be a local filesystem or S3-compatible OCFL root.
- The host MUST be multi-user. It MUST authenticate users and enforce group and collection permissions on every API call. It MUST support personal and group-owned libraries. Group membership MUST NOT grant ownership of another member's libraries. Billing administration MUST grant no access to private libraries ([FileSquid U-06](./product-vision.md#platform-ownership-and-access), [T-053](./product-vision.md#api-and-hosts)).
- Operational data MUST live in a store that is rebuildable from OCFL or holds operational data only.
- `filesquid-server` MUST NOT offer operator-key encryption at rest. At-rest protection of its local or S3-compatible storage is the operator's disk or bucket encryption, outside FileSquid, and the privacy boundary statement MUST say so; an administrator recovery key for users' content MUST NOT exist ([PRD F-057](./prd.md#f-057-desktop-and-server-hosts), [SRS-F-035](#srs-f-035-optional-service-extensions)).
- Sharing (new shares), public links, re-sharing and download MUST be switchable per server by an administrator, journaled and reflected in the UI ([SRS-F-066](#srs-f-066-file-and-folder-sharing-roles-links-and-shares-view)).

**Privacy boundary per host**
- Each host MUST state its privacy boundary in the UI: desktop, the owner's device only; a self-run `filesquid-server`, that server's operator can read files; encrypted hosting, ciphertext only and the operator holds no keys. The UI MUST show which host the user is connected to ([FileSquid U-20](./product-vision.md#one-api-hosts-and-distributions)).
- The API MUST expose a host descriptor to every authenticated caller: the host type (desktop, server or browser), the operator's name on a server, and the state of encrypted backup and sync to the ciphertext-only hosted service (enrolled, backup on, sync on, service host), which a host without enrolment reports as not enrolled. It MUST publish a change event when any of these change, and MUST carry no account, device, key or content detail. The UI MUST take these facts from the descriptor and MUST NOT infer them ([API-S-020](../apps/filesquid/crates/api/resources/srs.md), [UI-S-018](../apps/filesquid/ui/resources/srs.md), [ADR 032](./architecture/032-hosting-and-privacy-model.md) Decision 7).
- Private-library processing MUST default to authorized endpoints: the desktop host, a `filesquid-server` that the owner or their organisation operates, or the encrypted browser host ([T-001](./product-vision.md#client-processing-and-hosted-mailbox)).

**Encrypted browser host (roadmap)**
- When it ships, the browser host MUST run the FileSquid modules as WebAssembly against the encrypted hosted OCFL root, behind the same API. The WASM graph MUST NOT depend on native processing and MUST NOT be treated as providing a local-disk OCFL root, libvips, FFmpeg or PhotoKit. Native Rust, WASM and WebCrypto MUST share matching test vectors for SHA-256, AES-256-GCM, HKDF, Ed25519, X25519 and Argon2id ([FileSquid U-22, U-23](./product-vision.md#one-api-hosts-and-distributions)).

**Source**
- [F-057](./prd.md#f-057-desktop-and-server-hosts)
- [FE-017](./product-vision.md#cross-platform-access), [FE-064](./product-vision.md#desktop-host-isolation), [FE-075](./product-vision.md#platform-api-and-hosts)
- [T-001, T-002](./product-vision.md#client-processing-and-hosted-mailbox)
- [FileSquid U-06](./product-vision.md#platform-ownership-and-access), [U-18–U-23, U-26, U-34](./product-vision.md#one-api-hosts-and-distributions)
- [T-049](./product-vision.md#api-and-hosts), [T-053](./product-vision.md#api-and-hosts)
- [ADR 002](./architecture/002-tauri2-desktop-shell.md), [ADR 030](./architecture/030-filesquid-api-and-transports.md), [ADR 032](./architecture/032-hosting-and-privacy-model.md)

**Tests**
- [`health.feature`](../apps/filesquid/crates/server/tests/features/health.feature)
- [`desktop_transport_F-057.feature`](../apps/filesquid/crates/desktop/tests/features/desktop_transport_F-057.feature)
- [`shared_health_status_CORE-S-004.feature`](../apps/filesquid/crates/core/tests/features/shared_health_status_CORE-S-004.feature)
- [`setup_updates_and_diagnostics_F-037.feature`](../apps/filesquid/ui/tests/features/setup_updates_and_diagnostics_F-037.feature)
- [`host_descriptor_API-S-020.feature`](../apps/filesquid/crates/api/tests/features/host_descriptor_API-S-020.feature) (host type, operator name and hosted backup and sync state from the API)
- [`host_privacy_indicator_UI-S-018.feature`](../apps/filesquid/ui/tests/features/host_privacy_indicator_UI-S-018.feature) (privacy statement per host, hosted service shown apart)
- Test gap: a dedicated `desktop_and_server_hosts_F-057.feature` (no TCP listener on desktop, multi-user permissions, privacy boundary text) is not yet written.

### SRS-F-058: White-label distributions

The system MUST implement [PRD F-058](./prd.md#f-058-white-label-distributions) with the acceptance criteria stated there.
- A distribution MUST be defined by view modules, design tokens (light and dark, [SRS-UI-007](#srs-ui-007-light-and-dark-themes)), a label table keyed by internal names, brand assets (logo, icons, product name) and a module set ([FE-076](./product-vision.md#platform-api-and-hosts), [FileSquid U-24](./product-vision.md#one-api-hosts-and-distributions)).
- Each distribution MUST declare itself in a `distribution.json` manifest: name, bundle identifier, enabled modules, default UI path and Tauri config overlay.
- Every capability a distribution needs (for example media derivatives and editing recipes for ImageSquid; curation jobs and Parquet for Arkrim) MUST be a FileSquid module behind the API, enabled per distribution by its manifest. A FileSquid user MUST be able to enable such a module without installing that distribution ([FE-077](./product-vision.md#platform-api-and-hosts), [FileSquid U-25](./product-vision.md#one-api-hosts-and-distributions)).
- The desktop installer for a distribution MUST be built from the one `filesquid-desktop` binary with a Tauri config overlay that sets the product name, identifier, icons and frontend directory. `filesquid-server` MUST select the distribution's UI directory from configuration.
- A missing token or label in a distribution MUST fail a test.
- The Arkrim distribution MUST use the internal names and archival terms.
- A distribution's label table or token file that lacks a key present in the FileSquid default MUST fail a test.
- A distribution MUST run on every FileSquid host through the same API ([SRS-F-057](#srs-f-057-desktop-and-server-hosts)).
- `@imagesquid/ui` MAY depend on `@filesquid/ui`. `@filesquid/ui` MUST NOT depend on any distribution package.

**Source**
- [F-058](./prd.md#f-058-white-label-distributions)
- [FE-076](./product-vision.md#platform-api-and-hosts), [FE-077](./product-vision.md#platform-api-and-hosts)
- [FileSquid U-24, U-25](./product-vision.md#one-api-hosts-and-distributions), [C-026](./product-vision.md#release-sequence), [T-051](./product-vision.md#api-and-hosts)
- [ADR 029](./architecture/029-filesquid-unified-product-and-distributions.md), [ADR 031](./architecture/031-vanilla-typescript-ui-and-white-label-theming.md)

**Tests**
- [`theme_tokens_SRS-UI-007.feature`](../apps/filesquid/ui/tests/features/theme_tokens_SRS-UI-007.feature)
- [`product_component_terminology_and_item_type_F-049.feature`](../apps/filesquid/ui/tests/features/product_component_terminology_and_item_type_F-049.feature)
- [`composable_filesquid_ui_F-055.feature`](../apps/filesquid/ui/tests/features/composable_filesquid_ui_F-055.feature)
- Test gap: a dedicated `white_label_distributions_F-058.feature` (manifest, overlay build, no backend code in a distribution) is not yet written.

### SRS-F-059: Hosted encrypted backup and sync service

The system MUST implement [PRD F-059](./prd.md#f-059-hosted-encrypted-backup-and-sync-service) with the acceptance criteria stated there. This block specifies the hosted service; client behaviour is in [SRS-F-029](#srs-f-029-client-encryption-and-key-custody), [SRS-F-030](#srs-f-030-personal-synchronization-and-availability), [SRS-F-031](#srs-f-031-encrypted-backup-and-verified-restore) and [SRS-F-032](#srs-f-032-accounts-entitlements-billing-and-cancellation).
- FileSquid MUST offer paid encrypted backup and sync at `*.filesquid.tech` immediately after the MVP. Ubuntu (GNOME) and Windows Tauri desktop releases run beside it and MUST NOT gate it. The rest of the first consumer release, and the ImageSquid release, follow it ([FE-078](./product-vision.md#platform-api-and-hosts), [C-025](./product-vision.md#release-sequence)).
- The service MUST store and relay ciphertext only. It MUST NOT hold library keys or private plaintext. It MUST NOT parse OCFL. It MUST authorise keys and fence OCFL head publication ([T-054](./product-vision.md#api-and-hosts)). An upload into a Drive MUST be authorised only for a device key on that node's whitelist. Several devices MAY be on that whitelist ([SRS-F-029](#srs-f-029-client-encryption-and-key-custody)). A password MUST NOT be that permission. The service MUST NOT read an Automerge document ([SRS-F-030](#srs-f-030-personal-synchronization-and-availability)).
- Each Drive MUST have its own backup on this service, and that backup MUST keep multiple geo-distributed copies of the Drive's recovery points ([SRS-F-031](#srs-f-031-encrypted-backup-and-verified-restore)). One region is not that backup.
- The service MUST NOT link private application logic, module processing of private content, libvips or FFmpeg. CI MUST fail the service build if libvips, FFmpeg or module code that processes private plaintext appears in its dependency graph. Cargo features MUST NOT be the only privacy boundary. Hosted Tokio runtimes MUST NOT run blocking native media work ([FileSquid U-20, U-30, U-31, U-32](./product-vision.md#one-api-hosts-and-distributions)).
- Backup and sync MUST be separate guarantees. Sync MUST NOT be presented as backup. Backup MUST keep independent retained recovery points with rehearsed restores ([FileSquid U-09](./product-vision.md#platform-ownership-and-access), [C-003](./product-vision.md#platforms-originals-and-service-limits)).
- Hosted roots MUST be private per stable user ID. Export and clean-device recovery MUST NOT depend on a live service catalog ([FileSquid U-10](./product-vision.md#platform-ownership-and-access), [FE-034](./product-vision.md#portable-preservation)).
- One account model MUST scale from personal to institutional quotas on the same infrastructure. ImageSquid MUST be able to sell the service under its own brand as a white label of FileSquid's backup and sync features, using the same accounts and infrastructure; other distributions MAY ([C-031](./product-vision.md#platforms-originals-and-service-limits)).
- The service MUST publish quotas, retention and export windows. Free local access and export MUST survive cancellation ([FileSquid U-12](./product-vision.md#platform-ownership-and-access), [C-007](./product-vision.md#platforms-originals-and-service-limits)).
- Marketing and UI MUST NOT claim that encryption eliminates service metadata, compromised endpoints or infrastructure-provider dependence ([C-004](./product-vision.md#platforms-originals-and-service-limits)).
- Before paying users: an independent crypto-protocol audit, named providers and regions in more than one geography, a payment processor and a GDPR role analysis ([constraint requirements](./product-vision.md#constraint-requirements), [T-034](./product-vision.md#later-gates)).

**Source**
- [F-059](./prd.md#f-059-hosted-encrypted-backup-and-sync-service)
- [FE-078](./product-vision.md#platform-api-and-hosts)
- [FE-020](./product-vision.md#managed-protection-and-connectivity), [FE-034](./product-vision.md#portable-preservation)
- [FileSquid U-09, U-10, U-12](./product-vision.md#platform-ownership-and-access), [U-20, U-21, U-30, U-31, U-32](./product-vision.md#one-api-hosts-and-distributions)
- [T-054](./product-vision.md#api-and-hosts), [C-003, C-004, C-007, C-031](./product-vision.md#platforms-originals-and-service-limits), [C-025](./product-vision.md#release-sequence)
- [ADR 004](./architecture/004-axum-hosted-ciphertext-mailbox.md), [ADR 012](./architecture/012-s3-compatible-ciphertext-storage.md), [ADR 032](./architecture/032-hosting-and-privacy-model.md)

**Tests**
- [`client_encryption_and_key_custody_F-029.feature`](../apps/filesquid/crates/server/tests/features/client_encryption_and_key_custody_F-029.feature)
- [`endpoint_processing_and_hosted_encryption_TC-002.feature`](../apps/filesquid/crates/server/tests/features/endpoint_processing_and_hosted_encryption_TC-002.feature)
- [`personal_synchronization_and_availability_F-030.feature`](../apps/filesquid/ui/tests/features/personal_synchronization_and_availability_F-030.feature)
- [`encrypted_backup_and_verified_restore_F-031.feature`](../apps/filesquid/ui/tests/features/encrypted_backup_and_verified_restore_F-031.feature)

### SRS-F-060: Archival curation jobs and datasets

The system MUST implement [PRD F-060](./prd.md#f-060-archival-curation-jobs-and-datasets) with the acceptance criteria stated there.
- Curation jobs (for example transcoding) MUST be a FileSquid module exposed through the FileSquid API ([FE-079](./product-vision.md#institutional-interoperability)).
- A curation job MUST run as a durable, resumable job with bounded resources ([SRS-NFR-003](#srs-nfr-003-bounded-processing-and-migration-endurance)). Its results MUST be committed as new OCFL versions through the change model of [SRS-F-051](#srs-f-051-ocfl-module). It MUST NOT silently rewrite accepted state ([FileSquid U-85](./product-vision.md#further-modules), [C-051](./product-vision.md#platforms-originals-and-service-limits)).
- Parquet projection MUST use the `filesquid-ocfl` `parquet` feature (default off) and the committed-state enumerator ([T-039](./product-vision.md#canonical-ocfl-state-and-evidence)), writing Apache Arrow record batches ([T-041](./product-vision.md#canonical-ocfl-state-and-evidence)). Outputs MUST go to a caller-chosen location outside OCFL object and storage-root reserved paths. A dataset accepted for preservation MUST re-enter as ordinary OCFL object content. Parquet, DuckDB and Iceberg MUST remain derived representations ([C-045](./product-vision.md#module-rules), [C-046](./product-vision.md#module-rules)).
- FAIR reporting MUST be exposed through the API ([FileSquid U-86](./product-vision.md#further-modules)).
- The Arkrim distribution MUST be the primary UI for these modules: an archives dashboard for ingest, batch curation jobs, and dataset and FAIR reporting. Any distribution MAY enable the modules through its manifest ([SRS-F-058](#srs-f-058-white-label-distributions)). Institutional interoperability follows [SRS-F-047](#srs-f-047-institutional-dam-and-iiif-interoperability).
- This feature MUST NOT become a prerequisite for the FileSquid or ImageSquid consumer release ([FE-063](./product-vision.md#institutional-interoperability)).
- These jobs MUST be a later paid layer after verified paid encrypted preservation. They MUST NOT replace the backup/sync subscription as the primary hosted service ([C-051](./product-vision.md#platforms-originals-and-service-limits)).

**Source**
- [F-060](./prd.md#f-060-archival-curation-jobs-and-datasets)
- [FE-063, FE-079](./product-vision.md#institutional-interoperability)
- [T-039, T-041](./product-vision.md#canonical-ocfl-state-and-evidence)
- [FileSquid U-47](./product-vision.md#ocfl-module), [U-85, U-86](./product-vision.md#further-modules), [U-39](./product-vision.md#one-api-hosts-and-distributions)
- [C-045, C-046](./product-vision.md#module-rules), [C-051](./product-vision.md#platforms-originals-and-service-limits)
- [ADR 023](./architecture/023-filesquid-ocfl-engine-and-parquet.md), [ADR 029](./architecture/029-filesquid-unified-product-and-distributions.md)

**Tests**
- [`reusable_ocfl_engine_F-051.feature`](../apps/filesquid/crates/ocfl/tests/features/reusable_ocfl_engine_F-051.feature)
- [`job_checkpoint_protocol_NFR-003.feature`](../apps/filesquid/crates/core/tests/features/job_checkpoint_protocol_NFR-003.feature)
- Test gap: a dedicated `archival_curation_jobs_and_datasets_F-060.feature` is not yet written.

### SRS-F-061: Activity journal and live health monitoring

The system MUST implement [PRD F-061](./prd.md#f-061-activity-journal-and-live-health-monitoring) with the acceptance criteria stated there.
- **Journal store.** Each host that runs `filesquid-api` (desktop and `filesquid-server`; the ciphertext-only service keeps a separate service-event journal, [ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md)) MUST keep an append-only `activity_journal` table in its operational store (SQLite in WAL mode on desktop, PostgreSQL on servers) with: id, time (RFC 3339 UTC), host, process or job id, actor, operation, outcome, level, opaque target ids, duration, error code, and the optional operation id and OCFL object and version the entry concerns ([API-S-028](../apps/filesquid/crates/api/resources/srs.md), [SRS-F-067](#srs-f-067-prov-o-history-of-every-version-and-change)). Rows MUST NOT be updated or deleted except by the configured retention or archive job, which itself is journaled ([T-055](./product-vision.md#api-and-hosts)). The store is host-injected: `filesquid-api` defines `JournalStore` and links no SQLite driver; the SQLite store is `filesquid-native`'s `SqliteJournal`. Until its PostgreSQL operational store exists, `filesquid-server` MAY keep its journal in that SQLite store (`<data dir>/filesquid.sqlite`) ([ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md) amendment 2026-10-18).
- **Write path.** Domain services MUST insert the journal row in the same transaction as the operational change, or before acknowledging the operation. A `tracing` layer MUST forward process, warning and error events through a bounded queue whose producers wait when it is full. A load test MUST show zero lost events at the declared desktop and server envelopes ([SRS-NFR-003](#srs-nfr-003-bounded-processing-and-migration-endurance)).
- **Join with PROV-O.** Entries carry the optional `operation_id` and `object_ref` (`urn:uuid:<object-id>@vN`) ([API-S-028](../apps/filesquid/crates/api/resources/srs.md)); events that do not become PROV-O records (refusals, failures, health, process, IPC, authentication) stay in the journal only.
- **Recoverable from disk.** A committed operation's entries MUST be written into the same OCFL commit as `provenance/<version>.journal.jsonl`; all other entries MUST be sealed as JSON Lines segments into journal OCFL objects (one per library, one per installation) every hour, on app close and on library close; at most one hour of entries that changed nothing MAY be lost with the database. The rebuild-from-files command MUST rebuild the journal, de-duplicated by journal ID ([API-S-032](../apps/filesquid/crates/api/resources/srs.md), [DE-S-013](../apps/filesquid/crates/desktop/resources/srs.md)).
- **Coverage.** Browser console warnings and errors through `POST /api/v1/client-events`, one journal entry per delivered event; the UI queues and retries so rate limits only delay events, writes each queued event to IndexedDB while storage accepts writes (removed once acknowledged), spills a full queue there, persists undelivered events there at page close and delivers them on the next open; only events it could not keep because IndexedDB was unavailable or refused a write are counted, in one page-close `ui.console.overflow` entry, the one recorded unavoidable gap, storage that is unavailable or cleared (events lost because the person cleared the site's data, and an event whose IndexedDB write had not committed when the page ended without `pagehide`, are lost uncounted, part of the same gap) ([API-S-030](../apps/filesquid/crates/api/resources/srs.md), [UI-S-023](../apps/filesquid/ui/resources/srs.md)); engine warnings, library intents and the `fsquid` CLI through the activity path; the CLI writes to the journal of its data directory (`FILESQUID_DATA_DIR`, otherwise the desktop app's) and refuses to run without it ([API-S-031](../apps/filesquid/crates/api/resources/srs.md)).
- **Logs view and export.** A Logs view for the desktop owner, reached from the sidebar item directly below Settings and the single place to browse the audit log, with filters (time, library, operation, outcome and level, file, object or operation ID), search, detail, links to Provenance activities and live updates, and a JSON Lines or CSV export journaled as `journal.export` ([UI-S-021](../apps/filesquid/ui/resources/srs.md), [API-S-031](../apps/filesquid/crates/api/resources/srs.md)).
- **No log files.** Hosts MUST NOT create text log files; stdout JSON is unchanged. `scripts/verify_centralized_logging.sh` MUST reject file appenders and `*.log` paths.
- **Redaction.** A test per event type MUST prove entries contain no file contents, titles, captions, pixels, keys or credentials, and that server administrators cannot read another user's private metadata through journal routes.
- **Process registry.** Every long-running unit MUST register with type, state, progress, started time, heartbeat interval and timeout; heartbeats older than the timeout MUST mark it stalled ([T-057](./product-vision.md#api-and-hosts)).
- **Health.** Overall status MUST be `ok`, `degraded` or `failing` with machine-readable reasons from the registry and host checks (storage roots reachable and writable, free space against thresholds, operational store reachable, queue depths, error rate over a window) and from OCFL validation: while the validator reports an open error for any object of an open library, health MUST be at least `degraded` ([API-S-022](../apps/filesquid/crates/api/resources/srs.md)). Checks that need storage I/O (data directory writable, free space) are host-injected through `filesquid-api`'s `HostChecks` trait; the native implementation is `filesquid-native`'s `DataDirChecks`.
- **API.** `filesquid-api` MUST expose `GET /api/v1/admin/health`, `GET /api/v1/admin/processes`, `GET /api/v1/admin/journal` (filters: from, to, actor, operation, outcome, level, process; cursor paging; order ascending or newest first), `GET /api/v1/admin/journal/export` (JSON Lines of every entry matching the same filters, including from and to, oldest first; a response that reaches its size cap sets `X-Journal-Next-Cursor`, passed back as `after` to continue) and `GET /api/v1/admin/metrics`, documented in the OpenAPI contract, plus event-channel topics `process.update`, `journal.entry` and `health.change`. Every admin route MUST reject callers who are not the desktop owner or a server administrator ([SRS-F-056](#srs-f-056-single-filesquid-api), [SRS-F-057](#srs-f-057-desktop-and-server-hosts)).
- **Metrics.** `/metrics` (Prometheus text, `filesquid-server`) and `GET /api/v1/admin/metrics` MUST report the same counters ([T-056](./product-vision.md#api-and-hosts)).
- **Monitor dashboard.** `@filesquid/ui` MUST provide the Monitor dashboard as custom elements: health with reasons; live processes and jobs (state, progress, rate, ETA, stalled flag); queues; throughput and error rates over time; storage roots and free space; open OCFL validation findings; journal search with live tail and JSON Lines export. Labels come from the label table and styling from design tokens ([SRS-F-058](#srs-f-058-white-label-distributions)).
- **Optional OTLP export.** Disabled by default. When enabled, it MUST read the journal through a persisted cursor and resume after outages without loss. The Compose `observability` profile (VictoriaLogs at `/insert/opentelemetry/v1/logs`, VictoriaMetrics scraping `/metrics`) is the reference test target. The desktop host MUST NOT bundle or start any telemetry server ([FE-081](./product-vision.md#platform-api-and-hosts)).
- The journal MUST be the single source for the user-facing activity stream and per-file activity in [SRS-F-064](#srs-f-064-file-activity-comments-and-user-activity-stream). Rows keep opaque IDs; name resolution happens at read time within the caller's permissions, and a caller MUST NOT receive rows about objects they may not read.

**Source**
- [F-061](./prd.md#f-061-activity-journal-and-live-health-monitoring)
- [FE-080, FE-081](./product-vision.md#platform-api-and-hosts), [T-055, T-056, T-057](./product-vision.md#api-and-hosts), [FileSquid U-87, U-88](./product-vision.md#administration-and-monitoring)
- [ADR 014](./architecture/014-stdout-observability.md), [ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md)

**Tests**
- [`activity_journal_and_live_health_monitoring_F-061.feature`](../apps/filesquid/crates/api/tests/features/activity_journal_and_live_health_monitoring_F-061.feature) (scenarios; step definitions, unit, load and redaction tests come with Task FS-2.4)
- [`native_journal_store_F-061.feature`](../apps/filesquid/crates/native/tests/features/native_journal_store_F-061.feature) (SQLite journal store and data-directory checks)
- [`desktop_refusals_F-061.feature`](../apps/filesquid/crates/desktop/tests/features/desktop_refusals_F-061.feature) (desktop shell refusals: IPC, new windows, permissions, downloads)
- [`ocfl_validation_findings_API-S-022.feature`](../apps/filesquid/crates/api/tests/features/ocfl_validation_findings_API-S-022.feature) (OCFL validation findings in health and the Monitor)
- [`durable_journal_and_journal_tab_F-061.feature`](../apps/filesquid/crates/api/tests/features/durable_journal_and_journal_tab_F-061.feature) (scenarios only; step definitions arrive with Task FS-2.9)

### SRS-F-062: Drive-style views, sorting, selection and layout preferences

The system MUST implement [PRD F-062](./prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences) with the acceptance criteria stated there. UI copy MUST come from the active distribution's label table; the item type in data and APIs MUST be **file**.
- **Views.** `filesquid-api` MUST expose one paged view operation parameterised by view (`home`, `all`, `personal`, `recent`, `favorites`, `shares`, `tags`, `deleted`), sort key (`name`, `size`, `modified`, `modified_by_me`, `opened_by_me`), direction, folder placement (`on_top`, `mixed`), a view-local text filter and a cursor. Ordering MUST be stable across pages ([SRS-F-003](#srs-f-003-browsing-selection-and-viewing)). `modified_by_me` and `opened_by_me` MUST be computed from the caller's own [SRS-F-064](#srs-f-064-file-activity-comments-and-user-activity-stream) events only.
- **Storage use.** The API MUST report bytes used per library and per collection for the caller, including version history ([SRS-F-065](#srs-f-065-file-version-history-and-retention)), and MUST accept `size` as a sort key.
- **Copy and move.** Copy MUST mint a new FileId and a new stored name, keep the display name, and derive the new file from the source in its PROV-O history ([SRS-F-067](#srs-f-067-prov-o-history-of-every-version-and-change)); move MUST commit a new OCFL version through the change model ([SRS-F-051](#srs-f-051-ocfl-module)). Neither MAY give one set of bytes two home collections ([FileSquid U-05](./product-vision.md#platform-ownership-and-access)).
- **Multi-download.** A ZIP download MUST stream from ranged reads within 1,000 members and 10 GiB per archive, MUST apply the [SRS-F-046](#srs-f-046-media-metadata-inspection-correction-and-privacy-exports) outgoing policy per member, MUST include a manifest entry naming omitted members, and MUST NOT hold the archive in memory ([SRS-NFR-003](#srs-nfr-003-bounded-processing-and-migration-endurance)).
- **Preferences.** Start view, sort key and direction, folder placement, layout and hover-card setting MUST be stored per user through the API and MUST be restored on every host the user signs in to.
- **Default UI.** `<fs-file-browser>` MUST implement the sidebar (view filter field, the eight views including home and deleted files, the deleted files of every library this host has opened that the caller may read shown as the one Recently deleted entry, the final row of the tree, a single row over the deleted files of every library this host has opened that the caller may read after a 1 px divider, with no section eyebrow, with no separate bottom item (owner answer 3, verbatim: "There is only one trash. Not a separate trash per library"), storage used, settings), the single New control, list as default layout with the stated columns and footer summary, column-header and grid sort controls, row and header checkboxes with a bulk action bar that states the effective scope, the per-item menu (favorite, details, manage tags, rename, move or copy, download, share, delete; reveal in file manager on desktop) also on right-click, drag-to-folder move and a folder picker for move or copy, shared and link badges, preview-or-download on click, an optional hover card that is off on touch and under reduced motion, and a start-view setting (home or personal library root). Fixed-order views MUST say so instead of showing a disabled sort ([UI-S-006](../apps/filesquid/ui/resources/srs.md)).

**Delivered by:** FileSquid (`filesquid-api`, `@filesquid/ui`); distributions MAY present the capability with their own views.

**Source**
- [F-062](./prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences)
- [FileSquid U-01, U-14](./product-vision.md#platform-ownership-and-access), [FE-073](./product-vision.md#independent-library)
- G how-to-use, how-to-reorder, how-to-search; N access-files
- [ADR 030](./architecture/030-filesquid-api-and-transports.md), [ADR 031](./architecture/031-vanilla-typescript-ui-and-white-label-theming.md)

**Tests**
- [`drive_style_views_and_preferences_F-062.feature`](../apps/filesquid/ui/tests/features/drive_style_views_and_preferences_F-062.feature) (scenarios only; step definitions arrive with Task FS-2.5)

### SRS-F-063: File search query language and filter chips

The system MUST implement [PRD F-063](./prd.md#f-063-file-search-query-language-and-filter-chips) with the acceptance criteria stated there.
- **Operation.** `filesquid-api` MUST expose one search operation taking free text plus operators and returning paged, permission-filtered results with stable ordering and a declared default of relevance. Results and suggestions MUST be filtered by the caller's permissions before ranking ([SRS-NFR-004](#srs-nfr-004-security-and-privacy)).
- **Grammar.** The grammar MUST be published in the API contract ([SRS-F-056](#srs-f-056-single-filesquid-api)): quoted phrases, `-term`, and the operators `type:` (`folder` from logical-path semantics; file classes from the metadata module), `in:` (library or folder path), `title:`, `owner:`, `creator:`, `tag:`, `is:favorite`, `is:deleted` (every deleted file in the libraries this host has opened that the caller may read, deleted one by one or with its collection; no `is:unfiled`, [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md): Task [API-F2.15](../apps/filesquid/crates/api/resources/plan.md) removes it from the search parser, the OpenAPI contract and their tests together, and until that task ships the built API still accepts it), `before:`, `after:`, `createdbefore:`, `createdafter:` (`YYYY-MM-DD`, boundaries in the caller's declared zone, capture dates without zone excluded from instant comparison), `source:`, `generator:` (absent from the grammar, and so an unknown operator, in builds without the `c2pa` module; where the module is built but switched off, the disabled-module unavailable error of API-S-007 applies), `size>` / `size<` (integer plus optional `B`, `KB`, `MB`, `GB` in powers of 1024; no unit means bytes), and on `filesquid-server` `sharedwith:` (`me`, user, group, `public`; `external` not adopted), `to:`, `from:`, `pendingowner:` (`me` or a user; pending ownership transfers, [SRS-F-066](#srs-f-066-file-and-folder-sharing-roles-links-and-shares-view)). An unknown operator MUST return an error code, never an empty-filter match. Case folding and Unicode normalisation (NFC, simple case fold) MUST be specified in the contract and tested.
- **Indexes.** Text, metadata and activity indexes live in the operational store and MUST be rebuildable from canonical OCFL state ([SRS-TC-001](#srs-tc-001-immutable-originals-and-explicit-identity)). Full-text content indexing covers PDF and plain text in the first release and Office documents later, each declared in the [SRS-F-040](#srs-f-040-compatibility-and-parity-register) register; media files are not content-indexed by this operator (OCR and semantic search remain [SRS-F-020](#srs-f-020-semantic-search-ocr-and-visual-understanding)); indexing MUST run only on the authorised endpoint ([SRS-TC-002](#srs-tc-002-endpoint-processing-and-hosted-encryption)).
- **AI.** Natural-language questions MUST go only through the caller's own [SRS-F-045](#srs-f-045-byo-ai-connections-and-per-task-model-choice) connection, opt-in per task, listing the files used as sources. No default answer model MAY run over private files.
- **Default UI.** The global search bar MUST offer Type, People, Modified and Location chips with typed suggestions, chip-plus-text layering, per-chip remove and clear-all, drop-down results and Enter for the full list with column sorting; the sidebar filter MUST narrow the current view only; the active scope and result summary MUST show as chips; unsupported operators MUST be shown as unsupported ([UI-S-007](../apps/filesquid/ui/resources/srs.md)).

**Delivered by:** FileSquid (`filesquid-api`, `filesquid-metadata` format identification, `@filesquid/ui`). [SRS-F-006](#srs-f-006-virtual-collections-and-metadata-search) keeps saved-query rules and media quick filters.

**Source**
- [F-063](./prd.md#f-063-file-search-query-language-and-filter-chips)
- [FileSquid U-14](./product-vision.md#platform-ownership-and-access), [FileSquid U-66](./product-vision.md#metadata-module)
- G how-to-search; N access-files
- [ADR 030](./architecture/030-filesquid-api-and-transports.md)

**Tests**
- [`file_search_query_language_F-063.feature`](../apps/filesquid/crates/api/tests/features/file_search_query_language_F-063.feature) (scenarios only; step definitions arrive with Task FS-2.5)

### SRS-F-064: File activity, comments and user activity stream

The system MUST implement [PRD F-064](./prd.md#f-064-file-activity-comments-and-user-activity-stream) with the acceptance criteria stated there.
- **Stream.** `filesquid-api` MUST derive the user activity stream and the per-file activity list from the `activity_journal` table ([SRS-F-061](#srs-f-061-activity-journal-and-live-health-monitoring)) by joining journal rows to objects the caller may read at read time; opaque IDs MUST be resolved to names only for those objects. A row about an object the caller may not read MUST be excluded before paging. Filters: `all`, `by_me`, `by_others`, `file_changes`, `favorites`, `shares`, `comments`, `tags`, plus each enabled module's event types. Results MUST be paged, grouped by day in the caller's zone and carry actor, action, object and RFC 3339 time.
- **Comments.** A comment MUST be a journaled application record bound to a FileId, stored in the operational store and in the file's OCFL object as recoverable application data ([SRS-F-042](#srs-f-042-ocfl-preservation-and-portable-recovery)), postable by the file's owner and by recipients with the commenter or editor role ([SRS-F-066](#srs-f-066-file-and-folder-sharing-roles-links-and-shares-view)), readable by anyone who may read the file, editable and deletable by its author, removable by the owner. Emoji and smart-picker insertion are not adopted. Comments MUST NOT be included in shared or published copies unless the owner selects them in the [SRS-F-046](#srs-f-046-media-metadata-inspection-correction-and-privacy-exports) outgoing policy. Mentions MAY notify under [SRS-F-039](#srs-f-039-consent-notifications-and-activity-privacy) and MUST NOT grant access.
- **Publishing.** No RSS, Atom or bearer-link feed MAY exist. The API MUST expose an opt-in `publish-atproto` capability (module set, [SRS-F-058](#srs-f-058-white-label-distributions)): connect an AT Protocol account (handle and personal data server), publish one chosen public file or readable activity entry per explicit request, disconnect. Publishable files are those marked public ([SRS-F-053](#srs-f-053-virtual-exhibit)); published media MUST pass the [SRS-F-046](#srs-f-046-media-metadata-inspection-correction-and-privacy-exports) outgoing policy with the disclosure preview; connect, publish, failure and disconnect MUST be journaled and a successful publication recorded as a PROV-O publish activity in every build ([SRS-F-067](#srs-f-067-prov-o-history-of-every-version-and-change)) and, where the `c2pa` module is enabled, as a disclosure action ([SRS-F-043](#srs-f-043-c2pa-records-and-action-disclosure)) with the post URI as an opaque reference; credentials MUST live in the host's secret store and MUST NOT reach the ciphertext-only service; nothing MAY publish automatically ([ADR 034](./architecture/034-at-protocol-publishing-connection.md)).
- **Events.** The event channel ([SRS-F-056](#srs-f-056-single-filesquid-api)) MUST push `activity.entry` and `comment.update` topics filtered per subscriber.
- **Default UI.** An **Activity** panel (`<fs-activity-panel>`) MUST show per-file activity and comments with a comment box, in a named sidebar slot, independent of other panels ([SRS-F-055](#srs-f-055-default-ui-and-panel-composition)); an activity view reachable from the header MUST show the stream with the filters above and relative times; a **Publish to Bluesky** action on a public file or a readable activity entry MUST open the outgoing-disclosure preview and publish only on explicit confirmation, saying that the remote network strips metadata ([UI-S-008](../apps/filesquid/ui/resources/srs.md)).

**Delivered by:** FileSquid (`filesquid-api`, both hosts, `@filesquid/ui`). Activity collections stay in [SRS-F-007](#srs-f-007-automatic-and-activity-collections); the administrator journal and Monitor stay in [SRS-F-061](#srs-f-061-activity-journal-and-live-health-monitoring).

**Source**
- [F-064](./prd.md#f-064-file-activity-comments-and-user-activity-stream)
- [FileSquid U-17](./product-vision.md#one-api-hosts-and-distributions), [FileSquid U-88](./product-vision.md#administration-and-monitoring), [FE-022](./product-vision.md#selective-collaboration), [FE-081](./product-vision.md#platform-api-and-hosts)
- G how-to-use; N access-files, activity-app
- [ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md), [ADR 034](./architecture/034-at-protocol-publishing-connection.md)

**Tests**
- [`file_activity_and_comments_F-064.feature`](../apps/filesquid/crates/api/tests/features/file_activity_and_comments_F-064.feature) (scenarios only; step definitions arrive with Task FS-2.5)

### SRS-F-065: File version history and retention

The system MUST implement [PRD F-065](./prd.md#f-065-file-version-history-and-retention) with the acceptance criteria stated there.
- **Listing and reads.** `filesquid-api` MUST list a file's versions from the OCFL inventory (version, `created`, `user`, `message`, size, digest, label) and serve any version's bytes with ranged reads ([SRS-F-051](#srs-f-051-ocfl-module), [SRS-F-056](#srs-f-056-single-filesquid-api)).
- **Restore.** Restore MUST be one API operation implemented as revert or reinstate through the change model, producing exactly one new version and leaving earlier versions untouched. Writes MUST NOT be coalesced by time; every content or organisation change is a version.
- **Labels.** A label set at commit time MUST be the `label` member of the version's structured `filesquid.operation.v1` message ([ADR 040](./architecture/040-prov-o-history.md)); a label set later MUST be stored as application data recoverable with the file and MUST be searchable. Labels MUST NOT influence retention.
- **Retention.** The API MUST NOT offer version deletion, expiry or thinning, and no job MAY remove inventory-referenced content to free space ([SRS-F-042](#srs-f-042-ocfl-preservation-and-portable-recovery)). The one exception is the expiry of a recoverably deleted file at the end of its retention period, governed by the expiry setting of [SRS-F-008](#srs-f-008-deletion-hidden-files-and-duplicate-review) (owner decisions 2026-10-22, verbatim: "Okay, keep the version history by default but provide an admin config option to delete the version history as well"; then "Okay, if the "delete all version history" setting is enabled then the OCFL and any index entries are deleted when the retention period expires, along with the files and metadata, etc.. If the "keep version history" setting is enabled then keep those source OCFL objects in the library, with only version history, no other content."; [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md) Decision 11). History size MUST be reported per file and per library ([SRS-F-062](#srs-f-062-drive-style-views-sorting-selection-and-layout-preferences)); capacity warnings MUST explain hardware or quota limits without discarding data ([SRS-F-001](#srs-f-001-independent-local-library-and-ownership)). The [SRS-F-040](#srs-f-040-compatibility-and-parity-register) drive-parity table MUST record this difference.
- **Default UI.** The Versions panel MUST list versions newest first with label, relative time and size, mark the current version, and offer restore, download and edit label per version; no delete control MAY appear ([UI-S-009](../apps/filesquid/ui/resources/srs.md)).

**Delivered by:** FileSquid (`ocfl` module, `filesquid-api`, `@filesquid/ui`). Edit recipes stay in [SRS-F-011](#srs-f-011-non-destructive-editing-workflow).

**Source**
- [F-065](./prd.md#f-065-file-version-history-and-retention)
- [FileSquid U-01, U-03](./product-vision.md#platform-ownership-and-access), [FileSquid U-51–U-53](./product-vision.md#ocfl-module), [C-015](./product-vision.md#provenance-metadata-and-disclosure)
- N version-control
- [ADR 010](./architecture/010-ocfl-1-1-dual-profile.md), [ADR 023](./architecture/023-filesquid-ocfl-engine-and-parquet.md), [ADR 041](./architecture/041-owner-folder-libraries-and-home-collections.md)

**Tests**
- [`file_version_history_F-065.feature`](../apps/filesquid/ui/tests/features/file_version_history_F-065.feature) (scenarios only; step definitions arrive with Task FS-2.5)

### SRS-F-066: File and folder sharing roles, links and shares view

The system MUST implement [PRD F-066](./prd.md#f-066-file-and-folder-sharing-roles-links-and-shares-view) with the acceptance criteria stated there.
- **Share record.** `filesquid-server` MUST store shares as operational records (target: file, logical-path prefix or collection; recipient: user, group, team or public link; role: `viewer`, `commenter`, `editor`; re-share allowed; password hash and expiry for links) and MUST evaluate them deny-by-default on every API call together with collection permissions ([SRS-F-057](#srs-f-057-desktop-and-server-hosts)). Role semantics: viewer reads and downloads; commenter adds [SRS-F-064](#srs-f-064-file-activity-comments-and-user-activity-stream) comments; editor commits new versions, renames and moves within the share. No role deletes a file from the library; deletion is the owner's ([SRS-F-008](#srs-f-008-deletion-hidden-files-and-duplicate-review)).
- **Links.** Public-link delivery MUST apply the [SRS-F-046](#srs-f-046-media-metadata-inspection-correction-and-privacy-exports) outgoing policy to every byte, journal every download, honour expiry and revocation, and disclose bearer access in the UI. An internal link MUST resolve only for callers who already have access.
- **Access listing and views.** The API MUST list who has access to a file and through which share, and MUST serve the `shares` view (with me, by me, by link) for [SRS-F-062](#srs-f-062-drive-style-views-sorting-selection-and-layout-preferences).
- **Ownership transfer.** Transfer MUST be offer-and-accept, journaled, with a `pending` state exposed to `pendingowner:` ([SRS-F-063](#srs-f-063-file-search-query-language-and-filter-chips)); bytes MUST NOT leave their collection's storage location without a verified copy ([SRS-F-009](#srs-f-009-multiple-libraries-and-filesystem-administration)).
- **Administrator switches.** Sharing itself (new shares), public links, re-sharing and download MUST each be switchable per server by an administrator; each change MUST be journaled and the UI MUST hide the control and attribute the restriction to the administrator. Billing administration MUST grant no share or read access.
- **Encrypted hosts.** Shares MUST use recipient keys ([SRS-F-029](#srs-f-029-client-encryption-and-key-custody)); no re-share MAY be required after a key change and no master key MAY exist ([SRS-F-034](#srs-f-034-family-accounts-and-shared-libraries)).
- **Default UI.** A **Sharing** panel (`<fs-sharing-panel>`) MUST show internal shares (people or teams picker, others with access, internal link) and external shares (public link with password and expiry) in a named slot; shared and link badges follow [SRS-F-062](#srs-f-062-drive-style-views-sorting-selection-and-layout-preferences) ([UI-S-010](../apps/filesquid/ui/resources/srs.md)).

**Delivered by:** FileSquid (`filesquid-server`, `filesquid-api`, `@filesquid/ui`). Albums and hosted galleries stay in [SRS-F-033](#srs-f-033-shared-albums-and-hosted-galleries); shared libraries in [SRS-F-034](#srs-f-034-family-accounts-and-shared-libraries).

**Source**
- [F-066](./prd.md#f-066-file-and-folder-sharing-roles-links-and-shares-view)
- [FileSquid U-06](./product-vision.md#platform-ownership-and-access), [FE-022](./product-vision.md#selective-collaboration), [T-053](./product-vision.md#api-and-hosts)
- G how-to-use, how-to-search; N access-files
- [ADR 032](./architecture/032-hosting-and-privacy-model.md)

**Tests**
- [`file_and_folder_sharing_F-066.feature`](../apps/filesquid/crates/server/tests/features/file_and_folder_sharing_F-066.feature) (scenarios only; step definitions arrive with Task FS-2.5)

### SRS-F-067: PROV-O history of every version and change

The system MUST implement [PRD F-067](./prd.md#f-067-prov-o-history-of-every-version-and-change) with the acceptance criteria stated there, in every build, whether or not the `c2pa` module is linked ([ADR 040](./architecture/040-prov-o-history.md)).
- **Engine.** `filesquid-prov` MUST own the PROV-O model, the `filesquid-prov/1` JSON-LD profile (embedded context; PROV namespace `http://www.w3.org/ns/prov#`), bounded parsing, validation and the history index, with no default dependency on another FileSquid crate, never a dependency on `filesquid-native` or `filesquid-c2pa`, and a `wasm32-unknown-unknown` build; its optional `ocfl` feature reads and writes records through `filesquid-ocfl` and derives backfill records ([PROV-S-001](../apps/filesquid/crates/prov/resources/srs.md)–[PROV-S-006](../apps/filesquid/crates/prov/resources/srs.md)).
- **Record per version.** Every commit by every writer (the API's change-model path, the metadata store, the desktop OCFL coordinator) MUST add `provenance/<version>.jsonld` to the same version, produced after the version is reserved so its name and inventory `created` time are final, and recomputed on rebase ([OCFL-S-024](../apps/filesquid/crates/ocfl/resources/srs.md), [MD-S-014](../apps/filesquid/crates/metadata/resources/srs.md), [API-S-028](../apps/filesquid/crates/api/resources/srs.md)). The record MUST hold exactly one `prov:Activity` for the committed OCFL object version it describes; an operation that commits several objects produces one activity per object version, all sharing the operation ID (operation kind and ID, OCFL object and version, journal entry IDs, start and end times in RFC 3339 UTC, the end equal to `created`), the used, generated, derived and invalidated (delete, purge) `prov:Entity` file and metadata states (object ID, version, logical path, SHA-256) and the agents: the installation agent (`prov:Agent`, `urn:uuid:<installation-uuid>`; no person identity or display name), a `prov:SoftwareAgent` for the host and its version (no host name), and a tool or model agent reserved for later AI actions. On `filesquid-server` each account MUST be its own `prov:Agent` (`urn:uuid:<account-uuid>`, a UUID v4 assigned at account creation; no user name, e-mail address or display name) linked to the installation agent by `prov:actedOnBehalfOf` ([ADR 040](./architecture/040-prov-o-history.md) Decision 4); desktop records are unchanged.
- **Installation UUID.** Each host MUST generate one UUID v4 on its first run (cryptographic random source), store it in its data directory as `installation.json` (owner-only permissions) and in its operational store, read it at every start (the same value across restarts and updates), restore it from whichever of the two copies survives, and, when both are lost, generate a new one and journal `host.installation.created`; it MUST never be derived from hardware, account or host names, and earlier records keep the value they name ([DE-S-012](../apps/filesquid/crates/desktop/resources/srs.md), [CORE-S-010](../apps/filesquid/crates/core/resources/srs.md)). Names and titles MAY appear inside the owner's library.
- **Scope.** Covered operations: import, upload, copy, rename, move, hide, unhide, delete, restore, purge, metadata edit, label, revert, reinstate, repair, export (including a ZIP download), scrub, share or publication output and catalog organisation (collections, virtual collections, memberships); a single-file download is not covered (journal only). A no-op command MUST commit no version and write no record; failed and refused attempts MUST be journaled only, never recorded in PROV-O.
- **Provenance-only versions.** An export, scrubbed export, ZIP download or share or publication output MUST commit its activity to the library's history object; a single-file download MUST be journaled only and MUST NOT create a record; a version-inventory repair MUST be followed by a provenance-only version of the repaired object carrying the repair run ID, retried from a pending intent if it fails.
- **Journal linkage.** Every commit MUST emit an `ocfl.commit` journal entry, MUST set the OCFL version `user` (name `FileSquid installation`, address `urn:uuid:<installation-uuid>`) and MUST carry the `filesquid.operation.v1` message (operation ID, kind, optional revision and label). Journal entry IDs MUST be assigned before the commit and named in the record; entries a crash left unwritten MUST be written at reconcile from the record with the same IDs ([DE-S-011](../apps/filesquid/crates/desktop/resources/srs.md)).
- **Backfill.** A one-time, bounded, resumable, cancellable job MUST give every version without a record a reconstructed one derived from its inventory and any journal entries, marked `reconstructed` with its own recording time, inventing no agent that was not recorded, committed as one provenance-only version per object; it MUST be idempotent.
- **Query and UI.** `filesquid-api` MUST serve paged history by file, object version, operation, agent and time range and the raw record of a version, under the file's read permission and route action `history.read`; any index MUST rebuild from the records ([API-S-029](../apps/filesquid/crates/api/resources/srs.md)). The Provenance tab of the file panel MUST list a file's activities, entities and agents and export them as JSON-LD; its **View in Logs** link MUST open the Logs view filtered to that file or operation, the single place to browse journal entries ([UI-S-021](../apps/filesquid/ui/resources/srs.md), [UI-S-020](../apps/filesquid/ui/resources/srs.md), [UI-S-022](../apps/filesquid/ui/resources/srs.md)).
- **Redaction.** Journal rows MUST carry only opaque IDs; records MUST be complete in the owner's library and follow the [SRS-F-046](#srs-f-046-media-metadata-inspection-correction-and-privacy-exports) outgoing policy on share and publication.
- **Core contract.** `filesquid-core` MUST provide the FileSquid-facing types: operation kind, the `filesquid.operation.v1` message, version reference, journal link and history entry ([CORE-S-010](../apps/filesquid/crates/core/resources/srs.md)).

**Delivered by:** FileSquid (`prov` module, `filesquid-api`, `filesquid-native` and the desktop host, `filesquid-metadata` store, `@filesquid/ui`).

**Source**
- [F-067](./prd.md#f-067-prov-o-history-of-every-version-and-change)
- [FE-082](./product-vision.md#prov-o-history), [U-89](./product-vision.md#prov-module), [U-78](./product-vision.md#c2pa-module), [T-037, T-058](./product-vision.md#canonical-ocfl-state-and-evidence)
- [ADR 040](./architecture/040-prov-o-history.md), [ADR 015](./architecture/015-prov-o-jsonld-and-c2pa-sdk.md), [ADR 033](./architecture/033-activity-journal-and-live-health-monitoring.md)

**Tests**
- [`prov_o_history_F-067.feature`](../apps/filesquid/crates/api/tests/features/prov_o_history_F-067.feature) (scenarios only; step definitions arrive with Task FS-2.8)
- [`prov_o_history_F-067.feature`](../apps/filesquid/ui/tests/features/prov_o_history_F-067.feature) (scenarios only)
- [`prov_o_server_agents_F-067.feature`](../apps/filesquid/crates/server/tests/features/prov_o_server_agents_F-067.feature) (server account agents; scenarios only, step definitions arrive with Task FS-3.10)

## Non-functional requirements

Non-functional requirements apply to FileSquid on every host and to every distribution.

### SRS-NFR-001: Preservation and transactional integrity

The system MUST satisfy [PRD NFR-001](./prd.md#nfr-001-preservation-and-transactional-integrity) as written. Numerical budgets in NFR-002/003 remain proposed until measured on packaged `filesquid-desktop` and `filesquid-server` builds, and on the encrypted browser host when it ships ([FileSquid U-11](./product-vision.md#platform-ownership-and-access)). The requirement applies to every host and every distribution.

**Source**
- [NFR-001](./prd.md#nfr-001-preservation-and-transactional-integrity)
- [ADR 007](./architecture/007-durable-jobs-and-interactive-performance.md)
- [ADR 014](./architecture/014-stdout-observability.md)

**Tests**
- [`preservation_and_transactional_integrity_NFR-001.feature`](../apps/filesquid/crates/server/tests/features/preservation_and_transactional_integrity_NFR-001.feature)
- [`command_query_ports_F-038.feature`](../apps/filesquid/crates/core/tests/features/command_query_ports_F-038.feature)

### SRS-NFR-002: Benchmark envelope and responsiveness

The system MUST satisfy [PRD NFR-002](./prd.md#nfr-002-benchmark-envelope-and-responsiveness) as written. Numerical budgets in NFR-002/003 remain proposed until measured on packaged `filesquid-desktop` and `filesquid-server` builds, and on the encrypted browser host when it ships ([FileSquid U-11](./product-vision.md#platform-ownership-and-access)). The requirement applies to every host and every distribution. The production UI build's own contribution is asserted as budgets in a browser by [UI-S-035](../apps/filesquid/ui/resources/srs.md) (Task UI-F3.1); that is not the packaged-build measurement this paragraph requires.

**Source**
- [NFR-002](./prd.md#nfr-002-benchmark-envelope-and-responsiveness)
- [ADR 007](./architecture/007-durable-jobs-and-interactive-performance.md)
- [ADR 014](./architecture/014-stdout-observability.md)

**Tests**
- [`benchmark_envelope_and_responsiveness_NFR-002.feature`](../apps/filesquid/crates/server/tests/features/benchmark_envelope_and_responsiveness_NFR-002.feature)
- [`production_build_browsing_performance_NFR-002.feature`](../apps/filesquid/ui/tests/features/production_build_browsing_performance_NFR-002.feature)

### SRS-NFR-003: Bounded processing and migration endurance

The system MUST satisfy [PRD NFR-003](./prd.md#nfr-003-bounded-processing-and-migration-endurance) as written. Numerical budgets in NFR-002/003 remain proposed until measured on packaged `filesquid-desktop` and `filesquid-server` builds, and on the encrypted browser host when it ships ([FileSquid U-11](./product-vision.md#platform-ownership-and-access)). The requirement applies to every host and every distribution.

**Source**
- [NFR-003](./prd.md#nfr-003-bounded-processing-and-migration-endurance)
- [ADR 007](./architecture/007-durable-jobs-and-interactive-performance.md)
- [ADR 014](./architecture/014-stdout-observability.md)

**Tests**
- [`bounded_processing_and_migration_endurance_NFR-003.feature`](../apps/filesquid/crates/server/tests/features/bounded_processing_and_migration_endurance_NFR-003.feature)
- [`job_checkpoint_protocol_NFR-003.feature`](../apps/filesquid/crates/core/tests/features/job_checkpoint_protocol_NFR-003.feature)
- [`production_build_browsing_performance_NFR-002.feature`](../apps/filesquid/ui/tests/features/production_build_browsing_performance_NFR-002.feature) (the bounded DOM window and the heap growth after warm-up of the production UI build)

### SRS-NFR-004: Security and privacy

The system MUST satisfy [PRD NFR-004](./prd.md#nfr-004-security-and-privacy) as written. Numerical budgets in NFR-002/003 remain proposed until measured on packaged `filesquid-desktop` and `filesquid-server` builds, and on the encrypted browser host when it ships ([FileSquid U-11](./product-vision.md#platform-ownership-and-access)). The requirement applies to every host and every distribution.

**Source**
- [NFR-004](./prd.md#nfr-004-security-and-privacy)
- [ADR 007](./architecture/007-durable-jobs-and-interactive-performance.md)
- [ADR 014](./architecture/014-stdout-observability.md)

**Tests**
- [`security_and_privacy_NFR-004.feature`](../apps/filesquid/crates/server/tests/features/security_and_privacy_NFR-004.feature)

### SRS-NFR-005: Recovery and service reliability

The system MUST satisfy [PRD NFR-005](./prd.md#nfr-005-recovery-and-service-reliability) as written. Numerical budgets in NFR-002/003 remain proposed until measured on packaged `filesquid-desktop` and `filesquid-server` builds, and on the encrypted browser host when it ships ([FileSquid U-11](./product-vision.md#platform-ownership-and-access)). The requirement applies to every host and every distribution.

**Source**
- [NFR-005](./prd.md#nfr-005-recovery-and-service-reliability)
- [ADR 007](./architecture/007-durable-jobs-and-interactive-performance.md)
- [ADR 014](./architecture/014-stdout-observability.md)

**Tests**
- [`recovery_and_service_reliability_NFR-005.feature`](../apps/filesquid/crates/server/tests/features/recovery_and_service_reliability_NFR-005.feature)

### SRS-NFR-006: Accessibility and usability

The system MUST satisfy [PRD NFR-006](./prd.md#nfr-006-accessibility-and-usability) as written. Numerical budgets in NFR-002/003 remain proposed until measured on packaged `filesquid-desktop` and `filesquid-server` builds, and on the encrypted browser host when it ships ([FileSquid U-11](./product-vision.md#platform-ownership-and-access)). The requirement applies to every host and every distribution.

**Source**
- [NFR-006](./prd.md#nfr-006-accessibility-and-usability)
- [ADR 007](./architecture/007-durable-jobs-and-interactive-performance.md)
- [ADR 014](./architecture/014-stdout-observability.md)

**Tests**
- [`accessibility_and_usability_NFR-006.feature`](../apps/filesquid/crates/server/tests/features/accessibility_and_usability_NFR-006.feature)
- [`accessibility_contracts_NFR-006.feature`](../apps/filesquid/ui/tests/features/accessibility_contracts_NFR-006.feature) (the UI contracts every `@filesquid/ui` element and distribution meets: [UI-S-016](../apps/filesquid/ui/resources/srs.md))

### SRS-NFR-007: Compatibility, rendering and localization

The system MUST satisfy [PRD NFR-007](./prd.md#nfr-007-compatibility-rendering-and-localization) as written. Numerical budgets in NFR-002/003 remain proposed until measured on packaged `filesquid-desktop` and `filesquid-server` builds, and on the encrypted browser host when it ships ([FileSquid U-11](./product-vision.md#platform-ownership-and-access)). The requirement applies to every host and every distribution.

**Source**
- [NFR-007](./prd.md#nfr-007-compatibility-rendering-and-localization)
- [ADR 007](./architecture/007-durable-jobs-and-interactive-performance.md)
- [ADR 014](./architecture/014-stdout-observability.md)

**Tests**
- [`compatibility_rendering_and_localization_NFR-007.feature`](../apps/filesquid/crates/server/tests/features/compatibility_rendering_and_localization_NFR-007.feature)

### SRS-NFR-008: Maintainability and verified engineering

The system MUST satisfy [PRD NFR-008](./prd.md#nfr-008-maintainability-and-verified-engineering) as written. Numerical budgets in NFR-002/003 remain proposed until measured on packaged `filesquid-desktop` and `filesquid-server` builds, and on the encrypted browser host when it ships ([FileSquid U-11](./product-vision.md#platform-ownership-and-access)). The requirement applies to every host and every distribution.

**Source**
- [NFR-008](./prd.md#nfr-008-maintainability-and-verified-engineering)
- [ADR 007](./architecture/007-durable-jobs-and-interactive-performance.md)
- [ADR 014](./architecture/014-stdout-observability.md)

**Tests**
- [`maintainability_and_verified_engineering_NFR-008.feature`](../apps/filesquid/crates/server/tests/features/maintainability_and_verified_engineering_NFR-008.feature)
- [`portable_contracts_isolation_CORE-S-003.feature`](../apps/filesquid/crates/core/tests/features/portable_contracts_isolation_CORE-S-003.feature)

### SRS-NFR-009: Observability and capacity economics

The system MUST satisfy [PRD NFR-009](./prd.md#nfr-009-observability-and-capacity-economics) as written. Numerical budgets in NFR-002/003 remain proposed until measured on packaged `filesquid-desktop` and `filesquid-server` builds, and on the encrypted browser host when it ships ([FileSquid U-11](./product-vision.md#platform-ownership-and-access)). The requirement applies to every host and every distribution.

**Source**
- [NFR-009](./prd.md#nfr-009-observability-and-capacity-economics)
- [ADR 007](./architecture/007-durable-jobs-and-interactive-performance.md)
- [ADR 014](./architecture/014-stdout-observability.md)

**Tests**
- [`observability_and_capacity_economics_NFR-009.feature`](../apps/filesquid/crates/server/tests/features/observability_and_capacity_economics_NFR-009.feature)

### SRS-NFR-010: Accurate public claims

The system MUST satisfy [PRD NFR-010](./prd.md#nfr-010-accurate-public-claims) as written. Public claims MUST describe the tested migration scope, encryption trust boundaries, hosting provider/geography and cancellation policy accurately, and MUST distinguish hypotheses from measured results. Numerical budgets in NFR-002/003 remain proposed until measured on packaged `filesquid-desktop` and `filesquid-server` builds, and on the encrypted browser host when it ships ([FileSquid U-11](./product-vision.md#platform-ownership-and-access)). The requirement applies to every host and every distribution.

**Source**
- [NFR-010](./prd.md#nfr-010-accurate-public-claims)
- [ADR 007](./architecture/007-durable-jobs-and-interactive-performance.md)
- [ADR 014](./architecture/014-stdout-observability.md)

**Tests**
- [`accurate_public_claims_NFR-010.feature`](../apps/filesquid/crates/server/tests/features/accurate_public_claims_NFR-010.feature)

## Technical constraints

### SRS-TC-001: Immutable originals and explicit identity

The system MUST satisfy [PRD TC-001](./prd.md#tc-001-immutable-originals-and-explicit-identity) as written. Newly generated portable domain identifiers MUST use RFC 4122 / RFC 9562 UUID version 4 as specified in [ADR 021](./architecture/021-uuid-v4-identifiers.md). SHA-256 digests remain distinct from those identifiers. Workspace crates (`filesquid-core`, `filesquid-ocfl`, `filesquid-metadata`, `filesquid-c2pa`, `filesquid-api`, the native-processing library `filesquid-native`, `filesquid-server`, `filesquid-desktop`) and UI packages (`@filesquid/ui`, `@imagesquid/ui`) MUST keep that isolation.

**Source**
- [TC-001](./prd.md#tc-001-immutable-originals-and-explicit-identity)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)
- [ADR 016](./architecture/016-rust-ocfl-library.md)
- [ADR 021](./architecture/021-uuid-v4-identifiers.md)

**Tests**
- [`immutable_originals_and_explicit_identity_TC-001.feature`](../apps/filesquid/crates/server/tests/features/immutable_originals_and_explicit_identity_TC-001.feature)
- [`immutable_originals_and_recipes_TC-001.feature`](../apps/filesquid/crates/core/tests/features/immutable_originals_and_recipes_TC-001.feature)

### SRS-TC-002: Endpoint processing and hosted encryption

The system MUST satisfy [PRD TC-002](./prd.md#tc-002-endpoint-processing-and-hosted-encryption) as written. Workspace crates (`filesquid-core`, `filesquid-ocfl`, `filesquid-metadata`, `filesquid-c2pa`, `filesquid-api`, the native-processing library `filesquid-native`, `filesquid-server`, `filesquid-desktop`) and UI packages (`@filesquid/ui`, `@imagesquid/ui`) MUST keep that isolation. Owner-operated `filesquid-server` deployments MAY process plaintext on the operator's server; the UI MUST state that the operator can read files ([SRS-F-057](#srs-f-057-desktop-and-server-hosts)). The `*.filesquid.tech` hosted service MUST remain ciphertext-only ([SRS-F-059](#srs-f-059-hosted-encrypted-backup-and-sync-service)).

**Source**
- [TC-002](./prd.md#tc-002-endpoint-processing-and-hosted-encryption)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)
- [ADR 016](./architecture/016-rust-ocfl-library.md)

**Tests**
- [`endpoint_processing_and_hosted_encryption_TC-002.feature`](../apps/filesquid/crates/server/tests/features/endpoint_processing_and_hosted_encryption_TC-002.feature)
- [`cryptographic_envelopes_F-029.feature`](../apps/filesquid/crates/core/tests/features/cryptographic_envelopes_F-029.feature)

### SRS-TC-003: Platform and provider limits

The system MUST satisfy [PRD TC-003](./prd.md#tc-003-platform-and-provider-limits) as written. Workspace crates (`filesquid-core`, `filesquid-ocfl`, `filesquid-metadata`, `filesquid-c2pa`, `filesquid-api`, the native-processing library `filesquid-native`, `filesquid-server`, `filesquid-desktop`) and UI packages (`@filesquid/ui`, `@imagesquid/ui`) MUST keep that isolation.

**Source**
- [TC-003](./prd.md#tc-003-platform-and-provider-limits)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)
- [ADR 016](./architecture/016-rust-ocfl-library.md)

**Tests**
- [`platform_and_provider_limits_TC-003.feature`](../apps/filesquid/crates/server/tests/features/platform_and_provider_limits_TC-003.feature)
- [`capability_register_F-040.feature`](../apps/filesquid/crates/core/tests/features/capability_register_F-040.feature)

### SRS-TC-004: Decision status and repository alignment

The system MUST satisfy [PRD TC-004](./prd.md#tc-004-decision-status-and-repository-alignment) as written. Workspace crates (`filesquid-core`, `filesquid-ocfl`, `filesquid-metadata`, `filesquid-c2pa`, `filesquid-api`, the native-processing library `filesquid-native`, `filesquid-server`, `filesquid-desktop`) and UI packages (`@filesquid/ui`, `@imagesquid/ui`) MUST keep that isolation.

**Source**
- [TC-004](./prd.md#tc-004-decision-status-and-repository-alignment)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)
- [ADR 016](./architecture/016-rust-ocfl-library.md)

**Tests**
- [`decision_status_and_repository_alignment_TC-004.feature`](../apps/filesquid/crates/server/tests/features/decision_status_and_repository_alignment_TC-004.feature)

### SRS-TC-005: Shared Rust graph, hosted mailbox isolation and OCFL writer

The system MUST satisfy [PRD TC-005](./prd.md#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer) as written. Workspace crates (`filesquid-core`, `filesquid-ocfl`, `filesquid-metadata`, `filesquid-c2pa`, `filesquid-api`, the native-processing library `filesquid-native`, `filesquid-server`, `filesquid-desktop`) and UI packages (`@filesquid/ui`, `@imagesquid/ui`) MUST keep that isolation. The hosted mailbox is the `*.filesquid.tech` service. `filesquid-server` is a separate, owner-operated deployment type that links `filesquid-api` and the modules ([ADR 032](./architecture/032-hosting-and-privacy-model.md)).

**Source**
- [TC-005](./prd.md#tc-005-shared-rust-graph-hosted-mailbox-isolation-and-ocfl-writer)
- [ADR 001](./architecture/001-shared-rust-core-with-tauri-and-axum-hosts.md)
- [ADR 016](./architecture/016-rust-ocfl-library.md)
- [ADR 023](./architecture/023-filesquid-ocfl-engine-and-parquet.md)
- [ADR 025](./architecture/025-client-side-filesquid-engines.md) (amended by [ADR 032](./architecture/032-hosting-and-privacy-model.md))

**Tests**
- [`shared_rust_graph_hosted_mailbox_isolation_and_ocf_TC-005.feature`](../apps/filesquid/crates/server/tests/features/shared_rust_graph_hosted_mailbox_isolation_and_ocf_TC-005.feature)
- [`portable_contracts_isolation_CORE-S-003.feature`](../apps/filesquid/crates/core/tests/features/portable_contracts_isolation_CORE-S-003.feature)

## Release criteria

RC-001–RC-014 in the PRD remain the completion gates. Each RC MUST be demonstrated through the SRS blocks of its parent F/NFR/TC rows. They do not mint separate product features. RC gates that name ImageSquid surfaces apply to the ImageSquid release; platform gates apply to the FileSquid release ([SRS-F-050](#srs-f-050-filesquid-platform-and-distributions)).

## Mockup coverage

The active mockup suite of the FileSquid default UI is [v3](./ui-mockups/v3/README.md) (owner decision 2026-10-22, verbatim: "yes, make v3 the active suite in that PR"); [v1](./ui-mockups/v1/README.md) and [v2](./ui-mockups/v2/README.md) are archived and are not requirement sources ([mockups README](./ui-mockups/README.md)). Its composition, strings, constants and states are recorded in the UI SRS rows below, each citing its mockup files; the PRD IDs are in each row's last cell. Distribution screenshots and their suites are recorded in each distribution's SRS.

| v3 mockups | UI SRS rows | PRD |
|---|---|---|
| every file (colours, type, spacing, radii, controls) | [UI-S-024](../apps/filesquid/ui/resources/srs.md) | [F-055](./prd.md#f-055-default-ui-and-panel-composition), [F-058](./prd.md#f-058-white-label-distributions), [NFR-006](./prd.md#nfr-006-accessibility-and-usability) |
| 05 (light, dark) and the shell of every browsing screen | [UI-S-025](../apps/filesquid/ui/resources/srs.md), [UI-S-006](../apps/filesquid/ui/resources/srs.md) | [F-055](./prd.md#f-055-default-ui-and-panel-composition), [F-062](./prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences), [F-057](./prd.md#f-057-desktop-and-server-hosts) |
| 05, 06, 10 to 15, 33, 44, 56 | [UI-S-026](../apps/filesquid/ui/resources/srs.md) | [F-062](./prd.md#f-062-drive-style-views-sorting-selection-and-layout-preferences), [F-003](./prd.md#f-003-browsing-selection-and-viewing), [F-002](./prd.md#f-002-safe-file-folder-and-device-import) |
| 01 to 04, 08, 09, 34, 41 (light, dark), 42, 43 | [UI-S-027](../apps/filesquid/ui/resources/srs.md) | [F-001](./prd.md#f-001-independent-local-library-and-ownership), [F-009](./prd.md#f-009-multiple-libraries-and-filesystem-administration), [F-042](./prd.md#f-042-ocfl-preservation-and-portable-recovery) |
| 57 | [UI-S-034](../apps/filesquid/ui/resources/srs.md) | [F-001](./prd.md#f-001-independent-local-library-and-ownership), [F-004](./prd.md#f-004-collections-folders-and-navigation), [F-009](./prd.md#f-009-multiple-libraries-and-filesystem-administration) |
| 07, 17, 18, 19, 51, 52 | [UI-S-028](../apps/filesquid/ui/resources/srs.md), [UI-S-002](../apps/filesquid/ui/resources/srs.md) | [F-002](./prd.md#f-002-safe-file-folder-and-device-import), [F-004](./prd.md#f-004-collections-folders-and-navigation) |
| 16 (light, dark), 45 to 50, 58 | [UI-S-029](../apps/filesquid/ui/resources/srs.md), [UI-S-002](../apps/filesquid/ui/resources/srs.md) | [F-008](./prd.md#f-008-deletion-hidden-files-and-duplicate-review), [F-004](./prd.md#f-004-collections-folders-and-navigation), [F-065](./prd.md#f-065-file-version-history-and-retention) |
| 20, 21, 53 | [UI-S-030](../apps/filesquid/ui/resources/srs.md), [UI-S-007](../apps/filesquid/ui/resources/srs.md) | [F-063](./prd.md#f-063-file-search-query-language-and-filter-chips) |
| 22 (light, dark) to 29, 35, 36 | [UI-S-031](../apps/filesquid/ui/resources/srs.md), [UI-S-022](../apps/filesquid/ui/resources/srs.md), [UI-S-009](../apps/filesquid/ui/resources/srs.md) | [F-055](./prd.md#f-055-default-ui-and-panel-composition), [F-065](./prd.md#f-065-file-version-history-and-retention), [F-067](./prd.md#f-067-prov-o-history-of-every-version-and-change) |
| 31 (light, dark), 38 (light, dark), 39, 54 (light, dark) | [UI-S-032](../apps/filesquid/ui/resources/srs.md), [UI-S-021](../apps/filesquid/ui/resources/srs.md) | [F-061](./prd.md#f-061-activity-journal-and-live-health-monitoring) |
| 32, 40 | [UI-S-033](../apps/filesquid/ui/resources/srs.md), [UI-S-018](../apps/filesquid/ui/resources/srs.md) | [F-057](./prd.md#f-057-desktop-and-server-hosts), [F-037](./prd.md#f-037-setup-updates-and-diagnostics) |
| 55 | [UI-S-017](../apps/filesquid/ui/resources/srs.md), [UI-S-025](../apps/filesquid/ui/resources/srs.md) | [F-004](./prd.md#f-004-collections-folders-and-navigation) |

**PRD gaps found in the active suite** (a surface or state with no PRD ID, specified anyway as the mockups README requires): the "files of this destination cannot be listed here yet" state of [`56-view-states.svg`](./ui-mockups/v3/56-view-states.svg) (`navigationFilesPending`, [UI-S-026](../apps/filesquid/ui/resources/srs.md)) has no PRD criterion; it is a stop-gap state of the built navigation.

## Summary traceability matrix

| PRD ID | SRS ID | Tests |
|---|---|---|
| Design-token contract | SRS-UI-007 | `../apps/filesquid/ui/tests/features/theme_tokens_SRS-UI-007.feature` |
| F-001 | SRS-F-001 | `../apps/filesquid/ui/tests/features/independent_local_library_and_ownership_F-001.feature`, `../apps/filesquid/crates/api/tests/features/local_library_and_version_inventory_repair_API-S-023.feature` |
| F-002 | SRS-F-002 | `../apps/filesquid/ui/tests/features/safe_file_folder_and_device_import_F-002.feature` |
| F-003 | SRS-F-003 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-004 | SRS-F-004 | `../apps/filesquid/ui/tests/features/albums_folders_and_navigation_F-004.feature`, `../apps/filesquid/crates/core/tests/features/collections_and_virtual_collections_F-004.feature`, `../apps/filesquid/crates/api/tests/features/navigation_listing_API-S-021.feature`, `../apps/filesquid/ui/tests/features/navigation_preferences_UI-S-017.feature` |
| F-005 | SRS-F-005 | `../apps/filesquid/ui/tests/features/favorites_metadata_and_keywords_F-005.feature` |
| F-006 | SRS-F-006 | `../apps/filesquid/ui/tests/features/smart_albums_and_metadata_search_F-006.feature` |
| F-007 | SRS-F-007 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-008 | SRS-F-008 | `../apps/filesquid/ui/tests/features/deletion_hidden_media_and_duplicate_review_F-008.feature` |
| F-009 | SRS-F-009 | `../apps/filesquid/ui/tests/features/multiple_libraries_and_filesystem_administration_F-009.feature` |
| F-010 | SRS-F-010 | `../apps/filesquid/ui/tests/features/export_and_portable_backup_F-010.feature` |
| F-011 | SRS-F-011 | `../apps/filesquid/crates/core/tests/features/immutable_originals_and_recipes_TC-001.feature` |
| F-012 | SRS-F-012 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-013 | SRS-F-013 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-014 | SRS-F-014 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-015 | SRS-F-015 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-016 | SRS-F-016 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-017 | SRS-F-017 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-018 | SRS-F-018 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-019 | SRS-F-019 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-020 | SRS-F-020 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-021 | SRS-F-021 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-022 | SRS-F-022 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-023 | SRS-F-023 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-024 | SRS-F-024 | `../apps/filesquid/ui/tests/features/system_sharing_and_extension_interfaces_F-024.feature` |
| F-025 | SRS-F-025 | `../apps/filesquid/ui/tests/features/migration_selection_inventory_and_pilot_F-025.feature` |
| F-026 | SRS-F-026 | `../apps/filesquid/ui/tests/features/migration_preservation_and_reconstruction_F-026.feature` |
| F-027 | SRS-F-027 | `../apps/filesquid/ui/tests/features/migration_execution_recovery_and_reconciliation_F-027.feature` |
| F-028 | SRS-F-028 | Backend scenarios in the implementing FileSquid module (view scenarios live with the distribution) |
| F-029 | SRS-F-029 | `../apps/filesquid/crates/server/tests/features/client_encryption_and_key_custody_F-029.feature`, `../apps/filesquid/crates/core/tests/features/cryptographic_envelopes_F-029.feature` |
| F-030 | SRS-F-030 | `../apps/filesquid/ui/tests/features/personal_synchronization_and_availability_F-030.feature` |
| F-031 | SRS-F-031 | `../apps/filesquid/ui/tests/features/encrypted_backup_and_verified_restore_F-031.feature` |
| F-032 | SRS-F-032 | `../apps/filesquid/ui/tests/features/accounts_entitlements_billing_and_cancellation_F-032.feature` |
| F-033 | SRS-F-033 | `../apps/filesquid/ui/tests/features/shared_albums_and_hosted_galleries_F-033.feature` |
| F-034 | SRS-F-034 | `../apps/filesquid/ui/tests/features/family_accounts_and_shared_libraries_F-034.feature` |
| F-035 | SRS-F-035 | `../apps/filesquid/crates/server/tests/features/optional_service_extensions_F-035.feature` |
| F-036 | SRS-F-036 | `../apps/filesquid/ui/tests/features/browser_and_mobileweb_experience_F-036.feature`, `../apps/filesquid/crates/api/tests/features/replica_cache_identity_and_pending_changes_API-S-024.feature` |
| F-037 | SRS-F-037 | `../apps/filesquid/ui/tests/features/setup_updates_and_diagnostics_F-037.feature`, `../apps/filesquid/crates/core/tests/features/shared_health_status_CORE-S-004.feature`, `../apps/filesquid/crates/desktop/tests/features/desktop_capabilities_F-037.feature`, `../apps/filesquid/crates/desktop/tests/features/desktop_refusals_F-061.feature` |
| F-038 | SRS-F-038 | `../apps/filesquid/crates/server/tests/features/open_interfaces_and_product_exit_F-038.feature`, `../apps/filesquid/crates/core/tests/features/command_query_ports_F-038.feature` |
| F-039 | SRS-F-039 | `../apps/filesquid/ui/tests/features/consent_notifications_and_activity_privacy_F-039.feature` |
| F-040 | SRS-F-040 | `../apps/filesquid/crates/server/tests/features/compatibility_and_parity_register_F-040.feature`, `../apps/filesquid/crates/core/tests/features/capability_register_F-040.feature` |
| F-041 | SRS-F-041 | `../apps/filesquid/ui/tests/features/sourceaware_web_clipping_and_http_evidence_F-041.feature` |
| F-042 | SRS-F-042 | `../apps/filesquid/ui/tests/features/ocfl_preservation_and_portable_recovery_F-042.feature`, `../apps/filesquid/crates/api/tests/features/local_library_and_version_inventory_repair_API-S-023.feature` |
| F-043 | SRS-F-043 | `../apps/filesquid/ui/tests/features/importtime_c2pa_records_and_action_disclosure_F-043.feature`, `../apps/filesquid/crates/core/tests/features/c2pa_and_provo_records_F-043.feature` |
| F-044 | SRS-F-044 | `../apps/filesquid/ui/tests/features/paid_c2pa_publishing_and_watermarking_F-044.feature` |
| F-045 | SRS-F-045 | `../apps/filesquid/ui/tests/features/byo_ai_connections_and_pertask_model_choice_F-045.feature` |
| F-046 | SRS-F-046 | `../apps/filesquid/ui/tests/features/media_metadata_inspection_correction_and_privacy_e_F-046.feature` |
| F-047 | SRS-F-047 | `../apps/filesquid/crates/server/tests/features/institutional_dam_and_iiif_interoperability_F-047.feature` |
| F-048 | SRS-F-048 | `../apps/filesquid/crates/server/tests/features/later_hosted_private_oss_inference_F-048.feature` |
| F-049 | SRS-F-049 | `../apps/filesquid/ui/tests/features/product_component_terminology_and_item_type_F-049.feature`, `../apps/filesquid/crates/core/tests/features/opaque_identifiers_CORE-S-001.feature`, `../apps/filesquid/crates/core/tests/features/item_type_is_file_CORE-S-002.feature` |
| F-050 | SRS-F-050 | `../apps/filesquid/crates/server/tests/features/later_filesquid_records_platform_F-050.feature`, `../apps/filesquid/crates/core/tests/features/portable_contracts_isolation_CORE-S-003.feature`, `../apps/filesquid/crates/cli/tests/features/single_fsquid_cli_F-050.feature` |
| F-051 | SRS-F-051 | `../apps/filesquid/crates/ocfl/tests/features/reusable_ocfl_engine_F-051.feature`, `../apps/filesquid/crates/ocfl/tests/features/OCFL_1.1_fixture_conformance.feature`, `../apps/filesquid/ui/tests/features/ocfl_preservation_and_portable_recovery_F-042.feature` |
| F-052 | SRS-F-052 | `../apps/filesquid/crates/metadata/tests/features/reusable_metadata_engine_F-052.feature`, `../apps/filesquid/ui/tests/features/media_metadata_inspection_correction_and_privacy_e_F-046.feature` |
| F-053 | SRS-F-053 | `../apps/filesquid/ui/tests/features/virtual_exhibit_F-053.feature`, `../apps/filesquid/crates/core/tests/features/collections_and_virtual_collections_F-004.feature` |
| F-054 | SRS-F-054 | `../apps/filesquid/crates/c2pa/tests/features/reusable_provenance_engine_F-054.feature`, `../apps/filesquid/crates/core/tests/features/c2pa_and_provo_records_F-043.feature` |
| F-055 | SRS-F-055 | `../apps/filesquid/ui/tests/features/composable_filesquid_ui_F-055.feature`, `../apps/filesquid/crates/ocfl/tests/features/reusable_ocfl_engine_F-051.feature` |
| F-056 | SRS-F-056 | `../apps/filesquid/crates/core/tests/features/command_query_ports_F-038.feature`, `../apps/filesquid/crates/server/tests/features/health.feature` (dedicated feature file not yet written) |
| F-057 | SRS-F-057 | `../apps/filesquid/crates/server/tests/features/health.feature`, `../apps/filesquid/crates/core/tests/features/shared_health_status_CORE-S-004.feature`, `../apps/filesquid/ui/tests/features/setup_updates_and_diagnostics_F-037.feature`, `../apps/filesquid/crates/api/tests/features/host_descriptor_API-S-020.feature`, `../apps/filesquid/ui/tests/features/host_privacy_indicator_UI-S-018.feature` (dedicated feature file not yet written) |
| F-058 | SRS-F-058 | `../apps/filesquid/ui/tests/features/theme_tokens_SRS-UI-007.feature`, `../apps/filesquid/ui/tests/features/product_component_terminology_and_item_type_F-049.feature`, `../apps/filesquid/ui/tests/features/composable_filesquid_ui_F-055.feature` (dedicated feature file not yet written) |
| F-059 | SRS-F-059 | `../apps/filesquid/crates/server/tests/features/client_encryption_and_key_custody_F-029.feature`, `../apps/filesquid/crates/server/tests/features/endpoint_processing_and_hosted_encryption_TC-002.feature`, `../apps/filesquid/ui/tests/features/personal_synchronization_and_availability_F-030.feature`, `../apps/filesquid/ui/tests/features/encrypted_backup_and_verified_restore_F-031.feature` |
| F-060 | SRS-F-060 | `../apps/filesquid/crates/ocfl/tests/features/reusable_ocfl_engine_F-051.feature`, `../apps/filesquid/crates/core/tests/features/job_checkpoint_protocol_NFR-003.feature` (dedicated feature file not yet written) |
| F-061 | SRS-F-061 | `../apps/filesquid/crates/api/tests/features/activity_journal_and_live_health_monitoring_F-061.feature`, `../apps/filesquid/crates/native/tests/features/native_journal_store_F-061.feature`, `../apps/filesquid/crates/desktop/tests/features/desktop_refusals_F-061.feature`, `../apps/filesquid/crates/api/tests/features/ocfl_validation_findings_API-S-022.feature` |
| F-062 | SRS-F-062 | `../apps/filesquid/ui/tests/features/drive_style_views_and_preferences_F-062.feature` (scenarios only; step definitions pending) |
| F-063 | SRS-F-063 | `../apps/filesquid/crates/api/tests/features/file_search_query_language_F-063.feature` (scenarios only; step definitions pending) |
| F-064 | SRS-F-064 | `../apps/filesquid/crates/api/tests/features/file_activity_and_comments_F-064.feature` (scenarios only; step definitions pending) |
| F-065 | SRS-F-065 | `../apps/filesquid/ui/tests/features/file_version_history_F-065.feature` (scenarios only; step definitions pending) |
| F-066 | SRS-F-066 | `../apps/filesquid/crates/server/tests/features/file_and_folder_sharing_F-066.feature` (scenarios only; step definitions pending) |
| F-067 | SRS-F-067 | `../apps/filesquid/crates/api/tests/features/prov_o_history_F-067.feature`, `../apps/filesquid/ui/tests/features/prov_o_history_F-067.feature` (scenarios only; step definitions arrive with Task FS-2.8) |
| NFR-001 | SRS-NFR-001 | `../apps/filesquid/crates/server/tests/features/preservation_and_transactional_integrity_NFR-001.feature`, `../apps/filesquid/crates/core/tests/features/command_query_ports_F-038.feature` |
| NFR-002 | SRS-NFR-002 | `../apps/filesquid/crates/server/tests/features/benchmark_envelope_and_responsiveness_NFR-002.feature` |
| NFR-003 | SRS-NFR-003 | `../apps/filesquid/crates/server/tests/features/bounded_processing_and_migration_endurance_NFR-003.feature`, `../apps/filesquid/crates/core/tests/features/job_checkpoint_protocol_NFR-003.feature` |
| NFR-004 | SRS-NFR-004 | `../apps/filesquid/crates/server/tests/features/security_and_privacy_NFR-004.feature` |
| NFR-005 | SRS-NFR-005 | `../apps/filesquid/crates/server/tests/features/recovery_and_service_reliability_NFR-005.feature` |
| NFR-006 | SRS-NFR-006 | `../apps/filesquid/crates/server/tests/features/accessibility_and_usability_NFR-006.feature`, `../apps/filesquid/ui/tests/features/accessibility_contracts_NFR-006.feature` |
| NFR-007 | SRS-NFR-007 | `../apps/filesquid/crates/server/tests/features/compatibility_rendering_and_localization_NFR-007.feature` |
| NFR-008 | SRS-NFR-008 | `../apps/filesquid/crates/server/tests/features/maintainability_and_verified_engineering_NFR-008.feature`, `../apps/filesquid/crates/core/tests/features/portable_contracts_isolation_CORE-S-003.feature` |
| NFR-009 | SRS-NFR-009 | `../apps/filesquid/crates/server/tests/features/observability_and_capacity_economics_NFR-009.feature` |
| NFR-010 | SRS-NFR-010 | `../apps/filesquid/crates/server/tests/features/accurate_public_claims_NFR-010.feature` |
| TC-001 | SRS-TC-001 | `../apps/filesquid/crates/server/tests/features/immutable_originals_and_explicit_identity_TC-001.feature`, `../apps/filesquid/crates/core/tests/features/immutable_originals_and_recipes_TC-001.feature` |
| TC-002 | SRS-TC-002 | `../apps/filesquid/crates/server/tests/features/endpoint_processing_and_hosted_encryption_TC-002.feature`, `../apps/filesquid/crates/core/tests/features/cryptographic_envelopes_F-029.feature` |
| TC-003 | SRS-TC-003 | `../apps/filesquid/crates/server/tests/features/platform_and_provider_limits_TC-003.feature`, `../apps/filesquid/crates/core/tests/features/capability_register_F-040.feature` |
| TC-004 | SRS-TC-004 | `../apps/filesquid/crates/server/tests/features/decision_status_and_repository_alignment_TC-004.feature` |
| TC-005 | SRS-TC-005 | `../apps/filesquid/crates/server/tests/features/shared_rust_graph_hosted_mailbox_isolation_and_ocf_TC-005.feature`, `../apps/filesquid/crates/core/tests/features/portable_contracts_isolation_CORE-S-003.feature` |
| RC-001 | Verified via parent F/NFR/TC blocks and [prd.md RC-001](./prd.md#rc-001-original-integrity-and-recovery) | parent feature files |
| RC-002 | Verified via parent F/NFR/TC blocks and [prd.md RC-002](./prd.md#rc-002-performance-and-resource-bounds) | parent feature files |
| RC-003 | Verified via parent F/NFR/TC blocks and [prd.md RC-003](./prd.md#rc-003-security-privacy-and-independent-browser-operation) | parent feature files |
| RC-004 | Verified via parent F/NFR/TC blocks and [prd.md RC-004](./prd.md#rc-004-migration-fidelity-and-accountability) | parent feature files |
| RC-005 | Verified via parent F/NFR/TC blocks and [prd.md RC-005](./prd.md#rc-005-functional-and-accessible-ui-quality) | parent feature files |
| RC-006 | Verified via parent F/NFR/TC blocks and [prd.md RC-006](./prd.md#rc-006-backup-sync-and-collaboration-correctness) | parent feature files |
| RC-007 | Verified via parent F/NFR/TC blocks and [prd.md RC-007](./prd.md#rc-007-operational-readiness) | parent feature files |
| RC-008 | Verified via parent F/NFR/TC blocks and [prd.md RC-008](./prd.md#rc-008-complete-parity-and-documentation-traceability) | parent feature files |
| RC-009 | Verified via parent F/NFR/TC blocks and [prd.md RC-009](./prd.md#rc-009-source-evidence-and-c2pa-lifecycle) | parent feature files |
| RC-010 | Verified via parent F/NFR/TC blocks and [prd.md RC-010](./prd.md#rc-010-complete-ocfl-preservation-and-independent-recovery) | parent feature files |
| RC-011 | Verified via parent F/NFR/TC blocks and [prd.md RC-011](./prd.md#rc-011-paid-third-party-publishing-and-watermark-delivery) | parent feature files |
| RC-012 | Verified via parent F/NFR/TC blocks and [prd.md RC-012](./prd.md#rc-012-user-funded-ai-connections-and-task-outcomes) | parent feature files |
| RC-013 | Verified via parent F/NFR/TC blocks and [prd.md RC-013](./prd.md#rc-013-metadata-fidelity-and-configurable-sharing-privacy) | parent feature files |
| RC-014 | Verified via parent F/NFR/TC blocks and [prd.md RC-014](./prd.md#rc-014-institutional-interoperability-and-service-readiness) | parent feature files |

100% of PRD F-001–F-067, NFR-001–NFR-010, and TC-001–TC-005 appear as SRS blocks. RC-001–RC-014 map through parent blocks. Every SRS ID of the previous ImageSquid SRS is retained; SRS-UI-001–006 and SRS-UI-008 now live in the ImageSquid distribution's SRS. Module-level requirements live in the module SRS files: [core](../apps/filesquid/crates/core/resources/srs.md), [ocfl](../apps/filesquid/crates/ocfl/resources/srs.md), [metadata](../apps/filesquid/crates/metadata/resources/srs.md), [c2pa](../apps/filesquid/crates/c2pa/resources/srs.md), [server](../apps/filesquid/crates/server/resources/srs.md), [desktop](../apps/filesquid/crates/desktop/resources/srs.md), [prov](../apps/filesquid/crates/prov/resources/srs.md). Distribution SRS files live with each distribution.
