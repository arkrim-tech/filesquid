/**
 * UI tests for the virtualised `<fs-nav-tree>` (Task UI-F3.4, UI-S-035, UI-S-017, UI-S-016):
 * a level of 10,000 nodes keeps only its window plus two overscans of 5 in the DOM; the tree
 * items state their level, set size and position over the whole level; the keys reach every
 * node of an open level, drawn or not; the collapse state and the pins survive a reload, a pin
 * beyond the cap included; and a capped level says so on the console and on screen.
 *
 * jsdom has no layout, so the tests give the tree a viewport of their own: the sections stacked
 * as the stylesheet stacks them, 30 px rows, a 600 px view and a scroll position they set.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  NavigationStore,
  defineElements,
  formatLabel,
  type FsNavTree,
  type TreeBand,
  type TreeViewport,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import {
  BIG_LIBRARY,
  BigNavigationSource,
  bigHandle,
  bigName,
  type BigTreeOptions,
} from "../support/big-navigation";

/** Row height in pixels (1.875 rem at 16 px, mockup 55). */
const ROW = 30;
/** Height of the view in pixels: 20 rows. */
const VIEW = 600;
/** Height of a section's head, and of what follows its tree (the Pin toggle and the gap). */
const HEAD = 34;
const FOOT = 54;
/** Most tree items the DOM may hold: the rows in view, a row cut by an edge, two overscans. */
const BOUND = VIEW / ROW + 1 + 2 * 5;

/** A viewport the test scrolls: sections stacked in document order, no layout needed. */
class FakeViewport implements TreeViewport {
  /** How far the sidebar is scrolled, in pixels. */
  scrollTop = 0;
  private readonly listeners = new Set<() => void>();

  /** @param tree The tree whose sections are stacked. */
  constructor(private readonly tree: () => FsNavTree) {}

  rowHeight(): number {
    return ROW;
  }

  bands(lists: readonly HTMLElement[]): TreeBand[] {
    return lists.map((list) => {
      const top = this.scrollTop - this.offsetOf(list);
      return { top, bottom: top + VIEW };
    });
  }

  follow(changed: () => void): () => void {
    this.listeners.add(changed);
    return () => void this.listeners.delete(changed);
  }

  /** Scroll to a position and tell the tree, as a scroll event does. */
  scrollTo(top: number): void {
    this.scrollTop = top;
    for (const listener of this.listeners) listener();
  }

  /** Scroll so the row at `index` of a section's tree is the first row in view. */
  scrollToRow(section: string, index: number): void {
    this.scrollTo(
      this.offsetOf(listOf(this.tree().shadowRoot as ShadowRoot, section)) + index * ROW,
    );
  }

  /** Number of listeners following the viewport. */
  get followers(): number {
    return this.listeners.size;
  }

  /** The distance from the top of the tree to the top of a section's list. */
  private offsetOf(list: HTMLElement): number {
    let offset = 0;
    for (const other of this.tree().shadowRoot?.querySelectorAll<HTMLElement>('[role="tree"]') ??
      []) {
      offset += HEAD;
      if (other === list) return offset;
      offset += rowsOf(other) * ROW + FOOT;
    }
    throw new Error("the list is not in the tree");
  }
}

/** The rows a list says it holds (`--rows`), drawn or not. */
function rowsOf(list: HTMLElement): number {
  return Number(list.style.getPropertyValue("--rows"));
}

/** A section's tree. */
function listOf(root: ShadowRoot, section: string): HTMLElement {
  const list = root.querySelector<HTMLElement>(`[data-section="${section}"] [role="tree"]`);
  if (!list) throw new Error(`no tree in ${section}`);
  return list;
}

/** The drawn tree items of a section, or of the whole tree. */
function items(root: ShadowRoot, section?: string): HTMLAnchorElement[] {
  const scope = section ? `[data-section="${section}"] ` : "";
  return [...root.querySelectorAll<HTMLAnchorElement>(`${scope}[role="treeitem"]`)];
}

