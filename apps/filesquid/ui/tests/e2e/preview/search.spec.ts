/**
 * Production-artifact proof of the global search bar, the search results and the filter chips
 * (UI-F2.9, F-063, UI-S-007, UI-S-024, UI-S-030): the FileSquid UI built by `vite build` and served
 * by `vite preview`, with the navigation listing, the view operation, the search operation and the
 * preferences answered by the test in the contract's own shape. At 1440 x 900, in light and dark,
 * it asserts the regions of mockups 20, 21 and 53 within 2 px (the mockups draw a 28 px title bar
 * the webview has not, so a mockup y of `n` is `n - 28` here), each constant as a computed style,
 * every word against the active label table, the requests the page sends (query, scope, order,
 * zone, limit) and that the elements mount with no console error.
 */
import { expect, test, type Page } from "@playwright/test";
import { COLOURS, expectBox, look, renderedBorder } from "../support/fidelity";
import { NOW, serveDrive, type DriveApi } from "../support/drive-api";
import { serveNavigation } from "../support/navigation-api";
import {
  ARCHIVE,
  HARBOUR_NODE,
  LIBRARY_NAME,
  LIBRARY_NODE,
  serveSearch,
  serveShell,
  type SearchApi,
} from "../support/search-api";
import {
  activeLabels,
  expectElementsDefined,
  label,
  rgbOf,
  setScheme,
  trackErrors,
} from "../support/strict-dom";

const labels = activeLabels();
const SCHEMES = ["light", "dark"] as const;

/** A label template with its `{placeholders}` filled, so no English word is typed in a test. */
function fill(key: string, values: Record<string, string | number>): string {
  return label(labels, key).replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in values ? String(values[name]) : whole,
  );
}

