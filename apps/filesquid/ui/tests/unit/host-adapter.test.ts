/**
 * Unit tests for runtime adapter selection (UI-S-012, ADR 030 Decision 3): the desktop
 * custom-scheme adapter or the server HTTP adapter, each with its own event transport.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { detectHost, selectHostAdapter, type HostEnvironment } from "../../src/api/client";
import { DESKTOP_EVENT, createHostClient } from "../../src/api/host";

/** Host facts for a platform, inside or outside the desktop webview. */
const env = (isTauri: boolean, platform: HostEnvironment["platform"]): HostEnvironment => ({
  isTauri,
  platform,
});

describe("selectHostAdapter", () => {
  it.each(["windows", "android", "macos", "linux", "other"] as const)(
    "uses the same-origin HTTP adapter and WebSocket events outside the desktop (%s)",
    (platform) => {
      expect(selectHostAdapter(env(false, platform))).toEqual({
        kind: "server-http",
        baseUrl: "",
        events: "websocket",
      });
    },
  );

  it.each(["macos", "linux", "other"] as const)(
    "uses filesquid://localhost and Tauri events on the %s desktop",
    (platform) => {
      expect(selectHostAdapter(env(true, platform))).toEqual({
        kind: "desktop-scheme",
        baseUrl: "filesquid://localhost",
        events: "tauri",
      });
    },
  );

  it.each(["windows", "android"] as const)(
    "uses http://filesquid.localhost and Tauri events on the %s desktop",
    (platform) => {
      expect(selectHostAdapter(env(true, platform))).toEqual({
        kind: "desktop-scheme",
        baseUrl: "http://filesquid.localhost",
        events: "tauri",
      });
    },
  );
});

describe("detectHost on iOS", () => {
  it("selects filesquid://localhost inside the iOS desktop webview", () => {
    const ios = {
      __TAURI_INTERNALS__: {},
      navigator: {
        userAgent:
          "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Mobile/15E148",
      },
    };
    expect(selectHostAdapter(detectHost(ios as unknown as Window)).baseUrl).toBe(
      "filesquid://localhost",
    );
  });
});

describe("createHostClient event transport", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    delete (window as unknown as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__;
  });

  it("listens to the Tauri event filesquid-event on the desktop and opens no WebSocket", async () => {
    const invoke = vi.fn(async () => 1);
    const internals = { invoke, transformCallback: vi.fn(() => 7) };
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = internals;
    const unregisterListener = vi.fn();
    (window as unknown as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener,
    };
    const Socket = vi.fn();
    vi.stubGlobal("WebSocket", Socket);
    const desktop = {
      __TAURI_INTERNALS__: internals,
      navigator: { userAgent: "Mozilla/5.0 (Macintosh; Mac OS X)" },
    };
    const unsubscribe = createHostClient(desktop as unknown as Window).subscribe(() => undefined);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalled());
    expect(invoke).toHaveBeenCalledWith(
      "plugin:event|listen",
      expect.objectContaining({ event: DESKTOP_EVENT, handler: 7 }),
      undefined,
    );
    expect(Socket).not.toHaveBeenCalled();
    unsubscribe();
    await vi.waitFor(() =>
      expect(unregisterListener).toHaveBeenCalledWith(DESKTOP_EVENT, expect.anything()),
    );
  });

  it("opens the WebSocket event channel on the server host", () => {
    const opened: string[] = [];
    class FakeSocket {
      onmessage: unknown = null;
      constructor(url: string) {
        opened.push(url);
      }
      close() {}
    }
    vi.stubGlobal("WebSocket", FakeSocket);
    const server = { navigator: { userAgent: "Mozilla/5.0 (X11; Linux x86_64)" } };
    const unsubscribe = createHostClient(server as unknown as Window).subscribe(() => undefined);
    expect(opened).toEqual([`ws://${window.location.host}/api/v1/events`]);
    unsubscribe();
  });
});
