/**
 * The shared per-user preferences of the UI (UI-F2.8, UI-S-006, UI-S-017; ADR 031 Decision 6:
 * a small store extending `EventTarget`). It reads the caller's preferences through the API
 * once, applies them, and saves every change as one complete `PUT`, so the choices follow the
 * user across hosts.
 *
 * On a host that serves preferences ("api" mode) the navigation tree's pins, section order and
 * collapsed nodes move here too: the first time, a `filesquid.navigation` value on the device is
 * copied to the API once and the key is removed from the device (UI-S-017). When the API
 * already holds navigation choices they win and the device value is only removed. On a host
 * that serves none (503, or any refusal; "device" mode) the navigation tree keeps using device
 * storage as before and the browsing choices apply until the page closes, with a console
 * message that says why.
 *
 * Saves are serialised: while one `PUT` is in flight, later changes are folded into the next
 * one, so the last value always wins and at most {@link MAX_FLUSHES} writes run per burst.
 */
import { ApiError } from "../api/client";
import type { DeviceStorage } from "../app/device-storage";
import { NAVIGATION_STORAGE_KEY, type NavigationStore } from "../navigation/store";
import { readStoredPreferences, type NavigationPreferences } from "../navigation/model";
import {
  DEFAULT_PREFERENCES,
  isEmptyNavigation,
  navigationOf,
  withNavigation,
  type UserPreferences,
} from "./model";
import type { PreferencesSource } from "./source";

/** Where the choices live. */
export type PreferencesMode = "pending" | "api" | "device";

/** The browsing choices a caller may change; navigation choices go through the tree's store. */
export type BrowsingChoices = Partial<Omit<UserPreferences, "schema" | "navigation">>;

/** Most writes one burst of changes may start before it waits for the next change. */
export const MAX_FLUSHES = 8;

/** Everything the service touches, injectable in tests. */
export interface PreferencesEnvironment {
  /** Reads and writes the API's preferences. */
  readonly source: PreferencesSource;
  /** The navigation tree's choices; moved to the API in "api" mode. */
  readonly navigation: NavigationStore;
  /** Device storage that may hold a `filesquid.navigation` value to move; none when refused. */
  readonly storage?: DeviceStorage;
  /** The device key; defaults to {@link NAVIGATION_STORAGE_KEY}. */
  readonly key?: string;
}

/** Shared preferences. Call `start()` once; it resolves when the mode is known. */
export class PreferencesService extends EventTarget {
  private prefs: UserPreferences = DEFAULT_PREFERENCES;
  private current: PreferencesMode = "pending";
  private failed = false;
  private dirty = false;
  private flushing?: Promise<void>;

  /** @param env The source, the navigation store and the device storage. */
  constructor(private readonly env: PreferencesEnvironment) {
    super();
  }

  /** The choices now. */
  get preferences(): UserPreferences {
    return this.prefs;
  }

  /** Where the choices live: pending until `start()` resolves. */
  get mode(): PreferencesMode {
    return this.current;
  }

  /** True while the last save was refused or failed (the choices still apply this session). */
  get saveFailed(): boolean {
    return this.failed;
  }

  /**
   * Read the preferences, apply them, and move the navigation choices to the API. Dispatches
   * `change` once the mode is known. Network I/O via the source; reads and writes Web Storage.
   *
   * @returns Resolves when the mode is known.
   */
  async start(): Promise<void> {
    const read = await this.readApi();
    if (!read) {
      this.current = "device";
      this.dispatchEvent(new Event("change"));
      return;
    }
    this.current = "api";
    this.prefs = await this.moveDeviceNavigation(read);
    this.env.navigation.useRemote((navigation) => this.saveNavigation(navigation));
    this.env.navigation.replace(navigationOf(this.prefs));
    this.dispatchEvent(new Event("change"));
  }

