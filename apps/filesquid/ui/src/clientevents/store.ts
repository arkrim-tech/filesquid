/**
 * The page's durable store of undelivered client events (UI-S-023, ADR 033 Decision 7): the
 * {@link EventStore} contract the queue writes through, spills into and persists at page close,
 * and {@link IndexedDbEventStore}, which keeps the events in the page's IndexedDB within a byte
 * budget of 50 MiB for the origin. Every stored event carries the host it was raised against
 * and the opaque principal of its session, so the queue can send it only under a credential of
 * that host and principal. Where IndexedDB is missing or cannot be opened (a private window) the
 * store is {@link UnavailableEventStore}: every write is refused, and the queue counts the events
 * it could not keep.
 *
 * Every method that touches IndexedDB says so in its name or its TSDoc; nothing here logs to
 * the console, so a console capture cannot loop on its own output.
 */

/** Bytes the store may hold for the origin (UI-S-023). */
export const STORE_BUDGET_BYTES = 50 * 1024 * 1024;

/** An event held for delivery. */
export interface StoredEvent {
  /** The client event ID, a UUID v4 the event keeps for life. */
  readonly id: string;
  /** Position in the delivery order, strictly increasing across page opens. */
  readonly seq: number;
  /** A console warning or error, or the page-close overflow count. */
  readonly kind: "console" | "overflow";
  /** `warn` or `error` (console events). */
  readonly level?: "warn" | "error";
  /** The fixed component identifier. */
  readonly component: string;
  /** The machine-readable error code (console events). */
  readonly error_code?: string;
  /** Up to 8 opaque IDs (console events). */
  readonly ids?: readonly string[];
  /** When the event was raised, RFC 3339 UTC. */
  readonly time: string;
  /** The number of events that could not be kept (overflow events). */
  readonly count?: number;
  /** The host the event was raised against (the API base URL of the page). */
  readonly host: string;
  /** The opaque principal of the session, or null when no acknowledgement had named it yet. */
  readonly principal: string | null;
}

/** What {@link EventStore.list} reads. */
export interface ListOptions {
  /** Only events with a greater `seq`. */
  readonly afterSeq?: number;
  /** Only events with a smaller `seq`. */
  readonly beforeSeq?: number;
  /** At most this many events (default 1,000). */
  readonly limit?: number;
  /** Only events of these principals (`null` is the unknown principal); all when absent. */
  readonly principals?: readonly (string | null)[];
}

/** Why the store refused a write. */
export type RefusalReason = "unavailable" | "quota" | "budget" | "failed";

/** A write the store refused: IndexedDB is unavailable, the quota or the 50 MiB budget is reached. */
export class StoreRefusedError extends Error {
  /** @param reason Why the write was refused. */
  constructor(readonly reason: RefusalReason) {
    super(`The event store refused a write (${reason})`);
    this.name = "StoreRefusedError";
  }
}

/** The durable store of the queue. */
export interface EventStore {
  /** False when there is no storage at all; every write is then refused. */
  readonly available: boolean;
  /** Write events (replacing those with the same ID). Rejects with {@link StoreRefusedError}. */
  put(events: readonly StoredEvent[]): Promise<void>;
  /** Remove events by ID; an ID that is not stored is ignored. */
  remove(ids: readonly string[]): Promise<void>;
  /** Read the events of `host`, oldest first. */
  list(host: string, options?: ListOptions): Promise<StoredEvent[]>;
  /** The number of events held for `host`. */
  count(host: string): Promise<number>;
}

/**
 * The bytes an event counts against the budget. Pure.
 *
 * @param event The event.
 * @returns The length of its JSON text.
 */
export function eventBytes(event: StoredEvent): number {
  return JSON.stringify(event).length;
}

/** A store that has no storage: every write is refused, and nothing is ever held. */
export class UnavailableEventStore implements EventStore {
  readonly available = false;

  /** @throws StoreRefusedError always. */
  async put(): Promise<void> {
    throw new StoreRefusedError("unavailable");
  }

  /** Nothing is held, so nothing is removed. */
  async remove(): Promise<void> {
    return undefined;
  }

  /** Nothing is held. */
  async list(): Promise<StoredEvent[]> {
    return [];
  }

  /** Nothing is held. */
  async count(): Promise<number> {
    return 0;
  }
}

/** Name of the IndexedDB database. */
const DB_NAME = "filesquid-client-events";
/** Object store of the events, keyed by client event ID. */
const EVENTS = "events";
/** Object store of the one record that counts the bytes held. */
const META = "meta";
/** The key of the byte count. */
const BYTES_KEY = "bytes";
/** Index over the host and the sequence number. */
const BY_HOST_SEQ = "byHostSeq";
/** Most records one list call looks at, so a store full of other principals' events is still bounded. */
const MAX_SCAN = 200_000;

/** A record as stored: the event and the bytes it counts. */
type Record = StoredEvent & { readonly bytes: number };

/** The result of an IDB request, as a promise. */
function result<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

/** A promise that settles when the transaction completes, aborts or fails. */
function done(tx: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onabort = () => reject(tx.error ?? new Error("IndexedDB transaction aborted"));
    tx.onerror = () => reject(tx.error ?? new Error("IndexedDB transaction failed"));
  });
}

/** Map an IndexedDB failure to the refusal the queue counts. */
function refusal(error: unknown): StoreRefusedError {
  if (error instanceof StoreRefusedError) return error;
  const quota = error instanceof DOMException && error.name === "QuotaExceededError";
  return new StoreRefusedError(quota ? "quota" : "failed");
}

