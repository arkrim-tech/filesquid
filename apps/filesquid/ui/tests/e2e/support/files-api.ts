/**
 * A fake of the file routes for the production-build Playwright tests (UI-F2.2): the page is the
 * real built UI, the routes (`files/listing`, `details`, `content`, `upload`, `folders`,
 * `moves`, `renames`, `removals`, `restorations`, `content-handles` and `content/<handle>`) are answered
 * here from a `MemoryFileSource`, which answers as the API does. It records what the browser
 * sent (ranges, handle mints, content requests and their headers) so a test can assert on the
 * wire, and can refuse the next content requests to exercise the single re-mint.
 */
import type { Page, Request, Route } from "@playwright/test";
import { ApiError } from "../../../src/api/client";
import { MemoryFileSource } from "../../../src/files/memory-source";
import type { FileLocation, FileRef, NameNormalization } from "../../../src/files/model";
import { serveNavigation } from "./navigation-api";

/** A 16x16 red PNG. */
export const PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAIAAACQkWg2AAAACXBIWXMAAAABAAAAAQBPJcTWAAAAF0lEQVR4nGP4y8BAEiJN9aiGUQ1DSgMAfsz9ATa/hGgAAAAASUVORK5CYII=",
  "base64",
);

/** A 16x16, 2 second VP8 WebM clip. */
export const WEBM = Buffer.from(
  "GkXfo59ChoEBQveBAULygQRC84EIQoKEd2VibUKHgQJChYECGFOAZwEAAAAAAAOxEU2bdLpNu4tTq4QVSalmU6yBoU27i1OrhBZUrmtTrIHYTbuMU6uEElTDZ1OsggEeTbuMU6uEHFO7a1OsggOb7AEAAAAAAABZAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAVSalmsirXsYMPQkBNgI1MYXZmNjAuMTYuMTAwV0GNTGF2ZjYwLjE2LjEwMESJiECfQAAAAAAAFlSua8GuAQAAAAAAADjXgQFzxYghjmaZ9bUytJyBACK1nIN1bmSIgQCGhVZfVlA4g4EBI+ODhAX14QDgibCBELqBEJqBAhJUw2f8c3OgY8CAZ8iaRaOHRU5DT0RFUkSHjUxhdmY2MC4xNi4xMDBzc9ZjwItjxYghjmaZ9bUytGfIoUWjh0VOQ09ERVJEh5RMYXZjNjAuMzEuMTAyIGxpYnZweGfIoUWjiERVUkFUSU9ORIeTMDA6MDA6MDIuMDAwMDAwMDAwAB9DtnVB9ueBAKO8gQAAgPACAJ0BKhAAEAAARwiFhYiFhIgCAgJ1qgP4A/oCDU0YAP79bvP/4rWHWcX/it//02TxV4z/6YsAo5WBAGQAsQEAARAQABgAGFgv9AAIAACjlYEAyACxAQABEBAAGAAYWC/0AAgAAKOVgQEsALEBAAEQEAAYABhYL/QACAAAo5WBAZAAsQEAARAQABgAGFgv9AAIAACjlYEB9ACxAQABEBAAGAAYWC/0AAgAAKOVgQJYALEBAAEQEAAYABhYL/QACAAAo5WBArwAsQEAARAQFGAAYWC/0AAgAACjlYEDIACxAQABEBAAGAAYWC/0AAgAAKOVgQOEALEBAAEQEAAYABhYL/QACAAAo5WBA+gAsQEAARAQABgAGFgv9AAIAACjlYEETACxAQABEBAAGAAYWC/0AAgAAKOVgQSwALEBAAEQEAAYABhYL/QACAAAo5WBBRQAsQEAARAQABgAGFgv9AAIAACjlYEFeACxAQABEBAAGAAYWC/0AAgAAKOVgQXcALEBAAEQEAAYABhYL/QACAAAo5WBBkAAsQEAARAQABgAGFgv9AAIAACjlYEGpACxAQABEBAAGAAYWC/0AAgAAKOVgQcIALEBAAEQEBRgAGFgv9AAIAAAo5WBB2wAsQEAARAQABgAGFgv9AAIAAAcU7trkbuPs4EAt4r3gQHxggGf8IED",
  "base64",
);

/**
 * A mono 8-bit PCM WAV of `seconds` at 8 kHz.
 *
 * @param seconds - Length of the tone.
 * @returns The WAV file bytes.
 */
export function wav(seconds = 1): Buffer {
  const rate = 8000;
  const samples = rate * seconds;
  const header = Buffer.alloc(44);
  header.write("RIFF", 0);
  header.writeUInt32LE(36 + samples, 4);
  header.write("WAVEfmt ", 8);
  header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20);
  header.writeUInt16LE(1, 22);
  header.writeUInt32LE(rate, 24);
  header.writeUInt32LE(rate, 28);
  header.writeUInt16LE(1, 32);
  header.writeUInt16LE(8, 34);
  header.write("data", 36);
  header.writeUInt32LE(samples, 40);
  const tone = Buffer.from(
    Array.from({ length: samples }, (_, i) => 128 + Math.round(40 * Math.sin(i / 8))),
  );
  return Buffer.concat([header, tone]);
}

