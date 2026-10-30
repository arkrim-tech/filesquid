/**
 * The dialogs and the toast of the Drive-style views (UI-F2.8, UI-S-026, mockups 12, 13, 14
 * and 44): rename (560 px wide), Download as ZIP (640 px) and Move or copy (600 px), and the
 * remove toast (440 px, inverse surface) with Undo. Each dialog is a `role="dialog"` modal on
 * the shared scrim, opens with focus on its first control, keeps Tab inside, closes on Escape
 * and returns focus to the control that opened it (UI-S-016). Every word comes from the label
 * table and every colour from a token; the stylesheet is {@link STYLE}.
 *
 * The builders only build and wire the DOM. What a submit does (the write, the refusal text)
 * is the view's decision, passed in as a callback that resolves or rejects.
 */
import { MAX_DISPLAY_NAME_BYTES, formatSize, validateName, type NameProblem } from "../files/model";
import { problemText, renameFailureText } from "../files/ops";
import { formatLabel, type LabelTable } from "../theme/labels";
import type { DriveRow, ZipCheck } from "../drive/model";
import { driveSummary, fileCountText } from "../drive/model";
import type { TransferMode } from "../drive/actions";
import { DestinationUnavailableError } from "../drive/actions";
import type { FsDestinationPicker } from "./fs-destination-picker";
import type { NavigationSource } from "../navigation/model";

/** The stylesheet of dialogs and the toast, tokens only; added after `CONTROL_STYLE`. */
export const STYLE = `
  .dialog { gap: var(--fs-space-3); }
  .dialog.rename { inline-size: min(35rem, 100%); min-block-size: 15.625rem; }
  .dialog.zip { inline-size: min(40rem, 100%); block-size: 32.5rem; }
  .dialog.move { inline-size: min(37.5rem, 100%); block-size: 40rem; grid-template-rows: auto auto auto auto minmax(0, 1fr) auto auto; }
  .dialog.rename .dialog-title { font-size: var(--fs-font-size-title); }
  .dialog label { display: grid; gap: var(--fs-space-1); font-size: var(--fs-font-size-small); font-weight: 600; color: var(--fs-on-surface-variant); }
  .dialog .note, .dialog .names { margin: 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .dialog .names { overflow-wrap: anywhere; color: var(--fs-on-surface); }
  .form-error { margin: 0; color: var(--fs-error); font-size: var(--fs-font-size-small); }
  .form-error:empty { display: none; }
  .save-as { display: flex; align-items: center; gap: var(--fs-space-2); }
  .save-as .input { flex: 1 1 auto; font-family: var(--fs-font-mono); font-size: var(--fs-font-size-small); }
  .policy { display: grid; gap: var(--fs-space-1); padding: var(--fs-space-3); border-radius: var(--fs-radius); background: var(--fs-surface-raised); }
  .policy .policy-title { margin: 0; font-size: var(--fs-font-size-ui); font-weight: 600; color: var(--fs-on-surface); }
  .chip.mono { font-family: var(--fs-font-mono); justify-self: start; }
  .section-label { margin: 0; font-size: var(--fs-font-size-small); font-weight: 600; color: var(--fs-on-surface); }
  .progress { margin: 0; font-size: var(--fs-font-size-small); font-weight: 500; color: var(--fs-on-surface); }
  .segmented { display: inline-flex; justify-self: start; padding: var(--fs-space-1); border-radius: var(--fs-radius-button); background: var(--fs-surface-raised); }
  .segmented button { min-block-size: 2rem; padding: 0 var(--fs-space-3); border: 0; border-radius: var(--fs-radius-input); background: none; color: var(--fs-on-surface-variant); font: inherit; font-size: var(--fs-font-size-small); font-weight: 500; cursor: pointer; }
  .segmented button[aria-checked="true"] { background: var(--fs-primary-container); color: var(--fs-on-primary-container); font-size: var(--fs-font-size-ui); font-weight: 600; }
  .destination-line { margin: 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .dialog-actions .spacer { flex: 1 1 auto; }
  .toast { position: fixed; inset-block-end: var(--fs-space-4); inset-inline: 0; z-index: 5; box-sizing: border-box; display: flex; align-items: center; gap: var(--fs-space-3); inline-size: min(27.5rem, calc(100vw - 2 * var(--fs-space-3))); min-block-size: 2.75rem; margin-inline: auto; padding: 0 var(--fs-space-3); border-radius: var(--fs-radius); background: var(--fs-inverse-surface); color: var(--fs-on-inverse-surface); font-size: var(--fs-font-size-small); font-weight: 500; }
  .toast .text { flex: 1 1 auto; min-width: 0; overflow-wrap: anywhere; }
  .toast button.undo { padding: 0; border: 0; background: none; color: inherit; font: inherit; font-weight: 600; text-decoration: underline; cursor: pointer; }
`;

