/**
 * The parts of the Logs view `<fs-logs>` (UI-F2.16; UI-S-021, UI-S-032, mockup
 * `resources/ui-mockups/v3/38-logs-view.svg`): its stylesheet, the head (title, intro, Live chip,
 * Download…), the filter row (search field and chips), the table with its heads and rows, the
 * placeholder rows and states, and the line under the table. They only build DOM from the label
 * table and the row views of `src/logs/model.ts`; every value is set with `textContent`, so
 * journal data never becomes markup. Colours, sizes and radii come from tokens only.
 */
import type { FilterChip, RowView } from "../logs/model";
import type { LabelTable } from "../theme/labels";
import { icon, type IconName } from "./icons";
import { stateButton } from "./property-table";

/**
 * The stylesheet of the view, added after the shared accessibility, control and property-table
 * stylesheets: the 30 px title, the 13 px intro, the 26 px Live chip and 32 px Download button,
 * the 32 px search field (206 px) and the 26 px chips of 1 px `--fs-outline-strong`, the table
 * (13 px 600 heads, 34 px rows over 1 px `--fs-outline` rules, mono 12 px cells, outcome chips of
 * 11 px 500 and 4 px radius), the selected row (`--fs-primary-container` with a 2.5 px
 * `--fs-primary` bar) and the 28 px Load older button.
 */
