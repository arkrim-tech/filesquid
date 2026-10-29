/**
 * The shared property table of the file panel's tabs (UI-F2.3; UI-S-031 mockups 35 and 25;
 * UI-S-022 asks UI-F2.15 to reuse it for Description and Provenance): a filter field, a count
 * eyebrow, rows of a 13 px muted label over a 13 px value (mono for identifiers, codes and
 * digests) with a copy button, the panel states that replace the rows (loading placeholder rows
 * with a busy line, no match with Clear filter, error with its reason, "Nothing was changed.",
 * the code and Try again), and the footer line. One treatment for every tab: the filter field
 * stays in the layout while a state replaces the rows, disabled until rows arrive, and its word
 * is reset when a load starts (`loading()`) or the file is cleared (`clear()`), so every file's
 * rows start unfiltered; the word is kept only across `rows(...)` renders of the same load.
 *
 * {@link PropertyTable} builds the DOM once from the label table and the tab's own words and
 * is driven by its host element: `loading()`, `rows(...)`, `error(...)` and `clear()`. Copy is
 * the only side effect it performs itself (the clipboard), named so at the call site. Muted text
 * is drawn only on the card surface: a hovered row turns its label to the secondary colour,
 * because muted text on `--fs-surface-raised` has no listed exemption (SRS-UI-007, UI-S-015).
 *
 * A tab may extend the rows (UI-F2.4, the Description tab of mockup 28): a row's label can be
 * a mono property key in the owner or extracted tone (`PropertyRow.labelStyle`), a tab can
 * give each row a source chip (`rowChip`) and action buttons before copy (`rowActions`), an
 * element below the rows (`afterRows`, the Add property button), an empty state for a record
 * with no rows (`emptyState`), and leave the footer out.
 */
import { formatLabel, type LabelTable } from "../theme/labels";
import { ApiError } from "../api/client";
import type { PropertyRow } from "../files/details";
import { reasonLabel } from "./denial";
import { icon, type IconName } from "./icons";
import { failureState } from "./view-state";

/** Placeholder rows the loading state draws (mockup 25). */
const PLACEHOLDER_ROWS = 10;
/** Most rows the table draws. */
export const MAX_PROPERTY_ROWS = 500;

/** The words and behaviour one tab gives its table. */
export interface PropertyTableOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** The filter field's placeholder and name ("Filter file info"). */
  readonly filterPlaceholder: string;
  /** The busy line while loading ("Loading file info…"). */
  readonly loadingText: string;
  /** The error title ("Could not load the file info"). */
  readonly errorTitle: string;
  /** The footer line; no footer when absent. */
  readonly footer?: string;
  /** The count eyebrow for `shown` of `total` rows. */
  readonly countText: (shown: number, total: number) => string;
  /** The no-match title for the filter word. */
  readonly noMatchTitle: (word: string) => string;
  /** Called by Try again. */
  readonly onRetry: () => void;
  /** Whether the browser is online; `navigator.onLine` when absent. */
  readonly online?: () => boolean;
  /** Told each time what the table shows changes. */
  readonly onState?: (state: PropertyTableState) => void;
  /** A chip at a row's top right (the source of a value), when the tab has one. */
  readonly rowChip?: (row: PropertyRow) => RowChip | undefined;
  /** Action buttons shown before a row's copy button, when the tab has any. */
  readonly rowActions?: (row: PropertyRow) => HTMLButtonElement[];
  /** The element below the rows (the Add property button), built on each render. */
  readonly afterRows?: () => HTMLElement;
  /** The state shown for a record with no rows, built on each render; none when absent. */
  readonly emptyState?: () => HTMLElement;
}

/** A row's chip: its words and its tone. */
export interface RowChip {
  /** The chip's text, from the label table. */
  readonly text: string;
  /** `ok` on the active tint, `muted` on the raised surface. */
  readonly tone: "ok" | "muted";
}

/** What the table shows. */
export type PropertyTableState = "idle" | "loading" | "ready" | "no-match" | "error" | "empty";

