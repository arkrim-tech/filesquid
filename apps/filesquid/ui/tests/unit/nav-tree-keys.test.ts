// @vitest-environment node
/**
 * UI-S-016 / WAI-ARIA treeview pattern: what each key does in `<fs-nav-tree>`, over the
 * visible items in document order. Pure.
 */
import { describe, expect, it } from "vitest";
import { treeKeyAction, type KeyItem } from "../../src/components/nav-tree-keys";

/**
 * Visible items:
 *   0 Holidays (open)
 *   1   2025 (closed)
 *   2   Archive (leaf)
 *   3 Family (closed)
 *   4 Hobbies (leaf)
 */
const items: KeyItem[] = [
  { name: "Holidays", parent: -1, expanded: true },
  { name: "2025", parent: 0, expanded: false },
  { name: "Archive", parent: 0 },
  { name: "Family", parent: -1, expanded: false },
  { name: "hobbies", parent: -1 },
];

describe("treeKeyAction", () => {
  it("moves to the next and previous visible item, and stops at the ends", () => {
    expect(treeKeyAction("ArrowDown", 0, items, false)).toEqual({ type: "focus", index: 1 });
    expect(treeKeyAction("ArrowUp", 3, items, false)).toEqual({ type: "focus", index: 2 });
    expect(treeKeyAction("ArrowUp", 0, items, false)).toEqual({ type: "none" });
    expect(treeKeyAction("ArrowDown", 4, items, false)).toEqual({ type: "none" });
  });

  it("moves to the first and last visible item with Home and End", () => {
    expect(treeKeyAction("Home", 3, items, false)).toEqual({ type: "focus", index: 0 });
    expect(treeKeyAction("End", 1, items, false)).toEqual({ type: "focus", index: 4 });
  });

  it("opens a closed parent, then moves into it, with the right arrow", () => {
    expect(treeKeyAction("ArrowRight", 3, items, false)).toEqual({ type: "expand", indexes: [3] });
    expect(treeKeyAction("ArrowRight", 0, items, false)).toEqual({ type: "focus", index: 1 });
    expect(treeKeyAction("ArrowRight", 2, items, false)).toEqual({ type: "none" });
  });

  it("closes an open parent, or moves to the parent, with the left arrow", () => {
    expect(treeKeyAction("ArrowLeft", 0, items, false)).toEqual({ type: "collapse", index: 0 });
    expect(treeKeyAction("ArrowLeft", 2, items, false)).toEqual({ type: "focus", index: 0 });
    expect(treeKeyAction("ArrowLeft", 1, items, false)).toEqual({ type: "focus", index: 0 });
    expect(treeKeyAction("ArrowLeft", 4, items, false)).toEqual({ type: "none" });
  });

  it("swaps the left and right arrows in a right-to-left layout", () => {
    expect(treeKeyAction("ArrowLeft", 3, items, true)).toEqual({ type: "expand", indexes: [3] });
    expect(treeKeyAction("ArrowRight", 0, items, true)).toEqual({ type: "collapse", index: 0 });
  });

  it("expands every closed sibling parent with *", () => {
    expect(treeKeyAction("*", 4, items, false)).toEqual({ type: "expand", indexes: [3] });
    expect(treeKeyAction("*", 2, items, false)).toEqual({ type: "expand", indexes: [1] });
  });

  it("moves to the next item starting with a typed character, wrapping, in any case", () => {
    expect(treeKeyAction("h", 0, items, false)).toEqual({ type: "focus", index: 4 });
    expect(treeKeyAction("H", 4, items, false)).toEqual({ type: "focus", index: 0 });
    expect(treeKeyAction("a", 3, items, false)).toEqual({ type: "focus", index: 2 });
    expect(treeKeyAction("z", 0, items, false)).toEqual({ type: "none" });
  });

  it("ignores other keys", () => {
    expect(treeKeyAction("Enter", 0, items, false)).toEqual({ type: "none" });
    expect(treeKeyAction("Tab", 0, items, false)).toEqual({ type: "none" });
    expect(treeKeyAction("F2", 0, items, false)).toEqual({ type: "none" });
  });
});
