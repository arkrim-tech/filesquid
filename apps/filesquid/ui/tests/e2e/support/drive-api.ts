/**
 * Fakes of the view operation and the preferences for the production-build Playwright tests
 * (UI-F2.8): the page is the real built UI, `GET /api/v1/files/view` is answered from a
 * `MemoryViewSource` in the contract's own shape (folder rows and file rows with the fields of
 * `ViewPage`, nothing more), and `GET` / `PUT /api/v1/preferences` keep one user's choices as a
 * host that stores them does. Each call is recorded, and a refusal can be switched on, so the
 * tests prove what the page sent and how it shows a refusal.
 */
import type { Page } from "@playwright/test";
import { ApiError } from "../../../src/api/client";
import { MemoryViewSource } from "../../../src/drive/memory-source";
import type { DriveRow, ViewQuery } from "../../../src/drive/model";
import { DEFAULT_PREFERENCES, type UserPreferences } from "../../../src/preferences/model";

/** The node of the library the fixture navigation tree names first (`FAMILY`'s `l1`). */
export const LIBRARY_NODE = "l1";

const file = (
  id: string,
  name: string,
  size: number,
  modified: string,
  extra: Partial<DriveRow> = {},
): DriveRow => ({
  key: id,
  kind: "file",
  name,
  fileId: id,
  objectId: `urn:uuid:${id}`,
  mediaType: "application/octet-stream",
  size,
  modified,
  ...extra,
});

/** Folder rows are collections of the library. */
const folder = (node: string, name: string): DriveRow => ({
  key: node,
  kind: "folder",
  name,
  node,
  nodeKind: "collection",
});

/** The clock of the fixture: every relative date is measured from here. */
export const NOW = new Date("2026-10-16T12:00:00Z");

/** The rows of the fixture library, as mockup 05 draws them (three folders, ten files). */
export const ROWS: readonly DriveRow[] = [
  folder("c-contracts", "Contracts"),
  folder("c-drawings", "Drawings"),
  folder("c-photos", "Site photos"),
  file("f1", "harbour-masterplan-v7.pdf", 19_293_798, "2026-10-16T10:00:00Z"),
  file("f2", "budget-2026.xlsx", 421_888, "2026-10-15T12:00:00Z"),
  file("f3", "facade-study.jpg", 6_500_000, "2026-10-15T08:00:00Z"),
  file("f4", "interview-jun-park.m4a", 35_756_646, "2026-10-14T08:00:00Z"),
  file("f5", "survey-points.csv", 90_112, "2026-10-13T08:00:00Z"),
  file("f6", "meeting-notes-14-oct.docx", 65_536, "2026-10-14T09:00:00Z"),
  file("f7", "site-walkthrough.mp4", 1_288_490_188, "2026-10-11T08:00:00Z"),
  file("f8", "tender-response.pdf", 3_040_870, "2026-10-03T08:00:00Z"),
  file("f9", "model-export.blend", 221_249_536, "2026-10-02T08:00:00Z"),
  file("f10", "notes.txt", 2048, "2026-10-01T08:00:00Z"),
];

/** What the fake saw and how to steer it. */
export interface DriveApi {
  /** The rows served; `replace` them to change the view. */
  readonly source: MemoryViewSource;
  /** Every view call's query, in order. */
  readonly calls: ViewQuery[];
  /** Every preferences value the page wrote, in order. */
  readonly saved: UserPreferences[];
  /** The preferences the host holds now. */
  stored: UserPreferences;
  /** Answer 503 `unavailable` on the preferences routes while true (a host with no store). */
  preferencesUnavailable: boolean;
  /** Milliseconds every view answer waits, to hold the loading state on screen. */
  delayMs: number;
  /** Every file write the page sent (route name and body), in order. */
  readonly writes: { route: string; body: Record<string, unknown> }[];
  /** The refusal (HTTP status and API error code) the next rename gets, or undefined. */
  renameRefusal?: { status: number; code: string };
}

/** The library the fake host has open. */
export const LIBRARY_ID = "lib-1";

/** A contract `ViewRow` for a row of the fixture. */
function viewRow(row: DriveRow): Record<string, unknown> {
  if (row.kind === "folder") {
    return { kind: "folder", node: row.node, node_kind: row.nodeKind, name: row.name };
  }
  return {
    kind: "file",
    file_id: row.fileId,
    object_id: row.objectId,
    display_name: row.name,
    media_type: row.mediaType,
    size: row.size,
    modified: row.modified,
    modified_by_me: row.modifiedByMe ?? null,
    opened_by_me: row.openedByMe ?? null,
  };
}

/** The query the page sent, as the view operation reads it. */
function queryOf(url: string): { query: ViewQuery; cursor: string | null; limit: number } {
  const params = new URL(url).searchParams;
  return {
    query: {
      node: params.get("node") ?? "",
      view: (params.get("view") ?? "all") as ViewQuery["view"],
      sort: (params.get("sort") ?? "name") as ViewQuery["sort"],
      direction: (params.get("direction") ?? "ascending") as ViewQuery["direction"],
      folders: (params.get("folders") ?? "on_top") as ViewQuery["folders"],
      filter: params.get("filter") ?? "",
    },
    cursor: params.get("cursor"),
    limit: Number(params.get("limit") ?? "200"),
  };
}

