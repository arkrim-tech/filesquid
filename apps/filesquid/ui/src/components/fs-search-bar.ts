/**
 * `<fs-search-bar>`: the global search bar of the header (UI-F2.9, F-063, UI-S-007, UI-S-030;
 * mockup `resources/ui-mockups/v3/20-search-suggestions.svg`). One field holds applied chips
 * (Type, Location, Modified and Title) before the text, with a remove control on each chip and
 * Clear all at the right; it is 520 x 36 px and widens to 620 px while focused or holding a
 * query. While it has focus a 620 x 520 px drop-down shows FILTERS (the Type, Location and
 * Modified chips, each opening its choices), SUGGESTIONS typed from the text (`in:` the folder
 * shown, `type:` a class, `title:` the text), FILES (the first three matches of a live search,
 * asked 250 ms after the last key and dropped when a newer one has begun) and OPERATORS (the ones
 * this device answers), and the hint that Enter shows all results. The People chip is left out:
 * the desktop MVP has no accounts.
 *
 * The bar never navigates and never fetches by itself: it asks a {@link SearchSource} for the live
 * files and a {@link NavigationSource} for the Location choices, and on Enter dispatches
 * `fs-search` (`detail.q`, the query chips and text make), which the route turns into the results
 * address. A file chosen in the drop-down dispatches `fs-drive-open` as a Drive row does. The
 * route sets `query` (chips and text come back from the address) and `folder` (the collection
 * shown, for the `in:` suggestion).
 *
 * Accessibility (UI-S-016): the field is a `combobox` over a `listbox` of options; the arrow keys
 * move the active option (`aria-activedescendant`), Enter applies it (or searches when none is
 * active), Escape closes the drop-down, Backspace on empty text removes the last chip; the remove
 * controls and Clear all are buttons with names from the label table; the focus ring is the
 * 2 px `--fs-focus-ring` border of the field.
 */
import { ApiError } from "../api/client";
import type { DriveRow } from "../drive/model";
import type { LibraryScope } from "../navigation/library-scope";
import type { NavigationPage, NavigationSource } from "../navigation/model";
import {
  FILTER_LABEL,
  MAX_LOCATIONS,
  MAX_LOCATION_PAGES,
  MAX_QUERY_BYTES,
  MODIFIED_RANGES,
  OPERATORS_ON_HOST,
  RANGE_LABEL,
  SUGGESTED_FILES,
  SUGGEST_DEBOUNCE_MS,
  TYPE_CLASSES,
  addChip,
  chipLabel,
  composeQuery,
  modifiedAfter,
  parseQuery,
  removeChip,
  typeSuggestions,
  zoneOf,
  type ChipFilter,
  type SearchChip,
} from "../search/model";
import type { SearchSource } from "../search/source";
import { formatLabel, type LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { icon } from "./icons";
import {
  STYLE as DROPDOWN_STYLE,
  optionsOf,
  renderDropdown,
  type DropdownModel,
  type DropdownOption,
  type FilterOption,
  type SuggestionOption,
} from "./search-dropdown";

/** What the bar needs. */
export interface SearchBarOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** Reads the live FILES suggestions. */
  readonly search: SearchSource;
  /** The navigation listing the Location choices come from; without it Location lists nothing. */
  readonly navigation?: NavigationSource;
  /** The library a search runs in; resolves to undefined when the host lists none. */
  readonly scope: () => Promise<LibraryScope | undefined>;
  /** The time now, for the Modified ranges and relative times; defaults to the clock. */
  readonly now?: () => Date;
  /** Milliseconds to wait after a key before asking; defaults to 250. */
  readonly debounceMs?: number;
}

/** The collection the route shows: the `in:` suggestion limits a search to it. */
export interface SearchFolder {
  /** Its node handle. */
  readonly node: string;
  /** Its name. */
  readonly name: string;
}

