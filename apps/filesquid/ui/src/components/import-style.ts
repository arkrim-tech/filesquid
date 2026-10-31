/**
 * The stylesheets of the upload flow (UI-1.2, UI-S-024, UI-S-028, mockups 52, 18, 19 and 48),
 * tokens only so a distribution restyles them without code. {@link IMPORT_STYLE} is the upload
 * dialog's (destination step 680 px wide, review 860 px wide), {@link JOBS_STYLE} the Jobs
 * panel's (440 px wide, radius 10 px), and {@link BADGE_STYLE} the 30 × 16 px file-type badge
 * both draw. Added after `CONTROL_STYLE`, which owns buttons, inputs, chips and the scrim.
 */

/** The file-type badge: 30 × 16 px, 10 px mono 700, colours by token (mockups 18 and 19). */
export const BADGE_STYLE = `
  .badge { display: inline-flex; align-items: center; justify-content: center; flex: 0 0 auto; box-sizing: border-box; min-inline-size: 1.875rem; block-size: 1rem; padding: 0 var(--fs-space-1); border-radius: var(--fs-radius-badge); background: var(--fs-surface-raised); color: var(--fs-on-surface-variant); font-family: var(--fs-font-mono); font-size: var(--fs-font-size-badge); font-weight: 700; text-transform: uppercase; }
  .badge.document { background: var(--fs-badge-document); color: var(--fs-on-badge-document); }
  .badge.data { background: var(--fs-badge-data); color: var(--fs-on-badge-data); }
  .badge.media { background: var(--fs-badge-media); color: var(--fs-on-badge-media); }
`;

/** The error card of a refusal (mockup 48): the error icon, the title, the reason, Dismiss. */
export const REFUSAL_STYLE = `
  .refusal { display: grid; gap: var(--fs-space-2); justify-items: start; }
  .refusal .note-box { inline-size: 100%; grid-template-columns: auto 1fr; column-gap: var(--fs-space-2); align-items: start; }
  .refusal .note-box .icon { color: var(--fs-error); margin-block-start: 2px; }
  .refusal .title { margin: 0; font-size: var(--fs-font-size-ui); font-weight: 600; color: var(--fs-on-surface); }
  .refusal .reason { margin: 0; grid-column: 2; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
`;

