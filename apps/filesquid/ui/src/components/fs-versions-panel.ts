/**
 * `<fs-versions-panel>`: the Versions view (UI-F2.3, UI-S-009, ADR 028 Decision 5), opened from
 * the file panel's history button (UI-S-031, mockups `resources/ui-mockups/v3/26-panel-versions.svg`
 * and `27-version-restore.svg`). It lists every version that changed the opened file, newest
 * first: the mono version chip, the label or "No label", the Current chip on the current
 * version, and "{relative time} · {size}"; under the Back row and the version count, the
 * eyebrow "HISTORY {size} · NEWEST FIRST"; and the note that FileSquid keeps every version. A
 * row has a menu ({@link versionActions}): **Restore as new version** (not on the current
 * version) opens the restore dialog and publishes one new version through `FileSource.restore`
 * (API-F2.5, `POST /api/v1/files/restorations`); **Download this version** (UI-F2.11) saves that
 * version's file under its display name through {@link DownloadSource}; **Edit label**
 * (UI-F2.11) opens a dialog and sets or clears the label through `FileSource.setVersionLabel`
 * (API-F2.5, `PUT /api/v1/files/labels`). Restore and Edit label are commands whose expected
 * revision is the object's head as last read (API-F2.13); a head another writer moved
 * meanwhile is refused as `conflict` and changes nothing. No delete, expiry or thinning
 * control exists.
 *
 * Reads: `GET /api/v1/files/history`, `GET /api/v1/files/versions` (the head, for the next
 * version) and `GET /api/v1/files/history-size`, through {@link HistorySource}.
 *
 * **Events** (bubbling and composed): `fs-versions-close` from Back; `fs-files-change`
 * (`detail.operation` `restore` or `label`, `detail.commit`) after a restore or a label edit
 * commits.
 *
 * Accessibility (UI-S-016): the shared stylesheet comes first; the rows are a named list; the
 * menu button has `aria-haspopup="menu"` and `aria-expanded`, the menu moves with the arrow
 * keys and Escape closes it and returns focus; the dialog is modal, keeps Tab inside, and
 * Escape cancels it and returns focus to the menu button; a restore, a label edit and a
 * download are announced in a polite status region; a failed download shows a visible alert;
 * focus is never moved by a load.
 */
