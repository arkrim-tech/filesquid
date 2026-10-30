/**
 * The virtualised listing of `<fs-drive-view>` (UI-F2.8, UI-S-026): a list (`role="table"`) or
 * a grid (`role="list"`) that keeps only the rows near the viewport in the DOM. Rows are keyed
 * by their file id or node handle in a map, never in the DOM, so a row that stays in the window
 * keeps its node (and its focus) across scrolls and refreshes. The listing has one Tab stop (the
 * active row's name control) and the list keys of `listKeyTarget`; Space ticks the active row,
 * F2 renames it, Delete removes it, and the menu key or Shift+F10 opens its menu.
 *
 * The listing owns layout and keys only. What a row means is told to the {@link DriveListHooks}
 * of the view. The row model is the shared {@link DriveRow}, so UI-F2.9's search results mount
 * the same listing.
 */
import { listKeyTarget } from "../files/keys";
import { columnsFor, revealScrollTop, visibleWindow } from "../files/window";
import type { DriveRow } from "../drive/model";
import type { Selection } from "../drive/selection";
import type { SortKey } from "../preferences/model";
import type { MenuAnchor } from "./drive-menu";
import {
  groupHeading,
  gridTile,
  headerRow,
  listRow,
  type RowContext,
  type SortState,
} from "./drive-row-view";

/** The two layouts. */
export type DriveLayout = "list" | "grid";

/** One slot of the listing: a row, or a day heading of the Recent view. */
export type DriveItem =
  | { readonly type: "row"; readonly row: DriveRow }
  | { readonly type: "group"; readonly key: string; readonly label: string };

/** What the view does with what happens in the listing. */
export interface DriveListHooks {
  /** A row was activated: a folder is entered, a file opens. */
  open(row: DriveRow): void;
  /** A row's checkbox was clicked; `range` for a Shift-click. */
  toggle(row: DriveRow, range: boolean): void;
  /** The header checkbox was clicked. */
  toggleAll(): void;
  /** A row's menu was asked for (the actions button, a right click or the menu key). */
  menu(row: DriveRow, anchor: MenuAnchor, opener: HTMLElement | null): void;
  /** A sortable column head was clicked. */
  sort(key: SortKey): void;
  /** F2 on a row. */
  rename(row: DriveRow): void;
  /** Delete on a row. */
  remove(row: DriveRow): void;
  /** The window is near the end of what is loaded. */
  more(): void;
  /** The pointer is over a row (or left them: undefined). */
  hover(row: DriveRow | undefined, target: HTMLElement | undefined): void;
}

/** What the listing needs. */
export interface DriveListOptions extends Omit<RowContext, "recent"> {
  /** The ticked rows. */
  readonly selection: Selection;
  /** True in a right-to-left layout. */
  readonly rtl: () => boolean;
  /** What the view does with the listing's events. */
  readonly hooks: DriveListHooks;
}

/** Row height in rem: 44 px (UI-S-026). */
const LIST_ROW_REM = 2.75;
/** Grid row pitch in rem: a 218 px tile and an 18 px gap (UI-S-026). */
const GRID_ROW_REM = 14.75;
/** Grid column pitch in rem: a 214 px tile and an 18 px gap (UI-S-026). */
const GRID_COLUMN_REM = 14.5;
/** Rows kept in the DOM above and below the visible ones. */
const OVERSCAN = 5;
/** Rows assumed visible when the scrolling area has no measured height yet. */
const FALLBACK_ROWS = 20;

/** Pixels per rem, from the root font size. */
function remPixels(): number {
  const size = parseFloat(getComputedStyle(document.documentElement).fontSize);
  return Number.isFinite(size) && size > 0 ? size : 16;
}

/** The virtualised listing. */
export class DriveList {
  /** The element to put in the page. */
  readonly element: HTMLElement;
  private readonly scroller: HTMLElement;
  private readonly canvas: HTMLElement;
  private readonly nodes = new Map<string, HTMLElement>();
  private readonly signatures = new WeakMap<HTMLElement, string>();
  private readonly itemOf = new WeakMap<HTMLElement, DriveRow>();
  private items: readonly DriveItem[] = [];
  private rowIndices: number[] = [];
  private layout: DriveLayout = "list";
  private recent = false;
  private sort?: SortState;
  private more = false;
  private placeholders = false;
  private chosen?: string;
  private lastTicked?: string;
  private observer?: ResizeObserver;