/** The upload dialog: the destination step (mockup 52) and the review (mockup 18). */
export const IMPORT_STYLE = `
  .dialog.import, .dialog.review { display: flex; flex-direction: column; gap: 0; padding: 0; overflow: hidden; }
  .dialog.import { inline-size: min(42.5rem, 100%); block-size: min(42.5rem, 100%); }
  .dialog.review { inline-size: min(53.75rem, 100%); block-size: min(45rem, 100%); }
  .import .head, .review .head { position: relative; padding: 1.375rem 1.75rem 0; }
  .import .head .dialog-title, .review .head .dialog-title { line-height: 1.3; }
  .head .close { position: absolute; inset-block-start: 1.25rem; inset-inline-end: 1.25rem; }
  .intro, .volume { margin: 0.375rem 0 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .body { flex: 1 1 auto; min-height: 0; padding: 0 1.75rem; overflow: auto; }
  .import .body { display: flex; flex-direction: column; }
  fieldset.source { margin: 1.1875rem 0 0; padding: 0; border: 0; }
  fieldset.source legend, .field-label { padding: 0; font-size: var(--fs-font-size-small); font-weight: 600; color: var(--fs-on-surface); }
  .radios { display: flex; gap: var(--fs-space-4); margin-block-start: 0.6875rem; }
  .radios label { position: relative; display: inline-flex; align-items: center; gap: var(--fs-space-2); font-size: var(--fs-font-size-ui); color: var(--fs-on-surface); cursor: pointer; }
  .radios input { appearance: none; flex: 0 0 auto; box-sizing: border-box; inline-size: 1rem; block-size: 1rem; margin: 0; border: 1.5px solid var(--fs-outline-control); border-radius: var(--fs-radius-pill); background: var(--fs-surface-card); }
  .radios input:checked { border-color: var(--fs-primary); }
  .radios input:checked::after { content: ""; display: block; inline-size: 0.5rem; block-size: 0.5rem; margin: 0.1875rem; border-radius: var(--fs-radius-pill); background: var(--fs-primary); }
  .radios input:focus-visible { outline: 2px solid var(--fs-focus-ring); outline-offset: 2px; }
  .field-head { display: flex; align-items: baseline; gap: var(--fs-space-2); margin-block-start: 2.025rem; }
  .field-head .tag { font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .destination-field { inline-size: 100%; margin-block-start: 0.4375rem; font-size: var(--fs-font-size-ui); }
  .destination-field:read-only { cursor: default; }
  .picker { flex: 0 0 auto; block-size: 18.75rem; margin-block-start: var(--fs-space-2); }
  .picker fs-destination-picker { block-size: 100%; }
  .help { margin: 0.75rem 0 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .import .refusal, .review .refusal { margin-block-start: var(--fs-space-2); }
  .planning { display: grid; justify-items: start; gap: var(--fs-space-2); margin-block-start: var(--fs-space-4); padding: var(--fs-space-3); border-radius: var(--fs-radius); background: var(--fs-surface-raised); color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  .foot { display: flex; align-items: flex-start; gap: var(--fs-space-2); box-sizing: border-box; min-block-size: 5.125rem; padding: 1.125rem 1.75rem 0; border-block-start: 1px solid var(--fs-outline); }
  .review .foot { min-block-size: 4rem; padding-block-start: 1rem; }
  .foot .reason { display: flex; align-items: center; flex: 1 1 auto; min-block-size: 2.25rem; margin: 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .foot .spacer { flex: 1 1 auto; }
  .cards { display: flex; gap: 0.75rem; margin-block-start: 1.4rem; }
  .cards .card { box-sizing: border-box; inline-size: 12rem; min-block-size: 4.5rem; padding: var(--fs-space-2) var(--fs-space-3); border-radius: var(--fs-radius); background: var(--fs-surface-raised); }
  .cards .count { margin: 0; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-card); font-weight: 600; color: var(--fs-on-surface-heading); }
  .cards .caption { margin: 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .files-head { margin: 1.5rem 0 0; }
  .files { box-sizing: border-box; max-block-size: 14.75rem; margin: 0.4375rem 0 0; padding: 0.34375rem 0 0; list-style: none; border: 1.5px solid var(--fs-outline-strong); border-radius: var(--fs-radius); background: var(--fs-surface-card); overflow: auto; }
  .files li { display: flex; align-items: center; gap: var(--fs-space-2); box-sizing: border-box; min-block-size: 2.375rem; padding: 0 var(--fs-space-3); border-block-end: 1px solid var(--fs-outline); font-size: var(--fs-font-size-small); }
  .files li:last-child { border-block-end: 0; }
  .files .mark { display: inline-flex; align-items: center; justify-content: center; flex: 0 0 auto; box-sizing: border-box; inline-size: 1rem; block-size: 1rem; border: 1.5px solid var(--fs-outline-control); border-radius: var(--fs-radius-check); background: var(--fs-surface-card); color: var(--fs-on-primary); }
  .files li[data-present="false"] .mark { border-color: var(--fs-primary); background: var(--fs-primary); }
  .files .name { flex: 1 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--fs-font-mono); color: var(--fs-on-surface); }
  .files .size { flex: 0 0 auto; color: var(--fs-on-surface-variant); }
  .files .status { flex: 0 0 9.5rem; color: var(--fs-on-surface-variant); }
  .files-note { margin: var(--fs-space-1) 0 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-muted); }
  .two { display: grid; grid-template-columns: 30.75rem 1fr; gap: 0; margin-block-start: 1.4375rem; align-items: start; }
  .chosen { display: flex; align-items: center; gap: 0.75rem; }
  .chosen .destination-field { flex: 0 0 25rem; inline-size: 25rem; margin: 0.4375rem 0 0; }
  .change { margin-block-start: 0.4375rem; padding-inline: 0; white-space: nowrap; }
  .review .note { margin: 0.3125rem 0 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .space p { margin: var(--fs-space-1) 0 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .space .field-label { margin: 0; }
  .integrity { display: grid; grid-template-columns: auto 1fr; gap: 0 var(--fs-space-2); align-items: start; margin-block-start: 1.375rem; padding: var(--fs-space-3); border-radius: var(--fs-radius); background: var(--fs-surface-raised); font-size: var(--fs-font-size-small); }
  .integrity .icon { grid-row: span 2; }
  .integrity p { margin: 0; color: var(--fs-on-surface-variant); }
  .integrity p.title { font-weight: 600; color: var(--fs-on-surface); }
  [hidden] { display: none !important; }
`;

