/**
 * Unit tests for the library scope lookups (UI-F2.8, UI-F2.9): the first library the host lists,
 * as node and name, and the per-route cache the search bar reads it through.
 */
import { describe, expect, it, vi } from "vitest";
import { cachedLibraryScope, firstLibraryScope } from "../../src/navigation/library-scope";
import { MemoryNavigationSource } from "../../src/navigation/source";
import { TREE } from "../support/search-fakes";

describe("firstLibraryScope", () => {
  it("answers the first library's node and name", async () => {
    expect(await firstLibraryScope(new MemoryNavigationSource(TREE))).toEqual({
      node: "node-library",
      name: "Family archive",
    });
  });

  it("answers undefined when the host lists no library or no repository", async () => {
    const empty = new MemoryNavigationSource({
      node: "r",
      kind: "repository",
      name: null,
      children: [],
    });
    expect(await firstLibraryScope(empty)).toBeUndefined();
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

  it("does not keep a failed read", async () => {
    const navigation = new MemoryNavigationSource(TREE);
    navigation.failure = new Error("offline");
    const scope = cachedLibraryScope(navigation);
    await expect(scope()).rejects.toThrow("offline");
    navigation.failure = undefined;
    expect((await scope())?.name).toBe("Family archive");
  });
});
