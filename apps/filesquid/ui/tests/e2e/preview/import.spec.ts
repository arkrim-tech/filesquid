/**
 * The upload flow against the production build (`vite build` plus `vite preview`) at the
 * mockups' 1440 × 900 window, in light and dark (UI-1.2, UI-S-024, UI-S-028, UI-S-029, UI-S-013):
 * mockup 52 (the destination step), 18 (the review), 19 (the Jobs panel, the header button and the
 * footer status) and 48 (the refusals). Each test asserts the custom elements are defined, every
 * listed string comes from the active label table, the regions' boxes match the mockup within
 * 2 px (a dialog's regions are measured from its own corner, because the window centres it), the
 * constants are the browser's computed styles, the review comes before the job starts and Cancel
 * discards the plan, and there is no console error. The fake host answers as the API states.
 */
import { expect, test, type Locator, type Page } from "@playwright/test";
import {
  activeLabels,
  expectElementsDefined,
  label,
  rgbOf,
  setScheme,
  trackErrors,
  type Scheme,
} from "../support/strict-dom";
import { COLOURS, look } from "../support/fidelity";
import {
  MOCKUP_ITEMS,
  PLAN,
  jobView,
  plannedFiles,
  planWith,
  serveImports,
  type ImportApi,
} from "../support/import-api";
import { jobItem } from "../../support/import-fakes";
import type { JobItem, JobView } from "../../../src/imports/model";

const labels = activeLabels();
const SCHEMES: readonly Scheme[] = ["light", "dark"];

/** Assert a region's box within 2 px of the mockup's (each key optional), reporting every miss. */
async function expectBox(
  locator: Locator,
  expected: { x?: number; y?: number; width?: number; height?: number },
): Promise<void> {
  const box = await locator.first().boundingBox();
  expect(box, "the region is rendered").not.toBeNull();
  for (const key of ["x", "y", "width", "height"] as const) {
    const want = expected[key];
    if (want === undefined) continue;
    const got = box?.[key] ?? Number.NaN;
    expect
      .soft(Math.abs(got - want), `${key} ${got} is within 2 px of ${want}`)
      .toBeLessThanOrEqual(2);
  }
}

/** Assert a region's box measured from its parent's top-left corner, within 2 px. */
async function expectWithin(
  parent: Locator,
  child: Locator,
  expected: { x?: number; y?: number; width?: number; height?: number },
): Promise<void> {
  const outer = await parent.first().boundingBox();
  const inner = await child.first().boundingBox();
  expect(outer, "the parent is rendered").not.toBeNull();
  expect(inner, "the region is rendered").not.toBeNull();
  const got = {
    x: (inner?.x ?? 0) - (outer?.x ?? 0),
    y: (inner?.y ?? 0) - (outer?.y ?? 0),
    width: inner?.width ?? 0,
    height: inner?.height ?? 0,
  };
  for (const key of ["x", "y", "width", "height"] as const) {
    const want = expected[key];
    if (want === undefined) continue;
    expect(
      Math.abs(got[key] - want),
      `${key} ${got[key]} is within 2 px of ${want}`,
    ).toBeLessThanOrEqual(2);
  }
}

/** Open a folder's view, then the New menu's Upload files… item. */
async function openFromNewMenu(page: Page): Promise<void> {
  await page.goto("/#/navigation/c2");
  await expect(page.locator("fs-navigation-view fs-drive-view")).toHaveAttribute(
    "data-state",
    "ready",
  );
  await page.locator("fs-app-shell nav .new").click();
  await page.locator("fs-new-menu [role=menuitem]").first().click();
  await expect(page.locator("fs-import-dialog [role=dialog]")).toBeVisible();
}

/** In the open dialog's tree, choose the Site photos folder of the Harbour project. */
async function chooseSiteFolder(page: Page): Promise<void> {
  const tree = dialog(page).locator("fs-destination-picker");
  await tree.locator('li[data-node="c2"] .chevron').click();
  await tree.locator('li[data-node="f3"]').click();
  await expect(dialog(page).locator(".destination-field")).toHaveValue(
    "Harbour project › Site photos",
  );
}