/** The start of a count chip that lists files with their size: "3 files · ", from the labels. */
function countPrefix(count: number): RegExp {
  const files = fill(count === 1 ? "fileCountOne" : "fileCountOther", { count });
  const text = fill("searchCountSize", { count: files, size: "\u0000" }).split("\u0000")[0] ?? "";
  return new RegExp("^" + text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
}

/** The refusals of the search operation the browser logs as console errors. */
const REFUSED_SEARCH = (text: string, url: string) =>
  /status of (400|503)/.test(text) && /\/api\/v1\/files\/search/.test(url);

/** Serve everything and open `hash` (the Harbour project collection by default, as mockup 20). */
async function open(
  page: Page,
  hash = `#/navigation/${HARBOUR_NODE}`,
): Promise<{ drive: DriveApi; search: SearchApi }> {
  await page.clock.setFixedTime(NOW);
  await serveShell(page);
  await serveNavigation(page, ARCHIVE);
  const drive = await serveDrive(page, undefined, undefined, [LIBRARY_NODE, HARBOUR_NODE]);
  const search = await serveSearch(page);
  await page.goto(`/${hash}`);
  return { drive, search };
}

const bar = (page: Page) => page.locator("fs-search-bar");
const field = (page: Page) => bar(page).locator("input.text");
const results = (page: Page) => page.locator("fs-app-shell > fs-drive-view");

/** Focus the field, apply Type: Document and Modified: last 30 days, type "harbour" (mockup 20). */
async function mockup20(page: Page): Promise<void> {
  await field(page).click();
  await bar(page).locator("button.filter-chip[data-filter=type]").click();
  await bar(page).locator(".suggestion").first().click();
  await bar(page).locator("button.filter-chip[data-filter=modified]").click();
  await bar(page).locator(".suggestion").nth(2).click();
  await field(page).fill("harbour");
  await expect(bar(page).locator(".file-option")).toHaveCount(3);
}

test.use({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => setScheme(page, scheme));

    test("the field (mockup 20): size, border, placeholder and the widening on focus", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await open(page);
      await expectElementsDefined(page, ["fs-search-bar", "fs-drive-view", "fs-app-shell"]);
      const frame = bar(page).locator(".frame");
      await expectBox(frame, { x: 264, y: 12, width: 520, height: 36 });
      await expect(field(page)).toHaveAttribute("placeholder", label(labels, "searchPlaceholder"));
      expect(await look(frame)).toMatchObject({
        borderWidth: await renderedBorder(page, "1.5px"),
        borderColour: rgbOf(c.control),
        radius: "6px",
        background: rgbOf(c.card),
      });
      expect(await look(field(page))).toMatchObject({
        size: "14px",
        weight: "400",
        colour: rgbOf(c.body),
      });
      await field(page).click();
      await expectBox(frame, { x: 264, y: 12, width: 620, height: 36 });
      await expect(frame).toHaveCSS("border-top-color", rgbOf(c.focus));
      expect(await look(frame)).toMatchObject({
        borderWidth: "2px",
        borderColour: rgbOf(c.focus),
        radius: "6px",
      });
      expect(errors).toEqual([]);
    });

    test("the drop-down (mockup 20): regions, sections and constants", async ({ page }) => {
      const errors = trackErrors(page);
      await open(page);
      await mockup20(page);
      const dropdown = bar(page).locator(".dropdown");
      await expectBox(dropdown, { x: 264, y: 54, width: 620, height: 520 });
      expect(await look(dropdown)).toMatchObject({
        background: rgbOf(c.overlay),
        borderWidth: "1px",
        borderColour: rgbOf(c.strong),
        radius: "8px",
      });

      // The applied chips inside the field, before the text, and Clear all at its right.
      const chips = bar(page).locator(".frame .chip");
      await expect(chips.locator(".chip-label")).toHaveText([
        fill("searchChip", {
          filter: label(labels, "searchFilterType"),
          value: label(labels, "searchTypeDocument"),
        }),
        fill("searchChip", {
          filter: label(labels, "searchFilterModified"),
          value: label(labels, "searchModifiedMonth"),
        }),
      ]);
      await expectBox(chips.first(), { x: 300, y: 18, height: 24 });
      expect(await look(chips.first())).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.active),
        background: rgbOf(c.tint),
        radius: "4px",
      });
      const clear = bar(page).locator("button.clear-all");
      await expect(clear).toHaveText(label(labels, "searchClearAll"));
      const clearBox = await clear.boundingBox();
      expect(Math.abs((clearBox?.x ?? 0) + (clearBox?.width ?? 0) - 872)).toBeLessThanOrEqual(2);
      expect(await look(clear)).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.primary),
      });

      // FILTERS, SUGGESTIONS, FILES and OPERATORS with their eyebrows.
      await expect(bar(page).locator("h3.eyebrow")).toHaveText([
        label(labels, "searchFilters"),
        label(labels, "searchSuggestions"),
        label(labels, "searchFiles"),
        label(labels, "searchOperators"),
      ]);
      expect(await look(bar(page).locator("h3.eyebrow").first())).toMatchObject({
        size: "11px",
        weight: "400",
        colour: rgbOf(c.muted),
        transform: "uppercase",
        spacing: "1.1px",
      });
      const filterChips = bar(page).locator("button.filter-chip");
      await expect(filterChips).toHaveText([
        label(labels, "searchFilterType"),
        label(labels, "searchFilterLocation"),
        label(labels, "searchFilterModified"),
      ]);
      await expectBox(filterChips.first(), { x: 280, y: 92, height: 28 });
      expect(await look(filterChips.first())).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.body),
        borderWidth: "1px",
        borderColour: rgbOf(c.strong),
        radius: "4px",
      });

      const suggestions = bar(page).locator(".suggestion");
      await expect(suggestions.locator(".op")).toHaveText(["in:", "type:", "title:"]);
      await expect(suggestions.locator(".value")).toHaveText([
        "Harbour project",
        "document",
        "harbour",
      ]);
      await expect(suggestions.locator(".hint")).toHaveText([
        label(labels, "searchHintFolder"),
        label(labels, "searchHintType"),
        label(labels, "searchHintTitle"),
      ]);
      await expectBox(suggestions.first(), { x: 268, y: 160, width: 612, height: 34 });
      await expectBox(suggestions.nth(2), { y: 228, height: 34 });
      const op = suggestions.first().locator(".op");
      await expectBox(op, { x: 306 });
      await expectBox(suggestions.first().locator(".value"), { x: 334 });
      expect(await look(op)).toMatchObject({
        size: "13px",
        weight: "400",
        colour: rgbOf(c.primary),
      });
      expect((await look(op)).family).toMatch(/mono/i);
      expect(await look(suggestions.first().locator(".value"))).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.body),
      });
      expect(await look(suggestions.first().locator(".hint"))).toMatchObject({
        size: "13px",
        weight: "400",
        colour: rgbOf(c.muted),
      });

      const files = bar(page).locator(".file-option");
      await expectBox(files.first(), { y: 312, height: 44 });
      await expectBox(files.first().locator(".badge"), { x: 280, y: 320, width: 30, height: 16 });
      await expectBox(files.first().locator(".file-name"), { x: 324 });
      expect(await look(files.first().locator(".file-name"))).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.body),
      });
      expect(await look(files.first().locator(".file-meta"))).toMatchObject({
        size: "13px",
        weight: "400",
        colour: rgbOf(c.muted),
      });
      expect(await look(files.first().locator(".badge"))).toMatchObject({
        size: "10px",
        weight: "700",
        radius: "3px",
      });

      const operators = bar(page).locator(".operator-chip");
      await expect(operators).toHaveCount(10);
      await expectBox(operators.first(), { x: 280, y: 482, height: 22 });
      expect(await look(operators.first())).toMatchObject({
        size: "11px",
        weight: "500",
        colour: rgbOf(c.variant),
        background: rgbOf(c.raised),
        radius: "4px",
      });
      expect((await look(operators.first())).family).toMatch(/mono/i);
      const hint = bar(page).locator(".enter-hint");
      await expect(hint).toHaveText(label(labels, "searchEnterHint"));
      await expectBox(hint, { x: 280, y: 548 });
      expect(await look(hint)).toMatchObject({
        size: "13px",
        weight: "400",
        colour: rgbOf(c.muted),
      });
      const divider = await bar(page)
        .locator(".section.files")
        .evaluate((el) => {
          const css = getComputedStyle(el);
          return { width: css.borderTopWidth, colour: css.borderTopColor };
        });
      expect(divider).toEqual({ width: "1px", colour: rgbOf(c.outline) });
      expect(errors).toEqual([]);
    });

    test("the active suggestion row is on the raised surface and named for assistive technology", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await open(page);
      await mockup20(page);
      const firstSuggestion = bar(page).locator(".suggestion").first();
      for (let step = 0; step < 4; step += 1) await field(page).press("ArrowDown");
      await expect(firstSuggestion).toHaveAttribute("aria-selected", "true");
      expect(await look(firstSuggestion)).toMatchObject({ background: rgbOf(c.raised) });
      const id = await firstSuggestion.getAttribute("id");
      await expect(field(page)).toHaveAttribute("aria-activedescendant", id ?? "");
      await expect(field(page)).toHaveAttribute("role", "combobox");
      await expect(field(page)).toHaveAttribute("aria-expanded", "true");
      await field(page).press("Escape");
      await expect(bar(page).locator(".dropdown")).toBeHidden();
      expect(errors).toEqual([]);
    });

    test("Enter shows the results in the Drive list, asked as the chips and text make it", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const { search, drive } = await open(page);
      await mockup20(page);
      await field(page).press("Enter");
      await expect(page).toHaveURL(/#\/search\//);
      await expect(results(page)).toHaveAttribute("data-state", "ready");
      const call = search.calls.at(-1);
      expect(call?.query).toEqual({
        q: "type:document after:2026-09-16 harbour",
        node: LIBRARY_NODE,
        sort: "relevance",
        direction: "descending",
        folders: "on_top",
        zone: "Z",
      });
      expect(call?.limit).toBe(200);

      const view = results(page);
      const title = view.locator(".page-title");
      await expect(title).toHaveText(label(labels, "searchResultsTitle"));
      expect(await look(title)).toMatchObject({
        size: "30px",
        weight: "700",
        colour: rgbOf(c.heading),
      });
      expect((await look(title)).family).toMatch(/^Georgia/);
      await expectBox(title, { x: 264, y: 76, height: 34 });
      const scope = view.locator(".scope-chip");
      await expect(scope).toHaveText(fill("searchScopeIn", { name: LIBRARY_NAME }));
      await expectBox(scope, { x: 264, y: 120, height: 26 });
      expect(await look(scope)).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.variant),
        background: rgbOf(c.raised),
        radius: "5px",
      });
      await expect(view.locator(".count-chip")).toHaveText(countPrefix(3));
      await expect(view.locator(".row .name")).toHaveText([
        "harbour-2025-report.pdf",
        "harbour-masterplan-v7.pdf",
        "harbour-walk-permit.pdf",
      ]);
      await expect(view.locator("[role=columnheader] button.sort")).toHaveText([
        label(labels, "fileColumnName"),
        label(labels, "fileColumnSize"),
        label(labels, "fileColumnModified"),
      ]);
      await expect(view.locator("[aria-sort]")).toHaveCount(0);
      await expect(page.locator('fs-app-shell [slot="summary"]')).toHaveText(
        fill("searchResultCountOther", { count: 3 }),
      );

      // The bar keeps the chips and the text of the address, and stays 620 px wide.
      await expect(bar(page).locator(".frame .chip .chip-label")).toHaveCount(2);
      await expect(field(page)).toHaveValue("harbour");
      await expectBox(bar(page).locator(".frame"), { width: 620, height: 36 });

      // A column sort is this search's own: sent to the API, saved nowhere.
      await view.locator("button.sort[data-sort=size]").click();
      await expect.poll(() => search.calls.at(-1)?.query.sort).toBe("size");
      expect(search.calls.at(-1)?.query.direction).toBe("descending");
      await expect(view.locator("[aria-sort]")).toHaveAttribute("aria-sort", "descending");
      expect(drive.saved).toEqual([]);
      expect(errors).toEqual([]);
    });

    test("an operator the device cannot answer is refused and named (mockup 21)", async ({
      page,
    }) => {
      const errors = trackErrors(page, REFUSED_SEARCH);
      const { search } = await open(page, "#/files");
      await field(page).click();
      await bar(page).locator("button.filter-chip[data-filter=type]").click();
      await bar(page).locator(".suggestion").first().click();
      await field(page).fill("harbour sharedwith:me");
      await field(page).press("Enter");
      const view = results(page);
      await expect(view).toHaveAttribute("data-state", "refused");
      expect(search.calls.at(-1)?.query.q).toBe("type:document harbour sharedwith:me");
      const box = view.locator(".refusal");
      await expect(box).toHaveAttribute("role", "alert");
      await expectBox(box, { x: 256, y: 162, width: 1160, height: 64 });
      expect(await look(box)).toMatchObject({
        borderWidth: "1px",
        borderColour: rgbOf(c.error),
        radius: "8px",
        background: rgbOf(c.card),
      });
      await expect(view.locator(".refusal-title")).toHaveText(
        fill("searchUnavailable", { operator: "sharedwith:" }),
      );
      await expect(view.locator(".refusal-help")).toHaveText(
        fill("searchUnavailableHelp", { operator: "sharedwith:" }),
      );
      expect(await look(view.locator(".refusal-title"))).toMatchObject({
        size: "13px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      expect(await look(view.locator(".refusal-help"))).toMatchObject({
        size: "13px",
        weight: "400",
        colour: rgbOf(c.variant),
      });
      await expectBox(view.locator(".refusal-title"), { x: 298 });
      expect(
        await view.locator(".refusal .icon").evaluate((el) => getComputedStyle(el).color),
      ).toBe(rgbOf(c.error));
      await expectBox(view.locator(".refusal .icon"), { x: 272, y: 176, width: 16, height: 16 });

      await expect(view.locator(".nothing-searched")).toHaveText(
        label(labels, "searchNothingSearched"),
      );
      expect(await look(view.locator(".nothing-searched"))).toMatchObject({
        size: "13px",
        weight: "400",
        colour: rgbOf(c.variant),
      });
      const eyebrow = view.locator(".operators-host .eyebrow");
      await expect(eyebrow).toHaveText(label(labels, "searchOperatorsOnHost"));
      expect(await look(eyebrow)).toMatchObject({ size: "11px", colour: rgbOf(c.muted) });
      const operators = view.locator(".operators-host .operator-chip");
      await expect(operators).toHaveCount(10);
      await expectBox(operators.first(), { x: 264, y: 330, height: 22 });
      expect(await look(operators.first())).toMatchObject({
        size: "13px",
        weight: "400",
        colour: rgbOf(c.body),
        background: rgbOf(c.raised),
        borderWidth: "1px",
        borderColour: rgbOf(c.outline),
        radius: "4px",
      });
      expect((await look(operators.first())).family).toMatch(/mono/i);
      await expect(view.locator(".row")).toHaveCount(0);
      await expect(view.locator(".scope-chip, .count-chip")).toHaveText([
        fill("searchScopeIn", { name: LIBRARY_NAME }),
        fill("fileCountOther", { count: 0 }),
      ]);
      await expect(page.locator('fs-app-shell [slot="summary"]')).toHaveText(
        fill("searchResultCountOther", { count: 0 }),
      );
      expect(errors).toEqual([]);
    });

    test("an operator that is not in the grammar is refused as an invalid request (mockup 53)", async ({
      page,
    }) => {
      const errors = trackErrors(page, REFUSED_SEARCH);
      await open(page, "#/files");
      await field(page).fill("harbour colour:red");
      await field(page).press("Enter");
      const view = results(page);
      await expect(view).toHaveAttribute("data-state", "refused");
      await expect(view.locator(".refusal-title")).toHaveText(
        fill("searchUnknownOperator", { operator: "colour:" }),
      );
      await expect(view.locator(".refusal-help")).toHaveText(
        label(labels, "searchUnknownOperatorHelp"),
      );
      await expectBox(view.locator(".refusal"), { x: 256, y: 162, width: 1160, height: 64 });
      await expect(view.locator(".operators-host .operator-chip")).toHaveCount(10);
      await expect(view.locator(".row")).toHaveCount(0);
      expect(errors).toEqual([]);
    });
  });
}

