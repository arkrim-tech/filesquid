/**
 * Unit tests for starting client-event delivery (UI-F2.16, UI-S-023): the page's console warnings
 * and errors reach `postClientEvents` as events of a level, one component and an error code with
 * a client event ID, in a page with no IndexedDB the events stay in memory and the page close
 * sends the overflow count, and stopping gives the console back.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ClientEventsAck, ClientEventsRequest } from "../../src/api/client";
import { startClientEvents } from "../../src/clientevents/start";

afterEach(() => vi.restoreAllMocks());

/** Let queued promise callbacks and zero-delay timers run. */
const settle = async () => {
  for (let i = 0; i < 6; i += 1) await new Promise<void>((resolve) => setTimeout(resolve, 0));
};

/** A console stand-in. */
function fakeConsole() {
  return { warn: vi.fn(), error: vi.fn() } as unknown as Console;
}

/** A client that acknowledges every event of a request. */
function fakeClient() {
  const requests: { request: ClientEventsRequest; keepalive?: boolean }[] = [];
  const postClientEvents = vi.fn(
    async (request: ClientEventsRequest, options?: { keepalive?: boolean }) => {
      requests.push({ request, keepalive: options?.keepalive });
      return {
        acknowledged: request.events.map((event) => event.id),
        journaled: request.events.length,
        principal: "urn:uuid:5a0c41e2-0000-4000-8000-00000000e19b",
      } satisfies ClientEventsAck;
    },
  );
  return { client: { postClientEvents }, requests };
}

describe("startClientEvents", () => {
  it("sends a console warning as an event with its level, the fixed component and a code, never its text", async () => {
    const { client, requests } = fakeClient();
    const page = fakeConsole();
    const running = await startClientEvents({
      win: window,
      client,
      host: "http://x",
      console: page,
    });
    page.warn("could not read /Users/me/IMG_4102.jpg");
    await settle();
    const event = requests[0]?.request.events[0];
    expect(event).toMatchObject({
      kind: "console",
      level: "warn",
      component: "console",
      error_code: "console_warn",
    });
    expect(event?.id).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    );
    expect(JSON.stringify(requests)).not.toContain("IMG_4102");
    running.stop();
  });

  it("keeps events in memory in a page with no IndexedDB, and sends the overflow count at page close", async () => {
    const { client, requests } = fakeClient();
    client.postClientEvents.mockImplementation(async () => {
      throw new Error("offline");
    });
    const page = fakeConsole();
    const running = await startClientEvents({
      win: window,
      client,
      host: "http://x",
      console: page,
    });
    page.error("a");
    page.error("b");
    window.dispatchEvent(new Event("pagehide"));
    await settle();
    const overflow = client.postClientEvents.mock.calls
      .map(([request, options]) => ({ request, options }))
      .find(({ request }) => request.events.some((event) => event.kind === "overflow"));
    const event = overflow?.request.events.find((e) => e.kind === "overflow");
    expect(event).toMatchObject({ kind: "overflow", count: 2 });
    expect(overflow?.options).toEqual({ keepalive: true });
    expect(requests).toHaveLength(0);
    running.stop();
  });

  it("gives the console back and stops listening when stopped", async () => {
    const { client } = fakeClient();
    const page = fakeConsole();
    const warn = page.warn;
    const running = await startClientEvents({
      win: window,
      client,
      host: "http://x",
      console: page,
    });
    expect(page.warn).not.toBe(warn);
    running.stop();
    expect(page.warn).toBe(warn);
    window.dispatchEvent(new Event("pagehide"));
    await settle();
    expect(client.postClientEvents).not.toHaveBeenCalled();
  });
});
