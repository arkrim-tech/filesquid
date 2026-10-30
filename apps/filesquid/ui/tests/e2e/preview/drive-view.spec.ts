/**
 * Production-artifact proof of the Drive-style views (UI-F2.8, F-062, UI-S-006, UI-S-024,
 * UI-S-025, UI-S-026, UI-S-033, UI-S-017): the FileSquid UI built by `vite build` and served by
 * `vite preview`, with the navigation listing, the view operation and the preferences answered
 * by the test in the contract's own shape. At 1440 x 900, in light and dark, it asserts the
 * regions of mockups 05, 06, 10, 11, 13, 14, 15, 32, 33 and 56 within 2 px, each constant as a
 * computed style, every word against the active label table, the requests the page sends
 * (sort, folder placement, filter, preferences) and that the elements mount with no console
 * error.
 */
import { expect, test, type Page } from "@playwright/test";
import { COLOURS, expectBox, look, renderedBorder } from "../support/fidelity";
import { LIBRARY_NODE, NOW, ROWS, serveDrive, type DriveApi } from "../support/drive-api";
import { serveNavigation } from "../support/navigation-api";
import {
  activeLabels,
  expectElementsDefined,
  label,
  rgbOf,
  setScheme,
  trackErrors,
} from "../support/strict-dom";

const labels = activeLabels();

/** A label template with its `{placeholders}` filled, so no English word is typed in a test. */
function fill(key: string, values: Record<string, string | number>): string {
  return label(labels, key).replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in values ? String(values[name]) : whole,
  );
}
const SCHEMES = ["light", "dark"] as const;

/** The shell's health, host and event channel, answered so only the build is under test. */
async function serveShell(page: Page): Promise<void> {
  await page.route("**/api/v1/health", (route) =>
    route.fulfill({ json: { schema_version: 1, status: "ok", product: "filesquid" } }),
  );
  await page.route("**/api/v1/host", (route) =>
    route.fulfill({
      json: {
        schema_version: 1,
        host_type: "desktop",
        operator_name: null,
        hosted_service: {
          enrolled: false,
          backup_enabled: false,
          sync_enabled: false,
          service_host: null,
        },
      },
    }),
  );
  await page.routeWebSocket(/\/api\/v1\/events$/, () => undefined);
}

/** Serve everything and open the library's view at `hash`. */
async function open(
  page: Page,
  hash = `#/navigation/${LIBRARY_NODE}`,
  setup?: (api: DriveApi) => void | Promise<void>,
): Promise<DriveApi> {
  await page.clock.setFixedTime(NOW);
  await serveShell(page);
  await serveNavigation(page);
  const api = await serveDrive(page);
  await setup?.(api);
  await page.goto(`/${hash}`);
  return api;
}

/** The drive view inside the navigation view. */
const drive = (page: Page) => page.locator("fs-navigation-view fs-drive-view");

/** A row of the drive view by its name. */
const rowNamed = (page: Page, name: string) =>
  drive(page)
    .locator(".row, .tile")
    .filter({ has: page.locator(`.name:text-is("${name}")`) });

