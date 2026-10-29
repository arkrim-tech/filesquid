/**
 * The restore dialog of the Versions view (UI-F2.3, UI-S-031, mockup
 * `resources/ui-mockups/v3/27-version-restore.svg`): "Restore v{n} as a new version?" naming
 * the file, the source version's line, the new version's line on the active tint, the optional
 * label field, the note on the versions that stay, and Cancel and **Restore**, the dialog's
 * one filled primary button. {@link buildRestoreDialog} builds it from the label table and
 * returns typed references, so the Versions panel never queries it by selector; the panel
 * performs the restore. {@link labelProblem} checks a typed label as the route does.
 *
 * The dialog sits on `--fs-surface-overlay`, so it draws no muted, placeholder or error text
 * there except the refusal message (on the card-coloured message box); the active text is drawn
 * only on the active tint, the one surface its listed exemption covers (SRS-UI-007, UI-S-015).
 */
import { formatLabel, type LabelTable } from "../theme/labels";
import { formatModified, formatSize, validLabel, type FileRef } from "../files/model";
import { restoreKeepText, type VersionRow } from "../files/versions";
import { icon } from "./icons";

export { MAX_LABEL_BYTES } from "../files/model";

/** What the dialog names. */
export interface RestoreDialogParams {
  /** The active label table. */
  readonly labels: LabelTable;
  /** The file restored. */
  readonly file: FileRef;
  /** The version restored. */
  readonly row: VersionRow;
  /** The version the restore publishes. */
  readonly next: string;
  /** The file's versions newer than the one restored, oldest first. */
  readonly later: readonly string[];
  /** BCP 47 tag for numbers and dates. */
  readonly locale: string;
}

/** References into a built dialog. */
export interface RestoreDialog {
  /** The scrim that holds the dialog; append it to the panel's shadow root. */
  readonly scrim: HTMLElement;
  /** The dialog. */
  readonly dialog: HTMLElement;
  /** The optional label field. */
  readonly input: HTMLInputElement;
  /** The close button in the head. */
  readonly close: HTMLButtonElement;
  /** Cancel. */
  readonly cancel: HTMLButtonElement;
  /** Restore. */
  readonly restore: HTMLButtonElement;
  /** The message line for a refused label or restore. */
  readonly error: HTMLElement;
}

/** The dialog's own rules on top of `CONTROL_STYLE`'s `.scrim`, `.dialog` and buttons. */
const STYLE = `
  .restore-dialog { inline-size: min(33.75rem, 100%); gap: 0; padding: 0; }
  .restore-body { padding: var(--fs-space-4) var(--fs-space-4) var(--fs-space-5); }
  .restore-body > * { margin-inline: var(--fs-space-1); }
  .restore-head { display: flex; align-items: start; gap: var(--fs-space-3); }
  .restore-head .dialog-title { flex: 1 1 auto; min-width: 0; overflow-wrap: anywhere; }
  .icon-button { display: inline-flex; align-items: center; justify-content: center; inline-size: 1.75rem; block-size: 1.75rem; padding: 0; border: 0; border-radius: var(--fs-radius-item); background: none; color: var(--fs-icon-muted); cursor: pointer; }
  .icon-button:not(:disabled):hover { background: var(--fs-surface-raised); color: var(--fs-icon); }
  .icon-button:disabled { color: var(--fs-icon-disabled); cursor: default; }
  .file-name { margin-block: var(--fs-space-1) var(--fs-space-4); font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); overflow-wrap: anywhere; }
  .change-box { display: grid; gap: var(--fs-space-1); padding: var(--fs-space-3); border-radius: var(--fs-radius); background: var(--fs-surface-raised); }
  .change-box p { display: flex; align-items: center; gap: var(--fs-space-2); margin: 0; font-size: var(--fs-font-size-small); overflow-wrap: anywhere; }
  .source-line { color: var(--fs-on-surface); }
  .new-line { padding: var(--fs-space-1); border-radius: var(--fs-radius-item); background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  .new-line .mono-chip { background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  .change-box .arrow { display: flex; padding-inline-start: var(--fs-space-1); color: var(--fs-icon-muted); }
  .label-field { display: block; margin-block: var(--fs-space-3) var(--fs-space-2); font-size: var(--fs-font-size-small); font-weight: 600; color: var(--fs-on-surface); }
  .label-input { display: block; box-sizing: border-box; inline-size: calc(100% - 2 * var(--fs-space-1)); }
  .error { margin-block: var(--fs-space-2) 0; padding: var(--fs-space-2); border-radius: var(--fs-radius-item); background: var(--fs-surface-card); color: var(--fs-error); font-size: var(--fs-font-size-small); }
  .error:empty { display: none; }
  .keep { margin-block: var(--fs-space-3) var(--fs-space-2); font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .restore-dialog .dialog-actions { padding: var(--fs-space-3) var(--fs-space-4); }
  .restore-dialog .dialog-actions .button { margin-inline: 0; }
  .restore-dialog .dialog-actions .restore { margin-inline-end: var(--fs-space-1); }
`;

