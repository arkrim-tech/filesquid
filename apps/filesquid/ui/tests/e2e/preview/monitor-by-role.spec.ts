/**
 * Production-artifact proof of the Monitor link and route by role (UI-F2.21; UI-S-032,
 * UI-S-026, UI-S-021; PRD F-061): the FileSquid UI built by `vite build` and served by
 * `vite preview`, with the host, the navigation tree and the admin routes answered by the test in
 * the shapes of the API contract. The page holds a credential (the desktop launch token global),
 * so the shell asks the admin health route once whether the caller may open the admin views.
 * With a 403 role refusal the header shows no Monitor button, the Logs row is hidden with it and
 * `#/monitor` shows `stateDenied` with no further admin call; with the owner's credential the
 * button and the dashboard are shown as before. Strict DOM assertions: custom elements
 * defined, every word from the active label table, computed token values in light and dark, no
 * console errors other than the browser's own line for the deliberate 403.
 * Owner decision 2026-10-31, verbatim: "accept recommendations".
 */
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
import { COLOURS, look } from "../support/fidelity";
import { serveNavigation } from "../support/navigation-api";

const labels = activeLabels();
const SCHEMES: readonly Scheme[] = ["light", "dark"];
/** A credential for the probe; any value, the test answers every route itself. */
const TOKEN = "e2e0".repeat(16);

/** The browser's own console line for the deliberate 403 of the admin health route. */
const refusedProbe = (text: string, url: string) =>
  /status of 403 \(/.test(text) && /\/api\/v1\/admin\/health$/.test(url);

/** Give the page the desktop launch token, so the shell holds a credential to probe with. */
async function withCredential(page: Page): Promise<void> {
  await page.addInitScript((value) => {
    Object.defineProperty(window, "__FILESQUID_LAUNCH_TOKEN__", { value, enumerable: false });
  }, TOKEN);
}

/**
 * Answer the shell's routes and every admin route; `refuse` makes each admin route answer the
 * 403 role refusal of API-S-004.
 *
 * @returns The paths of the admin requests the page made, in order.
 */
async function serve(page: Page, refuse: boolean): Promise<string[]> {
  const admin: string[] = [];
  await serveNavigation(page);
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
  await page.route(/\/api\/v1\/admin\//, (route) => {
    const url = new URL(route.request().url());
    admin.push(url.pathname);
    if (refuse) {
      const body = { code: "forbidden", reason: "not_permitted", retryable: false };
      return route.fulfill({ status: 403, json: body });
    }
    return route.fulfill({ json: adminAnswer(url.pathname) });
  });
  return admin;
}

/** The owner's answer of each admin route the Monitor reads. */
function adminAnswer(path: string): unknown {
  if (path.endsWith("/admin/health")) {
    const checks = {
      stalled: 0,
      store_ok: true,
      data_dir_writable: true,
      free_bytes: 412 * 1024 ** 3,
      queue_fill: 0,
      errors_last_5m: 0,
      ocfl_error_codes: [],
      ocfl_error_objects: 0,
    };
    return { status: "ok", reasons: [], checks };
  }
  if (path.endsWith("/admin/processes")) return [];
  if (path.endsWith("/admin/journal")) return { entries: [], next_cursor: null };
  if (path.endsWith("/admin/ocfl/findings")) {
    const settings = { interval_ms: 604_800_000, object_cap: 100_000 };
    return { findings: [], truncated: false, settings, libraries: [] };
  }
  return {};
}

/** Wait until the host badge has read the host, so the shell has rendered its rows. */
async function shellReady(page: Page): Promise<void> {
  await expect(page.locator("fs-app-shell fs-host-indicator")).toHaveAttribute(
    "data-state",
    "ready",
  );
}

for (const scheme of SCHEMES) {
  const c = COLOURS[scheme];

  test.describe(`${scheme} theme`, () => {
    test.beforeEach(async ({ page }) => {
      await setScheme(page, scheme);
      await withCredential(page);
    });

    test("a caller the admin routes refuse sees no Monitor button and no Logs row", async ({
      page,
    }) => {
      const errors = trackErrors(page, refusedProbe);
      const admin = await serve(page, true);
      await page.goto("/");
      await shellReady(page);
      await expectElementsDefined(page, ["fs-app-shell", "fs-host-indicator"]);
      const shell = page.locator("fs-app-shell");
      await expect(shell.locator("header .monitor")).toBeHidden();
      await expect(shell.locator("nav li.logs")).toBeHidden();
      await expect(shell.locator("nav li.settings a")).toHaveText(label(labels, "settings"));
      expect(admin).toEqual(["/api/v1/admin/health"]);
      expect(errors).toEqual([]);
    });

    test("a direct visit to #/monitor shows stateDenied with no further admin call", async ({
      page,
    }) => {
      const errors = trackErrors(page, refusedProbe);
      const admin = await serve(page, true);
      await page.goto("/#/monitor");
      await shellReady(page);
      await expectElementsDefined(page, ["fs-monitor", "fs-states"]);
      const states = page.locator("fs-monitor fs-states");
      await expect(states).toHaveAttribute("data-state", "denied");
      await expect(states.locator(".title")).toHaveText(label(labels, "stateDenied"));
      await expect(states.locator(".detail")).toHaveText(label(labels, "authNotPermitted"));
      expect(await look(states.locator(".title"))).toMatchObject({
        colour: rgbOf(c.body),
        weight: "600",
      });
      expect((await look(states.locator(".detail"))).colour).toBe(rgbOf(c.variant));
      await expect(page.locator("fs-monitor form[data-form='token']")).toHaveCount(0);
      await expect(page.locator("fs-monitor [data-section]")).toHaveCount(0);
      await expect(page.locator("fs-app-shell header .monitor")).toBeHidden();
      // Wait past a poll interval: the denied Monitor reads nothing more.
      await page.waitForTimeout(6000);
      expect(admin).toEqual(["/api/v1/admin/health"]);
      expect(errors).toEqual([]);
    });

    test("the owner sees the Monitor button and the dashboard as before", async ({ page }) => {
      const errors = trackErrors(page);
      await serve(page, false);
      await page.goto("/");
      await shellReady(page);
      const shell = page.locator("fs-app-shell");
      const button = shell.locator("header .monitor");
      await expect(button).toBeVisible();
      await expect(button).toHaveText(label(labels, "monitor"));
      expect(await look(button)).toMatchObject({
        colour: rgbOf(c.body),
        background: rgbOf(c.card),
        borderColour: rgbOf(c.strong),
        weight: "600",
        radius: "7px",
      });
      await expect(shell.locator("nav li.logs")).toBeVisible();
      await button.click();
      await expect(page).toHaveURL(/#\/monitor$/);
      await expect(button).toHaveAttribute("aria-current", "page");
      const head = page.locator("fs-monitor .page-head");
      await expect(head.locator("h2")).toHaveText(label(labels, "monitor"));
      await expect(head.locator(".owner")).toHaveText(label(labels, "monitorOwnerOnly"));
      await expect(page.locator('fs-monitor [data-section="health"]')).toHaveAttribute(
        "data-status",
        "ok",
      );
      await expect(page.locator("fs-monitor fs-states[data-state='denied']")).toHaveCount(0);
      expect(errors).toEqual([]);
    });
  });
}
