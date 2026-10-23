/**
 * The static frame of `<fs-file-browser>` (UI-F2.2): the breadcrumb, the toolbar, the name form
 * used for a new folder and for rename or move, the two live regions, the listing area (states,
 * the list itself, the summary and the drop target), the viewer with its preview, and the panel
 * area. {@link buildFrame} builds it from the active label table and returns typed references,
 * so the element and the writes controller never query the DOM by selector. Every word is set
 * with `textContent` from the label table.
 */
import type { LabelTable } from "../theme/labels";
import type { FsBreadcrumb } from "./fs-breadcrumb";
import type { FsFilePreview } from "./fs-file-preview";
import type { FsStates } from "./fs-states";

/** References into the frame. */
export interface Frame {
  /** The outer element. */
  readonly root: HTMLElement;
  /** Folder trail. */
  readonly breadcrumb: FsBreadcrumb;
  /** Toolbar controls. */
  readonly bar: {
    readonly upload: HTMLButtonElement;
    readonly newFolder: HTMLButtonElement;
    readonly rename: HTMLButtonElement;
    readonly remove: HTMLButtonElement;
    readonly list: HTMLButtonElement;
    readonly grid: HTMLButtonElement;
    readonly input: HTMLInputElement;
  };
  /** The name form. */
  readonly form: {
    readonly element: HTMLFormElement;
    /** The visible title, "Rename {name}" or "New folder in {folder}" (mockup 14). */
    readonly title: HTMLElement;
    readonly name: HTMLInputElement;
    readonly folderField: HTMLElement;
    readonly folder: HTMLInputElement;
    readonly error: HTMLElement;
    readonly submit: HTMLButtonElement;
    readonly cancel: HTMLButtonElement;
  };
  /** Polite live region. */
  readonly status: HTMLElement;
  /** Assertive live region. */
  readonly alert: HTMLElement;
  /** The listing area. */
  readonly listing: {
    readonly element: HTMLElement;
    readonly drop: HTMLElement;
    readonly states: FsStates;
    readonly host: HTMLElement;
    readonly summary: HTMLElement;
    readonly more: HTMLElement;
    readonly limit: HTMLElement;
  };
  /** The viewer. */
  readonly viewer: {
    readonly element: HTMLElement;
    readonly back: HTMLButtonElement;
    readonly title: HTMLElement;
    readonly position: HTMLElement;
    readonly previous: HTMLButtonElement;
    readonly next: HTMLButtonElement;
    readonly preview: FsFilePreview;
  };
  /** The panel area. */
  readonly panels: HTMLElement;
}

/**
 * The frame's stylesheet (UI-S-024, UI-S-026): 36 px buttons with 14 px 600 text, radius 7 px
 * and a 1.5 px `--fs-outline-strong` border on `--fs-surface-card` (Save filled
 * `--fs-primary`, the pressed layout half on the active tint); the name form drawn as the rename
 * form of mockup 14 (`--fs-surface-overlay`, a 1 px `--fs-outline-strong` border, radius 10 px,
 * the 20 px 700 heading-font title) with its 36 px input (1.5 px `--fs-outline-control`, radius
 * 6 px, a 2 px `--fs-focus-ring` border on focus) and, for a refusal (mockup 44), a 2 px
 * `--fs-error` border on the field and the message below it in `--fs-error`.
 */