test.use({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => setScheme(page, scheme));

    test("the list (mockup 05, UI-S-026): regions, columns and constants", async ({ page }) => {
      const errors = trackErrors(page);
      await open(page);
      await expectElementsDefined(page, ["fs-drive-view", "fs-navigation-view", "fs-new-menu"]);
      await expect(drive(page)).toHaveAttribute("data-state", "ready");
      const view = drive(page);
      await expectBox(view.locator(".page-title"), { x: 264, y: 100, height: 34 });
      await expectBox(view.locator(".layout"), { x: 1344, width: 72, height: 32 });
      await expectBox(view.locator(".head"), { x: 264, y: 142, width: 1152, height: 38 });
      await expectBox(view.locator(".row").first(), { x: 264, y: 180, width: 1152, height: 44 });
      await expectBox(rowNamed(page, "harbour-masterplan-v7.pdf").locator("input.check"), {
        x: 268.75,
        width: 14.5,
        height: 14.5,
      });
      await expectBox(rowNamed(page, "harbour-masterplan-v7.pdf").locator(".badge"), {
        x: 296,
        width: 30,
        height: 16,
      });
      await expectBox(rowNamed(page, "harbour-masterplan-v7.pdf").locator(".name"), { x: 348 });
      await expectBox(rowNamed(page, "harbour-masterplan-v7.pdf").locator(".modified"), {
        x: 1182,
      });
      await expectBox(rowNamed(page, "harbour-masterplan-v7.pdf").locator(".version"), { x: 1292 });
      const sizeBox = await rowNamed(page, "harbour-masterplan-v7.pdf")
        .locator(".size")
        .boundingBox();
      expect(Math.abs((sizeBox?.x ?? 0) + (sizeBox?.width ?? 0) - 1152)).toBeLessThanOrEqual(2);

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
      await expect(heads.nth(4)).toContainText(label(labels, "fileColumnVersion"));
      expect(await look(heads.nth(1))).toMatchObject({
        size: "13px",
        weight: "600",
        colour: rgbOf(c.body),
      });
      expect(await look(heads.nth(2))).toMatchObject({
        size: "13px",
        weight: "500",
        colour: rgbOf(c.variant),
      });
      expect(await look(view.locator(".head"))).toMatchObject({ borderWidth: "0px" });
      const rule = await view.locator(".head").evaluate((el) => {
        const css = getComputedStyle(el);
        return { width: css.borderBottomWidth, colour: css.borderBottomColor };
      });
      expect(rule).toEqual({ width: "1px", colour: rgbOf(c.strong) });

      const pdf = rowNamed(page, "harbour-masterplan-v7.pdf");
      expect(await look(pdf.locator(".name"))).toMatchObject({
        size: "14px",
        colour: rgbOf(c.body),
      });
      expect(await look(pdf.locator(".size"))).toMatchObject({
        size: "13px",
        colour: rgbOf(c.variant),
      });
      expect(await look(pdf.locator(".badge"))).toMatchObject({
        size: "10px",
        weight: "700",
        background: rgbOf("#fd1e23"),
        colour: rgbOf("#ffffff"),
        radius: "3px",
        transform: "uppercase",
      });
      expect((await look(pdf.locator(".badge"))).family).toMatch(/mono/i);
      expect(await look(rowNamed(page, "budget-2026.xlsx").locator(".badge"))).toMatchObject({
        background: rgbOf("#fcd805"),
        colour: rgbOf("#111111"),
      });
      expect(await look(rowNamed(page, "facade-study.jpg").locator(".badge"))).toMatchObject({
        background: rgbOf("#007af9"),
      });
      expect(await look(pdf.locator("input.check"))).toMatchObject({
        borderWidth: await renderedBorder(page, "1.5px"),
        borderColour: rgbOf(c.control),
        radius: "3.5px",
      });
      await expect(pdf.locator(".modified")).toHaveText("2 hours ago");
      await expect(rowNamed(page, "budget-2026.xlsx").locator(".modified")).toHaveText(
        label(labels, "fileTimeYesterday"),
      );
      await expect(rowNamed(page, "Contracts")).toHaveCount(1);
      await expect(view.locator(".row").first().locator(".name")).toHaveText("Contracts");

      await pdf.hover();
      expect(await look(pdf)).toMatchObject({ background: rgbOf(c.raised) });
      await expectBox(pdf.locator("button.actions"), { x: 1378, width: 30, height: 30 });
      expect(await look(pdf.locator("button.actions"))).toMatchObject({
        radius: "6px",
        borderWidth: "1px",
        borderColour: rgbOf(c.strong),
      });
      await expect(rowNamed(page, "budget-2026.xlsx").locator("button.actions")).toBeHidden();
      expect(errors).toEqual([]);
    });

    test("sorting and folder placement are sent to the API and saved as preferences", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      const api = await open(page);
      await expect(drive(page)).toHaveAttribute("data-state", "ready");
      await drive(page).locator("button.sort[data-sort=modified]").click();
      await expect
        .poll(() => api.calls.at(-1))
        .toMatchObject({ sort: "modified", direction: "descending", folders: "on_top" });
      await expect(drive(page).locator("[aria-sort=descending]")).toContainText(
        label(labels, "fileColumnModified"),
      );
      await expect(drive(page).locator(".row .name").nth(3)).toHaveText(
        "harbour-masterplan-v7.pdf",
      );
      await drive(page).locator("button.sort[data-sort=modified]").click();
      await expect.poll(() => api.calls.at(-1)?.direction).toBe("ascending");
      await expect
        .poll(() => api.saved.at(-1))
        .toMatchObject({ sort_key: "modified", sort_direction: "ascending", layout: "list" });
      expect(await look(drive(page).locator("[aria-sort] .sort"))).toMatchObject({ weight: "600" });
      expect(errors).toEqual([]);
    });

    test("the grid (mockup 06): tiles, Sort button and menu", async ({ page }) => {
      const errors = trackErrors(page);
      const api = await open(page);
      await drive(page).locator("button[data-layout=grid]").click();
      const view = drive(page);
      await expect.poll(() => api.saved.at(-1)?.layout).toBe("grid");
      const tiles = view.locator(".tile");
      await expect(tiles).toHaveCount(ROWS.length);
      await expectBox(tiles.nth(0), { x: 264, width: 214, height: 218 });
      await expectBox(tiles.nth(1), { x: 264 + 232, width: 214, height: 218 });
      await expectBox(tiles.nth(4), { x: 264 + 4 * 232, width: 214, height: 218 });
      await expectBox(tiles.nth(5), { x: 264, width: 214 });
      await expectBox(rowNamed(page, "harbour-masterplan-v7.pdf").locator(".thumb"), {
        width: 202,
        height: 154,
      });
      expect(await look(rowNamed(page, "harbour-masterplan-v7.pdf"))).toMatchObject({
        radius: "8px",
        borderWidth: "1px",
        borderColour: rgbOf(c.outline),
      });
      expect(
        await look(rowNamed(page, "harbour-masterplan-v7.pdf").locator(".thumb")),
      ).toMatchObject({
        radius: "5px",
        background: rgbOf(c.raised),
      });
      expect(
        await look(rowNamed(page, "harbour-masterplan-v7.pdf").locator(".name")),
      ).toMatchObject({
        size: "13px",
        weight: "500",
      });
      expect(
        await look(rowNamed(page, "harbour-masterplan-v7.pdf").locator(".meta")),
      ).toMatchObject({
        size: "13px",
        colour: rgbOf(c.muted),
      });
      await expect(rowNamed(page, "harbour-masterplan-v7.pdf").locator(".meta")).toHaveText(
        "18.4 MB · 2 hours ago",
      );
      const sort = view.locator("button.sort-button");
      await expect(sort).toContainText(fill("fileSortButton", { key: label(labels, "sortName") }));
      await sort.click();
      const menu = view.locator("[role=menu].sort-menu");
      await expect(menu.locator(".eyebrow")).toHaveText(label(labels, "fileSortBy"));
      await expect(menu.locator("[role=menuitemradio]")).toHaveCount(9);
      await expect(menu.locator("[aria-checked=true]")).toHaveCount(3);
      expect(await look(menu.locator(".eyebrow"))).toMatchObject({
        size: "11px",
        weight: "400",
        transform: "uppercase",
        colour: rgbOf(c.muted),
      });
      await menu.getByRole("menuitemradio", { name: label(labels, "sortSize") }).click();
      await expect
        .poll(() => api.calls.at(-1))
        .toMatchObject({ sort: "size", direction: "ascending" });
      await expect(sort).toContainText(fill("fileSortButton", { key: label(labels, "sortSize") }));
      expect(errors).toEqual([]);
    });

    test("selection (mockup 11): bulk bar, scope and the header checkbox", async ({ page }) => {
      const errors = trackErrors(page);
      await open(page);
      const view = drive(page);
      await expect(view).toHaveAttribute("data-state", "ready");
      await rowNamed(page, "harbour-masterplan-v7.pdf").locator("input.check").check();
      await rowNamed(page, "budget-2026.xlsx").locator("input.check").check();
      const bar = view.locator(".bulk");
      await expect(bar.locator(".bulk-count")).toHaveText(fill("fileSelected", { count: 2 }));
      await expect(bar.locator(".bulk-scope")).toHaveText(
        fill("fileSelectionScopeFiles", {
          files: fill("fileCountOther", { count: 2 }),
          size: "18.8 MB",
        }),
      );
      await expectBox(bar, { x: 264, y: 142, width: 1152, height: 56 });
      await expectBox(view.locator(".head"), { y: 198 });
      expect(await look(bar)).toMatchObject({ background: rgbOf(c.tint) });
      expect(await look(bar.locator(".bulk-count"))).toMatchObject({
        size: "14px",
        weight: "600",
        colour: rgbOf(c.active),
      });
      await expect(bar.locator("button")).toHaveText([
        label(labels, "fileDownloadZip"),
        label(labels, "fileMoveOrCopy"),
        label(labels, "fileRemove"),
      ]);
      expect(await look(bar.locator(".bulk-remove"))).toMatchObject({
        colour: rgbOf(c.error),
        borderColour: rgbOf(c.error),
      });
      expect(
        await view
          .locator("input.check-all")
          .evaluate((el) => (el as HTMLInputElement).indeterminate),
      ).toBe(true);
      await view.locator("input.check-all").click();
      await expect(bar.locator(".bulk-count")).toHaveText(
        fill("fileSelected", { count: ROWS.length }),
      );
      await view.locator("input.check-all").click();
      await expect(bar).toHaveCount(0);
      expect(errors).toEqual([]);
    });

    test("menus (mockup 10): the New menu and the item menu", async ({ page }) => {
      const errors = trackErrors(page);
      await open(page);
      await expect(drive(page)).toHaveAttribute("data-state", "ready");
      await page.locator("fs-app-shell nav .new").click();
      const newMenu = page.locator("fs-new-menu [role=menu]");
      await expectBox(newMenu, { width: 232 });
      await expect(newMenu.locator(".label")).toHaveText([
        label(labels, "fileUploadFiles"),
        label(labels, "fileUploadFolder"),
        label(labels, "fileNewFolder"),
      ]);
      await expect(newMenu.locator(".shortcut")).toHaveText(["⌘O", "⇧⌘N"]);
      expect(await look(newMenu)).toMatchObject({
        background: rgbOf(c.overlay),
        borderWidth: "1px",
        borderColour: rgbOf(c.strong),
        radius: "8px",
      });
      expect(await look(newMenu.locator(".shortcut").first())).toMatchObject({
        colour: rgbOf(c.muted),
      });
      await page.keyboard.press("Escape");
      await expect(newMenu).toHaveCount(0);

      const pdf = rowNamed(page, "harbour-masterplan-v7.pdf");
      await pdf.hover();
      await pdf.locator("button.actions").click();
      const item = drive(page).locator("[role=menu].item-menu");
      await expectBox(item, { width: 240 });
      await expectBox(item.locator("[role=menuitem]").first(), { height: 34 });
      expect(await look(item.locator(".remove"))).toMatchObject({ colour: rgbOf(c.error) });
      await page.keyboard.press("Escape");
      await pdf.click({ button: "right" });
      await expect(item.locator("[role=menuitem]")).toHaveCount(6);
      expect(errors).toEqual([]);
    });

    test("rename (mockups 14, 44): the dialog, its refusals and the write", async ({ page }) => {
      const errors = trackErrors(page);
      const api = await open(page);
      const view = drive(page);
      await expect(view).toHaveAttribute("data-state", "ready");
      const pdf = rowNamed(page, "harbour-masterplan-v7.pdf");
      await pdf.hover();
      await pdf.locator("button.actions").click();
      await view.getByRole("menuitem", { name: label(labels, "fileRename") }).click();
      const dialog = view.locator("[role=dialog]");
      await expectBox(dialog, { width: 560, height: 250 });
      await expect(dialog.locator(".dialog-title")).toHaveText(
        fill("fileRenameOnlyTitle", { name: "harbour-masterplan-v7.pdf" }),
      );
      expect(await look(dialog.locator(".dialog-title"))).toMatchObject({
        size: "20px",
        weight: "700",
        colour: rgbOf(c.heading),
      });
      expect((await look(dialog.locator(".dialog-title"))).family).toMatch(/^Georgia/);
      expect(await look(dialog)).toMatchObject({
        background: rgbOf(c.overlay),
        radius: "10px",
        borderColour: rgbOf(c.strong),
      });
      const input = dialog.locator("input");
      await expect(input).toBeFocused();
      await expect(input).toHaveValue("harbour-masterplan-v7.pdf");
      await expectBox(input, { height: 36 });
      await expect(dialog.locator("label")).toContainText(label(labels, "fileFieldName"));
      await expect(dialog.locator("button[type=submit]")).toHaveText(label(labels, "fileSave"));
      await expectBox(dialog.locator("button[type=submit]"), { height: 38 });
      expect(await look(dialog.locator("button[type=submit]"))).toMatchObject({
        background: rgbOf(c.primary),
        colour: rgbOf(c.onPrimary),
        size: "14px",
        weight: "600",
        radius: "7px",
      });

      await input.fill("x".repeat(1025));
      await expect(dialog.locator(".form-error")).toHaveText(
        label(labels, "fileDisplayNameTooLong"),
      );
      expect(await look(dialog.locator(".form-error"))).toMatchObject({ colour: rgbOf(c.error) });
      await expect
        .poll(() => look(input))
        .toMatchObject({ borderWidth: "2px", borderColour: rgbOf(c.error) });
      await expect(dialog.locator("button[type=submit]")).toBeDisabled();

      api.renameRefusal = { status: 409, code: "conflict" };
      await input.fill("taken.pdf");
      await dialog.locator("button[type=submit]").click();
      await expect(dialog.locator(".form-error")).toHaveText(label(labels, "fileRenameConflict"));
      await expect(input).toHaveValue("taken.pdf");
      await expect
        .poll(() => look(input))
        .toMatchObject({ borderWidth: "2px", borderColour: rgbOf(c.error) });

      api.renameRefusal = undefined;
      await dialog.locator("button[type=submit]").click();
      await expect(dialog).toHaveCount(0);
      const write = api.writes.filter((w) => w.route === "renames").at(-1);
      expect(write?.body).toMatchObject({
        envelope: { expected_revision: "v4", scope: "library:lib-1" },
        input: { library_id: "lib-1", file_id: "f1", display_name: "taken.pdf" },
      });
      expect(errors.filter((text) => !/status of (409|400)/.test(text))).toEqual([]);
    });

    test("remove (mockup 14): the toast with the version and Undo", async ({ page }) => {
      const errors = trackErrors(page);
      const api = await open(page);
      const view = drive(page);
      await expect(view).toHaveAttribute("data-state", "ready");
      const pdf = rowNamed(page, "harbour-masterplan-v7.pdf");
      await pdf.hover();
      await pdf.locator("button.actions").click();
      await view.getByRole("menuitem", { name: label(labels, "fileRemove") }).click();
      const toast = view.locator(".toast");
      await expect(toast.locator(".text")).toHaveText(
        fill("fileRemoved", { name: "harbour-masterplan-v7.pdf", version: "v5" }),
      );
      await expectBox(toast, { width: 440, height: 44 });
      expect(await look(toast)).toMatchObject({
        background: rgbOf(scheme === "light" ? "#111111" : c.overlay),
        colour: rgbOf(scheme === "light" ? "#ffffff" : "#f5f5f5"),
        radius: "8px",
        size: "13px",
      });
      expect(api.writes.at(-1)).toMatchObject({
        route: "removals",
        body: { envelope: { expected_revision: "v4" }, input: { path: "original/3f9a0001.pdf" } },
      });
      await toast.getByRole("button", { name: label(labels, "fileUndo") }).click();
      await expect(toast).toHaveCount(0);
      expect(api.writes.at(-1)).toMatchObject({
        route: "restorations",
        body: { input: { version: "v4", paths: ["original/3f9a0001.pdf"] } },
      });
      expect(errors).toEqual([]);
    });

    test("move or copy (mockup 13): the dialog, the destination tree and the honest refusal", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await open(page);
      const view = drive(page);
      await rowNamed(page, "harbour-masterplan-v7.pdf").locator("input.check").check();
      await rowNamed(page, "tender-response.pdf").locator("input.check").check();
      await view.locator(".bulk-move").click();
      const move = view.locator("[role=dialog]");
      await expectBox(move, { width: 600, height: 640 });
      await expect(move.locator(".dialog-title")).toHaveText(
        fill("fileMoveTitleMany", { count: 2 }),
      );
      await expect(move.locator(".names")).toHaveText(
        "harbour-masterplan-v7.pdf, tender-response.pdf",
      );
      expect(await look(move.locator(".dialog-title"))).toMatchObject({
        size: "22px",
        weight: "700",
        colour: rgbOf(c.heading),
      });
      expect(await look(move)).toMatchObject({ background: rgbOf(c.overlay), radius: "10px" });
      await expect(move.locator("[role=radio]")).toHaveText([
        label(labels, "fileMoveMode"),
        label(labels, "fileCopyMode"),
      ]);
      await expect(move).toContainText(label(labels, "fileMoveExplain"));
      expect(await look(move.locator("[role=radio][aria-checked=true]"))).toMatchObject({
        background: rgbOf(c.tint),
        colour: rgbOf(c.active),
      });
      const tree = move.locator("fs-destination-picker");
      await expect(tree.getByRole("treeitem")).toHaveText([/Family/, /Photos/, /Hobbies/]);
      await expect(tree.getByRole("treeitem", { name: "Family" })).toHaveAttribute(
        "data-selectable",
        "false",
      );
      await expect(tree.getByRole("treeitem", { name: /Best of 2025/ })).toHaveCount(0);
      const start = move.locator("button.start");
      await expect(start).toBeDisabled();
      await expect(start).toHaveText(label(labels, "fileMoveHere"));
      await tree.getByRole("treeitem", { name: /Photos/ }).click();
      await expect(tree.getByRole("treeitem", { name: /Photos/ })).toHaveAttribute(
        "aria-selected",
        "true",
      );
      expect(await look(tree.getByRole("treeitem", { name: /Photos/ }))).toMatchObject({
        background: rgbOf(c.tint),
        colour: rgbOf(c.active),
        radius: "5px",
      });
      await expect(move.locator(".destination-line")).toContainText("Family › Photos");
      await move.getByRole("radio", { name: label(labels, "fileCopyMode") }).click();
      await expect(start).toHaveText(label(labels, "fileCopyHere"));
      await expect(move).toContainText(label(labels, "fileCopyExplain"));
      await expect(start).toBeEnabled();
      await start.click();
      await expect(move.locator(".form-error")).toHaveText(
        label(labels, "fileMoveCopyUnavailable"),
      );
      await page.keyboard.press("Escape");
      await expect(move).toHaveCount(0);
      expect(errors).toEqual([]);
    });

    test("Download as ZIP (mockup 12): the dialog, no save path, and the caps", async ({
      page,
    }) => {
      const errors = trackErrors(page);
      await open(page);
      const view = drive(page);
      await rowNamed(page, "harbour-masterplan-v7.pdf").locator("input.check").check();
      await rowNamed(page, "budget-2026.xlsx").locator("input.check").check();
      await view.locator(".bulk-zip").click();
      const zip = view.locator("[role=dialog]");
      await expectBox(zip, { width: 640, height: 520 });
      await expect(zip.locator(".dialog-title")).toHaveText(label(labels, "fileZipTitle"));
      await expect(zip).toContainText(
        fill("fileZipSummary", { files: fill("fileCountOther", { count: 2 }), size: "18.8 MB" }),
      );
      await expect(zip.locator("input")).toHaveValue("Family.zip");
      expect((await look(zip.locator("input"))).family).toMatch(/mono/i);
      await expect(zip).toContainText(label(labels, "fileZipPolicyTitle"));
      await expect(zip.locator(".chip")).toHaveText(label(labels, "fileZipPolicyChip"));
      expect(await look(zip.locator(".chip"))).toMatchObject({
        size: "11px",
        weight: "500",
        background: rgbOf(c.tint),
        colour: rgbOf(c.active),
      });
      await expect(zip).toContainText(label(labels, "fileZipLimits"));
      await expect(zip).not.toContainText(/\/home|\/Users|C:\\/);
      await zip.locator("button.start").click();
      await expect(zip.locator(".form-error")).toHaveText(label(labels, "fileZipUnavailable"));
      await page.keyboard.press("Escape");
      await expect(zip).toHaveCount(0);
      expect(errors).toEqual([]);
    });
  });
}

