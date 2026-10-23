/**
 * End-to-end (SRS-UI-007, SRS-F-058, NFR-006): the BUILT FileSquid UI served by the real
 * `filesquid-server` shows the official brand, applies the official tokens in light and dark,
 * follows the OS by default, keeps an explicit theme choice (made in Settings, Appearance, owner
 * answer 7) across a reload, and meets WCAG 2.2 AA in the computed styles except for the owner's
 * listed design-guide pairs at exactly their listed values (SRS-UI-007, UI-S-015, CA-1 "a"),
 * with the focus ring and control outline at 3:1 on every surface. No pixel or screenshot
 * comparison.
 */
import { expect, test, type Page } from "@playwright/test";
import { ADMIN_TOKEN } from "./admin-token";
import { bodyBackground, tokenColour, trackErrors } from "./support/strict-dom";
import {
  CONTRAST_PAIRS,
  CONTROL_CONTRAST_PAIRS,
  FILESQUID_CONTRAST_EXEMPTIONS,
  FOCUS_CONTRAST_PAIRS,
} from "../../src/theme/tokens";

/** Authorise the shell with the desktop host's launch token so no call is refused. */
async function withLaunchToken(page: Page): Promise<void> {
  await page.addInitScript((value) => {
    Object.defineProperty(window, "__FILESQUID_LAUNCH_TOKEN__", { value, enumerable: false });
  }, ADMIN_TOKEN);
}

/**
 * Compute the WCAG 2.2 contrast of every pair of the token test from the page's computed tokens,
 * the same rule as `contrastFailures` in `src/theme/tokens.ts`: text pairs below 4.5:1 fail
 * unless the pair is one of the owner's listed exemptions for this theme and both computed
 * colours equal its listed values; focus-ring and control-outline pairs fail below 3:1 with no
 * exception. Returns the failures, so an empty list means the rule holds.
 */
function contrastFailuresInPage(page: Page, theme: "light" | "dark"): Promise<string[]> {
  const exemptions: [string, string, string, string][] = FILESQUID_CONTRAST_EXEMPTIONS.filter(
    (e) => e.theme === theme,
  ).map((e) => [e.foreground, e.background, e.foregroundValue, e.backgroundValue]);
  const group = (
    pairs: readonly (readonly [string, string])[],
    minimum: number,
    exemptable: boolean,
  ) => ({ pairs: pairs.map(([fg, bg]) => [fg, bg] as [string, string]), minimum, exemptable });
  const groups: PairGroup[] = [
    group(CONTRAST_PAIRS, 4.5, true),
    group(FOCUS_CONTRAST_PAIRS, 3, false),
    group(CONTROL_CONTRAST_PAIRS, 3, false),
  ];
  return page.evaluate(
    ({ groups, exemptions }) => {
      const probe = document.createElement("span");
      document.body.append(probe);
      const rgb = (token: string): number[] => {
        probe.style.color = `var(${token})`;
        return (getComputedStyle(probe).color.match(/\d+/g) ?? []).map(Number);
      };
      const hex = (token: string): string =>
        `#${rgb(token)
          .map((c) => c.toString(16).padStart(2, "0"))
          .join("")}`;
      const channel = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
      const lum = (token: string): number => {
        const [r, g, b] = rgb(token).map((c) => channel(c / 255));
        return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
      };
      const exempt = (fg: string, bg: string): boolean =>
        exemptions.some(
          ([f, b, fv, bv]) => f === fg && b === bg && hex(fg) === fv && hex(bg) === bv,
        );
      const failures: string[] = [];
      for (const { pairs, minimum, exemptable } of groups) {
        for (const [fg, bg] of pairs) {
          const [a, b] = [lum(fg), lum(bg)];
          const ratio = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
          if (ratio >= minimum || (exemptable && exempt(fg, bg))) continue;
          failures.push(`${fg} on ${bg}: ${ratio.toFixed(2)}`);
        }
      }
      probe.remove();
      return failures;
    },
    { groups, exemptions },
  );
}

/** One group of token pairs and the ratio it must reach. */
interface PairGroup {
  pairs: [string, string][];
  minimum: number;
  exemptable: boolean;
}

/** The Settings, Appearance theme control's radio for one choice (owner answer 7). */
const themeRadio = (page: Page, value: string) =>
  page.locator(`fs-settings fs-theme-control input[value="${value}"]`);

