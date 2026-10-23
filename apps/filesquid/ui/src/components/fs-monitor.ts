/**
 * `<fs-monitor>`: the FileSquid Monitor dashboard (ADR 033, SRS-F-061). Shows overall
 * health with reasons, the OCFL validation findings and runs (`<fs-ocfl-findings>`, UI-F2.14,
 * refreshed on `health.change` and on `ocfl.validation` process updates), live processes,
 * storage, throughput and queues, and the activity journal with search, live tail and JSON
 * Lines export. Every value is rendered with `textContent`; journal data never becomes markup.
 *
 * Look (UI-S-032, mockups `resources/ui-mockups/v3/31-monitor.svg` and its dark twin): a page
 * head with the 30 px title, `monitorOwnerOnly`, the Live chip on the active tint and the
 * secondary Download button (`monitorExport`, "Download JSON Lines", owner answer 8); each
 * section a card on `--fs-surface-card` with a 1 px `--fs-outline` border and radius 10 px, its
 * heading an 11 px eyebrow; the health status word 22 px 700 in the heading font; column heads
 * 13 px 500 in `--fs-on-surface-variant`; a stalled or failed process's state in `--fs-error`
 * (warnings use the error colour too, owner answer 14); journal rows in mono.
 *
 * Accessibility (UI-S-016): each table is named by its section heading and sits in a named,
 * focusable scroll region, so narrow windows scroll the table rather than the page; when a
 * re-render removes the focused element, focus moves to the token field, the alert or the first
 * section heading. Focus moves only when it was inside the Monitor's replaced content: a
 * background load, poll or failed poll never takes focus from anywhere else on the page.
 */
import {
  ApiError,
  type ApiEvent,
  type FileSquidClient,
  type HealthReason,
  type HealthReport,
  type JournalEntry,
  type MetricsSnapshot,
  type ProcessView,
} from "../api/client";
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";
import { denialLabel, type DenialText } from "./denial";
import type { FsOcflFindings } from "./fs-ocfl-findings";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { replaceKeepingFocus } from "../a11y/focus";

/** The part of the API client the Monitor needs (injectable in tests). */
export type MonitorClient = Pick<
  FileSquidClient,
  | "adminHealth"
  | "adminProcesses"
  | "adminJournal"
  | "adminMetrics"
  | "adminJournalExport"
  | "adminOcflFindings"
  | "startOcflValidation"
  | "subscribe"
>;

/** Kind of the process a validation run registers (API-S-022). */
const VALIDATION_PROCESS = "ocfl.validation";
/** Code of the health reason for open OCFL validation errors (API-S-022). */
const OCFL_REASON = "ocfl_validation_errors";
/** Topic the event channel sends when it may have dropped events; everything is read again. */
const RESYNC_TOPIC = "events.resync";

/** Where the server host's token is kept while the page is open (in memory only). */
export interface TokenStore {
  /** The stored token, if any. */
  get(): string | undefined;
  /** Store a token. */
  set(token: string): void;
}

/** Options for {@link FsMonitor.configure}. */
export interface MonitorOptions {
  /** The distribution's label table. */
  labels: LabelTable;
  /** Admin API client. */
  client: MonitorClient;
  /** Token store for the server host; omit on the desktop host (owner access). */
  tokens?: TokenStore;
  /** Interval for refreshing health, processes and metrics; 0 disables polling. Default 5000. */
  pollMs?: number;
  /**
   * The address of a library's version-inventory repair offer (UI-1.1). The OCFL validation
   * section links its W010 note there; without it the note has no link.
   */
  repairHref?: (libraryId: string) => string;
}

/** Everything the dashboard shows after a full load. */
interface Snapshot {
  health: HealthReport;
  processes: ProcessView[];
  metrics: MetricsSnapshot;
  entries: JournalEntry[];
}

/** Journal rows kept on screen; older rows drop off the live tail. */
const MAX_JOURNAL_ROWS = 200;
/** Live journal entries held while the first page loads; the tail shows at most 200 anyway. */
const MAX_BUFFERED_ENTRIES = 1000;
const GIB = 1024 ** 3;