test("the loading state (mockup 33): placeholder rows and the footer line", async ({ page }) => {
  const errors = trackErrors(page);
  await open(page, `#/navigation/${LIBRARY_NODE}`, (api) => void (api.delayMs = 1500));
  const view = drive(page);
  await expect(view).toHaveAttribute("data-state", "loading");
  await expect(view.locator(".head")).toBeVisible();
  await expect(view.locator(".placeholder-row").first()).toBeVisible();
  await expect(page.locator('fs-app-shell [slot="summary"]')).toHaveText(
    fill("fileLoadingName", { name: "Family" }),
  );
  await expect(view).toHaveAttribute("data-state", "ready", { timeout: 8000 });
  await expect(page.locator('fs-app-shell [slot="summary"]')).toContainText(
    fill("fileSummary", { folders: 3, files: 10, size: "" }).split(" · Loaded")[0] as string,
  );
  await expect(page.locator('fs-app-shell [slot="commit-state"]')).toHaveText(
    label(labels, "fileCommitted"),
  );
  expect(errors).toEqual([]);
});

test("the storage block (UI-S-025) shows the bytes the open library keeps and the free space", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await open(page);
  const nav = page.locator("fs-app-shell nav").first();
  await expect(nav.locator(".storage-value")).toHaveText("4.2 GB");
  await expect(nav.locator(".storage-title")).toHaveText(label(labels, "fileStorageUsed"));
  await expect(nav.locator(".storage-free")).toHaveText(
    fill("fileStorageFree", { free: "466 GB" }),
  );
  await expect(nav.locator(".storage-history")).toBeHidden();
  expect(errors).toEqual([]);
});

