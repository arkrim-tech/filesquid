/**
 * Delivery of the UI's console warnings and errors to `POST /api/v1/client-events` (UI-S-023,
 * API-S-030, ADR 033 Decision 7). {@link ClientEventQueue} holds every event until the route
 * acknowledges it, and every acknowledged event is journaled once, as its own entry, under the
 * client event ID the event keeps for life.
 *
 * - **Bounds.** At most 10,000 events in memory; batches of at most 20 events and 16 KiB; at most
 *   60 requests in any rolling minute. The bounds only delay events, never merge them.
 * - **Retries.** After a 429, a 401 or 403, a 5xx or a network failure the same events are sent
 *   again with exponential backoff from 1 s, doubling, at most 60 s between attempts, and never
 *   sooner than `Retry-After`, for as long as the page is open. A 400 (or 413) batch is not
 *   retried as it is: each of its events is sent alone, and an event the route refuses on its own
 *   is removed and replaced by one `client_event_refused` error event naming its component.
 * - **Storage.** While storage accepts writes every queued event is also written to it as it is
 *   queued and removed when acknowledged; past the memory cap events spill into the store, in
 *   order, and return to the queue oldest first as it drains; at page close what is not stored
 *   is, and that write settles before the overflow count is sent. Writes to the store run one
 *   after another. The next open delivers the stored events of this host oldest first, each under
 *   the credential of the principal that raised it.
 * - **Principals.** The route names the credential's principal in each acknowledgement. Events
 *   of earlier pages carry the principal their page was acknowledged under, and are sent only
 *   once this page's principal is known and the same: the open raises one `persisted_events_found`
 *   warning, which the route acknowledges, to learn it. The events of another principal stay in
 *   the store untouched until that principal sends again. Events of a page that never learned a
 *   principal (`null`) go to the first principal that does. Events raised in this page carry no
 *   principal claim; when the page's credential changes ({@link ClientEventQueue.credentialChanged})
 *   the events still queued are parked in the store under their own principal. A response to a
 *   request sent before that change is ignored, so it cannot delete the parked events or name
 *   the new principal.
 * - **The one gap.** When storage is unavailable or refuses a write, events fall back to the
 *   in-memory queue, every event that could not be kept is counted, and at page close one
 *   `ui.console.overflow` event (own ID, count 1 to 1,000,000) goes out as a keep-alive request.
 *
 * Events hold a level, a component identifier, an error code and opaque IDs; never message text,
 * URLs, file names or stack traces. The queue never writes to the console itself (it would be
 * forwarded back); it reports to `options.log`.
 */
import type { ClientEventsAck, ClientEventsRequest } from "../api/client";
import { ApiError } from "../api/client";
import { STORE_BUDGET_BYTES, type EventStore, type StoredEvent } from "./store";

/** Events held in memory at most. */
export const MEMORY_CAP = 10_000;
/** Events in one request at most. */
export const BATCH_EVENTS = 20;
/** Bytes of one request body at most. */
export const BATCH_BYTES = 16 * 1024;
/** Requests in any rolling minute at most. */
export const REQUESTS_PER_MINUTE = 60;
/** The first wait after a failure. */
export const BACKOFF_START_MS = 1_000;
/** The longest wait between attempts (a longer `Retry-After` still wins). */
export const BACKOFF_MAX_MS = 60_000;
/** The largest overflow count. */
export const OVERFLOW_MAX = 1_000_000;
/** The budget of the store, restated for the README and the tests. */
export const STORE_BUDGET = STORE_BUDGET_BYTES;

/** The rolling window of the rate bound. */
const WINDOW_MS = 60_000;
/** Component of the queue's own events. */
export const QUEUE_COMPONENT = "client-events";
/** Error code of the warning that tells the route a page found events of an earlier page. */
export const PERSISTED_CODE = "persisted_events_found";
/** Error code of the event that replaces one the route refused. */
export const REFUSED_CODE = "client_event_refused";
/** Longest opaque identifier the route accepts. */
const MAX_OPAQUE = 128;
/** Most opaque IDs on one event. */
const MAX_IDS = 8;
/** Stored events stamped with a principal in one write. */
const STAMP_PAGE = 1_000;

/** A console event as the page raises it. */
export interface ClientEventInput {
  /** `warn` or `error`. */
  readonly level: "warn" | "error";
  /** The fixed component identifier. */
  readonly component: string;
  /** The machine-readable error code. */
  readonly error_code: string;
  /** Up to 8 opaque IDs. */
  readonly ids?: readonly string[];
}

