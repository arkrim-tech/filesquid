/**
 * Unit tests for the client-event queue (UI-F2.16, UI-S-023, API-S-030, ADR 033 Decision 7):
 * batches of at most 20 events and 16 KiB, at most 60 requests a minute, a sent batch kept until
 * acknowledged, backoff from 1 s doubling to 60 s and never before `Retry-After`, a 400 batch
 * split into single events with a refused one replaced by `client_event_refused`, the 10,000
 * in-memory cap with spill into the store, write-through, persistence at page close, delivery of
 * persisted events oldest first under the credential of their host and principal, and the
 * one-entry overflow count of events that could not be kept. Fake timers, a scripted transport
 * and an in-memory store stand in for the page.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  BACKOFF_MAX_MS,
  BATCH_BYTES,
  BATCH_EVENTS,
  MEMORY_CAP,
  OVERFLOW_MAX,
  REQUESTS_PER_MINUTE,
  ClientEventQueue,
  overflowCount,
  type QueueOptions,
} from "../../src/clientevents/queue";
import { MemoryEventStore, ScriptedTransport } from "../support/client-event-fakes";

const UUID_V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

/** Let timers due within `ms` run, with the promises they start. */
async function settle(ms = 50): Promise<void> {
  await vi.advanceTimersByTimeAsync(ms);
}

interface Rig {
  readonly queue: ClientEventQueue;
  readonly transport: ScriptedTransport;
  readonly store: MemoryEventStore;
  readonly log: ReturnType<typeof vi.fn>;
}

/** A queue over a scripted transport and an in-memory store. */
function rig(overrides: Partial<QueueOptions> = {}, parts: Partial<Rig> = {}): Rig {
  const transport = parts.transport ?? new ScriptedTransport();
  const store = parts.store ?? new MemoryEventStore();
  const log = vi.fn();
  const queue = new ClientEventQueue({
    transport: transport.send,
    store,
    host: "desktop",
    log,
    ...overrides,
  });
  return { queue, transport, store, log };
}

/** A console warning with a distinct error code. */
function warn(queue: ClientEventQueue, n: number, component = "fs-test"): string {
  return queue.record({ level: "warn", component, error_code: `code_${n}` });
}

beforeEach(() => {
  vi.useFakeTimers({ now: new Date("2026-10-19T12:00:00.000Z") });
});
afterEach(() => {
  vi.useRealTimers();
});

describe("the thresholds of UI-S-023", () => {
  it("are 10,000 events in memory, 20 events and 16 KiB a batch, 60 requests a minute, 60 s backoff", () => {
    expect(MEMORY_CAP).toBe(10_000);
    expect(BATCH_EVENTS).toBe(20);
    expect(BATCH_BYTES).toBe(16 * 1024);
    expect(REQUESTS_PER_MINUTE).toBe(60);
    expect(BACKOFF_MAX_MS).toBe(60_000);
    expect(OVERFLOW_MAX).toBe(1_000_000);
  });

  it("clamps the overflow count to 1 to 1,000,000 and sends none for 0", () => {
    expect(overflowCount(0)).toBeUndefined();
    expect(overflowCount(-3)).toBeUndefined();
    expect(overflowCount(1)).toBe(1);
    expect(overflowCount(1_000_000)).toBe(1_000_000);
    expect(overflowCount(5_000_000)).toBe(1_000_000);
    expect(overflowCount(2.9)).toBe(2);
  });
});

describe("what an event carries", () => {
  it("sends the level, component, error code, opaque ids and time under its own v4 id", async () => {
    const { queue, transport } = rig();
    const id = queue.record({
      level: "error",
      component: "fs-monitor",
      error_code: "load_failed",
      ids: ["urn:uuid:aaaa"],
    });
    await settle();
    const sent = transport.requests[0]?.request.events[0];
    expect(sent).toMatchObject({
      kind: "console",
      id,
      level: "error",
      component: "fs-monitor",
      error_code: "load_failed",
      ids: ["urn:uuid:aaaa"],
      time: "2026-10-19T12:00:00.000Z",
    });
    expect(id).toMatch(UUID_V4);
    expect(Object.keys(sent ?? {}).sort()).toEqual(
      ["component", "error_code", "id", "ids", "kind", "level", "time"].sort(),
    );
  });

  it("keeps the id of an event for life: a resend after a lost acknowledgement carries it", async () => {
    const transport = new ScriptedTransport();
    transport.script = [{ kind: "lost-ack" }, { kind: "ack" }];
    const { queue } = rig({}, { transport });
    const id = warn(queue, 1);
    await settle(5_000);
    expect(transport.requests).toHaveLength(2);
    expect(transport.requests.map((r) => r.request.events[0]?.id)).toEqual([id, id]);
    expect(transport.journaled).toEqual([id]);
  });

  it("learns the principal from the acknowledgement and says so in its stats", async () => {
    const { queue, transport } = rig();
    transport.principal = "urn:uuid:ana";
    warn(queue, 1);
    await settle();
    expect(queue.stats().principal).toBe("urn:uuid:ana");
  });
});

