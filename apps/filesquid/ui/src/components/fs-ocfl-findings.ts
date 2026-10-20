/**
 * `<fs-ocfl-findings>`: the Monitor's OCFL validation section (UI-F2.14, API-S-022,
 * SRS-F-061). It reads `GET /api/v1/admin/ocfl/findings` and shows, per open library, whether
 * a run is going, the last run (start, end, outcome, objects checked and not checked) and the
 * next scheduled run; the schedule in force; the open errors and, apart from them, the open
 * warnings; for libraries with W010 objects a note that their files can be read but not changed,
 * linking to the UI-1.1 repair offer when the distribution gives its address; and a Validate
 * now control that starts a run of every open library. A location the API withholds is never
 * shown. `refresh()` re-reads the route; the Monitor calls it on `health.change` and on
 * `ocfl.validation` process updates. Refreshes are coalesced: one request in flight, at most
 * one queued behind it.
 *
 * A failed read shows `<fs-states>` (permission denied, unavailable on a 503, offline or error)
 * with Retry inside this section only, so the rest of the Monitor stays up.
 *
 * Accessibility (UI-S-016): each table is named by its heading inside a named, focusable
 * scroll region; a refresh replaces only table rows and notes, so focus on Validate now stays.
 */
import type { FileSquidClient, FindingsView } from "../api/client";
import { formatLabel, type LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { reasonLabel } from "./denial";
import { RETRY_EVENT, type FsStates } from "./fs-states";
import {
  FINDING_COLUMNS,
  RUN_COLUMNS,
  emptyRow,
  findingHeads,
  findingRow,
  readOnlyNotes,
  runHeads,
  runRow,
  runSummary,
  scheduleItems,
} from "./ocfl-findings-view";
import { failureState, type ViewState } from "./view-state";

/** The part of the API client the findings section needs (injectable in tests). */
export type FindingsClient = Pick<FileSquidClient, "adminOcflFindings" | "startOcflValidation">;

/** Options for {@link FsOcflFindings.configure}. */
export interface FindingsOptions {
  /** The distribution's label table. */
  labels: LabelTable;
  /** Admin API client. */
  client: FindingsClient;
  /**
   * The address of a library's version-inventory repair offer (UI-1.1), for the W010 note's
   * link. Without it the note is shown without a link.
   */
  repairHref?: (libraryId: string) => string;
  /** Whether the browser is online, for the failure state; `navigator.onLine` by default. */
  online?: () => boolean;
}

/** Which state the section is in, mirrored on the host as `data-state`. */
type Phase = "loading" | "ready" | "failed";

const STYLE = `
  :host { display: block; color: var(--fs-on-surface); font-family: var(--fs-font-sans); }
  h3 { margin: var(--fs-space-3) 0 var(--fs-space-1); font-size: var(--fs-font-size-body); }
  table { width: 100%; border-collapse: collapse; }
  .scroll { overflow-x: auto; }
  th, td { text-align: start; padding: var(--fs-space-1) var(--fs-space-2); border-block-end: 1px solid var(--fs-outline); overflow-wrap: anywhere; }
  td { font-family: var(--fs-font-mono); }
  [data-part="errors"] .code { color: var(--fs-error); font-weight: 600; }
  [data-part="warnings"] .code { color: var(--fs-on-surface-variant); font-weight: 600; }
  td[data-withheld] { color: var(--fs-on-surface-variant); font-family: var(--fs-font-sans); }
  dl { display: grid; grid-template-columns: minmax(0, max-content) minmax(0, 1fr); gap: var(--fs-space-1) var(--fs-space-4); margin: 0; }
  dt, dd { min-width: 0; overflow-wrap: anywhere; }
  dd { margin: 0; font-family: var(--fs-font-mono); }
  form { display: flex; flex-wrap: wrap; gap: var(--fs-space-2); align-items: center; margin: 0; }
  p { margin: var(--fs-space-1) 0; overflow-wrap: anywhere; }
  [data-part="run-result"][data-outcome="failed"] { color: var(--fs-error); }
  .note { color: var(--fs-on-surface-variant); }
  a { color: var(--fs-primary); }
`;

/** Custom element that renders the open OCFL validation findings and runs. */
export class FsOcflFindings extends HTMLElement {
  private options?: FindingsOptions;
  private inFlight?: Promise<void>;
  private queued?: Promise<void>;
  private starting = false;
  /** Bumped on disconnect; a read that finishes under an older generation is discarded. */
  private generation = 0;

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set labels, client and repair address, build the section, and read the findings when
   * connected.
   *
   * @param options See {@link FindingsOptions}.
   */
  configure(options: FindingsOptions): void {
    this.options = options;
    this.skeleton();
    if (this.isConnected) void this.refresh();
  }

  /** Read the findings when attached to the document. Network I/O. */
  connectedCallback(): void {
    if (!this.options) return;
    this.skeleton();
    void this.refresh();
  }

  /** Discard any read still in flight. */
  disconnectedCallback(): void {
    this.generation += 1;
    this.inFlight = undefined;
    this.queued = undefined;
  }

  /**
   * Re-read the findings route and render it. Network I/O. Coalesced: while a read is in
   * flight, every further call shares one read queued behind it.
   *
   * @returns Resolves when the read that covers this call has rendered (or failed in place).
   */
  refresh(): Promise<void> {
    if (this.queued) return this.queued;
    const current = this.inFlight;
    if (!current) return this.load();
    const generation = this.generation;
    this.queued = current.then(() => {
      if (generation !== this.generation) return;
      this.queued = undefined;
      return this.load();
    });
    return this.queued;
  }

  /** Start one read and remember it as the one in flight. Network I/O. */
  private load(): Promise<void> {
    const read = this.read().finally(() => {
      if (this.inFlight === read) this.inFlight = undefined;
    });
    this.inFlight = read;
    return read;
  }

  /** Read the findings route and render the answer or the failure state. Network I/O. */
  private async read(): Promise<void> {
    const options = this.options;
    if (!options) return;
    const generation = this.generation;
    try {
      const view = await options.client.adminOcflFindings();
      if (generation === this.generation) this.render(view);
    } catch (error) {
      if (generation !== this.generation) return;
      console.warn("OCFL findings could not be read", error);
      this.showState(failureState(error, this.online()));
    }
  }

  /** Whether the browser reports a connection. */
  private online(): boolean {
    return this.options?.online?.() ?? navigator.onLine;
  }

  /** Build the stylesheet, the states element and the empty content once. */
  private skeleton(): void {
    const root = this.shadowRoot;
    const labels = this.options?.labels;
    if (!root || !labels || root.querySelector(".content")) return;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + STYLE;
    const states = document.createElement("fs-states") as FsStates;
    states.configure({ labels });
    states.addEventListener(RETRY_EVENT, () => {
      this.showState({ kind: "loading" });
      void this.refresh();
    });
    const content = document.createElement("div");
    content.className = "content";
    content.append(...this.contentParts(labels));
    root.replaceChildren(style, states, content);
    this.showState({ kind: "loading" });
  }

  /** The Validate now form, the runs, the schedule and the two findings tables. */
  private contentParts(labels: LabelTable): Node[] {
    const schedule = document.createElement("dl");
    schedule.dataset.part = "schedule";
    const truncated = paragraph(labels.findingsTruncated, "truncated");
    truncated.hidden = true;
    const notes = document.createElement("div");
    notes.dataset.part = "notes";
    return [
      this.validateForm(labels),
      tablePart("runs", labels.findingsRuns, runHeads(labels)),
      schedule,
      tablePart("errors", labels.findingsErrors, findingHeads(labels)),
      tablePart("warnings", labels.findingsWarnings, findingHeads(labels)),
      notes,
      truncated,
    ];
  }

  /** Build the Validate now control and its status line. */
  private validateForm(labels: LabelTable): HTMLFormElement {
    const form = document.createElement("form");
    form.dataset.form = "validate";
    const button = document.createElement("button");
    button.type = "submit";
    button.dataset.action = "validate";
    button.textContent = labels.findingsValidate;
    const result = document.createElement("p");
    result.dataset.part = "run-result";
    result.setAttribute("role", "status");
    form.append(button, result);
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      void this.startRun();
    });
    return form;
  }

  /** Start a run of every open library, say what started, then re-read. Network I/O. */
  private async startRun(): Promise<void> {
    const options = this.options;
    if (!options || this.starting) return;
    this.starting = true;
    try {
      const accepted = await options.client.startOcflValidation({});
      this.setResult(runSummary(accepted, options.labels), "started");
    } catch (error) {
      console.warn("OCFL validation could not start", error);
      const reason = this.failureWords(error, options.labels);
      this.setResult(formatLabel(options.labels.findingsRunFailed, { reason }), "failed");
    } finally {
      this.starting = false;
    }
    await this.refresh();
  }

  /** The words for a failed call, from the label table. */
  private failureWords(error: unknown, labels: LabelTable): string {
    const state = failureState(error, this.online());
    if (state.kind === "denied")
      return state.reason ? reasonLabel(state.reason, labels) : labels.stateDenied;
    if (state.kind === "unavailable") return labels.stateUnavailable;
    if (state.kind === "offline") return labels.stateOffline;
    return labels.stateError;
  }

  /** Show the outcome of Validate now in its status line. */
  private setResult(text: string, outcome: "started" | "failed"): void {
    const result = this.shadowRoot?.querySelector<HTMLElement>('[data-part="run-result"]');
    if (!result) return;
    result.textContent = text;
    result.dataset.outcome = outcome;
  }

  /** Show a non-content state (or hide the states element for `ready`). */
  private showState(state: ViewState | "ready"): void {
    const root = this.shadowRoot;
    const states = root?.querySelector<FsStates>("fs-states");
    const content = root?.querySelector<HTMLElement>(".content");
    if (!states || !content) return;
    const ready = state === "ready";
    const phase: Phase = ready ? "ready" : state.kind === "loading" ? "loading" : "failed";
    this.dataset.state = phase;
    states.hidden = ready;
    content.hidden = !ready;
    if (!ready) states.state = state;
  }

  /** Render one findings answer: runs, schedule, errors, warnings and notes. */
  private render(view: FindingsView): void {
    const root = this.shadowRoot;
    const labels = this.options?.labels;
    if (!root || !labels) return;
    const runs = view.libraries.map((library) => runRow(library, labels));
    fillBody(root, "runs", runs, labels.findingsNoLibraries, RUN_COLUMNS);
    const errors = view.findings.filter((f) => f.severity === "error");
    const warnings = view.findings.filter((f) => f.severity === "warning");
    const rows = (list: typeof errors) => list.map((finding) => findingRow(finding, labels));
    fillBody(root, "errors", rows(errors), labels.findingsNoErrors, FINDING_COLUMNS);
    fillBody(root, "warnings", rows(warnings), labels.findingsNoWarnings, FINDING_COLUMNS);
    root
      .querySelector('[data-part="schedule"]')
      ?.replaceChildren(...scheduleItems(view.settings, labels));
    const notes = readOnlyNotes(warnings, labels, view.truncated, this.options?.repairHref);
    notes.forEach((note) => note.classList.add("note"));
    root.querySelector('[data-part="notes"]')?.replaceChildren(...notes);
    const truncated = root.querySelector<HTMLElement>('[data-note="truncated"]');
    if (truncated) truncated.hidden = !view.truncated;
    this.showState("ready");
  }
}

