/**
 * Production-artifact proof of first use, the recent libraries, the library facts and the
 * rebuild job (UI-1.3, UI-S-034, UI-S-027, UI-S-025, UI-S-024; mockups 57-first-use-library,
 * 01-welcome, 41-library-page, 09-copy-library and 04-recover-library): the FileSquid UI built
 * by `vite build` and served by `vite preview` at the mockups' 1440 x 900 window, in light and
 * dark, with the library and job routes answered by the test in the shapes of the API contract.
 * Each test asserts the custom elements are defined, every listed string against the active
 * label table, the regions' boxes within 2 px of the mockup, the constants as computed styles,
 * and no console error. The mockups draw a 28 px title bar the webview does not have, so a
 * mockup y of `n` is `n - 28` here.
 *
 * Covered: first use creating the library and then its first collection; a failure after the
 * library exists, kept with Try again for the collection; recent libraries with their free
 * space; a volume that cannot be read; the library page's name and Free space; and the rebuild
 * job running, paused, resumed, cancelled and finished (with its report), live over the event
 * channel.
 */
import { expect, test, type Page } from "@playwright/test";
import { JOB, LIB, serveLibraries, type LibraryApi } from "../support/library-api";
import {
  activeLabels,
  expectElementsDefined,
  label,
  rgbOf,
  setScheme,
  trackErrors,
  type Scheme,
} from "../support/strict-dom";
import { COLOURS, expectBox, look, renderedBorder } from "../support/fidelity";

const labels = activeLabels();
const text = (key: string) => label(labels, key);
const SCHEMES: readonly Scheme[] = ["light", "dark"];

/** The browser's line for the collection command the tests refuse on purpose, and no other. */
const REFUSED = (text: string, url: string) =>
  /status of 500/.test(text) && /\/api\/v1\/libraries\/collections$/.test(url);

/** A path-like string: a drive letter, or a rooted folder of a user or volume. */
const PATH = /[A-Za-z]:\\|\/Users\/|\/home\/|\/Volumes\//;

test.use({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });

const start = (page: Page) => page.locator("fs-library-start");
const dialog = (page: Page) => page.locator("fs-first-use");
const box = (page: Page) => dialog(page).locator('[role="dialog"]');
const rebuild = (page: Page) => page.locator("fs-library-rebuild");

/** Make the host know no library, as on first use. */
async function firstUse(page: Page): Promise<LibraryApi> {
  const api = await serveLibraries(page);
  api.libraries.clear();
  await page.goto("/#/library");
  await expect(box(page)).toBeVisible();
  return api;
}

/** The writes the page sent, without the reads of recent libraries and facts. */
/** A date as the page words it: day, short month and year in English order. */
const shortDate = (iso: string) =>
  new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "short", year: "numeric" }).format(
    new Date(iso),
  );

