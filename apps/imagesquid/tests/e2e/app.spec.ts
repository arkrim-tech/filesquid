/**
 * End-to-end: the BUILT ImageSquid distribution served by the real `filesquid-server`.
 * ImageSquid is FileSquid underneath: same deny-by-default API, ImageSquid brand, vocabulary
 * and tokens.
 */
import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import { ADMIN_TOKEN } from "./admin-token";
import { tokenColour as token, trackErrors } from "./support/strict-dom";

/** The browser's own console lines for a deliberately refused fetch or WebSocket upgrade. */
const REFUSED_CALL = /status of 40[13] \(|WebSocket connection to .* failed/;

test("ImageSquid brand, album vocabulary and the API status in Settings only", async ({ page }) => {
  const errors = trackErrors(page);
  // Stand in for the desktop host, which defines its per-launch token for its own pages.
  await page.addInitScript((value) => {
    Object.defineProperty(window, "__FILESQUID_LAUNCH_TOKEN__", { value, enumerable: false });
  }, ADMIN_TOKEN);
  await page.goto("/");
  await expect(page).toHaveTitle("ImageSquid");
  const shell = page.locator("fs-app-shell");
  await expect(shell.locator("h1")).toHaveText("ImageSquid");
  await expect(shell.locator("nav .brand img")).toHaveAttribute("src", "/imagesquid-logo.svg");
  // The server host hides the desktop-only Logs row (UI-S-021).
  await expect(shell.locator("nav li:visible")).toHaveText([
    "All photos",
    "Recent",
    "Libraries",
    "Albums",
    "Galleries",
    "Exhibits",
    "Recently deleted",
    "Create or open a library",
    "Settings",
  ]);
  await expect(shell.locator("header .monitor")).toHaveText("Monitor");
  await expect(shell.locator("header p.api")).toHaveCount(0);
  await page.goto("/#/settings");
  await expect(page.locator('fs-settings .api[role="status"]')).toContainText("API: ok · contract");
  expect(errors).toEqual([]);
});

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
  // Owner answer 7: the theme control lives in Settings, Appearance.
  await page.goto("/#/settings");
  const control = page.locator("fs-settings fs-theme-control");
  await expect(control.locator("label")).toHaveText(["System", "Light", "Dark"]);
  await expect(control.locator('input[value="system"]')).toBeChecked();
  await control.locator("label", { hasText: "Dark" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(control.locator('input[value="dark"]')).toBeChecked();
  expect(await token(page, "--fs-primary")).toBe("rgb(169, 199, 255)"); // #a9c7ff
  await expect(page.locator("fs-app-shell nav .brand img")).toHaveAttribute(
    "src",
    "/imagesquid-logo.svg",
  );
  expect(errors).toEqual([]);
});

test("ImageSquid names missing credentials in its own words", async ({ page }) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await page.goto("/#/settings");
  const alert = page.locator('fs-settings .api[role="alert"]');
  await expect(alert).toHaveAttribute("data-reason", "missing_credentials");
  await expect(alert).toHaveText("Archives unavailable: Missing credentials");
  expect(errors).toEqual([]);
});

test("ImageSquid reaches the FileSquid Monitor with its own vocabulary", async ({ page }) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await page.goto("/");
  await page.locator("fs-app-shell header a.monitor").click();
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
  expect(errors).toEqual([]);
});