export const LOGS_STYLE = `
  :host { display: block; color: var(--fs-on-surface); font-family: var(--fs-font-sans); font-size: var(--fs-font-size-small); }
  .head { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-1) var(--fs-space-3); margin-block: calc(var(--fs-space-2) - 1px) var(--fs-space-3); min-block-size: calc(var(--fs-space-5) - var(--fs-space-1)); column-gap: calc(var(--fs-space-1) + var(--fs-space-2)); }
  .head h2 { margin: 0; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-page); font-weight: 700; line-height: 1.2; color: var(--fs-on-surface-heading); }
  .head .intro { flex: 1 1 calc(14 * var(--fs-space-3)); margin: 0; min-width: 0; color: var(--fs-on-surface-variant); }
  .head .button { min-block-size: calc(var(--fs-space-3) * 2); gap: var(--fs-space-1); padding-inline: var(--fs-space-3) calc(var(--fs-space-1) + var(--fs-space-2)); }
  .chip.live { box-sizing: border-box; gap: var(--fs-space-2); min-block-size: calc(var(--fs-space-4) + var(--fs-space-1) / 2); padding: 0 var(--fs-space-3) 0 calc(var(--fs-space-2) + var(--fs-space-1) / 2); border: 0; border-radius: var(--fs-radius-chip); font-family: inherit; font-size: var(--fs-font-size-small); cursor: default; }
  .chip.live.fresh { cursor: pointer; }
  .chip.live .dot { inline-size: calc(var(--fs-space-2) - 1px); block-size: calc(var(--fs-space-2) - 1px); border-radius: var(--fs-radius-pill); background: var(--fs-primary); }
  .filters { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-2) calc(var(--fs-space-1) + var(--fs-space-2)); margin-block-end: calc(var(--fs-space-1) + var(--fs-space-2) + 1px); }
  .search { position: relative; flex: 0 0 calc(12.875 * var(--fs-space-3)); }
  .search .icon { position: absolute; inset-inline-start: calc(var(--fs-space-2) + var(--fs-space-1) / 2); inset-block-start: 50%; translate: 0 -50%; color: var(--fs-icon-muted); pointer-events: none; }
  .search input { inline-size: 100%; min-block-size: calc(var(--fs-space-3) * 2); padding-inline: calc(var(--fs-space-3) * 2) var(--fs-space-4); font-size: var(--fs-font-size-small); }
  .search .clear { position: absolute; inset-inline-end: var(--fs-space-1); inset-block-start: 50%; translate: 0 -50%; display: inline-flex; padding: var(--fs-space-1); border: 0; border-radius: var(--fs-radius-item); background: none; color: var(--fs-icon-muted); cursor: pointer; }
  .chips { display: flex; flex-wrap: wrap; align-items: center; gap: calc(var(--fs-space-1) * 1.5); margin: 0; padding: 0; list-style: none; }
  .filter-chip { display: inline-flex; align-items: center; gap: var(--fs-space-2); box-sizing: border-box; min-block-size: calc(var(--fs-space-4) + var(--fs-space-1) / 2); padding: 0 calc(var(--fs-space-2) + var(--fs-space-1) / 2); border: 1px solid var(--fs-outline-strong); border-radius: var(--fs-radius-chip); background: var(--fs-surface-card); color: var(--fs-on-surface); font-family: inherit; font-size: var(--fs-font-size-small); font-weight: 500; line-height: 1.2; cursor: pointer; }
  .filter-chip:hover { background: var(--fs-surface-raised); }
  .filter-chip.range { padding-inline-start: var(--fs-space-2); }
  .filter-chip[data-active="true"] { border-color: var(--fs-primary); background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  .filter-chip .remove { display: inline-flex; color: inherit; }
  .hint { flex: 1 0 100%; margin: 0; color: var(--fs-on-surface-variant); }
  .hint.invalid { color: var(--fs-error); }
  .scroll { overflow-x: auto; margin-inline: calc(var(--fs-space-2) * -1); }
  table { inline-size: 100%; min-inline-size: calc(50.5 * var(--fs-space-3)); table-layout: fixed; border-collapse: collapse; }
  th { white-space: nowrap; block-size: calc(var(--fs-space-3) * 2); padding: 0 var(--fs-space-2); border-block-end: 1px solid var(--fs-outline); color: var(--fs-on-surface-variant); font-weight: 600; text-align: start; }
  td { box-sizing: border-box; block-size: calc(var(--fs-space-3) * 2 + 2px); padding: 0 var(--fs-space-2); border-block-end: 1px solid var(--fs-outline); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  td.mono, td .mono { font-family: var(--fs-font-mono); font-size: calc((var(--fs-font-size-small) + var(--fs-font-size-eyebrow)) / 2); }
  td.time { color: var(--fs-on-surface-variant); }
  td.level { font-weight: 600; color: var(--fs-on-surface-variant); }
  td.level.alert { color: var(--fs-error); }
  td.target { color: var(--fs-on-surface); }
  td.operation-id { color: var(--fs-on-surface-muted); }
  td.operation-id .journal-only { margin-inline-start: var(--fs-space-2); font-family: var(--fs-font-mono); font-size: var(--fs-font-size-eyebrow); font-weight: 500; font-style: italic; color: var(--fs-on-surface-muted); }
  td .chip { box-sizing: border-box; border-radius: var(--fs-radius-chip); line-height: 1.6; }
  td .none { color: var(--fs-on-surface-muted); }
  td.operation-id .none { font-family: var(--fs-font-mono); font-size: calc((var(--fs-font-size-small) + var(--fs-font-size-eyebrow)) / 2); }
  tbody tr { position: relative; cursor: pointer; }
  tbody tr:hover td { background: var(--fs-surface-raised); }
  tbody tr[aria-selected="true"] td { background: var(--fs-primary-container); }
  tbody tr td:first-child { position: relative; }
  tbody tr[aria-selected="true"] td:first-child::before { content: ""; position: absolute; inset-block: 0; inset-inline-start: 0; inline-size: calc(var(--fs-space-1) * 0.625); background: var(--fs-primary); }
  td button.select { display: block; max-inline-size: 100%; margin: 0; padding: 0; border: 0; background: none; color: inherit; font: inherit; text-align: start; cursor: pointer; }
  tbody.loading tr { cursor: default; }
  tbody.loading tr:hover td { background: none; }
  .bar { display: block; block-size: calc(var(--fs-space-2) + 1px); border-radius: var(--fs-radius-pill); background: var(--fs-outline); }
  .more { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: var(--fs-space-2) var(--fs-space-3); margin-block-start: calc(var(--fs-space-1) + var(--fs-space-2)); }
  .more p { margin: 0; color: var(--fs-on-surface-muted); }
  .more .button { min-block-size: calc(var(--fs-space-4) + var(--fs-space-1)); padding: 0 calc(var(--fs-space-1) + var(--fs-space-2)); }
  .more[hidden], .states[hidden], .scroll[hidden] { display: none; }
  .states .state { padding-block-start: var(--fs-space-5); }
  [hidden] { display: none !important; }
  .announce { position: absolute; inline-size: 1px; block-size: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
`;

/** Create an element with a class and optional text. */
export function node<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  element.className = className;
  if (text !== undefined) element.textContent = text;
  return element;
}

/** The head's controls, for the view to wire. */
export interface HeadParts {
  /** The head element. */
  readonly root: HTMLElement;
  /** The Live chip (a button). */
  readonly live: HTMLButtonElement;
  /** The Download… button. */
  readonly download: HTMLButtonElement;
}

/**
 * The head: the title, `logsIntro`, the Live chip and Download… (mockup 38).
 *
 * @param labels The active label table.
 */
export function buildHead(labels: LabelTable): HeadParts {
  const root = node("div", "head");
  const live = node("button", "chip ok live");
  live.type = "button";
  live.hidden = true;
  live.append(node("span", "dot"), node("span", "text", labels.logsLive));
  const download = node("button", "button secondary download");
  download.type = "button";
  download.append(icon("download", 16), document.createTextNode(labels.logsDownload));
  root.append(node("h2", "", labels.logs), node("p", "intro", labels.logsIntro), live, download);
  return { root, live, download };
}

