/**
 * The seeded catalogue behind the browsing performance suite (Task UI-F3.1, NFR-002 profiles).
 *
 * A million assets are never one list: the API bounds every view (10,000 files and 1,000
 * folder rows, 200 rows per page, at most 60 pages; API-S-011) and every navigation level
 * (1,000 nodes per page, at most 50 pages). The UI therefore sees the Small, Medium and Large
 * profiles through those bounds, and each profile fixes the navigation tree (one library with
 * as many collections as the profile has albums) and the worst-case view under test. The
 * content has the mix NFR-002 names: Unicode names (CJK, Arabic, accents composed and
 * decomposed, emoji), one display name on many rows, pairs sharing a stem, file sizes up to a
 * 250 GB archive part and badge classes of every kind.
 *
 * Everything here is a pure function of a seed: the same rows on every run, no clock and no
 * `Math.random`. It answers as the host does (stable order across pages, cursors bound to their
 * query, the API's refusals) and is served to the page by `host.ts`.
 */
import type { components } from "../../src/api/schema";

/** A row of a view page, as the contract shapes it. */
export type ViewRow = components["schemas"]["ViewRow"];

/** The profile names of NFR-002. */
export type ProfileId = "small" | "medium" | "large";

/** One catalog profile as the UI sees it. */
export interface Profile {
  /** Profile name. */
  readonly id: ProfileId;
  /** Assets in the catalog (NFR-002). */
  readonly assets: number;
  /** Resources in the catalog (NFR-002). */
  readonly resources: number;
  /** Albums, so the collections of the one library's navigation level (NFR-002). */
  readonly albums: number;
  /** Memberships, where NFR-002 states them. */
  readonly memberships: number | null;
  /** Collections the navigation lists under the library. */
  readonly navCollections: number;
  /** Files in the scope of the view under test, before the API's cap. */
  readonly viewFiles: number;
  /** Folders in the scope of the view under test, before the API's cap. */
  readonly viewFolders: number;
}

/** The profiles. */
export const PROFILES: Readonly<Record<ProfileId, Profile>> = {
  small: {
    id: "small",
    assets: 10_000,
    resources: 20_000,
    albums: 100,
    memberships: null,
    navCollections: 100,
    viewFiles: 10_000,
    viewFolders: 100,
  },
  medium: {
    id: "medium",
    assets: 100_000,
    resources: 200_000,
    albums: 5_000,
    memberships: null,
    navCollections: 5_000,
    viewFiles: 10_000,
    viewFolders: 1_000,
  },
  large: {
    id: "large",
    assets: 1_000_000,
    resources: 2_000_000,
    albums: 50_000,
    memberships: 10_000_000,
    navCollections: 50_000,
    viewFiles: 12_500,
    viewFolders: 1_200,
  },
};

/** The bounds of the API's views and the UI's paging (API-S-011). */
export const VIEW_CAPS = { files: 10_000, folders: 1_000, pageSize: 200, maxPages: 60 } as const;

/** Most nodes one navigation page holds (API-S-021). */
const NAVIGATION_PAGE_MAX = 1_000;

/** The repository node. */
export const REPOSITORY_NODE = "r";
/** The one library. */
export const LIBRARY_NODE = "l1";
/** The library's name. */
export const LIBRARY_NAME = "Family archive";
/** The collection that holds the view under test. */
export const BIG_NODE = "c-0";
/** The big collection's name. */
export const BIG_NAME = "Archive 2026";

/** What a refused call answers: the HTTP status and the API's error code. */
export class FixtureError extends Error {
  /**
   * @param status HTTP status.
   * @param code The API's error code.
   */
  constructor(
    readonly status: number,
    readonly code: string,
  ) {
    super(`${status} ${code}`);
    this.name = "FixtureError";
  }
}

/** The query of a view read. */
export interface ViewParams {
  /** The node. */
  readonly node: string;
  /** The view; only `all` is served. */
  readonly view: string;
  /** Sort key. */
  readonly sort: string;
  /** Direction. */
  readonly direction: "ascending" | "descending";
  /** Folder placement. */
  readonly folders: "on_top" | "mixed";
  /** Name filter, case-insensitive substring. */
  readonly filter: string;
}

/** The query of a search read. */
export interface SearchParams {
  /** The query text. */
  readonly q: string;
  /** The node searched, when narrowed. */
  readonly node?: string;
  /** Sort key or `relevance`. */
  readonly sort: string;
  /** Direction. */
  readonly direction?: "ascending" | "descending";
  /** Folder placement. */
  readonly folders?: "on_top" | "mixed";
}

