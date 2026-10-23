/**
 * End-to-end (F-004 default UI, UI-S-017, UI-F2.1a, UI-S-016): the BUILT FileSquid UI served by
 * the real `filesquid-server` defines `<fs-nav-tree>`, `<fs-breadcrumb>` and `<fs-states>`;
 * slotted into the shell they are worded from the active label table, follow the ARIA treeview
 * keys, keep pins, section order and library collapse on the device across a reload, survive
 * refused storage, show the five view states distinctly with token colours in light and dark,
 * draw the 2px primary focus ring, honour reduced motion and forced colours, and reflow without
 * horizontal scrolling. The navigation data comes from an in-page fixture source shaped on the
 * API's navigation listing (API-S-021), given to the app's own tree: the real server answers
 * that route with 503 until it installs the navigation service (the live source is proven
 * against the production build in `preview/navigation.spec.ts`).
 */
import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import { ADMIN_TOKEN } from "./admin-token";
import { tokenColour, trackErrors } from "./support/strict-dom";
import type { LabelTable } from "../../src/theme/labels";

/** The active (default) label table, as built into the UI. */
const labels = JSON.parse(
  readFileSync(new URL("../../src/theme/labels.en.json", import.meta.url), "utf8"),
) as LabelTable;

/** A fixture node: what the listing answers, with its children. */
interface Fixture {
  node: string;
  kind: string;
  name: string | null;
  children?: Fixture[];
}

/** One library with collections (one holding a folder), a virtual collection and a folder. */
const oneLibrary: Fixture = {
  node: "r",
  kind: "repository",
  name: null,
  children: [
    {
      node: "l1",
      kind: "library",
      name: "Family",
      children: [
        {
          node: "c1",
          kind: "collection",
          name: "Photos",
          children: [{ node: "f1", kind: "folder", name: "2025" }],
        },
        { node: "c2", kind: "collection", name: "Hobbies" },
        { node: "v1", kind: "virtual_collection", name: "Best of 2025" },
        { node: "f9", kind: "folder", name: "inbox" },
      ],
    },
  ],
};

/** Two libraries, each with a collection. */
const twoLibraries: Fixture = {
  node: "r",
  kind: "repository",
  name: "Home",
  children: [
    {
      node: "l1",
      kind: "library",
      name: "Family",
      children: [{ node: "c1", kind: "collection", name: "Photos" }],
    },
    {
      node: "l2",
      kind: "library",
      name: "Archive",
      children: [{ node: "c3", kind: "collection", name: "Old" }],
    },
  ],
};

/** Collect console warnings. */
function trackWarnings(page: Page): string[] {
  const warnings: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "warning") warnings.push(msg.text());
  });
  return warnings;
}

/** Open the app authorised with the desktop launch token and wait for the shell. */
async function open(page: Page): Promise<void> {
  await page.addInitScript((value) => {
    Object.defineProperty(window, "__FILESQUID_LAUNCH_TOKEN__", { value, enumerable: false });
  }, ADMIN_TOKEN);
  await page.goto("/");
  await expect(page.locator("fs-app-shell header fs-host-indicator")).toHaveAttribute(
    "data-state",
    "ready",
  );
}

/**
 * Slot a navigation tree into the shell's nav over an in-page fixture source, and add a
 * breadcrumb for folder `f1` to the shell's main area when the fixture has it. The source fails
 * while `window.__navFail` is true and counts calls in `window.__navCalls`.
 */