/** A dialog that is open: its element and a way to close it. */
export interface OpenDialog {
  /** The `role="dialog"` element. */
  readonly element: HTMLElement;
  /** Close the dialog and return focus to the opener. Idempotent. */
  close(): void;
}

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

/** A `type="button"` control with the shared button classes. */
function button(
  className: string,
  text: string,
  kind: "primary" | "secondary" | "ghost" = "secondary",
): HTMLButtonElement {
  const out = node("button", `${className} button large ${kind}`, text);
  out.type = "button";
  return out;
}

/** The focusable controls of a dialog, in order. */
function focusables(dialog: HTMLElement): HTMLElement[] {
  const selector = "button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex='-1'])";
  return [...dialog.querySelectorAll<HTMLElement>(selector)];
}

/** Keep Tab inside the dialog and close on Escape. */
function onKeydown(event: KeyboardEvent, dialog: HTMLElement, close: () => void): void {
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    return close();
  }
  if (event.key !== "Tab") return;
  const items = focusables(dialog);
  const first = items[0];
  const last = items.at(-1);
  const active = (dialog.getRootNode() as ShadowRoot).activeElement;
  if (event.shiftKey && active === first) last?.focus();
  else if (!event.shiftKey && active === last) first?.focus();
  else return;
  event.preventDefault();
}

/**
 * Show a dialog on the scrim in `layer`. Side effects: adds the scrim, moves focus to the first
 * control (or `initial`), and on close removes the scrim and returns focus to `opener`.
 *
 * @param layer Where the scrim is added.
 * @param dialog The `role="dialog"` element.
 * @param opener The control that opened it.
 * @param initial The control to focus first; the first focusable control by default.
 * @returns The open dialog.
 */
export function showDialog(
  layer: HTMLElement,
  dialog: HTMLElement,
  opener: HTMLElement | null,
  initial?: HTMLElement,
): OpenDialog {
  const scrim = node("div", "scrim");
  scrim.append(dialog);
  layer.append(scrim);
  let closed = false;
  const close = () => {
    if (closed) return;
    closed = true;
    scrim.remove();
    opener?.focus();
  };
  dialog.addEventListener("keydown", (event) => onKeydown(event, dialog, close));
  (initial ?? focusables(dialog)[0])?.focus();
  return { element: dialog, close };
}

/** The frame of a dialog: role, name and the title. */
function frame(className: string, titleText: string): { dialog: HTMLElement; title: HTMLElement } {
  const dialog = node("div", `dialog ${className}`);
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  const title = node("h2", "dialog-title", titleText);
  title.id = `dialog-title-${className}`;
  dialog.setAttribute("aria-labelledby", title.id);
  dialog.append(title);
  return { dialog, title };
}

/** What the rename dialog needs. */
export interface RenameDialogOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** The file being renamed. */
  readonly row: DriveRow;
  /** The control that opened the dialog. */
  readonly opener: HTMLElement | null;
  /** Where the dialog is shown. */
  readonly layer: HTMLElement;
  /** Renames; resolves when done, rejects with the API's refusal. */
  readonly submit: (name: string) => Promise<void>;
}

/** Refresh the error text, the invalid mark and Save for the typed name. */
function checkName(
  input: HTMLInputElement,
  error: HTMLElement,
  save: HTMLButtonElement,
  labels: LabelTable,
) {
  const problem: NameProblem | undefined = validateName(input.value, MAX_DISPLAY_NAME_BYTES);
  const shown = problem !== undefined && problem !== "empty";
  error.textContent = shown ? problemText(problem, labels, true) : "";
  if (shown) input.setAttribute("aria-invalid", "true");
  else input.removeAttribute("aria-invalid");
  save.toggleAttribute("disabled", problem !== undefined);
}

/**
 * Open the rename dialog (mockup 14, 44): "Rename {name}" with the Name field focused, Save and
 * Cancel. A name over 1,024 bytes is refused before sending, with Save disabled; a refusal of
 * the route is shown below the field in the error colour with a 2 px error border and the form
 * stays open with the typed name.
 *
 * @param options Labels, the file, the opener, the layer and the submit callback.
 * @returns The open dialog.
 */
