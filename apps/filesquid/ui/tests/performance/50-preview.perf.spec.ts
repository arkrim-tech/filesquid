/**
 * Preview readiness on the production build (Task UI-F3.1, NFR-002): a prepared viewer image
 * must "paint the correct image at p95 <= 100 ms from the request; a placeholder, stale image or
 * input spinner MUST NOT pass", with preparation coverage of the exercised folder reported and
 * the input acknowledgment, the first useful preview and the final requested-quality image
 * measured on their own.
 *
 * The preview is the real viewer of the file route with its file panel: Next and Previous send
 * real clicks, and a sample ends when the `<img>` whose `alt` is the new file's display name is
 * loaded and decoded (`complete` and a natural width), so a spinner, a stale image or an image
 * of the previous file cannot satisfy it. Every request is a cache miss: each preview mints a
 * new content handle, so the browser can reuse nothing.
 *
 * Classes: 60 different display-size images (all previewed: coverage 100%), 12-megapixel
 * originals (no prepared derivative exists in the UI yet, so first useful equals final), text
 * (one ranged read of the first 64 KiB), a cold first preview in a new context, and content that
 * cannot be read (the error state must show, never a spinner).
 */
import { expect, test, type Browser, type Page } from "@playwright/test";
import { BUDGETS, DEVICE_SCALE_FACTOR, VIEWPORT } from "./budgets";
import { Catalogue, PROFILES } from "./fixtures";
import { idle, runSamples, type Session } from "./harness";
import { serveHost } from "./host";
import { Probe, type Pred } from "./probe";
import {
  LARGE_COUNT,
  SMALL_COUNT,
  TEXT_COUNT,
  largeName,
  previewFiles,
  smallName,
  textName,
} from "./preview-fixtures";
import { assertBudgets, latency, publish, type Measurement } from "./report";
import { MemoryFileSource } from "../../src/files/memory-source";
import { serveFiles, type FilesApi } from "../e2e/support/files-api";
import { trackErrors } from "../e2e/support/strict-dom";

const BROWSER = "fs-app-shell >> fs-file-browser";
const TITLE = `${BROWSER} >> h2.viewer-title`;
const PREVIEW = `${BROWSER} >> fs-file-preview`;
const IMAGE = `${PREVIEW} >> img`;
const WARMUP = BUDGETS.samples.warmup;

/** A page on the preview route with the host, the files and the probe. */
interface PreviewSession extends Session {
  /** What the file routes saw. */
  readonly files: FilesApi;
}

/** Open the viewer on the first file of a folder of the generated object. */
async function openPreview(
  page: Page,
  browser: Browser,
  folder: string,
  first: string,
  tolerate?: RegExp,
): Promise<PreviewSession> {
  const errors = trackErrors(page, tolerate);
  const probe = await Probe.install(page);
  const catalogue = new Catalogue(PROFILES.small);
  const host = await serveHost(page, catalogue);
  const source = new MemoryFileSource({
    library_id: "lib",
    object_id: "obj",
    files: (await previewFiles(browser)).files,
  });
  // The file routes are registered after the host, so they answer the shared routes first.
  const files = await serveFiles(page, source);
  await page.goto(`/#/files/lib/obj/${folder}`);
  await expect(page.locator("fs-file-browser")).toHaveAttribute("data-state", "ready");
  await page.getByRole("button", { name: first, exact: true }).click();
  await expect(page.locator("fs-file-browser h2.viewer-title")).toHaveText(first);
  return { page, probe, host, catalogue, errors, files };
}

/** The predicate: the viewer names this file and its image is loaded and decoded. */
function imageShown(name: string): { ack: Pred; ready: Pred } {
  const ack: Pred = { k: "text", at: TITLE, equals: name };
  return { ack, ready: { k: "all", of: [ack, { k: "img", at: IMAGE, alt: name }] } };
}

/** Step through a folder with Next and Previous, alternating direction at the ends. */
async function stepThrough(
  session: PreviewSession,
  names: (index: number) => string,
  total: number,
  steps: number,
  predicate: (name: string) => { ack: Pred; ready: Pred },
): Promise<{ ack: number[]; ready: number[]; seen: Set<string> }> {
  const { page } = session;
  const seen = new Set<string>();
  let index = 0;
  let direction = 1;
  const result = await runSamples({
    session,
    warmup: WARMUP,
    count: steps - WARMUP,
    spec: () => {
      if (index + direction < 0 || index + direction >= total) direction = -direction;
      const name = names(index + direction);
      return { events: ["click"], ...predicate(name), timeoutMs: BUDGETS.sampleTimeoutMs };
    },
    trigger: async (i) => {
      index += direction;
      if (i >= WARMUP) seen.add(names(index));
      await page.locator(`fs-file-browser button.${direction > 0 ? "next" : "previous"}`).click();
    },
  });
  return { ...result, seen };
}

