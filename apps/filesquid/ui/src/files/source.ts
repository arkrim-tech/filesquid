/**
 * The file operations `<fs-file-browser>` and its previews and panels use (UI-F2.2): the
 * {@link FileSource} interface, shaped on the API's file routes (API-F1.3), and
 * {@link ClientFileSource}, which answers it through `FileSquidClient`. The elements never
 * fetch by themselves, so a fixture ({@link MemoryFileSource}) stands in for the API in tests
 * and a later source (a library-level listing, the browser host) plugs in unchanged.
 *
 * Each write method is one API call that commits one OCFL version (API-S-008); each name says
 * whether it reads, writes or mints, so the side effect is obvious at the call site.
 */
import type { ContentHandleRequest, FileSquidClient } from "../api/client";
import type { ByteRange } from "../api/commands";
import type {
  FileCommit,
  FileDetails,
  FileLocation,
  FileRef,
  FolderAnswer,
  FolderPage,
  ObjectRef,
} from "./model";
import { PAGE_SIZE } from "./model";

/** Cancels a call that is no longer needed. */
export interface SourceOptions {
  /** Cancels the call. */
  readonly signal?: AbortSignal;
}

/** Everything the file browser asks of the API. */
export interface FileSource {
  /** List one page of a folder at the head version. Reads. */
  list(
    location: FileLocation,
    cursor?: string | null,
    limit?: number,
    options?: SourceOptions,
  ): Promise<FolderPage>;
  /** Read the storage facts of one file version. Reads. */
  details(file: FileRef, options?: SourceOptions): Promise<FileDetails>;
  /** Read an inclusive byte range of one file version (at most 16 MiB per call). Reads. */
  readRange(file: FileRef, range: ByteRange, options?: SourceOptions): Promise<Uint8Array>;
  /** Mint a content handle for one file version; the URL is only a media `src`. Mints. */
  mintContent(file: FileRef, options?: SourceOptions): Promise<string>;
  /** Add or replace one file (at most 64 MiB); commits one version. Writes. */
  upload(object: ObjectRef, path: string, body: Blob, options?: SourceOptions): Promise<FileCommit>;
  /** Create a folder; it stays pending until a file is added beneath it. Writes nothing. */
  createFolder(object: ObjectRef, path: string, options?: SourceOptions): Promise<FolderAnswer>;
  /** Move or rename one file or folder; commits one version. Writes. */
  move(object: ObjectRef, from: string, to: string, options?: SourceOptions): Promise<FileCommit>;
  /** Remove one file or folder (its bytes stay in history); commits one version. Writes. */
  remove(object: ObjectRef, path: string, options?: SourceOptions): Promise<FileCommit>;
  /** Restore files or folders from an earlier version; commits one version. Writes. */
  restore(
    object: ObjectRef,
    version: string,
    paths: readonly string[],
    options?: SourceOptions,
  ): Promise<FileCommit>;
}

/** The part of `FileSquidClient` the source needs (injectable in tests). */
export type FileClient = Pick<
  FileSquidClient,
  "request" | "read" | "putBytes" | "mintContentHandle"
>;

/** Build a query string from the defined, non-empty values. */
function query(values: Readonly<Record<string, string | undefined>>): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(values)) {
    if (value !== undefined && value !== "") params.set(key, value);
  }
  return `?${params.toString()}`;
}

/** The query naming one file version. */
function fileQuery(file: FileRef): string {
  return query({
    library_id: file.library_id,
    object_id: file.object_id,
    version: file.version,
    path: file.path,
  });
}

/** The two fields that name an object in a JSON body. */
function bodyOf(object: ObjectRef): ObjectRef {
  return { library_id: object.library_id, object_id: object.object_id };
}

/** A {@link FileSource} over `FileSquidClient` and the file routes. */
export class ClientFileSource implements FileSource {
  /** @param client The API client (its transport adapter and token are already chosen). */
  constructor(private readonly client: FileClient) {}

  /**
   * List one page of a folder. Network I/O.
   *
   * @param location The object and folder.
   * @param cursor The previous page's `next_cursor`, if any.
   * @param limit Page size (default 200).
   * @param options Abort signal.
   * @returns The page.
   * @throws ApiError when the API refuses.
   */
  list(
    location: FileLocation,
    cursor?: string | null,
    limit: number = PAGE_SIZE,
    options: SourceOptions = {},
  ): Promise<FolderPage> {
    const path = `/api/v1/files/listing${query({
      library_id: location.library_id,
      object_id: location.object_id,
      prefix: location.prefix,
      cursor: cursor ?? undefined,
      limit: String(limit),
    })}`;
    return this.client.request<FolderPage>(path, { signal: options.signal });
  }

  /**
   * Read the storage facts of one file version. Network I/O.
   *
   * @param file The file version.
   * @param options Abort signal.
   * @returns The facts.
   * @throws ApiError when the API refuses.
   */
  details(file: FileRef, options: SourceOptions = {}): Promise<FileDetails> {
    return this.client.request<FileDetails>(`/api/v1/files/details${fileQuery(file)}`, {
      signal: options.signal,
    });
  }

