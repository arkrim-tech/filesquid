/**
 * Packaged-window proof of the desktop transport (Task DE-F1.1; DE-S-001, DE-S-005; ADR 030
 * S1 items (a) and (b)): in a real Tauri webview, the app page holds the per-launch token,
 * reads a 100 MB file through the `filesquid://` scheme in byte ranges, receives events on
 * the event channel, and cannot be navigated off the app's origins.
 *
 * Each `browser.execute` body runs in the app page, so it cannot use this module's scope; the
 * API base for the platform is passed in as an argument.
 */
import { browser, expect } from "@wdio/globals";

/** The 100 MB range fixture of the test-instrumented build (`src/test_fixture.rs`). */
const FIXTURE_PATH = "/__filesquid_test__/range-fixture";
const FIXTURE_LEN = 104857600;

/**
 * The scheme base URL the app page uses on this platform (ADR 030 Decision 3).
 *
 * @returns `http://filesquid.localhost` on Windows, `filesquid://localhost` elsewhere.
 */
function apiBase() {
  return process.platform === "win32" ? "http://filesquid.localhost" : "filesquid://localhost";
}

/**
 * The scheme and host of a page address; custom-scheme URLs have an opaque `origin`.
 *
 * @param address A page URL.
 * @returns `scheme://host`.
 */
function schemeAndHost(address) {
  const url = new URL(address);
  return `${url.protocol}//${url.host}`;
}

describe("filesquid-desktop transport in a packaged window", () => {
  before(async () => {
    await browser.waitUntil(
      async () => (await browser.execute(() => document.readyState)) === "complete",
      { timeout: 60000, timeoutMsg: "the app page did not load" },
    );
  });

  it("gives the app page a 64-hex per-launch token", async () => {
    const token = await browser.execute(() => window.__FILESQUID_LAUNCH_TOKEN__);
    expect(token).toMatch(/^[0-9a-f]{64}$/);
  });

  it("answers API calls with the token and refuses them without it", async () => {
    const result = await browser.execute(async (base) => {
      const token = window.__FILESQUID_LAUNCH_TOKEN__;
      const withToken = await fetch(`${base}/api/v1/health`, {
        headers: { Authorization: `Bearer ${token}` },
      });
      const without = await fetch(`${base}/api/v1/health`);
      return { ok: withToken.status, refused: without.status, body: await without.json() };
    }, apiBase());
    expect(result.ok).toBe(200);
    expect(result.refused).toBe(401);
    expect(result.body.reason).toBe("missing_credentials");
  });

  it("reads the last bytes of a 100 MB file as partial content", async () => {
    const result = await browser.execute(
      async (base, path) => {
        const token = window.__FILESQUID_LAUNCH_TOKEN__;
        const response = await fetch(`${base}${path}`, {
          headers: { Authorization: `Bearer ${token}`, Range: "bytes=104857500-104857599" },
        });
        const bytes = new Uint8Array(await response.arrayBuffer());
        return {
          status: response.status,
          range: response.headers.get("Content-Range"),
          length: bytes.length,
          exact: bytes.every((value, i) => value === (104857500 + i) % 251),
        };
      },
      apiBase(),
      FIXTURE_PATH,
    );
    expect(result).toEqual({
      status: 206,
      range: "bytes 104857500-104857599/104857600",
      length: 100,
      exact: true,
    });
  });

  it("reads the whole 100 MB file through capped ranges", async () => {
    const result = await browser.execute(
      async (base, path, length) => {
        const token = window.__FILESQUID_LAUNCH_TOKEN__;
        let next = 0;
        let reads = 0;
        let exact = true;
        while (next < length && reads < 20) {
          const response = await fetch(`${base}${path}`, {
            headers: { Authorization: `Bearer ${token}`, Range: `bytes=${next}-` },
          });
          if (response.status !== 206) return { failedStatus: response.status, reads };
          const bytes = new Uint8Array(await response.arrayBuffer());
          const range = `bytes ${next}-${next + bytes.length - 1}/${length}`;
          exact = exact && response.headers.get("Content-Range") === range;
          for (let i = 0; i < bytes.length && exact; i += 1) {
            exact = bytes[i] === (next + i) % 251;
          }
          next += bytes.length;
          reads += 1;
        }
        return { total: next, reads, exact };
      },
      apiBase(),
      FIXTURE_PATH,
      FIXTURE_LEN,
    );
    expect(result).toEqual({ total: FIXTURE_LEN, reads: 13, exact: true });
  });

  it("pushes events to the app window on the event channel", async () => {
    const received = await browser.execute(async (base) => {
      const internals = window.__TAURI_INTERNALS__;
      const events = [];
      const handler = internals.transformCallback((event) => events.push(event.payload));
      await internals.invoke("plugin:event|listen", {
        event: "filesquid-event",
        target: { kind: "Any" },
        handler,
      });
      // A refused call is journaled, and every journal entry is published as `journal.entry`.
      await fetch(`${base}/api/v1/health`);
      const isDenial = (e) => e.topic === "journal.entry" && e.data.operation === "api.auth.denied";
      for (let waited = 0; waited < 100 && !events.some(isDenial); waited += 1) {
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
      return events.filter(isDenial).map((e) => e.data.operation);
    }, apiBase());
    expect(received).toContain("api.auth.denied");
  });

  it("keeps the app window on the app's own origin", async () => {
    const before = schemeAndHost(await browser.getUrl());
    await browser.execute(() => {
      window.location.href = "https://example.com/";
    });
    await browser.pause(2000);
    expect(schemeAndHost(await browser.getUrl())).toBe(before);
    const token = await browser.execute(() => typeof window.__FILESQUID_LAUNCH_TOKEN__);
    expect(token).toBe("string");
  });
});
