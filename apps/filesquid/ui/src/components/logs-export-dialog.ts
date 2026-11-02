/**
 * The Download logs dialog of the Logs view (UI-F2.16; UI-S-021, UI-S-032, mockup
 * `resources/ui-mockups/v3/39-logs-export.svg`): a 560 x 632 px `role="dialog"` modal on the
 * shared scrim. **Scope** is a radio group (Current filter, Time range, All entries, each with
 * its line), **Format** two 248 x 58 px option cards (JSON Lines, CSV; the chosen one on
 * `--fs-primary-container` with a 1.5 px `--fs-primary` border), then the note that a download
 * keeps opaque IDs only, the File name (mono) with **Choose…** (the platform's save dialog, where
 * the browser has one; otherwise it is disabled and says so) and **Cancel** and **Download**.
 *
 * Download reads the export through the route for the scope (`LogsSource.export`, the filtered,
 * ranged or whole journal), saves it, then closes; while it runs the buttons are disabled and a
 * status line says so; when it fails an alert says nothing was saved and the dialog stays open
 * with the choices as they were. Focus starts on the first radio and stays inside; Escape closes.
 * Every word comes from the label table, every colour from a token.
 */
import type { JournalFilters } from "../api/client";
import {
  exportFileName,
  exportFilterLine,
  exportQuery,
  utcOffsetLabel,
  type ExportChoice,
  type ExportFormat,
  type ExportScope,
  type LoadedCount,
  type LogsFilterState,
  type LogsLibrary,
} from "../logs/model";
import { formatLabel, type LabelTable } from "../theme/labels";
import { showDialog, type OpenDialog } from "./drive-dialogs";
import { icon } from "./icons";
import { node } from "./logs-view-parts";
import { pickSaveFile, writeToPickedFile, type SaveTarget } from "./save-file";

/** The stylesheet of the dialog, added after the shared control stylesheet and the Drive dialogs'. */
export const EXPORT_STYLE = `
  .dialog.logs-export { box-sizing: border-box; inline-size: min(35rem, 100%); min-block-size: 39.5rem; gap: 0; padding: 1.5rem 1.75rem 0; grid-template-rows: auto; align-content: start; }
  .dialog.logs-export .dialog-title { margin: 0; line-height: 1.2; }
  .logs-export .sub { margin: var(--fs-space-1) 0 0; color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  .logs-export fieldset { margin: 0; padding: 0; border: 0; min-inline-size: 0; }
  .logs-export legend { padding: 0; font-size: var(--fs-font-size-small); font-weight: 600; color: var(--fs-on-surface); }
  .logs-export .scope { margin-block-start: 2.0625rem; }
  .scrim:has(.dialog.logs-export) { align-items: start; padding-block-start: 5.125rem; }
  .logs-export .scope legend { margin-block-end: var(--fs-space-2); }
  .logs-export .scope-option { display: grid; grid-template-columns: 1rem minmax(0, 1fr); column-gap: var(--fs-space-2); align-items: start; min-block-size: 2.875rem; padding-block: 0.1875rem 0; cursor: pointer; }
  .logs-export .scope-option .words { display: grid; gap: 0; }
  .logs-export .scope-option .name { font-size: var(--fs-font-size-small); font-weight: 500; color: var(--fs-on-surface); line-height: 1.3; }
  .logs-export .scope-option .line { color: var(--fs-on-surface-muted); font-size: var(--fs-font-size-small); line-height: 1.3; overflow-wrap: anywhere; }
  .logs-export .range-fields { display: flex; flex-wrap: wrap; gap: var(--fs-space-2); margin-block-start: var(--fs-space-1); }
  .logs-export .range-fields label { display: grid; gap: 2px; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .logs-export .range-fields .input { min-block-size: 2rem; font-size: var(--fs-font-size-small); }
  .logs-export input[type="radio"] { appearance: none; position: relative; inline-size: 1rem; block-size: 1rem; margin: 0; border: 1.5px solid var(--fs-outline-control); border-radius: var(--fs-radius-pill); background: var(--fs-surface-overlay); }
  .logs-export input[type="radio"]:checked { border: 2px solid var(--fs-primary); }
  .logs-export input[type="radio"]:checked::before { content: ""; position: absolute; inset: 3px; border-radius: var(--fs-radius-pill); background: var(--fs-primary); }
  .logs-export .format { margin-block-start: 0.125rem; }
  .logs-export .format legend { margin-block-end: var(--fs-space-2); }
  .logs-export .cards { display: flex; gap: var(--fs-space-2); }
  .logs-export .card { display: grid; grid-template-columns: 1rem minmax(0, 1fr); grid-template-rows: auto auto; column-gap: 0.625rem; row-gap: 0.125rem; align-content: center; box-sizing: border-box; flex: 0 0 15.5rem; max-inline-size: 100%; min-block-size: 3.625rem; padding: 0 var(--fs-space-2) 0 0.625rem; border: 1.5px solid var(--fs-outline-strong); border-radius: var(--fs-radius); background: var(--fs-surface-overlay); cursor: pointer; }
  .logs-export .card:has(input:checked) { border-color: var(--fs-primary); background: var(--fs-primary-container); }
  .logs-export .card .name { grid-column: 2; font-size: var(--fs-font-size-small); font-weight: 600; color: var(--fs-on-surface); line-height: 1.2; }
  .logs-export .card .hint { grid-column: 2; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); line-height: 1.2; }
  .logs-export .card input { grid-row: 1; align-self: center; }
  .logs-export .note-box.raised { display: flex; align-items: start; gap: var(--fs-space-2); margin-block-start: 1.25rem; padding: 0.75rem var(--fs-space-3); border-radius: var(--fs-radius); font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .logs-export .note-box p { margin: 0; }
  .logs-export .file { margin-block-start: 1.375rem; display: grid; gap: var(--fs-space-2); }
  .logs-export .file label { font-size: var(--fs-font-size-small); font-weight: 600; color: var(--fs-on-surface); }
  .logs-export .file-row { display: flex; align-items: center; gap: 0.75rem; }
  .logs-export .file-row .input { flex: 1 1 auto; font-family: var(--fs-font-mono); font-size: var(--fs-font-size-small); }
  .logs-export .file-row .button { min-block-size: 1.75rem; min-inline-size: 6.25rem; padding: 0 0.75rem; }
  .logs-export .where { margin: 0; color: var(--fs-on-surface-muted); font-size: var(--fs-font-size-small); }
  .logs-export .failed { margin: var(--fs-space-2) 0 0; color: var(--fs-error); font-size: var(--fs-font-size-small); }
  .logs-export .failed:empty, .logs-export .busy:empty { display: none; }
  .logs-export .busy { margin: var(--fs-space-2) 0 0; color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  .logs-export .dialog-actions { margin: 1.125rem -1.75rem 0; padding: 1.125rem 1.75rem 1rem; }
  .logs-export .dialog-actions { gap: 0.75rem; }
  .logs-export .dialog-actions .button { inline-size: 5.625rem; padding: 0; }
`;