  /** @param options Labels, locale, clock, links, selection and hooks. */
  constructor(private readonly options: DriveListOptions) {
    this.element = document.createElement("div");
    this.element.className = "table";
    this.scroller = document.createElement("div");
    this.scroller.className = "scroller";
    this.canvas = document.createElement("div");
    this.canvas.className = "space";
    this.scroller.append(this.canvas);
    this.scroller.addEventListener("scroll", () => this.render());
    this.element.addEventListener("focusin", (event) => this.onFocusin(event));
    this.element.addEventListener("click", (event) => this.onClick(event));
    this.element.addEventListener("keydown", (event) => this.onKeydown(event));
    this.element.addEventListener("contextmenu", (event) => this.onContextMenu(event));
    this.element.addEventListener("pointerover", (event) => this.onHover(event, true));
    this.element.addEventListener("pointerout", (event) => this.onHover(event, false));
    this.build();
  }

  /** Follow size changes of the scrolling area. Side effect: observes the element. */
  connect(): void {
    this.observer?.disconnect();
    if (typeof ResizeObserver === "undefined") return;
    this.observer = new ResizeObserver(() => this.render());
    this.observer.observe(this.scroller);
  }

  /** Stop following size changes. Idempotent. */
  disconnect(): void {
    this.observer?.disconnect();
    this.observer = undefined;
  }

  /** True when focus is inside the listing. */
  hasFocus(): boolean {
    const active = this.shadow()?.activeElement;
    return active !== null && active !== undefined && this.element.contains(active);
  }

  /**
   * Show a layout, a sort and the Recent mode, and rebuild the header and the rows.
   *
   * @param layout List or grid.
   * @param sort The sort for the column heads, or undefined for a fixed order.
   * @param recent Whether the Recent layout is shown.
   */
  configure(layout: DriveLayout, sort: SortState | undefined, recent: boolean): void {
    this.layout = layout;
    this.sort = sort;
    this.recent = recent;
    this.placeholders = false;
    this.nodes.clear();
    this.canvas.replaceChildren();
    this.build();
    this.render();
  }

  /**
   * Show these items. A row whose key is still listed keeps its node; when focus was inside the
   * listing and the focused row is gone, focus moves to the active row's control (UI-S-016).
   *
   * @param items Rows and day headings, in order.
   * @param more True when more pages can be loaded.
   */
  setItems(items: readonly DriveItem[], more: boolean): void {
    const hadFocus = this.hasFocus();
    this.placeholders = false;
    this.items = items;
    this.rowIndices = items.flatMap((item, index) => (item.type === "row" ? [index] : []));
    this.more = more;
    const keys = new Set(this.rowKeys());
    if (this.chosen !== undefined && !keys.has(this.chosen)) this.chosen = undefined;
    this.render();
    if (hadFocus && !this.hasFocus()) this.focusPosition(Math.max(0, this.tabPosition()));
  }

  /**
   * Show placeholder rows in place of the rows (mockup 33), keeping the header.
   *
   * @param count How many placeholder rows.
   */
  showPlaceholders(count: number): void {
    this.items = [];
    this.rowIndices = [];
    this.more = false;
    this.placeholders = true;
    this.nodes.clear();
    const rows = Array.from({ length: count }, () => {
      const row = document.createElement("div");
      row.className = "placeholder-row";
      row.setAttribute("aria-hidden", "true");
      return row;
    });
    this.canvas.replaceChildren(...rows);
    this.canvas.style.setProperty("--rows", String(count));
    this.refreshSelection();
  }

