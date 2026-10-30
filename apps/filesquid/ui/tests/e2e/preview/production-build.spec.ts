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
import { serveNavigation } from "../support/navigation-api";
import { CONTRACT_VERSION } from "../../../src/api/contract-version";

const labels = activeLabels();
const ELEMENTS = [
  "fs-app-shell",
  "fs-theme-control",
  "fs-states",
  "fs-nav-tree",
  "fs-breadcrumb",
  "fs-navigation-view",
  "fs-settings",
  "fs-file-browser",
  "fs-file-preview",
  "fs-host-indicator",
  "fs-monitor",
  "fs-search-bar",
  "fs-drive-view",
];

test.beforeEach(async ({ page }) => {
  await page.route("**/api/v1/health", (route) =>
    route.fulfill({
      json: { schema_version: 1, status: "ok", product: "filesquid" },
    }),
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
  // The host indicator follows the event channel; answer it so no real socket is attempted.
  await page.routeWebSocket(/\/api\/v1\/events$/, () => undefined);
  await serveNavigation(page);
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

test("sidebar, header and Settings words come from the active label table (UI-S-025, UI-S-033)", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await page.goto("/");
  const shell = page.locator("fs-app-shell");
  // The live navigation tree fills the shell's navigation slot.
  await expect(shell.locator("fs-nav-tree section h2")).toHaveText(
    ["collections", "virtualCollections", "folders"].map((key) => label(labels, key)),
  );
  await expect(shell.locator("nav ul.views li")).toHaveText(
    ["fileRoot", "fileRecent"].map((key) => label(labels, key)),
  );
  await expect(shell.locator("nav ul.deleted li")).toHaveText(
    label(labels, "navigationRecentlyDeleted"),
  );
  // The host descriptor names the desktop host, so the Logs row is shown (UI-S-021).
  await expect(shell.locator("nav > ul.tools li")).toHaveText(
    ["libraryNavLink", "settings", "logs"].map((key) => label(labels, key)),
  );
  await expect(shell.locator("nav .new")).toHaveText(label(labels, "fileNew"));
  await expect(shell.locator("nav input.filter")).toHaveAttribute(
    "placeholder",
    label(labels, "fileFilterView"),
  );
  await expect(shell.locator("fs-search-bar input.text")).toHaveAttribute(
    "placeholder",
    label(labels, "searchPlaceholder"),
  );
  await expect(shell.locator("header .monitor")).toHaveText(label(labels, "monitor"));
  // Owner answers 7 and 12: no theme control and no API status outside Settings.
  await expect(shell.locator("header fs-theme-control")).toHaveCount(0);
  await expect(shell.locator("p.api")).toHaveCount(0);
  await page.goto("/#/settings");
  const settings = page.locator("fs-settings");
  await expect(settings.locator(".api[role='status']")).toHaveText(
    label(labels, "settingsApiStatus")
      .replace("{status}", "ok")
      .replace("{version}", CONTRACT_VERSION),
  );
  await expect(settings.locator("fs-theme-control legend")).toHaveText(label(labels, "theme"));
  expect(errors).toEqual([]);
});

test("computed token values are exact in light and dark (UI-S-024, CA-1)", async ({ page }) => {
  const errors = trackErrors(page);
  await setScheme(page, "light");
  await page.goto("/");
  await expect(page.locator("fs-app-shell h1")).toHaveText("FileSquid");
  expect(await tokenColour(page, "--fs-primary")).toBe(rgbOf("#16ac49"));
  expect(await tokenColour(page, "--fs-on-primary")).toBe(rgbOf("#ffffff"));
  expect(await tokenColour(page, "--fs-error")).toBe(rgbOf("#e53935"));
  expect(await tokenColour(page, "--fs-focus-ring")).toBe(rgbOf("#0f8a39"));
  expect(await tokenColour(page, "--fs-outline-control")).toBe(rgbOf("#8e8e8e"));
  expect(await bodyBackground(page)).toBe(rgbOf("#ffffff"));

  await setScheme(page, "dark");
  expect(await tokenColour(page, "--fs-primary")).toBe(rgbOf("#16ac49"));
  expect(await tokenColour(page, "--fs-on-primary")).toBe(rgbOf("#ffffff"));
  expect(await tokenColour(page, "--fs-error")).toBe(rgbOf("#ef5350"));
  expect(await tokenColour(page, "--fs-focus-ring")).toBe(rgbOf("#16ac49"));
  expect(await tokenColour(page, "--fs-outline-control")).toBe(rgbOf("#757575"));
  expect(await bodyBackground(page)).toBe(rgbOf("#141414"));
  expect(errors).toEqual([]);
});

test("a refused health call shows the unavailable alert in Settings, About, not a console error", async ({
  page,
}) => {
  await page.unroute("**/api/v1/health");
  await page.route("**/api/v1/health", (route) =>
    route.fulfill({ status: 503, json: { code: "unavailable", message: "down" } }),
  );
  const errors = trackErrors(page, /status of 503 \(/);
  await page.goto("/#/settings");
  await expect(page.locator('fs-settings .api[role="alert"]')).toContainText(
    label(labels, "apiUnavailable"),
  );
  await expect(page.locator("fs-app-shell header p.api")).toHaveCount(0);
  expect(errors).toEqual([]);
});