const STYLE = `
  :host { position: relative; display: block; box-sizing: border-box; inline-size: 32.5rem; max-inline-size: 100%; min-width: 0; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); color: var(--fs-on-surface); }
  :host([expanded]) { inline-size: 38.75rem; }
  .frame { display: flex; align-items: center; gap: var(--fs-space-2); box-sizing: border-box; block-size: 2.25rem; min-width: 0; padding: 0 var(--fs-space-2); border: 1.5px solid var(--fs-outline-control); border-radius: var(--fs-radius-input); background: var(--fs-surface-card); transition: border-color var(--fs-motion-duration); }
  .frame:focus-within { border: 2px solid var(--fs-focus-ring); }
  .frame > .icon { margin-inline-end: var(--fs-space-1); color: var(--fs-icon-muted); }
  .chips { display: flex; flex: 0 1 auto; gap: var(--fs-space-1); min-width: 0; overflow: hidden; }
  .chips:empty { display: none; }
  .chip { display: inline-flex; flex: 0 0 auto; align-items: center; box-sizing: border-box; block-size: 1.5rem; max-inline-size: 14rem; padding: 0 var(--fs-space-2); border-radius: var(--fs-radius-chip); background: var(--fs-primary-container); color: var(--fs-on-primary-container); font-size: var(--fs-font-size-small); font-weight: 500; white-space: nowrap; }
  .chip-label { overflow: hidden; text-overflow: ellipsis; }
  .chip-remove { position: relative; display: inline-flex; align-items: center; justify-content: center; padding: 0; border: 0; background: none; color: inherit; cursor: pointer; }
  .chip-remove::after { content: ""; position: absolute; inset: -0.375rem; }
  input.text { flex: 1 1 4rem; min-width: 3rem; padding: 0; border: 0; outline: none; background: none; color: var(--fs-on-surface); font-family: inherit; font-size: var(--fs-font-size-ui); font-weight: 400; }
  input.text::placeholder { color: var(--fs-on-surface-placeholder); opacity: 1; }
  .clear-all { flex: 0 0 auto; margin-inline-end: var(--fs-space-1); padding: 0; border: 0; background: none; color: var(--fs-primary); font-family: inherit; font-size: var(--fs-font-size-ui); font-weight: 600; cursor: pointer; }
  [hidden] { display: none !important; }
`;

/** Prefix of the option ids, so the input can name the active one. */
const OPTION_ID = "opt-";

/** Custom element that is the global search bar. */
export class FsSearchBar extends HTMLElement {
  private options?: SearchBarOptions;
  private chips: SearchChip[] = [];
  private input?: HTMLInputElement;
  private opened = false;
  private choice?: FilterOption["filter"];
  private locations: string[] = [];
  private found: DriveRow[] = [];
  private suggestedFor?: string;
  private shown: DropdownOption[] = [];
  private active = -1;
  private place?: SearchFolder;
  private generation = 0;
  private abort?: AbortController;
  private timer?: number;

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the labels, sources and scope and build the field. Call before or after connecting.
   *
   * @param options Labels, the search and navigation sources, the scope and the clock.
   */
  configure(options: SearchBarOptions): void {
    this.options = options;
    this.build();
  }

  /** The query the chips and the text make now; empty when there is neither. */
  get query(): string {
    return composeQuery(this.chips, this.input?.value ?? "", this.clock());
  }

  /**
   * Show a query as chips and text (the route's address). The same query again changes nothing,
   * so the caret stays where the user left it.
   */
  set query(query: string) {
    if (!this.input || query === this.query) return;
    const parsed = parseQuery(query, this.clock());
    this.chips = parsed.chips;
    this.input.value = parsed.text;
    this.choice = undefined;
    this.found = [];
    this.suggestedFor = undefined;
    this.renderField();
    this.renderDropdown();
  }

  /** The collection shown, or undefined. */
  get folder(): SearchFolder | undefined {
    return this.place;
  }

  /** Tell the bar which collection is shown, for its `in:` suggestion; undefined for none. */
  set folder(place: SearchFolder | undefined) {
    this.place = place;
    if (this.opened) this.renderDropdown();
  }

  /** Stop the debounce timer and any live read. */
  disconnectedCallback(): void {
    window.clearTimeout(this.timer);
    this.abort?.abort();
  }

