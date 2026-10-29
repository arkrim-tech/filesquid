/**
 * `<fs-metadata-panel>`: the Metadata panel (UI-F2.4, MD-S-011, ADR 028 Decision 6), shown as
 * the Description tab of the file panel (UI-S-022, UI-S-031, mockups
 * `resources/ui-mockups/v3/28-panel-metadata.svg`, `29-metadata-add-property.svg` and
 * `25-panel-states.svg`). It lists the file's descriptive statements, each owned by the
 * metadata module (UI-S-003): the `dcterms:`, `schema:` and registered-namespace rows keyed
 * by their CURIE, an owner-asserted value in the owner tone with the chip "You" and edit and
 * remove, a value read from the file in the extracted tone with the chip "File" and copy only.
 * `exif:` statements are the File info tab's technical rows (UI-S-031) and are not listed.
 * Add property opens the dialog of mockup 29: the `/terms/` register first, then the
 * Schema.org suggestion, then registered namespaces, or a key in a declared custom namespace.
 *
 * It reads and writes only through a {@link MetadataSource} (the metadata property routes of
 * API-S-035, UI-S-012). Each set or removal is one call that commits one OCFL version; the
 * panel then announces it, dispatches `fs-files-change` (`detail.operation` `metadata.set` or
 * `metadata.remove` with the `commit`) so the page lists the new version, and reads the
 * statements of that version again.
 *
 * It is a panel of the file panel's named slots: it receives the file as its `fileRef`
 * property and holds no reference to any other panel, so it mounts alone. Its table is the
 * shared {@link PropertyTable} (filter, count, rows with copy, the loading, no-match and error
 * states of mockup 25), extended with the source chips, row actions, the Add property button
 * and the empty state.
 *
 * Accessibility (UI-S-016): the shared stylesheet comes first; the rows are a description list;
 * the actions are named by their property; a commit is announced in a polite status region;
 * the error state is an alert; the dialog is modal, keeps Tab inside, and Escape cancels it
 * and returns focus to the control that opened it; a load never moves focus.
 */
import { formatLabel, type LabelTable } from "../theme/labels";
import { ApiError } from "../api/client";
import { focusElement } from "../a11y/focus";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { icon } from "./icons";
import {
  METADATA_DIALOG_STYLE,
  buildMetadataDialog,
  chooseProperty,
  keepTabInside,
  renderCandidates,
  type MetadataDialog,
  type MetadataDialogMode,
} from "./metadata-dialog";
import { PROPERTY_TABLE_STYLE, PropertyTable } from "./property-table";
import {
  customKeyProblem,
  customNamespaces,
  dctermsDescription,
  groupCandidates,
  matchingTerms,
  metadataCountText,
  statementRows,
  validTermName,
  valueProblem,
  type MetadataCommit,
  type MetadataRow,
  type NamespaceRegistration,
  type TermCandidates,
} from "../files/metadata";
import type { MetadataSource } from "../files/metadata-source";
import type { FileRef } from "../files/model";
import { errorKind, failureText } from "../files/ops";

/** What the panel needs. */
export interface MetadataPanelOptions {
  /** The active label table. */
  labels: LabelTable;
  /** Reads statements, places terms and namespaces; sets and removes statements. */
  source: MetadataSource;
  /** Whether the browser is online; `navigator.onLine` when absent. */
  online?: () => boolean;
}

/** The `detail.operation` values of the `fs-files-change` events the panel dispatches. */
export const METADATA_OPERATIONS = { set: "metadata.set", remove: "metadata.remove" } as const;

/** An open dialog and what the panel knows about it. */
interface OpenDialog {
  readonly parts: MetadataDialog;
  readonly mode: MetadataDialogMode;
  readonly opener: HTMLElement;
  busy: boolean;
  /** The CURIE chosen, from a candidate or the custom key. */
  selected?: string;
  /** The API's add order for the name last looked up. */
  answer?: TermCandidates;
  /** Cancels the lookup in flight. */
  lookup?: AbortController;
}

