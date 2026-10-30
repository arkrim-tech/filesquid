/**
 * `<fs-drive-view>`: the Drive-style view of one library, collection or virtual collection
 * (UI-F2.8, F-062, UI-S-006, UI-S-026; mockups 05, 06, 10 to 15, 33, 44 and 56). It reads one
 * view of a navigation node page by page through the view operation of API-S-011 and shows it as
 * a list (default) or a grid, with the page title, the layout control, column-header sorting
 * with reverse (the grid has the Sort menu), row and header checkboxes with a bulk bar that
 * states the effective scope, the per-item menu on the actions button and on right click, rename,
 * remove with Undo, move or copy through the shared destination picker, and the Download as ZIP
 * dialog within its caps. The Recent view groups rows by day and has a fixed order.
 *
 * The choices that survive a restart (layout, sort, folder placement, hover card) are the
 * per-user preferences of {@link PreferencesService}; a change is applied at once and saved
 * through the API. Every name shown is the API's display name (UI-S-019): no name, path or stored
 * name reaches an attribute. The view never fetches by itself: it asks a {@link ViewSource}, and
 * its writes go through the {@link FileSource} of the file routes (UI-S-012).
 *
 * **Events** (bubbling, composed): `fs-summary` (`detail.text`: the footer summary or loading
 * line), `fs-drive-open` (`detail.row`, `detail.intent`: a file was opened), and
 * `fs-drive-change` (after a write committed). A folder row is a link to its own node route.
 *
 * Accessibility (UI-S-016): see {@link DriveList} for the keys and the roving Tab stop; menus
 * and dialogs return focus to the control that opened them; the shared stylesheet comes first.
 */
import { ApiError } from "../api/client";
import { DriveActions, UnavailableActionError, type DriveFiles } from "../drive/actions";
import {
  VIEW_PAGE_SIZE,
  MAX_VIEW_PAGES,
  activityOf,
  driveSummary,
  recentGroup,
  SORT_LABEL,
  nextSort,
  scopeText,
  zipCheck,
  type DriveRow,
  type ViewQuery,
} from "../drive/model";
import { Selection } from "../drive/selection";
import type { ViewSource } from "../drive/source";
import { formatSize } from "../files/model";
import type { NavigationSource } from "../navigation/model";
import { navigationHref } from "../navigation/model";
import type { PreferencesService } from "../preferences/service";
import type {
  Layout,
  SortDirection,
  SortKey,
  UserPreferences,
  ViewKind,
} from "../preferences/model";
import { formatLabel, type LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import {
  STYLE as DIALOG_STYLE,
  moveDialog,
  renameDialog,
  showToast,
  zipDialog,
} from "./drive-dialogs";
import { itemEntries, sortEntries } from "./drive-entries";
import { DriveList, type DriveItem, type DriveListHooks } from "./drive-list";
import { STYLE as MENU_STYLE, openMenu, type MenuAnchor, type OpenMenu } from "./drive-menu";
import { STYLE as ROW_STYLE } from "./drive-row-view";
import { focusElement } from "../a11y/focus";
import { RETRY_EVENT, type FsStates } from "./fs-states";
import { icon } from "./icons";
import { failureState, type ViewState } from "./view-state";

/** Streams a ZIP of the selected rows; absent on a host without the ZIP route. */
export type ZipDownload = (
  rows: readonly DriveRow[],
  name: string,
  progress: (done: number, bytes: number) => void,
) => Promise<void>;

/** What the view needs. */
export interface DriveViewOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** Reads the views. */
  readonly source: ViewSource;
  /** The per-user preferences: layout, sort, folder placement, hover card. */
  readonly preferences: Pick<
    PreferencesService,
    "preferences" | "update" | "addEventListener" | "removeEventListener"
  >;
  /** The file writes of rename and remove; without them those actions are not offered. */
  readonly files?: DriveFiles;
  /** The navigation listing the destination picker reads. */
  readonly navigation?: NavigationSource;
  /** Streams a ZIP; without it the dialog says the download is not available. */
  readonly zip?: ZipDownload;
  /** The route of a folder row; defaults to the navigation route. */
  readonly href?: (node: string) => string;
  /** Whether the browser is online; defaults to `navigator.onLine`. */
  readonly online?: () => boolean;
  /** The time now, for relative dates; defaults to the clock. */
  readonly now?: () => Date;
}