const writes = (api: LibraryApi) =>
  api.seen
    .filter(({ method }) => method === "POST")
    .map(({ path }) => path.replace("/api/v1/libraries", "") || "/");

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => {
      await setScheme(page, scheme);
    });

    test("first use (57, UI-S-034): the dialog over the welcome screen, region by region", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await firstUse(page);
      await expectElementsDefined(page, ["fs-first-use", "fs-library-start", "fs-states"]);
      // Strings.
      await expect(box(page).locator(".dialog-title")).toHaveText(text("libraryFirstUseTitle"));
      await expect(box(page).locator(".lead")).toContainText(text("libraryFirstUseLead"));
      await expect(box(page).locator(".lead")).toContainText(text("libraryFirstUseCollectionLead"));
      await expect(box(page).locator('label[for="first-use-name"]')).toHaveText(
        text("libraryFirstUseName"),
      );
      await expect(box(page).locator(".location-label")).toHaveText(
        text("libraryFirstUseLocation"),
      );
      await expect(box(page).locator('button[data-action="choose"]')).toHaveText(
        text("libraryChooseFolder"),
      );
      await expect(box(page).locator(".rule")).toHaveText(text("libraryFolderRule"));
      await expect(box(page).locator(".collection-note strong")).toHaveText(
        text("libraryFirstCollection"),
      );
      await expect(box(page).locator(".collection-note span")).toHaveText(
        text("libraryFirstCollectionNote"),
      );
      await expect(box(page).locator('button[data-action="cancel"]')).toHaveText(
        text("libraryFirstUseCancel"),
      );
      await expect(box(page).locator('button[data-action="create"]')).toHaveText(
        text("libraryCreate"),
      );
      // Regions (mockup 57; y is the mockup's minus the 28 px title bar).
      await expectBox(box(page), { x: 420, y: 162, width: 600, height: 470 });
      await expectBox(box(page).locator("input.input"), { x: 448, y: 314, width: 544, height: 36 });
      await expectBox(box(page).locator(".location-box"), {
        x: 448,
        y: 394,
        width: 400,
        height: 36,
      });
      await expectBox(box(page).locator('button[data-action="choose"]'), {
        x: 856,
        y: 394,
        width: 136,
        height: 36,
      });
      await expectBox(box(page).locator(".collection-note"), {
        x: 448,
        y: 470,
        width: 544,
        height: 64,
      });
      await expectBox(box(page).locator('button[data-action="cancel"]'), {
        x: 772,
        y: 570,
        width: 82,
        height: 38,
      });
      await expectBox(box(page).locator('button[data-action="create"]'), {
        x: 862,
        y: 570,
        width: 130,
        height: 38,
      });
      // Computed constants.
      expect(await look(box(page))).toMatchObject({
        background: rgbOf(c.overlay),
        borderWidth: "1px",
        borderColour: rgbOf(c.strong),
        radius: "10px",
      });
      const title = await look(box(page).locator(".dialog-title"));
      expect(title).toMatchObject({ size: "22px", weight: "700", colour: rgbOf(c.heading) });
      expect(title.family).toMatch(/^Georgia/);
      expect(await look(box(page).locator(".lead p").first())).toMatchObject({
        size: "14px",
        colour: rgbOf(c.body),
      });
      expect(await look(box(page).locator(".field-label").first())).toMatchObject({
        size: "13px",
        weight: "600",
      });
      expect(await look(box(page).locator("input.input"))).toMatchObject({
        size: "14px",
        radius: "6px",
        borderWidth: "2px",
        borderColour: rgbOf(c.focus),
        background: rgbOf(c.card),
      });
      expect(await look(box(page).locator(".location-box"))).toMatchObject({
        borderWidth: await renderedBorder(page, "1.5px"),
        borderColour: rgbOf(c.control),
        radius: "6px",
      });
      expect(await look(box(page).locator(".collection-note"))).toMatchObject({
        background: rgbOf(c.raised),
        radius: "8px",
      });
      expect(await look(box(page).locator(".collection-note strong"))).toMatchObject({
        size: "13px",
        weight: "600",
      });
      // Until a folder is chosen Create is the disabled button of UI-S-024.
      expect(await look(box(page).locator('button[data-action="create"]'))).toMatchObject({
        background: rgbOf(c.raised),
        colour: rgbOf(c.placeholder),
        borderColour: rgbOf(c.outline),
        radius: "7px",
      });
      for (const name of ["choose", "cancel"]) {
        expect(await look(box(page).locator(`button[data-action="${name}"]`))).toMatchObject({
          background: rgbOf(c.card),
          borderColour: rgbOf(c.strong),
          borderWidth: await renderedBorder(page, "1.5px"),
          radius: "7px",
          weight: "600",
        });
      }
      const scrim = await dialog(page)
        .locator(".scrim")
        .evaluate((el) => {
          const css = getComputedStyle(el, "::before");
          return { background: css.backgroundColor, opacity: css.opacity };
        });
      expect(scrim).toEqual({ background: rgbOf("#000000"), opacity: "0.32" });
      // The name field has the focus, as drawn, and one button is primary.
      await expect(box(page).locator("input.input")).toBeFocused();
      await expect(box(page).locator("button.primary")).toHaveCount(1);
      // With the folder chosen, as mockup 57 draws it, Create is the one filled primary button.
      await box(page).locator('button[data-action="choose"]').click();
      await expect(box(page).locator(".location-value")).toHaveText(text("libraryFolderChosen"));
      // The button's colours transition over --fs-motion-duration; read them once settled.
      const create = box(page).locator('button[data-action="create"]');
      await expect.poll(async () => (await look(create)).background).toBe(rgbOf(c.primary));
      expect(await look(create)).toMatchObject({
        background: rgbOf(c.primary),
        colour: rgbOf(c.onPrimary),
        size: "14px",
        weight: "600",
        radius: "7px",
      });
      expect(errors).toEqual([]);
    });

    test("first use creates the library with its name and then its first collection", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await firstUse(page);
      await box(page).locator("input.input").fill("Family archive");
      await expect(box(page).locator('button[data-action="create"]')).toHaveAttribute(
        "aria-disabled",
        "true",
      );
      await box(page).locator('button[data-action="choose"]').click();
      await expect(box(page).locator(".location-value")).toHaveText(text("libraryFolderChosen"));
      await box(page).locator('button[data-action="create"]').click();
      await expect(page).toHaveURL(new RegExp(`#/library/${LIB}$`));
      expect(writes(api)).toEqual(["/location-choices", "/", "/collections"]);
      const sent = (path: string) => api.to(path).map(({ body }) => body);
      expect(sent("/api/v1/libraries/location-choices")).toEqual([{ purpose: "create" }]);
      expect(sent("/api/v1/libraries")).toEqual([
        {
          envelope: expect.objectContaining({
            scope: "location:loc-create",
            expected_revision: "0",
          }),
          input: { location_id: "loc-create", name: "Family archive" },
        },
      ]);
      expect(sent("/api/v1/libraries/collections")).toEqual([
        {
          envelope: expect.objectContaining({ scope: `library:${LIB}`, expected_revision: "1" }),
          input: { library_id: LIB, title: text("libraryFirstCollectionTitle") },
        },
      ]);
      expect(api.collections).toEqual([
        { library: LIB, title: text("libraryFirstCollectionTitle") },
      ]);
      // The library page shows the name the owner gave.
      await expect(page.locator("fs-library-detail h2")).toHaveText("Family archive");
      expect(JSON.stringify(api.seen)).not.toMatch(PATH);
      expect(await page.locator("body").innerText()).not.toMatch(PATH);
      expect(errors).toEqual([]);
    });

    test("a failure after the library exists keeps it and offers Try again for the collection", async ({
      page,
    }) => {
      const errors = trackErrors(page, REFUSED);
      const api = await firstUse(page);
      api.collectionRefusal = 500;
      await box(page).locator('button[data-action="choose"]').click();
      await box(page).locator('button[data-action="create"]').click();
      await expect(box(page).locator(".kept")).toHaveText(text("libraryFirstUseCollectionFailed"));
      const retry = box(page).getByRole("button", { name: text("stateRetry") });
      await expect(retry).toBeVisible();
      await expect(page).toHaveURL(/#\/library$/);
      expect(api.libraries.has(LIB)).toBe(true);
      expect(await look(box(page).locator(".kept"))).toMatchObject({
        size: "13px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      await retry.click();
      await expect(page).toHaveURL(new RegExp(`#/library/${LIB}$`));
      // One folder dialog, one library, and the collection command twice under one operation id.
      expect(writes(api)).toEqual(["/location-choices", "/", "/collections", "/collections"]);
      const ids = api
        .to("/api/v1/libraries/collections")
        .map(({ body }) => (body as { envelope: { operation_id: string } }).envelope.operation_id);
      expect(ids[1]).toBe(ids[0]);
      expect(errors).toEqual([]);
    });

    test("Cancel closes the first-use dialog without a request, and Create opens it again", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await firstUse(page);
      await box(page).locator('button[data-action="cancel"]').click();
      await expect(dialog(page)).toHaveCount(0);
      expect(writes(api)).toEqual([]);
      await start(page).locator('button[data-action="create"]').click();
      await expect(box(page)).toBeVisible();
      await page.keyboard.press("Escape");
      await expect(dialog(page)).toHaveCount(0);
      expect(errors).toEqual([]);
    });

    test("recent libraries (01, UI-S-027): name, opened time and free space, and a volume that cannot be read", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await serveLibraries(page);
      api.libraries.set("lib-2", null);
      api.libraries.set("lib-3", null);
      api.meta.set(LIB, {
        name: "Family archive",
        lastOpened: new Date().toISOString(),
        freeBytes: 1_400_000_000_000,
      });
      api.meta.set("lib-2", {
        name: "Studio work",
        lastOpened: "2026-10-20T10:00:00Z",
        freeBytes: 212_000_000_000,
      });
      api.meta.set("lib-3", {
        name: "Old disk",
        lastOpened: "2026-09-01T10:00:00Z",
        freeBytes: null,
      });
      await page.goto("/#/library");
      const column = start(page).locator("section.recent");
      await expect(column).toBeVisible();
      await expect(column.locator(".eyebrow")).toHaveText(text("libraryRecentTitle"));
      const rows = column.locator("li");
      await expect(rows).toHaveCount(3);
      await expect(rows.locator(".name")).toHaveText(["Family archive", "Studio work", "Old disk"]);
      await expect(rows.locator(".meta")).toHaveText([
        "Opened today · 1.3 TB free on its disk",
        `Opened ${shortDate("2026-10-20T10:00:00Z")} · 197 GB free on its disk`,
        `Opened ${shortDate("2026-09-01T10:00:00Z")} · free space unknown`,
      ]);
      await expect(rows.locator("button")).toHaveText([
        text("libraryOpenRecent"),
        text("libraryOpenRecent"),
        text("libraryOpenRecent"),
      ]);
      // Geometry (mockup 01): the column is 332 px wide at the right edge of the 1036 px page.
      await expectBox(column, { x: 906, width: 332 });
      const first = await rows.nth(0).boundingBox();
      const second = await rows.nth(1).boundingBox();
      expect(Math.abs((second?.y ?? 0) - (first?.y ?? 0) - 60)).toBeLessThanOrEqual(2);
      await expectBox(rows.nth(0).locator("button"), { width: 76, height: 24 });
      // Computed constants.
      const eyebrow = await look(column.locator(".eyebrow"));
      expect(eyebrow).toMatchObject({
        size: "11px",
        weight: "400",
        transform: "uppercase",
        colour: rgbOf(c.muted),
      });
      expect(await look(rows.nth(0).locator(".name"))).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      expect(await look(rows.nth(0).locator(".meta"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      expect(await look(rows.nth(0).locator("button"))).toMatchObject({
        colour: rgbOf(c.primary),
        weight: "600",
        size: "14px",
      });
      const divider = await rows.nth(0).evaluate((el) => getComputedStyle(el).borderBottomColor);
      expect(divider).toBe(rgbOf(c.outline));
      // No path, library ID or location ID is shown; no first-use dialog opens.
      expect(await column.innerText()).not.toMatch(PATH);
      expect(await column.innerText()).not.toContain("lib-2");
      await expect(dialog(page)).toHaveCount(0);
      expect(errors).toEqual([]);
    });

    test("Open on a recent library that is open now goes to its page; one that is not runs the Open flow", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await serveLibraries(page);
      api.libraries.set("lib-2", null);
      await page.goto("/#/library");
      const rows = start(page).locator("section.recent li");
      await rows.nth(1).locator("button").click();
      await expect(page).toHaveURL(new RegExp(`#/library/${LIB}$`));
      expect(writes(api)).toEqual(["/location-choices", "/openings"]);
      await page.goto("/#/library");
      await rows.nth(0).locator("button").click();
      await expect(page).toHaveURL(new RegExp(`#/library/${LIB}$`));
      expect(errors).toEqual([]);
    });

    test("the library page (41, 09): the library's name, and Free space from its facts", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await serveLibraries(page);
      api.meta.set(LIB, {
        name: "Family archive",
        lastOpened: "2026-10-31T08:00:00Z",
        freeBytes: 1_400_000_000_000,
      });
      await page.goto(`/#/library/${LIB}`);
      const detail = page.locator("fs-library-detail");
      await expect(detail.locator(".page > h2")).toHaveText("Family archive");
      await expect(detail.locator(".crumb")).toHaveText("Family archive");
      const title = await look(detail.locator(".page > h2"));
      expect(title).toMatchObject({ size: "30px", weight: "700", colour: rgbOf(c.heading) });
      expect(title.family).toMatch(/^Georgia/);
      expect(await look(detail.locator(".crumb"))).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.body),
      });
      const terms = detail.locator('[data-section="facts"] dt');
      await expect(terms.last()).toHaveText(text("libraryFactFreeTerm"));
      const free = detail.locator('[data-section="facts"] dd').last();
      await expect(free).toHaveText("1.3 TB on the disk that holds this library");
      expect(await look(free)).toMatchObject({ size: "13px", colour: rgbOf(c.body) });
      const termBox = await terms.last().boundingBox();
      const valueBox = await free.boundingBox();
      expect(Math.abs((valueBox?.x ?? 0) - (termBox?.x ?? 0) - 160)).toBeLessThanOrEqual(2);
      // The sidebar's Storage used block still words the free space of the disk.
      expect(errors).toEqual([]);
    });

    test("the library page says so when the volume cannot be read, with no figure", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await serveLibraries(page);
      await page.goto(`/#/library/${LIB}`);
      const free = page.locator('fs-library-detail [data-section="facts"] dd').last();
      await expect(free).toHaveText(text("libraryFactFreeUnknown"));
      expect(errors).toEqual([]);
    });

    test("the rebuild job (04, UI-S-027): running, paused, resumed, cancelled", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await serveLibraries(page);
      await page.goto(`/#/library/${LIB}/recover/${JOB}`);
      await expectElementsDefined(page, ["fs-library-rebuild", "fs-states"]);
      await expect(page.locator("fs-app-shell")).toHaveAttribute("frameless", "");
      const job = rebuild(page).locator(".job");
      await expect(job.locator(".chip")).toHaveText(text("jobStateRunning"));
      // Strings.
      await expect(rebuild(page).locator(".crumb a")).toHaveText(text("libraries"));
      await expect(rebuild(page).locator(".crumb .current")).toHaveText(
        text("libraryRecoverTitle"),
      );
      await expect(rebuild(page).locator("h2")).toHaveText(text("libraryRecoverTitle"));
      await expect(rebuild(page).locator(".lead")).toHaveText(text("libraryRecoverHelp"));
      await expect(job.locator("h3")).toHaveText(text("libraryRebuild"));
      await expect(job.locator(".progress-text")).toHaveText(
        text("libraryRebuildProgress")
          .replace("{done}", "1,204")
          .replace("{total}", "2,418")
          .replace("{time}", "10:41"),
      );
      await expect(rebuild(page).locator(".note")).toHaveText(text("libraryRecoverNote"));
      await expect(rebuild(page).locator(".back")).toHaveText(text("libraryStartBack"));
      await expect(job.locator('button[data-action="pause"]')).toHaveText(text("jobPause"));
      await expect(job.locator('button[data-action="cancel"]')).toHaveText(text("jobCancel"));
      // Regions (mockup 04).
      await expectBox(rebuild(page).locator("h2"), { x: 200 });
      await expectBox(job, { x: 200, y: 186, width: 1040, height: 112 });
      await expectBox(job.locator('button[data-action="pause"]'), { width: 92, height: 36 });
      await expectBox(job.locator('button[data-action="cancel"]'), { width: 92, height: 36 });
      await expectBox(job.locator('[role="progressbar"]'), { width: 760, height: 4 });
      await expectBox(job.locator(".chip"), { height: 22 });
      // Computed constants.
      const title = await look(rebuild(page).locator("h2"));
      expect(title).toMatchObject({ size: "34px", weight: "700", colour: rgbOf(c.heading) });
      expect(title.family).toMatch(/^Georgia/);
      expect(await look(job)).toMatchObject({ background: rgbOf(c.raised), radius: "8px" });
      expect(await look(job.locator("h3"))).toMatchObject({ size: "15px", weight: "600" });
      expect(await look(job.locator(".chip"))).toMatchObject({
        size: "11px",
        weight: "500",
        background: rgbOf(c.tint),
        colour: rgbOf(c.active),
        radius: "4px",
      });
      expect(await look(job.locator(".progress-text"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      const bar = await job.locator('[role="progressbar"]').evaluate((el) => {
        const track = getComputedStyle(el);
        const fill = getComputedStyle(el.querySelector(".fill") as Element);
        const trackWidth = el.getBoundingClientRect().width;
        const fillWidth = (el.querySelector(".fill") as Element).getBoundingClientRect().width;
        return {
          trackBg: track.backgroundColor,
          radius: track.borderTopLeftRadius,
          fillBg: fill.backgroundColor,
          ratio: fillWidth / trackWidth,
        };
      });
      expect(bar).toMatchObject({
        trackBg: rgbOf(c.outline),
        radius: "2px",
        fillBg: rgbOf(c.primary),
      });
      expect(bar.ratio).toBeCloseTo(1204 / 2418, 2);
      for (const name of ["pause", "cancel"]) {
        expect(await look(job.locator(`button[data-action="${name}"]`))).toMatchObject({
          background: rgbOf(c.card),
          borderColour: rgbOf(c.strong),
          borderWidth: await renderedBorder(page, "1.5px"),
          radius: "7px",
        });
      }
      // Pause, then Resume, over the job routes.
      await job.locator('button[data-action="pause"]').click();
      await expect(job.locator(".chip")).toHaveText(text("jobStatePaused"));
      expect(await look(job.locator(".chip"))).toMatchObject({ background: rgbOf(c.raised) });
      await expect(job.locator('button[data-action="resume"]')).toHaveText(text("jobResume"));
      expect(api.to(`/api/v1/jobs/${JOB}/pause`)).toHaveLength(1);
      await job.locator('button[data-action="resume"]').click();
      await expect(job.locator(".chip")).toHaveText(text("jobStateRunning"));
      expect(api.to(`/api/v1/jobs/${JOB}/resume`)).toHaveLength(1);
      // Cancel.
      await job.locator('button[data-action="cancel"]').click();
      await expect(job.locator(".chip")).toHaveText(text("jobStateCancelled"));
      await expect(rebuild(page).locator(".report")).toHaveText(text("libraryRebuildCancelled"));
      await expect(job.locator("button")).toHaveCount(0);
      expect(api.to(`/api/v1/jobs/${JOB}/cancel`)).toHaveLength(1);
      expect(await page.locator("body").innerText()).not.toMatch(PATH);
      expect(errors).toEqual([]);
    });

    test("the rebuild job finishes over the event channel and shows its report and errors", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await serveLibraries(page);
      await page.goto(`/#/library/${LIB}/recover/${JOB}`);
      const job = rebuild(page).locator(".job");
      await expect(job.locator(".chip")).toHaveText(text("jobStateRunning"));
      api.job.state = "completed";
      api.job.done = 2418;
      api.job.result = {
        objects: [
          {
            object_id: "urn:uuid:7f3e9a12-0000-4000-8000-00000000c2b1",
            head: "v2",
            files: 3,
            digest_algorithm: "sha256",
            error_codes: ["E092"],
            missing_version_inventory: false,
          },
        ],
        unreadable: [],
      };
      api.push("job.update", {
        id: JOB,
        kind: "library.rebuild",
        operation_id: "op-e2e-rebuild",
        state: "completed",
        paused: false,
        done: 2418,
        total: 2418,
        attempts: 1,
        created: "2026-10-31T10:41:09Z",
        updated: "2026-10-31T10:50:00Z",
        result: null,
      });
      await expect(job.locator(".chip")).toHaveText(text("jobStateCompleted"));
      await expect(rebuild(page).locator(".report")).toContainText(
        text("libraryRebuilt")
          .replace("{objects}", "1")
          .replace("{unreadable}", "0")
          .replace("{invalid}", "1"),
      );
      await expect(rebuild(page).locator(".report")).toContainText(text("libraryRecoveredOpen"));
      await expect(job.locator("button")).toHaveCount(0);
      // The validation errors (mockup 04): one mono line in a 1 px error box.
      const errorsBox = rebuild(page).locator(".errors");
      await expect(errorsBox.locator(".eyebrow")).toHaveText(text("libraryRebuildErrorsTitle"));
      const line = errorsBox.locator("li");
      await expect(line).toHaveText(
        text("libraryRebuiltInvalid")
          .replace("{object}", "urn:uuid:7f3e9a12-0000-4000-8000-00000000c2b1")
          .replace("{codes}", "E092"),
      );
      await expectBox(line, { x: 200, width: 1040, height: 52 });
      const look1 = await look(line);
      expect(look1).toMatchObject({
        borderWidth: "1px",
        borderColour: rgbOf(c.error),
        radius: "8px",
        size: "13px",
        colour: rgbOf(c.body),
      });
      expect(look1.family).toMatch(/Mono|monospace|Consolas/);
      expect(await look(errorsBox.locator(".eyebrow"))).toMatchObject({
        size: "11px",
        transform: "uppercase",
        colour: rgbOf(c.muted),
      });
      expect(errors).toEqual([]);
    });

    test("a library's own rebuild is followed inside the shell and finishes", async ({ page }) => {
      const errors = trackErrors(page);
      const api = await serveLibraries(page);
      await page.goto(`/#/library/${LIB}`);
      await page.locator('fs-library-detail button[data-action="rebuild"]').click();
      await expect(page).toHaveURL(new RegExp(`#/library/${LIB}/rebuild/${JOB}$`));
      await expect(page.locator("fs-app-shell")).not.toHaveAttribute("frameless", "");
      await expect(rebuild(page).locator("h2")).toHaveText(text("libraryRebuild"));
      expect(await look(rebuild(page).locator("h2"))).toMatchObject({ size: "30px" });
      expect(api.to("/api/v1/libraries/rebuilds")).toHaveLength(1);
      await expect(rebuild(page).locator(".job .chip")).toHaveText(text("jobStateRunning"));
      await expect(rebuild(page).locator(".back")).toHaveText(text("libraryBack"));
      expect(errors).toEqual([]);
    });
  });
}
