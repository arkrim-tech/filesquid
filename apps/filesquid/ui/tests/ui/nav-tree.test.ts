/**
 * UI tests for `<fs-nav-tree>` (F-004 default UI, UI-S-017, UI-F2.1a): kind sections worded
 * from the label table over an injected source shaped on API-S-021, ARIA treeview keys with
 * children read as nodes open, pinned destinations (unavailable ones with a Remove action),
 * section reordering, library collapse kept in the store, the distinct load states, change
 * events, and the focus rules of UI-S-016.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  MemoryNavigationSource,
  NavigationStore,
  accessibilityViolations,
  defineElements,
  formatLabel,
  type FixtureNode,
  type FsNavTree,
  type NavigationSource,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { fakeStorage } from "../support/theme-fakes";

/** One library: two collections (one with folders), a virtual collection and a folder. */
const oneLibrary: FixtureNode = {
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
          children: [
            { node: "f1", kind: "folder", name: "2025" },
            { node: "f2", kind: "folder", name: "Archive" },
          ],
        },
        { node: "c2", kind: "collection", name: "Hobbies" },
        { node: "v1", kind: "virtual_collection", name: "Best of 2025" },
        { node: "f9", kind: "folder", name: "inbox" },
      ],
    },
  ],
};

/** Two libraries, each with a collection. */
const twoLibraries: FixtureNode = {
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

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

/** Let pending source answers and renders finish. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Mount a tree over a source and a store with `saved` choices, and wait for its first render. */
async function mount<S extends NavigationSource = MemoryNavigationSource>(
  source: S = new MemoryNavigationSource(oneLibrary) as unknown as S,
  saved?: object,
  online = true,
) {
  const initial: Record<string, string> = saved
    ? { "filesquid.navigation": JSON.stringify({ version: 1, ...saved }) }
    : {};
  const { storage, data } = fakeStorage(initial);
  const store = new NavigationStore({ storage });
  store.start();
  const tree = document.createElement("fs-nav-tree") as FsNavTree;
  tree.configure({ labels, source, store, online: () => online });
  document.body.replaceChildren(tree);
  await settle();
  return { tree, source, store, data, root: tree.shadowRoot as ShadowRoot };
}

/** Section headings, in display order. */
function headings(root: ShadowRoot): string[] {
  return [...root.querySelectorAll("section h2")].map((h) => h.textContent ?? "");
}

/** The treeitem named `name` in section `section`. */
function item(root: ShadowRoot, section: string, name: string): HTMLAnchorElement {
  const found = [
    ...root.querySelectorAll<HTMLAnchorElement>(`[data-section="${section}"] [role="treeitem"]`),
  ].find((a) => a.querySelector(".name")?.textContent === name);
  if (!found) throw new Error(`no item ${name} in ${section}`);
  return found;
}

/** A section's button by class. */
function button(root: ShadowRoot, section: string, cls: string): HTMLButtonElement {
  const found = root.querySelector<HTMLButtonElement>(`[data-section="${section}"] button.${cls}`);
  if (!found) throw new Error(`no ${cls} button in ${section}`);
  return found;
}

/** Press a key on the focused element inside the tree. */
function press(root: ShadowRoot, key: string, init: KeyboardEventInit = {}): KeyboardEvent {
  const event = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...init });
  (root.activeElement ?? root.host).dispatchEvent(event);
  return event;
}

/** The inner states element. */
const statesOf = (root: ShadowRoot) => root.querySelector("fs-states") as HTMLElement;