/**
 * The stylesheet of the table and of the panel states (UI-S-024): the 32 px filter of mockup
 * 35 with its search icon, the count eyebrow, 44 px rows whose chip and action buttons sit in
 * the flow beside the key, and whose action buttons sit at the bottom right over space the
 * value reserves (`--fs-row-actions`, set per row), so a long key or value wraps instead of
 * running under them, the 16 px copy icon in `--fs-icon-muted` (`--fs-icon` on hover and
 * focus), the placeholder bars on `--fs-outline`,
 * the centred states (14 px 600 title, 13 px secondary detail, 13 px mono code, 36 px buttons)
 * and the 44 px footer line above a 1 px `--fs-outline` border. Content sits 20 px from the panel
 * edges: a 16 px padding and a 4 px inset.
 */
const STYLE = `
  .table { display: flex; flex-direction: column; flex: 1 1 auto; min-block-size: 0; }
  .filter-wrap { padding: var(--fs-space-3) var(--fs-space-3) 0; }
  .filter-field { position: relative; margin: 0 var(--fs-space-1); }
  .filter-field .icon { position: absolute; inset-inline-start: 10px; inset-block-start: 50%; translate: 0 -50%; color: var(--fs-icon-muted); pointer-events: none; }
  input.filter { display: block; inline-size: 100%; min-block-size: 2rem; padding-inline: var(--fs-space-4) var(--fs-space-5); text-indent: 8px; font-size: var(--fs-font-size-small); }
  input.filter:disabled { border-color: var(--fs-outline); background: var(--fs-surface-raised); }
  .clear-filter { position: absolute; inset-inline-end: 4px; inset-block-start: 50%; translate: 0 -50%; display: inline-flex; padding: var(--fs-space-1); border: 0; border-radius: var(--fs-radius-item); background: none; color: var(--fs-icon-muted); cursor: pointer; }
  .clear-filter .icon { position: static; translate: none; }
  .count { margin: var(--fs-space-3) var(--fs-space-3) var(--fs-space-1); padding: 0 var(--fs-space-1); line-height: 1.4; }
  .count.busy { font-weight: 500; text-transform: none; }
  .body { flex: 1 1 auto; min-block-size: 0; }
  dl.rows { margin: 0; padding: 0; }
  .row { --fs-row-actions: 0; position: relative; box-sizing: border-box; min-block-size: 2.75rem; padding: var(--fs-space-1) var(--fs-space-3); }
  .row::after { content: ""; position: absolute; inset-inline: 20px; inset-block-end: 0; border-block-end: 1px solid var(--fs-outline); }
  .row:hover, .row:focus-within { background: var(--fs-surface-raised); }
  .row dt, .row dd { margin: 0 var(--fs-space-1); font-size: var(--fs-font-size-small); line-height: 1.3; overflow-wrap: anywhere; }
  .row dt { display: flex; align-items: start; justify-content: space-between; gap: var(--fs-space-2); color: var(--fs-on-surface-muted); }
  .row dt .label, .row dt .key { flex: 1 1 auto; min-width: 0; }
  .row dd .value { display: block; padding-inline-end: var(--fs-row-actions); }
  .row:hover dt, .row:focus-within dt { color: var(--fs-on-surface-variant); }
  .row dd { color: var(--fs-on-surface); }
  .row dt .key { font-family: var(--fs-font-mono); font-weight: 500; }
  .row dt .key-owner, .row:hover dt .key-owner, .row:focus-within dt .key-owner { color: var(--fs-primary); }
  .row dt .key-extracted, .row:hover dt .key-extracted, .row:focus-within dt .key-extracted { color: var(--fs-on-surface-variant); }
  .row dt .chip.source { flex: 0 0 auto; align-self: start; position: relative; inset-block-start: 2px; border-radius: var(--fs-radius-chip); line-height: 1.6; }
  .mono { font-family: var(--fs-font-mono); }
  .actions { position: absolute; inset-inline-end: 20px; inset-block-end: 0; display: inline-flex; align-items: center; gap: 0; }
  .actions .button-icon { opacity: 0; transition: opacity var(--fs-motion-duration); }
  .row:hover .actions .button-icon, .row:focus-within .actions .button-icon, .actions .button-icon:focus-visible { opacity: 1; }
  .button-icon, .copy { display: inline-flex; align-items: center; justify-content: center; inline-size: 1.5rem; block-size: 1.5rem; padding: 0; border: 0; border-radius: var(--fs-radius-item); background: none; color: var(--fs-icon-muted); cursor: pointer; }
  .button-icon:hover, .button-icon:focus-visible, .copy:hover, .copy:focus-visible, .row:hover .copy { color: var(--fs-icon); }
  .body > .after-rows { margin: var(--fs-space-2) var(--fs-space-3) var(--fs-space-3); padding: 0 var(--fs-space-1); }
  .state.empty .button { margin-block-start: var(--fs-space-3); }
  .placeholder { display: grid; gap: var(--fs-space-2); box-sizing: border-box; min-block-size: 2.75rem; margin: 0 var(--fs-space-3); padding: var(--fs-space-2) var(--fs-space-1); border-block-end: 1px solid var(--fs-outline); }
  .bar { display: block; block-size: 0.5625rem; border-radius: var(--fs-radius-pill); background: var(--fs-outline); }
  .bar.wide { block-size: 0.6875rem; }
  .state { display: flex; flex-direction: column; align-items: center; gap: var(--fs-space-1); padding: var(--fs-space-6) var(--fs-space-4) var(--fs-space-4); text-align: center; }
  .state .icon { margin-block-end: var(--fs-space-3); color: var(--fs-icon-muted); }
  .state .icon.alerting { color: var(--fs-error); }
  .state p { margin: 0; overflow-wrap: anywhere; }
  .state .title { font-size: var(--fs-font-size-ui); font-weight: 600; color: var(--fs-on-surface); }
  .state .detail { font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .state .code { font-family: var(--fs-font-mono); font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .state .button { margin-block-start: var(--fs-space-3); }
  .footer { margin: 0; box-sizing: border-box; min-block-size: 2.75rem; padding: var(--fs-space-3) var(--fs-space-4) var(--fs-space-2) var(--fs-space-4); border-block-start: 1px solid var(--fs-outline); color: var(--fs-on-surface-muted); font-size: var(--fs-font-size-small); }
  .announce { position: absolute; inline-size: 1px; block-size: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
`;