/** One page of rows. */
export interface RowPage {
  /** The rows. */
  readonly rows: ViewRow[];
  /** The cursor of the next page, or null on the last. */
  readonly next_cursor: string | null;
  /** False when a cap left rows out. */
  readonly complete: boolean;
}

/** One node of a navigation listing. */
export interface NavNode {
  /** Handle. */
  readonly node: string;
  /** Kind. */
  readonly kind: "repository" | "library" | "collection" | "virtual_collection" | "folder";
  /** Name, or null for the repository. */
  readonly name: string | null;
  /** Whether it has children. */
  readonly has_children: boolean;
}

/** One page of a navigation listing. */
export interface NavPage {
  /** Children of the parent. */
  readonly nodes: NavNode[];
  /** The path to the parent, the parent last. */
  readonly ancestors: NavNode[];
  /** Cursor of the next page. */
  readonly next_cursor: string | null;
}

/** A file or folder as the catalogue holds it. */
interface Entry {
  readonly key: string;
  readonly name: string;
  readonly folded: string;
  readonly folder: boolean;
  readonly mediaType: string;
  readonly size: number;
  readonly modified: string;
}

/** The entries of one scope and whether the API would call the answer whole. */
interface Scope {
  readonly entries: readonly Entry[];
  readonly complete: boolean;
}

/**
 * A deterministic pseudo-random generator (mulberry32).
 *
 * @param seed Any 32-bit integer.
 * @returns A function giving a number in [0, 1) each call.
 */