/**
 * Serve `GET /api/v1/files/view` and the preferences routes.
 *
 * @param page - The page under test.
 * @param rows - The rows of the library node (default {@link ROWS}).
 * @param stored - The preferences the host holds at the start.
 * @param nodes - The library, collection or virtual-collection nodes that have this view
 *   (default: the library only); a node not listed answers 404 as an unknown one does.
 * @returns What the fake saw, and its switches.
 */
export async function serveDrive(
  page: Page,
  rows: readonly DriveRow[] = ROWS,
  stored: UserPreferences = DEFAULT_PREFERENCES,
  nodes: readonly string[] = [LIBRARY_NODE],
): Promise<DriveApi> {
  const views = new Map(nodes.map((node) => [node, { all: rows, deleted: [] }] as const));
  const source = new MemoryViewSource(views);
  const api: DriveApi = {
    source,
    calls: [],
    saved: [],
    stored,
    preferencesUnavailable: false,
    delayMs: 0,
    writes: [],
  };
  await page.route(/\/api\/v1\/files\/view\?/, async (route) => {
    const { query, cursor, limit } = queryOf(route.request().url());
    api.calls.push(query);
    if (api.delayMs > 0) await new Promise((resolve) => setTimeout(resolve, api.delayMs));
    try {
      const answer = await source.view(query, cursor, limit);
      const body = {
        node: query.node,
        view: query.view,
        sort: query.sort,
        direction: query.direction,
        folders: query.folders,
        rows: answer.rows.map(viewRow),
        next_cursor: answer.nextCursor,
        complete: answer.complete,
      };
      await route.fulfill({ json: body });
    } catch (error) {
      const status = error instanceof ApiError ? error.status : 500;
      const code = error instanceof ApiError ? (error.code ?? "internal") : "internal";
      await route.fulfill({ status, json: { code, message: "refused", retryable: false } });
    }
  });
  await serveFiles(page, api);
  await page.route(/\/api\/v1\/preferences$/, async (route) => {
    if (api.preferencesUnavailable) {
      const body = { code: "unavailable", message: "not installed", retryable: true };
      await route.fulfill({ status: 503, json: body });
      return;
    }
    if (route.request().method() === "PUT") {
      api.stored = route.request().postDataJSON() as UserPreferences;
      api.saved.push(api.stored);
    }
    await route.fulfill({ json: api.stored });
  });
  return api;
}

/** The one library the fake host has open, as `GET /api/v1/libraries/recent` lists it. */
const RECENT = {
  libraries: [
    {
      library_id: LIBRARY_ID,
      name: "Family",
      last_opened: "2026-10-16T00:00:00Z",
      open: true,
      free_bytes: 500_000_000_000,
      free_bytes_reason: null,
    },
  ],
};

/** The head version of every file object, before a write. */
const HEAD = "v4";

/** A write command as the page sends it. */
interface WriteBody {
  envelope: { operation_id: string };
  input: Record<string, unknown>;
}

/** Serve the library list and the file routes the actions use (listing and the writes). */
async function serveFiles(page: Page, api: DriveApi): Promise<void> {
  await page.route(/\/api\/v1\/libraries\/recent$/, (route) => route.fulfill({ json: RECENT }));
  await page.route(/\/api\/v1\/files\/library-history-size\?/, (route) =>
    route.fulfill({
      json: {
        library_id: LIBRARY_ID,
        objects: 13,
        contents: 20,
        bytes: 4_509_715_661,
        truncated: false,
      },
    }),
  );
  await page.route(/\/api\/v1\/files\/listing\?/, (route) => {
    const params = new URL(route.request().url()).searchParams;
    const entry = {
      kind: "file",
      name: "3f9a0001.pdf",
      path: "original/3f9a0001.pdf",
      display_name: "stored-name-must-not-show.pdf",
      size: 1,
      type_hint: "pdf",
      modified: "2026-10-16T10:00:00Z",
      modified_version: HEAD,
    };
    const entries = params.get("prefix") === "original" ? [entry] : [];
    return route.fulfill({
      json: {
        library_id: LIBRARY_ID,
        object_id: params.get("object_id"),
        version: HEAD,
        prefix: params.get("prefix") ?? "",
        entries,
        next_cursor: null,
      },
    });
  });
  await page.route(/\/api\/v1\/files\/(renames|removals|restorations)$/, (route) => {
    const name = new URL(route.request().url()).pathname.split("/").at(-1) as string;
    const body = route.request().postDataJSON() as WriteBody;
    api.writes.push({ route: name, body: body as unknown as Record<string, unknown> });
    if (name === "renames" && api.renameRefusal) {
      const { status, code } = api.renameRefusal;
      return route.fulfill({ status, json: { code, message: "refused", retryable: false } });
    }
    const version = name === "restorations" ? "v6" : "v5";
    const result = {
      library_id: LIBRARY_ID,
      object_id: body.input.object_id ?? null,
      version,
      created: "2026-10-16T12:00:00Z",
    };
    const answer = {
      operation_id: body.envelope.operation_id,
      kind: "committed",
      error: null,
      result,
    };
    return route.fulfill({ json: answer });
  });
}