async function mountNavigation(
  page: Page,
  fixture: Fixture = oneLibrary,
  waitForSections = true,
): Promise<void> {
  await page.evaluate(
    ({ labels: table, fixture: root }) => {
      type Node = { node: string; kind: string; name: string | null; children?: Node[] };
      type Configurable = HTMLElement & { configure(options: unknown): void; items?: unknown };
      const w = window as unknown as Record<string, unknown>;
      const index = new Map<string, { item: Node; path: Node[] }>();
      const walk = (item: Node, path: Node[]): void => {
        index.set(item.node, { item, path });
        for (const child of item.children ?? []) walk(child, [...path, item]);
      };
      walk(root, []);
      const listed = (i: Node) => ({
        node: i.node,
        kind: i.kind,
        name: i.name,
        has_children: (i.children ?? []).length > 0,
      });
      w.__navCalls = 0;
      const source = {
        list: async (parent: string | null) => {
          w.__navCalls = Number(w.__navCalls) + 1;
          if (w.__navFail) throw new Error("navigation unavailable");
          const entry = parent === null ? undefined : index.get(parent);
          if (parent !== null && !entry) throw new Error(`unknown node ${parent}`);
          const level = entry ? (entry.item.children ?? []) : [root];
          const ancestors = entry ? [...entry.path, entry.item].map(listed) : [];
          return { nodes: level.map(listed), ancestors, next_cursor: null };
        },
      };
      // The app mounts its live tree in the slot; give that tree the fixture source.
      const shell = document.querySelector("fs-app-shell");
      const tree = (shell?.querySelector("fs-nav-tree") ??
        document.createElement("fs-nav-tree")) as Configurable;
      tree.slot = "navigation";
      tree.configure({ labels: table, source });
      if (!tree.isConnected) shell?.append(tree);
      const folder = index.get("f1");
      if (!folder) return;
      const crumb = document.createElement("fs-breadcrumb") as Configurable;
      crumb.configure({ labels: table });
      crumb.items = [...folder.path, folder.item].map((n) => ({
        node: n.node,
        name: n.name ?? table.repository,
        href: `#/navigation/${n.node}`,
      }));
      shell?.append(crumb);
    },
    { labels, fixture },
  );
  if (waitForSections) {
    await expect(page.locator("fs-nav-tree").locator("section").first()).toBeVisible();
  }
}

/** Section headings of the tree, in display order. */
function headings(page: Page) {
  return page.locator("fs-nav-tree").locator("section h2");
}