/** The records of a class: acknowledgment (title) and the correct image painted. */
function imageRecords(
  id: string,
  title: string,
  run: { ack: number[]; ready: number[] },
  ready: Measurement["notes"],
  budget: { p95: number; note: string },
): Measurement[] {
  return [
    latency({
      id: `${id}-ack`,
      title: `${title}: input acknowledgment (the viewer names the new file)`,
      budget: "NFR-002 acknowledgment p95 <= 100 ms",
      samples: run.ack,
      warmup: WARMUP,
      limit: {
        minSamples: BUDGETS.samples.minimum,
        p95Max: BUDGETS.warmFeedbackP95Ms,
        p99Max: BUDGETS.warmTailP99Ms,
      },
    }),
    latency({
      id: `${id}-image`,
      title: `${title}: the correct decoded image painted`,
      budget: budget.note,
      samples: run.ready,
      warmup: WARMUP,
      limit: { minSamples: BUDGETS.samples.minimum, p95Max: budget.p95 },
      notes: ready,
    }),
  ];
}

test("a prepared display-size image paints at p95 <= 100 ms, over every image of the folder", async ({
  page,
  browser,
}, info) => {
  const session = await openPreview(page, browser, "small", smallName(1));
  const steps = WARMUP + BUDGETS.samples.warm;
  const run = await stepThrough(session, (i) => smallName(i + 1), SMALL_COUNT, steps, imageShown);
  const records = imageRecords(
    "preview-small",
    `Warm preview of a 1280 x 720 PNG`,
    run,
    {
      distinctImagesPreviewed: run.seen.size,
      imagesInFolder: SMALL_COUNT,
      coveragePercent: Math.round((run.seen.size / SMALL_COUNT) * 100),
      contentHandlesMinted: session.files.mints(),
    },
    { p95: BUDGETS.preparedImageP95Ms, note: "NFR-002 prepared viewer image paints p95 <= 100 ms" },
  );
  await publish(info, records);
  assertBudgets(records);
  expect(run.seen.size).toBe(SMALL_COUNT);
  // Every preview is a cache miss: the first preview and each of the steps minted a new handle.
  expect(session.files.mints()).toBe(steps + 1);
  expect(session.errors).toEqual([]);
  expect(session.host.unserved).toEqual([]);
});

test("a 12-megapixel original: first useful equals final, within 2 s warm", async ({
  page,
  browser,
}, info) => {
  const session = await openPreview(page, browser, "large", largeName(1));
  const steps = WARMUP + BUDGETS.samples.warm;
  const run = await stepThrough(session, (i) => largeName(i + 1), LARGE_COUNT, steps, imageShown);
  const records = imageRecords(
    "preview-large",
    "Warm preview of a 4000 x 3000 JPEG (12 megapixels)",
    run,
    {
      distinctImagesPreviewed: run.seen.size,
      imagesInFolder: LARGE_COUNT,
      firstUsefulEqualsFinal: true,
      note: "no prepared derivative exists in the UI yet, so the first useful and the final image are one",
    },
    {
      p95: BUDGETS.largeImageWarmP95Ms,
      note: "UI-S-035 12-megapixel original warm p95 <= 2,000 ms",
    },
  );
  await publish(info, records);
  assertBudgets(records);
  expect(session.errors).toEqual([]);
  expect(session.host.unserved).toEqual([]);
});

test("text reads one ranged request of at most 64 KiB, painted at p95 <= 100 ms", async ({
  page,
  browser,
}, info) => {
  const session = await openPreview(page, browser, "text", textName(1));
  const marker = (name: string) => `NOTE ${name.replace(/\D/g, "")}`;
  const run = await stepThrough(
    session,
    (i) => textName(i + 1),
    TEXT_COUNT,
    WARMUP + BUDGETS.samples.warm,
    (name) => {
      const ack: Pred = { k: "text", at: TITLE, equals: name };
      return {
        ack,
        ready: {
          k: "all",
          of: [ack, { k: "text", at: `${PREVIEW} >> pre`, includes: marker(name) }],
        },
      };
    },
  );
  const ranges = session.files.ranges.filter((range) => range !== "");
  const records = imageRecords(
    "preview-text",
    "Warm preview of a 150 KiB text file",
    run,
    {
      rangedRequests: ranges.length,
      largestRangeBytes: Math.max(
        ...ranges.map((r) => {
          const m = /bytes=(\d+)-(\d+)/.exec(r);
          return m ? Number(m[2]) - Number(m[1]) + 1 : 0;
        }),
      ),
    },
    { p95: BUDGETS.preparedImageP95Ms, note: "NFR-002 prepared view paints p95 <= 100 ms" },
  );
  await publish(info, records);
  assertBudgets(records);
  // Range-read previews: one read of the first 64 KiB for the first preview and one for each
  // step, never the whole file and never a second read of the same preview.
  expect(ranges).toEqual(
    Array.from({ length: WARMUP + BUDGETS.samples.warm + 1 }, () => "bytes=0-65535"),
  );
  expect(session.errors).toEqual([]);
  expect(session.host.unserved).toEqual([]);
});

