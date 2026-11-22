/**
 * The rows of the virtualised navigation tree (Task UI-F3.4, UI-S-035, UI-S-016): a section's
 * open levels as one flat list in document order. Each row carries what its tree item states
 * (its level, its place in its level and the size of that level, whether it is open), counted
 * over the whole level and never over what is drawn, and a capped level ends in a limit row
 * (UI-S-017).
 */
import { describe, expect, it } from "vitest";
import {
  keyItems,
  sectionRows,
  type LimitRow,
  type NodeRow,
  type RowContext,
  type TreeRow,
} from "../../src/components/nav-tree-rows";
import {
  sectionViews,
  type SectionItem,
  type SectionView,
} from "../../src/components/nav-tree-view";
import type { NavigationKind, NavigationNode } from "../../src/navigation/model";
import labels from "../../src/theme/labels.en.json";

/** A node named by its handle. */
function node(
  id: string,
  has_children = false,
  kind: NavigationKind = "collection",
): NavigationNode {
  return { node: id, kind, name: id, has_children };
}

/** A section of the given top-level items. */
function view(items: SectionItem[], limit?: number): SectionView {
  return { id: "collection", heading: "Collections", items, unavailable: [], limit };
}

/** A context over open handles, loaded children, busy handles and capped levels. */
function context(
  over: {
    open?: string[];
    children?: Record<string, NavigationNode[]>;
    busy?: string[];
    limits?: Record<string, number>;
  } = {},
): RowContext {
  return {
    isOpen: (n) => (over.open ?? []).includes(n.node),
    childrenOf: (n) => over.children?.[n.node],
    isBusy: (n) => (over.busy ?? []).includes(n.node),
    limitOf: (n) => over.limits?.[n.node],
  };
}

/** The node rows as short facts: handle, level, place of size, parent row. */
function facts(rows: readonly TreeRow[]): string[] {
  return rows.map((row) => {
    if (row.type === "limit") return `limit(${row.level}):${row.count}`;
    if (row.type === "pad") return "pad";
    return `${row.node.node} L${row.level} ${row.posinset}/${row.setsize} ^${row.parent}`;
  });
}

describe("sectionRows", () => {
  it("counts a level of 10,000 nodes whole: every row knows its place and the level's size", () => {
    const items = Array.from({ length: 10_000 }, (_, i) => ({ node: node(`c-${i + 1}`) }));
    const rows = sectionRows(view(items), context());
    expect(rows).toHaveLength(10_000);
    const middle = rows[4999] as NodeRow;
    expect(middle.node.node).toBe("c-5000");
    expect([middle.level, middle.posinset, middle.setsize, middle.parent]).toEqual([
      1, 5000, 10_000, -1,
    ]);
    const last = rows[9999] as NodeRow;
    expect([last.posinset, last.setsize]).toEqual([10_000, 10_000]);
  });

  it("puts the children of an open parent under it, each counted in its own level", () => {
    const rows = sectionRows(
      view([{ node: node("a", true) }, { node: node("b") }]),
      context({
        open: ["a", "a1"],
        children: { a: [node("a1", true, "folder"), node("a2", false, "folder")], a1: [node("x")] },
      }),
    );
    expect(facts(rows)).toEqual([
      "a L1 1/2 ^-1",
      "a1 L2 1/2 ^0",
      "x L3 1/1 ^1",
      "a2 L2 2/2 ^0",
      "b L1 2/2 ^-1",
    ]);
  });

  it("says whether a parent is open, and nothing for a node without children", () => {
    const rows = sectionRows(
      view([{ node: node("a", true) }, { node: node("b", true) }, { node: node("c") }]),
      context({ open: ["a"], children: { a: [node("a1")] }, busy: ["b"] }),
    ) as NodeRow[];
    expect(rows.map((row) => row.expanded)).toEqual([true, undefined, false, undefined]);
    expect(rows.map((row) => row.busy)).toEqual([false, false, true, false]);
  });

  it("shows an open parent whose children are not loaded yet as open with no rows under it", () => {
    const rows = sectionRows(view([{ node: node("a", true) }]), context({ open: ["a"] }));
    expect(facts(rows)).toEqual(["a L1 1/1 ^-1"]);
    expect((rows[0] as NodeRow).expanded).toBe(true);
  });

  it("takes the children a section fixes (a library's nodes of the section's kind)", () => {
    const library = node("l1", true, "library");
    const rows = sectionRows(
      view([
        { node: library, fixed: [node("c1"), node("c2")] },
        { node: node("l2", true, "library"), fixed: [] },
      ]),
      context({ open: ["l1", "l2"], children: { l1: [node("ignored")] } }),
    );
    expect(facts(rows)).toEqual(["l1 L1 1/2 ^-1", "c1 L2 1/2 ^0", "c2 L2 2/2 ^0", "l2 L1 2/2 ^-1"]);
    expect((rows[3] as NodeRow).expanded).toBeUndefined();
  });

  it("ends a capped top level in a limit row, two rows tall, described on its last node", () => {
    const rows = sectionRows(view([{ node: node("a") }, { node: node("b") }], 10_000), context());
    expect(facts(rows)).toEqual(["a L1 1/2 ^-1", "b L1 2/2 ^-1", "limit(1):10000", "pad"]);
    expect((rows[1] as NodeRow).limit).toBe(2);
    expect((rows[0] as NodeRow).limit).toBeUndefined();
  });

  it("puts the limit row of an open node's capped level after all that is under it", () => {
    const rows = sectionRows(
      view([{ node: node("a", true) }, { node: node("b") }]),
      context({
        open: ["a", "a2"],
        children: { a: [node("a1"), node("a2", true)], a2: [node("deep")] },
        limits: { a: 10_000 },
      }),
    );
    expect(facts(rows)).toEqual([
      "a L1 1/2 ^-1",
      "a1 L2 1/2 ^0",
      "a2 L2 2/2 ^0",
      "deep L3 1/1 ^2",
      "limit(2):10000",
      "pad",
      "b L1 2/2 ^-1",
    ]);
    // The level's last node (a2) is the one the line describes, not the row just above the line.
    expect((rows[2] as NodeRow).limit).toBe(4);
    expect((rows[3] as NodeRow).limit).toBeUndefined();
    expect((rows[4] as LimitRow).count).toBe(10_000);
  });

  it("ends a library's fixed children in a limit row when the section says so", () => {
    const rows = sectionRows(
      view([{ node: node("l1", true, "library"), fixed: [node("c1")], limit: 10_000 }]),
      context({ open: ["l1"] }),
    );
    expect(facts(rows)).toEqual(["l1 L1 1/1 ^-1", "c1 L2 1/1 ^0", "limit(2):10000", "pad"]);
  });

  it("shows no limit row for a closed node", () => {
    const rows = sectionRows(
      view([{ node: node("a", true) }]),
      context({ children: { a: [node("a1")] }, limits: { a: 10_000 } }),
    );
    expect(facts(rows)).toEqual(["a L1 1/1 ^-1"]);
  });
});

