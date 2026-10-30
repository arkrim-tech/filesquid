/**
 * Unit tests for the per-user preferences (UI-F2.8, UI-S-006, UI-S-017): the contract check, the
 * API source, and the service that reads them once, saves every change as one complete PUT,
 * copies a device `filesquid.navigation` value to the API once and removes the key, and keeps
 * working when the host stores no preferences or storage is refused.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { NavigationStore, NAVIGATION_STORAGE_KEY } from "../../src/navigation/store";
import {
  DEFAULT_PREFERENCES,
  isEmptyNavigation,
  navigationOf,
  parsePreferences,
  withNavigation,
  type UserPreferences,
} from "../../src/preferences/model";
import { PreferencesService, MAX_FLUSHES } from "../../src/preferences/service";
import {
  ClientPreferencesSource,
  MemoryPreferencesSource,
  type PreferencesSource,
} from "../../src/preferences/source";

let warnings: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  warnings = vi.spyOn(console, "warn").mockImplementation(() => undefined);
});
afterEach(() => vi.restoreAllMocks());

/** A storage over a map, optionally refusing every call. */
function storage(initial: Record<string, string> = {}, refuse = false) {
  const data = new Map(Object.entries(initial));
  const guard = () => {
    if (refuse) throw new DOMException("blocked", "SecurityError");
  };
  return {
    data,
    getItem: (key: string) => (guard(), data.get(key) ?? null),
    setItem: (key: string, value: string) => (guard(), void data.set(key, value)),
    removeItem: (key: string) => (guard(), void data.delete(key)),
  };
}

const NAV = {
  version: 1,
  pinned: [{ node: "c1", kind: "collection" }],
  order: ["pinned", "collections"],
  collapsed: ["l1"],
};

describe("parsePreferences", () => {
  it("accepts the defaults and a full value", () => {
    expect(parsePreferences(DEFAULT_PREFERENCES)).toEqual(DEFAULT_PREFERENCES);
    const full = { ...DEFAULT_PREFERENCES, layout: "grid", hover_card: false, navigation: NAV };
    expect(parsePreferences(full)).toMatchObject({
      layout: "grid",
      navigation: { pinned: NAV.pinned },
    });
  });
  it("refuses a missing, unknown-valued or mistyped member", () => {
    expect(parsePreferences(null)).toBeUndefined();
    expect(parsePreferences({ ...DEFAULT_PREFERENCES, layout: "cards" })).toBeUndefined();
    expect(parsePreferences({ ...DEFAULT_PREFERENCES, sort_key: "created" })).toBeUndefined();
    expect(parsePreferences({ ...DEFAULT_PREFERENCES, hover_card: "yes" })).toBeUndefined();
    expect(parsePreferences({ ...DEFAULT_PREFERENCES, schema: 2 })).toBeUndefined();
  });
  it("refuses navigation choices over a bound, with another version or repeated", () => {
    const nav = (patch: object) => ({ ...DEFAULT_PREFERENCES, navigation: { ...NAV, ...patch } });
    const pins = (n: number) =>
      Array.from({ length: n }, (_, i) => ({ node: `n${i}`, kind: "collection" }));
    expect(parsePreferences(nav({ pinned: pins(100) }))).toBeDefined();
    expect(parsePreferences(nav({ pinned: pins(101) }))).toBeUndefined();
    expect(
      parsePreferences(nav({ order: Array.from({ length: 17 }, (_, i) => `s${i}`) })),
    ).toBeUndefined();
    expect(
      parsePreferences(nav({ collapsed: Array.from({ length: 1001 }, (_, i) => `n${i}`) })),
    ).toBeUndefined();
    expect(parsePreferences(nav({ version: 2 }))).toBeUndefined();
    expect(parsePreferences(nav({ collapsed: ["a", "a"] }))).toBeUndefined();
    expect(parsePreferences(nav({ pinned: [{ node: "a", kind: "nonsense" }] }))).toBeUndefined();
  });
  it("keeps only the contract members", () => {
    const out = parsePreferences({ ...DEFAULT_PREFERENCES, extra: "x" }) as unknown as Record<
      string,
      unknown
    >;
    expect(out.extra).toBeUndefined();
  });
  it("moves navigation choices in and out of a preferences value", () => {
    const prefs = withNavigation(DEFAULT_PREFERENCES, NAV as never);
    expect(navigationOf(prefs)).toEqual({
      pinned: NAV.pinned,
      order: NAV.order,
      collapsed: NAV.collapsed,
    });
    expect(isEmptyNavigation(navigationOf(DEFAULT_PREFERENCES))).toBe(true);
    expect(isEmptyNavigation(navigationOf(prefs))).toBe(false);
  });
});

