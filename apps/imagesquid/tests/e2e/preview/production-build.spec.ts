/**
 * Production-artifact proof (UI-S-013 suite shape, run for the ImageSquid distribution): the
 * UI built by `vite build` and served by `vite preview`, with no FileSquid server behind it.
 * The test answers the shell's one health call itself. Strict DOM assertions: custom elements
 * defined, words from the active label table, computed token values in light and dark, no
 * console errors.
 */
import { readFileSync } from "node:fs";
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

/**
 * The contract version the linked `@filesquid/ui` targets, read from its generated module (the
 * package entry defines custom elements, which a Node test process cannot load).
 */
const CONTRACT_VERSION =
  /CONTRACT_VERSION = "([^"]+)"/.exec(
    readFileSync("node_modules/@filesquid/ui/src/api/contract-version.ts", "utf8"),
  )?.[1] ?? "";

const labels = activeLabels();
const ELEMENTS = [
  "fs-app-shell",
  "fs-theme-control",
  "fs-states",
  "fs-nav-tree",
  "fs-breadcrumb",
  "fs-host-indicator",
  "fs-monitor",
  "fs-library-start",
  "fs-library-detail",
  "fs-library-repair",
];

test.beforeEach(async ({ page }) => {
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
  // The host indicator follows the event channel; answer it so no real socket is attempted.
  await page.routeWebSocket(/\/api\/v1\/events$/, () => undefined);
});

test("the built page is static, mounts the shell and defines every custom element", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const response = await page.goto("/");
  expect(response?.headers()["content-type"]).toContain("text/html");
  await expect(page).toHaveTitle("ImageSquid");
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
  // The host is the desktop here, so the Logs row is shown (UI-S-021).
  await expect(shell.locator("nav li")).toHaveText(
    [
      "fileRoot",
      "fileRecent",
      "libraries",
      "collections",
      "virtualCollections",
      "virtualExhibits",
      "navigationRecentlyDeleted",
      "libraryNavLink",
      "settings",
      "logs",
    ].map((key) => label(labels, key)),
  );
  await expect(shell.locator("header .monitor")).toHaveText(label(labels, "monitor"));
  await expect(shell.locator("header input.search")).toHaveAttribute(
    "placeholder",
    label(labels, "searchPlaceholder"),
  );
  // Owner answers 7 and 12: the theme control and the API status live in Settings only.
  await expect(shell.locator("header fs-theme-control")).toHaveCount(0);
  await expect(shell.locator("header p.api")).toHaveCount(0);
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

test("computed ImageSquid token values are exact in light and dark", async ({ page }) => {
  const errors = trackErrors(page);
  await setScheme(page, "light");
  await page.goto("/");
  await expect(page.locator("fs-app-shell h1")).toHaveText("ImageSquid");
  expect(await tokenColour(page, "--fs-primary")).toBe(rgbOf("#005db6"));
  expect(await tokenColour(page, "--fs-on-primary")).toBe(rgbOf("#ffffff"));
  expect(await tokenColour(page, "--fs-surface-card")).toBe(rgbOf("#f4f5f7"));
  expect(await tokenColour(page, "--fs-focus-ring")).toBe(rgbOf("#005db6"));
  expect(await tokenColour(page, "--fs-outline-control")).toBe(rgbOf("#7d8490"));
  expect(await bodyBackground(page)).toBe(rgbOf("#ffffff"));

  await setScheme(page, "dark");
  expect(await tokenColour(page, "--fs-primary")).toBe(rgbOf("#a9c7ff"));
  expect(await tokenColour(page, "--fs-on-primary")).toBe(rgbOf("#003063"));
  expect(await tokenColour(page, "--fs-surface-card")).toBe(rgbOf("#1b1e24"));
  expect(await tokenColour(page, "--fs-focus-ring")).toBe(rgbOf("#a9c7ff"));
  expect(await tokenColour(page, "--fs-outline-control")).toBe(rgbOf("#858c96"));
  expect(await bodyBackground(page)).toBe(rgbOf("#121418"));
  expect(errors).toEqual([]);
});

test("a refused health call shows the unavailable alert in Settings, About", async ({ page }) => {
  await page.unroute("**/api/v1/health");
  await page.route("**/api/v1/health", (route) =>
    route.fulfill({ status: 503, json: { code: "unavailable", message: "down" } }),
  );
  const errors = trackErrors(page, /status of 503 \(/);
  await page.goto("/#/settings");
  await expect(page.locator('fs-settings .api[role="alert"]')).toContainText(
    label(labels, "apiUnavailable"),
  );
  expect(errors).toEqual([]);
});

test("the library flow runs in ImageSquid with its own label table (UI-1.1)", async ({ page }) => {
  const errors = trackErrors(page);
  const sent: string[] = [];
  await page.route("**/api/v1/libraries/**", (route) => {
    const url = new URL(route.request().url());
    sent.push(`${route.request().method()} ${url.pathname}`);
    if (url.pathname.endsWith("/location-choices")) {
      return route.fulfill({ json: { outcome: "chosen", location_id: "loc-1" } });
    }
    if (url.pathname.endsWith("/repair-candidates")) {
      return route.fulfill({ json: { candidates: [], truncated: false } });
    }
    return route.fulfill({ json: { library_id: "lib-1", spec_version: "1.1", objects: 4 } });
  });
  await page.goto("/#/library");
  const start = page.locator("fs-library-start");
  await expect(start.locator(".page > h2")).toHaveText(label(labels, "libraryWelcomeTitle"));
  await expect(start.locator(".eyebrow")).toHaveText(label(labels, "libraries"));
  await expect(start.locator("section[data-flow] button")).toHaveText([
    label(labels, "libraryCreate"),
    label(labels, "libraryOpen"),
    label(labels, "libraryRecover"),
  ]);
  await start.locator('button[data-action="open"]').click();
  await expect(page).toHaveURL(/#\/library\/lib-1$/);
  await expect(page.locator("fs-library-detail .page > h2")).toHaveText(label(labels, "library"));
  expect(sent.slice(0, 2)).toEqual([
    "POST /api/v1/libraries/location-choices",
    "POST /api/v1/libraries/openings",
  ]);
  expect(errors).toEqual([]);
});
