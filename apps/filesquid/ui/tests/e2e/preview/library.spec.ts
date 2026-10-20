/**
 * Production-artifact proof of the library create, open and recover UI (UI-1.1, SRS-F-001,
 * API-S-023, API-S-025, API-S-034): the FileSquid UI built by `vite build` and served by
 * `vite preview`, with the library routes answered by the test in the shapes of the API
 * contract. It covers the plan row's acceptance criteria: (1) a chosen folder answered as a
 * location ID and passed on to create, open, recover and copy, (2) cancel changing nothing,
 * (3) `503 unavailable` shown as the unavailable state with its reason and a Retry while the
 * choices stay, (4) the consent dialog's confirm, cancel and a refused repair, (5) the Monitor's
 * W010 link reaching the per-library repair offer. Strict DOM assertions: custom elements
 * defined, every word from the active label table, computed token values in light and dark, no
 * console errors, and no filesystem path in any request or in the page.
 */
import { expect, test, type Locator, type Page } from "@playwright/test";
import { LIB, OBJECTS, serveLibraries } from "../support/library-api";
import {
  activeLabels,
  expectElementsDefined,
  label,
  setScheme,
  tokenColour,
  trackErrors,
} from "../support/strict-dom";

const labels = activeLabels();
const text = (key: string) => label(labels, key);

/** Console lines the browser prints for the refusals these tests provoke on purpose. */
const REFUSED = /status of (409|503)/;

/** A path-like string: a drive letter, or a rooted folder of a user or volume. */
const PATH = /[A-Za-z]:\\|\/Users\/|\/home\/|\/Volumes\//;

const start = (page: Page): Locator => page.locator("fs-library-start");
const detail = (page: Page): Locator => page.locator("fs-library-detail");
const repair = (page: Page): Locator => page.locator("fs-library-repair");
const flow = (page: Page, name: string): Locator =>
  start(page).locator(`button[data-action="${name}"]`);

/** Open the start page. */
async function openStart(page: Page): Promise<void> {
  await page.goto("/#/library");
  await expect(start(page).locator("h2")).toHaveText(text("libraries"));
}

test("the start page offers create, open and recover in the active vocabulary", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await serveLibraries(page);
  await openStart(page);
  await expectElementsDefined(page, [
    "fs-library-start",
    "fs-library-detail",
    "fs-library-repair",
    "fs-states",
  ]);
  await expect(start(page).locator(".intro")).toHaveText(text("libraryIntro"));
  await expect(start(page).locator("section[data-flow] h3")).toHaveText([
    text("libraryCreateTitle"),
    text("libraryOpenTitle"),
    text("libraryRecoverTitle"),
  ]);
  await expect(start(page).locator("section[data-flow] button")).toHaveText([
    text("libraryCreate"),
    text("libraryOpen"),
    text("libraryRecover"),
  ]);
  await expect(page.locator("fs-app-shell nav > ul.tools a").first()).toHaveText(
    text("libraryNavLink"),
  );
  expect(errors).toEqual([]);
});

test("(1) create asks for a folder, passes the location ID on and shows the library, with no path", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await serveLibraries(page);
  await openStart(page);
  await flow(page, "create").click();
  await expect(page).toHaveURL(new RegExp(`#/library/${LIB}$`));
  await expect(detail(page).locator('[data-section="facts"] dd')).toHaveText([
    LIB,
    text("libraryFactFormatValue").replace("{version}", "1.1"),
    "2",
  ]);
  expect(api.seen.slice(0, 2).map(({ path, method, body }) => [method, path, body])).toEqual([
    ["POST", "/api/v1/libraries/location-choices", { purpose: "create" }],
    ["POST", "/api/v1/libraries", { location_id: "loc-create" }],
  ]);
  expect(JSON.stringify(api.seen)).not.toMatch(PATH);
  expect(await page.locator("body").innerText()).not.toMatch(PATH);
  expect(errors).toEqual([]);
});

test("(1) open passes the location ID to the open route", async ({ page }) => {
  const errors = trackErrors(page);
  const api = await serveLibraries(page);
  await openStart(page);
  await flow(page, "open").click();
  await expect(page).toHaveURL(new RegExp(`#/library/${LIB}$`));
  expect(api.seen.map(({ path, body }) => [path, body])).toContainEqual([
    "/api/v1/libraries/openings",
    { location_id: "loc-open" },
  ]);
  expect(api.to("/api/v1/libraries")).toEqual([]);
  expect(errors).toEqual([]);
});

test("(1) recover opens the copy, rebuilds from files and reports without repairing", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await serveLibraries(page);
  await openStart(page);
  await flow(page, "recover").click();
  await expect(page).toHaveURL(new RegExp(`#/library/${LIB}$`));
  expect(api.seen.map(({ path }) => path)).toEqual([
    "/api/v1/libraries/location-choices",
    "/api/v1/libraries/openings",
    "/api/v1/libraries/rebuilds",
    "/api/v1/libraries/repair-candidates",
  ]);
  const report = detail(page).locator('[data-result="rebuild"]');
  await expect(report.locator("p")).toHaveText(
    text("libraryRebuilt")
      .replace("{objects}", "2")
      .replace("{unreadable}", "0")
      .replace("{invalid}", "1"),
  );
  await expect(report.locator("li")).toHaveText(
    text("libraryRebuiltInvalid")
      .replace("{object}", OBJECTS[1] ?? "")
      .replace("{codes}", "E023"),
  );
  await expect(detail(page).locator('[data-section="repair-note"] p')).toHaveText(
    text("libraryReadOnly").replace("{count}", "2"),
  );
  expect(api.to("/api/v1/libraries/repairs")).toEqual([]);
  expect(errors).toEqual([]);
});

