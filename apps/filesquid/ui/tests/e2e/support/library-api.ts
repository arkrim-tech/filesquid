/**
 * A fake of the library routes for the production-build Playwright tests (UI-1.1): the page is
 * the real built UI; `POST /api/v1/libraries/location-choices`, `/libraries`, `/openings`,
 * `/snapshot-copies`, `/rebuilds`, `GET /libraries/repair-candidates` and `POST
 * /libraries/repairs` are answered here in the shapes of the API contract (`LibraryView`,
 * `SnapshotCopyReport`, `RebuildJobAnswer`, `RepairCandidates`, `RepairAnswer`). The four
 * command routes (API-F2.19) take `{ envelope, input }` and answer an outcome for the sent
 * operation id with the route's result (`committed`; `pending` with the job for a rebuild).
 * Like the real routes it refuses an unknown body field or a missing envelope (400), and the
 * repair route refuses a request whose consent is missing, unconfirmed or names other objects
 * than the request (409). It records every request, so a test can prove what was sent, and
 * what was not.
 */
import type { Page, WebSocketRoute } from "@playwright/test";
import { FAMILY, serveNavigation } from "./navigation-api";

/** The library the fake serves. */
export const LIB = "5e2e0000-0000-4000-8000-000000000001";

/** Two objects that lack their version inventory. */
export const OBJECTS = [
  "urn:uuid:7c1e0000-0000-4000-8000-00000000000a",
  "urn:uuid:7c1e0000-0000-4000-8000-00000000000b",
];

/** One request as the fake saw it. */
export interface Seen {
  /** Path without the query. */
  readonly path: string;
  /** HTTP method. */
  readonly method: string;
  /** Parsed JSON body, or the query string for a GET. */
  readonly body: unknown;
}

/** What the folder chooser answers. */
export type Chooser = "chosen" | "cancelled" | "unavailable";

/** What the fake saw and how to steer it. */
export interface LibraryApi {
  /** Every library request, in order. */
  readonly seen: Seen[];
  /** How the chooser answers; `chosen` registers a location ID named after the purpose. */
  chooser: Chooser;
  /** Make the next repair request refuse with this status and code. */
  repairRefusal?: { status: number; code: string; reason?: string };
  /** Objects still needing repair. */
  needing: string[];
  /**
   * The libraries the host knows, by ID: the revision of an open one, `null` for one it
   * remembers but has not open now. A library the fake creates or opens is added at `1`;
   * the facts of any other ID are 404 `not_found`.
   */
  libraries: Map<string, string | null>;
  /** Each known library's name, last opened time and free bytes (API-S-038); defaults apply. */
  meta: Map<string, LibraryMeta>;
  /** Collections created, in order: `{ library, title }`. */
  readonly collections: { library: string; title: string }[];
  /** Make the next create-collection command refuse with this HTTP status. */
  collectionRefusal?: number;
  /** The rebuild job the job routes answer (UI-1.3); a test steers its state. */
  job: JobState;
  /** Requests of one path, in order. */
  to(path: string): Seen[];
  /** Push one event to every open event channel (for example `job.update`). */
  push(topic: string, data: unknown): void;
}

/** What the fake knows of a library besides its revision (API-S-038). */
export interface LibraryMeta {
  readonly name: string;
  /** RFC 3339 time. */
  readonly lastOpened: string;
  /** The free bytes of its volume, or null when it cannot be read. */
  readonly freeBytes: number | null;
}

/** The rebuild job as the fake serves it (the fields of the contract's `JobView`). */
export interface JobState {
  state: "pending" | "running" | "completed" | "failed" | "cancelled";
  paused: boolean;
  control?: "pause" | "cancel";
  done: number;
  total?: number;
  /** The report of a completed job. */
  result: unknown;
  errorCode?: string;
  /** The operation id of the rebuild command that submitted it; set when one is sent. */
  operationId?: string;
}

/** The job ID the fake serves. */
export const JOB = "job-e2e-1";

