/**
 * End-to-end (UI-S-016, SRS-NFR-006, RC-005): the BUILT FileSquid UI served by the real
 * `filesquid-server` meets the accessibility contracts in Chromium's own accessibility tree and
 * computed styles: named landmarks and controls from the label table, keyboard order, skip
 * control, visible focus ring in light and dark, focus recovery after a route change and a
 * sign-in, reduced motion, contrast preferences, reflow at 320 px and 200 % text, and RTL.
 * No pixel or screenshot comparison.
 */
import { expect, test, type Page } from "@playwright/test";
import { ADMIN_TOKEN } from "./admin-token";
import { tokenColour, trackErrors } from "./support/strict-dom";

/** The browser's own console lines for a deliberately refused fetch or WebSocket upgrade. */
const REFUSED_CALL = /status of 40[13] \(|WebSocket connection to .* failed/;

/** Authorise the page with the desktop host's launch token so no call is refused. */
async function withLaunchToken(page: Page): Promise<void> {
  await page.addInitScript((value) => {
    Object.defineProperty(window, "__FILESQUID_LAUNCH_TOKEN__", { value, enumerable: false });
  }, ADMIN_TOKEN);
}

/** What has focus, followed into shadow roots: tag, accessible text and the focus ring drawn. */
interface Focused {
  tag: string;
  text: string;
  outline: string;
  offset: string;
}

/** Describe the deep active element; a radio's ring is drawn on its label. */
function focused(page: Page): Promise<Focused> {
  return page.evaluate(() => {
    let active = document.activeElement;
    while (active?.shadowRoot?.activeElement) active = active.shadowRoot.activeElement;
    const ringOn = active?.closest("label") ?? active;
    const style = ringOn ? getComputedStyle(ringOn) : undefined;
    return {
      tag: active?.tagName.toLowerCase() ?? "",
      text: (active?.closest("label") ?? active)?.textContent?.trim() ?? "",
      outline: style ? `${style.outlineStyle} ${style.outlineWidth} ${style.outlineColor}` : "",
      offset: style?.outlineOffset ?? "",
    };
  });
}

/** True when the document scrolls horizontally. */
function scrollsHorizontally(page: Page): Promise<boolean> {
  return page.evaluate(
    () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
  );
}

/** Bounding boxes of shell parts, by selector inside the shell's shadow root. */
function shellBoxes(page: Page, selectors: string[]): Promise<Record<string, DOMRect>> {
  return page.evaluate((list) => {
    const root = document.querySelector("fs-app-shell")?.shadowRoot;
    const boxes: Record<string, DOMRect> = {};
    for (const selector of list) {
      const box = root?.querySelector(selector)?.getBoundingClientRect();
      if (box) boxes[selector] = box.toJSON() as DOMRect;
    }
    return boxes;
  }, selectors);
}

test("landmarks, headings and control names come from the label table", async ({ page }) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.locator("html")).toHaveAttribute("dir", "ltr");
  const shell = page.locator("fs-app-shell");
  await expect(shell.locator("header fs-host-indicator")).toHaveAttribute("data-state", "ready");
  await expect(shell).toMatchAriaSnapshot(`
    - button "Skip to main content"
    - navigation "Main navigation":
      - img "FileSquid logo"
      - heading "FileSquid" [level=1]
      - button "New"
      - textbox "Filter this view"
      - list:
        - listitem:
          - link "All files"
        - listitem:
          - link "Recent"
      - status
      - alert:
        - paragraph: This is not available right now
        - button "Try again"
      - list:
        - listitem:
          - link "Recently deleted"
      - paragraph: Storage used
      - list:
        - listitem:
          - link "Create or open a library"
        - listitem:
          - link "Settings"
    - banner:
      - searchbox "Search files and folders"
      - group "Where your files are":
        - button "Server"
      - link "Monitor"
    - main
  `);
  await expect(page.getByRole("heading", { level: 1 })).toHaveCount(1);
  expect(errors).toEqual([]);
});

for (const scheme of ["light", "dark"] as const) {
  test(`Tab order and a 2px --fs-focus-ring focus ring in the ${scheme} theme`, async ({
    page,
  }) => {
    const errors = trackErrors(page);
    await withLaunchToken(page);
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto("/");
    // The badge's label comes from the host descriptor.
    await expect(page.locator("fs-host-indicator button.badge")).toHaveText("Server");
    const colour = await tokenColour(page, "--fs-focus-ring");
    const ring = `solid 2px ${colour}`;
    const seen: Focused[] = [];
    for (let step = 0; step < 3; step += 1) {
      await page.keyboard.press("Tab");
      seen.push(await focused(page));
    }
    // UI-S-025: the skip control, then the sidebar's New button and its view filter.
    expect(seen.map(({ tag, text }) => `${tag} ${text}`)).toEqual([
      "button Skip to main content",
      "button New",
      "input ",
    ]);
    for (const item of seen.slice(0, 2)) {
      expect(item.outline).toBe(ring);
      expect(item.offset).toBe("2px");
    }
    // UI-S-024: a focused input shows a 2 px --fs-focus-ring border instead of the ring.
    // Polled: the border colour has a 150 ms transition.
    const filter = () =>
      page.locator("fs-app-shell nav input.filter").evaluate((el) => {
        const css = getComputedStyle(el);
        return `${css.borderTopStyle} ${css.borderTopWidth} ${css.borderTopColor}`;
      });
    await expect.poll(filter).toBe(ring);
    expect(errors).toEqual([]);
  });
}

