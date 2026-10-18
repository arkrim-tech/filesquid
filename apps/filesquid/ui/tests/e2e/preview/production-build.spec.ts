/**
 * Production-artifact proof (UI-S-013): the FileSquid UI built by `vite build` and served by
 * `vite preview`, with no FileSquid server behind it. The test answers the shell's one health
 * call itself, so only the static build is under test. Strict DOM assertions: custom elements
 * defined, words from the active label table, computed token values in light and dark, no
 * console errors.
 */
import { expect, test } from "@playwright/test";
import {
  activeLabels,
  bodyBackground,
  expectElementsDefined,
  label,
  rgbOf,
  setScheme,
  tokenColour,
  trackErrors,
} from "../support/strict-dom";

const labels = activeLabels();
const ELEMENTS = [
  "fs-app-shell",
  "fs-theme-control",
  "fs-states",
  "fs-nav-tree",
  "fs-breadcrumb",
  "fs-monitor",
];

test.beforeEach(async ({ page }) => {
  await page.route("**/api/v1/health", (route) =>
    route.fulfill({
      json: { schema_version: 1, status: "ok", product: "filesquid" },
    }),
  );
});

test("the built page is static, mounts the shell and defines every custom element", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const response = await page.goto("/");
  expect(response?.headers()["content-type"]).toContain("text/html");
  await expect(page).toHaveTitle("FileSquid");
  await expectElementsDefined(page, ELEMENTS);
  const scripts = await page
    .locator("script[src]")
    .evaluateAll((all) => all.map((script) => script.getAttribute("src") ?? ""));
  expect(scripts.length).toBeGreaterThan(0);
  for (const src of scripts) expect(src).toMatch(/^\/assets\/[A-Za-z0-9_]+-[A-Za-z0-9_-]{8}\.js$/);
  expect(await page.locator("script:not([src])").count()).toBe(0);
  expect(errors).toEqual([]);
});

test("navigation, API status and theme legend words come from the active label table", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await page.goto("/");
  const shell = page.locator("fs-app-shell");
  await expect(shell.locator("nav li")).toHaveText(
    ["libraries", "collections", "virtualCollections", "virtualExhibits", "monitor"].map((key) =>
      label(labels, key),
    ),
  );
  await expect(shell.locator('[role="status"]')).toHaveText(
    `${label(labels, "apiStatus")}: ok (filesquid)`,
  );
  await expect(shell.locator("fs-theme-control legend")).toHaveText(label(labels, "theme"));
  expect(errors).toEqual([]);
});

test("computed token values are exact in light and dark", async ({ page }) => {
  const errors = trackErrors(page);
  await setScheme(page, "light");
  await page.goto("/");
  await expect(page.locator("fs-app-shell h1")).toHaveText("FileSquid");
  expect(await tokenColour(page, "--fs-primary")).toBe(rgbOf("#0b7a32"));
  expect(await tokenColour(page, "--fs-on-primary")).toBe(rgbOf("#ffffff"));
  expect(await bodyBackground(page)).toBe(rgbOf("#ffffff"));

  await setScheme(page, "dark");
  expect(await tokenColour(page, "--fs-primary")).toBe(rgbOf("#16ac49"));
  expect(await tokenColour(page, "--fs-on-primary")).toBe(rgbOf("#0d2b18"));
  expect(await bodyBackground(page)).toBe(rgbOf("#141414"));
  expect(errors).toEqual([]);
});

test("a refused health call shows the unavailable alert, not a console error from the app", async ({
  page,
}) => {
  await page.unroute("**/api/v1/health");
  await page.route("**/api/v1/health", (route) =>
    route.fulfill({ status: 503, json: { code: "unavailable", message: "down" } }),
  );
  const errors = trackErrors(page, /status of 503 \(/);
  await page.goto("/");
  await expect(page.locator('fs-app-shell [role="alert"]')).toContainText(
    label(labels, "apiUnavailable"),
  );
  expect(errors).toEqual([]);
});