/** The meta of a library the fake does not know more about. */
const UNKNOWN_META: LibraryMeta = {
  name: "Untitled library",
  lastOpened: "2026-10-31T08:00:00Z",
  freeBytes: null,
};

/** An API error body. */
const error = (code: string) => ({ code, message: "refused", retryable: code === "unavailable" });

/** The repair-candidates answer for the objects still needing repair. */
function candidates(ids: string[]) {
  return {
    candidates: ids.map((object_id, index) => ({
      object_id,
      head: "v2",
      digest_algorithm: index === 1 ? "sha512" : "sha256",
      interrupted: false,
      committable_after_repair: index !== 1,
    })),
    truncated: false,
  };
}

/** The job view of the fake's job (the contract's `JobView`); `result` only once completed. */
function jobView(job: JobState, withResult = true) {
  return {
    id: JOB,
    kind: "library.rebuild",
    operation_id: job.operationId ?? "op-e2e-rebuild",
    state: job.state,
    paused: job.paused,
    ...(job.control === undefined ? {} : { control: job.control }),
    done: job.done,
    ...(job.total === undefined ? {} : { total: job.total }),
    attempts: 1,
    ...(job.errorCode === undefined ? {} : { error_code: job.errorCode }),
    created: "2026-10-31T10:41:09Z",
    updated: "2026-10-31T10:42:09Z",
    result: withResult && job.state === "completed" ? job.result : null,
  };
}

/**
 * The pending rebuild job a rebuild command answers (API-S-025): a fresh submission with no
 * progress and no attempt yet, carrying the command's operation id. The job the routes serve
 * afterwards is the one the test steers in `api.job`.
 */
function rebuildJob(api: LibraryApi, operationId: string) {
  api.job.operationId = operationId;
  const fresh = { ...api.job, state: "pending", paused: false, control: undefined, done: 0 };
  return {
    outcome: "pending",
    job_id: JOB,
    job: { ...jobView({ ...fresh, total: undefined } as JobState, false), attempts: 0 },
    scheduled: true,
  };
}

/** The routes that take a command envelope (API-F2.19). */
const COMMAND_ROUTES = [
  "/api/v1/libraries",
  "/api/v1/libraries/openings",
  "/api/v1/libraries/snapshot-copies",
  "/api/v1/libraries/rebuilds",
  "/api/v1/libraries/collections",
];

/** The envelope of a command body, or undefined when the body carries no valid one. */
function envelopeOf(body: unknown): { operation_id: string } | undefined {
  const record = (body ?? {}) as Record<string, unknown>;
  const envelope = record.envelope as Record<string, unknown> | null | undefined;
  const keys = Object.keys(record);
  const shaped =
    typeof envelope === "object" &&
    envelope !== null &&
    keys.every((key) => key === "envelope" || key === "input") &&
    envelope.schema_version === 1 &&
    typeof envelope.operation_id === "string" &&
    envelope.operation_id !== "" &&
    typeof envelope.expected_revision === "string" &&
    typeof envelope.scope === "string";
  return shaped ? { operation_id: envelope.operation_id as string } : undefined;
}

/** The outcome answer of a command: the outcome fields for `operationId` and `result`. */
function outcome(operationId: string, kind: "committed" | "pending", result: unknown) {
  return { status: 200, json: { operation_id: operationId, kind, error: null, result } };
}

/** Why the repair route would refuse this body, or undefined when it accepts it. */
function repairRefusal(
  body: Record<string, unknown>,
): { status: number; code: string } | undefined {
  const ids = body.object_ids;
  const valid = Array.isArray(ids) && ids.length >= 1 && ids.length <= 1000;
  if (!valid || new Set(ids).size !== ids.length) return { status: 400, code: "invalid_request" };
  const consent = body.consent as Record<string, unknown> | null | undefined;
  if (!consent || consent.confirmed !== true) return { status: 409, code: "conflict" };
  const same = JSON.stringify(consent.object_ids) === JSON.stringify(ids);
  const named =
    consent.library_id === body.library_id &&
    consent.statement_id === "version_inventory_repair.v2";
  return same && named ? undefined : { status: 409, code: "conflict" };
}