const STYLE = `
  :host { display: flex; flex-direction: column; min-block-size: 100%; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); color: var(--fs-on-surface); }
  :host([hidden]) { display: none; }
  .announce { position: absolute; inline-size: 1px; block-size: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
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

/** True when two references name the same file at the same version. */
function sameRef(a: FileRef | null, b: FileRef | null): boolean {
  if (a === null || b === null) return a === b;
  return (
    a.library_id === b.library_id &&
    a.object_id === b.object_id &&
    a.path === b.path &&
    a.version === b.version
  );
}

/**
 * The words for a refused metadata write: the route's 400 as a refused property or value,
 * its 404 as a property no longer in the record, otherwise the shared failure words. Pure.
 */
function refusalText(error: unknown, labels: LabelTable): string {
  if (error instanceof ApiError && error.code === "invalid_request") return labels.metadataRefused;
  if (error instanceof ApiError && error.code === "not_found") return labels.metadataNotFound;
  return failureText(error, labels);
}

/** Press the candidate of `curie` and release every other, in place. Side effect: the DOM. */
function markSelected(dialog: OpenDialog, curie: string | undefined): void {
  const buttons = dialog.parts.results?.querySelectorAll<HTMLElement>("button.candidate") ?? [];
  for (const button of buttons) {
    button.setAttribute("aria-pressed", String(button.dataset.curie === curie));
  }
}

/** The Add property button below the rows. */
function addButtonOf(root: ShadowRoot): HTMLElement | null {
  return root.querySelector<HTMLElement>(".body > button.add");
}

/** The edit button of the row keyed by `curie`, once the rows show again. */
function editButtonOf(curie: string): (root: ShadowRoot) => HTMLElement | null {
  return (root) => {
    const rows = [...root.querySelectorAll<HTMLElement>(".row")];
    const row = rows.find((element) => element.dataset.key === curie);
    return row?.querySelector<HTMLElement>("button.edit") ?? null;
  };
}

/** Custom element that shows and edits the descriptive statements of one file. */
export class FsMetadataPanel extends HTMLElement {
  private options?: MetadataPanelOptions;
  private table?: PropertyTable;
  private announcer?: HTMLElement;
  private file: FileRef | null = null;
  private rows: MetadataRow[] = [];
  private generation = 0;
  private abort?: AbortController;
  private dialog?: OpenDialog;
  private registry?: { library: string; namespaces: NamespaceRegistration[] };

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the labels and source, render, and load the file already given, if any.
   *
   * @param options Labels, the source and an optional online check.
   */
  configure(options: MetadataPanelOptions): void {
    this.options = options;
    this.build(options);
    void this.load();
  }

  /** The tab label this panel asks for ("Description"). */
  get tabLabel(): string {
    return this.options?.labels.filePanelDescription ?? "";
  }

  /** The file version shown, or null. */
  get fileRef(): FileRef | null {
    return this.file;
  }

  /**
   * Show another file version (null clears the panel). Any change drops an open dialog; the
   * same version already shown is not read again.
   */
  set fileRef(file: FileRef | null) {
    this.dropDialog();
    const shown = ["ready", "empty", "no-match"].includes(this.table?.state ?? "");
    if (sameRef(file, this.file) && shown) return;
    this.file = file;
    void this.load();
  }

  /** Cancel a read in flight and drop the dialog when the element leaves the document. */
  disconnectedCallback(): void {
    this.abort?.abort();
    this.dropDialog();
  }

  /** Build the stylesheet, the announcer and the table. */
  private build(options: MetadataPanelOptions): void {
    const labels = options.labels;
    const locale = () => document.documentElement.lang || "en";
    const table = new PropertyTable({
      labels,
      filterPlaceholder: labels.metadataFilter,
      loadingText: labels.metadataLoading,
      errorTitle: labels.metadataError,
      countText: (shown, total) => metadataCountText(labels, shown, total, locale()),
      noMatchTitle: (word) => formatLabel(labels.metadataNoMatch, { word }),
      onRetry: () => void this.load(),
      online: options.online,
      onState: (state) => void (this.dataset.state = state),
      rowChip: (row) => this.chipFor(row as MetadataRow),
      rowActions: (row) => this.actionsFor(row as MetadataRow),
      afterRows: () => this.addButton(),
      emptyState: () => this.emptyState(),
    });
    this.table = table;
    this.announcer = node("p", "announce");
    this.announcer.setAttribute("role", "status");
    const style = document.createElement("style");
    style.textContent =
      A11Y_STYLE + CONTROL_STYLE + STYLE + PROPERTY_TABLE_STYLE + METADATA_DIALOG_STYLE;
    this.shadowRoot?.replaceChildren(style, this.announcer, table.element);
  }

  /** Read the statements of the file shown and fill the table; ignore stale answers. */
  private async load(): Promise<void> {
    const { options, table, file } = this;
    if (!options || !table) return;
    this.abort?.abort();
    const generation = ++this.generation;
    if (!file) {
      this.rows = [];
      return table.clear();
    }
    const abort = new AbortController();
    this.abort = abort;
    table.loading();
    try {
      const answer = await options.source.statements(file, { signal: abort.signal });
      if (generation !== this.generation) return;
      this.rows = statementRows(answer.statements);
      table.rows(this.rows);
    } catch (error) {
      if (generation !== this.generation || abort.signal.aborted) return;
      console.warn("FileSquid could not read file statements", errorKind(error));
      table.error(error);
    }
  }

  /** The source chip of a row: "You" on the active tint, or "File" on the raised surface. */
  private chipFor(row: MetadataRow): { text: string; tone: "ok" | "muted" } | undefined {
    const labels = this.options?.labels;
    if (!labels) return undefined;
    return row.source === "asserted"
      ? { text: labels.metadataSourceOwner, tone: "ok" }
      : { text: labels.metadataSourceFile, tone: "muted" };
  }

  /** Edit and remove for an owner value; nothing for an extracted one. */
  private actionsFor(row: MetadataRow): HTMLButtonElement[] {
    const labels = this.options?.labels;
    if (!labels || !row.editable) return [];
    const edit = node("button", "edit");
    edit.type = "button";
    edit.setAttribute("aria-label", formatLabel(labels.metadataEdit, { property: row.curie }));
    edit.append(icon("edit", 16));
    edit.addEventListener("click", () => void this.openEdit(row, edit));
    const remove = node("button", "remove");
    remove.type = "button";
    remove.setAttribute("aria-label", formatLabel(labels.metadataRemove, { property: row.curie }));
    remove.append(icon("trash", 16));
    remove.addEventListener("click", () => void this.removeRow(row));
    return [edit, remove];
  }

  /** The ghost Add property button. */
  private addButton(): HTMLButtonElement {
    const button = node("button", "button ghost add");
    button.type = "button";
    button.append(icon("plus"), document.createTextNode(this.options?.labels.metadataAdd ?? ""));
    button.addEventListener("click", () => void this.openAdd(button));
    return button;
  }

  /** The empty state of mockup 25: the title, the hint and Add property. */
  private emptyState(): HTMLElement {
    const labels = this.options?.labels;
    const state = node("div", "state");
    state.setAttribute("role", "status");
    state.append(
      icon("info", 24),
      node("p", "title", labels?.metadataEmpty),
      node("p", "detail", labels?.metadataEmptyHint),
      this.addButton(),
    );
    return state;
  }

  /** Remove an owner value in one call, then announce it and list the new version. */
  private async removeRow(row: MetadataRow): Promise<void> {
    const { options, file } = this;
    if (!options || !file) return;
    try {
      const commit = await options.source.removeStatement(file, row.curie);
      this.afterCommit(METADATA_OPERATIONS.remove, file, commit, {
        words: formatLabel(options.labels.metadataRemoved, {
          property: row.curie,
          version: commit.version,
        }),
        focus: addButtonOf,
      });
    } catch (error) {
      console.warn("FileSquid could not remove a statement", errorKind(error));
      this.announce(refusalText(error, options.labels));
    }
  }

  /** Say something in the polite status region. Side effect: the DOM. */
  private announce(words: string): void {
    if (!this.announcer) return;
    this.announcer.textContent = "";
    this.announcer.textContent = words;
  }

  /**
   * A write committed: tell the page, announce it, read the statements of the new version
   * and move focus to `focus` once they show.
   */
  private afterCommit(
    operation: string,
    file: FileRef,
    commit: MetadataCommit,
    after: { words: string; focus: (root: ShadowRoot) => HTMLElement | null },
  ): void {
    this.dispatchEvent(
      new CustomEvent("fs-files-change", {
        detail: { operation, commit },
        bubbles: true,
        composed: true,
      }),
    );
    this.announce(after.words);
    this.file = { ...file, version: commit.version };
    void this.load().then(() => {
      const target = this.shadowRoot ? after.focus(this.shadowRoot) : null;
      if (target) focusElement(target);
    });
  }

  /** The library's registry, read once per library; an unreadable one counts as empty. */
  private async namespacesOf(file: FileRef): Promise<NamespaceRegistration[]> {
    const { options, registry } = this;
    if (!options) return [];
    if (registry?.library === file.library_id) return registry.namespaces;
    try {
      const answer = await options.source.namespaces({ library_id: file.library_id });
      this.registry = { library: file.library_id, namespaces: answer.namespaces };
    } catch (error) {
      console.warn("FileSquid could not read the namespace registry", errorKind(error));
      this.registry = { library: file.library_id, namespaces: [] };
    }
    return this.registry.namespaces;
  }

  /** Open the Add property dialog. */
  private async openAdd(opener: HTMLElement): Promise<void> {
    const { options, file } = this;
    if (!options || !file || this.dialog) return;
    const namespaces = await this.namespacesOf(file);
    if (this.file !== file || this.dialog) return;
    const mode: MetadataDialogMode = {
      kind: "add",
      customAllowed: customNamespaces(namespaces).length > 0,
    };
    const { parts } = this.show(mode, opener, file);
    parts.search?.addEventListener("input", () => this.onSearch());
    parts.customButton?.addEventListener("click", () => this.revealCustomKey());
    parts.customInput?.addEventListener("input", () => this.onCustomKey(namespaces));
    parts.results?.addEventListener("click", (event) => this.onCandidate(event));
    if (parts.search) focusElement(parts.search);
  }

  /** Open the edit form of one owner value. */
  private openEdit(row: MetadataRow, opener: HTMLElement): void {
    const { options, file } = this;
    if (!options || !file || this.dialog) return;
    const open = this.show({ kind: "edit", curie: row.curie, value: row.value }, opener, file);
    open.selected = row.curie;
    focusElement(open.parts.valueInput);
  }

  /** Build a dialog, wire its close, cancel, submit and keys, and attach it. */
  private show(mode: MetadataDialogMode, opener: HTMLElement, file: FileRef): OpenDialog {
    const labels = (this.options as MetadataPanelOptions).labels;
    const parts = buildMetadataDialog({ labels, file, mode });
    const open: OpenDialog = { parts, mode, opener, busy: false };
    this.dialog = open;
    parts.close.addEventListener("click", () => this.closeDialog());
    parts.cancel.addEventListener("click", () => this.closeDialog());
    parts.dialog.addEventListener("submit", (event) => {
      event.preventDefault();
      void this.submit();
    });
    parts.dialog.addEventListener("keydown", (event) => this.onDialogKey(event, parts.dialog));
    this.shadowRoot?.append(parts.scrim);
    return open;
  }

  /** Escape cancels; Tab and Shift+Tab stay inside the dialog. */
  private onDialogKey(event: KeyboardEvent, dialog: HTMLElement): void {
    if (event.key === "Escape") {
      event.preventDefault();
      this.closeDialog();
      return;
    }
    keepTabInside(event, dialog, this.shadowRoot?.activeElement ?? null);
  }

  /** The owner typed in the search: match locally, draw, and ask the API for a valid name. */
  private onSearch(): void {
    const dialog = this.dialog;
    const search = dialog?.parts.search;
    if (!dialog || !search || !this.options || !this.file) return;
    dialog.selected = undefined;
    chooseProperty(dialog.parts, this.options.labels, undefined);
    const typed = search.value.trim();
    this.drawCandidates(dialog, typed);
    dialog.lookup?.abort();
    if (!validTermName(typed)) return;
    const lookup = new AbortController();
    dialog.lookup = lookup;
    void this.lookUp(dialog, typed, lookup);
  }

  /** Ask the API where the typed name can go; draw the answer if the text is unchanged. */
  private async lookUp(dialog: OpenDialog, typed: string, lookup: AbortController): Promise<void> {
    const { options, file } = this;
    if (!options || !file) return;
    try {
      const answer = await options.source.placeTerm({ library_id: file.library_id }, typed, {
        signal: lookup.signal,
      });
      if (this.dialog !== dialog || lookup.signal.aborted) return;
      dialog.answer = answer;
      this.drawCandidates(dialog, dialog.parts.search?.value.trim() ?? "");
    } catch (error) {
      if (lookup.signal.aborted) return;
      console.warn("FileSquid could not place a term name", errorKind(error));
    }
  }

  /** Draw the groups for `typed` from the local names and the API's answer for it. */
  private drawCandidates(dialog: OpenDialog, typed: string): void {
    const labels = (this.options as MetadataPanelOptions).labels;
    const groups = groupCandidates(typed, dialog.answer, matchingTerms(typed));
    const iris = new Map((this.registry?.namespaces ?? []).map((n) => [n.prefix, n.iri]));
    const describe = (curie: string) => {
      const [prefix, term] = [
        curie.slice(0, curie.indexOf(":")),
        curie.slice(curie.indexOf(":") + 1),
      ];
      return prefix === "dcterms" ? dctermsDescription(term, labels) : iris.get(prefix);
    };
    renderCandidates(dialog.parts, labels, typed, groups, describe, dialog.selected);
  }

  /** A candidate was pressed: choose it and move to the value. */
  private onCandidate(event: Event): void {
    const dialog = this.dialog;
    const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button.candidate");
    if (!dialog || !button || !this.options) return;
    const curie = button.dataset.curie as string;
    dialog.selected = curie;
    if (dialog.parts.customInput) dialog.parts.customInput.value = "";
    dialog.parts.error.textContent = "";
    markSelected(dialog, curie);
    chooseProperty(dialog.parts, this.options.labels, curie);
    focusElement(dialog.parts.valueInput);
  }

  /** Show the custom-key field and focus it. */
  private revealCustomKey(): void {
    const parts = this.dialog?.parts;
    if (!parts?.customField || !parts.customInput || !parts.customButton) return;
    parts.customField.hidden = false;
    parts.customButton.setAttribute("aria-expanded", "true");
    focusElement(parts.customInput);
  }

  /** The owner typed a custom key: check it, then choose it or say what is wrong. */
  private onCustomKey(namespaces: readonly NamespaceRegistration[]): void {
    const dialog = this.dialog;
    const input = dialog?.parts.customInput;
    if (!dialog || !input || !this.options) return;
    const labels = this.options.labels;
    const key = input.value.trim();
    const problem = key === "" ? "format" : customKeyProblem(key, namespaces);
    dialog.selected = problem ? undefined : key;
    markSelected(dialog, dialog.selected);
    chooseProperty(dialog.parts, labels, dialog.selected);
    input.setAttribute("aria-invalid", String(problem !== undefined && key !== ""));
    dialog.parts.error.textContent = problem && key !== "" ? labels.metadataCustomKeyInvalid : "";
  }

  /** Check the value, then set the statement in one call. */
  private async submit(): Promise<void> {
    const { options, file, dialog } = this;
    if (!options || !file || !dialog || dialog.busy || !dialog.selected) return;
    const { valueInput, error } = dialog.parts;
    if (valueProblem(valueInput.value)) {
      valueInput.setAttribute("aria-invalid", "true");
      error.textContent = options.labels.metadataValueInvalid;
      return;
    }
    valueInput.removeAttribute("aria-invalid");
    error.textContent = "";
    const curie = dialog.selected;
    this.setBusy(true);
    try {
      const commit = await options.source.setStatement(file, curie, valueInput.value.trim());
      if (this.dialog === dialog) this.dropDialog();
      const focus = dialog.mode.kind === "edit" ? editButtonOf(curie) : addButtonOf;
      const words = formatLabel(options.labels.metadataSet, {
        property: curie,
        version: commit.version,
      });
      this.afterCommit(METADATA_OPERATIONS.set, file, commit, { words, focus });
    } catch (failure) {
      console.warn("FileSquid could not set a statement", errorKind(failure));
      if (this.dialog !== dialog) return;
      this.setBusy(false);
      error.textContent = refusalText(failure, options.labels);
    }
  }

  /** Disable or enable the dialog's controls while the write runs. */
  private setBusy(busy: boolean): void {
    const dialog = this.dialog;
    if (!dialog) return;
    dialog.busy = busy;
    const { close, cancel, confirm, valueInput, search, customInput } = dialog.parts;
    for (const control of [close, cancel, confirm, valueInput, search, customInput]) {
      if (control) control.disabled = busy;
    }
  }

  /** Remove the dialog, even while its write runs, and cancel a lookup in flight. */
  private dropDialog(): void {
    this.dialog?.lookup?.abort();
    this.dialog?.parts.scrim.remove();
    this.dialog = undefined;
  }

  /** Close the dialog without sending anything and return focus to its opener. */
  private closeDialog(): void {
    const dialog = this.dialog;
    if (!dialog || dialog.busy) return;
    this.dropDialog();
    focusElement(dialog.opener);
  }
}