const STYLE = `
  :host { display: flex; flex-direction: column; block-size: 100%; min-block-size: 0; min-width: 0; color: var(--fs-on-surface); font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); }
  :host([hidden]) { display: none; }
  .view { display: flex; flex-direction: column; flex: 1 1 auto; min-height: 0; min-width: 0; }
  .top { display: flex; flex-wrap: wrap; align-items: flex-start; justify-content: space-between; gap: var(--fs-space-3); margin-block-end: var(--fs-space-2); }
  .top .page-title { flex: 1 1 auto; min-width: 0; }
  .tools { display: flex; align-items: center; gap: var(--fs-space-3); margin-block-start: var(--fs-space-1); }
  .recent-note { margin: 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .layout { display: inline-flex; box-sizing: border-box; inline-size: 4.5rem; block-size: 2rem; border: 1.5px solid var(--fs-outline-strong); border-radius: var(--fs-radius-button); background: var(--fs-surface-card); overflow: hidden; }
  .layout button { flex: 1 1 50%; display: inline-flex; align-items: center; justify-content: center; padding: 0; border: 0; background: none; color: var(--fs-icon); cursor: pointer; }
  .layout button + button { border-inline-start: 1px solid var(--fs-outline-strong); }
  .layout button[aria-pressed="true"] { background: var(--fs-primary-container); color: var(--fs-primary); }
  .bulk { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-2) var(--fs-space-3); box-sizing: border-box; min-block-size: 3.5rem; padding: var(--fs-space-2) var(--fs-space-3); border-radius: var(--fs-radius); background: var(--fs-primary-container); }
  .bulk-text { flex: 1 1 16rem; display: grid; min-width: 0; }
  .bulk-count { margin: 0; font-size: var(--fs-font-size-ui); font-weight: 600; color: var(--fs-on-primary-container); }
  .bulk-scope { margin: 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); overflow-wrap: anywhere; }
  .bulk .button { min-block-size: 2.25rem; }
  .bulk .button[aria-disabled="true"] { cursor: default; }
  fs-states { flex: 1 1 auto; }
  .hover-card { position: fixed; z-index: 4; box-sizing: border-box; max-inline-size: 18rem; padding: var(--fs-space-2) var(--fs-space-3); border: 1px solid var(--fs-outline-strong); border-radius: var(--fs-radius); background: var(--fs-surface-overlay); font-size: var(--fs-font-size-small); color: var(--fs-on-surface); overflow-wrap: anywhere; }
  [hidden] { display: none !important; }
`;

/** Rows shown as placeholders while the first page loads (mockup 33). */
const PLACEHOLDER_ROWS = 10;
/** Delay before a hover card shows, in milliseconds. */
const HOVER_DELAY_MS = 400;

/** True when the pointer is a fine, hovering one and the user did not ask for reduced motion. */
function hoverAllowed(): boolean {
  const query = (text: string) => window.matchMedia?.(text)?.matches ?? false;
  const fine =
    typeof window.matchMedia !== "function" || query("(hover: hover) and (pointer: fine)");
  return fine && !query("(prefers-reduced-motion: reduce)");
}