  /**
   * Apply browsing choices and save them. A change that changes nothing is neither saved nor
   * announced. Network I/O in "api" mode.
   *
   * @param choices The members to change.
   */
  update(choices: BrowsingChoices): void {
    const next = { ...this.prefs, ...choices };
    if (JSON.stringify(next) === JSON.stringify(this.prefs)) return;
    this.prefs = next;
    this.dispatchEvent(new Event("change"));
    this.scheduleSave();
  }

  /**
   * Resolve once every save started so far has finished.
   *
   * @returns Resolves when no save is in flight.
   */
  async settled(): Promise<void> {
    while (this.flushing) await this.flushing;
  }

  /** Read the API's preferences; undefined (with a console message) when it keeps none. */
  private async readApi(): Promise<UserPreferences | undefined> {
    try {
      return await this.env.source.read();
    } catch (error) {
      const served = !(error instanceof ApiError && error.status === 503);
      const why = served ? "could not read" : "does not store";
      console.warn(`FileSquid host ${why} preferences; choices apply until the page closes`, error);
      return undefined;
    }
  }

  /** Keep the tree's choices in the preferences and save them. */
  private saveNavigation(navigation: NavigationPreferences): void {
    this.prefs = withNavigation(this.prefs, navigation);
    this.scheduleSave();
  }

  /**
   * Copy a device navigation value to the API once and remove it (UI-S-017). The API's own
   * choices win when it already holds some. Returns the preferences to apply.
   */
  private async moveDeviceNavigation(api: UserPreferences): Promise<UserPreferences> {
    const device = this.readDevice();
    if (!device) return api;
    if (!isEmptyNavigation(navigationOf(api)) || isEmptyNavigation(device)) {
      this.removeDevice();
      return api;
    }
    const merged = withNavigation(api, device);
    try {
      await this.env.source.write(merged);
      this.removeDevice();
    } catch (error) {
      console.warn("FileSquid could not copy the navigation choices to the API yet", error);
    }
    return merged;
  }

  /** The device navigation value, or undefined when there is none or it is unusable. */
  private readDevice(): NavigationPreferences | undefined {
    const storage = this.env.storage;
    if (!storage) return undefined;
    try {
      const text = storage.getItem(this.key);
      if (text === null) return undefined;
      const stored = readStoredPreferences(JSON.parse(text));
      if (stored.ok) return stored.prefs;
      console.warn(`FileSquid ignored the device navigation choices (${stored.problem})`);
    } catch (error) {
      console.warn("FileSquid could not read the device navigation choices", error);
    }
    return undefined;
  }

  /** Remove the device navigation value. */
  private removeDevice(): void {
    try {
      this.env.storage?.removeItem(this.key);
    } catch (error) {
      console.warn("FileSquid could not remove the device navigation choices", error);
    }
  }

  /** Ask for a save; a save already running picks the change up next. */
  private scheduleSave(): void {
    if (this.current !== "api") return;
    this.dirty = true;
    this.flushing ??= this.flush();
  }

  /** Write the latest value, again while changes arrive, at most {@link MAX_FLUSHES} times. */
  private async flush(): Promise<void> {
    try {
      for (let round = 0; round < MAX_FLUSHES && this.dirty; round += 1) {
        this.dirty = false;
        await this.writeOnce();
      }
    } finally {
      this.flushing = undefined;
      if (this.dirty) this.scheduleSave();
    }
  }

  /** One `PUT`; a failure is shown through `saveFailed` and the console, never swallowed. */
  private async writeOnce(): Promise<void> {
    const before = this.failed;
    try {
      await this.env.source.write(this.prefs);
      this.failed = false;
    } catch (error) {
      this.failed = true;
      console.warn("FileSquid could not save the preferences; they apply this session", error);
    }
    if (before !== this.failed) this.dispatchEvent(new Event("change"));
  }

  /** The device key in use. */
  private get key(): string {
    return this.env.key ?? NAVIGATION_STORAGE_KEY;
  }
}
