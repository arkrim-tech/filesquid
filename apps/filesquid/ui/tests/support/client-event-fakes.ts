/**
 * Fakes for the client-event queue tests (UI-F2.16, UI-S-023): an in-memory event store that
 * follows the {@link EventStore} contract (host scoping, principal filter, oldest first, a byte
 * budget, a switch that makes writes fail, a switch for unavailable storage) and a scriptable
 * transport that records every request and plays the route's answers (acknowledge, refuse,
 * rate limit, fail). Test code only.
 */
import { ApiError } from "../../src/api/client";
import type { ClientEventsAck, ClientEventsRequest } from "../../src/api/client";
import {
  StoreRefusedError,
  eventBytes,
  type EventStore,
  type ListOptions,
  type StoredEvent,
} from "../../src/clientevents/store";

/** An in-memory {@link EventStore}. */
export class MemoryEventStore implements EventStore {
  /** False models IndexedDB being unavailable (a private window). */
  available = true;
  /** True makes every write fail as a refused write (quota). */
  refusing = false;
  /** True makes the next write fail once, before it changes anything stored. */
  failNext = false;
  /** Milliseconds `put` waits before it writes, so a test can pause a write-through. */
  delayMs = 0;
  /** Bytes the store holds at most; a write over it is refused. */
  budget = 50 * 1024 * 1024;
  /** Everything written that is not removed, by id. */
  readonly records = new Map<string, StoredEvent>();
  /** Writes made, for assertions. */
  puts = 0;

  /** All events of a host, oldest first (a test read, not part of the contract). */
  all(host?: string): StoredEvent[] {
    return [...this.records.values()]
      .filter((event) => host === undefined || event.host === host)
      .sort((a, b) => a.seq - b.seq);
  }

  private held = 0;

  /** Bytes held now. */
  bytes(): number {
    return this.held;
  }

  async put(events: readonly StoredEvent[]): Promise<void> {
    if (!this.available) throw new StoreRefusedError("unavailable");
    if (this.refusing) throw new StoreRefusedError("quota");
    // Decided before the delay: a put already waiting must not consume a refusal aimed at the next one.
    if (this.failNext) {
      this.failNext = false;
      throw new StoreRefusedError("failed");
    }
    if (this.delayMs > 0) {
      await new Promise<void>((resolve) => setTimeout(resolve, this.delayMs));
    }
    const held = this.heldAfter(events);
    if (held > this.budget) throw new StoreRefusedError("budget");
    this.puts += 1;
    this.held = held;
    for (const event of events) this.records.set(event.id, { ...event });
  }

  /** The byte total after `events` replace whatever is already stored under their ids. */
  private heldAfter(events: readonly StoredEvent[]): number {
    let held = this.held;
    const replaced = new Map<string, number>();
    for (const event of events) {
      const size = eventBytes(event);
      const known = replaced.get(event.id);
      const stored = this.records.get(event.id);
      const previous = known ?? (stored === undefined ? undefined : eventBytes(stored));
      if (previous !== undefined) held -= previous;
      held += size;
      replaced.set(event.id, size);
    }
    return held;
  }

  async remove(ids: readonly string[]): Promise<void> {
    if (!this.available) return;
    for (const id of ids) {
      const old = this.records.get(id);
      if (old) this.held -= eventBytes(old);
      this.records.delete(id);
    }
  }

  async list(host: string, options: ListOptions = {}): Promise<StoredEvent[]> {
    if (!this.available) return [];
    const {
      afterSeq = 0,
      beforeSeq = Number.POSITIVE_INFINITY,
      limit = 1000,
      principals,
    } = options;
    return this.all(host)
      .filter((event) => event.seq > afterSeq && event.seq < beforeSeq)
      .filter((event) => principals === undefined || principals.includes(event.principal))
      .slice(0, limit);
  }

  async count(host: string): Promise<number> {
    if (!this.available) return 0;
    return this.all(host).length;
  }

  async maxSeq(host: string): Promise<number> {
    if (!this.available) return 0;
    return this.all(host).reduce((max, event) => (event.seq > max ? event.seq : max), 0);
  }
}

/** What the route answers one request. */
export type Answer =
  | { readonly kind: "ack" }
  | { readonly kind: "status"; readonly status: number; readonly retryAfterSeconds?: number }
  | { readonly kind: "network" }
  | { readonly kind: "lost-ack" };

/** A request the transport saw. */
export interface SentRequest {
  readonly at: number;
  readonly request: ClientEventsRequest;
  readonly keepalive: boolean;
}

/** A transport that plays `script` per request (the last answer repeats) and records them. */
export class ScriptedTransport {
  readonly requests: SentRequest[] = [];
  /** Ids the route has journaled, once each (API-S-030). */
  readonly journaled: string[] = [];
  /** Principal the route reports for the credential. */
  principal = "urn:uuid:principal-a";
  /** The answer for each request, in order; the last repeats. */
  script: Answer[] = [{ kind: "ack" }];
  /** Events the route refuses on their own with 400 (it refuses a batch that holds one). */
  refuse: (event: ClientEventsRequest["events"][number]) => boolean = () => false;
  /** Set to hold every answer until `release` is called. */
  hold = false;
  private waiting: (() => void)[] = [];

  /** Let held requests answer. */
  release(): void {
    this.hold = false;
    for (const resume of this.waiting.splice(0)) resume();
  }

  /** The `FileSquidClient.postClientEvents` signature. */
  send = async (
    request: ClientEventsRequest,
    options: { keepalive?: boolean } = {},
  ): Promise<ClientEventsAck> => {
    const index = this.requests.length;
    this.requests.push({ at: Date.now(), request, keepalive: options.keepalive === true });
    if (this.hold) await new Promise<void>((resolve) => this.waiting.push(resolve));
    const answer = this.script[Math.min(index, this.script.length - 1)] as Answer;
    return this.play(answer, request);
  };

  /** Play one answer. */
  private play(answer: Answer, request: ClientEventsRequest): ClientEventsAck {
    if (answer.kind === "network") throw new TypeError("Failed to fetch");
    if (answer.kind === "status") throw this.refusal(answer);
    if (request.events.some((event) => this.refuse(event))) {
      throw new ApiError(400, "/api/v1/client-events", { code: "invalid_request" });
    }
    let journaled = 0;
    for (const event of request.events) {
      if (this.journaled.includes(event.id)) continue;
      this.journaled.push(event.id);
      journaled += 1;
    }
    if (answer.kind === "lost-ack") throw new TypeError("Failed to fetch");
    return {
      acknowledged: request.events.map((event) => event.id),
      journaled,
      principal: this.principal,
    };
  }

  /** The ApiError of a refusing status. */
  private refusal(answer: Extract<Answer, { kind: "status" }>): ApiError {
    const code = answer.status === 429 ? "rate_limited" : "internal";
    const retryAfterMs =
      answer.retryAfterSeconds === undefined ? undefined : answer.retryAfterSeconds * 1000;
    return new ApiError(
      answer.status,
      "/api/v1/client-events",
      { code, retryable: true },
      retryAfterMs,
    );
  }

  /** Ids of the events the route has journaled whose entry is a console event, in order. */
  journaledEvents(): { id: string; kind: string; component: string; error_code?: string }[] {
    const byId = new Map(
      this.requests.flatMap((sent) => sent.request.events).map((event) => [event.id, event]),
    );
    return this.journaled.map((id) => {
      const event = byId.get(id) as { kind: string; component: string; error_code?: string };
      return { id, kind: event.kind, component: event.component, error_code: event.error_code };
    });
  }
}
