/**
 * Unit tests for the virtual window and list keys of the file browser (UI-F2.2): which items
 * are in the DOM for a scroll position, how many columns a grid has, how far to scroll to show
 * an item, and where each key moves the active item.
 */
import { describe, expect, it } from "vitest";
import { columnsFor, revealScrollTop, visibleWindow } from "../../src/files/window";
import { listKeyTarget } from "../../src/files/keys";

describe("visibleWindow", () => {
  const base = { columns: 1, rowHeight: 48, viewport: 480, overscan: 5 };

  it("holds the rows in view plus the overscan below at the top of the list", () => {
    expect(visibleWindow({ ...base, count: 1000, scrollTop: 0 })).toEqual({
      start: 0,
      end: 15,
      rows: 1000,
    });
  });

  it("holds the overscan above and below in the middle", () => {
    expect(visibleWindow({ ...base, count: 1000, scrollTop: 4800 })).toEqual({
      start: 95,
      end: 115,
      rows: 1000,
    });
  });

  it("never runs past the last item", () => {
    expect(visibleWindow({ ...base, count: 50, scrollTop: 48 * 45 })).toEqual({
      start: 40,
      end: 50,
      rows: 50,
    });
  });

  it("is empty for no items", () => {
    expect(visibleWindow({ ...base, count: 0, scrollTop: 0 })).toEqual({
      start: 0,
      end: 0,
      rows: 0,
    });
  });

  it("windows a grid by whole rows of tiles", () => {
    const window = visibleWindow({
      count: 100,
      columns: 4,
      rowHeight: 144,
      viewport: 288,
      scrollTop: 1440,
      overscan: 1,
    });
    expect(window).toEqual({ start: 36, end: 52, rows: 25 });
    expect(window.start % 4).toBe(0);
  });

  it("copes with a zero-sized viewport and bad inputs without looping or throwing", () => {
    expect(visibleWindow({ ...base, viewport: 0, count: 100, scrollTop: 0 }).end).toBe(5);
    expect(visibleWindow({ ...base, columns: 0, rowHeight: 0, count: 3, scrollTop: -10 })).toEqual({
      start: 0,
      end: 3,
      rows: 3,
    });
  });

  it("keeps the window small however long the list is", () => {
    const window = visibleWindow({ ...base, count: 100_000, scrollTop: 2_400_000 });
    expect(window.end - window.start).toBeLessThanOrEqual(20);
  });
});

describe("columnsFor", () => {
  it("fits whole tiles in the width, at least one", () => {
    expect(columnsFor(1000, 160)).toBe(6);
    expect(columnsFor(100, 160)).toBe(1);
    expect(columnsFor(0, 160)).toBe(1);
  });
});

describe("revealScrollTop", () => {
  const input = { columns: 1, rowHeight: 48, viewport: 480 };
  it("leaves the scroll position when the item is in view", () => {
    expect(revealScrollTop({ ...input, index: 3, scrollTop: 0 })).toBe(0);
  });
  it("scrolls up to an item above the view", () => {
    expect(revealScrollTop({ ...input, index: 2, scrollTop: 480 })).toBe(96);
  });
  it("scrolls down just far enough to show an item below the view", () => {
    expect(revealScrollTop({ ...input, index: 20, scrollTop: 0 })).toBe(20 * 48 + 48 - 480);
  });
  it("uses the tile's row in a grid", () => {
    expect(revealScrollTop({ ...input, columns: 4, index: 40, scrollTop: 0 })).toBe(
      10 * 48 + 48 - 480,
    );
  });
});

describe("listKeyTarget", () => {
  const list = { count: 100, columns: 1, pageSize: 10, rtl: false };

  it("moves one row with the arrows and stops at the ends", () => {
    expect(listKeyTarget("ArrowDown", { ...list, index: 5 })).toBe(6);
    expect(listKeyTarget("ArrowUp", { ...list, index: 5 })).toBe(4);
    expect(listKeyTarget("ArrowUp", { ...list, index: 0 })).toBe(0);
    expect(listKeyTarget("ArrowDown", { ...list, index: 99 })).toBe(99);
  });

  it("jumps with Home, End, PageUp and PageDown", () => {
    expect(listKeyTarget("Home", { ...list, index: 50 })).toBe(0);
    expect(listKeyTarget("End", { ...list, index: 50 })).toBe(99);
    expect(listKeyTarget("PageDown", { ...list, index: 50 })).toBe(60);
    expect(listKeyTarget("PageUp", { ...list, index: 5 })).toBe(0);
    expect(listKeyTarget("PageDown", { ...list, index: 95 })).toBe(99);
  });

  it("ignores the sideways arrows and other keys in a list", () => {
    expect(listKeyTarget("ArrowLeft", { ...list, index: 5 })).toBeUndefined();
    expect(listKeyTarget("ArrowRight", { ...list, index: 5 })).toBeUndefined();
    expect(listKeyTarget("a", { ...list, index: 5 })).toBeUndefined();
    expect(listKeyTarget("Enter", { ...list, index: 5 })).toBeUndefined();
  });

  it("moves across and down a grid, mirrored in right-to-left", () => {
    const grid = { ...list, columns: 4 };
    expect(listKeyTarget("ArrowRight", { ...grid, index: 5 })).toBe(6);
    expect(listKeyTarget("ArrowLeft", { ...grid, index: 5 })).toBe(4);
    expect(listKeyTarget("ArrowDown", { ...grid, index: 5 })).toBe(9);
    expect(listKeyTarget("ArrowUp", { ...grid, index: 5 })).toBe(1);
    expect(listKeyTarget("ArrowRight", { ...grid, rtl: true, index: 5 })).toBe(4);
    expect(listKeyTarget("ArrowLeft", { ...grid, rtl: true, index: 5 })).toBe(6);
  });

  it("answers nothing for an empty list", () => {
    expect(listKeyTarget("ArrowDown", { ...list, count: 0, index: 0 })).toBeUndefined();
  });
});
