/**
 * `<fs-log-entry>`: the detail panel of one Logs entry (UI-F2.16; UI-S-021, UI-S-032, mockup
 * `resources/ui-mockups/v3/38-logs-view.svg`), shown in the shell's `panel` slot, which widens to
 * 360 px for it (`data-panel-wide`). It reads like the file panel's tabs: its rows are drawn by
 * the shared {@link PropertyTable} (the same rows, copy buttons and states), without the filter,
 * under "Entry detail", the operation in mono 16 px 600 and the outcome chip.
 *
 * The rows are what the journal carries (`detailRows`): the times, the level and outcome, the
 * actor and software agent, the library, the object, the file's display name where the route
 * resolved it, the operation ID and the duration. Below them are **View provenance**, offered
 * only for an entry of a committed operation (its activities share the operation ID), or the
 * journal-only note for a failed or refused entry or a download, **Show file**, **Copy as JSON**
 * (the entry as stored: no display name) and the note that logs hold opaque IDs only.
 *
 * It shows the entry the list gave at once and reads the entry again (`GET .../entries/{id}`,
 * with the file's display name) for what a live event did not carry; a failed read keeps what the
 * list gave and says so. Every value is set with `textContent`. Accessibility (UI-S-016): the
 * shared stylesheet comes first, the panel is a named region, links are native anchors, the copy
 * is announced in a polite status region, and selecting an entry never moves focus.
 */
