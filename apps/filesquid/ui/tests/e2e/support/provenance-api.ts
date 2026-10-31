/**
 * Fakes of the routes the File info and Provenance tabs read (UI-F2.15; API-S-027, API-S-029)
 * for the production-build Playwright tests: `GET files/facts`, `GET files/metadata` (the
 * technical rows), `GET files/{id}/history` (cursor pages, oldest first), `GET
 * files/{id}/versions/{version}/prov` and `GET files/{id}/history/export`, answering the data
 * of mockups 35 and 36: `facade-study.jpg`, four versions, the head v4 a metadata edit by the
 * installation `urn:uuid:5a0c41e2-…-e19b` and FileSquid 0.1.0. `servePanel` registers them with
 * its own routes, so every file panel spec has them; a spec steers them through the returned
 * {@link ProvenanceApi}, and a later `page.route` (the metadata spec's statements) takes
 * precedence over these.
 */
import { randomUUID } from "node:crypto";
import type { Page, Route } from "@playwright/test";
import { PROV_OBJECT, provEntries, provFacts, provRecord } from "../../support/provenance-fakes";
import type { HistoryEntry } from "../../../src/files/provenance";
import type { FileFacts } from "../../../src/files/facts-source";

/** What the page asked, and ways to steer the fake. */
export interface ProvenanceApi {
  /** The cursor of every history request (null for the first page). */
  readonly historyRequests: (string | null)[];
  /** The version of every record request. */
  readonly recordRequests: string[];
  /** The query of every facts request. */
  readonly factsRequests: Record<string, string>[];
  /** How many statement reads (the technical rows) the page sent. */
  statementReads: number;
  /** How many exports the page asked for. */
  exports: number;
  /** The `exif:` statements `files/metadata` answers; none by default (nothing read). */
  technical: { curie: string; value: string }[];
  /** The activities `files/{id}/history` answers, oldest first. */
  entries: HistoryEntry[];
  /** The facts `files/facts` answers. */
  facts: FileFacts;
  /** The versions whose record is answered 500. */
  failRecords: Set<string>;
  /** Entries per history page. */
  pageSize: number;
  /** Answer the history route with this status and code instead (an error state). */
  failHistory(status: number, code: string): void;
  /** Answer the history route normally again. */
  healHistory(): void;
  /** Hold the history route until the returned function is called (the loading state). */
  holdHistory(): () => void;
  /** Answer the facts route with this status and code instead. */
  failFacts(status: number, code: string): void;
  /** Answer the export route with this status and code instead. */
  failExport(status: number, code: string): void;
  /** Whether the export reports a truncated body. */
  truncatedExport: boolean;
}

/** The file the routes answer for: what the file panel fixture says about it. */
export interface ProvenanceFile {
  /** The name users see. */
  readonly displayName: string;
  /** The stored path of its original (its extension is the file's). */
  readonly path: string;
  /** Its media type. */
  readonly mediaType: string;
  /** Its size in bytes at the head. */
  readonly size: number;
}

/** The format labels the facts route answers for the media types of the fixtures. */
const FORMATS: Readonly<Record<string, string>> = {
  "image/jpeg": "JPEG image",
  "application/pdf": "PDF document",
};

/** The facts of the file: its own names, type and size, over the mockup's SHA-256 and import. */
function factsOf(file: ProvenanceFile): FileFacts {
  const extension = file.path.split(".").pop() ?? null;
  return provFacts({
    original_name: file.displayName,
    display_name: file.displayName,
    extension,
    format: FORMATS[file.mediaType] ?? null,
    media_type: file.mediaType,
    size: file.size,
  });
}

/** A JSON error body of the contract. */
function refusal(status: number, code: string) {
  return { status, json: { code, message: "refused", retryable: status >= 500 } };
}

/**
 * Serve the File info and Provenance routes for the fixture file of mockups 35 and 36.
 *
 * @param page - The page under test.
 * @returns What the fake saw, and ways to steer it.
 */