/** The record to store for an event. */
function recordOf(event: StoredEvent): Record {
  return { ...event, bytes: eventBytes(event) };
}

/** The event of a stored record. */
function eventOf(record: Record): StoredEvent {
  const { bytes: _bytes, ...event } = record;
  void _bytes;
  return event;
}

/** The events of one origin in IndexedDB, within the 50 MiB budget. */
export class IndexedDbEventStore implements EventStore {
  readonly available = true;

  private constructor(
    private readonly db: IDBDatabase,
    private readonly budget: number,
  ) {}

  /**
   * Open the database. Opens IndexedDB (I/O).
   *
   * @param factory The page's `indexedDB`, or undefined when there is none.
   * @param budget Bytes the store may hold (default 50 MiB).
   * @returns The store, or an {@link UnavailableEventStore} when there is no IndexedDB or it
   *   cannot be opened (a private window refuses it); the caller is told by `available`.
   */
  static async open(
    factory: IDBFactory | undefined,
    budget: number = STORE_BUDGET_BYTES,
  ): Promise<EventStore> {
    if (!factory) return new UnavailableEventStore();
    try {
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = factory.open(DB_NAME, 1);
        request.onupgradeneeded = () => IndexedDbEventStore.upgrade(request.result);
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error ?? new Error("IndexedDB open failed"));
        request.onblocked = () => reject(new Error("IndexedDB open blocked"));
      });
      return new IndexedDbEventStore(db, budget);
    } catch {
      return new UnavailableEventStore();
    }
  }

  /** Create the object stores and the index of a new database. */
  private static upgrade(db: IDBDatabase): void {
    const events = db.createObjectStore(EVENTS, { keyPath: "id" });
    events.createIndex(BY_HOST_SEQ, ["host", "seq"]);
    db.createObjectStore(META);
  }

  /**
   * Write events in one transaction, refusing the whole write when it would pass the budget.
   * Writes IndexedDB (I/O).
   *
   * @param events The events; one already stored under the same ID is replaced.
   * @throws StoreRefusedError when the budget or the browser's quota is reached, or the write
   *   fails.
   */
  async put(events: readonly StoredEvent[]): Promise<void> {
    if (events.length === 0) return;
    try {
      const tx = this.db.transaction([EVENTS, META], "readwrite");
      const store = tx.objectStore(EVENTS);
      const meta = tx.objectStore(META);
      let bytes = ((await result(meta.get(BYTES_KEY))) as number | undefined) ?? 0;
      for (const event of events) {
        const record = recordOf(event);
        const old = (await result(store.get(event.id))) as Record | undefined;
        bytes += record.bytes - (old?.bytes ?? 0);
        if (bytes > this.budget) {
          tx.abort();
          await done(tx).catch(() => undefined);
          throw new StoreRefusedError("budget");
        }
        store.put(record);
      }
      meta.put(bytes, BYTES_KEY);
      await done(tx);
    } catch (error) {
      throw refusal(error);
    }
  }

  /**
   * Remove events by ID in one transaction. Writes IndexedDB (I/O).
   *
   * @param ids The client event IDs.
   * @throws Error when the transaction fails.
   */
  async remove(ids: readonly string[]): Promise<void> {
    if (ids.length === 0) return;
    const tx = this.db.transaction([EVENTS, META], "readwrite");
    const store = tx.objectStore(EVENTS);
    const meta = tx.objectStore(META);
    let bytes = ((await result(meta.get(BYTES_KEY))) as number | undefined) ?? 0;
    for (const id of ids) {
      const old = (await result(store.get(id))) as Record | undefined;
      if (!old) continue;
      bytes -= old.bytes;
      store.delete(id);
    }
    meta.put(Math.max(0, bytes), BYTES_KEY);
    await done(tx);
  }

  /**
   * Read the events of `host`, oldest first. Reads IndexedDB (I/O).
   *
   * @param host The host.
   * @param options The range, the limit and the principals wanted.
   * @returns The events.
   * @throws Error when the read fails.
   */
  async list(host: string, options: ListOptions = {}): Promise<StoredEvent[]> {
    const { afterSeq = 0, beforeSeq = Number.MAX_SAFE_INTEGER, limit = 1000, principals } = options;
    const range = IDBKeyRange.bound([host, afterSeq], [host, beforeSeq], true, true);
    const index = this.db.transaction(EVENTS, "readonly").objectStore(EVENTS).index(BY_HOST_SEQ);
    const found: StoredEvent[] = [];
    let scanned = 0;
    let cursor = await result(index.openCursor(range));
    while (cursor && found.length < limit && scanned < MAX_SCAN) {
      scanned += 1;
      const event = eventOf(cursor.value as Record);
      if (principals === undefined || principals.includes(event.principal)) found.push(event);
      cursor = await advance(cursor);
    }
    return found;
  }

  /**
   * Count the events of `host`. Reads IndexedDB (I/O).
   *
   * @param host The host.
   * @returns The count.
   * @throws Error when the read fails.
   */
  async count(host: string): Promise<number> {
    const range = IDBKeyRange.bound([host, 0], [host, Number.MAX_SAFE_INTEGER]);
    const index = this.db.transaction(EVENTS, "readonly").objectStore(EVENTS).index(BY_HOST_SEQ);
    return result(index.count(range));
  }
}

/** Move a cursor on and wait for the next record (null at the end). */
function advance(cursor: IDBCursorWithValue): Promise<IDBCursorWithValue | null> {
  return new Promise((resolve, reject) => {
    const request = cursor.request as IDBRequest<IDBCursorWithValue | null>;
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB cursor failed"));
    cursor.continue();
  });
}