describe("<fs-nav-tree> structure", () => {
  it("is a defined custom element", () => {
    expect(customElements.get("fs-nav-tree")).toBeDefined();
  });

  it("groups one library's children into kind sections worded from the label table", async () => {
    const { root, tree } = await mount();
    expect(headings(root)).toEqual([labels.collections, labels.virtualCollections, labels.folders]);
    const collections = root.querySelector('[data-section="collection"] [role="tree"]');
    expect(collections?.getAttribute("aria-labelledby")).toBe(
      root.querySelector('[data-section="collection"] h2')?.id,
    );
    const photos = item(root, "collection", "Photos");
    expect(photos.getAttribute("href")).toBe("#/navigation/c1");
    expect(photos.getAttribute("aria-expanded")).toBe("false");
    expect(photos.getAttribute("aria-level")).toBe("1");
    expect(item(root, "collection", "Hobbies").hasAttribute("aria-expanded")).toBe(false);
    expect(item(root, "virtual_collection", "Best of 2025").hasAttribute("aria-expanded")).toBe(
      false,
    );
    expect(item(root, "folder", "inbox").dataset.kind).toBe("folder");
    expect(accessibilityViolations(tree)).toEqual([]);
  });

  it("gives each tree exactly one Tab stop and no positive tabindex", async () => {
    const { root } = await mount();
    for (const tree of root.querySelectorAll('[role="tree"]')) {
      const stops = [...tree.querySelectorAll('[role="treeitem"]')].map((a) =>
        a.getAttribute("tabindex"),
      );
      expect(stops.filter((t) => t === "0")).toHaveLength(1);
      expect(stops.every((t) => t === "0" || t === "-1")).toBe(true);
    }
  });

  it("marks the current destination with aria-current and makes it the Tab stop", async () => {
    const { root, tree } = await mount();
    tree.current = "c2";
    expect(item(root, "collection", "Hobbies").getAttribute("aria-current")).toBe("page");
    expect(item(root, "collection", "Hobbies").getAttribute("tabindex")).toBe("0");
    expect(item(root, "collection", "Photos").getAttribute("tabindex")).toBe("-1");
  });

  it("takes routes from the configured href function", async () => {
    const store = new NavigationStore({});
    store.start();
    const tree = document.createElement("fs-nav-tree") as FsNavTree;
    const source = new MemoryNavigationSource(oneLibrary);
    tree.configure({ labels, source, store, href: (n) => `#/x/${n.kind}/${n.node}` });
    document.body.replaceChildren(tree);
    await settle();
    expect(item(tree.shadowRoot as ShadowRoot, "collection", "Photos").getAttribute("href")).toBe(
      "#/x/collection/c1",
    );
  });

  it("puts the shared accessibility stylesheet first", async () => {
    const { root } = await mount();
    expect(root.querySelector("style")?.textContent).toMatch(
      /^\s*:focus-visible \{ outline: 2px solid var\(--fs-primary\)/,
    );
  });
});

describe("<fs-nav-tree> keyboard", () => {
  it("moves through visible items with the arrow keys, Home and End", async () => {
    const { root } = await mount();
    item(root, "collection", "Photos").focus();
    press(root, "ArrowDown");
    expect(root.activeElement?.textContent).toBe("Hobbies");
    expect(item(root, "collection", "Photos").getAttribute("tabindex")).toBe("-1");
    press(root, "Home");
    expect(root.activeElement?.textContent).toBe("Photos");
    press(root, "End");
    expect(root.activeElement?.textContent).toBe("Hobbies");
  });

  it("opens a collection by reading its children, enters them, and closes it again", async () => {
    const { root, source } = await mount();
    const spy = vi.spyOn(source, "list");
    item(root, "collection", "Photos").focus();
    press(root, "ArrowRight");
    expect(item(root, "collection", "Photos").getAttribute("aria-busy")).toBe("true");
    await settle();
    expect(spy).toHaveBeenCalledWith("c1", null);
    const photos = item(root, "collection", "Photos");
    expect(photos.getAttribute("aria-expanded")).toBe("true");
    expect(photos.hasAttribute("aria-busy")).toBe(false);
    expect(root.activeElement).toBe(photos);
    expect(root.getElementById(photos.getAttribute("aria-owns") ?? "")?.getAttribute("role")).toBe(
      "group",
    );
    expect(item(root, "collection", "2025").getAttribute("aria-level")).toBe("2");
    press(root, "ArrowRight");
    expect(root.activeElement?.textContent).toBe("2025");
    press(root, "ArrowLeft");
    expect(root.activeElement?.textContent).toBe("Photos");
    press(root, "ArrowLeft");
    expect(item(root, "collection", "Photos").getAttribute("aria-expanded")).toBe("false");
  });

  it("expands every sibling with * and finds items by typed character", async () => {
    const { root } = await mount();
    item(root, "collection", "Hobbies").focus();
    press(root, "*");
    await settle();
    expect(item(root, "collection", "Photos").getAttribute("aria-expanded")).toBe("true");
    press(root, "a");
    expect(root.activeElement?.textContent).toBe("Archive");
  });

  it("leaves modified keys to the browser", async () => {
    const { root } = await mount();
    item(root, "collection", "Photos").focus();
    const event = press(root, "ArrowDown", { altKey: true });
    expect(event.defaultPrevented).toBe(false);
    expect(root.activeElement?.textContent).toBe("Photos");
  });

  it("moves the focused section with Alt+Shift+Down and keeps focus", async () => {
    const { root, store } = await mount();
    item(root, "collection", "Photos").focus();
    const event = press(root, "ArrowDown", { altKey: true, shiftKey: true });
    expect(event.defaultPrevented).toBe(true);
    expect(headings(root)).toEqual([labels.virtualCollections, labels.collections, labels.folders]);
    expect(store.preferences.order).toEqual(["virtual_collection", "collection", "folder"]);
    expect(root.activeElement?.textContent).toBe("Photos");
  });

  it("toggles a parent when its twisty is clicked, without following the link", async () => {
    const { root } = await mount();
    const twisty = item(root, "collection", "Photos").querySelector(".twisty") as HTMLElement;
    const click = new MouseEvent("click", { bubbles: true, cancelable: true });
    twisty.dispatchEvent(click);
    await settle();
    expect(click.defaultPrevented).toBe(true);
    expect(item(root, "collection", "Photos").getAttribute("aria-expanded")).toBe("true");
  });
});

describe("<fs-nav-tree> sections and pins", () => {
  it("reorders sections with named Move buttons, keeping focus on the pressed button", async () => {
    const { root, store } = await mount();
    const down = button(root, "collection", "down");
    expect(down.getAttribute("aria-label")).toBe(
      formatLabel(labels.navigationMoveDown, { name: labels.collections }),
    );
    expect(button(root, "collection", "up").getAttribute("aria-disabled")).toBe("true");
    down.focus();
    down.click();
    expect(store.preferences.order).toEqual(["virtual_collection", "collection", "folder"]);
    expect(root.activeElement).toBe(button(root, "collection", "down"));
    button(root, "virtual_collection", "up").click();
    expect(store.preferences.order).toEqual(["virtual_collection", "collection", "folder"]);
  });

  it("pins the active item into a Pinned section, storing its handle and kind only", async () => {
    const { root, store, data } = await mount();
    item(root, "collection", "Hobbies").focus();
    const pin = button(root, "collection", "pin");
    expect(pin.textContent).toBe(labels.navigationPin);
    expect(pin.getAttribute("aria-label")).toBe(
      formatLabel(labels.navigationPinItem, { name: "Hobbies" }),
    );
    expect(pin.getAttribute("aria-pressed")).toBe("false");
    pin.click();
    expect(store.preferences.pinned).toEqual([{ node: "c2", kind: "collection" }]);
    expect(data.get("filesquid.navigation")).not.toContain("Hobbies");
    expect(headings(root)[0]).toBe(labels.navigationPinned);
    expect(item(root, "pinned", "Hobbies").getAttribute("href")).toBe("#/navigation/c2");
    expect(button(root, "collection", "pin").getAttribute("aria-pressed")).toBe("true");
    button(root, "pinned", "pin").click();
    expect(store.preferences.pinned).toEqual([]);
    expect(root.querySelector('[data-section="pinned"]')).toBeNull();
  });

  it("restores pins, read through the source by handle, and the section order", async () => {
    const saved = { pinned: [{ node: "f2", kind: "folder" }], order: ["folder", "pinned"] };
    const { root } = await mount(undefined, saved);
    expect(headings(root)).toEqual([
      labels.folders,
      labels.navigationPinned,
      labels.collections,
      labels.virtualCollections,
    ]);
    expect(item(root, "pinned", "Archive").getAttribute("href")).toBe("#/navigation/f2");
  });

  it("shows a pin the source answers not_found for as unavailable, with a Remove action", async () => {
    const { root, store } = await mount(undefined, {
      pinned: [{ node: "gone", kind: "collection" }],
    });
    const entry = root.querySelector('[data-section="pinned"] .unavailable li');
    const unavailable = formatLabel(labels.navigationUnavailable, { kind: labels.collection });
    expect(entry?.querySelector("span")?.textContent).toBe(unavailable);
    const remove = button(root, "pinned", "remove");
    expect(remove.textContent).toBe(labels.navigationRemove);
    expect(remove.getAttribute("aria-label")).toBe(
      formatLabel(labels.navigationRemoveItem, { name: unavailable }),
    );
    remove.focus();
    remove.click();
    expect(store.preferences.pinned).toEqual([]);
    expect(root.querySelector('[data-section="pinned"]')).toBeNull();
    expect(root.activeElement?.getAttribute("role")).toBe("treeitem");
  });

  it("lists each library as an open parent when there are several, and keeps its collapse", async () => {
    const { root, store } = await mount(new MemoryNavigationSource(twoLibraries));
    const archive = item(root, "collection", "Archive");
    expect(archive.getAttribute("aria-expanded")).toBe("true");
    expect(item(root, "collection", "Old").getAttribute("aria-level")).toBe("2");
    archive.focus();
    press(root, "ArrowLeft");
    expect(store.preferences.collapsed).toEqual(["l2"]);
    expect(item(root, "collection", "Archive").getAttribute("aria-expanded")).toBe("false");
    expect(root.activeElement).toBe(item(root, "collection", "Archive"));
    const again = await mount(new MemoryNavigationSource(twoLibraries), { collapsed: ["l2"] });
    expect(item(again.root, "collection", "Archive").getAttribute("aria-expanded")).toBe("false");
  });
});

describe("<fs-nav-tree> load states", () => {
  it("shows loading until the first answer, then the sections, without moving focus", async () => {
    let release: () => void = () => undefined;
    const fixture = new MemoryNavigationSource(oneLibrary);
    const gate = new Promise<void>((resolve) => (release = resolve));
    const slow: NavigationSource = { list: async (...args) => (await gate, fixture.list(...args)) };
    const outside = document.createElement("button");
    const { root } = await mount(slow);
    document.body.append(outside);
    outside.focus();
    expect(statesOf(root).dataset.state).toBe("loading");
    expect(root.querySelector<HTMLElement>(".sections")?.hidden).toBe(true);
    release();
    await settle();
    expect(statesOf(root).hidden).toBe(true);
    expect(headings(root)).toHaveLength(3);
    expect(document.activeElement).toBe(outside);
  });

  it("shows the empty state for a repository without libraries", async () => {
    const { root } = await mount(
      new MemoryNavigationSource({ node: "r", kind: "repository", name: null }),
    );
    expect(statesOf(root).dataset.state).toBe("empty");
  });

  it("shows error, offline and permission denied distinctly, and logs the failure", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const failing = (error: unknown) => {
      const source = new MemoryNavigationSource(oneLibrary);
      source.failure = error;
      return source;
    };
    expect(statesOf((await mount(failing(new Error("x")))).root).dataset.state).toBe("error");
    expect(
      statesOf((await mount(failing(new TypeError("fetch")), undefined, false)).root).dataset.state,
    ).toBe("offline");
    const denied = new ApiError(403, "/api/v1/navigation", { reason: "not_permitted" });
    expect(statesOf((await mount(failing(denied))).root).dataset.state).toBe("denied");
    expect(warn).toHaveBeenCalledTimes(3);
  });

  it("reads again when Retry is pressed", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new MemoryNavigationSource(oneLibrary);
    source.failure = new Error("x");
    const { root } = await mount(source);
    source.failure = undefined;
    statesOf(root).shadowRoot?.querySelector<HTMLButtonElement>("button")?.click();
    await settle();
    expect(statesOf(root).hidden).toBe(true);
    expect(headings(root)).toHaveLength(3);
  });

  it("follows the source's change events, keeping opened nodes open", async () => {
    const { root, source } = await mount();
    item(root, "collection", "Photos").focus();
    press(root, "ArrowRight");
    await settle();
    source.replace({ ...oneLibrary });
    await settle();
    expect(item(root, "collection", "Photos").getAttribute("aria-expanded")).toBe("true");
    expect(item(root, "collection", "2025")).toBeDefined();
  });

  it("stops following the source once detached", async () => {
    const { tree, source } = await mount();
    const spy = vi.spyOn(source, "list");
    tree.remove();
    source.replace({ ...oneLibrary });
    await settle();
    expect(spy).not.toHaveBeenCalled();
  });
});

