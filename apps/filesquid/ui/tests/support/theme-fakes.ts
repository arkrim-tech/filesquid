/**
 * Test doubles for the theme store: an in-memory storage and a controllable
 * `prefers-color-scheme: dark` media query. Shared by every theme test.
 */
import { ThemeStore, type ThemeMedia, type ThemeStorage } from "../../src/theme/theme-store";

/** A media query fake whose `set` changes the OS preference and notifies listeners. */
export interface FakeMedia extends ThemeMedia {
  /** Change the OS preference and notify listeners. */
  set(dark: boolean): void;
  /** Number of listeners currently registered. */
  listenerCount(): number;
}

/**
 * Create a media query fake.
 *
 * @param dark Whether the OS prefers dark at first.
 * @returns The fake.
 */
export function fakeMedia(dark: boolean): FakeMedia {
  const listeners = new Set<() => void>();
  let matches = dark;
  return {
    get matches() {
      return matches;
    },
    addEventListener: (_type, listener) => void listeners.add(listener),
    removeEventListener: (_type, listener) => void listeners.delete(listener),
    set(value: boolean) {
      matches = value;
      for (const listener of [...listeners]) listener();
    },
    listenerCount: () => listeners.size,
  };
}

/**
 * Create an in-memory storage.
 *
 * @param initial Entries the storage starts with (a choice saved at an earlier launch).
 * @returns The storage and its backing map.
 */
export function fakeStorage(initial: Record<string, string> = {}): {
  storage: ThemeStorage;
  data: Map<string, string>;
} {
  const data = new Map(Object.entries(initial));
  return {
    data,
    storage: {
      getItem: (key) => data.get(key) ?? null,
      setItem: (key, value) => void data.set(key, value),
      removeItem: (key) => void data.delete(key),
    },
  };
}

/**
 * Create a started theme store over fakes, applying to a detached root element.
 *
 * @param dark Whether the OS prefers dark at first.
 * @param initial Storage entries saved at an earlier launch.
 * @returns The store, its media fake, storage and root element.
 */
export function fakeThemeStore(dark = false, initial: Record<string, string> = {}) {
  const media = fakeMedia(dark);
  const { storage, data } = fakeStorage(initial);
  const root = document.createElement("html");
  const store = new ThemeStore({ storage, media, root });
  store.start();
  return { store, media, storage, data, root };
}
