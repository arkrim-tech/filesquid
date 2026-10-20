/**
 * The virtualised listing of `<fs-file-browser>` (UI-F2.2): a list (`role="table"`) or a grid
 * (`role="list"`) that keeps only the rows near the viewport in the DOM. Rows are keyed by their
 * logical path in a map, never in the DOM, so a row that stays in the window keeps its node
 * (and so its focus) across scrolls and refreshes, and the stored name is never in an
 * attribute (UI-S-019). The listing has one Tab stop (the active row's name control) and the
 * list keys of {@link listKeyTarget}; a row that has focus is never removed by scrolling.
 *
 * The listing owns layout and keys only. What a row means (open it, rename it, remove it, load
 * more) is told to the {@link ListHooks} of the browser.
 */
import type { LabelTable } from "../theme/labels";
import { listKeyTarget } from "../files/keys";
import {
  FALLBACK_VIEWPORT_ROWS,
  GRID_ROW_REM,
  GRID_TILE_MIN_REM,
  LIST_ROW_REM,
  OVERSCAN_ROWS,
  type FileRow,
} from "../files/model";
import { columnsFor, revealScrollTop, visibleWindow } from "../files/window";
import { gridTile, headerRow, listRow, type RowContext } from "./file-row-view";

/** The two layouts. */
export type FileLayout = "list" | "grid";

/** What the browser does with what happens in the listing. */
export interface ListHooks {
  /** A row was activated: a file opens in the viewer, a folder is entered. */
  open(row: FileRow): void;
  /** Focus moved to a row (or left them): the row the toolbar now acts on. */
  choose(row: FileRow | undefined): void;
  /** F2 on a row. */
  rename(row: FileRow): void;
  /** Delete on a row. */
  remove(row: FileRow): void;
  /** The window is near the end of what is loaded. */
  more(): void;
}

/** What the listing needs. */
export interface ListOptions {
  /** The active label table. */
  labels: LabelTable;
  /** BCP 47 tag for numbers and dates. */
  locale: string;
  /** The link of a folder row. */
  href: (row: FileRow) => string;
  /** True in a right-to-left layout. */
  rtl: () => boolean;
  /** What the browser does with the listing's events. */
  hooks: ListHooks;
}

/** Pixels per rem, from the root font size. */
function remPixels(): number {
  const size = parseFloat(getComputedStyle(document.documentElement).fontSize);
  return Number.isFinite(size) && size > 0 ? size : 16;
}

/** The virtualised listing. */
export class FileList {
  /** The element to put in the page. */
  readonly element: HTMLElement;
  private readonly scroller: HTMLElement;
  private readonly canvas: HTMLElement;
  private readonly nodes = new Map<string, HTMLElement>();
  private readonly signatures = new WeakMap<HTMLElement, string>();
  private readonly rowOf = new WeakMap<HTMLElement, FileRow>();
  private readonly indices = new WeakMap<HTMLElement, number>();
  private rows: readonly FileRow[] = [];
  private keys: ReadonlySet<string> = new Set();
  private layout: FileLayout = "list";
  private more = false;
  private chosen?: string;
  private observer?: ResizeObserver;

