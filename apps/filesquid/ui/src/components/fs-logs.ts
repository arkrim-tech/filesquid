/**
 * `<fs-logs>`: the Logs view of the default UI (UI-F2.16; UI-S-021, UI-S-032, mockups
 * `resources/ui-mockups/v3/38-logs-view.svg`, its dark twin and `39-logs-export.svg`), the one
 * place to browse the activity journal. It is its own view, not the Monitor's tail, and reads
 * only through a {@link LogsSource}.
 *
 * It lists the journal newest first, one page at a time (**Load older**), filtered by the five
 * chips (time range, library, operation, outcome, level) and by a search that names a file, an
 * object, an operation ID or an operation (`classifySearch`); a link from the Provenance tab
 * (`#/logs?file=` or `#/logs?operation_id=`) opens it filtered through `routeFilter`. One row can
 * be selected: the view dispatches `fs-logs-select` with the entry and the libraries, for the
 * detail panel (`<fs-log-entry>`), and `fs-logs-footer` with the sentences of the shell footer.
 * **Live** follows `journal.entry` events: entries that match the filter are counted ("Live ·
 * {count} new") and added to the list when the chip is chosen; after `events.resync` or a
 * reopened channel the list is read again. **Download…** opens the dialog of mockup 39.
 *
 * States, each in the property table's look: loading (placeholder rows), empty, no match with
 * Clear filters, error with the reason, its code and Try again, and denied for a caller who is
 * not the owner (no entries, and the journal is not asked again).
 *
 * The journal holds opaque IDs: a name appears only where the route resolved it (`names=true`).
 * Every value is set with `textContent`. Accessibility (UI-S-016): the shared stylesheet comes
 * first; the table is in a named, focusable scroll region; each row's time is a button; the
 * arrow keys, Home and End move between rows; a load never moves focus; menus and the dialog
 * return focus to their opener.
 */
import { ApiError, type ApiEvent, type JournalEntryView } from "../api/client";
import {
  DEFAULT_RANGE,
  EMPTY_FILTER,
  LEVELS,
  MAX_PAGES,
  OUTCOME_FILTERS,
  RANGES,
  classifySearch,
  entryCountText,
  filterChips,
  footerZoneText,
  isLiveEntry,
  journalQuery,
  liveEntryView,
  matchesFilter,
  mergeNewest,
  rowView,
  searchedFilter,
  showingText,
  utcOffsetLabel,
  type ChipId,
  type LogsFilterState,
  type LogsLibrary,
  type RangeId,
} from "../logs/model";
import type { LogsFilter } from "../app/routes";
import type { LogsSource } from "../logs/source";
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { reasonLabel } from "./denial";
import { STYLE as DIALOG_STYLE } from "./drive-dialogs";
import { STYLE as MENU_STYLE, openMenu, type MenuEntry, type OpenMenu } from "./drive-menu";
import { EXPORT_STYLE, chooser, openExportDialog } from "./logs-export-dialog";
import {
  LOGS_STYLE,
  buildFilters,
  buildHead,
  buildMore,
  buildTable,
  chipElement,
  node,
  placeholderRows,
  rowElement,
  stateElement,
  type FilterParts,
  type HeadParts,
  type MoreParts,
  type StateWords,
  type TableParts,
} from "./logs-view-parts";
import { PROPERTY_TABLE_STYLE } from "./property-table";
import { canPickSaveFile, saveFileToDevice } from "./save-file";
import { failureState } from "./view-state";

/** Name of the event a selected entry is announced with (`detail.entry`, `detail.libraries`). */
export const LOGS_SELECT_EVENT = "fs-logs-select";
/** Name of the event the footer sentences are announced with (`detail.zone`, `detail.count`). */
export const LOGS_FOOTER_EVENT = "fs-logs-footer";

/** Placeholder rows of the loading state. */
const PLACEHOLDER_ROWS = 10;
/** Live entries held for the chip at most; a longer stream is counted, not held. */
const MAX_LIVE = 1000;
/** Topic the event channel sends when it may have dropped events. */
const RESYNC_TOPIC = "events.resync";

