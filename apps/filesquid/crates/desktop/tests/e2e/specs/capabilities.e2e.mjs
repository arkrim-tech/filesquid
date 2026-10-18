/**
 * Packaged-window proof of the deny-by-default capabilities (Task DE-0.1; DE-S-001, DE-S-002,
 * DE-S-006; ADR 002 minimum IPC policy): in a real Tauri webview, the app page may listen for
 * and stop listening for FileSquid events, and every other IPC command is refused by Tauri's
 * access control: core commands the capability does not grant (event emit, path, window,
 * webview, app), file, SQL, program, dialog and HTTP plugins, and any custom app command.
 *
 * Each `browser.execute` body runs in the app page, so it cannot use this module's scope; the
 * command and its arguments are passed in as arguments.
 */
import { browser, expect } from "@wdio/globals";

/**
 * Invoke one IPC command from the app page and report how it ended. The embedded driver
 * reports a rejected invoke as a `WebDriverError` carrying Tauri's message, so a refusal is
 * caught here as well as in the page.
 *
 * @param command The Tauri IPC command, for example `plugin:event|emit`.
 * @param args The command's arguments.
 * @returns `{ ok: true, value }` when Tauri ran the command, `{ ok: false, error }` when it
 *   refused it.
 */
async function invokeFromPage(command, args) {
  try {
    return await browser.execute(
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
  } catch (error) {
    return { ok: false, error: String(error?.message ?? error) };
  }
}

/** Commands the capability does not grant, with harmless arguments. */
const DENIED = [
  ["plugin:event|emit", { event: "filesquid-event", payload: {} }],
  ["plugin:path|resolve_directory", { directory: 14 }],
  ["plugin:window|title", { label: "main" }],
  ["plugin:webview|get_all_webviews", {}],
  ["plugin:app|version", {}],
  ["plugin:fs|read_text_file", { path: "/etc/hosts" }],
  ["plugin:sql|load", { db: "sqlite:filesquid.db" }],
  ["plugin:shell|execute", { program: "sh", args: ["-c", "true"] }],
  ["plugin:dialog|open", { options: {} }],
  ["plugin:http|fetch", { clientConfig: { url: "https://example.com/", method: "GET" } }],
  ["health_ping", {}],
];

describe("filesquid-desktop capabilities in a packaged window", () => {
  before(async () => {
    await browser.waitUntil(
      async () => (await browser.execute(() => document.readyState)) === "complete",
      { timeout: 60000, timeoutMsg: "the app page did not load" },
    );
  });

  it("lets the app page listen for and stop listening for FileSquid events", async () => {
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

  for (const [command, args] of DENIED) {
    it(`refuses ${command}`, async () => {
      const result = await invokeFromPage(command, args);
      expect(result.ok).toBe(false);
      expect(result.error).toMatch(/not allowed/);
    });
  }

  it("never hands the app page a filesystem path", async () => {
    const result = await invokeFromPage("plugin:path|resolve_directory", { directory: 14 });
    expect(result.ok).toBe(false);
    expect(result.error).not.toMatch(/filesquid-desktop-e2e-|\/home\/|\/Users\/|[A-Z]:\\/);
  });
});
