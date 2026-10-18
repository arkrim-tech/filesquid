/**
 * The owner's navigation choices (F-004 default UI, UI-S-017): pinned destinations, section
 * order and collapsed nodes, shared by every `<fs-nav-tree>` on the page (ADR 031 Decision 6:
 * a small store extending `EventTarget`).
 *
 * Until per-user preferences reach the API (API-F2.2, moved there by UI-F2.8), the choices are
 * kept on this device in Web Storage under {@link NAVIGATION_STORAGE_KEY}, as the theme is:
 * a version 1 JSON value of handles, kinds and section ids only, never a name, path, token or
 * content handle. When device storage refuses access the choices still apply until the page
 * closes and the console says why; a stored value that does not parse, has another version or
 * breaks a bound is ignored with a console message and the defaults are used.
 */
import { deviceStorage, type DeviceStorage } from "../app/device-storage";
import {
  EMPTY_NAVIGATION_PREFERENCES,
  MAX_COLLAPSED,
  MAX_PINNED,
  MAX_SECTIONS,
  moveSection,
  readStoredPreferences,
  sectionOrder,
  serializeNavigationPreferences,
  withMember,
  type NavigationPreferences,
  type PinnedDestination,
} from "./model";

/** Web Storage key holding the navigation choices. */
export const NAVIGATION_STORAGE_KEY = "filesquid.navigation";

/** Everything the store touches, injectable in tests. */
export interface NavigationEnvironment {
  /** Device storage; without it the choices last for this page only. */
  storage?: DeviceStorage;
  /** Storage key; defaults to {@link NAVIGATION_STORAGE_KEY}. */
  key?: string;
  /**
   * Where `storage` events from other tabs arrive (the window). When given, a change saved in
   * another tab is read here, so this tab's next save does not overwrite it.
   */
  events?: Pick<EventTarget, "addEventListener" | "removeEventListener">;
}

/**
 * Shared navigation choices. Call `start()` once to restore the saved choices and follow other
 * tabs' saves, and `dispose()` to stop following them. Every change is applied at once, saved
 * on the device and announced with a `change` event; a change that changes nothing is neither
 * saved nor announced.
 */
export class NavigationStore extends EventTarget {
  private prefs: NavigationPreferences = EMPTY_NAVIGATION_PREFERENCES;
  private listening = false;
  private readonly onStorage = (event: Event) => this.otherTabSaved(event);

  /**
   * @param env Device storage, key and the target of other tabs' `storage` events.
   */
  constructor(private readonly env: NavigationEnvironment) {
    super();
  }

  /** The choices now. */
  get preferences(): NavigationPreferences {
    return this.prefs;
  }

  /**
   * Restore the saved choices, dispatch `change` when they differ from the current ones, and
   * follow other tabs' saves. Safe to call again, also after `dispose()`: when storage is absent,
   * refused or holds an unusable value, the choices made on this page are kept. I/O: reads Web
   * Storage.
   */
  start(): void {
    if (!this.listening) this.env.events?.addEventListener("storage", this.onStorage);
    this.listening = true;
    const saved = this.readSaved();
    if (saved) this.adopt(saved);
  }

  /** Stop following other tabs' saves. */
  dispose(): void {
    this.env.events?.removeEventListener("storage", this.onStorage);
    this.listening = false;
  }

  /** Take another tab's save of this key (or a cleared storage) as the current choices. */
  private otherTabSaved(event: Event): void {
    const key = (event as StorageEvent).key;
    if (key !== this.key && key !== null) return;
    const saved = this.readSaved();
    if (saved) this.adopt(saved);
  }

  /** Make `prefs` current and dispatch `change` when they differ from the current ones. */
  private adopt(prefs: NavigationPreferences): void {
    if (serializeNavigationPreferences(prefs) === serializeNavigationPreferences(this.prefs)) {
      return;
    }
    this.prefs = prefs;
    this.dispatchEvent(new Event("change"));
  }

  /**
   * Move a section one step up or down among the sections shown now. I/O: writes Web Storage.
   *
   * @param id The section to move.
   * @param delta -1 for up, +1 for down.
   * @param ids The section ids shown now, in default order.
   */
  moveSection(id: string, delta: -1 | 1, ids: readonly string[]): void {
    const order = sectionOrder(this.prefs.order, ids);
    const moved = moveSection(order, id, delta);
    if (moved.join("\n") === order.join("\n")) return;
    this.change({ order: moved.slice(0, MAX_SECTIONS) });
  }