/** What the view needs. */
export interface LogsViewOptions {
  /** The active label table. */
  labels: LabelTable;
  /** Reads the journal. */
  source: LogsSource;
  /** The IANA zone times are shown in; the viewer's when absent. */
  zone?: string;
  /** The current time; the clock when absent. */
  now?: () => Date;
  /** Whether the browser is online; `navigator.onLine` when absent. */
  online?: () => boolean;
  /** Saves a download through the browser; the browser's own download when absent. */
  save?: (blob: Blob, name: string) => void;
}

/** What the view is showing. */
type Status = "idle" | "loading" | "ready" | "empty" | "error" | "denied";

/** The stylesheet of the whole view. */
const STYLE = PROPERTY_TABLE_STYLE + LOGS_STYLE + DIALOG_STYLE + MENU_STYLE + EXPORT_STYLE;

/** True for a refusal of the caller: 401, 403 or a contract denial reason. */
function isDenied(error: unknown): boolean {
  if (!(error instanceof ApiError)) return false;
  return error.status === 401 || error.status === 403 || error.reason !== undefined;
}

/** The id the filter's menu gives a value: the value itself, or `""` for all. */
const ALL = "";

/** The parts of the view's shadow DOM. */
interface ViewParts {
  head: HeadParts;
  filters: FilterParts;
  table: TableParts;
  states: HTMLElement;
  more: MoreParts;
  status: HTMLElement;
  layer: HTMLElement;
}

/** Custom element that lists the activity journal. */
export class FsLogs extends HTMLElement {
  private options?: LogsViewOptions;
  private parts?: ViewParts;
  private filter: LogsFilterState = EMPTY_FILTER;
  private entries: JournalEntryView[] = [];
  private cursor: number | null = null;
  private pages = 0;
  private state: Status = "idle";
  private selected: number | undefined;
  private libraries: LogsLibrary[] = [];
  private oldest: string | undefined;
  private live: JournalEntryView[] = [];
  private liveCount = 0;
  private unsubscribe?: () => void;
  private menu?: OpenMenu;
  private generation = 0;
  private abort?: AbortController;
  private routeKey: string | undefined;
  /** A bare UUID just typed in the search, until the read it started has answered. */
  private typedUuid: string | undefined;
  /** Whether the event channel has opened once since the view subscribed. */
  private channelOpened = false;

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the labels and the source, render, and read the journal.
   *
   * @param options See {@link LogsViewOptions}.
   */
  configure(options: LogsViewOptions): void {
    this.options = options;
    this.build(options);
    if (this.isConnected) this.start();
  }

  /**
   * Open the view with the filter a link carries (`#/logs?file=<uuid>` or
   * `#/logs?operation_id=<id>`, UI-S-021), or with the default filter for the bare route. A
   * route that is the one already applied changes nothing. A filtered link covers all time, so
   * no entry of the file or operation is left out by the range.
   *
   * @param filter The filter of the address, or undefined for the bare route.
   */
  set routeFilter(filter: LogsFilter | undefined) {
    const key = JSON.stringify(filter ?? {});
    if (key === this.routeKey) return;
    this.routeKey = key;
    const linked = filter?.file !== undefined || filter?.operation_id !== undefined;
    this.filter = linked ? { range: "all", ...filter } : EMPTY_FILTER;
    this.showSearchText(this.filter);
    if (this.isConnected && this.options) void this.load();
  }

  /** Read the journal and follow the event channel when attached. */
  connectedCallback(): void {
    if (this.options) this.start();
  }

  /** Release the event subscription, the open menu and any read in flight. */
  disconnectedCallback(): void {
    this.abort?.abort();
    this.stopLive();
    this.menu?.close(false);
  }

  /** Select the entry of a row (null clears the selection) and announce it. */
  select(id: number | undefined): void {
    this.selected = id;
    for (const row of this.parts?.table.body.querySelectorAll<HTMLElement>("tr[data-key]") ?? []) {
      row.setAttribute("aria-selected", String(row.dataset.key === String(id)));
    }
    const entry = this.entries.find((e) => e.id === id) ?? null;
    this.dispatchEvent(
      new CustomEvent(LOGS_SELECT_EVENT, {
        detail: { entry, libraries: this.libraries },
        bubbles: true,
        composed: true,
      }),
    );
  }

