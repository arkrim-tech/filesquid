/**
 * The window of the virtualised navigation tree (Task UI-F3.4, UI-S-035): which rows of one
 * section's tree are drawn for the part of it that is in view. The overscan of 5 rows is
 * measured from the edges of the view, not per tree, so the tree items of every section
 * together stay within the rows in view plus two overscans of 5.
 */
import { describe, expect, it } from "vitest";
import { TREE_OVERSCAN, treeWindow } from "../../src/components/nav-tree-window";

/** Row height in pixels (30 px, mockup 55). */
const ROW = 30;

/** The window of a tree of `count` rows whose band in view is `top` to `bottom`. */
function win(count: number, top: number, bottom: number) {
  return treeWindow({ count, rowHeight: ROW, top, bottom, overscan: TREE_OVERSCAN });
}

describe("treeWindow", () => {
  it("has an overscan of 5", () => {
    expect(TREE_OVERSCAN).toBe(5);
  });

  it("holds the rows in view plus 5 above and 5 below", () => {
    expect(win(10_000, 3000, 3600)).toEqual({ start: 95, end: 125, lead: 100 });
  });

  it("starts at the first row when the top of the tree is in view", () => {
    expect(win(10_000, 0, 600)).toEqual({ start: 0, end: 25, lead: 0 });
  });

  it("ends at the last row when the end of the tree is in view", () => {
    expect(win(10_000, 299_400, 300_000)).toEqual({ start: 9975, end: 10_000, lead: 9980 });
  });

  it("holds the first rows of a tree that starts part of the way down the view", () => {
    // The tree's top edge is 200 px below the top of the view, which is 600 px tall.
    expect(win(10_000, -200, 400)).toEqual({ start: 0, end: 19, lead: 0 });
  });

  it("holds only overscan rows of a tree that ended just above the view", () => {
    // 100 rows end at 3,000 px; the view starts 60 px below that.
    expect(win(100, 3060, 3660)).toEqual({ start: 97, end: 100, lead: 99 });
  });

  it("holds only overscan rows of a tree that starts just below the view", () => {
    // The tree starts 60 px below the bottom of the view: 3 of the 5 overscan rows are its.
    expect(win(100, -660, -60)).toEqual({ start: 0, end: 3, lead: 0 });
  });

  it("holds nothing of a tree far above or far below the view", () => {
    expect(win(100, 5000, 5600)).toEqual({ start: 100, end: 100, lead: -1 });
    expect(win(100, -2000, -1400)).toEqual({ start: 0, end: 0, lead: -1 });
  });

  it("holds nothing of an empty tree", () => {
    expect(win(0, 0, 600)).toEqual({ start: 0, end: 0, lead: -1 });
  });

  it("treats bad input as the smallest usable value, so the answer stays finite", () => {
    const bad = treeWindow({
      count: -3,
      rowHeight: 0,
      top: Number.NaN,
      bottom: Number.NaN,
      overscan: Number.NaN,
    });
    expect(bad).toEqual({ start: 0, end: 0, lead: -1 });
    const noHeight = treeWindow({
      count: 50,
      rowHeight: Number.NaN,
      top: 0,
      bottom: 10,
      overscan: 5,
    });
    expect(noHeight.end - noHeight.start).toBeLessThanOrEqual(50);
    expect(Number.isFinite(noHeight.end)).toBe(true);
  });

  it("keeps every section together within the rows in view plus two overscans of 5", () => {
    // Four trees stacked as the sections are: a 34 px head above each, a 38 px Pin toggle and
    // a 16 px gap below it. Their rows: 3 pins, 10,000 collections, 40 and 2,500 others.
    const counts = [3, 10_000, 40, 2500];
    const tops: number[] = [];
    let offset = 0;
    for (const count of counts) {
      tops.push(offset + 34);
      offset += 34 + count * ROW + 38 + 16;
    }
    const view = 583;
    const bound = Math.ceil(view / ROW) + 2 * TREE_OVERSCAN + 1;
    let most = 0;
    for (let scroll = 0; scroll <= offset - view; scroll += 7) {
      const drawn = counts
        .map((count, i) =>
          win(count, scroll - (tops[i] as number), scroll - (tops[i] as number) + view),
        )
        .reduce((sum, range) => sum + (range.end - range.start), 0);
      most = Math.max(most, drawn);
    }
    expect(most).toBeLessThanOrEqual(bound);
    expect(most).toBeGreaterThan(Math.ceil(view / ROW));
  });
});