/** The drawn tree item named `name`, or undefined. */
function named(
  root: ShadowRoot,
  name: string,
  section = "collection",
): HTMLAnchorElement | undefined {
  return items(root, section).find((a) => a.querySelector(".name")?.textContent === name);
}

/** Press a key on the focused element inside the tree. */
function press(root: ShadowRoot, key: string): void {
  const event = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true });
  (root.activeElement ?? root.host).dispatchEvent(event);
}

/** Let pending source answers and renders finish. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Mount a tree over a long level, a store over `data` and a fake viewport. */
async function mount(options: BigTreeOptions, data = new Map<string, string>()) {
  // The same map across two mounts is the device storage across a reload.
  const storage = {
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => void data.set(key, value),
    removeItem: (key: string) => void data.delete(key),
  };
  const store = new NavigationStore({ storage });
  store.start();
  const source = new BigNavigationSource(options);
  const tree = document.createElement("fs-nav-tree") as FsNavTree;
  const viewport = new FakeViewport(() => tree);
  tree.configure({ labels, source, store, viewport });
  document.body.replaceChildren(tree);
  await settle();
  return { tree, source, store, data, viewport, root: tree.shadowRoot as ShadowRoot };
}

/** A stored value of navigation choices. */
function saved(value: object): Map<string, string> {
  return new Map([["filesquid.navigation", JSON.stringify({ version: 1, ...value })]]);
}

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

describe("<fs-nav-tree> window (UI-S-035)", () => {
  it("keeps at most the window plus two overscans of 5 tree items for 10,000 nodes", async () => {
    const { root, viewport } = await mount({ total: 10_000 });
    const list = listOf(root, "collection");
    expect(rowsOf(list)).toBe(10_000);
    expect(items(root).length).toBeGreaterThan(VIEW / ROW);
    for (const top of [0, 15, 4_000, 150_000, 299_000, 300_000, 310_000]) {
      viewport.scrollTo(top);
      expect(items(root).length, `at ${top}`).toBeLessThanOrEqual(BOUND);
      expect(root.querySelectorAll("li").length, `at ${top}`).toBeLessThanOrEqual(BOUND);
    }
  });

  it("draws the rows around the scroll position, each placed by its row index", async () => {
    const { root, viewport } = await mount({ total: 10_000 });
    viewport.scrollToRow("collection", 5000);
    const drawn = items(root, "collection");
    expect(drawn[0]?.textContent).toBe(bigName(4995));
    expect(drawn.at(-1)?.textContent).toBe(bigName(5024));
    for (const [at, item] of drawn.entries()) {
      const row = item.closest("li") as HTMLElement;
      expect(row.getAttribute("role")).toBe("none");
      expect(row.style.getPropertyValue("--r")).toBe(String(4995 + at));
    }
  });

  it("keeps the nodes in document order however the window moved", async () => {
    const { root, viewport } = await mount({ total: 10_000 });
    for (const row of [40, 30, 35, 5000, 4990, 12]) viewport.scrollToRow("collection", row);
    const places = items(root, "collection").map((item) =>
      Number(item.getAttribute("aria-posinset")),
    );
    expect(places).toEqual([...places].sort((a, b) => a - b));
    expect(new Set(places).size).toBe(places.length);
  });

  it("stops following the viewport once detached, and follows again when attached", async () => {
    const { tree, viewport } = await mount({ total: 10_000 });
    expect(viewport.followers).toBe(1);
    tree.remove();
    expect(viewport.followers).toBe(0);
    document.body.append(tree);
    await settle();
    expect(viewport.followers).toBe(1);
  });

  it("draws the same window again after a change event reads the tree again", async () => {
    const { root, viewport, source } = await mount({ total: 10_000 });
    viewport.scrollToRow("collection", 5000);
    source.emit({ all: true });
    await settle();
    await settle();
    expect(items(root, "collection")[0]?.textContent).toBe(bigName(4995));
    expect(items(root).length).toBeLessThanOrEqual(BOUND);
  });
});

