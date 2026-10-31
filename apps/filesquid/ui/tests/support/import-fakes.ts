/**
 * Shared fixtures of the upload flow tests (UI-1.2): the contract's answers for a plan, its
 * files, a job and its files, with the numbers of mockups 18 and 19, and a fake API client that
 * records every request and answers from a table of routes.
 */
import { ApiError } from "../../src/api/client";
import type { ApiEvent } from "../../src/api/client";
import type { ImportPlan, JobItem, JobView, PlanItem } from "../../src/imports/model";

/** The library of the fixtures. */
export const LIBRARY_ID = "lib-1";
/** The collection of the fixtures. */
export const COLLECTION_ID = "col-1";

/** The plan of mockup 18: 214 files found, 196 new, 18 already in the library, 3.1 GB. */
export const PLAN: ImportPlan = {
  plan_id: "plan-1",
  source: "files",
  destination: { collection_id: COLLECTION_ID, folder: "Site photos" },
  source_volume: "EOS_DIGITAL",
  files_found: 214,
  files_new: 196,
  files_present: 18,
  bytes_to_copy: 3_328_599_654,
  names_numbered: 1,
  free_bytes_after: 439_000_000_000,
  staging_limit_bytes: 10 * 1024 ** 3,
  expires: "2026-10-31T12:02:00Z",
};

/** The planned files of mockup 18, the last one already in the library. */
export const PLAN_ITEMS: readonly PlanItem[] = [
  { item_id: "p1", name: "IMG_4102.CR3", bytes: 24_800_000, present: false, numbered: false },
  { item_id: "p2", name: "IMG_4102.JPG", bytes: 6_400_000, present: false, numbered: true },
  { item_id: "p3", name: "IMG_4103.CR3", bytes: 25_100_000, present: false, numbered: false },
  { item_id: "p4", name: "IMG_4103.JPG", bytes: 6_200_000, present: false, numbered: false },
  { item_id: "p5", name: "MVI_4104.MOV", bytes: 412_000_000, present: false, numbered: false },
  { item_id: "p6", name: "IMG_4021.JPG", bytes: 5_900_000, present: true, numbered: false },
];

/** A job view; each test overrides what it needs. */
export function jobView(over: Partial<JobView> = {}): JobView {
  return {
    id: "job-1",
    kind: "import",
    operation_id: "op-1",
    destination: { collection_id: COLLECTION_ID, folder: "Site photos" },
    state: "running",
    paused: false,
    done: 82,
    total: 196,
    attempts: 1,
    created: "2026-10-31T11:02:00Z",
    updated: "2026-10-31T11:03:00Z",
    result: null,
    ...over,
  };
}

/** A job file; each test overrides what it needs. */
export function jobItem(name: string, over: Partial<JobItem> = {}): JobItem {
  return {
    item_id: `item-${name}`,
    name,
    folder: "Site photos",
    state: "queued",
    bytes_done: 0,
    bytes_total: 1000,
    ...over,
  };
}

/** The four files of mockup 19: copied, copying at 62 %, queued and failed. */
export const MOCKUP_ITEMS: readonly JobItem[] = [
  jobItem("IMG_4101.JPG", { state: "copied", bytes_done: 1000 }),
  jobItem("IMG_4102.CR3", { state: "copying", bytes_done: 620 }),
  jobItem("IMG_4103.JPG"),
  jobItem("IMG_4099.JPG", { state: "failed", error_code: "source_changed" }),
];

/** An `ApiError` as the client raises it for a non-2xx answer. */
export function refusal(status: number, code: string, path = "/api/v1/imports"): ApiError {
  return new ApiError(status, path, { code: code as never, retryable: false });
}

/** One request the fake client saw. */
export interface SeenRequest {
  readonly path: string;
  readonly method: string;
  readonly body: unknown;
}

/** How a fake route answers: a value, or a function of the request. */
export type Answer = unknown | ((seen: SeenRequest) => unknown);

/** A fake API client: records requests, answers from `routes`, and lets a test push events. */
export class FakeClient {
  readonly seen: SeenRequest[] = [];
  private listeners = new Set<(event: ApiEvent) => void>();
  private openers = new Set<() => void>();
  subscriptions = 0;

  /** @param routes Answers by `METHOD path` (the query string included). */
  constructor(readonly routes: Record<string, Answer> = {}) {}

  /** Answer one request, or reject with the `ApiError` a route holds. */
  request = async <T>(
    path: string,
    options: { method?: string; body?: unknown } = {},
  ): Promise<T> => {
    const seen = { path, method: options.method ?? "GET", body: options.body };
    this.seen.push(seen);
    const key = `${seen.method} ${path}`;
    if (!(key in this.routes)) throw new Error(`no fake route for ${key}`);
    const route = this.routes[key];
    const answer =
      typeof route === "function" ? (route as (s: SeenRequest) => unknown)(seen) : route;
    if (answer instanceof Error) throw answer;
    return answer as T;
  };

  /** Subscribe like `FileSquidClient.subscribe`. */
  subscribe = (listener: (event: ApiEvent) => void, onOpen?: () => void): (() => void) => {
    this.subscriptions += 1;
    this.listeners.add(listener);
    if (onOpen) this.openers.add(onOpen);
    return () => {
      this.listeners.delete(listener);
      if (onOpen) this.openers.delete(onOpen);
    };
  };

  /** Push an event to every subscriber. */
  emit(event: ApiEvent): void {
    for (const listener of [...this.listeners]) listener(event);
  }

  /** Tell every subscriber the channel opened. */
  open(): void {
    for (const opener of [...this.openers]) opener();
  }

  /** How many listeners are subscribed now. */
  get listening(): number {
    return this.listeners.size;
  }
}