const STYLE = `
  :host { display: block; color: var(--fs-on-surface); font-family: var(--fs-font-sans); font-size: var(--fs-font-size-small); }
  .page-head { display: flex; flex-wrap: wrap; align-items: baseline; gap: var(--fs-space-2) var(--fs-space-3); margin-block-end: var(--fs-space-4); }
  .page-head .owner { flex: 1 1 auto; margin: 0; color: var(--fs-on-surface-variant); }
  .page-head .button { align-self: center; }
  section { margin-block-end: var(--fs-space-4); padding: var(--fs-space-4); border: 1px solid var(--fs-outline); border-radius: var(--fs-radius-dialog); background: var(--fs-surface-card); }
  h2 { margin: 0 0 var(--fs-space-3); font-size: var(--fs-font-size-eyebrow); font-weight: 400; letter-spacing: 0.1em; text-transform: uppercase; color: var(--fs-on-surface-muted); }
  .page-head h2 { margin: 0; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-page); font-weight: 700; letter-spacing: normal; text-transform: none; color: var(--fs-on-surface-heading); }
  table { width: 100%; border-collapse: collapse; }
  .scroll { overflow-x: auto; }
  th, td { text-align: start; padding: var(--fs-space-2); border-block-end: 1px solid var(--fs-outline); }
  th { font-weight: 500; color: var(--fs-on-surface-variant); }
  [data-section="journal"] td { font-family: var(--fs-font-mono); }
  dl { display: grid; grid-template-columns: minmax(0, max-content) minmax(0, 1fr); gap: var(--fs-space-1) var(--fs-space-4); margin: 0; }
  dt, dd { min-width: 0; overflow-wrap: anywhere; }
  dt { color: var(--fs-on-surface-variant); }
  dd { margin: 0; color: var(--fs-on-surface); }
  .status { margin: 0 0 var(--fs-space-2); font-family: var(--fs-font-heading); font-size: var(--fs-font-size-heading); font-weight: 700; color: var(--fs-on-surface-heading); }
  [data-section="health"] ul { margin: 0; padding-inline-start: var(--fs-space-3); }
  [data-status="degraded"] .status, [data-status="failing"] .status, tr[data-state="stalled"] td:nth-child(2), tr[data-state="failed"] td:nth-child(2), tr[data-outcome="failed"] td { color: var(--fs-error); }
  form { display: flex; flex-wrap: wrap; gap: var(--fs-space-2); align-items: center; margin-block-end: var(--fs-space-3); }
  form .input { flex: 0 1 22.5rem; }
  [role="alert"] { color: var(--fs-error); overflow-wrap: anywhere; }
`;

/** Custom element that renders the Monitor dashboard. */
export class FsMonitor extends HTMLElement {
  private options?: MonitorOptions;
  private processes = new Map<string, ProcessView>();
  private journal: JournalEntry[] = [];
  private operationFilter = "";
  /**
   * Live journal entries that arrived while a journal page was being read; merged into the
   * page when it answers, so a re-read never drops a newer entry.
   */
  private liveDuringRead?: JournalEntry[];
  /** The OCFL validation section, refreshed on `health.change` and validation updates. */
  private findings?: FsOcflFindings;
  private unsubscribe?: () => void;
  private timer?: ReturnType<typeof setInterval>;
  /** Bumped by every stop; a load that finishes under an older generation is discarded. */
  private generation = 0;

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set labels, client and token store. Loads data when connected.
   *
   * @param options See {@link MonitorOptions}.
   */
  configure(options: MonitorOptions): void {
    this.options = options;
    if (this.isConnected) void this.start();
  }

  /** Load data when attached to the document. */
  connectedCallback(): void {
    if (this.options) void this.start();
  }

  /** Stop polling and end the event subscription. */
  disconnectedCallback(): void {
    this.stop();
  }

