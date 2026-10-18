/**
 * UI-S-017 (UI-F2.1a): the navigation choices (pins, section order, collapsed nodes) are kept
 * on the device under `filesquid.navigation` and survive a restart; refused device storage
 * keeps them for this page and says why; a stored value that does not parse, has another
 * version or breaks a bound is ignored with a console message (as the theme store, UI-S-015).
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  NAVIGATION_STORAGE_KEY,
  NavigationStore,
  createNavigationStore,
  sharedNavigationStore,
} from "../../src/navigation/store";
import { MAX_PINNED } from "../../src/navigation/model";
import type { DeviceStorage } from "../../src/app/device-storage";
import { fakeStorage } from "../support/theme-fakes";

afterEach(() => vi.restoreAllMocks());

/** A started store over an in-memory storage. */
function started(initial: Record<string, string> = {}) {
  const { storage, data } = fakeStorage(initial);
  const store = new NavigationStore({ storage });
  store.start();
  return { store, data };
}

/** The saved JSON value, parsed. */
function saved(data: Map<string, string>): unknown {
  return JSON.parse(data.get(NAVIGATION_STORAGE_KEY) ?? "null");
}

describe("NavigationStore", () => {
  it("uses the namespaced key filesquid.navigation", () => {
    expect(NAVIGATION_STORAGE_KEY).toBe("filesquid.navigation");
  });

  it("starts with no saved choices", () => {
    expect(started().store.preferences).toEqual({ pinned: [], order: [], collapsed: [] });
  });

  it("restores the choices saved at an earlier launch", () => {
    const value = JSON.stringify({
      version: 1,
      pinned: [{ node: "c1", kind: "collection" }],
      order: ["folder"],
      collapsed: ["l1"],
    });
    const { store } = started({ [NAVIGATION_STORAGE_KEY]: value });
    expect(store.preferences.pinned).toEqual([{ node: "c1", kind: "collection" }]);
    expect(store.preferences.collapsed).toEqual(["l1"]);
  });

  it("saves every change as handles, kinds and section ids, and dispatches change", () => {
    const { store, data } = started();
    const changes: string[] = [];
    store.addEventListener("change", () => changes.push("change"));
    store.moveSection("folder", -1, ["collection", "virtual_collection", "folder"]);
    store.setCollapsed("l1", true);
    store.setPinned({ node: "c1", kind: "collection" }, true);
    expect(saved(data)).toEqual({
      version: 1,
      pinned: [{ node: "c1", kind: "collection" }],
      order: ["collection", "folder", "virtual_collection"],
      collapsed: ["l1"],
    });
    expect(changes).toHaveLength(3);
    store.setPinned({ node: "c1", kind: "collection" }, false);
    store.setCollapsed("l1", false);
    expect(saved(data)).toMatchObject({ pinned: [], collapsed: [] });
  });

  it("dispatches nothing and writes nothing when a change changes nothing", () => {
    const { store, data } = started();
    const listener = vi.fn();
    store.addEventListener("change", listener);
    store.moveSection("collection", -1, ["collection", "folder"]);
    store.setCollapsed("l1", false);
    store.setPinned({ node: "x", kind: "folder" }, false);
    expect(listener).not.toHaveBeenCalled();
    expect(data.has(NAVIGATION_STORAGE_KEY)).toBe(false);
  });

  it("refuses a pin beyond the bound, with a console message", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { store } = started();
    for (let i = 0; i < MAX_PINNED; i += 1)
      store.setPinned({ node: `n${i}`, kind: "folder" }, true);
    expect(store.setPinned({ node: "one-more", kind: "folder" }, true)).toBe(false);
    expect(store.preferences.pinned).toHaveLength(MAX_PINNED);
    expect(warn).toHaveBeenCalledOnce();
    expect(store.setPinned({ node: "n0", kind: "folder" }, true)).toBe(true);
  });

  it.each([
    ["text that is not JSON", "{not json", /could not read/],
    ["an empty string", "", /could not read/],
    ["an explicit null list", JSON.stringify({ version: 1, pinned: null }), /ignored .*pins/],
    ["another version", JSON.stringify({ version: 2 }), /ignored .*version is not 1/],
    [
      "a stored name",
      JSON.stringify({ version: 1, pinned: [{ node: "a", kind: "folder", name: "n" }] }),
      /ignored/,
    ],
  ])("ignores %s and says why", (_case, value, reason) => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { store } = started({ [NAVIGATION_STORAGE_KEY]: value });
    expect(store.preferences).toEqual({ pinned: [], order: [], collapsed: [] });
    expect(warn).toHaveBeenCalledOnce();
    expect(String(warn.mock.calls[0]?.[0])).toMatch(reason);
  });

  it("still applies choices when storage throws, and logs why", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const broken: DeviceStorage = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("quota");
      },
      removeItem: () => {
        throw new Error("blocked");
      },
    };
    const store = new NavigationStore({ storage: broken });
    store.start();
    store.setCollapsed("l1", true);
    expect(store.preferences.collapsed).toEqual(["l1"]);
    expect(warn).toHaveBeenCalledTimes(2);
  });

  it("works without storage (choices last for this page only)", () => {
    const store = new NavigationStore({});
    store.start();
    store.setCollapsed("l1", true);
    expect(store.preferences.collapsed).toEqual(["l1"]);
    store.start();
    expect(store.preferences.collapsed).toEqual(["l1"]);
  });

  it("keeps this page's choices when started again while storage cannot be read", () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    let refuse = false;
    const { storage } = fakeStorage();
    const flaky: DeviceStorage = {
      getItem: (key) => {
        if (refuse) throw new Error("blocked");
        return storage.getItem(key);
      },
      setItem: (key, value) => storage.setItem(key, value),
      removeItem: (key) => storage.removeItem(key),
    };
    const store = new NavigationStore({ storage: flaky });
    store.start();
    store.setPinned({ node: "c1", kind: "collection" }, true);
    refuse = true;
    store.start();
    expect(store.preferences.pinned).toEqual([{ node: "c1", kind: "collection" }]);
  });
});