export const STYLE = `
  .frame { display: flex; flex-direction: column; gap: var(--fs-space-3); min-width: 0; font-size: var(--fs-font-size-ui); }
  .toolbar { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-2); }
  .spacer { flex: 1 1 auto; }
  .group { display: inline-flex; gap: var(--fs-space-1); }
  button { box-sizing: border-box; min-block-size: 2.25rem; padding: 0 var(--fs-space-3); border: 1.5px solid var(--fs-outline-strong); border-radius: var(--fs-radius-button); background: var(--fs-surface-card); color: var(--fs-on-surface); font: inherit; font-size: var(--fs-font-size-ui); font-weight: 600; cursor: pointer; transition: background-color var(--fs-motion-duration), color var(--fs-motion-duration); }
  button:hover { background: var(--fs-surface-raised); }
  button[aria-pressed="true"] { border-color: var(--fs-primary-container); background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  button.primary { border-color: var(--fs-primary); background: var(--fs-primary); color: var(--fs-on-primary); }
  button.primary:hover { border-color: var(--fs-on-primary-container); background: var(--fs-on-primary-container); }
  button[aria-disabled="true"] { border-color: var(--fs-outline); background: var(--fs-surface-raised); color: var(--fs-on-surface-placeholder); cursor: default; }
  .name-form { display: flex; flex-wrap: wrap; align-items: end; gap: var(--fs-space-3); box-sizing: border-box; max-inline-size: 35rem; padding: var(--fs-space-4); border: 1px solid var(--fs-outline-strong); border-radius: var(--fs-radius-dialog); background: var(--fs-surface-overlay); }
  .form-title { flex: 1 1 100%; margin: 0; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-title); font-weight: 700; color: var(--fs-on-surface-heading); overflow-wrap: anywhere; }
  .form-title:empty { display: none; }
  .name-form label { display: flex; flex: 1 1 100%; flex-direction: column; gap: var(--fs-space-1); font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .name-form input { box-sizing: border-box; min-block-size: 2.25rem; padding: 0 var(--fs-space-2); border: 1.5px solid var(--fs-outline-control); border-radius: var(--fs-radius-input); background: var(--fs-surface-card); color: var(--fs-on-surface); font: inherit; font-size: var(--fs-font-size-ui); }
  .name-form input:focus-visible { outline: none; border: 2px solid var(--fs-focus-ring); }
  .name-form:has(.form-error:not(:empty)) input[name="name"] { border: 2px solid var(--fs-error); }
  .name-form button { min-block-size: 2.375rem; }
  .form-error { flex: 1 1 100%; margin: 0; color: var(--fs-error); font-size: var(--fs-font-size-small); }
  .form-error:empty { display: none; }
  .status, .alert { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-2) var(--fs-space-3); overflow-wrap: anywhere; font-size: var(--fs-font-size-small); }
  .status:empty, .alert:empty { display: none; }
  .status { color: var(--fs-on-surface-variant); }
  .alert { color: var(--fs-error); }
  .alert p, .status p { margin: 0; }
  .main { display: flex; flex-wrap: wrap; align-items: flex-start; gap: var(--fs-space-4); min-width: 0; }
  .listing, .viewer { position: relative; flex: 1 1 24rem; display: flex; flex-direction: column; gap: var(--fs-space-2); min-width: 0; }
  .panels { flex: 0 1 20rem; display: flex; flex-direction: column; gap: var(--fs-space-3); min-width: 0; }
  .panels section { min-width: 0; }
  .drop { position: absolute; inset: 0; z-index: 1; display: flex; align-items: center; justify-content: center; padding: var(--fs-space-4); border: 2px dashed var(--fs-primary); border-radius: var(--fs-radius); background: var(--fs-primary-container); color: var(--fs-on-primary-container); font-weight: 600; text-align: center; }
  .summary, .more, .limit { margin: 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .viewer header { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-2) var(--fs-space-3); }
  .viewer h2 { flex: 1 1 8rem; min-width: 0; margin: 0; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); font-weight: 600; color: var(--fs-on-surface); overflow-wrap: anywhere; }
  .position { font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  [hidden] { display: none !important; }
`;

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

/** A `type="button"` with a class and its label. */
function button(className: string, text: string): HTMLButtonElement {
  const out = node("button", className, text);
  out.type = "button";
  return out;
}

/** A labelled text input, returning the label element and the input. */
function field(text: string, name: string): { label: HTMLLabelElement; input: HTMLInputElement } {
  const label = document.createElement("label");
  const input = document.createElement("input");
  input.type = "text";
  input.name = name;
  input.autocomplete = "off";
  label.append(node("span", "field-label", text), input);
  return { label, input };
}

