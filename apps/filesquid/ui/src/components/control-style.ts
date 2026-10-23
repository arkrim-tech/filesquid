/**
 * The control stylesheet of the active mockup suite (UI-S-024), shared by every component so
 * buttons, inputs, eyebrows, chips and dialogs look the same on every screen and a distribution
 * restyles them through tokens alone:
 *
 * - `.button` 36 px tall, 14 px weight 600, radius 7 px; `.primary` filled `--fs-primary` with
 *   `--fs-on-primary` (its hover keeps the same colours, so the text pair stays the tested one);
 *   `.secondary` on `--fs-surface-card` with a 1.5 px `--fs-outline-strong`
 *   border; `.ghost` transparent with `--fs-primary` text; `.destructive` with `--fs-error` text
 *   and border; `.plain` transparent with body text; `.large` 38 px (the sidebar New button and
 *   dialog footers); disabled on `--fs-surface-raised` with a 1.5 px `--fs-outline` border and
 *   `--fs-on-surface-placeholder` text;
 * - `.input` 36 px tall, 14 px, radius 6 px, a 1.5 px `--fs-outline-control` border, a 2 px
 *   `--fs-focus-ring` border on focus and a 2 px `--fs-error` border with `[aria-invalid]`;
 * - `.eyebrow` 11 px weight 400 upper case with 0.1em tracking in `--fs-on-surface-muted`;
 * - `.chip` 11 px weight 500 status chips: `.ok` on the active tint, `.alert` outlined in
 *   `--fs-error`, `.muted` in the secondary colour; `.mono-chip` version chips;
 * - `.scrim` the dialog scrim (`--fs-scrim` at `--fs-scrim-opacity`) and `.dialog` the dialog
 *   frame on `--fs-surface-overlay` with radius 10 px.
 *
 * Fills are flat: no gradient, glow or shadow (UI-S-024).
 */

const STYLE = `
  .button { display: inline-flex; align-items: center; justify-content: center; gap: var(--fs-space-2); box-sizing: border-box; min-block-size: 2.25rem; padding: 0 var(--fs-space-3); border: 1.5px solid var(--fs-outline-strong); border-radius: var(--fs-radius-button); background: var(--fs-surface-card); color: var(--fs-on-surface); font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); font-weight: 600; line-height: 1.2; text-decoration: none; cursor: pointer; transition: background-color var(--fs-motion-duration), color var(--fs-motion-duration), border-color var(--fs-motion-duration); }
  .button:hover { background: var(--fs-surface-raised); }
  .button.large { min-block-size: 2.375rem; }
  .button.primary { border-color: var(--fs-primary); background: var(--fs-primary); color: var(--fs-on-primary); }
  .button.ghost { border-style: none; background: none; color: var(--fs-primary); }
  .button.ghost:hover { background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  .button.plain { border-style: none; background: none; color: var(--fs-on-surface); }
  .button.plain:hover { background: var(--fs-surface-raised); }
  .button.destructive { border-color: var(--fs-error); color: var(--fs-error); }
  .button.destructive:hover { background: var(--fs-surface-raised); }
  .button:disabled, .button[aria-disabled="true"], .button.button:disabled:hover, .button.button[aria-disabled="true"]:hover { border-color: var(--fs-outline); border-style: solid; background: var(--fs-surface-raised); color: var(--fs-on-surface-placeholder); cursor: default; }
  .input { box-sizing: border-box; min-width: 0; max-inline-size: 100%; min-block-size: 2.25rem; padding: 0 var(--fs-space-2); border: 1.5px solid var(--fs-outline-control); border-radius: var(--fs-radius-input); background: var(--fs-surface-card); color: var(--fs-on-surface); font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); transition: border-color var(--fs-motion-duration); }
  .input::placeholder { color: var(--fs-on-surface-placeholder); opacity: 1; }
  .input:focus-visible { outline: none; border: 2px solid var(--fs-focus-ring); }
  .input[aria-invalid="true"] { border: 2px solid var(--fs-error); }
  .input[aria-invalid="true"]:focus-visible { outline: 2px solid var(--fs-focus-ring); outline-offset: 2px; }
  .eyebrow { margin: 0; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-eyebrow); font-weight: 400; letter-spacing: 0.1em; text-transform: uppercase; color: var(--fs-on-surface-muted); }
  .chip { display: inline-flex; align-items: center; gap: var(--fs-space-1); box-sizing: border-box; padding: 0 var(--fs-space-2); border-radius: var(--fs-radius-pill); font-family: var(--fs-font-sans); font-size: var(--fs-font-size-eyebrow); font-weight: 500; line-height: 1.6; color: var(--fs-on-surface-variant); }
  .chip.ok { background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  .chip.alert { border: 1px solid var(--fs-error); color: var(--fs-error); }
  .chip.muted { background: var(--fs-surface-raised); color: var(--fs-on-surface-variant); }
  .mono-chip { display: inline-block; padding: 0 var(--fs-space-1); border-radius: var(--fs-radius-chip); background: var(--fs-surface-raised); font-family: var(--fs-font-mono); font-size: var(--fs-font-size-eyebrow); font-weight: 500; color: var(--fs-on-surface-variant); }
  .scrim { position: fixed; inset: 0; z-index: 3; display: grid; place-items: center; padding: var(--fs-space-4); }
  .scrim::before { content: ""; position: absolute; inset: 0; background: var(--fs-scrim); opacity: var(--fs-scrim-opacity); }
  .scrim > * { position: relative; }
  .dialog { display: grid; gap: var(--fs-space-3); box-sizing: border-box; inline-size: min(37.5rem, 100%); max-block-size: 100%; padding: var(--fs-space-4); border: 1px solid var(--fs-outline-strong); border-radius: var(--fs-radius-dialog); background: var(--fs-surface-overlay); color: var(--fs-on-surface); overflow: auto; }
  .dialog-title { margin: 0; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-heading); font-weight: 700; color: var(--fs-on-surface-heading); }
  .dialog-actions { display: flex; flex-wrap: wrap; justify-content: end; gap: var(--fs-space-2); padding-block-start: var(--fs-space-3); border-block-start: 1px solid var(--fs-outline); }
  .dialog-actions .button { min-block-size: 2.375rem; }
  .page-title { margin: 0; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-page); font-weight: 700; color: var(--fs-on-surface-heading); overflow-wrap: anywhere; }
  .section-title { margin: 0; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-heading); font-weight: 600; color: var(--fs-on-surface-heading); }
  .note-box { display: grid; gap: var(--fs-space-1); box-sizing: border-box; padding: var(--fs-space-3); border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface-card); }
  .note-box.error { border-color: var(--fs-error); }
  .note-box.raised { border-style: none; background: var(--fs-surface-raised); }
  .icon { flex: 0 0 auto; color: var(--fs-icon); }
  .icon.muted { color: var(--fs-icon-muted); }
  @media (forced-colors: active) {
    .button.primary, .chip.ok { forced-color-adjust: none; background: Highlight; color: HighlightText; }
  }
`;

/** CSS text of the shared control stylesheet; components add it after {@link A11Y_STYLE}. */
export const CONTROL_STYLE = STYLE;
