/**
 * An in-memory {@link ViewSource} that answers as the view operation does (API-S-011): the
 * views of a node, a stable order with ties broken by name then key, a missing value sorted
 * after every value in both directions, folder placement, a view-local filter, cursors bound to
 * the parameters and to the rows (a changed view answers 409 `conflict`, other parameters 400),
 * 404 for an unknown node and 400 for a view the host does not serve yet. It is the fixture of
 * the unit tests and of the production-build tests' fake API.
 */
import { ApiError } from "../api/client";
import type { ViewKind } from "../preferences/model";
import { VIEW_PAGE_SIZE, type DrivePage, type DriveRow, type ViewQuery } from "./model";
import type { ViewCall, ViewSource } from "./source";

/** The rows of one node, by view; a view not listed is "not available yet" (400). */
export type NodeViews = Partial<Record<ViewKind, readonly DriveRow[]>>;

/** One recorded call. */
export interface ViewCallRecord {
  /** The query. */
  readonly query: ViewQuery;
  /** The cursor sent. */
  readonly cursor: string | null;
  /** The page size. */
  readonly limit: number;
}

const PATH = "/api/v1/files/view";

/** A value that sorts after every value in both directions when missing. */
type Sortable = number | string | undefined | null;

/** Compare two present values. */
function compareValues(a: number | string, b: number | string): number {
  if (typeof a === "number" && typeof b === "number") return a - b;
  return String(a).localeCompare(String(b));
}

/** The sort value of a row for a key. */
function sortValue(row: DriveRow, key: ViewQuery["sort"]): Sortable {
  switch (key) {
    case "name":
      return row.name.toLowerCase();
    case "size":
      return row.kind === "file" ? row.size : undefined;
    case "modified":
      return row.modified;
    case "modified_by_me":
      return row.modifiedByMe;
    default:
      return row.openedByMe;
  }
}

/** Compare two rows by the query's sort, a missing value last in both directions. */
function compareRows(a: DriveRow, b: DriveRow, query: ViewQuery): number {
  const x = sortValue(a, query.sort);
  const y = sortValue(b, query.sort);
  const sign = query.direction === "ascending" ? 1 : -1;
  const missingX = x === undefined || x === null;
  const missingY = y === undefined || y === null;
  if (missingX !== missingY) return missingX ? 1 : -1;
  if (!missingX && !missingY && x !== y) return sign * compareValues(x, y);
  const byName = a.name.toLowerCase().localeCompare(b.name.toLowerCase());
  return byName !== 0 ? byName : a.key.localeCompare(b.key);
}

/** The parameters a cursor is bound to. */
function binding(query: ViewQuery): string {
  return JSON.stringify([
    query.node,
    query.view,
    query.sort,
    query.direction,
    query.folders,
    query.filter,
  ]);
}

/** An in-memory view source. */
export class MemoryViewSource implements ViewSource {
  /** Every call, in order. */
  readonly calls: ViewCallRecord[] = [];
  private revision = 0;

  /**
   * @param nodes The views of each node, by node handle.
   * @param libraryId The library the rows belong to, when the fixture should report it.
   */
  constructor(
    private readonly nodes: Map<string, NodeViews>,
    private readonly libraryId?: string,
  ) {}

  /**
   * Replace the rows of a node's view, so cursors made before it answer 409.
   *
   * @param node The node.
   * @param view The view.
   * @param rows The new rows.
   */
  replace(node: string, view: ViewKind, rows: readonly DriveRow[]): void {
    const views = this.nodes.get(node) ?? {};
    this.nodes.set(node, { ...views, [view]: rows });
    this.revision += 1;
  }

  /**
   * Read one page.
   *
   * @param query The query.
   * @param cursor The cursor of the previous page.
   * @param limit Rows per page.
   * @param call Abort signal.
   * @returns The page.
   * @throws ApiError 404 for an unknown node, 400 for an unavailable view, an unusable limit or
   *   a cursor of other parameters, 409 `conflict` for a cursor of rows that changed.
   */
  view(
    query: ViewQuery,
    cursor: string | null = null,
    limit: number = VIEW_PAGE_SIZE,
    call: ViewCall = {},
  ): Promise<DrivePage> {
    this.calls.push({ query, cursor, limit });
    try {
      call.signal?.throwIfAborted();
      return Promise.resolve(this.page(query, cursor, limit));
    } catch (error) {
      return Promise.reject(error);
    }
  }

  /** The page, or the API's refusal. */
  private page(query: ViewQuery, cursor: string | null, limit: number): DrivePage {
    const views = this.nodes.get(query.node);
    if (!views) throw new ApiError(404, PATH, { code: "not_found" });
    const rows = views[query.view];
    if (!rows || limit < 1 || limit > 1000) {
      throw new ApiError(400, PATH, { code: "invalid_request" });
    }
    const wanted = rows
      .filter((row) => row.name.toLowerCase().includes(query.filter.toLowerCase()))
      .map((row) => ({ ...row, libraryId: row.libraryId ?? this.libraryId }));
    const ordered = this.arrange(wanted, query);
    const start = this.offset(cursor, query);
    const end = Math.min(start + limit, ordered.length);
    const next = end < ordered.length ? `${end}:${this.revision}:${binding(query)}` : null;
    return { rows: ordered.slice(start, end), nextCursor: next, complete: true };
  }

  /** Sort the rows and place the folders. */
  private arrange(rows: readonly DriveRow[], query: ViewQuery): DriveRow[] {
    const sorted = [...rows].sort((a, b) => compareRows(a, b, query));
    if (query.folders === "mixed") return sorted;
    return [
      ...sorted.filter((r) => r.kind === "folder"),
      ...sorted.filter((r) => r.kind === "file"),
    ];
  }

  /** The row a cursor starts at; 0 without a cursor. */
  private offset(cursor: string | null, query: ViewQuery): number {
    if (cursor === null) return 0;
    const [offset, revision, ...rest] = cursor.split(":");
    if (rest.join(":") !== binding(query))
      throw new ApiError(400, PATH, { code: "invalid_request" });
    if (Number(revision) !== this.revision) throw new ApiError(409, PATH, { code: "conflict" });
    return Number(offset);
  }
}