/** The toolbar and its controls. */
function buildToolbar(labels: LabelTable): { element: HTMLElement; bar: Frame["bar"] } {
  const element = node("div", "toolbar");
  element.setAttribute("role", "toolbar");
  element.setAttribute("aria-label", labels.fileToolbar);
  const upload = button("upload primary", labels.fileUpload);
  const newFolder = button("new-folder", labels.fileNewFolder);
  const rename = button("rename", labels.fileRenameMove);
  const remove = button("remove", labels.fileRemove);
  for (const target of [rename, remove]) target.setAttribute("aria-disabled", "true");
  const list = button("layout", labels.fileViewList);
  const grid = button("layout", labels.fileViewGrid);
  list.dataset.layout = "list";
  grid.dataset.layout = "grid";
  list.setAttribute("aria-pressed", "true");
  grid.setAttribute("aria-pressed", "false");
  const group = node("div", "group");
  group.setAttribute("role", "group");
  group.setAttribute("aria-label", labels.fileLayouts);
  group.append(list, grid);
  const input = document.createElement("input");
  input.type = "file";
  input.multiple = true;
  input.hidden = true;
  input.tabIndex = -1;
  input.setAttribute("aria-hidden", "true");
  element.append(upload, newFolder, rename, remove, node("span", "spacer"), group, input);
  return { element, bar: { upload, newFolder, rename, remove, list, grid, input } };
}

/** The name form. */
function buildForm(labels: LabelTable): Frame["form"] {
  const element = node("form", "name-form");
  element.hidden = true;
  element.noValidate = true;
  const name = field(labels.fileFieldName, "name");
  const folder = field(labels.fileFieldFolder, "folder");
  const title = node("h3", "form-title");
  const error = node("p", "form-error");
  error.setAttribute("role", "alert");
  const submit = node("button", "submit primary", labels.fileSave);
  submit.type = "submit";
  const cancel = button("cancel", labels.fileCancel);
  element.append(title, name.label, folder.label, error, submit, cancel);
  return {
    element,
    title,
    name: name.input,
    folderField: folder.label,
    folder: folder.input,
    error,
    submit,
    cancel,
  };
}

/** The listing area. */
function buildListing(labels: LabelTable): Frame["listing"] {
  const element = node("section", "listing");
  const drop = node("div", "drop");
  drop.hidden = true;
  const states = document.createElement("fs-states") as FsStates;
  states.configure({ labels });
  states.hidden = true;
  const host = node("div", "list-host");
  const summary = node("p", "summary");
  const more = node("p", "more", labels.fileMoreToLoad);
  const limit = node("p", "limit");
  more.hidden = true;
  limit.hidden = true;
  element.append(drop, states, host, summary, more, limit);
  return { element, drop, states, host, summary, more, limit };
}

/** The viewer with its header and preview. */
function buildViewer(labels: LabelTable): Frame["viewer"] {
  const element = node("section", "viewer");
  element.hidden = true;
  element.setAttribute("aria-label", labels.filePreview);
  const back = button("back", labels.fileBackToList);
  const title = node("h2", "viewer-title");
  title.tabIndex = -1;
  const position = node("span", "position");
  const previous = button("previous", labels.filePrevious);
  const next = button("next", labels.fileNext);
  const header = document.createElement("header");
  header.append(back, title, position, previous, next);
  const preview = document.createElement("fs-file-preview") as FsFilePreview;
  element.append(header, preview);
  return { element, back, title, position, previous, next, preview };
}

/**
 * Build the frame.
 *
 * @param labels The active label table.
 * @returns The frame and its references.
 */
export function buildFrame(labels: LabelTable): Frame {
  const root = node("div", "frame");
  const breadcrumb = document.createElement("fs-breadcrumb") as FsBreadcrumb;
  breadcrumb.configure({ labels });
  const { element: toolbar, bar } = buildToolbar(labels);
  const form = buildForm(labels);
  const status = node("div", "status");
  status.setAttribute("role", "status");
  const alert = node("div", "alert");
  alert.setAttribute("role", "alert");
  const listing = buildListing(labels);
  const viewer = buildViewer(labels);
  const panels = node("aside", "panels");
  panels.hidden = true;
  panels.setAttribute("aria-label", labels.filePanels);
  const main = node("div", "main");
  main.append(listing.element, viewer.element, panels);
  root.append(breadcrumb, toolbar, form.element, status, alert, main);
  return { root, breadcrumb, bar, form, status, alert, listing, viewer, panels };
}