/** Fill one table's body with `rows`, or with one empty-state row. */
function fillBody(
  root: ShadowRoot,
  part: string,
  rows: HTMLTableRowElement[],
  empty: string,
  columns: number,
): void {
  const body = root.querySelector(`[data-part="${part}"] tbody`);
  body?.replaceChildren(...(rows.length > 0 ? rows : [emptyRow(empty, columns)]));
}

/** Id of a part's heading, which names its table and scroll region. */
function headingId(part: string): string {
  return `ocfl-${part}-heading`;
}

/**
 * Build a titled table part: a heading and the table inside a focusable scroll region, both
 * named by the heading.
 */
function tablePart(part: string, title: string, heads: string[]): HTMLElement {
  const wrapper = document.createElement("div");
  wrapper.dataset.part = part;
  const heading = document.createElement("h3");
  heading.id = headingId(part);
  heading.textContent = title;
  const table = document.createElement("table");
  table.setAttribute("aria-labelledby", heading.id);
  const headRow = document.createElement("tr");
  headRow.append(
    ...heads.map((text) => Object.assign(document.createElement("th"), { textContent: text })),
  );
  table.createTHead().append(headRow);
  table.createTBody();
  const region = document.createElement("div");
  region.className = "scroll";
  region.setAttribute("role", "region");
  region.setAttribute("tabindex", "0");
  region.setAttribute("aria-labelledby", heading.id);
  region.append(table);
  wrapper.append(heading, region);
  return wrapper;
}

/** Create a paragraph marked as a note. */
function paragraph(text: string, note: string): HTMLParagraphElement {
  const result = document.createElement("p");
  result.textContent = text;
  result.dataset.note = note;
  result.className = "note";
  return result;
}