  /** The entry selected now, or undefined. */
  get selectedEntry(): JournalEntryView | undefined {
    return this.entries.find((e) => e.id === this.selected);
  }

  /** Build the shadow DOM once. */
  private build(options: LogsViewOptions): void {
    const { labels } = options;
    const head = buildHead(labels);
    const filters = buildFilters(labels);
    const table = buildTable(labels);
    const states = node("div", "states");
    states.hidden = true;
    const more = buildMore(labels);
    const status = node("p", "announce");
    status.setAttribute("role", "status");
    const layer = node("div", "layer");
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + CONTROL_STYLE + STYLE;
    this.shadowRoot?.replaceChildren(
      style,
      head.root,
      filters.root,
      table.scroll,
      states,
      more.root,
      status,
      layer,
    );
    this.parts = { head, filters, table, states, more, status, layer };
    this.wire(this.parts);
    this.renderChips();
  }

  /** Attach the listeners of the parts. */
  private wire(parts: ViewParts): void {
    parts.head.download.addEventListener("click", () => void this.openDownload());
    parts.head.live.addEventListener("click", () => this.showLive());
    parts.more.older.addEventListener("click", () => void this.loadOlder());
    parts.filters.chips.addEventListener("click", (event) => this.onChipClick(event));
    parts.filters.root.addEventListener("submit", (event) => {
      event.preventDefault();
      void this.search();
    });
    parts.filters.search.addEventListener("input", () => this.onSearchInput());
    parts.filters.clear.addEventListener("click", () => this.clearSearch());
    parts.table.body.addEventListener("click", (event) => this.onRowClick(event));
    parts.table.body.addEventListener("keydown", (event) => this.onRowKey(event));
  }

  /** Subscribe to the event channel and read the journal. */
  private start(): void {
    this.stopLive();
    this.channelOpened = false;
    const options = this.options;
    if (!options) return;
    try {
      this.unsubscribe = options.source.subscribe(
        (event) => this.onEvent(event),
        () => {
          // The first open is the channel starting; a later open means it was lost and may
          // have dropped events, so the list is read again.
          if (this.channelOpened) void this.load(false);
          this.channelOpened = true;
        },
      );
    } catch (error) {
      console.warn("FileSquid Logs could not follow the event channel", errorName(error));
    }
    this.showLiveChip();
    void this.load();
  }

  /** End the event subscription. */
  private stopLive(): void {
    this.unsubscribe?.();
    this.unsubscribe = undefined;
    this.showLiveChip();
  }

  /** The time now. */
  private now(): Date {
    return this.options?.now?.() ?? new Date();
  }

  /**
   * Read the first page of the journal for the filter, the libraries and the oldest entry, and
   * draw it. A newer read cancels an older one. Network I/O.
   *
   * @param loading Whether to show the placeholder rows while it reads (not on a quiet refresh).
   */
  private async load(loading = true): Promise<void> {
    const options = this.options;
    if (!options || !this.parts) return;
    this.abort?.abort();
    this.abort = new AbortController();
    const generation = ++this.generation;
    if (loading) this.showLoading();
    try {
      const [page, libraries] = await Promise.all([
        this.firstPage(options),
        options.source.libraries(),
      ]);
      if (generation !== this.generation) return;
      this.libraries = libraries;
      const pending = this.live;
      this.live = [];
      this.liveCount = 0;
      this.entries = mergeNewest(page.entries, pending);
      this.cursor = page.next_cursor;
      this.pages = 1;
      this.draw();
      void this.readOldest(generation);
    } catch (error) {
      if (generation === this.generation) this.fail(error);
    }
  }

  /**
   * The first page. A bare UUID typed in the search is an operation ID first; when no entry has
   * it, it is tried as a file ID (the journal cannot say which it is). Network I/O.
   */
  private async firstPage(options: LogsViewOptions) {
    const uuid = this.typedUuid;
    this.typedUuid = undefined;
    const page = await options.source.page(journalQuery(this.filter, this.now()));
    if (page.entries.length > 0 || uuid === undefined) return page;
    this.filter = stripUndefined({ ...this.filter, operation_id: undefined, file: uuid });
    return options.source.page(journalQuery(this.filter, this.now()));
  }