/** What the dialog needs. */
export interface ExportDialogOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** Where the dialog is shown (inside the view's shadow root). */
  readonly layer: HTMLElement;
  /** The control that opened it, which gets focus back on close. */
  readonly opener: HTMLElement | null;
  /** The view's filter, for the Current filter scope. */
  readonly filter: LogsFilterState;
  /** The libraries the host opened, for the filter line. */
  readonly libraries: readonly LogsLibrary[];
  /** What the view has read, for the count in the filter line. */
  readonly loaded?: LoadedCount;
  /** The time of the oldest entry, for the All entries line. */
  readonly oldest?: string;
  /** The time the download is made at. */
  readonly now: Date;
  /** The IANA zone dates are shown in; the viewer's when absent. */
  readonly zone?: string;
  /** The page's language. */
  readonly lang: string;
  /** Reads the export of a scope (`LogsSource.export`). */
  readonly run: (query: Omit<JournalFilters, "after" | "limit" | "names">) => Promise<string>;
  /** Saves the export through the browser's download. */
  readonly save: (blob: Blob, name: string) => void;
  /** Whether the platform's save dialog is offered. */
  readonly canChoose: boolean;
  /** Opens the platform's save dialog; undefined when the person cancelled it. */
  readonly choose?: (suggested: string, format: ExportFormat) => Promise<SaveTarget | undefined>;
  /** Told after a successful download, with the saved name. */
  readonly onDone?: (name: string) => void;
}

/** The choices of the open dialog. */
interface State {
  scope: ExportScope;
  format: ExportFormat;
  from: string;
  to: string;
  name: string;
  edited: boolean;
  busy: boolean;
  target?: SaveTarget;
}

/** The media type and extension of a format. */
const FORMAT_FILE: Readonly<Record<ExportFormat, { mime: string; extension: string }>> = {
  jsonl: { mime: "application/x-ndjson", extension: ".jsonl" },
  csv: { mime: "text/csv", extension: ".csv" },
};

