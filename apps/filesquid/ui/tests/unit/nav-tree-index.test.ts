/**
 * The index of the drawn navigation tree (UI-F2.1a, Task UI-F3.1): built in one pass over the
 * sections, it moves `aria-current` and the Tab stops by touching only the items involved, so a
 * destination change costs the same on a level of 10,000 nodes as on a level of ten.
 */
import { beforeEach, describe, expect, it } from "vitest";
import { TreeIndex } from "../../src/components/nav-tree-index";

/** A section with treeitems named by handle; `stop` carries tabindex 0, `current` aria-current. */
function section(id: string, nodes: string[], stop?: string, current?: string): HTMLElement {
  const out = document.createElement("section");
  out.dataset.section = id;
  for (const node of nodes) {
    const a = document.createElement("a");
    a.setAttribute("role", "treeitem");
    a.dataset.node = node;
    a.tabIndex = node === stop ? 0 : -1;
    if (node === current) a.setAttribute("aria-current", "page");
    out.append(a);
  }
  return out;
}

let box: HTMLElement;
let index: TreeIndex;

beforeEach(() => {
  box = document.createElement("div");
  box.append(
    section("pinned", ["c2"], "c2", "c2"),
    section("collection", ["c1", "c2", "c3"], "c1"),
    section("folder", []),
  );
  index = new TreeIndex();
  index.build(box);
});

const item = (id: string, node: string) => index.itemIn(id, node) as HTMLElement;

describe("TreeIndex", () => {
  it("is empty until a tree is built, and after it is cleared", () => {
    expect(new TreeIndex().empty).toBe(true);
    expect(index.empty).toBe(false);
    index.clear();
    expect(index.empty).toBe(true);
    expect(index.sectionIds()).toEqual([]);
  });

  it("knows its sections in order, each item by handle and each section's first item", () => {
    expect(index.sectionIds()).toEqual(["pinned", "collection", "folder"]);
    expect(index.section("collection")).toBe(box.children[1]);
    expect(item("collection", "c3").dataset.node).toBe("c3");
    expect(index.first("collection")).toBe(item("collection", "c1"));
    expect(index.first("folder")).toBeUndefined();
    expect(index.itemIn("collection", "zz")).toBeUndefined();
    expect(index.itemIn("collection", undefined)).toBeUndefined();
    expect(index.itemIn("nope", "c1")).toBeUndefined();
  });

  it("moves aria-current to the node's item in every section that draws it", () => {
    index.mark("c2");
    expect(item("pinned", "c2").getAttribute("aria-current")).toBe("page");
    expect(item("collection", "c2").getAttribute("aria-current")).toBe("page");
    index.mark("c1");
    expect(item("pinned", "c2").hasAttribute("aria-current")).toBe(false);
    expect(item("collection", "c2").hasAttribute("aria-current")).toBe(false);
    expect(item("collection", "c1").getAttribute("aria-current")).toBe("page");
    expect(box.querySelectorAll('[aria-current="page"]')).toHaveLength(1);
  });

  it("takes aria-current from the items that had it when built, and clears it for no node", () => {
    index.mark(undefined);
    expect(box.querySelectorAll('[aria-current="page"]')).toHaveLength(0);
    index.mark("zz");
    expect(box.querySelectorAll('[aria-current="page"]')).toHaveLength(0);
  });

  it("moves a section's Tab stop and leaves exactly one", () => {
    index.setStop("collection", item("collection", "c3"));
    expect(item("collection", "c1").tabIndex).toBe(-1);
    expect(item("collection", "c3").tabIndex).toBe(0);
    index.setStop("collection", item("collection", "c3"));
    const stops = [...(index.section("collection")?.querySelectorAll("a") ?? [])].filter(
      (a) => a.tabIndex === 0,
    );
    expect(stops).toHaveLength(1);
    expect(item("pinned", "c2").tabIndex).toBe(0);
  });

  it("indexes again when built again, forgetting the earlier tree", () => {
    const next = document.createElement("div");
    next.append(section("collection", ["x"], "x"));
    index.build(next);
    expect(index.sectionIds()).toEqual(["collection"]);
    expect(index.itemIn("collection", "c1")).toBeUndefined();
    expect(item("collection", "x").tabIndex).toBe(0);
  });
});