  /** The oldest entry's time, for the dialog's All entries line. A failed read is logged. */
  private async readOldest(generation: number): Promise<void> {
    try {
      const entry = await this.options?.source.oldest();
      if (generation === this.generation) this.oldest = entry?.time;
    } catch (error) {
      console.warn("FileSquid Logs could not read the first entry", errorName(error));
    }
  }

  /** Read the next page of older entries and add its rows. Network I/O. */
  private async loadOlder(): Promise<void> {
    const options = this.options;
    const parts = this.parts;
    if (!options || !parts || this.cursor === null || this.pages >= MAX_PAGES) return;
    const generation = this.generation;
    parts.more.older.disabled = true;
    parts.more.older.textContent = options.labels.logsLoadingOlder;
    try {
      const page = await options.source.page(
        journalQuery(this.filter, this.now(), { after: this.cursor }),
      );
      if (generation !== this.generation) return;
      this.entries = [...this.entries, ...page.entries];
      this.cursor = page.next_cursor;
      this.pages += 1;
    } catch (error) {
      if (generation !== this.generation) return;
      console.warn("FileSquid Logs could not read older entries", errorName(error));
      parts.more.older.textContent = options.labels.logsLoadOlder;
      parts.more.older.disabled = false;
      parts.status.textContent = options.labels.logsErrorTitle;
      return;
    }
    parts.more.older.textContent = options.labels.logsLoadOlder;
    parts.more.older.disabled = false;
    this.draw();
  }

  /** The placeholder rows of the loading state. */
  private showLoading(): void {
    const parts = this.parts;
    const labels = this.options?.labels;
    if (!parts || !labels) return;
    this.state = "loading";
    parts.states.hidden = true;
    parts.table.scroll.hidden = false;
    parts.table.body.className = "loading";
    parts.table.body.replaceChildren(...placeholderRows(PLACEHOLDER_ROWS));
    parts.table.body.setAttribute("aria-busy", "true");
    parts.more.root.hidden = true;
    parts.status.textContent = labels.logsLoading;
    this.dataset.state = "loading";
  }

  /** Draw the table, the line under it and the footer from the entries read. */
  private draw(): void {
    const parts = this.parts;
    const options = this.options;
    if (!parts || !options) return;
    parts.table.body.removeAttribute("aria-busy");
    parts.table.body.className = "";
    parts.status.textContent = "";
    this.renderChips();
    this.showLiveChip();
    if (this.entries.length === 0) return this.showEmpty();
    this.state = "ready";
    this.dataset.state = "ready";
    parts.states.hidden = true;
    parts.table.scroll.hidden = false;
    const { labels, zone } = options;
    const own = new Map(this.libraries.map((library) => [library.library_id, library.name]));
    const rows = this.entries.map((entry) =>
      rowElement(rowView(entry, labels, zone, own), labels, entry.id === this.selected),
    );
    parts.table.body.replaceChildren(...rows);
    this.rovingTab();
    this.showMore();
    this.announceFooter();
  }

  /** The line under the rows and Load older. */
  private showMore(): void {
    const parts = this.parts;
    const labels = this.options?.labels;
    if (!parts || !labels) return;
    const more = this.cursor !== null;
    parts.more.root.hidden = false;
    parts.more.text.textContent = showingText(labels, this.entries.length, more, this.filter.range);
    parts.more.older.hidden = !more || this.pages >= MAX_PAGES;
    parts.more.older.disabled = false;
    parts.more.older.textContent = labels.logsLoadOlder;
  }

  /** Tell the shell what the footer says. */
  private announceFooter(): void {
    const labels = this.options?.labels;
    if (!labels) return;
    const zone = footerZoneText(labels, utcOffsetLabel(this.now(), this.options?.zone));
    const count = this.state === "ready" ? entryCountText(labels, this.entries.length) : "";
    this.dispatchEvent(
      new CustomEvent(LOGS_FOOTER_EVENT, {
        detail: { zone, count },
        bubbles: true,
        composed: true,
      }),
    );
  }

