# Task UI-F2.18 Plan: Conform the built surfaces and the shell to the active v3 mockups

## Traceability

- PRD: [F-055](../../../../../resources/prd.md#f-055-default-ui-and-panel-composition), [F-058](../../../../../resources/prd.md#f-058-white-label-distributions), [NFR-006](../../../../../resources/prd.md#nfr-006-accessibility-and-usability)
- SRS: [SRS-UI-007](../../../../../resources/srs.md#srs-ui-007-light-and-dark-themes); UI SRS rows [UI-S-005, UI-S-015, UI-S-016, UI-S-017, UI-S-024 to UI-S-034](../srs.md)
- Mockups: [v3 README](../../../../../resources/ui-mockups/v3/README.md) (active suite), files 01, 02, 03, 04, 08, 09, 14, 22, 22 dark, 23, 24, 31, 31 dark, 34, 40, 41, 41 dark, 42, 43, 44, 54, 54 dark, 55, 56
- Gherkin: `tests/features/theme_tokens_SRS-UI-007.feature`, `tests/features/ui_mockup_fidelity_UI-S-024.feature`, `tests/features/production_test_suite_UI-S-013.feature`, `tests/features/composable_filesquid_ui_F-055.feature`

## Owner answers that bind this task (v3 README, 2026-10-22 and 2026-10-23)

- 7: theme control in Settings, Appearance; none in the header.
- 8: user-facing Import and Export read Upload and Download; keys and API names keep their names.
- 12: no API information on browsing screens; "API: {status} · contract {version}" only in Settings, About.
- 14: 24 px logo mark; warnings in the error colour; no warning token.
- 15 and CA-1 "a": the guide's primary `#16ac49` with white text, active text `#0f8a39` / `#4aca74`, error `#e53935` / `#ef5350`, kept as a listed contrast exception for the FileSquid default only.
- 2026-10-23 "1. darken them": control outlines `#8e8e8e` / `#757575`; "3. keep": focus ring `#0f8a39` / `#16ac49`; "4. yes": the host badge stays in the header.
- Open (do not block): the repair-consent wording of mockup 42; `repairConsent` stays as is.

## Constraints extracted from the SRS (every granular value)

### UI-S-024 colour roles (light / dark)

| Token | Light | Dark | Use |
|---|---|---|---|
| `--fs-surface` | `#ffffff` | `#141414` | page |
| `--fs-surface-card` (new) | `#ffffff` | `#1c1c1c` | sidebar, panels, cards, inputs, secondary buttons |
| `--fs-surface-raised` (new) | `#f5f5f5` | `#242424` | hovered/selected rows, hovered menu items, version chips, host badge, disabled buttons, segmented tracks |
| `--fs-surface-overlay` (new) | `#ffffff` | `#2c2c2c` | dialogs, menus, popovers |
| `--fs-surface-container` | `#f5f5f5` | `#1c1c1c` | kept |
| `--fs-outline` | `#e0e0e0` | `#2a2a2a` | subtle borders: cards, dividers, progress tracks |
| `--fs-outline-strong` (new) | `#cccccc` | `#333333` | tables, secondary buttons, menus, dialogs |
| `--fs-outline-control` (new) | `#8e8e8e` (3.28:1 on `#ffffff`, 3.01:1 on `#f5f5f5`) | `#757575` (4.00:1 on `#141414`, 3.70:1 on `#1c1c1c`, 3.37:1 on `#242424`, 3.03:1 on `#2c2c2c`) | text inputs, search fields, selects, checkboxes, radios; 3:1 on every surface, no exception |
| `--fs-on-surface-heading` (new) | `#000000` | `#f5f5f5` | headings |
| `--fs-on-surface` | `#111111` | `#e0e0e0` | body |
| `--fs-on-surface-variant` | `#555555` | `#a0a0a0` | secondary and meta text |
| `--fs-on-surface-muted` (new) | `#777777` | `#6a6a6a` | eyebrows, muted captions |
| `--fs-on-surface-placeholder` (new) | `#999999` | `#6a6a6a` | placeholders, disabled button text |
| `--fs-primary-container` | `#f0faf4` | `#0d2b18` | active tint |
| `--fs-on-primary-container` | `#0f8a39` | `#4aca74` | active text |
| `--fs-primary` | `#16ac49` | `#16ac49` | primary actions, active icons and links |
| `--fs-on-primary` | `#ffffff` | `#ffffff` | text on primary |
| `--fs-error` | `#e53935` | `#ef5350` | errors and warnings (no warning token) |
| `--fs-focus-ring` (new) | `#0f8a39` | `#16ac49` | focus ring and focused input border, 3:1 or more |
| `--fs-brand` (new) | `#16ac49` | `#16ac49` | marks without text only: logo head, 2.5 px active-row bar, storage-used bar |
| `--fs-icon` (new) | `#111111` | `#a0a0a0` | default icons everywhere, sidebar and list rows included |
| `--fs-icon-muted` (new) | `#777777` | `#6a6a6a` | muted icons |
| `--fs-icon-disabled` (new) | `#cccccc` | `#3a3a3a` | disabled icons |
| `--fs-badge-document` (new) | `#fd1e23` | `#fd1e23` | file-type badge, `#ffffff` text |
| `--fs-badge-data` (new) | `#fcd805` | `#fcd805` | file-type badge, `#111111` text |
| `--fs-badge-media` (new) | `#007af9` | `#007af9` | file-type badge, `#ffffff` text |
| `--fs-on-badge-document/data/media` (new) | `#ffffff` / `#111111` / `#ffffff` | same | badge text, so the pair is testable |
| `--fs-inverse-surface` (new) | `#111111` | `#2c2c2c` | toast, with `--fs-on-inverse-surface` `#ffffff` / `#f5f5f5` |
| `--fs-scrim` (new), `--fs-scrim-opacity` (new) | `#000000`, `0.32` | same | dialog scrim |

Flat fills only: no gradient, glow or shadow in either theme.

### UI-S-024 type

- Headings `--fs-font-heading` (Georgia stack); UI text `--fs-font-sans`; identifiers, codes, digests, operators, version chips, badges, Logs times `--fs-font-mono`.
- Sizes (rem tokens, 16 px root): welcome title 56 px 700; page titles 30 px 700 (Recover 34 px); dialog titles 22 px 700; Settings section headings 22 px 600; form and side-panel titles 20 px 700; sidebar product name 17 px 700; welcome lead 17 px; empty and error leads 16 px 400; list names, input text and buttons 14 px (buttons 600); meta text, captions, column heads, breadcrumb, menu items, footer, filter chips at least 13 px; eyebrows 11 px 400 upper case letter-spacing 0.1em `--fs-on-surface-muted`; status chips 11 px 500; version chips 11 px 500 mono; file-type badges 10 px 700 mono upper case.
- Token names chosen: `--fs-font-size-badge` 0.625rem (10), `--fs-font-size-eyebrow` 0.6875rem (11), `--fs-font-size-small` 0.8125rem (13), `--fs-font-size-ui` 0.875rem (14), `--fs-font-size-body` 1rem (16), `--fs-font-size-lead` 1.0625rem (17), `--fs-font-size-title` 1.25rem (20), `--fs-font-size-heading` 1.375rem (22), `--fs-font-size-page` 1.875rem (30), `--fs-font-size-page-large` 2.125rem (34), `--fs-font-size-display` 3.5rem (56).

### UI-S-024 spacing, radii, controls, icons

- `--fs-space-1` to `--fs-space-7` = 4, 8, 16, 24, 40, 64, 96 px (replaces the built 12 px step); content column starts 24 px after the sidebar; sidebar controls 16 px from its edges.
- Radii: buttons and segmented controls 7 px (`--fs-radius-button`); inputs 6 px (`--fs-radius-input`); dialogs, cards, empty-state frame 10 px (`--fs-radius-dialog`); menus and toasts 8 px (`--fs-radius`); menu items and selected rows in dialogs 5 px (`--fs-radius-item`); version chips 4 px (`--fs-radius-chip`); file-type badges 3 px (`--fs-radius-badge`); checkboxes 3.5 px (`--fs-radius-check`); host badge and filter chips fully rounded (`--fs-radius-pill`).
- Buttons 36 px tall; sidebar New and dialog footer buttons 38 px; primary filled `--fs-primary` with `--fs-on-primary`; secondary `--fs-surface-card` with 1.5 px `--fs-outline-strong` and `--fs-on-surface` text; ghost transparent with `--fs-primary` text; destructive `--fs-surface-card`, `--fs-error` text, 1.5 px `--fs-error` border; disabled `--fs-surface-raised`, 1.5 px `--fs-outline`, `--fs-on-surface-placeholder` text. At most one filled primary button per screen besides the sidebar New, one per dialog.
- Inputs and selects 36 px tall, 1.5 px `--fs-outline-control`, focus 2 px `--fs-focus-ring`, error 2 px `--fs-error` with message below in `--fs-error`. Checkboxes 14.5 px and radios with 1.5 px `--fs-outline-control`.
- Status chips always carry text: ok-type on the active tint, warnings and errors outlined in `--fs-error`.
- Icons: line icons, 1.6 px stroke, round caps and joins, no fill, 16, 20 or 24 px. Logo mark 24 px tall.
- Focus: UI-S-016 ring, 2 px solid `--fs-focus-ring` offset 2 px.

### UI-S-025 shell composition (1440 × 900)

- Sidebar left, 240 px, full height, `--fs-surface-card`, 1 px `--fs-outline` line to the content. Top down: logo mark + product name (17 px 700 heading font); New primary button 208 × 38 (`fileNew` "New"); view filter field 208 × 34 (placeholder `fileFilterView` "Filter this view"); views All files (`fileRoot`) and Recent (`fileRecent` "Recent"); tree in sections by kind (PINNED, COLLECTIONS with each library a parent row: 14 px chevron `--fs-icon-muted` 14 px from edge, library icon, name 13 px 500 `--fs-on-surface` at 58 px, more button at right; collections 13 px 400 `--fs-on-surface-variant` at 60 px; VIRTUAL COLLECTIONS; FOLDERS only when a library has folders) then after a 1 px `--fs-outline` divider the single Recently deleted row (trash icon, `navigationRecentlyDeleted` "Recently deleted", count right-aligned 13 px `--fs-on-surface-muted`); eyebrows 11 px 400, 8 px above first row; icons 16 px; rows 30 px; 1 px divider above storage block; active row on `--fs-primary-container` with 2.5 px `--fs-brand` bar at left edge, text 500 `--fs-on-primary-container`. Bottom: Storage used (`fileStorageUsed`) value right-aligned 13 px 600, 4 px bar (`--fs-outline` track, `--fs-brand` fill, radius 2 px), captions `fileStorageCaptionHistory` "incl. {history} history" and `fileStorageFree` "{free} free on disk" in `--fs-on-surface-muted`; Settings (`settings`) and Logs (`logs` "Logs") rows with icons; Logs for the desktop owner only.
- Header across content, 60 px, 1 px `--fs-outline` bottom border: search field 520 × 36 (placeholder `searchPlaceholder` "Search files and folders") 24 px from the sidebar; right: host badge (28 px, pill, `--fs-surface-raised`, device icon, `hostBadgeDesktop` 13 px 600) and Monitor secondary button 96 × 36 (`monitor`).
- Content: breadcrumb 13 px (ancestors `--fs-on-surface-variant`, current 500 `--fs-on-surface`), page title 30 px 700 heading font, the view.
- Footer 40 px, 1 px `--fs-outline` top border: `fileSummary` left, commit state right, both 13 px `--fs-on-surface-variant`: `fileCommitted` "All changes committed to OCFL", `fileLibraryReady` "Library ready", `fileUploadJobsProgress`, `fileChangesPending`, `fileStorageNotReadable`.
- File panel 320 px at right, full height, when open.
- Header MUST NOT show permanent privacy text, the API status, or the theme control.

### UI-S-026 (built parts: rename form, refusals, view states)

- Rename dialog 560 × 250, title "Rename {name}" (`fileRenameOnlyTitle`) 20 px 700, Name field focused, Save (primary) and Cancel; refusals below the field in `--fs-error` with 2 px error border, form kept open with the typed name: `fileDisplayNameTooLong` (Save disabled), `fileRenameInvalid`, `fileRenameNotFound`, `fileRenameConflict`, `fileRenameUnavailable`.
- Remove toast 440 × 44 on `--fs-inverse-surface`: `fileRemoved` with Undo (`fileUndo`).
- View states (56): one centred treatment that replaces the list while header, sidebar and breadcrumb stay: title 15 px 600, detail 13 px, Try again secondary 36 px radius 7: `stateLoading`, `stateEmpty`, `stateError` + `fileActionFailed` + Try again, `stateOffline` + Try again, `stateDenied` + `authNotPermitted`, `stateUnavailable` + reason + Try again, `navigationGone`, `navigationFilesPending`.

### UI-S-027 library screens

- Welcome (01, 02) without the shell on `--fs-surface`: title "Your files. Private." 56 px 700 heading (`libraryWelcomeTitle`), lead `libraryIntro` 17 px, eyebrow LIBRARIES (`libraries`), three cards 332 × 248 radius 10 with 1.5 px `--fs-outline-strong` (Create card 2 px `--fs-primary`), titles 21 px 600 heading, help texts, buttons `libraryCreate` primary, `libraryOpen` and `libraryRecover` secondary; privacy box 1036 × 86 `--fs-surface-raised` radius 8 with device icon, "This device", `hostDesktop`, `hostDesktopNoTelemetry`; footer `appFooter` "Open source under the MIT licence · Version {version} · macOS". `libraryChoosing` below the card's button while the dialog is open; no path. Recent libraries (`libraryRecentTitle`, `libraryRecentMeta`, `libraryOpenRecent`) need API-S-038, not delivered: reported, not built.
- Folder-choice answers (03): `libraryCancelled` in the status line; unavailable `stateUnavailable` + `libraryChooserUnavailable` + Try again with the choices kept; `libraryConflictOpen`/`Create`/`Copy`; `libraryFailed` + Try again reusing the chosen folder.
- Recover (04): title 34 px; durable job (API-S-025/037, not delivered): reported.
- Library page (41): title the library name (API-S-038 name not delivered: `library` label); eyebrow ABOUT THIS LIBRARY with rows (labels 13 px `--fs-on-surface-muted`, values at 160 px): `libraryFactId` mono, `libraryFactFormat`, `libraryFactObjects`; storage rows and STORAGE BY COLLECTION need API-S-038: reported; eyebrow ACTIONS (`libraryActionsTitle`) with `libraryCopy` and `libraryRebuild` secondary and help; copy result box or W010 note box 448 × 64 radius 8, 1 px `--fs-error` border (answer 14), `libraryReadOnly` 13 px 600, `libraryReadOnlyReason` "Their latest version has no version inventory (W010).", ghost `libraryReviewRepair`.
- Repair (42, 43): title `repairTitle` 30 px, `repairIntro`, eyebrow `repairObjectsTitle` with mono `repairObject` lines, notes; `repairReview` primary; dialog 600 wide radius 10 on `--fs-surface-overlay` under a `#000000` 0.32 scrim: `repairDialogTitle` 22 px 700, `repairConsent`, mono objects, Cancel (first focus) and `repairConfirm` primary; results eyebrow `repairResultsTitle` lines; refused box with 1 px `--fs-error` border and `libraryBack`.
- Storage unavailable (34): strings need the unavailable state of the file browser (UI-F2.17): reported.

### UI-S-031 preview

- Preview bar: badge, display name 14 px 600, `filePosition`, previous and next; media on `--fs-surface-raised`; unsupported: badge, `fileNoPreview` 22 px 700 heading, `fileNoPreviewKeep` "FileSquid keeps the file and every version." and `fileNoPreviewOpenElsewhere` "Open it in the app that made it." (replace `fileNoPreviewHelp`). Download and Show in Finder, zoom chip and filmstrip belong to UI-F2.8 / UI-F2.15.

### UI-S-032 Monitor

- Title "Monitor" 30 px with `monitorOwnerOnly` "Only you can open this on this device." (13 px variant), `monitorLive` "Live" chip, Download secondary (`monitorExport` "Download JSON Lines"); cards on `--fs-surface-card` with 1 px `--fs-outline` radius 10; eyebrows HEALTH, PROCESSES AND JOBS, ACTIVITY JOURNAL etc. 11 px 400 muted; status word 22 px 700 heading; column heads 13 px 500 `--fs-on-surface-variant`; a stalled process's state and heartbeat in `--fs-error`; warn and error levels in `--fs-error`.
- OCFL validation (54): Running chip on the active tint, Idle muted; `findingsValidate` secondary; code chips outlined in `--fs-error` for errors and warnings alike (answer 14); `findingsRepair` ghost.

### UI-S-033 Settings and host popover

- Sidebar Settings row active; title "Settings" 30 px; sections with 22 px 600 heading font: Appearance (segmented System | Light | Dark, chosen 600 on the active tint; `themeHelp` "Follows the macOS appearance. Your choice is kept across launches."); Privacy (inline statement in a raised box: device icon, "This device", `hostDesktop`, `hostDesktopNoTelemetry` "FileSquid sends nothing off this computer. No account, no telemetry."); About (`settingsAbout` "About {product}", logo, "{product} {version}" 16 px 600, `settingsBuildNote`, `settingsLicence`, `settingsFormats`, right-aligned `settingsApiStatus` "API: {status} · contract {version}"). Browsing (UI-F2.8) and Recently deleted (UI-F2.17) join with their tasks.
- Host popover (40): badge button; popover 372 px wide, radius 10, `--fs-surface-overlay`, 1 px `--fs-outline-strong`; device icon; "This device" 14 px 600; `hostDesktop`; `hostDesktopBackup` only when backup or sync is on; ghost link `hostPrivacySettings`; Enter, Space, click open; Escape or outside click close and return focus to the badge.

### UI-S-015 token test

- Exempt pairs (FileSquid default only, both colours exactly at the listed value): light `--fs-on-primary` `#ffffff` on `--fs-primary` `#16ac49` 2.99; `--fs-primary` `#16ac49` on `#ffffff` (`--fs-surface`, `--fs-surface-card`, `--fs-surface-overlay`) 2.99; `--fs-on-primary-container` `#0f8a39` on `--fs-primary-container` `#f0faf4` 4.18; `--fs-error` `#e53935` on `#ffffff` (three surfaces) 4.23 and on `#f5f5f5` (`--fs-surface-container`, `--fs-surface-raised`) 3.88; `--fs-on-surface-muted` `#777777` on `#ffffff` (three) 4.48; `--fs-on-surface-placeholder` `#999999` on `#ffffff` (three) 2.85; dark `--fs-on-primary` `#ffffff` on `--fs-primary` `#16ac49` 2.99; `--fs-on-surface-muted` and `--fs-on-surface-placeholder` `#6a6a6a` on `--fs-surface` `#141414` 3.41 and `--fs-surface-card` `#1c1c1c` 3.15; both: `#ffffff` on `--fs-badge-document` `#fd1e23` 3.90 and on `--fs-badge-media` `#007af9` 4.08.
- Every other text pair 4.5:1; focus pairs (`--fs-focus-ring` on surface, surface-container, primary-container, surface-card, surface-raised, surface-overlay) and control-outline pairs (on surface, surface-card, surface-container, surface-raised, surface-overlay) 3:1, no exception. ImageSquid: no exemption.

### UI-S-016

- Shell: skip control first, `header`, `nav` named `navigationMain`, `main`; product name the only `h1`; focus ring 2 px `--fs-focus-ring` offset 2 px; motion from `--fs-motion-duration`; type sizes rem; no fixed pixel text height; below 40rem the navigation stacks above main.

### UI-S-005 strings to add (default and ImageSquid tables)

`fileNew`, `fileFilterView`, `fileRecent`, `navigationRecentlyDeleted`, `fileStorageUsed`, `fileStorageCaptionHistory`, `fileStorageFree`, `logs`, `searchPlaceholder`, `fileCommitted`, `fileLibraryReady`, `settingsAppearance`, `themeHelp`, `settingsAbout`, `settingsVersion`, `settingsBuildNote`, `settingsLicence`, `settingsFormats`, `settingsApiStatus`, `hostDesktopNoTelemetry`, `monitorOwnerOnly`, `monitorLive`, `libraryWelcomeTitle`, `appFooter`, `libraryReadOnlyReason`, `fileNoPreviewKeep`, `fileNoPreviewOpenElsewhere`, `fileUploadFiles`, `fileUploadFolder`; `monitorExport` reworded "Download JSON Lines"; `fileNoPreviewHelp` removed.

## Technical approach

1. Tokens: rewrite `src/theme/tokens.css` and `apps/imagesquid/src/theme/tokens.css`; extend `src/theme/tokens.ts` with the new required tokens, pairs, `--fs-focus-ring` focus pairs, `--fs-outline-control` pairs and an exemption list applied only by the default's test (`contrastFailures(css, exemptions?)`, `FILESQUID_CONTRAST_EXEMPTIONS`).
2. Shell: rebuild `<fs-app-shell>` as the UI-S-025 frame with named slots (`new`, `filter`, `views`, `navigation`, `search`, `summary`, `commit-state`, `panel`) and the storage block; move the theme control to Settings; drop the API status line; Logs row from the host descriptor (`fs-host` event of the indicator); `frameless` attribute for the welcome screen.
3. Settings: Appearance, Privacy, About sections; `settingsApiStatus` from `client.health()` and the contract version from the generated contract.
4. Restyle built surfaces: states, host indicator, nav tree (sections as eyebrows, icons, active bar, Recently deleted row), library pages, repair, preview, Monitor, findings, file-browser frame and rename form.
5. Tests first: unit (theme, component styles, labels), UI (shell, settings, theme control, host indicator, states, preview, tree, library, monitor), Playwright preview `mockup-fidelity.spec.ts` at 1440 × 900 in light and dark; update the server-backed specs; ImageSquid tests.
6. Docs: feature files, README token paragraph, plan and checklist boxes, walkthrough.

## Verification plan

- `npm test` (Vitest, jsdom) and `npx playwright test -c playwright.preview.config.ts` (production build + `vite preview`, port 4173) in `apps/filesquid/ui`; `npm test` and the preview suite in `apps/imagesquid` (port 4174).
- `npx prettier --write .`, `npx eslint . --fix`, `npx tsc --noEmit` in both packages; `bash scripts/pre-commit.sh` at the root (npm-only scope).