/** What the fake saw and how to steer it. */
export interface FilesApi {
  /** The object the fake serves. */
  readonly source: MemoryFileSource;
  /** Every `Range` header the content route received, in order. */
  readonly ranges: string[];
  /** Number of content handles minted. */
  readonly mints: () => number;
  /** Requests to `content/<handle>`: URL path and whether an `Authorization` header was sent. */
  readonly contentRequests: { path: string; authorised: boolean }[];
  /** Refuse (HTTP 410) the next `count` requests to `content/<handle>`. */
  failContent(count: number): void;
  /** The paths of `files/listing` cursors seen: one entry per listing call. */
  readonly listings: { prefix: string; cursor: string | null }[];
  /** Methods and paths of every write the page sent. */
  readonly writes: string[];
  /** The query of every upload the page sent, as sent. */
  readonly uploads: Record<string, string>[];
}

/** The normalisation forms the upload route names. */
const NAME_FORMS = new Set(["nfc", "nfd", "nfkc", "nfkd", "unnormalised"]);

/** The query keys `PUT /api/v1/files/upload` takes (contract 0.18.0); any other is refused. */
const UPLOAD_KEYS = new Set([
  "library_id",
  "file_id",
  "name",
  "size",
  "media_type",
  "last_modified",
  "name_normalization",
  "message",
]);

/** The query of a request as an object. */
function queryOf(request: Request): URLSearchParams {
  return new URL(request.url()).searchParams;
}

/** The file reference a query names. */
function refOf(q: URLSearchParams): FileRef {
  return {
    library_id: q.get("library_id") ?? "",
    object_id: q.get("object_id") ?? "",
    version: q.get("version") ?? "",
    path: q.get("path") ?? "",
    display_name: "",
  };
}

/** Answer a route with JSON or the API's error for a rejected call. */
async function answer(route: Route, call: () => Promise<unknown>): Promise<void> {
  try {
    await route.fulfill({ json: (await call()) as object });
  } catch (error) {
    const status = error instanceof ApiError ? error.status : 500;
    const code = error instanceof ApiError ? (error.code ?? "internal") : "internal";
    await route.fulfill({ status, json: { code, message: "refused", retryable: false } });
  }
}

/** Parse `bytes=a-b` into start and end (inclusive). */
function parseRange(header: string, size: number): { start: number; end: number } {
  const match = /^bytes=(\d+)-(\d*)$/.exec(header);
  const start = match ? Number(match[1]) : 0;
  const end = match?.[2] ? Math.min(Number(match[2]), size - 1) : size - 1;
  return { start, end };
}

/**
 * Serve the file routes of one object for a page.
 *
 * @param page - The page under test.
 * @param source - The object the routes answer from.
 * @returns What the fake saw, and a way to refuse content requests.
 */
export async function serveFiles(page: Page, source: MemoryFileSource): Promise<FilesApi> {
  const handles = new Map<string, FileRef>();
  const refuse = { count: 0 };
  const api: FilesApi = {
    source,
    ranges: [],
    mints: () => handles.size,
    contentRequests: [],
    failContent: (count) => void (refuse.count = count),
    listings: [],
    writes: [],
    uploads: [],
  };
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
  await serveNavigation(page);
  await routeReads(page, api);
  await routeWrites(page, api);
  await routeContent(page, api, handles, refuse);
  return api;
}

/** The read routes: listing, details and ranged content. */
async function routeReads(page: Page, api: FilesApi): Promise<void> {
  await page.route(/\/api\/v1\/files\/listing\?/, (route) => {
    const q = queryOf(route.request());
    const location: FileLocation = {
      library_id: q.get("library_id") ?? "",
      object_id: q.get("object_id") ?? "",
      prefix: q.get("prefix") ?? "",
    };
    api.listings.push({ prefix: location.prefix, cursor: q.get("cursor") });
    return answer(route, () => api.source.list(location, q.get("cursor"), Number(q.get("limit"))));
  });
  await page.route(/\/api\/v1\/files\/details\?/, (route) =>
    answer(route, () => api.source.details(refOf(queryOf(route.request())))),
  );
  await page.route(/\/api\/v1\/files\/content\?/, async (route) => {
    const header = route.request().headers()["range"] ?? "";
    api.ranges.push(header);
    const ref = refOf(queryOf(route.request()));
    const { size } = await api.source.details(ref);
    const { start, end } = parseRange(header, size);
    const body = await api.source.readRange(ref, { start, end });
    await route.fulfill({
      status: 206,
      headers: { "Content-Range": `bytes ${start}-${end}/${size}` },
      body: Buffer.from(body),
    });
  });
}

