# Implementation Plan: imagesquid (ImageSquid distribution)

ImageSquid is the photo-album distribution of FileSquid and its second consumer release ([ImageSquid vision](./product-vision.md)). This package (`@imagesquid/ui`) is a UI layer only: views, theme tokens (light and dark), a label table, brand assets and a distribution manifest. It contains **no Rust and no API code of its own**; every view composes `@filesquid/ui` custom elements and calls the FileSquid API through `FileSquidClient` (FileSquid ADR 031). Any backend capability ImageSquid needs (derivatives, editing recipes, people, migration routes, AI connections, publication) is a FileSquid module behind the API. Generic shell, host-adapter, token and file-management rows moved to the `@filesquid/ui` plan in FileSquid with their FE IDs; photo views stay here. Requirements: the [ImageSquid PRD](./prd.md) and [ImageSquid SRS](./srs.md); FileSquid requirement IDs are named in plain text.

Current screenshots (informative): [ui-mockups](./ui-mockups/README.md). Function parity is required; pixel-identical screenshots are forbidden. ImageSquid labels: repo, library, album, gallery, exhibit; preservation-store label **Archives**; skim origin labels **AI** or **RW** (Real World); item type stays **file** internally. Official logo: [ImageSquid-logo.svg](./assets/ImageSquid-logo.svg). Sequence: [Sequence and FileSquid prerequisites](#sequence-and-filesquid-prerequisites).

Commands run inside `apps/imagesquid/` (Vite dev `127.0.0.1:5174`, preview `4174`): `npm ci`, `npx prettier --check .`, `npx eslint .`, `npm run check`, `npm test`, `npm run build`, `npm run test:e2e`. UIs are vanilla TypeScript with no UI framework. UI tests: Playwright against `vite build` plus `vite preview`, strict DOM and state assertions ([UI-S-013](./srs.md#inherited-from-filesquidui), FileSquid ADR 017). Allure under `tests/allure-results/`.

**Holistic UI:** implement chrome, gallery navigation, studio recipes, inspector and theme so later requirements extend the same interaction model. Do not ship throwaway pan/zoom or a second shell.

## Sequence and FileSquid prerequisites

This plan is ImageSquid's own roadmap. FileSquid's implementation plan sequences FileSquid only and does not dispatch the rows below; its phases I1–I7 hold the backend capabilities these views call. FileSquid task IDs are named in plain text (not linked) so this plan survives the move to its own repository (FileSquid Task FS-3.8).

ImageSquid is the second consumer release. Encrypted backup and sync (FileSquid Task FS-4.2) is the product after the FileSquid MVP. ImageSquid work still starts after that backup release, then FileSquid release 1 (FileSquid Task FS-3.5), then the extraction of ImageSquid to its own repository (FileSquid Task FS-3.8, which needs `@filesquid/ui` and the API contract published with versions by FileSquid Task FS-3.7). When ImageSquid work starts, its full lint, typecheck, Vitest and Playwright suite MUST return to its own repository's gates (or to the FileSquid pre-commit hook if ImageSquid is still in that repository); during the FileSquid MVP that hook runs no ImageSquid suite and checks only this package's formatting and, through `@filesquid/ui`'s distribution-table test, its label table and token file (FileSquid owner decision 2026-10-23). Each row's **Dependencies** names the FileSquid task that delivers its backend capability; a row MUST NOT start before that task is checked in FileSquid. A capability FileSquid lacks is added to FileSquid as a module behind the API, never here (IS-S-001).

| Phase | ImageSquid rows | FileSquid tasks they wait on |
|---|---|---|
| F0 | IS-F0.1 (done) | FS-0.0 (done) |
| I1 | IS-0.3, IS-0.4, IS-1.3, IS-1.4, IS-1.5, IS-1.6, then IS-1.7 (MVP gate) | FS-2.1, FS-2.2, FS-4.2, UI-1.2, UI-1.6, 1.3, 1.4, 1.5, 1.7 |
| I2 | IS-2.2, IS-2.3, IS-2.4, IS-2.5 | 2.2, 2.3, 2.4, 2.5 |
| I3 | IS-3.1 | 0.12, 3.1 |
| I4 | IS-4.1, IS-4.2 | 4.1, 4.2 |
| I5 | IS-5.4, IS-6.1 | UI-5.3, 5.6, 6.1 |
| I6 | IS-7.1–IS-7.5 | 7.1–7.5, UI-1.8 |
| I7 | IS-8.1, IS-8.2, IS-8.4 | 8.1, 8.2, 8.4, 5.5 |
| L | IS-9.1, IS-9.3 | 9.1, 9.2, 9.3 (after FileSquid Task 9.0 orders IIIF) |
| V | IS-10.1 | 1.7, 10.1 |

The ImageSquid release gate (RC-001–013 with ImageSquid's views) precedes FileSquid Task 9.0, the owner's post-ImageSquid order decision.

## Phase F0: Themed shell and distribution manifest (dispatched by FileSquid Task FS-0.0)

- [x] **Task IS-F0.1: ImageSquid themed shell, labels and distribution manifest**
  - **Description:** Depend on `@filesquid/ui` by path; supply `src/theme/tokens.css` (light and dark), `labels.en.json` and brand assets, and render the same `<fs-app-shell>` themed as ImageSquid. Add `distribution.json` and the `tauri.imagesquid.json` overlay. No API code of its own.
  - **Estimated Time:** 8 hours
  - **Dependencies:** UI-F0.1, UI-F0.2
  - **Related Requirements:** [F-058](./prd.md#f-058-white-label-distributions), [F-050](./prd.md#f-050-filesquid-platform-and-distributions), [F-037](./prd.md#f-037-setup-updates-and-diagnostics), [IS-S-001](./srs.md#distribution-requirements), [IS-S-002](./srs.md#distribution-requirements), [IS-S-004](./srs.md#distribution-requirements), [IS-S-008](./srs.md#distribution-requirements), [IS-S-009](./srs.md#distribution-requirements), [UI-S-011](./srs.md#inherited-from-filesquidui), [UI-S-012](./srs.md#inherited-from-filesquidui), [UI-S-013](./srs.md#inherited-from-filesquidui), FileSquid SRS-UI-007, FileSquid ADR 031
  - **Related Tests:** [`library_chrome_SRS-UI-001.feature`](../tests/features/library_chrome_SRS-UI-001.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-F0.1. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against vite build plus vite preview asserting the ImageSquid label, logo and computed token values in light and dark. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-F0.1-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

## Phase I1: Chrome and overflow (formerly FileSquid global Task IS-1.0)

- [ ] **Task IS-1.0: ImageSquid distribution chrome and overflow**
  - **Status:** Moved from FileSquid's implementation plan, where it was a global task. Phase closer: check it when IS-0.3 and IS-0.4 are checked.
  - **Description:** ImageSquid library chrome (sidebar with Library, Galleries, Albums and smart queries; library switcher; search; OCFL, sync and CR status; theme control; inspector toggle) (IS-0.3) and the More/overflow surface for unmatched v2 controls (IS-0.4), as views composing `@filesquid/ui` custom elements with ImageSquid tokens and labels. ImageSquid contains no Rust and no API code of its own. Function parity with the current ImageSquid screenshots is required; pixel-identical screenshots are forbidden.
  - **Estimated Time:** included in IS-0.3 and IS-0.4
  - **Dependencies:** Task IS-0.3, Task IS-0.4
  - **Related Requirements:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome), [SRS-UI-008](./srs.md#srs-ui-008-unmatched-v2-controls), [F-003](./prd.md#f-003-gallery-selection-and-viewing), [F-058](./prd.md#f-058-white-label-distributions), [IS-S-009](./srs.md#distribution-requirements), [ImageSquid vision](./product-vision.md), FileSquid ADR 031
  - **Related Tests:** [`library_chrome_SRS-UI-001.feature`](../tests/features/library_chrome_SRS-UI-001.feature), [`overflow_v2_SRS-UI-008.feature`](../tests/features/overflow_v2_SRS-UI-008.feature)
  - **Agent Prompt:** None. Phase closer; execute IS-0.3 and IS-0.4, then check this box.

- [ ] **Task IS-0.3: Library chrome (sidebar, status, grid frame)**
  - **Description:** Persistent sidebar (Library, Galleries, Albums, Smart queries) and main content region. Library switcher, search, OCFL/sync status, C2PA/CR status, theme control, profile, zoom, Filter, share, inspector toggle, Select. Empty/loading/error/offline/permission-denied states distinct. UI nouns: repo, library, album, gallery.
  - **Estimated Time:** 12 hours
  - **Dependencies:** FileSquid Task UI-0.2 (token contract and brand slots, in `@filesquid/ui`); FileSquid Task FS-2.1 (default UI shell and host adapter); FileSquid Task FS-4.2 (encrypted backup and sync release gate; ImageSquid work starts after it)
  - **Related Requirements:** [SRS-UI-001](./srs.md#srs-ui-001-library-chrome), [F-003](./prd.md#f-003-gallery-selection-and-viewing), [F-004](./prd.md#f-004-albums-folders-and-navigation), [F-049](./prd.md#f-049-product-component-terminology-and-item-type), [IS-S-003](./srs.md#distribution-requirements), [IS-S-005](./srs.md#distribution-requirements), [IS-S-006](./srs.md#distribution-requirements)
  - **Related Tests:** [`library_chrome_SRS-UI-001.feature`](../tests/features/library_chrome_SRS-UI-001.feature), FileSquid `product_component_terminology_and_item_type_F-049.feature` (in `ui`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-0.3. Then begin development, executing all commands strictly within apps/imagesquid/. Design the chrome to support the final gallery/album/inspector/studio destinations. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-0.3-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-0.4: More/overflow for unmatched v2 controls**
  - **Description:** Reach vault/OCFL root, migration, C2PA publish, task monitor, DID, ⌘K, deep inspector, pairtree/digest detail from More/overflow/profile. Do not drop v2 controls because v3 is quieter.
  - **Estimated Time:** 6 hours
  - **Dependencies:** Task IS-0.3
  - **Related Requirements:** [SRS-UI-008](./srs.md#srs-ui-008-unmatched-v2-controls)
  - **Related Tests:** [`overflow_v2_SRS-UI-008.feature`](../tests/features/overflow_v2_SRS-UI-008.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-0.4. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-0.4-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

## Phase I1: MVP surfaces and MVP gate

- [ ] **Task IS-1.3: Virtualized gallery, selection, viewer**
  - **Description:** Virtualized grid with stable file keys, Years/Months/Days/All Photos, selection including frozen query + exceptions, viewer, basic video. Useful-image paint before refining quality. Placeholders do not count as useful-image readiness. Combine with chrome from IS-0.3.
  - **Estimated Time:** 16 hours
  - **Dependencies:** Task IS-0.3; FileSquid Task UI-1.2 (import UI with job progress, in `@filesquid/ui`); FileSquid Task 1.3 (paged catalog queries and derivative handles)
  - **Related Requirements:** [F-003](./prd.md#f-003-gallery-selection-and-viewing), FileSquid SRS-F-003, [NFR-002](./prd.md#nfr-002-benchmark-envelope-and-responsiveness)
  - **Related Tests:** [`gallery_selection_and_viewing_F-003.feature`](../tests/features/gallery_selection_and_viewing_F-003.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-1.3. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions for ordering, focus, selection, and useful-image replacement. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-1.3-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-1.4: Albums, favorites, search, activity, hide/delete**
  - **Description:** MVP organize surfaces in the existing sidebar, including hand-picked galleries (Add To a second album creates or extends one) and Edit-menu/keyboard Undo and Redo for organization commands through the FileSquid compensating commands. Hidden/deleted filtering on every collection, suggestion, preview, and notification from this task onward.
  - **Estimated Time:** 14 hours
  - **Dependencies:** Task IS-1.3; FileSquid Task 1.4 (organize commands: albums, favorites, search, activity, hide/delete)
  - **Related Requirements:** [F-004](./prd.md#f-004-albums-folders-and-navigation), [F-005](./prd.md#f-005-favorites-metadata-and-keywords), [F-006](./prd.md#f-006-smart-galleries-and-metadata-search), [F-007](./prd.md#f-007-automatic-and-activity-collections), [F-008](./prd.md#f-008-deletion-hidden-media-and-duplicate-review)
  - **Related Tests:** FileSquid `albums_folders_and_navigation_F-004.feature` (in `ui`), FileSquid `deletion_hidden_media_and_duplicate_review_F-008.feature` (in `ui`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-1.4. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-1.4-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-1.5: Inspector (metadata + C2PA) and export**
  - **Description:** One inspector surface composed from the `@filesquid/ui` panels (FileSquid ADR 031, superseding the packages of ADR 028): `<fs-metadata-panel>` for Overview/Descriptive, `<fs-credentials-panel>` for C2PA, the Details and Versions panels for Archive; Source stays an ImageSquid view. ImageSquid wording via string tables and theme via tokens. Selected file, populated `exif:` / `dcterms:` / `schema:` rows with prefixes, C2PA/provenance overview. Unsigned/signed/invalid/unavailable distinct. C2PA is not EU AI Act certification. Export originals/JPEG.
  - **Estimated Time:** 12 hours
  - **Dependencies:** Task IS-1.3; FileSquid Task 1.5 (export, metadata extract, C2PA import records, local OCFL recover); FileSquid Task FS-2.2 (Details, Versions and Metadata panels); FileSquid Task FS-C.1 (Content Credentials panel and routes, FileSquid phase C)
  - **Related Requirements:** [SRS-UI-002](./srs.md#srs-ui-002-media-inspector), [IS-S-007](./srs.md#distribution-requirements), [IS-S-010](./srs.md#distribution-requirements), [F-055](./prd.md#f-055-default-ui-and-panel-composition), [F-010](./prd.md#f-010-export-and-portable-backup), [F-043](./prd.md#f-043-import-time-c2pa-records-and-action-disclosure), [F-046](./prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports)
  - **Related Tests:** [`media_inspector_SRS-UI-002.feature`](../tests/features/media_inspector_SRS-UI-002.feature), FileSquid `importtime_c2pa_records_and_action_disclosure_F-043.feature` (in `ui`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-1.5. Then begin development, executing all commands strictly within apps/imagesquid/. Keep inspector as one surface for metadata and C2PA. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-1.5-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-1.6: Mobile PWA photo sync and in-app camera**
  - **Description:** Phone main screen of the installed PWA ([SRS-UI-006](./srs.md#srs-ui-006-mobile-web-companion)): **Sync Now**, last-synced line, pending count and progress, **Take Photo**. One capability record (directory access, permission state, multi-file picker, camera capture, IndexedDB) chosen by feature detection, never user agent, selects the path: File System Access with a remembered `FileSystemDirectoryHandle` in IndexedDB and persistent read permission (expected Android and desktop), or the one-tap multi-select file input with the "select each session" notice (expected iOS). Sync on launch only when the stored handle reports `granted`; otherwise a one-tap **Resume photo access**, and no permission or picker call before a user gesture. Per-device, per-folder sync time in RFC 3339 UTC, advanced only after every candidate is verified or reported failed; `lastModified` filter for remembered folders, FileSquid previously-imported detection for every candidate and for picker sessions; a lost sync record rescans without duplicates. Bounded directory walk. Captured photos go to a bounded IndexedDB upload queue, shown "Not yet in library" until verified. Unsupported controls are absent. Every ingest uses `FileSquidClient.upload()` into the FileSquid import workflow; no background or after-close sync. Sync status stays distinct from the **Archives** chip.
  - **Estimated Time:** 14 hours
  - **Dependencies:** Task IS-1.3; FileSquid Task UI-1.2 (import UI with job progress, in `@filesquid/ui`); FileSquid Task UI-1.6 (installable mobile-web layout, in `@filesquid/ui`); FileSquid Task 1.5 (C2PA import records)
  - **Related Requirements:** [F-036](./prd.md#f-036-browser-and-mobile-web-experience), [SRS-UI-006](./srs.md#srs-ui-006-mobile-web-companion), [IS-S-011–IS-S-021](./srs.md#mobile-photo-sync-and-capture), FileSquid F-002, FileSquid F-043, FileSquid C-001
  - **Related Tests:** [`mobile_companion_SRS-UI-006.feature`](../tests/features/mobile_companion_SRS-UI-006.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-1.6. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against vite build plus vite preview with strict DOM assertions in mobile viewports for each capability record: directory access with granted, prompt and denied permission; picker-only (iOS profile, showDirectoryPicker absent); camera capture absent; IndexedDB unavailable. Assert sync-on-launch runs only when granted, no picker or permission call before a user gesture, the pending count, last-synced line, no duplicate ingest after a lost sync record, the sync time not advancing on a partial sync, and unsupported controls absent. Additionally, invoke the @feature-writer skill to translate IS-S-011 to IS-S-021 into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-1.6-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-1.7: ImageSquid MVP verification gate**
  - **Status:** UI half of the former FileSquid global Task 1.7 (MVP verification gate). FileSquid Task 1.7 records the FileSquid evidence (RC-002/009/010/013 through the API); this gate records the ImageSquid views on top of it.
  - **Description:** Demonstrate RC-005 (functional and accessible UI quality) and NFR-006 for the ImageSquid MVP views (chrome and overflow, gallery and viewer, organize, inspector and export, mobile photo sync and camera) on Linux desktop and independent browser, and the installed PWA on iOS Safari and Android Chrome, against production builds. Real-device iOS and Android runs record which capability record each device reported. Independent-browser rows require FileSquid's browser workspace UI (FileSquid Task UI-1.6, in `@filesquid/ui`) and encrypted browser host (FileSquid Tasks FS-5.1 and 1.8); if it has not shipped, record them as pending, not passed. No paid-service claim.
  - **Estimated Time:** 8 hours
  - **Dependencies:** Task IS-0.3, Task IS-0.4, Task IS-1.3, Task IS-1.4, Task IS-1.5, Task IS-1.6; FileSquid Task 1.7 (MVP capability verification gate); FileSquid Task UI-1.6 (browser workspace UI) for the browser rows
  - **Related Requirements:** [RC-005](./prd.md#rc-005-functional-and-accessible-ui-quality), [NFR-006](./prd.md#nfr-006-accessibility-and-usability), [SRS-UI-001](./srs.md#srs-ui-001-library-chrome), [SRS-UI-002](./srs.md#srs-ui-002-media-inspector), [SRS-UI-006](./srs.md#srs-ui-006-mobile-web-companion), [UI-S-013](./srs.md#inherited-from-filesquidui), [UI-S-016](./srs.md#inherited-from-filesquidui) (inherited accessibility contracts)
  - **Related Tests:** [`library_chrome_SRS-UI-001.feature`](../tests/features/library_chrome_SRS-UI-001.feature), [`gallery_selection_and_viewing_F-003.feature`](../tests/features/gallery_selection_and_viewing_F-003.feature), [`media_inspector_SRS-UI-002.feature`](../tests/features/media_inspector_SRS-UI-002.feature), [`mobile_companion_SRS-UI-006.feature`](../tests/features/mobile_companion_SRS-UI-006.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-1.7. Then run the complete ImageSquid suite inside apps/imagesquid/ (npx prettier --check ., npx eslint ., npm run check, npm test, npm run build, npm run test:e2e) against the FileSquid build that passed FileSquid Task 1.7, emit Allure reports under tests/allure-results/, and copy raw CLI output into apps/imagesquid/resources/tasks/Task-IS-1.7-Walkthrough.md with SRS ID and full title at heading 2. Invoke the @feature-writer skill if gaps remain. Delete Plan.md and ToDos.md. Check off this box. Then commit your changes and push the branch to origin.`

## Phase I2: Archive and preservation UI

- [ ] **Task IS-2.2: Smart galleries, bursts, metadata correction, scan reports**
  - **Description:** Smart query editor, burst review, single/batch `dcterms:` CRUD with Schema.org suggestion and populated-only progressive disclosure, free C2PA scan/report UI. Scans do not sign or rewrite originals.
  - **Estimated Time:** 14 hours
  - **Dependencies:** Task IS-1.5; FileSquid Task 2.2 (saved-query evaluation, bursts, metadata write, C2PA scans)
  - **Related Requirements:** [F-006](./prd.md#f-006-smart-galleries-and-metadata-search), [F-016](./prd.md#f-016-live-photos-bursts-and-motion-images), [F-043](./prd.md#f-043-import-time-c2pa-records-and-action-disclosure), [F-046](./prd.md#f-046-media-metadata-inspection-correction-and-privacy-exports), FileSquid ADR 022
  - **Related Tests:** FileSquid `smart_albums_and_metadata_search_F-006.feature` (in `ui`), FileSquid `media_metadata_inspection_correction_and_privacy_e_F-046.feature` (in `ui`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-2.2. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-2.2-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-2.3: Preservation and migration manager**
  - **Description:** Job progress, checkpoint/resume, incoming buffer, storage allocation, OCFL snapshot history. Processing completion, media verification, and preservation protection remain separate states.
  - **Estimated Time:** 12 hours
  - **Dependencies:** Task IS-0.4; FileSquid Task 2.3 (migration engine and file/export routes)
  - **Related Requirements:** [SRS-UI-004](./srs.md#srs-ui-004-preservation-and-migration), [F-025](./prd.md#f-025-migration-selection-inventory-and-pilot), [F-027](./prd.md#f-027-migration-execution-recovery-and-reconciliation), [F-042](./prd.md#f-042-ocfl-preservation-and-portable-recovery), [NFR-003](./prd.md#nfr-003-bounded-processing-and-migration-endurance)
  - **Related Tests:** [`preservation_SRS-UI-004.feature`](../tests/features/preservation_SRS-UI-004.feature), FileSquid `migration_execution_recovery_and_reconciliation_F-027.feature` (in `ui`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-2.3. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-2.3-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-2.4: Source-aware web clipping UI**
  - **Description:** Source & History for web clipping with honest acquisition gaps. Evidence rendering disables active HTML/scripts. URLs or sanitized HAR cannot prove full capture.
  - **Estimated Time:** 10 hours
  - **Dependencies:** Task IS-1.5; FileSquid Task 2.4 (recorder and HTTP evidence)
  - **Related Requirements:** [F-041](./prd.md#f-041-source-aware-web-clipping-and-http-evidence), FileSquid SRS-F-041, [RC-009](./prd.md#rc-009-source-evidence-and-c2pa-lifecycle)
  - **Related Tests:** FileSquid `sourceaware_web_clipping_and_http_evidence_F-041.feature` (in `ui`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-2.4. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-2.4-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-2.5: M-B PhotoKit permission and progress UI**
  - **Description:** macOS-only PhotoKit permission, bounded extraction progress, and honest capability limits. MUST NOT gate Linux/browser suites.
  - **Estimated Time:** 6 hours
  - **Dependencies:** Task IS-2.3; FileSquid Task 2.5 (PhotoKit acquisition)
  - **Related Requirements:** [F-028](./prd.md#f-028-supported-migration-routes), [TC-003](./prd.md#tc-003-platform-and-provider-limits)
  - **Related Tests:** [`supported_migration_routes_F-028.feature`](../tests/features/supported_migration_routes_F-028.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-2.5. Then begin development, executing all commands strictly within apps/imagesquid/. This UI is macOS-capability gated. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-2.5-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

## Phase I3: Holistic studio

- [ ] **Task IS-3.1: Non-destructive studio (recipes, composition, tone, detail, RAW)**
  - **Description:** One studio interaction model for F-011/012/013. Immutable recipes, undo/revert/branches, composition, six selective-color samples, histogram, finishing/retouch, RAW pair. Assistive standard edits MUST NOT be labeled AI-generated. Originals unchanged.
  - **Estimated Time:** 24 hours
  - **Dependencies:** Task IS-1.5; FileSquid Task 0.12 (recipe and color prototypes); FileSquid Task 3.1 (recipe documents and render workers)
  - **Related Requirements:** [SRS-UI-003](./srs.md#srs-ui-003-non-destructive-studio), [F-011](./prd.md#f-011-non-destructive-editing-workflow), [F-012](./prd.md#f-012-composition-and-tonalcolor-editing), [F-013](./prd.md#f-013-detail-filters-retouch-and-raw)
  - **Related Tests:** [`studio_SRS-UI-003.feature`](../tests/features/studio_SRS-UI-003.feature), [`nondestructive_editing_workflow_F-011.feature`](../tests/features/nondestructive_editing_workflow_F-011.feature), [`composition_and_tonal_color_editing_F-012.feature`](../tests/features/composition_and_tonal_color_editing_F-012.feature), [`detail_filters_retouch_and_raw_F-013.feature`](../tests/features/detail_filters_retouch_and_raw_F-013.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-3.1. Then begin development, executing all commands strictly within apps/imagesquid/. Implement one recipe interaction model covering F-011/012/013 together. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-3.1-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

## Phase I4: Discovery and BYO AI UI

- [ ] **Task IS-4.1: People, pets, places, semantic search UI**
  - **Description:** Identity corrections/groups, Places/Trips, OCR/semantic search. Consent precedes suggestions. Private-content exclusion. Recognition (people, pets and biometric categorisation) MUST NOT be released until the GDPR and biometric-analysis review required by [FE-016](./product-vision.md#private-discovery) is completed and recorded; consent alone does not satisfy F-019.
  - **Estimated Time:** 16 hours
  - **Dependencies:** Task IS-1.4; FileSquid Task 4.1 (people, places and semantic search indexes); the recorded GDPR and biometric-analysis review ([FE-016](./product-vision.md#private-discovery)) before recognition is released
  - **Related Requirements:** [F-019](./prd.md#f-019-people-pets-and-groups), [F-020](./prd.md#f-020-semantic-search-ocr-and-visual-understanding), [F-021](./prd.md#f-021-places-trips-and-curation-controls), [F-039](./prd.md#f-039-consent-notifications-and-activity-privacy)
  - **Related Tests:** [`people_pets_and_groups_F-019.feature`](../tests/features/people_pets_and_groups_F-019.feature), [`semantic_search_ocr_and_visual_understanding_F-020.feature`](../tests/features/semantic_search_ocr_and_visual_understanding_F-020.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-4.1. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-4.1-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-4.2: BYO AI connection and per-task model UI**
  - **Description:** Connect local/frontier accounts, per-task model choice, budgets, no silent remote fallback. Standing AI Connections disclosure, inspectable How AI Connections work explanation, and per-job destination overlay. ImageSquid does not bill inference.
  - **Estimated Time:** 8 hours
  - **Dependencies:** Task IS-2.2; FileSquid Task UI-1.6 (browser workspace UI, in `@filesquid/ui`); FileSquid Task 4.2 (BYO AI connections and per-task model choice)
  - **Related Requirements:** [F-045](./prd.md#f-045-byo-ai-connections-and-per-task-model-choice), FileSquid FE-067, FileSquid SRS-F-045
  - **Related Tests:** FileSquid `byo_ai_connections_and_pertask_model_choice_F-045.feature` (in `ui`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-4.2. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-4.2-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

## Phase I5: Sharing UI

- [ ] **Task IS-5.4: Shared albums and family libraries (F-033, F-034)**
  - **Description:** Shared albums/hosted galleries. Family/non-family groups. Payer/access separation, revocation, organizer-plus-five proposed minimum. Separate from personal sync UI.
  - **Estimated Time:** 10 hours
  - **Dependencies:** FileSquid Task UI-5.3 (accounts, entitlements and job billing UI, in `@filesquid/ui`); FileSquid Task 5.6 (shared-library key envelopes and collaboration evidence)
  - **Related Requirements:** [F-033](./prd.md#f-033-shared-albums-and-hosted-galleries), [F-034](./prd.md#f-034-family-accounts-and-shared-libraries)
  - **Related Tests:** FileSquid `shared_albums_and_hosted_galleries_F-033.feature` (in `ui`), FileSquid `family_accounts_and_shared_libraries_F-034.feature` (in `ui`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-5.4. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-5.4-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

## Phases I5–I7, L and V: Projects, advanced studio, sharing, publication

- [ ] **Task IS-6.1: Memories, slideshows, print projects**
  - **Description:** Editable memory timelines, memory favorites, creation from an album, gallery or selection with an all/created switch, saved slideshows, print/PDF. Failed slides visible rather than silently skipped.
  - **Estimated Time:** 14 hours
  - **Dependencies:** Task IS-3.1, Task IS-4.1; FileSquid Task 6.1 (memory, slideshow, print and project rendering)
  - **Related Requirements:** [F-022](./prd.md#f-022-memories-and-storytelling), [F-023](./prd.md#f-023-slideshows-printing-and-photo-projects)
  - **Related Tests:** [`memories_and_storytelling_F-022.feature`](../tests/features/memories_and_storytelling_F-022.feature), [`slideshows_printing_and_photo_projects_F-023.feature`](../tests/features/slideshows_printing_and_photo_projects_F-023.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-6.1. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions for slide order, timing, and missing-resource states. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-6.1-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-7.1: AI-assisted cleanup and styles UI (F-014)**
  - **Description:** BYO cleanup/styles as recipe operations, plus Extend (generative frame extension) and AI Reframe in the Tools group, each AI-marked under FE-010. Do not invent a second editor.
  - **Estimated Time:** 8 hours
  - **Dependencies:** Task IS-3.1, Task IS-4.2; FileSquid Task 7.1 (AI-assisted cleanup worker)
  - **Related Requirements:** [F-014](./prd.md#f-014-ai-assisted-cleanup-and-styles)
  - **Related Tests:** [`aiassisted_cleanup_and_styles_F-014.feature`](../tests/features/aiassisted_cleanup_and_styles_F-014.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-7.1. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-7.1-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-7.2: Portrait, depth, Cinematic, spatial UI (F-015)**
  - **Description:** Surfaces for portrait/depth/Cinematic/spatial when data exists; honest F-040 gaps.
  - **Estimated Time:** 8 hours
  - **Dependencies:** Task IS-3.1; FileSquid Task 7.2 (portrait, depth, Cinematic and spatial workers)
  - **Related Requirements:** [F-015](./prd.md#f-015-portrait-depth-cinematic-and-spatial-media), [F-040](./prd.md#f-040-compatibility-and-parity-register)
  - **Related Tests:** [`portrait_depth_cinematic_and_spatial_media_F-015.feature`](../tests/features/portrait_depth_cinematic_and_spatial_media_F-015.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-7.2. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-7.2-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-7.3: Remaining Live Photo and motion UI (F-016)**
  - **Description:** Remaining Live/motion edit/export UI beyond burst review.
  - **Estimated Time:** 6 hours
  - **Dependencies:** Task IS-2.2, Task IS-3.1; FileSquid Task 7.3 (Live Photo and motion workers)
  - **Related Requirements:** [F-016](./prd.md#f-016-live-photos-bursts-and-motion-images)
  - **Related Tests:** [`live_photos_bursts_and_motion_images_F-016.feature`](../tests/features/live_photos_bursts_and_motion_images_F-016.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-7.3. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-7.3-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-7.4: Video and audio editor UI (F-017)**
  - **Description:** Timeline/trim/adjustment/export UI on the existing studio chrome.
  - **Estimated Time:** 12 hours
  - **Dependencies:** Task IS-3.1; FileSquid Task 7.4 (LGPL FFmpeg video/audio worker)
  - **Related Requirements:** [F-017](./prd.md#f-017-video-and-audio-editing)
  - **Related Tests:** [`video_and_audio_editing_F-017.feature`](../tests/features/video_and_audio_editing_F-017.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-7.4. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-7.4-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-7.5: Markup and remaining mobile UI (F-018 / F-036)**
  - **Description:** Markup/companion annotation UI and remaining F-036 mobile-web operations (phone photo sync and camera capture are IS-1.6).
  - **Estimated Time:** 8 hours
  - **Dependencies:** Task IS-3.1; FileSquid Task UI-1.8 (uploaded-ciphertext unlock and desktop-off path, in `@filesquid/ui`); FileSquid Task 7.5 (markup flattening worker)
  - **Related Requirements:** [F-018](./prd.md#f-018-markup-and-companion-annotation), [F-036](./prd.md#f-036-browser-and-mobile-web-experience), [SRS-UI-006](./srs.md#srs-ui-006-mobile-web-companion)
  - **Related Tests:** [`markup_and_companion_annotation_F-018.feature`](../tests/features/markup_and_companion_annotation_F-018.feature), [`mobile_companion_SRS-UI-006.feature`](../tests/features/mobile_companion_SRS-UI-006.feature)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-7.5. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-7.5-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-8.1: System sharing UI**
  - **Description:** OS share/external editor round trips, the small share-destination menu (enabled app destinations plus Save as, user-ordered, with the F-046 policy shown per destination) and remaining camera/phone capability presentation. Web clipping is IS-2.4.
  - **Estimated Time:** 8 hours
  - **Dependencies:** Task IS-1.5, Task IS-7.5; FileSquid Task 8.1 (system sharing contracts and camera adapters)
  - **Related Requirements:** [F-024](./prd.md#f-024-system-sharing-and-extension-interfaces)
  - **Related Tests:** FileSquid `system_sharing_and_extension_interfaces_F-024.feature` (in `ui`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-8.1. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-8.1-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-8.2: C-7 paid C2PA publication sheet**
  - **Description:** Base C-7 in-app flow: select output, scrubbing policy, selected-publisher timestamp, price, Sign & Publish, progress, cancel/retry, retrieve retained output. Originals unchanged. An external website alone fails. Does not wait for family libraries. Professional EU-icon disclosure is IS-8.4.
  - **Estimated Time:** 10 hours
  - **Dependencies:** Task IS-3.1, Task IS-2.4; FileSquid Task UI-5.3 (accounts and entitlements UI); FileSquid Task 8.2 (C-7 publication capability)
  - **Related Requirements:** [SRS-UI-005](./srs.md#srs-ui-005-paid-c2pa-publication), [F-044](./prd.md#f-044-paid-c2pa-publishing-and-watermarking), [RC-011](./prd.md#rc-011-paid-third-party-publishing-and-watermark-delivery)
  - **Related Tests:** [`publication_SRS-UI-005.feature`](../tests/features/publication_SRS-UI-005.feature), FileSquid `paid_c2pa_publishing_and_watermarking_F-044.feature` (in `ui`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-8.2. Then begin development, executing all commands strictly within apps/imagesquid/. Implement C-7 base publication; leave professional EU-icon disclosure to IS-8.4. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-8.2-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-8.4: Professional Article 50 disclosure UI**
  - **Description:** After the hosted-protection readiness close, human-perceivable EU icons or permitted accessible alternatives on first public exposure. A C2PA manifest or watermark alone does not satisfy visible disclosure. No certification claim.
  - **Estimated Time:** 8 hours
  - **Dependencies:** Task IS-8.2; FileSquid Task 5.5 (hosted-protection readiness close); FileSquid Task 8.4 (Article 50 disclosure rendering)
  - **Related Requirements:** [F-044](./prd.md#f-044-paid-c2pa-publishing-and-watermarking), [F-014](./prd.md#f-014-ai-assisted-cleanup-and-styles)
  - **Related Tests:** FileSquid `paid_c2pa_publishing_and_watermarking_F-044.feature` (in `ui`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-8.4. Then begin development, executing all commands strictly within apps/imagesquid/. Confirm Task 5.5 before professional disclosure. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-8.4-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-9.1: IIIF-2 import and viewing UI**
  - **Status:** Phase L. The institutional IIIF UI MAY move to the Arkrim distribution when Task 9.0 orders it.
  - **Description:** After the IIIF-2 gate, bounded import/viewing of selected archive items. Finite limits visible in the UI.
  - **Estimated Time:** 12 hours
  - **Dependencies:** Task IS-2.3; FileSquid Task 9.1 (institutional profiles confirmed); FileSquid Task 9.2 (IIIF-2 bounded import)
  - **Related Requirements:** [F-047](./prd.md#f-047-institutional-dam-and-iiif-interoperability)
  - **Related Tests:** FileSquid `institutional_dam_and_iiif_interoperability_F-047.feature` (in `crates/server`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-9.1 only after the IIIF-2 gate. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-9.1-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-9.3: IIIF-3 publish UI**
  - **Status:** Phase L. The institutional IIIF UI MAY move to the Arkrim distribution when Task 9.0 orders it.
  - **Description:** After the IIIF-3 gate, authorized projection/publish UI. Private OCFL roots stay private.
  - **Estimated Time:** 10 hours
  - **Dependencies:** Task IS-9.1, Task IS-8.2; FileSquid Task 9.3 (IIIF-3 projection and publishing)
  - **Related Requirements:** [F-047](./prd.md#f-047-institutional-dam-and-iiif-interoperability)
  - **Related Tests:** FileSquid `institutional_dam_and_iiif_interoperability_F-047.feature` (in `crates/server`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-9.3 only after the IIIF-3 gate. Then begin development, executing all commands strictly within apps/imagesquid/. Write Playwright UI tests against production preview with strict DOM assertions. Additionally, invoke the @feature-writer skill to translate SRS requirements into executable BDD feature tests. Upon completion, YOU MUST generate apps/imagesquid/resources/tasks/Task-IS-9.3-Walkthrough.md directly in the project repository with clear SRS mapping (including BOTH the ID and full human-readable title text) in the secondary section, and strictly delete the internal tracking Plan/ToDos documents before checking off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

- [ ] **Task IS-10.1: Browser performance evidence**
  - **Description:** Measure independent-browser NFR-002 profiles (useful-image, interaction, cache miss) on production preview. Placeholders do not pass.
  - **Estimated Time:** 8 hours
  - **Dependencies:** Task IS-1.3; FileSquid Task 1.7 (MVP capability verification gate)
  - **Related Requirements:** [NFR-002](./prd.md#nfr-002-benchmark-envelope-and-responsiveness), [RC-002](./prd.md#rc-002-performance-and-resource-bounds)
  - **Related Tests:** FileSquid `benchmark_envelope_and_responsiveness_NFR-002.feature` (in `crates/server`)
  - **Agent Prompt:** `@AGENTS.md Begin by creating a new feature branch for Task IS-10.1. Then write and execute Performance tests against production preview. Copy raw CLI output into apps/imagesquid/resources/tasks/Task-IS-10.1-Walkthrough.md with SRS ID and full title at heading 2. Delete Plan.md and ToDos.md. Check off the Markdown box locally in apps/imagesquid/resources/plan.md. Then commit your changes and push the branch to origin.`

## Summary Timeline

- **Total Estimated Time:** see each task (the FE rows, IS-F0.1 and IS-1.7); FileSquid backend hours are in FileSquid's plan
- **Critical Path:** encrypted backup and sync, then FileSquid release 1, then extraction (FileSquid Tasks FS-4.2, FS-3.5, FS-3.8) → themed shell → chrome/overflow → gallery/inspector → mobile PWA photo sync and camera → ImageSquid MVP gate → organize/preservation → F-041 clipping → holistic studio → discovery/BYO → unlock/sync/accounts → C-7 publication → Article 50 after hosted-protection readiness → performance
