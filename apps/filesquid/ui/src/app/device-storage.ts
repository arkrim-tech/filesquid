/**
 * Device storage for display preferences (the theme choice, navigation choices). Web Storage
 * can refuse access (private browsing, blocked site data), so every store reaches it through
 * {@link deviceStorage}, which returns nothing instead of throwing and says why in the console.
 * A store without storage keeps its choices until the page closes.
 */

/** The part of Web Storage the preference stores use. */
export type DeviceStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

/**
 * Return the window's `localStorage`, or undefined when the browser refuses access. I/O: reads
 * the window's storage accessor.
 *
 * @param win The window whose storage is used.
 * @param what What will not be kept without storage, for the console message (for example
 *   "the theme choice").
 * @returns The storage, or undefined (after a console warning) when access is refused.
 */
export function deviceStorage(win: Window, what: string): DeviceStorage | undefined {
  try {
    return win.localStorage;
  } catch (error) {
    console.warn(`FileSquid cannot use device storage; ${what} will not be kept`, error);
    return undefined;
  }
}
