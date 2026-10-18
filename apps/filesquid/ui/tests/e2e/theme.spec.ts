/**
 * End-to-end (SRS-UI-007, SRS-F-058, NFR-006): the BUILT FileSquid UI served by the real
 * `filesquid-server` shows the official brand, applies the official tokens in light and dark,
 * follows the OS by default, keeps an explicit theme choice across a reload, and meets
 * WCAG 2.2 AA in the computed styles. No pixel or screenshot comparison.
 */
import { expect, test, type Page } from "@playwright/test";
import { ADMIN_TOKEN } from "./admin-token";

/** Collect console errors and uncaught page errors for the whole test. */
function trackErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") errors.push(msg.text());
  });
  page.on("pageerror", (err) => errors.push(err.message));
  return errors;
}

/** Authorise the shell with the desktop host's launch token so no call is refused. */
async function withLaunchToken(page: Page): Promise<void> {
  await page.addInitScript((value) => {
    Object.defineProperty(window, "__FILESQUID_LAUNCH_TOKEN__", { value, enumerable: false });
  }, ADMIN_TOKEN);
}

/**
 * Resolve a colour token to the browser's computed `rgb(...)` form through a probe element,
 * so the check does not depend on how the build minified the hex value.
 */
function tokenColour(page: Page, name: string): Promise<string> {
  return page.evaluate((prop) => {
    const probe = document.createElement("span");
    probe.style.color = `var(${prop})`;
    document.body.append(probe);
    const colour = getComputedStyle(probe).color;
    probe.remove();
    return colour;
  }, name);
}

/** Computed body background colour. */
function bodyBackground(page: Page): Promise<string> {
  return page.evaluate(() => getComputedStyle(document.body).backgroundColor);
}

/**
 * Compute the WCAG 2.2 contrast of every required pair from the page's computed tokens.
 * Returns the pairs below 4.5:1, so an empty list means AA holds.
 */
function contrastFailuresInPage(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const pairs = [
      ["--fs-on-surface", "--fs-surface"],
      ["--fs-on-surface", "--fs-surface-container"],
      ["--fs-on-surface-variant", "--fs-surface"],
      ["--fs-on-surface-variant", "--fs-surface-container"],
      ["--fs-primary", "--fs-surface"],
      ["--fs-on-primary", "--fs-primary"],
      ["--fs-on-primary-container", "--fs-primary-container"],
      ["--fs-error", "--fs-surface"],
      ["--fs-error", "--fs-surface-container"],
    ];
    const probe = document.createElement("span");
    document.body.append(probe);
    const rgb = (token: string) => {
      probe.style.color = `var(${token})`;
      return (getComputedStyle(probe).color.match(/\d+/g) ?? []).map(Number);
    };
    const channel = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
    const lum = (token: string) => {
      const [r, g, b] = rgb(token).map((c) => channel(c / 255));
      return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
    };
    const failures = pairs
      .map(([fg, bg]) => {
        const [a, b] = [lum(fg ?? ""), lum(bg ?? "")];
        return { pair: `${fg} on ${bg}`, ratio: (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05) };
      })
      .filter(({ ratio }) => !(ratio >= 4.5))
      .map(({ pair, ratio }) => `${pair}: ${ratio.toFixed(2)}`);
    probe.remove();
    return failures;
  });
}

/** The header theme control's radio for one choice. */
const themeRadio = (page: Page, value: string) =>
  page.locator(`fs-app-shell fs-theme-control input[value="${value}"]`);

test("official FileSquid tokens apply in light and dark and meet WCAG 2.2 AA", async ({ page }) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  await expect(page.locator("fs-app-shell h1")).toHaveText("FileSquid");
  expect(await tokenColour(page, "--fs-primary")).toBe("rgb(11, 122, 50)"); // #0b7a32
  expect(await tokenColour(page, "--fs-on-primary")).toBe("rgb(255, 255, 255)");
  expect(await bodyBackground(page)).toBe("rgb(255, 255, 255)");
  expect(await page.evaluate(() => getComputedStyle(document.body).color)).toBe("rgb(17, 17, 17)");
  expect(await contrastFailuresInPage(page)).toEqual([]);
  const heading = await page
    .locator("fs-app-shell h1")
    .evaluate((h1) => getComputedStyle(h1).fontFamily);
  expect(heading).toMatch(/^Georgia/);

  await page.emulateMedia({ colorScheme: "dark" });
  expect(await tokenColour(page, "--fs-primary")).toBe("rgb(22, 172, 73)"); // #16ac49
  expect(await tokenColour(page, "--fs-on-primary")).toBe("rgb(13, 43, 24)"); // #0d2b18
  expect(await bodyBackground(page)).toBe("rgb(20, 20, 20)");
  expect(await contrastFailuresInPage(page)).toEqual([]);
  expect(errors).toEqual([]);
});