/** The write routes. */
async function routeWrites(page: Page, api: FilesApi): Promise<void> {
  const object = { library_id: "lib", object_id: "obj" };
  await page.route(/\/api\/v1\/files\/upload\?/, (route) => {
    const q = queryOf(route.request());
    const bytes = route.request().postDataBuffer() ?? Buffer.alloc(0);
    api.uploads.push(Object.fromEntries(q));
    api.writes.push(`PUT upload ${q.get("name") ?? ""}`);
    return answer(route, () => uploadAsTheApi(api.source, q, bytes));
  });
  const post = (name: string, call: (body: Record<string, unknown>) => Promise<unknown>) =>
    page.route(new RegExp(`/api/v1/files/${name}$`), (route) => {
      const body = route.request().postDataJSON() as Record<string, unknown>;
      api.writes.push(`POST ${name} ${JSON.stringify(body)}`);
      return answer(route, () => call(body));
    });
  await post("folders", (b) => api.source.createFolder(object, String(b.path)));
  await post("moves", (b) => api.source.move(object, String(b.from), String(b.to)));
  await post("renames", (b) =>
    api.source.rename(
      { library_id: String(b.library_id) },
      String(b.file_id),
      String(b.display_name),
    ),
  );
  await post("removals", (b) => api.source.remove(object, String(b.path)));
  await post("restorations", (b) =>
    api.source.restore(object, String(b.version), b.paths as string[]),
  );
}

/**
 * Answer an upload as the API's query rules do: an unknown key (a `path` or `object_id` of the
 * old upload form among them), a missing `library_id`, `name` or `size`, a normalisation form
 * the contract does not name, or a declared size that is not the body's is refused with
 * `invalid_request` before the source is asked.
 */
async function uploadAsTheApi(
  source: MemoryFileSource,
  q: URLSearchParams,
  bytes: Buffer,
): Promise<unknown> {
  const unknown = [...q.keys()].some((key) => !UPLOAD_KEYS.has(key));
  const missing = ["library_id", "name", "size"].some((key) => q.get(key) === null);
  const form = q.get("name_normalization");
  const badForm = form !== null && !NAME_FORMS.has(form);
  const badSize = q.get("size") !== String(bytes.byteLength);
  if (unknown || missing || badForm || badSize) {
    throw new ApiError(400, "/api/v1/files/upload", { code: "invalid_request" });
  }
  const facts = {
    name: q.get("name") ?? "",
    file_id: q.get("file_id") ?? undefined,
    media_type: q.get("media_type") ?? undefined,
    last_modified: q.get("last_modified") ?? undefined,
    name_normalization: (q.get("name_normalization") ?? undefined) as NameNormalization | undefined,
  };
  const body = new Blob([new Uint8Array(bytes)]);
  return source.upload({ library_id: q.get("library_id") ?? "" }, facts, body);
}

/** Mint a handle and serve the content it names. */
async function routeContent(
  page: Page,
  api: FilesApi,
  handles: Map<string, FileRef>,
  refuse: { count: number },
): Promise<void> {
  let counter = 0;
  await page.route(/\/api\/v1\/content-handles$/, (route) => {
    const ref = route.request().postDataJSON() as FileRef;
    counter += 1;
    const handle = `${String(counter).padStart(4, "0")}`.padEnd(43, "A");
    handles.set(handle, ref);
    return route.fulfill({
      json: { handle, url: `/api/v1/content/${handle}`, expires_at: "2026-10-20T11:00:00Z" },
    });
  });
  await page.route(/\/api\/v1\/content\/[A-Za-z0-9_-]{43}$/, async (route) => {
    const request = route.request();
    api.contentRequests.push({
      path: new URL(request.url()).pathname,
      authorised: request.headers()["authorization"] !== undefined,
    });
    if (refuse.count > 0) {
      refuse.count -= 1;
      return route.fulfill({ status: 410, body: "gone" });
    }
    const handle = new URL(request.url()).pathname.split("/").pop() ?? "";
    const ref = handles.get(handle);
    if (!ref) return route.fulfill({ status: 404, body: "" });
    const details = await api.source.details(ref);
    const header = request.headers()["range"] ?? "";
    const { start, end } = parseRange(header, details.size);
    const body = await api.source.readRange(ref, { start, end });
    return route.fulfill({
      status: header ? 206 : 200,
      headers: {
        "Content-Type": details.media_type,
        "Cache-Control": "private, no-store",
        "Accept-Ranges": "bytes",
        ...(header ? { "Content-Range": `bytes ${start}-${end}/${details.size}` } : {}),
      },
      body: Buffer.from(body),
    });
  });
}