  /** Show a state in place of the table. */
  private showState(state: Status, words: StateWords, role: "alert" | "status"): void {
    const parts = this.parts;
    if (!parts) return;
    this.state = state;
    this.dataset.state = state;
    parts.table.scroll.hidden = true;
    parts.more.root.hidden = true;
    parts.states.hidden = false;
    parts.states.replaceChildren(stateElement(words, role));
    this.showLiveChip();
    this.announceFooter();
  }

  /** The empty state: no entries at all, or none for the filter. */
  private showEmpty(): void {
    const labels = this.options?.labels;
    if (!labels) return;
    const filtered = this.isFiltered();
    const words: StateWords = {
      glyph: "logs",
      title: filtered ? labels.logsNoMatchTitle : labels.logsEmptyTitle,
      details: [filtered ? labels.logsNoMatchDetail : labels.logsEmptyDetail],
      ...(filtered
        ? { action: { text: labels.logsClearFilters, run: () => this.clearFilters() } }
        : {}),
    };
    this.showState("empty", words, "status");
  }

  /** Whether anything narrows the journal beyond the default range. */
  private isFiltered(): boolean {
    const { range: _range, ...rest } = this.filter;
    void _range;
    return this.filter.range !== DEFAULT_RANGE || Object.values(rest).some((v) => v !== undefined);
  }

  /** The error or denied state of a failed read. */
  private fail(error: unknown): void {
    const labels = this.options?.labels;
    if (!labels) return;
    this.entries = [];
    this.cursor = null;
    if (isDenied(error)) {
      this.stopLive();
      const reason =
        error instanceof ApiError && error.reason ? reasonLabel(error.reason, labels) : undefined;
      const words: StateWords = {
        glyph: "denied",
        title: labels.logsDeniedTitle,
        details: [labels.logsDeniedDetail, ...(reason ? [reason] : [])],
      };
      this.parts?.table.body.replaceChildren();
      return this.showState("denied", words, "status");
    }
    console.warn("FileSquid Logs could not read the journal", errorName(error));
    const online = this.options?.online ? this.options.online() : navigator.onLine;
    const view = failureState(error, online);
    const reason =
      view.kind === "offline"
        ? labels.stateOffline
        : view.kind === "unavailable"
          ? labels.stateUnavailable
          : labels.stateError;
    const code = error instanceof ApiError ? error.code : undefined;
    this.showState(
      "error",
      {
        glyph: "error",
        alerting: true,
        title: labels.logsErrorTitle,
        details: [reason, labels.filePanelNothingChanged],
        ...(code ? { code } : {}),
        action: { text: labels.stateRetry, icon: "refresh", run: () => void this.load() },
      },
      "alert",
    );
  }

  /** Draw the chips from the filter. */
  private renderChips(): void {
    const parts = this.parts;
    const labels = this.options?.labels;
    if (!parts || !labels) return;
    const chips = filterChips(this.filter, labels, this.libraries);
    parts.filters.chips.replaceChildren(...chips.map((chip) => chipElement(chip, labels)));
  }

  /** A click on a chip: open its menu, or remove a file, object or operation ID filter. */
  private onChipClick(event: Event): void {
    const button = (event.target as Element).closest<HTMLButtonElement>("button[data-chip]");
    const id = button?.dataset.chip as ChipId | undefined;
    if (!button || !id) return;
    if (id === "file" || id === "object" || id === "operation_id") return this.removeFilter(id);
    this.openChipMenu(button, id);
  }

  /** Remove a file, object or operation ID filter and read again. */
  private removeFilter(id: "file" | "object" | "operation_id"): void {
    const { [id]: _removed, ...rest } = this.filter;
    void _removed;
    this.filter = rest as LogsFilterState;
    this.showSearchText(this.filter);
    void this.load();
  }