/** CSS text of the property table and panel states; hosts add it after `CONTROL_STYLE`. */
export const PROPERTY_TABLE_STYLE = STYLE;

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

/** A secondary button with an optional leading icon. */
export function stateButton(className: string, text: string, glyph?: IconName): HTMLButtonElement {
  const button = node("button", `button secondary ${className}`);
  button.type = "button";
  if (glyph) button.append(icon(glyph));
  button.append(document.createTextNode(text));
  return button;
}

/** The words a failed read gets: its reason and, for an API refusal, its code. */
export interface FailureWords {
  /** Why it failed, from the label table. */
  readonly reason: string;
  /** The contract error code, when the API answered one. */
  readonly code?: string;
}

/**
 * Describe a failed read in the active labels: a denial by its reason, a 503 as unavailable,
 * an offline browser as offline, anything else as an error, with the API's code. Pure.
 *
 * @param error What the read rejected with.
 * @param labels The active label table.
 * @param online Whether the browser reports a connection.
 * @returns The reason and the code.
 */
export function failureWords(error: unknown, labels: LabelTable, online: boolean): FailureWords {
  const state = failureState(error, online);
  const code = error instanceof ApiError ? error.code : undefined;
  if (state.kind === "denied") {
    return { reason: state.reason ? reasonLabel(state.reason, labels) : labels.stateDenied, code };
  }
  if (state.kind === "unavailable") return { reason: labels.stateUnavailable, code };
  if (state.kind === "offline") return { reason: labels.stateOffline, code };
  return { reason: labels.stateError, code };
}

/**
 * The error state of a panel (mockup 25): the error icon in `--fs-error`, the title, the
 * reason, "Nothing was changed.", the code and Try again, in an alert region.
 *
 * @param title The panel's error title.
 * @param words The reason and code.
 * @param labels The active label table.
 * @param retry Called by Try again.
 * @returns The state element.
 */
export function errorState(
  title: string,
  words: FailureWords,
  labels: LabelTable,
  retry: () => void,
): HTMLElement {
  const state = node("div", "state");
  state.setAttribute("role", "alert");
  const glyph = icon("error", 24);
  glyph.classList.add("alerting");
  state.append(glyph, node("p", "title", title), node("p", "detail", words.reason));
  state.append(node("p", "detail", labels.filePanelNothingChanged));
  if (words.code) state.append(node("p", "code", words.code));
  const button = stateButton("retry", labels.stateRetry, "refresh");
  button.addEventListener("click", retry);
  state.append(button);
  return state;
}

