/**
 * Keyboard rules of `<fs-nav-tree>`, after the WAI-ARIA Authoring Practices treeview pattern
 * (UI-S-016), as a pure function over the visible items in document order:
 *
 * - Down / Up: next / previous visible item.
 * - Right: open a closed parent; on an open parent, move to its first child.
 * - Left: close an open parent; otherwise move to the parent.
 * - Home / End: first / last visible item.
 * - `*`: expand every closed parent at the focused item's level under the same parent.
 * - A printable character: the next item whose name starts with it, wrapping, ignoring case.
 *
 * In a right-to-left layout Right and Left swap. Enter is left to the native link.
 */

/** What the key rules need to know about one visible item. */
export interface KeyItem {
  /** The item's visible name, for type-ahead. */
  readonly name: string;
  /** Index of the parent item among the visible items, or -1 for a top-level item. */
  readonly parent: number;
  /** True or false for a parent (open or closed); undefined for an item without children. */
  readonly expanded?: boolean;
}

/** What a key press does. */
export type TreeKeyAction =
  | { readonly type: "focus"; readonly index: number }
  | { readonly type: "expand"; readonly indexes: readonly number[] }
  | { readonly type: "collapse"; readonly index: number }
  | { readonly type: "none" };

const NONE: TreeKeyAction = { type: "none" };

/**
 * Decide what a key does on the item at `index`. Pure.
 *
 * @param key `KeyboardEvent.key`.
 * @param index Index of the focused item among `items`.
 * @param items The visible items, in document order.
 * @param rtl Whether the tree is laid out right to left.
 * @returns The action to take; `none` for keys the tree does not handle.
 */
export function treeKeyAction(
  key: string,
  index: number,
  items: readonly KeyItem[],
  rtl: boolean,
): TreeKeyAction {
  const logical = rtl ? mirrored(key) : key;
  switch (logical) {
    case "ArrowDown":
      return focusAt(index + 1, items);
    case "ArrowUp":
      return focusAt(index - 1, items);
    case "Home":
      return focusAt(0, items);
    case "End":
      return focusAt(items.length - 1, items);
    case "ArrowRight":
      return openOrEnter(index, items);
    case "ArrowLeft":
      return closeOrLeave(index, items);
    case "*":
      return expandSiblings(index, items);
    default:
      return typeAhead(key, index, items);
  }
}

/** Swap Right and Left for a right-to-left layout. */
function mirrored(key: string): string {
  if (key === "ArrowRight") return "ArrowLeft";
  if (key === "ArrowLeft") return "ArrowRight";
  return key;
}

/** Focus the item at `index` when it exists. */
function focusAt(index: number, items: readonly KeyItem[]): TreeKeyAction {
  return index >= 0 && index < items.length ? { type: "focus", index } : NONE;
}

/** Right: open a closed parent, or move into an open one. */
function openOrEnter(index: number, items: readonly KeyItem[]): TreeKeyAction {
  const item = items[index];
  if (item?.expanded === false) return { type: "expand", indexes: [index] };
  if (item?.expanded === true && items[index + 1]?.parent === index) {
    return { type: "focus", index: index + 1 };
  }
  return NONE;
}

/** Left: close an open parent, or move to the parent. */
function closeOrLeave(index: number, items: readonly KeyItem[]): TreeKeyAction {
  const item = items[index];
  if (item?.expanded === true) return { type: "collapse", index };
  return item && item.parent >= 0 ? { type: "focus", index: item.parent } : NONE;
}

/** `*`: expand every closed parent that shares the focused item's parent. */
function expandSiblings(index: number, items: readonly KeyItem[]): TreeKeyAction {
  const parent = items[index]?.parent;
  const indexes = items
    .map((item, i) => (item.parent === parent && item.expanded === false ? i : -1))
    .filter((i) => i >= 0);
  return indexes.length > 0 ? { type: "expand", indexes } : NONE;
}

/** A printable character: the next item, after `index` and wrapping, whose name starts with it. */
function typeAhead(key: string, index: number, items: readonly KeyItem[]): TreeKeyAction {
  if ([...key].length !== 1 || key.trim() === "") return NONE;
  const wanted = key.toLocaleLowerCase();
  for (let step = 1; step <= items.length; step += 1) {
    const at = (index + step) % items.length;
    if (items[at]?.name.toLocaleLowerCase().startsWith(wanted)) return { type: "focus", index: at };
  }
  return NONE;
}