/** The transport: `FileSquidClient.postClientEvents`. */
export type SendEvents = (
  request: ClientEventsRequest,
  options?: { keepalive?: boolean },
) => Promise<ClientEventsAck>;

/** What the queue needs. */
export interface QueueOptions {
  /** Sends one batch. */
  readonly transport: SendEvents;
  /** The durable store. */
  readonly store: EventStore;
  /** The host the events are raised against (the API base URL of the page). */
  readonly host: string;
  /** Reports a problem the queue handled; never the console of the page. */
  readonly log?: (message: string, error?: unknown) => void;
  /** Events held in memory at most (default {@link MEMORY_CAP}). */
  readonly memoryCap?: number;
  /** Events in a request at most (default {@link BATCH_EVENTS}). */
  readonly batchEvents?: number;
  /** Bytes of a request body at most (default {@link BATCH_BYTES}). */
  readonly batchBytes?: number;
  /** Requests a minute at most (default {@link REQUESTS_PER_MINUTE}). */
  readonly requestsPerMinute?: number;
}

/** What the queue holds, for the tests and for diagnostics. */
export interface QueueStats {
  /** Events in memory. */
  readonly memory: number;
  /** Events only in the store, behind the memory queue. */
  readonly spilled: number;
  /** Events that could not be kept so far. */
  readonly lost: number;
  /** The principal the acknowledgements named, once one did. */
  readonly principal: string | undefined;
}

/**
 * The count a page-close overflow event carries. Pure.
 *
 * @param lost How many events could not be kept.
 * @returns The count from 1 to {@link OVERFLOW_MAX}, or undefined when nothing was lost (no
 *   overflow event is sent for 0).
 */
export function overflowCount(lost: number): number | undefined {
  const count = Math.floor(lost);
  return count >= 1 ? Math.min(count, OVERFLOW_MAX) : undefined;
}

/**
 * A random version 4 UUID, from `crypto.getRandomValues`. Reads the browser's random source.
 *
 * @returns The UUID in its 8-4-4-4-12 form.
 */