  /**
   * Read an inclusive byte range with one `Range` header. The body is consumed and cut to the
   * range, so a host that ignores `Range` cannot make the preview hold a whole file. Network I/O.
   *
   * @param file The file version.
   * @param range The inclusive range.
   * @param options Abort signal.
   * @returns The bytes, at most `range.end - range.start + 1` of them.
   * @throws ApiError when the API refuses.
   */
  async readRange(
    file: FileRef,
    range: ByteRange,
    options: SourceOptions = {},
  ): Promise<Uint8Array> {
    const answer = await this.client.read(`/api/v1/files/content${fileQuery(file)}`, {
      range,
      signal: options.signal,
    });
    const bytes = new Uint8Array(await answer.response.arrayBuffer());
    const limit = range.end === undefined ? bytes.byteLength : range.end - range.start + 1;
    return bytes.byteLength > limit ? bytes.slice(0, limit) : bytes;
  }

  /**
   * Mint a content handle (`POST /api/v1/content-handles`) and return its URL. The URL is a
   * credential: only a media element's `src` may hold it. Network I/O.
   *
   * @param file The file version (a concrete version, never `head`).
   * @param options Abort signal.
   * @returns The URL on the active host.
   * @throws ApiError when the mint is refused.
   */
  async mintContent(file: FileRef, options: SourceOptions = {}): Promise<string> {
    const request: ContentHandleRequest = {
      library_id: file.library_id,
      object_id: file.object_id,
      version: file.version,
      path: file.path,
    };
    return (await this.client.mintContentHandle(request, options)).url;
  }

  /**
   * Add or replace one file with `PUT /api/v1/files/upload`. Network I/O; commits a version.
   *
   * @param object The library and object.
   * @param path The file's logical path.
   * @param body The bytes (at most 64 MiB; the caller checks first).
   * @param options Abort signal.
   * @returns The version published.
   * @throws ApiError when the API refuses, `conflict` included.
   */
  upload(
    object: ObjectRef,
    path: string,
    body: Blob,
    options: SourceOptions = {},
  ): Promise<FileCommit> {
    const route = `/api/v1/files/upload${query({ ...bodyOf(object), path })}`;
    return this.client.putBytes<FileCommit>(route, body, options);
  }

  /**
   * Create a folder with `POST /api/v1/files/folders`. Network I/O; writes nothing.
   *
   * @param object The library and object.
   * @param path The folder's logical path.
   * @param options Abort signal.
   * @returns Whether the folder exists or stays pending.
   * @throws ApiError when the API refuses, `conflict` included.
   */
  createFolder(
    object: ObjectRef,
    path: string,
    options: SourceOptions = {},
  ): Promise<FolderAnswer> {
    return this.post<FolderAnswer>("folders", { ...bodyOf(object), path }, options);
  }

  /**
   * Move or rename one file or folder with `POST /api/v1/files/moves`. Network I/O; commits a
   * version.
   *
   * @param object The library and object.
   * @param from The file or folder to move.
   * @param to Its new logical path.
   * @param options Abort signal.
   * @returns The version published.
   * @throws ApiError when the API refuses, `conflict` included.
   */
  move(
    object: ObjectRef,
    from: string,
    to: string,
    options: SourceOptions = {},
  ): Promise<FileCommit> {
    return this.post<FileCommit>("moves", { ...bodyOf(object), from, to }, options);
  }

  /**
   * Remove one file or folder with `POST /api/v1/files/removals`. Network I/O; commits a
   * version; the bytes stay in history.
   *
   * @param object The library and object.
   * @param path The file or folder.
   * @param options Abort signal.
   * @returns The version published.
   * @throws ApiError when the API refuses, `conflict` included.
   */
  remove(object: ObjectRef, path: string, options: SourceOptions = {}): Promise<FileCommit> {
    return this.post<FileCommit>("removals", { ...bodyOf(object), path }, options);
  }

  /**
   * Restore files or folders from an earlier version with `POST /api/v1/files/restorations`.
   * Network I/O; commits a version.
   *
   * @param object The library and object.
   * @param version The earlier version: `v1`, `v2`, ...
   * @param paths The files or folders (1 to 1,000).
   * @param options Abort signal.
   * @returns The version published.
   * @throws ApiError when the API refuses, `conflict` included.
   */
  restore(
    object: ObjectRef,
    version: string,
    paths: readonly string[],
    options: SourceOptions = {},
  ): Promise<FileCommit> {
    return this.post<FileCommit>("restorations", { ...bodyOf(object), version, paths }, options);
  }

  /** `POST /api/v1/files/<route>` with a JSON body. Network I/O. */
  private post<T>(route: string, body: unknown, options: SourceOptions): Promise<T> {
    return this.client.request<T>(`/api/v1/files/${route}`, {
      method: "POST",
      body,
      signal: options.signal,
    });
  }
}
