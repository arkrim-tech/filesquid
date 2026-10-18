/**
 * ThemeStore (SRS-UI-007): follows the OS colour scheme by default, applies an explicit
 * light or dark choice to the root element, keeps that choice on the device across launches,
 * and tells subscribers when the resolved theme changes.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  DARK_SCHEME_QUERY,
  THEME_STORAGE_KEY,
  ThemeStore,
  createThemeStore,
  parseThemeChoice,
  resolveTheme,
  type ThemeMedia,
  type ThemeStorage,
} from "../../src/theme/theme-store";

/** In-memory stand-in for localStorage that records every call. */
function fakeStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  const storage: ThemeStorage = {
    getItem: (key) => data.get(key) ?? null,
    setItem: (key, value) => void data.set(key, value),
    removeItem: (key) => void data.delete(key),
  };
  return { storage, data };
}

/** Stand-in for matchMedia("(prefers-color-scheme: dark)") whose answer the test changes. */
function fakeMedia(initial: boolean) {
  const listeners = new Set<() => void>();
  let matches = initial;
  const media: ThemeMedia & { set(value: boolean): void; listeners: Set<() => void> } = {
    get matches() {
      return matches;
    },
    addEventListener: (_type: "change", listener: () => void) => void listeners.add(listener),
    removeEventListener: (_type: "change", listener: () => void) => void listeners.delete(listener),
    set(value: boolean) {
      matches = value;
      for (const listener of [...listeners]) listener();
    },
    listeners,
  };
  return media;
}

/** Build a started store over fakes. */
function started(options: { stored?: string; dark?: boolean } = {}) {
  const { storage, data } = fakeStorage(
    options.stored === undefined ? {} : { [THEME_STORAGE_KEY]: options.stored },
  );
  const media = fakeMedia(options.dark ?? false);
  const root = document.createElement("html");
  const store = new ThemeStore({ storage, media, root });
  store.start();
  return { store, storage, data, media, root };
}

afterEach(() => vi.restoreAllMocks());

describe("parseThemeChoice and resolveTheme", () => {
  it.each([
    ["light", "light"],
    ["dark", "dark"],
    ["system", "system"],
    [null, "system"],
    ["sepia", "system"],
    [42, "system"],
  ] as const)("reads %j as %s", (value, choice) => {
    expect(parseThemeChoice(value)).toBe(choice);
  });
  it("resolves system from the OS preference and keeps an explicit choice", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });
  it("names the storage key and media query", () => {
    expect(THEME_STORAGE_KEY).toBe("filesquid.theme");
    expect(DARK_SCHEME_QUERY).toBe("(prefers-color-scheme: dark)");
  });
});

describe("ThemeStore", () => {
  it("follows the OS by default: no data-theme and the OS scheme resolved", () => {
    const { store, root } = started({ dark: true });
    expect(store.choice).toBe("system");
    expect(store.resolved).toBe("dark");
    expect(root.hasAttribute("data-theme")).toBe(false);
  });

  it("restores a stored choice at start and applies it to the root", () => {
    const { store, root } = started({ stored: "dark", dark: false });
    expect(store.choice).toBe("dark");
    expect(store.resolved).toBe("dark");
    expect(root.getAttribute("data-theme")).toBe("dark");
  });

  it("ignores an unknown stored value", () => {
    const { store, root } = started({ stored: "neon" });
    expect(store.choice).toBe("system");
    expect(root.hasAttribute("data-theme")).toBe(false);
  });

  it("persists an explicit choice and removes it for system", () => {
    const { store, data, root } = started();
    store.setChoice("light");
    expect(data.get(THEME_STORAGE_KEY)).toBe("light");
    expect(root.getAttribute("data-theme")).toBe("light");
    store.setChoice("system");
    expect(data.has(THEME_STORAGE_KEY)).toBe(false);
    expect(root.hasAttribute("data-theme")).toBe(false);
  });

  it("a new store over the same storage starts with the saved choice (next launch)", () => {
    const { store, storage } = started();
    store.setChoice("dark");
    const next = new ThemeStore({
      storage,
      media: fakeMedia(false),
      root: document.createElement("html"),
    });
    next.start();
    expect(next.choice).toBe("dark");
  });

  it("dispatches change with the new resolved theme", () => {
    const { store } = started();
    const seen: string[] = [];
    store.addEventListener("change", () => seen.push(`${store.choice}/${store.resolved}`));
    store.setChoice("dark");
    store.setChoice("dark");
    store.setChoice("system");
    expect(seen).toEqual(["dark/dark", "system/light"]);
  });

  it("follows OS changes while the choice is system, and ignores them otherwise", () => {
    const { store, media } = started({ dark: false });
    const resolved: string[] = [];
    store.addEventListener("change", () => resolved.push(store.resolved));
    media.set(true);
    expect(resolved).toEqual(["dark"]);
    store.setChoice("light");
    media.set(false);
    media.set(true);
    expect(resolved).toEqual(["dark", "light"]);
  });

  it("start is idempotent and dispose removes the OS listener", () => {
    const { store, media } = started();
    store.start();
    expect(media.listeners.size).toBe(1);
    store.dispose();
    expect(media.listeners.size).toBe(0);
  });

  it("still applies the choice when storage throws, and logs why", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const broken: ThemeStorage = {
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
    const root = document.createElement("html");
    const store = new ThemeStore({ storage: broken, media: fakeMedia(false), root });
    store.start();
    expect(store.choice).toBe("system");
    store.setChoice("dark");
    expect(root.getAttribute("data-theme")).toBe("dark");
    expect(warn).toHaveBeenCalledTimes(2);
  });

  it("works without storage (choice lasts for this page only)", () => {
    const root = document.createElement("html");
    const store = new ThemeStore({ media: fakeMedia(false), root });
    store.start();
    store.setChoice("dark");
    expect(store.resolved).toBe("dark");
  });
});

describe("createThemeStore", () => {
  it("uses the window's localStorage, matchMedia and document element", () => {
    const media = fakeMedia(true);
    const matchMedia = vi.fn(() => media as unknown as MediaQueryList);
    const win = { matchMedia, localStorage: fakeStorage().storage, document } as unknown as Window;
    const store = createThemeStore(win);
    store.start();
    expect(matchMedia).toHaveBeenCalledWith(DARK_SCHEME_QUERY);
    store.setChoice("light");
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");
    store.setChoice("system");
    store.dispose();
  });

  it("falls back to no storage when reading localStorage throws", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const win = {
      matchMedia: () => fakeMedia(false),
      document,
      get localStorage(): Storage {
        throw new Error("SecurityError");
      },
    } as unknown as Window;
    const store = createThemeStore(win);
    store.start();
    store.setChoice("dark");
    expect(store.resolved).toBe("dark");
    expect(warn).toHaveBeenCalledOnce();
    store.setChoice("system");
    store.dispose();
  });
});