describe("<fs-nav-tree> asynchronous answers", () => {
  it("reads a failing pin once per load or store change, never in a loop", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new MemoryNavigationSource(oneLibrary);
    const original = source.list.bind(source);
    const reads = vi
      .spyOn(source, "list")
      .mockImplementation((parent, cursor, limit) =>
        parent === "f1"
          ? Promise.reject(new ApiError(503, "/api/v1/navigation"))
          : original(parent, cursor, limit),
      );
    const { root, store } = await mount(source, { pinned: [{ node: "f1", kind: "folder" }] });
    for (let i = 0; i < 5; i += 1) await settle();
    const pinReads = () => reads.mock.calls.filter(([parent]) => parent === "f1").length;
    expect(pinReads()).toBe(1);
    expect(root.querySelector('[data-section="pinned"]')).toBeNull();
    store.setCollapsed("l1", true);
    for (let i = 0; i < 5; i += 1) await settle();
    expect(pinReads()).toBe(2);
  });

  it("drops a child answer that arrives after a newer load", async () => {
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const source = new MemoryNavigationSource(oneLibrary);
    const original = source.list.bind(source);
    let held = true;
    vi.spyOn(source, "list").mockImplementation(async (parent, cursor, limit) => {
      if (parent === "c1" && held) {
        held = false;
        await gate;
      }
      return original(parent, cursor, limit);
    });
    const { root } = await mount(source);
    item(root, "collection", "Photos").focus();
    press(root, "ArrowRight");
    await settle();
    const renamed: FixtureNode = structuredClone(oneLibrary);
    const photos = renamed.children?.[0]?.children?.[0] as { children?: FixtureNode[] };
    photos.children = [{ node: "f5", kind: "folder", name: "Renamed" }];
    source.replace(renamed);
    await settle();
    expect(item(root, "collection", "Renamed").getAttribute("aria-level")).toBe("2");
    release();
    await settle();
    expect(item(root, "collection", "Renamed")).toBeDefined();
    expect(() => item(root, "collection", "2025")).toThrow();
  });

  it("keeps the Pinned section, with Remove, when the repository has no library", async () => {
    const empty = new MemoryNavigationSource({ node: "r", kind: "repository", name: null });
    const { root, store } = await mount(empty, { pinned: [{ node: "gone", kind: "folder" }] });
    expect(statesOf(root).hidden).toBe(true);
    expect(headings(root)).toEqual([labels.navigationPinned]);
    button(root, "pinned", "remove").click();
    expect(store.preferences.pinned).toEqual([]);
    expect(statesOf(root).dataset.state).toBe("empty");
  });
});