/** CSS text of the restore dialog. */
export const RESTORE_DIALOG_STYLE = STYLE;

/** Create an element with a class and optional text. */
function node<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  element.className = className;
  if (text !== undefined) element.textContent = text;
  return element;
}

/**
 * Check a typed label as the restorations route does ({@link validLabel}): blank means no
 * label; otherwise the trimmed text is sent, so it is 1 to 512 UTF-8 bytes after trimming and
 * has no control character. Pure.
 *
 * @param raw The label as typed.
 * @returns `invalid` when the route would refuse it, otherwise undefined.
 */
export function labelProblem(raw: string): "invalid" | undefined {
  const label = raw.trim();
  if (label === "") return undefined;
  return validLabel(label) ? undefined : "invalid";
}

/** One line of the change box: the version chip and its words. */
function changeLine(className: string, version: string, text: string): HTMLParagraphElement {
  const line = node("p", className);
  line.append(node("span", "mono-chip", version), node("span", "line-text", text));
  return line;
}

/** The source and new-version lines with the arrow between them. */
function changeBox(params: RestoreDialogParams): HTMLElement {
  const { labels, row, next, locale } = params;
  const source = formatLabel(labels.versionsRestoreSource, {
    label: row.label ?? labels.versionsNoLabel,
    date: formatModified(row.created, locale),
    size: row.size === null ? labels.versionsRemoved : formatSize(row.size, labels, locale),
  });
  const box = node("div", "change-box");
  const arrow = node("span", "arrow");
  arrow.append(icon("arrow-down", 16));
  box.append(
    changeLine("source-line", row.version, source),
    arrow,
    changeLine("new-line", next, formatLabel(labels.versionsRestoreNew, { version: row.version })),
  );
  return box;
}

/** The head: the title and the close button. */
function head(
  labels: LabelTable,
  version: string,
): { element: HTMLElement; close: HTMLButtonElement } {
  const element = node("div", "restore-head");
  const title = node("h2", "dialog-title", formatLabel(labels.versionsRestoreTitle, { version }));
  title.id = "restore-title";
  const close = node("button", "icon-button close");
  close.type = "button";
  close.setAttribute("aria-label", labels.versionsDialogClose);
  close.append(icon("close", 20));
  element.append(title, close);
  return { element, close };
}

/** The footer: Cancel and Restore. */
function actions(labels: LabelTable): {
  element: HTMLElement;
  cancel: HTMLButtonElement;
  restore: HTMLButtonElement;
} {
  const element = node("div", "dialog-actions");
  const cancel = node("button", "button secondary large cancel", labels.fileCancel);
  cancel.type = "button";
  const restore = node("button", "button primary large restore");
  restore.type = "submit";
  restore.append(icon("history"), document.createTextNode(labels.versionsRestore));
  element.append(cancel, restore);
  return { element, cancel, restore };
}

/**
 * Build the restore dialog for one version.
 *
 * @param params The file, the version, the next version, the kept versions and the words.
 * @returns The dialog and its references; nothing is attached or sent.
 */
export function buildRestoreDialog(params: RestoreDialogParams): RestoreDialog {
  const { labels, file, next, later } = params;
  const top = head(labels, params.row.version);
  const name = node("p", "file-name", file.display_name);
  name.id = "restore-file";
  const field = node(
    "label",
    "label-field",
    formatLabel(labels.versionsRestoreLabel, { version: next }),
  );
  field.htmlFor = "restore-label";
  const input = node("input", "input label-input");
  input.id = "restore-label";
  input.type = "text";
  input.autocomplete = "off";
  const error = node("p", "error");
  error.setAttribute("role", "alert");
  const buttons = actions(labels);
  const dialog = node("form", "dialog restore-dialog");
  dialog.noValidate = true;
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  dialog.setAttribute("aria-labelledby", "restore-title");
  dialog.setAttribute("aria-describedby", "restore-file");
  const body = node("div", "restore-body");
  body.append(top.element, name, changeBox(params), field, input, error);
  const keep = restoreKeepText(later, labels);
  if (keep) body.append(node("p", "keep", keep));
  dialog.append(body, buttons.element);
  const scrim = node("div", "scrim");
  scrim.append(dialog);
  return { scrim, dialog, input, close: top.close, error, ...buttons };
}