  /** Release the poll timer and the event subscription, and invalidate any pending load. */
  private stop(): void {
    this.generation += 1;
    if (this.timer !== undefined) clearInterval(this.timer);
    this.timer = undefined;
    this.unsubscribe?.();
    this.unsubscribe = undefined;
  }

  /**
   * Subscribe to live events, load everything, then start polling. Network I/O.
   *
   * The subscription opens before the first journal page is read, so no entry published in
   * between is lost: journal entries that arrive during the load are merged by id once it
   * finishes. Processes and health come from the loaded snapshot and the poll. If the element
   * is disconnected (or restarted) during the load, `stop` has already closed the
   * subscription and the stale load sets nothing up.
   */
  private async start(): Promise<void> {
    const options = this.options;
    if (!options) return;
    this.stop();
    const generation = this.generation;
    let buffered: JournalEntry[] | undefined = [];
    let snapshot: Snapshot;
    try {
      this.unsubscribe = options.client.subscribe((event) => {
        if (!buffered) return this.applyEvent(event);
        if (event.topic === "journal.entry" && isObject(event.data)) {
          if (buffered.length < MAX_BUFFERED_ENTRIES) buffered.push(event.data as JournalEntry);
        }
      });
      snapshot = await this.fetchAll(options.client);
    } catch (error) {
      if (generation === this.generation) this.showError(error);
      return;
    }
    if (generation !== this.generation) return;
    this.renderAll(snapshot);
    const early = buffered;
    buffered = undefined;
    early.forEach((entry) => this.addJournalEntry(entry));
    const pollMs = options.pollMs ?? 5000;
    if (pollMs > 0) this.timer = setInterval(() => void this.refresh(), pollMs);
  }

  /** Fetch health, processes, metrics and the newest journal page. Network I/O. */
  private async fetchAll(client: MonitorClient): Promise<Snapshot> {
    const [health, processes, metrics, page] = await Promise.all([
      client.adminHealth(),
      client.adminProcesses(),
      client.adminMetrics(),
      client.adminJournal(this.journalFilters()),
    ]);
    return { health, processes, metrics, entries: page.entries };
  }

  /** Replace the dashboard with a freshly fetched snapshot. */
  private renderAll({ health, processes, metrics, entries }: Snapshot): void {
    this.processes = new Map(processes.map((p) => [p.id, p]));
    this.journal = entries;
    this.renderLayout();
    this.renderHealth(health);
    this.renderProcesses();
    this.renderMetrics(metrics, health);
    this.renderJournal();
  }

  /** Re-read the polled resources; errors are reported in place. Network I/O. */
  private async refresh(): Promise<void> {
    const client = this.options?.client;
    if (!client) return;
    try {
      const [health, processes, metrics] = await Promise.all([
        client.adminHealth(),
        client.adminProcesses(),
        client.adminMetrics(),
      ]);
      this.processes = new Map(processes.map((p) => [p.id, p]));
      this.renderHealth(health);
      this.renderProcesses();
      this.renderMetrics(metrics, health);
    } catch (error) {
      this.showError(error);
    }
  }

  /** Current journal query: newest first, optionally one operation. */
  private journalFilters() {
    return { order: "desc" as const, limit: 50, operation: this.operationFilter || undefined };
  }

  /** Apply one live event from the event channel. */
  private applyEvent(event: ApiEvent): void {
    if (event.topic === RESYNC_TOPIC) return this.resync();
    if (!isObject(event.data)) return;
    if (event.topic === "process.update") {
      const view = event.data as ProcessView;
      this.processes.set(view.id, view);
      this.renderProcesses();
      if (view.kind === VALIDATION_PROCESS) void this.findings?.refresh();
    } else if (event.topic === "health.change") {
      this.renderHealth(event.data as HealthReport);
      void this.findings?.refresh();
    } else if (event.topic === "journal.entry") {
      this.addJournalEntry(event.data as JournalEntry);
    }
  }

  /**
   * Read everything again after `events.resync`, which the event channel sends when it may
   * have dropped events: health, processes and metrics, the journal page and the OCFL
   * findings. Network I/O.
   */
  private resync(): void {
    void this.refresh();
    void this.searchJournal();
    void this.findings?.refresh();
  }

