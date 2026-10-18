/**
 * UI-F2.1a: the in-memory navigation source answers as the API's navigation listing does
 * (API-S-021: root repository, one level per call, ancestors, cursor paging, `not_found`,
 * `invalid_request`), and the loader reads whole levels, the start of the tree and the pins
 * through any source, within its bounds.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { MemoryNavigationSource, type FixtureNode } from "../../src/navigation/source";
import { readLevel, readNavigation, readPins } from "../../src/navigation/loader";
import { MAX_PAGES, type NavigationSource } from "../../src/navigation/model";

afterEach(() => vi.restoreAllMocks());

const fixture: FixtureNode = {
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
        { node: "v1", kind: "virtual_collection", name: "Best of 2025" },
      ],
    },
  ],
};

describe("MemoryNavigationSource", () => {
  it("answers the repository at the root level, without ancestors", async () => {
    const page = await new MemoryNavigationSource(fixture).list(null);
    expect(page).toEqual({
      nodes: [{ node: "r", kind: "repository", name: null, has_children: true }],
      ancestors: [],
      next_cursor: null,
    });
  });

  it("answers one level of children with the ancestors down to the listed node", async () => {
    const page = await new MemoryNavigationSource(fixture).list("c1");
    expect(page.nodes).toEqual([{ node: "f1", kind: "folder", name: "2025", has_children: false }]);
    expect(page.ancestors.map((n) => n.node)).toEqual(["r", "l1", "c1"]);
  });

  it("pages with opaque cursors", async () => {
    const source = new MemoryNavigationSource(fixture);
    const first = await source.list("l1", null, 1);
    expect(first.nodes.map((n) => n.node)).toEqual(["c1"]);
    const second = await source.list("l1", first.next_cursor, 1);
    expect(second.nodes.map((n) => n.node)).toEqual(["v1"]);
    expect(second.next_cursor).toBeNull();
  });

  it("answers not_found for an unknown node and invalid_request for a bad cursor", async () => {
    const source = new MemoryNavigationSource(fixture);
    await expect(source.list("nope")).rejects.toMatchObject({ code: "not_found", status: 404 });
    await expect(source.list("l1", "x!")).rejects.toMatchObject({ code: "invalid_request" });
  });

  it("rejects with the configured failure, and announces replaced data", async () => {
    const source = new MemoryNavigationSource(fixture);
    const listener = vi.fn();
    const stop = source.subscribe(listener);
    source.failure = new TypeError("offline");
    await expect(source.list(null)).rejects.toBeInstanceOf(TypeError);
    source.failure = undefined;
    source.replace({ ...fixture, name: "Home" });
    expect(listener).toHaveBeenCalledWith({ all: true });
    expect((await source.list(null)).nodes[0]?.name).toBe("Home");
    stop();
    source.emit({ parents: ["l1"] });
    expect(listener).toHaveBeenCalledOnce();
  });
});

describe("loader", () => {
  it("reads every page of a level", async () => {
    const source = new MemoryNavigationSource(fixture);
    const original = source.list.bind(source);
    const spy = vi
      .spyOn(source, "list")
      .mockImplementation((parent, cursor) => original(parent, cursor, 1));
    const level = await readLevel(source, "l1");
    expect(level.nodes.map((n) => n.node)).toEqual(["c1", "v1"]);
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it("stops after MAX_PAGES pages and says so", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const endless: NavigationSource = {
      list: async () => ({ nodes: [], ancestors: [], next_cursor: "more" }),
    };
    const spy = vi.spyOn(endless, "list");
    await readLevel(endless, null);
    expect(spy).toHaveBeenCalledTimes(MAX_PAGES);
    expect(warn).toHaveBeenCalledOnce();
  });

  it("reads the repository, its libraries and each library's children", async () => {
    const data = await readNavigation(new MemoryNavigationSource(fixture));
    expect(data.repository?.node).toBe("r");
    expect(data.libraries.map((n) => n.name)).toEqual(["Family"]);
    expect(data.children.get("l1")?.map((n) => n.kind)).toEqual([
      "collection",
      "virtual_collection",
    ]);
  });

  it("answers no libraries for an empty root level", async () => {
    const empty: NavigationSource = {
      list: async () => ({ nodes: [], ancestors: [], next_cursor: null }),
    };
    expect(await readNavigation(empty)).toEqual({ libraries: [], children: new Map() });
  });

  it("reads pins by their ancestors, marks not_found ones and logs other failures", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new MemoryNavigationSource(fixture);
    const original = source.list.bind(source);
    vi.spyOn(source, "list").mockImplementation((parent, cursor, limit) =>
      parent === "boom"
        ? Promise.reject(new ApiError(500, "/api/v1/navigation"))
        : original(parent, cursor, limit),
    );
    const pins = await readPins(source, [
      { node: "f1", kind: "folder" },
      { node: "gone", kind: "collection" },
      { node: "boom", kind: "folder" },
    ]);
    expect(pins).toEqual([
      {
        pin: { node: "f1", kind: "folder" },
        node: { node: "f1", kind: "folder", name: "2025", has_children: false },
      },
      { pin: { node: "gone", kind: "collection" }, node: null },
    ]);
    expect(warn).toHaveBeenCalledOnce();
  });
});