for (const kind of ["small", "large"] as const) {
  test(`cold: the first ${kind} preview after load, in a new context (20)`, async ({
    browser,
  }, info) => {
    const first = kind === "small" ? smallName(1) : largeName(1);
    const ready: number[] = [];
    for (let i = 0; i < BUDGETS.samples.cold; i += 1) {
      const context = await browser.newContext({
        viewport: VIEWPORT,
        deviceScaleFactor: DEVICE_SCALE_FACTOR,
        timezoneId: "UTC",
      });
      const page = await context.newPage();
      const session = await openPreviewCold(page, browser, kind, first);
      const spec = { events: ["click"], ...imageShown(first), timeoutMs: BUDGETS.sampleTimeoutMs };
      const result = await session.probe.measure(spec, () =>
        page.getByRole("button", { name: first, exact: true }).click(),
      );
      await context.close();
      if (result.timedOut || result.readyMs === null)
        throw new Error(`cold preview ${i} never painted`);
      expect(session.errors).toEqual([]);
      ready.push(result.readyMs);
    }
    const limit = kind === "small" ? BUDGETS.preparedImageP95Ms : BUDGETS.largeImageColdMs;
    const record = latency({
      id: `preview-cold-${kind}`,
      title: `Cold first preview of a ${kind === "small" ? "1280 x 720 PNG" : "12-megapixel JPEG"} (20 new contexts)`,
      budget:
        kind === "small"
          ? "reported: cold behaviour is reported apart from the warm p95 (NFR-002)"
          : "UI-S-035 12-megapixel original cold <= 3,000 ms",
      samples: ready,
      warmup: 0,
      limit:
        kind === "large"
          ? { minSamples: BUDGETS.samples.cold, p95Max: limit, maxMax: limit }
          : undefined,
    });
    await publish(info, [record]);
    assertBudgets([record]);
  });
}

/** A cold page: the viewer route open on the folder's list, before the first file is opened. */
async function openPreviewCold(
  page: Page,
  browser: Browser,
  folder: string,
  first: string,
): Promise<PreviewSession> {
  const errors = trackErrors(page);
  const probe = await Probe.install(page);
  const host = await serveHost(page, new Catalogue(PROFILES.small));
  const source = new MemoryFileSource({
    library_id: "lib",
    object_id: "obj",
    files: (await previewFiles(browser)).files,
  });
  const files = await serveFiles(page, source);
  await page.goto(`/#/files/lib/obj/${folder}`);
  await expect(page.getByRole("button", { name: first, exact: true })).toBeVisible();
  await expect(page.locator("fs-file-browser")).toHaveAttribute("data-state", "ready");
  return { page, probe, host, catalogue: host.catalogue, errors, files };
}

test("content that cannot be read shows the error state within 2 s, never a spinner", async ({
  page,
  browser,
}, info) => {
  const session = await openPreview(page, browser, "small", smallName(1), /status of (410|404)/);
  await idle(page);
  session.files.failContent(1_000_000);
  let index = 0;
  const run = await runSamples({
    session,
    warmup: 0,
    count: BUDGETS.samples.failure,
    spec: () => {
      const name = smallName(index + 2);
      const ack: Pred = { k: "text", at: TITLE, equals: name };
      const failed: Pred = { k: "attr", at: PREVIEW, name: "data-state", value: "error" };
      const alert: Pred = { k: "text", at: `${PREVIEW} >> .media-error`, includes: "" };
      return {
        events: ["click"],
        ack,
        ready: { k: "all", of: [ack, failed, alert] },
        timeoutMs: BUDGETS.sampleTimeoutMs,
      };
    },
    trigger: async () => {
      index += 1;
      await page.locator("fs-file-browser button.next").click();
    },
  });
  const record = latency({
    id: "preview-failure-visible",
    title: "Time until a preview whose content cannot be read shows its error state",
    budget: "UI-S-035 failure visible <= 2,000 ms (never a spinner)",
    samples: run.ready,
    warmup: 0,
    limit: {
      minSamples: BUDGETS.samples.failure,
      p95Max: BUDGETS.failureVisibleMs,
      maxMax: BUDGETS.failureVisibleMs,
    },
    notes: { handlesMinted: session.files.mints() },
  });
  await publish(info, [record]);
  assertBudgets([record]);
  // The refused content reads are tolerated by name (410, 404); nothing else may be logged.
  expect(session.errors).toEqual([]);
  expect(session.host.unserved).toEqual([]);
});