describe("batches", () => {
  it("send at most 20 events a request, oldest first, each event once", async () => {
    const { queue, transport } = rig();
    const ids = Array.from({ length: 45 }, (_, i) => warn(queue, i));
    await settle(1_000);
    expect(transport.requests.map((r) => r.request.events.length)).toEqual([20, 20, 5]);
    expect(transport.journaled).toEqual(ids);
  });

  it("stay within 16 KiB: large events make smaller batches", async () => {
    const { queue, transport } = rig();
    const ids = Array.from({ length: 8 }, (_, i) => `urn:uuid:${String(i).repeat(100)}`);
    for (let i = 0; i < 40; i += 1) {
      queue.record({ level: "warn", component: "fs-test", error_code: `c${i}`, ids });
    }
    await settle(2_000);
    const sizes = transport.requests.map(
      (r) => new TextEncoder().encode(JSON.stringify(r.request)).length,
    );
    expect(Math.max(...sizes)).toBeLessThanOrEqual(BATCH_BYTES);
    expect(transport.requests.length).toBeGreaterThan(2);
    expect(transport.journaled).toHaveLength(40);
  });

  it("keep a sent batch queued until the route acknowledges it", async () => {
    const { queue, transport } = rig();
    transport.hold = true;
    warn(queue, 1);
    warn(queue, 2);
    await settle();
    expect(transport.requests).toHaveLength(1);
    expect(queue.stats().memory).toBe(2);
    transport.release();
    await settle();
    expect(queue.stats().memory).toBe(0);
  });

  it("make at most 60 requests in any rolling minute and only delay the rest", async () => {
    const { queue, transport } = rig();
    const ids = Array.from({ length: 1_500 }, (_, i) => warn(queue, i));
    await settle(5_000);
    expect(transport.requests).toHaveLength(60);
    await settle(60_000);
    expect(transport.requests).toHaveLength(75);
    const [first, sixtyFirst] = [transport.requests[0], transport.requests[60]];
    expect((sixtyFirst?.at ?? 0) - (first?.at ?? 0)).toBeGreaterThanOrEqual(60_000);
    expect(transport.journaled).toEqual(ids);
    expect(
      transport.requests.every((r) => r.request.events.every((e) => e.kind === "console")),
    ).toBe(true);
  });
});

describe("retries", () => {
  /** Gaps between the first n request times. */
  function gaps(transport: ScriptedTransport, n: number): number[] {
    return transport.requests
      .slice(0, n)
      .map((r, i, all) => (i === 0 ? 0 : r.at - (all[i - 1]?.at ?? 0)));
  }

  it("backs off from 1 s, doubling, to at most 60 s, on a 5xx", async () => {
    const transport = new ScriptedTransport();
    transport.script = [
      ...Array.from({ length: 8 }, () => ({ kind: "status" as const, status: 503 })),
      { kind: "ack" },
    ];
    const { queue } = rig({}, { transport });
    warn(queue, 1);
    await settle(400_000);
    expect(gaps(transport, 9).slice(1)).toEqual([
      1_000, 2_000, 4_000, 8_000, 16_000, 32_000, 60_000, 60_000,
    ]);
    expect(queue.stats().memory).toBe(0);
  });

  it("starts again from 1 s after a success", async () => {
    const transport = new ScriptedTransport();
    transport.script = [
      { kind: "status", status: 500 },
      { kind: "status", status: 500 },
      { kind: "ack" },
      { kind: "status", status: 500 },
      { kind: "ack" },
    ];
    const { queue } = rig({}, { transport });
    warn(queue, 1);
    await settle(10_000);
    warn(queue, 2);
    await settle(10_000);
    expect(gaps(transport, 3).slice(1)).toEqual([1_000, 2_000]);
    expect(transport.requests[4]?.at).toBe((transport.requests[3]?.at ?? 0) + 1_000);
  });

  it("retries after 401, 403 and a network failure, for as long as the page is open", async () => {
    const transport = new ScriptedTransport();
    transport.script = [
      { kind: "status", status: 401 },
      { kind: "status", status: 403 },
      { kind: "network" },
      { kind: "ack" },
    ];
    const { queue } = rig({}, { transport });
    const id = warn(queue, 1);
    await settle(20_000);
    expect(transport.requests).toHaveLength(4);
    expect(transport.journaled).toEqual([id]);
  });

  it("waits for Retry-After after a 429, even past the backoff cap, never less", async () => {
    const transport = new ScriptedTransport();
    transport.script = [{ kind: "status", status: 429, retryAfterSeconds: 90 }, { kind: "ack" }];
    const { queue } = rig({}, { transport });
    warn(queue, 1);
    await settle(89_000);
    expect(transport.requests).toHaveLength(1);
    await settle(1_500);
    expect(transport.requests).toHaveLength(2);
    expect(
      (transport.requests[1]?.at ?? 0) - (transport.requests[0]?.at ?? 0),
    ).toBeGreaterThanOrEqual(90_000);
  });

  it("delivers 150 queued warnings, one entry each, after a 429 with Retry-After, with no overflow", async () => {
    const transport = new ScriptedTransport();
    transport.script = [
      { kind: "status", status: 429, retryAfterSeconds: 3 },
      { kind: "status", status: 429, retryAfterSeconds: 3 },
      { kind: "ack" },
    ];
    const { queue } = rig({}, { transport });
    const ids = Array.from({ length: 150 }, (_, i) => warn(queue, i));
    await settle(60_000);
    expect(transport.journaled).toEqual(ids);
    expect(transport.journaledEvents().filter((e) => e.kind === "overflow")).toEqual([]);
  });
});

