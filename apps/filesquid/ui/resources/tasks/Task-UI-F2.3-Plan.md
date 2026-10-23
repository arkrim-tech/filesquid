# Task UI-F2.3 Plan: Details and Versions panels

Branch `feat/ui-f2.3-details-versions-panels`. Plan row: [Task UI-F2.3](../plan.md). Dependencies: UI-F2.2 (done), UI-F2.18 (done, tokens, shell and panel region this task builds on).

## Requirements

- [PRD F-055](../../../../../resources/prd.md#f-055-default-ui-and-panel-composition) Default UI and panel composition; [C-055](../../../../../resources/product-vision.md#module-rules).
- [UI-S-003](../srs.md) Panels, including the file panel's tabs, plug into named sidebar slots and do not depend on each other; each file fact is shown by the panel of its owning module.
- [UI-S-009](../srs.md) Versions view opened from the file panel header (composition UI-S-031, 26, 27).
- [UI-S-024](../srs.md) Visual constants of the active mockup suite; [UI-S-031](../srs.md) Preview and file panel composition.
- [OCFL-S-012](../../../crates/ocfl/resources/srs.md) storage facts and version history.
- Mockups (v3): [35-panel-file-info](../../../../../resources/ui-mockups/v3/35-panel-file-info.svg), [26-panel-versions](../../../../../resources/ui-mockups/v3/26-panel-versions.svg), [27-version-restore](../../../../../resources/ui-mockups/v3/27-version-restore.svg), [25-panel-states](../../../../../resources/ui-mockups/v3/25-panel-states.svg).
- Gherkin: [composable_filesquid_ui_F-055.feature](../../tests/features/composable_filesquid_ui_F-055.feature), [file_version_history_F-065.feature](../../tests/features/file_version_history_F-065.feature).

## API routes used (all on main, contract 0.24.0)

- `GET /api/v1/files/details` (API-F1.3): storage facts (`FileDetails`).
- `GET /api/v1/files/history` (API-F1.3): the versions that changed one file, oldest first, with `size` (`FileHistory`).
- `GET /api/v1/files/versions` (API-F2.5): every version of the object with its `head` (`ObjectVersions`).
- `GET /api/v1/files/history-size` (API-F2.5): bytes the file's history keeps (`FileHistorySize`).
- `POST /api/v1/files/restorations` (API-F2.5) through the existing `FileSource.restore`, extended with the optional `label` member the route already takes. API-F2.13 (PR #135) moves this route to a command envelope through the same method.

No route, field or event is assumed or stubbed.

## Approach (holistic-ui rule)

UI-F2.11 extends the Versions view (download, edit label, history size in storage used) and UI-F2.15 turns the file panel into three tabs sharing one table component. So this task builds the structure they extend, not a throwaway:

- `<fs-file-panel>`: the file panel frame of UI-S-031 (header with badge, display name, history button and close; a tab strip whose tabs are registered panels in named slots, `registerTab(id, label, create)`; the Versions view shown in place of the tabs by the history button). Mounted by the files route into the shell's `panel` slot and fed the opened `FileRef` from `fs-file-open`.
- `property-table.ts`: the shared property/value table (filter field, count eyebrow, rows of muted label over value with copy, loading, empty, no-match and error states, footer line) that UI-F2.15 reuses for Description and Provenance.
- `<fs-details-panel>`: the File info tab with the storage facts of `GET /files/details` and the version total of `GET /files/versions`.
- `<fs-versions-panel>`: the Versions view (Back, count, eyebrow with the history size, 62 px rows newest first, Current chip, row menu with Restore as new version, footer note) and the restore dialog of mockup 27.
- `HistorySource` / `ClientHistorySource` (new file, reads only) so PR #135's edits to `source.ts` and `memory-source.ts` do not collide.
- Shell: the panel region moves under the header, beside the main column and the footer (mockups 35, 26: header 1200 px wide, footer 880 px, panel 320 × 840 from y 61).
- `FsFileBrowser.reload()` becomes public so the files route reloads the listing after a restore.

Not built here (later tasks): Download this version and Edit label (UI-F2.11); Uploaded and Source rows, the format name and the RESOURCES IN THIS FILE list (the facts route belongs to UI-F2.15, and no route lists a logical file's resources); Description and Provenance tabs and View in Logs (UI-F2.4, UI-F2.15, UI-F2.16); panel opening on row selection (UI-F2.8). The stored name and the logical path are never shown (UI-S-019; mockup 35 desc), and the content location is never answered by the API (API-S-005).

## Granular constraints (UI-S-024, UI-S-031, mockups)

Colours (light / dark), each through its token:
- Panel `--fs-surface-card` #ffffff / #1c1c1c with a 1 px `--fs-outline` #e0e0e0 / #2a2a2a left border.
- Display name 14 px 600 `--fs-on-surface-heading` #000000 / #f5f5f5, truncated with an ellipsis.
- Tabs 13 px; active 600 `--fs-on-primary-container` #0f8a39 / #4aca74 with a 2 px `--fs-primary` #16ac49 underline; others 500 `--fs-on-surface-variant` #555555 / #a0a0a0.
- Filter field: 32 px tall (mockup), radius 6 px, 1.5 px `--fs-outline-control` #8e8e8e / #757575 border, placeholder `--fs-on-surface-placeholder` #999999 / #6a6a6a, 2 px `--fs-focus-ring` #0f8a39 / #16ac49 border on focus.
- Eyebrows 11 px 400 upper case, letter-spacing 0.1em, `--fs-on-surface-muted` #777777 / #6a6a6a.
- Property rows 44 px: label 13 px muted (UI-S-024 minimum; UI-S-031 says 12 px, mockup draws 13), value 13 px `--fs-on-surface` #111111 / #e0e0e0, mono `--fs-font-mono` for extension, digest and identifiers; 1 px `--fs-outline` divider; hovered row on `--fs-surface-raised` #f5f5f5 / #242424 with the label in `--fs-on-surface-variant` (muted text is not drawn on raised: no listed exemption).
- Copy icon 16 px `--fs-icon-muted` #777777 / #6a6a6a, `--fs-icon` #111111 / #a0a0a0 on hover and focus.
- Footer line 13 px muted above a 1 px `--fs-outline` border, 44 px.
- Versions: Back 13 px 500 variant; count 13 px muted; rows 58 px with a 62 px pitch (hover rect of mockup 26: 304 × 58, radius 6 px on `--fs-surface-raised`); timeline dot 10 px, current filled `--fs-brand` #16ac49, others `--fs-surface-card` with a 1.5 px `--fs-outline-strong` #cccccc / #333333 ring; connector 1.5 px `--fs-outline`; version chip mono 11 px 500 `--fs-on-surface-variant` on `--fs-surface-raised`, radius 4 px; label 14 px 600 body; "No label" 13 px italic muted (variant on a raised row); Current chip 11 px 500 on `--fs-primary-container` #f0faf4 / #0d2b18 with `--fs-on-primary-container` text, radius 4 px; meta "{relative time} · {size}" 13 px variant.
- Row menu: `--fs-surface-overlay` #ffffff / #2c2c2c, 1 px `--fs-outline-strong` border, radius 8 px, items 34 px radius 5 px 13 px body, hovered item `--fs-surface-raised`, 16 px icons.
- Footer note: `--fs-surface-raised`, radius 8 px, 296 × 72, info icon 16 px, title 13 px 600 body, text 13 px variant.
- Restore dialog: 540 px wide on `--fs-surface-overlay` with a 1 px `--fs-outline-strong` border, radius 10 px, over the scrim `--fs-scrim` #000000 at `--fs-scrim-opacity` 0.32; title Georgia 22 px 700 heading; file name 13 px variant; the version box on `--fs-surface-raised`, radius 8 px; the new-version line on the active tint (UI-S-031) with `--fs-on-primary-container` text (the only listed active-text exemption, #0f8a39 on #f0faf4 4.18:1); label 13 px 600 body; input 36 px radius 6 px; note 13 px variant; a 1 px `--fs-outline` divider; Cancel secondary and Restore primary, 38 px, 14 px 600, radius 7 px; one filled primary per dialog.
- Error state icon in `--fs-error` #e53935 / #ef5350; no warning colour.
- Fills flat: no gradient, glow or shadow.

Type and sizes: buttons 14 px 600 and 36 px (38 px in dialog footers), meta text at least 13 px, eyebrows 11 px 400, mono for identifiers, codes and digests; sizes as `rem` tokens.

Spacing and radii: the 8 px scale `--fs-space-1` to `--fs-space-7` (4, 8, 16, 24, 40, 64, 96); radii buttons 7, inputs 6, dialogs and cards 10, menus 8, menu items 5, chips 4, badges 3.

Geometry at 1440 × 900 (mockup y minus 28 for the title bar):
- Header 1200 × 60 at x 240; footer 880 × 40 at x 240, y 860 while the panel shows; panel 320 wide at x 1120 from y 61 to 900.
- Panel: badge x 1140; history button 30 × 28 at y 82; tab underline at y 150; filter 280 × 32 at x 1140, y 168; first property row y 238, 44 px; footer 44 px at y 856.
- Versions: rows from y 188 with a 62 px pitch; menu 232 × 110; footer note 296 × 72 at x 1132, y 776.
- Restore dialog 540 wide at x 450.
- Icons: 1.6 px line icons at 16, 20 or 24 px.

Accessibility (UI-S-016): shared stylesheet first; tabs as `role="tablist"` / `tab` / `tabpanel` with arrow keys and a roving Tab stop (no positive tabindex); history button `aria-pressed`; menu button `aria-haspopup="menu"` and `aria-expanded`, menu items `role="menuitem"`, Escape closes and returns focus; the restore dialog `role="dialog"`, `aria-modal`, labelled by its title, Tab kept inside, Escape cancels and returns focus to the menu button; 2 px `--fs-focus-ring` ring; logical properties only; motion from `--fs-motion-duration`; no fixed pixel height on text containers.

Strings: every word from the label table; new keys under `filePanel*` and `versions*` in the default and ImageSquid tables (UI-S-005).

## Verification

- Vitest (jsdom): pure models (version rows, relative time, short digest and identifier, detail rows), `ClientHistorySource` paths, the restore label body, `<fs-file-panel>`, `<fs-details-panel>`, `<fs-versions-panel>`, the shell's panel layout, component-style rules for the new elements, label completeness.
- Playwright against `vite build` plus `vite preview` at 1440 × 900, light and dark: elements defined, labels from the table, computed styles, boxes within 2 px for 35, 26, 27 and the 25 states (loading, no match, error), no console errors; restore sends one call with the label and the list reloads.
- Gherkin scenarios for UI-S-003, UI-S-009 and UI-S-031 in the two feature files.