  /** Open the menu of one chip below it. */
  private openChipMenu(button: HTMLButtonElement, id: ChipId): void {
    const parts = this.parts;
    if (!parts) return;
    const wasOpen = button.getAttribute("aria-expanded") === "true";
    this.menu?.close(false);
    for (const chip of parts.filters.chips.querySelectorAll("button")) {
      chip.setAttribute("aria-expanded", "false");
    }
    if (wasOpen) return;
    const entries = this.menuEntries(id);
    if (entries.length === 0) return;
    const labels = this.options?.labels as LabelTable;
    button.setAttribute("aria-expanded", "true");
    const menu = openMenu(parts.layer, { element: button }, button, {
      className: "filter-menu",
      label: labels[MENU_LABEL[id as keyof typeof MENU_LABEL]],
      entries,
      onChoose: (choice) => this.choose(id, choice),
    });
    this.menu = menu;
  }

  /** The entries of a chip's menu: a radio for "all" and for each value. */
  private menuEntries(id: ChipId): MenuEntry[] {
    const labels = this.options?.labels as LabelTable;
    const radio = (value: string, label: string, checked: boolean): MenuEntry => ({
      kind: "radio",
      id: value,
      label,
      checked,
    });
    if (id === "range") {
      return RANGES.map((r) => radio(r, labels[RANGE_LABEL[r]], this.filter.range === r));
    }
    if (id === "library") {
      return [
        radio(ALL, labels.logsMenuAllLibraries, this.filter.library === undefined),
        ...this.libraries.map((l) =>
          radio(l.library_id, l.name, this.filter.library === l.library_id),
        ),
      ];
    }
    if (id === "operation") return this.operationEntries();
    if (id === "outcome") {
      return [
        radio(ALL, labels.logsMenuAllOutcomes, this.filter.outcome === undefined),
        ...OUTCOME_FILTERS.map((o) => radio(o, labels[OUTCOME_MENU[o]], this.filter.outcome === o)),
      ];
    }
    return [
      radio(ALL, labels.logsMenuAllLevels, this.filter.level === undefined),
      ...LEVELS.map((l) => radio(l, labels[LEVEL_MENU[l]], this.filter.level === l)),
    ];
  }

  /** The operation menu: "all" and each operation read so far (the API has no list of them). */
  private operationEntries(): MenuEntry[] {
    const labels = this.options?.labels as LabelTable;
    const names = new Set(this.entries.map((e) => e.operation));
    if (this.filter.operation !== undefined) names.add(this.filter.operation);
    const sorted = [...names].sort();
    return [
      {
        kind: "radio",
        id: ALL,
        label: labels.logsMenuAllOperations,
        checked: this.filter.operation === undefined,
      },
      ...sorted.map((name): MenuEntry => ({
        kind: "radio",
        id: name,
        label: name,
        checked: this.filter.operation === name,
      })),
    ];
  }

  /** Apply a menu choice to the filter and read again. */
  private choose(id: ChipId, choice: string): void {
    const value = choice === ALL ? undefined : choice;
    const next = { ...this.filter } as Record<string, unknown>;
    if (id === "range") next["range"] = (value ?? DEFAULT_RANGE) as RangeId;
    else next[id] = value;
    this.filter = stripUndefined(next as unknown as LogsFilterState);
    if (id === "operation") this.showSearchText(this.filter);
    void this.load();
  }

  /** Put the search text that names the filter's file, object, operation ID or operation. */
  private showSearchText(filter: LogsFilterState): void {
    const parts = this.parts;
    if (!parts) return;
    const text = filter.file ?? filter.object ?? filter.operation_id ?? filter.operation ?? "";
    parts.filters.search.value = text;
    parts.filters.clear.hidden = text === "";
    parts.filters.hint.hidden = true;
  }

  /** Show the clear button while the search field holds text. */
  private onSearchInput(): void {
    const parts = this.parts;
    if (!parts) return;
    parts.filters.clear.hidden = parts.filters.search.value === "";
  }

  /** Empty the search field and read again without the search. */
  private clearSearch(): void {
    const parts = this.parts;
    if (!parts) return;
    const typed = parts.filters.search.value.trim();
    parts.filters.search.value = "";
    parts.filters.clear.hidden = true;
    const next = searchedFilter(this.filter, { kind: "empty" });
    // The search field names the operation too when the filter's operation is what it holds.
    this.filter = typed !== "" && typed === next.operation ? omitOperation(next) : next;
    this.applySearchHint(undefined);
    void this.load();
    parts.filters.search.focus();
  }

