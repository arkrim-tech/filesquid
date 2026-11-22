/**
 * The rows of the virtualised navigation tree (Task UI-F3.4, UI-S-035): a section's open
 * levels as one flat list in document order, so a scroll position names the rows in view and
 * the tree draws only those. A row carries everything its tree item states, counted over the
 * whole level and never over what is drawn (UI-S-016): its level, its place in its level, the
 * size of that level and whether it is open. Pure apart from the context's questions.
 *
 * A level that stopped at its cap (UI-S-017) ends in a limit row, after everything under its
 * last node. The line is two rows tall so its sentence fits the sidebar, so a pad row follows
 * it: every index is still one row height.
 */
import { MAX_DEPTH, type NavigationNode } from "../navigation/model";
import type { KeyItem } from "./nav-tree-keys";
import type { SectionItem, SectionView } from "./nav-tree-view";

/** What the rows ask the element about nodes. */
export interface RowContext {
  /** Whether a parent is open. */
  isOpen(node: NavigationNode): boolean;
  /** A node's loaded children, or undefined while they are not loaded. */
  childrenOf(node: NavigationNode): readonly NavigationNode[] | undefined;
  /** Whether a node's children are loading. */
  isBusy(node: NavigationNode): boolean;
  /** The nodes shown of a node's level when it stopped at its cap, else undefined. */
  limitOf(node: NavigationNode): number | undefined;
}

/** The row of one node. */
export interface NodeRow {
  /** Discriminator. */
  readonly type: "node";
  /** The node. */
  readonly node: NavigationNode;
  /** Depth in the section's tree, 1 for a top-level item (`aria-level`). */
  readonly level: number;
  /** Place among the nodes of its level, from 1 (`aria-posinset`). */
  readonly posinset: number;
  /** Number of nodes in its level (`aria-setsize`). */
  readonly setsize: number;
  /** Index of the parent's row, or -1 for a top-level item. */
  readonly parent: number;
  /** True or false for a parent (open or closed); undefined for a node that cannot open. */
  readonly expanded?: boolean;
  /** Whether the node's children are loading. */
  readonly busy: boolean;
  /** Index of the limit row of this node's level, on the level's last node only. */
  limit?: number;
}

/** The line that says a level stopped at its cap. */
export interface LimitRow {
  /** Discriminator. */
  readonly type: "limit";
  /** The level of the nodes it follows, for its indent. */
  readonly level: number;
  /** How many nodes of the level are shown. */
  readonly count: number;
}

/** The second row height of a limit line; nothing is drawn for it. */
export interface PadRow {
  /** Discriminator. */
  readonly type: "pad";
}

/** One row of a section's tree. */
export type TreeRow = NodeRow | LimitRow | PadRow;

/** The nodes of a section's rows as the key rules read them. */
export interface TreeKeys {
  /** The nodes in document order, each with its parent among them. */
  readonly items: readonly KeyItem[];
  /** The row index of each item. */
  readonly rows: readonly number[];
  /**
   * The item at a row index.
   *
   * @param row A row index.
   * @returns The item's index, or -1 when the row is not a node.
   */
  at(row: number): number;
}

/** One level being walked: its items, depth, parent row and the nodes shown when capped. */
interface Level {
  readonly items: readonly SectionItem[];
  readonly depth: number;
  readonly parent: number;
  readonly limit?: number;
}

/**
 * The rows a section shows now, in document order, down to {@link MAX_DEPTH} levels.
 *
 * @param view The section.
 * @param ctx Open state, loaded children and capped levels.
 * @returns The rows.
 */
export function sectionRows(view: SectionView, ctx: RowContext): TreeRow[] {
  const rows: TreeRow[] = [];
  appendLevel(rows, { items: view.items, depth: 1, parent: -1, limit: view.limit }, ctx);
  return rows;
}

/** Append one level, each node followed by what is under it, then its limit row if capped. */
function appendLevel(rows: TreeRow[], level: Level, ctx: RowContext): void {
  const setsize = level.items.length;
  let last: NodeRow | undefined;
  for (let at = 0; at < setsize; at += 1) {
    const item = level.items[at] as SectionItem;
    const canOpen = level.depth < MAX_DEPTH && expandable(item);
    const open = canOpen && ctx.isOpen(item.node);
    last = {
      type: "node",
      node: item.node,
      level: level.depth,
      posinset: at + 1,
      setsize,
      parent: level.parent,
      expanded: canOpen ? open : undefined,
      busy: canOpen && ctx.isBusy(item.node),
    };
    const index = rows.push(last) - 1;
    if (open) appendLevel(rows, childLevel(item, level.depth + 1, index, ctx), ctx);
  }
  if (level.limit === undefined || !last) return;
  last.limit = rows.push({ type: "limit", level: level.depth, count: level.limit }) - 1;
  rows.push({ type: "pad" });
}

/** The level under an open item: its fixed children, or its loaded children. */
function childLevel(item: SectionItem, depth: number, parent: number, ctx: RowContext): Level {
  const nodes = item.fixed ?? ctx.childrenOf(item.node) ?? [];
  const limit = item.fixed ? item.limit : ctx.limitOf(item.node);
  return { items: nodes.map((node) => ({ node })), depth, parent, limit };
}

/** True when an item can open. */
function expandable(item: SectionItem): boolean {
  return item.fixed ? item.fixed.length > 0 : item.node.has_children;
}

/**
 * The nodes of some rows as the key rules of `nav-tree-keys.ts` read them, with the way back
 * from an item to its row. Pure.
 *
 * @param rows A section's rows.
 * @param nameOf The visible name of a node, for type-ahead.
 * @returns The items, their row indexes and the lookup from a row to its item.
 */
export function keyItems(
  rows: readonly TreeRow[],
  nameOf: (node: NavigationNode) => string,
): TreeKeys {
  const items: KeyItem[] = [];
  const indexes: number[] = [];
  const itemOf = new Map<number, number>();
  rows.forEach((row, index) => {
    if (row.type !== "node") return;
    itemOf.set(index, items.length);
    indexes.push(index);
    const parent = row.parent < 0 ? -1 : (itemOf.get(row.parent) ?? -1);
    items.push({ name: nameOf(row.node), parent, expanded: row.expanded });
  });
  return { items, rows: indexes, at: (row) => itemOf.get(row) ?? -1 };
}
