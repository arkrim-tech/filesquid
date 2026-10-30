/**
 * An in-memory {@link SearchSource} that answers as the search operation does (API-S-012) for the
 * grammar the default UI offers: free text and quoted phrases matched in the display name,
 * `-term` exclusion, `type:`, `in:`, `title:`, `before:`, `after:`, `size>` and `size<`, and
 * `is:deleted`. Free text and `title:` stand for the name here (the fixture rows carry no
 * metadata); the operators of the grammar that need metadata the fixture lacks (`creator:`,
 * `tag:`, `source:`, `createdbefore:`, `createdafter:`, `owner:me`) match nothing. It refuses what
 * the desktop host refuses: 503 `unavailable` for `is:favorite`, `owner:` of another user and the
 * server's operators, 400 `invalid_request` for an unknown operator, a value that does not parse
 * and an unclosed quote, 404 for an unknown scope. Relevance adds 3 for each free-text term the
 * name holds; any view sort key replaces it; ties break by name then key; cursors are bound to the
 * query and to the rows. It is the fixture of the unit tests and of the production-build tests'
 * fake API.
 */
import { ApiError } from "../api/client";
import { VIEW_PAGE_SIZE, type DrivePage, type DriveRow } from "../drive/model";
import type { ViewCall } from "../drive/source";
import { MAX_QUERY_BYTES, isTypeClass, tokenizeQuery, typeClassOf } from "./model";
import type { QueryToken } from "./model";
import type { SearchQuery, SearchSource } from "./source";

/** One recorded call. */
export interface SearchCallRecord {
  /** The query. */
  readonly query: SearchQuery;
  /** The cursor sent. */
  readonly cursor: string | null;
  /** The page size. */
  readonly limit: number;
}

/** What the fixture knows beyond the rows. */
export interface MemorySearchOptions {
  /** The library the rows belong to, when the fixture should report it. */
  readonly libraryId?: string;
  /** Keys of the rows that are deleted: hidden unless `is:deleted` asks for them. */
  readonly deleted?: ReadonlySet<string>;
}

const PATH = "/api/v1/files/search";
const SERVER_OPERATORS = new Set(["sharedwith", "to", "from", "pendingowner"]);
const UNMODELLED = new Set(["creator", "tag", "source", "createdbefore", "createdafter"]);

/** A refusal as the API words it. */
function refuse(status: 400 | 503): ApiError {
  const code = status === 400 ? "invalid_request" : "unavailable";
  return new ApiError(status, PATH, { code });
}

/** Text as search compares it: NFC, lower case, NFC again. */
function fold(text: string): string {
  return text.normalize("NFC").toLowerCase().normalize("NFC");
}

/** The bytes a `size>` value names: an integer and an optional B, KB, MB or GB. */
function bytesOf(value: string): number {
  const match = /^(\d+)(b|kb|mb|gb)?$/.exec(fold(value));
  if (!match) throw refuse(400);
  const scale = { b: 1, kb: 1024, mb: 1024 ** 2, gb: 1024 ** 3 }[match[2] ?? "b"] as number;
  return Number(match[1]) * scale;
}

/** The calendar date `YYYY-MM-DD` of a value, or a refusal. */
function dateOf(value: string): string {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) throw refuse(400);
  return value;
}

/** One clause of a parsed query: whether a row matches it. */
type Clause = (row: DriveRow) => boolean;

/** The clause of `type:`. */
function typeClause(value: string): Clause {
  if (!isTypeClass(value)) throw refuse(400);
  if (value === "folder") return (row) => row.kind === "folder";
  return (row) => row.kind === "file" && typeClassOf(row.mediaType ?? "") === value;
}

/** The offset of a zone (`Z` or `+HH:MM`) in milliseconds; UTC for anything else. */
function zoneMs(zone: string | undefined): number {
  const match = /^([+-])(\d{2}):(\d{2})$/.exec(zone ?? "");
  if (!match) return 0;
  const minutes = Number(match[2]) * 60 + Number(match[3]);
  return (match[1] === "-" ? -minutes : minutes) * 60_000;
}

/** The calendar date of an instant in the caller's zone, as `YYYY-MM-DD`. */
function dayInZone(instant: string, zone: string | undefined): string {
  return new Date(Date.parse(instant) + zoneMs(zone)).toISOString().slice(0, 10);
}

