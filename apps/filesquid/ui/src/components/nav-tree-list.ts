/**
 * The drawn window of one section's tree (Task UI-F3.4, UI-S-035). The section's
 * `ul[role=tree]` is as tall as all its rows and holds only the rows the window names, plus any
 * row the element asks to keep (the one that has focus). Rows are held by row index in a map,
 * so a row that stays in the window keeps its element, its focus and its hover across scrolls,
 * and the children of the list stay in row order however the window moved.
 *
 * The list owns which rows are in the DOM. What a row looks like is the builder's (see
 * `rowElement`), and where the view is and which row is the Tab stop are the element's.
 */
import type { NavigationNode } from "../navigation/model";
import { keyItems, type TreeKeys, type TreeRow } from "./nav-tree-rows";
import type { TreeWindowRange } from "./nav-tree-window";

/** Builds the element of a row, or undefined for a row that draws nothing. */
export type RowBuilder = (index: number) => HTMLLIElement | undefined;

/** One section's tree: its rows, and the ones drawn now. */
export class TreeList {
  /** The drawn rows, by row index. */
  private readonly drawn = new Map<number, HTMLLIElement>();
  /** The first row of each node handle; built when first asked. */
  private byNode?: Map<string, number>;
  /** The rows as the key rules read them; built when first asked. */
  private keyCache?: TreeKeys;
  /** The window of the last draw. */
  private range: TreeWindowRange = { start: 0, end: 0, lead: -1 };

  /**
   * @param id The section's id.
   * @param element The section's `ul[role=tree]`.
   * @param rows The section's rows, drawn or not.
   * @param build Builds the element of a row.
   */
  constructor(
    readonly id: string,
    readonly element: HTMLElement,
    readonly rows: readonly TreeRow[],
    private readonly build: RowBuilder,
  ) {}

  /**
   * Put the rows of a window in the list and take the others out. Side effects: adds and
   * removes children of the list.
   *
   * @param range The window.
   * @param keep Rows to keep although they are outside the window; out-of-range ones are ignored.
   */
  draw(range: TreeWindowRange, keep: readonly number[]): void {
    this.range = range;
    const wanted = new Set<number>();
    for (let index = range.start; index < range.end; index += 1) this.want(wanted, index);
    for (const index of keep) this.want(wanted, index);
    for (const [index, element] of this.drawn) {
      if (wanted.has(index)) continue;
      element.remove();
      this.drawn.delete(index);
    }
    let before: HTMLLIElement | undefined;
    for (const index of [...wanted].sort((a, b) => a - b)) {
      before = this.place(index, before) ?? before;
    }
  }

  /**
   * Draw one row if it is not drawn, where its index puts it. Side effect: may add a child.
   *
   * @param index The row.
   * @returns The row's tree item, or undefined when the row is not a node.
   */
  ensure(index: number): HTMLElement | undefined {
    if (this.rows[index]?.type !== "node") return undefined;
    const earlier = [...this.drawn.keys()].filter((drawn) => drawn < index);
    const before = earlier.length > 0 ? this.drawn.get(Math.max(...earlier)) : undefined;
    this.place(index, before);
    return this.item(index);
  }

  /**
   * The tree item of a drawn row.
   *
   * @param index The row.
   * @returns The `a[role=treeitem]`, or undefined when the row is not drawn or not a node.
   */
  item(index: number): HTMLElement | undefined {
    return this.drawn.get(index)?.querySelector<HTMLElement>('[role="treeitem"]') ?? undefined;
  }

  /** The drawn tree items, in no particular order. */
  items(): HTMLElement[] {
    return [...this.drawn.keys()].flatMap((index) => this.item(index) ?? []);
  }

  /**
   * The tree item that stands in as the Tab stop when the active item is not drawn: the first
   * node in view, else the nearest drawn node of the window.
   *
   * @returns The tree item, or undefined when the window holds no node.
   */
  leadItem(): HTMLElement | undefined {
    const { start, end, lead } = this.range;
    for (let index = lead; index >= 0 && index < end; index += 1) {
      const item = this.item(index);
      if (item) return item;
    }
    for (let index = lead - 1; index >= start; index -= 1) {
      const item = this.item(index);
      if (item) return item;
    }
    return undefined;
  }

  /**
   * Whether a row is inside the window of the last draw (not merely kept).
   *
   * @param index The row.
   * @returns True when the window names it.
   */
  inWindow(index: number): boolean {
    return index >= this.range.start && index < this.range.end;
  }

  /**
   * The row of a node.
   *
   * @param handle A node handle, or undefined.
   * @returns The index of the node's first row, or -1 when the section does not show it.
   */
  rowOf(handle: string | undefined): number {
    if (handle === undefined) return -1;
    this.byNode ??= firstRows(this.rows);
    return this.byNode.get(handle) ?? -1;
  }

  /**
   * The node of a row.
   *
   * @param index The row.
   * @returns The node, or undefined when the row is not a node.
   */
  nodeAt(index: number): NavigationNode | undefined {
    const row = this.rows[index];
    return row?.type === "node" ? row.node : undefined;
  }

  /**
   * The rows as the key rules read them.
   *
   * @param nameOf The visible name of a node, for type-ahead.
   * @returns The nodes with their parents and open state, and the way back to their rows.
   */
  keys(nameOf: (node: NavigationNode) => string): TreeKeys {
    this.keyCache ??= keyItems(this.rows, nameOf);
    return this.keyCache;
  }

  /** Add a row to the wanted set, with the limit line that describes it. */
  private want(wanted: Set<number>, index: number): void {
    const row = this.rows[index];
    if (!row || row.type === "pad") return;
    wanted.add(index);
    // A node that names a limit line as its description needs the line in the DOM with it.
    if (row.type === "node" && row.limit !== undefined) wanted.add(row.limit);
  }

  /**
   * Make sure a row is drawn, after `before` (or first). Side effect: may add a child.
   *
   * @returns The row's element, or undefined for a row that draws nothing.
   */
  private place(index: number, before: HTMLLIElement | undefined): HTMLLIElement | undefined {
    const known = this.drawn.get(index);
    if (known) return known;
    const built = this.build(index);
    if (!built) return undefined;
    this.drawn.set(index, built);
    this.element.insertBefore(built, before ? before.nextSibling : this.element.firstChild);
    return built;
  }
}

/** The first row index of each node handle. */
function firstRows(rows: readonly TreeRow[]): Map<string, number> {
  const out = new Map<string, number>();
  rows.forEach((row, index) => {
    if (row.type === "node" && !out.has(row.node.node)) out.set(row.node.node, index);
  });
  return out;
}
