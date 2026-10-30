/**
 * A fake of the search operation for the production-build Playwright tests (UI-F2.9): the page is
 * the real built UI, `GET /api/v1/files/search` is answered from a `MemorySearchSource` in the
 * contract's own shape (`SearchPage`: the rows of a view page plus `q`, `node`, `sort`,
 * `direction`, `folders`), refusing as the desktop host does (503 `unavailable` for an operator
 * it cannot answer, 400 `invalid_request` for an unknown one). Each call is recorded, so the tests
 * prove what the page sent.
 */
import type { Page } from "@playwright/test";
import { ApiError } from "../../../src/api/client";
import type { DriveRow } from "../../../src/drive/model";
import type { FixtureNode } from "../../../src/navigation/source";
import { MemorySearchSource } from "../../../src/search/memory-source";
import { refusalOf } from "../../../src/search/model";
import type { SearchQuery } from "../../../src/search/source";
import { LIBRARY_ID, viewRow } from "./drive-api";

/** The library node of the fixture tree and its name. */
export const LIBRARY_NODE = "l1";
/** The library's name, as the scope chip words it. */
export const LIBRARY_NAME = "Family archive";
/** The collection mockup 20 shows. */
export const HARBOUR_NODE = "c-harbour";

/** The repository, one library "Family archive" and the collections of mockup 20. */
export const ARCHIVE: FixtureNode = {
  node: "r",
  kind: "repository",
  name: null,
  children: [
    {
      node: LIBRARY_NODE,
      kind: "library",
      name: LIBRARY_NAME,
      children: [
        { node: "c-docs", kind: "collection", name: "Documents" },
        { node: HARBOUR_NODE, kind: "collection", name: "Harbour project" },
        { node: "c-photos", kind: "collection", name: "Photos 2026" },
        { node: "c-rec", kind: "collection", name: "Recordings" },
        { node: "c-tax", kind: "collection", name: "Taxes" },
        { node: "v-best", kind: "virtual_collection", name: "Best of 2026" },
      ],
    },
  ],
};

const file = (
  id: string,
  name: string,
  collection: string,
  size: number,
  modified: string,
  mediaType: string,
) =>
  ({
    key: id,
    kind: "file",
    name,
    fileId: id,
    objectId: `urn:uuid:${id}`,
    mediaType,
    size,
    modified,
    collection,
  }) as DriveRow;

/** The files of the fixture library: three PDFs that match "harbour", and others. */
export const FILES: readonly DriveRow[] = [
  file(
    "f1",
    "harbour-masterplan-v7.pdf",
    "Harbour project",
    19_293_798,
    "2026-10-16T10:00:00Z",
    "application/pdf",
  ),
  file(
    "f2",
    "harbour-2025-report.pdf",
    "Harbour project",
    2_000_000,
    "2026-09-18T10:00:00Z",
    "application/pdf",
  ),
  file(
    "f3",
    "harbour-walk-permit.pdf",
    "Documents",
    100_000,
    "2026-09-20T10:00:00Z",
    "application/pdf",
  ),
  file("f4", "facade-study.jpg", "Photos 2026", 6_500_000, "2026-10-15T08:00:00Z", "image/jpeg"),
];

/** What the fake saw. */
export interface SearchApi {
  /** The rows served; the in-memory source answers as the operation does. */
  readonly source: MemorySearchSource;
  /** Every search call's query, cursor and limit, in order. */
  readonly calls: { query: SearchQuery; cursor: string | null; limit: number }[];
  /** Milliseconds every answer waits, to hold the loading state on screen. */
  delayMs: number;
}

/** The message of a refusal, naming the operator as API-S-012 requires. */
function refusalMessage(error: unknown, q: string): string {
  const refusal = refusalOf(error, q);
  if (refusal?.kind === "unavailable") {
    return `The search operator ${refusal.operator} is not available on this host.`;
  }
  if (refusal) return `The search operator ${refusal.operator} is unknown.`;
  return "The search was refused.";
}

/** The query the page sent, as the search operation reads it. */
function queryOf(url: string): { query: SearchQuery; cursor: string | null; limit: number } {
  const params = new URL(url).searchParams;
  return {
    query: {
      q: params.get("q") ?? "",
      node: params.get("node") ?? undefined,
      sort: (params.get("sort") ?? "relevance") as SearchQuery["sort"],
      direction: (params.get("direction") ?? "descending") as SearchQuery["direction"],
      folders: (params.get("folders") ?? "on_top") as SearchQuery["folders"],
      zone: params.get("zone") ?? undefined,
    },
    cursor: params.get("cursor"),
    limit: Number(params.get("limit") ?? "200"),
  };
}

/**
 * Serve `GET /api/v1/files/search`.
 *
 * @param page - The page under test.
 * @param rows - The rows of the library node (default {@link FILES}).
 * @returns What the fake saw, and its switches.
 */
export async function serveSearch(
  page: Page,
  rows: readonly DriveRow[] = FILES,
): Promise<SearchApi> {
  const source = new MemorySearchSource(new Map([[LIBRARY_NODE, rows]]), { libraryId: LIBRARY_ID });
  const api: SearchApi = { source, calls: [], delayMs: 0 };
  await page.route(/\/api\/v1\/files\/search\?/, async (route) => {
    const { query, cursor, limit } = queryOf(route.request().url());
    api.calls.push({ query, cursor, limit });
    if (api.delayMs > 0) await new Promise((resolve) => setTimeout(resolve, api.delayMs));
    try {
      const answer = await source.search(query, cursor, limit);
      await route.fulfill({
        json: {
          q: query.q,
          node: query.node ?? null,
          sort: query.sort,
          direction: query.direction,
          folders: query.folders,
          rows: answer.rows.map(viewRow),
          next_cursor: answer.nextCursor,
          complete: answer.complete,
        },
      });
    } catch (error) {
      const status = error instanceof ApiError ? error.status : 500;
      const code = error instanceof ApiError ? (error.code ?? "internal") : "internal";
      await route.fulfill({
        status,
        json: { code, message: refusalMessage(error, query.q), retryable: code === "unavailable" },
      });
    }
  });
  return api;
}

/** The shell's health, host and event channel, answered so only the build is under test. */
export async function serveShell(page: Page): Promise<void> {
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
