/**
 * What the global search reads (UI-F2.9): the {@link SearchSource} interface, shaped on the
 * search operation of API-S-012 (`GET /api/v1/files/search`), and {@link ClientSearchSource},
 * which answers it through `FileSquidClient`. The elements never fetch by themselves, so a
 * fixture ({@link MemorySearchSource}) stands in for the API in tests. A search answers the
 * same rows as a view, so a page is the {@link DrivePage} the Drive list already shows.
 *
 * Each page is one API call; the cursor is bound to the query and to the rows, so a changed
 * result answers 409 `conflict` and the caller starts again without a cursor. A refusal is the
 * host's answer, never a search run without the operator: 400 `invalid_request` for an unknown
 * operator or a bound, 503 `unavailable` for an operator the host cannot answer.
 */
import type { FileSquidClient } from "../api/client";
import type { components } from "../api/schema";
import { VIEW_PAGE_SIZE, type DrivePage } from "../drive/model";
import { parseViewPage, type ViewCall } from "../drive/source";
import type { FolderPlacement, SortDirection } from "../preferences/model";

/** What a search is ordered by: relevance, or a view sort key (generated from the contract). */
export type SearchSort = components["schemas"]["SearchSort"];

/** What a search is asked for. */
export interface SearchQuery {
  /** The query: free text and operators (at most 1,024 bytes and 32 terms). */
  readonly q: string;
  /** The library, collection or virtual-collection node searched; the repository when absent. */
  readonly node?: string;
  /** The order: relevance, or a view sort key. */
  readonly sort: SearchSort;
  /** The direction; descending is relevance's own and the default of the date keys. */
  readonly direction: SortDirection;
  /** Where folder rows go. */
  readonly folders: FolderPlacement;
  /** The caller's zone for date boundaries: `Z` or a fixed offset such as `-08:00`. */
  readonly zone?: string;
}

/** Everything the search asks of the API. */
export interface SearchSource {
  /**
   * Read one page of results. Reads.
   *
   * @param query The query, scope, order, folder placement and zone.
   * @param cursor The previous page's `nextCursor`, or null for the first page.
   * @param limit Rows per page, 1 to 1,000.
   * @param call Abort signal.
   * @returns The page.
   */
  search(
    query: SearchQuery,
    cursor?: string | null,
    limit?: number,
    call?: ViewCall,
  ): Promise<DrivePage>;
}

/** The part of `FileSquidClient` the source needs (injectable in tests). */
export type SearchClient = Pick<FileSquidClient, "request">;

/** The search operation (API-S-012). */
const SEARCH_PATH = "/api/v1/files/search";

/** The most rows one page may ask for (API-S-012 follows the view's bound). */
const MAX_SEARCH_LIMIT = 1000;

/**
 * The query string of a search request: `q`, `folders` and `limit` are always sent; the node and
 * the zone when there are some; the sort and direction only for a view sort key (relevance is the
 * default); the cursor only after the first page.
 */
function searchParams(query: SearchQuery, cursor: string | null, limit: number): string {
  const params = new URLSearchParams({ q: query.q });
  if (query.node !== undefined) params.set("node", query.node);
  if (query.sort !== "relevance") {
    params.set("sort", query.sort);
    params.set("direction", query.direction);
  }
  params.set("folders", query.folders);
  if (query.zone !== undefined) params.set("zone", query.zone);
  if (cursor) params.set("cursor", cursor);
  params.set("limit", String(limit));
  return `?${params.toString()}`;
}

/** A {@link SearchSource} over `FileSquidClient` and the search operation. */
export class ClientSearchSource implements SearchSource {
  /**
   * @param client The API client (its transport adapter and token are already chosen).
   * @param libraryOf Says which library the rows belong to, since the rows do not (see
   *   `singleOpenLibrary`); without it a row's library stays unknown.
   */
  constructor(
    private readonly client: SearchClient,
    private readonly libraryOf?: () => Promise<string | undefined>,
  ) {}

  /**
   * Read one page with `GET /api/v1/files/search`. Network I/O.
   *
   * @param query The query, scope, order, folder placement and zone.
   * @param cursor The previous page's `nextCursor`, or null for the first page.
   * @param limit Rows per page (default 200).
   * @param call Abort signal.
   * @returns The page.
   * @throws RangeError when `limit` is not an integer from 1 to 1,000 (nothing is sent).
   * @throws ApiError when the API refuses: 400 for an unknown operator or a bound, 404 for an
   *   unreadable scope, 409 `conflict` for a cursor of results that changed, 503 for an operator
   *   the host cannot answer.
   * @throws TypeError when the answer breaks the contract.
   */
  async search(
    query: SearchQuery,
    cursor: string | null = null,
    limit: number = VIEW_PAGE_SIZE,
    call: ViewCall = {},
  ): Promise<DrivePage> {
    if (!Number.isInteger(limit) || limit < 1 || limit > MAX_SEARCH_LIMIT) {
      throw new RangeError(`A search page holds 1 to ${MAX_SEARCH_LIMIT} rows, not ${limit}`);
    }
    const path = `${SEARCH_PATH}${searchParams(query, cursor, limit)}`;
    const answer = await this.client.request<unknown>(path, { signal: call.signal });
    return parseViewPage(answer, await this.libraryOf?.(), SEARCH_PATH);
  }
}