/** True when a row's label, value or copy text contains `word` (case-insensitive). */
function matches(row: PropertyRow, word: string): boolean {
  const needle = word.toLocaleLowerCase();
  return [row.label, row.value, row.copy ?? ""].some((text) =>
    text.toLocaleLowerCase().includes(needle),
  );
}

/** The table of one tab. */
export class PropertyTable {
  /** The table's root, to append to the host's shadow root. */
  readonly element: HTMLElement;
  private readonly filter: HTMLInputElement;
  private readonly clearButton: HTMLButtonElement;
  private readonly count: HTMLElement;
  private readonly body: HTMLElement;
  private readonly announcer: HTMLElement;
  private all: PropertyRow[] = [];
  private current: PropertyTableState = "idle";

  /** @param options The tab's words and Try again. */
  constructor(private readonly options: PropertyTableOptions) {
    this.element = node("div", "table");
    this.filter = this.buildFilter();
    this.clearButton = this.buildClear();
    const wrap = node("div", "filter-wrap");
    const field = node("div", "filter-field");
    field.append(icon("search", 16), this.filter, this.clearButton);
    wrap.append(field);
    this.count = node("p", "count eyebrow");
    this.body = node("div", "body");
    this.announcer = node("p", "announce");
    this.announcer.setAttribute("role", "status");
    this.element.append(wrap, this.count, this.body, this.announcer);
    if (options.footer !== undefined) this.element.append(node("p", "footer", options.footer));
    this.clear();
  }

  /** What the table shows now. */
  get state(): PropertyTableState {
    return this.current;
  }

  /** Show nothing: no file. The filter is emptied, so the next file starts unfiltered. */
  clear(): void {
    this.resetFilter();
    this.all = [];
    this.count.classList.remove("busy");
    this.count.textContent = "";
    this.body.removeAttribute("aria-busy");
    this.body.replaceChildren();
    this.filter.disabled = true;
    this.setState("idle");
  }

  /**
   * Show the busy line and placeholder rows. The filter is emptied (reset on load) and disabled
   * until rows arrive, so the rows of this load start unfiltered.
   */
  loading(): void {
    this.resetFilter();
    this.setState("loading");
    this.filter.disabled = true;
    this.count.textContent = this.options.loadingText;
    this.count.classList.add("busy");
    const bars = Array.from({ length: PLACEHOLDER_ROWS }, (_, i) => placeholder(i));
    this.body.replaceChildren(...bars);
    this.body.setAttribute("aria-busy", "true");
  }

  /** Show `rows`, filtered by the word typed since this load started. */
  rows(rows: readonly PropertyRow[]): void {
    this.all = rows.slice(0, MAX_PROPERTY_ROWS);
    this.filter.disabled = false;
    this.render();
  }

  /** Show the error state for a failed read; the filter is disabled, and Try again resets it. */
  error(error: unknown): void {
    this.setState("error");
    this.filter.disabled = true;
    this.clearButton.hidden = true;
    this.count.classList.remove("busy");
    this.count.textContent = "";
    this.body.removeAttribute("aria-busy");
    const online = this.options.online ? this.options.online() : navigator.onLine;
    const words = failureWords(error, this.options.labels, online);
    this.body.replaceChildren(
      errorState(this.options.errorTitle, words, this.options.labels, this.options.onRetry),
    );
  }

  /** Draw the rows that match the filter, or the no-match state. */
  private render(): void {
    const word = this.filter.value.trim();
    const shown = word === "" ? this.all : this.all.filter((row) => matches(row, word));
    this.clearButton.hidden = this.filter.value === "";
    this.count.classList.remove("busy");
    this.body.removeAttribute("aria-busy");
    this.count.textContent = this.options.countText(shown.length, this.all.length);
    const empty = this.all.length === 0 ? this.options.emptyState?.() : undefined;
    if (empty) {
      empty.classList.add("empty");
      this.filter.disabled = true;
      this.body.replaceChildren(empty);
      this.setState("empty");
      return;
    }
    if (shown.length === 0 && word !== "") {
      this.body.replaceChildren(this.noMatch(word));
      this.setState("no-match");
      return;
    }
    const list = node("dl", "rows");
    list.append(...shown.map((row) => this.row(row)));
    const after = this.options.afterRows?.();
    if (after) after.classList.add("after-rows");
    this.body.replaceChildren(list, ...(after ? [after] : []));
    this.setState("ready");
  }

  /** Record what the table shows and tell the host. */
  private setState(state: PropertyTableState): void {
    this.current = state;
    this.options.onState?.(state);
  }