describe("NavigationStore across tabs", () => {
  /** A store following storage events on `events`, over `storage`. */
  function tab(events: EventTarget, storage = fakeStorage()) {
    const store = new NavigationStore({ storage: storage.storage, events });
    store.start();
    return { store, ...storage };
  }

  it("takes another tab's save, so this tab's next save does not overwrite it", () => {
    const events = new EventTarget();
    const { store, data } = tab(events);
    const listener = vi.fn();
    store.addEventListener("change", listener);
    const other = { version: 1, pinned: [{ node: "c7", kind: "collection" }], collapsed: ["l2"] };
    data.set(NAVIGATION_STORAGE_KEY, JSON.stringify(other));
    events.dispatchEvent(new StorageEvent("storage", { key: NAVIGATION_STORAGE_KEY }));
    expect(listener).toHaveBeenCalledOnce();
    store.setCollapsed("l1", true);
    expect(saved(data)).toMatchObject({
      pinned: [{ node: "c7", kind: "collection" }],
      collapsed: ["l2", "l1"],
    });
  });

  it("ignores other keys, and stops following after dispose", () => {
    const events = new EventTarget();
    const { store, data } = tab(events);
    data.set(NAVIGATION_STORAGE_KEY, JSON.stringify({ version: 1, collapsed: ["l2"] }));
    events.dispatchEvent(new StorageEvent("storage", { key: "filesquid.theme" }));
    expect(store.preferences.collapsed).toEqual([]);
    store.dispose();
    events.dispatchEvent(new StorageEvent("storage", { key: NAVIGATION_STORAGE_KEY }));
    expect(store.preferences.collapsed).toEqual([]);
  });
});

describe("createNavigationStore", () => {
  it("uses the window's localStorage and follows its storage events", () => {
    const { storage, data } = fakeStorage();
    const win = Object.assign(new EventTarget(), { localStorage: storage }) as unknown as Window;
    const store = createNavigationStore(win);
    store.start();
    store.setCollapsed("l1", true);
    expect(data.has(NAVIGATION_STORAGE_KEY)).toBe(true);
    data.set(NAVIGATION_STORAGE_KEY, JSON.stringify({ version: 1, collapsed: ["l9"] }));
    win.dispatchEvent(new StorageEvent("storage", { key: NAVIGATION_STORAGE_KEY }));
    expect(store.preferences.collapsed).toEqual(["l9"]);
    store.dispose();
  });

  it("gives every caller the same store, started again after a dispose", () => {
    const { storage, data } = fakeStorage();
    const win = Object.assign(new EventTarget(), { localStorage: storage }) as unknown as Window;
    const store = sharedNavigationStore(win);
    store.setCollapsed("l1", true);
    store.dispose();
    expect(sharedNavigationStore(win)).toBe(store);
    expect(store.preferences.collapsed).toEqual(["l1"]);
    data.set(NAVIGATION_STORAGE_KEY, JSON.stringify({ version: 1, collapsed: ["l4"] }));
    win.dispatchEvent(new StorageEvent("storage", { key: NAVIGATION_STORAGE_KEY }));
    expect(store.preferences.collapsed).toEqual(["l4"]);
    store.dispose();
  });

  it("falls back to no storage when reading localStorage throws", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const win = new EventTarget() as unknown as Window;
    Object.defineProperty(win, "localStorage", {
      get(): Storage {
        throw new Error("SecurityError");
      },
    });
    const store = createNavigationStore(win);
    store.start();
    store.setCollapsed("l1", true);
    expect(store.preferences.collapsed).toEqual(["l1"]);
    expect(warn).toHaveBeenCalledOnce();
    expect(String(warn.mock.calls[0]?.[0])).toMatch(/navigation choices will not be kept/);
  });
});
