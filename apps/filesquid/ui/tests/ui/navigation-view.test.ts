/**
 * UI tests for `<fs-navigation-view>` and `bindNavigationRoute` (UI-F2.1b part 2, F-004,
 * API-S-021): `#/navigation/<handle>` shows the destination with a breadcrumb filled from one
 * listing's `ancestors`, marks the tree's current node, follows `navigation.change` for the
 * trail, and shows loading, a gone destination (`not_found`), the unavailable state (503),
 * error and denied with no guessed trail.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  MemoryNavigationSource,
  accessibilityViolations,
  bindNavigationRoute,
  defineElements,
  type FixtureNode,
  type FsBreadcrumb,
  type FsNavigationView,
  type FsStates,
  type NavigationPage,
  type NavigationSource,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";

const tree: FixtureNode = {
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
          children: [{ node: "f/1 é", kind: "folder", name: "2025" }],
        },
      ],
    },
  ],
};

beforeEach(() => defineElements());
afterEach(() => {
  vi.restoreAllMocks();
  window.location.hash = "";
});

/** Let pending answers and renders finish. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Mount a view of `node` over `source`. */
async function mount(node: string, source: NavigationSource = new MemoryNavigationSource(tree)) {
  const view = document.createElement("fs-navigation-view") as FsNavigationView;
  view.configure({ labels, source, online: () => true });
  view.node = node;
  document.body.replaceChildren(view);
  await settle();
  return { view, root: view.shadowRoot as ShadowRoot };
}

/** The breadcrumb's links: text, href and whether each is the current page. */
function trail(root: ShadowRoot) {
  const crumb = root.querySelector<FsBreadcrumb>("fs-breadcrumb");
  return [...(crumb?.shadowRoot?.querySelectorAll("a") ?? [])].map((a) => [
    a.textContent,
    a.getAttribute("href"),
    a.getAttribute("aria-current"),
  ]);
}

/** The view's states element. */
const statesOf = (root: ShadowRoot) => root.querySelector<FsStates>("fs-states") as FsStates;

describe("<fs-navigation-view>", () => {
  it("is a defined custom element", () => {
    expect(customElements.get("fs-navigation-view")).toBeDefined();
  });

  it("fills the breadcrumb from ancestors, repository first, the destination last and current", async () => {
    const { root, view } = await mount("f/1 é");
    expect(trail(root)).toEqual([
      [labels.repository, "#/navigation/r", null],
      ["Family", "#/navigation/l1", null],
      ["Photos", "#/navigation/c1", null],
      ["2025", `#/navigation/${encodeURIComponent("f/1 é")}`, "page"],
    ]);
    expect(root.querySelector("h2")?.textContent).toBe("2025");
    expect(view.dataset.state).toBe("ready");
    expect(accessibilityViolations(view)).toEqual([]);
  });

  it("asks for one node of the level only: the trail needs ancestors, not children", async () => {
    const source = new MemoryNavigationSource(tree);
    const spy = vi.spyOn(source, "list");
    await mount("c1", source);
    expect(spy.mock.calls).toEqual([["c1", null, 1]]);
  });

  it("says the files of a destination cannot be listed here yet", async () => {
    const { root } = await mount("c1");
    expect(root.querySelector(".files")?.textContent).toBe(labels.navigationFilesPending);
  });

  it("shows loading and no trail until the listing answers", async () => {
    let release: (page: NavigationPage) => void = () => undefined;
    const source: NavigationSource = { list: () => new Promise((resolve) => (release = resolve)) };
    const { root, view } = await mount("c1", source);
    expect(view.dataset.state).toBe("loading");
    expect(statesOf(root).state).toEqual({ kind: "loading" });
    expect(trail(root)).toEqual([]);
    release(await new MemoryNavigationSource(tree).list("c1", null, 1));
    await settle();
    expect(view.dataset.state).toBe("ready");
    expect(statesOf(root).hidden).toBe(true);
  });

  it("shows a gone destination for not_found, with no trail", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { root, view } = await mount("gone");
    expect(view.dataset.state).toBe("empty");
    expect(statesOf(root).state).toEqual({ kind: "empty", message: labels.navigationGone });
    expect(trail(root)).toEqual([]);
    expect(root.querySelector("h2")).toBeNull();
  });

  it.each([
    [new ApiError(503, "/api/v1/navigation", { code: "unavailable" }), "unavailable"],
    [new ApiError(500, "/api/v1/navigation"), "error"],
    [new ApiError(403, "/api/v1/navigation", { reason: "not_permitted" }), "denied"],
  ])("shows %s as the %s state", async (failure, kind) => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new MemoryNavigationSource(tree);
    source.failure = failure;
    const { root, view } = await mount("c1", source);
    expect(view.dataset.state).toBe(kind);
    expect(statesOf(root).state.kind).toBe(kind);
    expect(trail(root)).toEqual([]);
  });

  it("reads again on Retry and shows the trail", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new MemoryNavigationSource(tree);
    source.failure = new ApiError(503, "/api/v1/navigation", { code: "unavailable" });
    const { root } = await mount("c1", source);
    source.failure = undefined;
    statesOf(root).shadowRoot?.querySelector("button")?.click();
    await settle();
    expect(trail(root).map(([name]) => name)).toEqual([labels.repository, "Family", "Photos"]);
  });

  it("reads the trail again when a change names one of its nodes, or all", async () => {
    const source = new MemoryNavigationSource(tree);
    const { root } = await mount("c1", source);
    const spy = vi.spyOn(source, "list");
    source.emit({ parents: ["elsewhere"] });
    await settle();
    expect(spy).not.toHaveBeenCalled();
    const lib = tree.children?.[0] as FixtureNode;
    source.replace({ ...tree, children: [{ ...lib, name: "Kin" }] }, { parents: ["r"] });
    await settle();
    expect(trail(root).map(([name]) => name)).toEqual([labels.repository, "Kin", "Photos"]);
    source.emit({ all: true });
    await settle();
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it("stops following the source once detached", async () => {
    const source = new MemoryNavigationSource(tree);
    const { view } = await mount("c1", source);
    const spy = vi.spyOn(source, "list");
    view.remove();
    source.emit({ all: true });
    await settle();
    expect(spy).not.toHaveBeenCalled();
  });

  it("drops an answer for a destination it has already left, even when it answers last", async () => {
    const memory = new MemoryNavigationSource(tree);
    const waiting = new Map<string, () => void>();
    const source: NavigationSource = {
      list: async (parent, cursor, limit) => {
        const page = await memory.list(parent, cursor, limit);
        if (parent !== "c1") await new Promise<void>((go) => waiting.set(parent as string, go));
        return page;
      },
    };
    const { root, view } = await mount("c1", source);
    view.node = "l1";
    view.node = "f/1 é";
    await settle();
    waiting.get("f/1 é")?.();
    await settle();
    waiting.get("l1")?.();
    await settle();
    expect(trail(root).at(-1)?.[0]).toBe("2025");
  });

  it("keeps the trail on screen while a change reads it again", async () => {
    const memory = new MemoryNavigationSource(tree);
    let release: () => void = () => undefined;
    let hold = false;
    let reads = 0;
    const source: NavigationSource = {
      list: async (parent, cursor, limit) => {
        reads += 1;
        const page = await memory.list(parent, cursor, limit);
        if (hold) await new Promise<void>((go) => (release = go));
        return page;
      },
      subscribe: (listener) => memory.subscribe(listener),
    };
    const { root, view } = await mount("c1", source);
    hold = true;
    memory.emit({ all: true });
    await settle();
    expect(reads).toBe(2);
    expect(view.dataset.state).toBe("ready");
    expect(trail(root).map(([name]) => name)).toEqual([labels.repository, "Family", "Photos"]);
    release();
    await settle();
    expect(view.dataset.state).toBe("ready");
  });

  it("drops the trail when configured with another source, showing loading until it answers", async () => {
    const { root, view } = await mount("c1");
    const pending: NavigationSource = { list: () => new Promise(() => undefined) };
    view.configure({ labels, source: pending, online: () => true });
    await settle();
    expect(view.dataset.state).toBe("loading");
    expect(trail(root)).toEqual([]);
  });

  it("does not read again for a change below the destination", async () => {
    const source = new MemoryNavigationSource(tree);
    await mount("c1", source);
    const spy = vi.spyOn(source, "list");
    source.emit({ parents: ["c1"] });
    await settle();
    expect(spy).not.toHaveBeenCalled();
  });

  it("shows the error state for a trail that does not start at a repository", async () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const photos = { node: "c1", kind: "collection", name: "Photos", has_children: true } as const;
    const source: NavigationSource = {
      list: async () => ({ nodes: [], ancestors: [photos], next_cursor: null }),
    };
    const { root, view } = await mount("c1", source);
    expect(view.dataset.state).toBe("error");
    expect(trail(root)).toEqual([]);
  });
});