/** Open the dialog with no destination, as an upload from anywhere does. */
async function openWithoutDestination(page: Page): Promise<void> {
  await page.goto("/#/navigation/l1");
  await expect(page.locator("fs-app-shell")).toBeVisible();
  await page.evaluate(() =>
    document.dispatchEvent(
      new CustomEvent("fs-new-action", { detail: { action: "upload-files" } }),
    ),
  );
  await expect(page.locator("fs-import-dialog [role=dialog]")).toBeVisible();
  await expect(page.locator("fs-import-dialog fs-destination-picker li").first()).toBeVisible();
}

const dialog = (page: Page) => page.locator("fs-import-dialog [role=dialog]");
const panel = (page: Page) => page.locator("fs-jobs-panel .panel");

// Transitions are off, so a colour read right after a state change is the final one.
test.use({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2, reducedMotion: "reduce" });

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => setScheme(page, scheme));

    test("destination step (mockup 52): regions, strings and constants, nothing sent before a destination", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await serveImports(page);
      await openWithoutDestination(page);
      await expectElementsDefined(page, [
        "fs-import-dialog",
        "fs-jobs-panel",
        "fs-destination-picker",
        "fs-new-menu",
      ]);
      const box = dialog(page);
      await expectBox(box, { x: 380, width: 680, height: 680 });
      const title = box.locator("h2");
      await expect(title).toHaveText(label(labels, "importTitle"));
      expect(await look(title)).toMatchObject({
        size: "22px",
        weight: "700",
        colour: rgbOf(c.heading),
      });
      expect((await look(title)).family).toMatch(/^Georgia/);
      expect(await look(box)).toMatchObject({ background: rgbOf(c.overlay), radius: "10px" });
      await expect(box.locator(".intro")).toHaveText(label(labels, "importIntro"));
      expect(await look(box.locator(".intro"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      await expect(box.locator("legend")).toHaveText(label(labels, "importSourceLegend"));
      await expect(box.locator(".radios label")).toHaveText([
        label(labels, "importSourceFiles"),
        label(labels, "importSourceFolder"),
      ]);
      await expect(box.locator('input[value="files"]')).toBeChecked();
      expect(await look(box.locator('input[value="files"]'))).toMatchObject({
        borderColour: rgbOf(c.primary),
      });
      expect(await look(box.locator('input[value="folder"]'))).toMatchObject({
        borderColour: rgbOf(c.control),
      });
      await expect(box.locator(".field-head")).toHaveText(
        `${label(labels, "importDestination")}${label(labels, "importRequired")}`,
      );
      const field = box.locator(".destination-field");
      await expectWithin(box, field, { x: 28, y: 192, width: 624, height: 36 });
      expect(await look(field)).toMatchObject({ size: "14px", radius: "6px" });
      await expect(field).toHaveAttribute(
        "placeholder",
        label(labels, "importDestinationPlaceholder"),
      );
      await expect(field).toBeFocused();
      const tree = box.locator(".picker");
      await expectWithin(box, tree, { x: 28, y: 236, width: 624, height: 300 });
      const list = box.locator("fs-destination-picker ul");
      expect(await look(list)).toMatchObject({ radius: "8px", borderColour: rgbOf(c.strong) });
      const libraryRow = box.locator('fs-destination-picker li[data-node="l1"]');
      await expectBox(libraryRow, { height: 31 });
      expect(await look(libraryRow)).toMatchObject({ colour: rgbOf(c.muted), weight: "500" });
      await expect(libraryRow.locator(".not-destination")).toHaveText(
        label(labels, "importNotDestination"),
      );
      await expect(box.locator('fs-destination-picker li[data-node="v1"]')).toHaveCount(0);
      await expect(box.locator(".help")).toHaveText(label(labels, "importDestinationNote"));
      const start = box.locator('button[data-action="start"]');
      await expect(start).toHaveText(label(labels, "importChooseFiles"));
      await expect(start).toBeDisabled();
      await expectWithin(box, start, { y: 616, height: 36 });
      expect(await look(start)).toMatchObject({
        background: rgbOf(c.raised),
        colour: rgbOf(c.muted === "#777777" ? "#999999" : "#6a6a6a"),
      });
      await expectWithin(box, box.locator('button[data-action="cancel"]'), {
        y: 616,
        height: 36,
      });
      await expect(box.locator(".foot .reason")).toHaveText(
        label(labels, "importChooseDestinationFirst"),
      );
      await expect(box).not.toContainText(/\bImport\b/);
      start.click({ force: true }).catch(() => undefined);
      expect(api.imports.requests).toEqual([]);
      expect(api.planBodies).toEqual([]);
      expect(errors).toEqual([]);
    });

    test("choosing a folder in the tree enables the start control, which plans with the destination's IDs", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await serveImports(page);
      await openWithoutDestination(page);
      const box = dialog(page);
      await box.locator('fs-destination-picker li[data-node="c2"] .chevron').click();
      await box.locator('fs-destination-picker li[data-node="f3"]').click();
      await expect(box.locator(".destination-field")).toHaveValue("Harbour project › Site photos");
      const start = box.locator('button[data-action="start"]');
      await expect(start).toBeEnabled();
      expect(await look(start)).toMatchObject({
        background: rgbOf(c.primary),
        colour: rgbOf(c.onPrimary),
        radius: "7px",
        weight: "600",
      });
      await start.click();
      await expect(box).toHaveClass(/review/);
      expect(api.planBodies).toHaveLength(1);
      expect(api.planBodies[0]).toMatchObject({
        library_id: "lib-1",
        collection_id: "col-harbour",
        folder: "Site photos",
        source: "files",
      });
      expect(api.lookups).toEqual(["f3"]);
      expect(errors).toEqual([]);
    });

    test("opening from a collection's New menu starts with that collection chosen", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await serveImports(page);
      await openFromNewMenu(page);
      await expect(dialog(page).locator(".destination-field")).toHaveValue("Harbour project");
      await expect(dialog(page).locator('button[data-action="start"]')).toBeEnabled();
      await expect(page.locator("fs-new-menu")).not.toContainText(/\bImport\b/);
      expect(errors).toEqual([]);
    });

    test("planning is shown without a percentage, then the review (mockup 18): regions, strings and constants", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await serveImports(page);
      api.planDelayMs = 400;
      await openFromNewMenu(page);
      await chooseSiteFolder(page);
      await dialog(page).locator('button[data-action="start"]').click();
      const planning = dialog(page).locator(".planning");
      await expect(planning).toHaveText(label(labels, "importPlanning"));
      await expect(planning).not.toContainText("%");
      const box = dialog(page);
      await expect(box).toHaveClass(/review/);
      await expectBox(box, { x: 290, width: 860, height: 720 });
      await expect(box.locator("h2")).toHaveText("Upload 214 files");
      await expect(box.locator(".volume")).toHaveText("From the volume EOS_DIGITAL");
      const cards = box.locator(".card");
      await expect(cards).toHaveCount(4);
      const xs = [28, 232, 436, 640];
      for (const [index, x] of xs.entries()) {
        await expectWithin(box, cards.nth(index), { x, y: 96, width: 192, height: 72 });
      }
      expect(await look(cards.first())).toMatchObject({
        background: rgbOf(c.raised),
        radius: "8px",
      });
      const count = await look(cards.first().locator(".count"));
      expect(count).toMatchObject({ size: "21px", weight: "600", colour: rgbOf(c.heading) });
      expect(count.family).toMatch(/^Georgia/);
      await expect(cards.locator(".count")).toHaveText(["214", "196", "18", "3.1 GB"]);
      await expect(cards.locator(".caption")).toHaveText([
        label(labels, "importCountFound"),
        label(labels, "importCountNew"),
        label(labels, "importCountPresent"),
        label(labels, "importCountToCopy"),
      ]);
      const files = box.locator("ul.files");
      await expectWithin(box, files, { x: 28, y: 214, width: 804, height: 236 });
      expect(await look(files)).toMatchObject({ radius: "8px", borderColour: rgbOf(c.strong) });
      const rows = files.locator("li");
      await expect(rows).toHaveCount(214);
      await expectBox(rows.first(), { height: 38 });
      const name = await look(rows.first().locator(".name"));
      expect(name.family).toMatch(/mono|Menlo|Consolas|Courier/i);
      await expect(rows.first().locator(".status")).toHaveText(label(labels, "importItemNew"));
      await expect(rows.nth(5).locator(".status")).toHaveText(label(labels, "importItemSkipped"));
      await expectBox(rows.first().locator(".badge"), { width: 30, height: 16 });
      const destination = box.locator(".destination .destination-field");
      await expectWithin(box, destination, { x: 28, y: 496, width: 400, height: 36 });
      await expect(destination).toHaveValue("Harbour project › Site photos");
      await expect(box.locator('button[data-action="change"]')).toHaveText(
        label(labels, "importChange"),
      );
      expect(await look(box.locator('button[data-action="change"]'))).toMatchObject({
        colour: rgbOf(c.primary),
      });
      await expect(box.locator(".note")).toHaveText(
        "Names taken in Site photos get the first free number, for example IMG_4102-1.JPG.",
      );
      await expectWithin(box, box.locator(".space"), { x: 520 });
      await expect(box.locator(".space .field-label")).toHaveText(label(labels, "importSpace"));
      await expect(box.locator(".space .free")).toHaveText(
        "After upload: 409 GB free on this computer",
      );
      await expect(box.locator(".space .staging")).toHaveText("Staging uses at most 10 GB.");
      const integrity = box.locator(".integrity");
      await expectWithin(box, integrity, { x: 28, y: 576, width: 804, height: 64 });
      expect(await look(integrity)).toMatchObject({ background: rgbOf(c.raised), radius: "8px" });
      await expect(integrity.locator(".title")).toHaveText(label(labels, "importIntegrityTitle"));
      await expectWithin(box, box.locator('button[data-action="cancel"]'), { y: 672, height: 36 });
      const upload = box.locator('button[data-action="start"]');
      await expect(upload).toHaveText("Upload 196 files");
      await expectWithin(box, upload, { y: 672, height: 36 });
      expect(await look(upload)).toMatchObject({
        background: rgbOf(c.primary),
        colour: rgbOf(c.onPrimary),
      });
      await expect(box).not.toContainText(/\/Users|[A-Z]:\\|file:\/\//);
      expect(api.imports.calls).toEqual(["items plan-1"]);
      expect(errors).toEqual([]);
    });

    test("Cancel discards the plan and starts nothing; the review comes before any job", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await serveImports(page);
      await openFromNewMenu(page);
      await dialog(page).locator('button[data-action="start"]').click();
      await expect(dialog(page)).toHaveClass(/review/);
      expect(api.imports.calls).not.toContain("start plan-1");
      expect(api.jobs.jobs).toEqual([]);
      await dialog(page).locator('button[data-action="cancel"]').click();
      await expect(dialog(page)).toHaveCount(0);
      await expect.poll(() => api.imports.calls).toContain("discard plan-1");
      expect(api.imports.calls).not.toContain("start plan-1");
      expect(api.jobs.jobs).toEqual([]);
      await expect(page.locator("fs-jobs-panel .jobs-button")).toBeHidden();
      expect(errors).toEqual([]);
    });

    test("Upload starts the job, opens the Jobs panel (mockup 19) and the footer shows the progress", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await serveImports(page, { imports: { items: plannedFiles(["IMG_4101.JPG"]) } });
      await openFromNewMenu(page);
      await dialog(page).locator('button[data-action="start"]').click();
      await dialog(page).locator('button[data-action="start"]').click();
      await expect(dialog(page)).toHaveCount(0);
      expect(api.imports.calls).toContain("start plan-1");
      await expect(panel(page)).toBeVisible();
      await expect(page.locator("fs-app-shell footer .upload-state")).toHaveText(
        "Uploading · 0 of 196",
      );
      api.jobs.itemsByJob["job-1"] = [...MOCKUP_ITEMS];
      api.emit(jobView({ id: "job-1", state: "running", done: 82, total: 196 }));
      await expect(page.locator("fs-app-shell footer .upload-state")).toHaveText(
        label(labels, "jobsFooter").replace("{done}", "82").replace("{total}", "196"),
      );
      expect(errors).toEqual([]);
    });
  });
}