for (const scheme of ["light", "dark"] as const) {
  test(`ImageSquid inherits the keyboard, naming and focus contracts in ${scheme} (UI-S-016)`, async ({
    page,
  }) => {
    const errors = trackErrors(page);
    await page.addInitScript((value) => {
      Object.defineProperty(window, "__FILESQUID_LAUNCH_TOKEN__", { value, enumerable: false });
    }, ADMIN_TOKEN);
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto("/");
    const shell = page.locator("fs-app-shell");
    await expect(shell.getByRole("navigation", { name: "Main navigation" })).toBeVisible();
    await expect(shell.getByRole("heading", { level: 1 })).toHaveText("ImageSquid");
    const ring = `solid 2px ${await token(page, "--fs-focus-ring")}`;
    const seen: string[] = [];
    await expect(page.locator("fs-host-indicator button.badge")).toHaveText(
      labels.hostBadgeServer ?? "",
    );
    // UI-S-025: the skip control, then the sidebar's New button.
    for (let step = 0; step < 2; step += 1) {
      await page.keyboard.press("Tab");
      seen.push(
        await page.evaluate(() => {
          let active = document.activeElement;
          while (active?.shadowRoot?.activeElement) active = active.shadowRoot.activeElement;
          const ringOn = active?.closest("label") ?? active;
          const style = ringOn ? getComputedStyle(ringOn) : undefined;
          const text = ringOn?.textContent?.trim() ?? "";
          return `${text} | ${style?.outlineStyle} ${style?.outlineWidth} ${style?.outlineColor}`;
        }),
      );
    }
    expect(seen).toEqual([`Skip to main content | ${ring}`, `${labels.fileNew} | ${ring}`]);
    await page.locator("fs-app-shell header a.monitor").focus();
    await page.keyboard.press("Enter");
    await expect(page).toHaveURL(/#\/monitor$/);
    await expect(
      page.locator("fs-monitor").getByRole("table", { name: "Processes" }),
    ).toBeVisible();
    expect(errors).toEqual([]);
  });
}

/** ImageSquid's label table, as built into the distribution. */
const labels = JSON.parse(
  readFileSync(new URL("../../src/theme/labels.en.json", import.meta.url), "utf8"),
) as Record<string, string>;

for (const scheme of ["light", "dark"] as const) {
  test(`the navigation tree and view states speak ImageSquid in ${scheme} (UI-F2.1a)`, async ({
    page,
  }) => {
    const errors = trackErrors(page);
    await page.addInitScript((value) => {
      Object.defineProperty(window, "__FILESQUID_LAUNCH_TOKEN__", { value, enumerable: false });
    }, ADMIN_TOKEN);
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto("/");
    await expect(page.locator("fs-app-shell header fs-host-indicator")).toHaveAttribute(
      "data-state",
      "ready",
    );
    const alertColour = await page.evaluate((table) => {
      type Configurable = HTMLElement & { configure(o: unknown): void; state?: unknown };
      const node = (id: string, kind: string, name: string | null, children: boolean) => ({
        node: id,
        kind,
        name,
        has_children: children,
      });
      const levels: Record<string, unknown[]> = {
        root: [node("r", "repository", null, true)],
        r: [node("l1", "library", "Family", true)],
        l1: [node("a1", "collection", "Coastal Days", false)],
      };
      const source = {
        list: async (parent: string | null) => ({
          nodes: levels[parent ?? "root"] ?? [],
          ancestors: [],
          next_cursor: null,
        }),
      };
      const tree = document.createElement("fs-nav-tree") as Configurable;
      tree.slot = "navigation";
      tree.configure({ labels: table, source });
      const states = document.createElement("fs-states") as Configurable;
      states.configure({ labels: table });
      states.state = { kind: "offline" };
      document.querySelector("fs-app-shell")?.append(tree, states);
      const icon = states.shadowRoot?.querySelector('[role="alert"] svg');
      return icon ? getComputedStyle(icon).color : "";
    }, labels);
    const tree = page.locator("fs-nav-tree");
    await expect(tree.locator("section h2")).toHaveText(["Albums", "Galleries", "Folders"]);
    await expect(tree.getByRole("treeitem", { name: "Coastal Days" })).toBeVisible();
    await expect(tree.getByRole("button", { name: "Move Albums down" })).toBeVisible();
    await expect(page.locator("fs-states").getByRole("alert")).toContainText("You are offline");
    // Mockup 56: the offline state's icon is drawn in the default icon colour.
    expect(alertColour).toBe(await token(page, "--fs-icon"));
    expect(errors).toEqual([]);
  });
}