  /**
   * Keep a live entry for the journal read in flight, if any. Bounded: only the newest
   * {@link MAX_JOURNAL_ROWS} are kept, since no more are ever shown, so a stalled read under a
   * continuing stream cannot grow memory.
   */
  private bufferDuringRead(entry: JournalEntry): void {
    const live = this.liveDuringRead;
    if (!live) return;
    live.push(entry);
    if (live.length > MAX_JOURNAL_ROWS) live.shift();
  }

  /** Add a live journal entry matching the current filter; newest first, each id once. */
  private addJournalEntry(entry: JournalEntry): void {
    if (this.operationFilter && entry.operation !== this.operationFilter) return;
    if (this.journal.some((row) => row.id === entry.id)) return;
    this.bufferDuringRead(entry);
    this.journal = newestFirst([entry, ...this.journal]);
    this.renderJournal();
  }

  /**
   * Show the sign-in form, with the denial reason when there is one, when a token store exists
   * and the error is a 401/403 or carries a denial reason (including a token the client
   * refused to send, status 0). Otherwise show an alert naming the reason, or the error
   * message for other failures. When this removed the focused element, focus moves to the
   * token field or the alert.
   */
  private showError(error: unknown): void {
    this.stop();
    this.findings = undefined;
    const root = this.shadowRoot;
    const labels = this.options?.labels;
    if (!root || !labels) return;
    const denial = denialLabel(error, labels);
    const denied = error instanceof ApiError && (error.status === 401 || error.status === 403);
    const tokens = this.options?.tokens;
    if (tokens && (denied || denial)) {
      const note = denial ? [denialNote(denial)] : [];
      const form = this.tokenForm(labels, tokens);
      replaceKeepingFocus(root, [style(), ...note, form], () => form.querySelector("input"));
      return;
    }
    const message = denial?.text ?? (error instanceof Error ? error.message : String(error));
    const alert = el("p", `${labels.monitorUnavailable}: ${message}`);
    alert.setAttribute("role", "alert");
    if (denial) alert.dataset.reason = denial.reason;
    replaceKeepingFocus(root, [style(), alert], () => alert);
  }