/** The filter row's controls. */
export interface FilterParts {
  /** The row. */
  readonly root: HTMLElement;
  /** The search field. */
  readonly search: HTMLInputElement;
  /** The button inside the field that empties it. */
  readonly clear: HTMLButtonElement;
  /** The list the chips are drawn in. */
  readonly chips: HTMLElement;
  /** The line under the row: what the search matched, or why it matched nothing. */
  readonly hint: HTMLElement;
}

/**
 * The filter row: the search field with `logsSearchPlaceholder` and the chip list.
 *
 * @param labels The active label table.
 */
export function buildFilters(labels: LabelTable): FilterParts {
  const root = node("div", "filters");
  const field = node("form", "search");
  field.setAttribute("role", "search");
  const search = node("input", "input");
  search.type = "text";
  search.autocomplete = "off";
  search.placeholder = labels.logsSearchPlaceholder;
  search.setAttribute("aria-label", labels.logsSearchLabel);
  const clear = node("button", "clear");
  clear.type = "button";
  clear.hidden = true;
  clear.setAttribute("aria-label", labels.filePanelClearFilter);
  clear.append(icon("close", 16));
  field.append(icon("search", 16), search, clear);
  const chips = node("ul", "chips");
  const hint = node("p", "hint");
  hint.setAttribute("role", "status");
  hint.hidden = true;
  root.append(field, chips, hint);
  return { root, search, clear, chips, hint };
}

/**
 * One chip of the filter row: a button that opens the chip's menu, or, for a file, object or
 * operation ID filter, one that removes it.
 *
 * @param chip The chip.
 * @param labels The active label table.
 */
export function chipElement(chip: FilterChip, labels: LabelTable): HTMLLIElement {
  const item = document.createElement("li");
  const button = node("button", `filter-chip ${chip.id}`);
  button.type = "button";
  button.dataset.chip = chip.id;
  button.dataset.active = String(chip.active);
  if (chip.removable) {
    button.setAttribute("aria-label", labels.logsRemoveFilter.replace("{filter}", chip.text));
    button.append(document.createTextNode(chip.text), glyph("close", 14, "remove"));
  } else {
    button.setAttribute("aria-haspopup", "menu");
    button.setAttribute("aria-expanded", "false");
    if (chip.id === "range") button.append(icon("calendar", 16));
    button.append(document.createTextNode(chip.text));
  }
  item.append(button);
  return item;
}

/** An icon wrapped in a span with a class. */
function glyph(name: IconName, size: number, className: string): HTMLElement {
  const span = node("span", className);
  span.append(icon(name, size));
  return span;
}

/** The table's elements. */
export interface TableParts {
  /** The scrolling region that holds the table. */
  readonly scroll: HTMLElement;
  /** The body the rows are drawn in. */
  readonly body: HTMLTableSectionElement;
}

/** The widths of the columns, in rem (mockup 38: 80, 60, 144, 88, 244 px; the rest for the ID). */
const COLUMN_REM = [5, 3.75, 9, 5.5, 15.25] as const;

/**
 * The table with the six heads of mockup 38, in a focusable, named scroll region so a narrow
 * window scrolls the table, not the page (UI-S-016).
 *
 * @param labels The active label table.
 */
export function buildTable(labels: LabelTable): TableParts {
  const scroll = node("div", "scroll");
  scroll.tabIndex = 0;
  scroll.setAttribute("role", "region");
  scroll.setAttribute("aria-label", labels.logsTableLabel);
  const table = document.createElement("table");
  const columns = document.createElement("colgroup");
  for (const rem of COLUMN_REM) {
    const col = document.createElement("col");
    col.style.inlineSize = `${rem}rem`;
    columns.append(col);
  }
  columns.append(document.createElement("col"));
  const head = document.createElement("thead");
  const row = document.createElement("tr");
  const titles = [
    labels.logsColTime,
    labels.logsColLevel,
    labels.logsColOperation,
    labels.logsColOutcome,
    labels.logsColTarget,
    labels.logsColOperationId,
  ];
  for (const title of titles) {
    const th = document.createElement("th");
    th.scope = "col";
    th.textContent = title;
    row.append(th);
  }
  head.append(row);
  const body = document.createElement("tbody");
  table.append(columns, head, body);
  scroll.append(table);
  return { scroll, body };
}

