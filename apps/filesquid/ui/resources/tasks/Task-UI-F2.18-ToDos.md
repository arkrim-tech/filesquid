# Task UI-F2.18 ToDos

## Tokens (UI-S-024, UI-S-015, UI-S-016)

- [ ] Light and dark values of every colour token in the Plan table, in both the `data-theme="dark"` block and the `prefers-color-scheme` block (default and ImageSquid files)
- [ ] `--fs-primary` `#16ac49` / `--fs-on-primary` `#ffffff` in both themes; `--fs-on-primary-container` `#0f8a39` / `#4aca74`; `--fs-error` `#e53935` / `#ef5350`
- [ ] `--fs-outline-control` `#8e8e8e` / `#757575`; `--fs-focus-ring` `#0f8a39` / `#16ac49`
- [ ] Badge tokens and their text tokens; inverse surface; scrim `#000000` at 0.32
- [ ] Type-size tokens 10, 11, 13, 14, 16, 17, 20, 22, 30, 34, 56 px as rem
- [ ] Spacing `--fs-space-1..7` = 4, 8, 16, 24, 40, 64, 96 px; component sheets remapped off the 12 px step
- [ ] Radius tokens 7, 6, 10, 8, 5, 4, 3, 3.5 px and pill
- [ ] Token test: exempt pairs at exact values pass; one step off fails; unlisted pair below 4.5 fails; focus pairs below 3 fail; control-outline pairs below 3 fail; ImageSquid gets no exemption
- [ ] Focus ring rule uses `--fs-focus-ring` in `a11y-style.ts`; component-styles test updated
- [ ] No gradient, glow or shadow in any component stylesheet

## Shell (UI-S-025)

- [ ] Sidebar 240 px on `--fs-surface-card` with 1 px `--fs-outline` edge; logo mark 24 px; product name 17 px 700 heading font, the only `h1`
- [ ] New button slot (208 × 38, primary, `fileNew`); filter slot (208 × 34, `fileFilterView`); views All files and Recent (`fileRoot`, `fileRecent`)
- [ ] Navigation slot; 1 px divider above the storage block; Storage used block (`fileStorageUsed`, 13 px 600 value, 4 px bar radius 2, captions `fileStorageCaptionHistory`, `fileStorageFree`)
- [ ] Settings and Logs rows (`settings`, `logs`), Logs only on the desktop host; active row on `--fs-primary-container` with 2.5 px `--fs-brand` bar and 500 `--fs-on-primary-container` text
- [ ] Header 60 px with 1 px `--fs-outline` bottom border: search slot (520 × 36, `searchPlaceholder`), host badge (28 px pill `--fs-surface-raised`, 13 px 600), Monitor secondary button 96 × 36
- [ ] Footer 40 px with 1 px `--fs-outline` top border, summary and commit-state slots 13 px `--fs-on-surface-variant`
- [ ] File panel region 320 px when the `panel` slot is filled
- [ ] No theme control, no API status and no permanent privacy text in the header
- [ ] `frameless` attribute for the welcome screen; skip control, landmarks and stacking below 40rem kept

## Settings and host popover (UI-S-033, UI-S-015, UI-S-018)

- [ ] Title 30 px; sections with 22 px 600 heading font
- [ ] Appearance: `<fs-theme-control>` segmented (chosen 600 on the active tint), `themeHelp`
- [ ] Privacy: raised box with device icon, "This device", `hostDesktop`, `hostDesktopNoTelemetry`
- [ ] About: logo, `settingsVersion` 16 px 600, `settingsBuildNote`, `settingsLicence`, `settingsFormats`, `settingsApiStatus` "API: {status} · contract {version}" right-aligned; a denied or failed health read shown as an alert there
- [ ] Popover 372 px, radius 10, `--fs-surface-overlay`, 1 px `--fs-outline-strong`, "This device" 14 px 600, `hostDesktopBackup` only when on, ghost `hostPrivacySettings`; keyboard open and close kept

## Built surfaces (UI-S-026, UI-S-027, UI-S-031, UI-S-032, UI-S-017)

- [ ] View states: centred treatment, title 15 px 600, detail 13 px, Try again secondary 36 px radius 7
- [ ] Nav tree: eyebrows 11 px 400 upper case muted; rows 30 px; 16 px line icons; library parent rows 13 px 500 with chevron; children 13 px 400 variant; active bar; divider and single Recently deleted row last
- [ ] Welcome: `libraryWelcomeTitle` 56 px; lead 17 px; eyebrow LIBRARIES; cards 332 × 248 radius 10 with 1.5 px `--fs-outline-strong` (Create 2 px `--fs-primary`); 21 px 600 titles; primary Create, secondary Open and Recover; privacy box; `appFooter`
- [ ] Library page: ABOUT THIS LIBRARY rows (labels 13 px muted, values at 160 px, ID mono); ACTIONS with secondary buttons; W010 box 1 px `--fs-error` border with `libraryReadOnlyReason` and ghost `libraryReviewRepair`
- [ ] Repair: title 30 px, eyebrow, mono lines, primary Review repair; dialog 600 wide radius 10 overlay on a 0.32 scrim, 22 px title, Cancel first focus, primary confirm; results eyebrow; refused box with `libraryBack`
- [ ] Preview: 14 px 600 name, media on `--fs-surface-raised`, unsupported with `fileNoPreview` 22 px heading, `fileNoPreviewKeep`, `fileNoPreviewOpenElsewhere`
- [ ] Rename form: 20 px 700 title, 36 px input with 1.5 px `--fs-outline-control`, 2 px focus ring, 2 px error border and message in `--fs-error`, Save primary
- [ ] Monitor: title 30 px, `monitorOwnerOnly`, Live chip, Download secondary (`monitorExport` "Download JSON Lines"), cards radius 10, eyebrows, 22 px 700 status word, column heads 13 px 500, stalled rows in `--fs-error`
- [ ] Findings: Running chip on the active tint, code chips outlined in `--fs-error` for errors and warnings, secondary Validate now, ghost Review repair

## Strings (UI-S-005, answer 8 and 13)

- [ ] Every new key in `labels.en.json` and the ImageSquid table; `monitorExport` "Download JSON Lines"; `fileNoPreviewHelp` removed; `REQUIRED_LABEL_KEYS` updated

## Tests and docs

- [ ] Failing unit, UI and Playwright tests written and run first
- [ ] `mockup-fidelity.spec.ts`: 1440 × 900, light and dark, region boxes ±2 px, strings from the label table, computed styles, no console errors
- [ ] Server-backed specs and ImageSquid tests updated
- [ ] Feature files updated with the feature-writer skill; README token paragraph; plan and checklist boxes; MVP progress; walkthrough