/** Answer the routes that hold the page together: health, host, navigation and the channel. */
async function serveShell(page: Page): Promise<void> {
  await serveNavigation(page, FAMILY);
  await page.route("**/api/v1/health", (route) =>
    route.fulfill({ json: { schema_version: 1, status: "ok", product: "filesquid" } }),
  );
  await page.route("**/api/v1/host", (route) =>
    route.fulfill({
      json: {
        schema_version: 1,
        host_type: "desktop",
        operator_name: null,
        hosted_service: {
          enrolled: false,
          backup_enabled: false,
          sync_enabled: false,
          service_host: null,
        },
      },
    }),
  );
  await page.routeWebSocket(/\/api\/v1\/events$/, (ws) => void sockets.push(ws));
}

/** The open event channels of the fake's page. */
const sockets: WebSocketRoute[] = [];

/**
 * Serve the library routes, the job routes and the shell's own routes.
 *
 * @param page - The page under test.
 * @returns The record of requests and the switches that steer the answers.
 */
export async function serveLibraries(page: Page): Promise<LibraryApi> {
  sockets.length = 0;
  const api: LibraryApi = {
    seen: [],
    chooser: "chosen",
    needing: [...OBJECTS],
    libraries: new Map([[LIB, "1"]]),
    meta: new Map(),
    collections: [],
    job: { state: "running", paused: false, done: 1204, total: 2418, result: null },
    to: (path) => api.seen.filter((request) => request.path === path),
    push: (topic, data) =>
      sockets.forEach((socket) => socket.send(JSON.stringify({ topic, data }))),
  };
  await serveShell(page);
  await serveJobs(page, api);
  await page.route(/\/api\/v1\/libraries(\/|\?|$)/, async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = request.method() === "GET" ? url.search : (request.postDataJSON() as unknown);
    api.seen.push({ path: url.pathname, method: request.method(), body });
    const answer =
      factsAnswer(api, request.method(), url) ??
      refusedShape(request.method(), url, body) ??
      answerFor(api, url.pathname, body);
    await route.fulfill({ status: answer.status, json: answer.json });
  });
  return api;
}

/** The job routes (API-S-037): read one job, pause, resume and cancel it. */
async function serveJobs(page: Page, api: LibraryApi): Promise<void> {
  await page.route(/\/api\/v1\/jobs\/[^/?]+(\/(pause|resume|cancel))?$/, async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    api.seen.push({ path: url.pathname, method: request.method(), body: request.postData() });
    const control = /\/(pause|resume|cancel)$/.exec(url.pathname)?.[1];
    const id = decodeURIComponent(url.pathname.split("/")[4] ?? "");
    if (id !== JOB) return route.fulfill({ status: 404, json: error("not_found") });
    const method = control === undefined ? "GET" : "POST";
    if (request.method() !== method) {
      return route.fulfill({ status: 405, json: error("method_not_allowed") });
    }
    if (request.postData()) {
      // The controls take no body (API-S-002).
      return route.fulfill({ status: 400, json: error("invalid_request") });
    }
    if (control === undefined) {
      return route.fulfill({ status: 200, json: jobView(api.job) });
    }
    if (control === "pause") api.job.paused = true;
    if (control === "resume") api.job.paused = false;
    if (control === "cancel") api.job.state = "cancelled";
    return route.fulfill({
      status: 200,
      json: { job: jobView(api.job), scheduled: control === "resume" },
    });
  });
}

/** The meta of a library, or the defaults. */
function metaOf(api: LibraryApi, id: string): LibraryMeta {
  return api.meta.get(id) ?? UNKNOWN_META;
}

/** A recent library as `GET /api/v1/libraries/recent` lists it (API-S-038). */
function recentEntry(api: LibraryApi, id: string, revision: string | null) {
  const meta = metaOf(api, id);
  return {
    library_id: id,
    name: meta.name,
    last_opened: meta.lastOpened,
    open: revision !== null,
    free_bytes: meta.freeBytes,
    free_bytes_reason: meta.freeBytes === null ? "volume_unreadable" : null,
  };
}

