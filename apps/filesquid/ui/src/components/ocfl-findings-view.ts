/**
 * Pure DOM builders for `<fs-ocfl-findings>` (UI-F2.14, API-S-022): the rows of the runs,
 * errors and warnings tables, the schedule, and the read-only notes of libraries whose objects
 * have no version inventory (W010). Every value is set with `textContent`; nothing the API
 * sends becomes markup, and a location the API withholds (null) is never filled in.
 */
import type {
  FindingView,
  LibraryRuns,
  RunAccepted,
  RunView,
  ValidationSettings,
} from "../api/client";
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";

/** The warning of an object whose head has no version inventory (OCFL-S-020, UI-1.1 repair). */
export const NO_VERSION_INVENTORY = "W010";

/** Columns of the runs table. */
export const RUN_COLUMNS = 8;
/** Columns of the errors and warnings tables. */
export const FINDING_COLUMNS = 5;

const HOUR_MS = 3_600_000;

/** Label of each run outcome. */
const OUTCOME_LABEL: Record<RunView["outcome"], LabelKey> = {
  completed: "runCompleted",
  capped: "runCapped",
  cancelled: "runCancelled",
  failed: "runFailed",
};

/**
 * Header texts of the runs table, from the label table. Pure.
 *
 * @param labels The active label table.
 * @returns One text per column.
 */
export function runHeads(labels: LabelTable): string[] {
  return [
    labels.library,
    labels.findingsState,
    labels.findingsLastStarted,
    labels.findingsLastEnded,
    labels.findingsOutcome,
    labels.findingsChecked,
    labels.findingsNotChecked,
    labels.findingsNextRun,
  ];
}

/**
 * Header texts of the errors and warnings tables, from the label table. Pure.
 *
 * @param labels The active label table.
 * @returns One text per column.
 */
export function findingHeads(labels: LabelTable): string[] {
  return [
    labels.library,
    labels.findingsObject,
    labels.findingsVersion,
    labels.findingsCode,
    labels.findingsLocation,
  ];
}

/**
 * The outcome cell of a run: its label, with the error code of a failed run. Pure.
 *
 * @param run The finished run.
 * @param labels The active label table.
 * @returns The cell text.
 */
export function outcomeText(run: RunView, labels: LabelTable): string {
  const text = labels[OUTCOME_LABEL[run.outcome]];
  return run.error_code ? `${text} (${run.error_code})` : text;
}

/**
 * The cells of one library's row in the runs table: library, running or idle, last run start,
 * end, outcome, checked and not checked objects, and the next scheduled run. Pure.
 *
 * @param library The library's runs as the findings route answers them.
 * @param labels The active label table.
 * @returns One text per column.
 */
export function runCells(library: LibraryRuns, labels: LabelTable): string[] {
  const run = library.last_run;
  const state = library.running ? labels.findingsRunning : labels.findingsIdle;
  const next = library.next_run ?? labels.findingsNotScheduled;
  if (!run) {
    const never = labels.findingsNever;
    return [library.library_id, state, never, never, never, "–", "–", next];
  }
  return [
    library.library_id,
    state,
    run.started,
    run.ended,
    outcomeText(run, labels),
    String(run.checked),
    String(run.not_checked),
    next,
  ];
}

/**
 * Build one library's row of the runs table.
 *
 * @param library The library's runs.
 * @param labels The active label table.
 * @returns The row, keyed by `data-library-id`.
 */
export function runRow(library: LibraryRuns, labels: LabelTable): HTMLTableRowElement {
  const row = document.createElement("tr");
  row.dataset.libraryId = library.library_id;
  row.dataset.running = String(library.running);
  if (library.last_run) row.dataset.outcome = library.last_run.outcome;
  const cells = runCells(library, labels).map((text) => cell(text));
  // UI-S-032: the state as a chip (Running on the active tint, Idle muted); times in mono.
  const state = cells[1] as HTMLTableCellElement;
  state.replaceChildren(chip(state.textContent ?? "", library.running ? "ok" : "muted"));
  if (library.last_run) for (const index of [2, 3]) cells[index]?.classList.add("mono");
  if (library.next_run) cells[7]?.classList.add("mono");
  row.append(...cells);
  return row;
}

/** A status chip with its text (UI-S-024: chips always carry text). */
function chip(value: string, kind: "ok" | "muted" | "alert"): HTMLSpanElement {
  const span = text("span", value);
  span.className = `chip ${kind}`;
  return span;
}

/**
 * Build one finding's row: library, object, version, code and location. A withheld location
 * shows the `findingsLocationHidden` word and is marked `data-withheld`.
 *
 * @param finding The finding as the findings route answers it.
 * @param labels The active label table.
 * @returns The row, keyed by `data-object-id` and `data-code`.
 */
