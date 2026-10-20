/**
 * UI tests for `<fs-nav-tree>` over live data (UI-F2.1b part 2, API-S-021): a
 * `navigation.change` that names parents reads only the levels it touches (the named parents
 * that are loaded, and the shown level holding each named node), the repository, `all` or a
 * change before the first answer reads the whole tree, a re-read pin is renamed, a level that
 * is gone closes, and a 503 shows the unavailable state with Retry.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  MemoryNavigationSource,
  NavigationStore,
  defineElements,
  type FixtureNode,
  type FsNavTree,
  type FsStates,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { fakeStorage } from "../support/theme-fakes";

/** One library with two collections (one with folders), a virtual collection and a folder. */
function library(over: { c2?: FixtureNode[]; f9?: string; c1?: FixtureNode[] } = {}): FixtureNode {
  return {
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
            children: over.c1 ?? [
              { node: "f1", kind: "folder", name: "2025" },
              { node: "f2", kind: "folder", name: "Archive" },
            ],
          },
          { node: "c2", kind: "collection", name: "Hobbies", children: over.c2 },
          { node: "v1", kind: "virtual_collection", name: "Best of 2025" },
          { node: "f9", kind: "folder", name: over.f9 ?? "inbox" },
        ],
      },
    ],
  };
}

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

/** Let pending answers and renders finish. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Mount a tree over a memory source and record every level it lists from then on. */
async function mount(saved?: object, source = new MemoryNavigationSource(library())) {
  const initial: Record<string, string> = saved
    ? { "filesquid.navigation": JSON.stringify({ version: 1, ...saved }) }
    : {};
  const store = new NavigationStore({ storage: fakeStorage(initial).storage });
  store.start();
  const tree = document.createElement("fs-nav-tree") as FsNavTree;
  tree.configure({ labels, source, store, online: () => true });
  document.body.replaceChildren(tree);
  await settle();
  const spy = vi.spyOn(source, "list");
  const listed = () => spy.mock.calls.map((call) => call[0]);
  return { tree, source, root: tree.shadowRoot as ShadowRoot, listed };
}

/** Names of the treeitems in a section. */
function names(root: ShadowRoot, section: string): string[] {
  return [...root.querySelectorAll(`[data-section="${section}"] [role="treeitem"] .name`)].map(
    (el) => el.textContent ?? "",
  );
}

/** The treeitem named `name` in `section`. */
function item(root: ShadowRoot, section: string, name: string): HTMLElement {
  const found = [
    ...root.querySelectorAll<HTMLElement>(`[data-section="${section}"] [role="treeitem"]`),
  ].find((el) => el.querySelector(".name")?.textContent === name);
  if (!found) throw new Error(`no item ${name} in ${section}`);
  return found;
}

describe("<fs-nav-tree> navigation.change with parents", () => {
  it("reads only the named loaded level and shows its new children", async () => {
    const { root, source, listed } = await mount();
    source.replace(library({ f9: "Inbox" }), { parents: ["l1"] });
    await settle();
    expect(listed()).toEqual(["l1"]);
    expect(names(root, "folder")).toEqual(["Inbox"]);
  });

  it("reads the shown level holding a named node that is not loaded, so has_children follows", async () => {
    const { root, source, listed } = await mount();
    expect(item(root, "collection", "Hobbies").hasAttribute("aria-expanded")).toBe(false);
    const added = [{ node: "f5", kind: "folder", name: "Kites" } as const];
    source.replace(library({ c2: [...added] }), { parents: ["c2"] });
    await settle();
    expect(listed()).toEqual(["l1"]);
    expect(item(root, "collection", "Hobbies").getAttribute("aria-expanded")).toBe("false");
  });

  it("reads an opened node's level again and keeps it open", async () => {
    const { root, source, listed } = await mount();
    item(root, "collection", "Photos").querySelector<HTMLElement>(".twisty")?.click();
    await settle();
    expect(names(root, "collection")).toEqual(["Photos", "2025", "Archive", "Hobbies"]);
    const before = listed().length;
    const renamed = [{ node: "f1", kind: "folder", name: "2026" } as const];
    source.replace(library({ c1: [...renamed] }), { parents: ["c1"] });
    await settle();
    expect(listed().slice(before).sort()).toEqual(["c1", "l1"]);
    expect(names(root, "collection")).toEqual(["Photos", "2026", "Hobbies"]);
  });

  it("reads nothing for a parent it does not show", async () => {
    const { source, listed } = await mount();
    source.emit({ parents: ["elsewhere"] });
    await settle();
    expect(listed()).toEqual([]);
  });

  it("reads the whole tree when the repository is named (the libraries changed)", async () => {
    const { source, listed } = await mount();
    source.emit({ parents: ["r"] });
    await settle();
    expect(listed().slice(0, 2)).toEqual([null, "r"]);
  });

  it("reads the whole tree on all", async () => {
    const { source, listed } = await mount();
    source.emit({ all: true });
    await settle();
    expect(listed().slice(0, 2)).toEqual([null, "r"]);
  });

  it("renames a pinned destination whose level was read again", async () => {
    const { root, source } = await mount({ pinned: [{ node: "f9", kind: "folder" }] });
    expect(names(root, "pinned")).toEqual(["inbox"]);
    source.replace(library({ f9: "Inbox" }), { parents: ["l1"] });
    await settle();
    expect(names(root, "pinned")).toEqual(["Inbox"]);
  });

  it("closes an opened node whose level is gone, and shows the parent without it", async () => {
    const { root, source } = await mount();
    item(root, "collection", "Photos").querySelector<HTMLElement>(".twisty")?.click();
    await settle();
    const tree = library();
    const lib = tree.children?.[0] as FixtureNode;
    const pruned = { ...lib, children: lib.children?.filter((child) => child.node !== "c1") };
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    source.replace({ ...tree, children: [pruned] }, { parents: ["c1", "l1"] });
    await settle();
    expect(names(root, "collection")).toEqual(["Hobbies"]);
  });
});

describe("<fs-nav-tree> an unavailable navigation service", () => {
  it("shows the unavailable state with Retry for a 503, then recovers on Retry", async () => {
    const source = new MemoryNavigationSource(library());
    source.failure = new ApiError(503, "/api/v1/navigation", { code: "unavailable" });
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { root } = await mount(undefined, source);
    const states = root.querySelector<FsStates>("fs-states");
    expect(states?.hidden).toBe(false);
    expect(states?.state).toEqual({ kind: "unavailable" });
    expect(states?.shadowRoot?.textContent).toContain(labels.stateUnavailable);
    source.failure = undefined;
    states?.shadowRoot?.querySelector("button")?.click();
    await settle();
    expect(states?.hidden).toBe(true);
    expect(names(root, "folder")).toEqual(["inbox"]);
  });
});