  /** Build the token form; submitting keeps the token in the store, clears the field, reloads. */
  private tokenForm(labels: LabelTable, tokens: TokenStore): HTMLFormElement {
    const form = document.createElement("form");
    form.dataset.form = "token";
    const input = document.createElement("input");
    input.type = "password";
    input.id = "admin-token";
    input.autocomplete = "current-password";
    input.required = true;
    const label = el("label", labels.monitorAdminToken);
    label.setAttribute("for", input.id);
    input.className = "input";
    const button = el("button", labels.monitorSignIn);
    button.className = "button primary";
    button.setAttribute("type", "submit");
    form.append(label, input, button);
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      tokens.set(input.value.trim());
      input.value = "";
      void this.start();
    });
    return form;
  }

  /**
   * Create the dashboard's empty sections. When this removed the focused element (the sign-in
   * form), focus moves to the first section heading.
   */
  private renderLayout(): void {
    const root = this.shadowRoot;
    const labels = this.options?.labels;
    if (!root || !labels) return;
    this.findings = this.findingsSection();
    const sections = [
      section("health", labels.monitorHealth, el("p", ""), document.createElement("ul")),
      section("ocfl", labels.monitorFindings, this.findings),
      section(
        "processes",
        labels.monitorProcesses,
        scrollRegion("processes", processTable(labels)),
      ),
      section("storage", labels.monitorStorage, document.createElement("dl")),
      section("rates", labels.monitorRates, document.createElement("dl")),
      section(
        "journal",
        labels.monitorJournal,
        this.journalForm(labels),
        scrollRegion("journal", journalTable(labels)),
      ),
    ];
    const head = this.pageHead(labels);
    const first = () => root.querySelector<HTMLElement>("section h2");
    replaceKeepingFocus(root, [style(), head, ...sections], first);
    root.querySelector('[data-section="health"] p')?.classList.add("status");
  }

  /**
   * The page head: title, the owner-only line, the Live chip and the Download button of the
   * journal's JSON Lines export.
   */
  private pageHead(labels: LabelTable): HTMLElement {
    const head = document.createElement("div");
    head.className = "page-head";
    const owner = el("p", labels.monitorOwnerOnly);
    owner.className = "owner";
    const live = el("span", labels.monitorLive);
    live.className = "chip ok live";
    const download = el("button", labels.monitorExport);
    download.className = "button secondary";
    download.setAttribute("type", "button");
    download.dataset.action = "export";
    download.addEventListener("click", () => void this.exportJournal());
    head.append(el("h2", labels.monitor), owner, live, download);
    return head;
  }

  /**
   * Create the OCFL validation section's element. It reads its findings once it is in the
   * document. Network I/O happens there, not here.
   */
  private findingsSection(): FsOcflFindings {
    const findings = document.createElement("fs-ocfl-findings") as FsOcflFindings;
    const options = this.options;
    if (options) {
      findings.configure({
        labels: options.labels,
        client: options.client,
        repairHref: options.repairHref,
      });
    }
    return findings;
  }

  /** Render the overall status and its reasons. */
  private renderHealth(health: HealthReport): void {
    const target = this.shadowRoot?.querySelector<HTMLElement>('[data-section="health"]');
    const labels = this.options?.labels;
    if (!target || !labels) return;
    target.dataset.status = health.status;
    const status = target.querySelector(".status");
    if (status) status.textContent = labels[STATUS_LABEL[health.status]];
    target
      .querySelector("ul")
      ?.replaceChildren(...health.reasons.map((r) => reasonItem(r, labels)));
    this.renderStorage(health);
  }

  /** Render one row per process, or an empty-state row. */
  private renderProcesses(): void {
    const body = this.shadowRoot?.querySelector('[data-section="processes"] tbody');
    const labels = this.options?.labels;
    if (!body || !labels) return;
    const rows = [...this.processes.values()].map(processRow);
    body.replaceChildren(...(rows.length > 0 ? rows : [emptyRow(labels.monitorNoProcesses, 5)]));
  }

  /** Render storage facts from the health checks. */
  private renderStorage(health: HealthReport): void {
    const list = this.shadowRoot?.querySelector('[data-section="storage"] dl');
    const labels = this.options?.labels;
    if (!list || !labels) return;
    const { checks } = health;
    const yesNo = (value: boolean) => (value ? labels.yes : labels.no);
    list.replaceChildren(
      ...pair(labels.storageFree, checks.free_bytes === null ? "–" : formatGiB(checks.free_bytes)),
      ...pair(labels.storageWritable, yesNo(checks.data_dir_writable)),
      ...pair(labels.storageStore, yesNo(checks.store_ok)),
    );
  }

  /** Render counters and the journal queue fill. */
  private renderMetrics(metrics: MetricsSnapshot, health: HealthReport): void {
    const list = this.shadowRoot?.querySelector('[data-section="rates"] dl');
    const labels = this.options?.labels;
    if (!list || !labels) return;
    const count = (name: string) => String(metrics[name] ?? 0);
    list.replaceChildren(
      ...pair(labels.metricEntries, count("filesquid_journal_entries_total")),
      ...pair(labels.metricWarnings, count("filesquid_warnings_total")),
      ...pair(
        labels.metricErrors,
        `${count("filesquid_errors_total")} (${health.checks.errors_last_5m} / 5 min)`,
      ),
      ...pair(
        labels.metricQueue,
        `${count("filesquid_journal_queue_depth")} (${Math.round(health.checks.queue_fill * 100)} %)`,
      ),
    );
  }

  /** Render the journal rows (newest first). */
  private renderJournal(): void {
    const body = this.shadowRoot?.querySelector('[data-section="journal"] tbody');
    if (!body) return;
    body.replaceChildren(...this.journal.map(journalRow));
  }

  /** Build the journal search controls; the Download button sits in the page head. */
  private journalForm(labels: LabelTable): HTMLFormElement {
    const form = document.createElement("form");
    form.dataset.form = "journal";
    const input = document.createElement("input");
    input.id = "journal-operation";
    input.name = "operation";
    input.placeholder = "ocfl.commit";
    const label = el("label", labels.monitorFilter);
    label.setAttribute("for", input.id);
    input.className = "input";
    const search = el("button", labels.monitorSearch);
    search.className = "button secondary";
    search.setAttribute("type", "submit");
    form.append(label, input, search);
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      this.operationFilter = input.value.trim();
      void this.searchJournal();
    });
    return form;
  }

  /** Reload the journal with the current filter. Network I/O. */
  private async searchJournal(): Promise<void> {
    const client = this.options?.client;
    if (!client) return;
    const live: JournalEntry[] = [];
    this.liveDuringRead = live;
    try {
      const page = await client.adminJournal(this.journalFilters());
      this.journal = newestFirst([...live, ...page.entries]);
      this.renderJournal();
    } catch (error) {
      this.showError(error);
    } finally {
      if (this.liveDuringRead === live) this.liveDuringRead = undefined;
    }
  }

  /** Download the matching journal entries as a JSON Lines file. Network I/O. */
  private async exportJournal(): Promise<void> {
    const client = this.options?.client;
    if (!client) return;
    try {
      const text = await client.adminJournalExport({
        operation: this.operationFilter || undefined,
      });
      downloadText(text, "filesquid-journal.jsonl", "application/x-ndjson");
    } catch (error) {
      this.showError(error);
    }
  }
}