test("official FileSquid tokens apply in light and dark and meet WCAG 2.2 AA", async ({ page }) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  await expect(page.locator("fs-app-shell h1")).toHaveText("FileSquid");
  expect(await tokenColour(page, "--fs-primary")).toBe("rgb(22, 172, 73)"); // #16ac49
  expect(await tokenColour(page, "--fs-on-primary")).toBe("rgb(255, 255, 255)");
  expect(await tokenColour(page, "--fs-focus-ring")).toBe("rgb(15, 138, 57)"); // #0f8a39
  expect(await bodyBackground(page)).toBe("rgb(255, 255, 255)");
  expect(await page.evaluate(() => getComputedStyle(document.body).color)).toBe("rgb(17, 17, 17)");
  expect(await contrastFailuresInPage(page, "light")).toEqual([]);
  const heading = await page
    .locator("fs-app-shell h1")
    .evaluate((h1) => getComputedStyle(h1).fontFamily);
  expect(heading).toMatch(/^Georgia/);

  await page.emulateMedia({ colorScheme: "dark" });
  expect(await tokenColour(page, "--fs-primary")).toBe("rgb(22, 172, 73)"); // #16ac49
  expect(await tokenColour(page, "--fs-on-primary")).toBe("rgb(255, 255, 255)");
  expect(await tokenColour(page, "--fs-focus-ring")).toBe("rgb(22, 172, 73)"); // #16ac49
  expect(await bodyBackground(page)).toBe("rgb(20, 20, 20)");
  expect(await contrastFailuresInPage(page, "dark")).toEqual([]);
  expect(errors).toEqual([]);
});

test("the official mark fills the brand slot, with its dark-surface variant in dark", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  const logo = page.locator("fs-app-shell nav .brand img");
  await expect(logo).toHaveAttribute("src", "/filesquid-mark.svg");
  await expect(logo).toHaveAttribute("alt", "FileSquid logo");
  await expect
    .poll(() => logo.evaluate((img: HTMLImageElement) => img.naturalWidth))
    .toBeGreaterThan(0);
  // UI-S-024, owner answer 14: the sidebar mark is 24 px tall.
  expect(await logo.evaluate((img) => img.getBoundingClientRect().height)).toBe(24);
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
  await page.goto("/#/settings");
  expect(await page.evaluate(() => customElements.get("fs-theme-control") !== undefined)).toBe(
    true,
  );
  await expect(page.locator("fs-app-shell header fs-theme-control")).toHaveCount(0);
  const control = page.locator("fs-settings fs-theme-control");
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

test("the chosen theme option and the Settings API status are drawn as UI-S-015 lists them", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto("/#/settings");
    await expect(page.locator('fs-settings .api[role="status"]')).toContainText(
      "API: ok · contract",
    );
    const read = () =>
      page.evaluate(() => {
        const settings = document.querySelector("fs-settings")?.shadowRoot;
        const control = settings?.querySelector("fs-theme-control")?.shadowRoot;
        const chosen = control?.querySelector("label:has(input:checked)");
        const status = settings?.querySelector(".api");
        const box = status?.parentElement;
        if (!chosen || !status || !box) return undefined;
        const css = (el: Element) => getComputedStyle(el);
        return {
          chosen: [css(chosen).color, css(chosen).backgroundColor],
          status: [css(status).color, css(box).backgroundColor],
        };
      });
    // The chosen option is active text on the active tint: an owner-listed pair in light.
    // Polled: the option's colours have a 150 ms transition after the scheme changes.
    const chosen = [
      await tokenColour(page, "--fs-on-primary-container"),
      await tokenColour(page, "--fs-primary-container"),
    ];
    await expect.poll(async () => (await read())?.chosen).toEqual(chosen);
    const rendered = await read();
    const ratio = await page.evaluate(([fg, bg]) => {
      const channel = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
      const lum = (rgb: string) => {
        const [r, g, b] = (rgb.match(/\d+(\.\d+)?/g) ?? []).map((n) => Number(n) / 255);
        return 0.2126 * channel(r ?? 0) + 0.7152 * channel(g ?? 0) + 0.0722 * channel(b ?? 0);
      };
      const [a, b] = [lum(fg ?? ""), lum(bg ?? "")];
      return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
    }, rendered?.status ?? []);
    expect(ratio).toBeGreaterThanOrEqual(4.5);
  }
  expect(errors).toEqual([]);
});
