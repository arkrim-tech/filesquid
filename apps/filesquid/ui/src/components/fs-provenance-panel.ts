/**
 * `<fs-provenance-panel>`: the Provenance tab of the file panel (UI-F2.15; UI-S-020, UI-S-022,
 * UI-S-031, mockups `resources/ui-mockups/v3/36-panel-provenance.svg` and `25-panel-states.svg`;
 * ADR 040). It lists the file's PROV-O history newest first from the history routes of
 * API-S-029: one block per activity with its kind (13 px 600), `prov:Activity`, the version chip
 * and its time in UTC, the latest expanded. An expanded block shows the `prov:` attributes: the
 * times and the agents from the history entry (the installation agent, the software agent with
 * its version, an agent that was not recorded shown as unrecorded) and `prov:used`,
 * `prov:wasGeneratedBy`, `prov:wasDerivedFrom` and `prov:wasInvalidatedBy` from the version's
 * JSON-LD record, which is read when the block is first expanded. A reconstructed activity is
 * marked. Nothing is invented: a relation the API does not return has no row.
 *
 * The footer holds **View in Logs** (the owner or an administrator only, `canViewLogs`), which
 * opens `#/logs?file=<uuid>`, and **JSON-LD**, which saves the file's history as JSON-LD. Each
 * expanded activity ends with its own View in Logs link for its operation. There is no per-file
 * audit log here: the journal is browsed only in the Logs view (UI-S-021).
 *
 * It is a panel of the file panel's named slots: it receives the file as its `fileRef` property
 * and reads only through a {@link ProvenanceSource}; it holds no reference to any other panel,
 * so it mounts alone. Its table is the shared {@link PropertyTable} (filter, count, the loading,
 * empty, no-match and error states of mockup 25, copy), which draws the activities through its
 * `renderBody`.
 *
 * Accessibility (UI-S-016): the shared stylesheet comes first; each activity's header is a
 * native button with `aria-expanded`; attributes are description lists; copy buttons are named
 * by their property; a copy and an export are announced in polite status regions; the error
 * state and a failed export are alerts; a load never moves focus, and a toggle keeps focus on
 * the button that was pressed.
 */
import { formatLabel, type LabelTable } from "../theme/labels";
import { logsHash } from "../app/routes";
import { focusElement } from "../a11y/focus";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { icon } from "./icons";
import { HOST_EVENT } from "./fs-host-indicator";
import { MAX_PROPERTY_ROWS, PROPERTY_TABLE_STYLE, PropertyTable } from "./property-table";
import { saveFileToDevice } from "./save-file";
import type { PropertyRow } from "../files/details";
import type { FileRef } from "../files/model";
import { fileIdOf } from "../files/ops";
import {
  activityAttributes,
  activityRow,
  activityTime,
  activityViews,
  kindWords,
  parseProvRecord,
  readAllHistory,
  type ActivityView,
  type AllHistory,
  type Attribute,
  type ProvRecord,
} from "../files/provenance";
import type { ProvenanceSource } from "../files/provenance-source";

/** What the panel needs. */
export interface ProvenancePanelOptions {
  /** The active label table. */
  labels: LabelTable;
  /** Reads the history, the records and the export. */
  source: ProvenanceSource;
  /**
   * Whether the viewer is the owner or an administrator, who may open the Logs view (UI-S-020);
   * asked on each render. No View in Logs link is drawn when it is absent or answers false.
   */
  canViewLogs?: () => boolean;
  /** Whether the browser is online; `navigator.onLine` when absent. */
  online?: () => boolean;
  /** Saves the exported file; the browser's own download when absent. */
  save?: (blob: Blob, name: string) => void;
}

/** The state of one activity's record read. */
type RecordState =
  | { readonly status: "loading" }
  | { readonly status: "ready"; readonly record: ProvRecord }
  | { readonly status: "error" };