test("(1) copy asks for a destination and passes it as destination_location_id", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await serveLibraries(page);
  await page.goto(`/#/library/${LIB}`);
  await detail(page).locator('button[data-action="copy"]').click();
  const result = detail(page).locator('[data-result="copy"]');
  await expect(result.locator("p").first()).toContainText("2");
  expect(api.to("/api/v1/libraries/location-choices").map(({ body }) => body)).toEqual([
    { purpose: "copy_destination" },
  ]);
  expect(api.to("/api/v1/libraries/snapshot-copies").map(({ body }) => body)).toEqual([
    { library_id: LIB, destination_location_id: "loc-copy_destination" },
  ]);
  await expect(result.locator("li")).toHaveText(
    text("libraryCopyInvalid")
      .replace("{object}", OBJECTS[1] ?? "")
      .replace("{codes}", "E023"),
  );
  expect(errors).toEqual([]);
});

test("(2) a cancelled folder choice sends no library request and says nothing changed", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await serveLibraries(page);
  api.chooser = "cancelled";
  await openStart(page);
  await flow(page, "create").click();
  await expect(start(page).locator(".feedback > p.status")).toHaveText(text("libraryCancelled"));
  expect(api.seen.map(({ path }) => path)).toEqual(["/api/v1/libraries/location-choices"]);
  await expect(page).toHaveURL(/#\/library$/);
  expect(errors).toEqual([]);
});

test("(3) 503 unavailable is the unavailable state with its reason and a Retry, and the choices stay", async ({
  page,
}) => {
  const errors = trackErrors(page, REFUSED);
  const api = await serveLibraries(page);
  api.chooser = "unavailable";
  await openStart(page);
  await flow(page, "open").click();
  const states = start(page).locator("fs-states");
  await expect(states).toHaveAttribute("data-state", "unavailable");
  await expect(states.locator(".title")).toHaveText(text("stateUnavailable"));
  await expect(states.locator(".detail")).toHaveText(text("libraryChooserUnavailable"));
  await expect(states.getByRole("button", { name: text("stateRetry") })).toBeVisible();
  for (const name of ["create", "open", "recover"]) await expect(flow(page, name)).toBeVisible();
  api.chooser = "chosen";
  await states.getByRole("button", { name: text("stateRetry") }).click();
  await expect(page).toHaveURL(new RegExp(`#/library/${LIB}$`));
  expect(api.seen.filter(({ path }) => path.endsWith("/location-choices"))).toHaveLength(2);
  expect(errors).toEqual([]);
});

/** Open the repair offer of the fixture library and wait for its list. */
async function openRepair(page: Page): Promise<void> {
  await page.goto(`/#/library/${LIB}/repair`);
  await expect(repair(page).locator('[data-list="objects"] li')).toHaveCount(2);
}

test("(4) confirm sends the consent for exactly the named objects and shows each result", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await serveLibraries(page);
  await openRepair(page);
  await repair(page).locator('button[data-action="review"]').click();
  const dialog = repair(page).locator('[role="dialog"]');
  await expect(dialog).toHaveAttribute("aria-modal", "true");
  await expect(dialog.locator("h3")).toHaveText(text("repairDialogTitle").replace("{count}", "2"));
  await expect(dialog.locator(".consent")).toHaveText(text("repairConsent"));
  await expect(dialog.locator("li")).toHaveText(OBJECTS);
  expect(api.to("/api/v1/libraries/repairs")).toEqual([]);
  await dialog.locator('button[data-action="confirm"]').click();
  await expect.poll(() => api.to("/api/v1/libraries/repairs").length).toBe(1);
  expect(api.to("/api/v1/libraries/repairs").map(({ body }) => body)).toEqual([
    {
      library_id: LIB,
      object_ids: OBJECTS,
      consent: {
        library_id: LIB,
        object_ids: OBJECTS,
        statement_id: "version_inventory_repair.v1",
        confirmed: true,
      },
    },
  ]);
  const results = repair(page).locator('[data-section="results"] li');
  await expect(results).toHaveText(
    OBJECTS.map((object) =>
      text("repairRepaired").replace("{object}", object).replace("{version}", "v2"),
    ),
  );
  await expect(repair(page).locator("fs-states")).toHaveAttribute("data-state", "empty");
  expect(errors).toEqual([]);
});

