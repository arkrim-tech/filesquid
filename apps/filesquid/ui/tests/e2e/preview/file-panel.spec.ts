/**
 * The file panel against the production build (`vite build` plus `vite preview`) at the
 * mockups' 1440 × 900 window, in light and dark (Task UI-F2.3; UI-S-003, UI-S-009, UI-S-024,
 * UI-S-031; UI-S-013): the File info tab of mockup 35, the Versions view of mockup 26, the
 * restore dialog of mockup 27 and the panel states of mockup 25. Each test asserts the custom
 * elements are defined, the strings come from the active label table, the regions' boxes match
 * the mockup within 2 px (a mockup y of `n` is `n - 28` here, below its title bar; regions held
 * at the window's foot keep the mockup's y), the constants are the browser's computed styles,
 * and there is no console error. The routes are faked from the mockups' data, and the clock is
 * fixed at 14:00 UTC on 16 October 2026, an hour after the newest version.
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
import { COLOURS, expectBox, look, renderedBorder } from "../support/fidelity";
import {
  FACADE,
  HARBOUR,
  PANEL_OBJECT,
  PANEL_ROUTE,
  PANEL_STORED,
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

/** Every text and attribute value in every shadow root of the page, as one string. */
function deepText(page: Page): Promise<string> {
  return page.evaluate(() => {
    const parts: string[] = [];
    const walk = (root: ParentNode) => {
      for (const el of root.querySelectorAll("*")) {
        parts.push(el.textContent ?? "", ...[...el.attributes].map((a) => `${a.name}=${a.value}`));
        if (el.shadowRoot) walk(el.shadowRoot);
      }
    };
    walk(document);
    return parts.join("\n");
  });
}

/** The inline-start border of an element: width and colour. */
function startEdge(locator: Locator): Promise<{ width: string; colour: string }> {
  return locator.evaluate((el) => {
    const css = getComputedStyle(el);
    return { width: css.borderInlineStartWidth, colour: css.borderInlineStartColor };
  });
}