export function renameDialog(options: RenameDialogOptions): OpenDialog {
  const { labels, row } = options;
  const { dialog } = frame("rename", formatLabel(labels.fileRenameOnlyTitle, { name: row.name }));
  const label = node("label", "", labels.fileFieldName);
  const input = node("input", "input");
  input.type = "text";
  input.value = row.name;
  input.autocomplete = "off";
  label.append(input);
  const error = node("p", "form-error");
  error.setAttribute("role", "alert");
  const save = button("start", labels.fileSave, "primary");
  save.type = "submit";
  const cancel = button("cancel", labels.fileCancel);
  const actions = node("div", "dialog-actions");
  actions.append(cancel, save);
  const form = node("form", "rename-form");
  form.noValidate = true;
  form.append(label, error, actions);
  dialog.append(form);
  const open = showDialog(options.layer, dialog, options.opener, input);
  input.select();
  input.addEventListener("input", () => checkName(input, error, save, labels));
  cancel.addEventListener("click", () => open.close());
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void submitRename(input, error, save, labels, options.submit, open);
  });
  return open;
}

/** Send the typed name; keep the form open with the refusal when the route refuses. */
async function submitRename(
  input: HTMLInputElement,
  error: HTMLElement,
  save: HTMLButtonElement,
  labels: LabelTable,
  submit: (name: string) => Promise<void>,
  open: OpenDialog,
): Promise<void> {
  if (save.hasAttribute("disabled")) return;
  save.setAttribute("disabled", "");
  try {
    await submit(input.value.trim());
    open.close();
  } catch (failure) {
    error.textContent = renameFailureText(failure, labels);
    input.setAttribute("aria-invalid", "true");
    save.removeAttribute("disabled");
  }
}

/** What the ZIP dialog needs. */
export interface ZipDialogOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** The selected rows. */
  readonly rows: readonly DriveRow[];
  /** The result of checking them against the caps. */
  readonly check: ZipCheck;
  /** The archive's default file name, without a path. */
  readonly fileName: string;
  /** BCP 47 tag for numbers. */
  readonly locale: string;
  /** The control that opened the dialog. */
  readonly opener: HTMLElement | null;
  /** Where the dialog is shown. */
  readonly layer: HTMLElement;
  /** Streams the archive; absent on a host without the ZIP route. */
  readonly download?: (
    name: string,
    progress: (done: number, bytes: number) => void,
  ) => Promise<void>;
}

/** The refusal text for a selection the caps refuse, empty for an allowed one. */
function zipRefusal(check: ZipCheck, labels: LabelTable): string {
  if (check.ok) return "";
  if (check.reason === "empty") return labels.fileZipEmpty;
  return check.reason === "too_big" ? labels.fileZipTooBig : labels.fileZipTooMany;
}

/** The static sections of the ZIP dialog: summary, save as, policy and limits. */
function zipBody(options: ZipDialogOptions): { nodes: HTMLElement[]; name: HTMLInputElement } {
  const { labels, rows, check, locale } = options;
  const sum = driveSummary(rows);
  const count = check.ok ? check.count : sum.files;
  const summary = node(
    "p",
    "note",
    formatLabel(labels.fileZipSummary, {
      files: fileCountText(count, labels, locale),
      size: formatSize(sum.bytes, labels, locale),
    }),
  );
  const name = node("input", "input");
  name.type = "text";
  name.value = options.fileName;
  name.readOnly = true;
  name.setAttribute("aria-label", labels.fileZipSaveAs);
  const choose = button("choose", labels.fileZipChoose);
  choose.addEventListener("click", () => {
    name.readOnly = false;
    name.focus();
  });
  const saveAs = node("div", "save-as");
  saveAs.append(name, choose);
  const label = node("p", "section-label", labels.fileZipSaveAs);
  const policy = node("div", "policy");
  policy.append(
    node("p", "policy-title", labels.fileZipPolicyTitle),
    node("p", "note", labels.fileZipPolicyText),
    node("span", "chip ok mono", labels.fileZipPolicyChip),
  );
  const limits = [
    node("p", "section-label", labels.fileZipLimitsTitle),
    node("p", "note", labels.fileZipLimits),
  ];
  return { nodes: [summary, label, saveAs, policy, ...limits], name };
}

/**
 * Open the Download as ZIP dialog (mockup 12): the member count and size, Save as with a mono
 * file name and Choose…, the outgoing metadata policy with its chip, the limits (1,000 files and
 * 10 GiB), the progress line once started, and Cancel. A selection over either cap is refused
 * with the cap stated and Download disabled, so the owner narrows the selection; a host without
 * the ZIP route says so and sends nothing. No save path is shown.
 *
 * @param options Labels, rows, the cap check, the default name and the optional download.
 * @returns The open dialog.
 */
