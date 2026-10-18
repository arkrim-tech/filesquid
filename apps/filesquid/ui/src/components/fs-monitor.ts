/**
 * `<fs-monitor>`: the FileSquid Monitor dashboard (ADR 033, SRS-F-061). Shows overall
 * health with reasons, live processes, storage, throughput and queues, and the activity
 * journal with search, live tail and JSON Lines export. Every value is rendered with
 * `textContent`; journal data never becomes markup.
 */
import {
  ApiError,
  type ApiEvent,
  type FileSquidClient,
  type HealthReport,
  type JournalEntry,
  type MetricsSnapshot,
  type ProcessView,
} from "../api/client";
import type { LabelKey, LabelTable } from "../theme/labels";
import { denialLabel, type DenialText } from "./denial";

/** The part of the API client the Monitor needs (injectable in tests). */
export type MonitorClient = Pick<
  FileSquidClient,
  | "adminHealth"
  | "adminProcesses"
  | "adminJournal"
  | "adminMetrics"
  | "adminJournalExport"
  | "subscribe"
>;

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
  :host { display: block; color: var(--fs-on-surface); font-family: var(--fs-font-sans); }
  section { margin-bottom: var(--fs-space-5); padding: var(--fs-space-4); border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface); }
  h2 { margin: 0 0 var(--fs-space-2); font-size: var(--fs-font-size-body); }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: var(--fs-space-1) var(--fs-space-2); border-bottom: 1px solid var(--fs-outline); }
  td { font-family: var(--fs-font-mono); }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: var(--fs-space-1) var(--fs-space-4); margin: 0; }
  dd { margin: 0; font-family: var(--fs-font-mono); }
  [data-status="ok"] .status { color: var(--fs-primary); }
  [data-status="degraded"] .status, [data-status="failing"] .status, tr[data-state="stalled"], tr[data-state="failed"] { color: var(--fs-error); }
  form { display: flex; gap: var(--fs-space-2); align-items: center; margin-bottom: var(--fs-space-2); }
  [role="alert"] { color: var(--fs-error); }
`;

/** Custom element that renders the Monitor dashboard. */
export class FsMonitor extends HTMLElement {
  private options?: MonitorOptions;
  private processes = new Map<string, ProcessView>();
  private journal: JournalEntry[] = [];
  private operationFilter = "";
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
    if (!isObject(event.data)) return;
    if (event.topic === "process.update") {
      const view = event.data as ProcessView;
      this.processes.set(view.id, view);
      this.renderProcesses();
    } else if (event.topic === "health.change") {
      this.renderHealth(event.data as HealthReport);
    } else if (event.topic === "journal.entry") {
      this.addJournalEntry(event.data as JournalEntry);
    }
  }

  /** Add a live journal entry matching the current filter; newest first, each id once. */
  private addJournalEntry(entry: JournalEntry): void {
    if (this.operationFilter && entry.operation !== this.operationFilter) return;
    if (this.journal.some((row) => row.id === entry.id)) return;
    this.journal = [entry, ...this.journal]
      .sort((a, b) => (b.id ?? 0) - (a.id ?? 0))
      .slice(0, MAX_JOURNAL_ROWS);
    this.renderJournal();
  }

  /**
   * Show the sign-in form, with the denial reason when there is one, when a token store exists
   * and the error is a 401/403 or carries a denial reason (including a token the client
   * refused to send, status 0). Otherwise show an alert naming the reason, or the error
   * message for other failures.
   */
  private showError(error: unknown): void {
    this.stop();
    const root = this.shadowRoot;
    const labels = this.options?.labels;
    if (!root || !labels) return;
    const denial = denialLabel(error, labels);
    const denied = error instanceof ApiError && (error.status === 401 || error.status === 403);
    const tokens = this.options?.tokens;
    if (tokens && (denied || denial)) {
      const note = denial ? [denialNote(denial)] : [];
      root.replaceChildren(style(), ...note, this.tokenForm(labels, tokens));
      return;
    }
    const message = denial?.text ?? (error instanceof Error ? error.message : String(error));
    const alert = el("p", `${labels.monitorUnavailable}: ${message}`);
    alert.setAttribute("role", "alert");
    if (denial) alert.dataset.reason = denial.reason;
    root.replaceChildren(style(), alert);
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
    const button = el("button", labels.monitorSignIn);
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

  /** Create the dashboard's empty sections. */
  private renderLayout(): void {
    const root = this.shadowRoot;
    const labels = this.options?.labels;
    if (!root || !labels) return;
    root.replaceChildren(
      style(),
      section("health", labels.monitorHealth, el("p", ""), document.createElement("ul")),
      section("processes", labels.monitorProcesses, processTable(labels)),
      section("storage", labels.monitorStorage, document.createElement("dl")),
      section("rates", labels.monitorRates, document.createElement("dl")),
      section("journal", labels.monitorJournal, this.journalForm(labels), journalTable(labels)),
    );
    root.querySelector('[data-section="health"] p')?.classList.add("status");
  }

  /** Render the overall status and its reasons. */
  private renderHealth(health: HealthReport): void {
    const target = this.shadowRoot?.querySelector<HTMLElement>('[data-section="health"]');
    const labels = this.options?.labels;
    if (!target || !labels) return;
    target.dataset.status = health.status;
    const status = target.querySelector(".status");
    if (status) status.textContent = labels[STATUS_LABEL[health.status]];
    target.querySelector("ul")?.replaceChildren(...health.reasons.map((r) => el("li", r.detail)));
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

  /** Build the journal search and export controls. */
  private journalForm(labels: LabelTable): HTMLFormElement {
    const form = document.createElement("form");
    form.dataset.form = "journal";
    const input = document.createElement("input");
    input.id = "journal-operation";
    input.name = "operation";
    input.placeholder = "ocfl.commit";
    const label = el("label", labels.monitorFilter);
    label.setAttribute("for", input.id);
    const search = el("button", labels.monitorSearch);
    search.setAttribute("type", "submit");
    const exportButton = el("button", labels.monitorExport);
    exportButton.setAttribute("type", "button");
    exportButton.dataset.action = "export";
    exportButton.addEventListener("click", () => void this.exportJournal());
    form.append(label, input, search, exportButton);
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
    try {
      this.journal = (await client.adminJournal(this.journalFilters())).entries;
      this.renderJournal();
    } catch (error) {
      this.showError(error);
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

/** Build a titled dashboard section. */
function section(name: string, title: string, ...children: Node[]): HTMLElement {
  const result = document.createElement("section");
  result.dataset.section = name;
  result.append(el("h2", title), ...children);
  return result;
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

/** Create the component stylesheet. */
function style(): HTMLStyleElement {
  return el("style", STYLE);
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
