/**
 * The fake host of the browsing performance suite (Task UI-F3.1): the routes the production
 * build reads, answered in process from the seeded {@link Catalogue}, in the contract's own
 * shape. The page is the real built UI served by `vite preview`; only the host is faked.
 *
 * Every answer is made from a precomputed order and serialised once, so the harness's own cost
 * stays small and constant, and the route round trip that remains (Playwright hands each
 * request to this process and the answer back) is measured by the suite as a control and
 * reported beside every result. No latency is modelled by default: the numbers are the UI's own
 * contribution, not a packaged host's (DE-F3.2 measures that).
 *
 * A catch-all route answers 404 to any API call not served here and records it, so a request
 * the suite did not plan for fails the test instead of being silently absorbed.
 */
import type { Page, Route, WebSocketRoute } from "@playwright/test";
import { DEFAULT_PREFERENCES, type UserPreferences } from "../../src/preferences/model";
import { Catalogue, FixtureError, type SearchParams, type ViewParams } from "./fixtures";

/** The library the fake host has open, as the storage block and the file routes name it. */
export const LIBRARY_ID = "lib-1";

/** What the fake saw, and how to steer it. */
export interface Host {
  /** The catalogue the host answers from. */
  readonly catalogue: Catalogue;
  /** Calls per route name. */
  readonly calls: Map<string, number>;
  /** The node, cursor and limit of every view request, in the order the page sent them. */
  readonly viewRequests: { node: string; cursor: string | null; limit: number }[];
  /** API calls no route served. A test asserts this is empty. */
  readonly unserved: string[];
  /** The event channel's open sockets, so a test can push events. */
  readonly sockets: WebSocketRoute[];
  /** Most API requests in flight at once, as the page sent them. */
  maxInFlight(): number;
  /** Set the milliseconds every view, search and navigation answer waits (default 0). */
  setDelay(ms: number): void;
  /** Push a `navigation.change` event for the library to every open event channel. */
  pushNavigationChange(): void;
}

/** Answer a route with JSON. */
function json(route: Route, body: unknown, status = 200): Promise<void> {
  return route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) });
}

/** The refusal body of an API error. */
function refusal(error: unknown): { status: number; body: Record<string, unknown> } {
  if (error instanceof FixtureError) {
    return {
      status: error.status,
      body: { code: error.code, message: "refused", retryable: false },
    };
  }
  throw error;
}

/** The view query of a request URL. */
function viewParams(params: URLSearchParams): ViewParams {
  return {
    node: params.get("node") ?? "",
    view: params.get("view") ?? "all",
    sort: params.get("sort") ?? "name",
    direction: params.get("direction") === "descending" ? "descending" : "ascending",
    folders: params.get("folders") === "mixed" ? "mixed" : "on_top",
    filter: params.get("filter") ?? "",
  };
}

/** The search query of a request URL. */
function searchParams(params: URLSearchParams): SearchParams {
  return {
    q: params.get("q") ?? "",
    node: params.get("node") ?? undefined,
    sort: params.get("sort") ?? "relevance",
    direction: params.get("direction") === "descending" ? "descending" : "ascending",
    folders: params.get("folders") === "mixed" ? "mixed" : "on_top",
  };
}

/** The shell's health, host and event channel. */
async function serveShell(page: Page, host: Host): Promise<void> {
  await page.route("**/api/v1/health", (route) =>
    json(route, { schema_version: 1, status: "ok", product: "filesquid" }),
  );
  await page.route("**/api/v1/host", (route) =>
    json(route, {
      schema_version: 1,
      host_type: "desktop",
      operator_name: null,
      hosted_service: {
        enrolled: false,
        backup_enabled: false,
        sync_enabled: false,
        service_host: null,
      },
    }),
  );
  await page.routeWebSocket(/\/api\/v1\/events$/, (ws) => void host.sockets.push(ws));
}