  /** Update the checkboxes and row marks of the nodes in the DOM and the header checkbox. */
  refreshSelection(): void {
    for (const [key, node] of this.nodes) {
      const on = this.options.selection.has(key);
      node.toggleAttribute("data-selected", on);
      const box = node.querySelector<HTMLInputElement>("input.check");
      if (box) box.checked = on;
    }
    const all = this.element.querySelector<HTMLInputElement>("input.check-all");
    if (!all) return;
    const state = this.options.selection.state(this.rowKeys().length);
    all.checked = state === "all";
    all.indeterminate = state === "some";
  }

  /**
   * Move focus to a row's control, scrolling it into the window first.
   *
   * @param key The row's key.
   */
  focusRow(key: string): void {
    const position = this.rowKeys().indexOf(key);
    if (position >= 0) this.focusPosition(position);
  }

  /** The keys of the rows (day headings left out), in order. */
  rowKeys(): string[] {
    return this.rowIndices.map((index) => (this.items[index] as { row: DriveRow }).row.key);
  }

  /** The element's shadow root, or null before it is attached. */
  private shadow(): ShadowRoot | null {
    const root = this.element.getRootNode();
    return root instanceof ShadowRoot ? root : null;
  }

  /** Rebuild the header and the scroller for the current layout. */
  private build(): void {
    const grid = this.layout === "grid";
    this.element.dataset.layout = this.layout;
    this.element.classList.toggle("recent", this.recent);
    this.element.setAttribute("role", grid ? "list" : "table");
    this.element.setAttribute("aria-label", this.options.labels.files);
    const rem = grid ? GRID_ROW_REM : LIST_ROW_REM;
    this.canvas.style.setProperty("--row-height", `${rem}rem`);
    const header = grid || this.recent ? [] : [headerRow(this.options.labels, this.sort)];
    this.element.replaceChildren(...header, this.scroller);
    this.refreshSelection();
  }

  /** The rem-to-pixel geometry the window needs now. */
  private geometry(): { rowHeight: number; viewport: number; columns: number } {
    const grid = this.layout === "grid";
    const rem = remPixels();
    const rowHeight = (grid ? GRID_ROW_REM : LIST_ROW_REM) * rem;
    const viewport = this.scroller.clientHeight || FALLBACK_ROWS * rowHeight;
    const columns = grid ? columnsFor(this.scroller.clientWidth + rem, GRID_COLUMN_REM * rem) : 1;
    return { rowHeight, viewport, columns };
  }

  /** Put the rows near the viewport in the DOM and take the others out. */
  render(): void {
    if (this.placeholders) return;
    const { rowHeight, viewport, columns } = this.geometry();
    for (const placeholder of this.canvas.querySelectorAll(".placeholder-row"))
      placeholder.remove();
    const range = visibleWindow({
      count: this.items.length,
      columns,
      rowHeight,
      viewport,
      scrollTop: this.scroller.scrollTop,
      overscan: OVERSCAN,
    });
    this.canvas.style.setProperty("--rows", String(range.rows));
    if (this.layout === "list") {
      const total = this.more ? "-1" : String(this.items.length + 1);
      this.element.setAttribute("aria-rowcount", total);
    } else this.element.removeAttribute("aria-rowcount");
    const wanted = new Set<string>();
    for (let i = range.start; i < range.end; i += 1) {
      wanted.add(this.keyOf(i));
      this.place(i, columns);
    }
    this.prune(wanted);
    this.applyTabStop();
    const near = this.items.length - OVERSCAN * columns;
    if (this.more && range.end >= near) this.options.hooks.more();
  }

  /** The key of the item at `index`. */
  private keyOf(index: number): string {
    const item = this.items[index] as DriveItem;
    return item.type === "row" ? item.row.key : item.key;
  }

  /** Create or update the node of item `index` and position it. */
  private place(index: number, columns: number): void {
    const item = this.items[index] as DriveItem;
    const key = this.keyOf(index);
    const signature = this.signatureOf(item);
    let node = this.nodes.get(key);
    if (!node || this.signatures.get(node) !== signature)
      node = this.replaceNode(item, node, signature);
    node.style.setProperty("--r", String(Math.floor(index / columns)));
    node.style.setProperty("--c", String(index % columns));
    node.setAttribute(
      this.layout === "list" ? "aria-rowindex" : "aria-posinset",
      String(index + 2),
    );
    if (node.parentElement !== this.canvas) this.insertSorted(node, index);
    node.dataset.index = String(index);
  }

