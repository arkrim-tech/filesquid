/**
 * First useful folder view and cold readiness (Task UI-F3.1, NFR-002: first useful local
 * viewport <= 3 s for the large catalog; RC-002: cold behaviour measured on its own). Each
 * sample is a new browser context (cold HTTP cache and cold JIT) that opens the folder under
 * test directly from the address and waits, from navigation start, until the viewport holds
 * the real first rows of that folder: the view is ready, the viewport's worth of rows is in the
 * DOM, no row is a placeholder and the first row shows the name the host's order puts first.
 *
 * The three catalog profiles of NFR-002 are seen through the API's bounds (see `fixtures.ts`):
 * Large has the 50,000-album navigation tree (50 pages of 1,000 nodes) beside the largest view
 * the API can answer, 10,000 files and 1,000 folders with `complete: false`. Every load is
 * decomposed (response, script, first view request, ready) so a slow one can be explained.
 * One traced load is recorded with a Playwright trace as evidence.
 */
import { expect, test } from "@playwright/test";
import { BUDGETS, DEVICE_SCALE_FACTOR, VIEWPORT } from "./budgets";
import { BIG_NODE, Catalogue, PROFILES, type ProfileId } from "./fixtures";
import { BIG_QUERY, VIEW, rowsShown } from "./harness";
import { serveHost } from "./host";
import { Probe, type SampleResult } from "./probe";
import { assertBudgets, latency, publish, RESULTS_DIR } from "./report";
import { summarise } from "./stats";
import { trackErrors } from "../e2e/support/strict-dom";
import { join } from "node:path";

const PROFILE_IDS: readonly ProfileId[] = ["small", "medium", "large"];

for (const id of PROFILE_IDS) {
  test(`first useful folder view, ${id} catalog, cold (NFR-002 <= 3 s)`, async ({
    browser,
  }, info) => {
    const catalogue = new Catalogue(PROFILES[id]);
    const first = catalogue.firstNames(BIG_QUERY, 1)[0] as string;
    const ready: number[] = [];
    const parts: Record<string, number[]> = {};
    for (let i = 0; i < BUDGETS.samples.cold; i += 1) {
      const context = await browser.newContext({
        viewport: VIEWPORT,
        deviceScaleFactor: DEVICE_SCALE_FACTOR,
        timezoneId: "UTC",
      });
      const page = await context.newPage();
      const errors = trackErrors(page);
      const probe = await Probe.install(page, {
        events: [],
        startAt: 0,
        ready: rowsShown(VIEW, first),
        timeoutMs: BUDGETS.sampleTimeoutMs,
      });
      const host = await serveHost(page, catalogue);
      await page.goto(`/#/navigation/${BIG_NODE}`);
      const result: SampleResult = await probe.result();
      await context.close();
      if (result.timedOut || result.readyMs === null) {
        throw new Error(
          `cold load ${i} never showed its first rows within ${BUDGETS.sampleTimeoutMs} ms`,
        );
      }
      expect(errors).toEqual([]);
      expect(host.unserved).toEqual([]);
      ready.push(result.readyMs);
      for (const [name, value] of Object.entries(result.milestones ?? {}))
        (parts[name] ??= []).push(value);
    }
    const notes: Record<string, number | string> = {
      profile: id,
      navigationCollections: PROFILES[id].navCollections,
      viewFilesAnswered: Math.min(PROFILES[id].viewFiles, 10_000),
      viewFoldersAnswered: Math.min(PROFILES[id].viewFolders, 1_000),
    };
    for (const [name, values] of Object.entries(parts))
      notes[`median_${name}`] = summarise(values).p50;
    const record = latency({
      id: `cold-first-useful-${id}`,
      title: `Cold first useful folder view, ${id} catalog (20 new contexts)`,
      budget: "NFR-002 first useful local viewport <= 3 s",
      samples: ready,
      warmup: 0,
      limit: {
        minSamples: BUDGETS.samples.cold,
        p95Max: BUDGETS.firstUsefulMs,
        maxMax: BUDGETS.firstUsefulMs,
      },
      notes,
    });
    await publish(info, [record]);
    assertBudgets([record]);
  });
}

test("a traced cold load of the Large catalog is recorded as evidence", async ({
  browser,
}, info) => {
  const catalogue = new Catalogue(PROFILES.large);
  const first = catalogue.firstNames(BIG_QUERY, 1)[0] as string;
  const context = await browser.newContext({
    viewport: VIEWPORT,
    deviceScaleFactor: DEVICE_SCALE_FACTOR,
    timezoneId: "UTC",
  });
  await context.tracing.start({ screenshots: true, snapshots: true });
  const page = await context.newPage();
  const probe = await Probe.install(page, {
    events: [],
    startAt: 0,
    ready: rowsShown(VIEW, first),
    timeoutMs: BUDGETS.sampleTimeoutMs,
  });
  await serveHost(page, catalogue);
  await page.goto(`/#/navigation/${BIG_NODE}`);
  const result = await probe.result();
  const path = join(RESULTS_DIR, "trace-cold-first-useful-large.zip");
  await context.tracing.stop({ path });
  await context.close();
  await info.attach("trace-cold-first-useful-large.zip", { path, contentType: "application/zip" });
  // The traced run is evidence of what the load does, not a timed sample: tracing perturbs it.
  expect(result.timedOut).toBe(false);
});