/** `GET /api/v1/libraries/{library}/facts` (API-S-038). */
const FACTS_PATH = /^\/api\/v1\/libraries\/([^/]+)\/facts$/;

/**
 * The facts of a library the page reads for its current revision, as the contract answers
 * them: a library the host knows with its revision (`null` when it is not open now) and no
 * free-space reading; any other library 404 `not_found`. Another method is 405, a query 400.
 *
 * @param api - The fake's state, holding the libraries the host knows.
 * @param method - The request method.
 * @param url - The request URL.
 * @returns The answer, or undefined when the request is not for the facts route.
 */
function factsAnswer(
  api: LibraryApi,
  method: string,
  url: URL,
): { status: number; json: unknown } | undefined {
  const match = FACTS_PATH.exec(url.pathname);
  if (!match) return undefined;
  if (method !== "GET") return { status: 405, json: error("method_not_allowed") };
  if (url.search !== "") return { status: 400, json: error("invalid_request") };
  const id = decodeURIComponent(match[1] ?? "");
  const revision = api.libraries.get(id);
  if (revision === undefined) return { status: 404, json: error("not_found") };
  const { name, freeBytes } = metaOf(api, id);
  const facts = {
    library_id: id,
    name,
    free_bytes: freeBytes,
    free_bytes_reason: freeBytes === null ? "volume_unreadable" : null,
    revision,
  };
  return { status: 200, json: facts };
}

/** The method, required fields and optional fields of each route, as the contract defines them. */
const ROUTE_SHAPES: Record<
  string,
  { method: string; required: string[]; optional: string[]; query?: string[] }
> = {
  "/api/v1/libraries/location-choices": { method: "POST", required: ["purpose"], optional: [] },
  "/api/v1/libraries": {
    method: "POST",
    required: ["location_id"],
    optional: ["library_id", "name"],
  },
  "/api/v1/libraries/recent": { method: "GET", required: [], optional: [], query: [] },
  "/api/v1/libraries/collections": {
    method: "POST",
    required: ["library_id", "title"],
    optional: [],
  },
  "/api/v1/libraries/openings": {
    method: "POST",
    required: ["location_id"],
    optional: ["library_id"],
  },
  "/api/v1/libraries/snapshot-copies": {
    method: "POST",
    required: ["library_id", "destination_location_id"],
    optional: [],
  },
  "/api/v1/libraries/rebuilds": { method: "POST", required: ["library_id"], optional: [] },
  "/api/v1/libraries/repair-candidates": { method: "GET", required: [], optional: [] },
  "/api/v1/libraries/repairs": {
    method: "POST",
    required: ["library_id", "object_ids"],
    optional: ["consent"],
  },
};

/**
 * Refuse what the real routes refuse before they read anything: another method (405), a body
 * with a field the route does not define or without one it requires (400), and for the
 * candidates query any parameter but `library_id`.
 *
 * @param method - The request method.
 * @param url - The request URL.
 * @param body - The parsed JSON body, or the query string of a GET.
 * @returns The refusal, or undefined when the request is well formed.
 */
function refusedShape(
  method: string,
  url: URL,
  body: unknown,
): { status: number; json: unknown } | undefined {
  const shape = ROUTE_SHAPES[url.pathname];
  if (!shape) return { status: 404, json: error("not_found") };
  if (method !== shape.method) return { status: 405, json: error("method_not_allowed") };
  if (method === "GET") {
    const keys = [...url.searchParams.keys()];
    const wanted = shape.query ?? ["library_id"];
    const ok = keys.length === wanted.length && keys.every((key) => wanted.includes(key));
    return ok ? undefined : { status: 400, json: error("invalid_request") };
  }
  const command = COMMAND_ROUTES.includes(url.pathname);
  if (command && envelopeOf(body) === undefined) {
    return { status: 400, json: error("invalid_request") };
  }
  const input = command ? (body as Record<string, unknown>).input : body;
  const keys = Object.keys((input ?? {}) as Record<string, unknown>);
  const allowed = [...shape.required, ...shape.optional];
  const complete = shape.required.every((key) => keys.includes(key));
  const known = keys.every((key) => allowed.includes(key));
  return complete && known ? undefined : { status: 400, json: error("invalid_request") };
}