describe("ClientPreferencesSource", () => {
  it("reads and writes /api/v1/preferences and checks the answer", async () => {
    const request = vi.fn().mockResolvedValue(DEFAULT_PREFERENCES);
    const source = new ClientPreferencesSource({ request } as never);
    await expect(source.read()).resolves.toEqual(DEFAULT_PREFERENCES);
    expect(request).toHaveBeenLastCalledWith("/api/v1/preferences", { signal: undefined });
    await source.write(DEFAULT_PREFERENCES);
    expect(request).toHaveBeenLastCalledWith(
      "/api/v1/preferences",
      expect.objectContaining({ method: "PUT", body: DEFAULT_PREFERENCES }),
    );
  });
  it("rejects an answer that breaks the contract", async () => {
    const request = vi.fn().mockResolvedValue({ layout: "list" });
    await expect(new ClientPreferencesSource({ request } as never).read()).rejects.toThrow(
      TypeError,
    );
  });
});

/** A started service over a memory source holding `stored`. */
async function started(stored: UserPreferences, store = storage(), refused = false) {
  const source = new MemoryPreferencesSource(stored);
  const navigation = new NavigationStore({});
  const service = new PreferencesService({
    source,
    navigation,
    storage: refused ? undefined : store,
  });
  await service.start();
  return { service, source, navigation, store };
}

describe("PreferencesService with a host that stores preferences", () => {
  it("applies what the API holds", async () => {
    const { service } = await started({ ...DEFAULT_PREFERENCES, layout: "grid", sort_key: "size" });
    expect(service.mode).toBe("api");
    expect(service.preferences).toMatchObject({ layout: "grid", sort_key: "size" });
  });

  it("saves a change as one complete PUT and announces it", async () => {
    const { service, source } = await started(DEFAULT_PREFERENCES);
    const seen = vi.fn();
    service.addEventListener("change", seen);
    service.update({ sort_key: "size", sort_direction: "descending" });
    await service.settled();
    expect(source.writes).toHaveLength(1);
    expect(source.writes[0]).toMatchObject({
      schema: 1,
      sort_key: "size",
      sort_direction: "descending",
      layout: "list",
    });
    expect(seen).toHaveBeenCalled();
  });

  it("neither saves nor announces a change that changes nothing", async () => {
    const { service, source } = await started(DEFAULT_PREFERENCES);
    const seen = vi.fn();
    service.addEventListener("change", seen);
    service.update({ layout: "list" });
    await service.settled();
    expect(source.writes).toHaveLength(0);
    expect(seen).not.toHaveBeenCalled();
  });

  it("folds changes made during a save into the next one, so the last value wins", async () => {
    const { service, source } = await started(DEFAULT_PREFERENCES);
    service.update({ layout: "grid" });
    service.update({ hover_card: false });
    service.update({ folder_placement: "mixed" });
    await service.settled();
    expect(source.writes.at(-1)).toMatchObject({
      layout: "grid",
      hover_card: false,
      folder_placement: "mixed",
    });
    expect(source.writes).toHaveLength(2);
    expect(MAX_FLUSHES).toBeGreaterThanOrEqual(2);
  });

  it("keeps the choice and says the save failed when the PUT is refused", async () => {
    const { service, source } = await started(DEFAULT_PREFERENCES);
    source.write = () =>
      Promise.reject(new ApiError(409, "/api/v1/preferences", { code: "conflict" }));
    service.update({ layout: "grid" });
    await service.settled();
    expect(service.preferences.layout).toBe("grid");
    expect(service.saveFailed).toBe(true);
    expect(warnings).toHaveBeenCalled();
  });

  it("moves the navigation choices to the API and saves later changes there, not on the device", async () => {
    const { service, source, navigation, store } = await started({
      ...DEFAULT_PREFERENCES,
      navigation: NAV as never,
    });
    expect(navigation.preferences.pinned).toEqual(NAV.pinned);
    navigation.setCollapsed("l2", true);
    await service.settled();
    expect(source.writes.at(-1)?.navigation.collapsed).toEqual(["l1", "l2"]);
    expect(store.data.has(NAVIGATION_STORAGE_KEY)).toBe(false);
  });
});