  /** The time now. */
  private clock(): Date {
    return (this.options?.now ?? (() => new Date()))();
  }

  /** Build the field and the drop-down and wire their events. */
  private build(): void {
    const options = this.options as SearchBarOptions;
    const { labels } = options;
    const root = this.shadowRoot as ShadowRoot;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + CONTROL_STYLE + STYLE + DROPDOWN_STYLE;
    const field = document.createElement("div");
    field.className = "frame";
    const chips = document.createElement("div");
    chips.className = "chips";
    const input = document.createElement("input");
    input.type = "text";
    input.className = "text";
    input.placeholder = labels.searchPlaceholder;
    input.autocomplete = "off";
    input.maxLength = MAX_QUERY_BYTES;
    this.input = input;
    this.setComboboxAttributes(input, labels);
    const clear = document.createElement("button");
    clear.type = "button";
    clear.className = "clear-all";
    clear.textContent = labels.searchClearAll;
    clear.hidden = true;
    field.append(icon("search"), chips, input, clear);
    const dropdown = document.createElement("div");
    dropdown.className = "dropdown";
    dropdown.id = `${OPTION_ID}list`;
    dropdown.setAttribute("role", "listbox");
    dropdown.setAttribute("aria-label", labels.searchSuggestions);
    dropdown.hidden = true;
    root.replaceChildren(style, field, dropdown);
    this.wire(field, dropdown, input, clear);
    this.renderField();
  }

  /** The combobox roles and names of the field. */
  private setComboboxAttributes(input: HTMLInputElement, labels: LabelTable): void {
    input.setAttribute("role", "combobox");
    input.setAttribute("aria-label", labels.searchPlaceholder);
    input.setAttribute("aria-autocomplete", "list");
    input.setAttribute("aria-haspopup", "listbox");
    input.setAttribute("aria-expanded", "false");
    input.setAttribute("aria-controls", `${OPTION_ID}list`);
  }

  /** Listen to the field and the drop-down. */
  private wire(
    field: HTMLElement,
    dropdown: HTMLElement,
    input: HTMLInputElement,
    clear: HTMLButtonElement,
  ): void {
    const root = this.shadowRoot as ShadowRoot;
    input.addEventListener("input", () => this.onInput());
    input.addEventListener("keydown", (event) => this.onKeydown(event));
    clear.addEventListener("click", () => this.clearAll());
    field.addEventListener("click", (event) => this.onFieldClick(event));
    root.addEventListener("focusin", () => this.open());
    root.addEventListener("focusout", (event) => this.onFocusout(event));
    // Pressing in the drop-down must not move focus out of the field, or it would close first.
    dropdown.addEventListener("pointerdown", (event) => event.preventDefault());
    dropdown.addEventListener("mousedown", (event) => event.preventDefault());
    dropdown.addEventListener("click", (event) => this.onDropdownClick(event));
  }

  /** Draw the applied chips, Clear all and the width of the field. */
  private renderField(): void {
    const options = this.options as SearchBarOptions;
    const holder = this.shadowRoot?.querySelector<HTMLElement>(".chips");
    const clear = this.shadowRoot?.querySelector<HTMLElement>(".clear-all");
    if (!holder || !clear || !this.input) return;
    holder.replaceChildren(...this.chips.map((chip, at) => this.chipNode(chip, at, options)));
    const something = this.chips.length > 0 || this.input.value !== "";
    clear.hidden = !something;
    this.toggleAttribute("expanded", this.opened || something);
  }

  /** One applied chip with its remove control. */
  private chipNode(chip: SearchChip, at: number, options: SearchBarOptions): HTMLElement {
    const { labels } = options;
    const text = chipLabel(chip, labels);
    const out = document.createElement("span");
    out.className = "chip";
    const label = document.createElement("span");
    label.className = "chip-label";
    label.textContent = text;
    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "chip-remove";
    remove.setAttribute("aria-label", formatLabel(labels.searchRemoveChip, { chip: text }));
    remove.append(icon("close", 12));
    remove.addEventListener("click", () => this.removeAt(at));
    out.append(label, remove);
    return out;
  }

