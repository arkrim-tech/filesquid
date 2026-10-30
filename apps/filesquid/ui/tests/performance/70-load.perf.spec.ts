/**
 * Warm browse under load (Task UI-F3.1, NFR-002, RC-002): "Tests MUST exercise idle operation
 * and concurrent import/migration, thumbnail generation, indexing, provenance scans and batch
 * metadata work ... and resource pressure".
 *
 * - Background work: while the event channel pushes `navigation.change` events at 5 per second
 *   (what concurrent import and indexing make the host announce), row selection and folder
 *   opening are measured with the same gates as when idle.
 * - Resource pressure: with the CPU slowed 4x (DevTools `Emulation.setCPUThrottlingRate`), the
 *   same classes run N = 100 and are reported, not asserted: the budgets are for the declared
 *   reference machine, and a slower machine's numbers must stay visible, not vanish.
 */
import { expect, test, type Page, type TestInfo } from "@playwright/test";
import { BACKGROUND_EVENTS_PER_SECOND, BUDGETS, PRESSURE_CPU_RATE } from "./budgets";
import { FILES_FIRST, folderClass, selectClass, sortClass, WARM } from "./browse-classes";
import { Catalogue, PROFILES } from "./fixtures";
import { devtools, openView } from "./harness";
import { assertBudgets, latency, publish, treeGap, type Measurement } from "./report";

/** A record of one measurement; asserted when `gated`, reported otherwise. */
function record(
  id: string,
  title: string,
  samples: readonly number[],
  gated: boolean,
  warmup: number,
): Measurement {
  return latency({
    id,
    title,
    budget: gated
      ? "NFR-002 p95 <= 100 ms; p99 <= 200 ms (same gates as idle)"
      : `reported at ${PRESSURE_CPU_RATE}x CPU slowdown; budgets are for the reference machine`,
    samples,
    warmup,
    limit: gated
      ? {
          minSamples: BUDGETS.samples.minimum,
          p95Max: BUDGETS.warmFeedbackP95Ms,
          p99Max: BUDGETS.warmTailP99Ms,
        }
      : undefined,
  });
}

test("background work: select and open-folder keep their budgets while events stream in", async ({
  page,
}, info) => {
  const catalogue = new Catalogue(PROFILES.large);
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
  let sent = 0;
  const timer = setInterval(() => {
    sent += 1;
    session.host.pushNavigationChange();
  }, 1000 / BACKGROUND_EVENTS_PER_SECOND);
  try {
    const select = await selectClass(session);
    const records = [
      record(
        "background-select-feedback-large",
        "Select under 5 navigation events per second, Large catalog",
        select.ack,
        true,
        WARM.warmup,
      ),
    ];
    await publish(info, records);
    assertBudgets(records);
    expect(sent).toBeGreaterThan(BACKGROUND_EVENTS_PER_SECOND);
  } finally {
    clearInterval(timer);
  }
  expect(session.errors).toEqual([]);
  expect(session.host.unserved).toEqual([]);
});

test("background work: opening folders under the same stream", async ({ page }, info) => {
  const session = await openView(page, new Catalogue(PROFILES.large));
  const timer = setInterval(
    () => session.host.pushNavigationChange(),
    1000 / BACKGROUND_EVENTS_PER_SECOND,
  );
  try {
    // A reload of the navigation tree can queue a view request for seconds (the page opens six
    // connections to the host at most); that wait is what is measured, so a sample may take up
    // to 20 s before the run stops.
    const folders = await folderClass(session, { ...WARM, timeoutMs: 20_000 });
    const records = [
      record(
        "background-open-folder-feedback-large",
        "Open folder (title) under 5 navigation events per second, Large catalog",
        folders.open.ack,
        true,
        WARM.warmup,
      ),
      record(
        "background-open-folder-result-large",
        "Open folder (rows painted) under 5 navigation events per second, Large catalog",
        folders.open.ready,
        true,
        WARM.warmup,
      ),
    ];
    await publish(info, records);
    expect(session.errors).toEqual([]);
    expect(session.host.unserved).toEqual([]);
    // The same known gap as the idle open-folder class (Finding 4): each navigation event draws
    // the 10,000-node tree again. Every other assertion above stays strict.
    assertBudgets(
      records,
      treeGap("background-open-folder-feedback-large", "background-open-folder-result-large"),
    );
  } finally {
    clearInterval(timer);
  }
});

test("resource pressure: select, sort and open-folder at 4x CPU slowdown are reported", async ({
  page,
}, info) => {
  const session = await openView(page, new Catalogue(PROFILES.large), {
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
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: PRESSURE_CPU_RATE });
  const counts = { warmup: WARM.warmup, count: BUDGETS.samples.pressure };
  try {
    const select = await selectClass(session, counts);
    const sort = await sortClass(session, counts);
    const records = [
      record(
        "pressure-select-feedback-large",
        "Select at 4x CPU slowdown, Large catalog",
        select.ack,
        false,
        counts.warmup,
      ),
      record(
        "pressure-sort-feedback-large",
        "Sort feedback at 4x CPU slowdown, Large catalog",
        sort.ack,
        false,
        counts.warmup,
      ),
      record(
        "pressure-sort-result-large",
        "Sort result at 4x CPU slowdown, Large catalog",
        sort.ready,
        false,
        counts.warmup,
      ),
    ];
    await publish(info, records);
  } finally {
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: 1 });
  }
  expect(session.errors).toEqual([]);
  expect(session.host.unserved).toEqual([]);
  await pressureFolders(page, counts, info);
});

/**
 * The folder class under the same slowdown, in a second page opened with the default
 * preferences (folders on top, which opening a folder needs); reported, not asserted.
 */
async function pressureFolders(
  page: Page,
  counts: { warmup: number; count: number },
  info: TestInfo,
): Promise<void> {
  const other = await page.context().newPage();
  try {
    const session = await openView(other, new Catalogue(PROFILES.large));
    const cdp = await devtools(other);
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: PRESSURE_CPU_RATE });
    const folders = await folderClass(session, counts);
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: 1 });
    const records = [
      record(
        "pressure-open-folder-feedback-large",
        "Open folder (title) at 4x CPU slowdown, Large catalog",
        folders.open.ack,
        false,
        counts.warmup,
      ),
      record(
        "pressure-open-folder-result-large",
        "Open folder (rows painted) at 4x CPU slowdown, Large catalog",
        folders.open.ready,
        false,
        counts.warmup,
      ),
      record(
        "pressure-back-result-large",
        "Back (rows painted) at 4x CPU slowdown, Large catalog",
        folders.back.ready,
        false,
        counts.warmup,
      ),
    ];
    await publish(info, records);
    expect(session.errors).toEqual([]);
    expect(session.host.unserved).toEqual([]);
  } finally {
    await other.close();
  }
}