/** The Jobs panel (mockup 19): 440 px wide, radius 10 px, at the right under the header. */
export const JOBS_STYLE = `
  :host { display: contents; }
  .jobs-button { position: relative; display: inline-flex; align-items: center; justify-content: center; box-sizing: border-box; inline-size: 1.75rem; block-size: 1.75rem; padding: 0; border: 0; border-radius: var(--fs-radius-button); background: var(--fs-primary-container); color: var(--fs-primary); cursor: pointer; }
  .jobs-button:hover { background: var(--fs-surface-raised); }
  .jobs-button[aria-expanded="true"] { background: var(--fs-primary-container); }
  .jobs-button .badge-count { position: absolute; inset-block-start: -0.375rem; inset-inline-end: -0.375rem; display: inline-flex; align-items: center; justify-content: center; box-sizing: border-box; min-inline-size: 1rem; block-size: 1rem; padding: 0 var(--fs-space-1); border-radius: var(--fs-radius-pill); background: var(--fs-primary); color: var(--fs-on-primary); font-size: var(--fs-font-size-badge); font-weight: 700; line-height: 1; }
  .jobs-button .badge-count[hidden] { display: none; }
  .panel { position: fixed; z-index: 4; inset-block-start: 3.5rem; inset-inline-end: var(--fs-space-4); display: flex; flex-direction: column; box-sizing: border-box; inline-size: min(27.5rem, calc(100vw - 2 * var(--fs-space-3))); block-size: min(42.5rem, calc(100vh - 4rem)); border: 1px solid var(--fs-outline-strong); border-radius: var(--fs-radius-dialog); background: var(--fs-surface-overlay); color: var(--fs-on-surface); font-family: var(--fs-font-sans); }
  .panel[hidden] { display: none; }
  .panel-head { display: flex; align-items: center; justify-content: space-between; padding: var(--fs-space-3) 1.1875rem 0.25rem; }
  .panel-close { min-block-size: 1.75rem; padding: 0 var(--fs-space-1); }
  .panel-title { margin: 0; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-title); font-weight: 700; color: var(--fs-on-surface-heading); }
  .panel-body { flex: 1 1 auto; min-height: 0; padding: 0 1.1875rem; overflow: auto; }
  .job { display: grid; gap: var(--fs-space-2); padding: 0.75rem 0; border-block-end: 1px solid var(--fs-outline); }
  .job .job-line { margin-block-start: -0.25rem; }
  .job-top { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--fs-space-2); }
  .job-title { margin: 0; font-size: var(--fs-font-size-ui); font-weight: 600; color: var(--fs-on-surface); overflow-wrap: anywhere; }
  .job-line { display: flex; align-items: center; flex-wrap: wrap; gap: var(--fs-space-2); font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .chip { min-block-size: 1.375rem; }
  .bar { block-size: 4px; border-radius: 2px; background: var(--fs-outline); overflow: hidden; }
  .bar .fill { block-size: 100%; border-radius: 2px; background: var(--fs-primary); }
  .actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: var(--fs-space-2); }
  .actions .button, .job-file .retry { gap: 0.375rem; min-block-size: 1.875rem; min-inline-size: 5.375rem; padding-inline: 0.6875rem; }
  .job .eyebrow { margin-block-start: 0.25rem; }
  .job-files { display: grid; gap: 0; margin: var(--fs-space-1) 0 0; padding: 0; list-style: none; }
  .job-file { position: relative; display: grid; grid-template-columns: auto minmax(0, 1fr) auto auto; align-items: center; gap: var(--fs-space-1) var(--fs-space-2); min-block-size: 2rem; font-size: var(--fs-font-size-small); }
  .job-file .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--fs-on-surface); }
  .job-file .where { color: var(--fs-on-surface-muted); white-space: nowrap; }
  .job-file .state { justify-self: end; color: var(--fs-on-surface-variant); white-space: nowrap; }
  .job-file[data-state="copied"] .state { color: var(--fs-on-primary-container); }
  .job-file[data-state="queued"] .state { color: var(--fs-on-surface-muted); }
  .job-file[data-state="failed"] .state { font-weight: 600; color: var(--fs-error); }
  .job-file .bar.small { position: absolute; inset-block-start: 1.125rem; inset-inline-start: 2.5rem; inline-size: 12.5rem; block-size: 3px; border-radius: 1.5px; }
  .job-file .bar.small .fill { border-radius: 1.5px; }
  .job-file .retry { grid-column: 3 / 5; justify-self: end; }
  .job-file .refusal { grid-column: 1 / -1; }
  .empty { margin: var(--fs-space-3) 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .panel-note { display: grid; grid-template-columns: auto 1fr; gap: var(--fs-space-2); margin: var(--fs-space-3) 1.1875rem 0.5rem; padding: 0.75rem var(--fs-space-3); border-radius: var(--fs-radius); background: var(--fs-surface-raised); font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .panel-note p { margin: 0; line-height: 1.55; }
  .alert { margin: var(--fs-space-2) 0 0; font-size: var(--fs-font-size-small); color: var(--fs-error); }
  [hidden] { display: none !important; }
`;