  /** One row: the label (and chip) over the value, with its actions and copy button. */
  private row(row: PropertyRow): HTMLElement {
    const element = node("div", "row");
    element.dataset.key = row.key;
    const value = node("span", row.mono ? "value mono" : "value", row.value);
    const actions = node("span", "actions");
    for (const action of this.options.rowActions?.(row) ?? []) {
      action.classList.add("button-icon");
      actions.append(action);
    }
    actions.append(this.copyButton(row));
    // The value keeps clear of the buttons: 1.5rem each, set as a token-named property.
    element.style.setProperty("--fs-row-actions", `${actions.childElementCount * 1.5}rem`);
    const dd = node("dd", "");
    dd.append(value, actions);
    element.append(this.term(row), dd);
    return element;
  }

  /** The row's term: a plain label, or a mono key in its tone, and the chip when there is one. */
  private term(row: PropertyRow): HTMLElement {
    const dt = node("dt", "");
    const label = node("span", row.labelStyle ? `key key-${row.labelStyle}` : "label", row.label);
    dt.append(label);
    const chip = this.options.rowChip?.(row);
    if (chip) dt.append(node("span", `chip source ${chip.tone}`, chip.text));
    return dt;
  }

  /** The row's copy button, named by its property. */
  private copyButton(row: PropertyRow): HTMLButtonElement {
    const copy = node("button", "copy");
    copy.type = "button";
    copy.setAttribute(
      "aria-label",
      formatLabel(this.options.labels.filePanelCopy, { property: row.label }),
    );
    copy.append(icon("copy", 16));
    copy.addEventListener("click", () => void this.copyToClipboard(row));
    return copy;
  }

  /** Put a row's whole value on the clipboard and announce it. Side effect: the clipboard. */
  private async copyToClipboard(row: PropertyRow): Promise<void> {
    this.announcer.textContent = "";
    try {
      await navigator.clipboard.writeText(row.copy ?? row.value);
      this.announcer.textContent = formatLabel(this.options.labels.filePanelCopied, {
        property: row.label,
      });
    } catch (error) {
      console.warn("FileSquid could not copy a value", error instanceof Error ? error.name : error);
    }
  }

  /** The no-match state: the title with the word, the hint and Clear filter. */
  private noMatch(word: string): HTMLElement {
    const labels = this.options.labels;
    const state = node("div", "state");
    state.setAttribute("role", "status");
    const clear = stateButton("clear", labels.filePanelClearFilter);
    clear.addEventListener("click", () => this.clearFilter());
    state.append(
      icon("search", 24),
      node("p", "title", this.options.noMatchTitle(word)),
      node("p", "detail", labels.filePanelNoMatchHint),
      clear,
    );
    return state;
  }

  /** Empty the filter field and hide its clear button, without drawing anything. */
  private resetFilter(): void {
    this.filter.value = "";
    this.clearButton.hidden = true;
  }

  /** Empty the filter, show every row and return focus to the field; only over rows. */
  private clearFilter(): void {
    if (this.current !== "ready" && this.current !== "no-match") return;
    this.filter.value = "";
    this.render();
    this.filter.focus();
  }

  /** The filter field. */
  private buildFilter(): HTMLInputElement {
    const input = node("input", "input filter");
    input.type = "text";
    input.autocomplete = "off";
    input.placeholder = this.options.filterPlaceholder;
    input.setAttribute("aria-label", this.options.filterPlaceholder);
    input.addEventListener("input", () => this.render());
    return input;
  }

  /** The button inside the field that empties it. */
  private buildClear(): HTMLButtonElement {
    const button = node("button", "clear-filter");
    button.type = "button";
    button.hidden = true;
    button.setAttribute("aria-label", this.options.labels.filePanelClearFilter);
    button.append(icon("close", 16));
    button.addEventListener("click", () => this.clearFilter());
    return button;
  }
}

/** One placeholder row: a short and a long bar, widths varying by row (mockup 25). */
function placeholder(index: number): HTMLElement {
  const row = node("div", "placeholder");
  row.setAttribute("aria-hidden", "true");
  const short = node("span", "bar");
  const long = node("span", "bar wide");
  short.style.inlineSize = `${25 + ((index * 7) % 20)}%`;
  long.style.inlineSize = `${55 + ((index * 11) % 30)}%`;
  row.append(short, long);
  return row;
}
