/**
 * The virtual window of the file browser (UI-F2.2): which items of a long list or grid are in
 * the DOM for a scroll position, how many tiles fit a grid row, and how far to scroll to show
 * one item. Pure: the element measures, this decides. Each item sits in a row of `columns`
 * items (one for a list), every row `rowHeight` tall, and the window holds the rows in view
 * plus an overscan above and below, so the DOM stays small however long the listing is.
 */

/** What decides the window. */
export interface WindowInput {
  /** Number of items. */
  readonly count: number;
  /** Items per row: 1 for a list, the tiles that fit for a grid. */
  readonly columns: number;
  /** Height of one row, in the unit of `viewport` and `scrollTop` (pixels). */
  readonly rowHeight: number;
  /** Height of the scrolling area. */
  readonly viewport: number;
  /** How far the area is scrolled. */
  readonly scrollTop: number;
  /** Rows kept above and below the visible ones. */
  readonly overscan: number;
}

/** The items to keep in the DOM. */
export interface WindowRange {
  /** Index of the first item. */
  readonly start: number;
  /** Index after the last item. */
  readonly end: number;
  /** Total rows of the whole list, for `aria-rowcount` and the scroll height. */
  readonly rows: number;
}

/** Clamp a number that should be at least `floor` (a NaN or smaller value becomes `floor`). */
function atLeast(value: number, floor: number): number {
  return Number.isFinite(value) && value > floor ? value : floor;
}

/**
 * The items in the window. Bad inputs (no columns, no row height, a negative scroll) are
 * treated as the smallest usable value, so the answer is always finite and small. Pure.
 *
 * @param input The list, its geometry and scroll position.
 * @returns The first and last item and the total rows.
 */
export function visibleWindow(input: WindowInput): WindowRange {
  const columns = Math.floor(atLeast(input.columns, 1));
  const rowHeight = atLeast(input.rowHeight, 1);
  const count = Math.max(0, Math.floor(input.count));
  const rows = Math.ceil(count / columns);
  const top = atLeast(input.scrollTop, 0);
  const bottom = top + atLeast(input.viewport, 0);
  const firstRow = Math.max(0, Math.floor(top / rowHeight) - input.overscan);
  const lastRow = Math.min(rows, Math.ceil(bottom / rowHeight) + input.overscan);
  return {
    start: Math.min(count, firstRow * columns),
    end: Math.min(count, lastRow * columns),
    rows,
  };
}

/**
 * How many tiles of `minTile` fit across `width`, at least one. Pure.
 *
 * @param width The width of the scrolling area.
 * @param minTile The least width of a tile, in the same unit.
 * @returns The number of columns.
 */
export function columnsFor(width: number, minTile: number): number {
  if (!Number.isFinite(width) || width <= 0 || minTile <= 0) return 1;
  return Math.max(1, Math.floor(width / minTile));
}

/** What decides how far to scroll to show an item. */
export interface RevealInput {
  /** The item to show. */
  readonly index: number;
  /** Items per row. */
  readonly columns: number;
  /** Height of one row. */
  readonly rowHeight: number;
  /** Height of the scrolling area. */
  readonly viewport: number;
  /** The current scroll position. */
  readonly scrollTop: number;
}

/**
 * The scroll position that shows an item: the same one when it is in view, the nearest edge
 * otherwise. Pure.
 *
 * @param input The item and the geometry.
 * @returns The new scroll position.
 */
export function revealScrollTop(input: RevealInput): number {
  const columns = Math.floor(atLeast(input.columns, 1));
  const top = Math.floor(Math.max(0, input.index) / columns) * input.rowHeight;
  const bottom = top + input.rowHeight;
  if (top < input.scrollTop) return top;
  if (bottom > input.scrollTop + input.viewport) return bottom - input.viewport;
  return input.scrollTop;
}