/** Custom element that shows one view of one node. */
export class FsDriveView extends HTMLElement {
  private options?: DriveViewOptions;
  private actions?: DriveActions;
  private list?: DriveList;
  private rows: DriveRow[] = [];
  private cursor: string | null = null;
  private pages = 0;
  private complete = true;
  private target = { node: "", view: "all" as ViewKind, filter: "" };
  private titleText = "";
  private readonly chosen = new Selection();
  private generation = 0;
  private abort?: AbortController;
  private loadingMore = false;
  private menu?: OpenMenu;
  private applied = "";
  private hoverTimer?: number;
  private summaryText = "";
  private readonly onPreferences = () => this.preferencesChanged();

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.addEventListener(RETRY_EVENT, (event) => {
      event.stopPropagation();
      void this.reload();
    });
    this.chosen.addEventListener("change", () => this.selectionChanged());
  }

  /**
   * Set the labels, sources and preferences and build the frame. Call before or after
   * connecting, then set `node`.
   *
   * @param options Labels, view source, preferences and the optional writes.
   */
  configure(options: DriveViewOptions): void {
    this.options?.preferences.removeEventListener("change", this.onPreferences);
    this.options = options;
    this.actions = options.files ? new DriveActions(options.files) : undefined;
    options.preferences.addEventListener("change", this.onPreferences);
    this.build();
    if (this.target.node !== "") void this.reload();
  }

  /** The page title: the node's name. */
  get heading(): string {
    return this.titleText;
  }

  /** Set the page title (the node's display name). */
  set heading(text: string) {
    this.titleText = text;
    const title = this.shadowRoot?.querySelector(".page-title");
    if (title) title.textContent = text;
  }

  /** The node shown. */
  get node(): string {
    return this.target.node;
  }

  /** Show another node; its first page is read again and the selection is cleared. */
  set node(handle: string) {
    if (handle === this.target.node) return;
    this.target = { ...this.target, node: handle };
    this.chosen.clear();
    if (this.options) void this.reload();
  }

  /** Which view of the node is shown. */
  get view(): ViewKind {
    return this.target.view;
  }

  /** Show another view of the node (all, recent, deleted); read again when it changes. */
  set view(view: ViewKind) {
    if (view === this.target.view) return;
    this.target = { ...this.target, view };
    this.chosen.clear();
    if (!this.options) return;
    this.configureList();
    this.drawTools();
    if (this.target.node !== "") void this.reload();
  }

  /** The sidebar filter text: narrows only this view (UI-S-007). */
  get filter(): string {
    return this.target.filter;
  }

  /** Narrow the view to rows whose name contains `text`; read again when it changes. */
  set filter(text: string) {
    if (text === this.target.filter) return;
    this.target = { ...this.target, filter: text };
    this.chosen.clear();
    if (this.options && this.target.node !== "") void this.reload();
  }

  /** The footer summary or loading line now. */
  get summary(): string {
    return this.summaryText;
  }

  /** Follow the preferences and the listing's size while attached. */
  connectedCallback(): void {
    this.options?.preferences.addEventListener("change", this.onPreferences);
    this.list?.connect();
  }

  /** Stop following; answers still in flight are dropped. */
  disconnectedCallback(): void {
    this.options?.preferences.removeEventListener("change", this.onPreferences);
    this.list?.disconnect();
    this.abort?.abort();
    this.menu?.close(false);
    window.clearTimeout(this.hoverTimer);
  }

  /**
   * Read the first page of the view again and show it, or the state for why it cannot be shown.
   * An answer for an older read is dropped. Network I/O via the source.
   *
   * @returns Resolves once the answer or the failure is shown.
   */
  async reload(): Promise<void> {
    const options = this.options;
    if (!options || this.target.node === "") return;
    this.abort?.abort();
    const abort = new AbortController();
    this.abort = abort;
    const generation = ++this.generation;
    this.applied = this.appliedKey();
    this.showLoading();
    try {
      const page = await options.source.view(this.query(), null, VIEW_PAGE_SIZE, {
        signal: abort.signal,
      });
      if (generation !== this.generation) return;
      this.rows = [...page.rows];
      this.cursor = page.nextCursor;
      this.complete = page.complete;
      this.pages = 1;
      this.showRows();
    } catch (error) {
      if (generation === this.generation && !abort.signal.aborted) this.fail(error);
    }
  }

  /** The query for the node, the view and the saved sort. */
  private query(): ViewQuery {
    const prefs = (this.options as DriveViewOptions).preferences.preferences;
    const recent = this.target.view === "recent";
    return {
      node: this.target.node,
      view: this.target.view,
      sort: recent ? "modified" : prefs.sort_key,
      direction: recent ? "descending" : prefs.sort_direction,
      folders: prefs.folder_placement,
      filter: this.target.filter,
    };
  }

  /** What a re-read depends on: the sort and the folder placement. */
  private appliedKey(): string {
    const prefs = (this.options as DriveViewOptions).preferences.preferences;
    return [prefs.sort_key, prefs.sort_direction, prefs.folder_placement].join("|");
  }

  /** The preferences changed: read again for a new sort, redraw for a new layout. */
  private preferencesChanged(): void {
    if (!this.list) return;
    if (this.appliedKey() !== this.applied && this.target.node !== "") {
      this.configureList();
      this.drawTools();
      void this.reload();
      return;
    }
    this.configureList();
    this.drawItems();
    this.drawTools();
  }

  /** Build the frame: title, tools, bulk bar, states, list and the menu layer. */
  private build(): void {
    const root = this.shadowRoot as ShadowRoot;
    const options = this.options as DriveViewOptions;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + CONTROL_STYLE + STYLE + ROW_STYLE + MENU_STYLE + DIALOG_STYLE;
    const view = document.createElement("div");
    view.className = "view";
    const top = document.createElement("header");
    top.className = "top";
    const title = document.createElement("h2");
    title.className = "page-title";
    title.textContent = this.titleText;
    const tools = document.createElement("div");
    tools.className = "tools";
    top.append(title, tools);
    const states = document.createElement("fs-states") as FsStates;
    states.configure({ labels: options.labels });
    states.hidden = true;
    this.list?.disconnect();
    this.list = new DriveList({
      labels: options.labels,
      locale: document.documentElement.lang || "en",
      now: options.now ?? (() => new Date()),
      href: (row) => this.hrefOf(row),
      selection: this.chosen,
      rtl: () => getComputedStyle(this).direction === "rtl",
      hooks: this.hooks(),
    });
    const layer = document.createElement("div");
    layer.className = "layer";
    view.append(top, states, this.list.element, layer);
    root.replaceChildren(style, view);
    this.configureList();
    this.drawTools();
    if (this.isConnected) this.list.connect();
  }

  /** The link of a folder row. */
  private hrefOf(row: DriveRow): string {
    const href = this.options?.href ?? ((node: string) => navigationHref({ node }));
    return row.node ? href(row.node) : "";
  }

  /** The element in the shadow root by class. */
  private piece<T extends HTMLElement = HTMLElement>(selector: string): T {
    return (this.shadowRoot as ShadowRoot).querySelector(selector) as T;
  }

  /** The layout now: always the list in Recent, else the saved layout. */
  private layout(): Layout {
    if (this.target.view === "recent") return "list";
    return (this.options as DriveViewOptions).preferences.preferences.layout;
  }

  /** Give the list its layout, its sort and the Recent mode. */
  private configureList(): void {
    const prefs = (this.options as DriveViewOptions).preferences.preferences;
    const recent = this.target.view === "recent";
    const sort = recent ? undefined : { sort: prefs.sort_key, direction: prefs.sort_direction };
    this.list?.configure(this.layout(), sort, recent);
  }

  /** Draw the layout control and the sort button or the Recent note. */
  private drawTools(): void {
    const options = this.options as DriveViewOptions;
    const { labels } = options;
    const tools = this.piece(".tools");
    const recent = this.target.view === "recent";
    if (recent) {
      const note = document.createElement("p");
      note.className = "recent-note";
      note.textContent = labels.fileRecentNote;
      tools.replaceChildren(note);
      return;
    }
    const prefs = options.preferences.preferences;
    const parts: HTMLElement[] = [];
    if (prefs.layout === "grid") parts.push(this.sortButton(prefs));
    parts.push(this.layoutControl(prefs.layout));
    tools.replaceChildren(...parts);
  }

  /** The grid's Sort button, "Sort: {key}". */
  private sortButton(prefs: UserPreferences): HTMLButtonElement {
    const { labels } = this.options as DriveViewOptions;
    const key = labels[SORT_LABEL[prefs.sort_key]];
    const button = document.createElement("button");
    button.type = "button";
    button.className = "button secondary sort-button";
    button.setAttribute("aria-haspopup", "menu");
    button.append(icon(prefs.sort_direction === "ascending" ? "arrow-up" : "arrow-down"));
    button.append(document.createTextNode(formatLabel(labels.fileSortButton, { key })));
    button.addEventListener("click", () => this.openSortMenu(button));
    return button;
  }

  /** The segmented list and grid control. */
  private layoutControl(layout: Layout): HTMLElement {
    const { labels } = this.options as DriveViewOptions;
    const group = document.createElement("div");
    group.className = "layout";
    group.setAttribute("role", "group");
    group.setAttribute("aria-label", labels.fileLayouts);
    for (const [value, name, glyph] of [
      ["list", labels.fileViewList, "list"],
      ["grid", labels.fileViewGrid, "grid"],
    ] as const) {
      const button = document.createElement("button");
      button.type = "button";
      button.dataset.layout = value;
      button.setAttribute("aria-label", name);
      button.setAttribute("aria-pressed", String(layout === value));
      button.append(icon(glyph, 16));
      button.addEventListener("click", () => this.setLayout(value));
      group.append(button);
    }
    return group;
  }

  /** Choose a layout and save it. */
  private setLayout(layout: Layout): void {
    (this.options as DriveViewOptions).preferences.update({ layout });
  }

  /** The hooks the listing calls. */
  private hooks(): DriveListHooks {
    return {
      open: (row) => this.openRow(row, "open"),
      toggle: (row, range) => this.toggleRow(row, range),
      toggleAll: () => this.toggleAll(),
      menu: (row, anchor, opener) => this.openItemMenu(row, anchor, opener),
      sort: (key) => this.sortBy(key),
      rename: (row) => this.rename(row, null),
      remove: (row) => void this.removeFile(row),
      more: () => void this.loadMore(),
      hover: (row, target) => this.hover(row, target),
    };
  }

  /** A column head was clicked: sort by it, reversing when it is the sort now. */
  private sortBy(key: SortKey): void {
    const prefs = (this.options as DriveViewOptions).preferences;
    const next = nextSort(
      { sort: prefs.preferences.sort_key, direction: prefs.preferences.sort_direction },
      key,
    );
    prefs.update({ sort_key: next.sort, sort_direction: next.direction });
  }

  /** Tick a row, or the range from the last ticked row. */
  private toggleRow(row: DriveRow, range: boolean): void {
    const keys = this.list?.rowKeys() ?? [];
    if (!range) return this.chosen.toggle(row.key);
    const last = this.chosen.keys().at(-1) ?? row.key;
    this.chosen.selectRange(keys, last, row.key);
  }

  /** The header checkbox: select every row, or clear when every row is selected. */
  private toggleAll(): void {
    const keys = this.list?.rowKeys() ?? [];
    if (this.chosen.state(keys.length) === "all") this.chosen.clear();
    else this.chosen.selectAll(keys);
  }

  /** The selection changed: mark the rows and redraw the bulk bar. */
  private selectionChanged(): void {
    this.list?.refreshSelection();
    this.drawBulk();
  }

  /** The selected rows, in display order. */
  private selectedRows(): DriveRow[] {
    return this.rows.filter((row) => this.chosen.has(row.key));
  }

  /** Draw the bulk action bar above the list while rows are selected, else take it away. */
  private drawBulk(): void {
    const { labels } = this.options as DriveViewOptions;
    const rows = this.selectedRows();
    const existing = this.shadowRoot?.querySelector(".bulk");
    if (rows.length === 0) return existing?.remove();
    const bar = existing ?? document.createElement("div");
    bar.className = "bulk";
    const locale = document.documentElement.lang || "en";
    const text = document.createElement("div");
    text.className = "bulk-text";
    const count = document.createElement("p");
    count.className = "bulk-count";
    count.textContent = formatLabel(labels.fileSelected, {
      count: rows.length.toLocaleString(locale),
    });
    const scope = document.createElement("p");
    scope.className = "bulk-scope";
    scope.textContent = scopeText(rows, labels, locale);
    text.append(count, scope);
    bar.replaceChildren(text, ...this.bulkButtons(rows));
    if (!existing) this.piece(".top").after(bar);
  }

  /** Download ZIP, Move or copy and Remove. */
  private bulkButtons(rows: readonly DriveRow[]): HTMLElement[] {
    const { labels } = this.options as DriveViewOptions;
    const make = (cls: string, text: string, kind: string, on: () => void, off?: boolean) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = `${cls} button ${kind}`;
      button.textContent = text;
      if (off) {
        button.setAttribute("aria-disabled", "true");
        button.title = labels.fileActionUnavailable;
      } else button.addEventListener("click", on);
      return button;
    };
    const files = rows.filter((row) => row.kind === "file");
    const writable = files.length === rows.length && files.every((row) => this.canWrite(row));
    return [
      make("bulk-zip", labels.fileDownloadZip, "secondary", () => this.openZip(rows)),
      make("bulk-move", labels.fileMoveOrCopy, "secondary", () => this.openMove(rows), !writable),
      make(
        "bulk-remove",
        labels.fileRemove,
        "destructive",
        () => void this.removeMany(rows),
        !writable,
      ),
    ];
  }

  /** Whether the writes can address a row: a file with a known library and writes wired. */
  private canWrite(row: DriveRow): boolean {
    return this.actions !== undefined && row.kind === "file" && !!row.libraryId;
  }

  /** The element of the first control that has focus, used as the opener of a menu or dialog. */
  private activeControl(): HTMLElement | null {
    return ((this.shadowRoot as ShadowRoot).activeElement as HTMLElement | null) ?? null;
  }

  /** Open the item menu of a row. */
  private openItemMenu(row: DriveRow, anchor: MenuAnchor, opener: HTMLElement | null): void {
    const { labels } = this.options as DriveViewOptions;
    this.menu?.close(false);
    const entries = itemEntries(labels, row, false);
    const adjusted = entries.map((entry) =>
      entry.kind === "item" && !this.menuAllowed(entry.id, row)
        ? { ...entry, disabled: true, reason: labels.fileActionUnavailable }
        : entry,
    );
    this.menu = openMenu(this.piece(".layer"), anchor, opener, {
      className: "item-menu",
      label: formatLabel(labels.fileActions, { name: row.name }),
      entries: adjusted,
      onChoose: (id) => this.chooseItem(id, row, opener),
    });
  }

  /** Whether the item menu may offer an action for a row on this host. */
  private menuAllowed(id: string, row: DriveRow): boolean {
    if (id === "open" || id === "download" || id === "details")
      return id === "open" || (row.kind === "file" && !!row.libraryId);
    return this.canWrite(row);
  }

  /** Run the chosen item of a row's menu. */
  private chooseItem(id: string, row: DriveRow, opener: HTMLElement | null): void {
    if (id === "open") return this.openRow(row, "open");
    if (id === "details") return this.openRow(row, "details");
    if (id === "rename") return this.rename(row, opener);
    if (id === "move") return this.openMove([row], opener);
    if (id === "remove") void this.removeFile(row);
  }

  /** Open a folder (its link) or announce that a file was opened. */
  private openRow(row: DriveRow, intent: "open" | "details"): void {
    if (row.kind === "folder") {
      const target = this.hrefOf(row);
      if (target !== "") window.location.hash = target.replace(/^#/, "");
      return;
    }
    this.dispatchEvent(
      new CustomEvent("fs-drive-open", { detail: { row, intent }, bubbles: true, composed: true }),
    );
  }

  /** Open the grid's sort menu below its button. */
  private openSortMenu(button: HTMLElement): void {
    const { labels, preferences } = this.options as DriveViewOptions;
    const prefs = preferences.preferences;
    this.menu?.close(false);
    this.menu = openMenu(this.piece(".layer"), { element: button }, button, {
      className: "sort-menu",
      label: labels.fileSortBy,
      entries: sortEntries(labels, {
        sort: prefs.sort_key,
        direction: prefs.sort_direction,
        folders: prefs.folder_placement,
      }),
      onChoose: (id) => this.chooseSort(id),
    });
  }

  /** Apply a chosen sort-menu item. */
  private chooseSort(id: string): void {
    const { preferences } = this.options as DriveViewOptions;
    const [group, value] = id.split(":") as ["sort" | "direction" | "folders", string];
    if (group === "sort") preferences.update({ sort_key: value as SortKey });
    if (group === "direction") preferences.update({ sort_direction: value as SortDirection });
    if (group === "folders")
      preferences.update({ folder_placement: value as UserPreferences["folder_placement"] });
  }

  /** Open the rename dialog of a file. */
  private rename(row: DriveRow, opener: HTMLElement | null): void {
    const { labels } = this.options as DriveViewOptions;
    if (!this.actions || !this.canWrite(row)) return;
    const actions = this.actions;
    renameDialog({
      labels,
      row,
      opener: opener ?? this.activeControl(),
      layer: this.piece(".layer"),
      submit: async (name) => {
        await actions.rename(row, name);
        await this.afterWrite();
      },
    });
  }

  /** Remove one file and offer Undo. */
  private async removeFile(row: DriveRow): Promise<void> {
    const { labels } = this.options as DriveViewOptions;
    if (!this.actions || !this.canWrite(row)) return;
    try {
      const receipt = await this.actions.remove(row);
      showToast({
        labels,
        name: receipt.name,
        version: receipt.version,
        layer: this.piece(".layer"),
        undo: async () => {
          await receipt.undo();
          await this.afterWrite();
        },
      });
      await this.afterWrite();
    } catch (error) {
      this.showActionFailure(error);
    }
  }

  /** Remove each selected file in turn; the first failure stops and is shown. */
  private async removeMany(rows: readonly DriveRow[]): Promise<void> {
    for (const row of rows) {
      if (row.kind === "file") await this.removeFile(row);
    }
    this.chosen.clear();
  }

  /** Show a failed action as an alert toast-less message in the live layer. */
  private showActionFailure(error: unknown): void {
    const { labels } = this.options as DriveViewOptions;
    console.warn(
      "FileSquid could not finish the action",
      error instanceof Error ? error.name : error,
    );
    const layer = this.piece(".layer");
    const note = document.createElement("p");
    note.className = "toast";
    note.setAttribute("role", "alert");
    note.textContent =
      error instanceof UnavailableActionError
        ? labels.fileActionUnavailable
        : labels.fileActionFailed;
    layer.querySelector(".toast")?.remove();
    layer.append(note);
  }

  /** Announce a committed write and read the view again. */
  private async afterWrite(): Promise<void> {
    this.dispatchEvent(new CustomEvent("fs-drive-change", { bubbles: true, composed: true }));
    await this.reload();
  }

  /** Open the Download as ZIP dialog for the selected rows. */
  private openZip(rows: readonly DriveRow[]): void {
    const options = this.options as DriveViewOptions;
    const check = zipCheck(rows);
    const zip = options.zip;
    zipDialog({
      labels: options.labels,
      rows,
      check,
      fileName: `${(this.titleText || "archive").replace(/[\\/]/g, "_")}.zip`,
      locale: document.documentElement.lang || "en",
      opener: this.activeControl(),
      layer: this.piece(".layer"),
      download: zip ? (name, progress) => zip(rows, name, progress) : undefined,
    });
  }

  /** Open the Move or copy dialog. */
  private openMove(rows: readonly DriveRow[], opener: HTMLElement | null = null): void {
    const options = this.options as DriveViewOptions;
    const actions = this.actions;
    moveDialog({
      labels: options.labels,
      rows,
      opener: opener ?? this.activeControl(),
      layer: this.piece(".layer"),
      navigation: options.navigation,
      current: this.target.node,
      submit: async (mode) => {
        if (!actions) return;
        actions.moveOrCopy(rows, mode);
      },
    });
  }

  /** Show a hover card for a row after a short delay, when the setting and the device allow. */
  private hover(row: DriveRow | undefined, target: HTMLElement | undefined): void {
    window.clearTimeout(this.hoverTimer);
    this.piece(".layer").querySelector(".hover-card")?.remove();
    const options = this.options as DriveViewOptions;
    if (!row || !target || !options.preferences.preferences.hover_card || !hoverAllowed()) return;
    this.hoverTimer = window.setTimeout(() => this.showHoverCard(row, target), HOVER_DELAY_MS);
  }

  /** Draw the hover card: the name, the size and the modified time. */
  private showHoverCard(row: DriveRow, target: HTMLElement): void {
    const { labels } = this.options as DriveViewOptions;
    const locale = document.documentElement.lang || "en";
    const size = row.size === undefined ? "" : formatSize(row.size, labels, locale);
    const date = row.modified ? new Date(row.modified).toLocaleString(locale) : "";
    const card = document.createElement("div");
    card.className = "hover-card";
    card.setAttribute("role", "tooltip");
    card.textContent =
      row.kind === "file"
        ? formatLabel(labels.fileHoverCardLabel, { name: row.name, size, date })
        : row.name;
    const box = target.getBoundingClientRect();
    card.style.left = `${box.left + 16}px`;
    card.style.top = `${box.bottom + 4}px`;
    this.piece(".layer").append(card);
  }

  /** Show the placeholder rows and the loading line, keeping the header and title in place. */
  private showLoading(): void {
    const { labels } = this.options as DriveViewOptions;
    this.setState({ kind: "loading" }, true);
    this.list?.showPlaceholders(PLACEHOLDER_ROWS);
    this.announce(formatLabel(labels.fileLoadingName, { name: this.titleText }));
  }

  /** Show the loaded rows, or the empty state. */
  private showRows(): void {
    this.chosen.retain(new Set(this.rows.map((row) => row.key)));
    this.drawItems();
    this.drawBulk();
    this.setState(this.rows.length === 0 ? { kind: "empty" } : undefined, false);
  }

  /** Build the list items (with day headings in Recent) and the footer summary. */
  private drawItems(): void {
    const { labels } = this.options as DriveViewOptions;
    const recent = this.target.view === "recent";
    const items = recent
      ? this.recentItems()
      : this.rows.map((row) => ({ type: "row", row }) as DriveItem);
    this.list?.setItems(items, this.cursor !== null);
    const locale = document.documentElement.lang || "en";
    if (recent) {
      this.announce(
        formatLabel(labels.fileRecentSummary, { count: this.rows.length.toLocaleString(locale) }),
      );
      return;
    }
    const sum = driveSummary(this.rows);
    const text = formatLabel(labels.fileSummary, {
      folders: String(sum.folders),
      files: String(sum.files),
      size: formatSize(sum.bytes, labels, locale),
    });
    const capped = this.complete
      ? ""
      : ` · ${formatLabel(labels.fileLimitReached, { count: String(this.rows.length) })}`;
    this.announce(`${text}${capped}`);
  }

  /** The rows of Recent under their day headings, newest first, within the last seven days. */
  private recentItems(): DriveItem[] {
    const { labels, now } = this.options as DriveViewOptions;
    const clock = now ?? (() => new Date());
    const names = {
      today: labels.fileRecentToday,
      yesterday: labels.fileRecentYesterday,
      earlier: labels.fileRecentEarlier,
    };
    const dated = this.rows
      .map((row) => ({ row, activity: activityOf(row) }))
      .filter((entry) => entry.activity !== undefined)
      .sort((a, b) => (b.activity?.at ?? "").localeCompare(a.activity?.at ?? ""));
    const items: DriveItem[] = [];
    let current: string | undefined;
    for (const { row, activity } of dated) {
      const group = recentGroup(activity?.at ?? "", clock());
      if (!group) continue;
      if (group !== current)
        items.push({ type: "group", key: `group:${group}`, label: names[group] });
      current = group;
      items.push({ type: "row", row });
    }
    return items;
  }

  /** Tell the shell's footer what to say. */
  private announce(text: string): void {
    this.summaryText = text;
    this.dispatchEvent(
      new CustomEvent("fs-summary", { detail: { text }, bubbles: true, composed: true }),
    );
  }

  /** Show a state in place of the list, or the list (state undefined). */
  private setState(state: ViewState | undefined, keepList: boolean): void {
    const states = this.piece<FsStates>("fs-states");
    const showList = state === undefined || (keepList && state.kind === "loading");
    states.hidden = showList;
    if (state && !showList) states.state = state;
    (this.list as DriveList).element.hidden = !showList;
    this.dataset.state = state?.kind ?? "ready";
  }

  /** Show a failed read: view not available, gone, or the failure states. */
  private fail(error: unknown): void {
    const { labels } = this.options as DriveViewOptions;
    console.warn("FileSquid could not read a view", error instanceof Error ? error.name : error);
    this.rows = [];
    this.cursor = null;
    this.drawBulk();
    const online = (this.options as DriveViewOptions).online ?? (() => navigator.onLine);
    if (error instanceof ApiError && error.status === 400) {
      return this.setState({ kind: "unavailable", reason: labels.fileViewUnavailable }, false);
    }
    if (error instanceof ApiError && error.code === "not_found") {
      return this.setState({ kind: "empty", message: labels.navigationGone }, false);
    }
    this.setState(failureState(error, online()), false);
  }

  /** Read the next page when the window nears the loaded end; a changed view starts again. */
  private async loadMore(): Promise<void> {
    const options = this.options;
    if (!options || this.cursor === null || this.loadingMore || this.pages >= MAX_VIEW_PAGES)
      return;
    this.loadingMore = true;
    const generation = this.generation;
    try {
      const page = await options.source.view(this.query(), this.cursor, VIEW_PAGE_SIZE, {
        signal: this.abort?.signal,
      });
      if (generation !== this.generation) return;
      this.rows = [...this.rows, ...page.rows];
      this.cursor = page.nextCursor;
      this.complete = this.complete && page.complete;
      this.pages += 1;
      this.drawItems();
    } catch (error) {
      if (generation !== this.generation) return;
      if (error instanceof ApiError && error.status === 409) void this.reload();
      else this.fail(error);
    } finally {
      this.loadingMore = false;
    }
  }

  /**
   * Show why a view cannot be read, as the failed read of the view itself does: the host's
   * refusal of a view, a gone node, or the failure states. Used by a route that could not find
   * the node to show.
   *
   * @param error What the lookup rejected with.
   */
  showFailure(error: unknown): void {
    this.abort?.abort();
    this.generation += 1;
    this.fail(error);
  }

  /** Focus the title area after a route change (the shell moves focus to main). */
  focusTitle(): void {
    const title = this.piece(".page-title");
    title.tabIndex = -1;
    focusElement(title);
  }
}
