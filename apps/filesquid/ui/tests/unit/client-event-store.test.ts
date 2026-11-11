/**
 * Unit tests for the client-event store contract (UI-F2.16, UI-S-023, ADR 033 Decision 7): the
 * {@link EventStore} the queue writes through, spills into and persists at page close. The
 * contract runs on the in-memory store the queue tests use, so those tests stand on a store that
 * keeps it: host scoping, oldest first by sequence, the range, limit and principal filters,
 * replacement by ID, removal, the greatest sequence, and the 50 MiB byte budget counted in UTF-8
 * bytes, a write over it refused whole. It also covers the store with no storage (every write
 * refused, nothing held) and the IndexedDB store's fallback to it when there is no IndexedDB or it
 * cannot be opened. The IndexedDB store itself runs in a real browser in
 * `tests/e2e/preview/client-events.spec.ts`.
 */
import { describe, expect, it } from "vitest";
import {
  IndexedDbEventStore,
  STORE_BUDGET_BYTES,
  StoreRefusedError,
  UnavailableEventStore,
  eventBytes,
  type EventStore,
  type StoredEvent,
} from "../../src/clientevents/store";
import { MemoryEventStore } from "../support/client-event-fakes";

const HOST = "http://127.0.0.1:8083";
const OTHER_HOST = "https://files.example.org";
const ANA = "urn:uuid:5b0c2a44-1d6f-4c55-9a51-3c8f3f1f0a01";
const BEN = "urn:uuid:5b0c2a44-1d6f-4c55-9a51-3c8f3f1f0a02";

/** A console event of `HOST` with sequence `seq`, an ID derived from it and no principal. */
function event(seq: number, overrides: Partial<StoredEvent> = {}): StoredEvent {
  return {
    id: `00000000-0000-4000-8000-${String(seq).padStart(12, "0")}`,
    seq,
    kind: "console",
    level: "warn",
    component: "ui.console",
    error_code: "fetch_failed",
    time: "2026-11-11T12:00:00Z",
    host: HOST,
    principal: null,
    ...overrides,
  };
}

/** The reason of the refusal `promise` rejects with, or a failure when it resolves. */
async function refusalOf(promise: Promise<unknown>): Promise<string> {
  const error: unknown = await promise.then(
    () => new Error("the write was not refused"),
    (reason: unknown) => reason,
  );
  if (!(error instanceof StoreRefusedError)) throw error;
  return error.reason;
}

describe("eventBytes", () => {
  it("counts the UTF-8 bytes of the event's JSON text, not its UTF-16 length", () => {
    const ascii = event(1, { component: "ui.console" });
    const wide = event(1, { component: "ui.cönsole" });
    expect(eventBytes(ascii)).toBe(JSON.stringify(ascii).length);
    expect(eventBytes(wide)).toBe(JSON.stringify(wide).length + 1);
  });
});

describe("the store contract (in-memory store)", () => {
  it("lists one host's events oldest first by sequence, never another host's", async () => {
    const store = new MemoryEventStore();
    await store.put([event(3), event(1), event(2, { host: OTHER_HOST }), event(4)]);
    expect((await store.list(HOST)).map((e) => e.seq)).toEqual([1, 3, 4]);
    expect((await store.list(OTHER_HOST)).map((e) => e.seq)).toEqual([2]);
    expect(await store.count(HOST)).toBe(3);
    expect(await store.count(OTHER_HOST)).toBe(1);
  });

  it("reads a range of sequences exclusively, at most the limit, oldest first", async () => {
    const store = new MemoryEventStore();
    await store.put([1, 2, 3, 4, 5, 6].map((seq) => event(seq)));
    expect((await store.list(HOST, { afterSeq: 2, beforeSeq: 6 })).map((e) => e.seq)).toEqual([
      3, 4, 5,
    ]);
    expect((await store.list(HOST, { limit: 2 })).map((e) => e.seq)).toEqual([1, 2]);
  });

  it("reads only the principals asked for, null being the unknown principal", async () => {
    const store = new MemoryEventStore();
    await store.put([
      event(1, { principal: ANA }),
      event(2, { principal: BEN }),
      event(3, { principal: null }),
    ]);
    expect((await store.list(HOST, { principals: [ANA] })).map((e) => e.seq)).toEqual([1]);
    expect((await store.list(HOST, { principals: [null, BEN] })).map((e) => e.seq)).toEqual([2, 3]);
    expect(await store.list(HOST, { principals: [] })).toEqual([]);
  });

  it("replaces an event written again under the same ID, and counts its bytes once", async () => {
    const store = new MemoryEventStore();
    await store.put([event(1)]);
    const parked = event(1, { principal: ANA });
    await store.put([parked]);
    expect(await store.list(HOST)).toEqual([parked]);
    expect(store.bytes()).toBe(eventBytes(parked));
  });

  it("removes events by ID, ignores an unknown ID, and gives their bytes back", async () => {
    const store = new MemoryEventStore();
    await store.put([event(1), event(2)]);
    await store.remove([event(1).id, "not-stored"]);
    expect((await store.list(HOST)).map((e) => e.seq)).toEqual([2]);
    expect(store.bytes()).toBe(eventBytes(event(2)));
  });

  it("answers the greatest stored sequence of a host, or 0 for a host with none", async () => {
    const store = new MemoryEventStore();
    expect(await store.maxSeq(HOST)).toBe(0);
    await store.put([event(7), event(9, { host: OTHER_HOST }), event(5)]);
    expect(await store.maxSeq(HOST)).toBe(7);
    expect(await store.maxSeq(OTHER_HOST)).toBe(9);
  });
});

