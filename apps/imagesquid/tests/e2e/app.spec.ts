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

/**
 * Resolve a colour token to the browser's computed `rgb(...)` form through a probe element,
 * so the check does not depend on how the build minified the hex value.
 */
function token(page: Page, name: string): Promise<string> {
  return page.evaluate((prop) => {
    const probe = document.createElement("span");
    probe.style.color = `var(${prop})`;
    document.body.append(probe);
    const colour = getComputedStyle(probe).color;
    probe.remove();
    return colour;
  }, name);
}

/** WCAG 2.2 contrast of two colour tokens, computed in the page from their resolved colours. */
function tokenContrast(page: Page, fg: string, bg: string): Promise<number> {
  return page.evaluate(
    ([a, b]) => {
      const probe = document.createElement("span");
      document.body.append(probe);
      const channel = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
      const lum = (name: string) => {
        probe.style.color = `var(${name})`;
        const rgb = (getComputedStyle(probe).color.match(/\d+/g) ?? []).map(Number);
        const [r, g, bl] = rgb.map((c) => channel(c / 255));
        return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (bl ?? 0);
      };
      const [x, y] = [lum(a), lum(b)];
      return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
    },
    [fg, bg] as const,
  );
}

test("ImageSquid tokens override FileSquid defaults in light and dark (IS-S-008)", async ({
  page,
}) => {
  const errors = trackErrors(page, REFUSED_CALL);
  const sidebar = () =>
    page.evaluate(() => {
      const nav = document.querySelector("fs-app-shell")?.shadowRoot?.querySelector("nav");
      return nav ? getComputedStyle(nav).backgroundColor : "";
    });
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  expect(await token(page, "--fs-primary")).toBe("rgb(0, 93, 182)"); // #005db6
  expect(await token(page, "--fs-on-primary")).toBe("rgb(255, 255, 255)"); // #ffffff
  expect(await sidebar()).toBe("rgb(244, 245, 247)");
  expect(await tokenContrast(page, "--fs-on-primary", "--fs-primary")).toBeGreaterThanOrEqual(4.5);
  expect(await tokenContrast(page, "--fs-on-surface", "--fs-surface")).toBeGreaterThanOrEqual(4.5);
  await page.emulateMedia({ colorScheme: "dark" });
  expect(await token(page, "--fs-primary")).toBe("rgb(169, 199, 255)"); // #a9c7ff
  expect(await token(page, "--fs-on-primary")).toBe("rgb(0, 48, 99)"); // #003063
  expect(await sidebar()).toBe("rgb(27, 30, 36)");
  expect(await tokenContrast(page, "--fs-on-primary", "--fs-primary")).toBeGreaterThanOrEqual(4.5);
  expect(await tokenContrast(page, "--fs-on-surface", "--fs-surface")).toBeGreaterThanOrEqual(4.5);
  expect(errors).toEqual([]);
});

test("ImageSquid's theme control keeps a dark choice across a reload", async ({ page }) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  const control = page.locator("fs-app-shell fs-theme-control");
  await expect(control.locator("label")).toHaveText(["System", "Light", "Dark"]);
  await expect(control.locator('input[value="system"]')).toBeChecked();
  await control.locator("label", { hasText: "Dark" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(control.locator('input[value="dark"]')).toBeChecked();
  expect(await token(page, "--fs-primary")).toBe("rgb(169, 199, 255)"); // #a9c7ff
  await expect(page.locator("fs-app-shell header img")).toHaveAttribute(
    "src",
    "/imagesquid-logo.svg",
  );
  expect(errors).toEqual([]);
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
