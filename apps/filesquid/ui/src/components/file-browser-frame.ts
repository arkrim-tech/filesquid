/**
 * The static frame of `<fs-file-browser>` (UI-F2.2): the breadcrumb, the toolbar, the name form
 * used for a new folder and for rename or move, the two live regions, the listing area (states,
 * the list itself, the summary and the drop target), the viewer with its preview, and the panel
 * area. {@link buildFrame} builds it from the active label table and returns typed references,
 * so the element and the writes controller never query the DOM by selector. Every word is set
 * with `textContent` from the label table.
 */
import type { LabelTable } from "../theme/labels";
import { icon, type IconName } from "./icons";
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
  /** The viewer: its top bar (UI-S-031) and the stage with the preview between the steps. */
  readonly viewer: {
    readonly element: HTMLElement;
    readonly back: HTMLButtonElement;
    /** The collection the shown file is in; hidden when the location names none. */
    readonly collection: HTMLElement;
    /** The file-type badge (UI-S-026). */
    readonly badge: HTMLElement;
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
 * The frame's stylesheet (UI-S-024, UI-S-026), after the shared `CONTROL_STYLE` that draws
 * its buttons (`.button`: 36 px, 14 px 600, radius 7 px; Upload and Save `.primary`) and inputs
 * (`.input`: 36 px, a 1.5 px `--fs-outline-control` border, a 2 px `--fs-focus-ring` border on
 * focus and a 2 px `--fs-error` border on the field marked `aria-invalid`, mockup 44): the
 * pressed layout half on the active tint, and the name form drawn as the rename form of mockup
 * 14 (a 1 px `--fs-outline-strong` border, radius 10 px, the 20 px 700 heading-font title) with
 * the message below the field in `--fs-error`. The form sits in the page, so it is on
 * `--fs-surface-card`: the error text there meets the contrast rule in both themes, which it
 * would not on `--fs-surface-overlay` in dark (4.01:1).
 *
 * The viewer (UI-S-031, mockup 22, Task UI-F2.19): while a file is shown the host carries
 * `viewing`, cancels the shell's `--fs-space-4` padding of `main` (the local
 * `--fs-viewer-bleed`) and fills it, and the folder's trail, toolbar and name form are hidden,
 * because mockup 22 draws the viewer's bar straight under the header. The bar is 52 px on
 * `--fs-surface-card` with a 1 px `--fs-outline` bottom line: back (a 16 px chevron), the
 * collection (13 px `--fs-on-surface-variant`), the badge of UI-S-026, the display name (14 px
 * 600 `--fs-on-surface-heading`) and the position (13 px `--fs-on-surface-muted`). The stage
 * on `--fs-surface-container` measures mockup 22: previous and next are 40 px round controls
 * 24 px from the sides, the media box starts 56 px inside them and 37 px under the bar, its 36 px
 * zoom chip row follows 20 px under it, and the last 228 px stay empty for the deferred
 * filmstrip (mockup 22 leaves 240 px under the chip in its window, whose bottom is 12 px below
 * the end of the shell's `main`).
 */
export const STYLE = `
  .frame { display: flex; flex-direction: column; gap: var(--fs-space-3); min-width: 0; font-size: var(--fs-font-size-ui); }
  .toolbar { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-2); }
  .spacer { flex: 1 1 auto; }
  .group { display: inline-flex; gap: var(--fs-space-1); }
  .button[aria-pressed="true"] { border-color: var(--fs-primary-container); background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  .name-form { display: flex; flex-wrap: wrap; align-items: end; gap: var(--fs-space-3); box-sizing: border-box; max-inline-size: 35rem; padding: var(--fs-space-4); border: 1px solid var(--fs-outline-strong); border-radius: var(--fs-radius-dialog); background: var(--fs-surface-card); }
  .form-title { flex: 1 1 100%; margin: 0; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-title); font-weight: 700; color: var(--fs-on-surface-heading); overflow-wrap: anywhere; }
  .form-title:empty { display: none; }
  .name-form label { display: flex; flex: 1 1 100%; flex-direction: column; gap: var(--fs-space-1); font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
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
  :host([viewing]) { --fs-viewer-bleed: calc(-1 * var(--fs-space-4)); display: block; margin: var(--fs-viewer-bleed); block-size: calc(100% + 2 * var(--fs-space-4)); }
  :host([viewing]) .frame { block-size: 100%; gap: 0; }
  :host([viewing]) fs-breadcrumb, :host([viewing]) .toolbar, :host([viewing]) .name-form { display: none; }
  :host([viewing]) .main { flex: 1 1 auto; flex-wrap: nowrap; align-items: stretch; gap: 0; min-height: 0; }
  .viewer { gap: 0; background: var(--fs-surface-container); }
  .bar { flex: none; display: flex; align-items: center; gap: var(--fs-space-2); box-sizing: border-box; block-size: 3.25rem; min-width: 0; padding: 0 var(--fs-space-3); border-block-end: 1px solid var(--fs-outline); background: var(--fs-surface-card); }
  .back { flex: none; display: inline-flex; align-items: center; justify-content: center; inline-size: 1rem; block-size: 1.5rem; padding: 0; border: 0; border-radius: var(--fs-radius-input); background: var(--fs-surface-card); color: var(--fs-icon); cursor: pointer; }
  .collection { flex: 0 1 auto; min-width: 0; margin-inline-end: var(--fs-space-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-font-size-small); font-weight: 400; color: var(--fs-on-surface-variant); }
  .bar .badge { flex: none; }
  .bar .badge:empty { display: none; }
  .viewer-title { flex: 0 1 auto; min-width: 0; margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); font-weight: 600; color: var(--fs-on-surface-heading); }
  .position { flex: none; font-size: var(--fs-font-size-small); font-weight: 400; color: var(--fs-on-surface-muted); }
  .stage { flex: 1 1 auto; display: grid; grid-template-columns: var(--fs-space-4) 2.5rem 3.5rem minmax(0, 1fr) 3.5rem 2.5rem var(--fs-space-4); grid-template-rows: 2.3125rem minmax(0, 1fr) 3.5rem 14.25rem; min-width: 0; min-height: 0; }
  .stage fs-file-preview { grid-column: 4; grid-row: 2 / 4; }
  .step { grid-row: 2; align-self: center; display: inline-flex; align-items: center; justify-content: center; box-sizing: border-box; inline-size: 2.5rem; block-size: 2.5rem; padding: 0; border: 1px solid var(--fs-outline); border-radius: var(--fs-radius-pill); background: var(--fs-surface-card); color: var(--fs-icon); cursor: pointer; }
  .step.previous { grid-column: 2; }
  .step.next { grid-column: 6; }
  .step[aria-disabled="true"] { color: var(--fs-icon-disabled); cursor: default; }
  @media (max-width: 40rem), (max-height: 40rem) {
    .stage { grid-template-columns: var(--fs-space-2) 2.5rem var(--fs-space-2) minmax(0, 1fr) var(--fs-space-2) 2.5rem var(--fs-space-2); grid-template-rows: var(--fs-space-3) minmax(0, 1fr) 3.5rem var(--fs-space-3); }
  }
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

/** A `type="button"` styled as a shared control button, with a class and its label. */
function button(
  className: string,
  text: string,
  kind: "primary" | "secondary" = "secondary",
): HTMLButtonElement {
  const out = node("button", `${className} button ${kind}`, text);
  out.type = "button";
  return out;
}

/** A labelled text input, returning the label element and the input. */
function field(text: string, name: string): { label: HTMLLabelElement; input: HTMLInputElement } {
  const label = document.createElement("label");
  const input = document.createElement("input");
  input.type = "text";
  input.className = "input";
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
  const upload = button("upload", labels.fileUpload, "primary");
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
  const submit = node("button", "submit button primary", labels.fileSave);
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

/** A `type="button"` that shows only an icon and is named by `name` from the label table. */
function iconButton(
  className: string,
  name: string,
  glyph: IconName,
  size: number,
): HTMLButtonElement {
  const out = node("button", className);
  out.type = "button";
  out.setAttribute("aria-label", name);
  out.append(icon(glyph, size));
  return out;
}

/**
 * The viewer: the top bar of UI-S-031 (the UI-F2.2 header, completed) and the stage with the
 * preview between the previous and next controls. No filmstrip (deferred, owner decision
 * 2026-10-30) and no zoom control beyond Fit.
 */
function buildViewer(labels: LabelTable): Frame["viewer"] {
  const element = node("section", "viewer");
  element.hidden = true;
  element.setAttribute("aria-label", labels.filePreview);
  const back = iconButton("back", labels.fileBackToList, "chevron-back", 16);
  const collection = node("span", "collection");
  collection.hidden = true;
  const badge = node("span", "badge");
  const title = node("h2", "viewer-title");
  title.tabIndex = -1;
  const position = node("span", "position");
  const header = node("header", "bar");
  header.append(back, collection, badge, title, position);
  const previous = iconButton("previous step", labels.filePrevious, "chevron-back", 20);
  const next = iconButton("next step", labels.fileNext, "chevron", 20);
  const preview = document.createElement("fs-file-preview") as FsFilePreview;
  const stage = node("div", "stage");
  stage.append(previous, preview, next);
  element.append(header, stage);
  return { element, back, collection, badge, title, position, previous, next, preview };
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