describe("<fs-nav-tree> tree semantics over the whole level (UI-S-035, UI-S-016)", () => {
  it("gives every drawn item its role, level, set size and place in the whole level", async () => {
    const { root, viewport } = await mount({ total: 10_000 });
    viewport.scrollToRow("collection", 5000);
    const list = listOf(root, "collection");
    expect(list.getAttribute("role")).toBe("tree");
    expect(list.getAttribute("aria-labelledby")).toBe(
      root.querySelector('[data-section="collection"] h2')?.id,
    );
    const drawn = items(root, "collection");
    expect(drawn.length).toBeGreaterThan(20);
    for (const item of drawn) {
      const place = Number(item.querySelector(".name")?.textContent?.replace("Harbour ", ""));
      expect(item.getAttribute("role")).toBe("treeitem");
      expect(item.getAttribute("aria-level")).toBe("1");
      expect(item.getAttribute("aria-setsize")).toBe("10000");
      expect(item.getAttribute("aria-posinset")).toBe(String(place));
    }
  });

  it("counts the children of an open node in their own level", async () => {
    const { root } = await mount({ total: 10_000, folders: (i) => (i === 2 ? 3 : 0) });
    const parent = named(root, bigName(2)) as HTMLAnchorElement;
    expect(parent.getAttribute("aria-expanded")).toBe("false");
    expect(named(root, bigName(1))?.hasAttribute("aria-expanded")).toBe(false);
    parent.focus();
    press(root, "ArrowRight");
    await settle();
    expect(named(root, bigName(2))?.getAttribute("aria-expanded")).toBe("true");
    const inner = named(root, "Inner 2") as HTMLAnchorElement;
    expect(inner.getAttribute("aria-level")).toBe("2");
    expect(inner.getAttribute("aria-setsize")).toBe("3");
    expect(inner.getAttribute("aria-posinset")).toBe("2");
    expect(named(root, bigName(3))?.getAttribute("aria-posinset")).toBe("4");
    expect(rowsOf(listOf(root, "collection"))).toBe(10_003);
  });

  it("marks the current destination when its row comes into the window", async () => {
    const { root, tree, viewport } = await mount({ total: 10_000 });
    tree.current = bigHandle(7000);
    expect(root.querySelectorAll('[aria-current="page"]')).toHaveLength(0);
    viewport.scrollToRow("collection", 6995);
    expect(named(root, bigName(7000))?.getAttribute("aria-current")).toBe("page");
    expect(root.querySelectorAll('[aria-current="page"]')).toHaveLength(1);
    tree.current = bigHandle(7001);
    expect(named(root, bigName(7000))?.hasAttribute("aria-current")).toBe(false);
    expect(named(root, bigName(7001))?.getAttribute("aria-current")).toBe("page");
  });

  it("keeps one Tab stop in the tree wherever the window is, and no positive tabindex", async () => {
    const { root, viewport, tree } = await mount({ total: 10_000 });
    const stops = () => items(root, "collection").filter((a) => a.getAttribute("tabindex") === "0");
    expect(stops().map((a) => a.textContent)).toEqual([bigName(0)]);
    viewport.scrollToRow("collection", 5000);
    // The remembered item is not drawn, so the first row in view stands in for it.
    expect(stops().map((a) => a.textContent)).toEqual([bigName(5000)]);
    tree.current = bigHandle(5010);
    expect(stops().map((a) => a.textContent)).toEqual([bigName(5010)]);
    const every = items(root).map((a) => a.getAttribute("tabindex"));
    expect(every.every((value) => value === "0" || value === "-1")).toBe(true);
  });

  it("words the Pin toggle for the tree's active item, drawn or not", async () => {
    const { root, viewport } = await mount({ total: 10_000 });
    named(root, bigName(3))?.focus();
    viewport.scrollToRow("collection", 5000);
    const pin = root.querySelector<HTMLButtonElement>('[data-section="collection"] button.pin');
    expect(pin?.dataset.node).toBe(bigHandle(3));
    expect(pin?.getAttribute("aria-label")).toBe(
      formatLabel(labels.navigationPinItem, { name: bigName(3) }),
    );
  });
});