describe("a refused batch", () => {
  it("is not retried as it is: each event is resent alone, and the refused one is replaced", async () => {
    const transport = new ScriptedTransport();
    transport.refuse = (event) =>
      "component" in event &&
      event.component === "bad" &&
      event.error_code !== "client_event_refused";
    const { queue } = rig({}, { transport });
    const a = warn(queue, 1);
    const b = warn(queue, 2, "bad");
    const c = warn(queue, 3);
    await settle(10_000);
    expect(transport.requests[0]?.request.events.map((e) => e.id)).toEqual([a, b, c]);
    expect(transport.requests.slice(1, 4).map((r) => r.request.events.length)).toEqual([1, 1, 1]);
    const journaled = transport.journaledEvents();
    expect(journaled.map((e) => e.id).slice(0, 2)).toEqual([a, c]);
    expect(journaled).toHaveLength(3);
    expect(journaled[2]).toMatchObject({
      kind: "console",
      component: "bad",
      error_code: "client_event_refused",
    });
    expect(journaled.map((e) => e.id)).not.toContain(b);
    expect(queue.stats().memory).toBe(0);
  });

  it("does not loop on a replacement the route refuses too: it is dropped and logged", async () => {
    const transport = new ScriptedTransport();
    transport.refuse = (event) => "component" in event && event.component === "bad";
    const { queue, log } = rig({}, { transport });
    warn(queue, 1, "bad");
    await settle(60_000);
    expect(transport.requests.length).toBeLessThanOrEqual(3);
    expect(transport.journaled).toEqual([]);
    expect(queue.stats().memory).toBe(0);
    expect(log).toHaveBeenCalled();
  });
});

describe("the in-memory cap and the store", () => {
  it("writes every queued event to the store as it is queued and removes it when acknowledged", async () => {
    const { queue, store, transport } = rig();
    transport.hold = true;
    const id = warn(queue, 1);
    await settle();
    expect(store.all().map((e) => e.id)).toEqual([id]);
    expect(store.all()[0]?.host).toBe("desktop");
    transport.release();
    await settle();
    expect(store.all()).toEqual([]);
  });

  it("spills events past the cap into the store in order and returns them oldest first as it drains, losing none", async () => {
    const transport = new ScriptedTransport();
    transport.script = [{ kind: "status", status: 503 }, { kind: "ack" }];
    const { queue, store } = rig({ memoryCap: 3, batchEvents: 2 }, { transport });
    const ids = Array.from({ length: 8 }, (_, i) => warn(queue, i));
    await settle();
    expect(queue.stats()).toMatchObject({ memory: 3, spilled: 5, lost: 0 });
    expect(store.all().map((e) => e.id)).toEqual(ids);
    await settle(20_000);
    expect(transport.journaled).toEqual(ids);
    expect(store.all()).toEqual([]);
    expect(transport.journaledEvents().some((e) => e.kind === "overflow")).toBe(false);
  });

  it("holds 10,000 events in memory and stores the 5 that follow, then delivers all 10,005", async () => {
    const transport = new ScriptedTransport();
    transport.script = [{ kind: "network" }];
    const { queue, store } = rig({}, { transport });
    const ids = Array.from({ length: MEMORY_CAP + 5 }, (_, i) => warn(queue, i));
    await settle(1_000);
    expect(queue.stats()).toMatchObject({ memory: MEMORY_CAP, spilled: 5, lost: 0 });
    expect(store.all()).toHaveLength(MEMORY_CAP + 5);
    transport.script = [{ kind: "ack" }];
    await settle(600_000);
    expect(transport.journaled).toEqual(ids);
    expect(queue.stats()).toMatchObject({ memory: 0, spilled: 0, lost: 0 });
  });
});

