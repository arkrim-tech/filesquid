/**
 * Unit tests for the host descriptor read and the event channel's connect signal
 * (UI-S-018): `FileSquidClient.host()` and `subscribe()`'s `onOpen`.
 */
import { describe, expect, it } from "vitest";
import { ApiError, FileSquidClient } from "../../src/api/client";

const BODY = {
  schema_version: 1,
  host_type: "server",
  operator_name: "Garderen Family",
  hosted_service: {
    enrolled: false,
    backup_enabled: false,
    sync_enabled: false,
    service_host: null,
  },
};

describe("FileSquidClient.host", () => {
  it("reads GET /api/v1/host with the bearer token and returns the descriptor", async () => {
    const seen: { url: string; method: string; auth: string | null }[] = [];
    const client = new FileSquidClient({
      baseUrl: "",
      token: "abc",
      fetch: async (input, init) => {
        seen.push({
          url: String(input),
          method: init?.method ?? "GET",
          auth: new Headers(init?.headers).get("Authorization"),
        });
        return new Response(JSON.stringify(BODY), {
          headers: { "content-type": "application/json" },
        });
      },
    });
    expect(await client.host()).toEqual(BODY);
    expect(seen).toEqual([{ url: "/api/v1/host", method: "GET", auth: "Bearer abc" }]);
  });

  it("rejects with ApiError when the host has not described itself yet", async () => {
    const client = new FileSquidClient({
      baseUrl: "",
      fetch: async () =>
        new Response(JSON.stringify({ code: "unavailable", message: "x", retryable: true }), {
          status: 503,
          headers: { "content-type": "application/json" },
        }),
    });
    await expect(client.host()).rejects.toBeInstanceOf(ApiError);
  });
});

describe("FileSquidClient.subscribe onOpen", () => {
  /** A socket fake whose `onopen` the test fires like the browser's open event. */
  class FakeSocket {
    static all: FakeSocket[] = [];
    onmessage: ((event: { data: string }) => void) | null = null;
    onopen: (() => void) | null = null;
    constructor() {
      FakeSocket.all.push(this);
    }
    close() {}
  }

  /** A client whose event channel is the fake socket. */
  function clientWithFake(): FileSquidClient {
    FakeSocket.all = [];
    return new FileSquidClient({
      baseUrl: "",
      eventsUrl: "ws://x/api/v1/events",
      WebSocket: FakeSocket as unknown as typeof WebSocket,
    });
  }

  it("calls onOpen each time the channel opens", () => {
    let opened = 0;
    clientWithFake().subscribe(
      () => undefined,
      () => void opened++,
    );
    FakeSocket.all[0]?.onopen?.();
    FakeSocket.all[0]?.onopen?.();
    expect(opened).toBe(2);
  });

  it("works without onOpen", () => {
    clientWithFake().subscribe(() => undefined);
    expect(FakeSocket.all[0]?.onopen).toBeNull();
  });
});
