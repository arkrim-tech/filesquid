/**
 * Production-artifact proof of the Monitor's OCFL validation findings (UI-F2.14, API-S-022,
 * SRS-F-061): the FileSquid UI built by `vite build` and served by `vite preview`, with the
 * admin routes, the findings and run routes and the event channel answered by the test in the
 * shapes of the API contract (`FindingsView`, `RunAccepted`, `HealthReport`). It covers the
 * plan row's acceptance criteria: (1) an E023 finding with its code and the degraded health
 * reason, (2) the finding gone after a clean revalidation's `health.change`, (3) a W010 warning
 * listed apart from errors with health still `ok`, (4) no location rendered when the API
 * withholds it, (5) the last and next run times from the findings route. Strict DOM
 * assertions: custom elements defined, every word from the active label table, computed token
 * values in light and dark, no console errors. The server-backed proof against the real routes
 * is `tests/e2e/ocfl-findings.spec.ts`.
 */
import { expect, test, type Locator, type Page, type WebSocketRoute } from "@playwright/test";
import {
  activeLabels,
  expectElementsDefined,
  label,
  rgbOf,
  setScheme,
  tokenColour,
  trackErrors,
} from "../support/strict-dom";
import { serveNavigation } from "../support/navigation-api";

const labels = activeLabels();
const LIB = "5e2e0000-0000-4000-8000-00000000e2e0";
const OBJECT = "urn:uuid:7c1e0000-0000-4000-8000-000000000001";
const LOCATION = "7c1/v2/content/stray.bin";

/** The health checks of a report; `codes` are the open OCFL error codes. */
const checks = (codes: string[]) => ({
  stalled: 0,
  store_ok: true,
  data_dir_writable: true,
  free_bytes: 50 * 1024 ** 3,
  queue_fill: 0,
  errors_last_5m: 0,
  ocfl_error_codes: codes,
  ocfl_error_objects: codes.length > 0 ? 1 : 0,
});

/** A health report: degraded with the OCFL reason while `codes` are open, else ok. */
function healthReport(codes: string[]) {
  if (codes.length === 0) return { status: "ok", reasons: [], checks: checks([]) };
  const reason = {
    code: "ocfl_validation_errors",
    severity: "degraded",
    detail: `OCFL validation errors (${codes.join(", ")}) in 1 object(s)`,
    codes,
    objects: 1,
  };
  return { status: "degraded", reasons: [reason], checks: checks(codes) };
}

/** One finding of the fixture library. */
function finding(code: string, location: string | null) {
  const severity = code.startsWith("E") ? "error" : "warning";
  return { library_id: LIB, object_id: OBJECT, version: "v2", code, severity, location };
}

/** A findings answer for the fixture library, which last ran at 10:00 and runs again in 7 days. */
function findingsView(findings: object[]) {
  return {
    findings,
    truncated: false,
    settings: { interval_ms: 604_800_000, object_cap: 100_000 },
    libraries: [
      {
        library_id: LIB,
        running: false,
        last_run: {
          trigger: "scheduled",
          started: "2026-10-20T10:00:00Z",
          ended: "2026-10-20T10:00:04Z",
          outcome: "completed",
          checked: 3,
          not_checked: 0,
          error_code: null,
        },
        next_run: "2026-10-27T10:00:00Z",
      },
    ],
  };
}

/** What the test controls: the findings and health answers, run requests and the channel. */
interface Api {
  /** Answer `findings` and the matching health from now on. */
  set(findings: object[], codes: string[]): void;
  /** Bodies of every `POST /api/v1/admin/ocfl/validation-runs`. */
  runs: unknown[];
  /** Number of findings reads so far. */
  reads: () => number;
  /** Push one event down every open event channel. */
  send(event: object): void;
}

/**
 * Answer the shell's routes, the Monitor's admin routes, the findings and run routes and the
 * event channel.
 *
 * @param page - The page.
 * @param findings - The first findings answer.
 * @param codes - The OCFL error codes the first health answer reports.
 * @returns The controls of {@link Api}.
 */