/** Label key for each overall status. */
const STATUS_LABEL: Record<HealthReport["status"], LabelKey> = {
  ok: "statusOk",
  degraded: "statusDegraded",
  failing: "statusFailing",
};

/**
 * The words of one health reason: the open OCFL validation errors from the label table, with
 * their codes and the number of affected objects; any other reason as the API words it. Pure.
 *
 * @param reason The reason as the health report carries it.
 * @param labels The active label table.
 * @returns The text to show.
 */
export function reasonText(reason: HealthReason, labels: LabelTable): string {
  if (reason.code !== OCFL_REASON || !reason.codes || reason.objects === undefined) {
    return reason.detail;
  }
  const values = { codes: reason.codes.join(", "), objects: String(reason.objects) };
  return formatLabel(labels.healthOcflErrors, values);
}

/** Build the list item of one health reason, keyed by its code. */
function reasonItem(reason: HealthReason, labels: LabelTable): HTMLLIElement {
  const item = el("li", reasonText(reason, labels));
  item.dataset.reason = reason.code;
  return item;
}

/**
 * Journal rows newest first by id, each id once, at most {@link MAX_JOURNAL_ROWS}. Pure.
 *
 * @param entries Entries in any order, possibly repeated.
 * @returns The rows to show.
 */
function newestFirst(entries: readonly JournalEntry[]): JournalEntry[] {
  const byId = new Map<number, JournalEntry>();
  const unnumbered: JournalEntry[] = [];
  for (const entry of entries) {
    if (entry.id === undefined || entry.id === null) unnumbered.push(entry);
    else if (!byId.has(entry.id)) byId.set(entry.id, entry);
  }
  return [...byId.values(), ...unnumbered]
    .sort((a, b) => (b.id ?? 0) - (a.id ?? 0))
    .slice(0, MAX_JOURNAL_ROWS);
}

/** Format a byte count as GiB with one decimal, for example "10.0 GiB". */
export function formatGiB(bytes: number): string {
  return `${(bytes / GIB).toFixed(1)} GiB`;
}

/** Progress, rate and ETA cells for one process. */
export function processCells(p: ProcessView): string[] {
  const progress = p.total === null ? String(p.done) : `${p.done} / ${p.total}`;
  const rate = p.rate_per_second === null ? "–" : `${Number(p.rate_per_second.toFixed(2))}/s`;
  const eta = p.eta_seconds === null ? "–" : `${Math.round(p.eta_seconds)} s`;
  return [p.kind, p.state, progress, rate, eta];
}

