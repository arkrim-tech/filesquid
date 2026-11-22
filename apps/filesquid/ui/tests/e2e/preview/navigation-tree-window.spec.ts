/**
 * Production-artifact proof of the virtualised navigation tree (Task UI-F3.4, UI-S-035,
 * UI-S-017, UI-S-016): the FileSquid UI built by `vite build` and served by `vite preview`, at
 * the mockups' 1440 x 900 window, with `GET /api/v1/navigation` answered by the test from a
 * library of 50,000 collections, so its level shows the first 10,000. In a real layout the tree
 * holds only the items in its window plus two overscans of 5, states each item's place in the
 * whole level, reaches the last node with End, says where the level stopped in a line worded
 * from the label table, opens a pin beyond the cap by its handle, and keeps the look of mockup
 * 55 in light and dark. Strict DOM assertions: custom elements defined, words from the active
 * label table, computed token values, no console errors.
 */
import { expect, test, type Page, type WebSocketRoute } from "@playwright/test";
import {
  activeLabels,
  expectElementsDefined,
  label,
  setScheme,
  tokenColour,
  trackErrors,
  type Scheme,
} from "../support/strict-dom";
import { serveDrive } from "../support/drive-api";
import { BIG_LIBRARY, BigNavigationSource, bigHandle, bigName } from "../../support/big-navigation";
import { ApiError } from "../../../src/api/client";
import { DEFAULT_PREFERENCES, type UserPreferences } from "../../../src/preferences/model";

const labels = activeLabels();

/** Rows kept above and below the view (UI-S-035). */
const OVERSCAN = 5;
/** Row height in pixels at the default text size (mockup 55). */
const ROW_PX = 30;

test.use({ viewport: { width: 1440, height: 900 } });

/** The shell's health, host and event channel, answered so only the build is under test. */
async function serveShell(page: Page): Promise<WebSocketRoute[]> {
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
  return sockets;
}

/** Answer the navigation listing from a library of `total` collections. */
async function serveBigNavigation(page: Page, total: number): Promise<BigNavigationSource> {
  const source = new BigNavigationSource({ total });
  await page.route(/\/api\/v1\/navigation(\?|$)/, async (route) => {
    const q = new URL(route.request().url()).searchParams;
    const limit = q.get("limit");
    try {
      const body = await source.list(
        q.get("node"),
        q.get("cursor"),
        limit ? Number(limit) : undefined,
      );
      await route.fulfill({ json: body });
    } catch (error) {
      const status = error instanceof ApiError ? error.status : 500;
      await route.fulfill({ status, json: { code: "not_found", message: "no", retryable: false } });
    }
  });
  return source;
}

/** Open the built UI on a library of 50,000 collections, with these navigation choices. */
async function open(page: Page, navigation: Partial<UserPreferences["navigation"]> = {}) {
  const errors = trackErrors(page);
  await serveShell(page);
  const source = await serveBigNavigation(page, 50_000);
  const stored = {
    ...DEFAULT_PREFERENCES,
    navigation: { ...DEFAULT_PREFERENCES.navigation, ...navigation },
  } as UserPreferences;
  const nodes = [BIG_LIBRARY, bigHandle(0), bigHandle(9999), bigHandle(47_999)];
  const drive = await serveDrive(page, undefined, stored, nodes);
  await page.goto(`/#/navigation/${bigHandle(0)}`);
  await expectElementsDefined(page, ["fs-app-shell", "fs-nav-tree"]);
  await expect(items(page).first()).toBeVisible();
  return { errors, source, drive };
}

/** The tree. */
const tree = (page: Page) => page.locator("fs-app-shell fs-nav-tree");
/** The drawn tree items of the Collections section. */
const items = (page: Page) => tree(page).locator('[data-section="collection"] [role="treeitem"]');

/** The tree's scrolling area in the sidebar: its height, content height and the items drawn. */
function measure(page: Page): Promise<{ client: number; content: number; drawn: number }> {
  return page.evaluate(() => {
    const shell = document.querySelector("fs-app-shell")?.shadowRoot;
    const area = shell?.querySelector<HTMLElement>(".tree");
    const root = document.querySelector("fs-nav-tree")?.shadowRoot;
    return {
      client: area?.clientHeight ?? 0,
      content: area?.scrollHeight ?? 0,
      drawn: root?.querySelectorAll('[role="treeitem"]').length ?? 0,
    };
  });
}

/** Scroll the tree's area to a position and let the window follow. */
async function scrollTree(page: Page, top: number): Promise<void> {
  await page.evaluate((y) => {
    const area = document.querySelector("fs-app-shell")?.shadowRoot?.querySelector(".tree");
    if (area) area.scrollTop = y;
  }, top);
  await page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => resolve(null))));
}

/** The most tree items the window may hold for an area this tall. */
const bound = (client: number) => Math.ceil(client / ROW_PX) + 1 + 2 * OVERSCAN;

test("the tree holds only its window of a 10,000-node level, counted over the whole level", async ({
  page,
}) => {
  const { errors } = await open(page);
  const start = await measure(page);
  expect(start.client).toBeGreaterThan(5 * ROW_PX);
  expect(start.client).toBeLessThanOrEqual(900);
  expect(start.content).toBeGreaterThanOrEqual(10_000 * ROW_PX);
  expect(start.drawn).toBeLessThanOrEqual(bound(start.client));
  expect(start.drawn).toBeGreaterThan(start.client / ROW_PX / 2);
  for (const top of [4_000, 150_000, 299_000, start.content]) {
    await scrollTree(page, top);
    const now = await measure(page);
    expect(now.drawn, `at ${top}`).toBeLessThanOrEqual(bound(now.client));
    expect(now.drawn, `at ${top}`).toBeGreaterThan(0);
  }
  await scrollTree(page, 150_000);
  const middle = items(page);
  const count = await middle.count();
  for (let i = 0; i < count; i += 1) {
    const item = middle.nth(i);
    const place = Number((await item.innerText()).replace("Harbour ", ""));
    await expect(item).toHaveAttribute("aria-level", "1");
    await expect(item).toHaveAttribute("aria-setsize", "10000");
    await expect(item).toHaveAttribute("aria-posinset", String(place));
  }
  await expect(tree(page).locator('[data-section="collection"] [role="tree"]')).toHaveAttribute(
    "aria-labelledby",
    /fs-nav-\d+-heading/,
  );
  expect(errors).toEqual([]);
});