describe("the one-time copy of the device navigation value (UI-S-017)", () => {
  const device = JSON.stringify(NAV);

  it("copies it to the API once, applies it, and removes the key from the device", async () => {
    const { service, source, navigation, store } = await started(
      DEFAULT_PREFERENCES,
      storage({ [NAVIGATION_STORAGE_KEY]: device }),
    );
    expect(source.writes).toHaveLength(1);
    expect(source.writes[0]?.navigation.pinned).toEqual(NAV.pinned);
    expect(store.data.has(NAVIGATION_STORAGE_KEY)).toBe(false);
    expect(navigation.preferences.pinned).toEqual(NAV.pinned);
    expect(service.preferences.navigation.order).toEqual(NAV.order);
  });

  it("merges the device value into the choices the API already holds, losing neither", async () => {
    const mine = { ...NAV, pinned: [{ node: "c9", kind: "collection" }], collapsed: ["l9"] };
    const { source, navigation, store } = await started(
      { ...DEFAULT_PREFERENCES, navigation: mine as never },
      storage({ [NAVIGATION_STORAGE_KEY]: device }),
    );
    expect(source.writes).toHaveLength(1);
    const merged = source.writes[0]?.navigation;
    expect(merged?.pinned).toEqual([
      { node: "c9", kind: "collection" },
      { node: "c1", kind: "collection" },
    ]);
    expect(merged?.order).toEqual(mine.order);
    expect(merged?.collapsed).toEqual(["l9", "l1"]);
    expect(store.data.has(NAVIGATION_STORAGE_KEY)).toBe(false);
    expect(navigation.preferences.pinned).toHaveLength(2);
  });

  it("writes nothing when the API already holds everything the device holds", async () => {
    const { source, store } = await started(
      { ...DEFAULT_PREFERENCES, navigation: NAV as never },
      storage({ [NAVIGATION_STORAGE_KEY]: device }),
    );
    expect(source.writes).toHaveLength(0);
    expect(store.data.has(NAVIGATION_STORAGE_KEY)).toBe(false);
  });

  it("keeps each merged list inside its bound", async () => {
    const pins = (from: number, n: number) =>
      Array.from({ length: n }, (_, i) => ({ node: `n${from + i}`, kind: "collection" }));
    const api = { ...NAV, pinned: pins(0, 99), order: [], collapsed: [] };
    const dev = { ...NAV, pinned: pins(99, 5) };
    const { source } = await started(
      { ...DEFAULT_PREFERENCES, navigation: api as never },
      storage({ [NAVIGATION_STORAGE_KEY]: JSON.stringify(dev) }),
    );
    expect(source.writes[0]?.navigation.pinned).toHaveLength(100);
  });

  it("does nothing when the device holds no value", async () => {
    const { source } = await started(DEFAULT_PREFERENCES);
    expect(source.writes).toHaveLength(0);
  });

  it("ignores a device value that does not parse, with a console message, and leaves it", async () => {
    const { source, store } = await started(
      DEFAULT_PREFERENCES,
      storage({ [NAVIGATION_STORAGE_KEY]: "{oops" }),
    );
    expect(source.writes).toHaveLength(0);
    expect(store.data.get(NAVIGATION_STORAGE_KEY)).toBe("{oops");
    expect(warnings).toHaveBeenCalled();
  });

  it("keeps the key when the copy to the API fails, says so, and tries again on the next start", async () => {
    const store = storage({ [NAVIGATION_STORAGE_KEY]: device });
    const source = new MemoryPreferencesSource(DEFAULT_PREFERENCES);
    const write = source.write.bind(source);
    source.write = () => Promise.reject(new Error("offline"));
    const navigation = new NavigationStore({});
    const service = new PreferencesService({ source, navigation, storage: store });
    await service.start();
    expect(store.data.has(NAVIGATION_STORAGE_KEY)).toBe(true);
    expect(navigation.preferences.pinned).toEqual(NAV.pinned);
    expect(service.saveFailed).toBe(true);
    source.write = write;
    const next = new PreferencesService({
      source,
      navigation: new NavigationStore({}),
      storage: store,
    });
    await next.start();
    expect(source.writes).toHaveLength(1);
    expect(store.data.has(NAVIGATION_STORAGE_KEY)).toBe(false);
  });

  it("survives refused device storage", async () => {
    const { service } = await started(DEFAULT_PREFERENCES, storage({}, true));
    expect(service.mode).toBe("api");
  });
});