/** Build a table row for one process. */
function processRow(p: ProcessView): HTMLTableRowElement {
  const row = document.createElement("tr");
  row.dataset.processId = p.id;
  row.dataset.state = p.state;
  row.append(...processCells(p).map((text) => el("td", text)));
  return row;
}

/** Build a table row for one journal entry. */
function journalRow(entry: JournalEntry): HTMLTableRowElement {
  const row = document.createElement("tr");
  row.dataset.outcome = entry.outcome;
  row.append(
    ...[entry.time, entry.operation, entry.outcome, entry.actor].map((text) => el("td", text)),
  );
  return row;
}

/** Build the processes table with its header. */
function processTable(labels: LabelTable): HTMLTableElement {
  const heads = [
    labels.processKind,
    labels.processState,
    labels.processProgress,
    labels.processRate,
    labels.processEta,
  ];
  return table(heads);
}

/** Build the journal table with its header. */
function journalTable(labels: LabelTable): HTMLTableElement {
  return table([
    labels.journalTime,
    labels.journalOperation,
    labels.journalOutcome,
    labels.journalActor,
  ]);
}

/** Build a table with a header row and an empty body. */
function table(heads: string[]): HTMLTableElement {
  const result = document.createElement("table");
  const headRow = document.createElement("tr");
  headRow.append(...heads.map((text) => el("th", text)));
  result.createTHead().append(headRow);
  result.createTBody();
  return result;
}

/** Build a single-cell row spanning `columns`. */
function emptyRow(text: string, columns: number): HTMLTableRowElement {
  const row = document.createElement("tr");
  const cell = el("td", text);
  cell.colSpan = columns;
  row.append(cell);
  return row;
}

/** Id of a section's heading, which names the section's table and scroll region. */
function headingId(name: string): string {
  return `monitor-${name}-heading`;
}

/** Build a titled dashboard section. */
function section(name: string, title: string, ...children: Node[]): HTMLElement {
  const result = document.createElement("section");
  result.dataset.section = name;
  const heading = el("h2", title);
  heading.id = headingId(name);
  result.append(heading, ...children);
  return result;
}

/**
 * Wrap a section's table in a scroll region named by the section heading. The region is
 * focusable so a keyboard user can scroll a wide table in a narrow window.
 */
function scrollRegion(name: string, content: HTMLTableElement): HTMLDivElement {
  const region = document.createElement("div");
  region.className = "scroll";
  region.setAttribute("role", "region");
  region.setAttribute("tabindex", "0");
  region.setAttribute("aria-labelledby", headingId(name));
  content.setAttribute("aria-labelledby", headingId(name));
  region.append(content);
  return region;
}

/** Build a term/value pair for a definition list. */
function pair(term: string, value: string): HTMLElement[] {
  return [el("dt", term), el("dd", value)];
}

/** Create an element with text content. */
function el<K extends keyof HTMLElementTagNameMap>(tag: K, text: string): HTMLElementTagNameMap[K] {
  const result = document.createElement(tag);
  result.textContent = text;
  return result;
}

/** Create the component stylesheet, the shared accessibility rules first. */
function style(): HTMLStyleElement {
  return el("style", A11Y_STYLE + CONTROL_STYLE + STYLE);
}

/** True for non-null objects (event payload guard). */
function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/** Offer `text` to the user as a file download. Side effect: a temporary object URL. */
function downloadText(text: string, filename: string, type: string): void {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

/**
 * A note naming why the API refused the call. A missing credential is the expected first
 * state on the server host, so it is a status; a refused credential is an alert.
 *
 * @param denial The reason and its label.
 * @returns The paragraph to show above the token form.
 */
function denialNote(denial: DenialText): HTMLParagraphElement {
  const note = document.createElement("p");
  note.textContent = denial.text;
  note.dataset.reason = denial.reason;
  note.setAttribute("role", denial.reason === "missing_credentials" ? "status" : "alert");
  return note;
}
