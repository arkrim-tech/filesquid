/**
 * Mockup fidelity of the active v3 suite (UI-S-024 to UI-S-033, Task UI-F2.18): the production
 * build (`vite build` plus `vite preview`) at the mockups' 1440 × 900 window, in light and dark.
 * Each test asserts the custom elements are defined, every listed string comes from the active
 * label table, the regions' boxes match the mockup within 2 px, and the constants are the
 * browser's computed styles (font family, size and weight, colour, background, border width and
 * colour, radius), with no console error. The mockups draw a 28 px macOS title bar above the
 * window content; the webview has none, so a mockup y of `n` is `n - 28` here.
 *
 * Mockups: 01-welcome, 03-open-library (unavailable answer), 31-monitor and its dark twin,
 * 40-host-indicator, 41-library-page and its dark twin, 42-repair-consent, 43-repair-results,
 * 54-monitor-ocfl-validation and its dark twin, 55-navigation-tree-built and 56-view-states,
 * all in `resources/ui-mockups/v3/`; the shell frame of every mockup (UI-S-025).
 */
import { expect, test, type Page } from "@playwright/test";
import {
  activeLabels,
  expectElementsDefined,
  label,
  rgbOf,
  setScheme,
  tokenColour,
  trackErrors,
  type Scheme,
} from "../support/strict-dom";
import { LIB, OBJECTS, serveLibraries } from "../support/library-api";
import { serveDrive } from "../support/drive-api";
import { COLOURS, expectBox, look, renderedBorder } from "../support/fidelity";

const labels = activeLabels();
const SCHEMES: readonly Scheme[] = ["light", "dark"];

/** Serve the shell's routes, the navigation tree, the library routes and the Monitor. */
async function serveAll(page: Page): Promise<void> {
  await serveLibraries(page);
  await serveMonitor(page);
  // The file view of a collection reads its rows through the view operation.
  await serveDrive(page, [], undefined, ["l1", "c1", "c2", "v1"]);
}

/** The Monitor's routes: degraded health, one running and one stalled process, one E023. */
async function serveMonitor(page: Page): Promise<void> {
  const checks = {
    stalled: 1,
    store_ok: true,
    data_dir_writable: true,
    free_bytes: 412 * 1024 ** 3,
    queue_fill: 0,
    errors_last_5m: 0,
    ocfl_error_codes: ["E023"],
    ocfl_error_objects: 1,
  };
  const reasons = [
    { code: "process_stalled", severity: "degraded", detail: "1 process(es) stalled" },
  ];
  await page.route("**/api/v1/admin/health", (route) =>
    route.fulfill({ json: { status: "degraded", reasons, checks } }),
  );
  const process = (id: string, state: string) => ({
    id,
    kind: "preview.worker",
    state,
    done: 3,
    total: 10,
    rate_per_second: 1,
    eta_seconds: 7,
    started: "2026-10-22T11:00:00Z",
    last_heartbeat: "2026-10-22T11:00:05Z",
    heartbeat_timeout_ms: 30000,
    error_code: null,
  });
  await page.route("**/api/v1/admin/processes", (route) =>
    route.fulfill({ json: [process("p1", "running"), process("p2", "stalled")] }),
  );
  await page.route("**/api/v1/admin/metrics", (route) => route.fulfill({ json: {} }));
  await page.route(/\/api\/v1\/admin\/journal(\?|$)/, (route) =>
    route.fulfill({ json: { entries: [], next_cursor: null } }),
  );
  const finding = (code: string, severity: string) => ({
    library_id: LIB,
    object_id: OBJECTS[0],
    version: "v7",
    code,
    severity,
    location: null,
  });
  await page.route("**/api/v1/admin/ocfl/findings", (route) =>
    route.fulfill({
      json: {
        findings: [finding("E023", "error"), finding("W010", "warning")],
        truncated: false,
        libraries: [{ library_id: LIB, running: true, last_run: null, next_run: null }],
        settings: { interval_ms: 86_400_000, object_cap: 10_000 },
      },
    }),
  );
}

/** Open the library page through the Open flow, so its facts are known. */
async function openLibraryPage(page: Page): Promise<void> {
  await page.goto("/#/library");
  await page.locator('fs-library-start button[data-action="open"]').click();
  await expect(page).toHaveURL(new RegExp(`#/library/${LIB}$`));
  await expect(page.locator("fs-library-detail [data-section='repair-note']")).toBeVisible();
}

