/**
 * The metadata reads and writes of the Description tab (UI-F2.4): the {@link MetadataSource}
 * interface over the metadata property routes of API-F1.4 (API-S-035) and
 * {@link ClientMetadataSource}, which answers it through `FileSquidClient` (UI-S-012). The
 * element never fetches by itself, so a fixture stands in for the API in tests.
 *
 * Each method's name says whether it reads or writes, so the side effect is obvious at the
 * call site. The set and remove routes take plain JSON bodies today (no `CommandEnvelope`;
 * owner decision open) and these two methods are their only call sites, so an envelope can be
 * added here later without touching the element.
 */
import type { FileSquidClient } from "../api/client";
import type { FileStatements, MetadataCommit, Namespaces, TermCandidates } from "./metadata";
import type { FileRef, LibraryRef } from "./model";
import { fileIdOf } from "./ops";
import type { SourceOptions } from "./source";

/** Everything the Description tab asks of the API. */
export interface MetadataSource {
  /** The file's populated statements at its version. Reads. */
  statements(file: FileRef, options?: SourceOptions): Promise<FileStatements>;
  /** Set or replace one statement; commits one version of the file's object. Writes. */
  setStatement(
    file: FileRef,
    curie: string,
    value: string,
    options?: SourceOptions,
  ): Promise<MetadataCommit>;
  /** Remove one statement; commits one version of the file's object. Writes. */
  removeStatement(file: FileRef, curie: string, options?: SourceOptions): Promise<MetadataCommit>;
  /** Where a bare term name can go, in the add order. Reads. */
  placeTerm(library: LibraryRef, name: string, options?: SourceOptions): Promise<TermCandidates>;
  /** The library's namespace registry. Reads. */
  namespaces(library: LibraryRef, options?: SourceOptions): Promise<Namespaces>;
}

/** The part of `FileSquidClient` the source needs (injectable in tests). */
export type MetadataClient = Pick<FileSquidClient, "request">;

/** The statements route. */
const STATEMENTS_PATH = "/api/v1/files/metadata";
/** The set route. */
const SET_PATH = "/api/v1/files/metadata/statements";
/** The remove route. */
const REMOVE_PATH = "/api/v1/files/metadata/removals";
/** The term placement route. */
const TERMS_PATH = "/api/v1/files/metadata/terms";
/** The registry route. */
const NAMESPACES_PATH = "/api/v1/files/metadata/namespaces";

/** A query string of the given values, in order. */
function query(values: Readonly<Record<string, string>>): string {
  return `?${new URLSearchParams(values).toString()}`;
}

/**
 * The file id of a file reference, from its object identifier `urn:uuid:<file-id>`
 * (API-S-026). Pure.
 *
 * @param file The file.
 * @returns The id.
 * @throws TypeError when the object is not a file's own object, so no request is sent.
 */
export function metadataFileId(file: FileRef): string {
  const id = fileIdOf(file.object_id);
  if (id === undefined) throw new TypeError("The object is not a file object");
  return id;
}

/** A {@link MetadataSource} over `FileSquidClient` and the metadata property routes. */
export class ClientMetadataSource implements MetadataSource {
  /** @param client The API client (its transport adapter and token are already chosen). */
  constructor(private readonly client: MetadataClient) {}

  /**
   * Read the file's statements at its version with `GET /api/v1/files/metadata`. Network I/O.
   *
   * @param file The file at one version.
   * @param options Abort signal.
   * @returns The populated statements, in record order.
   * @throws TypeError for a file that is not in its own object (nothing is sent).
   * @throws ApiError when the API refuses.
   */
  async statements(file: FileRef, options: SourceOptions = {}): Promise<FileStatements> {
    const file_id = metadataFileId(file);
    const path = `${STATEMENTS_PATH}${query({
      library_id: file.library_id,
      file_id,
      version: file.version,
    })}`;
    return this.client.request<FileStatements>(path, { signal: options.signal });
  }

  /**
   * Set or replace one statement with `PUT /api/v1/files/metadata/statements`. Network I/O.
   *
   * @param file The file.
   * @param curie `prefix:term`.
   * @param value The value, 1 to 65,536 bytes.
   * @param options Abort signal.
   * @returns The version committed.
   * @throws TypeError for a file that is not in its own object (nothing is sent).
   * @throws ApiError 400 for an unknown name or prefix, 409 when nothing would change or
   *   another writer moved the record first.
   */
  async setStatement(
    file: FileRef,
    curie: string,
    value: string,
    options: SourceOptions = {},
  ): Promise<MetadataCommit> {
    const file_id = metadataFileId(file);
    return this.client.request<MetadataCommit>(SET_PATH, {
      method: "PUT",
      body: { library_id: file.library_id, file_id, curie, value },
      signal: options.signal,
    });
  }

  /**
   * Remove one statement with `POST /api/v1/files/metadata/removals`. Network I/O.
   *
   * @param file The file.
   * @param curie `prefix:term`.
   * @param options Abort signal.
   * @returns The version committed.
   * @throws TypeError for a file that is not in its own object (nothing is sent).
   * @throws ApiError 404 when the record does not hold it, 409 when another writer moved the
   *   record first.
   */
  async removeStatement(
    file: FileRef,
    curie: string,
    options: SourceOptions = {},
  ): Promise<MetadataCommit> {
    const file_id = metadataFileId(file);
    return this.client.request<MetadataCommit>(REMOVE_PATH, {
      method: "POST",
      body: { library_id: file.library_id, file_id, curie },
      signal: options.signal,
    });
  }

  /**
   * Ask where a bare term name can go with `GET /api/v1/files/metadata/terms`. Network I/O.
   *
   * @param library The library whose registry applies.
   * @param name The bare name, 1 to 128 bytes without `:`.
   * @param options Abort signal.
   * @returns The candidates in the add order.
   * @throws ApiError when the API refuses.
   */
  placeTerm(
    library: LibraryRef,
    name: string,
    options: SourceOptions = {},
  ): Promise<TermCandidates> {
    const path = `${TERMS_PATH}${query({ library_id: library.library_id, name })}`;
    return this.client.request<TermCandidates>(path, { signal: options.signal });
  }

  /**
   * Read the library's registry with `GET /api/v1/files/metadata/namespaces`. Network I/O.
   *
   * @param library The library.
   * @param options Abort signal.
   * @returns Every registration, in registration order.
   * @throws ApiError when the API refuses.
   */
  namespaces(library: LibraryRef, options: SourceOptions = {}): Promise<Namespaces> {
    const path = `${NAMESPACES_PATH}${query({ library_id: library.library_id })}`;
    return this.client.request<Namespaces>(path, { signal: options.signal });
  }
}
