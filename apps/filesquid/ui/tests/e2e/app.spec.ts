/**
 * End-to-end: the BUILT FileSquid UI served by the real `filesquid-server`, calling the
 * real, deny-by-default FileSquid API (API-S-004). Strict DOM assertions; any console error
 * fails the test, except the browser's own network lines for a deliberately refused call.
 */
import { expect, test, type Page } from "@playwright/test";
import { ADMIN_TOKEN } from "./admin-token";

/**
 * The browser itself logs a refused fetch or WebSocket upgrade as a console error. Tests that
 * expect a 401 or 403 tolerate exactly those lines and nothing else.
 */
const REFUSED_CALL = /status of 40[13] \(|WebSocket connection to .* failed/;

/** Collect console errors and uncaught page errors for the whole test. */
function trackErrors(page: Page, tolerate?: RegExp): string[] {
  const errors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error" && !tolerate?.test(msg.text())) errors.push(msg.text());
  });
  page.on("pageerror", (err) => errors.push(err.message));
  return errors;
}

/** Define the desktop host's per-launch token global before any page script runs. */
async function injectLaunchToken(page: Page, token: string): Promise<void> {
  await page.addInitScript((value) => {
    Object.defineProperty(window, "__FILESQUID_LAUNCH_TOKEN__", {
      value,
      writable: false,
      enumerable: false,
      configurable: false,
    });
  }, token);
}

/** Sign in on the Monitor's token form. */
async function signIn(page: Page, token: string): Promise<void> {
  const form = page.locator("fs-monitor form[data-form='token']");
  await form.locator("input").fill(token);
  await form.locator("button").click();
}