/** The outcome cell: a chip, or the dash of an entry with no outcome. */
function outcomeCell(view: RowView): HTMLTableCellElement {
  const cell = document.createElement("td");
  if (view.outcome.tone === "none") {
    cell.append(node("span", "none", view.outcome.text));
    return cell;
  }
  const tone = view.outcome.tone === "alert" ? "alert" : view.outcome.tone;
  cell.append(node("span", `chip ${tone}`, view.outcome.text));
  return cell;
}

/** The operation ID cell: the short ID or a dash, and the journal-only tag. */
function operationIdCell(view: RowView, labels: LabelTable): HTMLTableCellElement {
  const cell = node("td", "operation-id mono");
  if (view.operationId === labels.logsNone) cell.append(node("span", "none", view.operationId));
  else cell.append(document.createTextNode(view.operationId));
  if (view.journalOnly) cell.append(node("span", "journal-only", labels.logsJournalOnly));
  return cell;
}

/** A cell with a class and text. */
function textCell(className: string, text: string): HTMLTableCellElement {
  return node("td", className, text);
}

/**
 * One row of the table. The time is a native button that selects the entry, named by its time,
 * operation and outcome, so the rows are reachable by keyboard and by screen reader; a click
 * anywhere on the row selects it too.
 *
 * @param view The row's cells.
 * @param labels The active label table.
 * @param selected Whether the entry is the one shown in the detail.
 */
export function rowElement(
  view: RowView,
  labels: LabelTable,
  selected: boolean,
): HTMLTableRowElement {
  const row = document.createElement("tr");
  row.dataset.key = view.key;
  row.setAttribute("aria-selected", String(selected));
  const time = node("td", "time mono");
  const select = node("button", "select", view.time);
  select.type = "button";
  select.tabIndex = -1;
  select.title = view.timeTitle;
  select.setAttribute("aria-label", `${view.time}, ${view.operation}, ${view.outcome.text}`);
  time.append(select);
  const level = textCell(`level mono${view.levelTone === "alert" ? " alert" : ""}`, view.level);
  row.append(
    time,
    level,
    textCell("operation mono", view.operation),
    outcomeCell(view),
    textCell("target", view.target),
    operationIdCell(view, labels),
  );
  return row;
}

/**
 * Placeholder rows for the loading state: bars of varying widths where the cells will be.
 *
 * @param count How many rows.
 */
export function placeholderRows(count: number): HTMLTableRowElement[] {
  return Array.from({ length: count }, (_, index) => {
    const row = document.createElement("tr");
    row.setAttribute("aria-hidden", "true");
    const cell = document.createElement("td");
    cell.colSpan = 6;
    const bar = node("span", "bar");
    bar.style.inlineSize = `${55 + ((index * 11) % 35)}%`;
    cell.append(bar);
    row.append(cell);
    return row;
  });
}

/** The words and icon of a state that replaces the table. */
export interface StateWords {
  /** The icon. */
  readonly glyph: IconName;
  /** The title. */
  readonly title: string;
  /** The detail lines. */
  readonly details: readonly string[];
  /** The contract error code, when the API answered one. */
  readonly code?: string;
  /** The error icon is drawn in the error colour. */
  readonly alerting?: boolean;
  /** A button under the words, with its handler. */
  readonly action?: { readonly text: string; readonly run: () => void; readonly icon?: IconName };
}

/**
 * A state of the view (empty, no match, denied, error), in the panel states' look and vocabulary
 * (`.state`, `.title`, `.detail`, `.code`, secondary button).
 *
 * @param words What it says.
 * @param role `alert` for an error, `status` otherwise.
 */
export function stateElement(words: StateWords, role: "alert" | "status"): HTMLElement {
  const state = node("div", "state");
  state.setAttribute("role", role);
  const glyphElement = icon(words.glyph, 24);
  if (words.alerting) glyphElement.classList.add("alerting");
  state.append(glyphElement, node("p", "title", words.title));
  for (const line of words.details) state.append(node("p", "detail", line));
  if (words.code) state.append(node("p", "code", words.code));
  if (words.action) {
    const button = stateButton("action", words.action.text, words.action.icon);
    button.addEventListener("click", words.action.run);
    state.append(button);
  }
  return state;
}

/** The line under the table and Load older. */
export interface MoreParts {
  /** The line. */
  readonly root: HTMLElement;
  /** "Showing …". */
  readonly text: HTMLElement;
  /** Load older. */
  readonly older: HTMLButtonElement;
}

/**
 * The line under the rows with Load older (mockup 38).
 *
 * @param labels The active label table.
 */
export function buildMore(labels: LabelTable): MoreParts {
  const root = node("div", "more");
  root.hidden = true;
  const text = node("p", "showing");
  const older = node("button", "button secondary older", labels.logsLoadOlder);
  older.type = "button";
  root.append(text, older);
  return { root, text, older };
}