/** Seed the jobs of mockup 19: running, paused, failed and cancelled. */
function mockupJobs(): { jobs: JobView[]; items: Record<string, JobItem[]> } {
  return {
    jobs: [
      jobView({ id: "run", created: "2026-10-31T11:02:00Z", done: 82, total: 196 }),
      jobView({
        id: "pause",
        state: "pending",
        paused: true,
        done: 14,
        total: 40,
        destination: { collection_id: "col-recordings" },
        created: "2026-10-31T10:00:00Z",
      }),
      jobView({
        id: "fail",
        state: "failed",
        done: 120,
        total: 300,
        error_code: "source_changed",
        destination: { collection_id: "col-photos" },
        created: "2026-10-31T09:00:00Z",
      }),
      jobView({
        id: "cancel",
        state: "cancelled",
        done: 3,
        total: 12,
        destination: { collection_id: "col-documents" },
        created: "2026-10-31T08:00:00Z",
      }),
    ],
    items: {
      run: [...MOCKUP_ITEMS],
      fail: [
        jobItem("a.jpg", { state: "failed", error_code: "source_changed" }),
        jobItem("b.jpg", { state: "failed", error_code: "source_changed" }),
        jobItem("c.jpg", { state: "failed", error_code: "source_changed" }),
      ],
    },
  };
}

