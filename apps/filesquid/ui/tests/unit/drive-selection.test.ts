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

  it("selects the rows between two keys in display order, either way round", () => {
    const selection = new Selection();
    selection.selectRange(["a", "b", "c", "d", "e"], "d", "b");
    expect(selection.keys().sort()).toEqual(["b", "c", "d"]);
    selection.selectRange(["a", "b"], "x", "a");
    expect(selection.has("a")).toBe(true);
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