test("the sidebar filter shows its scope and count as chips and narrows only the current view", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const { drive, search } = await open(page, "#/files");
  await expect(page.locator("fs-app-shell > fs-drive-view")).toHaveAttribute("data-state", "ready");
  await page.locator("fs-app-shell nav input.filter").fill("harbour");
  await expect.poll(() => drive.calls.at(-1)?.filter).toBe("harbour");
  const view = results(page);
  await expect(view.locator(".scope-chip")).toHaveText(
    fill("searchScopeIn", { name: label(labels, "fileRoot") }),
  );
  await expect(view.locator(".count-chip")).toHaveText(countPrefix(1));
  expect(search.calls).toEqual([]);
  await page.locator("fs-app-shell nav input.filter").fill("");
  await expect(view.locator(".scope-chip")).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("Back and Forward bring the search and its chips back, and All files clears the bar", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await open(page, "#/files");
  await field(page).fill("type:image facade");
  await field(page).press("Enter");
  await expect(results(page).locator(".row .name")).toHaveText(["facade-study.jpg"]);
  await page.goBack();
  await expect(page).toHaveURL(/#\/files$/);
  await expect(field(page)).toHaveValue("");
  await expect(bar(page).locator(".frame .chip")).toHaveCount(0);
  await expect(results(page).locator(".page-title")).toHaveText(label(labels, "fileRoot"));
  await page.goForward();
  await expect(results(page).locator(".row .name")).toHaveText(["facade-study.jpg"]);
  await expect(field(page)).toHaveValue("facade");
  await expect(bar(page).locator(".frame .chip .chip-label")).toHaveText([
    fill("searchChip", {
      filter: label(labels, "searchFilterType"),
      value: label(labels, "searchTypeImage"),
    }),
  ]);
  expect(errors).toEqual([]);
});

test("a file chosen in the drop-down opens as a Drive row does", async ({ page }) => {
  const errors = trackErrors(page);
  await open(page, "#/files");
  await field(page).fill("masterplan");
  await bar(page).locator(".file-option").first().click();
  await expect(page).toHaveURL(/#\/files\/lib-1\/urn%3Auuid%3Af1\/original$/);
  expect(errors).toEqual([]);
});

test("Clear all empties the field and runs no search", async ({ page }) => {
  const errors = trackErrors(page);
  const { search } = await open(page, "#/files");
  await field(page).fill("harbour");
  await expect(bar(page).locator(".file-option")).toHaveCount(3);
  const before = search.calls.length;
  await bar(page).locator("button.clear-all").click();
  await expect(field(page)).toHaveValue("");
  await expect(page).not.toHaveURL(/#\/search\//);
  expect(search.calls.length).toBe(before);
  expect(errors).toEqual([]);
});

test("the Location chip limits the search to a collection", async ({ page }) => {
  const errors = trackErrors(page);
  const { search } = await open(page, "#/files");
  await field(page).click();
  await bar(page).locator("button.filter-chip[data-filter=in]").click();
  await bar(page).locator(".suggestion", { hasText: "Harbour project" }).click();
  await field(page).fill("harbour");
  await field(page).press("Enter");
  await expect(results(page).locator(".row .name")).toHaveText([
    "harbour-2025-report.pdf",
    "harbour-masterplan-v7.pdf",
  ]);
  expect(search.calls.at(-1)?.query.q).toBe('in:"Harbour project" harbour');
  expect(errors).toEqual([]);
});

test("the live files wait for a pause in typing and a refusal there is not a console error of ours", async ({
  page,
}) => {
  const errors = trackErrors(page, REFUSED_SEARCH);
  const { search } = await open(page, "#/files");
  await field(page).click();
  await field(page).pressSequentially("harb", { delay: 20 });
  await expect(bar(page).locator(".file-option")).toHaveCount(3);
  expect(search.calls.filter((call) => call.query.q === "harb")).toHaveLength(1);
  expect(search.calls.every((call) => call.limit === 3)).toBe(true);
  await field(page).fill("sharedwith:me");
  await expect(bar(page).locator(".file-option")).toHaveCount(0);
  await expect(bar(page).locator(".suggestion")).not.toHaveCount(0);
  expect(errors).toEqual([]);
});