/** The storage block's reads and the preferences, which start as `initial` over the defaults. */
async function serveSettings(page: Page, initial: Partial<UserPreferences>): Promise<void> {
  const recent = {
    libraries: [
      {
        library_id: LIBRARY_ID,
        name: "Family archive",
        last_opened: "2026-10-16T00:00:00Z",
        open: true,
        free_bytes: 500_000_000_000,
        free_bytes_reason: null,
      },
    ],
  };
  await page.route(/\/api\/v1\/libraries\/recent$/, (route) => json(route, recent));
  await page.route(/\/api\/v1\/files\/library-history-size\?/, (route) =>
    json(route, {
      library_id: LIBRARY_ID,
      objects: 13,
      contents: 20,
      bytes: 4_509_715_661,
      truncated: false,
    }),
  );
  let stored: UserPreferences = { ...DEFAULT_PREFERENCES, ...initial };
  await page.route(/\/api\/v1\/preferences$/, (route) => {
    if (route.request().method() === "PUT")
      stored = route.request().postDataJSON() as UserPreferences;
    return json(route, stored);
  });
}

/**
 * Serve the host.
 *
 * @param page - The page under test, not yet navigated.
 * @param catalogue - The seeded catalogue to answer from.
 * @param preferences - Preferences the host holds at the start, over the defaults.
 * @returns What the fake saw, and its switches.
 */
export async function serveHost(
  page: Page,
  catalogue: Catalogue,
  preferences: Partial<UserPreferences> = {},
): Promise<Host> {
  const calls = new Map<string, number>();
  const unserved: string[] = [];
  let delay = 0;
  let inFlight = 0;
  let highest = 0;
  const host: Host = {
    catalogue,
    calls,
    viewRequests: [],
    unserved,
    sockets: [],
    maxInFlight: () => highest,
    setDelay: (ms) => void (delay = ms),
    pushNavigationChange: () => {
      const event = { topic: "navigation.change", data: { parents: ["l1"] } };
      for (const socket of host.sockets) socket.send(JSON.stringify(event));
    },
  };
  page.on("request", (request) => {
    if (!request.url().includes("/api/v1/")) return;
    inFlight += 1;
    highest = Math.max(highest, inFlight);
  });
  const settled = (request: { url(): string }) => {
    if (request.url().includes("/api/v1/")) inFlight = Math.max(0, inFlight - 1);
  };
  page.on("requestfinished", settled);
  page.on("requestfailed", settled);
  const count = (name: string) => calls.set(name, (calls.get(name) ?? 0) + 1);
  const wait = () =>
    delay > 0 ? new Promise<void>((resolve) => setTimeout(resolve, delay)) : undefined;

  // The catch-all is registered first, so every later route takes precedence over it.
  await page.route(/\/api\/v1\//, (route) => {
    unserved.push(route.request().url());
    return json(route, { code: "not_found", message: "not served by the performance host" }, 404);
  });
  await serveShell(page, host);
  await serveSettings(page, preferences);
  await page.route(/\/api\/v1\/navigation(\?|$)/, async (route) => {
    count("navigation");
    const q = new URL(route.request().url()).searchParams;
    await wait();
    try {
      const page_ = catalogue.navigationPage(
        q.get("node"),
        q.get("cursor"),
        Number(q.get("limit") ?? 200),
      );
      return await json(route, page_);
    } catch (error) {
      const { status, body } = refusal(error);
      return json(route, body, status);
    }
  });
  await page.route(/\/api\/v1\/files\/view\?/, async (route) => {
    count("view");
    const q = new URL(route.request().url()).searchParams;
    await wait();
    const query = viewParams(q);
    host.viewRequests.push({
      node: query.node,
      cursor: q.get("cursor"),
      limit: Number(q.get("limit") ?? 200),
    });
    try {
      const answer = catalogue.viewPage(query, q.get("cursor"), Number(q.get("limit") ?? 200));
      return await json(route, {
        node: query.node,
        view: query.view,
        sort: query.sort,
        direction: query.direction,
        folders: query.folders,
        ...answer,
      });
    } catch (error) {
      const { status, body } = refusal(error);
      return json(route, body, status);
    }
  });
  await page.route(/\/api\/v1\/files\/search\?/, async (route) => {
    count("search");
    const q = new URL(route.request().url()).searchParams;
    await wait();
    const query = searchParams(q);
    try {
      const answer = catalogue.searchPage(query, q.get("cursor"), Number(q.get("limit") ?? 200));
      return await json(route, {
        q: query.q,
        node: query.node ?? null,
        sort: query.sort,
        direction: query.direction,
        folders: query.folders,
        ...answer,
      });
    } catch (error) {
      const { status, body } = refusal(error);
      return json(route, body, status);
    }
  });
  return host;
}