/** The clause of `before:` or `after:` on the day a row was modified in the caller's zone. */
function dayClause(value: string, before: boolean, zone: string | undefined): Clause {
  const day = dateOf(value);
  return (row) => {
    if (row.modified === undefined) return false;
    const modified = dayInZone(row.modified, zone);
    return before ? modified < day : modified >= day;
  };
}

/** The clause of `is:`; only `deleted` is answered here. */
function isClause(value: string, deleted: ReadonlySet<string>): Clause {
  if (value === "favorite") throw refuse(503);
  if (value !== "deleted") throw refuse(400);
  return (row) => deleted.has(row.key);
}

/** The clause of `size>` (`larger`) or `size<`. */
function sizeClause(value: string, larger: boolean): Clause {
  const limit = bytesOf(value);
  return (row) => {
    if (row.kind !== "file") return false;
    return larger ? (row.size ?? 0) > limit : (row.size ?? 0) < limit;
  };
}

/** The clause of an operator token, or the refusal the host gives. */
function operatorClause(
  token: QueryToken,
  deleted: ReadonlySet<string>,
  zone: string | undefined,
): Clause {
  const name = token.operator as string;
  const value = fold(token.value);
  if (SERVER_OPERATORS.has(name)) throw refuse(503);
  if (name === "owner" && value.trim() !== "me") throw refuse(503);
  if (value.trim() === "") throw refuse(400);
  if (name === "type") return typeClause(value);
  if (name === "in") return (row) => row.kind === "file" && fold(row.collection ?? "") === value;
  if (name === "title") return (row) => fold(row.name).includes(value);
  if (name === "before" || name === "after") return dayClause(token.value, name === "before", zone);
  if (name === "is") return isClause(value, deleted);
  if (name === "size>" || name === "size<") return sizeClause(value, name === "size>");
  if (name === "owner" || UNMODELLED.has(name)) return () => false;
  throw refuse(400);
}

/** A parsed query: its clauses and the free-text terms that score. */
interface Parsed {
  readonly clauses: { clause: Clause; negated: boolean }[];
  readonly words: string[];
  readonly wantsDeleted: boolean;
}

/** Parse a query the way the operation does, or refuse it. */
function parseQuery(q: string, deleted: ReadonlySet<string>, zone: string | undefined): Parsed {
  if (q.trim() === "") throw refuse(400);
  const quotes = [...q].filter((c) => c === '"').length;
  if (new TextEncoder().encode(q).length > MAX_QUERY_BYTES || quotes % 2 === 1) throw refuse(400);
  const clauses: Parsed["clauses"] = [];
  const words: string[] = [];
  let wantsDeleted = false;
  for (const token of tokenizeQuery(q)) {
    if (token.operator === undefined) {
      const word = fold(token.value);
      if (word.trim() === "") throw refuse(400);
      clauses.push({ clause: (row) => fold(row.name).includes(word), negated: token.negated });
      if (!token.negated) words.push(word);
      continue;
    }
    if (token.operator === "is" && fold(token.value) === "deleted" && !token.negated) {
      wantsDeleted = true;
    }
    clauses.push({ clause: operatorClause(token, deleted, zone), negated: token.negated });
  }
  return { clauses, words, wantsDeleted };
}

/** The binding a cursor carries: the parameters that decide the rows. */
function binding(query: SearchQuery): string {
  return JSON.stringify([
    query.q,
    query.node,
    query.sort,
    query.direction,
    query.folders,
    query.zone,
  ]);
}

/** A value that sorts after every value in both directions when missing. */
type Sortable = number | string | undefined | null;

/** The sort value of a row for a view sort key. */
function sortValue(row: DriveRow, key: SearchQuery["sort"]): Sortable {
  if (key === "name") return row.name.toLowerCase();
  if (key === "size") return row.kind === "file" ? row.size : undefined;
  if (key === "modified") return row.modified;
  if (key === "modified_by_me") return row.modifiedByMe;
  return row.openedByMe;
}

/** An in-memory search source. */
export class MemorySearchSource implements SearchSource {
  /** Every call, in order. */
  readonly calls: SearchCallRecord[] = [];
  private revision = 0;