async function serve(page: Page, findings: object[], codes: string[]): Promise<Api> {
  let current = { findings, codes };
  let reads = 0;
  const runs: unknown[] = [];
  const sockets: WebSocketRoute[] = [];
  await serveNavigation(page);
  await page.route("**/api/v1/health", (route) =>
    route.fulfill({ json: { schema_version: 1, status: "ok", product: "filesquid" } }),
  );
  await page.route("**/api/v1/host", (route) =>
    route.fulfill({
      json: {
        schema_version: 1,
        host_type: "server",
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
  await page.route("**/api/v1/admin/health", (route) =>
    route.fulfill({ json: healthReport(current.codes) }),
  );
  await page.route("**/api/v1/admin/processes", (route) => route.fulfill({ json: [] }));
  await page.route("**/api/v1/admin/metrics", (route) => route.fulfill({ json: {} }));
  await page.route(/\/api\/v1\/admin\/journal(\?|$)/, (route) =>
    route.fulfill({ json: { entries: [], next_cursor: null } }),
  );
  await page.route("**/api/v1/admin/ocfl/findings", (route) => {
    reads += 1;
    return route.fulfill({ json: findingsView(current.findings) });
  });
  await page.route("**/api/v1/admin/ocfl/validation-runs", (route) => {
    runs.push(route.request().postDataJSON());
    return route.fulfill({ json: { started: [LIB], refused: [] } });
  });
  await page.routeWebSocket(/\/api\/v1\/events$/, (ws) => void sockets.push(ws));
  return {
    set: (next, nextCodes) => (current = { findings: next, codes: nextCodes }),
    runs,
    reads: () => reads,
    send: (event) => sockets.forEach((ws) => ws.send(JSON.stringify(event))),
  };
}

/** The Monitor's OCFL validation element. */
const findingsEl = (page: Page): Locator => page.locator("fs-monitor fs-ocfl-findings");

/** The cells of the body rows of one table part, as text. */
async function cells(page: Page, part: string): Promise<string[][]> {
  const rows = findingsEl(page).locator(`[data-part="${part}"] tbody tr`);
  return rows.evaluateAll((list) =>
    list.map((row) => [...row.querySelectorAll("td")].map((td) => td.textContent ?? "")),
  );
}

/** Open the Monitor and wait until the findings section has read its route. */
async function openMonitor(page: Page): Promise<void> {
  await page.goto("/#/monitor");
  await expect(findingsEl(page)).toHaveAttribute("data-state", "ready");
}

test("(1) an E023 finding shows with its code beside the degraded health reason", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await serve(page, [finding("E023", LOCATION)], ["E023"]);
  await openMonitor(page);
  await expectElementsDefined(page, ["fs-monitor", "fs-ocfl-findings", "fs-states"]);
  const health = page.locator('fs-monitor [data-section="health"]');
  await expect(health).toHaveAttribute("data-status", "degraded");
  await expect(health.locator(".status")).toHaveText(label(labels, "statusDegraded"));
  const reason = label(labels, "healthOcflErrors")
    .replace("{codes}", "E023")
    .replace("{objects}", "1");
  await expect(health.locator('li[data-reason="ocfl_validation_errors"]')).toHaveText(reason);
  const section = page.locator('fs-monitor [data-section="ocfl"]');
  await expect(section.locator("h2")).toHaveText(label(labels, "monitorFindings"));
  await expect(findingsEl(page).locator("h3")).toHaveText([
    label(labels, "findingsRuns"),
    label(labels, "findingsErrors"),
    label(labels, "findingsWarnings"),
  ]);
  expect(await cells(page, "errors")).toEqual([[LIB, OBJECT, "v2", "E023", LOCATION]]);
  expect(await cells(page, "warnings")).toEqual([[label(labels, "findingsNoWarnings")]]);
  expect(errors).toEqual([]);
});

test("(2) the finding disappears after a clean revalidation's health.change, without a reload", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await serve(page, [finding("E023", LOCATION)], ["E023"]);
  await openMonitor(page);
  expect(await cells(page, "errors")).toHaveLength(1);
  const reads = api.reads();
  api.set([], []);
  await page.evaluate(() => ((window as unknown as { marker: number }).marker = 1));
  api.send({ topic: "health.change", data: healthReport([]) });
  const health = page.locator('fs-monitor [data-section="health"]');
  await expect(health).toHaveAttribute("data-status", "ok");
  await expect.poll(() => cells(page, "errors")).toEqual([[label(labels, "findingsNoErrors")]]);
  expect(api.reads()).toBeGreaterThan(reads);
  expect(await page.evaluate(() => (window as unknown as { marker?: number }).marker)).toBe(1);
  await expect(health.locator("li")).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("(3) a W010 warning is listed apart from errors and health stays ok", async ({ page }) => {
  const errors = trackErrors(page);
  await serve(page, [finding("W010", LOCATION)], []);
  await openMonitor(page);
  await expect(page.locator('fs-monitor [data-section="health"]')).toHaveAttribute(
    "data-status",
    "ok",
  );
  expect(await cells(page, "errors")).toEqual([[label(labels, "findingsNoErrors")]]);
  expect((await cells(page, "warnings")).map((row) => row[3])).toEqual(["W010"]);
  const note = findingsEl(page).locator('[data-note="read-only"]');
  // The app wires `repairHref` (UI-1.1), so the note ends with the link to the repair offer.
  const said = label(labels, "findingsReadOnly").replace("{count}", "1").replace("{name}", LIB);
  await expect(note).toHaveText(`${said} ${label(labels, "findingsRepair")}`);
  await expect(note.locator("a")).toHaveAttribute("href", `#/library/${LIB}/repair`);
  expect(errors).toEqual([]);
});

test("(4) no location is rendered when the API withholds it", async ({ page }) => {
  const errors = trackErrors(page);
  await serve(page, [finding("E023", null)], ["E023"]);
  await openMonitor(page);
  const cell = findingsEl(page).locator('[data-part="errors"] td[data-withheld]');
  await expect(cell).toHaveText(label(labels, "findingsLocationHidden"));
  const text = await findingsEl(page).evaluate((el) => el.shadowRoot?.textContent ?? "");
  expect(text).not.toContain("content/");
  expect(text).not.toContain("stray.bin");
  expect(errors).toEqual([]);
});

test("(5) the last and next run times come from the findings route; Validate now starts a run", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const api = await serve(page, [], []);
  await openMonitor(page);
  await expect(findingsEl(page).locator('[data-part="runs"] th')).toHaveText([
    label(labels, "library"),
    label(labels, "findingsState"),
    label(labels, "findingsLastStarted"),
    label(labels, "findingsLastEnded"),
    label(labels, "findingsOutcome"),
    label(labels, "findingsChecked"),
    label(labels, "findingsNotChecked"),
    label(labels, "findingsNextRun"),
  ]);
  expect(await cells(page, "runs")).toEqual([
    [
      LIB,
      label(labels, "findingsIdle"),
      "2026-10-20T10:00:00Z",
      "2026-10-20T10:00:04Z",
      label(labels, "runCompleted"),
      "3",
      "0",
      "2026-10-27T10:00:00Z",
    ],
  ]);
  const reads = api.reads();
  const button = findingsEl(page).getByRole("button", { name: label(labels, "findingsValidate") });
  await button.click();
  await expect(findingsEl(page).locator('[data-part="run-result"]')).toHaveText(
    label(labels, "findingsRunResult").replace("{started}", "1").replace("{refused}", "0"),
  );
  expect(api.runs).toEqual([{}]);
  await expect.poll(() => api.reads()).toBeGreaterThan(reads);
  expect(errors).toEqual([]);
});

test("error and warning codes take their colours from tokens in light and dark", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await serve(page, [finding("E023", LOCATION), finding("W010", null)], ["E023"]);
  const colours = async () =>
    findingsEl(page).evaluate((el) => {
      const colour = (selector: string) => {
        const node = el.shadowRoot?.querySelector(selector);
        return node ? getComputedStyle(node).color : "";
      };
      return {
        error: colour('[data-part="errors"] td.code'),
        warning: colour('[data-part="warnings"] td.code'),
      };
    });
  for (const [scheme, error, variant] of [
    ["light", "#c62828", "#555555"],
    ["dark", "#ef5350", "#a0a0a0"],
  ] as const) {
    await setScheme(page, scheme);
    await openMonitor(page);
    expect(await tokenColour(page, "--fs-error")).toBe(rgbOf(error));
    expect(await colours()).toEqual({ error: rgbOf(error), warning: rgbOf(variant) });
  }
  expect(errors).toEqual([]);
});