/** Open the page with the mockup's jobs and the panel open. */
async function openJobs(page: Page, extra?: Partial<ImportApi>): Promise<ImportApi> {
  const seed = mockupJobs();
  const api = await serveImports(page, { jobs: seed.jobs, items: seed.items });
  Object.assign(api, extra);
  await page.goto("/#/navigation/c2");
  await expect(page.locator("fs-jobs-panel .jobs-button")).toBeVisible();
  await page.locator("fs-jobs-panel .jobs-button").click();
  await expect(panel(page)).toBeVisible();
  await expect(panel(page).locator("section.job")).toHaveCount(4);
  return api;
}

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme, Jobs panel and refusals`, () => {
    test.beforeEach(async ({ page }) => setScheme(page, scheme));

    test("the Jobs panel (mockup 19): button, panel, blocks, chips, bars, controls, files and note", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await openJobs(page);
      const button = page.locator("fs-jobs-panel .jobs-button");
      await expectBox(button, { y: 16, width: 28, height: 28 });
      expect(await look(button)).toMatchObject({ background: rgbOf(c.tint), radius: "7px" });
      await expect(button.locator(".badge-count")).toHaveText("2");
      const badge = await look(button.locator(".badge-count"));
      expect(badge).toMatchObject({
        size: "10px",
        weight: "700",
        background: rgbOf(c.primary),
        colour: rgbOf(c.onPrimary),
      });
      const box = panel(page);
      await expectBox(box, { x: 976, y: 56, width: 440, height: 680 });
      expect(await look(box)).toMatchObject({
        radius: "10px",
        background: rgbOf(c.overlay),
        borderColour: rgbOf(c.strong),
      });
      const title = box.locator(".panel-title");
      await expect(title).toHaveText(label(labels, "jobsTitle"));
      expect(await look(title)).toMatchObject({
        size: "20px",
        weight: "700",
        colour: rgbOf(c.heading),
      });
      expect((await look(title)).family).toMatch(/^Georgia/);
      const run = box.locator('section[data-job-id="run"]');
      await expect(run.locator(".job-title")).toHaveText("Upload into Collection › Site photos");
      expect(await look(run.locator(".job-title"))).toMatchObject({ size: "14px", weight: "600" });
      const chip = run.locator(".chip");
      await expect(chip).toHaveText("running");
      await expectBox(chip, { height: 22 });
      expect(await look(chip)).toMatchObject({
        background: rgbOf(c.tint),
        colour: rgbOf(c.active),
        size: "11px",
        weight: "500",
        radius: "999px",
      });
      await expect(run.locator(".detail")).toHaveText("82 of 196 files · started 11:02");
      const bar = run.locator(":scope > .bar");
      await expectBox(bar, { width: 400, height: 4 });
      expect(await look(bar)).toMatchObject({ background: rgbOf(c.outline), radius: "2px" });
      expect(await look(bar.locator(".fill"))).toMatchObject({ background: rgbOf(c.primary) });
      const pause = run.locator('button[data-action="pause"]');
      await expectBox(pause, { width: 86, height: 30 });
      expect(await look(pause)).toMatchObject({ radius: "7px", weight: "600", size: "14px" });
      await expect(run.locator('button[data-action="cancel"]')).toHaveText(
        label(labels, "jobsCancel"),
      );
      const eyebrow = run.locator(".eyebrow");
      await expect(eyebrow).toHaveText("FILES · 4 OF 196 SHOWN");
      expect(await look(eyebrow)).toMatchObject({
        size: "11px",
        transform: "uppercase",
        colour: rgbOf(c.muted),
      });
      const files = run.locator(".job-file");
      await expect(files).toHaveCount(4);
      await expect(files.locator(".state")).toHaveText([
        "Copied",
        "Copying · 62%",
        "Queued",
        "Failed · source_changed",
      ]);
      await expectBox(files.first().locator(".badge"), { width: 30, height: 16 });
      expect(await look(files.nth(0).locator(".state"))).toMatchObject({ colour: rgbOf(c.active) });
      expect(await look(files.nth(1).locator(".state"))).toMatchObject({
        colour: rgbOf(c.variant),
      });
      expect(await look(files.nth(2).locator(".state"))).toMatchObject({ colour: rgbOf(c.muted) });
      expect(await look(files.nth(3).locator(".state"))).toMatchObject({
        colour: rgbOf(c.error),
        weight: "600",
      });
      await expectBox(files.nth(1).locator(".bar.small"), { height: 3 });
      await expect(files.nth(3).locator(".retry")).toHaveText(label(labels, "jobsRetry"));
      await expect(files.nth(0).locator(".where")).toHaveText("→ Site photos");
      const paused = box.locator('section[data-job-id="pause"]');
      await expect(paused.locator(".chip")).toHaveText("paused");
      expect(await look(paused.locator(".chip"))).toMatchObject({
        background: rgbOf(c.raised),
        colour: rgbOf(c.variant),
      });
      await expect(paused.locator(".detail")).toHaveText(
        "14 of 40 files · resumes from its checkpoint",
      );
      await expect(paused.locator('button[data-action="resume"]')).toHaveText(
        label(labels, "jobsResume"),
      );
      const failed = box.locator('section[data-job-id="fail"]');
      expect(await look(failed.locator(".chip"))).toMatchObject({
        borderColour: rgbOf(c.error),
        colour: rgbOf(c.error),
        background: "rgba(0, 0, 0, 0)",
      });
      await expect(failed.locator(".detail")).toHaveText(
        "120 of 300 files · 3 failed: source_changed",
      );
      const cancelled = box.locator('section[data-job-id="cancel"]');
      await expect(cancelled.locator(".detail")).toHaveText("3 of 12 files · committed files stay");
      const note = box.locator(".panel-note");
      await expect(note).toHaveText(label(labels, "jobsNote"));
      await expectBox(note, { width: 400 });
      expect(await look(note)).toMatchObject({ background: rgbOf(c.raised), radius: "8px" });
      await expect(box).not.toContainText(/\bImport\b|\/Users|[A-Z]:\\/);
      expect(errors).toEqual([]);
    });

    test("Retry puts one failed file back, Retry failed puts the rest back, Pause and Resume go through the job routes", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await openJobs(page);
      const run = panel(page).locator('section[data-job-id="run"]');
      await run.locator(".job-file .retry").click();
      await expect.poll(() => api.jobs.calls).toContain("retry run item-IMG_4099.JPG");
      await expect(run.locator('.job-file[data-state="failed"]')).toHaveCount(0);
      await run.locator('button[data-action="pause"]').click();
      await expect.poll(() => api.jobs.calls).toContain("pause run");
      await expect(panel(page).locator('section[data-job-id="run"] .chip')).toHaveText("paused");
      await panel(page).locator('section[data-job-id="run"] button[data-action="resume"]').click();
      await expect.poll(() => api.jobs.calls).toContain("resume run");
      expect(errors).toEqual([]);
    });

    test("a pending job whose files ended with a failure is waiting for the owner: no bar, Retry failed and Cancel (DE-1.8)", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await serveImports(page, {
        jobs: [jobView({ id: "wait", state: "pending", done: 3, total: 3 })],
        items: {
          wait: [
            jobItem("ok.jpg", { state: "copied", bytes_done: 1000 }),
            jobItem("x.jpg", { state: "failed", error_code: "source_missing", item_id: "ix" }),
            jobItem("y.jpg", { state: "failed", error_code: "source_missing", item_id: "iy" }),
          ],
        },
      });
      await page.goto("/#/navigation/c2");
      await page.locator("fs-jobs-panel .jobs-button").click();
      const wait = panel(page).locator('section[data-job-id="wait"]');
      await expect(wait.locator(".chip")).toHaveText(label(labels, "jobsStateAttention"));
      expect(await look(wait.locator(".chip"))).toMatchObject({
        colour: rgbOf(c.error),
        borderColour: rgbOf(c.error),
      });
      await expect(wait.locator("[role=progressbar]")).toHaveCount(0);
      await expect(wait.locator(".actions button")).toHaveText([
        label(labels, "jobsRetryFailed"),
        label(labels, "jobsCancel"),
      ]);
      await wait.locator('button[data-action="retry-failed"]').click();
      await expect.poll(() => api.jobs.calls).toContain("retry-failed wait");
      expect(errors).toEqual([]);
    });

    test("refusals (mockup 48): collection_not_found for a 404 of the plan, path_suffix_exhausted for a file, each with Dismiss", async ({
      page,
    }) => {
      const errors = trackErrors(page, /status of 404/);
      const api = await serveImports(page, {
        jobs: [jobView({ id: "wait", state: "pending", done: 1, total: 1 })],
        items: {
          wait: [jobItem("photo.jpg", { state: "failed", error_code: "path_suffix_exhausted" })],
        },
      });
      api.refuseWith = { status: 404, code: "not_found" };
      await openFromNewMenu(page);
      await dialog(page).locator('button[data-action="start"]').click();
      const card = dialog(page).locator(".refusal");
      await expect(card.locator(".reason")).toHaveText(label(labels, "fileCollectionNotFound"));
      const frame = card.locator(".note-box");
      expect(await look(frame)).toMatchObject({
        borderColour: rgbOf(c.error),
        borderWidth: "1px",
        radius: "8px",
      });
      expect(await look(card.locator(".title"))).toMatchObject({ size: "14px", weight: "600" });
      expect(await look(card.locator("svg"))).toMatchObject({ colour: rgbOf(c.error) });
      await expect(card.locator("button")).toHaveText(label(labels, "fileDismiss"));
      await card.locator("button").click();
      await expect(dialog(page).locator(".refusal")).toHaveCount(0);
      await dialog(page).locator('button[data-action="cancel"]').click();
      await page.locator("fs-jobs-panel .jobs-button").click();
      const jobCard = panel(page).locator('section[data-job-id="wait"] .refusal');
      await expect(jobCard.locator(".title")).toHaveText("Not uploaded: photo.jpg");
      await expect(jobCard.locator(".reason")).toHaveText(
        "Every name from photo-1.jpg to photo-9999.jpg is taken in this folder. Nothing was changed. Rename or remove one, then try again.",
      );
      await jobCard.locator("button").click();
      await expect(panel(page).locator(".refusal")).toHaveCount(0);
      expect(errors).toEqual([]);
    });

    test("a host that cannot name the collection yet refuses honestly and sends no plan request", async ({
      page,
    }) => {
      const errors = trackErrors(page, /status of 404/);
      const api = await serveImports(page);
      await page.route(/\/api\/v1\/navigation\/destination\?/, (route) =>
        route.fulfill({
          status: 404,
          json: { code: "not_found", message: "no", retryable: false },
        }),
      );
      await openFromNewMenu(page);
      await dialog(page).locator('button[data-action="start"]').click();
      await expect(dialog(page).locator(".refusal .reason")).toHaveText(
        label(labels, "importDestinationUnresolved"),
      );
      expect(api.planBodies).toEqual([]);
      expect(errors).toEqual([]);
    });
  });
}

test("the plan of a smaller upload shows its own counts and disables Upload when nothing is new", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await serveImports(page, {
    imports: {
      plan: planWith({ files_found: 3, files_new: 0, files_present: 3, bytes_to_copy: 0 }),
      items: plannedFiles(["a.jpg", "b.jpg", "c.jpg"]).map((item) => ({ ...item, present: true })),
    },
  });
  await openFromNewMenu(page);
  await dialog(page).locator('button[data-action="start"]').click();
  await expect(dialog(page).locator("h2")).toHaveText("Upload 3 files");
  await expect(dialog(page).locator('button[data-action="start"]')).toBeDisabled();
  await expect(dialog(page).locator(".foot .reason")).toHaveText(label(labels, "importNothingNew"));
  expect(PLAN.files_found).toBe(214);
  expect(errors).toEqual([]);
});
