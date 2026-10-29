/**
 * A fake of the library routes for the production-build Playwright tests (UI-1.1): the page is
 * the real built UI; `POST /api/v1/libraries/location-choices`, `/libraries`, `/openings`,
 * `/snapshot-copies`, `/rebuilds`, `GET /libraries/repair-candidates` and `POST
 * /libraries/repairs` are answered here in the shapes of the API contract (`LibraryView`,
 * `SnapshotCopyReport`, `RebuildReport`, `RepairCandidates`, `RepairAnswer`). Like the real
 * routes it refuses an unknown body field (400), and the repair route refuses a request whose
 * consent is missing, unconfirmed or names other objects than the request (409). It records
 * every request, so a test can prove what was sent, and what was not.
 */
import type { Page } from "@playwright/test";
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
  /** Requests of one path, in order. */
  to(path: string): Seen[];
}

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

/** The rebuild report: both objects flagged, one with an error code. */
function rebuild(ids: string[]) {
  return {
    objects: ids.map((object_id, index) => ({
      object_id,
      head: "v2",
      files: 3,
      digest_algorithm: "sha256",
      error_codes: index === 1 ? ["E023"] : [],
      missing_version_inventory: true,
    })),
    unreadable: [],
  };
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
  await page.routeWebSocket(/\/api\/v1\/events$/, () => undefined);
}

/**
 * Serve the library routes and the shell's own routes.
 *
 * @param page - The page under test.
 * @returns The record of requests and the switches that steer the answers.
 */
export async function serveLibraries(page: Page): Promise<LibraryApi> {
  const api: LibraryApi = {
    seen: [],
    chooser: "chosen",
    needing: [...OBJECTS],
    to: (path) => api.seen.filter((request) => request.path === path),
  };
  await serveShell(page);
  await page.route(/\/api\/v1\/libraries(\/|\?|$)/, async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const body = request.method() === "GET" ? url.search : (request.postDataJSON() as unknown);
    api.seen.push({ path: url.pathname, method: request.method(), body });
    const answer = refusedShape(request.method(), url, body) ?? answerFor(api, url.pathname, body);
    await route.fulfill({ status: answer.status, json: answer.json });
  });
  return api;
}

/** The method, required fields and optional fields of each route, as the contract defines them. */
const ROUTE_SHAPES: Record<string, { method: string; required: string[]; optional: string[] }> = {
  "/api/v1/libraries/location-choices": { method: "POST", required: ["purpose"], optional: [] },
  "/api/v1/libraries": { method: "POST", required: ["location_id"], optional: ["library_id"] },
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
    const ok = keys.length === 1 && keys[0] === "library_id";
    return ok ? undefined : { status: 400, json: error("invalid_request") };
  }
  const keys = Object.keys((body ?? {}) as Record<string, unknown>);
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
  if (path.endsWith("/rebuilds")) return ok(rebuild(OBJECTS));
  if (path.endsWith("/snapshot-copies")) {
    return ok({
      objects: 2,
      files: 6,
      bytes: 3072,
      invalid_objects: [{ object_id: OBJECTS[1], error_codes: ["E023"] }],
    });
  }
  if (path.endsWith("/repairs")) return repairAnswer(api, record);
  return ok({
    library_id: typeof record.library_id === "string" ? record.library_id : LIB,
    spec_version: "1.1",
    objects: 2,
  });
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
