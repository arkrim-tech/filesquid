/**
 * Production-artifact proof of the live navigation data (UI-F2.1b part 2, F-004, API-S-021):
 * the FileSquid UI built by `vite build` and served by `vite preview`, with
 * `GET /api/v1/navigation` and the event channel answered by the test. The shell's navigation
 * slot holds the live tree; levels are read page by page through the cursor; a destination
 * opens at `#/navigation/<handle>` with a breadcrumb filled from `ancestors`; a
 * `navigation.change` reads only the named level again, without a reload; a 503 shows the
 * unavailable state with Retry. Strict DOM assertions: custom elements defined, every word
 * from the active label table, computed token values in light and dark, no console errors.
 */
import { expect, test, type Page, type WebSocketRoute } from "@playwright/test";
import {
  activeLabels,
  expectElementsDefined,
  label,
  rgbOf,
  setScheme,
  tokenColour,
  trackErrors,
} from "../support/strict-dom";
import { FAMILY, serveNavigation, type NavigationApi } from "../support/navigation-api";
import { serveDrive } from "../support/drive-api";
import type { FixtureNode } from "../../../src/navigation/source";

const labels = activeLabels();

/** The shell's health, host and event channel, answered so only the build is under test. */
async function serveShell(page: Page): Promise<{ send: (event: object) => void }> {
  const sockets: WebSocketRoute[] = [];
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
  await page.routeWebSocket(/\/api\/v1\/events$/, (ws) => void sockets.push(ws));
  // The file view of a library or collection reads its rows through the view operation.
  await serveDrive(page, [], undefined, ["l1", "l2", "c1", "c2", "c3", "v1"]);
  return { send: (event) => sockets.forEach((ws) => ws.send(JSON.stringify(event))) };
}

/** The tree in the shell. */
const tree = (page: Page) => page.locator("fs-app-shell fs-nav-tree");

/** The treeitem named `name`. */
const treeItem = (page: Page, name: string) =>
  tree(page).getByRole("treeitem", { name, exact: true });

/** Calls to one level, after the page settled. */
const callsFor = (api: NavigationApi, node: string | null) =>
  api.calls.filter((call) => call.node === node);

test("the live tree fills the shell's navigation slot, worded from the label table", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await serveShell(page);
  const api = await serveNavigation(page);
  await page.goto("/");
  await expectElementsDefined(page, ["fs-nav-tree", "fs-navigation-view", "fs-breadcrumb"]);
  await expect(tree(page)).toHaveAttribute("slot", "navigation");
  await expect(tree(page).locator("section h2")).toHaveText([
    label(labels, "collections"),
    label(labels, "virtualCollections"),
    label(labels, "folders"),
  ]);
  await expect(treeItem(page, "Photos")).toHaveAttribute("href", "#/navigation/c1");
  expect(callsFor(api, null).length).toBeGreaterThan(0);
  expect(callsFor(api, "r").length).toBeGreaterThan(0);
  expect(api.calls.every((call) => call.limit === null)).toBe(true);
  expect(errors).toEqual([]);
});

test("a level is read page by page through next_cursor until it is null", async ({ page }) => {
  const errors = trackErrors(page);
  await serveShell(page);
  const many: FixtureNode = {
    node: "r",
    kind: "repository",
    name: "Home",
    children: [
      {
        node: "l1",
        kind: "library",
        name: "Family",
        children: ["A", "B", "C", "D", "E"].map((name) => ({
          node: `c${name}`,
          kind: "collection" as const,
          name,
        })),
      },
    ],
  };
  const api = await serveNavigation(page, many, 2);
  await page.goto("/");
  await expect(tree(page).locator('[data-section="collection"] [role="treeitem"]')).toHaveText([
    "A",
    "B",
    "C",
    "D",
    "E",
  ]);
  // Every read of the level is three pages: a first page, then two through cursors.
  const library = callsFor(api, "l1");
  const firsts = library.filter((call) => call.cursor === null).length;
  expect(library[0]?.cursor).toBeNull();
  expect(firsts).toBeGreaterThan(0);
  expect(library.length).toBe(firsts * 3);
  expect(errors).toEqual([]);
});