  /** Apply the search text: filter by what it names, or say what it can be. */
  private async search(): Promise<void> {
    const parts = this.parts;
    const labels = this.options?.labels;
    if (!parts || !labels) return;
    const text = parts.filters.search.value;
    const kind = classifySearch(text);
    if (kind.kind === "none") {
      this.applySearchHint(labels.logsSearchHint, true);
      return;
    }
    this.filter = searchedFilter(this.filter, kind);
    this.typedUuid = kind.kind === "uuid" ? kind.value : undefined;
    this.applySearchHint(kind.kind === "empty" ? undefined : this.matchedText(kind), false);
    await this.load();
  }

  /** The words for what the search matched ("Searching operation: ocfl.commit"). */
  private matchedText(kind: { kind: string; value: string }): string {
    const labels = this.options?.labels as LabelTable;
    const word: Record<string, LabelKey> = {
      object: "logsSearchKindObject",
      uuid: "logsSearchKindOperationId",
      operation_id: "logsSearchKindOperationId",
      operation: "logsSearchKindOperation",
    };
    const key = word[kind.kind] ?? "logsSearchKindObject";
    return formatLabel(labels.logsSearchMatched, { kind: labels[key], value: kind.value });
  }

  /** Show or hide the line under the filter row. */
  private applySearchHint(text: string | undefined, invalid = false): void {
    const hint = this.parts?.filters.hint;
    if (!hint) return;
    hint.hidden = text === undefined;
    hint.textContent = text ?? "";
    hint.classList.toggle("invalid", invalid);
  }

  /** Clear every filter: the default range and nothing else. */
  private clearFilters(): void {
    this.filter = EMPTY_FILTER;
    this.showSearchText(this.filter);
    void this.load();
  }

  /** A click on a row selects its entry. */
  private onRowClick(event: Event): void {
    const row = (event.target as Element).closest<HTMLElement>("tr[data-key]");
    if (!row) return;
    const id = Number(row.dataset.key);
    this.select(Number.isNaN(id) ? undefined : id);
  }

  /** Arrow keys, Home and End move between the rows' buttons. */
  private onRowKey(event: KeyboardEvent): void {
    const keys = ["ArrowDown", "ArrowUp", "Home", "End"];
    if (!keys.includes(event.key)) return;
    const buttons = [
      ...(this.parts?.table.body.querySelectorAll<HTMLButtonElement>("button.select") ?? []),
    ];
    const here = buttons.indexOf(
      (event.target as Element).closest("button.select") as HTMLButtonElement,
    );
    if (here < 0) return;
    event.preventDefault();
    const target =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? buttons.length - 1
          : here + (event.key === "ArrowDown" ? 1 : -1);
    const next = buttons[Math.max(0, Math.min(buttons.length - 1, target))];
    if (!next) return;
    for (const button of buttons) button.tabIndex = button === next ? 0 : -1;
    next.focus();
  }

  /** One Tab stop in the rows: the selected row's button, else the first. */
  private rovingTab(): void {
    const buttons = [
      ...(this.parts?.table.body.querySelectorAll<HTMLButtonElement>("button.select") ?? []),
    ];
    const chosen =
      buttons.find((b) => b.closest("tr")?.getAttribute("aria-selected") === "true") ?? buttons[0];
    for (const button of buttons) button.tabIndex = button === chosen ? 0 : -1;
  }

  /** An event of the channel: a journal entry or a resync. */
  private onEvent(event: ApiEvent): void {
    if (event.topic === RESYNC_TOPIC) return void this.load(false);
    if (event.topic !== "journal.entry" || !isLiveEntry(event.data)) return;
    const view = liveEntryView(event.data);
    if (!matchesFilter(view, this.filter, this.now())) return;
    if (this.entries.some((e) => e.id === view.id) || this.live.some((e) => e.id === view.id))
      return;
    if (this.live.length < MAX_LIVE) this.live.push(view);
    this.liveCount += 1;
    this.showLiveChip();
    const labels = this.options?.labels;
    if (this.parts && labels) {
      this.parts.status.textContent = formatLabel(labels.logsLiveNewAnnounce, {
        count: String(this.liveCount),
      });
    }
  }

