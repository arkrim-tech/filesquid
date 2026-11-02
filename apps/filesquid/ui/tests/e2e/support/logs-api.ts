/**
 * A fake desktop host for the Logs view's production-build Playwright tests (UI-F2.16; API-S-030,
 * API-S-031): the page is the real built UI, and these routes answer as the desktop host does:
 * the host descriptor (desktop, so the sidebar shows Logs), `GET /api/v1/libraries/recent` (one
 * remembered, not open, library, so the storage block asks nothing more), the journal routes over
 * the entries of mockup 38 (`GET /api/v1/admin/journal` with the contract's filters, cursor, order
 * and `names`, `GET .../entries/{id}`, `GET .../export` in JSON Lines and CSV with
 * `X-Journal-Next-Cursor` when a response is cut) and `POST /api/v1/client-events`, which
 * acknowledges every event. Live entries are pushed over the event channel. Every call is
 * recorded, so a test asserts what the page asked and what it did not ask.
 */
import type { Page, WebSocketRoute } from "@playwright/test";
import type { JournalEntryView, JournalFilters } from "../../../src/api/client";
import { serveNavigation } from "./navigation-api";
import { LIBRARY, csvOf, journalPage, mockupEntries, matchesRoute } from "../../support/logs-fakes";

export { LIBRARY };

/** The desktop host's descriptor. */
const DESKTOP = {
  schema_version: 1,
  host_type: "desktop",
  operator_name: null,
  hosted_service: {
    enrolled: false,
    backup_enabled: false,
    sync_enabled: false,
    service_host: null,
  },
};

/** What the page asked, and ways to steer the fake. */
export interface LogsApi {
  /** The journal, in any order; a test may push entries. */
  entries: JournalEntryView[];
  /** The query string of every page request, as filters. */
  readonly pageQueries: JournalFilters[];
  /** The query string of every export request, as parsed text. */
  readonly exportQueries: Record<string, string>[];
  /** The id of every single-entry read. */
  readonly entryReads: number[];
  /** Every `POST /api/v1/client-events` body. */
  readonly clientEvents: { events: Record<string, unknown>[] }[];
  /** Answer the journal routes with this status and code instead (a caller who is not the owner). */
  deny(status: number, code: string): void;
  /** Give every first page a cursor, as if older entries existed, though none do. */
  forceMore: boolean;
  /** Entries per export response, to cut an export (CSV header test); the route's cap when 0. */
  exportChunk: number;
  /** Push a live `journal.entry` event. */
  emit(entry: JournalEntryView): void;
  /** Push any event. */
  emitTopic(topic: string, data?: unknown): void;
  /** Resolves when the page's event channel is connected. */
  readonly connected: Promise<void>;
}

/** The numeric filters and text filters of a request URL, as the contract names them. */
function filtersOf(url: URL): JournalFilters {
  const q = url.searchParams;
  const text = (name: string) => q.get(name) ?? undefined;
  const filters: Record<string, unknown> = {};
  for (const name of [
    "from",
    "to",
    "actor",
    "operation",
    "outcome",
    "level",
    "process",
    "library",
    "object",
    "file",
    "operation_id",
    "order",
    "format",
  ]) {
    const value = text(name);
    if (value !== undefined) filters[name] = value;
  }
  if (q.has("after")) filters["after"] = Number(q.get("after"));
  if (q.has("limit")) filters["limit"] = Number(q.get("limit"));
  if (q.get("names") === "true") filters["names"] = true;
  return filters as JournalFilters;
}

/** A JSON error body of the contract. */
function refusal(status: number, code: string) {
  return { status, json: { code, message: "refused", retryable: status >= 500 } };
}

/**
 * Serve the shell, the libraries, the journal routes and the client-events route.
 *
 * @param page - The page under test.
 * @param entries - The journal; the sixteen entries of mockup 38 by default.
 * @returns What the fake saw, and ways to steer it.
 */