export async function serveProvenance(page: Page, file: ProvenanceFile): Promise<ProvenanceApi> {
  const fail: {
    history?: { status: number; code: string };
    facts?: { status: number; code: string };
    export?: { status: number; code: string };
  } = {};
  const cursors = new Map<string, number>();
  let hold: Promise<void> | undefined;
  const api: ProvenanceApi = {
    historyRequests: [],
    recordRequests: [],
    factsRequests: [],
    statementReads: 0,
    exports: 0,
    technical: [],
    entries: provEntries(),
    facts: factsOf(file),
    failRecords: new Set(),
    pageSize: 100,
    truncatedExport: false,
    failHistory: (status, code) => void (fail.history = { status, code }),
    healHistory: () => void (fail.history = undefined),
    holdHistory: () => {
      let release: () => void = () => undefined;
      hold = new Promise<void>((resolve) => (release = resolve));
      return () => release();
    },
    failFacts: (status, code) => void (fail.facts = { status, code }),
    failExport: (status, code) => void (fail.export = { status, code }),
  };
  const json = (route: Route, body: unknown) => route.fulfill({ json: body });
  await page.route(/\/api\/v1\/files\/facts\?/, (route) => {
    api.factsRequests.push(Object.fromEntries(new URL(route.request().url()).searchParams));
    if (fail.facts) return route.fulfill(refusal(fail.facts.status, fail.facts.code));
    const body = {
      library_id: "lib",
      file_id: PROV_OBJECT.slice(9),
      version: "v4",
      facts: api.facts,
    };
    return json(route, body);
  });
  await page.route(/\/api\/v1\/files\/metadata\?/, (route) => {
    api.statementReads += 1;
    const statements = api.technical.map(({ curie, value }) => {
      const [prefix, term] = curie.split(":") as [string, string];
      return { curie, prefix, term, value, source: "extracted" };
    });
    return json(route, {
      library_id: "lib",
      file_id: PROV_OBJECT.slice(9),
      version: "v4",
      statements,
    });
  });
  await page.route(/\/api\/v1\/files\/[0-9a-f-]{36}\/history(\?|$)/, async (route) => {
    const url = new URL(route.request().url());
    api.historyRequests.push(url.searchParams.get("cursor"));
    await hold;
    if (fail.history) return route.fulfill(refusal(fail.history.status, fail.history.code));
    // The cursor is opaque to the page: the fake issues tokens and maps them back to offsets.
    const token = url.searchParams.get("cursor");
    const start = token === null ? 0 : (cursors.get(token) ?? 0);
    const items = api.entries.slice(start, start + api.pageSize);
    const more = start + api.pageSize < api.entries.length;
    const next = more ? `c-${randomUUID()}` : null;
    if (next !== null) cursors.set(next, start + api.pageSize);
    return json(route, { items, next_cursor: next, truncated: false });
  });
  await page.route(/\/api\/v1\/files\/[0-9a-f-]{36}\/versions\/v\d+\/prov$/, (route) => {
    const version = /\/versions\/(v\d+)\/prov$/.exec(route.request().url())?.[1] ?? "v1";
    api.recordRequests.push(version);
    if (api.failRecords.has(version)) return route.fulfill(refusal(500, "internal"));
    return route.fulfill({
      contentType: "application/ld+json",
      body: JSON.stringify(provRecord(Number(version.slice(1)))),
    });
  });
  await page.route(/\/api\/v1\/files\/[0-9a-f-]{36}\/history\/export/, (route) => {
    api.exports += 1;
    if (fail.export) return route.fulfill(refusal(fail.export.status, fail.export.code));
    const headers: Record<string, string> = {
      "content-type": "application/ld+json",
      "content-disposition": 'attachment; filename="history.jsonld"',
    };
    if (api.truncatedExport) headers["X-History-Truncated"] = "true";
    return route.fulfill({ status: 200, headers, body: JSON.stringify([{ "@id": "x" }]) });
  });
  return api;
}
