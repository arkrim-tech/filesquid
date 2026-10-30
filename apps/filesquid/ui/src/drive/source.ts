/**
 * What the Drive-style views read (UI-F2.8): the {@link ViewSource} interface, shaped on the
 * view operation of API-S-011 (`GET /api/v1/files/view`), and {@link ClientViewSource}, which
 * answers it through `FileSquidClient`. The elements never fetch by themselves, so a fixture
 * ({@link MemoryViewSource}) stands in for the API in tests, and UI-F2.9's search results reuse
 * the same {@link DrivePage}.
 *
 * Each page is one API call; the cursor is bound to the query and to the rows, so a changed
 * view answers 409 `conflict` and the caller starts again without a cursor.
 */
import type { FileSquidClient } from "../api/client";
import type { components } from "../api/schema";
import { driveRow, type DrivePage, type ViewPage, type ViewQuery, type ViewRow } from "./model";
import { VIEW_PAGE_SIZE } from "./model";

/** Cancels a call that is no longer needed. */
export interface ViewCall {
  /** Cancels the call. */
  readonly signal?: AbortSignal;
}

/** Everything the views ask of the API. */
export interface ViewSource {
  /**
   * Read one page of a view. Reads.
   *
   * @param query The node, view, sort, folder placement and filter.
   * @param cursor The previous page's `nextCursor`, or null for the first page.
   * @param limit Rows per page, 1 to 1,000.
   * @param call Abort signal.
   * @returns The page.
   */
  view(
    query: ViewQuery,
    cursor?: string | null,
    limit?: number,
    call?: ViewCall,
  ): Promise<DrivePage>;
}

/** The part of `FileSquidClient` the source needs (injectable in tests). */
export type ViewClient = Pick<FileSquidClient, "request">;

/** The view operation (API-S-011). */
const VIEW_PATH = "/api/v1/files/view";

/** True for a plain object. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** True for a usable view row; unknown members are ignored. */
function validRow(row: unknown): row is ViewRow {
  if (!isRecord(row)) return false;
  if (row.kind === "folder") {
    return typeof row.node === "string" && typeof row.name === "string" && !!row.node_kind;
  }
  return (
    row.kind === "file" &&
    typeof row.file_id === "string" &&
    typeof row.object_id === "string" &&
    typeof row.display_name === "string" &&
    typeof row.media_type === "string" &&
    typeof row.size === "number" &&
    typeof row.modified === "string"
  );
}

/**
 * Check one answer of the view operation and turn it into a {@link DrivePage}. Pure.
 *
 * @param value The parsed answer.
 * @param libraryId The library the rows belong to, when known (the rows do not name it).
 * @returns The page.
 * @throws TypeError when the answer breaks the contract.
 */
export function parseViewPage(value: unknown, libraryId?: string): DrivePage {
  if (!isRecord(value) || !Array.isArray(value.rows)) {
    throw new TypeError(`${VIEW_PATH} answered a value that breaks the contract`);
  }
  const answer = value as unknown as ViewPage;
  if (!answer.rows.every(validRow)) {
    throw new TypeError(`${VIEW_PATH} answered a row that breaks the contract`);
  }
  const cursor = answer.next_cursor;
  return {
    rows: answer.rows.map((row) => driveRow(row, libraryId)),
    nextCursor: typeof cursor === "string" && cursor !== "" ? cursor : null,
    complete: answer.complete !== false,
  };
}

/** The query string of a view request: only what differs from the API's defaults is sent. */
function viewQuery(query: ViewQuery, cursor: string | null, limit: number): string {
  const params = new URLSearchParams({ node: query.node, view: query.view });
  params.set("sort", query.sort);
  params.set("direction", query.direction);
  params.set("folders", query.folders);
  if (query.filter !== "") params.set("filter", query.filter);
  if (cursor) params.set("cursor", cursor);
  params.set("limit", String(limit));
  return `?${params.toString()}`;
}

/** A {@link ViewSource} over `FileSquidClient` and the view operation. */
export class ClientViewSource implements ViewSource {
  /**
   * @param client The API client (its transport adapter and token are already chosen).
   * @param libraryOf Says which library the rows belong to, since the view rows do not (see
   *   `singleOpenLibrary`); without it a row's library stays unknown.
   */
  constructor(
    private readonly client: ViewClient,
    private readonly libraryOf?: () => Promise<string | undefined>,
  ) {}

  /**
   * Read one page with `GET /api/v1/files/view`. Network I/O.
   *
   * @param query The node, view, sort, folder placement and filter.
   * @param cursor The previous page's `nextCursor`, or null for the first page.
   * @param limit Rows per page (default 200).
   * @param call Abort signal.
   * @returns The page.
   * @throws ApiError when the API refuses: 400 for a view that is not available yet, 404 for an
   *   unknown node, 409 `conflict` for a cursor of a view that changed.
   * @throws TypeError when the answer breaks the contract.
   */
  async view(
    query: ViewQuery,
    cursor: string | null = null,
    limit: number = VIEW_PAGE_SIZE,
    call: ViewCall = {},
  ): Promise<DrivePage> {
    const path = `${VIEW_PATH}${viewQuery(query, cursor, limit)}`;
    const answer = await this.client.request<unknown>(path, { signal: call.signal });
    return parseViewPage(answer, await this.libraryOf?.());
  }
}

/** The kinds of folder a view row may be (re-exported for fixtures). */
export type ViewFolderKind = components["schemas"]["ViewFolderKind"];