export async function serveLogs(
  page: Page,
  entries: JournalEntryView[] = mockupEntries(),
): Promise<LogsApi> {
  const sockets: WebSocketRoute[] = [];
  let denied: { status: number; code: string } | undefined;
  let markConnected = (): void => undefined;
  const connected = new Promise<void>((resolve) => {
    markConnected = resolve;
  });
  const api: LogsApi = {
    entries,
    pageQueries: [],
    exportQueries: [],
    entryReads: [],
    clientEvents: [],
    deny: (status, code) => void (denied = { status, code }),
    forceMore: false,
    exportChunk: 0,
    emit: (entry) => {
      const { provenance: _p, display_name: _n, ...live } = entry;
      void _p;
      void _n;
      api.emitTopic("journal.entry", live);
    },
    emitTopic: (topic, data = {}) =>
      sockets.forEach((socket) => socket.send(JSON.stringify({ topic, data }))),
    connected,
  };
  await page.route("**/api/v1/health", (route) =>
    route.fulfill({ json: { schema_version: 1, status: "ok", product: "filesquid" } }),
  );
  await page.route(/\/api\/v1\/host$/, (route) => route.fulfill({ json: DESKTOP }));
  await page.routeWebSocket(/\/api\/v1\/events$/, (socket) => {
    sockets.push(socket);
    markConnected();
  });
  await page.route(/\/api\/v1\/libraries\/recent$/, (route) =>
    route.fulfill({
      json: {
        libraries: [
          {
            library_id: LIBRARY,
            name: "Family archive",
            last_opened: "2026-10-19T09:00:00Z",
            open: false,
            free_bytes: null,
            free_bytes_reason: null,
          },
        ],
      },
    }),
  );
  await page.route(/\/api\/v1\/client-events$/, async (route) => {
    const body = route.request().postDataJSON() as { events: Record<string, unknown>[] };
    api.clientEvents.push(body);
    await route.fulfill({
      json: {
        acknowledged: body.events.map((event) => event["id"]),
        journaled: body.events.length,
        principal: "urn:uuid:5a0c41e2-0000-4000-8000-00000000e19b",
      },
    });
  });
  await serveNavigation(page);
  await serveJournal(page, api, () => denied);
  return api;
}

/** The journal routes: the page, one entry and the export. */
async function serveJournal(
  page: Page,
  api: LogsApi,
  denied: () => { status: number; code: string } | undefined,
): Promise<void> {
  await page.route(/\/api\/v1\/admin\/journal\/entries\/\d+/, (route) => {
    const refused = denied();
    if (refused) return route.fulfill(refusal(refused.status, refused.code));
    const id = Number(new URL(route.request().url()).pathname.split("/").pop());
    api.entryReads.push(id);
    const found = api.entries.find((entry) => entry.id === id);
    if (!found) return route.fulfill(refusal(404, "not_found"));
    return route.fulfill({ json: found });
  });
  await page.route(/\/api\/v1\/admin\/journal\/export/, (route) => {
    const refused = denied();
    if (refused) return route.fulfill(refusal(refused.status, refused.code));
    const url = new URL(route.request().url());
    api.exportQueries.push(Object.fromEntries(url.searchParams));
    return route.fulfill(exportAnswer(api, filtersOf(url)));
  });
  await page.route(/\/api\/v1\/admin\/journal(\?|$)/, (route) => {
    const refused = denied();
    if (refused) return route.fulfill(refusal(refused.status, refused.code));
    const filters = filtersOf(new URL(route.request().url()));
    api.pageQueries.push(filters);
    const answer = journalPage(api.entries, filters);
    if (api.forceMore && filters.after === undefined && answer.next_cursor === null) {
      const last = answer.entries[answer.entries.length - 1]?.id ?? null;
      return route.fulfill({ json: { ...answer, next_cursor: last } });
    }
    return route.fulfill({ json: answer });
  });
}

/** One export response: the entries after the cursor, cut at the chunk size with the next cursor. */
function exportAnswer(api: LogsApi, filters: JournalFilters) {
  const ordered = api.entries
    .filter((entry) => matchesRoute(entry, filters))
    .sort((a, b) => (a.id ?? 0) - (b.id ?? 0))
    .filter((entry) => filters.after === undefined || (entry.id ?? 0) > filters.after);
  const chunk = api.exportChunk > 0 ? api.exportChunk : 50_000;
  const part = ordered.slice(0, chunk).map((entry) => {
    const { display_name: _displayName, provenance: _provenance, ...rest } = entry;
    void _displayName;
    void _provenance;
    return rest;
  });
  const next = ordered.length > chunk ? part[part.length - 1]?.id : undefined;
  const csv = filters.format === "csv";
  const body = csv
    ? csvOf(part, true)
    : part.map((entry) => JSON.stringify(entry)).join("\n") + (part.length > 0 ? "\n" : "");
  return {
    status: 200,
    contentType: csv ? "text/csv" : "application/x-ndjson",
    headers: (next === undefined ? {} : { "X-Journal-Next-Cursor": String(next) }) as Record<
      string,
      string
    >,
    body,
  };
}