describe("<fs-nav-tree> keyboard over undrawn nodes (UI-S-035, UI-S-016)", () => {
  it("End draws and focuses the last of 10,000 nodes, Home the first", async () => {
    const { root } = await mount({ total: 10_000 });
    named(root, bigName(0))?.focus();
    expect(named(root, bigName(9999))).toBeUndefined();
    press(root, "End");
    expect(root.activeElement?.textContent).toBe(bigName(9999));
    expect(root.activeElement?.getAttribute("aria-posinset")).toBe("10000");
    expect(items(root, "collection").filter((a) => a.tabIndex === 0)).toEqual([root.activeElement]);
    expect(items(root).length).toBeLessThanOrEqual(BOUND + 1);
    press(root, "Home");
    expect(root.activeElement?.textContent).toBe(bigName(0));
    expect(items(root).length).toBeLessThanOrEqual(BOUND + 1);
  });

  it("the arrow keys walk past the window, one node at a time", async () => {
    const { root, viewport } = await mount({ total: 10_000 });
    named(root, bigName(0))?.focus();
    for (let step = 1; step <= 60; step += 1) {
      press(root, "ArrowDown");
      expect(root.activeElement?.textContent).toBe(bigName(step));
      expect(items(root).length).toBeLessThanOrEqual(BOUND + 1);
    }
    viewport.scrollToRow("collection", 55);
    press(root, "ArrowUp");
    expect(root.activeElement?.textContent).toBe(bigName(59));
    expect(items(root).length).toBeLessThanOrEqual(BOUND);
  });

  it("opens, enters, leaves and closes a node that was not drawn at first", async () => {
    const { root, source } = await mount({ total: 10_000, folders: (i) => (i === 9999 ? 2 : 0) });
    named(root, bigName(0))?.focus();
    press(root, "End");
    expect(root.activeElement?.getAttribute("aria-expanded")).toBe("false");
    press(root, "ArrowRight");
    await settle();
    expect(source.calls.at(-1)?.parent).toBe(bigHandle(9999));
    expect(root.activeElement?.textContent).toBe(bigName(9999));
    expect(root.activeElement?.getAttribute("aria-expanded")).toBe("true");
    press(root, "ArrowRight");
    expect(root.activeElement?.textContent).toBe("Inner 1");
    expect(root.activeElement?.getAttribute("aria-level")).toBe("2");
    press(root, "End");
    expect(root.activeElement?.textContent).toBe("Inner 2");
    press(root, "ArrowLeft");
    expect(root.activeElement?.textContent).toBe(bigName(9999));
    press(root, "ArrowLeft");
    expect(root.activeElement?.getAttribute("aria-expanded")).toBe("false");
    expect(rowsOf(listOf(root, "collection"))).toBe(10_000);
  });

  it("* opens every closed sibling of the level, drawn or not", async () => {
    const { root, source } = await mount({
      total: 10_000,
      folders: (i) => (i === 10 || i === 8000 ? 1 : 0),
    });
    named(root, bigName(0))?.focus();
    press(root, "*");
    await settle();
    const opened = source.calls.map((call) => call.parent).filter((p) => p?.startsWith("c-"));
    expect(opened.sort()).toEqual([bigHandle(10), bigHandle(8000)].sort());
    expect(rowsOf(listOf(root, "collection"))).toBe(10_002);
  });

  it("a typed character finds a node far outside the window", async () => {
    const { root } = await mount({
      total: 10_000,
      name: (i) => (i === 7777 ? "Quay" : bigName(i)),
    });
    named(root, bigName(0))?.focus();
    press(root, "q");
    expect(root.activeElement?.textContent).toBe("Quay");
    expect(root.activeElement?.getAttribute("aria-posinset")).toBe("7778");
  });

  it("keeps the focused item while it is scrolled out of view, and lets it go after", async () => {
    const { root, viewport } = await mount({ total: 10_000 });
    const first = named(root, bigName(0)) as HTMLAnchorElement;
    first.focus();
    viewport.scrollToRow("collection", 5000);
    expect(root.activeElement).toBe(first);
    expect(first.isConnected).toBe(true);
    expect(items(root).length).toBeLessThanOrEqual(BOUND + 1);
    first.blur();
    viewport.scrollToRow("collection", 5001);
    expect(first.isConnected).toBe(false);
    expect(items(root).length).toBeLessThanOrEqual(BOUND);
  });

  it("keeps focus on the same item when the tree is read again", async () => {
    const { root, source } = await mount({ total: 10_000 });
    named(root, bigName(0))?.focus();
    press(root, "End");
    source.emit({ all: true });
    await settle();
    await settle();
    expect(root.activeElement?.textContent).toBe(bigName(9999));
  });
});