describe("page close", () => {
  it("persists the events whose write-through failed, when storage accepts writes again", async () => {
    const transport = new ScriptedTransport();
    transport.script = [{ kind: "network" }];
    const { queue, store } = rig({}, { transport });
    store.refusing = true;
    const ids = [warn(queue, 1), warn(queue, 2)];
    await settle();
    expect(store.all()).toEqual([]);
    store.refusing = false;
    ids.push(warn(queue, 3));
    await settle();
    queue.pagehide();
    await settle();
    expect(store.all().map((e) => e.id)).toEqual(ids);
    expect(transport.requests.some((r) => r.keepalive)).toBe(false);
  });

  it("stores the principal with the events it persists", async () => {
    const transport = new ScriptedTransport();
    const { queue, store } = rig({}, { transport });
    transport.principal = "urn:uuid:ana";
    warn(queue, 1);
    await settle();
    transport.script = [{ kind: "network" }];
    warn(queue, 2);
    await settle();
    queue.pagehide();
    await settle();
    expect(store.all().map((e) => e.principal)).toEqual(["urn:uuid:ana"]);
  });

  it("counts the events it could not keep and sends one overflow entry as a keep-alive request", async () => {
    const transport = new ScriptedTransport();
    transport.script = [{ kind: "network" }];
    const { queue, store } = rig({ memoryCap: 3 }, { transport });
    store.refusing = true;
    for (let i = 0; i < 7; i += 1) warn(queue, i);
    await settle();
    expect(queue.stats().lost).toBe(4);
    transport.script = [{ kind: "ack" }];
    queue.pagehide();
    await settle();
    const overflow = transport.requests.filter((r) => r.keepalive);
    expect(overflow).toHaveLength(1);
    const event = overflow[0]?.request.events[0];
    expect(overflow[0]?.request.events).toHaveLength(1);
    expect(event).toMatchObject({ kind: "overflow", count: 7 });
    expect(event?.id).toMatch(UUID_V4);
    expect(Object.keys(event ?? {}).sort()).toEqual(["component", "count", "id", "kind"]);
  });

  it("counts the unsent events when IndexedDB is unavailable, and the delivered ones keep their entries", async () => {
    const transport = new ScriptedTransport();
    const { queue, store } = rig({}, { transport });
    store.available = false;
    const delivered = [warn(queue, 1), warn(queue, 2)];
    await settle();
    transport.script = [{ kind: "network" }];
    warn(queue, 3);
    warn(queue, 4);
    warn(queue, 5);
    await settle(100);
    transport.script = [{ kind: "ack" }];
    queue.pagehide();
    await settle();
    const overflow = transport.requests.filter((r) => r.keepalive);
    expect(overflow).toHaveLength(1);
    expect(overflow[0]?.request.events[0]).toMatchObject({ kind: "overflow", count: 3 });
    expect(
      transport
        .journaledEvents()
        .filter((e) => e.kind === "console")
        .map((e) => e.id)
        .slice(0, 2),
    ).toEqual(delivered);
  });

  it("sends no overflow entry when every event was kept", async () => {
    const transport = new ScriptedTransport();
    transport.script = [{ kind: "network" }];
    const { queue } = rig({}, { transport });
    warn(queue, 1);
    await settle();
    queue.pagehide();
    await settle();
    expect(transport.requests.some((r) => r.keepalive)).toBe(false);
  });
});