export function seeded(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Name stems; some are not Latin, and one accent is written composed and one decomposed. */
const STEMS = [
  "harbour-masterplan",
  "survey-points",
  "facade-study",
  "budget",
  "interview",
  "tender-response",
  "meeting-notes",
  "site-walkthrough",
  "drawing",
  "model-export",
  "contract",
  "invoice",
  "東京タワー",
  "الحساب-الختامي",
  "Åre-café",
  "Åre-café",
  "😀-party",
  "ünïcode-täst",
];

/** Extensions and the media type each stands for, across the three badge classes. */
const TYPES: readonly (readonly [string, string])[] = [
  ["pdf", "application/pdf"],
  ["docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document"],
  ["txt", "text/plain"],
  ["xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"],
  ["csv", "text/csv"],
  ["blend", "application/octet-stream"],
  ["gz", "application/gzip"],
  ["jpg", "image/jpeg"],
  ["png", "image/png"],
  ["m4a", "audio/mp4"],
  ["mp4", "video/mp4"],
  ["mov", "video/quicktime"],
  ["cr3", "image/x-canon-cr3"],
];

/** Media type of an extension; unknown ones are plain bytes. */
function mediaTypeOf(extension: string): string {
  return TYPES.find(([ext]) => ext === extension)?.[1] ?? "application/octet-stream";
}

/** A number as four digits, or more when it needs them. */
function pad(value: number): string {
  return String(value).padStart(4, "0");
}

/** The newest modified time of the catalogue: 12:00 UTC on 16 October 2026. */
const NEWEST = Date.UTC(2026, 9, 16, 12, 0, 0);

/**
 * The display name of file `i`: every ninth is the one name `scan.pdf` (a duplicate name on
 * many rows), two of every thirteen are a JPEG beside its RAW of one stem, the rest take a
 * stem and an extension from the tables.
 */
function nameOf(i: number, salt: string): string {
  if (i % 9 === 8) return "scan.pdf";
  if (i % 13 <= 1) return `IMG_${salt}${pad(Math.floor(i / 13))}.${i % 13 === 0 ? "jpg" : "dng"}`;
  const stem = STEMS[(i * 7) % STEMS.length] as string;
  const [extension] = TYPES[(i * 5) % TYPES.length] as readonly [string, string];
  return `${stem}-${salt}${i}.${extension}`;
}

/** The size of file `i`: mostly megabytes, with a 100 GB and a 250 GB resource now and then. */
function sizeOf(i: number, random: () => number): number {
  const base = 1_000 + Math.floor(random() * 50_000_000);
  if (i % 5_000 === 0) return 250_000_000_000;
  if (i % 2_500 === 0) return 100_000_000_000;
  return base;
}

/** The entries of `count` files, named under `prefix` and `salt`. */
function fileEntries(prefix: string, salt: string, count: number, seed: number): Entry[] {
  const random = seeded(seed);
  return Array.from({ length: count }, (_, i) => {
    const name = nameOf(i, salt);
    const extension = name.slice(name.lastIndexOf(".") + 1);
    return {
      key: `${prefix}-${String(i).padStart(8, "0")}`,
      name,
      folded: name.toLowerCase(),
      folder: false,
      mediaType: mediaTypeOf(extension),
      size: sizeOf(i, random),
      modified: new Date(NEWEST - i * 11 * 60 * 1000).toISOString(),
    };
  });
}

/** The name of folder `j` of the big collection: Latin, Japanese and accented albums. */
function folderName(j: number): string {
  if (j % 5 === 0) return `アルバム ${pad(j)}`;
  if (j % 5 === 1) return `Álbum ${pad(j)}`;
  return `Album ${pad(j)}`;
}

/** The entries of `count` folders with the given node-handle prefix. */
function folderEntries(prefix: string, count: number, name: (j: number) => string): Entry[] {
  return Array.from({ length: count }, (_, j) => {
    const label = name(j);
    return {
      key: `${prefix}-${j}`,
      name: label,
      folded: label.toLowerCase(),
      folder: true,
      mediaType: "",
      size: 0,
      modified: "",
    };
  });
}

/** Compare two present values; numbers numerically, text by code unit. */
function compareValues(a: number | string, b: number | string): number {
  if (typeof a === "number" && typeof b === "number") return a - b;
  const x = String(a);
  const y = String(b);
  return x < y ? -1 : x > y ? 1 : 0;
}

/** The sort value of an entry, or undefined when it has none (so it sorts last both ways). */
function sortValue(entry: Entry, sort: string): number | string | undefined {
  if (sort === "name") return entry.folded;
  if (entry.folder) return undefined;
  if (sort === "size") return entry.size;
  if (sort === "modified") return entry.modified;
  return undefined;
}

/** Order two entries as the view operation does: missing last, ties by name then key. */
function compareEntries(a: Entry, b: Entry, sort: string, direction: string): number {
  const x = sortValue(a, sort);
  const y = sortValue(b, sort);
  const sign = direction === "ascending" ? 1 : -1;
  if ((x === undefined) !== (y === undefined)) return x === undefined ? 1 : -1;
  if (x !== undefined && y !== undefined && x !== y) return sign * compareValues(x, y);
  const byName = compareValues(a.folded, b.folded);
  return byName !== 0 ? byName : compareValues(a.key, b.key);
}

/** The row of an entry in the contract's shape. */
function toRow(entry: Entry): ViewRow {
  if (entry.folder) {
    return { kind: "folder", node: entry.key, node_kind: "collection", name: entry.name };
  }
  const id = `00000000-0000-4000-8000-${entry.key.replace(/\D/g, "").padStart(12, "0").slice(-12)}`;
  return {
    kind: "file",
    file_id: id,
    object_id: `urn:uuid:${id}`,
    display_name: entry.name,
    media_type: entry.mediaType,
    size: entry.size,
    modified: entry.modified,
    modified_by_me: null,
    opened_by_me: null,
  };
}

/** Check a page size the way the route does. */
function checkLimit(limit: number): void {
  if (!Number.isInteger(limit) || limit < 1 || limit > 1_000) {
    throw new FixtureError(400, "invalid_request");
  }
}

/** The offset a cursor stands for, bound to `binding`; a cursor of another query is refused. */
function offsetOf(cursor: string | null, binding: string): number {
  if (cursor === null) return 0;
  const split = cursor.indexOf("~");
  if (split < 0 || cursor.slice(split + 1) !== binding)
    throw new FixtureError(400, "invalid_request");
  const offset = Number(cursor.slice(0, split));
  if (!Number.isInteger(offset) || offset < 0) throw new FixtureError(400, "invalid_request");
  return offset;
}

/** The seeded catalogue of one profile and the host's answers over it. */
export class Catalogue {
  private readonly big: Scope;
  private readonly bigFiles: readonly Entry[];
  private readonly scopes = new Map<string, Scope>();
  private readonly orders = new Map<string, readonly Entry[]>();
  private readonly matches = new Map<string, readonly Entry[]>();

  /**
   * @param profile The catalog profile.
   */
  constructor(readonly profile: Profile) {
    const files = fileEntries("big", "", profile.viewFiles, 0x5eed).slice(0, VIEW_CAPS.files);
    const folders = folderEntries("f", profile.viewFolders, folderName).slice(0, VIEW_CAPS.folders);
    this.bigFiles = files;
    this.big = {
      entries: [...folders, ...files],
      complete: profile.viewFiles <= VIEW_CAPS.files && profile.viewFolders <= VIEW_CAPS.folders,
    };
  }

  /** The folder nodes of the big view, in the API's order of listing. */
  folderNodes(): string[] {
    return this.big.entries.filter((entry) => entry.folder).map((entry) => entry.key);
  }

  /**
   * The scope of a node: the big collection, a folder, a sub-folder or another collection.
   *
   * @throws FixtureError 404 `not_found` for a node the catalogue does not hold.
   */
  private scopeOf(node: string): Scope {
    if (node === BIG_NODE) return this.big;
    const cached = this.scopes.get(node);
    if (cached) return cached;
    const match = /^(?:f|c|sf)-(\d+)(?:-(\d+))?$/.exec(node);
    const index = match ? Number(match[1]) : -1;
    const limit = node.startsWith("f-") ? this.profile.viewFolders : this.profile.navCollections;
    if (!match || index < 0 || index >= limit) throw new FixtureError(404, "not_found");
    const salt = `${node.replace(/\D+/g, "")}-`;
    const files = fileEntries(node, salt, 30 + ((index * 37) % 371), 0xc0de + index);
    const subs = folderEntries(`sf-${index}`, index % 6, (j) => `Sub ${index}-${j}`);
    const scope = { entries: [...subs, ...files], complete: true };
    this.scopes.set(node, scope);
    if (this.scopes.size > 64) this.scopes.delete(this.scopes.keys().next().value as string);
    return scope;
  }

  /** The entries of a scope in the order a query asks, folders on top or mixed. */
  private ordered(node: string, scope: Scope, params: ViewParams): readonly Entry[] {
    const key = `${node}|${params.sort}|${params.direction}|${params.folders}`;
    const cached = this.orders.get(key);
    if (cached) return cached;
    const sorted = [...scope.entries].sort((a, b) =>
      compareEntries(a, b, params.sort, params.direction),
    );
    const placed =
      params.folders === "mixed"
        ? sorted
        : [...sorted.filter((e) => e.folder), ...sorted.filter((e) => !e.folder)];
    this.orders.set(key, placed);
    if (this.orders.size > 24) this.orders.delete(this.orders.keys().next().value as string);
    return placed;
  }

  /**
   * One page of a view, as `GET /api/v1/files/view` answers it.
   *
   * @param params The query.
   * @param cursor The previous page's cursor, or null.
   * @param limit Rows per page, 1 to 1,000.
   * @returns The rows, the next cursor and whether the view is whole.
   * @throws FixtureError 404 for an unknown node, 400 for a view that is not `all`, a bad limit
   *   or a cursor made for another query.
   */
  viewPage(params: ViewParams, cursor: string | null, limit: number): RowPage {
    checkLimit(limit);
    const scope = this.scopeOf(params.node);
    if (params.view !== "all") throw new FixtureError(400, "invalid_request");
    const binding = JSON.stringify([
      params.node,
      params.sort,
      params.direction,
      params.folders,
      params.filter,
    ]);
    const wanted = params.filter.toLowerCase();
    const all = this.ordered(params.node, scope, params);
    const entries = wanted === "" ? all : all.filter((entry) => entry.folded.includes(wanted));
    const start = offsetOf(cursor, binding);
    const end = Math.min(start + limit, entries.length);
    return {
      rows: entries.slice(start, end).map(toRow),
      next_cursor: end < entries.length ? `${end}~${binding}` : null,
      complete: scope.complete,
    };
  }

  /** The names of the first `count` rows of a view, in the order a query asks. */
  firstNames(params: ViewParams, count: number): string[] {
    const page = this.viewPage(params, null, Math.min(1_000, Math.max(1, count)));
    return page.rows
      .slice(0, count)
      .map((row) => (row.kind === "file" ? row.display_name : row.name));
  }

  /** The files of the big view that contain every term, in document order. */
  private matching(terms: readonly string[]): readonly Entry[] {
    const key = terms.join(" ");
    const cached = this.matches.get(key);
    if (cached) return cached;
    const found = this.bigFiles.filter((entry) =>
      terms.every((term) => entry.folded.includes(term)),
    );
    this.matches.set(key, found);
    if (this.matches.size > 32) this.matches.delete(this.matches.keys().next().value as string);
    return found;
  }

  /**
   * One page of search results, as `GET /api/v1/files/search` answers it: the files whose name
   * holds every word of the query, in relevance order (document order here) or a view sort.
   *
   * @param params The query.
   * @param cursor The previous page's cursor, or null.
   * @param limit Rows per page, 1 to 1,000.
   * @returns The rows and the next cursor.
   * @throws FixtureError 400 for a bad limit or a cursor made for another query.
   */
  searchPage(params: SearchParams, cursor: string | null, limit: number): RowPage {
    checkLimit(limit);
    const terms = params.q
      .toLowerCase()
      .split(/\s+/)
      .filter((term) => term !== "" && !term.includes(":"));
    const binding = JSON.stringify([
      params.q,
      params.node ?? null,
      params.sort,
      params.direction ?? null,
    ]);
    const found = this.matching(terms);
    const direction = params.direction ?? "ascending";
    const ranked =
      params.sort === "relevance"
        ? found
        : [...found].sort((a, b) => compareEntries(a, b, params.sort, direction));
    const start = offsetOf(cursor, binding);
    const end = Math.min(start + limit, ranked.length);
    return {
      rows: ranked.slice(start, end).map(toRow),
      next_cursor: end < ranked.length ? `${end}~${binding}` : null,
      complete: true,
    };
  }

  /**
   * Queries that each match at least one file, varied in stem and number, all ASCII so a test
   * can type them.
   *
   * @param count How many distinct queries.
   * @returns The queries.
   */
  searchTerms(count: number): string[] {
    const terms: string[] = [];
    const seen = new Set<string>();
    for (let i = 0; terms.length < count && i < this.bigFiles.length; i += 1) {
      const entry = this.bigFiles[(i * 83 + 7) % this.bigFiles.length] as Entry;
      const stem = entry.folded.slice(0, entry.folded.lastIndexOf("."));
      if (!/^[\x20-\x7e]+$/.test(stem) || entry.name === "scan.pdf" || seen.has(stem)) continue;
      seen.add(stem);
      terms.push(stem);
    }
    return terms;
  }

  /**
   * One level of the navigation listing, as `GET /api/v1/navigation` answers it.
   *
   * @param parent A node handle, or null for the repository.
   * @param cursor The previous page's cursor, or null.
   * @param limit Page size, 1 to 1,000.
   * @returns The page.
   * @throws FixtureError 404 for an unknown node, 400 for a bad limit or cursor.
   */
  navigationPage(parent: string | null, cursor: string | null, limit: number): NavPage {
    checkLimit(limit);
    if (limit > NAVIGATION_PAGE_MAX) throw new FixtureError(400, "invalid_request");
    const children = this.childrenOf(parent);
    const start = offsetOf(cursor, parent ?? "");
    const end = Math.min(start + limit, children.count);
    return {
      nodes: Array.from({ length: end - start }, (_, i) => children.at(start + i)),
      ancestors: this.pathTo(parent),
      next_cursor: end < children.count ? `${end}~${parent ?? ""}` : null,
    };
  }

  /** The children of a node: their number and a way to make the one at an index. */
  private childrenOf(parent: string | null): { count: number; at: (i: number) => NavNode } {
    const repository: NavNode = {
      node: REPOSITORY_NODE,
      kind: "repository",
      name: null,
      has_children: true,
    };
    const library: NavNode = {
      node: LIBRARY_NODE,
      kind: "library",
      name: LIBRARY_NAME,
      has_children: true,
    };
    if (parent === null) return { count: 1, at: () => repository };
    if (parent === REPOSITORY_NODE) return { count: 1, at: () => library };
    if (parent === LIBRARY_NODE)
      return { count: this.profile.navCollections, at: (i) => this.collection(i) };
    if (parent === BIG_NODE) return { count: this.profile.viewFolders, at: (i) => this.folder(i) };
    this.scopeOf(parent);
    return { count: 0, at: () => repository };
  }

  /** The collection at an index of the library's level; index 0 is the big one. */
  private collection(i: number): NavNode {
    if (i === 0) return { node: BIG_NODE, kind: "collection", name: BIG_NAME, has_children: true };
    return {
      node: `c-${i}`,
      kind: "collection",
      name: `Collection ${pad(i)}`,
      has_children: false,
    };
  }

  /**
   * The folder at an index of the big collection's level. A folder row of a view is a collection
   * (API-S-011 `ViewFolderKind`), and the UI shows the file view only of a library, a collection
   * or a virtual collection, so the node is a collection.
   */
  private folder(j: number): NavNode {
    return { node: `f-${j}`, kind: "collection", name: folderName(j), has_children: false };
  }

  /** The nodes from the repository down to `parent`, the parent last; empty for the root. */
  private pathTo(parent: string | null): NavNode[] {
    if (parent === null) return [];
    const root = this.childrenOf(null).at(0);
    if (parent === REPOSITORY_NODE) return [root];
    const library = this.childrenOf(REPOSITORY_NODE).at(0);
    if (parent === LIBRARY_NODE) return [root, library];
    if (parent === BIG_NODE) return [root, library, this.collection(0)];
    const index = Number(/\d+/.exec(parent)?.[0] ?? 0);
    if (parent.startsWith("f-")) return [root, library, this.collection(0), this.folder(index)];
    return [root, library, this.collection(index)];
  }
}