test("Recent (mockup 15) says the host does not serve it yet; the view stays under the shell", async ({
  page,
}) => {
  const errors = trackErrors(page, /status of 400/);
  await open(page, "#/recent");
  const view = page.locator("fs-app-shell > fs-drive-view");
  await expect(view).toHaveAttribute("data-state", "unavailable");
  await expect(view.locator(".page-title")).toHaveText(label(labels, "fileRecent"));
  await expect(view.locator(".recent-note")).toHaveText(label(labels, "fileRecentNote"));
  await expect(view.locator("fs-states")).toContainText(label(labels, "fileViewUnavailable"));
  await expect(page.locator("fs-app-shell nav").first()).toBeVisible();
  expect(errors).toEqual([]);
});

test("the sidebar filter narrows only the current view and is sent to the API", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await open(page);
  await expect(drive(page)).toHaveAttribute("data-state", "ready");
  await page.locator("fs-app-shell nav input.filter").fill("survey");
  await expect.poll(() => api.calls.at(-1)?.filter).toBe("survey");
  await expect(drive(page).locator(".row .name")).toHaveText(["survey-points.csv"]);
  expect(errors).toEqual([]);
});

test("a failed view read shows the error state with Try again, which reads again (mockup 56)", async ({
  page,
}) => {
  const errors = trackErrors(page, /status of 500/);
  let fail = true;
  await open(page, `#/navigation/${LIBRARY_NODE}`, async () => {
    await page.route(/\/api\/v1\/files\/view\?/, (route) =>
      fail
        ? route.fulfill({
            status: 500,
            json: { code: "internal", message: "boom", retryable: false },
          })
        : route.fallback(),
    );
  });
  const view = drive(page);
  await expect(view).toHaveAttribute("data-state", "error");
  await expect(view.locator("fs-states")).toContainText(label(labels, "stateError"));
  await expect(view.locator(".page-title")).toHaveText("Family");
  await expect(page.locator("fs-app-shell nav").first()).toBeVisible();
  fail = false;
  await view.locator("fs-states button").click();
  await expect(view).toHaveAttribute("data-state", "ready");
  expect(errors).toEqual([]);
});