describe("the byte budget", () => {
  it("is 50 MiB for the origin", () => {
    expect(STORE_BUDGET_BYTES).toBe(50 * 1024 * 1024);
    expect(new MemoryEventStore().budget).toBe(STORE_BUDGET_BYTES);
  });

  it("refuses a whole write that would pass the budget, keeping what was stored", async () => {
    const store = new MemoryEventStore();
    store.budget = eventBytes(event(1)) * 2;
    await store.put([event(1), event(2)]);
    expect(await refusalOf(store.put([event(3)]))).toBe("budget");
    expect(await refusalOf(store.put([event(3), event(4)]))).toBe("budget");
    expect((await store.list(HOST)).map((e) => e.seq)).toEqual([1, 2]);
    expect(store.bytes()).toBe(store.budget);
  });

  it("accepts a write again once removals make room", async () => {
    const store = new MemoryEventStore();
    store.budget = eventBytes(event(1)) * 2;
    await store.put([event(1), event(2)]);
    await store.remove([event(1).id]);
    await store.put([event(3)]);
    expect((await store.list(HOST)).map((e) => e.seq)).toEqual([2, 3]);
  });

  it("counts a replacement by its new size, refusing a larger one that would pass the budget", async () => {
    const store = new MemoryEventStore();
    store.budget = eventBytes(event(1));
    await store.put([event(1)]);
    expect(await refusalOf(store.put([event(1, { principal: ANA })]))).toBe("budget");
    expect(await store.list(HOST)).toEqual([event(1)]);
  });
});

describe("a store with no storage", () => {
  it("refuses every write as unavailable and holds nothing", async () => {
    const store: EventStore = new UnavailableEventStore();
    expect(store.available).toBe(false);
    expect(await refusalOf(store.put([event(1)]))).toBe("unavailable");
    await store.remove([event(1).id]);
    expect(await store.list(HOST)).toEqual([]);
    expect(await store.count(HOST)).toBe(0);
    expect(await store.maxSeq(HOST)).toBe(0);
  });

  it("is what the IndexedDB store opens as in a page with no IndexedDB", async () => {
    const store = await IndexedDbEventStore.open(undefined);
    expect(store).toBeInstanceOf(UnavailableEventStore);
    expect(store.available).toBe(false);
  });

  it("is what the IndexedDB store opens as when IndexedDB refuses to open (a private window)", async () => {
    const refusing = {
      open: () => {
        const request = {} as IDBOpenDBRequest & { error: DOMException };
        Object.defineProperty(request, "error", {
          value: new DOMException("refused", "InvalidStateError"),
        });
        queueMicrotask(() => request.onerror?.call(request, new Event("error")));
        return request;
      },
    } as unknown as IDBFactory;
    const store = await IndexedDbEventStore.open(refusing);
    expect(store).toBeInstanceOf(UnavailableEventStore);
  });
});
