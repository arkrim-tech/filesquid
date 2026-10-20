/**
 * End-to-end: the BUILT FileSquid UI served by the real `filesquid-server`, calling the
 * real, deny-by-default FileSquid API (API-S-004). Strict DOM assertions; any console error
 * fails the test, except the browser's own network lines for a deliberately refused call.
 */
import { expect, test, type Page } from "@playwright/test";
import { ADMIN_TOKEN } from "./admin-token";
import { trackErrors } from "./support/strict-dom";

/**
 * The browser itself logs a refused fetch or WebSocket upgrade as a console error. Tests that
 * expect a 401 or 403 tolerate exactly those lines and nothing else.
 */
const REFUSED_CALL = /status of 40[13] \(|WebSocket connection to .* failed/;

/** Define the desktop host's per-launch token global before any page script runs. */
async function injectLaunchToken(page: Page, token: string): Promise<void> {
  await page.addInitScript((value) => {
    Object.defineProperty(window, "__FILESQUID_LAUNCH_TOKEN__", {
      value,
      writable: false,
      enumerable: false,
      configurable: false,
    });
  }, token);
}

/** Sign in on the Monitor's token form. */
async function signIn(page: Page, token: string): Promise<void> {
  const form = page.locator("fs-monitor form[data-form='token']");
  await form.locator("input").fill(token);
  await form.locator("button").click();
}

test("shell mounts with FileSquid brand, internal vocabulary and live API health", async ({
  page,
}) => {
  const errors = trackErrors(page);
  await injectLaunchToken(page, ADMIN_TOKEN);
  await page.goto("/");
  await expect(page).toHaveTitle("FileSquid");
  expect(await page.evaluate(() => customElements.get("fs-app-shell") !== undefined)).toBe(true);

  const shell = page.locator("fs-app-shell");
  await expect(shell.locator("h1")).toHaveText("FileSquid");
  await expect(shell.locator("header img")).toHaveAttribute("alt", "FileSquid logo");
  // The live navigation tree fills the navigation slot; this server has no navigation service
  // yet, so the tree shows the unavailable state (UI-F2.1b). The Monitor link follows it.
  await expect(shell.locator("fs-nav-tree fs-states")).toHaveAttribute("data-state", "unavailable");
  await expect(shell.locator("nav > ul.tools li")).toHaveText(["Monitor", "Settings"]);
  await expect(shell.locator('p.api[role="status"]')).toHaveText("API: ok (filesquid)");
  await expect(shell.locator('p.api[role="alert"]')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("design tokens switch between light and dark", async ({ page }) => {
  const background = () => page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/");
  expect(await background()).toBe("rgb(255, 255, 255)");
  await page.emulateMedia({ colorScheme: "dark" });
  expect(await background()).toBe("rgb(20, 20, 20)");
  // Wait until the theme store has handled the OS change, so its listener cannot run after
  // (and undo) the explicit data-theme set below.
  await expect(page.locator("fs-app-shell fs-theme-control")).toHaveAttribute(
    "data-resolved",
    "dark",
  );
  await page.evaluate(() => document.documentElement.setAttribute("data-theme", "light"));
  expect(await background()).toBe("rgb(255, 255, 255)");
});

test("client-side routes fall back to the distribution index", async ({ page }) => {
  const response = await page.goto("/library/some/deep/link");
  expect(response?.status()).toBe(200);
  await expect(page.locator("fs-app-shell h1")).toHaveText("FileSquid");
});

test("without a token the shell names the missing credentials from the label table", async ({
  page,
}) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await page.goto("/");
  const alert = page.locator('fs-app-shell p.api[role="alert"]');
  await expect(alert).toHaveText("API unavailable: Missing credentials");
  await expect(alert).toHaveAttribute("data-reason", "missing_credentials");
  await expect(page.locator('fs-app-shell p.api[role="status"]')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("Monitor asks for the admin token, then shows live health, processes and the journal", async ({
  page,
}) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await page.goto("/#/monitor");
  const monitor = page.locator("fs-app-shell fs-monitor");
  const form = monitor.locator("form[data-form='token']");
  await expect(form.locator("label")).toHaveText("Admin token");
  await expect(form.locator("input")).toHaveAttribute("type", "password");
  const note = monitor.locator('[data-reason="missing_credentials"]');
  await expect(note).toHaveText("Missing credentials");
  await expect(note).toHaveAttribute("role", "status");
  await signIn(page, ADMIN_TOKEN);

  await expect(monitor.locator('[data-section="health"] h2')).toHaveText("Health");
  await expect(monitor.locator('[data-section="health"]')).toHaveAttribute(
    "data-status",
    /^(ok|degraded|failing)$/,
  );
  const host = monitor.locator('tr[data-process-id^="host-"]');
  await expect(host.locator("td")).toHaveText(["host", "running", "0", "–", "–"]);
  await expect(monitor.locator('[data-section="storage"] dd').first()).toHaveText(/ GiB$/);
  // The data dir persists between runs, so earlier host starts may be listed too.
  await expect(
    monitor
      .locator('[data-section="journal"] tbody tr td:nth-child(2)', { hasText: "host.started" })
      .first(),
  ).toBeVisible();
  // Signing in also reloads the shell's API status with the same in-memory token.
  await expect(page.locator('fs-app-shell p.api[role="status"]')).toHaveText("API: ok (filesquid)");
  expect(errors).toEqual([]);
});

test("a wrong token is named as invalid credentials", async ({ page }) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await page.goto("/#/monitor");
  await signIn(page, "f".repeat(64));
  const note = page.locator('fs-monitor [data-reason="invalid_credentials"]');
  await expect(note).toHaveText("Invalid credentials");
  await expect(note).toHaveAttribute("role", "alert");
  await expect(page.locator("fs-monitor form[data-form='token']")).toBeVisible();
  expect(errors).toEqual([]);
});

test("the signed-in token stays in memory: not in storage, URL or DOM, gone on reload", async ({
  page,
}) => {
  const errors = trackErrors(page, REFUSED_CALL);
  await page.goto("/");
  await page.locator('fs-app-shell nav a[href="#/monitor"]').click();
  await expect(page).toHaveURL(/#\/monitor$/);
  await signIn(page, ADMIN_TOKEN);
  await expect(page.locator('fs-monitor [data-section="processes"] h2')).toHaveText("Processes");
  await expect(page.locator("fs-monitor form[data-form='token']")).toHaveCount(0);
  const leaks = await page.evaluate((token) => {
    const shadows = [...document.querySelectorAll("fs-app-shell, fs-monitor")]
      .map((el) => el.shadowRoot?.innerHTML ?? "")
      .join("");
    const stored = JSON.stringify({ ...localStorage, ...sessionStorage });
    return [stored, location.href, document.documentElement.outerHTML, shadows].filter((text) =>
      text.includes(token),
    ).length;
  }, ADMIN_TOKEN);
  expect(leaks).toBe(0);

  await page.evaluate(() => (location.hash = ""));
  await expect(page.locator("fs-monitor")).toHaveCount(0);
  await page.evaluate(() => (location.hash = "#/monitor"));
  await expect(page.locator('fs-monitor [data-section="processes"] h2')).toHaveText("Processes");

  await page.reload();
  await expect(page.locator("fs-monitor form[data-form='token']")).toBeVisible();
  expect(errors).toEqual([]);
});

test("the desktop launch token authorises the shell, the Monitor and the event channel", async ({
  page,
}) => {
  const errors = trackErrors(page);
  const upgrades: string[] = [];
  page.on("websocket", (socket) => upgrades.push(socket.url()));
  await injectLaunchToken(page, ADMIN_TOKEN);
  await page.goto("/#/monitor");
  await expect(page.locator('fs-app-shell p.api[role="status"]')).toHaveText("API: ok (filesquid)");
  await expect(page.locator('fs-monitor [data-section="health"] h2')).toHaveText("Health");
  await expect(page.locator("fs-monitor form[data-form='token']")).toHaveCount(0);
  // One channel each for the host indicator, the navigation tree and the Monitor.
  const events = "ws://127.0.0.1:4173/api/v1/events";
  await expect.poll(() => upgrades).toEqual([events, events, events]);
  expect(errors).toEqual([]);
});

/** A Windows desktop webview's user agent: the custom scheme is then `http://filesquid.localhost`. */
const WINDOWS_UA =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0";
/** Origin of the built UI under test. */
const UI_ORIGIN = "http://127.0.0.1:4173";

/**
 * Stand in for the Tauri webview before any page script runs: `__TAURI_INTERNALS__` records
 * every IPC call and keeps event callbacks so the test can emit a desktop event.
 */
async function emulateDesktopWebview(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const record = window as unknown as Record<string, unknown>;
    const calls: { cmd: string; args: Record<string, unknown> }[] = [];
    const callbacks = new Map<number, (payload: unknown) => void>();
    let next = 1;
    record.__fsIpcCalls = calls;
    record.__fsEmit = (handler: number, payload: unknown) => callbacks.get(handler)?.(payload);
    record.__TAURI_INTERNALS__ = {
      transformCallback(callback: (payload: unknown) => void) {
        const id = next++;
        callbacks.set(id, callback);
        return id;
      },
      invoke(cmd: string, args: Record<string, unknown> = {}) {
        calls.push({ cmd, args });
        return Promise.resolve(cmd === "plugin:event|listen" ? next++ : null);
      },
    };
    record.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  });
}

/**
 * Serve the Windows custom-scheme origin from the real `filesquid-server`, with the CORS
 * answers `filesquid-desktop` gives its own pages. Records each forwarded request.
 */
async function bridgeCustomScheme(page: Page): Promise<{ path: string; auth?: string }[]> {
  const forwarded: { path: string; auth?: string }[] = [];
  const cors = {
    "access-control-allow-origin": UI_ORIGIN,
    "access-control-allow-headers": "authorization, content-type",
    "access-control-allow-methods": "GET, HEAD, POST, PUT, PATCH, DELETE",
  };
  await page.route("http://filesquid.localhost/**", async (route) => {
    const request = route.request();
    if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers: cors });
    const url = new URL(request.url());
    forwarded.push({ path: url.pathname, auth: request.headers()["authorization"] });
    const response = await route.fetch({ url: `${UI_ORIGIN}${url.pathname}${url.search}` });
    return route.fulfill({ response, headers: { ...response.headers(), ...cors } });
  });
  return forwarded;
}

/** Emit one `filesquid-event` to the page's Tauri listener, as the desktop host would. */
async function emitDesktopEvent(page: Page, payload: unknown): Promise<void> {
  await page.evaluate((data) => {
    const win = window as unknown as {
      __fsIpcCalls: { cmd: string; args: { handler?: number } }[];
      __fsEmit: (handler: number, payload: unknown) => void;
    };
    // Every listener receives the event, as Tauri delivers it to each registered handler.
    for (const call of win.__fsIpcCalls) {
      if (call.cmd !== "plugin:event|listen") continue;
      win.__fsEmit(call.args.handler ?? -1, { event: "filesquid-event", id: 1, payload: data });
    }
  }, payload);
}

test.describe("inside the desktop webview (custom URI scheme adapter)", () => {
  test.use({ userAgent: WINDOWS_UA });

  test("requests use the custom scheme and events use Tauri, not same-origin HTTP or WebSocket", async ({
    page,
  }) => {
    const errors = trackErrors(page);
    const sameOrigin: string[] = [];
    const upgrades: string[] = [];
    page.on("request", (request) => {
      if (request.url().startsWith(`${UI_ORIGIN}/api/`)) sameOrigin.push(request.url());
    });
    page.on("websocket", (socket) => upgrades.push(socket.url()));
    await emulateDesktopWebview(page);
    await injectLaunchToken(page, ADMIN_TOKEN);
    const forwarded = await bridgeCustomScheme(page);

    await page.goto("/#/monitor");
    const shell = page.locator("fs-app-shell");
    await expect(shell.locator('p.api[role="status"]')).toHaveText("API: ok (filesquid)");
    const monitor = shell.locator("fs-monitor");
    await expect(monitor.locator('[data-section="health"] h2')).toHaveText("Health");
    await expect(monitor.locator("form[data-form='token']")).toHaveCount(0);

    const paths = forwarded.map((call) => call.path);
    expect(paths).toContain("/api/v1/health");
    expect(paths).toContain("/api/v1/admin/health");
    expect(forwarded.every((call) => call.auth === `Bearer ${ADMIN_TOKEN}`)).toBe(true);
    const listened = await page.evaluate(() =>
      (
        window as unknown as { __fsIpcCalls: { cmd: string; args: { event?: string } }[] }
      ).__fsIpcCalls
        .filter((call) => call.cmd === "plugin:event|listen")
        .map((call) => call.args.event),
    );
    // One listener each for the host indicator, the navigation tree and the Monitor.
    expect(listened).toEqual(["filesquid-event", "filesquid-event", "filesquid-event"]);
    expect(paths).toContain("/api/v1/host");

    await emitDesktopEvent(page, {
      topic: "journal.entry",
      data: {
        id: Number.MAX_SAFE_INTEGER,
        time: "2026-10-18T00:00:00Z",
        host: "desktop",
        actor: "owner",
        operation: "ui.e2e.desktop_event",
        outcome: "succeeded",
        level: "info",
        target_ids: [],
      },
    });
    const newest = monitor.locator('[data-section="journal"] tbody tr').first();
    await expect(newest.locator("td").nth(1)).toHaveText("ui.e2e.desktop_event");

    expect(sameOrigin).toEqual([]);
    expect(upgrades).toEqual([]);
    expect(errors).toEqual([]);
  });
});