export function uuidV4(): string {
  const bytes = new Uint8Array(16);
  globalThis.crypto.getRandomValues(bytes);
  bytes[6] = ((bytes[6] ?? 0) & 0x0f) | 0x40;
  bytes[8] = ((bytes[8] ?? 0) & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

/** Keep only the characters an opaque identifier may hold, at most 128 of them; `fallback` when none. */
function opaque(value: string, fallback: string): string {
  const kept = value.replace(/[^A-Za-z0-9._:-]/g, "_").slice(0, MAX_OPAQUE);
  return kept === "" ? fallback : kept;
}

/** The bytes of a request body. */
function bodyBytes(request: ClientEventsRequest): number {
  return new TextEncoder().encode(JSON.stringify(request)).length;
}

/** The delay the backoff gives after `failures` failures in a row. */
export function backoffDelay(failures: number): number {
  return Math.min(BACKOFF_START_MS * 2 ** Math.max(0, failures - 1), BACKOFF_MAX_MS);
}

/** True for a refusal of the batch itself: 400 or 413. */
function isBatchRefusal(error: unknown): boolean {
  return error instanceof ApiError && (error.status === 400 || error.status === 413);
}

/** The queue of one page. */
export class ClientEventQueue {
  private memory: StoredEvent[] = [];
  private spilled = 0;
  private lost = 0;
  private principal: string | undefined;
  private lastSeq = 0;
  private floor: number;
  private readonly unpersisted = new Set<string>();
  private readonly singles = new Set<string>();
  private readonly replacements = new Set<string>();
  private storeRefusing = false;
  private backlogPending = false;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private inFlight = false;
  private stopped = false;
  private failures = 0;
  private requests: number[] = [];
  /** Increments when the credential changes, so an older acknowledgement can be ignored. */
  private epoch = 0;
  /** Store writes, one after another; a rejection here does not stop the next write. */
  private writes: Promise<void> = Promise.resolve();
  private readonly cap: number;
  private readonly batchEvents: number;
  private readonly batchBytes: number;
  private readonly perMinute: number;

  /** @param options The transport, the store, the host and the optional thresholds. */
  constructor(private readonly options: QueueOptions) {
    this.cap = options.memoryCap ?? MEMORY_CAP;
    this.batchEvents = options.batchEvents ?? BATCH_EVENTS;
    this.batchBytes = options.batchBytes ?? BATCH_BYTES;
    this.perMinute = options.requestsPerMinute ?? REQUESTS_PER_MINUTE;
    this.floor = this.nextSeq();
  }

  /** What the queue holds now. */
  stats(): QueueStats {
    return {
      memory: this.memory.length,
      spilled: this.spilled,
      lost: this.lost,
      principal: this.principal,
    };
  }

  /**
   * Look in the store for events of an earlier page of this host and, when there are any, raise
   * the one warning that lets the route name this page's principal, so they can be delivered
   * under it. The floor is raised above every stored sequence first, so a page opened in the
   * same millisecond still lists them. Reads the store (I/O).
   */
  async start(): Promise<void> {
    await this.raiseFloor();
    let found = 0;
    try {
      found = await this.options.store.count(this.options.host);
    } catch (error) {
      this.report("could not look for undelivered client events", error);
    }
    if (found === 0) return;
    this.backlogPending = true;
    this.record({ level: "warn", component: QUEUE_COMPONENT, error_code: PERSISTED_CODE });
  }

  /**
   * Queue an event. It goes to memory while there is room and nothing is waiting in the store,
   * otherwise it spills into the store; when neither keeps it, it is counted as lost. Writes the
   * store (I/O) and schedules delivery.
   *
   * @param input The level, component, error code and opaque IDs.
   * @returns The client event ID the event keeps for life.
   */
  record(input: ClientEventInput): string {
    const event = this.makeEvent(input);
    if (this.spilled === 0 && this.memory.length < this.cap) this.keepInMemory(event);
    else this.spill(event);
    this.schedule(0);
    return event.id;
  }

  /**
   * The page is closing: persist what the store does not hold yet and send one overflow event,
   * as a keep-alive request, with the count of events that could not be kept. The overflow goes
   * out only after that write settles. Writes the store and sends a request (I/O).
   */
  async pagehide(): Promise<void> {
    this.clearTimer();
    const lost = await this.lostAtClose();
    this.sendOverflow(lost);
  }

  /**
   * The page's credential changed (a token was typed in): park every event still queued in the
   * store under its own principal, forget this page's principal and look for events to deliver
   * under the new one. An acknowledgement of a request sent before this returns is ignored.
   * Writes the store (I/O).
   */
  async credentialChanged(): Promise<void> {
    this.epoch += 1;
    this.clearTimer();
    const parked = this.memory.map((event) => this.withPrincipal(event));
    if (parked.length > 0) {
      try {
        await this.write(parked);
      } catch (error) {
        this.loseUnpersisted(parked);
        this.report("could not park", error);
      }
    }
    this.memory = [];
    this.spilled = 0;
    this.unpersisted.clear();
    this.singles.clear();
    this.principal = undefined;
    this.failures = 0;
    this.floor = this.nextSeq();
    await this.start();
  }

  /** Stop delivering; nothing more is sent and no timer stays. */
  stop(): void {
    this.stopped = true;
    this.clearTimer();
  }

  /** The next sequence number, strictly increasing and time-ordered across page opens. */
  private nextSeq(): number {
    this.lastSeq = Math.max(this.lastSeq + 1, Date.now() * 1000);
    return this.lastSeq;
  }

  /** Raise the floor above every event this host already has stored. */
  private async raiseFloor(): Promise<void> {
    try {
      const max = await this.options.store.maxSeq(this.options.host);
      if (max < this.floor) return;
      this.lastSeq = Math.max(this.lastSeq, max);
      this.floor = this.nextSeq();
    } catch (error) {
      this.report("could not read the stored sequence", error);
    }
  }

  /** True while `epoch` is still the session a delivery step started in. */
  private sameEpoch(epoch: number): boolean {
    return epoch === this.epoch;
  }

  /**
   * Queue a store write behind the ones already started. The returned promise rejects when this
   * write fails; the chain itself does not, so a later write still runs.
   */
  private write(events: readonly StoredEvent[]): Promise<void> {
    const run = this.writes.then(() =>
      events.length === 0 ? undefined : this.options.store.put(events),
    );
    this.writes = run.then(
      () => undefined,
      () => undefined,
    );
    return run;
  }

  /** Count events the store does not hold, and forget them. */
  private loseUnpersisted(events: readonly StoredEvent[]): void {
    for (const event of events) {
      if (!this.unpersisted.delete(event.id)) continue;
      this.lost += 1;
    }
  }

  /** Build the stored event for an input. */
  private makeEvent(input: ClientEventInput): StoredEvent {
    const ids = input.ids?.slice(0, MAX_IDS).map((id) => opaque(id, "id"));
    return {
      id: uuidV4(),
      seq: this.nextSeq(),
      kind: "console",
      level: input.level,
      component: opaque(input.component, "unknown"),
      error_code: opaque(input.error_code, "unknown"),
      ...(ids && ids.length > 0 ? { ids } : {}),
      time: new Date().toISOString(),
      host: this.options.host,
      principal: this.principal ?? null,
    };
  }

  /** Put an event in the memory queue and write it through to the store. */
  private keepInMemory(event: StoredEvent): void {
    this.memory.push(event);
    this.unpersisted.add(event.id);
    this.write([event]).then(
      () => {
        this.unpersisted.delete(event.id);
        this.storeRefusing = false;
      },
      (error: unknown) => {
        this.storeRefusing = true;
        this.report("could not write an event through", error);
      },
    );
  }

  /** Put an event only in the store, behind the memory queue; count it when the store refuses. */
  private spill(event: StoredEvent): void {
    if (!this.options.store.available) {
      this.lost += 1;
      return;
    }
    this.spilled += 1;
    this.write([event]).then(
      () => {
        this.storeRefusing = false;
      },
      (error: unknown) => {
        this.storeRefusing = true;
        this.spilled -= 1;
        this.lost += 1;
        this.report("could not spill an event", error);
      },
    );
  }

  /** The event with this page's principal, when it is known and the event has none. */
  private withPrincipal(event: StoredEvent): StoredEvent {
    if (event.principal !== null || this.principal === undefined) return event;
    return { ...event, principal: this.principal };
  }

  /** True when the event was stored before the page's principal was known. */
  private needsPrincipal(event: StoredEvent): boolean {
    return event.principal === null && this.principal !== undefined;
  }

  /** How many events could not be kept, once the page-close write has settled. */
  private async lostAtClose(): Promise<number> {
    let lost = this.lost;
    const unsaved = this.memory
      .filter((event) => this.unpersisted.has(event.id) || this.needsPrincipal(event))
      .map((event) => this.withPrincipal(event));
    if (unsaved.length === 0) return lost;
    const pending = this.memory.filter((event) => this.unpersisted.has(event.id)).length;
    if (!this.options.store.available || this.storeRefusing) return lost + pending;
    try {
      await this.write(unsaved);
    } catch (error) {
      this.report("could not persist", error);
      lost += pending;
    }
    return lost;
  }

  /** Schedule the next delivery step unless one is already scheduled. */
  private schedule(delayMs: number): void {
    if (this.stopped || this.timer !== undefined) return;
    this.timer = setTimeout(() => {
      this.timer = undefined;
      void this.step();
    }, delayMs);
  }

  /** Cancel the scheduled step. */
  private clearTimer(): void {
    if (this.timer !== undefined) clearTimeout(this.timer);
    this.timer = undefined;
  }

  /** The milliseconds until the rate bound lets another request go; 0 when it does now. */
  private rateWait(): number {
    const now = Date.now();
    this.requests = this.requests.filter((at) => now - at < WINDOW_MS);
    if (this.requests.length < this.perMinute) return 0;
    return (this.requests[0] ?? now) + WINDOW_MS - now;
  }

  /** One delivery step: one request, then the next step is scheduled. */
  private async step(): Promise<void> {
    if (this.stopped || this.inFlight) return;
    const wait = this.rateWait();
    if (wait > 0) return this.schedule(wait);
    this.inFlight = true;
    const epoch = this.epoch;
    let batch: StoredEvent[] = [];
    try {
      batch = await this.nextBatch();
      if (!this.sameEpoch(epoch) || batch.length === 0) return;
      this.requests.push(Date.now());
      const ack = await this.options.transport(this.requestOf(batch));
      if (!this.sameEpoch(epoch)) return;
      await this.accept(batch, ack, epoch);
    } catch (error) {
      if (this.sameEpoch(epoch)) await this.failed(batch, error, epoch);
    } finally {
      this.inFlight = false;
      if (!this.stopped && !this.sameEpoch(epoch)) this.schedule(0);
    }
    if (this.sameEpoch(epoch) && !this.stopped && (this.memory.length > 0 || this.backlogPending)) {
      this.schedule(0);
    }
  }

  /** The events of the next request, oldest first, within the bounds. */
  private async nextBatch(): Promise<StoredEvent[]> {
    const epoch = this.epoch;
    const alone = this.memory.find((event) => this.singles.has(event.id));
    if (alone) return [alone];
    if (this.backlogPending && this.principal === undefined) return this.memory.slice(0, 1);
    if (!this.backlogPending) return this.fit(this.memory);
    const earlier = await this.options.store.list(this.options.host, {
      beforeSeq: this.floor,
      limit: this.batchEvents,
      principals: [this.principal ?? null, null],
    });
    if (!this.sameEpoch(epoch)) return [];
    const named = earlier.find((event) => this.singles.has(event.id));
    if (named) return [this.withPrincipal(named)];
    if (earlier.length > 0) return this.fit(earlier.map((event) => this.withPrincipal(event)));
    this.backlogPending = false;
    return this.fit(this.memory);
  }

  /** The leading events that fit the request bounds (at least one). */
  private fit(events: readonly StoredEvent[]): StoredEvent[] {
    const taken: StoredEvent[] = [];
    for (const event of events) {
      if (taken.length >= this.batchEvents) break;
      const candidate = [...taken, event];
      if (taken.length > 0 && bodyBytes(this.requestOf(candidate)) > this.batchBytes) break;
      taken.push(event);
    }
    return taken;
  }

  /** The request body of a batch: the contract's fields only. */
  private requestOf(batch: readonly StoredEvent[]): ClientEventsRequest {
    const events = batch.map((event) => this.wire(event, event.seq < this.floor));
    return { events } as ClientEventsRequest;
  }

  /**
   * One event as the route reads it. An event of an earlier page names its principal (`named`),
   * so the route can refuse it under another credential; an event of this page names none.
   */
  private wire(event: StoredEvent, named: boolean): ClientEventsRequest["events"][number] {
    const principal = named ? (event.principal ?? this.principal) : undefined;
    const claim = principal === undefined ? {} : { principal };
    if (event.kind === "overflow") {
      return {
        kind: "overflow",
        id: event.id,
        component: event.component,
        count: event.count ?? 1,
        ...claim,
      };
    }
    return {
      kind: "console",
      id: event.id,
      level: event.level ?? "error",
      component: event.component,
      error_code: event.error_code ?? "unknown",
      ...(event.ids ? { ids: [...event.ids] } : {}),
      time: event.time,
      ...claim,
    };
  }

  /** The route acknowledged a batch: forget its events, learn the principal, refill. */
  private async accept(
    batch: readonly StoredEvent[],
    ack: ClientEventsAck,
    epoch: number,
  ): Promise<void> {
    if (!this.sameEpoch(epoch)) return;
    this.failures = 0;
    const acknowledged = new Set(ack.acknowledged);
    const ids = batch.filter((event) => acknowledged.has(event.id)).map((event) => event.id);
    this.memory = this.memory.filter((event) => !acknowledged.has(event.id));
    for (const id of ids) {
      this.unpersisted.delete(id);
      this.singles.delete(id);
    }
    await this.learnPrincipal(ack.principal, epoch);
    if (!this.sameEpoch(epoch)) return;
    await this.options.store
      .remove(ids)
      .catch((error: unknown) => this.report("could not remove", error));
    if (!this.sameEpoch(epoch)) return;
    await this.refill();
  }

  /** Take the principal an acknowledgement named, and store it on every event still queued. */
  private async learnPrincipal(principal: string, epoch: number): Promise<void> {
    if (!this.sameEpoch(epoch) || this.principal === principal) return;
    this.principal = principal;
    const mine: StoredEvent[] = [];
    const held: StoredEvent[] = [];
    for (const event of this.memory) {
      (event.principal === null || event.principal === principal ? mine : held).push(event);
    }
    await this.parkHeld(held, epoch);
    if (!this.sameEpoch(epoch)) return;
    this.memory = mine.map((event) => this.withPrincipal(event));
    await this.persistPrincipal(epoch);
    if (!this.sameEpoch(epoch)) return;
    await this.stampStored(principal, epoch);
  }

  /** Keep events of another principal in the store; count the ones that could not be written. */
  private async parkHeld(held: readonly StoredEvent[], epoch: number): Promise<void> {
    if (held.length === 0 || !this.sameEpoch(epoch)) return;
    try {
      await this.write(held);
      if (!this.sameEpoch(epoch)) return;
      for (const event of held) this.unpersisted.delete(event.id);
    } catch (error) {
      this.loseUnpersisted(held);
      this.report("could not park", error);
    }
  }

  /** Write this page's principal onto the events still in memory. */
  private async persistPrincipal(epoch: number): Promise<void> {
    try {
      await this.write(this.memory);
      if (!this.sameEpoch(epoch)) return;
      for (const event of this.memory) this.unpersisted.delete(event.id);
    } catch (error) {
      for (const event of this.memory) this.unpersisted.add(event.id);
      this.report("could not store the principal", error);
    }
  }

  /** Write this page's principal onto stored events that have none yet. */
  private async stampStored(principal: string, epoch: number): Promise<void> {
    let afterSeq = 0;
    for (;;) {
      if (!this.sameEpoch(epoch)) return;
      const page = await this.options.store.list(this.options.host, {
        afterSeq,
        limit: STAMP_PAGE,
        principals: [null],
      });
      if (!this.sameEpoch(epoch) || page.length === 0) return;
      const last = page[page.length - 1]?.seq ?? afterSeq;
      try {
        await this.write(page.map((event) => ({ ...event, principal })));
      } catch (error) {
        this.report("could not store the principal", error);
      }
      if (!this.sameEpoch(epoch) || last <= afterSeq) return;
      afterSeq = last;
    }
  }

  /** Move events that spilled into the store back into memory, oldest first, while there is room. */
  private async refill(): Promise<void> {
    const epoch = this.epoch;
    const room = this.cap - this.memory.length;
    if (this.spilled === 0 || room <= 0) return;
    const tail = this.memory[this.memory.length - 1]?.seq ?? this.floor - 1;
    const read = await this.options.store.list(this.options.host, {
      afterSeq: Math.max(tail, this.floor - 1),
      limit: room,
      principals: [this.principal ?? null, null],
    });
    if (!this.sameEpoch(epoch) || read.length === 0) {
      if (this.sameEpoch(epoch) && read.length === 0) this.spilled = 0;
      return;
    }
    this.spilled = Math.max(0, this.spilled - read.length);
    this.memory.push(...read.map((event) => this.withPrincipal(event)));
  }

  /** A request failed: split a refused batch, otherwise wait and send the same events again. */
  private async failed(
    batch: readonly StoredEvent[],
    error: unknown,
    epoch: number,
  ): Promise<void> {
    if (!this.sameEpoch(epoch)) return;
    if (batch.length === 0) {
      this.report("could not read the stored client events", error);
      this.failures += 1;
      return this.schedule(backoffDelay(this.failures));
    }
    if (isBatchRefusal(error)) return this.refused(batch, epoch);
    this.failures += 1;
    const retryAfter = error instanceof ApiError ? (error.retryAfterMs ?? 0) : 0;
    this.schedule(Math.max(backoffDelay(this.failures), retryAfter));
  }

  /** A batch was refused: resend its events alone; a single one the route refuses is replaced. */
  private async refused(batch: readonly StoredEvent[], epoch: number): Promise<void> {
    if (!this.sameEpoch(epoch)) return;
    if (batch.length > 1) {
      for (const event of batch) this.singles.add(event.id);
      return this.schedule(0);
    }
    const [event] = batch as [StoredEvent];
    this.singles.delete(event.id);
    this.memory = this.memory.filter((held) => held.id !== event.id);
    this.unpersisted.delete(event.id);
    if (!this.sameEpoch(epoch)) return;
    await this.options.store
      .remove([event.id])
      .catch((error: unknown) => this.report("could not remove", error));
    if (!this.sameEpoch(epoch)) return;
    if (this.replacements.has(event.id)) {
      this.report(`the route refused a ${REFUSED_CODE} event of ${event.component}; it is dropped`);
      return this.schedule(0);
    }
    const replacement = this.record({
      level: "error",
      component: event.component,
      error_code: REFUSED_CODE,
    });
    this.replacements.add(replacement);
  }

  /** Send the page-close overflow count as a keep-alive request, when there is one. */
  private sendOverflow(lost: number): void {
    const count = overflowCount(lost);
    if (count === undefined) return;
    const event: StoredEvent = {
      id: uuidV4(),
      seq: this.nextSeq(),
      kind: "overflow",
      component: QUEUE_COMPONENT,
      time: new Date().toISOString(),
      count,
      host: this.options.host,
      principal: this.principal ?? null,
    };
    const request = { events: [this.wire(event, false)] } as ClientEventsRequest;
    this.options.transport(request, { keepalive: true }).catch((error: unknown) => {
      this.report("could not send the overflow count", error);
    });
  }

  /** Report a handled problem to the owner of the queue. */
  private report(message: string, error?: unknown): void {
    this.options.log?.(message, error);
  }
}