describe("sectionViews with a capped level (UI-S-017)", () => {
  const library = node("l1", true, "library");
  const other = node("l2", true, "library");
  const prefs = { pinned: [], order: [], collapsed: [] };

  /** The limit of each kind section, by section id. */
  function limits(views: readonly SectionView[]): Record<string, number | undefined> {
    return Object.fromEntries(views.map((section) => [section.id, section.limit]));
  }

  it("puts the limit in the section that holds the level's last node", () => {
    const children = new Map([
      ["l1", [node("c1"), node("c2"), node("f1", false, "folder"), node("f2", false, "folder")]],
    ]);
    const data = { repository: node("r"), libraries: [library], children, capped: new Set(["l1"]) };
    expect(limits(sectionViews(data, [], prefs, labels))).toEqual({
      collection: undefined,
      virtual_collection: undefined,
      folder: 4,
    });
  });

  it("shows no limit for a level that was read whole", () => {
    const children = new Map([["l1", [node("c1")]]]);
    const data = {
      repository: node("r"),
      libraries: [library],
      children,
      capped: new Set<string>(),
    };
    expect(limits(sectionViews(data, [], prefs, labels))).toEqual({
      collection: undefined,
      virtual_collection: undefined,
      folder: undefined,
    });
  });

  it("with several libraries, puts the limit under the capped library's children", () => {
    const children = new Map([
      ["l1", [node("c1"), node("c2")]],
      ["l2", [node("c3")]],
    ]);
    const data = {
      repository: node("r"),
      libraries: [library, other],
      children,
      capped: new Set(["l1"]),
    };
    const [collections] = sectionViews(data, [], prefs, labels);
    expect(collections?.limit).toBeUndefined();
    expect(collections?.items.map((item) => [item.node.node, item.limit])).toEqual([
      ["l1", 2],
      ["l2", undefined],
    ]);
    const rows = sectionRows(collections as SectionView, context({ open: ["l1", "l2"] }));
    expect(facts(rows)).toEqual([
      "l1 L1 1/2 ^-1",
      "c1 L2 1/2 ^0",
      "c2 L2 2/2 ^0",
      "limit(2):2",
      "pad",
      "l2 L1 2/2 ^-1",
      "c3 L2 1/1 ^5",
    ]);
  });
});

describe("keyItems", () => {
  it("lists the nodes only, in order, each with its parent among them and its open state", () => {
    const rows = sectionRows(
      view([{ node: node("a", true) }, { node: node("b", true) }], 10_000),
      context({ open: ["a"], children: { a: [node("a1")] }, limits: { a: 10_000 } }),
    );
    // Rows: a, a1, limit, pad, b, limit, pad.
    const keys = keyItems(rows, (n) => n.node.toUpperCase());
    expect(keys.items).toEqual([
      { name: "A", parent: -1, expanded: true },
      { name: "A1", parent: 0, expanded: undefined },
      { name: "B", parent: -1, expanded: false },
    ]);
    expect(keys.rows).toEqual([0, 1, 4]);
    expect(keys.at(4)).toBe(2);
    expect(keys.at(2)).toBe(-1);
  });
});