test("the Browsing settings (mockup 32) save through the API and come back on a reload", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await open(page, "#/settings");
  const settings = page.locator("fs-settings");
  const browsing = settings.locator("[data-section=browsing]");
  await expect(browsing.locator("h3")).toHaveText(label(labels, "settingsBrowsing"));
  expect(await look(browsing.locator("h3"))).toMatchObject({ size: "22px", weight: "600" });
  await expect(browsing.locator(".pref-row label")).toHaveText([
    label(labels, "settingsDefaultLayout"),
    label(labels, "settingsSortBy"),
    label(labels, "settingsFolders"),
    label(labels, "settingsStartView"),
    label(labels, "settingsHoverCard"),
  ]);
  await expect(browsing.locator(".pref-note.help")).toHaveText(
    label(labels, "settingsBrowsingHelp"),
  );
  await browsing.locator("#pref-layout").selectOption("grid");
  await browsing.locator("#pref-sort").selectOption("size:descending");
  await browsing.locator("#pref-folders").selectOption("mixed");
  await browsing.locator("#pref-hover").uncheck();
  await expect
    .poll(() => api.saved.at(-1))
    .toMatchObject({
      layout: "grid",
      sort_key: "size",
      sort_direction: "descending",
      folder_placement: "mixed",
      hover_card: false,
    });
  await page.goto(`/#/navigation/${LIBRARY_NODE}`);
  await page.reload();
  await expect(drive(page).locator(".tile").first()).toBeVisible();
  await expect
    .poll(() => api.calls.at(-1))
    .toMatchObject({ sort: "size", direction: "descending", folders: "mixed" });
  expect(errors).toEqual([]);
});

