/**
 * The history reads of the file panel (UI-F2.3): the {@link HistorySource} interface over the
 * existing routes `GET /api/v1/files/history` and `GET /api/v1/files/versions` (API-F1.3,
 * API-F2.5) and `GET /api/v1/files/history-size` (API-F2.5), and {@link ClientHistorySource},
 * which answers it through `FileSquidClient`. The panels never fetch by themselves, so a
 * fixture stands in for the API in tests. Every method only reads: its name says so.
 */
import type { FileSquidClient } from "../api/client";
import type { components } from "../api/schema";
import type { FileRef, ObjectRef } from "./model";
import type { SourceOptions } from "./source";

/** One version of an OCFL object, generated from the API contract. */
export type VersionRecord = components["schemas"]["VersionRecord"];
/** The versions that changed one file, oldest first. */
export type FileHistory = components["schemas"]["FileHistory"];
/** One version that changed a file, with its change, digest and size. */
export type FileHistoryEntry = components["schemas"]["FileHistoryEntry"];
/** Every version of an object, oldest first, with its head. */
export type ObjectVersions = components["schemas"]["ObjectVersions"];
/** The bytes one file's history keeps, each content once. */
export type FileHistorySize = components["schemas"]["FileHistorySize"];

/** The history reads the Details and Versions panels ask of the API. */
export interface HistorySource {
  /** The versions that changed one file, oldest first. Reads. */
  history(file: FileRef, options?: SourceOptions): Promise<FileHistory>;
  /** Every version of the object, oldest first, with its head. Reads. */
  versions(object: ObjectRef, options?: SourceOptions): Promise<ObjectVersions>;
  /** The bytes the file's history keeps. Reads. */
  historySize(file: FileRef, options?: SourceOptions): Promise<FileHistorySize>;
}

/** The part of `FileSquidClient` the source needs (injectable in tests). */
export type HistoryClient = Pick<FileSquidClient, "request">;

/** A query string of the given values, in order. */
function query(values: Readonly<Record<string, string>>): string {
  return `?${new URLSearchParams(values).toString()}`;
}

/** The query naming one file by library, object and logical path. */
function fileQuery(file: FileRef): string {
  return query({ library_id: file.library_id, object_id: file.object_id, path: file.path });
}

/** A {@link HistorySource} over `FileSquidClient` and the file routes. */
export class ClientHistorySource implements HistorySource {
  /** @param client The API client (its transport adapter and token are already chosen). */
  constructor(private readonly client: HistoryClient) {}

  /**
   * Read the versions that changed one file with `GET /api/v1/files/history`. Network I/O.
   *
   * @param file The file (its version is not sent: the history covers every version).
   * @param options Abort signal.
   * @returns The history, oldest first.
   * @throws ApiError when the API refuses.
   */
  history(file: FileRef, options: SourceOptions = {}): Promise<FileHistory> {
    return this.client.request<FileHistory>(`/api/v1/files/history${fileQuery(file)}`, {
      signal: options.signal,
    });
  }

  /**
   * Read every version of an object with `GET /api/v1/files/versions`. Network I/O.
   *
   * @param object The library and object.
   * @param options Abort signal.
   * @returns The versions, oldest first, and the head.
   * @throws ApiError when the API refuses.
   */
  versions(object: ObjectRef, options: SourceOptions = {}): Promise<ObjectVersions> {
    const path = `/api/v1/files/versions${query({
      library_id: object.library_id,
      object_id: object.object_id,
    })}`;
    return this.client.request<ObjectVersions>(path, { signal: options.signal });
  }

  /**
   * Read the bytes one file's history keeps with `GET /api/v1/files/history-size`. Network I/O.
   *
   * @param file The file.
   * @param options Abort signal.
   * @returns The versions, distinct contents and bytes.
   * @throws ApiError when the API refuses.
   */
  historySize(file: FileRef, options: SourceOptions = {}): Promise<FileHistorySize> {
    return this.client.request<FileHistorySize>(`/api/v1/files/history-size${fileQuery(file)}`, {
      signal: options.signal,
    });
  }
}