  /** What makes a node stale: the layout and every visible field of the row. */
  private signatureOf(item: DriveItem): string {
    if (item.type === "group") return `group|${item.label}`;
    const r = item.row;
    const parts = [this.layout, this.recent, r.name, r.size, r.modified, r.version, r.fileCount];
    return [...parts, r.collection, r.modifiedByMe, r.openedByMe, this.options.href(r)].join("|");
  }

  /** Insert a new node before the first node of a later item, so DOM order follows item order. */
  private insertSorted(node: HTMLElement, index: number): void {
    const later = [...this.canvas.children].find(
      (child) => Number((child as HTMLElement).dataset.index ?? -1) > index,
    );
    this.canvas.insertBefore(node, later ?? null);
  }

  /** Build the node of an item, replacing the old node and keeping focus on its control. */
  private replaceNode(
    item: DriveItem,
    old: HTMLElement | undefined,
    signature: string,
  ): HTMLElement {
    const active = this.shadow()?.activeElement ?? null;
    const hadFocus = old !== undefined && active !== null && old.contains(active);
    const context: RowContext = { ...this.options, recent: this.recent };
    const node = item.type === "group" ? groupHeading(item.label) : this.rowNode(item.row, context);
    this.signatures.set(node, signature);
    old?.replaceWith(node);
    this.nodes.set(item.type === "row" ? item.row.key : item.key, node);
    if (hadFocus) node.querySelector<HTMLElement>(".name")?.focus();
    return node;
  }

  /** The row or tile node of a row. */
  private rowNode(row: DriveRow, context: RowContext): HTMLElement {
    const selected = this.options.selection.has(row.key);
    const node =
      this.layout === "grid" ? gridTile(row, context, selected) : listRow(row, context, selected);
    this.itemOf.set(node, row);
    return node;
  }

  /** Remove the nodes outside the window, except a listed row that has focus. */
  private prune(wanted: ReadonlySet<string>): void {
    const focused = this.shadow()?.activeElement ?? null;
    const listed = new Set(this.items.map((_, index) => this.keyOf(index)));
    for (const [key, node] of this.nodes) {
      const keepFocus = focused !== null && node.contains(focused) && listed.has(key);
      if (wanted.has(key) || keepFocus) continue;
      node.remove();
      this.nodes.delete(key);
    }
  }

  /** Position of the row that is the Tab stop among the rows: the chosen row, else the first. */
  private tabPosition(): number {
    const position = this.rowKeys().indexOf(this.chosen ?? "");
    return position >= 0 ? position : 0;
  }

  /** Make exactly one name control focusable by Tab: the chosen row's, else the first row's. */
  private applyTabStop(): void {
    const stop = this.rowKeys()[this.tabPosition()];
    const first = [...this.canvas.children].find((node) => this.itemOf.has(node as HTMLElement));
    const standIn = this.nodes.has(stop ?? "") ? this.nodes.get(stop as string) : first;
    for (const node of this.nodes.values()) {
      node.querySelector(".name")?.setAttribute("tabindex", node === standIn ? "0" : "-1");
    }
  }

  /** Scroll the row at `position` (among the rows) into the window and focus its control. */
  private focusPosition(position: number): void {
    const index = this.rowIndices[position];
    if (index === undefined) return;
    const { rowHeight, viewport, columns } = this.geometry();
    this.scroller.scrollTop = revealScrollTop({
      index,
      columns,
      rowHeight,
      viewport,
      scrollTop: this.scroller.scrollTop,
    });
    this.render();
    this.nodes.get(this.keyOf(index))?.querySelector<HTMLElement>(".name")?.focus();
  }

  /** The row a UI event came from. */
  private rowFrom(event: Event): { row: DriveRow; node: HTMLElement } | undefined {
    const target = event.target;
    const node = target instanceof Element ? target.closest<HTMLElement>(".row, .tile") : null;
    const row = node ? this.itemOf.get(node) : undefined;
    return node && row ? { row, node } : undefined;
  }

