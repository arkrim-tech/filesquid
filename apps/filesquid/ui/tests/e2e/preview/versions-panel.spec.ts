/**
 * The Versions view additions of Task UI-F2.11 against the production build (`vite build` plus
 * `vite preview`) at the mockups' 1440 × 900 window, in light and dark (UI-S-009, UI-S-024,
 * UI-S-031, UI-S-013; PRD F-065): the per-version menu with Restore as new version, Download
 * this version and Edit label (mockup 26), the Edit label dialog (built like mockup 27's restore
 * dialog; no mockup draws it), the download of one version, and the history size row of File
 * info. Each test asserts the custom elements are defined, the strings come from the active
 * label table, the regions' boxes match the mockup within 2 px (a mockup y of `n` is `n - 28`
 * here), the constants are the browser's computed styles, and there is no console error. The
 * routes are faked from the mockups' data; the clock is fixed at 14:00 UTC on 16 October 2026.
 */
import { statSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
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
import {
  FACADE,
  HARBOUR,
  PANEL_OBJECT,
  PANEL_ROUTE,
  servePanel,
  type PanelApi,
  type PanelFixture,
} from "../support/file-panel-api";

const labels = activeLabels();
const SCHEMES: readonly Scheme[] = ["light", "dark"];
const NOW = new Date("2026-10-16T14:00:00Z");

// A 2x (Retina) display, as the macOS desktop target has.
test.use({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });

/** Fill the placeholders of a label. */
function fill(key: string, values: Record<string, string>): string {
  return label(labels, key).replace(/\{(\w+)\}/g, (whole, name: string) => values[name] ?? whole);
}

/** Serve `fixture`, open its folder and open the file, so the file panel shows it. */
async function openPanel(page: Page, fixture: PanelFixture): Promise<PanelApi> {
  await page.clock.setFixedTime(NOW);
  const api = await servePanel(page, fixture);
  await page.goto(PANEL_ROUTE);
  await expect(page.locator("fs-file-browser")).toHaveAttribute("data-state", "ready");
  await page.getByRole("button", { name: fixture.displayName }).click();
  await expect(page.locator("fs-file-panel")).toBeVisible();
  return api;
}

/** Open the Versions view of the harbour fixture and wait for its rows. */
async function openVersions(page: Page): Promise<PanelApi> {
  const api = await openPanel(page, HARBOUR);
  await page.locator("fs-file-panel button.history").click();
  await expect(page.locator("fs-versions-panel")).toHaveAttribute("data-state", "ready");
  return api;
}

/** Open a version's menu and choose `action`. */
async function choose(page: Page, version: string, action: string): Promise<void> {
  const view = page.locator("fs-versions-panel");
  await view.locator(`li[data-version="${version}"] button.more`).click();
  await view.locator(`[role="menuitem"][data-action="${action}"]`).click();
}

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => setScheme(page, scheme));

    test("menu: the current version offers Download and Edit label, with 16 px icons in the icon colour", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await openVersions(page);
      await expectElementsDefined(page, ["fs-file-panel", "fs-versions-panel"]);
      const view = page.locator("fs-versions-panel");
      const current = view.locator('li[data-version="v7"]');
      await expect(current.locator(".current")).toHaveText(label(labels, "versionsCurrent"));
      await current.locator("button.more").click();
      const menu = view.locator('[role="menu"]');
      const items = menu.locator('[role="menuitem"]');
      await expect(items).toHaveText([
        label(labels, "versionsDownloadAction"),
        label(labels, "versionsEditLabelAction"),
      ]);
      await expectBox(menu, { width: 232, height: 76 });
      await expect(items.first()).toBeFocused();
      const icon = items.first().locator("svg");
      await expectBox(icon, { width: 16, height: 16 });
      await expect(icon).toHaveAttribute("stroke-width", "1.6");
      expect((await look(icon)).colour).toBe(rgbOf(c.icon));
      await items.nth(1).hover();
      expect(await look(items.nth(1))).toMatchObject({
        background: rgbOf(c.raised),
        radius: "5px",
      });
      await page.keyboard.press("Escape");
      await expect(menu).toHaveCount(0);
      expect(errors).toEqual([]);
    });

    test("menu: a version offers no delete, expiry or thinning", async ({ page }) => {
      const errors = trackErrors(page);
      await openVersions(page);
      const view = page.locator("fs-versions-panel");
      for (const version of ["v7", "v5", "v1"]) {
        await view.locator(`li[data-version="${version}"] button.more`).click();
        const words = (await view.locator('[role="menu"]').innerText()).toLowerCase();
        expect(words).not.toMatch(/delet|expir|thin/);
        await page.keyboard.press("Escape");
      }
      expect(errors).toEqual([]);
    });

    test("Edit label: the dialog, one call with the head as revision, and the new label in the row", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await openVersions(page);
      const view = page.locator("fs-versions-panel");
      await choose(page, "v6", "edit-label");
      const dialog = view.locator('[role="dialog"]');
      await expect(dialog).toBeVisible();
      await expectBox(dialog, { x: 450, width: 540 });
      expect(await look(dialog)).toMatchObject({
        background: rgbOf(c.overlay),
        borderWidth: "1px",
        borderColour: rgbOf(c.strong),
        radius: "10px",
      });
      const title = dialog.locator(".dialog-title");
      expect(await look(title)).toMatchObject({
        size: "22px",
        weight: "700",
        colour: rgbOf(c.heading),
      });
      expect((await look(title)).family).toMatch(/^Georgia/);
      await expect(title).toHaveText(fill("versionsEditLabelTitle", { version: "v6" }));
      await expect(dialog.locator(".file-name")).toHaveText("harbour-masterplan-v7.pdf");
      expect(await look(dialog.locator(".label-field"))).toMatchObject({
        size: "13px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      await expect(dialog.locator(".label-field")).toHaveText(
        label(labels, "versionsEditLabelField"),
      );
      expect(await look(dialog.locator(".help"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      await expect(dialog.locator(".help")).toHaveText(label(labels, "versionsEditLabelHelp"));
      const input = dialog.locator("input.label-input");
      await expect(input).toBeFocused();
      await expect(input).toHaveValue("");
      await expectBox(input, { height: 36 });
      await expect
        .poll(() => look(input))
        .toMatchObject({
          size: "14px",
          radius: "6px",
          borderWidth: "2px",
          borderColour: rgbOf(c.focus),
        });
      const cancel = dialog.locator("button.cancel");
      const save = dialog.locator("button.save");
      await expect(dialog.locator("button.primary")).toHaveCount(1);
      await expectBox(cancel, { height: 38 });
      await expectBox(save, { height: 38 });
      expect(await look(cancel)).toMatchObject({
        size: "14px",
        weight: "600",
        background: rgbOf(c.card),
        borderColour: rgbOf(c.strong),
        radius: "7px",
      });
      expect(await look(save)).toMatchObject({
        size: "14px",
        weight: "600",
        background: rgbOf(c.primary),
        colour: rgbOf(c.onPrimary),
        radius: "7px",
      });
      await expect(cancel).toHaveText(label(labels, "fileCancel"));
      await expect(save).toHaveText(label(labels, "versionsEditLabelSave"));

      await input.fill("  Sent to the client ");
      await save.click();
      await expect(dialog).toHaveCount(0);
      expect(api.labelEdits).toEqual([
        { library_id: "lib", object_id: PANEL_OBJECT, version: "v6", label: "Sent to the client" },
      ]);
      expect(api.labelRevisions).toEqual(["v7"]);
      await expect(view.locator("p.announce")).toHaveText(
        fill("versionsLabelSaved", { version: "v6" }),
      );
      const row = view.locator('li[data-version="v6"]');
      await expect(row.locator(".label")).toHaveText("Sent to the client");
      expect(await look(row.locator(".label"))).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      await expect(view.locator(".back-row .count")).toHaveText(
        fill("versionsCount", { count: "7" }),
      );

      await choose(page, "v6", "edit-label");
      await expect(dialog.locator("input.label-input")).toHaveValue("Sent to the client");
      await dialog.locator("input.label-input").fill("");
      await dialog.locator("button.save").click();
      await expect(dialog).toHaveCount(0);
      expect(api.labelEdits[1]).toMatchObject({ version: "v6", label: null });
      expect(api.labelRevisions).toEqual(["v7", "v8"]);
      await expect(row.locator(".no-label")).toHaveText(label(labels, "versionsNoLabel"));
      await expect(view.locator("p.announce")).toHaveText(
        fill("versionsLabelCleared", { version: "v6" }),
      );
      expect(errors).toEqual([]);
    });

    test("Edit label: an unchanged label sends nothing, and a head moved meanwhile is refused in the dialog", async ({
      page,
    }) => {
      const errors = trackErrors(
        page,
        (text, url) => /status of 409/.test(text) && url.includes("/api/v1/files/labels"),
      );
      const api = await openVersions(page);
      const view = page.locator("fs-versions-panel");
      const dialog = view.locator('[role="dialog"]');
      await choose(page, "v5", "edit-label");
      await expect(dialog.locator("input.label-input")).toHaveValue("Client review");
      await dialog.locator("button.save").click();
      await expect(dialog).toHaveCount(0);
      expect(api.labelEdits).toEqual([]);

      await choose(page, "v5", "edit-label");
      api.moveHead();
      await dialog.locator("input.label-input").fill("Approved");
      await dialog.locator("button.save").click();
      const message = dialog.locator(".error");
      await expect(message).toHaveText(label(labels, "fileConflict"));
      expect(await look(message)).toMatchObject({ size: "13px", colour: rgbOf(c.error) });
      await expect(dialog).toBeVisible();
      expect(api.labelRevisions).toEqual(["v7"]);
      await page.keyboard.press("Escape");
      await expect(dialog).toHaveCount(0);
      await expect(view.locator('li[data-version="v5"] button.more')).toBeFocused();
      expect(errors).toEqual([]);
    });

    test("Download this version: saves that version's file under the display name", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await openVersions(page);
      const [download] = await Promise.all([
        page.waitForEvent("download"),
        choose(page, "v4", "download"),
      ]);
      expect(download.suggestedFilename()).toBe("harbour-masterplan-v7.pdf");
      expect(api.downloads).toEqual([
        { library_id: "lib", object_id: PANEL_OBJECT, version: "v4", path: HARBOUR.path },
      ]);
      await expect(page.locator("fs-versions-panel .notice")).toHaveCount(0);
      expect(errors).toEqual([]);
    });

    test("Download this version: a version above the desktop cap is read in ranged 8 MiB reads and saved whole", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await openVersions(page);
      const [download] = await Promise.all([
        page.waitForEvent("download"),
        choose(page, "v5", "download"),
      ]);
      expect(download.suggestedFilename()).toBe("harbour-masterplan-v7.pdf");
      const saved = await download.path();
      expect(statSync(saved).size).toBe(HARBOUR.versions[4]?.[2]);
      expect(api.contentReads).toEqual([
        "bytes=0-8388607",
        "bytes=8388608-16777215",
        "bytes=16777216-18769509",
      ]);
      expect(api.downloads).toEqual([]);
      await expect(page.locator("fs-versions-panel .notice")).toHaveCount(0);
      expect(errors).toEqual([]);
    });

    test("Download this version: a refused download shows a visible alert and saves nothing", async ({
      page,
    }) => {
      const errors = trackErrors(
        page,
        (text, url) => /status of 503/.test(text) && url.includes("/api/v1/files/download"),
      );
      await openVersions(page);
      await page.route(/\/api\/v1\/files\/download\?/, (route) =>
        route.fulfill({
          status: 503,
          json: { code: "unavailable", message: "refused", retryable: true },
        }),
      );
      let saved = false;
      page.on("download", () => (saved = true));
      await choose(page, "v4", "download");
      const alert = page.locator('fs-versions-panel [role="alert"].notice');
      await expect(alert).toHaveText(fill("versionsDownloadFailed", { version: "v4" }));
      expect(await look(alert)).toMatchObject({
        size: "13px",
        colour: rgbOf(c.error),
        background: rgbOf(c.card),
        borderColour: rgbOf(c.error),
        radius: "5px",
        borderWidth: await renderedBorder(page, "1.5px"),
      });
      expect(saved).toBe(false);
      expect(errors).toEqual([]);
    });

    test("File info: the history size row follows the size, from the history-size route", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await openPanel(page, FACADE);
      const details = page.locator("fs-details-panel");
      await expect(details).toHaveAttribute("data-state", "ready");
      const rows = details.locator(".row");
      await expect(rows.nth(3).locator("dt")).toHaveText(label(labels, "filePanelSize"));
      const row = details.locator('.row[data-key="history-size"]');
      await expect(row.locator("dt")).toHaveText(label(labels, "filePanelHistorySize"));
      await expect(row.locator(".value")).toHaveText(
        fill("filePanelHistorySizeValue", {
          size: "96.3 MB",
          versions: fill("versionsCount", { count: "4" }),
        }),
      );
      expect(await look(row.locator(".value"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.body),
      });
      await expect(details.locator(".count")).toHaveText(
        fill("filePanelInfoCount", { count: "9" }),
      );
      expect(errors).toEqual([]);
    });
  });
}
