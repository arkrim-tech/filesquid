/**
 * End-to-end: the BUILT ImageSquid distribution served by the real `filesquid-server`.
 * ImageSquid is FileSquid underneath: same deny-by-default API, ImageSquid brand, vocabulary
 * and tokens.
 */
import { expect, test, type Page } from "@playwright/test";
import { ADMIN_TOKEN } from "./admin-token";

/** The browser's own console lines for a deliberately refused fetch or WebSocket upgrade. */
const REFUSED_CALL = /status of 40[13] \(|WebSocket connection to .* failed/;

/** Collect console errors (except tolerated lines) and uncaught page errors. */
function trackErrors(page: Page, tolerate?: RegExp): string[] {
  const errors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error" && !tolerate?.test(msg.text())) errors.push(msg.text());
  });
  page.on("pageerror", (err) => errors.push(err.message));
  return errors;
}

test("ImageSquid brand, album vocabulary and FileSquid API health", async ({ page }) => {
  const errors = trackErrors(page);
  // Stand in for the desktop host, which defines its per-launch token for its own pages.
  await page.addInitScript((value) => {
    Object.defineProperty(window, "__FILESQUID_LAUNCH_TOKEN__", { value, enumerable: false });
  }, ADMIN_TOKEN);
  await page.goto("/");
  await expect(page).toHaveTitle("ImageSquid");
  const shell = page.locator("fs-app-shell");
  await expect(shell.locator("h1")).toHaveText("ImageSquid");
  await expect(shell.locator("header img")).toHaveAttribute("src", "/imagesquid-logo.svg");
  await expect(shell.locator("nav li")).toHaveText([
    "Libraries",
    "Albums",
    "Galleries",
    "Exhibits",
    "Monitor",
  ]);
  await expect(shell.locator('[role="status"]')).toHaveText("Archives API: ok (filesquid)");
  expect(errors).toEqual([]);
});

test("ImageSquid tokens override FileSquid defaults in light and dark", async ({ page }) => {
  const primary = () =>
    page.evaluate(() =>
      getComputedStyle(document.documentElement).getPropertyValue("--fs-primary").trim(),
    );
  const sidebar = () =>
    page.evaluate(() => {
      const nav = document.querySelector("fs-app-shell")?.shadowRoot?.querySelector("nav");
      return nav ? getComputedStyle(nav).backgroundColor : "";
    });
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  expect(await primary()).toBe("#1d3f73");
  expect(await sidebar()).toBe("rgb(244, 245, 247)");
  await page.emulateMedia({ colorScheme: "dark" });
  expect(await primary()).toBe("#9cc0ff");
  expect(await sidebar()).toBe("rgb(27, 30, 36)");
});

test("ImageSquid names missing credentials in its own words", async ({ page }) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await page.goto("/");
  const alert = page.locator('fs-app-shell [role="alert"]');
  await expect(alert).toHaveAttribute("data-reason", "missing_credentials");
  await expect(alert).toHaveText("Archives unavailable: Missing credentials");
  expect(errors).toEqual([]);
});

test("ImageSquid reaches the FileSquid Monitor with its own vocabulary", async ({ page }) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await page.goto("/");
  await page.locator("fs-app-shell nav a", { hasText: "Monitor" }).click();
  const monitor = page.locator("fs-monitor");
  const form = monitor.locator("form[data-form='token']");
  await form.locator("input").fill(ADMIN_TOKEN);
  await form.locator("button").click();
  await expect(monitor.locator('[data-section="journal"] h2')).toHaveText("Activity history");
  await expect(monitor.locator('[data-section="storage"] h2')).toHaveText("Storage");
  await expect(monitor.locator('tr[data-process-id^="host-"]')).toHaveAttribute(
    "data-state",
    "running",
  );
  await expect(page.locator('fs-app-shell [role="status"]')).toHaveText(
    "Archives API: ok (filesquid)",
  );
  expect(errors).toEqual([]);
});
