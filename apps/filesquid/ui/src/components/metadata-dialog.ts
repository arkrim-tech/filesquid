/**
 * The Add property dialog of the Description tab (UI-F2.4, UI-S-031, mockup
 * `resources/ui-mockups/v3/29-metadata-add-property.svg`): "Add property" naming the file, the
 * search field, the candidate groups DUBLIN CORE TERMS, SCHEMA.ORG SUGGESTION ("Added only
 * after you confirm. Not part of Dublin Core.") and REGISTERED NAMESPACES, "Key in a custom
 * namespace…" when the library declares one, "Value for {property}" with its field, and
 * Cancel and **Add**, the dialog's one filled primary button. The same builder makes the edit
 * form: the property fixed, the value filled, **Save**. {@link buildMetadataDialog} builds it
 * from the label table and returns typed references, so the panel never queries it by
 * selector; the panel performs the reads and the write.
 *
 * The dialog sits on `--fs-surface-overlay`, so it draws no muted, placeholder or error text
 * there except the eyebrows, the search placeholder and the refusal message (on the
 * card-coloured message box); the selected candidate sits on the active tint, the one surface
 * the active text's exemption covers (SRS-UI-007, UI-S-015).
 */
import { formatLabel, type LabelTable } from "../theme/labels";
import type { CandidateGroups } from "../files/metadata";
import type { FileRef } from "../files/model";
import { icon } from "./icons";

/** What the dialog is for: adding a property, or editing one value. */
export type MetadataDialogMode =
  | { readonly kind: "add"; readonly customAllowed: boolean }
  | { readonly kind: "edit"; readonly curie: string; readonly value: string };

/** What the dialog names. */
export interface MetadataDialogParams {
  /** The active label table. */
  readonly labels: LabelTable;
  /** The file described. */
  readonly file: FileRef;
  /** Add or edit. */
  readonly mode: MetadataDialogMode;
}

/** References into a built dialog. */
export interface MetadataDialog {
  /** The scrim that holds the dialog; append it to the panel's shadow root. */
  readonly scrim: HTMLElement;
  /** The dialog. */
  readonly dialog: HTMLFormElement;
  /** The close button in the head. */
  readonly close: HTMLButtonElement;
  /** Cancel. */
  readonly cancel: HTMLButtonElement;
  /** Add or Save. */
  readonly confirm: HTMLButtonElement;
  /** The message line for a refused value, key or write. */
  readonly error: HTMLElement;
  /** The search field (add only). */
  readonly search?: HTMLInputElement;
  /** The candidate groups (add only). */
  readonly results?: HTMLElement;
  /** Reveals the custom-key field (add, with a custom namespace). */
  readonly customButton?: HTMLButtonElement;
  /** The custom-key field (add, with a custom namespace). */
  readonly customField?: HTMLElement;
  /** The custom key (add, with a custom namespace). */
  readonly customInput?: HTMLInputElement;
  /** The value section, hidden until a property is chosen. */
  readonly valueSection: HTMLElement;
  /** "Value for {property}". */
  readonly valueLabel: HTMLElement;
  /** The value. */
  readonly valueInput: HTMLInputElement;
}

/** Most candidates one group lists. */
export const MAX_DIALOG_CANDIDATES = 20;

