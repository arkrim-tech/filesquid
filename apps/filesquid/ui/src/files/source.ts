/**
 * The file operations `<fs-file-browser>` and its previews and panels use (UI-F2.2): the
 * {@link FileSource} interface, shaped on the API's file routes (API-F1.3), and
 * {@link ClientFileSource}, which answers it through `FileSquidClient`. The elements never
 * fetch by themselves, so a fixture ({@link MemoryFileSource}) stands in for the API in tests
 * and a later source (a library-level listing, the browser host) plugs in unchanged.
 *
 * Each write method is one API call that commits one OCFL version (API-S-008); each name says
 * whether it reads, writes or mints, so the side effect is obvious at the call site. Every write
 * is a command (API-F2.13, UI-S-012): it carries a `CommandEnvelope` with a new operation id and
 * the revision the caller last read, and only a `committed` outcome resolves; a `conflict`
 * (a stale revision, or a write that would change nothing) rejects as `ApiError` 409.
 */
import { ApiError, type ContentHandleRequest, type FileSquidClient } from "../api/client";
import { commandEnvelope, type ByteRange, type CommandAnswer } from "../api/commands";
import type {
  FileCommit,
  FileDetails,
  FileLocation,
  FileRef,
  FolderAnswer,
  FolderPage,
  LibraryRef,
  ObjectRef,
  RenameCommit,
  RenameRequest,
  UploadCommit,
  UploadFacts,
} from "./model";
import { PAGE_SIZE } from "./model";

/** Cancels a call that is no longer needed. */
export interface SourceOptions {
  /** Cancels the call. */
  readonly signal?: AbortSignal;
}

/** The revision a write expects, and a way to cancel it. */
export interface WriteOptions extends SourceOptions {
  /**
   * The head version of the object the write changes, as the caller last read it (`v1`,
   * `v2`, ...); the API refuses a stale one as `conflict` and changes nothing (API-S-002).
   */
  readonly revision: string;
}

/** The revision of an upload's file object before it exists: a new file (API-F2.13). */
export const NEW_FILE_REVISION = "0";

/** Options of an upload: the revision of the file it replaces, its operation id, and a way to cancel it. */
export interface UploadSourceOptions extends SourceOptions {
  /** The file object's head for new content of `file_id`; {@link NEW_FILE_REVISION} by default. */
  readonly revision?: string;
  /**
   * The operation id to send; a new one when absent. Reuse it to retry an upload whose answer
   * was lost: the API then answers the first outcome instead of making a second file.
   */
  readonly operationId?: string;
}

/** Options of a restore: the revision it expects, the new version's optional label, and a way to cancel it. */
export interface RestoreOptions extends WriteOptions {
  /**
   * The new version's label (API-F2.5): 1 to 512 bytes, not blank, no control characters; the
   * label member of the version's `filesquid.operation.v1` message. No label when absent.
   */
  readonly label?: string;
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
  /**
   * Upload one file (at most 64 MiB) as its own file, or as new content of `facts.file_id`;
   * commits one version in that file's object. Writes.
   */
  upload(
    library: LibraryRef,
    facts: UploadFacts,
    body: Blob,
    options?: UploadSourceOptions,
  ): Promise<UploadCommit>;
  /**
   * Create a folder at the object's head `options.revision`; it stays pending until a file is
   * added beneath it. Writes nothing.
   */
  createFolder(object: ObjectRef, path: string, options: WriteOptions): Promise<FolderAnswer>;
  /** Move one file or folder, or rename a folder; commits one version. Writes. */
  move(object: ObjectRef, from: string, to: string, options: WriteOptions): Promise<FileCommit>;
  /** Set a file's display name and nothing else; commits one version of its record. Writes. */
  rename(
    library: LibraryRef,
    fileId: string,
    displayName: string,
    options: WriteOptions,
  ): Promise<RenameCommit>;
  /** Remove one file or folder (its bytes stay in history); commits one version. Writes. */
  remove(object: ObjectRef, path: string, options: WriteOptions): Promise<FileCommit>;
  /**
   * Restore files or folders from an earlier version, with an optional label for the new
   * version; commits one version. Writes.
   */
  restore(
    object: ObjectRef,
    version: string,
    paths: readonly string[],
    options: RestoreOptions,
  ): Promise<FileCommit>;
  /**
   * Set or clear the label of one version after it was committed (API-F2.5); commits one
   * version of the object's labels record. Writes.
   */
  setVersionLabel(
    object: ObjectRef,
    version: string,
    label: string | null,
    options: WriteOptions,
  ): Promise<FileCommit>;
}