/** The focused element (followed into shadow roots) and the focus ring it draws. */
function focusRing(page: Page): Promise<{ text: string; ring: string; offset: string }> {
  return page.evaluate(() => {
    let active = document.activeElement;
    while (active?.shadowRoot?.activeElement) active = active.shadowRoot.activeElement;
    const style = active ? getComputedStyle(active) : undefined;
    return {
      text: active?.getAttribute("aria-label") ?? active?.textContent?.trim() ?? "",
      ring: style ? `${style.outlineStyle} ${style.outlineWidth} ${style.outlineColor}` : "",
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

test("the elements are defined and worded from the label table inside the shell", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await open(page);
  const defined = await page.evaluate(() =>
    ["fs-nav-tree", "fs-breadcrumb", "fs-states"].map((n) => customElements.get(n) !== undefined),
  );
  expect(defined).toEqual([true, true, true]);
  await mountNavigation(page);
  const shell = page.locator("fs-app-shell");
  // UI-S-025: the tree sits in the sidebar between the views and the Recently deleted row.
  await expect(shell.getByRole("navigation", { name: labels.navigationMain })).toMatchAriaSnapshot(`
    - list:
      - listitem:
        - link "${labels.fileRoot}"
      - listitem:
        - link "${labels.fileRecent}"
    - heading "${labels.collections}" [level=2]
    - button "Move ${labels.collections} up" [disabled]
    - button "Move ${labels.collections} down"
    - tree "${labels.collections}":
      - treeitem "Photos" [expanded=false]
      - treeitem "Hobbies"
    - button "Pin Photos" [pressed=false]
    - heading "${labels.virtualCollections}" [level=2]
    - tree "${labels.virtualCollections}":
      - treeitem "Best of 2025"
    - heading "${labels.folders}" [level=2]
    - tree "${labels.folders}":
      - treeitem "inbox"
    - list:
      - listitem:
        - link "${labels.navigationRecentlyDeleted}"
    - paragraph: ${labels.fileStorageUsed}
    - list:
      - listitem:
        - link "${labels.libraryNavLink}"
      - listitem:
        - link "${labels.settings}"
  `);
  await expect(shell.getByRole("main")).toMatchAriaSnapshot(`
    - navigation "${labels.navigationBreadcrumb}":
      - list:
        - listitem:
          - link "${labels.repository}"
        - listitem:
          - link "Family"
        - listitem:
          - link "Photos"
        - listitem:
          - link "2025"
  `);
  await expect(shell.getByRole("link", { name: "2025" })).toHaveAttribute("aria-current", "page");
  await expect(shell.locator('nav slot[name="navigation"] ul')).toBeHidden();
  expect(errors).toEqual([]);
});

test("the tree follows the ARIA treeview keys and reads children as nodes open", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await open(page);
  await mountNavigation(page);
  const tree = page.locator("fs-nav-tree");
  const photos = tree.getByRole("treeitem", { name: "Photos" });
  await photos.focus();
  await page.keyboard.press("ArrowDown");
  await expect(tree.getByRole("treeitem", { name: "Hobbies" })).toBeFocused();
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowRight");
  await expect(photos).toHaveAttribute("aria-expanded", "true");
  await expect(photos).toBeFocused();
  await page.keyboard.press("ArrowRight");
  await expect(tree.getByRole("treeitem", { name: "2025", exact: true })).toBeFocused();
  await expect(tree.getByRole("treeitem", { name: "2025", exact: true })).toHaveAttribute(
    "aria-level",
    "2",
  );
  await page.keyboard.press("ArrowLeft");
  await expect(photos).toBeFocused();
  await page.keyboard.press("h");
  await expect(tree.getByRole("treeitem", { name: "Hobbies" })).toBeFocused();
  await page.keyboard.press("Alt+Shift+ArrowDown");
  await expect(headings(page)).toHaveText([
    labels.virtualCollections,
    labels.collections,
    labels.folders,
  ]);
  await expect(tree.getByRole("treeitem", { name: "Hobbies" })).toBeFocused();
  expect(await tree.locator('[role="treeitem"][tabindex="0"]').count()).toBe(3);
  expect(errors).toEqual([]);
});

test("pins, section order and library collapse are kept on the device across a reload", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await open(page);
  await mountNavigation(page, twoLibraries);
  const tree = page.locator("fs-nav-tree");
  await tree.getByRole("treeitem", { name: "Archive" }).focus();
  await page.keyboard.press("ArrowLeft");
  await expect(tree.getByRole("treeitem", { name: "Archive" })).toHaveAttribute(
    "aria-expanded",
    "false",
  );
  await tree.getByRole("treeitem", { name: "Photos" }).focus();
  await tree.getByRole("button", { name: "Pin Photos" }).click();
  await tree.getByRole("button", { name: `Move ${labels.collections} up` }).click();
  const saved = await page.evaluate(() => localStorage.getItem("filesquid.navigation"));
  expect(JSON.parse(saved ?? "null")).toEqual({
    version: 1,
    pinned: [{ node: "c1", kind: "collection" }],
    order: ["collection", "pinned", "virtual_collection", "folder"],
    collapsed: ["l2"],
  });
  expect(saved).not.toMatch(/Photos|Archive|Family/);
  await page.reload();
  await expect(page.locator("fs-app-shell header fs-host-indicator")).toHaveAttribute(
    "data-state",
    "ready",
  );
  await mountNavigation(page, twoLibraries);
  await expect(headings(page)).toHaveText([
    labels.collections,
    labels.navigationPinned,
    labels.virtualCollections,
    labels.folders,
  ]);
  await expect(tree.getByRole("treeitem", { name: "Archive" })).toHaveAttribute(
    "aria-expanded",
    "false",
  );
  await expect(tree.locator('[data-section="pinned"] [role="treeitem"]')).toHaveText(["Photos"]);
  expect(errors).toEqual([]);
});

test("refused device storage keeps the choices for this page and says why", async ({ page }) => {
  const errors = trackErrors(page);
  const warnings = trackWarnings(page);
  await page.addInitScript(() => {
    Object.defineProperty(window, "localStorage", {
      get() {
        throw new DOMException("storage is blocked", "SecurityError");
      },
    });
  });
  await open(page);
  await mountNavigation(page);
  await page
    .locator("fs-nav-tree")
    .getByRole("button", { name: `Move ${labels.collections} down` })
    .click();
  await expect(headings(page)).toHaveText([
    labels.virtualCollections,
    labels.collections,
    labels.folders,
  ]);
  expect(warnings.some((w) => w.includes("the navigation choices will not be kept"))).toBe(true);
  expect(errors).toEqual([]);
});

test("the tree shows the error and offline states and reads again on Retry", async ({
  page,
  context,
}) => {
  const errors = trackErrors(page);
  await open(page);
  await page.evaluate(() => ((window as unknown as Record<string, unknown>).__navFail = true));
  await mountNavigation(page, oneLibrary, false);
  const tree = page.locator("fs-nav-tree");
  await expect(tree.getByRole("alert")).toContainText(labels.stateError);
  await expect(tree.getByRole("tree")).toHaveCount(0);
  await context.setOffline(true);
  await tree.getByRole("button", { name: labels.stateRetry }).click();
  await expect(tree.getByRole("alert")).toContainText(labels.stateOffline);
  await context.setOffline(false);
  await page.evaluate(() => ((window as unknown as Record<string, unknown>).__navFail = false));
  await tree.getByRole("button", { name: labels.stateRetry }).click();
  await expect(tree.getByRole("tree")).toHaveCount(3);
  expect(errors).toEqual([]);
});

for (const scheme of ["light", "dark"] as const) {
  test(`the five states are distinct and use token colours in the ${scheme} theme`, async ({
    page,
  }) => {
    const errors = trackErrors(page);
    await page.emulateMedia({ colorScheme: scheme });
    await open(page);
    const seen = await page.evaluate((table) => {
      type States = HTMLElement & { configure(o: unknown): void; state: unknown };
      const states = document.createElement("fs-states") as States;
      states.configure({ labels: table });
      document.querySelector("fs-app-shell")?.append(states);
      const kinds = [
        { kind: "loading" },
        { kind: "empty" },
        { kind: "error" },
        { kind: "offline" },
        { kind: "denied", reason: "not_permitted" },
      ];
      return kinds.map((state) => {
        states.state = state;
        const root = states.shadowRoot;
        const status = root?.querySelector('[role="status"]');
        const alert = root?.querySelector('[role="alert"]');
        const region = alert?.textContent ? alert : status;
        const title = region?.querySelector("p");
        const icon = region?.querySelector("svg");
        const words = [...(alert?.children ?? [])].filter((c) => c.tagName.toLowerCase() !== "svg");
        return {
          status: status?.textContent ?? "",
          alert: words.map((c) => c.textContent).join("|"),
          colour: title ? getComputedStyle(title).color : "",
          icon: icon ? getComputedStyle(icon).color : "",
          name: icon instanceof SVGElement ? (icon.dataset.icon ?? "") : "",
        };
      });
    }, labels);
    // Mockup 56 (UI-S-026): every title in the body colour, each state with its own icon, the
    // error icon in the error colour.
    const error = await tokenColour(page, "--fs-error");
    const body = await tokenColour(page, "--fs-on-surface");
    const icon = await tokenColour(page, "--fs-icon");
    expect(seen).toEqual([
      { status: labels.stateLoading, alert: "", colour: body, icon, name: "clock" },
      { status: labels.stateEmpty, alert: "", colour: body, icon, name: "collection" },
      {
        status: "",
        alert: `${labels.stateError}|${labels.fileActionFailed}|${labels.stateRetry}`,
        colour: body,
        icon: error,
        name: "error",
      },
      {
        status: "",
        alert: `${labels.stateOffline}|${labels.stateRetry}`,
        colour: body,
        icon,
        name: "offline",
      },
      {
        status: "",
        alert: `${labels.stateDenied}|${labels.authNotPermitted}`,
        colour: body,
        icon,
        name: "denied",
      },
    ]);
    expect(errors).toEqual([]);
  });

  test(`tree controls draw the 2px --fs-focus-ring ring in the ${scheme} theme`, async ({
    page,
  }) => {
    const errors = trackErrors(page);
    await page.emulateMedia({ colorScheme: scheme });
    await open(page);
    await mountNavigation(page);
    const ring = `solid 2px ${await tokenColour(page, "--fs-focus-ring")}`;
    // UI-S-025: the sidebar's Recent view comes just before the tree in the Tab order.
    await page.locator('fs-app-shell nav a[href="#/recent"]').focus();
    const seen = [];
    for (let step = 0; step < 4; step += 1) {
      await page.keyboard.press("Tab");
      seen.push(await focusRing(page));
    }
    expect(seen.map((s) => s.text)).toEqual([
      `Move ${labels.collections} up`,
      `Move ${labels.collections} down`,
      "Photos",
      "Pin Photos",
    ]);
    for (const item of seen) expect([item.ring, item.offset]).toEqual([ring, "2px"]);
    expect(errors).toEqual([]);
  });
}

test("tree motion follows the token, stops under reduced motion, and forced colours mark the current item", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await open(page);
  await mountNavigation(page);
  const durations = () =>
    page.evaluate(() => {
      const root = document.querySelector("fs-nav-tree")?.shadowRoot;
      return ["button.up", '[role="treeitem"]'].map((selector) => {
        const element = root?.querySelector(selector);
        return element ? getComputedStyle(element).transitionDuration : "";
      });
    });
  expect(await durations()).toEqual(["0.15s, 0.15s", "0.15s, 0.15s"]);
  await page.emulateMedia({ reducedMotion: "reduce" });
  expect(await durations()).toEqual(["0s", "0s"]);
  await page.evaluate(() => {
    const tree = document.querySelector("fs-nav-tree") as HTMLElement & { current?: string };
    tree.current = "c2";
  });
  await page.emulateMedia({ forcedColors: "active" });
  const current = page.locator("fs-nav-tree").getByRole("treeitem", { name: "Hobbies" });
  await expect(current).toHaveAttribute("aria-current", "page");
  const colours = await current.evaluate((el) => {
    const probe = document.createElement("span");
    probe.style.color = "Highlight";
    document.body.append(probe);
    const highlight = getComputedStyle(probe).color;
    probe.remove();
    return { background: getComputedStyle(el).backgroundColor, highlight };
  });
  expect(colours.background).toBe(colours.highlight);
  expect(errors).toEqual([]);
});

for (const { width, scale } of [
  { width: 320, scale: 100 },
  { width: 320, scale: 200 },
  { width: 1280, scale: 200 },
]) {
  test(`the tree and breadcrumb reflow at ${width} px and ${scale} % text`, async ({ page }) => {
    const errors = trackErrors(page);
    await page.setViewportSize({ width, height: 800 });
    await open(page);
    await page.addStyleTag({ content: `html { font-size: ${scale}%; }` });
    await mountNavigation(page);
    await page.locator("fs-nav-tree").getByRole("treeitem", { name: "Photos" }).focus();
    await page.keyboard.press("ArrowRight");
    await expect(
      page.locator("fs-nav-tree").getByRole("treeitem", { name: "2025", exact: true }),
    ).toBeVisible();
    expect(await scrollsHorizontally(page)).toBe(false);
    expect(errors).toEqual([]);
  });
}
