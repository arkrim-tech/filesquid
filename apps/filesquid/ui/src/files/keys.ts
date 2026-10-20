/**
 * The keys of the file listing (UI-F2.2, UI-S-016): where each navigation key moves the active
 * item of a list or grid. The listing has one Tab stop (the active item); these keys move it.
 * Pure.
 */

/** Where the active item is and how the listing is laid out. */
export interface ListKeyInput {
  /** Index of the active item. */
  readonly index: number;
  /** Number of items. */
  readonly count: number;
  /** Items per row: 1 for a list. */
  readonly columns: number;
  /** Items a page key moves over. */
  readonly pageSize: number;
  /** True in a right-to-left layout, where the sideways arrows swap. */
  readonly rtl: boolean;
}

/** A move: a signed step, or the first or last item. */
type Move = number | "first" | "last";

/**
 * The item a key moves to. Moves stop at the ends (nothing wraps). The sideways arrows move
 * only in a grid.
 *
 * @param key A `KeyboardEvent.key` value.
 * @param input The active item and layout.
 * @returns The new index, or undefined when the key is not a listing key here.
 */
export function listKeyTarget(key: string, input: ListKeyInput): number | undefined {
  if (input.count <= 0) return undefined;
  const move = keyMove(key, input);
  if (move === undefined) return undefined;
  const last = input.count - 1;
  if (move === "first") return 0;
  if (move === "last") return last;
  return Math.min(last, Math.max(0, input.index + move));
}

/** The move a key asks for, or undefined for a key that is not a listing key. */
function keyMove(key: string, input: ListKeyInput): Move | undefined {
  const grid = input.columns > 1;
  const forward = input.rtl ? -1 : 1;
  switch (key) {
    case "ArrowDown":
      return input.columns;
    case "ArrowUp":
      return -input.columns;
    case "ArrowRight":
      return grid ? forward : undefined;
    case "ArrowLeft":
      return grid ? -forward : undefined;
    case "PageDown":
      return input.pageSize;
    case "PageUp":
      return -input.pageSize;
    case "Home":
      return "first";
    case "End":
      return "last";
    default:
      return undefined;
  }
}
