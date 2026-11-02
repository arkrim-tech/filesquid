/**
 * The launch redirect to first use (UI-1.3, UI-S-034, owner answer 4: "The first collection
 * should be created automatically upon install or first use"): when the app opens on no address
 * of its own and the host knows no library, it goes to the welcome screen, whose dialog creates
 * the first library and its first collection. A host that knows a library, keeps no library
 * registry (503) or fails leaves the launch address alone, and an address the owner already
 * chose, before or while the host was answering, is never replaced.
 */
import { ApiError } from "../api/client";
import { LIBRARY_ROUTE, type LibrarySource } from "../libraries/model";

/** HTTP status of a host that keeps no library registry. */
const HTTP_UNAVAILABLE = 503;

/** Hashes that mean "no address of its own": the app was just launched. */
const LAUNCH_HASHES: readonly string[] = ["", "#", "#/"];

/** True when the window shows no address of its own. */
function atLaunch(win: Window): boolean {
  return LAUNCH_HASHES.includes(win.location.hash);
}

/**
 * Go to the welcome screen when the host knows no library. Network I/O through the source.
 *
 * @param win The window whose address is read and replaced.
 * @param source The library source; only its `recent` read is used.
 * @returns True when the address was replaced; false when it was left alone, which a failure
 *   (other than a host without a registry, which is expected) logs.
 */
export async function redirectToFirstUse(
  win: Window,
  source: Pick<LibrarySource, "recent">,
): Promise<boolean> {
  if (!atLaunch(win)) return false;
  try {
    const { libraries } = await source.recent();
    if (libraries.length > 0 || !atLaunch(win)) return false;
  } catch (error) {
    const quiet = error instanceof ApiError && error.status === HTTP_UNAVAILABLE;
    if (!quiet) console.warn("FileSquid could not read the recent libraries at launch", error);
    return false;
  }
  win.location.replace(LIBRARY_ROUTE);
  return true;
}