test("a capped level says so on screen under its last node, from the label table", async ({
  page,
}) => {
  const { errors } = await open(page);
  await scrollTree(page, 1_000_000);
  const line = tree(page).locator("li.limit");
  const text = label(labels, "navigationLevelLimit").replace("{count}", "10,000");
  expect(text).toBe("Showing the first 10,000 only. Use search to find the rest.");
  await expect(line).toHaveText(text);
  await expect(line).toBeVisible();
  const last = items(page).filter({ hasText: bigName(9999) });
  await expect(last).toHaveAttribute(
    "aria-describedby",
    (await line.locator("span").getAttribute("id")) ?? "",
  );
  // The line sits under the last node shown, inside the scrolling area.
  const lastBox = await last.boundingBox();
  const lineBox = await line.boundingBox();
  expect(lineBox?.y ?? 0).toBeGreaterThanOrEqual((lastBox?.y ?? 0) + (lastBox?.height ?? 0) - 1);
  expect(errors).toEqual([]);
});

test("End reaches the last of 10,000 nodes and Home the first, with one Tab stop", async ({
  page,
}) => {
  const { errors } = await open(page);
  await items(page)
    .filter({ hasText: bigName(0) })
    .first()
    .focus();
  await page.keyboard.press("End");
  const focused = tree(page).locator('[role="treeitem"]:focus');
  await expect(focused).toHaveText(bigName(9999));
  await expect(focused).toHaveAttribute("aria-posinset", "10000");
  await expect(focused).toBeInViewport();
  await expect(items(page).and(page.locator('[tabindex="0"]'))).toHaveCount(1);
  expect((await measure(page)).drawn).toBeLessThanOrEqual(bound((await measure(page)).client) + 1);
  await page.keyboard.press("Home");
  await expect(focused).toHaveText(bigName(0));
  await expect(focused).toBeInViewport();
  for (let i = 0; i < 40; i += 1) await page.keyboard.press("ArrowDown");
  await expect(focused).toHaveText(bigName(40));
  await expect(focused).toBeInViewport();
  expect((await measure(page)).drawn).toBeLessThanOrEqual(bound((await measure(page)).client) + 1);
  expect(errors).toEqual([]);
});

test("a pin beyond the cap opens by its handle and survives a reload", async ({ page }) => {
  const { errors, source } = await open(page, {
    pinned: [{ node: bigHandle(47_999), kind: "collection" }],
  });
  const pin = tree(page).locator('[data-section="pinned"] [role="treeitem"]');
  await expect(pin).toHaveText("Harbour 48000");
  await expect(pin).toHaveAttribute("href", `#/navigation/${bigHandle(47_999)}`);
  await pin.click();
  await expect(page).toHaveURL(new RegExp(`#/navigation/${bigHandle(47_999)}$`));
  await expect(pin).toHaveAttribute("aria-current", "page");
  expect(source.calls.some((call) => call.parent === bigHandle(47_999))).toBe(true);
  await page.reload();
  await expect(pin).toHaveText("Harbour 48000");
  expect((await measure(page)).drawn).toBeLessThanOrEqual(bound((await measure(page)).client) + 1);
  expect(errors).toEqual([]);
});

for (const scheme of ["light", "dark"] as Scheme[]) {
  test(`rows keep the look of mockup 55 in ${scheme}`, async ({ page }) => {
    await setScheme(page, scheme);
    const { errors } = await open(page);
    const current = items(page)
      .filter({ hasText: bigName(0) })
      .first();
    await expect(current).toHaveAttribute("aria-current", "page");
    const look = await current.evaluate((el) => {
      const style = getComputedStyle(el);
      const box = el.getBoundingClientRect();
      return {
        height: box.height,
        fontSize: style.fontSize,
        background: style.backgroundColor,
        color: style.color,
        whiteSpace: style.whiteSpace,
      };
    });
    expect(look.height).toBe(ROW_PX);
    expect(look.fontSize).toBe("13px");
    expect(look.whiteSpace).toBe("nowrap");
    expect(look.background).toBe(await tokenColour(page, "--fs-primary-container"));
    expect(look.color).toBe(await tokenColour(page, "--fs-on-primary-container"));
    const other = items(page)
      .filter({ hasText: bigName(1) })
      .first();
    expect(await other.evaluate((el) => getComputedStyle(el).color)).toBe(
      await tokenColour(page, "--fs-on-surface-variant"),
    );
    await scrollTree(page, 1_000_000);
    const line = tree(page).locator("li.limit");
    expect(await line.evaluate((el) => getComputedStyle(el).color)).toBe(
      await tokenColour(page, "--fs-on-surface-muted"),
    );
    expect(await line.evaluate((el) => el.getBoundingClientRect().height)).toBe(2 * ROW_PX);
    // The shell's Recently deleted row stays the tree's final row, after the scrolling area.
    const deleted = page.locator("fs-app-shell .deleted a");
    await expect(deleted).toHaveText(label(labels, "navigationRecentlyDeleted"));
    expect(errors).toEqual([]);
  });
}
