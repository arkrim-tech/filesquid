/**
 * Reduced-motion hook (UI-S-016, PRD NFR-006). CSS motion already stops under
 * `prefers-reduced-motion: reduce` through the shared component stylesheet; script-driven
 * motion (for example the file browser's hover card, UI-S-006) asks this function first.
 */

/** The media query for a reduced-motion request. */
export const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";

/** The part of `window` the check needs. */
export interface MotionEnvironment {
  /** `window.matchMedia`, absent in some test environments. */
  matchMedia?: (query: string) => Pick<MediaQueryList, "matches">;
}

/**
 * Whether the user asked the operating system for reduced motion. Reads the media query.
 *
 * @param win The window (or a stand-in) to ask.
 * @returns True when reduced motion is requested; false when not, or when it cannot be read.
 */
export function prefersReducedMotion(win: MotionEnvironment): boolean {
  if (typeof win.matchMedia !== "function") return false;
  return win.matchMedia(REDUCED_MOTION_QUERY).matches;
}
