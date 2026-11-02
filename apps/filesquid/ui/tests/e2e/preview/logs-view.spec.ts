/**
 * The Logs view and its download dialog against the production build (`vite build` plus `vite
 * preview`) at the mockups' 1440 x 900 window, in light and dark (Task UI-F2.16; UI-S-021,
 * UI-S-024, UI-S-032; UI-S-013): the view of mockup 38 (head, filter row, table, rows, selected
 * row, Load older, footer, the 360 px detail panel) and the dialog of mockup 39, and the
 * behaviour: the sidebar item below Settings, paging, every filter, search by operation and
 * object ID, opening filtered from View in Logs, the detail and its PROV-O links, a live entry
 * arriving, the JSON Lines and CSV downloads, and refusal for a non-owner. Each test asserts the
 * custom elements are defined, the strings come from the active label table, the regions' boxes
 * match the mockup within 2 px (a mockup y of `n` is `n - 28` here, below its title bar), the
 * constants are the browser's computed styles, and there is no console error. The routes are
 * faked from the mockup's data; the clock is fixed at 13:10 on 19 October 2026 in UTC+2, the zone
 * of the mockup's footer.
 */
import { expect, test, type Locator, type Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
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
import { serveLogs, type LogsApi } from "../support/logs-api";
import {
  IMG_FILE,
  IMG_OPERATION,
  LIBRARY,
  NEWEST_ID,
  PLAN_FILE,
  PLAN_OPERATION,
  mockupEntries,
  olderEntries,
} from "../../support/logs-fakes";

const labels = activeLabels();
const SCHEMES: readonly Scheme[] = ["light", "dark"];
const NOW = new Date("2026-10-19T11:10:00Z");

// A 2x (Retina) display, as the macOS desktop target has, in the mockup's zone (UTC+2).
test.use({
  viewport: { width: 1440, height: 900 },
  deviceScaleFactor: 2,
  timezoneId: "Europe/Berlin",
});

/** Fill the placeholders of a label. */
function fill(key: string, values: Record<string, string>): string {
  return label(labels, key).replace(/\{(\w+)\}/g, (whole, name: string) => values[name] ?? whole);
}

/** Open the Logs view on the fixture and wait for its rows. */
async function open(
  page: Page,
  setup?: (api: LogsApi) => void,
  hash = "#/logs",
  entries = mockupEntries(),
): Promise<LogsApi> {
  await page.clock.setFixedTime(NOW);
  const api = await serveLogs(page, entries);
  setup?.(api);
  await page.goto(`/${hash}`);
  await expect(page.locator("fs-logs")).toHaveAttribute("data-state", /ready|empty|denied|error/);
  return api;
}

/** The queries of the list itself (not the one-entry read of the oldest entry). */
const pages = (api: LogsApi) => api.pageQueries.filter((q) => q.limit === 50);

/** The row of mockup 38's selected entry (metadata.set on the plan). */
const SELECTED = NEWEST_ID - 5;

/** The row of an entry id. */
const row = (page: Page, id: number): Locator => page.locator(`fs-logs tr[data-key="${id}"]`);

/** Select an entry and wait for its detail. */
async function select(page: Page, id: number): Promise<void> {
  await row(page, id).locator("button.select").click();
  await expect(page.locator("fs-log-entry")).toBeVisible();
}

/** Press a chip and choose a menu item by its text. */
async function choose(page: Page, chip: string, item: string): Promise<void> {
  await page.locator(`fs-logs button[data-chip="${chip}"]`).click();
  await page.locator("fs-logs .menu-item", { hasText: item }).first().click();
}

/** The block-end border of an element: width and colour. */
function bottomEdge(locator: Locator): Promise<{ width: string; colour: string }> {
  return locator.evaluate((el) => {
    const css = getComputedStyle(el);
    return { width: css.borderBottomWidth, colour: css.borderBottomColor };
  });
}

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => setScheme(page, scheme));

    test("Logs (38): head, filter row and chips match the mockup", async ({ page }) => {
      const errors = trackErrors(page);
      const api = await open(page, (a) => (a.forceMore = true));
      await select(page, SELECTED);
      await expectElementsDefined(page, ["fs-app-shell", "fs-logs", "fs-log-entry"]);
      api.emit({ ...mockupEntries()[0], id: 300 } as never);
      api.emit({ ...mockupEntries()[0], id: 301 } as never);
      const title = page.locator("fs-logs h2");
      await expect(title).toHaveText(label(labels, "logs"));
      await expectBox(title, { x: 264 });
      expect(await look(title)).toMatchObject({
        size: "30px",
        weight: "700",
        colour: rgbOf(c.heading),
      });
      expect((await look(title)).family).toContain("Georgia");
      const intro = page.locator("fs-logs .intro");
      await expect(intro).toHaveText(label(labels, "logsIntro"));
      expect(await look(intro)).toMatchObject({
        size: "13px",
        weight: "400",
        colour: rgbOf(c.variant),
      });
      const live = page.locator("fs-logs .chip.live");
      await expect(live).toHaveText(fill("logsLiveNew", { count: "2" }));
      await expectBox(live, { y: 96, height: 26 });
      const liveBox = await live.boundingBox();
      const downloadBox = await page.locator("fs-logs .download").boundingBox();
      // 12 px between the chip and Download…, which ends where the content does (x 1056).
      expect(
        Math.abs((liveBox?.x ?? 0) + (liveBox?.width ?? 0) + 12 - (downloadBox?.x ?? 0)),
      ).toBeLessThanOrEqual(2);
      expect(
        Math.abs((downloadBox?.x ?? 0) + (downloadBox?.width ?? 0) - 1056),
      ).toBeLessThanOrEqual(2);
      expect(await look(live)).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.active),
        background: rgbOf(c.tint),
        radius: "4px",
      });
      const dot = page.locator("fs-logs .chip.live .dot");
      await expectBox(dot, { width: 7, height: 7 });
      expect((await look(dot)).background).toBe(rgbOf(c.primary));
      const download = page.locator("fs-logs .download");
      await expect(download).toHaveText(label(labels, "logsDownload"));
      await expectBox(download, { y: 93, height: 32 });
      await expect(download.locator("svg")).toHaveCount(1);
      expect(await look(download)).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.body),
        background: rgbOf(c.card),
        radius: "7px",
        borderColour: rgbOf(c.strong),
      });
      expect((await look(download)).borderWidth).toBe(await renderedBorder(page, "1.5px"));
      expect(errors).toEqual([]);
    });

    test("Logs (38): the search field and the five chips", async ({ page }) => {
      const errors = trackErrors(page);
      await open(page);
      const search = page.locator("fs-logs .search input");
      await expect(search).toHaveAttribute("placeholder", label(labels, "logsSearchPlaceholder"));
      await expectBox(page.locator("fs-logs .search"), { x: 264, y: 142, width: 206, height: 32 });
      expect(await look(search)).toMatchObject({
        size: "13px",
        radius: "6px",
        borderColour: rgbOf(c.control),
        colour: rgbOf(c.body),
      });
      expect((await look(search)).borderWidth).toBe(await renderedBorder(page, "1.5px"));
      const chips = [
        ["range", "Last 24 hours"],
        ["library", "Library: all"],
        ["operation", "Operation: all"],
        ["outcome", "Outcome: all"],
        ["level", "Level: all"],
      ] as const;
      // The chips start 12 px after the field (x 482) and stand 6 px apart; their widths follow
      // the text, which this machine's sans face sets a little narrower than the mockup's.
      let edge = 482;
      for (const [id, text] of chips) {
        const chip = page.locator(`fs-logs button[data-chip="${id}"]`);
        await expect(chip).toHaveText(text);
        await expectBox(chip, { x: edge, y: 145, height: 26 });
        const box = await chip.boundingBox();
        edge = (box?.x ?? 0) + (box?.width ?? 0) + 6;
        expect(await look(chip)).toMatchObject({
          size: "13px",
          weight: "500",
          colour: rgbOf(c.body),
          radius: "4px",
          borderWidth: "1px",
          borderColour: rgbOf(c.strong),
        });
      }
      expect(errors).toEqual([]);
    });

    test("Logs (38): the table, its rows, outcome chips, journal-only tags and the selected row", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await open(page, (a) => (a.forceMore = true));
      await select(page, SELECTED);
      const heads = page.locator("fs-logs th");
      await expect(heads).toHaveText([
        label(labels, "logsColTime"),
        label(labels, "logsColLevel"),
        label(labels, "logsColOperation"),
        label(labels, "logsColOutcome"),
        label(labels, "logsColTarget"),
        label(labels, "logsColOperationId"),
      ]);
      expect(await look(heads.first())).toMatchObject({
        size: "13px",
        weight: "600",
        colour: rgbOf(c.variant),
      });
      const xs = [264, 344, 404, 548, 636, 880];
      for (const [i, x] of xs.entries()) {
        const box = await heads.nth(i).boundingBox();
        expect(Math.abs((box?.x ?? 0) + 8 - x), `head ${i} text x`).toBeLessThanOrEqual(2);
      }
      const rows = page.locator("fs-logs tbody tr[data-key]");
      await expect(rows).toHaveCount(16);
      await expectBox(rows.first(), { x: 256, y: 221, width: 808, height: 34 });
      await expectBox(rows.nth(5), { y: 391, height: 34 });
      await expectBox(heads.first(), { x: 256, width: 80 });
      expect(await bottomEdge(rows.first().locator("td").first())).toEqual({
        width: "1px",
        colour: rgbOf(c.outline),
      });
      const first = rows.first();
      expect(await look(first.locator("td").nth(0))).toMatchObject({
        size: "12px",
        colour: rgbOf(c.variant),
      });
      expect((await look(first.locator("td").nth(0))).family).toMatch(
        /mono|Menlo|Consolas|Courier/i,
      );
      expect(await look(first.locator("td").nth(1))).toMatchObject({
        size: "12px",
        weight: "600",
        colour: rgbOf(c.variant),
      });
      expect(await look(first.locator("td").nth(2))).toMatchObject({
        size: "12px",
        weight: "400",
        colour: rgbOf(c.body),
      });
      expect(await look(first.locator("td").nth(4))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.body),
      });
      expect(await look(first.locator("td").nth(5))).toMatchObject({
        size: "12px",
        colour: rgbOf(c.muted),
      });
      const ok = first.locator(".chip");
      await expectBox(ok, { x: 548, width: 28.4, height: 18 });
      expect(await look(ok)).toMatchObject({
        size: "11px",
        weight: "500",
        colour: rgbOf(c.active),
        background: rgbOf(c.tint),
        radius: "4px",
      });
      const warn = row(page, NEWEST_ID - 2)
        .locator("td")
        .nth(1);
      expect((await look(warn)).colour).toBe(rgbOf(c.error));
      const failed = row(page, NEWEST_ID - 3);
      const failedChip = failed.locator(".chip");
      await expectBox(failedChip, { x: 548, height: 18 });
      expect(await look(failedChip)).toMatchObject({
        colour: rgbOf(c.error),
        borderWidth: "1px",
        borderColour: rgbOf(c.error),
        background: "rgba(0, 0, 0, 0)",
      });
      const tag = failed.locator(".journal-only");
      await expect(tag).toHaveText(label(labels, "logsJournalOnly"));
      expect(await look(tag)).toMatchObject({
        size: "11px",
        weight: "500",
        colour: rgbOf(c.muted),
      });
      await expect(tag).toHaveCSS("font-style", "italic");
      await expect(row(page, NEWEST_ID - 6).locator(".chip")).toHaveText(
        label(labels, "logsOutcomeRefused"),
      );
      await expect(
        row(page, NEWEST_ID - 2)
          .locator("td")
          .nth(3),
      ).toHaveText(label(labels, "logsNone"));
      const selected = row(page, SELECTED);
      await expect(selected).toHaveAttribute("aria-selected", "true");
      expect((await look(selected.locator("td").first())).background).toBe(rgbOf(c.tint));
      const bar = await selected
        .locator("td")
        .first()
        .evaluate((el) => {
          const css = getComputedStyle(el, "::before");
          return { width: css.width, background: css.backgroundColor };
        });
      expect(bar).toEqual({ width: "2.5px", background: rgbOf(c.primary) });
      expect(errors).toEqual([]);
    });

    test("Logs (38): the line under the rows, Load older and the footer", async ({ page }) => {
      const errors = trackErrors(page);
      await open(page, (a) => (a.forceMore = true));
      await select(page, SELECTED);
      const older = page.locator("fs-logs button.older");
      await expect(older).toHaveText(label(labels, "logsLoadOlder"));
      await expectBox(older, { y: 776, height: 28 });
      const olderBox = await older.boundingBox();
      expect(Math.abs((olderBox?.x ?? 0) + (olderBox?.width ?? 0) - 1056)).toBeLessThanOrEqual(2);
      expect(await look(older)).toMatchObject({
        size: "14px",
        weight: "600",
        radius: "7px",
        borderColour: rgbOf(c.strong),
        colour: rgbOf(c.body),
      });
      const showing = page.locator("fs-logs .showing");
      await expect(showing).toHaveText(
        fill("logsShowingMore", { shown: "16", range: label(labels, "logsRangePhraseDay") }),
      );
      expect(await look(showing)).toMatchObject({ size: "13px", colour: rgbOf(c.muted) });
      const footer = page.locator("fs-app-shell footer");
      // The footer is held at the window's foot, so it keeps the mockup's y of 860.
      await expectBox(footer, { x: 240, y: 860, width: 840, height: 40 });
      await expect(page.locator('fs-app-shell > [slot="summary"]')).toHaveText(
        fill("logsFooterZone", { zone: "UTC+2" }),
      );
      await expect(page.locator('fs-app-shell > [slot="commit-state"]')).toHaveText(
        fill("logsEntryCountOther", { count: "16" }),
      );
      expect(await look(page.locator("fs-app-shell footer p.summary"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      expect(errors).toEqual([]);
    });

    test("Logs (38): the 360 px detail panel and its rows", async ({ page }) => {
      const errors = trackErrors(page);
      await open(page);
      await select(page, SELECTED);
      await expectElementsDefined(page, ["fs-log-entry"]);
      const aside = page.locator("fs-app-shell aside.panel");
      await expectBox(aside, { x: 1080, width: 360 });
      const panel = page.locator("fs-log-entry");
      await expect(panel.locator(".head-top h2")).toHaveText(label(labels, "logsDetailTitle"));
      expect(await look(panel.locator(".head-top h2"))).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.muted),
      });
      const operation = panel.locator(".operation");
      await expect(operation).toHaveText("metadata.set");
      expect(await look(operation)).toMatchObject({
        size: "17px",
        weight: "600",
        colour: rgbOf(c.heading),
      });
      expect((await look(operation)).family).toMatch(/mono|Menlo|Consolas|Courier/i);
      const chip = panel.locator(".title .chip");
      await expect(chip).toHaveText(label(labels, "logsOutcomeOk"));
      expect(await look(chip)).toMatchObject({
        background: rgbOf(c.tint),
        colour: rgbOf(c.active),
      });
      const keys = [
        ["time-utc", "logsDetailTimeUtc", "2026-10-19T11:03:02.418Z"],
        ["time-local", "logsDetailTimeLocal", "Oct 19, 2026, 13:03:02 (UTC+2)"],
        ["level", "logsDetailLevel", "info"],
        ["outcome", "logsDetailOutcome", "ok"],
        ["software", "logsDetailSoftware", "desktop"],
        ["library", "logsDetailLibrary", "Family archive"],
        ["object", "logsDetailObject", `urn:uuid:${PLAN_FILE} → v8`],
        ["operation-id", "logsDetailOperationId", PLAN_OPERATION],
        ["duration", "logsDetailDuration", "38 ms"],
      ] as const;
      for (const [key, name, value] of keys) {
        const r = panel.locator(`.row[data-key="${key}"]`);
        await expect(r.locator("dt")).toHaveText(label(labels, name));
        await expect(r.locator(".value")).toHaveText(value);
        expect(await look(r.locator("dt"))).toMatchObject({ size: "13px", colour: rgbOf(c.muted) });
        await expectBox(r, { x: 1080, width: 360, height: 42 });
      }
      const actor = panel.locator('.row[data-key="actor"]');
      await expect(actor.locator(".note")).toHaveText(label(labels, "logsActorOwner"));
      const file = panel.locator('.row[data-key="file"]');
      await expect(file.locator(".value")).toHaveText("harbour-masterplan-v7.pdf");
      await expect(file.locator(".note")).toHaveText(label(labels, "logsLookedUp"));
      expect(await look(file.locator(".note"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.muted),
      });
      const provenance = panel.locator("a.view-provenance");
      await expect(provenance).toHaveText(label(labels, "logsViewProvenance"));
      expect(await look(provenance)).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.active),
      });
      await expect(panel.locator("a.show-file")).toHaveText(label(labels, "logsShowFile"));
      const copy = panel.locator("button.copy-json");
      await expect(copy).toHaveText(label(labels, "logsCopyJson"));
      // The rows stand 42 px apart as the mockup's do; the file row wraps its "looked up" note on this
      // machine's wider face, so the controls below sit up to 2 lines lower than the mockup's.
      await expectBox(copy, { x: 1100, height: 28 });
      const copyBox = await copy.boundingBox();
      expect(copyBox?.y ?? 0).toBeGreaterThanOrEqual(669);
      expect(copyBox?.y ?? 0).toBeLessThanOrEqual(711);
      const note = panel.locator(".note-box.raised");
      await expectBox(note, { x: 1100, width: 320 });
      expect(await look(note)).toMatchObject({ background: rgbOf(c.raised), radius: "8px" });
      await expect(note).toContainText(label(labels, "logsOpaqueNote"));
      expect(errors).toEqual([]);
    });

    test("Download logs (39): the dialog, scope, formats, note, file name and buttons", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await open(page);
      await page.locator("fs-logs .download").click();
      const dialog = page.locator("fs-logs .dialog.logs-export");
      await expect(dialog).toBeVisible();
      await expectBox(dialog, { x: 440, y: 82, width: 560, height: 632 });
      expect(await look(dialog)).toMatchObject({
        background: rgbOf(c.overlay),
        radius: "10px",
        borderWidth: "1px",
        borderColour: rgbOf(c.strong),
      });
      const scrim = await page.locator("fs-logs .scrim").evaluate((el) => {
        const css = getComputedStyle(el, "::before");
        return { opacity: css.opacity, background: css.backgroundColor };
      });
      expect(scrim.opacity).toBe("0.32");
      const title = dialog.locator(".dialog-title");
      await expect(title).toHaveText(label(labels, "logsExportTitle"));
      expect(await look(title)).toMatchObject({
        size: "22px",
        weight: "700",
        colour: rgbOf(c.heading),
      });
      const sub = dialog.locator(".sub");
      await expect(sub).toHaveText(label(labels, "logsExportSub"));
      expect(await look(sub)).toMatchObject({ size: "13px", colour: rgbOf(c.variant) });
      await expect(dialog.locator("fieldset.scope legend")).toHaveText(
        label(labels, "logsExportScope"),
      );
      const names = dialog.locator(".scope-option .name");
      await expect(names).toHaveText([
        label(labels, "logsExportScopeFilter"),
        label(labels, "logsExportScopeRange"),
        label(labels, "logsExportScopeAll"),
      ]);
      expect(await look(names.first())).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.body),
      });
      await expect(dialog.locator(".scope-option .line").first()).toHaveText(
        "Last 24 hours · all libraries · all operations · 16 entries",
      );
      expect(await look(dialog.locator(".scope-option .line").first())).toMatchObject({
        size: "13px",
        colour: rgbOf(c.muted),
      });
      const radio = dialog.locator('input[name="scope"]').first();
      await expectBox(radio, { x: 469, height: 16, width: 16 });
      expect((await look(radio)).borderColour).toBe(rgbOf(c.primary));
      const cards = dialog.locator(".card");
      await expectBox(cards.nth(0), { x: 468, y: 382, width: 248, height: 58 });
      await expectBox(cards.nth(1), { x: 724, y: 382, width: 248, height: 58 });
      expect(await look(cards.nth(0))).toMatchObject({
        background: rgbOf(c.tint),
        borderColour: rgbOf(c.primary),
        radius: "8px",
      });
      expect(await look(cards.nth(1))).toMatchObject({
        borderColour: rgbOf(c.strong),
        radius: "8px",
      });
      expect((await look(cards.nth(0))).borderWidth).toBe(await renderedBorder(page, "1.5px"));
      await expect(cards.nth(0).locator(".name")).toHaveText(label(labels, "logsFormatJsonl"));
      await expect(cards.nth(0).locator(".hint")).toHaveText(label(labels, "logsFormatJsonlHint"));
      await expect(cards.nth(1).locator(".name")).toHaveText(label(labels, "logsFormatCsv"));
      await expect(cards.nth(1).locator(".hint")).toHaveText(label(labels, "logsFormatCsvHint"));
      const note = dialog.locator(".note-box.raised");
      await expectBox(note, { x: 468, y: 460, width: 504, height: 56 });
      expect((await look(note)).background).toBe(rgbOf(c.raised));
      await expect(note).toContainText(label(labels, "logsExportNote"));
      const name = dialog.locator("#logs-export-name");
      await expect(name).toHaveValue("filesquid-logs-2026-10-19.jsonl");
      await expectBox(name, { x: 468, y: 564, width: 392, height: 36 });
      expect((await look(name)).family).toMatch(/mono|Menlo|Consolas|Courier/i);
      await expect(dialog.locator("label[for='logs-export-name']")).toHaveText(
        label(labels, "logsExportFileName"),
      );
      const choose = dialog.locator("button.choose");
      await expect(choose).toHaveText(label(labels, "logsExportChoose"));
      await expectBox(choose, { x: 872, y: 568, width: 100, height: 28 });
      // This browser has the platform save dialog, so Choose… is offered (the unit tests cover a
      // browser without it).
      await expect(dialog.locator(".where")).toHaveText(label(labels, "logsExportSavedTo"));
      const cancel = dialog.locator("button.cancel");
      const submit = dialog.locator("button.submit");
      await expectBox(cancel, { x: 780, y: 660, width: 90, height: 38 });
      await expectBox(submit, { x: 882, y: 660, width: 90, height: 38 });
      expect(await look(submit)).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.onPrimary),
        background: rgbOf(c.primary),
        radius: "7px",
      });
      expect(await look(cancel)).toMatchObject({
        background: rgbOf(c.card),
        colour: rgbOf(c.body),
      });
      await name.focus();
      // The border colour eases to the focus ring (--fs-motion-duration), so it is polled.
      await expect.poll(async () => (await look(name)).borderColour).toBe(rgbOf(c.focus));
      expect((await look(name)).borderWidth).toBe("2px");
      expect(errors).toEqual([]);
    });
  });
}

