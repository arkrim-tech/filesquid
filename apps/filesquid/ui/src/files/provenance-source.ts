/**
 * The reads of the Provenance tab (UI-F2.15; UI-S-020, API-S-029, ADR 040): the
 * {@link ProvenanceSource} interface over the history routes `GET /api/v1/files/{id}/history`,
 * `GET /api/v1/files/{id}/versions/{version}/prov` and `GET /api/v1/files/{id}/history/export`
 * (all under the route action `history.read`, granted to the owner and the administrator) and
 * {@link ClientProvenanceSource}, which answers it through `FileSquidClient` (UI-S-012). The
 * `{id}` is the UUID of the file's OCFL object `urn:uuid:<id>`; a file whose object is not named
 * by a UUID is refused before anything is sent. The tab never fetches by itself, so a fixture
 * stands in for the API in tests. Every method only reads: its name says so.
 */
import type { FileSquidClient } from "../api/client";
import type { FileRef } from "./model";
import { fileIdOf, isOriginalPath } from "./ops";
import type { HistoryPage } from "./provenance";
import type { SourceOptions } from "./source";

/** Entries in one history page (the route takes 1 to 1,000). */
export const HISTORY_PAGE_SIZE = 200;

/** The JSON-LD export of a file's history. */
export interface ExportedHistory {
  /** The JSON-LD array of the file's records, as the API sent it. */
  readonly blob: Blob;
  /** True when the API left records out (`X-History-Truncated`). */
  readonly truncated: boolean;
}

/** One page request: the cursor of the previous page, and an abort signal. */
export interface HistoryPageOptions extends SourceOptions {
  /** The opaque cursor the previous page returned; the first page when absent. */
  readonly cursor?: string;
}

/** What the Provenance tab asks of the API. */
export interface ProvenanceSource {
  /** One page of the file's activities, oldest first. Reads. */
  history(file: FileRef, options?: HistoryPageOptions): Promise<HistoryPage>;
  /** The JSON-LD record (profile `filesquid-prov/1`) of one version, as parsed JSON. Reads. */
  record(file: FileRef, version: string, options?: SourceOptions): Promise<unknown>;
  /** Every record of the file's object as one JSON-LD document. Reads. */
  exportHistory(file: FileRef, options?: SourceOptions): Promise<ExportedHistory>;
}

/** The part of `FileSquidClient` the source needs (injectable in tests). */
export type ProvenanceClient = Pick<FileSquidClient, "request" | "read">;

/**
 * The route prefix of one file's history: `/api/v1/files/<id>`.
 *
 * @param file The file.
 * @returns The prefix, with the id percent-encoded.
 * @throws TypeError when the reference is not a file's own stored original in an object named
 *   by a UUID (`original/<id>[.<ext>]` in `urn:uuid:<id>`, API-S-026): another path in such an
 *   object would show that object's history, not the file's. No request is sent.
 */
function filePrefix(file: FileRef): string {
  const id = fileIdOf(file.object_id);
  if (id === undefined || !isOriginalPath(file.path, id)) {
    throw new TypeError("The reference has no file history");
  }
  return `/api/v1/files/${encodeURIComponent(id)}`;
}

/** A {@link ProvenanceSource} over `FileSquidClient` and the history routes. */
export class ClientProvenanceSource implements ProvenanceSource {
  /** @param client The API client (its transport adapter and token are already chosen). */
  constructor(private readonly client: ProvenanceClient) {}

  /**
   * Read one page of the file's history with `GET /api/v1/files/{id}/history`. Network I/O.
   *
   * @param file The file (its object names the history; its version is not sent).
   * @param options The cursor of the previous page and an abort signal.
   * @returns The page, oldest first, with the next cursor.
   * @throws TypeError when the reference has no file history (nothing is sent).
   * @throws ApiError when the API refuses (the history routes need `history.read`).
   */
  async history(file: FileRef, options: HistoryPageOptions = {}): Promise<HistoryPage> {
    const query = new URLSearchParams({ limit: String(HISTORY_PAGE_SIZE) });
    if (options.cursor !== undefined) query.set("cursor", options.cursor);
    const path = `${filePrefix(file)}/history?${query.toString()}`;
    return this.client.request<HistoryPage>(path, { signal: options.signal });
  }

  /**
   * Read the record of one version with `GET /api/v1/files/{id}/versions/{version}/prov`.
   * Network I/O.
   *
   * @param file The file.
   * @param version The OCFL version name, for example `v3`.
   * @param options Abort signal.
   * @returns The JSON-LD record, parsed; the caller validates it.
   * @throws TypeError when the reference has no file history (nothing is sent).
   * @throws ApiError when the API refuses (404 for a version with no record).
   */
  async record(file: FileRef, version: string, options: SourceOptions = {}): Promise<unknown> {
    const path = `${filePrefix(file)}/versions/${encodeURIComponent(version)}/prov`;
    return this.client.request<unknown>(path, { signal: options.signal });
  }

  /**
   * Download the file's history as JSON-LD with `GET /api/v1/files/{id}/history/export`.
   * Network I/O; the whole body is read into memory (the API bounds it to 64 MiB).
   *
   * @param file The file.
   * @param options Abort signal.
   * @returns The JSON-LD document and whether the export left records out.
   * @throws TypeError when the reference has no file history (nothing is sent).
   * @throws ApiError when the API refuses.
   */
  async exportHistory(file: FileRef, options: SourceOptions = {}): Promise<ExportedHistory> {
    const path = `${filePrefix(file)}/history/export`;
    const { response } = await this.client.read(path, { signal: options.signal });
    const type = response.headers.get("content-type") ?? "application/ld+json";
    const blob = new Blob([await response.arrayBuffer()], { type });
    return { blob, truncated: response.headers.get("X-History-Truncated") === "true" };
  }
}