import { formatLabel, type LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { focusElement } from "../a11y/focus";
import { icon } from "./icons";
import { PROPERTY_TABLE_STYLE, errorState, failureWords } from "./property-table";
import {
  RESTORE_DIALOG_STYLE,
  buildRestoreDialog,
  labelProblem,
  type DialogParts,
} from "./version-restore-dialog";
import { buildLabelDialog, labelChange } from "./version-label-dialog";
import { saveFileToDevice } from "./save-file";
import { failureText } from "../files/ops";
import { formatSize, type FileCommit, type FileRef } from "../files/model";
import type { FileSource } from "../files/source";
import type { HistorySource } from "../files/history-source";
import type { DownloadSource } from "../files/download-source";
import { ApiError } from "../api/client";
import {
  laterVersions,
  nextVersion,
  relativeTime,
  versionCountText,
  versionActions,
  versionRows,
  versionTotal,
  type VersionAction,
  type VersionRow,
} from "../files/versions";

/** The reads, the restore, the label edit and the download the view needs. */
export type VersionsSource = Pick<FileSource, "restore" | "setVersionLabel"> &
  HistorySource &
  DownloadSource;

/** What the view needs. */
export interface VersionsPanelOptions {
  /** The active label table. */
  labels: LabelTable;
  /** Reads the history and restores. */
  source: VersionsSource;
  /** Whether the browser is online; `navigator.onLine` when absent. */
  online?: () => boolean;
  /** The present, for relative times; the clock when absent. */
  now?: () => Date;
  /** Saves a downloaded file; the browser's own download ({@link saveFileToDevice}) when absent. */
  save?: (blob: Blob, name: string) => void;
}

/** Name of the event Back dispatches. */
export const VERSIONS_CLOSE_EVENT = "fs-versions-close";

/** What one load read. */
interface Loaded {
  readonly rows: VersionRow[];
  /** Every version that changed the file, though `rows` holds at most `MAX_VERSION_ROWS`. */
  readonly total: number;
  readonly head: string;
  readonly bytes?: number;
}

const STYLE = `
  :host { position: relative; display: flex; flex-direction: column; min-block-size: 100%; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); color: var(--fs-on-surface); }
  :host([hidden]) { display: none; }
  .back-row { display: flex; align-items: center; justify-content: space-between; gap: var(--fs-space-2); box-sizing: border-box; min-block-size: 2.5625rem; padding: var(--fs-space-1) var(--fs-space-3) 0; border-block-end: 1px solid var(--fs-outline); }
  .back-row .back { min-block-size: 2rem; padding: 0 var(--fs-space-1); font-size: var(--fs-font-size-small); font-weight: 500; color: var(--fs-on-surface-variant); }
  .back-row .back .icon { color: var(--fs-icon); }
  :host(:dir(rtl)) .back .icon { transform: scaleX(-1); }
  .back-row .count { margin: 0 var(--fs-space-1) 0 0; padding: 0; font-size: var(--fs-font-size-small); line-height: normal; color: var(--fs-on-surface-muted); }
  .content { flex: 1 1 auto; padding: var(--fs-space-3) var(--fs-space-2) var(--fs-space-3); }
  .content > .eyebrow { margin: 0 var(--fs-space-2) var(--fs-space-2); padding-inline: var(--fs-space-1); line-height: 1.4; }
  .eyebrow.busy { font-weight: 500; text-transform: none; }
  ul.versions { list-style: none; display: grid; gap: var(--fs-space-1); margin: 0; padding: 0; }
  ul.versions:focus { outline: none; }
  .version { position: relative; display: grid; grid-template-columns: 1.25rem minmax(0, 1fr); align-content: start; column-gap: var(--fs-space-2); row-gap: var(--fs-space-1); box-sizing: border-box; min-block-size: 3.625rem; padding: var(--fs-space-1) var(--fs-space-2) var(--fs-space-1) var(--fs-space-1); border-radius: var(--fs-radius-input); }
  .version:hover, .version:focus-within { background: var(--fs-surface-raised); }
  .version:not(:last-child)::after { content: ""; position: absolute; inset-inline-start: 13px; inset-block: 22px -10px; border-inline-start: 1.5px solid var(--fs-outline); }
  .dot { grid-row: 1; justify-self: center; align-self: center; box-sizing: border-box; inline-size: 0.625rem; block-size: 0.625rem; border: 1.5px solid var(--fs-outline-strong); border-radius: var(--fs-radius-pill); background: var(--fs-surface-card); }
  .version[data-current] .dot { border-color: var(--fs-brand); background: var(--fs-brand); }
  .line { grid-column: 2; display: flex; align-items: center; gap: var(--fs-space-2); min-width: 0; min-block-size: 1.5rem; }
  .version .mono-chip { flex: 0 0 auto; padding: var(--fs-space-1) var(--fs-space-2); line-height: 1.2; }
  .label { flex: 1 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-font-size-ui); font-weight: 600; color: var(--fs-on-surface); }
  .no-label { flex: 1 1 auto; min-width: 0; font-size: var(--fs-font-size-small); font-style: italic; color: var(--fs-on-surface-muted); }
  .version:hover .no-label, .version:focus-within .no-label { color: var(--fs-on-surface-variant); }
  .current { flex: 0 0 auto; padding: var(--fs-space-1) var(--fs-space-2); border-radius: var(--fs-radius-chip); line-height: 1.2; }
  .more { flex: 0 0 auto; display: inline-flex; align-items: center; justify-content: center; inline-size: 1.5rem; block-size: 1.5rem; padding: 0; border: 0; border-radius: var(--fs-radius-item); background: none; color: var(--fs-icon-muted); cursor: pointer; }
  .more:hover, .more:focus-visible, .more[aria-expanded="true"], .version:hover .more { color: var(--fs-icon); }
  .meta { grid-column: 2; margin: 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .menu { position: absolute; z-index: 2; inset-inline-end: 12px; inset-block-start: 30px; display: grid; box-sizing: border-box; inline-size: 14.5rem; padding: var(--fs-space-1); border: 1px solid var(--fs-outline-strong); border-radius: var(--fs-radius); background: var(--fs-surface-overlay); }
  .menu-item { display: flex; align-items: center; gap: var(--fs-space-2); box-sizing: border-box; min-block-size: 2.125rem; padding: 0 var(--fs-space-2); border: 0; border-radius: var(--fs-radius-item); background: none; color: var(--fs-on-surface); font-family: inherit; font-size: var(--fs-font-size-small); text-align: start; cursor: pointer; }
  .menu-item:hover, .menu-item:focus-visible { background: var(--fs-surface-raised); }
  .note { display: grid; grid-template-columns: auto minmax(0, 1fr); column-gap: var(--fs-space-2); box-sizing: border-box; inline-size: min(18.5rem, calc(100% - 2 * var(--fs-space-2))); margin: 0 auto var(--fs-space-4); padding: var(--fs-space-3) var(--fs-space-3) var(--fs-space-2); border-radius: var(--fs-radius); background: var(--fs-surface-raised); }
  .note .icon { grid-row: 1 / span 2; color: var(--fs-icon); }
  .note p { margin: 0; font-size: var(--fs-font-size-small); line-height: 1.23; }
  .note-title { font-weight: 600; color: var(--fs-on-surface); }
  .note-detail { color: var(--fs-on-surface-variant); }
  .notices:empty { display: none; }
  .notice { margin: var(--fs-space-2) var(--fs-space-3) 0; padding: var(--fs-space-2); border: 1.5px solid var(--fs-error); border-radius: var(--fs-radius-item); background: var(--fs-surface-card); color: var(--fs-error); font-size: var(--fs-font-size-small); overflow-wrap: anywhere; }
  .more[aria-busy="true"] { color: var(--fs-icon-disabled); cursor: progress; }
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

/** Custom element that lists a file's versions and restores one as a new version. */
export class FsVersionsPanel extends HTMLElement {
  private options?: VersionsPanelOptions;
  private file: FileRef | null = null;
  private loaded?: Loaded;
  private generation = 0;
  private abort?: AbortController;
  private content?: HTMLElement;
  private count?: HTMLElement;
  private announcer?: HTMLElement;
  private menu?: { element: HTMLElement; button: HTMLButtonElement };
  private notices?: HTMLElement;
  private downloadAbort?: AbortController;
  private dialog?: { parts: DialogParts; opener: HTMLButtonElement; busy: boolean };
  private readonly onOutside = (event: Event) => this.closeMenuOutside(event);

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the labels and source, render, and load the file already given, if any.
   *
   * @param options Labels, the source, and optional online check and clock.
   */
  configure(options: VersionsPanelOptions): void {
    this.options = options;
    this.build(options.labels);
    void this.load();
  }

  /** The file whose versions are listed, or null. */
  get fileRef(): FileRef | null {
    return this.file;
  }

  /**
   * List another file's versions (null clears the view). Any other reference, a new version
   * of the same file included, drops an open dialog; the rows shown are kept only for a new
   * version of the same file, until its answer replaces them, so the list does not flash.
   */
  set fileRef(file: FileRef | null) {
    const samePath =
      file !== null &&
      this.file !== null &&
      file.library_id === this.file.library_id &&
      file.object_id === this.file.object_id &&
      file.path === this.file.path;
    if (file !== this.file) this.dropDialog();
    if (!samePath) this.loaded = undefined;
    this.file = file;
    this.closeMenu(false);
    this.stopDownload();
    void this.load();
  }

  /** Cancel a read in flight and close the menu and the dialog when the element leaves. */
  disconnectedCallback(): void {
    this.abort?.abort();
    this.stopDownload();
    this.closeMenu(false);
    this.dropDialog();
  }

  /** Build the stylesheet, the Back row, the content area, the note and the live region. */
  private build(labels: LabelTable): void {
    const style = document.createElement("style");
    style.textContent =
      A11Y_STYLE + CONTROL_STYLE + STYLE + PROPERTY_TABLE_STYLE + RESTORE_DIALOG_STYLE;
    const bar = node("div", "back-row");
    const back = node("button", "button plain back");
    back.type = "button";
    back.append(icon("chevron-back"), document.createTextNode(labels.versionsBack));
    back.addEventListener("click", () => this.emit(VERSIONS_CLOSE_EVENT, undefined));
    this.count = node("p", "count");
    bar.append(back, this.count);
    this.notices = node("div", "notices");
    this.content = node("div", "content");
    this.announcer = node("p", "announce");
    this.announcer.setAttribute("role", "status");
    this.shadowRoot?.replaceChildren(
      style,
      bar,
      this.notices,
      this.content,
      note(labels),
      this.announcer,
    );
  }

  /** Read the history, the head and the history size, then list them; ignore stale answers. */
  private async load(): Promise<void> {
    const { options, file } = this;
    if (!options || !this.content) return;
    this.abort?.abort();
    const generation = ++this.generation;
    if (!file) return this.showIdle();
    const abort = new AbortController();
    this.abort = abort;
    if (!this.loaded) this.showLoading(options.labels);
    try {
      const loaded = await this.read(options.source, file, abort.signal);
      if (generation !== this.generation) return;
      this.loaded = loaded;
      this.showRows(options.labels, file, loaded);
    } catch (error) {
      if (generation !== this.generation || abort.signal.aborted) return;
      console.warn("FileSquid could not read the versions", errorName(error));
      this.showError(options, error);
    }
  }

  /** The three reads of one load; a history size that cannot be read is left out. */
  private async read(source: VersionsSource, file: FileRef, signal: AbortSignal): Promise<Loaded> {
    const size = source.historySize(file, { signal }).then(
      (answer) => answer.bytes,
      (error: unknown) => {
        console.warn("FileSquid could not read the history size", errorName(error));
        return undefined;
      },
    );
    const [history, versions, bytes] = await Promise.all([
      source.history(file, { signal }),
      source.versions(file, { signal }),
      size,
    ]);
    return {
      rows: versionRows(history),
      total: versionTotal(history),
      head: versions.head,
      bytes,
    };
  }

  /** No file: nothing listed. */
  private showIdle(): void {
    this.loaded = undefined;
    this.content?.replaceChildren();
    if (this.count) this.count.textContent = "";
    this.dataset.state = "idle";
  }

  /** The busy line while the first answer is awaited. */
  private showLoading(labels: LabelTable): void {
    this.content?.replaceChildren(node("p", "eyebrow busy", labels.versionsLoading));
    if (this.count) this.count.textContent = "";
    this.dataset.state = "loading";
  }

  /** The error state with Try again. */
  private showError(options: VersionsPanelOptions, error: unknown): void {
    const online = options.online ? options.online() : navigator.onLine;
    const words = failureWords(error, options.labels, online);
    const state = errorState(options.labels.versionsError, words, options.labels, () => {
      this.loaded = undefined;
      void this.load();
    });
    this.content?.replaceChildren(state);
    this.dataset.state = "error";
  }

  /** The eyebrow, the count and the rows. */
  private showRows(labels: LabelTable, file: FileRef, loaded: Loaded): void {
    const locale = document.documentElement.lang || "en";
    const eyebrow =
      loaded.bytes === undefined
        ? labels.versionsEyebrowNoSize
        : formatLabel(labels.versionsEyebrow, { size: formatSize(loaded.bytes, labels, locale) });
    const list = node("ul", "versions");
    list.tabIndex = -1;
    list.setAttribute("aria-label", formatLabel(labels.versionsList, { name: file.display_name }));
    const now = this.options?.now ? this.options.now() : new Date();
    list.append(...loaded.rows.map((row) => this.rowElement(labels, row, now, locale)));
    this.content?.replaceChildren(node("p", "eyebrow", eyebrow), list);
    if (this.count) this.count.textContent = versionCountText(loaded.total, labels, locale);
    this.dataset.state = "ready";
  }

  /** One version row. */
  private rowElement(labels: LabelTable, row: VersionRow, now: Date, locale: string): HTMLElement {
    const item = node("li", "version");
    item.dataset.version = row.version;
    if (row.current) item.dataset.current = "";
    const dot = node("span", "dot");
    dot.setAttribute("aria-hidden", "true");
    const line = node("div", "line");
    line.append(node("span", "mono-chip", row.version));
    line.append(
      row.label === null
        ? node("span", "no-label", labels.versionsNoLabel)
        : node("span", "label", row.label),
    );
    if (row.current) line.append(node("span", "chip ok current", labels.versionsCurrent));
    line.append(this.menuButton(labels, row));
    item.append(dot, line, metaLine(labels, row, now, locale));
    return item;
  }

  /** The menu button of a row. */
  private menuButton(labels: LabelTable, row: VersionRow): HTMLButtonElement {
    const button = node("button", "more");
    button.type = "button";
    button.setAttribute(
      "aria-label",
      formatLabel(labels.versionsActions, { version: row.version }),
    );
    button.setAttribute("aria-haspopup", "menu");
    button.setAttribute("aria-expanded", "false");
    button.append(icon("more-vertical"));
    button.addEventListener("click", () => this.toggleMenu(button, row));
    return button;
  }

  /** Open the row's menu, or close it when it is open. */
  private toggleMenu(button: HTMLButtonElement, row: VersionRow): void {
    const open = this.menu?.button === button;
    this.closeMenu(false);
    if (!open) this.openMenu(button, row);
  }

  /** Open the menu under a row's button and focus its first item. */
  private openMenu(button: HTMLButtonElement, row: VersionRow): void {
    const labels = (this.options as VersionsPanelOptions).labels;
    const element = node("div", "menu");
    element.setAttribute("role", "menu");
    element.setAttribute("aria-label", button.getAttribute("aria-label") ?? "");
    const items = versionActions(row).map((action) => this.menuItem(labels, action, button, row));
    element.append(...items);
    element.addEventListener("keydown", (event) => this.onMenuKey(event, element));
    button.closest("li")?.append(element);
    button.setAttribute("aria-expanded", "true");
    this.menu = { element, button };
    document.addEventListener("pointerdown", this.onOutside, true);
    focusElement(items[0] as HTMLElement);
  }

  /** One item of a row's menu: its icon and words, and what choosing it does. */
  private menuItem(
    labels: LabelTable,
    action: VersionAction,
    button: HTMLButtonElement,
    row: VersionRow,
  ): HTMLButtonElement {
    const words = {
      restore: { icon: "history", text: labels.versionsRestoreAction },
      download: { icon: "download", text: labels.versionsDownloadAction },
      "edit-label": { icon: "edit", text: labels.versionsEditLabelAction },
    } as const;
    const item = node("button", "menu-item");
    item.type = "button";
    item.setAttribute("role", "menuitem");
    item.tabIndex = -1;
    item.dataset.action = action;
    item.append(icon(words[action].icon), document.createTextNode(words[action].text));
    item.addEventListener("click", () => this.chooseAction(action, button, row));
    return item;
  }

  /** Close the menu, then do what the chosen item says. */
  private chooseAction(action: VersionAction, button: HTMLButtonElement, row: VersionRow): void {
    this.closeMenu(false);
    if (action === "restore") this.chooseRestore(button, row);
    else if (action === "edit-label") this.chooseEditLabel(button, row);
    else void this.runDownload(button, row);
  }

  /** Close the menu; when `focus`, return focus to its button. */
  private closeMenu(focus: boolean): void {
    const menu = this.menu;
    if (!menu) return;
    this.menu = undefined;
    document.removeEventListener("pointerdown", this.onOutside, true);
    menu.element.remove();
    menu.button.setAttribute("aria-expanded", "false");
    if (focus) focusElement(menu.button);
  }

  /** A pointer press outside the open menu and its button closes the menu. */
  private closeMenuOutside(event: Event): void {
    const path = event.composedPath();
    const menu = this.menu;
    if (menu && !path.includes(menu.element) && !path.includes(menu.button)) this.closeMenu(false);
  }

  /** Keys in the menu: Escape closes it, the arrows move between items, Tab leaves it. */
  private onMenuKey(event: KeyboardEvent, element: HTMLElement): void {
    const items = [...element.querySelectorAll<HTMLElement>('[role="menuitem"]')];
    const at = items.findIndex((item) => item === this.shadowRoot?.activeElement);
    if (event.key === "Escape") {
      event.preventDefault();
      this.closeMenu(true);
    } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const step = event.key === "ArrowDown" ? 1 : -1;
      items[(at + step + items.length) % items.length]?.focus();
    } else if (event.key === "Tab") {
      this.closeMenu(false);
    }
  }

  /** Restore as new version was chosen: open the restore dialog. */
  private chooseRestore(button: HTMLButtonElement, row: VersionRow): void {
    const { options, file, loaded } = this;
    if (!options || !file || !loaded) return;
    const parts = buildRestoreDialog({
      labels: options.labels,
      file,
      row,
      next: nextVersion(loaded.head) ?? loaded.head,
      later: laterVersions(loaded.rows, row.version),
      locale: document.documentElement.lang || "en",
    });
    this.openDialog(parts, button, () => this.submitRestore(row));
  }

  /** Edit label was chosen: open the label dialog holding the version's label. */
  private chooseEditLabel(button: HTMLButtonElement, row: VersionRow): void {
    const { options, file } = this;
    if (!options || !file) return;
    const parts = buildLabelDialog({
      labels: options.labels,
      file,
      version: row.version,
      current: row.label,
    });
    this.openDialog(parts, button, () => this.submitLabel(row));
  }

  /** Show a built dialog, wire its buttons and Escape, and focus its field. */
  private openDialog(
    parts: DialogParts,
    opener: HTMLButtonElement,
    submit: () => Promise<void>,
  ): void {
    this.dialog = { parts, opener, busy: false };
    parts.close.addEventListener("click", () => this.closeDialog());
    parts.cancel.addEventListener("click", () => this.closeDialog());
    parts.dialog.addEventListener("submit", (event) => {
      event.preventDefault();
      void submit();
    });
    parts.submit.addEventListener("click", (event) => {
      event.preventDefault();
      void submit();
    });
    parts.dialog.addEventListener("keydown", (event) => this.onDialogKey(event, parts.dialog));
    this.shadowRoot?.append(parts.scrim);
    focusElement(parts.input);
  }

  /** Escape cancels; Tab and Shift+Tab stay inside the dialog. */
  private onDialogKey(event: KeyboardEvent, dialog: HTMLElement): void {
    if (event.key === "Escape") {
      event.preventDefault();
      this.closeDialog();
      return;
    }
    if (event.key !== "Tab") return;
    const stops = [...dialog.querySelectorAll<HTMLButtonElement | HTMLInputElement>("*")].filter(
      (el) => (el instanceof HTMLButtonElement || el instanceof HTMLInputElement) && !el.disabled,
    );
    const first = stops[0];
    const last = stops[stops.length - 1];
    const active = this.shadowRoot?.activeElement;
    if (!event.shiftKey && active === last && first) {
      event.preventDefault();
      first.focus();
    } else if (event.shiftKey && active === first && last) {
      event.preventDefault();
      last.focus();
    }
  }

  /** Remove the dialog of a file that is no longer shown, even while its restore runs. */
  private dropDialog(): void {
    this.dialog?.parts.scrim.remove();
    this.dialog = undefined;
  }

  /** Close the dialog without sending anything and return focus to the menu button. */
  private closeDialog(): void {
    const dialog = this.dialog;
    if (!dialog || dialog.busy) return;
    this.dialog = undefined;
    dialog.parts.scrim.remove();
    focusElement(dialog.opener);
  }

  /** Check the label, then restore the version as a new version in one call. */
  private async submitRestore(row: VersionRow): Promise<void> {
    const { options, file, dialog, loaded } = this;
    if (!options || !file || !dialog || !loaded || dialog.busy) return;
    const { input, error } = dialog.parts;
    if (labelProblem(input.value)) {
      input.setAttribute("aria-invalid", "true");
      error.textContent = options.labels.versionsRestoreLabelInvalid;
      return;
    }
    input.removeAttribute("aria-invalid");
    error.textContent = "";
    const label = input.value.trim();
    this.setBusy(true);
    try {
      const object = { library_id: file.library_id, object_id: file.object_id };
      const commit = await options.source.restore(object, row.version, [file.path], {
        revision: loaded.head,
        label: label === "" ? undefined : label,
      });
      const words = formatLabel(options.labels.fileRestored, {
        name: file.display_name,
        version: commit.version,
      });
      this.afterCommit("restore", words, commit, dialog);
    } catch (failure) {
      console.warn("FileSquid could not restore a version", errorName(failure));
      if (this.dialog !== dialog) return;
      this.setBusy(false);
      error.textContent = failureText(failure, options.labels);
    }
  }

  /** Disable or enable the dialog's controls while the restore runs. */
  private setBusy(busy: boolean): void {
    const dialog = this.dialog;
    if (!dialog) return;
    dialog.busy = busy;
    const { close, cancel, submit, input } = dialog.parts;
    for (const control of [close, cancel, submit, input]) control.disabled = busy;
  }

  /**
   * A restore or a label edit committed: tell the page, so its listing shows the new version;
   * then, unless the view moved on to another file or version meanwhile (which dropped the
   * dialog), close the dialog, announce `words` and list again.
   */
  private afterCommit(
    operation: "restore" | "label",
    words: string,
    commit: FileCommit,
    dialog: NonNullable<FsVersionsPanel["dialog"]>,
  ): void {
    this.emit("fs-files-change", { operation, commit });
    if (this.dialog !== dialog) return;
    this.dialog.parts.scrim.remove();
    this.dialog = undefined;
    if (this.announcer) this.announcer.textContent = words;
    void this.load().then(() => {
      const list = this.shadowRoot?.querySelector<HTMLElement>("ul.versions");
      if (list) focusElement(list);
    });
  }

  /** Check the typed label, then set or clear the version's label in one call. */
  private async submitLabel(row: VersionRow): Promise<void> {
    const { options, file, dialog, loaded } = this;
    if (!options || !file || !dialog || !loaded || dialog.busy) return;
    const { input, error } = dialog.parts;
    const change = labelChange(input.value, row.label);
    if (change.kind === "invalid") {
      input.setAttribute("aria-invalid", "true");
      error.textContent = options.labels.versionsRestoreLabelInvalid;
      return;
    }
    input.removeAttribute("aria-invalid");
    error.textContent = "";
    if (change.kind === "unchanged") return this.closeDialog();
    this.setBusy(true);
    try {
      const object = { library_id: file.library_id, object_id: file.object_id };
      const commit = await options.source.setVersionLabel(object, row.version, change.label, {
        revision: loaded.head,
      });
      const key = change.label === null ? "versionsLabelCleared" : "versionsLabelSaved";
      const words = formatLabel(options.labels[key], { version: row.version });
      this.afterCommit("label", words, commit, dialog);
    } catch (failure) {
      console.warn("FileSquid could not set a version label", errorName(failure));
      if (this.dialog !== dialog) return;
      this.setBusy(false);
      error.textContent = failureText(failure, options.labels);
    }
  }

  /** Read one version's file and hand it to the browser; a failure shows an alert. */
  private async runDownload(button: HTMLButtonElement, row: VersionRow): Promise<void> {
    const { options, file } = this;
    if (!options || !file) return;
    this.stopDownload();
    this.notices?.replaceChildren();
    const abort = new AbortController();
    this.downloadAbort = abort;
    const words = { version: row.version, name: file.display_name };
    if (this.announcer) {
      this.announcer.textContent = formatLabel(options.labels.versionsDownloading, words);
    }
    button.setAttribute("aria-busy", "true");
    try {
      const blob = await options.source.readVersionFile(
        { ...file, version: row.version },
        { signal: abort.signal },
      );
      if (abort.signal.aborted) return;
      (options.save ?? saveFileToDevice)(blob, file.display_name);
    } catch (failure) {
      if (abort.signal.aborted) return;
      console.warn("FileSquid could not download a version", errorName(failure));
      this.showNotice(downloadFailure(options.labels, row.version, failure));
    } finally {
      button.removeAttribute("aria-busy");
    }
  }

  /** Cancel a download in flight. */
  private stopDownload(): void {
    this.downloadAbort?.abort();
    this.downloadAbort = undefined;
    this.notices?.replaceChildren();
  }

  /** Show a visible alert above the rows. */
  private showNotice(text: string): void {
    const alert = node("p", "notice", text);
    alert.setAttribute("role", "alert");
    this.notices?.replaceChildren(alert);
  }

  /** Dispatch a bubbling, composed event. */
  private emit(name: string, detail: unknown): void {
    this.dispatchEvent(new CustomEvent(name, { detail, bubbles: true, composed: true }));
  }
}

/** The note at the foot of the view: FileSquid keeps every version. */
function note(labels: LabelTable): HTMLElement {
  const box = node("div", "note");
  box.append(
    icon("info"),
    node("p", "note-title", labels.versionsKeepEvery),
    node("p", "note-detail", labels.versionsKeepDetail),
  );
  return box;
}

/** "{relative time} · {size}", with the time as a `<time>` element; "Removed" for a removal. */
function metaLine(labels: LabelTable, row: VersionRow, now: Date, locale: string): HTMLElement {
  const meta = node("p", "meta");
  const time = document.createElement("time");
  time.dateTime = row.created;
  time.textContent = relativeTime(row.created, now, locale);
  const size = row.size === null ? labels.versionsRemoved : formatSize(row.size, labels, locale);
  const [before = "", after = ""] = labels.versionsMeta.split("{time}");
  meta.append(
    document.createTextNode(before.replace("{size}", size)),
    time,
    document.createTextNode(after.replace("{size}", size)),
  );
  return meta;
}

/** The words of a failed download: it failed, and for a version too large to read, why. */
function downloadFailure(labels: LabelTable, version: string, error: unknown): string {
  const failed = formatLabel(labels.versionsDownloadFailed, { version });
  const tooLarge = error instanceof ApiError && error.code === "payload_too_large";
  return tooLarge ? `${failed} ${labels.versionsDownloadTooLarge}` : failed;
}

/** The kind of an error, without its message. */
function errorName(error: unknown): string {
  return error instanceof Error ? error.name : typeof error;
}