  /** Remove the chip at `at`, keep the focus in the field and ask again. */
  private removeAt(at: number): void {
    this.chips = removeChip(this.chips, at);
    this.afterChange();
  }

  /** Clear every chip and the text; no search runs. */
  private clearAll(): void {
    this.chips = [];
    if (this.input) this.input.value = "";
    this.choice = undefined;
    this.afterChange();
  }

  /** The chips or the text changed: redraw, ask for new live files, keep the focus. */
  private afterChange(): void {
    this.active = -1;
    this.renderField();
    this.renderDropdown();
    this.scheduleSuggest();
    this.input?.focus();
  }

  /** A click on the field's blank part focuses the text. */
  private onFieldClick(event: Event): void {
    const target = event.target;
    if (target instanceof Element && target.closest("button")) return;
    this.input?.focus();
  }

  /** Focus left the bar: close the drop-down, unless it moved to something inside the bar. */
  private onFocusout(event: Event): void {
    const next = (event as FocusEvent).relatedTarget;
    if (next instanceof Node && this.shadowRoot?.contains(next)) return;
    this.close();
  }

  /** Open the drop-down. */
  private open(): void {
    if (this.opened) return;
    this.opened = true;
    this.renderField();
    this.renderDropdown();
    this.scheduleSuggest();
  }

  /** Close the drop-down; the text and the chips stay. */
  private close(): void {
    this.opened = false;
    this.active = -1;
    this.choice = undefined;
    this.renderField();
    this.renderDropdown();
  }

  /** The text changed. */
  private onInput(): void {
    this.active = -1;
    this.choice = undefined;
    this.opened = true;
    this.renderField();
    this.renderDropdown();
    this.scheduleSuggest();
  }

  /** The keys of the combobox. */
  private onKeydown(event: KeyboardEvent): void {
    if (event.key === "ArrowDown") return this.move(event, 1);
    if (event.key === "ArrowUp") return this.move(event, -1);
    if (event.key === "Enter") return this.onEnter(event);
    if (event.key === "Escape" && this.opened) {
      event.preventDefault();
      return this.close();
    }
    if (event.key === "Backspace" && this.input?.value === "" && this.chips.length > 0) {
      this.removeAt(this.chips.length - 1);
    }
  }

  /** Move the active option; the drop-down opens first when it is closed. */
  private move(event: KeyboardEvent, step: number): void {
    event.preventDefault();
    if (!this.opened) this.open();
    this.active = Math.max(-1, Math.min(this.shown.length - 1, this.active + step));
    this.markActive();
  }

  /** Enter applies the active option, or runs the search when none is active. */
  private onEnter(event: KeyboardEvent): void {
    event.preventDefault();
    if (this.opened && this.active >= 0) return this.run(this.active);
    this.submit();
  }

  /** A click in the drop-down runs the option under it. */
  private onDropdownClick(event: Event): void {
    const target = event.target instanceof Element ? event.target : null;
    const index = target?.closest<HTMLElement>("[data-index]")?.dataset.index;
    if (index !== undefined) this.run(Number(index));
  }

  /** Run the option at `index`. */
  private run(index: number): void {
    const option = this.shown[index];
    if (!option) return;
    if (option.kind === "filter") return this.chooseFilter(option.filter);
    if (option.kind === "suggestion") return this.applySuggestion(option);
    if (option.kind === "file") return this.openFile(option.row);
    this.insertOperator(option.text);
  }

  /** Show a filter's choices in place of the typed suggestions, or hide them again. */
  private chooseFilter(filter: FilterOption["filter"]): void {
    this.choice = this.choice === filter ? undefined : filter;
    this.active = -1;
    this.renderDropdown();
    if (this.choice === "in") void this.loadLocations();
  }

