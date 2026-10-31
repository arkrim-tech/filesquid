/**
 * The quit between the two journal-rebuild specs (Task DE-0.11; DE-S-013), run by
 * `wdio.conf.mjs` once the first spec's worker has ended, outside any test session: close the
 * running app's last window through its embedded WebDriver (a graceful quit, so the app seals
 * its journal at close), wait until the app has quit, then delete its operational store. The
 * Tauri service starts the app again for the next spec, over the same data directory.
 *
 * Kept free of `@wdio/globals`, so the launcher can load it.
 */
import { existsSync, rmSync } from "node:fs";
import path from "node:path";

/** The operational store and its write-ahead files. */
const STORE_FILES = ["filesquid.sqlite", "filesquid.sqlite-wal", "filesquid.sqlite-shm"];

/**
 * The test-instrumented app's data directory (`FILESQUID_E2E_DATA_DIR`, set by `wdio.conf.mjs`).
 *
 * @returns Its path.
 * @throws When the variable is unset: the specs never touch the platform's app data directory.
 */
export function dataDir() {
  const dir = process.env.FILESQUID_E2E_DATA_DIR;
  if (!dir) {
    throw new Error("FILESQUID_E2E_DATA_DIR is not set; refusing to touch the app data directory");
  }
  return dir;
}

/**
 * The base URL of the app's embedded WebDriver.
 *
 * @returns `http://127.0.0.1:<TAURI_WEBDRIVER_PORT>`.
 */
function driverBase() {
  return `http://127.0.0.1:${Number(process.env.TAURI_WEBDRIVER_PORT ?? "4445")}`;
}

/**
 * Wait, at most `limit` milliseconds, until the app's embedded WebDriver no longer answers,
 * which means the app has quit.
 *
 * @param limit The longest wait, in milliseconds.
 * @returns Whether the app quit in time.
 */
export async function waitForQuit(limit) {
  const deadline = Date.now() + limit;
  while (Date.now() < deadline) {
    try {
      await fetch(`${driverBase()}/status`, { signal: AbortSignal.timeout(1000) });
    } catch {
      return true;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  return false;
}

/**
 * Close the app's last window through a session of its own, wait for the app to quit (at most
 * a minute), and delete its operational store.
 *
 * @throws When no session can be opened, the app does not quit, the journal object is missing
 *   or the store is still there.
 */
export async function closeAppAndLoseStore() {
  const created = await fetch(`${driverBase()}/session`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ capabilities: { alwaysMatch: { browserName: "tauri" } } }),
  });
  const session = (await created.json())?.value?.sessionId;
  if (!session) {
    throw new Error(`no WebDriver session for the quit (status ${created.status})`);
  }
  try {
    await fetch(`${driverBase()}/session/${session}/window`, { method: "DELETE" });
  } catch {
    // The app may quit before it answers: the wait below decides.
  }
  if (!(await waitForQuit(60000))) {
    throw new Error("the app did not quit after its last window closed");
  }
  const dir = dataDir();
  if (!existsSync(path.join(dir, "journal.ocfl", "0=ocfl_1.1"))) {
    throw new Error("the app left no sealed journal");
  }
  for (const name of STORE_FILES) {
    rmSync(path.join(dir, name), { force: true });
  }
  if (existsSync(path.join(dir, "filesquid.sqlite"))) {
    throw new Error("the operational store is still there");
  }
}