/** The status and JSON the fake answers for one library request. */
function answerFor(
  api: LibraryApi,
  path: string,
  body: unknown,
): { status: number; json: unknown } {
  const ok = (json: unknown) => ({ status: 200, json });
  const record = (body ?? {}) as Record<string, unknown>;
  if (path.endsWith("/location-choices")) return choose(api, record);
  if (path.endsWith("/repair-candidates")) return ok(candidates(api.needing));
  if (path.endsWith("/repairs")) return repairAnswer(api, record);
  if (path.endsWith("/recent")) {
    // Newest first and at most 20, as the route lists them (API-S-038).
    const newest = [...api.libraries]
      .sort(([a], [b]) => metaOf(api, b).lastOpened.localeCompare(metaOf(api, a).lastOpened))
      .slice(0, 20);
    return ok({ libraries: newest.map(([id, revision]) => recentEntry(api, id, revision)) });
  }
  const operation = envelopeOf(body)?.operation_id ?? "";
  const input = (record.input ?? {}) as Record<string, unknown>;
  if (path.endsWith("/collections")) return collectionAnswer(api, operation, input);
  if (path.endsWith("/rebuilds")) return outcome(operation, "pending", rebuildJob(api, operation));
  if (path.endsWith("/snapshot-copies")) {
    return outcome(operation, "committed", {
      objects: 2,
      files: 6,
      bytes: 3072,
      invalid_objects: [{ object_id: OBJECTS[1], error_codes: ["E023"] }],
    });
  }
  const libraryId = typeof input.library_id === "string" ? input.library_id : LIB;
  api.libraries.set(libraryId, "1");
  if (typeof input.name === "string") {
    api.meta.set(libraryId, { ...metaOf(api, libraryId), name: input.name });
  }
  return outcome(operation, "committed", {
    library_id: libraryId,
    spec_version: "1.1",
    objects: 2,
    revision: "1",
  });
}

/** The create-collection command's answer: the collection made, or a scripted refusal. */
function collectionAnswer(
  api: LibraryApi,
  operation: string,
  input: Record<string, unknown>,
): { status: number; json: unknown } {
  const status = api.collectionRefusal;
  if (status !== undefined) {
    api.collectionRefusal = undefined;
    return { status, json: error("internal") };
  }
  const library = String(input.library_id);
  const title = String(input.title);
  api.collections.push({ library, title });
  return outcome(operation, "committed", { collection_id: "col-e2e-1", title });
}

/** The location-choices answer for the chooser's current mode. */
function choose(api: LibraryApi, body: Record<string, unknown>): { status: number; json: unknown } {
  if (api.chooser === "unavailable") return { status: 503, json: error("unavailable") };
  if (api.chooser === "cancelled") return { status: 200, json: { outcome: "cancelled" } };
  return { status: 200, json: { outcome: "chosen", location_id: `loc-${String(body.purpose)}` } };
}

/** The repair route's answer: a scripted refusal, the consent checks, or one outcome per object. */
function repairAnswer(
  api: LibraryApi,
  body: Record<string, unknown>,
): { status: number; json: unknown } {
  const scripted = api.repairRefusal;
  if (scripted) {
    api.repairRefusal = undefined;
    return { status: scripted.status, json: { ...error(scripted.code), reason: scripted.reason } };
  }
  const refused = repairRefusal(body);
  if (refused) return { status: refused.status, json: error(refused.code) };
  const ids = body.object_ids as string[];
  const objects = ids.map((object_id) => ({
    object_id,
    version: "v2",
    result: "repaired",
    error_code: null,
    retryable: false,
  }));
  api.needing = api.needing.filter((id) => !ids.includes(id));
  return { status: 200, json: { library_id: LIB, run_id: "run-1", objects } };
}