  /**
   * Read the library's collections for the Location choices. It follows the listing's cursor, at
   * most {@link MAX_LOCATION_PAGES} pages of {@link MAX_LOCATIONS}, and logs a failure. Network I/O.
   */
  private async loadLocations(): Promise<void> {
    const options = this.options as SearchBarOptions;
    try {
      const scope = await options.scope();
      if (!scope || !options.navigation) return;
      this.locations = await this.readCollections(options.navigation, scope.node);
    } catch (error) {
      console.warn("FileSquid could not list the collections for the Location filter", error);
      this.locations = [];
    }
    if (this.choice === "in") this.renderDropdown();
  }

  /** The titles of a library's collections, following the listing's cursor, at most 10 pages. */
  private async readCollections(navigation: NavigationSource, node: string): Promise<string[]> {
    const names: string[] = [];
    let cursor: string | null = null;
    for (let pages = 0; pages < MAX_LOCATION_PAGES; pages += 1) {
      const page: NavigationPage = await navigation.list(node, cursor, MAX_LOCATIONS);
      for (const child of page.nodes) {
        if (child.kind === "collection" && child.name !== null) names.push(child.name);
      }
      cursor = page.next_cursor;
      if (cursor === null) break;
    }
    return names;
  }

  /** Apply a suggestion as a chip. A Title chip takes the typed text. */
  private applySuggestion(option: SuggestionOption): void {
    this.chips = addChip(this.chips, { filter: option.filter, value: option.chip });
    if (option.filter === "title" && this.input) this.input.value = "";
    this.choice = undefined;
    this.afterChange();
  }

  /** Open a file from the drop-down as a Drive row opens. */
  private openFile(row: DriveRow): void {
    this.close();
    this.dispatchEvent(
      new CustomEvent("fs-drive-open", {
        detail: { row, intent: "open" },
        bubbles: true,
        composed: true,
      }),
    );
  }

  /** Put an operator into the text, ready for its value; focus stays in the field. */
  private insertOperator(text: string): void {
    const input = this.input;
    if (!input) return;
    const gap = input.value === "" || input.value.endsWith(" ") ? "" : " ";
    const phrase = text === OPERATORS_ON_HOST[0];
    input.value = `${input.value}${gap}${phrase ? '""' : text}`;
    input.focus();
    if (phrase) input.setSelectionRange(input.value.length - 1, input.value.length - 1);
    this.onInput();
  }

  /** Run the search: close the drop-down and tell the route the query. An empty query is not run. */
  private submit(): void {
    const q = this.query;
    if (q === "") return;
    this.close();
    this.dispatchEvent(
      new CustomEvent("fs-search", { detail: { q }, bubbles: true, composed: true }),
    );
  }

  /** Ask for live files after the debounce, unless the answer for this query is already shown. */
  private scheduleSuggest(): void {
    window.clearTimeout(this.timer);
    const q = this.query;
    if (!this.opened || q === this.suggestedFor) return;
    if (q === "") {
      this.abort?.abort();
      this.found = [];
      this.suggestedFor = q;
      return this.renderDropdown();
    }
    const wait = this.options?.debounceMs ?? SUGGEST_DEBOUNCE_MS;
    this.timer = window.setTimeout(() => void this.suggest(q), wait);
  }

  /** Read the first files for `q`; an answer for an older query is dropped. Network I/O. */
  private async suggest(q: string): Promise<void> {
    const options = this.options as SearchBarOptions;
    this.abort?.abort();
    const abort = new AbortController();
    this.abort = abort;
    const generation = ++this.generation;
    let rows: DriveRow[] = [];
    try {
      const scope = await options.scope();
      const page = await options.search.search(
        {
          q,
          node: scope?.node,
          sort: "relevance",
          direction: "descending",
          folders: "mixed",
          zone: zoneOf(this.clock()),
        },
        null,
        SUGGESTED_FILES,
        { signal: abort.signal },
      );
      rows = page.rows.filter((row) => row.kind === "file").slice(0, SUGGESTED_FILES);
    } catch (error) {
      if (generation !== this.generation || abort.signal.aborted) return;
      // A refusal (an operator the host cannot answer) is the results page's to explain.
      if (!(error instanceof ApiError)) console.warn("FileSquid could not read live files", error);
    }
    if (generation !== this.generation) return;
    this.found = rows;
    this.suggestedFor = q;
    this.renderDropdown();
  }

