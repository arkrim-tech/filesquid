/**
 * Unit tests for the selection model (UI-F2.8, UI-S-006): the state of a listing's checkboxes,
 * the header checkbox's partial state, range selection, the bound, and the change event.
 */
import { describe, expect, it, vi } from "vitest";
import { MAX_SELECTED, Selection } from "../../src/drive/selection";

describe("Selection", () => {
  it("toggles a row and announces only real changes", () => {
    const selection = new Selection();
    const seen = vi.fn();
    selection.addEventListener("change", seen);
    selection.toggle("a");
    selection.toggle("a", true);
    expect(selection.has("a")).toBe(true);
    expect(seen).toHaveBeenCalledTimes(1);
    selection.toggle("a");
    expect(selection.size).toBe(0);
    expect(seen).toHaveBeenCalledTimes(2);
  });

  it("reports none, some and all for the header checkbox", () => {
    const selection = new Selection();
    expect(selection.state(3)).toBe("none");
    selection.toggle("a");
    expect(selection.state(3)).toBe("some");
    selection.selectAll(["a", "b", "c"]);
    expect(selection.state(3)).toBe("all");
    expect(selection.state(0)).toBe("some");
  });

  it("selects the rows between two keys in display order, from the first to the second", () => {
    const selection = new Selection();
    selection.selectRange(["a", "b", "c", "d", "e"], "b", "d");
    expect(selection.keys()).toEqual(["b", "c", "d"]);
  });

  it("selects the same rows when the second key comes before the first", () => {
    const selection = new Selection();
    selection.selectRange(["a", "b", "c", "d", "e"], "d", "b");
    expect(selection.keys()).toEqual(["b", "c", "d"]);
  });

  it("selects just the clicked row when the anchor is not in the view", () => {
    const selection = new Selection();
    selection.selectRange(["a", "b"], "x", "a");
    expect(selection.keys()).toEqual(["a"]);
  });

  it("remembers the last row clicked as the anchor of a range, not the last key added", () => {
    const selection = new Selection();
    const order = ["a", "b", "c", "d", "e"];
    expect(selection.anchor).toBeUndefined();
    selection.toggle("b");
    expect(selection.anchor).toBe("b");
    selection.selectRange(order, "b", "d");
    expect(selection.anchor).toBe("b");
    selection.toggle("c");
    expect(selection.has("c")).toBe(false);
    expect(selection.anchor).toBe("c");
    selection.clear();
    expect(selection.anchor).toBeUndefined();
  });

  it("forgets an anchor whose row left the view", () => {
    const selection = new Selection();
    selection.toggle("a");
    selection.retain(new Set(["b"]));
    expect(selection.anchor).toBeUndefined();
  });

  it("clears, and drops keys that left the view", () => {
    const selection = new Selection();
    selection.selectAll(["a", "b", "c"]);
    selection.retain(new Set(["a", "c"]));
    expect(selection.keys()).toEqual(["a", "c"]);
    selection.clear();
    expect(selection.size).toBe(0);
  });

  it("never holds more than the largest view", () => {
    const selection = new Selection();
    selection.selectAll(Array.from({ length: MAX_SELECTED + 5 }, (_, i) => `k${i}`));
    expect(selection.size).toBe(MAX_SELECTED);
    selection.toggle("extra", true);
    expect(selection.has("extra")).toBe(false);
  });
});