/** The dialog's own rules on top of `CONTROL_STYLE`'s `.scrim`, `.dialog` and buttons. */
const STYLE = `
  .metadata-dialog { gap: 0; padding: 0; }
  .metadata-body { padding: var(--fs-space-4) var(--fs-space-4) var(--fs-space-5); }
  .metadata-body > * { margin-inline: var(--fs-space-1); }
  .metadata-head { display: flex; align-items: start; gap: var(--fs-space-3); }
  .metadata-head .dialog-title { flex: 1 1 auto; min-width: 0; overflow-wrap: anywhere; }
  .icon-button { display: inline-flex; align-items: center; justify-content: center; inline-size: 1.75rem; block-size: 1.75rem; padding: 0; border: 0; border-radius: var(--fs-radius-item); background: none; color: var(--fs-icon-muted); cursor: pointer; }
  .icon-button:not(:disabled):hover { background: var(--fs-surface-raised); color: var(--fs-icon); }
  .icon-button:disabled { color: var(--fs-icon-disabled); cursor: default; }
  .file-name { margin-block: var(--fs-space-1) var(--fs-space-4); font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); overflow-wrap: anywhere; }
  .search-field { position: relative; }
  .search-field .icon { position: absolute; inset-inline-start: 10px; inset-block-start: 50%; translate: 0 -50%; color: var(--fs-icon-muted); pointer-events: none; }
  input.search { display: block; box-sizing: border-box; inline-size: 100%; padding-inline-start: var(--fs-space-5); }
  .results { margin-block-start: var(--fs-space-3); }
  .group { margin-block-end: var(--fs-space-3); }
  .group .eyebrow { margin-block-end: var(--fs-space-2); }
  .candidate { display: grid; gap: var(--fs-space-1); inline-size: 100%; box-sizing: border-box; margin: 0; padding: var(--fs-space-2) var(--fs-space-2); border: 0; border-radius: var(--fs-radius-input); background: none; color: var(--fs-on-surface); font-family: var(--fs-font-sans); text-align: start; cursor: pointer; transition: background-color var(--fs-motion-duration); }
  .candidate:hover { background: var(--fs-surface-raised); }
  .candidate[aria-pressed="true"], .candidate[aria-pressed="true"]:hover { background: var(--fs-primary-container); }
  .candidate .curie { font-family: var(--fs-font-mono); font-size: var(--fs-font-size-small); font-weight: 600; color: var(--fs-on-surface-variant); overflow-wrap: anywhere; }
  .group[data-group="dcterms"] .candidate .curie { color: var(--fs-primary); }
  .group .candidate[aria-pressed="true"] .curie { color: var(--fs-on-primary-container); }
  .candidate .description { font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); overflow-wrap: anywhere; }
  .candidate[aria-pressed="true"] .description { color: var(--fs-on-primary-container); }
  .no-candidates { margin: var(--fs-space-3) 0 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); overflow-wrap: anywhere; }
  .custom-row { margin-block-start: var(--fs-space-2); padding-block-start: var(--fs-space-2); border-block-start: 1px solid var(--fs-outline); }
  .custom-row .button { padding-inline: var(--fs-space-2); }
  .custom-field { display: block; margin-block-start: var(--fs-space-2); }
  .field-label { display: block; margin-block: var(--fs-space-3) var(--fs-space-2); font-size: var(--fs-font-size-small); font-weight: 600; color: var(--fs-on-surface); overflow-wrap: anywhere; }
  .field-input { display: block; box-sizing: border-box; inline-size: 100%; }
  .error { margin-block: var(--fs-space-2) 0; padding: var(--fs-space-2); border-radius: var(--fs-radius-item); background: var(--fs-surface-card); color: var(--fs-error); font-size: var(--fs-font-size-small); overflow-wrap: anywhere; }
  .error:empty { display: none; }
  .metadata-dialog .dialog-actions { padding: var(--fs-space-3) var(--fs-space-4); }
  .metadata-dialog .dialog-actions .button { margin-inline: 0; }
  .metadata-dialog .dialog-actions .confirm { margin-inline-end: var(--fs-space-1); }
  [hidden] { display: none !important; }
`;

/** CSS text of the Add property dialog. */
export const METADATA_DIALOG_STYLE = STYLE;

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

/** The head: the title and the close button. */
function head(
  labels: LabelTable,
  title: string,
): { element: HTMLElement; close: HTMLButtonElement } {
  const element = node("div", "metadata-head");
  const heading = node("h2", "dialog-title", title);
  heading.id = "metadata-title";
  const close = node("button", "icon-button close");
  close.type = "button";
  close.setAttribute("aria-label", labels.metadataDialogClose);
  close.append(icon("close", 20));
  element.append(heading, close);
  return { element, close };
}

/** The search field with its icon. */
function searchField(labels: LabelTable): { element: HTMLElement; input: HTMLInputElement } {
  const element = node("div", "search-field");
  const input = node("input", "input search");
  input.type = "text";
  input.autocomplete = "off";
  input.placeholder = labels.metadataSearch;
  input.setAttribute("aria-label", labels.metadataSearch);
  element.append(icon("search", 16), input);
  return { element, input };
}

