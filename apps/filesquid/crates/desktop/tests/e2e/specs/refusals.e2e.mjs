/**
 * Packaged-window proof that every refusal in the desktop shell is journaled (Task DE-0.9;
 * DE-S-002, DE-S-003; ADR 002 amendment of 2026-10-18). In a real Tauri webview the app page
 * makes calls the host refuses (through the host's `ipc` gate, through Tauri's `postMessage`
 * path with a forged key and with the real key, a new window and a foreign navigation), then
 * reads the refusals back from the activity journal through the API
 * (`GET /api/v1/admin/journal`), and checks that no entry holds the payload or the token.
 *
 * Each `browser.execute` body runs in the app page, so it cannot use this module's scope; the
 * values it needs are passed in as arguments.
 */
import { browser, expect } from "@wdio/globals";

/**
 * The API base the app page uses.
 *
 * @returns `http://filesquid.localhost` on Windows, `filesquid://localhost` elsewhere.
 */
function apiBase() {
  return process.platform === "win32" ? "http://filesquid.localhost" : "filesquid://localhost";
}

/** A payload path the journal must never hold. */
const PRIVATE_PATH = "/home/owner/Private/holiday.jpg";

/**
 * Read the journal entries with `operation` from the app page, newest first.
 *
 * @param operation The journal operation, for example `desktop.ipc.refused`.
 * @returns The entries, as the API returns them.
 */