  /** Focus moved into a row: it is now the active row and the Tab stop. */
  private onFocusin(event: Event): void {
    const found = this.rowFrom(event);
    if (!found || this.chosen === found.row.key) return;
    this.chosen = found.row.key;
    this.applyTabStop();
  }

  /** Clicks on the header, the checkbox, the actions button and the name. */
  private onClick(event: Event): void {
    const target = event.target instanceof Element ? event.target : null;
    const sort = target?.closest<HTMLElement>("button.sort");
    if (sort) return this.options.hooks.sort(sort.dataset.sort as SortKey);
    if (target?.closest("input.check-all")) return this.options.hooks.toggleAll();
    const found = this.rowFrom(event);
    if (!found || !target) return;
    const mouse = event as MouseEvent;
    if (target.closest("input.check")) return this.tick(found.row, mouse.shiftKey);
    const actions = target.closest<HTMLElement>("button.actions");
    if (actions) return this.options.hooks.menu(found.row, { element: actions }, actions);
    if (target.closest(".name")) this.openName(found.row, mouse);
  }

  /** Tick a row and remember it as the anchor of a Shift-click range. */
  private tick(row: DriveRow, range: boolean): void {
    this.options.hooks.toggle(row, range && this.lastTicked !== undefined);
    this.lastTicked = row.key;
  }

  /** A click on a name opens it; a modified click on a folder link stays native. */
  private openName(row: DriveRow, mouse: MouseEvent): void {
    const native =
      mouse.ctrlKey || mouse.metaKey || mouse.shiftKey || mouse.altKey || mouse.button !== 0;
    if (row.kind === "folder" && native) return;
    mouse.preventDefault();
    this.options.hooks.open(row);
  }

  /** A right click opens the same menu as the actions button, at the pointer. */
  private onContextMenu(event: Event): void {
    const found = this.rowFrom(event);
    if (!found) return;
    event.preventDefault();
    const mouse = event as MouseEvent;
    const opener = found.node.querySelector<HTMLElement>(".name");
    this.options.hooks.menu(found.row, { x: mouse.clientX, y: mouse.clientY }, opener);
  }

  /** Tell the view which row the pointer is over. */
  private onHover(event: Event, entering: boolean): void {
    const found = this.rowFrom(event);
    const related = (event as PointerEvent).relatedTarget;
    if (!entering && related instanceof Node && found?.node.contains(related)) return;
    this.options.hooks.hover(entering ? found?.row : undefined, entering ? found?.node : undefined);
  }

  /** List keys move the active row; Space ticks it, F2 renames and Delete removes it. */
  private onKeydown(event: Event): void {
    const key = event as KeyboardEvent;
    const found = this.rowFrom(event);
    const onName = (event.target as Element | null)?.classList?.contains("name");
    if (!found || !onName || key.ctrlKey || key.metaKey || key.altKey) return;
    const { hooks } = this.options;
    if (key.key === "F2") return this.act(event, () => hooks.rename(found.row));
    if (key.key === "Delete") return this.act(event, () => hooks.remove(found.row));
    if (key.key === " ") return this.act(event, () => this.tick(found.row, key.shiftKey));
    if (key.key === "ContextMenu" || (key.key === "F10" && key.shiftKey)) {
      const name = found.node.querySelector<HTMLElement>(".name");
      return this.act(event, () => hooks.menu(found.row, { element: name as HTMLElement }, name));
    }
    this.moveFocus(key, found.row);
  }

  /** Move the active row with the list keys. */
  private moveFocus(key: KeyboardEvent, row: DriveRow): void {
    const { columns, viewport, rowHeight } = this.geometry();
    const target = listKeyTarget(key.key, {
      index: this.rowKeys().indexOf(row.key),
      count: this.rowIndices.length,
      columns,
      pageSize: Math.max(1, Math.floor(viewport / rowHeight)),
      rtl: this.options.rtl(),
    });
    if (target !== undefined) this.act(key, () => this.focusPosition(target));
  }

  /** Run `action` for a key the listing handles, so the view does not also handle it. */
  private act(event: Event, action: () => void): void {
    event.preventDefault();
    action();
  }
}