test("a destination opens with a breadcrumb filled from ancestors", async ({ page }) => {
  const errors = trackErrors(page);
  await serveShell(page);
  const api = await serveNavigation(page);
  await page.goto("/");
  await treeItem(page, "Photos").locator(".twisty").click();
  await treeItem(page, "2025").click();
  await expect(page).toHaveURL(/#\/navigation\/f1$/);
  const view = page.locator("fs-app-shell fs-navigation-view");
  await expect(view).toHaveAttribute("data-state", "ready");
  const crumbs = view.locator("fs-breadcrumb a");
  await expect(crumbs).toHaveText([label(labels, "repository"), "Family", "Photos", "2025"]);
  await expect(crumbs.last()).toHaveAttribute("aria-current", "page");
  await expect(crumbs.first()).toHaveAttribute("href", "#/navigation/r");
  await expect(view.locator("h2")).toHaveText("2025");
  await expect(view.locator(".files")).toHaveText(label(labels, "navigationFilesPending"));
  await expect(treeItem(page, "2025")).toHaveAttribute("aria-current", "page");
  expect(api.calls).toContainEqual({ node: "f1", cursor: null, limit: "1" });
  expect(errors).toEqual([]);
});

test("a deep link to a destination shows its breadcrumb after a reload", async ({ page }) => {
  const errors = trackErrors(page);
  await serveShell(page);
  await serveNavigation(page);
  await page.goto("/#/navigation/c1");
  const crumbs = page.locator("fs-navigation-view fs-breadcrumb a");
  await expect(crumbs).toHaveText([label(labels, "repository"), "Family", "Photos"]);
  await page.reload();
  await expect(crumbs).toHaveText([label(labels, "repository"), "Family", "Photos"]);
  expect(errors).toEqual([]);
});

test("navigation.change reads only the named level again, without a reload", async ({ page }) => {
  const errors = trackErrors(page);
  const shell = await serveShell(page);
  const api = await serveNavigation(page);
  await page.goto("/");
  await expect(treeItem(page, "inbox")).toBeVisible();
  await page.evaluate(() => ((window as unknown as { marker: string }).marker = "same page"));
  // The read at start, then the whole tree again when the event channel opens.
  await expect.poll(() => callsFor(api, "l1").length).toBeGreaterThanOrEqual(2);
  await expect(treeItem(page, "inbox")).toBeVisible();
  const before = api.calls.length;
  const lib = FAMILY.children?.[0] as FixtureNode;
  const renamed = lib.children?.map((c) => (c.node === "f9" ? { ...c, name: "Inbox" } : c));
  api.source.replace({ ...FAMILY, children: [{ ...lib, children: renamed }] }, { parents: [] });
  shell.send({ topic: "navigation.change", data: { parents: ["l1"] } });
  await expect(treeItem(page, "Inbox")).toBeVisible();
  expect(api.calls.slice(before).map((call) => call.node)).toEqual(["l1"]);
  expect(await page.evaluate(() => (window as unknown as { marker: string }).marker)).toBe(
    "same page",
  );
  expect(errors).toEqual([]);
});

test("a 503 shows the unavailable state with Retry, never an empty tree", async ({ page }) => {
  // trackErrors excuses only the browser's network line for this route's 503.
  const errors = trackErrors(page);
  await serveShell(page);
  const api = await serveNavigation(page);
  api.unavailable = true;
  await page.goto("/");
  const states = tree(page).locator("fs-states");
  await expect(states).toHaveAttribute("data-state", "unavailable");
  await expect(states.locator('[role="alert"]')).toContainText(label(labels, "stateUnavailable"));
  await expect(tree(page).locator("section")).toHaveCount(0);
  api.unavailable = false;
  await states.getByRole("button", { name: label(labels, "stateRetry") }).click();
  await expect(treeItem(page, "Photos")).toBeVisible();
  expect(errors).toEqual([]);
});

test("computed token values of the tree and breadcrumb are exact in light and dark", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await serveShell(page);
  await serveNavigation(page);
  await setScheme(page, "light");
  await page.goto("/#/navigation/c1");
  const current = treeItem(page, "Photos");
  await expect(current).toHaveAttribute("aria-current", "page");
  const link = page.locator("fs-navigation-view fs-breadcrumb a").first();
  const currentCrumb = page.locator('fs-navigation-view fs-breadcrumb a[aria-current="page"]');
  const colours = async () => ({
    current: await current.evaluate((el) => getComputedStyle(el).backgroundColor),
    currentText: await current.evaluate((el) => getComputedStyle(el).color),
    crumb: await link.evaluate((el) => getComputedStyle(el).color),
    currentCrumb: await currentCrumb.evaluate((el) => getComputedStyle(el).color),
  });
  // UI-S-025: breadcrumb ancestors in --fs-on-surface-variant; the active row on the tint.
  const tokens = async () => ({
    current: await tokenColour(page, "--fs-primary-container"),
    currentText: await tokenColour(page, "--fs-on-primary-container"),
    crumb: await tokenColour(page, "--fs-on-surface-variant"),
    currentCrumb: await tokenColour(page, "--fs-on-surface"),
  });
  // Polled: the items and links transition their colours when the scheme changes.
  await expect.poll(colours).toEqual(await tokens());
  expect(await tokens()).toEqual({
    current: rgbOf("#f0faf4"),
    currentText: rgbOf("#0f8a39"),
    crumb: rgbOf("#555555"),
    currentCrumb: rgbOf("#111111"),
  });
  await setScheme(page, "dark");
  const dark = await tokens();
  expect(dark).toEqual({
    current: rgbOf("#0d2b18"),
    currentText: rgbOf("#4aca74"),
    crumb: rgbOf("#a0a0a0"),
    currentCrumb: rgbOf("#e0e0e0"),
  });
  await expect.poll(colours).toEqual(dark);
  expect(errors).toEqual([]);
});