  /** Show or hide the Live chip, with the count of new entries. */
  private showLiveChip(): void {
    const parts = this.parts;
    const labels = this.options?.labels;
    if (!parts || !labels) return;
    const chip = parts.head.live;
    chip.hidden = this.unsubscribe === undefined || this.state === "denied";
    const text = chip.querySelector(".text");
    if (text) {
      text.textContent =
        this.liveCount > 0
          ? formatLabel(labels.logsLiveNew, { count: String(this.liveCount) })
          : labels.logsLive;
    }
    chip.classList.toggle("fresh", this.liveCount > 0);
  }

  /** Add the live entries to the top of the list and scroll to them. */
  private showLive(): void {
    if (this.liveCount === 0) return;
    this.entries = mergeNewest(this.entries, this.live);
    this.live = [];
    this.liveCount = 0;
    if (this.state === "empty") this.state = "ready";
    this.draw();
    const scroll = this.parts?.table.scroll;
    if (scroll && typeof scroll.scrollIntoView === "function") {
      scroll.scrollIntoView({ block: "nearest" });
    }
  }

  /** Open the Download dialog (mockup 39). */
  private async openDownload(): Promise<void> {
    const options = this.options;
    const parts = this.parts;
    if (!options || !parts) return;
    const win = window;
    const canChoose = canPickSaveFile(win);
    openExportDialog({
      labels: options.labels,
      layer: parts.layer,
      opener: parts.head.download,
      filter: this.filter,
      libraries: this.libraries,
      loaded:
        this.state === "ready"
          ? { count: this.entries.length, more: this.cursor !== null }
          : undefined,
      oldest: this.oldest,
      now: this.now(),
      zone: options.zone,
      lang: document.documentElement.lang || "en",
      run: (query) => options.source.export(query),
      save: options.save ?? saveFileToDevice,
      canChoose,
      choose: canChoose ? chooser(win, options.labels) : undefined,
      onDone: (name) => {
        parts.status.textContent = formatLabel(options.labels.logsExportDone, { name });
      },
    });
  }
}

/** The filter without its operation. */
function omitOperation(filter: LogsFilterState): LogsFilterState {
  const next: { -readonly [K in keyof LogsFilterState]: LogsFilterState[K] } = { ...filter };
  delete next.operation;
  return next;
}

/** The filter without its undefined members. */
function stripUndefined(filter: LogsFilterState): LogsFilterState {
  const entries = Object.entries(filter).filter(([, value]) => value !== undefined);
  return Object.fromEntries(entries) as unknown as LogsFilterState;
}

/** The kind of an error, without its message. */
function errorName(error: unknown): string {
  return error instanceof Error ? error.name : typeof error;
}

/** The menu name of each chip. */
const MENU_LABEL = {
  range: "logsMenuRange",
  library: "logsMenuLibrary",
  operation: "logsMenuOperation",
  outcome: "logsMenuOutcome",
  level: "logsMenuLevel",
} as const satisfies Record<string, LabelKey>;

/** The chip word of each range. */
const RANGE_LABEL: Record<RangeId, LabelKey> = {
  hour: "logsRangeHour",
  day: "logsRangeDay",
  week: "logsRangeWeek",
  month: "logsRangeMonth",
  all: "logsRangeAll",
};

/** The menu word of each outcome. */
const OUTCOME_MENU: Record<(typeof OUTCOME_FILTERS)[number], LabelKey> = {
  // `info` is no menu choice: an entry with no outcome cannot be filtered for.
  succeeded: "logsOutcomeOk",
  failed: "logsOutcomeFailed",
  refused: "logsOutcomeRefused",
  started: "logsOutcomeStarted",
  stalled: "logsOutcomeStalled",
  resumed: "logsOutcomeResumed",
  cancelled: "logsOutcomeCancelled",
};

/** The menu word of each level. */
const LEVEL_MENU: Record<(typeof LEVELS)[number], LabelKey> = {
  info: "logsLevelInfo",
  warn: "logsLevelWarn",
  error: "logsLevelError",
};
