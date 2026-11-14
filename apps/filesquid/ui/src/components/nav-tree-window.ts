/**
 * The window of the virtualised navigation tree (Task UI-F3.4, UI-S-035): which rows of one
 * section's tree are in the DOM for the part of that tree that is in view. Pure: the element
 * measures, this decides. Every row is `rowHeight` tall, and a tree is as tall as all its rows,
 * so a scroll position names the rows in view by arithmetic.
 *
 * The overscan is measured from the edges of the view, not from each tree. The sections stack
 * in one scrolling area, so an overscan per tree would add up; measured from the view, the tree
 * items of every section together stay within the rows in view plus two overscans.
 */

/** Rows kept in the DOM above and below the view (UI-S-035: two overscans of 5). */
export const TREE_OVERSCAN = 5;

/** The part of a tree that is in view, in pixels from the tree's own top edge. */
export interface TreeBand {
  /** Where the view starts; negative when the tree starts below the top of the view. */
  readonly top: number;
  /** Where the view ends; negative when the whole tree is below the view. */
  readonly bottom: number;
}

/** What decides the window of one tree. */
export interface TreeWindowInput extends TreeBand {
  /** Number of rows the tree holds. */
  readonly count: number;
  /** Height of one row, in the unit of `top` and `bottom` (pixels). */
  readonly rowHeight: number;
  /** Rows kept above and below the view. */
  readonly overscan: number;
}

/** The rows to keep in the DOM. */
export interface TreeWindowRange {
  /** Index of the first row. */
  readonly start: number;
  /** Index after the last row; equal to `start` when no row is in the window. */
  readonly end: number;
  /** The first row in view (not in the overscan), or -1 when no row is in the window. */
  readonly lead: number;
}

/** A number that should be at least `floor`: a NaN or smaller value becomes `floor`. */
function atLeast(value: number, floor: number): number {
  return Number.isFinite(value) && value > floor ? value : floor;
}

/** A finite number, or 0 for NaN and the infinities. */
function finite(value: number): number {
  return Number.isFinite(value) ? value : 0;
}

/** `value` held between `low` and `high`. */
function clamp(value: number, low: number, high: number): number {
  return Math.min(high, Math.max(low, value));
}

/**
 * The rows of one tree in the window. Bad input (no row height, a NaN edge, a negative count)
 * is treated as the smallest usable value, so the answer is always finite. Pure.
 *
 * @param input The tree's row count and row height, the band in view and the overscan.
 * @returns The first row, the row after the last, and the first row in view.
 */
export function treeWindow(input: TreeWindowInput): TreeWindowRange {
  const count = Math.max(0, Math.floor(finite(input.count)));
  const rowHeight = atLeast(input.rowHeight, 1);
  const pad = Math.floor(atLeast(input.overscan, 0)) * rowHeight;
  const top = finite(input.top);
  const bottom = finite(input.bottom);
  const start = clamp(Math.floor((top - pad) / rowHeight), 0, count);
  const end = clamp(Math.ceil((bottom + pad) / rowHeight), start, count);
  if (end === start) return { start, end, lead: -1 };
  const lead = clamp(Math.floor(Math.max(0, top) / rowHeight), start, end - 1);
  return { start, end, lead };
}
