/**
 * End-to-end: the Monitor's OCFL validation findings (UI-F2.14, API-S-022) in the BUILT
 * FileSquid UI served by the real `filesquid-server`, reading the real
 * `GET /api/v1/admin/ocfl/findings` and starting a run through the real
 * `POST /api/v1/admin/ocfl/validation-runs`. The server opens no OCFL library until its
 * libraries arrive (SV-F1.2), so it answers no findings and no library runs; the E023, W010,
 * withheld-location and run-time cases are proven against the production build with
 * contract-shaped answers in `preview/ocfl-findings.spec.ts`. Strict DOM assertions: elements
 * defined, words from the active label table, no console errors beyond the refused first call.
 */
import { expect, test, type Page } from "@playwright/test";
import { ADMIN_TOKEN } from "./admin-token";
import { activeLabels, expectElementsDefined, label, trackErrors } from "./support/strict-dom";

const labels = activeLabels();

/** The browser's own console line for the Monitor's first, unauthenticated call. */
const REFUSED_CALL = /status of 401 \(|WebSocket connection to .* failed/;

/** Open the Monitor and sign in with the administrator token. */
async function signIn(page: Page): Promise<void> {
  await page.goto("/#/monitor");
  const form = page.locator("fs-monitor form[data-form='token']");
  await form.locator("input").fill(ADMIN_TOKEN);
  await form.locator("button").click();
}

/** The cells of the body rows of one table part, as text. */
async function cells(page: Page, part: string): Promise<string[][]> {
  const rows = page.locator(`fs-monitor fs-ocfl-findings [data-part="${part}"] tbody tr`);
  return rows.evaluateAll((list) =>
    list.map((row) => [...row.querySelectorAll("td")].map((td) => td.textContent ?? "")),
  );
}

test("the findings section reads the real findings route after sign-in", async ({ page }) => {
  const errors = trackErrors(page, REFUSED_CALL);
  const answer = page.waitForResponse(
    (response) =>
      response.url().endsWith("/api/v1/admin/ocfl/findings") && response.status() === 200,
  );
  await signIn(page);
  await answer;
  const findings = page.locator("fs-monitor fs-ocfl-findings");
  await expect(findings).toHaveAttribute("data-state", "ready");
  await expectElementsDefined(page, ["fs-monitor", "fs-ocfl-findings", "fs-states"]);
  await expect(page.locator('fs-monitor [data-section="ocfl"] h2')).toHaveText(
    label(labels, "monitorFindings"),
  );
  expect(await cells(page, "runs")).toEqual([[label(labels, "findingsNoLibraries")]]);
  expect(await cells(page, "errors")).toEqual([[label(labels, "findingsNoErrors")]]);
  expect(await cells(page, "warnings")).toEqual([[label(labels, "findingsNoWarnings")]]);
  // With no OCFL error finding open, the server reports no ocfl_validation_errors reason.
  await expect(
    page.locator('fs-monitor [data-section="health"] li[data-reason="ocfl_validation_errors"]'),
  ).toHaveCount(0);
  const schedule = findings.locator('[data-part="schedule"]');
  await expect(schedule.locator("dt")).toHaveText([
    label(labels, "findingsSchedule"),
    label(labels, "findingsCap"),
  ]);
  await expect(schedule.locator("dd").first()).toHaveText(/^Every [0-9.]+ h$/);
  await expect(schedule.locator("dd").last()).toHaveText(/^[0-9]+$/);
  expect(errors).toEqual([]);
});

test("Validate now starts runs through the real run route and says nothing was open", async ({
  page,
}) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await signIn(page);
  const findings = page.locator("fs-monitor fs-ocfl-findings");
  await expect(findings).toHaveAttribute("data-state", "ready");
  const posted = page.waitForResponse(
    (response) =>
      response.url().endsWith("/api/v1/admin/ocfl/validation-runs") &&
      response.request().method() === "POST",
  );
  await findings.getByRole("button", { name: label(labels, "findingsValidate") }).click();
  const response = await posted;
  expect(response.status()).toBe(200);
  expect(await response.json()).toEqual({ started: [], refused: [] });
  await expect(findings.locator('[data-part="run-result"]')).toHaveText(
    label(labels, "findingsNoLibraries"),
  );
  expect(errors).toEqual([]);
});