export function zipDialog(options: ZipDialogOptions): OpenDialog {
  const { labels, check } = options;
  const { dialog } = frame("zip", labels.fileZipTitle);
  const body = zipBody(options);
  const progress = node("p", "progress");
  progress.setAttribute("role", "status");
  const error = node("p", "form-error", zipRefusal(check, labels));
  error.setAttribute("role", "alert");
  const cancel = button("cancel", labels.fileCancel);
  const start = button("start", labels.fileZipStart, "primary");
  start.toggleAttribute("disabled", !check.ok);
  const actions = node("div", "dialog-actions");
  actions.append(cancel, start);
  dialog.append(...body.nodes, progress, error, actions);
  const open = showDialog(options.layer, dialog, options.opener, check.ok ? start : cancel);
  cancel.addEventListener("click", () => open.close());
  start.addEventListener(
    "click",
    () => void runZip(options, body.name.value, progress, error, open),
  );
  return open;
}

/** Start the download and show its progress, or the honest refusal. */
async function runZip(
  options: ZipDialogOptions,
  name: string,
  progress: HTMLElement,
  error: HTMLElement,
  open: OpenDialog,
): Promise<void> {
  const { labels, locale } = options;
  if (!options.download) {
    error.textContent = labels.fileZipUnavailable;
    return;
  }
  const sum = driveSummary(options.rows);
  const total = fileCountText(options.check.ok ? options.check.count : sum.files, labels, locale);
  error.textContent = "";
  try {
    await options.download(name, (done, bytes) => {
      progress.textContent = formatLabel(labels.fileZipProgress, {
        done: done.toLocaleString(locale),
        total,
        bytes: formatSize(bytes, labels, locale),
        size: formatSize(sum.bytes, labels, locale),
      });
    });
    open.close();
  } catch (failure) {
    console.warn("FileSquid could not download the archive", failure);
    error.textContent = labels.fileActionFailed;
  }
}

/** What the move or copy dialog needs. */
export interface MoveDialogOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** The rows to move or copy. */
  readonly rows: readonly DriveRow[];
  /** The control that opened the dialog. */
  readonly opener: HTMLElement | null;
  /** Where the dialog is shown. */
  readonly layer: HTMLElement;
  /** The navigation listing the destination tree reads. */
  readonly navigation?: NavigationSource;
  /** The node the files are in now, marked "Current folder". */
  readonly current?: string;
  /** Moves or copies into the chosen node; rejects with the refusal. */
  readonly submit: (mode: TransferMode, destination: string) => Promise<void>;
}

/** The title and the names line of the move or copy dialog. */
function moveHeading(options: MoveDialogOptions): { title: string; names: string } {
  const { labels, rows } = options;
  const only = rows.length === 1 ? rows[0] : undefined;
  const title = only
    ? formatLabel(labels.fileMoveTitleOne, { name: only.name })
    : formatLabel(labels.fileMoveTitleMany, { count: String(rows.length) });
  return { title, names: rows.map((row) => row.name).join(", ") };
}

/** The Move | Copy segmented control. */
function modeControl(labels: LabelTable): HTMLElement {
  const group = node("div", "segmented");
  group.setAttribute("role", "radiogroup");
  for (const [mode, text] of [
    ["move", labels.fileMoveMode],
    ["copy", labels.fileCopyMode],
  ] as const) {
    const radio = node("button", "", text);
    radio.type = "button";
    radio.dataset.mode = mode;
    radio.setAttribute("role", "radio");
    radio.setAttribute("aria-checked", String(mode === "move"));
    group.append(radio);
  }
  return group;
}

/**
 * Open the Move or copy dialog (mockup 13): the title naming the files, the Move | Copy control
 * with the explanation of the chosen mode, the destination tree of collections and folders only,
 * the Destination line, New folder, Cancel and Move here or Copy here (enabled once a
 * destination is chosen). A refusal is shown below the tree and the dialog stays open.
 *
 * @param options Labels, rows, the layer, the navigation source and the submit callback.
 * @returns The open dialog.
 */