export function findingRow(finding: FindingView, labels: LabelTable): HTMLTableRowElement {
  const row = document.createElement("tr");
  row.dataset.libraryId = finding.library_id;
  row.dataset.objectId = finding.object_id;
  row.dataset.code = finding.code;
  row.dataset.severity = finding.severity;
  // UI-S-032: the code as a chip outlined in the error colour, for errors and warnings alike
  // (owner answer 14); object, version and location in mono.
  const code = document.createElement("td");
  code.className = "code";
  code.append(chip(finding.code, "alert"));
  const location = cell(finding.location ?? labels.findingsLocationHidden);
  if (finding.location === null) location.dataset.withheld = "";
  else location.className = "mono";
  const object = cell(finding.object_id);
  object.className = "mono";
  const version = cell(finding.version ?? "–");
  version.className = "mono";
  row.append(cell(finding.library_id), object, version, code, location);
  return row;
}

/**
 * Build a single-cell row spanning `columns`, for an empty table.
 *
 * @param text The empty-state words.
 * @param columns How many columns the row spans.
 * @returns The row.
 */
export function emptyRow(text: string, columns: number): HTMLTableRowElement {
  const row = document.createElement("tr");
  const only = cell(text);
  only.colSpan = columns;
  row.append(only);
  return row;
}

/**
 * The schedule in force as definition-list pairs: time between scheduled runs in hours and
 * the object cap of a run.
 *
 * @param settings The settings the findings route answers.
 * @param labels The active label table.
 * @returns The `dt` and `dd` elements, in order.
 */
export function scheduleItems(settings: ValidationSettings, labels: LabelTable): HTMLElement[] {
  const hours = String(Number((settings.interval_ms / HOUR_MS).toFixed(2)));
  return [
    text("dt", labels.findingsSchedule),
    text("dd", formatLabel(labels.findingsInterval, { hours })),
    text("dt", labels.findingsCap),
    text("dd", String(settings.object_cap)),
  ];
}

/**
 * How many distinct objects of each library have the W010 warning, in first-seen order. Pure.
 *
 * @param findings The open findings.
 * @returns Library ID to the number of its W010 objects.
 */
export function readOnlyCounts(findings: readonly FindingView[]): Map<string, number> {
  const objects = new Map<string, Set<string>>();
  for (const finding of findings) {
    if (finding.code !== NO_VERSION_INVENTORY) continue;
    const set = objects.get(finding.library_id) ?? new Set<string>();
    set.add(finding.object_id);
    objects.set(finding.library_id, set);
  }
  return new Map([...objects].map(([library, set]) => [library, set.size]));
}

/**
 * One note per library with W010 objects: their files can be read but not changed, with a
 * link to the UI-1.1 repair offer when the distribution gives its address.
 *
 * When the findings list is truncated the count is a lower bound and says "at least".
 *
 * @param findings The open findings.
 * @param labels The active label table.
 * @param truncated Whether the route listed fewer findings than are open.
 * @param repairHref The address of a library's repair offer, when the distribution has one.
 * @returns The note paragraphs.
 */
export function readOnlyNotes(
  findings: readonly FindingView[],
  labels: LabelTable,
  truncated: boolean,
  repairHref?: (libraryId: string) => string,
): HTMLParagraphElement[] {
  const template = truncated ? labels.findingsReadOnlyPartial : labels.findingsReadOnly;
  return [...readOnlyCounts(findings)].map(([library, count]) => {
    const note = text("p", formatLabel(template, { count: String(count), name: library }));
    note.dataset.note = "read-only";
    note.dataset.libraryId = library;
    if (!repairHref) return note;
    const link = text("a", labels.findingsRepair);
    link.className = "button ghost";
    link.href = repairHref(library);
    link.dataset.action = "repair";
    note.append(" ", link);
    return note;
  });
}

/**
 * The words for an answered Validate now: nothing open to validate when no library was
 * named, otherwise how many runs started and were not, followed by each refused library and
 * its error code. Pure.
 *
 * @param accepted The run route's answer.
 * @param labels The active label table.
 * @returns The status text.
 */
export function runSummary(accepted: RunAccepted, labels: LabelTable): string {
  if (accepted.started.length + accepted.refused.length === 0) return labels.findingsNoLibraries;
  const summary = formatLabel(labels.findingsRunResult, {
    started: String(accepted.started.length),
    refused: String(accepted.refused.length),
  });
  const refusals = accepted.refused.map((refusal) =>
    formatLabel(labels.findingsRunRefused, { name: refusal.library_id, code: refusal.error_code }),
  );
  return [summary, ...refusals].join(" ");
}

/** Create a table cell with text. */
function cell(value: string): HTMLTableCellElement {
  return text("td", value);
}

/** Create an element with text content. */
function text<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  value: string,
): HTMLElementTagNameMap[K] {
  const result = document.createElement(tag);
  result.textContent = value;
  return result;
}