  /** @param options Labels, locale, links and hooks. */
  constructor(private readonly options: ListOptions) {
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

  /** The row the toolbar acts on: the last row that had focus, while it is still listed. */
  chosenRow(): FileRow | undefined {
    return this.rows.find((row) => row.key === this.chosen);
  }

  /** True when focus is inside the listing. */
  hasFocus(): boolean {
    return this.containsFocus(this.element);
  }

  /**
   * Show another layout. Rebuilds the rows; the chosen row is kept.
   *
   * @param layout `list` or `grid`.
   */
  setLayout(layout: FileLayout): void {
    if (layout === this.layout) return;
    this.layout = layout;
    this.clearNodes();
    this.build();
    this.render();
  }

  /**
   * Show these rows. A row whose key is still listed keeps its node; when focus was inside the
   * listing and the focused row is gone, focus moves to the active row's control (UI-S-016).
   *
   * @param rows The rows, in order.
   * @param more True when more pages can be loaded (the row count is then unknown).
   */
  setRows(rows: readonly FileRow[], more: boolean): void {
    const hadFocus = this.hasFocus();
    this.rows = rows;
    this.keys = new Set(rows.map((row) => row.key));
    this.more = more;
    if (this.chosen !== undefined && !rows.some((row) => row.key === this.chosen))
      this.chosen = undefined;
    this.render();
    if (hadFocus && !this.hasFocus()) this.focusIndex(Math.max(0, this.indexOfTab()));
  }

  /**
   * Move focus to the control of a row, scrolling it into the window first. A row that is not
   * listed leaves focus alone.
   *
   * @param key The row's key (its logical path).
   */
  focusRow(key: string): void {
    const index = this.rows.findIndex((row) => row.key === key);
    if (index >= 0) this.focusIndex(index);
  }

  /** Focus the first row's control (after entering a folder from the keyboard). */
  focusFirst(): void {
    if (this.rows.length > 0) this.focusIndex(0);
  }

  /** True when `node` is, or holds, the focused element of its shadow root. */
  private containsFocus(node: Node): boolean {
    const active = this.shadow()?.activeElement;
    return active !== null && active !== undefined && node.contains(active);
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
    this.element.setAttribute("role", grid ? "list" : "table");
    this.element.setAttribute("aria-label", this.options.labels.files);
    const rem = grid ? GRID_ROW_REM : LIST_ROW_REM;
    this.canvas.style.setProperty("--fs-file-row-height", `${rem}rem`);
    this.element.replaceChildren(
      ...(grid ? [this.scroller] : [headerRow(this.options.labels), this.scroller]),
    );
  }

  /** Drop every row node. */
  private clearNodes(): void {
    this.nodes.clear();
    this.canvas.replaceChildren();
  }

  /** The rem-to-pixel geometry the window needs now. */
  private geometry(): { rowHeight: number; viewport: number; columns: number } {
    const grid = this.layout === "grid";
    const rem = remPixels();
    const rowHeight = (grid ? GRID_ROW_REM : LIST_ROW_REM) * rem;
    const viewport = this.scroller.clientHeight || FALLBACK_VIEWPORT_ROWS * rowHeight;
    const columns = grid ? columnsFor(this.scroller.clientWidth, GRID_TILE_MIN_REM * rem) : 1;
    return { rowHeight, viewport, columns };
  }

  /** Put the rows near the viewport in the DOM and take the others out. */
  render(): void {
    const { rowHeight, viewport, columns } = this.geometry();
    const range = visibleWindow({
      count: this.rows.length,
      columns,
      rowHeight,
      viewport,
      scrollTop: this.scroller.scrollTop,
      overscan: OVERSCAN_ROWS,
    });
    this.canvas.style.setProperty("--rows", String(range.rows));
    this.canvas.style.setProperty("--cols", String(columns));
    this.element.setAttribute("aria-rowcount", this.more ? "-1" : String(this.rows.length + 1));
    const wanted = new Set<string>();
    for (let i = range.start; i < range.end; i += 1) {
      wanted.add((this.rows[i] as FileRow).key);
      this.place(i, columns);
    }
    this.prune(wanted);
    this.applyTabStop();
    if (this.more && range.end >= this.rows.length - OVERSCAN_ROWS * columns)
      this.options.hooks.more();
  }

  /** Create or update the node of row `index` and position it. */
  private place(index: number, columns: number): void {
    const row = this.rows[index] as FileRow;
    const signature = `${this.layout}|${row.name}|${row.size}|${row.modified}|${row.version}|${row.pending}`;
    let node = this.nodes.get(row.key);
    if (!node || this.signatures.get(node) !== signature)
      node = this.replaceNode(row, node, signature);
    node.style.setProperty("--r", String(Math.floor(index / columns)));
    node.style.setProperty("--c", String(index % columns));
    const posinset = String(index + 1);
    if (this.layout === "list") node.setAttribute("aria-rowindex", String(index + 2));
    else node.setAttribute("aria-posinset", posinset);
    if (this.layout === "grid")
      node.setAttribute("aria-setsize", this.more ? "-1" : String(this.rows.length));
    this.indices.set(node, index);
    if (node.parentElement !== this.canvas) this.insertSorted(node, index);
  }

  /** Insert a new node before the first node of a later row, so DOM order follows row order. */
  private insertSorted(node: HTMLElement, index: number): void {
    const later = [...this.canvas.children].find(
      (child) => (this.indices.get(child as HTMLElement) ?? -1) > index,
    );
    this.canvas.insertBefore(node, later ?? null);
  }

  /** Build the node of a row, replacing the old node and keeping focus on its control. */
  private replaceNode(row: FileRow, old: HTMLElement | undefined, signature: string): HTMLElement {
    const context: RowContext = this.options;
    const hadFocus = old !== undefined && this.containsFocus(old);
    const node = this.layout === "grid" ? gridTile(row, context) : listRow(row, context);
    this.signatures.set(node, signature);
    this.rowOf.set(node, row);
    old?.replaceWith(node);
    this.nodes.set(row.key, node);
    if (hadFocus) node.querySelector<HTMLElement>(".name")?.focus();
    return node;
  }

  /** Remove the nodes outside the window, except a listed row that has focus. */
  private prune(wanted: ReadonlySet<string>): void {
    const focused = this.shadow()?.activeElement ?? null;
    for (const [key, node] of this.nodes) {
      const keepFocus = focused !== null && node.contains(focused) && this.keys.has(key);
      if (wanted.has(key) || keepFocus) continue;
      node.remove();
      this.nodes.delete(key);
    }
  }

  /** Index of the row that is the Tab stop: the chosen row, else the first. */
  private indexOfTab(): number {
    const index = this.rows.findIndex((row) => row.key === this.chosen);
    return index >= 0 ? index : 0;
  }

  /** Make exactly one name control focusable by Tab: the chosen row's, else the first row's. */
  private applyTabStop(): void {
    const stop = this.rows[this.indexOfTab()]?.key;
    for (const [key, node] of this.nodes) {
      node.querySelector(".name")?.setAttribute("tabindex", key === stop ? "0" : "-1");
      if (key === this.chosen) node.dataset.active = "";
      else delete node.dataset.active;
    }
  }

  /** Scroll row `index` into the window and focus its control. */
  private focusIndex(index: number): void {
    const { rowHeight, viewport, columns } = this.geometry();
    this.scroller.scrollTop = revealScrollTop({
      index,
      columns,
      rowHeight,
      viewport,
      scrollTop: this.scroller.scrollTop,
    });
    this.render();
    const row = this.rows[index];
    this.nodes
      .get(row?.key ?? "")
      ?.querySelector<HTMLElement>(".name")
      ?.focus();
  }

  /** The row a UI event came from. */
  private rowFrom(event: Event): FileRow | undefined {
    const target = event.target;
    const node = target instanceof Element ? target.closest<HTMLElement>(".row, .tile") : null;
    return node ? this.rowOf.get(node) : undefined;
  }

  /** Focus moved into a row: it is now the active row and the Tab stop. */
  private onFocusin(event: Event): void {
    const row = this.rowFrom(event);
    if (!row || this.chosen === row.key) return;
    this.chosen = row.key;
    this.applyTabStop();
    this.options.hooks.choose(row);
  }

  /** A click on a row's name opens it; modified and non-primary clicks stay native. */
  private onClick(event: Event): void {
    const row = this.rowFrom(event);
    const target = event.target instanceof Element ? event.target.closest(".name") : null;
    if (!row || !target) return;
    const mouse = event as MouseEvent;
    const native =
      mouse.ctrlKey || mouse.metaKey || mouse.shiftKey || mouse.altKey || mouse.button !== 0;
    if (row.kind === "folder" && native) return;
    event.preventDefault();
    this.options.hooks.open(row);
  }

  /** List keys move the active row; F2 renames and Delete removes it. */
  private onKeydown(event: Event): void {
    const key = event as KeyboardEvent;
    const row = this.rowFrom(event);
    if (!row || key.ctrlKey || key.metaKey || key.altKey) return;
    if (key.key === "F2") return this.act(event, () => this.options.hooks.rename(row));
    if (key.key === "Delete") return this.act(event, () => this.options.hooks.remove(row));
    const { columns, viewport, rowHeight } = this.geometry();
    const target = listKeyTarget(key.key, {
      index: this.rows.findIndex((candidate) => candidate.key === row.key),
      count: this.rows.length,
      columns,
      pageSize: Math.max(1, Math.floor(viewport / rowHeight)),
      rtl: this.options.rtl(),
    });
    if (target !== undefined) this.act(event, () => this.focusIndex(target));
  }

  /** Run `action` for a key the listing handles, so the browser does not also handle it. */
  private act(event: Event, action: () => void): void {
    event.preventDefault();
    action();
  }
}