test.describe("behaviour", () => {
  test("the sidebar shows Logs directly below Settings, current on the Logs view", async ({
    page,
  }) => {
    const errors = trackErrors(page);
    await open(page);
    const links = page.locator("fs-app-shell nav ul.tools a");
    await expect(links).toHaveText([
      label(labels, "libraryNavLink"),
      label(labels, "settings"),
      label(labels, "logs"),
    ]);
    const settings = await links.nth(1).boundingBox();
    const logs = await links.nth(2).boundingBox();
    expect((logs?.y ?? 0) - ((settings?.y ?? 0) + (settings?.height ?? 0))).toBeLessThanOrEqual(2);
    await expect(links.nth(2)).toHaveAttribute("aria-current", "page");
    await expect(links.nth(1)).not.toHaveAttribute("aria-current");
    expect(errors).toEqual([]);
  });

  test("lists the journal newest first and says how many are shown", async ({ page }) => {
    const errors = trackErrors(page);
    const api = await open(page);
    const times = await page.locator("fs-logs tbody tr[data-key] td:first-child").allTextContents();
    expect(times[0]).toBe("13:05:42");
    expect([...times].sort().reverse()).toEqual(times);
    await expect(page.locator("fs-logs .showing")).toHaveText(
      fill("logsShowing", { shown: "16", total: "16", range: label(labels, "logsRangePhraseDay") }),
    );
    expect(pages(api)[0]).toMatchObject({ order: "desc", names: true, limit: 50 });
    expect(errors).toEqual([]);
  });

  test("pages: Load older adds the next page, with the cursor, until the end", async ({ page }) => {
    const errors = trackErrors(page);
    const api = await open(page, undefined, "#/logs", [...mockupEntries(), ...olderEntries(60)]);
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(50);
    await expect(page.locator("fs-logs .showing")).toHaveText(
      fill("logsShowingMore", { shown: "50", range: label(labels, "logsRangePhraseDay") }),
    );
    await page.locator("fs-logs button.older").click();
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(76);
    expect(pages(api)[1]?.after).toBe(NEWEST_ID - 49);
    await expect(page.locator("fs-logs button.older")).toBeHidden();
    expect(errors).toEqual([]);
  });

  test("every filter narrows the list: range, library, operation, outcome and level", async ({
    page,
  }) => {
    const errors = trackErrors(page);
    const api = await open(page);
    await choose(page, "range", label(labels, "logsRangeHour"));
    await expect(page.locator('fs-logs button[data-chip="range"]')).toHaveText(
      label(labels, "logsRangeHour"),
    );
    expect(pages(api).at(-1)?.from).toBe("2026-10-19T10:10:00.000Z");
    await choose(page, "range", label(labels, "logsRangeDay"));
    await choose(page, "library", "Family archive");
    expect(pages(api).at(-1)?.library).toBe(LIBRARY);
    await expect(page.locator('fs-logs button[data-chip="library"]')).toHaveText(
      "Library: Family archive",
    );
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(14);
    await choose(page, "library", label(labels, "logsMenuAllLibraries"));
    await choose(page, "operation", "metadata.set");
    expect(pages(api).at(-1)?.operation).toBe("metadata.set");
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(1);
    await choose(page, "operation", label(labels, "logsMenuAllOperations"));
    await choose(page, "outcome", label(labels, "logsOutcomeFailed"));
    expect(pages(api).at(-1)?.outcome).toBe("failed");
    // The two failed operations and the refusal, whose outcome is failed too.
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(3);
    await choose(page, "outcome", label(labels, "logsOutcomeRefused"));
    expect(pages(api).at(-1)?.operation).toBe("api.auth.denied");
    await expect(page.locator("fs-logs tbody tr[data-key] .chip")).toHaveText(
      label(labels, "logsOutcomeRefused"),
    );
    await choose(page, "outcome", label(labels, "logsMenuAllOutcomes"));
    await choose(page, "level", label(labels, "logsLevelError"));
    expect(pages(api).at(-1)?.level).toBe("error");
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(2);
    expect(errors).toEqual([]);
  });

  test("searches by operation, by object ID and by operation ID", async ({ page }) => {
    const errors = trackErrors(page);
    const api = await open(page);
    const input = page.locator("fs-logs .search input");
    await input.fill("ocfl.commit");
    await input.press("Enter");
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(2);
    expect(pages(api).at(-1)?.operation).toBe("ocfl.commit");
    await input.fill(`urn:uuid:${PLAN_FILE}@v8`);
    await input.press("Enter");
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(2);
    expect(pages(api).at(-1)?.object).toBe(`urn:uuid:${PLAN_FILE}`);
    await expect(page.locator("fs-logs button.object")).toContainText(`urn:uuid:${PLAN_FILE}`);
    await input.fill(PLAN_OPERATION);
    await input.press("Enter");
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(1);
    expect(pages(api).at(-1)?.operation_id).toBe(PLAN_OPERATION);
    await input.fill("harbour-masterplan-v7.pdf");
    const reads = pages(api).length;
    await input.press("Enter");
    await expect(page.locator("fs-logs .hint.invalid")).toHaveText(label(labels, "logsSearchHint"));
    expect(pages(api)).toHaveLength(reads);
    expect(errors).toEqual([]);
  });

  test("opens filtered from View in Logs: by file and by operation", async ({ page }) => {
    const errors = trackErrors(page);
    const api = await open(page, undefined, `#/logs?file=${PLAN_FILE}`);
    expect(pages(api)[0]?.file).toBe(PLAN_FILE);
    expect(pages(api)[0]?.from).toBeUndefined();
    await expect(page.locator("fs-logs .search input")).toHaveValue(PLAN_FILE);
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(2);
    await expect(page.locator("fs-logs button.file")).toContainText(PLAN_FILE);
    await page.evaluate((id) => (location.hash = `#/logs?operation_id=${id}`), IMG_OPERATION);
    await expect(page.locator("fs-logs .search input")).toHaveValue(IMG_OPERATION);
    expect(pages(api).at(-1)?.operation_id).toBe(IMG_OPERATION);
    expect(pages(api).at(-1)?.file).toBeUndefined();
    await expect(page.locator("fs-app-shell nav ul.tools a").nth(2)).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(IMG_FILE).toBeTruthy();
    expect(errors).toEqual([]);
  });

  test("the detail shows the entry and links View provenance and Show file to the file's panel", async ({
    page,
  }) => {
    const errors = trackErrors(page);
    const api = await open(page);
    await select(page, SELECTED);
    expect(api.entryReads).toEqual([SELECTED]);
    const panel = page.locator("fs-log-entry");
    await expect(panel.locator("a.view-provenance")).toHaveAttribute(
      "href",
      `#/files/${LIBRARY}/urn%3Auuid%3A${PLAN_FILE}/original?open=${PLAN_FILE}&tab=provenance`,
    );
    await expect(panel.locator("a.show-file")).toHaveAttribute(
      "href",
      `#/files/${LIBRARY}/urn%3Auuid%3A${PLAN_FILE}/original?open=${PLAN_FILE}&tab=details`,
    );
    await panel.locator("button.close").click();
    await expect(panel).toHaveCount(0);
    await expect(page.locator("fs-app-shell aside.panel")).toBeHidden();
    expect(errors).toEqual([]);
  });

  test("a failed entry and a refusal are journal-only and offer no View provenance", async ({
    page,
  }) => {
    const errors = trackErrors(page);
    await open(page);
    await select(page, NEWEST_ID - 3);
    const panel = page.locator("fs-log-entry");
    await expect(panel.locator(".title .chip")).toHaveText(label(labels, "logsOutcomeFailed"));
    await expect(panel.locator("a.view-provenance")).toBeHidden();
    await expect(panel.locator(".journal-only")).toHaveText(label(labels, "logsJournalOnlyNote"));
    await select(page, NEWEST_ID - 6);
    await expect(panel.locator(".title .chip")).toHaveText(label(labels, "logsOutcomeRefused"));
    await expect(panel.locator("a.view-provenance")).toBeHidden();
    expect(errors).toEqual([]);
  });

  test("a live entry is counted and added when the chip is chosen", async ({ page }) => {
    const errors = trackErrors(page);
    const api = await open(page);
    await expect(page.locator("fs-logs .chip.live")).toHaveText(label(labels, "logsLive"));
    api.emit({
      ...mockupEntries()[0],
      id: 400,
      operation: "export.zip",
      time: "2026-10-19T11:09:00.000Z",
    } as never);
    await expect(page.locator("fs-logs .chip.live")).toHaveText(
      fill("logsLiveNew", { count: "1" }),
    );
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(16);
    await page.locator("fs-logs .chip.live").click();
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(17);
    await expect(
      page.locator("fs-logs tbody tr[data-key]").first().locator("td").nth(2),
    ).toHaveText("export.zip");
    await expect(page.locator("fs-logs .chip.live")).toHaveText(label(labels, "logsLive"));
    expect(errors).toEqual([]);
  });

  test("downloads the filtered journal as JSON Lines, oldest first, with opaque ids only", async ({
    page,
  }) => {
    const errors = trackErrors(page);
    const api = await open(page);
    await page.locator("fs-logs .download").click();
    const download = page.waitForEvent("download");
    await page.locator("fs-logs button.submit").click();
    const saved = await download;
    expect(saved.suggestedFilename()).toBe("filesquid-logs-2026-10-19.jsonl");
    const path = await saved.path();
    const text = await readFile(path, "utf8");
    const lines = text
      .trim()
      .split("\n")
      .map((l) => JSON.parse(l) as Record<string, unknown>);
    expect(lines).toHaveLength(16);
    expect(lines[0]?.["operation"]).toBe("ocfl.commit");
    expect((lines[0]?.["id"] as number) < (lines[15]?.["id"] as number)).toBe(true);
    expect(text).not.toContain("display_name");
    expect(text).not.toContain("harbour");
    expect(api.exportQueries[0]).toMatchObject({
      format: "jsonl",
      from: "2026-10-18T11:10:00.000Z",
    });
    await expect(page.locator("fs-logs .dialog.logs-export")).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test("downloads a time range as CSV with one header row, though the export took several requests", async ({
    page,
  }) => {
    const errors = trackErrors(page);
    const api = await open(page, (a) => (a.exportChunk = 6));
    await page.locator("fs-logs .download").click();
    const dialog = page.locator("fs-logs .dialog.logs-export");
    await dialog.locator('input[name="scope"][value="range"]').check();
    await dialog.locator('input[name="format"][value="csv"]').check();
    await expect(dialog.locator("#logs-export-name")).toHaveValue("filesquid-logs-2026-10-19.csv");
    const [from, to] = [
      dialog.locator(".range-fields input").nth(0),
      dialog.locator(".range-fields input").nth(1),
    ];
    await from.fill("2026-10-19T00:00");
    await to.fill("2026-10-19T23:59");
    const download = page.waitForEvent("download");
    await dialog.locator("button.submit").click();
    const saved = await download;
    const text = await readFile(await saved.path(), "utf8");
    const lines = text.trim().split("\r\n");
    expect(lines[0]).toBe(
      "id,time,host,actor,operation,outcome,level,target_ids,operation_id,object_ref",
    );
    expect(lines.filter((l) => l.startsWith("id,time"))).toHaveLength(1);
    expect(lines).toHaveLength(17);
    expect(api.exportQueries.length).toBeGreaterThanOrEqual(3);
    expect(api.exportQueries[0]).toMatchObject({
      format: "csv",
      from: "2026-10-18T22:00:00.000Z",
      to: "2026-10-19T21:59:00.000Z",
    });
    expect(errors).toEqual([]);
  });

  test("shows a permission state with no entries for a caller who is not the owner", async ({
    page,
  }) => {
    const errors = trackErrors(page, /status of 40[13] \(/);
    await open(page, (a) => a.deny(403, "forbidden"));
    await expect(page.locator("fs-logs")).toHaveAttribute("data-state", "denied");
    await expect(page.locator("fs-logs .state .title")).toHaveText(
      label(labels, "logsDeniedTitle"),
    );
    await expect(page.locator("fs-logs .state .detail").first()).toHaveText(
      label(labels, "logsDeniedDetail"),
    );
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(0);
    await expect(page.locator("fs-logs .chip.live")).toBeHidden();
    expect(errors).toEqual([]);
  });

  test("shows the empty state for an empty journal", async ({ page }) => {
    const errors = trackErrors(page);
    await open(page, undefined, "#/logs", []);
    await expect(page.locator("fs-logs .state .title")).toHaveText(label(labels, "logsEmptyTitle"));
    await expect(page.locator("fs-logs .state button")).toHaveCount(0);
    expect(errors).toEqual([]);
  });

  test("shows no match with Clear filters, and the list returns when they are cleared", async ({
    page,
  }) => {
    const errors = trackErrors(page);
    await open(page);
    await choose(page, "operation", "app.start");
    await choose(page, "level", label(labels, "logsLevelError"));
    await expect(page.locator("fs-logs .state .title")).toHaveText(
      label(labels, "logsNoMatchTitle"),
    );
    await page.locator("fs-logs .state button").click();
    await expect(page.locator("fs-logs tbody tr[data-key]")).toHaveCount(16);
    expect(errors).toEqual([]);
  });
});