test("(4) cancel and Escape send nothing and return focus to the opener", async ({ page }) => {
  const errors = trackErrors(page);
  const api = await serveLibraries(page);
  await openRepair(page);
  const review = repair(page).locator('button[data-action="review"]');
  const dialog = repair(page).locator('[role="dialog"]');
  await review.click();
  await expect(dialog.locator('button[data-action="cancel"]')).toBeFocused();
  await dialog.locator('button[data-action="cancel"]').click();
  await expect(dialog).toHaveCount(0);
  await expect(review).toBeFocused();
  await review.click();
  await page.keyboard.press("Tab");
  await expect(dialog.locator('button[data-action="confirm"]')).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(dialog.locator('button[data-action="cancel"]')).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(review).toBeFocused();
  expect(api.to("/api/v1/libraries/repairs")).toEqual([]);
  expect(errors).toEqual([]);
});

test("(4) a refused repair says nothing was changed and shows no result", async ({ page }) => {
  const errors = trackErrors(page, REFUSED);
  const api = await serveLibraries(page);
  api.repairRefusal = { status: 409, code: "conflict" };
  await openRepair(page);
  await repair(page).locator('button[data-action="review"]').click();
  await repair(page).locator('button[data-action="confirm"]').click();
  await expect(repair(page).locator(".feedback > p.alert")).toHaveText(
    text("repairRequestRefusedConflict"),
  );
  await expect(repair(page).locator('[data-section="results"]')).toHaveCount(0);
  await expect(repair(page).locator('[data-list="objects"] li')).toHaveCount(2);
  expect(errors).toEqual([]);
});

test("(5) the Monitor's W010 note links to the library's repair offer", async ({ page }) => {
  const errors = trackErrors(page);
  await serveLibraries(page);
  await serveMonitor(page);
  await page.goto("/#/monitor");
  const link = page.locator("fs-monitor fs-ocfl-findings").locator('[data-note="read-only"] a');
  await expect(link).toHaveAttribute("href", `#/library/${LIB}/repair`);
  await link.click();
  await expect(page).toHaveURL(new RegExp(`#/library/${LIB}/repair$`));
  await expect(repair(page).locator('[data-list="objects"] li')).toHaveCount(2);
  expect(errors).toEqual([]);
});

/** Answer the Monitor's routes with one W010 warning for the fixture library. */
async function serveMonitor(page: Page): Promise<void> {
  const checks = {
    stalled: 0,
    store_ok: true,
    data_dir_writable: true,
    free_bytes: 50 * 1024 ** 3,
    queue_fill: 0,
    errors_last_5m: 0,
    ocfl_error_codes: [],
    ocfl_error_objects: 0,
  };
  await page.route("**/api/v1/admin/health", (route) =>
    route.fulfill({ json: { status: "ok", reasons: [], checks } }),
  );
  await page.route("**/api/v1/admin/processes", (route) => route.fulfill({ json: [] }));
  await page.route("**/api/v1/admin/metrics", (route) => route.fulfill({ json: {} }));
  await page.route(/\/api\/v1\/admin\/journal(\?|$)/, (route) =>
    route.fulfill({ json: { entries: [], next_cursor: null } }),
  );
  const warning = {
    library_id: LIB,
    object_id: OBJECTS[0],
    version: "v2",
    code: "W010",
    severity: "warning",
    location: null,
  };
  await page.route("**/api/v1/admin/ocfl/findings", (route) =>
    route.fulfill({
      json: {
        findings: [warning],
        truncated: false,
        settings: { interval_ms: 604_800_000, object_cap: 100_000 },
        libraries: [{ library_id: LIB, running: false, last_run: null, next_run: null }],
      },
    }),
  );
}

test("colours come from tokens in light and dark", async ({ page }) => {
  const errors = trackErrors(page, REFUSED);
  const api = await serveLibraries(page);
  api.chooser = "unavailable";
  for (const scheme of ["light", "dark"] as const) {
    await setScheme(page, scheme);
    await openRepair(page);
    await repair(page).locator('button[data-action="review"]').click();
    const colours = await repair(page).evaluate((el) => {
      const root = el.shadowRoot as ShadowRoot;
      const style = (selector: string) => getComputedStyle(root.querySelector(selector) as Element);
      return {
        heading: style("h2").color,
        card: style("section").backgroundColor,
        dialog: style('[role="dialog"]').backgroundColor,
        confirmBackground: style("button.primary").backgroundColor,
        confirmText: style("button.primary").color,
        cancelBorder: style('button[data-action="cancel"]').borderTopColor,
      };
    });
    expect(colours).toEqual({
      heading: await tokenColour(page, "--fs-on-surface"),
      card: await tokenColour(page, "--fs-surface-container"),
      dialog: await tokenColour(page, "--fs-surface"),
      confirmBackground: await tokenColour(page, "--fs-primary"),
      confirmText: await tokenColour(page, "--fs-on-primary"),
      cancelBorder: await tokenColour(page, "--fs-primary"),
    });
    await page.goto("/#/library");
    await flow(page, "create").waitFor();
    await flow(page, "open").click();
    const alert = start(page).locator("fs-states .title");
    await expect(alert).toBeVisible();
    const error = await alert.evaluate((node) => getComputedStyle(node).color);
    expect(error).toBe(await tokenColour(page, "--fs-error"));
  }
  expect(errors).toEqual([]);
});