// A 2x (Retina) display, as the macOS desktop target has.
test.use({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => {
      await setScheme(page, scheme);
      await serveAll(page);
    });

    test("the shell frame (UI-S-025): sidebar, header, footer, controls and strings", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await page.goto("/#/monitor");
      await expectElementsDefined(page, ["fs-app-shell", "fs-nav-tree", "fs-host-indicator"]);
      const shell = page.locator("fs-app-shell");
      await expect(shell.locator("fs-host-indicator")).toHaveAttribute("data-state", "ready");
      await expectBox(shell.locator("nav").first(), { x: 0, y: 0, width: 240, height: 900 });
      await expectBox(shell.locator("header"), { x: 240, y: 0, width: 1200, height: 60 });
      await expectBox(shell.locator("footer"), { x: 240, y: 860, width: 1200, height: 40 });
      await expectBox(shell.locator("nav .new"), { x: 16, width: 208, height: 38 });
      await expectBox(shell.locator("nav input.filter"), { x: 16, width: 208, height: 34 });
      await expectBox(shell.locator("fs-search-bar .frame"), { x: 264, width: 520, height: 36 });
      await expectBox(shell.locator("fs-host-indicator button.badge"), { height: 28 });
      await expectBox(shell.locator("header .monitor"), { height: 36 });
      await expectBox(shell.locator("nav .brand img"), { height: 24 });
      expect(await look(shell.locator("nav").first())).toMatchObject({
        background: rgbOf(c.card),
        borderWidth: "0px",
      });
      const navEdge = await shell
        .locator("nav")
        .first()
        .evaluate((el) => {
          const css = getComputedStyle(el);
          return { width: css.borderInlineEndWidth, colour: css.borderInlineEndColor };
        });
      expect(navEdge).toEqual({ width: "1px", colour: rgbOf(c.outline) });
      expect(await look(shell.locator("h1"))).toMatchObject({
        size: "17px",
        weight: "700",
        colour: rgbOf(c.heading),
      });
      expect((await look(shell.locator("h1"))).family).toMatch(/^Georgia/);
      expect(await look(shell.locator("nav .new"))).toMatchObject({
        size: "14px",
        weight: "600",
        background: rgbOf(c.primary),
        colour: rgbOf(c.onPrimary),
        radius: "7px",
      });
      expect(await look(shell.locator("fs-search-bar .frame"))).toMatchObject({
        size: "14px",
        borderWidth: await renderedBorder(page, "1.5px"),
        borderColour: rgbOf(c.control),
        radius: "6px",
        background: rgbOf(c.card),
      });
      expect(await look(shell.locator("fs-host-indicator button.badge"))).toMatchObject({
        size: "13px",
        weight: "600",
        background: rgbOf(c.raised),
      });
      expect(await look(shell.locator("header"))).toMatchObject({ borderWidth: "0px" });
      expect(await look(shell.locator("footer"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
        borderWidth: "1px",
        borderColour: rgbOf(c.outline),
      });
      await expect(shell.locator("nav .new")).toHaveText(label(labels, "fileNew"));
      await expect(shell.locator("nav input.filter")).toHaveAttribute(
        "placeholder",
        label(labels, "fileFilterView"),
      );
      await expect(shell.locator("fs-search-bar input.text")).toHaveAttribute(
        "placeholder",
        label(labels, "searchPlaceholder"),
      );
      await expect(shell.locator("nav .storage-title")).toHaveText(
        label(labels, "fileStorageUsed"),
      );
      await expect(shell.locator("nav ul.deleted a")).toHaveText(
        label(labels, "navigationRecentlyDeleted"),
      );
      await expect(shell.locator("fs-host-indicator .label")).toHaveText(
        label(labels, "hostBadgeDesktop"),
      );
      await expect(shell.locator("header fs-theme-control")).toHaveCount(0);
      expect(await tokenColour(page, "--fs-surface")).toBe(rgbOf(c.surface));
      expect(errors).toEqual([]);
    });

    test("the navigation tree as built (55, UI-S-017): eyebrows, 30 px rows, 16 px icons", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await page.goto("/#/navigation/c1");
      const tree = page.locator("fs-app-shell fs-nav-tree");
      const eyebrow = tree.locator("section h2").first();
      await expect(eyebrow).toHaveText(label(labels, "collections"));
      expect(await look(eyebrow)).toMatchObject({
        size: "11px",
        weight: "400",
        transform: "uppercase",
        spacing: "1.1px",
        colour: rgbOf(c.muted),
      });
      const current = tree.locator('[role="treeitem"][aria-current="page"]');
      await expectBox(current, { height: 30 });
      await expect
        .poll(() => look(current))
        .toMatchObject({
          size: "13px",
          weight: "500",
          background: rgbOf(c.tint),
          colour: rgbOf(c.active),
        });
      await expectBox(current.locator(".kind-icon"), { width: 16, height: 16 });
      const bar = await current.evaluate((el) => {
        const css = getComputedStyle(el, "::before");
        return { width: css.width, colour: css.backgroundColor };
      });
      expect(bar).toEqual({ width: "2.5px", colour: rgbOf(c.primary) });
      const divider = await page
        .locator("fs-app-shell nav ul.deleted")
        .evaluate((el) => getComputedStyle(el).borderTopColor);
      expect(divider).toBe(rgbOf(c.outline));
      expect(errors).toEqual([]);
    });

    test("welcome (01, UI-S-027): title, lead, cards, privacy box and footer", async ({ page }) => {
      const errors = trackErrors(page);
      await page.goto("/#/library");
      const start = page.locator("fs-library-start");
      await expect(page.locator("fs-app-shell")).toHaveAttribute("frameless", "");
      const title = start.locator(".page > h2");
      await expect(title).toHaveText(label(labels, "libraryWelcomeTitle"));
      const titleLook = await look(title);
      expect(titleLook).toMatchObject({ size: "56px", weight: "700", colour: rgbOf(c.heading) });
      expect(titleLook.family).toMatch(/^Georgia/);
      expect(await look(start.locator(".intro"))).toMatchObject({ size: "17px" });
      await expect(start.locator(".page > .eyebrow")).toHaveText(label(labels, "libraries"));
      const cards = start.locator(".cards section");
      await expect(cards).toHaveCount(3);
      for (const [index, x] of [202, 554, 906].entries()) {
        await expectBox(cards.nth(index), { x, width: 332, height: 248 });
      }
      expect(await look(cards.nth(0))).toMatchObject({
        borderWidth: "2px",
        borderColour: rgbOf(c.primary),
        radius: "10px",
        background: rgbOf(c.card),
      });
      expect(await look(cards.nth(1))).toMatchObject({
        borderWidth: await renderedBorder(page, "1.5px"),
        borderColour: rgbOf(c.strong),
        radius: "10px",
      });
      const cardTitle = await look(cards.nth(0).locator("h3"));
      expect(cardTitle).toMatchObject({ size: "21px", weight: "600" });
      expect(cardTitle.family).toMatch(/^Georgia/);
      expect(await look(cards.nth(0).locator("button"))).toMatchObject({
        background: rgbOf(c.primary),
        colour: rgbOf(c.onPrimary),
        size: "14px",
        weight: "600",
      });
      expect(await look(cards.nth(1).locator("button"))).toMatchObject({
        background: rgbOf(c.card),
        borderColour: rgbOf(c.strong),
        borderWidth: await renderedBorder(page, "1.5px"),
      });
      await expectBox(start.locator(".privacy"), { x: 202, width: 1036 });
      expect(await look(start.locator(".privacy"))).toMatchObject({
        background: rgbOf(c.raised),
        radius: "8px",
      });
      await expect(start.locator(".privacy fs-host-indicator .statement").first()).toHaveText(
        label(labels, "hostDesktop"),
      );
      await expect(start.locator(".app-footer")).toHaveText(
        label(labels, "appFooter").replace("{version}", "0.1.0"),
      );
      expect(errors).toEqual([]);
    });

    test("folder-choice unavailable (03) and the view states treatment (56)", async ({ page }) => {
      const errors = trackErrors(page, /status of 503 \(/);
      const api = await serveLibraries(page);
      api.chooser = "unavailable";
      await page.goto("/#/library");
      await page.locator('fs-library-start button[data-action="open"]').click();
      const states = page.locator("fs-library-start fs-states");
      await expect(states.locator(".title")).toHaveText(label(labels, "stateUnavailable"));
      await expect(states.locator(".detail")).toHaveText(
        label(labels, "libraryChooserUnavailable"),
      );
      expect(await look(states.locator(".title"))).toMatchObject({
        size: "15px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      expect(await look(states.locator(".detail"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      const retry = states.locator("button");
      await expect(retry).toHaveText(label(labels, "stateRetry"));
      await expectBox(retry, { height: 36 });
      expect(await look(retry)).toMatchObject({
        radius: "7px",
        borderWidth: await renderedBorder(page, "1.5px"),
        borderColour: rgbOf(c.strong),
        background: rgbOf(c.card),
      });
      await expectBox(states.locator("svg.icon"), { width: 24, height: 24 });
      expect(await look(states.locator("svg.icon"))).toMatchObject({ colour: rgbOf(c.error) });
      await expect(page.locator("fs-library-start .cards section")).toHaveCount(3);
      expect(errors).toEqual([]);
    });

    test("library page (41, UI-S-027): facts rows, actions and the W010 note", async ({ page }) => {
      const errors = trackErrors(page);
      await openLibraryPage(page);
      const detail = page.locator("fs-library-detail");
      const title = detail.locator(".page > h2");
      // The page is titled with the library's name from its facts (the fake's default name).
      await expect(title).toHaveText("Untitled library");
      expect(await look(title)).toMatchObject({ size: "30px", weight: "700" });
      await expect(detail.locator("[data-section='facts'] h3")).toHaveText(
        label(labels, "libraryFactsTitle"),
      );
      expect(await look(detail.locator("[data-section='facts'] h3"))).toMatchObject({
        size: "11px",
        weight: "400",
        transform: "uppercase",
        colour: rgbOf(c.muted),
      });
      const term = detail.locator("dt").first();
      const value = detail.locator("dd").first();
      await expect(term).toHaveText(label(labels, "libraryFactId"));
      await expect(value).toHaveText(LIB);
      expect(await look(term)).toMatchObject({ size: "13px", colour: rgbOf(c.muted) });
      expect((await look(value)).family).toMatch(/Mono|monospace|Consolas/);
      const termBox = await term.boundingBox();
      const valueBox = await value.boundingBox();
      expect(Math.abs((valueBox?.x ?? 0) - (termBox?.x ?? 0) - 160)).toBeLessThanOrEqual(2);
      const actions = detail.locator("[data-section='actions'] button");
      await expect(actions).toHaveText([
        label(labels, "libraryCopy"),
        label(labels, "libraryRebuild"),
      ]);
      expect(await look(actions.first())).toMatchObject({
        borderColour: rgbOf(c.strong),
        background: rgbOf(c.card),
        radius: "7px",
      });
      const note = detail.locator("[data-section='repair-note']");
      await expectBox(note, { width: 448 });
      expect(await look(note)).toMatchObject({
        borderWidth: "1px",
        borderColour: rgbOf(c.error),
        radius: "8px",
      });
      await expect(note.locator(".reason")).toHaveText(label(labels, "libraryReadOnlyReason"));
      expect(await look(note.locator("a"))).toMatchObject({
        colour: rgbOf(c.primary),
        weight: "600",
      });
      expect(errors).toEqual([]);
    });

    test("repair consent (42) and results (43): dialog, scrim, chips and the refused box", async ({
      page,
    }) => {
      const errors = trackErrors(page, /status of 409 \(/);
      const api = await serveLibraries(page);
      await page.goto(`/#/library/${LIB}/repair`);
      const repair = page.locator("fs-library-repair");
      const review = repair.locator('button[data-action="review"]');
      await expect(review).toHaveText(label(labels, "repairReview"));
      expect(await look(review)).toMatchObject({ background: rgbOf(c.primary) });
      await expect(repair.locator(".page > h2")).toHaveText(label(labels, "repairTitle"));
      expect((await look(repair.locator(".offer li"))).family).toMatch(/Mono|monospace|Consolas/);
      await review.click();
      const dialog = repair.locator('[role="dialog"]');
      await expectBox(dialog, { width: 600 });
      expect(await look(dialog)).toMatchObject({
        background: rgbOf(c.overlay),
        borderColour: rgbOf(c.strong),
        radius: "10px",
      });
      const dialogTitle = await look(dialog.locator("h3"));
      expect(dialogTitle).toMatchObject({ size: "22px", weight: "700" });
      expect(dialogTitle.family).toMatch(/^Georgia/);
      await expect(dialog.locator(".consent")).toHaveText(label(labels, "repairConsent"));
      const scrim = await repair.locator(".scrim").evaluate((el) => {
        const css = getComputedStyle(el, "::before");
        return { background: css.backgroundColor, opacity: css.opacity };
      });
      expect(scrim).toEqual({ background: rgbOf("#000000"), opacity: "0.32" });
      await expect(dialog.locator('button[data-action="cancel"]')).toBeFocused();
      await expectBox(dialog.locator('button[data-action="confirm"]'), { height: 38 });
      api.repairRefusal = { status: 409, code: "conflict" };
      await dialog.locator('button[data-action="confirm"]').click();
      const refused = repair.locator(".feedback > p.alert");
      await expect(refused).toBeVisible();
      expect(await look(refused)).toMatchObject({
        borderWidth: "1px",
        borderColour: rgbOf(c.error),
        radius: "8px",
        size: "13px",
        weight: "600",
      });
      expect(errors).toEqual([]);
    });

    test("Monitor (31) and its OCFL validation section (54)", async ({ page }) => {
      const errors = trackErrors(page);
      await page.goto("/#/monitor");
      const monitor = page.locator("fs-monitor");
      const head = monitor.locator(".page-head");
      await expect(head.locator("h2")).toHaveText(label(labels, "monitor"));
      expect(await look(head.locator("h2"))).toMatchObject({ size: "30px", weight: "700" });
      await expect(head.locator(".owner")).toHaveText(label(labels, "monitorOwnerOnly"));
      await expect(head.locator(".chip")).toHaveText(label(labels, "monitorLive"));
      expect(await look(head.locator(".chip"))).toMatchObject({
        background: rgbOf(c.tint),
        colour: rgbOf(c.active),
        size: "11px",
        weight: "500",
      });
      const download = head.locator('[data-action="export"]');
      await expect(download).toHaveText(label(labels, "monitorExport"));
      expect(await look(download)).toMatchObject({ borderColour: rgbOf(c.strong) });
      const health = monitor.locator('[data-section="health"]');
      expect(await look(health)).toMatchObject({
        background: rgbOf(c.card),
        borderWidth: "1px",
        borderColour: rgbOf(c.outline),
        radius: "10px",
      });
      await expect(health.locator(".status")).toHaveText(label(labels, "statusDegraded"));
      await expect(health.locator('li[data-reason="process_stalled"]')).toHaveText(
        "1 process(es) stalled",
      );
      const status = await look(health.locator(".status"));
      expect(status).toMatchObject({ size: "22px", weight: "700", colour: rgbOf(c.error) });
      expect(status.family).toMatch(/^Georgia/);
      expect(await look(monitor.locator('[data-section="processes"] th'))).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.variant),
      });
      const stalled = monitor.locator('tr[data-state="stalled"] td:nth-child(2)');
      await expect(stalled).toHaveText("stalled");
      expect(await look(stalled)).toMatchObject({ colour: rgbOf(c.error) });
      const findings = monitor.locator("fs-ocfl-findings");
      await expect(findings).toHaveAttribute("data-state", "ready");
      for (const part of ["errors", "warnings"]) {
        const code = findings.locator(`[data-part="${part}"] td.code .chip`);
        expect(await look(code)).toMatchObject({
          borderWidth: "1px",
          borderColour: rgbOf(c.error),
          colour: rgbOf(c.error),
          size: "11px",
        });
      }
      const running = findings.locator('[data-part="runs"] tbody td:nth-child(2) .chip');
      await expect(running).toHaveText(label(labels, "findingsRunning"));
      expect(await look(running)).toMatchObject({ background: rgbOf(c.tint) });
      expect(await look(findings.locator('[data-action="validate"]'))).toMatchObject({
        borderColour: rgbOf(c.strong),
        background: rgbOf(c.card),
      });
      expect(errors).toEqual([]);
    });

    test("host popover (40) and Settings (32): 372 px overlay, Appearance and About", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await page.goto("/#/settings");
      const badge = page.locator("fs-app-shell header fs-host-indicator button.badge");
      await expect(page.locator("fs-app-shell header fs-host-indicator")).toHaveAttribute(
        "data-state",
        "ready",
      );
      await badge.click();
      const popover = page.locator('fs-app-shell header fs-host-indicator [role="dialog"]');
      await expectBox(popover, { width: 372 });
      expect(await look(popover)).toMatchObject({
        background: rgbOf(c.overlay),
        borderColour: rgbOf(c.strong),
        radius: "10px",
      });
      await page.keyboard.press("Escape");
      const settings = page.locator("fs-settings");
      expect(await look(settings.locator(".page-title"))).toMatchObject({
        size: "30px",
        weight: "700",
      });
      const sectionTitle = settings.locator(".section-title").first();
      await expect(sectionTitle).toHaveText(label(labels, "settingsAppearance"));
      expect(await look(sectionTitle)).toMatchObject({ size: "22px", weight: "600" });
      const chosen = settings.locator("fs-theme-control label:has(input:checked)");
      expect(await look(chosen)).toMatchObject({
        weight: "600",
        background: rgbOf(c.tint),
        colour: rgbOf(c.active),
      });
      await expect(settings.locator(".help").first()).toHaveText(label(labels, "themeHelp"));
      await expect(settings.locator(".api")).toContainText("API: ok · contract");
      expect(errors).toEqual([]);
    });
  });
}