describe("the next open", () => {
  it("delivers an event queued while storage accepted writes after a page that ended without pagehide", async () => {
    const store = new MemoryEventStore();
    const first = new ScriptedTransport();
    first.script = [{ kind: "network" }];
    const page1 = rig({}, { store, transport: first });
    const queued = [warn(page1.queue, 1), warn(page1.queue, 2), warn(page1.queue, 3)];
    await settle();
    page1.queue.stop();
    expect(store.all().map((e) => e.id)).toEqual(queued);

    const second = new ScriptedTransport();
    const page2 = rig({}, { store, transport: second });
    await page2.queue.start();
    await settle(5_000);
    const events = second.journaledEvents();
    expect(events.filter((e) => queued.includes(e.id)).map((e) => e.id)).toEqual(queued);
    expect(events.filter((e) => e.id === queued[0])).toHaveLength(1);
    expect(store.all()).toEqual([]);
  });

  it("delivers persisted events oldest first before the events raised after the page opened", async () => {
    const store = new MemoryEventStore();
    const first = new ScriptedTransport();
    first.script = [{ kind: "network" }];
    const page1 = rig({}, { store, transport: first });
    const old = [warn(page1.queue, 1), warn(page1.queue, 2)];
    await settle();
    page1.queue.pagehide();
    await settle();

    const second = new ScriptedTransport();
    const page2 = rig({}, { store, transport: second });
    await page2.queue.start();
    const fresh = warn(page2.queue, 99);
    await settle(5_000);
    const order = second.journaledEvents().map((e) => e.id);
    expect(order.indexOf(old[0] as string)).toBeLessThan(order.indexOf(old[1] as string));
    expect(order.indexOf(old[1] as string)).toBeLessThan(order.indexOf(fresh));
  });

  it("asks nothing when nothing was left, and says so with one event when something was", async () => {
    const quiet = rig();
    await quiet.queue.start();
    await settle();
    expect(quiet.transport.requests).toHaveLength(0);

    const store = new MemoryEventStore();
    const first = new ScriptedTransport();
    first.script = [{ kind: "network" }];
    const page1 = rig({}, { store, transport: first });
    warn(page1.queue, 1);
    await settle();
    page1.queue.stop();
    const second = new ScriptedTransport();
    const page2 = rig({}, { store, transport: second });
    await page2.queue.start();
    await settle(5_000);
    const notes = second.journaledEvents().filter((e) => e.error_code === "persisted_events_found");
    expect(notes).toHaveLength(1);
    expect(notes[0]).toMatchObject({ kind: "console", component: "client-events" });
  });

  it("never sends the events of another host", async () => {
    const store = new MemoryEventStore();
    const other = rig({ host: "https://other.example" }, { store });
    other.transport.script = [{ kind: "network" }];
    warn(other.queue, 1);
    await settle();
    other.queue.stop();
    const here = rig({ host: "desktop" }, { store });
    await here.queue.start();
    warn(here.queue, 2);
    await settle(5_000);
    expect(store.all("https://other.example")).toHaveLength(1);
    expect(here.transport.journaled).toHaveLength(1);
  });

  it("holds the events of another principal untouched until that principal signs in again", async () => {
    const store = new MemoryEventStore();
    const ana = rig({ host: "server" }, { store });
    ana.transport.principal = "urn:uuid:ana";
    warn(ana.queue, 0);
    await settle();
    ana.transport.script = [{ kind: "network" }];
    const left = [warn(ana.queue, 1), warn(ana.queue, 2), warn(ana.queue, 3)];
    await settle(200);
    ana.queue.pagehide();
    await settle();
    expect(store.all().map((e) => e.principal)).toEqual(Array(3).fill("urn:uuid:ana"));

    const ben = rig({ host: "server" }, { store });
    ben.transport.principal = "urn:uuid:ben";
    await ben.queue.start();
    await settle(10_000);
    const sentToBen = ben.transport.requests.flatMap((r) => r.request.events.map((e) => e.id));
    for (const id of left) expect(sentToBen).not.toContain(id);
    expect(store.all().map((e) => e.id)).toEqual(left);

    const again = rig({ host: "server" }, { store });
    again.transport.principal = "urn:uuid:ana";
    await again.queue.start();
    await settle(10_000);
    expect(
      again.transport
        .journaledEvents()
        .filter((e) => left.includes(e.id))
        .map((e) => e.id),
    ).toEqual(left);
    expect(store.all()).toEqual([]);
  });

  it("parks the events of a session when its credential changes, and holds them for their principal", async () => {
    const store = new MemoryEventStore();
    const page = rig({ host: "server" }, { store });
    page.transport.principal = "urn:uuid:ana";
    warn(page.queue, 0);
    await settle();
    page.transport.script = [{ kind: "status", status: 403 }];
    const parked = warn(page.queue, 1);
    await settle(100);
    page.queue.credentialChanged();
    page.transport.script = [{ kind: "ack" }];
    page.transport.principal = "urn:uuid:ben";
    await settle(10_000);
    expect(page.transport.journaled).not.toContain(parked);
    expect(store.all().map((e) => e.id)).toContain(parked);
    expect(store.all().find((e) => e.id === parked)?.principal).toBe("urn:uuid:ana");
  });
});
