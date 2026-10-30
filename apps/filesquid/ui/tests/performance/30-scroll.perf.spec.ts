/**
 * Scrolling at 60 Hz on the production build (Task UI-F3.1, NFR-002: "Declared 60 Hz scrolling
 * MUST use a proposed 16.7 ms frame budget ... report stalls and p95/p99 timing rather than
 * averages alone"). The Large catalog's view (10,000 files and 1,000 folders, paged in 200s) is
 * scrolled from inside the page at a constant 6,000 px/s, one step per animation frame, so
 * every frame of the run is one scroll step plus whatever the app does with it, including
 * the next page of rows being read and drawn. Both layouts; a run reverses direction halfway,
 * and one run starts with a jump to the top (navigation reversals and jumps, RC-002).
 *
 * Limits (UI-S-035, defined by this task): a frame is missed above 25.05 ms (1.5 budgets), at
 * most 5% of frames missed, p99 interval <= 50 ms, stalls above 100 ms at most 0.1% of the frames
 * (a V8 major-GC finalisation of 117 to 167 ms shows once in about 1,900 frames; Walkthrough,
 * Finding 5), at least 1,500
 * frames per layout, the first 5 frames of each run discarded. The rows in the DOM stay within
 * the window however far the list has loaded.
 */
import { expect, test } from "@playwright/test";
import { BUDGETS } from "./budgets";
import { FILES_FIRST } from "./browse-classes";
import { Catalogue, PROFILES } from "./fixtures";
import { SCROLLER, VIEW_LOCATOR, openView } from "./harness";
import { assertBudgets, frames, publish, value } from "./report";

const SPEED_PX_PER_SECOND = 6000;
const RUN_MS = 8000;
const RUNS = 4;
/** Visible rows (or tiles) plus two overscans, with room for the columns of a grid row. */
const DOM_BOUND = 120;

for (const layout of ["list", "grid"] as const) {
  test(`${layout}: frames while scrolling stay inside the 60 Hz limits`, async ({ page }, info) => {
    const session = await openView(page, new Catalogue(PROFILES.large), {
      preferences: { ...FILES_FIRST, layout },
      query: {
        node: "c-0",
        view: "all",
        sort: "modified",
        direction: "descending",
        folders: "mixed",
        filter: "",
      },
    });
    const { probe } = session;
    // A warm-up run: the JIT, style caches and the first pages are not what is being judged.
    await probe.scrollRun(SCROLLER, SPEED_PX_PER_SECOND, 3000, false);
    await page.locator(`${VIEW_LOCATOR} .scroller`).evaluate((el) => void (el.scrollTop = 0));
    const runs: number[][] = [];
    let highestDomRows = 0;
    let scrolled = 0;
    for (let run = 0; run < RUNS; run += 1) {
      if (run === 2)
        await page.locator(`${VIEW_LOCATOR} .scroller`).evaluate((el) => void (el.scrollTop = 0));
      const measured = await probe.scrollRun(SCROLLER, SPEED_PX_PER_SECOND, RUN_MS, run % 2 === 1);
      runs.push(measured.intervals);
      highestDomRows = Math.max(highestDomRows, measured.domRows);
      scrolled = Math.max(scrolled, measured.maxScrollTop);
    }
    const longTasks = await probe.longTasks();
    const records = [
      frames({
        id: `scroll-frames-${layout}`,
        title: `Frame intervals while scrolling the ${layout} at ${SPEED_PX_PER_SECOND} px/s, Large catalog`,
        runs,
        app: true,
        notes: {
          runs: RUNS,
          runMs: RUN_MS,
          reversalRuns: RUNS / 2,
          maxScrollTopPx: Math.round(scrolled),
          viewPagesRead: session.host.calls.get("view") ?? 0,
          longTasksOver50ms: longTasks.length,
          longestTaskMs: Math.round(Math.max(0, ...longTasks.map((task) => task.duration))),
        },
      }),
      value({
        id: `scroll-dom-rows-${layout}`,
        title: `Most rows or tiles in the DOM at the end of a scroll run, ${layout}`,
        budget: "NFR-003 bounded window: visible + 2 x overscan of 5",
        value: highestDomRows,
        max: DOM_BOUND,
      }),
    ];
    await publish(info, records);
    assertBudgets(records);
    expect(BUDGETS.minFramesPerLayout).toBeLessThanOrEqual(records[0]?.frames?.frames ?? 0);
    expect(session.errors).toEqual([]);
    expect(session.host.unserved).toEqual([]);
  });
}