import type { JournalEntryView } from "../api/client";
import { openFileHash } from "../files/model";
import {
  detailRows,
  outcomeKind,
  rowView,
  viewProvenanceTarget,
  type LogsLibrary,
} from "../logs/model";
import type { LogsSource } from "../logs/source";
import type { LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { icon } from "./icons";
import { PANEL_WIDE } from "./panel-slot";
import { PROPERTY_TABLE_STYLE, PropertyTable } from "./property-table";

/** Name of the event the close button dispatches (bubbling, composed). */
export const LOG_ENTRY_CLOSE_EVENT = "fs-log-entry-close";

/** What the panel needs. */
export interface LogEntryOptions {
  /** The active label table. */
  labels: LabelTable;
  /** Reads one entry again, with its display name. */
  source: Pick<LogsSource, "entry">;
  /** The IANA zone times are shown in; the viewer's when absent. */
  zone?: string;
  /** Whether the browser is online; `navigator.onLine` when absent. */
  online?: () => boolean;
}

const ENTRY_STYLE = `
  :host { display: flex; flex-direction: column; min-block-size: 100%; box-sizing: border-box; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); color: var(--fs-on-surface); }
  :host([hidden]) { display: none; }
  .head { display: grid; gap: var(--fs-space-2); padding: var(--fs-space-3) var(--fs-space-3) var(--fs-space-2) 1.25rem; border-block-end: 1px solid var(--fs-outline); }
  .head-top { display: flex; align-items: center; justify-content: space-between; gap: var(--fs-space-2); }
  .head-top h2 { margin: 0; font-size: var(--fs-font-size-small); font-weight: 500; letter-spacing: 0.1em; color: var(--fs-on-surface-muted); }
  .close { display: inline-flex; align-items: center; justify-content: center; inline-size: 1.5rem; block-size: 1.5rem; padding: 0; border: 0; border-radius: var(--fs-radius-item); background: none; color: var(--fs-icon-muted); cursor: pointer; }
  .close:hover, .close:focus-visible { color: var(--fs-icon); }
  .title { display: flex; align-items: center; justify-content: space-between; gap: var(--fs-space-2); }
  .operation { margin: 0; min-width: 0; font-family: var(--fs-font-mono); font-size: var(--fs-font-size-lead); font-weight: 600; color: var(--fs-on-surface-heading); overflow-wrap: anywhere; }
  .title .chip { flex: 0 0 auto; }
  .rows { padding-block-start: var(--fs-space-1); }
  .rows .row { min-block-size: 2.625rem; }
  .rows .row::after { display: none; }
  .rows .row dd .value.mono { font-size: 0.75rem; }
  .rows .row[data-key="operation-id"] dd .value { font-size: var(--fs-font-size-eyebrow); }
  .actions { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-3); padding: var(--fs-space-3) 1.25rem 0; }
  .actions a { display: inline-block; padding-block: var(--fs-space-1); color: var(--fs-on-primary-container); font-size: var(--fs-font-size-ui); font-weight: 600; text-decoration: none; }
  .actions a:hover { text-decoration: underline; }
  .copy-json { margin: var(--fs-space-2) 1.25rem 0; min-block-size: 1.75rem; align-self: start; padding: 0 var(--fs-space-3); }
  .journal-only { margin: var(--fs-space-3) 1.25rem 0; color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  .journal-only[hidden], .actions[hidden] { display: none; }
  .note-box.raised { display: flex; align-items: start; gap: var(--fs-space-2); margin: var(--fs-space-3) 1.25rem var(--fs-space-3); padding: var(--fs-space-3); border-radius: var(--fs-radius); font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .note-box p { margin: 0; }
  .read-failed { margin: var(--fs-space-2) 1.25rem 0; color: var(--fs-error); font-size: var(--fs-font-size-small); }
  .read-failed[hidden] { display: none; }
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

/** The entry as the journal stores it: the display name is a lookup, never stored. */
function storedJson(entry: JournalEntryView): string {
  const { display_name: _name, ...stored } = entry;
  void _name;
  return JSON.stringify(stored, null, 2);
}

/** Custom element that shows one journal entry in detail. */
export class FsLogEntry extends HTMLElement {
  private options?: LogEntryOptions;
  private table?: PropertyTable;
  private shown: JournalEntryView | null = null;
  private known: readonly LogsLibrary[] = [];
  private generation = 0;
  private abort?: AbortController;

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the labels and the source and render.
   *
   * @param options See {@link LogEntryOptions}.
   */
  configure(options: LogEntryOptions): void {
    this.options = options;
    this.build(options);
    this.show();
  }

  /** The entry shown, or null. */
  get entry(): JournalEntryView | null {
    return this.shown;
  }

  /** Show another entry (null clears the panel) and read it again for its display name. */
  set entry(entry: JournalEntryView | null) {
    this.shown = entry;
    this.show();
    void this.refresh();
  }

  /** The libraries the host opened, for naming the entry's library and linking its file. */
  set libraries(libraries: readonly LogsLibrary[]) {
    this.known = libraries;
    this.show();
  }

  /** Cancel a read in flight when the element leaves the document. */
  disconnectedCallback(): void {
    this.abort?.abort();
  }

  /** Build the stylesheet, the header, the table and the actions. */
  private build(options: LogEntryOptions): void {
    const { labels } = options;
    const table = new PropertyTable({
      labels,
      filterable: false,
      filterPlaceholder: labels.logsDetailTitle,
      loadingText: labels.logsLoading,
      errorTitle: labels.logsEntryLoadFailed,
      countText: () => "",
      noMatchTitle: () => "",
      onRetry: () => void this.refresh(),
      online: options.online,
    });
    this.table = table;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + CONTROL_STYLE + PROPERTY_TABLE_STYLE + ENTRY_STYLE;
    const region = node("section", "detail");
    region.setAttribute("aria-label", labels.logsDetailTitle);
    const rows = node("div", "rows");
    rows.append(table.element);
    region.append(this.header(labels), rows, this.actions(labels), this.footerNotes(labels));
    this.shadowRoot?.replaceChildren(style, region);
  }

  /** The header: "Entry detail" with close, the operation and the outcome chip. */
  private header(labels: LabelTable): HTMLElement {
    const head = node("div", "head");
    const top = node("div", "head-top");
    top.append(node("h2", "", labels.logsDetailTitle));
    const close = node("button", "close");
    close.type = "button";
    close.setAttribute("aria-label", labels.logsDetailClose);
    close.append(icon("close", 16));
    close.addEventListener("click", () =>
      this.dispatchEvent(new CustomEvent(LOG_ENTRY_CLOSE_EVENT, { bubbles: true, composed: true })),
    );
    top.append(close);
    const title = node("div", "title");
    title.append(node("p", "operation"), node("span", "chip"));
    head.append(top, title);
    return head;
  }

  /** The links, the journal-only line and Copy as JSON. */
  private actions(labels: LabelTable): HTMLElement {
    const box = node("div", "actions-box");
    const links = node("div", "actions");
    const provenance = node("a", "view-provenance", labels.logsViewProvenance);
    const file = node("a", "show-file", labels.logsShowFile);
    links.append(provenance, file);
    const only = node("p", "journal-only", labels.logsJournalOnlyNote);
    const copy = node("button", "button secondary copy-json");
    copy.type = "button";
    copy.append(icon("copy", 16), document.createTextNode(labels.logsCopyJson));
    copy.addEventListener("click", () => void this.copyJson());
    const status = node("p", "announce");
    status.setAttribute("role", "status");
    box.append(links, only, copy, status);
    return box;
  }

  /** The failed-read line and the note that logs hold opaque IDs only. */
  private footerNotes(labels: LabelTable): HTMLElement {
    const box = node("div", "notes");
    const failed = node("p", "read-failed", labels.logsEntryLoadFailed);
    failed.setAttribute("role", "alert");
    failed.hidden = true;
    const note = node("div", "note-box raised");
    note.append(icon("info", 20), node("p", "", labels.logsOpaqueNote));
    box.append(failed, note);
    return box;
  }

  /** Draw the entry's header, rows and links from what is known now. */
  private show(): void {
    const root = this.shadowRoot;
    const options = this.options;
    const table = this.table;
    const entry = this.shown;
    if (!root || !options || !table) return;
    this.toggleAttribute("hidden", entry === null);
    this.setAttribute(PANEL_WIDE, "");
    this.dataset.state = entry === null ? "idle" : "ready";
    if (entry === null) return table.clear();
    const { labels } = options;
    const kind = rowView(entry, labels, options.zone).outcome;
    const operation = root.querySelector(".operation");
    if (operation) operation.textContent = entry.operation;
    const chip = root.querySelector<HTMLElement>(".title .chip");
    if (chip) {
      chip.textContent = kind.text;
      chip.className = `chip ${kind.tone === "ok" ? "ok" : kind.tone === "alert" ? "alert" : "muted"}`;
      chip.hidden = outcomeKind(entry) === "none";
    }
    const lang = document.documentElement.lang || "en";
    const libraryId = this.libraryOf(entry);
    table.rows(
      detailRows(entry, labels, { zone: options.zone, libraries: this.known, lang, libraryId }),
    );
    this.showLinks(entry, libraryId);
  }

  /** The library of an entry: the first of its targets the host opened. */
  private libraryOf(entry: JournalEntryView): string | undefined {
    return entry.target_ids.find((id) => this.known.some((l) => l.library_id === id));
  }

  /** Point View provenance and Show file at the entry's file, or leave them out. */
  private showLinks(entry: JournalEntryView, libraryId: string | undefined): void {
    const root = this.shadowRoot;
    if (!root) return;
    const target = viewProvenanceTarget(entry);
    const file = target?.file ?? this.fileOf(entry);
    const provenance = root.querySelector<HTMLAnchorElement>("a.view-provenance");
    const show = root.querySelector<HTMLAnchorElement>("a.show-file");
    const only = root.querySelector<HTMLElement>(".journal-only");
    const canLink = libraryId !== undefined && file !== undefined;
    if (provenance) {
      provenance.hidden = !(canLink && target !== undefined);
      if (canLink) provenance.href = openFileHash(libraryId, file, "provenance");
    }
    if (show) {
      show.hidden = !canLink;
      if (canLink) show.href = openFileHash(libraryId, file, "details");
    }
    if (only) only.hidden = !entry.provenance?.journal_only;
    const links = root.querySelector<HTMLElement>(".actions");
    if (links) links.hidden = !canLink;
  }

  /** The file an entry names through its object reference, when it names one. */
  private fileOf(entry: JournalEntryView): string | undefined {
    const object = (entry.object_ref ?? entry.provenance?.object_ref)?.split("@v", 1)[0];
    const match = /^urn:uuid:([0-9a-f-]{36})$/i.exec(object ?? "");
    return match?.[1];
  }

  /**
   * Read the entry again with its display name; a failed read keeps what the list gave and says
   * so. Network I/O.
   */
  private async refresh(): Promise<void> {
    const { options, shown } = this;
    this.abort?.abort();
    const generation = ++this.generation;
    const failed = this.shadowRoot?.querySelector<HTMLElement>(".read-failed");
    if (failed) failed.hidden = true;
    if (!options || shown === null || shown.id === undefined) return;
    try {
      const fresh = await options.source.entry(shown.id);
      if (generation !== this.generation) return;
      this.shown = fresh;
      this.show();
    } catch (error) {
      if (generation !== this.generation) return;
      console.warn(
        "FileSquid could not read a log entry",
        error instanceof Error ? error.name : error,
      );
      if (failed) failed.hidden = false;
    }
  }

  /** Put the entry as the journal stores it on the clipboard and announce it. Side effect: the clipboard. */
  private async copyJson(): Promise<void> {
    const { options, shown } = this;
    const status = this.shadowRoot?.querySelector(".announce");
    if (!options || shown === null || !status) return;
    status.textContent = "";
    try {
      await navigator.clipboard.writeText(storedJson(shown));
      status.textContent = options.labels.logsCopied;
    } catch (error) {
      console.warn(
        "FileSquid could not copy a log entry",
        error instanceof Error ? error.name : error,
      );
      status.textContent = options.labels.logsCopyFailed;
    }
  }
}