  /**
   * @param scopes The rows of each scope (library, collection or virtual collection), by node.
   * @param options The library the rows belong to and which rows are deleted.
   */
  constructor(
    private readonly scopes: Map<string, readonly DriveRow[]>,
    private readonly options: MemorySearchOptions = {},
  ) {}

  /**
   * Replace the rows of a scope, so cursors made before it answer 409 `conflict`.
   *
   * @param node The scope's node.
   * @param rows The new rows.
   */
  replace(node: string, rows: readonly DriveRow[]): void {
    this.scopes.set(node, rows);
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
   * @throws ApiError as the operation refuses (see the module comment).
   */
  search(
    query: SearchQuery,
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
  private page(query: SearchQuery, cursor: string | null, limit: number): DrivePage {
    if (!Number.isInteger(limit) || limit < 1 || limit > 1000) throw refuse(400);
    const scope = this.rowsOf(query.node);
    const deleted = this.options.deleted ?? new Set<string>();
    const parsed = parseQuery(query.q, deleted, query.zone);
    const matched = scope.filter((row) => this.matches(row, parsed, deleted));
    const ordered = this.arrange(matched, parsed, query);
    const start = this.offset(cursor, query);
    const end = Math.min(start + limit, ordered.length);
    const rows = ordered
      .slice(start, end)
      .map((row) => ({ ...row, libraryId: row.libraryId ?? this.options.libraryId }));
    return {
      rows,
      nextCursor: end < ordered.length ? `${end}:${this.revision}:${binding(query)}` : null,
      complete: true,
    };
  }

  /** The rows of a scope: the whole repository when no node is named; 404 for an unknown node. */
  private rowsOf(node: string | undefined): readonly DriveRow[] {
    if (node === undefined) return [...this.scopes.values()].flat();
    const rows = this.scopes.get(node);
    if (!rows) throw new ApiError(404, PATH, { code: "not_found" });
    return rows;
  }

  /** Whether a row is in the result: deleted rows only for `is:deleted`, every clause held. */
  private matches(row: DriveRow, parsed: Parsed, deleted: ReadonlySet<string>): boolean {
    if (deleted.has(row.key) && !parsed.wantsDeleted) return false;
    return parsed.clauses.every(({ clause, negated }) => clause(row) !== negated);
  }

  /** Score, sort and place the folders. */
  private arrange(rows: readonly DriveRow[], parsed: Parsed, query: SearchQuery): DriveRow[] {
    const score = (row: DriveRow) =>
      parsed.words.filter((word) => fold(row.name).includes(word)).length * 3;
    const sign = query.direction === "ascending" ? 1 : -1;
    const compare = (a: DriveRow, b: DriveRow): number => {
      const x = query.sort === "relevance" ? score(a) : sortValue(a, query.sort);
      const y = query.sort === "relevance" ? score(b) : sortValue(b, query.sort);
      const missingX = x === undefined || x === null;
      const missingY = y === undefined || y === null;
      if (missingX !== missingY) return missingX ? 1 : -1;
      if (!missingX && !missingY && x !== y) return sign * (x < y ? -1 : 1);
      const names = fold(a.name) < fold(b.name) ? -1 : fold(a.name) > fold(b.name) ? 1 : 0;
      return names !== 0 ? names : a.key < b.key ? -1 : 1;
    };
    const sorted = [...rows].sort(compare);
    if (query.folders === "mixed") return sorted;
    return [
      ...sorted.filter((r) => r.kind === "folder"),
      ...sorted.filter((r) => r.kind === "file"),
    ];
  }

  /**
   * The row a cursor starts at; 0 without a cursor; 400 for a malformed cursor or one of other
   * parameters, 409 `conflict` for one of rows that changed.
   */
  private offset(cursor: string | null, query: SearchQuery): number {
    if (cursor === null) return 0;
    const match = /^(\d+):(\d+):([\s\S]*)$/.exec(cursor);
    if (!match || match[3] !== binding(query)) throw refuse(400);
    if (Number(match[2]) !== this.revision) throw new ApiError(409, PATH, { code: "conflict" });
    return Number(match[1]);
  }
}