describe("<fs-nav-tree> level cap (UI-S-017)", () => {
  it("shows the first 10,000 nodes of a longer level and says so on the console", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { root, source } = await mount({ total: 50_000 });
    expect(source.calls.filter((call) => call.parent === BIG_LIBRARY)).toHaveLength(50);
    expect(rowsOf(listOf(root, "collection"))).toBe(10_002);
    expect(named(root, bigName(0))?.getAttribute("aria-setsize")).toBe("10000");
    const said = warn.mock.calls.map((call) => String(call[0]));
    expect(
      said.filter((line) => line.includes("first 50 pages of a navigation level")),
    ).toHaveLength(1);
  });

  it("shows the limit line under the last node, worded from the label table", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { root } = await mount({ total: 50_000 });
    expect(root.querySelector(".limit")).toBeNull();
    named(root, bigName(0))?.focus();
    press(root, "End");
    const last = root.activeElement as HTMLElement;
    expect(last.textContent).toBe(bigName(9999));
    const line = root.querySelector<HTMLElement>('[data-section="collection"] li.limit');
    expect(labels.navigationLevelLimit).toBe(
      "Showing the first {count} only. Use search to find the rest.",
    );
    expect(line?.textContent).toBe("Showing the first 10,000 only. Use search to find the rest.");
    expect(line?.textContent).toBe(formatLabel(labels.navigationLevelLimit, { count: "10,000" }));
    expect(line?.getAttribute("role")).toBe("none");
    expect(line?.querySelector('[role="treeitem"]')).toBeNull();
    // It is the row after the last node, two rows tall, and the last node's description.
    expect(line?.style.getPropertyValue("--r")).toBe("10000");
    expect(last.getAttribute("aria-describedby")).toBe(line?.querySelector("span")?.id);
    expect(line?.querySelector("span")?.id).toBeTruthy();
  });

  it("shows no limit line for a level that fits", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { root } = await mount({ total: 10_000 });
    named(root, bigName(0))?.focus();
    press(root, "End");
    expect(root.querySelector(".limit")).toBeNull();
    expect(root.activeElement?.hasAttribute("aria-describedby")).toBe(false);
    expect(warn).not.toHaveBeenCalled();
  });

  it("shows the limit line of an opened node's capped level under its last child", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { root, viewport } = await mount({ total: 3, folders: (i) => (i === 1 ? 10_050 : 0) });
    named(root, bigName(1))?.focus();
    press(root, "ArrowRight");
    await settle();
    // Rows: Harbour 1, Harbour 2, its 10,000 folders, the line (two rows), Harbour 3.
    expect(rowsOf(listOf(root, "collection"))).toBe(10_005);
    viewport.scrollToRow("collection", 9995);
    const line = root.querySelector<HTMLElement>("li.limit");
    expect(line?.style.getPropertyValue("--r")).toBe("10002");
    const last = named(root, "Inner 10000") as HTMLAnchorElement;
    expect(last.getAttribute("aria-describedby")).toBe(line?.querySelector("span")?.id);
    // The line is indented as the nodes of its level are, one level in from their parent.
    const indent = (element?: HTMLElement | null) =>
      element?.style.getPropertyValue("padding-inline-start");
    expect(indent(line)).toBe(indent(last));
    expect(indent(line)).toContain("1 * (var(--fs-space-3) + var(--fs-space-1))");
    expect(indent(named(root, bigName(2)))).toContain("0 * (var(--fs-space-3)");
    expect(named(root, bigName(2))?.closest("li")?.style.getPropertyValue("--r")).toBe("10004");
    expect(items(root).length).toBeLessThanOrEqual(BOUND + 1);
  });
});