test("the navigation choices on the device move to the API once and the key is removed (UI-S-017)", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const value = JSON.stringify({
    version: 1,
    pinned: [{ node: "c1", kind: "collection" }],
    order: [],
    collapsed: [],
  });
  await page.addInitScript(
    (text) => window.localStorage.setItem("filesquid.navigation", text),
    value,
  );
  const api = await open(page);
  await expect.poll(() => api.saved.length).toBeGreaterThan(0);
  expect(api.saved[0]?.navigation.pinned).toEqual([{ node: "c1", kind: "collection" }]);
  await expect
    .poll(() => page.evaluate(() => window.localStorage.getItem("filesquid.navigation")))
    .toBeNull();
  await expect(
    page.locator("fs-nav-tree").getByRole("treeitem", { name: "Photos", exact: true }).first(),
  ).toBeVisible();
  expect(errors).toEqual([]);
});

test("a host with no preference store keeps working and says so in Settings", async ({ page }) => {
  const errors = trackErrors(page);
  await open(page, "#/settings", (api) => void (api.preferencesUnavailable = true));
  await expect(page.locator("fs-settings [data-section=browsing] .pref-status")).toHaveText(
    label(labels, "settingsBrowsingDeviceOnly"),
  );
  await page.locator("fs-settings #pref-layout").selectOption("grid");
  await expect(page.locator("fs-settings #pref-layout")).toHaveValue("grid");
  expect(errors).toEqual([]);
});
