/**
 * Unit tests for the FileSquid API client: host detection, URLs and requests.
 */
import { describe, expect, it, vi } from "vitest";
import { ApiError, FileSquidClient, apiBaseUrl, eventsUrl } from "../../src/api/client";

describe("apiBaseUrl", () => {
  it("is same-origin in a browser served by filesquid-server", () => {
    expect(apiBaseUrl({ isTauri: false, platform: "linux" })).toBe("");
  });
  it("uses the filesquid:// scheme in the macOS and Linux desktop webview", () => {
    expect(apiBaseUrl({ isTauri: true, platform: "macos" })).toBe("filesquid://localhost");
    expect(apiBaseUrl({ isTauri: true, platform: "linux" })).toBe("filesquid://localhost");
  });
  it("uses http://filesquid.localhost in the Windows desktop webview", () => {
    expect(apiBaseUrl({ isTauri: true, platform: "windows" })).toBe("http://filesquid.localhost");
  });
  it("uses http://filesquid.localhost in the Android webview", () => {
    expect(apiBaseUrl({ isTauri: true, platform: "android" })).toBe("http://filesquid.localhost");
  });
});

describe("eventsUrl", () => {
  it("maps http to ws and https to wss on the events path", () => {
    expect(eventsUrl(new URL("http://127.0.0.1:4173/library"))).toBe(
      "ws://127.0.0.1:4173/api/v1/events",
    );
    expect(eventsUrl(new URL("https://files.example.org/"))).toBe(
      "wss://files.example.org/api/v1/events",
    );
  });
});

describe("FileSquidClient.health", () => {
  it("requests /api/v1/health under the base URL and returns the payload", async () => {
    const seen: string[] = [];
    const client = new FileSquidClient({
      baseUrl: "filesquid://localhost",
      fetch: async (input) => {
        seen.push(String(input));
        return new Response(
          JSON.stringify({ schema_version: 1, status: "ok", product: "filesquid" }),
          {
            status: 200,
            headers: { "content-type": "application/json" },
          },
        );
      },
    });
    const health = await client.health();
    expect(seen).toEqual(["filesquid://localhost/api/v1/health"]);
    expect(health.product).toBe("filesquid");
  });

  it("throws ApiError with the HTTP status when the API answers with an error", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const client = new FileSquidClient({
      baseUrl: "",
      fetch: async () => new Response("nope", { status: 503 }),
    });
    await expect(client.health()).rejects.toEqual(new ApiError(503, "/api/v1/health"));
    expect(warn).toHaveBeenCalledOnce();
    expect(String(warn.mock.calls[0]?.[0])).not.toContain("nope");
    warn.mockRestore();
  });
});

describe("FileSquidClient.subscribe over WebSocket", () => {
  it("delivers parsed events and closes the socket on unsubscribe", () => {
    const sockets: FakeSocket[] = [];
    class FakeSocket {
      onmessage: ((event: { data: string }) => void) | null = null;
      closed = false;
      constructor(public url: string) {
        sockets.push(this);
      }
      close() {
        this.closed = true;
      }
    }
    const client = new FileSquidClient({
      baseUrl: "",
      eventsUrl: "ws://127.0.0.1:4173/api/v1/events",
      WebSocket: FakeSocket as unknown as typeof WebSocket,
    });
    const received: string[] = [];
    const unsubscribe = client.subscribe((event) => received.push(event.topic));
    sockets[0]?.onmessage?.({ data: JSON.stringify({ topic: "job.progress", data: { done: 1 } }) });
    unsubscribe();
    expect(sockets[0]?.url).toBe("ws://127.0.0.1:4173/api/v1/events");
    expect(received).toEqual(["job.progress"]);
    expect(sockets[0]?.closed).toBe(true);
  });
});
