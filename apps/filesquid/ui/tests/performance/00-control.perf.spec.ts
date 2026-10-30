/**
 * Controls of the browsing performance run (Task UI-F3.1, NFR-002, RC-002): what the machine and
 * the harness cost before the app is measured, so no result is read without its floor.
 *
 * - The browser build is recorded beside the machine (the profile NFR-002 asks each run to
 *   declare).
 * - An idle page must keep 60 Hz: more than 1% missed frames means the machine is too busy to
 *   judge the app either way, and the run fails with that reason instead of passing or failing
 *   the app on noise (UI-S-035, environment validity).
 * - The route round trip (a one-byte fetch answered by the fake host) is the floor under every
 *   request the app makes in this harness; it is reported, not asserted.
 */
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { BUDGETS } from "./budgets";
import { Catalogue, PROFILES } from "./fixtures";
import { openView } from "./harness";
import { serveHost } from "./host";
import { Probe } from "./probe";
import { assertBudgets, frames, latency, publish, RESULTS_DIR } from "./report";
import { trackErrors } from "../e2e/support/strict-dom";

const IDLE_FRAMES_MS = 10_000;
const ROUND_TRIPS = 200;

test("the browser build is recorded with the machine", async ({ browser }) => {
  const file = join(RESULTS_DIR, "environment.json");
  const environment = JSON.parse(readFileSync(file, "utf8")) as Record<string, unknown>;
  environment["browser"] = `Chromium ${browser.version()} (headless, Playwright)`;
  writeFileSync(file, JSON.stringify(environment, null, 2));
  expect(String(environment["browser"])).toMatch(/Chromium \d+/);
});

test("control: an idle page keeps 60 Hz, so the machine can judge the app", async ({
  page,
}, info) => {
  const errors = trackErrors(page);
  const probe = await Probe.install(page);
  await serveHost(page, new Catalogue(PROFILES.small));
  await page.goto("/#/settings");
  await expect(page.locator("fs-settings")).toBeVisible();
  await page.waitForTimeout(500);
  await probe.framesStart();
  await page.waitForTimeout(IDLE_FRAMES_MS);
  const intervals = await probe.framesStop();
  const record = frames({
    id: "control-idle-frames",
    title: "Control: frame intervals of an idle page (the machine's own 60 Hz)",
    runs: [intervals],
    app: false,
    minFrames: 550,
  });
  await publish(info, [record]);
  assertBudgets([record]);
  expect(errors).toEqual([]);
});

test("control: the route round trip under every request of the harness", async ({ page }, info) => {
  const session = await openView(page, new Catalogue(PROFILES.small));
  await page.route("**/api/v1/control", (route) =>
    route.fulfill({ body: "x", contentType: "text/plain" }),
  );
  const samples = await page.evaluate(async (count) => {
    const times: number[] = [];
    for (let i = 0; i < count; i += 1) {
      const start = performance.now();
      const response = await fetch("/api/v1/control");
      await response.text();
      times.push(performance.now() - start);
    }
    return times;
  }, ROUND_TRIPS + BUDGETS.samples.warmup);
  const record = latency({
    id: "control-route-round-trip",
    title: "Control: round trip of a one-byte fetch answered by the fake host",
    budget: "reported; the floor under every request in this harness",
    samples: samples.slice(BUDGETS.samples.warmup),
    warmup: BUDGETS.samples.warmup,
  });
  await publish(info, [record]);
  expect(session.errors).toEqual([]);
  expect(session.host.unserved).toEqual([]);
});
