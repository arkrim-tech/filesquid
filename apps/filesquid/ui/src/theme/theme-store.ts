/**
 * Theme state shared by the shell, the theme control and any view (ADR 031 Decision 6: a
 * small store extending `EventTarget`). The theme follows the operating-system colour scheme
 * by default; an explicit light or dark choice is applied as `data-theme` on the root element
 * and kept on this device so it survives a relaunch (SRS-UI-007).
 *
 * The choice is stored in Web Storage under {@link THEME_STORAGE_KEY}. It is a display
 * preference, not library data or a credential. The API has no preferences route yet; when
 * per-user preferences arrive (UI-F2.8), a storage backed by the API can replace this one
 * through {@link ThemeStorage} without changing the control.
 */

/** What the owner chose: follow the OS, or force light or dark. */
export type ThemeChoice = "system" | "light" | "dark";

/** The theme actually shown. */
export type ResolvedTheme = "light" | "dark";

/** Every choice, in the order the theme control shows them. */
export const THEME_CHOICES: readonly ThemeChoice[] = ["system", "light", "dark"];

/** Web Storage key holding an explicit choice (absent means follow the OS). */
export const THEME_STORAGE_KEY = "filesquid.theme";

/** Media query that reports the OS dark preference. */
export const DARK_SCHEME_QUERY = "(prefers-color-scheme: dark)";

/** The part of Web Storage the store uses. */
export type ThemeStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

/** The part of a `MediaQueryList` the store uses. */
export interface ThemeMedia {
  /** Whether the OS currently prefers dark. */
  readonly matches: boolean;
  /** Listen for OS preference changes. */
  addEventListener(type: "change", listener: () => void): void;
  /** Stop listening for OS preference changes. */
  removeEventListener(type: "change", listener: () => void): void;
}

/** Everything the store touches, injectable in tests. */
export interface ThemeEnvironment {
  /** Device storage for the choice; without it the choice lasts for this page only. */
  storage?: ThemeStorage;
  /** The `prefers-color-scheme: dark` query. */
  media: ThemeMedia;
  /** Element that receives `data-theme` (the document element). */
  root: HTMLElement;
  /** Storage key; defaults to {@link THEME_STORAGE_KEY}. */
  key?: string;
}

/**
 * Read a stored value as a choice. Anything unknown means follow the OS.
 *
 * @param value Stored value, for example from `localStorage.getItem`.
 * @returns The choice.
 */
export function parseThemeChoice(value: unknown): ThemeChoice {
  return value === "light" || value === "dark" ? value : "system";
}

/**
 * Decide which theme to show.
 *
 * @param choice The owner's choice.
 * @param systemDark Whether the OS prefers dark.
 * @returns The theme to show.
 */
export function resolveTheme(choice: ThemeChoice, systemDark: boolean): ResolvedTheme {
  if (choice !== "system") return choice;
  return systemDark ? "dark" : "light";
}

/**
 * Shared theme state. Call `start()` once to restore the saved choice and follow the OS,
 * and `dispose()` to stop following it. Dispatches `change` whenever the choice or the
 * resolved theme changes.
 */
export class ThemeStore extends EventTarget {
  private current: ThemeChoice = "system";
  private shown: ResolvedTheme = "light";
  private listening = false;
  private readonly onSystemChange = () => this.update(this.current);

  /**
   * @param env Storage, OS media query and root element to apply the theme to.
   */
  constructor(private readonly env: ThemeEnvironment) {
    super();
  }

  /** The owner's choice. */
  get choice(): ThemeChoice {
    return this.current;
  }

  /** The theme shown now. */
  get resolved(): ResolvedTheme {
    return this.shown;
  }

  /** Restore the saved choice, apply it, and follow OS changes. Safe to call again. */
  start(): void {
    if (!this.listening) this.env.media.addEventListener("change", this.onSystemChange);
    this.listening = true;
    this.current = this.readSavedChoice();
    this.shown = resolveTheme(this.current, this.env.media.matches);
    this.applyToRoot();
  }

  /**
   * Choose a theme: apply it now and save it on this device. Side effects: writes the root
   * element's `data-theme` and the device storage.
   *
   * @param choice The new choice.
   */
  setChoice(choice: ThemeChoice): void {
    if (choice === this.current) return;
    this.saveChoice(choice);
    this.update(choice);
  }

  /** Stop following OS changes. */
  dispose(): void {
    this.env.media.removeEventListener("change", this.onSystemChange);
    this.listening = false;
  }

  /** Apply a choice and dispatch `change` when the choice or the shown theme changed. */
  private update(choice: ThemeChoice): void {
    const shown = resolveTheme(choice, this.env.media.matches);
    if (choice === this.current && shown === this.shown) return;
    this.current = choice;
    this.shown = shown;
    this.applyToRoot();
    this.dispatchEvent(new Event("change"));
  }

  /** Set `data-theme` for an explicit choice, or remove it so the CSS follows the OS. */
  private applyToRoot(): void {
    if (this.current === "system") this.env.root.removeAttribute("data-theme");
    else this.env.root.setAttribute("data-theme", this.current);
  }

  /** Read the saved choice from device storage; a storage failure means follow the OS. */
  private readSavedChoice(): ThemeChoice {
    try {
      return parseThemeChoice(this.env.storage?.getItem(this.key));
    } catch (error) {
      console.warn("FileSquid could not read the saved theme; following the system", error);
      return "system";
    }
  }

  /** Write the choice to device storage (remove it for `system`). I/O: Web Storage. */
  private saveChoice(choice: ThemeChoice): void {
    try {
      if (choice === "system") this.env.storage?.removeItem(this.key);
      else this.env.storage?.setItem(this.key, choice);
    } catch (error) {
      console.warn("FileSquid could not save the theme; it applies until the app closes", error);
    }
  }

  /** The storage key in use. */
  private get key(): string {
    return this.env.key ?? THEME_STORAGE_KEY;
  }
}

/**
 * Create the theme store for a window: its `localStorage`, its `prefers-color-scheme` query
 * and its document element. Call `start()` on the result before the first render.
 *
 * @param win The window whose theme is managed.
 * @param key Storage key; defaults to {@link THEME_STORAGE_KEY}.
 * @returns A store that has not started yet.
 */
export function createThemeStore(win: Window, key?: string): ThemeStore {
  return new ThemeStore({
    storage: deviceStorage(win),
    media: win.matchMedia(DARK_SCHEME_QUERY),
    root: win.document.documentElement,
    key,
  });
}

/** Return the window's `localStorage`, or undefined when the browser refuses access. */
function deviceStorage(win: Window): ThemeStorage | undefined {
  try {
    return win.localStorage;
  } catch (error) {
    console.warn("FileSquid cannot use device storage; the theme choice will not be kept", error);
    return undefined;
  }
}