export function moveDialog(options: MoveDialogOptions): OpenDialog {
  const { labels } = options;
  const heading = moveHeading(options);
  const { dialog } = frame("move", heading.title);
  const names = node("p", "names", heading.names);
  const mode = modeControl(labels);
  const explain = node("p", "note", labels.fileMoveExplain);
  const picker = document.createElement("fs-destination-picker") as FsDestinationPicker;
  const line = node("p", "destination-line", labels.fileDestination);
  const error = node("p", "form-error");
  error.setAttribute("role", "alert");
  const state = { mode: "move" as TransferMode };
  const start = button("start", labels.fileMoveHere, "primary");
  start.setAttribute("disabled", "");
  const cancel = button("cancel", labels.fileCancel);
  const create = button("new-folder", labels.fileNewFolder, "ghost");
  create.setAttribute("disabled", "");
  create.title = labels.fileActionUnavailable;
  const actions = node("div", "dialog-actions");
  actions.append(create, node("span", "spacer"), cancel, start);
  dialog.append(names, mode, explain, picker, line, error, actions);
  const open = showDialog(
    options.layer,
    dialog,
    options.opener,
    mode.querySelector("button") as HTMLElement,
  );
  if (options.navigation)
    picker.configure({ labels, source: options.navigation, current: options.current });
  wireMoveDialog({ options, mode, explain, start, line, picker, error, state, open });
  cancel.addEventListener("click", () => open.close());
  return open;
}

/** The parts of the move dialog that react to each other. */
interface MoveParts {
  readonly options: MoveDialogOptions;
  readonly mode: HTMLElement;
  readonly explain: HTMLElement;
  readonly start: HTMLButtonElement;
  readonly line: HTMLElement;
  readonly picker: FsDestinationPicker;
  readonly error: HTMLElement;
  readonly state: { mode: TransferMode };
  readonly open: OpenDialog;
}

/** Wire the mode control, the picker and the start button. */
function wireMoveDialog(parts: MoveParts): void {
  const { labels } = parts.options;
  parts.mode.addEventListener("click", (event) => {
    const radio = (event.target as Element).closest<HTMLElement>("[role=radio]");
    if (!radio) return;
    parts.state.mode = radio.dataset.mode as TransferMode;
    for (const other of parts.mode.querySelectorAll("[role=radio]")) {
      other.setAttribute("aria-checked", String(other === radio));
    }
    const copy = parts.state.mode === "copy";
    parts.explain.textContent = copy ? labels.fileCopyExplain : labels.fileMoveExplain;
    parts.start.textContent = copy ? labels.fileCopyHere : labels.fileMoveHere;
  });
  parts.picker.addEventListener("fs-destination-change", (event) => {
    const choice = (event as CustomEvent<{ trail: string[] } | null>).detail;
    parts.start.toggleAttribute("disabled", !choice);
    parts.line.textContent = choice
      ? `${labels.fileDestination}: ${choice.trail.join(" › ")}`
      : labels.fileDestination;
  });
  parts.start.addEventListener("click", () => void runMove(parts));
}

/** Send the move or copy; show the refusal below the tree. */
async function runMove(parts: MoveParts): Promise<void> {
  const destination = parts.picker.selected?.node;
  if (!destination) return;
  parts.error.textContent = "";
  try {
    await parts.options.submit(parts.state.mode, destination);
    parts.open.close();
  } catch (failure) {
    const unavailable = failure instanceof DestinationUnavailableError;
    if (!unavailable) console.warn("FileSquid could not move or copy", failure);
    const { labels } = parts.options;
    parts.error.textContent = unavailable
      ? labels.fileMoveCopyUnavailable
      : labels.fileActionFailed;
  }
}

/** What the remove toast needs. */
export interface ToastOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** The removed file's display name. */
  readonly name: string;
  /** The version the removal committed. */
  readonly version: string;
  /** Where the toast is shown. */
  readonly layer: HTMLElement;
  /** Undo the removal; resolves when it is done. */
  readonly undo: () => Promise<void>;
}

/**
 * Show the remove toast (mockup 14): "{name} removed. Version {version} committed." on the
 * inverse surface with Undo, in a polite live region. It stays until Undo is chosen or another
 * toast replaces it.
 *
 * @param options Labels, the name, the version, the layer and the undo callback.
 * @returns A function that removes the toast.
 */
export function showToast(options: ToastOptions): () => void {
  const { labels } = options;
  options.layer.querySelector(".toast")?.remove();
  const toast = node("div", "toast");
  toast.setAttribute("role", "status");
  const text = node(
    "span",
    "text",
    formatLabel(labels.fileRemoved, { name: options.name, version: options.version }),
  );
  const undo = node("button", "undo", labels.fileUndo);
  undo.type = "button";
  const remove = () => toast.remove();
  undo.addEventListener("click", () => {
    undo.setAttribute("disabled", "");
    options.undo().then(remove, (failure: unknown) => {
      console.warn("FileSquid could not undo the removal", failure);
      text.textContent = labels.fileActionFailed;
      undo.remove();
    });
  });
  toast.append(text, undo);
  options.layer.append(toast);
  return remove;
}