const STYLE = `
  :host { display: flex; flex-direction: column; min-block-size: 100%; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); color: var(--fs-on-surface); }
  :host([hidden]) { display: none; }
  ol.timeline { position: relative; margin: 0; padding: var(--fs-space-1) var(--fs-space-3) var(--fs-space-3); list-style: none; }
  ol.timeline::before { content: ""; position: absolute; inset-block: var(--fs-space-3); inset-inline-start: 25px; border-inline-start: 1.5px solid var(--fs-outline); }
  .activity { position: relative; margin-block-end: var(--fs-space-2); margin-inline-end: var(--fs-space-1); padding-inline-start: var(--fs-space-4); }
  .dot { position: absolute; inset-inline-start: 4.5px; inset-block-start: 0.4375rem; box-sizing: border-box; inline-size: 0.5625rem; block-size: 0.5625rem; border: 1.5px solid var(--fs-outline-strong); border-radius: var(--fs-radius-pill); background: var(--fs-surface-card); }
  .activity[data-expanded="true"] .dot { border-color: var(--fs-primary); background: var(--fs-primary); }
  .toggle { display: flex; align-items: center; gap: 0; inline-size: 100%; min-block-size: 1.5rem; margin: 0; padding: 0; border: 0; border-radius: var(--fs-radius-item); background: none; color: var(--fs-on-surface); font: inherit; text-align: start; cursor: pointer; }
  .toggle:hover { background: var(--fs-surface-raised); }
  .chev { display: inline-flex; flex: 0 0 auto; color: var(--fs-icon-muted); transition: transform var(--fs-motion-duration); }
  .activity[data-expanded="true"] .chev { transform: rotate(90deg); }
  .kind { font-size: var(--fs-font-size-small); font-weight: 600; color: var(--fs-on-surface); }
  .type { font-family: var(--fs-font-mono); font-size: var(--fs-font-size-eyebrow); color: var(--fs-on-surface-muted); }
  .chip.version { margin-inline-start: auto; padding-inline: var(--fs-space-2); line-height: 1.6; }
  .when { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-2); margin: 0; padding-inline-start: var(--fs-space-3); font-size: var(--fs-font-size-small); line-height: 1.3; color: var(--fs-on-surface-muted); }
  .activity[data-expanded="true"] .when { color: var(--fs-on-surface-variant); }
  .details { padding-inline-start: var(--fs-space-3); }
  .details[hidden] { display: none; }
  dl.attrs { margin: var(--fs-space-2) 0 0; padding: 0; }
  .attr { margin-block-end: var(--fs-space-1); }
  .attr dt.name { margin: 0; font-family: var(--fs-font-mono); font-size: var(--fs-font-size-small); font-weight: 500; line-height: 1.3; color: var(--fs-on-surface-muted); overflow-wrap: anywhere; }
  .attr dd.val { position: relative; display: flex; align-items: start; gap: var(--fs-space-1); margin: 0; font-size: var(--fs-font-size-small); line-height: 1.3; overflow-wrap: anywhere; }
  .val .text { flex: 1 1 auto; min-width: 0; }
  .val.mono .text, .val.mono-muted .text { font-family: var(--fs-font-mono); }
  .val.mono-muted .text, .val.plain .text { color: var(--fs-on-surface-variant); }
  .val.muted .text { color: var(--fs-on-surface-muted); }
  .val .copy { opacity: 0; transition: opacity var(--fs-motion-duration); }
  .val:hover .copy, .val:focus-within .copy, .val .copy:focus-visible { opacity: 1; }
  .record-note { margin: var(--fs-space-2) 0 0; color: var(--fs-on-surface-muted); font-size: var(--fs-font-size-small); }
  .record-error { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-2); margin-block-start: var(--fs-space-2); color: var(--fs-error); font-size: var(--fs-font-size-small); }
  .record-error p { margin: 0; }
  a.activity-logs { display: inline-block; margin-block-start: var(--fs-space-1); padding-block: var(--fs-space-1); color: var(--fs-primary); font-size: var(--fs-font-size-small); font-weight: 600; text-decoration: none; }
  a.activity-logs:hover { text-decoration: underline; }
  .footer.bar { display: flex; align-items: center; justify-content: space-between; gap: var(--fs-space-2); padding-block: var(--fs-space-1); }
  .footer .note { margin: 0; color: var(--fs-on-surface-muted); font-size: var(--fs-font-size-small); }
  .footer .actions { display: flex; align-items: center; justify-content: space-between; gap: var(--fs-space-2); inline-size: 100%; }
  .footer [hidden] { display: none; }
  .footer .status { position: absolute; inline-size: 1px; block-size: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  .footer .failed { margin: 0; color: var(--fs-error); font-size: var(--fs-font-size-small); }
  .after-rows { color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  .after-rows p { margin: 0; }
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

/** The kind of an error, without its message. */
function errorName(error: unknown): string {
  return error instanceof Error ? error.name : typeof error;
}

/** Custom element that shows the PROV-O history of one file. */
export class FsProvenancePanel extends HTMLElement {
  private options?: ProvenancePanelOptions;
  private table?: PropertyTable;
  private footer?: HTMLElement;
  private file: FileRef | null = null;
  private generation = 0;
  private abort?: AbortController;
  private views: ActivityView[] = [];
  private records = new Map<string, RecordState>();
  private expanded = new Set<string>();
  private partial = { capped: false, truncated: false };
  /** True when the element left the document while it was loading. */
  private interrupted = false;

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the labels and source, render, and load the file already given, if any.
   *
   * @param options Labels, the source and the optional owner check, online check and saver.
   */
  configure(options: ProvenancePanelOptions): void {
    this.options = options;
    this.build(options);
    void this.load();
  }

  /** The tab label this panel asks for ("Provenance"). */
  get tabLabel(): string {
    return this.options?.labels.filePanelProvenance ?? "";
  }

  /** The file shown, or null. */
  get fileRef(): FileRef | null {
    return this.file;
  }

  /**
   * Show another file or another version of it (null clears the panel). A commit adds an
   * activity, so a new version reads the history again; the same reference reads nothing.
   */
  set fileRef(file: FileRef | null) {
    const same = file !== null && sameRef(file, this.file);
    this.file = file;
    if (same) return;
    void this.load();
  }

  /**
   * Cancel the reads in flight when the element leaves the document; a load that was cut short
   * starts again when the element is put back, so a reused panel is never left loading. The
   * panel also follows the host (the owner check of View in Logs) while it is in the document.
   */
  disconnectedCallback(): void {
    document.removeEventListener(HOST_EVENT, this.onHostChange);
    if (this.table?.state === "loading") this.interrupted = true;
    this.abort?.abort();
  }

  /** Restart a load that leaving the document cut short, and follow the host again. */
  connectedCallback(): void {
    document.addEventListener(HOST_EVENT, this.onHostChange);
    if (!this.interrupted) return;
    this.interrupted = false;
    void this.load();
  }

  /** The host became known or changed: draw View in Logs for the owner check as it now stands. */
  private readonly onHostChange = (): void => {
    if (this.table?.state !== "ready") return;
    this.syncFooterLink();
    this.renderRows();
  };

  /** Build the stylesheet, the table and the footer. */
  private build(options: ProvenancePanelOptions): void {
    const labels = options.labels;
    this.footer = this.buildFooter(labels);
    const table = new PropertyTable({
      labels,
      filterPlaceholder: labels.provenanceFilter,
      loadingText: labels.provenanceLoading,
      errorTitle: labels.provenanceError,
      footerElement: this.footer,
      countText: (shown, total) =>
        shown === total
          ? labels.provenanceCount
          : formatLabel(labels.provenanceCountFiltered, {
              shown: String(shown),
              count: String(total),
            }),
      noMatchTitle: (word) => formatLabel(labels.provenanceNoMatch, { word }),
      onRetry: () => void this.load(),
      online: options.online,
      onState: (state) => this.onState(state),
      renderBody: (rows) => this.body(rows),
      afterRows: () => this.notes(),
      emptyState: () => this.emptyState(),
      errorExtra: () => this.logsLink("button"),
    });
    this.table = table;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + CONTROL_STYLE + STYLE + PROPERTY_TABLE_STYLE;
    this.shadowRoot?.replaceChildren(style, table.element);
  }

  /** Record what the table shows and show the footer that goes with it. */
  private onState(state: string): void {
    this.dataset.state = state;
    const actions = this.footer?.querySelector<HTMLElement>(".actions");
    const note = this.footer?.querySelector<HTMLElement>(".note");
    if (actions) actions.hidden = state !== "ready";
    if (note) note.hidden = state === "ready";
    if (state === "ready") this.syncFooterLink();
  }

  /** Put the footer's View in Logs link in place for this file, or remove it when not allowed. */
  private syncFooterLink(): void {
    const actions = this.footer?.querySelector<HTMLElement>(".actions");
    if (!actions) return;
    actions.querySelector("a.logs")?.remove();
    const link = this.logsLink("footer");
    if (link) actions.prepend(link);
  }

  /** The footer: the line of the other states, and View in Logs and JSON-LD over the activities. */
  private buildFooter(labels: LabelTable): HTMLElement {
    const footer = node("div", "bar");
    const actions = node("div", "actions");
    actions.hidden = true;
    const status = node("p", "status");
    status.setAttribute("role", "status");
    const failed = node("p", "failed");
    failed.setAttribute("role", "alert");
    failed.hidden = true;
    const button = node("button", "button secondary export", labels.provenanceExport);
    button.type = "button";
    button.setAttribute("aria-label", labels.provenanceExportName);
    button.addEventListener("click", () => void this.exportHistory(status, failed));
    actions.append(button, status, failed);
    footer.append(node("p", "note", labels.provenanceFooter), actions);
    return footer;
  }

  /** The View in Logs link for the file, or undefined when the viewer may not open the Logs view. */
  private logsLink(variant: "footer" | "button"): HTMLElement | undefined {
    const { options, file } = this;
    const id = file === null ? undefined : fileIdOf(file.object_id);
    if (!options?.canViewLogs?.() || id === undefined) return undefined;
    const link = node(
      "a",
      variant === "footer" ? "button ghost logs" : "button ghost logs in-state",
    );
    link.href = logsHash({ file: id });
    link.textContent = options.labels.provenanceViewInLogs;
    return link;
  }

  /** The empty state of a file with no recorded activity. */
  private emptyState(): HTMLElement {
    const labels = this.options?.labels;
    const state = node("div", "state");
    state.setAttribute("role", "status");
    state.append(
      icon("history", 24),
      node("p", "title", labels?.provenanceEmpty),
      node("p", "detail", labels?.provenanceEmptyHint),
    );
    return state;
  }

  /** The notes below the list: the page bound reached, or the index that left activities out. */
  private notes(): HTMLElement | undefined {
    const labels = this.options?.labels;
    const element = node("div", "");
    if (!labels || (!this.partial.capped && !this.partial.truncated)) return undefined;
    if (this.partial.capped) {
      const count = new Intl.NumberFormat(document.documentElement.lang || "en").format(
        this.views.length,
      );
      element.append(node("p", "", formatLabel(labels.provenanceListLimit, { count })));
    }
    if (this.partial.truncated) element.append(node("p", "", labels.provenanceIndexTruncated));
    return element;
  }

  /** Read the history of the file shown and fill the table; ignore stale answers. */
  private async load(): Promise<void> {
    const { options, table, file } = this;
    if (!options || !table) return;
    this.abort?.abort();
    const generation = ++this.generation;
    this.views = [];
    this.records = new Map();
    this.expanded = new Set();
    this.partial = { capped: false, truncated: false };
    this.clearExportResult();
    if (!file) return table.clear();
    const abort = new AbortController();
    this.abort = abort;
    table.loading();
    try {
      const all = await readAllHistory(options.source, file, { signal: abort.signal });
      if (generation !== this.generation) return;
      this.show(all);
    } catch (error) {
      if (generation !== this.generation || abort.signal.aborted) return;
      // A reference with no file history (not a file's own original) has no activities.
      if (error instanceof TypeError)
        return this.show({ entries: [], truncated: false, capped: false });
      console.warn("FileSquid could not read the provenance", errorName(error));
      table.error(error);
    }
  }

  /** Show what the history reads found: the newest activities up to the table's bound, the latest expanded. */
  private show(all: AllHistory): void {
    const newest = activityViews(all.entries);
    this.views = newest.slice(0, MAX_PROPERTY_ROWS);
    const cut = newest.length > this.views.length;
    this.partial = { capped: all.capped || cut, truncated: all.truncated };
    const latest = this.views[0];
    if (latest) this.expanded.add(latest.key);
    this.renderRows();
    if (latest) void this.ensureRecord(latest);
  }

  /** Forget the last export's announcement and failure, so another file does not inherit them. */
  private clearExportResult(): void {
    const status = this.footer?.querySelector<HTMLElement>(".status");
    const failed = this.footer?.querySelector<HTMLElement>(".failed");
    if (status) status.textContent = "";
    if (failed) {
      failed.textContent = "";
      failed.hidden = true;
    }
  }

  /** The rows the table keeps, one per activity, with the record text once read. */
  private renderRows(): void {
    const labels = this.options?.labels;
    if (!labels || !this.table) return;
    const locale = document.documentElement.lang || "en";
    const rows = this.views.map((view) => {
      const state = this.records.get(view.key);
      return activityRow(
        view,
        labels,
        locale,
        state?.status === "ready" ? state.record : undefined,
      );
    });
    const focused = this.focusedActivity();
    this.table.rows(rows);
    if (focused !== undefined) this.focusToggle(focused);
  }

  /**
   * The key of the activity that holds focus (its toggle, copy button or retry button), so a
   * render that replaces the list can put focus back on that activity's toggle; undefined when
   * focus is elsewhere, so a background load never takes it.
   */
  private focusedActivity(): string | undefined {
    const active = this.shadowRoot?.activeElement;
    return active?.closest<HTMLElement>("li.activity")?.dataset.key;
  }

  /** Draw the activities whose rows match the filter, newest first. */
  private body(rows: readonly PropertyRow[]): HTMLElement {
    const list = node("ol", "timeline");
    const byKey = new Map(this.views.map((view) => [view.key, view]));
    for (const row of rows) {
      const view = byKey.get(row.key);
      if (view) list.append(this.activity(view));
    }
    return list;
  }

  /** One activity: the toggle header, the time with the reconstructed mark, and its details. */
  private activity(view: ActivityView): HTMLElement {
    const labels = this.options?.labels as LabelTable;
    const locale = document.documentElement.lang || "en";
    const open = this.expanded.has(view.key);
    const item = node("li", "activity");
    item.dataset.key = view.key;
    item.dataset.expanded = String(open);
    const kind = kindWords(view.rawKind, labels);
    const toggle = node("button", "toggle");
    toggle.type = "button";
    toggle.setAttribute("aria-expanded", String(open));
    toggle.setAttribute("aria-controls", `details-${view.key.replace(/[^\w-]/g, "_")}`);
    toggle.setAttribute(
      "aria-label",
      formatLabel(labels.provenanceToggle, { kind, version: view.version }),
    );
    toggle.addEventListener("click", () => this.toggle(view));
    const chevron = node("span", "chev");
    chevron.append(icon("chevron", 16));
    toggle.append(
      chevron,
      node("span", "kind", kind),
      node("span", "type", "prov:Activity"),
      node("span", "chip version mono-chip", view.version),
    );
    const when = node("p", "when", activityTime(view.endedAt, locale));
    if (view.reconstructed) when.append(this.reconstructedChip(labels));
    item.append(node("span", "dot"), toggle, when, this.details(view, open));
    return item;
  }

  /** The chip that marks a reconstructed activity, with its explanation as a title. */
  private reconstructedChip(labels: LabelTable): HTMLElement {
    const chip = node("span", "chip reconstructed muted", labels.provenanceReconstructed);
    chip.title = labels.provenanceReconstructedHelp;
    return chip;
  }

  /** The details of an activity: its attributes, its record's state and its View in Logs link. */
  private details(view: ActivityView, open: boolean): HTMLElement {
    const labels = this.options?.labels as LabelTable;
    const locale = document.documentElement.lang || "en";
    const details = node("div", "details");
    details.id = `details-${view.key.replace(/[^\w-]/g, "_")}`;
    details.hidden = !open;
    if (!open) return details;
    const state = this.records.get(view.key);
    const record = state?.status === "ready" ? state.record : undefined;
    const attrs = node("dl", "attrs");
    attrs.append(...activityAttributes(view, record, labels, locale).map((a) => this.attribute(a)));
    details.append(attrs);
    if (state?.status === "loading") this.recordNote(details, labels.provenanceRecordLoading);
    if (state?.status === "error") details.append(this.recordError(view));
    const logs = this.operationLink(view);
    if (logs) details.append(logs);
    return details;
  }

  /** A polite status line inside an activity. */
  private recordNote(details: HTMLElement, text: string): void {
    const note = node("p", "record-note", text);
    note.setAttribute("role", "status");
    details.append(note);
  }

  /** The alert of an activity whose record could not be read, with Try again. */
  private recordError(view: ActivityView): HTMLElement {
    const labels = this.options?.labels as LabelTable;
    const box = node("div", "record-error");
    box.setAttribute("role", "alert");
    const retry = node("button", "button secondary retry-record", labels.stateRetry);
    retry.type = "button";
    retry.addEventListener("click", () => {
      this.records.delete(view.key);
      void this.ensureRecord(view);
    });
    box.append(
      node("p", "", formatLabel(labels.provenanceRecordError, { version: view.version })),
      retry,
    );
    return box;
  }

  /** The View in Logs link of one activity's operation, for the owner or an administrator. */
  private operationLink(view: ActivityView): HTMLElement | undefined {
    const labels = this.options?.labels as LabelTable;
    if (!this.options?.canViewLogs?.()) return undefined;
    const link = node("a", "logs activity-logs", labels.provenanceViewInLogs);
    link.href = logsHash({ operation_id: view.operationId });
    link.setAttribute(
      "aria-label",
      formatLabel(labels.provenanceViewOperationInLogs, {
        kind: kindWords(view.rawKind, labels),
        version: view.version,
      }),
    );
    return link;
  }

  /** One attribute: its name over its values, each with a copy button when it has a whole text. */
  private attribute(attribute: Attribute): HTMLElement {
    const labels = this.options?.labels as LabelTable;
    const element = node("div", "attr");
    element.dataset.name = attribute.name;
    element.append(node("dt", "name", attribute.name));
    for (const value of attribute.values) {
      const dd = node("dd", `val ${value.tone}`);
      dd.append(node("span", "text", value.text));
      const whole = value.copy;
      if (whole !== undefined) dd.append(this.copyButton(attribute.name, whole, labels));
      element.append(dd);
    }
    return element;
  }

  /** A copy button for one value, named by its attribute. */
  private copyButton(property: string, whole: string, labels: LabelTable): HTMLButtonElement {
    const copy = node("button", "copy");
    copy.type = "button";
    copy.setAttribute("aria-label", formatLabel(labels.filePanelCopy, { property }));
    copy.append(icon("copy", 16));
    copy.addEventListener("click", () => void this.table?.copyValue(property, whole));
    return copy;
  }

  /** Expand or collapse an activity; the first expansion reads its record. Focus stays on the toggle. */
  private toggle(view: ActivityView): void {
    if (this.expanded.has(view.key)) this.expanded.delete(view.key);
    else this.expanded.add(view.key);
    this.renderRows();
    this.focusToggle(view.key);
    if (this.expanded.has(view.key)) void this.ensureRecord(view);
  }

  /** Move focus to the toggle of an activity after a render replaced it. */
  private focusToggle(key: string): void {
    const items = this.shadowRoot?.querySelectorAll<HTMLElement>("li.activity") ?? [];
    for (const item of items) {
      const toggle =
        item.dataset.key === key ? item.querySelector<HTMLElement>("button.toggle") : null;
      if (toggle) focusElement(toggle);
    }
  }

  /** Read the record of an activity once, and draw its relations when they arrive. Network I/O. */
  private async ensureRecord(view: ActivityView): Promise<void> {
    const { options, file } = this;
    if (!options || !file || this.records.has(view.key)) return;
    const generation = this.generation;
    const signal = this.abort?.signal;
    this.records.set(view.key, { status: "loading" });
    this.renderRows();
    try {
      const json = await options.source.record(file, view.version, { signal });
      if (generation !== this.generation) return;
      this.records.set(view.key, { status: "ready", record: parseProvRecord(json) });
    } catch (error) {
      if (generation !== this.generation || signal?.aborted) return;
      console.warn("FileSquid could not read a provenance record", errorName(error));
      this.records.set(view.key, { status: "error" });
    }
    this.renderRows();
  }

  /**
   * Save the file's history as JSON-LD and announce it. A failure is an alert and changes
   * nothing. Network I/O, then the browser's download through `save`.
   */
  private async exportHistory(status: HTMLElement, failed: HTMLElement): Promise<void> {
    const { options, file } = this;
    if (!options || !file) return;
    const labels = options.labels;
    const generation = this.generation;
    const signal = this.abort?.signal;
    status.textContent = "";
    failed.hidden = true;
    try {
      const exported = await options.source.exportHistory(file, { signal });
      // Another file opened meanwhile: its panel must not announce this file's export.
      if (generation !== this.generation) return;
      (options.save ?? saveFileToDevice)(exported.blob, `${file.display_name}.history.jsonld`);
      const done = formatLabel(labels.provenanceExported, { name: file.display_name });
      status.textContent = exported.truncated ? `${done} ${labels.provenanceExportPartial}` : done;
    } catch (error) {
      if (generation !== this.generation) return;
      console.warn("FileSquid could not export the history", errorName(error));
      failed.textContent = labels.provenanceExportFailed;
      failed.hidden = false;
    }
  }
}