test("shell mounts with FileSquid brand, internal vocabulary and live API health", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await injectLaunchToken(page, ADMIN_TOKEN);
  await page.goto("/");
  await expect(page).toHaveTitle("FileSquid");
  expect(await page.evaluate(() => customElements.get("fs-app-shell") !== undefined)).toBe(true);

  const shell = page.locator("fs-app-shell");
  await expect(shell.locator("h1")).toHaveText("FileSquid");
  await expect(shell.locator("header img")).toHaveAttribute("alt", "FileSquid logo");
  await expect(shell.locator("nav li")).toHaveText([
    "Libraries",
    "Collections",
    "Virtual collections",
    "Virtual exhibits",
    "Monitor",
  ]);
  await expect(shell.locator('[role="status"]')).toHaveText("API: ok (filesquid)");
  await expect(shell.locator('[role="alert"]')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("design tokens switch between light and dark", async ({ page }) => {
  const background = () => page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  expect(await background()).toBe("rgb(255, 255, 255)");
  await page.emulateMedia({ colorScheme: "dark" });
  expect(await background()).toBe("rgb(16, 19, 27)");
  await page.evaluate(() => document.documentElement.setAttribute("data-theme", "light"));
  expect(await background()).toBe("rgb(255, 255, 255)");
});

test("client-side routes fall back to the distribution index", async ({ page }) => {
  const response = await page.goto("/library/some/deep/link");
  expect(response?.status()).toBe(200);
  await expect(page.locator("fs-app-shell h1")).toHaveText("FileSquid");
});

test("without a token the shell names the missing credentials from the label table", async ({
  page,
}) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await page.goto("/");
  const alert = page.locator('fs-app-shell [role="alert"]');
  await expect(alert).toHaveText("API unavailable: Missing credentials");
  await expect(alert).toHaveAttribute("data-reason", "missing_credentials");
  await expect(page.locator('fs-app-shell [role="status"]')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("Monitor asks for the admin token, then shows live health, processes and the journal", async ({
  page,
}) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await page.goto("/#/monitor");
  const monitor = page.locator("fs-app-shell fs-monitor");
  const form = monitor.locator("form[data-form='token']");
  await expect(form.locator("label")).toHaveText("Admin token");
  await expect(form.locator("input")).toHaveAttribute("type", "password");
  const note = monitor.locator('[data-reason="missing_credentials"]');
  await expect(note).toHaveText("Missing credentials");
  await expect(note).toHaveAttribute("role", "status");
  await signIn(page, ADMIN_TOKEN);

  await expect(monitor.locator('[data-section="health"] h2')).toHaveText("Health");
  await expect(monitor.locator('[data-section="health"]')).toHaveAttribute(
    "data-status",
    /^(ok|degraded|failing)$/,
  );
  const host = monitor.locator('tr[data-process-id^="host-"]');
  await expect(host.locator("td")).toHaveText(["host", "running", "0", "–", "–"]);
  await expect(monitor.locator('[data-section="storage"] dd').first()).toHaveText(/ GiB$/);
  // The data dir persists between runs, so earlier host starts may be listed too.
  await expect(
    monitor
      .locator('[data-section="journal"] tbody tr td:nth-child(2)', { hasText: "host.started" })
      .first(),
  ).toBeVisible();
  // Signing in also reloads the shell's API status with the same in-memory token.
  await expect(page.locator('fs-app-shell [role="status"]')).toHaveText("API: ok (filesquid)");
  expect(errors).toEqual([]);
});

test("a wrong token is named as invalid credentials", async ({ page }) => {
  trackErrors(page, REFUSED_CALL);
  await page.goto("/#/monitor");
  await signIn(page, "f".repeat(64));
  const note = page.locator('fs-monitor [data-reason="invalid_credentials"]');
  await expect(note).toHaveText("Invalid credentials");
  await expect(note).toHaveAttribute("role", "alert");
  await expect(page.locator("fs-monitor form[data-form='token']")).toBeVisible();
});

test("the signed-in token stays in memory: not in storage, URL or DOM, gone on reload", async ({
  page,
}) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await page.goto("/");
  await page.locator("fs-app-shell nav a").click();
  await expect(page).toHaveURL(/#\/monitor$/);
  await signIn(page, ADMIN_TOKEN);
  await expect(page.locator('fs-monitor [data-section="processes"] h2')).toHaveText("Processes");
  await expect(page.locator("fs-monitor form[data-form='token']")).toHaveCount(0);
  const leaks = await page.evaluate((token) => {
    const shadows = [...document.querySelectorAll("fs-app-shell, fs-monitor")]
      .map((el) => el.shadowRoot?.innerHTML ?? "")
      .join("");
    const stored = JSON.stringify({ ...localStorage, ...sessionStorage });
    return [stored, location.href, document.documentElement.outerHTML, shadows].filter((text) =>
      text.includes(token),
    ).length;
  }, ADMIN_TOKEN);
  expect(leaks).toBe(0);

  await page.evaluate(() => (location.hash = ""));
  await expect(page.locator("fs-monitor")).toHaveCount(0);
  await page.evaluate(() => (location.hash = "#/monitor"));
  await expect(page.locator('fs-monitor [data-section="processes"] h2')).toHaveText("Processes");

  await page.reload();
  await expect(page.locator("fs-monitor form[data-form='token']")).toBeVisible();
  expect(errors).toEqual([]);
});

test("the desktop launch token authorises the shell, the Monitor and the event channel", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const upgrades: string[] = [];
  page.on("websocket", (socket) => upgrades.push(socket.url()));
  await injectLaunchToken(page, ADMIN_TOKEN);
  await page.goto("/#/monitor");
  await expect(page.locator('fs-app-shell [role="status"]')).toHaveText("API: ok (filesquid)");
  await expect(page.locator('fs-monitor [data-section="health"] h2')).toHaveText("Health");
  await expect(page.locator("fs-monitor form[data-form='token']")).toHaveCount(0);
  await expect.poll(() => upgrades).toEqual(["ws://127.0.0.1:4173/api/v1/events"]);
  expect(errors).toEqual([]);
});