describe("bindNavigationRoute", () => {
  /** A shell stand-in with a tree whose current node the route sets. */
  function host() {
    const shell = document.createElement("div") as HTMLDivElement & { focusMain: () => void };
    shell.focusMain = vi.fn();
    const navTree = document.createElement("fs-nav-tree") as HTMLElement & { current?: string };
    document.body.replaceChildren(shell);
    return { shell, navTree };
  }

  it("mounts the view for #/navigation/<handle>, decoding the handle and marking the tree", async () => {
    const { shell, navTree } = host();
    window.location.hash = `#/navigation/${encodeURIComponent("f/1 é")}`;
    const source = new MemoryNavigationSource(tree);
    const stop = bindNavigationRoute(shell, { labels, source, tree: navTree });
    await settle();
    const view = shell.querySelector<FsNavigationView>("fs-navigation-view");
    expect(view?.node).toBe("f/1 é");
    expect(navTree.current).toBe("f/1 é");
    stop();
  });

  it("follows hash changes, moves focus to main, and removes the view on another route", async () => {
    const { shell, navTree } = host();
    const source = new MemoryNavigationSource(tree);
    const stop = bindNavigationRoute(shell, { labels, source, tree: navTree });
    expect(shell.querySelector("fs-navigation-view")).toBeNull();
    window.location.hash = "#/navigation/c1";
    await settle();
    expect(shell.querySelector<FsNavigationView>("fs-navigation-view")?.node).toBe("c1");
    expect(navTree.current).toBe("c1");
    expect(shell.focusMain).toHaveBeenCalled();
    window.location.hash = "#/monitor";
    await settle();
    expect(shell.querySelector("fs-navigation-view")).toBeNull();
    expect(navTree.current).toBeUndefined();
    stop();
  });

  it("ignores a malformed escape in the hash rather than throwing", () => {
    const { shell, navTree } = host();
    window.location.hash = "#/navigation/%E0%A4%A";
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const stop = bindNavigationRoute(shell, {
      labels,
      source: new MemoryNavigationSource(tree),
      tree: navTree,
    });
    expect(shell.querySelector("fs-navigation-view")).toBeNull();
    stop();
  });
});