/** The custom-key control: the button that reveals the field, and the field. */
function customKey(labels: LabelTable): {
  row: HTMLElement;
  button: HTMLButtonElement;
  field: HTMLElement;
  input: HTMLInputElement;
} {
  const row = node("div", "custom-row");
  const button = node("button", "button ghost custom-key");
  button.type = "button";
  button.setAttribute("aria-expanded", "false");
  button.append(icon("plus"), document.createTextNode(labels.metadataCustomKey));
  const field = node("label", "custom-field");
  field.hidden = true;
  const words = node("span", "field-label", labels.metadataCustomKeyField);
  const input = node("input", "input field-input custom-input mono");
  input.type = "text";
  input.autocomplete = "off";
  field.append(words, input);
  row.append(button, field);
  return { row, button, field, input };
}

/** The value section: its label and field. */
function valueSection(): {
  section: HTMLElement;
  label: HTMLElement;
  input: HTMLInputElement;
} {
  const section = node("div", "value-section");
  const label = node("label", "field-label value-label");
  label.htmlFor = "metadata-value";
  const input = node("input", "input field-input value-input");
  input.id = "metadata-value";
  input.type = "text";
  input.autocomplete = "off";
  section.append(label, input);
  return { section, label, input };
}

/** The footer: Cancel and Add or Save. */
function actions(
  labels: LabelTable,
  mode: MetadataDialogMode,
): { element: HTMLElement; cancel: HTMLButtonElement; confirm: HTMLButtonElement } {
  const element = node("div", "dialog-actions");
  const cancel = node("button", "button secondary large cancel", labels.fileCancel);
  cancel.type = "button";
  const text = mode.kind === "add" ? labels.metadataAddConfirm : labels.fileSave;
  const confirm = node("button", "button primary large confirm", text);
  confirm.type = "submit";
  confirm.disabled = mode.kind === "add";
  element.append(cancel, confirm);
  return { element, cancel, confirm };
}

/** The add-mode parts: search, results and the custom key. */
function addParts(
  labels: LabelTable,
  customAllowed: boolean,
): Partial<MetadataDialog> & {
  nodes: HTMLElement[];
} {
  const search = searchField(labels);
  const results = node("div", "results");
  const nodes: HTMLElement[] = [search.element, results];
  const parts: Partial<MetadataDialog> = { search: search.input, results };
  if (customAllowed) {
    const custom = customKey(labels);
    nodes.push(custom.row);
    Object.assign(parts, {
      customButton: custom.button,
      customField: custom.field,
      customInput: custom.input,
    });
  }
  return { ...parts, nodes };
}

/**
 * Build the Add property dialog, or the edit form of one value.
 *
 * @param params The file, the mode and the words.
 * @returns The dialog and its references; nothing is attached or sent.
 */
export function buildMetadataDialog(params: MetadataDialogParams): MetadataDialog {
  const { labels, file, mode } = params;
  const title =
    mode.kind === "add"
      ? labels.metadataAdd
      : formatLabel(labels.metadataEditTitle, { property: mode.curie });
  const top = head(labels, title);
  const name = node("p", "file-name", file.display_name);
  name.id = "metadata-file";
  const add = mode.kind === "add" ? addParts(labels, mode.customAllowed) : { nodes: [] };
  const value = valueSection();
  const error = node("p", "error");
  error.setAttribute("role", "alert");
  const buttons = actions(labels, mode);
  const dialog = node("form", "dialog metadata-dialog");
  dialog.noValidate = true;
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  dialog.setAttribute("aria-labelledby", "metadata-title");
  dialog.setAttribute("aria-describedby", "metadata-file");
  const body = node("div", "metadata-body");
  body.append(top.element, name, ...add.nodes, value.section, error);
  dialog.append(body, buttons.element);
  const scrim = node("div", "scrim");
  scrim.append(dialog);
  const built: MetadataDialog = {
    scrim,
    dialog,
    close: top.close,
    cancel: buttons.cancel,
    confirm: buttons.confirm,
    error,
    search: add.search,
    results: add.results,
    customButton: add.customButton,
    customField: add.customField,
    customInput: add.customInput,
    valueSection: value.section,
    valueLabel: value.label,
    valueInput: value.input,
  };
  if (mode.kind === "edit") {
    chooseProperty(built, labels, mode.curie);
    value.input.value = mode.value;
  } else {
    value.section.hidden = true;
  }
  return built;
}