/** `YYYY-MM-DDTHH:mm`, the value of a datetime-local input, for an instant in the browser's zone. */
function inputValue(at: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${at.getFullYear()}-${pad(at.getMonth() + 1)}-${pad(at.getDate())}T${pad(at.getHours())}:${pad(at.getMinutes())}`;
}

/** A radio with the dialog's look, in a group. */
function radio(name: string, value: string, checked: boolean): HTMLInputElement {
  const input = node("input", "");
  input.type = "radio";
  input.name = name;
  input.value = value;
  input.checked = checked;
  return input;
}

/** One scope option: the radio, its name and its line. */
function scopeOption(
  scope: ExportScope,
  name: string,
  line: string,
  checked: boolean,
): { label: HTMLLabelElement; line: HTMLElement } {
  const label = node("label", "scope-option");
  const lineElement = node("span", "line", line);
  const words = node("span", "words");
  words.append(node("span", "name", name), lineElement);
  label.append(radio("scope", scope, checked), words);
  return { label, line: lineElement };
}

/** One format card. */
function formatCard(
  format: ExportFormat,
  name: string,
  hint: string,
  checked: boolean,
): HTMLLabelElement {
  const card = node("label", "card");
  card.append(
    radio("format", format, checked),
    node("span", "name", name),
    node("span", "hint", hint),
  );
  return card;
}

/** The text of a range line: the two instants in the viewer's time and the zone. */
function rangeLine(labels: LabelTable, from: string, to: string, zone: string): string {
  const text = (value: string) => value.replace("T", " ");
  return formatLabel(labels.logsExportRangeLine, { from: text(from), to: text(to), zone });
}

/** The All entries line: since the oldest entry's date, or a plain line. */
function allLine(options: ExportDialogOptions): string {
  const { labels, oldest, lang, zone } = options;
  if (oldest === undefined) return labels.logsExportAllLine;
  const date = new Intl.DateTimeFormat(lang, {
    dateStyle: "medium",
    ...(zone ? { timeZone: zone } : {}),
  });
  return formatLabel(labels.logsExportAllSince, { date: date.format(new Date(oldest)) });
}

/** The initial choices: the current filter, JSON Lines, a week up to now for a range. */
function initialState(options: ExportDialogOptions): State {
  const to = options.now;
  const from = new Date(to.getTime() - 7 * 24 * 3_600_000);
  return {
    scope: "filter",
    format: "jsonl",
    from: inputValue(from),
    to: inputValue(to),
    name: exportFileName(options.labels, "jsonl", options.now, options.zone),
    edited: false,
    busy: false,
  };
}

/** What the dialog's controls are, to read and update. */
interface Parts {
  readonly dialog: HTMLElement;
  readonly name: HTMLInputElement;
  readonly choose: HTMLButtonElement;
  readonly where: HTMLElement;
  readonly failed: HTMLElement;
  readonly busy: HTMLElement;
  readonly submit: HTMLButtonElement;
  readonly cancel: HTMLButtonElement;
  readonly rangeFields: HTMLElement;
  readonly rangeLine: HTMLElement;
  readonly from: HTMLInputElement;
  readonly to: HTMLInputElement;
}

/** The scope fieldset with its three options and the range fields. */
function scopeGroup(
  options: ExportDialogOptions,
  state: State,
): {
  group: HTMLFieldSetElement;
  rangeFields: HTMLElement;
  rangeLine: HTMLElement;
  from: HTMLInputElement;
  to: HTMLInputElement;
} {
  const { labels } = options;
  const zone = utcOffsetLabel(options.now, options.zone);
  const group = node("fieldset", "scope");
  group.append(node("legend", "", labels.logsExportScope));
  const filter = scopeOption(
    "filter",
    labels.logsExportScopeFilter,
    exportFilterLine(options.filter, labels, options.libraries, options.loaded),
    true,
  );
  const range = scopeOption(
    "range",
    labels.logsExportScopeRange,
    rangeLine(labels, state.from, state.to, zone),
    false,
  );
  const all = scopeOption("all", labels.logsExportScopeAll, allLine(options), false);
  const fields = node("div", "range-fields");
  fields.hidden = true;
  const from = dateField(labels.logsExportRangeFrom, state.from);
  const to = dateField(labels.logsExportRangeTo, state.to);
  fields.append(from.label, to.label);
  group.append(filter.label, range.label, fields, all.label);
  return { group, rangeFields: fields, rangeLine: range.line, from: from.input, to: to.input };
}

/** A labelled datetime-local field. */
function dateField(
  text: string,
  value: string,
): { label: HTMLLabelElement; input: HTMLInputElement } {
  const label = node("label", "", text);
  const input = node("input", "input");
  input.type = "datetime-local";
  input.value = value;
  label.append(input);
  return { label, input };
}

/** The dialog's markup and its parts. */
function build(options: ExportDialogOptions, state: State): Parts {
  const { labels } = options;
  const dialog = node("div", "dialog logs-export");
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  const title = node("h2", "dialog-title", labels.logsExportTitle);
  title.id = "dialog-title-logs-export";
  dialog.setAttribute("aria-labelledby", title.id);
  const scope = scopeGroup(options, state);
  const format = node("fieldset", "format");
  format.append(node("legend", "", labels.logsExportFormat));
  const cards = node("div", "cards");
  cards.append(
    formatCard("jsonl", labels.logsFormatJsonl, labels.logsFormatJsonlHint, true),
    formatCard("csv", labels.logsFormatCsv, labels.logsFormatCsvHint, false),
  );
  format.append(cards);
  const note = node("div", "note-box raised");
  note.append(icon("info", 20), node("p", "", labels.logsExportNote));
  const file = fileSection(options, state);
  const failed = node("p", "failed");
  failed.setAttribute("role", "alert");
  const busy = node("p", "busy");
  busy.setAttribute("role", "status");
  const actions = node("div", "dialog-actions");
  const cancel = node("button", "button large secondary cancel", labels.fileCancel);
  cancel.type = "button";
  const submit = node("button", "button large primary submit", labels.logsExportSubmit);
  submit.type = "button";
  actions.append(cancel, submit);
  dialog.append(
    title,
    node("p", "sub", labels.logsExportSub),
    scope.group,
    format,
    note,
    file.section,
    busy,
    failed,
    actions,
  );
  return { dialog, ...file.parts, failed, busy, submit, cancel, ...scope };
}

/** The File name section: label, field with Choose…, and the line about where it is saved. */
function fileSection(
  options: ExportDialogOptions,
  state: State,
): { section: HTMLElement; parts: Pick<Parts, "name" | "choose" | "where"> } {
  const { labels } = options;
  const section = node("div", "file");
  const label = node("label", "", labels.logsExportFileName);
  const name = node("input", "input");
  name.type = "text";
  name.value = state.name;
  name.autocomplete = "off";
  name.spellcheck = false;
  name.id = "logs-export-name";
  label.htmlFor = name.id;
  const choose = node("button", "button secondary choose", labels.logsExportChoose);
  choose.type = "button";
  choose.disabled = !options.canChoose;
  const row = node("div", "file-row");
  row.append(name, choose);
  const where = node(
    "p",
    "where",
    options.canChoose ? labels.logsExportSavedTo : labels.logsExportSavedToOnly,
  );
  section.append(label, row, where);
  return { section, parts: { name, choose, where } };
}

/** The choice the controls hold now. */
function readChoice(dialog: HTMLElement, state: State): ExportChoice {
  const value = (name: string) =>
    dialog.querySelector<HTMLInputElement>(`input[name="${name}"]:checked`)?.value;
  const scope = (value("scope") as ExportScope | undefined) ?? state.scope;
  const format = (value("format") as ExportFormat | undefined) ?? state.format;
  if (scope !== "range") return { scope, format };
  return { scope, format, from: toUtc(state.from), to: toUtc(state.to) };
}

/** An input value (the browser's local time) as RFC 3339 UTC, or undefined when it is blank. */
function toUtc(value: string): string | undefined {
  const at = new Date(value);
  return Number.isNaN(at.getTime()) ? undefined : at.toISOString();
}

/** A file name the platforms accept: not empty, no separators, at most 255 characters. */
function nameUsable(name: string): boolean {
  const trimmed = name.trim();
  return (
    trimmed !== "" && trimmed.length <= 255 && !/[\\/]/.test(trimmed) && !/\p{Cc}/u.test(trimmed)
  );
}

/**
 * Open the Download logs dialog. Side effects: adds the scrim, moves focus into the dialog and
 * back, reads the export and saves a file when Download is pressed.
 *
 * @param options See {@link ExportDialogOptions}.
 * @returns The open dialog.
 */
export function openExportDialog(options: ExportDialogOptions): OpenDialog {
  const state = initialState(options);
  const parts = build(options, state);
  const open = showDialog(options.layer, parts.dialog, options.opener);
  wire(options, state, parts, open);
  return open;
}

/** Update the state, the file name and the range line when a control changes. */
function onChange(options: ExportDialogOptions, state: State, parts: Parts, event: Event): void {
  const input = event.target as HTMLInputElement;
  const { labels } = options;
  if (input === parts.name) {
    state.name = input.value;
    state.edited = true;
    state.target = undefined;
    return;
  }
  const choice = readChoice(parts.dialog, state);
  state.scope = choice.scope;
  if (input === parts.from) state.from = input.value;
  if (input === parts.to) state.to = input.value;
  parts.rangeFields.hidden = choice.scope !== "range";
  const zone = utcOffsetLabel(options.now, options.zone);
  parts.rangeLine.textContent = rangeLine(labels, state.from, state.to, zone);
  if (choice.format !== state.format) {
    state.format = choice.format;
    if (!state.edited && state.target === undefined) {
      state.name = exportFileName(labels, choice.format, options.now, options.zone);
      parts.name.value = state.name;
    }
  }
}

/** Attach the listeners of the dialog. */
function wire(options: ExportDialogOptions, state: State, parts: Parts, open: OpenDialog): void {
  parts.dialog.addEventListener("change", (event) => onChange(options, state, parts, event));
  parts.dialog.addEventListener("input", (event) => {
    if (event.target === parts.name || event.target === parts.from || event.target === parts.to) {
      onChange(options, state, parts, event);
    }
  });
  parts.cancel.addEventListener("click", () => {
    if (!state.busy) open.close();
  });
  parts.choose.addEventListener("click", () => void chooseFile(options, state, parts));
  parts.submit.addEventListener("click", () => void download(options, state, parts, open));
}

/** Open the platform's save dialog and take the chosen name. Side effect: shows that dialog. */
async function chooseFile(options: ExportDialogOptions, state: State, parts: Parts): Promise<void> {
  if (!options.choose || state.busy) return;
  try {
    const target = await options.choose(state.name, state.format);
    if (target === undefined) return;
    state.target = target;
    state.name = target.name;
    parts.name.value = target.name;
  } catch (error) {
    console.warn(
      "FileSquid could not open the save dialog",
      error instanceof Error ? error.name : error,
    );
    parts.failed.textContent = options.labels.logsExportFailed;
  }
}

/** Disable or enable the controls while a download runs. */
function setBusy(options: ExportDialogOptions, state: State, parts: Parts, busy: boolean): void {
  state.busy = busy;
  for (const control of [parts.submit, parts.cancel, parts.name, parts.choose]) {
    if (control === parts.choose) control.disabled = busy || !options.canChoose;
    else control.disabled = busy;
  }
  parts.dialog.setAttribute("aria-busy", String(busy));
  parts.busy.textContent = busy ? options.labels.logsExportBusy : "";
}

/** The problem with the choices, in words, or undefined when they can be downloaded. */
function problem(
  options: ExportDialogOptions,
  state: State,
  choice: ExportChoice,
): string | undefined {
  if (!nameUsable(state.name)) return options.labels.logsExportNameInvalid;
  if (choice.scope !== "range") return undefined;
  const { from, to } = choice;
  if (from === undefined || to === undefined || from >= to)
    return options.labels.logsExportRangeInvalid;
  return undefined;
}

/**
 * Read the export for the choice and save it. A failure keeps the dialog open with an alert;
 * nothing is saved then. Network I/O and a file write.
 */
async function download(
  options: ExportDialogOptions,
  state: State,
  parts: Parts,
  open: OpenDialog,
): Promise<void> {
  if (state.busy) return;
  const choice = readChoice(parts.dialog, state);
  const invalid = problem(options, state, choice);
  parts.failed.textContent = invalid ?? "";
  if (invalid !== undefined) return;
  setBusy(options, state, parts, true);
  try {
    const text = await options.run(exportQuery(choice, options.filter, options.now));
    const file = FORMAT_FILE[choice.format];
    const blob = new Blob([text], { type: `${file.mime};charset=utf-8` });
    const name = state.name.trim();
    if (state.target) await writeToPickedFile(state.target, blob);
    else options.save(blob, name);
    open.close();
    options.onDone?.(name);
  } catch (error) {
    console.warn(
      "FileSquid could not make the log download",
      error instanceof Error ? error.name : error,
    );
    setBusy(options, state, parts, false);
    parts.failed.textContent = options.labels.logsExportFailed;
  }
}

/**
 * Open the platform's save dialog for an export, for {@link ExportDialogOptions.choose}.
 *
 * @param win The window.
 * @param labels The active label table.
 * @returns A function that opens the dialog for a name and format.
 */
export function chooser(
  win: Window,
  labels: LabelTable,
): (suggested: string, format: ExportFormat) => Promise<SaveTarget | undefined> {
  return (suggested, format) => {
    const file = FORMAT_FILE[format];
    const description = format === "csv" ? labels.logsFormatCsv : labels.logsFormatJsonl;
    return pickSaveFile(win, suggested, description, file.extension, file.mime);
  };
}