async function journalEntries(operation) {
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
 * Wait until the journal holds an entry with `operation` that `matches` accepts. The journal
 * writer persists entries from a bounded queue, so a refusal appears shortly after it happens.
 *
 * @param operation The journal operation.
 * @param matches A predicate on one entry.
 * @returns The first matching entry.
 */
async function waitForEntry(operation, matches) {
  let found;
  await browser.waitUntil(
    async () => {
      found = (await journalEntries(operation)).find(matches);
      return Boolean(found);
    },
    { timeout: 15000, interval: 250, timeoutMsg: `no ${operation} entry was journaled` },
  );
  return found;
}

/**
 * Run `body` in the app page. The embedded driver reports a rejected invoke inside the page as
 * a `WebDriverError` carrying Tauri's message, so that error is returned as a refusal.
 *
 * @param body The function to run in the page; it returns `{ ok, error? }`.
 * @param args Its arguments.
 * @returns What `body` returned, or `{ ok: false, error }` when the driver reported a rejection.
 */
async function executeOrRefusal(body, ...args) {
  try {
    return await browser.execute(body, ...args);
  } catch (error) {
    return { ok: false, error: String(error?.message ?? error) };
  }
}

/**
 * Invoke one IPC command from the app page and report how it ended.
 *
 * @param command The Tauri IPC command.
 * @param args The command's arguments.
 * @returns `{ ok, value }` or `{ ok: false, error }`.
 */
async function invokeFromPage(command, args) {
  return executeOrRefusal(
    async (cmd, payload) => {
      try {
        const value = await window.__TAURI_INTERNALS__.invoke(cmd, payload);
        return { ok: true, value: JSON.stringify(value ?? null) };
      } catch (error) {
        return { ok: false, error: String(error) };
      }
    },
    command,
    args,
  );
}

/**
 * Assert that the whole journal holds neither `PRIVATE_PATH` nor the launch token.
 */
async function expectJournalRedacted() {
  const result = await browser.execute(async (base) => {
    const token = window.__FILESQUID_LAUNCH_TOKEN__;
    const response = await fetch(`${base}/api/v1/admin/journal?order=desc&limit=500`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    return { text: await response.text(), token };
  }, apiBase());
  expect(result.text).not.toContain(PRIVATE_PATH);
  expect(result.text).not.toContain("holiday");
  expect(result.text).not.toContain(result.token);
}

describe("filesquid-desktop journals every refusal in a packaged window", () => {
  before(async () => {
    await browser.waitUntil(
      async () => (await browser.execute(() => document.readyState)) === "complete",
      { timeout: 60000, timeoutMsg: "the app page did not load" },
    );
  });

  it("journals a command the host's IPC gate refuses, without its payload", async () => {
    const result = await invokeFromPage("plugin:fs|read_text_file", { path: PRIVATE_PATH });
    expect(result.ok).toBe(false);
    expect(result.error).toMatch(/not allowed/);
    const entry = await waitForEntry("desktop.ipc.refused", (e) =>
      e.target_ids?.includes("plugin:fs.read_text_file"),
    );
    expect(entry.error_code).toBe("command_not_allowed");
    expect(entry.outcome).toBe("failed");
    expect(entry.level).toBe("warn");
    expect(entry.actor).toBe("webview:main");
    await expectJournalRedacted();
  });

  it("still lets the app page listen for FileSquid events", async () => {
    const result = await browser.execute(async () => {
      const internals = window.__TAURI_INTERNALS__;
      const handler = internals.transformCallback(() => {});
      const eventId = await internals.invoke("plugin:event|listen", {
        event: "filesquid-event",
        target: { kind: "Any" },
        handler,
      });
      await internals.invoke("plugin:event|unlisten", { event: "filesquid-event", eventId });
      return typeof eventId;
    });
    expect(result).toBe("number");
  });

  it("journals a forged call on Tauri's postMessage path without either key", async () => {
    const sent = await browser.execute((path) => {
      if (!window.ipc || typeof window.ipc.postMessage !== "function") return false;
      window.ipc.postMessage(
        JSON.stringify({
          cmd: "plugin:fs|read_text_file",
          callback: 1,
          error: 2,
          payload: { path },
          __TAURI_INVOKE_KEY__: "forged-key-from-page",
        }),
      );
      return true;
    }, PRIVATE_PATH);
    expect(sent).toBe(true);
    const entry = await waitForEntry(
      "desktop.ipc.refused",
      (e) => e.error_code === "invalid_invoke_key",
    );
    expect(entry.target_ids ?? []).toEqual([]);
    await expectJournalRedacted();
  });

  it("refuses and journals a new window", async () => {
    const opened = await browser.execute(() => {
      const handle = window.open("https://example.com/", "_blank");
      return handle === null || handle === undefined;
    });
    expect(opened).toBe(true);
    const entry = await waitForEntry("desktop.window.refused", () => true);
    expect(entry.error_code).toBe("new_window_denied");
    const text = JSON.stringify(await journalEntries("desktop.window.refused"));
    expect(text).not.toContain("example.com");
  });

  it("refuses and journals a navigation off the app's origins", async () => {
    await browser.execute(() => {
      window.location.href = "https://example.com/elsewhere";
    });
    const entry = await waitForEntry("desktop.navigation.refused", () => true);
    expect(entry.error_code).toBe("foreign_origin");
    const origin = await browser.execute(() => window.location.origin);
    expect(origin).not.toContain("example.com");
  });

  it("journals Tauri's own refusal on the postMessage fallback, with the real key", async () => {
    // With `fetch` failing, Tauri's IPC script falls back to `postMessage` with its real key,
    // past the host's gate; Tauri's access control refuses the call and the host's observer
    // journals it from Tauri's own trace.
    const result = await executeOrRefusal(async (path) => {
      const original = window.fetch;
      window.fetch = () => Promise.reject(new TypeError("fetch blocked by the test"));
      try {
        await window.__TAURI_INTERNALS__.invoke("plugin:fs|read_text_file", { path });
        return { ok: true };
      } catch (error) {
        return { ok: false, error: String(error) };
      } finally {
        window.fetch = original;
      }
    }, PRIVATE_PATH);
    expect(result.ok).toBe(false);
    expect(result.error).toMatch(/not allowed/);
    const entry = await waitForEntry(
      "desktop.ipc.refused",
      (e) => e.error_code === "acl_denied" && e.target_ids?.includes("plugin:fs.read_text_file"),
    );
    expect(entry.outcome).toBe("failed");
    await expectJournalRedacted();
    // Tauri's IPC script stays on `postMessage` for this page from now on; reload it.
    await browser.refresh();
  });
});
