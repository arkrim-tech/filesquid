/**
 * Unit tests for the library scope lookups (UI-F2.8, UI-F2.9): the first library the host lists,
 * as node and name, and the per-route cache the search bar reads it through.
 */
import { describe, expect, it, vi } from "vitest";
import {
  MAX_PAGES,
  type NavigationNode,
  type NavigationPage,
  type NavigationSource,
} from "../../src/navigation/model";
import { cachedLibraryScope, firstLibraryScope } from "../../src/navigation/library-scope";
import { MemoryNavigationSource } from "../../src/navigation/source";
import { TREE } from "../support/search-fakes";

/** A navigation source that answers each level from a table, to shape any tree. */
function stub(levels: Record<string, readonly NavigationNode[]>): NavigationSource {
  return {
    async list(parent): Promise<NavigationPage> {
      return { nodes: levels[parent ?? "root"] ?? [], ancestors: [], next_cursor: null };
    },
  };
}

const node = (id: string, kind: NavigationNode["kind"], name: string | null): NavigationNode => ({
  node: id,
  kind,
  name,
  has_children: true,
});

describe("firstLibraryScope", () => {
  it("answers the first library's node and name", async () => {
    expect(await firstLibraryScope(new MemoryNavigationSource(TREE))).toEqual({
      node: "node-library",
      name: "Family archive",
    });
  });

  it("answers the first of several libraries, in the host's order", async () => {
    const navigation = stub({
      root: [node("r", "repository", null)],
      r: [node("l2", "library", "Second"), node("l1", "library", "First")],
    });
    expect(await firstLibraryScope(navigation)).toEqual({ node: "l2", name: "Second" });
  });

  it("answers a library with no name as null, for the caller to word", async () => {
    const navigation = stub({
      root: [node("r", "repository", null)],
      r: [node("l1", "library", null)],
    });
    expect(await firstLibraryScope(navigation)).toEqual({ node: "l1", name: null });
  });

  it("finds a repository and a library that a later page of the listing holds", async () => {
    const pages: Record<string, readonly [NavigationNode[], string | null]> = {
      "root:null": [[node("x", "library", "Stray")], "p2"],
      "root:p2": [[node("r", "repository", null)], null],
      "r:null": [[node("c", "collection", "Inbox")], "q2"],
      "r:q2": [[node("l9", "library", "Late")], null],
    };
    const list = vi.fn(async (parent: string | null, cursor?: string | null) => {
      const [nodes, next] = pages[`${parent ?? "root"}:${cursor ?? "null"}`] ?? [[], null];
      return { nodes, ancestors: [], next_cursor: next };
    });
    expect(await firstLibraryScope({ list })).toEqual({ node: "l9", name: "Late" });
    expect(list).toHaveBeenCalledTimes(4);
  });

  it("stops following a listing that never ends", async () => {
    const list = vi.fn(async () => ({
      nodes: [node("c", "collection", "Inbox")],
      ancestors: [],
      next_cursor: "more",
    }));
    expect(await firstLibraryScope({ list })).toBeUndefined();
    expect(list.mock.calls.length).toBeLessThanOrEqual(MAX_PAGES);
  });

  it("answers undefined when the host lists no library", async () => {
    const empty = new MemoryNavigationSource({
      node: "r",
      kind: "repository",
      name: null,
      children: [],
    });
    expect(await firstLibraryScope(empty)).toBeUndefined();
  });

  it("answers undefined, and reads no further, when the root lists no repository", async () => {
    const list = vi.fn(async () => ({ nodes: [], ancestors: [], next_cursor: null }));
    expect(await firstLibraryScope({ list })).toBeUndefined();
    expect(list).toHaveBeenCalledTimes(1);
  });
});

describe("cachedLibraryScope", () => {
  it("reads once, then again after the address changes", async () => {
    const navigation = new MemoryNavigationSource(TREE);
    const list = vi.spyOn(navigation, "list");
    const events = new EventTarget();
    const scope = cachedLibraryScope(navigation, events);
    await scope();
    await scope();
    expect(list).toHaveBeenCalledTimes(2);
    events.dispatchEvent(new Event("hashchange"));
    await scope();
    expect(list).toHaveBeenCalledTimes(4);
  });

  it("reads again after the navigation source reports a change to the tree", async () => {
    const navigation = new MemoryNavigationSource(TREE);
    const list = vi.spyOn(navigation, "list");
    const scope = cachedLibraryScope(navigation);
    await scope();
    expect(list).toHaveBeenCalledTimes(2);
    navigation.replace(TREE);
    await scope();
    expect(list).toHaveBeenCalledTimes(4);
  });

  it("does not keep a failed read", async () => {
    const navigation = new MemoryNavigationSource(TREE);
    navigation.failure = new Error("offline");
    const scope = cachedLibraryScope(navigation);
    await expect(scope()).rejects.toThrow("offline");
    navigation.failure = undefined;
    expect((await scope())?.name).toBe("Family archive");
  });
});