test("light-DOM content slotted into the shell gets the same focus ring", async ({ page }) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  await page.goto("/");
  await page.evaluate(() => {
    const button = document.createElement("button");
    button.type = "button";
    button.textContent = "Slotted action";
    document.querySelector("fs-app-shell")?.append(button);
  });
  const ring = `solid 2px ${await tokenColour(page, "--fs-focus-ring")}`;
  // The sidebar, the header and then main, where the slotted button sits (UI-S-025).
  await expect(page.locator("fs-nav-tree fs-states")).toHaveAttribute("data-state", "unavailable");
  await expect(page.locator("fs-host-indicator button.badge")).toHaveText("Server");
  // Tab until the slotted button has focus (the body's own text is the button's text too).
  const reached = (seen: Focused) => seen.tag === "button" && seen.text === "Slotted action";
  let item = await focused(page);
  for (let step = 0; step < 30 && !reached(item); step += 1) {
    await page.keyboard.press("Tab");
    item = await focused(page);
  }
  expect(item.tag).toBe("button");
  expect(item.text).toBe("Slotted action");
  expect(item.outline).toBe(ring);
  expect(item.offset).toBe("2px");
  expect(errors).toEqual([]);
});

test("the skip control appears on focus and moves focus to main without routing", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  await page.goto("/");
  const skip = page.locator("fs-app-shell .skip");
  expect((await skip.boundingBox())?.width).toBeLessThanOrEqual(1);
  await page.keyboard.press("Tab");
  await expect(skip).toBeFocused();
  expect((await skip.boundingBox())?.width).toBeGreaterThan(40);
  const url = page.url();
  await page.keyboard.press("Enter");
  expect((await focused(page)).tag).toBe("main");
  expect(page.url()).toBe(url);
  expect(errors).toEqual([]);
});

test("opening the Monitor from the keyboard moves focus to main", async ({ page }) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  await page.goto("/");
  await page.locator('fs-app-shell header a[href="#/monitor"]').focus();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/#\/monitor$/);
  const monitor = page.locator("fs-monitor");
  await expect(monitor.getByRole("heading", { name: "Health" })).toBeVisible();
  expect((await focused(page)).tag).toBe("main");
  await expect(monitor.getByRole("region", { name: "Activity journal" })).toBeVisible();
  await expect(monitor.getByRole("table", { name: "Processes" })).toBeVisible();
  await page.goBack();
  await expect(monitor).toHaveCount(0);
  expect((await focused(page)).tag).toBe("main");
  expect(errors).toEqual([]);
});

test("a keyboard sign-in to the Monitor keeps focus in the Monitor", async ({ page }) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await page.goto("/#/monitor");
  const monitor = page.locator("fs-monitor");
  const field = monitor.getByLabel("Admin token");
  await field.focus();
  await page.keyboard.type("wrong-token");
  await page.keyboard.press("Enter");
  await expect(monitor.locator('[role="alert"]')).toHaveText("Invalid credentials");
  await expect(field).toBeFocused();
  await page.keyboard.type(ADMIN_TOKEN);
  await page.keyboard.press("Enter");
  const heading = monitor.getByRole("heading", { name: "Health" });
  await expect(heading).toBeFocused();
  expect(errors).toEqual([]);
});

test("reduced motion stops every transition", async ({ page }) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  const durations = () =>
    page.evaluate(() => {
      const label = document
        .querySelector("fs-settings")
        ?.shadowRoot?.querySelector("fs-theme-control")
        ?.shadowRoot?.querySelector("label");
      return label ? getComputedStyle(label).transitionDuration : "";
    });
  await page.emulateMedia({ reducedMotion: "no-preference" });
  // The theme control lives in Settings, Appearance (owner answer 7).
  await page.goto("/#/settings");
  await expect(page.locator("fs-settings fs-theme-control label").first()).toBeVisible();
  expect(await durations()).toBe("0.15s, 0.15s, 0.15s");
  await page.emulateMedia({ reducedMotion: "reduce" });
  expect(await durations()).toBe("0s");
  // Every element in every component shadow root, the Monitor included, has no motion.
  await page.goto("/#/monitor");
  await expect(page.locator("fs-monitor").getByRole("heading", { name: "Health" })).toBeVisible();
  const moving = await page.evaluate(() => {
    const found: string[] = [];
    const walk = (root: ShadowRoot) => {
      for (const el of root.querySelectorAll("*")) {
        const style = getComputedStyle(el);
        // Fallback content of an assigned slot is not rendered and has no computed style.
        if (style.transitionDuration === "") continue;
        const motion = `${style.transitionDuration} ${style.animationDuration}`;
        if (motion !== "0s 0s") found.push(`${el.tagName.toLowerCase()}: ${motion}`);
        if (el.shadowRoot) walk(el.shadowRoot);
      }
    };
    for (const host of document.querySelectorAll("fs-app-shell, fs-monitor")) {
      if (host.shadowRoot) walk(host.shadowRoot);
    }
    return found;
  });
  expect(moving).toEqual([]);
  expect(errors).toEqual([]);
});