/**
 * Set the property the value is for: show "Value for {property}" and enable Add or Save;
 * with no property, hide the value section and disable the button. Mutates the dialog.
 *
 * @param dialog The built dialog.
 * @param labels The active label table.
 * @param curie The chosen property, or undefined for none.
 */
export function chooseProperty(
  dialog: MetadataDialog,
  labels: LabelTable,
  curie: string | undefined,
): void {
  dialog.valueSection.hidden = curie === undefined;
  dialog.confirm.disabled = curie === undefined;
  if (curie === undefined) return;
  dialog.valueLabel.textContent = formatLabel(labels.metadataValueFor, { property: curie });
}

/** One candidate button. */
function candidate(
  curie: string,
  description: string | undefined,
  selected: boolean,
): HTMLButtonElement {
  const button = node("button", "candidate");
  button.type = "button";
  button.dataset.curie = curie;
  button.setAttribute("aria-pressed", String(selected));
  button.append(node("span", "curie", curie));
  if (description !== undefined) button.append(node("span", "description", description));
  return button;
}

/** One group of candidates under its eyebrow. */
function group(
  name: string,
  eyebrow: string,
  entries: readonly { curie: string; description?: string }[],
  selected: string | undefined,
): HTMLElement {
  const element = node("div", "group");
  element.dataset.group = name;
  element.setAttribute("role", "group");
  element.setAttribute("aria-label", eyebrow);
  element.append(node("p", "eyebrow", eyebrow));
  for (const entry of entries.slice(0, MAX_DIALOG_CANDIDATES)) {
    element.append(candidate(entry.curie, entry.description, entry.curie === selected));
  }
  return element;
}

/** How a candidate is described: by its CURIE, for the dcterms and registered groups. */
export type CandidateDescriber = (curie: string) => string | undefined;

/**
 * Draw the candidate groups for what the owner typed, in the API's add order, or the
 * no-candidates line. Mutates the dialog's results.
 *
 * @param dialog The built dialog (add mode).
 * @param labels The active label table.
 * @param typed The text typed.
 * @param groups The groups to draw.
 * @param describe Describes a dcterms or registered candidate.
 * @param selected The chosen CURIE, drawn pressed.
 */
export function renderCandidates(
  dialog: MetadataDialog,
  labels: LabelTable,
  typed: string,
  groups: CandidateGroups,
  describe: CandidateDescriber,
  selected: string | undefined,
): void {
  const results = dialog.results;
  if (!results) return;
  const blocks: HTMLElement[] = [];
  const entry = (curie: string) => ({ curie, description: describe(curie) });
  if (groups.dcterms.length > 0) {
    blocks.push(group("dcterms", labels.metadataGroupDcterms, groups.dcterms.map(entry), selected));
  }
  if (groups.schema !== undefined) {
    const schema = [{ curie: groups.schema, description: labels.metadataSchemaNote }];
    blocks.push(group("schema", labels.metadataGroupSchema, schema, selected));
  }
  if (groups.registered.length > 0) {
    const registered = groups.registered.map((candidate) => entry(candidate.curie));
    blocks.push(group("registered", labels.metadataGroupRegistered, registered, selected));
  }
  if (blocks.length === 0 && typed.trim() !== "") {
    const word = typed.trim();
    blocks.push(node("p", "no-candidates", formatLabel(labels.metadataNoCandidates, { word })));
  }
  results.replaceChildren(...blocks);
}

/**
 * Keep Tab and Shift+Tab inside a modal dialog: from its last control Tab wraps to the
 * first, and from the first Shift+Tab wraps to the last. Moves focus.
 *
 * @param event The keydown event.
 * @param dialog The dialog.
 * @param active The element focused now (the shadow root's active element).
 */
export function keepTabInside(
  event: KeyboardEvent,
  dialog: HTMLElement,
  active: Element | null,
): void {
  if (event.key !== "Tab") return;
  const stops = [
    ...dialog.querySelectorAll<HTMLButtonElement | HTMLInputElement>("button, input"),
  ].filter((el) => !el.disabled && !el.hidden && el.closest("[hidden]") === null);
  const first = stops[0];
  const last = stops[stops.length - 1];
  if (!event.shiftKey && active === last && first) {
    event.preventDefault();
    first.focus();
  } else if (event.shiftKey && active === first && last) {
    event.preventDefault();
    last.focus();
  }
}