  /**
   * Collapse or expand a node that is open by default. I/O: writes Web Storage.
   *
   * @param node The node handle.
   * @param collapsed True to collapse.
   */
  setCollapsed(node: string, collapsed: boolean): void {
    this.change({ collapsed: withMember(this.prefs.collapsed, node, collapsed, MAX_COLLAPSED) });
  }

  /**
   * Pin or unpin a destination. Pinning beyond {@link MAX_PINNED} is refused with a console
   * message. I/O: writes Web Storage.
   *
   * @param pin The destination's handle and kind.
   * @param pinned True to pin.
   * @returns False when a pin was refused because the limit is reached; true otherwise.
   */
  setPinned(pin: PinnedDestination, pinned: boolean): boolean {
    const others = this.prefs.pinned.filter((entry) => entry.node !== pin.node);
    if (!pinned) {
      this.change({ pinned: others });
      return true;
    }
    if (others.length < this.prefs.pinned.length) return true;
    if (others.length >= MAX_PINNED) {
      console.warn(`FileSquid keeps at most ${MAX_PINNED} pinned destinations; unpin one first`);
      return false;
    }
    this.change({ pinned: [...others, { node: pin.node, kind: pin.kind }] });
    return true;
  }

  /** Apply a partial change; save and announce it when something changed. */
  private change(patch: Partial<NavigationPreferences>): void {
    const next = { ...this.prefs, ...patch };
    if (serializeNavigationPreferences(next) === serializeNavigationPreferences(this.prefs)) {
      return;
    }
    this.prefs = next;
    this.save();
    this.dispatchEvent(new Event("change"));
  }

  /**
   * Read the saved choices: none saved means the defaults. Without storage, unreadable
   * storage, text that is not JSON, and a value `readStoredPreferences` refuses each give
   * undefined (the last three with a console message), so the caller keeps the choices it has,
   * which are the defaults at first start. I/O: Web Storage.
   */
  private readSaved(): NavigationPreferences | undefined {
    if (!this.env.storage) return undefined;
    try {
      const text = this.env.storage.getItem(this.key);
      if (text === null) return EMPTY_NAVIGATION_PREFERENCES;
      const stored = readStoredPreferences(JSON.parse(text));
      if (stored.ok) return stored.prefs;
      console.warn(`FileSquid ignored the saved navigation choices: ${stored.problem}`);
    } catch (error) {
      console.warn("FileSquid could not read the saved navigation choices; using none", error);
    }
    return undefined;
  }

  /** Write the choices to device storage. I/O: Web Storage. */
  private save(): void {
    try {
      this.env.storage?.setItem(this.key, serializeNavigationPreferences(this.prefs));
    } catch (error) {
      console.warn(
        "FileSquid could not save the navigation choices; they apply until the app closes",
        error,
      );
    }
  }

  /** The storage key in use. */
  private get key(): string {
    return this.env.key ?? NAVIGATION_STORAGE_KEY;
  }
}

/**
 * Create the navigation store for a window, over its `localStorage`. Call `start()` on the
 * result before the first render.
 *
 * @param win The window whose storage is used.
 * @param key Storage key; defaults to {@link NAVIGATION_STORAGE_KEY}.
 * @returns A store that has not started yet.
 */
export function createNavigationStore(win: Window, key?: string): NavigationStore {
  const storage = deviceStorage(win, "the navigation choices");
  return new NavigationStore({ storage, key, events: win });
}

/** The started store each window shares when a tree is given none. */
const shared = new WeakMap<Window, NavigationStore>();

/**
 * The window's shared, started navigation store, created on first use, so every tree on the
 * page follows the same choices. A store a caller disposed is started again here, so it
 * follows other tabs' saves again. I/O: reads Web Storage.
 *
 * @param win The window.
 * @returns The shared store.
 */
export function sharedNavigationStore(win: Window): NavigationStore {
  const existing = shared.get(win);
  if (existing) {
    existing.start(); // follows other tabs again if a caller disposed it; keeps its choices
    return existing;
  }
  const store = createNavigationStore(win);
  store.start();
  shared.set(win, store);
  return store;
}