describe("PreferencesService on a host that stores none", () => {
  /** A source whose every call answers 503, as a host without a preference store does. */
  function unavailable(): PreferencesSource {
    const refuse = () =>
      Promise.reject(new ApiError(503, "/api/v1/preferences", { code: "unavailable" }));
    return { read: refuse, write: refuse };
  }

  it("keeps the choices for this page only, leaves the navigation on the device, and says why", async () => {
    const navigation = new NavigationStore({ storage: storage() });
    const service = new PreferencesService({ source: unavailable(), navigation });
    await service.start();
    expect(service.mode).toBe("device");
    service.update({ layout: "grid" });
    expect(service.preferences.layout).toBe("grid");
    expect(warnings).toHaveBeenCalled();
    navigation.setCollapsed("l1", true);
    expect(navigation.preferences.collapsed).toEqual(["l1"]);
  });

  it("does not remove the device value it did not move", async () => {
    const store = storage({ [NAVIGATION_STORAGE_KEY]: JSON.stringify(NAV) });
    const service = new PreferencesService({
      source: unavailable(),
      navigation: new NavigationStore({}),
      storage: store,
    });
    await service.start();
    expect(store.data.has(NAVIGATION_STORAGE_KEY)).toBe(true);
  });
});

describe("PreferencesService changes made while it starts, and a later start", () => {
  /** A source whose read waits until `release` is called. */
  function slowSource(stored: UserPreferences) {
    const inner = new MemoryPreferencesSource(stored);
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const source: PreferencesSource & { writes: UserPreferences[] } = {
      writes: inner.writes,
      read: async () => (await gate, inner.read()),
      write: (prefs) => inner.write(prefs),
    };
    return { source, release };
  }

  it("applies a change made before the mode is known on top of what the API holds, and saves it", async () => {
    const held = { ...DEFAULT_PREFERENCES, sort_key: "size", navigation: NAV } as UserPreferences;
    const { source, release } = slowSource(held);
    const service = new PreferencesService({ source, navigation: new NavigationStore({}) });
    const starting = service.start();
    service.update({ layout: "grid" });
    expect(service.preferences.layout).toBe("grid");
    expect(source.writes).toHaveLength(0);
    release();
    await starting;
    await service.settled();
    expect(service.mode).toBe("api");
    expect(source.writes).toHaveLength(1);
    expect(source.writes[0]).toMatchObject({ layout: "grid", sort_key: "size" });
    expect(source.writes[0]?.navigation.pinned).toEqual(NAV.pinned);
  });

  it("starts again after a failed read and then saves in the API", async () => {
    const inner = new MemoryPreferencesSource(DEFAULT_PREFERENCES);
    let reads = 0;
    const source: PreferencesSource = {
      read: () =>
        ++reads === 1
          ? Promise.reject(new ApiError(401, "/api/v1/preferences", { code: "unauthorized" }))
          : inner.read(),
      write: (prefs) => inner.write(prefs),
    };
    const service = new PreferencesService({ source, navigation: new NavigationStore({}) });
    await service.start();
    expect(service.mode).toBe("device");
    service.update({ layout: "grid" });
    await service.start();
    await service.settled();
    expect(service.mode).toBe("api");
    expect(inner.writes.at(-1)).toMatchObject({ layout: "grid" });
  });

  it("does not start a second read while one is running", async () => {
    const read = vi.fn().mockResolvedValue(DEFAULT_PREFERENCES);
    const service = new PreferencesService({
      source: { read, write: vi.fn() },
      navigation: new NavigationStore({}),
    });
    await Promise.all([service.start(), service.start()]);
    expect(read).toHaveBeenCalledTimes(1);
  });

  it("hands out copies of the navigation choices and keeps the defaults frozen", () => {
    const nav = navigationOf(DEFAULT_PREFERENCES);
    (nav.pinned as unknown[]).push({ node: "x", kind: "collection" });
    expect(DEFAULT_PREFERENCES.navigation.pinned).toHaveLength(0);
    expect(Object.isFrozen(DEFAULT_PREFERENCES.navigation.pinned)).toBe(true);
    expect(Object.isFrozen(DEFAULT_PREFERENCES.navigation.order)).toBe(true);
    expect(Object.isFrozen(DEFAULT_PREFERENCES.navigation.collapsed)).toBe(true);
  });
});
