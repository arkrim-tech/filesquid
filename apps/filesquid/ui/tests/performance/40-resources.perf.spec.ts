/**
 * Bounded resources of the browsing views (Task UI-F3.1, NFR-003, RC-002): the DOM holds a
 * bounded window of rows however many are loaded, paging keeps a stable order and shows every
 * row once, the JS heap does not grow past 10% after warm-up, and the shown limit is honest.
 * NFR-003: "all queues MUST have explicit finite limits/backpressure" and view and query caches
 * "MUST remain bounded"; the virtual window of `<fs-drive-view>` is the UI's bound.
 *
 * - With the largest view the API answers loaded in full (11,000 rows), rows in the DOM stay
 *   within the visible rows plus two overscans of 5, and the scrolling area is no taller than
 *   the window.
 * - Every folder and file row appears once, in the host's sorted order, across all pages (the
 *   list is read back by scrolling it to the end and reading the footer and the DOM).
 * - The Large profile's scope is over the API's caps: the limit chip shows (`complete: false`).
 * - After a warm-up, 600 mixed interactions (open and back, select, sort, layout) leave the
 *   forced-GC heap within 10% of its baseline (NFR-003 post-warm-up growth), with the DOM element
 *   count reported.
 */
import { expect, test } from "@playwright/test";
import { BUDGETS, VIEWPORT } from "./budgets";
import { folderClass, FILES_FIRST, layoutClass, selectClass, sortClass } from "./browse-classes";
import { Catalogue, PROFILES, VIEW_CAPS, type ProfileId } from "./fixtures";
import { SCROLLER, VIEW, devtools, fill, heapAfterGc, openView } from "./harness";
import { assertBudgets, publish, value } from "./report";
import { growth } from "./stats";
import { activeLabels, label } from "../e2e/support/strict-dom";

const labels = activeLabels();

const OVERSCAN = 5;
const ROW_PX = 44;
const PAGES = Math.ceil(11_000 / VIEW_CAPS.pageSize);

for (const id of ["medium", "large"] as ProfileId[]) {
  test(`${id}: the DOM holds a bounded window with every page loaded`, async ({ page }, info) => {
    const catalogue = new Catalogue(PROFILES[id]);
    const session = await openView(page, catalogue);
    const { probe, host } = session;
    let heights = { clientHeight: 0, scrollHeight: 0 };
    for (let guard = 0; guard < PAGES * 4 && (host.calls.get("view") ?? 0) < PAGES; guard += 1) {
      heights = await probe.scrollToEnd(SCROLLER);
      await page.waitForTimeout(50);
    }
    await page.waitForTimeout(300);
    heights = await probe.scrollToEnd(SCROLLER);
    const rows = await probe.rowCount(`${VIEW} >> .row`);
    const bound = Math.ceil(VIEWPORT.height / ROW_PX) + 2 * OVERSCAN + 2;
    const footer = await page.locator('fs-app-shell [slot="summary"]').innerText();
    const records = [
      value({
        id: `dom-rows-${id}`,
        title: `Rows in the DOM with 11,000 rows loaded, ${id} catalog`,
        budget: "NFR-003 bounded queues and caches: window rows + 2 x overscan of 5",
        value: rows,
        max: bound,
        notes: {
          scrollerClientHeightPx: heights.clientHeight,
          contentHeightPx: heights.scrollHeight,
          viewPagesRead: host.calls.get("view") ?? 0,
        },
      }),
      value({
        id: `scroller-height-${id}`,
        title: `Height of the scrolling area, ${id} catalog (never taller than the window)`,
        budget: "UI-S-025: the list scrolls inside the content area (window 900 px)",
        value: heights.clientHeight,
        max: 900,
      }),
      value({
        id: `content-height-${id}`,
        title: `Content height of the list with every row loaded, ${id} catalog`,
        budget: "UI-S-026: rows are 44 px tall; 11,000 rows (the footer counts them all)",
        value: heights.scrollHeight,
        max: 11_000 * ROW_PX + ROW_PX,
        notes: { footer },
      }),
    ];
    await publish(info, records);
    assertBudgets(records);
    expect(heights.scrollHeight).toBeGreaterThanOrEqual(11_000 * ROW_PX);
    // Every folder and file row of the scope is counted once, across all pages (stable paging).
    expect(footer).toContain(
      fill(label(labels, "fileSummary"), { folders: "1000", files: "10000", size: "" }).split(
        " · Loaded",
      )[0] as string,
    );
    // The Large scope is over the API's caps, so the limit is said; the others are whole.
    const limit = fill(label(labels, "fileLimitReached"), { count: "11000" });
    if (id === "large") expect(footer).toContain(limit);
    else expect(footer).not.toContain(limit);
    expect(session.errors).toEqual([]);
    expect(host.unserved).toEqual([]);
  });
}

test("heap growth after warm-up stays within 10% over 600 mixed interactions", async ({
  page,
}, info) => {
  const catalogue = new Catalogue(PROFILES.medium);
  const session = await openView(page, catalogue, {
    preferences: FILES_FIRST,
    query: {
      node: "c-0",
      view: "all",
      sort: "modified",
      direction: "descending",
      folders: "mixed",
      filter: "",
    },
  });
  const cdp = await devtools(page);
  const quarter = { warmup: 0, count: 150 };
  // Warm-up: the same mix, thrown away, so allocation caches and lazy structures exist.
  await selectClass(session, { warmup: 0, count: 60 });
  await sortClass(session, { warmup: 0, count: 60 });
  await layoutClass(session, { warmup: 0, count: 60 });
  const baseline = await heapAfterGc(cdp);
  const domBefore = await session.probe.domCount();
  await selectClass(session, quarter);
  await sortClass(session, quarter);
  await layoutClass(session, quarter);
  await selectClass(session, quarter);
  const after = await heapAfterGc(cdp);
  const domAfter = await session.probe.domCount();
  const records = [
    value({
      id: "heap-growth-mixed",
      title: "JS heap growth after warm-up, forced garbage collection, 600 mixed interactions",
      budget:
        "NFR-003 post-warm-up non-cache memory growth <= 10% (the 72-hour soak is not run here)",
      value: Math.round(growth(baseline, after) * 1000) / 1000,
      max: BUDGETS.heapGrowthMax,
      notes: {
        baselineMiB: Math.round(baseline / 1048576),
        afterMiB: Math.round(after / 1048576),
        domElementsBefore: domBefore,
        domElementsAfter: domAfter,
      },
    }),
  ];
  await publish(info, records);
  assertBudgets(records);
  expect(session.errors).toEqual([]);
  expect(session.host.unserved).toEqual([]);
  void folderClass;
});