  /** The Type, Location or Modified choices of the open filter. */
  private choiceRows(filter: FilterOption["filter"]): SuggestionOption[] {
    const { labels } = this.options as SearchBarOptions;
    if (filter === "type") {
      return TYPE_CLASSES.map((cls) => this.row("type", cls, "type:", cls, labels.searchHintType));
    }
    if (filter === "in") {
      const hint = labels.searchHintFolder;
      return this.locations.map((name) => this.row("in", name, "in:", name, hint));
    }
    const now = this.clock();
    return MODIFIED_RANGES.map((range) =>
      this.row("modified", range, "after:", modifiedAfter(range, now), labels[RANGE_LABEL[range]]),
    );
  }

  /** A suggestion row. */
  private row(
    filter: ChipFilter,
    chip: string,
    op: string,
    value: string,
    hint: string,
  ): SuggestionOption {
    return { kind: "suggestion", filter, chip, op, value, hint };
  }

  /** The rows the typed text brings: `in:` the folder shown, `type:` a class, `title:` the text. */
  private typedRows(text: string): SuggestionOption[] {
    const { labels } = this.options as SearchBarOptions;
    if (text === "") return [];
    const rows: SuggestionOption[] = [];
    const place = this.place;
    if (place) rows.push(this.row("in", place.name, "in:", place.name, labels.searchHintFolder));
    const mediaTypes = this.liveFiles().map((row) => row.mediaType ?? "");
    const type = typeSuggestions(text, mediaTypes)[0];
    if (type) rows.push(this.row("type", type, "type:", type, labels.searchHintType));
    rows.push(this.row("title", text, "title:", text, labels.searchHintTitle));
    return rows;
  }

  /** The live files, only while they answer the query in the field (never an older one). */
  private liveFiles(): DriveRow[] {
    return this.suggestedFor === this.query ? this.found : [];
  }

  /** What the drop-down shows now. */
  private model(): DropdownModel {
    const { labels } = this.options as SearchBarOptions;
    const filters = (["type", "in", "modified"] as const).map((filter): FilterOption => ({
      kind: "filter",
      filter,
      text: labels[FILTER_LABEL[filter]],
      open: this.choice === filter,
    }));
    const text = this.input?.value.trim() ?? "";
    const choosing = this.choice !== undefined;
    return {
      filters,
      suggestions: this.choice ? this.choiceRows(this.choice) : this.typedRows(text),
      files: choosing ? [] : this.liveFiles().map((row) => ({ kind: "file", row })),
      operators: OPERATORS_ON_HOST.map((text) => ({ kind: "operator", text })),
    };
  }

  /** Redraw the drop-down for the current state. */
  private renderDropdown(): void {
    const dropdown = this.shadowRoot?.querySelector<HTMLElement>(".dropdown");
    const input = this.input;
    if (!dropdown || !input || !this.options) return;
    dropdown.hidden = !this.opened;
    input.setAttribute("aria-expanded", String(this.opened));
    if (!this.opened) {
      input.removeAttribute("aria-activedescendant");
      return;
    }
    const model = this.model();
    this.shown = optionsOf(model);
    this.active = Math.min(this.active, this.shown.length - 1);
    dropdown.replaceChildren(
      ...renderDropdown(model, {
        labels: this.options.labels,
        locale: document.documentElement.lang || "en",
        now: () => this.clock(),
        active: this.active,
        idPrefix: OPTION_ID,
      }),
    );
    this.markActive();
  }

  /** Mark the active option and name it in `aria-activedescendant`. */
  private markActive(): void {
    const options = this.shadowRoot?.querySelectorAll<HTMLElement>("[role=option]") ?? [];
    for (const option of options) {
      const on = Number(option.dataset.index) === this.active;
      option.setAttribute("aria-selected", String(on));
      if (on) option.scrollIntoView?.({ block: "nearest" });
    }
    if (this.active >= 0)
      this.input?.setAttribute("aria-activedescendant", `${OPTION_ID}${this.active}`);
    else this.input?.removeAttribute("aria-activedescendant");
  }
}