/** The block-end border of an element: width and colour. */
function endEdge(locator: Locator): Promise<{ width: string; colour: string }> {
  return locator.evaluate((el) => {
    const css = getComputedStyle(el);
    return { width: css.borderBlockEndWidth, colour: css.borderBlockEndColor };
  });
}

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => setScheme(page, scheme));

    test("File info (35): header, tab, filter, count, rows and footer", async ({ page }) => {
      const errors = trackErrors(page);
      await openPanel(page, FACADE);
      await expectElementsDefined(page, ["fs-app-shell", "fs-file-panel", "fs-details-panel"]);
      const details = page.locator("fs-details-panel");
      await expect(details).toHaveAttribute("data-state", "ready");
      const shell = page.locator("fs-app-shell");
      const panel = page.locator("fs-file-panel");
      await expectBox(shell.locator("aside.panel"), { x: 1120, y: 61, width: 320, height: 840 });
      await expectBox(shell.locator("header"), { x: 240, y: 0, width: 1200, height: 60 });
      await expectBox(shell.locator("footer"), { x: 240, y: 860, width: 880, height: 40 });
      await expectBox(panel.locator(".badge"), { x: 1140, y: 87, height: 16 });
      await expectBox(panel.locator("button.history"), { x: 1364, y: 82, width: 30, height: 28 });
      await expectBox(panel.locator('[role="tab"]').first(), { x: 1140 });
      await expectBox(details.locator("input.filter"), { x: 1140, y: 168, width: 280, height: 32 });
      await expectBox(details.locator(".row").first(), { x: 1121, y: 238, width: 319, height: 44 });
      await expectBox(details.locator(".row").nth(1), { y: 282, height: 44 });
      await expectBox(details.locator(".footer"), { x: 1121, y: 856, width: 319, height: 44 });
      expect(await look(shell.locator("aside.panel"))).toMatchObject({ background: rgbOf(c.card) });
      expect(await startEdge(shell.locator("aside.panel"))).toEqual({
        width: "1px",
        colour: rgbOf(c.outline),
      });
      expect(await look(panel.locator("h2.name"))).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.heading),
      });
      await expect(panel.locator("h2.name")).toHaveCSS("text-overflow", "ellipsis");
      expect(await look(panel.locator(".badge"))).toMatchObject({
        size: "10px",
        weight: "700",
        radius: "3px",
        transform: "uppercase",
      });
      expect((await look(panel.locator(".badge"))).family).toMatch(/Mono|monospace/);
      const tab = panel.locator('[role="tab"][aria-selected="true"]');
      expect(await look(tab)).toMatchObject({
        size: "13px",
        weight: "600",
        colour: rgbOf(c.active),
      });
      expect(await endEdge(tab)).toEqual({ width: "2px", colour: rgbOf(c.primary) });
      expect(await look(details.locator("input.filter"))).toMatchObject({
        size: "13px",
        borderWidth: await renderedBorder(page, "1.5px"),
        borderColour: rgbOf(c.control),
        radius: "6px",
        background: rgbOf(c.card),
      });
      expect(await look(details.locator(".count"))).toMatchObject({
        size: "11px",
        weight: "400",
        transform: "uppercase",
        spacing: "1.1px",
        colour: rgbOf(c.muted),
      });
      expect(await look(details.locator(".row dt").first())).toMatchObject({
        size: "13px",
        colour: rgbOf(c.muted),
      });
      expect(await look(details.locator(".row .value").first())).toMatchObject({
        size: "13px",
        colour: rgbOf(c.body),
      });
      const digest = details.locator('.row[data-key="digest-sha256"] .value');
      expect((await look(digest)).family).toMatch(/Mono|monospace/);
      expect(await look(details.locator(".footer"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.muted),
      });
      await expect(panel.locator("h2.name")).toHaveText("facade-study.jpg");
      await expect(tab).toHaveText(label(labels, "filePanelInfo"));
      await expect(panel.locator("button.history")).toHaveAttribute(
        "aria-label",
        label(labels, "filePanelHistory"),
      );
      await expect(details.locator("input.filter")).toHaveAttribute(
        "placeholder",
        label(labels, "filePanelFilterInfo"),
      );
      await expect(details.locator(".count")).toHaveText(
        fill("filePanelInfoCount", { count: "8" }),
      );
      await expect(details.locator(".row dt")).toHaveText([
        label(labels, "filePanelOriginalName"),
        label(labels, "filePanelExtension"),
        label(labels, "filePanelFormat"),
        label(labels, "filePanelSize"),
        fill("filePanelDigest", { algorithm: "SHA-256" }),
        fill("filePanelDigest", { algorithm: "SHA-512" }),
        label(labels, "filePanelObject"),
        label(labels, "filePanelVersion"),
      ]);
      await expect(details.locator(".row .value")).toHaveText([
        "facade-study.jpg",
        ".jpg",
        "image/jpeg",
        fill("filePanelSizeValue", { size: "6.2 MB", bytes: "6,481,920" }),
        "9b1f4e…c07a2d",
        "5a5a5a…5a5a5a",
        "urn:uuid:7f3c9a1e-…-41d2b8",
        fill("filePanelVersionValue", { version: "v4", total: "4", time: "Oct 16, 2026, 11:05" }),
      ]);
      await expect(details.locator(".footer")).toHaveText(label(labels, "filePanelInfoFooter"));
      const everything = await deepText(page);
      for (const stored of PANEL_STORED) expect(everything).not.toContain(stored);
      expect(errors).toEqual([]);
    });

    test("Versions (26): Back, count, eyebrow, rows, Current, menu and note", async ({ page }) => {
      const errors = trackErrors(page);
      await openVersions(page);
      await expectElementsDefined(page, ["fs-versions-panel"]);
      const view = page.locator("fs-versions-panel");
      const history = page.locator("fs-file-panel button.history");
      await expect(history).toHaveAttribute("aria-pressed", "true");
      await expect(page.locator('fs-file-panel [role="tablist"]')).toBeHidden();
      const rows = view.locator("li.version");
      await expect(rows).toHaveCount(7);
      await expectBox(rows.first(), { x: 1128, y: 192, width: 304, height: 58 });
      await expectBox(rows.nth(1), { y: 254, height: 58 });
      await expectBox(rows.nth(2), { y: 316 });
      await expectBox(rows.first().locator(".mono-chip"), { x: 1162, y: 197, height: 21 });
      await expectBox(rows.first().locator(".current"), { height: 21 });
      await expectBox(view.locator(".note"), { x: 1132, y: 804, width: 296, height: 72 });
      // The pressed state fades in over --fs-motion-duration.
      await expect.poll(async () => (await look(history)).background).toBe(rgbOf(c.tint));
      expect(await look(view.locator("button.back"))).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.variant),
      });
      expect(await look(view.locator(".back-row .count"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.muted),
      });
      expect(await look(view.locator(".content > .eyebrow"))).toMatchObject({
        size: "11px",
        weight: "400",
        transform: "uppercase",
        colour: rgbOf(c.muted),
      });
      const chip = rows.first().locator(".mono-chip");
      expect(await look(chip)).toMatchObject({
        size: "11px",
        weight: "500",
        colour: rgbOf(c.variant),
        background: rgbOf(c.raised),
        radius: "4px",
      });
      expect((await look(chip)).family).toMatch(/Mono|monospace/);
      expect(await look(rows.first().locator(".label"))).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      expect(await look(rows.first().locator(".current"))).toMatchObject({
        size: "11px",
        weight: "500",
        colour: rgbOf(c.active),
        background: rgbOf(c.tint),
        radius: "4px",
      });
      const noLabel = rows.nth(1).locator(".no-label");
      expect(await look(noLabel)).toMatchObject({ size: "13px", colour: rgbOf(c.muted) });
      await expect(noLabel).toHaveCSS("font-style", "italic");
      expect(await look(rows.first().locator(".meta"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      expect(await look(view.locator(".note"))).toMatchObject({
        background: rgbOf(c.raised),
        radius: "8px",
      });
      expect(await look(view.locator(".note-title"))).toMatchObject({
        size: "13px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      await expect(view.locator("button.back")).toHaveText(label(labels, "versionsBack"));
      await expect(view.locator(".back-row .count")).toHaveText(
        fill("versionsCount", { count: "7" }),
      );
      await expect(view.locator(".content > .eyebrow")).toHaveText(
        fill("versionsEyebrow", { size: "96.3 MB" }),
      );
      await expect(rows.locator(".mono-chip")).toHaveText([
        "v7",
        "v6",
        "v5",
        "v4",
        "v3",
        "v2",
        "v1",
      ]);
      await expect(rows.first().locator(".label")).toHaveText("Issued for tender");
      await expect(rows.first().locator(".current")).toHaveText(label(labels, "versionsCurrent"));
      await expect(rows.first().locator(".meta")).toHaveText(
        fill("versionsMeta", { time: "2 hours ago", size: "18.4 MB" }),
      );
      await expect(rows.nth(1).locator(".meta")).toHaveText(
        fill("versionsMeta", { time: "Yesterday", size: "18.1 MB" }),
      );
      await expect(noLabel).toHaveText(label(labels, "versionsNoLabel"));
      await expect(rows.first().locator("button.more")).toHaveCount(0);
      await expect(view.locator(".note-title")).toHaveText(label(labels, "versionsKeepEvery"));
      await expect(view.locator(".note-detail")).toHaveText(label(labels, "versionsKeepDetail"));

      const more = rows.nth(2).locator("button.more");
      await more.click();
      await expect(more).toHaveAttribute("aria-expanded", "true");
      const menu = view.locator('[role="menu"]');
      await expectBox(menu, { x: 1188, y: 346, width: 232 });
      expect(await look(menu)).toMatchObject({
        background: rgbOf(c.overlay),
        borderWidth: "1px",
        borderColour: rgbOf(c.strong),
        radius: "8px",
      });
      const item = menu.locator('[role="menuitem"]');
      await expectBox(item, { height: 34 });
      expect(await look(item)).toMatchObject({
        size: "13px",
        colour: rgbOf(c.body),
        radius: "5px",
      });
      await expect(item).toHaveText([label(labels, "versionsRestoreAction")]);
      await expect(item).toBeFocused();
      await page.keyboard.press("Escape");
      await expect(menu).toHaveCount(0);
      await expect(more).toBeFocused();
      expect(errors).toEqual([]);
    });

    test("restore (27): the dialog, one call with the label, and the new current version", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await openVersions(page);
      const view = page.locator("fs-versions-panel");
      await view.locator('li[data-version="v5"] button.more').click();
      await view.locator('[role="menuitem"][data-action="restore"]').click();
      const dialog = view.locator('[role="dialog"]');
      await expect(dialog).toBeVisible();
      await expectBox(dialog, { x: 450, width: 540, height: 420 });
      expect(await look(dialog)).toMatchObject({
        background: rgbOf(c.overlay),
        borderWidth: "1px",
        borderColour: rgbOf(c.strong),
        radius: "10px",
      });
      const scrim = await view.locator(".scrim").evaluate((el) => {
        const css = getComputedStyle(el, "::before");
        return { background: css.backgroundColor, opacity: css.opacity };
      });
      expect(scrim).toEqual({ background: "rgb(0, 0, 0)", opacity: "0.32" });
      const title = dialog.locator(".dialog-title");
      expect(await look(title)).toMatchObject({
        size: "22px",
        weight: "700",
        colour: rgbOf(c.heading),
      });
      expect((await look(title)).family).toMatch(/^Georgia/);
      expect(await look(dialog.locator(".file-name"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      expect(await look(dialog.locator(".change-box"))).toMatchObject({
        background: rgbOf(c.raised),
        radius: "8px",
      });
      expect(await look(dialog.locator(".new-line"))).toMatchObject({
        background: rgbOf(c.tint),
        colour: rgbOf(c.active),
      });
      expect(await look(dialog.locator(".label-field"))).toMatchObject({
        size: "13px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      const input = dialog.locator("input.label-input");
      await expect(input).toBeFocused();
      await expectBox(input, { height: 36 });
      // The focused border fades in over --fs-motion-duration.
      await expect
        .poll(() => look(input))
        .toMatchObject({
          size: "14px",
          radius: "6px",
          borderWidth: "2px",
          borderColour: rgbOf(c.focus),
        });
      const cancel = dialog.locator("button.cancel");
      const restore = dialog.locator("button.restore");
      await expectBox(cancel, { height: 38 });
      await expectBox(restore, { height: 38 });
      expect(await look(cancel)).toMatchObject({
        size: "14px",
        weight: "600",
        background: rgbOf(c.card),
        borderColour: rgbOf(c.strong),
        radius: "7px",
      });
      expect(await look(restore)).toMatchObject({
        size: "14px",
        weight: "600",
        background: rgbOf(c.primary),
        colour: rgbOf(c.onPrimary),
        radius: "7px",
      });
      await expect(title).toHaveText(fill("versionsRestoreTitle", { version: "v5" }));
      await expect(dialog.locator(".file-name")).toHaveText("harbour-masterplan-v7.pdf");
      await expect(dialog.locator(".source-line .line-text")).toHaveText(
        fill("versionsRestoreSource", {
          label: "Client review",
          date: "Oct 9, 2026",
          size: "17.9 MB",
        }),
      );
      await expect(dialog.locator(".new-line .line-text")).toHaveText(
        fill("versionsRestoreNew", { version: "v5" }),
      );
      await expect(dialog.locator(".label-field")).toHaveText(
        fill("versionsRestoreLabel", { version: "v8" }),
      );
      await expect(dialog.locator(".keep")).toHaveText(
        fill("versionsRestoreKeepTwo", { first: "v6", second: "v7" }),
      );
      await expect(cancel).toHaveText(label(labels, "fileCancel"));
      await expect(restore).toHaveText(label(labels, "versionsRestore"));

      await input.fill("Back to client review version");
      await restore.click();
      await expect(dialog).toHaveCount(0);
      expect(api.restores).toEqual([
        {
          library_id: "lib",
          object_id: PANEL_OBJECT,
          version: "v5",
          paths: [HARBOUR.path],
          label: "Back to client review version",
        },
      ]);
      expect(api.revisions).toEqual(["v7"]);
      await expect(view.locator(".announce")).toHaveText(
        fill("fileRestored", { name: "harbour-masterplan-v7.pdf", version: "v8" }),
      );
      const first = view.locator("li.version").first();
      await expect(first.locator(".mono-chip")).toHaveText("v8");
      await expect(first.locator(".label")).toHaveText("Back to client review version");
      await expect(first.locator(".current")).toBeVisible();
      await expect(view.locator(".back-row .count")).toHaveText(
        fill("versionsCount", { count: "8" }),
      );
      expect(errors).toEqual([]);
    });

    test("panel states (25): loading, no match and error keep the header, tabs and filter", async ({
      page,
    }) => {
      // Only the deliberately refused details read is excused.
      const errors = trackErrors(
        page,
        (text, url) => /status of 503/.test(text) && /\/api\/v1\/files\/details\?/.test(url),
      );
      await page.clock.setFixedTime(NOW);
      const api = await servePanel(page, FACADE);
      const release = api.holdDetails();
      await page.goto(PANEL_ROUTE);
      await page.getByRole("button", { name: "facade-study.jpg" }).click();
      const details = page.locator("fs-details-panel");
      await expect(details).toHaveAttribute("data-state", "loading");
      await expect(details.locator(".count")).toHaveText(label(labels, "filePanelInfoLoading"));
      expect(await look(details.locator(".count"))).toMatchObject({
        size: "11px",
        weight: "500",
        transform: "none",
        colour: rgbOf(c.muted),
      });
      await expect(details.locator(".placeholder")).toHaveCount(10);
      await expect(details.locator("input.filter")).toBeDisabled();
      expect(await look(details.locator(".bar").first())).toMatchObject({
        background: rgbOf(c.outline),
      });
      await expect(page.locator("fs-file-panel h2.name")).toHaveText("facade-study.jpg");
      release();
      await expect(details).toHaveAttribute("data-state", "ready");

      await details.locator("input.filter").fill("copyright");
      await expect(details).toHaveAttribute("data-state", "no-match");
      const state = details.locator(".state");
      await expect(state.locator(".title")).toHaveText(
        fill("filePanelInfoNoMatch", { word: "copyright" }),
      );
      await expect(state.locator(".detail")).toHaveText(label(labels, "filePanelNoMatchHint"));
      expect(await look(state.locator(".title"))).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      expect(await look(state.locator(".detail"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      const clear = state.locator("button.clear");
      await expect(clear).toHaveText(label(labels, "filePanelClearFilter"));
      await expectBox(clear, { height: 36 });
      await clear.click();
      await expect(details.locator(".row")).toHaveCount(8);
      await expect(details.locator("input.filter")).toBeFocused();

      api.failDetails(503, "unavailable");
      await page.locator("fs-file-panel button.close").click();
      await expect(page.locator("fs-file-panel")).toHaveCount(0);
      await page.getByRole("button", { name: label(labels, "fileNext") }).click();
      await expect(details).toHaveAttribute("data-state", "error");
      const alert = details.locator('.state[role="alert"]');
      await expect(alert.locator(".title")).toHaveText(label(labels, "filePanelInfoError"));
      await expect(alert.locator(".detail")).toHaveText([
        label(labels, "stateUnavailable"),
        label(labels, "filePanelNothingChanged"),
      ]);
      await expect(alert.locator(".code")).toHaveText("unavailable");
      expect((await look(alert.locator(".code"))).family).toMatch(/Mono|monospace/);
      expect(await alert.locator("svg.alerting").evaluate((el) => getComputedStyle(el).color)).toBe(
        rgbOf(c.error),
      );
      await expect(alert.locator("button.retry")).toHaveText(label(labels, "stateRetry"));
      await expect(page.locator("fs-file-panel input, fs-details-panel input.filter")).toHaveCount(
        1,
      );
      expect(errors).toEqual([]);
    });

    test("keyboard: the tab, the history button and the dialog keep focus where it belongs", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await openVersions(page);
      const view = page.locator("fs-versions-panel");
      await view.locator('li[data-version="v2"] button.more').click();
      await page.keyboard.press("Enter");
      const dialog = view.locator('[role="dialog"]');
      await expect(dialog).toBeVisible();
      await page.keyboard.press("Escape");
      await expect(dialog).toHaveCount(0);
      await expect(view.locator('li[data-version="v2"] button.more')).toBeFocused();
      await view.locator("button.back").click();
      await expect(page.locator("fs-file-panel button.history")).toBeFocused();
      await expect(page.locator("fs-file-panel button.history")).toHaveAttribute(
        "aria-pressed",
        "false",
      );
      const tab = page.locator('fs-file-panel [role="tab"]');
      await expect(tab).toHaveAttribute("tabindex", "0");
      await page.keyboard.press("Tab");
      await page.keyboard.press("Tab");
      await expect(tab).toBeFocused();
      const outline = await tab.evaluate((el) => {
        const css = getComputedStyle(el);
        return { style: css.outlineStyle, width: css.outlineWidth, colour: css.outlineColor };
      });
      expect(outline).toEqual({ style: "solid", width: "2px", colour: rgbOf(c.focus) });
      expect(errors).toEqual([]);
    });
  });
}
