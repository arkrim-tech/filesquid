/**
 * Helpers of the journal-rebuild specs (Task DE-0.11; DE-S-013): read the activity journal
 * through the API from the app page, and hand a journal entry from the first spec to the
 * second. The quit between them is `app-restart.mjs`.
 */
import { browser } from "@wdio/globals";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

/**
 * The scheme base URL the app page uses on this platform (ADR 030 Decision 3).
 *
 * @returns `http://filesquid.localhost` on Windows, `filesquid://localhost` elsewhere.
 */
export function apiBase() {
  return process.platform === "win32" ? "http://filesquid.localhost" : "filesquid://localhost";
}

/**
 * Read the journal entries with `operation` from the app page, newest first.
 *
 * @param operation The journal operation, for example `host.started`.
 * @returns The entries, as the API returns them.
 */
export async function journalEntries(operation) {
  return browser.execute(
    async (base, op) => {
      const token = window.__FILESQUID_LAUNCH_TOKEN__;
      const query = new URLSearchParams({ operation: op, order: "desc" });
      const response = await fetch(`${base}/api/v1/admin/journal?${query}`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      const page = await response.json();
      return page.entries ?? [];
    },
    apiBase(),
    operation,
  );
}

/**
 * Wait until the journal holds an entry with `operation`, and answer the newest.
 *
 * @param operation The journal operation.
 * @returns The newest matching entry.
 */
export async function waitForEntry(operation) {
  let found;
  await browser.waitUntil(
    async () => {
      [found] = await journalEntries(operation);
      return Boolean(found);
    },
    { timeout: 30000, interval: 250, timeoutMsg: `no ${operation} entry was journaled` },
  );
  return found;
}

/**
 * The file that hands the first spec's journal entry to the second, in the run directory.
 *
 * @returns Its path.
 */
function handoverFile() {
  return path.join(process.env.FILESQUID_DESKTOP_E2E_RUN, "journal-rebuild.json");
}

/**
 * Keep `entry` for the second spec.
 *
 * @param entry A journal entry as the API returned it.
 */
export function keepEntry(entry) {
  writeFileSync(handoverFile(), JSON.stringify(entry));
}

/**
 * The entry the first spec kept.
 *
 * @returns The journal entry.
 */
export function keptEntry() {
  return JSON.parse(readFileSync(handoverFile(), "utf8"));
}
