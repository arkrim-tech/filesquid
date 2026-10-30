/**
 * The composition that is measured is the v3 composition, not a stripped page (Task UI-F3.1,
 * UI-S-024, UI-S-025, UI-S-026, UI-S-031, mockups 05 and 22): at the mockups' 1440 x 900 window,
 * in light and dark, with the Large catalog loaded (11,000 rows, the 50,000-album tree), the
 * shell regions, the list's header, rows and controls and the preview's file panel hold their
 * boxes within 2 px, the constants are the browser's computed styles, every word comes from the
 * active label table, the custom elements are defined and the console has no error. A list that
 * only kept its look with 13 rows would not be the product the budgets describe.
 *
 * The mockups draw a 28 px macOS title bar the webview has not, so a mockup y of n is n - 28 here.
 * Mockup 22's bar (collection, badge, zoom chip) and its filmstrip are built by no task done so
 * far; the preview is measured as built (viewer header, media, file panel) and the gap is
 * recorded in the Walkthrough.
 */
import { expect, test, type Page } from "@playwright/test";
import { FILES_FIRST } from "./browse-classes";
import { Catalogue, PROFILES } from "./fixtures";
import { VIEW_LOCATOR, openView } from "./harness";
import { Probe } from "./probe";
import { serveHost } from "./host";
import { smallName, previewFiles } from "./preview-fixtures";
import { MemoryFileSource } from "../../src/files/memory-source";
import { COLOURS, expectBox, look } from "../e2e/support/fidelity";
import { serveFiles } from "../e2e/support/files-api";
import {
  activeLabels,
  expectElementsDefined,
  label,
  rgbOf,
  setScheme,
  trackErrors,
  type Scheme,
} from "../e2e/support/strict-dom";

const labels = activeLabels();
const SCHEMES: readonly Scheme[] = ["light", "dark"];

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => setScheme(page, scheme));

    test("the shell and the list of mockup 05 hold with the Large catalog loaded", async ({
      page,
    }) => {
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
      await expectElementsDefined(page, [
        "fs-app-shell",
        "fs-nav-tree",
        "fs-navigation-view",
        "fs-drive-view",
        "fs-search-bar",
        "fs-host-indicator",
      ]);
      const shell = page.locator("fs-app-shell");
      const view = page.locator(VIEW_LOCATOR);
      await expectBox(shell.locator("nav").first(), { x: 0, y: 0, width: 240, height: 900 });
      await expectBox(shell.locator("header"), { x: 240, y: 0, width: 1200, height: 60 });
      await expectBox(shell.locator("footer"), { x: 240, y: 860, width: 1200, height: 40 });
      await expectBox(shell.locator("nav .new"), { x: 16, width: 208, height: 38 });
      await expectBox(shell.locator("nav input.filter"), { x: 16, width: 208, height: 34 });
      await expectBox(shell.locator("fs-search-bar .frame"), { x: 264, width: 520, height: 36 });
      await expectBox(view.locator(".page-title"), { x: 264, y: 100, height: 34 });
      await expectBox(view.locator(".layout"), { x: 1344, width: 72, height: 32 });
      await expectBox(view.locator(".head"), { x: 264, y: 142, width: 1152, height: 38 });
      await expectBox(view.locator(".row").first(), { x: 264, y: 180, width: 1152, height: 44 });
      expect(await look(shell.locator("nav").first())).toMatchObject({ background: rgbOf(c.card) });
      expect(await look(view.locator(".page-title"))).toMatchObject({
        size: "30px",
        weight: "700",
        colour: rgbOf(c.heading),
      });
      expect((await look(view.locator(".page-title"))).family).toMatch(/^Georgia/);
      const heads = view.locator("[role=columnheader]");
      await expect(heads.nth(1)).toContainText(label(labels, "fileColumnName"));
      await expect(heads.nth(2)).toContainText(label(labels, "fileColumnSize"));
      await expect(heads.nth(3)).toContainText(label(labels, "fileColumnModified"));
      expect(await look(heads.nth(2))).toMatchObject({ size: "13px", colour: rgbOf(c.variant) });
      const pdf = view
        .locator(".row")
        .filter({ has: page.locator('.name:text-is("scan.pdf")') })
        .first();
      expect(await look(pdf.locator(".name"))).toMatchObject({
        size: "14px",
        colour: rgbOf(c.body),
      });
      expect(await look(pdf.locator(".badge"))).toMatchObject({
        size: "10px",
        weight: "700",
        background: rgbOf("#fd1e23"),
        colour: rgbOf("#ffffff"),
        radius: "3px",
        transform: "uppercase",
      });
      await expectBox(pdf.locator(".badge"), { width: 30, height: 16 });
      await pdf.hover();
      expect(await look(pdf)).toMatchObject({ background: rgbOf(c.raised) });
      await expectBox(pdf.locator("button.actions"), { width: 30, height: 30 });
      await expect(shell.locator("nav .new")).toHaveText(label(labels, "fileNew"));
      await expect(shell.locator("fs-search-bar input.text")).toHaveAttribute(
        "placeholder",
        label(labels, "searchPlaceholder"),
      );
      expect(session.errors).toEqual([]);
      expect(session.host.unserved).toEqual([]);
    });

    test("the preview with its file panel (mockups 22 and 35) as built, over generated images", async ({
      page,
      browser,
    }) => {
      const errors = trackErrors(page);
      await Probe.install(page);
      const host = await serveHost(page, new Catalogue(PROFILES.small));
      const source = new MemoryFileSource({
        library_id: "lib",
        object_id: "obj",
        files: (await previewFiles(browser)).files,
      });
      await serveFiles(page, source);
      await page.goto("/#/files/lib/obj/small");
      await page.getByRole("button", { name: smallName(1), exact: true }).click();
      const preview = page.locator("fs-file-preview");
      await expect(preview).toHaveAttribute("data-state", "ready");
      await expect(page.locator("fs-file-panel")).toBeVisible();
      await expectElementsDefined(page, [
        "fs-file-browser",
        "fs-file-preview",
        "fs-file-panel",
        "fs-details-panel",
      ]);
      const shell = page.locator("fs-app-shell");
      await expectBox(shell.locator("aside.panel"), { x: 1120, y: 61, width: 320, height: 840 });
      await expectBox(shell.locator("header"), { x: 240, y: 0, width: 1200, height: 60 });
      await expectBox(shell.locator("footer"), { x: 240, y: 860, width: 880, height: 40 });
      expect(await look(shell.locator("aside.panel"))).toMatchObject({ background: rgbOf(c.card) });
      const edge = await shell.locator("aside.panel").evaluate((el) => {
        const css = getComputedStyle(el);
        return { width: css.borderInlineStartWidth, colour: css.borderInlineStartColor };
      });
      expect(edge).toEqual({ width: "1px", colour: rgbOf(c.outline) });
      expect(await look(preview.locator("img"))).toMatchObject({
        background: rgbOf(c.raised),
        // As built: the token radius (8 px). Mockup 22 draws 10 px; recorded as a gap in the
        // Walkthrough, since no task built the mockup's media frame yet.
        radius: "8px",
      });
      await expect(page.locator("fs-file-browser .viewer-title")).toHaveText(smallName(1));
      await expect(page.locator("fs-file-browser button.next")).toHaveText(
        label(labels, "fileNext"),
      );
      expect(errors).toEqual([]);
      expect(host.unserved).toEqual([]);
    });
  });
}

/** Keep the type import used when a helper above is trimmed. */
export type { Page };