test("the official mark fills the brand slot, with its dark-surface variant in dark", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  const logo = page.locator("fs-app-shell header img");
  await expect(logo).toHaveAttribute("src", "/filesquid-mark.svg");
  await expect(logo).toHaveAttribute("alt", "FileSquid logo");
  await expect
    .poll(() => logo.evaluate((img: HTMLImageElement) => img.naturalWidth))
    .toBeGreaterThan(0);
  expect(await logo.evaluate((img) => img.getBoundingClientRect().height)).toBeGreaterThanOrEqual(
    24,
  );
  await expect(page.locator('link[rel="icon"]')).toHaveAttribute("href", "/filesquid-mark.svg");

  await page.emulateMedia({ colorScheme: "dark" });
  await expect(logo).toHaveAttribute("src", "/filesquid-mark-dark.svg");
  await expect
    .poll(() => logo.evaluate((img: HTMLImageElement) => img.naturalWidth))
    .toBeGreaterThan(0);
  expect(errors).toEqual([]);
});

test("the theme follows the OS by default and the control's choice persists across a reload", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  expect(await page.evaluate(() => customElements.get("fs-theme-control") !== undefined)).toBe(
    true,
  );
  const control = page.locator("fs-app-shell fs-theme-control");
  await expect(control.locator("legend")).toHaveText("Theme");
  await expect(control.locator("label")).toHaveText(["System", "Light", "Dark"]);
  await expect(themeRadio(page, "system")).toBeChecked();
  expect(await page.evaluate(() => document.documentElement.hasAttribute("data-theme"))).toBe(
    false,
  );

  await control.locator("label", { hasText: "Dark" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  expect(await bodyBackground(page)).toBe("rgb(20, 20, 20)");
  expect(await page.evaluate(() => localStorage.getItem("filesquid.theme"))).toBe("dark");

  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(themeRadio(page, "dark")).toBeChecked();
  expect(await bodyBackground(page)).toBe("rgb(20, 20, 20)");

  // Keyboard only: arrow keys move through the native radio group and choose as they go.
  await themeRadio(page, "dark").focus();
  await page.keyboard.press("ArrowUp");
  await expect(themeRadio(page, "light")).toBeFocused();
  await expect(themeRadio(page, "light")).toBeChecked();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.keyboard.press("ArrowUp");
  await expect(themeRadio(page, "system")).toBeChecked();
  expect(await page.evaluate(() => document.documentElement.hasAttribute("data-theme"))).toBe(
    false,
  );
  expect(await bodyBackground(page)).toBe("rgb(255, 255, 255)");
  expect(await page.evaluate(() => localStorage.getItem("filesquid.theme"))).toBeNull();
  expect(errors).toEqual([]);
});

test("the selected theme option and the API status meet AA as rendered", async ({ page }) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto("/");
    await expect(page.locator('fs-app-shell [role="status"]')).toHaveText("API: ok (filesquid)");
    const ratios = await page.evaluate(() => {
      const shell = document.querySelector("fs-app-shell");
      const control = shell?.shadowRoot?.querySelector("fs-theme-control")?.shadowRoot;
      const chosen = control?.querySelector("label:has(input:checked)");
      const status = shell?.shadowRoot?.querySelector('[role="status"]');
      const channel = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
      const lum = (rgb: string) => {
        const [r, g, b] = (rgb.match(/\d+(\.\d+)?/g) ?? []).map((n) => Number(n) / 255);
        return 0.2126 * channel(r ?? 0) + 0.7152 * channel(g ?? 0) + 0.0722 * channel(b ?? 0);
      };
      const ratio = (fg: string, bg: string) => {
        const [a, b] = [lum(fg), lum(bg)];
        return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
      };
      if (!shell || !chosen || !status) return [];
      const style = (el: Element) => getComputedStyle(el);
      return [
        ratio(style(chosen).color, style(chosen).backgroundColor),
        ratio(style(status).color, style(shell).backgroundColor),
      ];
    });
    expect(ratios).toHaveLength(2);
    for (const value of ratios) expect(value).toBeGreaterThanOrEqual(4.5);
  }
  expect(errors).toEqual([]);
});