test("more contrast strengthens borders; forced colours keep the chosen option", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  const headerBorder = () =>
    page.evaluate(() => {
      const header = document.querySelector("fs-app-shell")?.shadowRoot?.querySelector("header");
      return header ? getComputedStyle(header).borderBottomColor : "";
    });
  await page.goto("/");
  expect(await headerBorder()).toBe(await tokenColour(page, "--fs-outline"));
  await page.emulateMedia({ contrast: "more" });
  expect(await headerBorder()).toBe(await tokenColour(page, "--fs-on-surface-variant"));
  await page.emulateMedia({ contrast: "no-preference", forcedColors: "active" });
  const highlight = await page.evaluate(() => {
    const probe = document.createElement("span");
    probe.style.cssText = "forced-color-adjust: none; background-color: Highlight";
    document.body.append(probe);
    const colour = getComputedStyle(probe).backgroundColor;
    probe.remove();
    return colour;
  });
  await page.goto("/#/settings");
  const backgrounds = () =>
    page.evaluate(() => {
      const labels = document
        .querySelector("fs-settings")
        ?.shadowRoot?.querySelector("fs-theme-control")
        ?.shadowRoot?.querySelectorAll("label");
      return [...(labels ?? [])].map((label) => getComputedStyle(label).backgroundColor);
    });
  // The option's colours change with a 150ms transition, so poll until it has finished.
  await expect.poll(async () => (await backgrounds())[0]).toBe(highlight);
  expect((await backgrounds())[1]).not.toBe(highlight);
  expect(errors).toEqual([]);
});

for (const { width, scale } of [
  { width: 320, scale: 100 },
  { width: 320, scale: 200 },
  { width: 1280, scale: 200 },
  { width: 640, scale: 200 },
]) {
  test(`no horizontal scroll at ${width} px wide and ${scale} % text`, async ({ page }) => {
    const errors = trackErrors(page);
    await withLaunchToken(page);
    await page.setViewportSize({ width, height: 800 });
    await page.goto("/#/monitor");
    await page.addStyleTag({ content: `html { font-size: ${scale}%; }` });
    const monitor = page.locator("fs-monitor");
    await expect(monitor.getByRole("heading", { name: "Health" })).toBeVisible();
    expect(await scrollsHorizontally(page)).toBe(false);
    const title = await page.evaluate(() => {
      const h1 = document.querySelector("fs-app-shell")?.shadowRoot?.querySelector("h1");
      return h1
        ? { size: getComputedStyle(h1).fontSize, clipped: h1.scrollWidth > h1.clientWidth }
        : undefined;
    });
    // UI-S-025: the sidebar product name is 17 px at 100 % text.
    expect(title).toEqual({ size: `${17 * (scale / 100)}px`, clipped: false });
    const boxes = await shellBoxes(page, ["nav", "main"]);
    const stacked = (boxes.nav?.bottom ?? 0) <= (boxes.main?.top ?? 0);
    expect(stacked).toBe(width <= 640);
    expect(errors).toEqual([]);
  });
}

test("a right-to-left locale mirrors the header and navigation", async ({ page }) => {
  const errors = trackErrors(page);
  await withLaunchToken(page);
  await page.goto("/");
  await expect(page.locator("fs-app-shell header fs-host-indicator")).toHaveAttribute(
    "data-state",
    "ready",
  );
  const ltr = await shellBoxes(page, ["h1", "fs-host-indicator", "nav", "main"]);
  expect(ltr["fs-host-indicator"]?.x).toBeGreaterThan(ltr.h1?.x ?? 0);
  expect(ltr.nav?.x).toBeLessThan(ltr.main?.x ?? 0);
  await page.evaluate(() => document.documentElement.setAttribute("dir", "rtl"));
  const rtl = await shellBoxes(page, ["h1", "fs-host-indicator", "nav", "main"]);
  expect(rtl["fs-host-indicator"]?.x).toBeLessThan(rtl.h1?.x ?? 0);
  expect(rtl.nav?.x).toBeGreaterThan(rtl.main?.x ?? 0);
  expect(await scrollsHorizontally(page)).toBe(false);
  expect(errors).toEqual([]);
});
