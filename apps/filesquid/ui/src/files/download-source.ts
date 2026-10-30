/**
 * Downloading one version of a file (UI-F2.11, UI-S-009, PRD F-065): the {@link DownloadSource}
 * interface and {@link ClientDownloadSource}, which answers it through `FileSquidClient`. The
 * panels never fetch by themselves, so a fixture stands in for the API in tests.
 *
 * The whole file comes from `GET /api/v1/files/download` (an attachment the API journals as
 * `file.download`) when it fits one desktop answer, which the desktop scheme caps at 16 MiB
 * (DE-S-005). A larger version is read with ranged `GET /api/v1/files/content` calls of at most
 * 8 MiB, at most {@link MAX_DOWNLOAD_READS} of them, so the loop is bounded; a version beyond
 * that bound is refused as `payload_too_large` before any byte is read. Each ranged answer must
 * be a 206 whose `Content-Range` names exactly the range asked for and whose body has that
 * length; any other answer (a host that ignored `Range`, a shifted or short range) is refused as
 * a `TypeError` instead of being appended, so a saved file is never silently corrupted. Every
 * method only reads: its name says so.
 */
import { ApiError, type FileSquidClient, type ReadResult } from "../api/client";
import type { FileRef } from "./model";
import type { SourceOptions } from "./source";

/** The largest answer the desktop scheme sends (DE-S-005): 16 MiB. */
export const DESKTOP_ANSWER_CAP = 16 * 1024 * 1024;
/** Bytes of one ranged read: 8 MiB, the API's own chunk. */
export const RANGE_CHUNK = 8 * 1024 * 1024;
/** Most ranged reads of one download: 4,096 of 8 MiB, 32 GiB. */
export const MAX_DOWNLOAD_READS = 4096;

/** What the History and Versions panels ask of the API to save a version. */
export interface DownloadSource {
  /** The whole file of one version, ready to save. Reads. */
  readVersionFile(file: FileRef, options?: SourceOptions): Promise<Blob>;
}

/** The part of `FileSquidClient` the source needs (injectable in tests). */
export type DownloadClient = Pick<FileSquidClient, "request" | "read">;

/** The query naming one file version by library, object, version and logical path. */
function versionQuery(file: FileRef): string {
  const values = {
    library_id: file.library_id,
    object_id: file.object_id,
    version: file.version,
    path: file.path,
  };
  return `?${new URLSearchParams(values).toString()}`;
}

/** Throw an `AbortError` when the signal has aborted. */
function throwIfAborted(signal?: AbortSignal): void {
  if (signal?.aborted) throw new DOMException("aborted", "AbortError");
}

/**
 * Check that a ranged answer is the range asked for. Pure.
 *
 * @param answer The answer of `FileSquidClient.read`.
 * @param start First byte asked for.
 * @param end Last byte asked for, inclusive.
 * @param bytes Length of the body received.
 * @throws TypeError when the answer is not a 206, names another range, or has another length.
 */
function expectRange(answer: ReadResult, start: number, end: number, bytes: number): void {
  const named = answer.contentRange;
  if (!answer.partial || !named) {
    throw new TypeError("The host did not answer the ranged read with a 206 and a Content-Range");
  }
  if (named.start !== start || named.end !== end) {
    throw new TypeError(`The host answered bytes ${named.start}-${named.end}, not ${start}-${end}`);
  }
  if (bytes !== end - start + 1) {
    throw new TypeError(`The host sent ${bytes} bytes for the range ${start}-${end}`);
  }
}

/** A {@link DownloadSource} over `FileSquidClient` and the file routes. */
export class ClientDownloadSource implements DownloadSource {
  /** @param client The API client (its transport adapter and token are already chosen). */
  constructor(private readonly client: DownloadClient) {}

  /**
   * Read the whole file of one version. Network I/O: one details read for the size, then one
   * download read, or ranged content reads above {@link DESKTOP_ANSWER_CAP}.
   *
   * @param file The file and the version to read.
   * @param options Abort signal.
   * @returns The file's bytes.
   * @throws ApiError when the API refuses, or 413 `payload_too_large` above the bound of reads.
   * @throws DOMException `AbortError` when the signal aborts.
   */
  async readVersionFile(file: FileRef, options: SourceOptions = {}): Promise<Blob> {
    const { signal } = options;
    const details = await this.client.request<{ size: number }>(
      `/api/v1/files/details${versionQuery(file)}`,
      { signal },
    );
    if (details.size <= DESKTOP_ANSWER_CAP) return this.readWhole(file, signal);
    return this.readRanged(file, details.size, signal);
  }

  /** The file through the download route. Network I/O. */
  private async readWhole(file: FileRef, signal?: AbortSignal): Promise<Blob> {
    const path = `/api/v1/files/download${versionQuery(file)}`;
    const { response } = await this.client.read(path, { signal });
    const type = response.headers.get("content-type") ?? "";
    return new Blob([await response.arrayBuffer()], { type });
  }

  /** The file through ranged content reads of at most {@link RANGE_CHUNK} bytes. Network I/O. */
  private async readRanged(file: FileRef, size: number, signal?: AbortSignal): Promise<Blob> {
    const reads = Math.ceil(size / RANGE_CHUNK);
    const path = `/api/v1/files/content${versionQuery(file)}`;
    if (reads > MAX_DOWNLOAD_READS) {
      throw new ApiError(413, path, { code: "payload_too_large" });
    }
    const parts: Blob[] = [];
    for (let index = 0; index < reads; index += 1) {
      throwIfAborted(signal);
      const start = index * RANGE_CHUNK;
      const end = Math.min(start + RANGE_CHUNK, size) - 1;
      const answer = await this.client.read(path, { range: { start, end }, signal });
      const body = await answer.response.arrayBuffer();
      expectRange(answer, start, end, body.byteLength);
      parts.push(new Blob([body]));
    }
    throwIfAborted(signal);
    return new Blob(parts);
  }
}