/** The part of `FileSquidClient` the source needs (injectable in tests). */
export type FileClient = Pick<
  FileSquidClient,
  "request" | "read" | "command" | "upload" | "mintContentHandle"
>;

/** The upload route (API-S-026). */
const UPLOAD_PATH = "/api/v1/files/upload";

/**
 * The result of a committed command answer. Pure.
 *
 * @param answer The command's answer.
 * @param path The route, named in an error.
 * @returns What the command made.
 * @throws ApiError 409 `conflict` for a `conflict` outcome.
 * @throws TypeError for any other outcome, or a committed one without a result.
 */
function committedResult<T>(answer: CommandAnswer<T>, path: string): T {
  if (answer.kind === "conflict") throw new ApiError(409, path, { code: "conflict" });
  if (answer.kind !== "committed" || answer.result === undefined || answer.result === null) {
    throw new TypeError(`${path} answered ${answer.kind}, not a committed result`);
  }
  return answer.result;
}

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

/**
 * Read `limit` bytes of a response body after skipping `skip` bytes, then cancel the rest, so a
 * host that ignores `Range` cannot make a preview buffer a whole file. Network I/O.
 */
async function readLimited(response: Response, skip: number, limit: number): Promise<Uint8Array> {
  const reader = response.body?.getReader();
  if (!reader) return new Uint8Array(0);
  const chunks: Uint8Array[] = [];
  let seen = 0;
  let kept = 0;
  try {
    for (let guard = 0; guard < 1_000_000 && kept < limit; guard += 1) {
      const { done, value } = await reader.read();
      if (done) break;
      const start = Math.max(0, skip - seen);
      seen += value.byteLength;
      const part = value.subarray(start, start + (limit - kept));
      chunks.push(part);
      kept += part.byteLength;
    }
  } finally {
    await reader.cancel().catch(() => undefined);
  }
  const out = new Uint8Array(kept);
  let at = 0;
  for (const chunk of chunks) {
    out.set(chunk, at);
    at += chunk.byteLength;
  }
  return out;
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
    const wanted = range.end === undefined ? Infinity : range.end - range.start + 1;
    // A host that ignores Range answers the whole file from byte 0: skip to the range start.
    const skip = answer.partial ? 0 : range.start;
    return readLimited(answer.response, skip, wanted);
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
   * Upload one file with `PUT /api/v1/files/upload` (API-S-026, API-S-035, API-F2.13): the
   * envelope, the facts as the `input` part with the declared size from the body, and the
   * bytes; no logical path. Network I/O; commits one version in the file's own object.
   *
   * @param library The library.
   * @param facts The file's name, its normalisation form and the other facts the client gives.
   * @param body The bytes (at most 64 MiB; the caller checks first).
   * @param options The revision of `facts.file_id`'s object (a new file by default) and abort
   *   signal.
   * @returns The version published, with the file's identifier and object.
   * @throws ApiError when the API refuses, `invalid_request` and `conflict` included.
   */
  async upload(
    library: LibraryRef,
    facts: UploadFacts,
    body: Blob,
    options: UploadSourceOptions = {},
  ): Promise<UploadCommit> {
    const input = { ...facts, library_id: library.library_id, size: body.size };
    const envelope = commandEnvelope({
      scope: `library:${library.library_id}`,
      expectedRevision: options.revision ?? NEW_FILE_REVISION,
      operationId: options.operationId,
    });
    const answer = await this.client.upload<UploadCommit>(UPLOAD_PATH, body, envelope, {
      method: "PUT",
      input,
      filename: facts.name,
      signal: options.signal,
    });
    return committedResult(answer, UPLOAD_PATH);
  }

  /**
   * Create a folder with `POST /api/v1/files/folders`. Network I/O; writes nothing.
   *
   * @param object The library and object.
   * @param path The folder's logical path.
   * @param options The object's head as last read, and abort signal.
   * @returns Whether the folder exists or stays pending.
   * @throws ApiError when the API refuses, `conflict` included.
   */
  createFolder(object: ObjectRef, path: string, options: WriteOptions): Promise<FolderAnswer> {
    return this.send<FolderAnswer>("folders", { ...bodyOf(object), path }, options);
  }

  /**
   * Set a file's display name with `POST /api/v1/files/renames` (API-S-027). Network I/O;
   * commits one version of the file's metadata record. The stored name, the logical path and
   * the bytes stay.
   *
   * @param library The library.
   * @param fileId The file (the UUID of its own object).
   * @param displayName The new display name (1 to 1,024 bytes, no `/` or NUL).
   * @param options The file object's head as last read, and abort signal.
   * @returns The version published.
   * @throws ApiError when the API refuses: `invalid_request` for a name it cannot take,
   *   `not_found` for an unknown file, `conflict` for the current name, a stale revision or a
   *   record another writer changed first, `unavailable` when storage is not reachable.
   */
  rename(
    library: LibraryRef,
    fileId: string,
    displayName: string,
    options: WriteOptions,
  ): Promise<RenameCommit> {
    const body: RenameRequest = {
      library_id: library.library_id,
      file_id: fileId,
      display_name: displayName,
    };
    return this.send<RenameCommit>("renames", body, options);
  }

  /**
   * Move one file or folder with `POST /api/v1/files/moves`. Nothing moves from or to
   * `original` or below it in a file's own object: the API refuses that with 400 and points to
   * {@link ClientFileSource.rename}.
   * Network I/O; commits a version.
   *
   * @param object The library and object.
   * @param from The file or folder to move.
   * @param to Its new logical path.
   * @param options The object's head as last read, and abort signal.
   * @returns The version published.
   * @throws ApiError when the API refuses, `conflict` included.
   */
  move(object: ObjectRef, from: string, to: string, options: WriteOptions): Promise<FileCommit> {
    return this.send<FileCommit>("moves", { ...bodyOf(object), from, to }, options);
  }

  /**
   * Remove one file or folder with `POST /api/v1/files/removals`. Network I/O; commits a
   * version; the bytes stay in history.
   *
   * @param object The library and object.
   * @param path The file or folder.
   * @param options The object's head as last read, and abort signal.
   * @returns The version published.
   * @throws ApiError when the API refuses, `conflict` included.
   */
  remove(object: ObjectRef, path: string, options: WriteOptions): Promise<FileCommit> {
    return this.send<FileCommit>("removals", { ...bodyOf(object), path }, options);
  }

  /**
   * Restore files or folders from an earlier version with `POST /api/v1/files/restorations`.
   * Network I/O; commits a version.
   *
   * @param object The library and object.
   * @param version The earlier version: `v1`, `v2`, ...
   * @param paths The files or folders (1 to 1,000).
   * @param options The object's head as last read, the new version's optional label, and
   *   abort signal.
   * @returns The version published.
   * @throws ApiError when the API refuses, `conflict` included.
   */
  restore(
    object: ObjectRef,
    version: string,
    paths: readonly string[],
    options: RestoreOptions,
  ): Promise<FileCommit> {
    const label = options.label === undefined ? {} : { label: options.label };
    const body = { ...bodyOf(object), version, paths, ...label };
    return this.send<FileCommit>("restorations", body, options);
  }

  /**
   * Set or clear a version's label with `PUT /api/v1/files/labels`. Network I/O; commits a
   * version. The API refuses a label that would change nothing as `conflict`, so callers
   * compare first.
   *
   * @param object The library and object.
   * @param version The version to label: `v1`, `v2`, ...
   * @param label The label (1 to 512 bytes, not blank, no control characters), or null to
   *   clear it.
   * @param options The object's head as last read, and abort signal.
   * @returns The version published.
   * @throws ApiError when the API refuses, `conflict` included.
   */
  setVersionLabel(
    object: ObjectRef,
    version: string,
    label: string | null,
    options: WriteOptions,
  ): Promise<FileCommit> {
    const body = { ...bodyOf(object), version, label };
    return this.send<FileCommit>("labels", body, options, "PUT");
  }

  /**
   * `POST` (or `PUT`) `/api/v1/files/<route>` of the command `{ envelope, input }` with a new
   * operation id, scoped to `input`'s library and expecting `options.revision`. Network I/O.
   */
  private async send<T>(
    route: string,
    input: { readonly library_id: string; readonly [field: string]: unknown },
    options: WriteOptions,
    method?: "PUT",
  ): Promise<T> {
    const path = `/api/v1/files/${route}`;
    const envelope = commandEnvelope({
      scope: `library:${input.library_id}`,
      expectedRevision: options.revision,
    });
    const answer = await this.client.command<T>(path, envelope, input, {
      signal: options.signal,
      ...(method ? { method } : {}),
    });
    return committedResult(answer, path);
  }
}