describe("<fs-nav-tree> saved choices (UI-S-017)", () => {
  it("opens a pin beyond the cap by its handle, and keeps it across a reload", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const data = saved({ pinned: [{ node: bigHandle(47_999), kind: "collection" }] });
    const first = await mount({ total: 50_000 }, data);
    const pin = named(first.root, "Harbour 48000", "pinned");
    expect(pin?.getAttribute("href")).toBe(`#/navigation/${bigHandle(47_999)}`);
    expect(first.source.calls.some((call) => call.parent === bigHandle(47_999))).toBe(true);
    // The node is not among the 10,000 the level shows.
    expect(rowsOf(listOf(first.root, "collection"))).toBe(10_002);
    const again = await mount({ total: 50_000 }, data);
    expect(named(again.root, "Harbour 48000", "pinned")).toBeDefined();
    expect(again.store.preferences.pinned).toEqual([
      { node: bigHandle(47_999), kind: "collection" },
    ]);
    expect(items(again.root).length).toBeLessThanOrEqual(BOUND);
  });

  it("keeps 100 pins and 1,000 collapsed nodes across a reload, inside the window", async () => {
    const pinned = Array.from({ length: 100 }, (_, i) => ({
      node: bigHandle(i * 90),
      kind: "collection",
    }));
    const collapsed = [...Array.from({ length: 999 }, (_, i) => `gone-${i}`), BIG_LIBRARY];
    const data = saved({ pinned, collapsed });
    const first = await mount({ total: 10_000 }, data);
    expect(first.store.preferences.collapsed).toHaveLength(1000);
    expect(first.store.preferences.pinned).toHaveLength(100);
    expect(rowsOf(listOf(first.root, "pinned"))).toBe(100);
    expect(items(first.root).length).toBeLessThanOrEqual(BOUND);
    const again = await mount({ total: 10_000 }, data);
    expect(again.store.preferences.collapsed).toEqual(collapsed);
    expect(again.store.preferences.pinned).toEqual(pinned);
    expect(named(again.root, bigName(90), "pinned")?.getAttribute("aria-setsize")).toBe("100");
    expect(named(again.root, bigName(90), "pinned")?.getAttribute("aria-posinset")).toBe("2");
    expect(items(again.root).length).toBeLessThanOrEqual(BOUND);
  });

  it("shows a pin the source answers not_found for as unavailable, with Remove", async () => {
    const data = saved({ pinned: [{ node: "c-99999", kind: "collection" }] });
    const { root, store } = await mount({ total: 10_000 }, data);
    const entry = root.querySelector('[data-section="pinned"] .unavailable li');
    expect(entry?.querySelector(".name")?.textContent).toBe(
      formatLabel(labels.navigationUnavailable, { kind: labels.collection }),
    );
    root.querySelector<HTMLButtonElement>('[data-section="pinned"] button.remove')?.click();
    expect(store.preferences.pinned).toEqual([]);
  });
});
