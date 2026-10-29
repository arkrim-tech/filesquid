/**
 * An in-memory {@link FileSource} over a fixture object, answering as the file routes do
 * (API-F1.3): folders first then files in code-point order, a display name apart from the
 * stored name, opaque cursors with a bounded page, ranged reads, one new OCFL version per
 * write, restore from an earlier version, a folder that stays pending until a file is added,
 * an upload that creates the file's own object (API-S-026) and refuses the facts the API
 * refuses, and the API's refusals as `ApiError` (`not_found`, `conflict`, `invalid_request`,
 * `range_not_satisfiable`). It also answers the history reads of the file panel (UI-F2.3) as
 * `GET /files/history`, `/files/versions` and `/files/history-size` do. Tests and demos use it;
 * the live sources are `ClientFileSource` and `ClientHistorySource`.
 */
import { ApiError } from "../api/client";
import { newOperationId, type ByteRange } from "../api/commands";
import type {
  FileCommit,
  FileDetails,
  FileLocation,
  FileRef,
  FolderAnswer,
  FolderEntry,
  FolderPage,
  LibraryRef,
  NameNormalization,
  ObjectRef,
  UploadCommit,
  UploadFacts,
} from "./model";
import { MAX_UPLOAD_BYTES, isControl, isMediaType, lastSegment, validLabel } from "./model";
import { fileIdOf, inOriginalFolder, isOriginalPath } from "./ops";
import {
  NEW_FILE_REVISION,
  type FileSource,
  type SourceOptions,
  type UploadSourceOptions,
  type RestoreOptions,
  type WriteOptions,
} from "./source";
import type {
  FileHistory,
  FileHistoryEntry,
  FileHistorySize,
  HistorySource,
  ObjectVersions,
} from "./history-source";

/** One file of the fixture. */
export interface MemoryFile {
  /** Logical path; its last segment is the stored name. */
  readonly path: string;
  /** The name users see; the stored name when absent. */
  readonly displayName?: string;
  /** Content: text, or bytes. */
  readonly content: string | Uint8Array;
  /** Media type; from the extension when absent. */
  readonly mediaType?: string;
}

/** The fixture object. */
export interface MemoryOptions extends ObjectRef {
  /** The files of version `v1`. */
  readonly files: readonly MemoryFile[];
}

/** One file an upload made, at its latest version, as the fixture keeps it. */
export interface MemoryUpload {
  /** The file (UUID v4); its object is `urn:uuid:<file_id>`. */
  readonly file_id: string;
  /** The name the upload sent. */
  readonly name: string;
  /** Its normalisation form: the one sent, or `nfc` for an ASCII name. */
  readonly name_normalization: NameNormalization;
  /** The media type sent, or null. */
  readonly media_type: string | null;
  /** The last-modified time sent, or null. */
  readonly last_modified: string | null;
  /** Bytes received. */
  readonly size: number;
  /** The file object's head version. */
  readonly version: string;
}

/** An uploaded file with its bytes. */
interface UploadedFile {
  /** What the upload recorded. */
  readonly upload: MemoryUpload;
  /** The bytes received. */
  readonly bytes: Uint8Array;
}

/** A file as stored in one version. */
interface Stored {
  readonly path: string;
  readonly displayName: string;
  readonly bytes: Uint8Array;
  readonly mediaType: string;
  readonly modified: string;
  readonly modifiedVersion: string;
}

/** One version: when it was made and the files it holds. */
interface Snapshot {
  readonly created: string;
  readonly files: ReadonlyMap<string, Stored>;
  /** The version's label, as a restore sets it; null for none. */
  readonly label: string | null;
}

/** Route named in the fixture's errors. */
const ROUTE = "/api/v1/files";
/** Default page size, as the API's. */
const DEFAULT_LIMIT = 200;
/** Largest page size the API accepts. */
const MAX_LIMIT = 1000;
/** Most cursors remembered; the oldest is forgotten first. */
const MAX_CURSORS = 1000;
/** Longest uploaded name the API accepts, in UTF-8 bytes. */
const MAX_UPLOAD_NAME_BYTES = 1024;
/** The shape of an RFC 3339 UTC time: date, clock and an optional fraction of any digits. */
const RFC3339_UTC = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d+)?Z$/;
/** A UUID v4 in lowercase or uppercase hex. */
const UUID_V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
/** Time of `v1`; each later version is one minute after the one before. */
const BASE_TIME = Date.UTC(2026, 9, 16, 9, 0, 0);

/** Media types by extension. */
const TYPES: Readonly<Record<string, string>> = {
  txt: "text/plain",
  csv: "text/csv",
  json: "application/json",
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  mp4: "video/mp4",
  webm: "video/webm",
  wav: "audio/wav",
  mp3: "audio/mpeg",
  pdf: "application/pdf",
};

/** An `ApiError` with a contract code. */
function refusal(status: number, code: "not_found" | "conflict" | "invalid_request"): ApiError {
  return new ApiError(status, ROUTE, { code });
}

/** Whether a display name is one the rename route takes: 1 to 1,024 bytes, no `/` or NUL. */
function validDisplayName(name: string): boolean {
  const bytes = new TextEncoder().encode(name).length;
  return bytes > 0 && bytes <= MAX_UPLOAD_NAME_BYTES && !name.includes("/") && !name.includes("\0");
}

/**
 * The display name as the API shows it (API-S-005): control characters removed, `/` and `\`
 * replaced by `_`, surrounding spaces trimmed, at most 1,024 bytes kept, and `file` for a name
 * left empty. A rename stores the name as sent; every listing and detail shows this form. Pure.
 */
function safeDisplayName(name: string): string {
  const cleaned = Array.from(name, (ch) => (ch === "/" || ch === "\\" ? "_" : ch))
    .filter((ch) => !isControl(ch))
    .join("")
    .trim();
  let kept = "";
  for (const ch of cleaned) {
    if (new TextEncoder().encode(kept + ch).length > MAX_UPLOAD_NAME_BYTES) break;
    kept += ch;
  }
  return kept.trim() === "" ? "file" : kept.trim();
}

/** The lowercase extension of a stored name, or null. */
function extensionOf(name: string): string | null {
  const dot = name.lastIndexOf(".");
  return dot > 0 && dot < name.length - 1 ? name.slice(dot + 1).toLowerCase() : null;
}

/**
 * The bytes of a Blob: `arrayBuffer()` where the platform has it (browsers, Node), otherwise
 * `FileReader` (jsdom).
 */
async function readBlob(blob: Blob): Promise<Uint8Array> {
  if (typeof blob.arrayBuffer === "function") return new Uint8Array(await blob.arrayBuffer());
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer));
    reader.onerror = () => reject(reader.error ?? new Error("FileReader failed"));
    reader.readAsArrayBuffer(blob);
  });
}

/** True when `path` is `base` or lies below it. */
function within(path: string, base: string): boolean {
  return path === base || path.startsWith(`${base}/`);
}

/** Compare by Unicode code point, as the API orders names. */
function byName(a: string, b: string): number {
  const left = Array.from(a, (ch) => ch.codePointAt(0) ?? 0);
  const right = Array.from(b, (ch) => ch.codePointAt(0) ?? 0);
  for (let i = 0; i < Math.min(left.length, right.length); i += 1) {
    if (left[i] !== right[i]) return (left[i] as number) < (right[i] as number) ? -1 : 1;
  }
  return left.length - right.length;
}

/** A file source answering from memory. */
export class MemoryFileSource implements FileSource, HistorySource {
  private readonly snapshots: Snapshot[] = [];
  private readonly cursors = new Map<string, { prefix: string; offset: number }>();
  private readonly uploaded = new Map<string, UploadedFile>();
  /** Uploads committed so far; each one's `created` is a minute after the one before. */
  private uploadCount = 0;
  /** When set, every call rejects with this value (a network error, a denial, ...). */
  failure: unknown = undefined;
  /** Every handle URL minted, in order, with the file it was minted for. */
  readonly mints: { readonly file: FileRef; readonly url: string }[] = [];

  /** @param options The object and the files of `v1`. */
  constructor(private readonly options: MemoryOptions) {
    const created = new Date(BASE_TIME).toISOString();
    const files = new Map<string, Stored>();
    for (const file of options.files) files.set(file.path, initialFile(file, created));
    this.snapshots.push({ created, files, label: null });
  }

  /** The head version label. */
  get head(): string {
    return `v${this.snapshots.length}`;
  }

  /** The files uploads made, in the order they were first uploaded, for assertions. */
  get uploads(): MemoryUpload[] {
    return [...this.uploaded.values()].map((file) => file.upload);
  }

  /** The files of the head version, for assertions. */
  get paths(): string[] {
    return [...this.latest().files.keys()].sort(byName);
  }

  /** List one page of a folder at the head version. */
  async list(
    location: FileLocation,
    cursor?: string | null,
    limit: number = DEFAULT_LIMIT,
    options?: SourceOptions,
  ): Promise<FolderPage> {
    if (options?.signal?.aborted) throw new DOMException("aborted", "AbortError");
    this.check();
    if (!Number.isInteger(limit) || limit < 1 || limit > MAX_LIMIT)
      throw refusal(400, "invalid_request");
    const all = this.children(location.prefix);
    const start = cursor == null ? 0 : this.offsetOf(cursor, location.prefix);
    const next = start + limit < all.length ? this.issue(location.prefix, start + limit) : null;
    return {
      library_id: this.options.library_id,
      object_id: this.options.object_id,
      version: this.head,
      prefix: location.prefix,
      entries: all.slice(start, start + limit),
      next_cursor: next,
    };
  }

  /** The storage facts of one file version. */
  async details(file: FileRef): Promise<FileDetails> {
    this.check();
    const stored = this.stored(file);
    const name = lastSegment(stored.path);
    return {
      library_id: this.options.library_id,
      object_id: this.options.object_id,
      version: file.version,
      path: stored.path,
      name,
      display_name: safeDisplayName(stored.displayName),
      digest: `fixture-${stored.bytes.byteLength}`,
      digest_algorithm: "sha512",
      size: stored.bytes.byteLength,
      fixity: {},
      last_change: {
        version: stored.modifiedVersion,
        created: stored.modified,
        message: null,
        user: null,
      },
      type_hint: extensionOf(name),
      media_type: stored.mediaType,
    };
  }

  /** An inclusive byte range of one file version. */
  async readRange(file: FileRef, range: ByteRange): Promise<Uint8Array> {
    this.check();
    const { bytes } = this.stored(file);
    if (range.start >= bytes.byteLength) {
      throw new ApiError(416, ROUTE, { code: "range_not_satisfiable" });
    }
    return bytes.slice(range.start, (range.end ?? bytes.byteLength - 1) + 1);
  }

  /** A fixture URL for one file version; each call returns a different one. */
  async mintContent(file: FileRef): Promise<string> {
    this.check();
    this.stored(file);
    const url = `memory://content/${this.mints.length + 1}`;
    this.mints.push({ file, url });
    return url;
  }

  /**
   * Upload one file as the route does (API-S-026): a new file object at `v1`, or the next
   * version of `facts.file_id`'s object. The object shown is not changed. Refuses what the
   * route refuses: a body over 64 MiB, a bad name, media type, time or `file_id`, or a
   * non-ASCII name without its form (`invalid_request`); an unknown file (`not_found`);
   * unchanged bytes, or a revision that is not the file's head (`0` for a new file)
   * (`conflict`).
   */
  async upload(
    library: LibraryRef,
    facts: UploadFacts,
    body: Blob,
    options: UploadSourceOptions = {},
  ): Promise<UploadCommit> {
    this.check();
    const form = uploadForm(facts);
    if (form === null || body.size > MAX_UPLOAD_BYTES) throw refusal(400, "invalid_request");
    const bytes = await readBlob(body);
    const previous = facts.file_id === undefined ? undefined : this.uploaded.get(facts.file_id);
    if (facts.file_id !== undefined && previous === undefined) throw refusal(404, "not_found");
    const head = previous?.upload.version ?? NEW_FILE_REVISION;
    if ((options.revision ?? NEW_FILE_REVISION) !== head) throw refusal(409, "conflict");
    if (previous && sameBytes(previous.bytes, bytes)) throw refusal(409, "conflict");
    const file_id = previous?.upload.file_id ?? newOperationId();
    const version = `v${previous ? Number(previous.upload.version.slice(1)) + 1 : 1}`;
    const upload: MemoryUpload = {
      file_id,
      name: facts.name,
      name_normalization: form,
      media_type: facts.media_type ?? null,
      last_modified: facts.last_modified ?? null,
      size: bytes.byteLength,
      version,
    };
    this.uploaded.set(file_id, { upload, bytes });
    this.uploadCount += 1;
    const created = new Date(BASE_TIME + this.uploadCount * 60_000).toISOString();
    const object_id = `urn:uuid:${file_id}`;
    return { library_id: library.library_id, file_id, object_id, version, created };
  }

  /** Create a folder: pending when no file lies below it yet. */
  async createFolder(
    _object: ObjectRef,
    path: string,
    options: WriteOptions,
  ): Promise<FolderAnswer> {
    this.check();
    this.expect(options.revision);
    const files = [...this.latest().files.keys()];
    const blocked = files.some((file) => within(path, file));
    if (blocked) throw refusal(409, "conflict");
    const exists = files.some((file) => file.startsWith(`${path}/`));
    return {
      library_id: this.options.library_id,
      object_id: this.options.object_id,
      path,
      pending: !exists,
    };
  }

  /**
   * Set the display name of the file this object holds (`POST /api/v1/files/renames`,
   * API-S-027): one new version that changes only the display name. Refuses what the route
   * refuses: an empty name, one over 1,024 bytes or with `/` or NUL (`invalid_request`); a file
   * this object does not hold (`not_found`); the current name, or a revision that is not the
   * head (`conflict`).
   */
  async rename(
    library: LibraryRef,
    fileId: string,
    displayName: string,
    options: WriteOptions,
  ): Promise<FileCommit> {
    this.check();
    const held = this.holdsFile(fileId) && library.library_id === this.options.library_id;
    if (!held) throw refusal(404, "not_found");
    if (!validDisplayName(displayName)) throw refusal(400, "invalid_request");
    this.expect(options.revision);
    return this.commit((files) => {
      for (const file of [...files.values()].filter((f) => isOriginalPath(f.path, fileId))) {
        files.set(file.path, { ...file, displayName });
      }
    });
  }

  /** True when this object is file `fileId`'s own object: it holds that file's stored original. */
  private holdsFile(fileId: string): boolean {
    const own = fileIdOf(this.options.object_id)?.toLowerCase() === fileId.toLowerCase();
    return own && [...this.latest().files.keys()].some((path) => isOriginalPath(path, fileId));
  }

  /**
   * Move one file or folder. As the API does, it refuses (`invalid_request`) a move from or to
   * `original` or below it in a file's own object, which only a rename changes.
   */
  async move(
    _object: ObjectRef,
    from: string,
    to: string,
    options: WriteOptions,
  ): Promise<FileCommit> {
    this.check();
    const id = fileIdOf(this.options.object_id);
    const own = id !== undefined && this.holdsFile(id);
    if (own && (inOriginalFolder(from) || inOriginalFolder(to))) {
      throw refusal(400, "invalid_request");
    }
    this.expect(options.revision);
    return this.commit((files, version, created) => {
      const moving = [...files.keys()].filter((path) => within(path, from));
      if (moving.length === 0) throw refusal(404, "not_found");
      for (const path of moving) {
        const entry = files.get(path) as Stored;
        files.delete(path);
        const target = `${to}${path.slice(from.length)}`;
        files.set(target, { ...entry, path: target, modified: created, modifiedVersion: version });
      }
    });
  }

  /** Remove one file or folder; earlier versions keep it. */
  async remove(_object: ObjectRef, path: string, options: WriteOptions): Promise<FileCommit> {
    this.check();
    this.expect(options.revision);
    return this.commit((files) => {
      const removing = [...files.keys()].filter((candidate) => within(candidate, path));
      if (removing.length === 0) throw refusal(404, "not_found");
      for (const candidate of removing) files.delete(candidate);
    });
  }

  /**
   * Restore files or folders from an earlier version as a new version, with its optional label.
   * A label the restorations route refuses (blank, over 512 bytes or with a control character)
   * is `invalid_request`, checked before the revision as the route checks its body first.
   */
  async restore(
    _object: ObjectRef,
    version: string,
    paths: readonly string[],
    options: RestoreOptions,
  ): Promise<FileCommit> {
    this.check();
    if (options.label !== undefined && !validLabel(options.label)) {
      throw refusal(400, "invalid_request");
    }
    this.expect(options.revision);
    const earlier = this.snapshot(version);
    const change = (files: Map<string, Stored>, label: string, created: string) => {
      for (const path of paths) {
        const found = [...earlier.files.values()].filter((file) => within(file.path, path));
        if (found.length === 0) throw refusal(404, "not_found");
        for (const file of found)
          files.set(file.path, { ...file, modified: created, modifiedVersion: label });
      }
    };
    return this.commit(change, options.label ?? null);
  }

  /** Every version of the object, oldest first, with the head. */
  async versions(): Promise<ObjectVersions> {
    this.check();
    return {
      library_id: this.options.library_id,
      object_id: this.options.object_id,
      head: this.head,
      display_name: this.fileName(),
      versions: this.snapshots.map((snapshot, i) => versionRecord(i, snapshot)),
    };
  }

  /** The display name of the file this object holds when it is a file object, else null. */
  private fileName(): string | null {
    const id = fileIdOf(this.options.object_id);
    const files = [...this.latest().files.values()];
    const original = id ? files.find((file) => isOriginalPath(file.path, id)) : undefined;
    return original ? safeDisplayName(original.displayName) : null;
  }

  /** The versions that changed the file at `file.path`, oldest first, or `not_found`. */
  async history(file: FileRef): Promise<FileHistory> {
    this.check();
    const entries: FileHistoryEntry[] = [];
    this.snapshots.forEach((snapshot, i) => {
      const before = this.snapshots[i - 1]?.files.get(file.path);
      const entry = historyEntry(i, snapshot, before, snapshot.files.get(file.path));
      if (entry) entries.push(entry);
    });
    const shown = this.latest().files.get(file.path) ?? this.snapshots[0]?.files.get(file.path);
    if (entries.length === 0) throw refusal(404, "not_found");
    return {
      library_id: this.options.library_id,
      object_id: this.options.object_id,
      path: file.path,
      display_name: safeDisplayName(shown?.displayName ?? lastSegment(file.path)),
      entries,
    };
  }

  /** The bytes the file's history keeps, each distinct content once. */
  async historySize(file: FileRef): Promise<FileHistorySize> {
    const { entries } = await this.history(file);
    const contents = new Map<string, number>();
    for (const entry of entries) {
      if (entry.digest !== null) contents.set(entry.digest, entry.size ?? 0);
    }
    return {
      library_id: this.options.library_id,
      object_id: this.options.object_id,
      path: file.path,
      versions: entries.length,
      contents: contents.size,
      bytes: [...contents.values()].reduce((sum, bytes) => sum + bytes, 0),
    };
  }

  /** Reject with the configured failure. */
  private check(): void {
    if (this.failure !== undefined) throw this.failure;
  }

  /** Refuse a write as `conflict` unless `revision` is the head, as the API does (API-F2.13). */
  private expect(revision: string): void {
    if (revision !== this.head) throw refusal(409, "conflict");
  }

  /** The newest snapshot. */
  private latest(): Snapshot {
    return this.snapshots[this.snapshots.length - 1] as Snapshot;
  }

  /** A snapshot by version label, or `not_found`. */
  private snapshot(version: string): Snapshot {
    const found = /^v([1-9][0-9]*)$/.exec(version);
    const snapshot = found ? this.snapshots[Number(found[1]) - 1] : undefined;
    if (!snapshot) throw refusal(404, "not_found");
    return snapshot;
  }

  /** One file of the version a reference names, or `not_found`. */
  private stored(file: FileRef): Stored {
    const stored = this.snapshot(file.version).files.get(file.path);
    if (!stored) throw refusal(404, "not_found");
    return stored;
  }

  /** Apply a change as the next version; a change that alters nothing is a conflict. */
  private commit(
    change: (files: Map<string, Stored>, version: string, created: string) => void,
    label: string | null = null,
  ): FileCommit {
    const files = new Map(this.latest().files);
    const version = `v${this.snapshots.length + 1}`;
    const created = new Date(BASE_TIME + this.snapshots.length * 60_000).toISOString();
    change(files, version, created);
    if (sameFiles(files, this.latest().files)) throw refusal(409, "conflict");
    this.snapshots.push({ created, files, label });
    return {
      library_id: this.options.library_id,
      object_id: this.options.object_id,
      version,
      created,
    };
  }

  /** The folders then files directly below `prefix` at the head, or `not_found`. */
  private children(prefix: string): FolderEntry[] {
    const below = prefix === "" ? "" : `${prefix}/`;
    const files = [...this.latest().files.values()].filter((file) => file.path.startsWith(below));
    if (prefix !== "" && files.length === 0) throw refusal(404, "not_found");
    const folders = new Set<string>();
    const direct: Stored[] = [];
    for (const file of files) {
      const rest = file.path.slice(below.length);
      if (rest.includes("/")) folders.add(rest.slice(0, rest.indexOf("/")));
      else direct.push(file);
    }
    const folderEntries = [...folders].sort(byName).map((name) => folderEntry(below, name));
    const fileEntries = direct.sort((a, b) => byName(a.path, b.path)).map(fileEntry);
    return [...folderEntries, ...fileEntries];
  }

  /** The offset a cursor stands for; refuses one not issued for this folder. */
  private offsetOf(cursor: string, prefix: string): number {
    const issued = this.cursors.get(cursor);
    if (!issued || issued.prefix !== prefix) throw refusal(400, "invalid_request");
    return issued.offset;
  }

  /** Issue an opaque cursor, keeping at most MAX_CURSORS. */
  private issue(prefix: string, offset: number): string {
    const cursor = newOperationId();
    this.cursors.set(cursor, { prefix, offset });
    if (this.cursors.size > MAX_CURSORS)
      this.cursors.delete(this.cursors.keys().next().value as string);
    return cursor;
  }
}

/** The version record of snapshot `index`. */
function versionRecord(index: number, snapshot: Snapshot) {
  return {
    version: `v${index + 1}`,
    created: snapshot.created,
    message: null,
    user: null,
    label: snapshot.label,
  };
}

/** The history entry of snapshot `index` for one file, or undefined when it left the file alone. */
function historyEntry(
  index: number,
  snapshot: Snapshot,
  before: Stored | undefined,
  after: Stored | undefined,
): FileHistoryEntry | undefined {
  if (!before && !after) return undefined;
  if (before && after && sameStored(before, after)) return undefined;
  const change = !before ? "added" : after ? "changed" : "deleted";
  return {
    version: versionRecord(index, snapshot),
    change,
    digest: after ? digestOf(after.bytes) : null,
    size: after ? after.bytes.byteLength : null,
  };
}

/** True when two stored files have the same content: the API's history compares digests only. */
function sameStored(a: Stored, b: Stored): boolean {
  return sameBytes(a.bytes, b.bytes);
}

/** A stand-in digest of some bytes: their hex, so two contents never share one. */
function digestOf(bytes: Uint8Array): string {
  return `fixture-${Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

/** A fixture file as stored in `v1`. */
function initialFile(file: MemoryFile, created: string): Stored {
  const name = lastSegment(file.path);
  const bytes =
    typeof file.content === "string" ? new TextEncoder().encode(file.content) : file.content;
  const mediaType = file.mediaType ?? TYPES[extensionOf(name) ?? ""] ?? "application/octet-stream";
  return {
    path: file.path,
    displayName: file.displayName ?? name,
    bytes,
    mediaType,
    modified: created,
    modifiedVersion: "v1",
  };
}

/**
 * The normalisation form of an upload's name when the route would accept its facts: the form
 * sent, or `nfc` for an ASCII name; null when a fact is refused. Pure.
 */
function uploadForm(facts: UploadFacts): NameNormalization | null {
  const bytes = new TextEncoder().encode(facts.name).length;
  const badName =
    bytes === 0 ||
    bytes > MAX_UPLOAD_NAME_BYTES ||
    facts.name.includes("/") ||
    facts.name.includes("\0");
  const badType = facts.media_type !== undefined && !isMediaType(facts.media_type);
  const badTime = facts.last_modified !== undefined && !isRfc3339Utc(facts.last_modified);
  const badId = facts.file_id !== undefined && !UUID_V4.test(facts.file_id);
  const ascii = /^\p{ASCII}*$/u.test(facts.name);
  const form = facts.name_normalization ?? (ascii ? "nfc" : null);
  return badName || badType || badTime || badId ? null : form;
}

/**
 * Whether `text` is an RFC 3339 UTC time the API accepts: a real calendar date, hours below
 * 24, minutes below 60 and seconds below 60, or 60 only as the leap second `23:59:60`. Pure.
 */
function isRfc3339Utc(text: string): boolean {
  const parts = RFC3339_UTC.exec(text)?.slice(1).map(Number);
  if (!parts) return false;
  const [year, month, day, hour, minute, second] = parts as [
    number,
    number,
    number,
    number,
    number,
    number,
  ];
  const date = new Date(0);
  date.setUTCFullYear(year, month - 1, day);
  const realDate =
    date.getUTCFullYear() === year && date.getUTCMonth() === month - 1 && date.getUTCDate() === day;
  const leap = hour === 23 && minute === 59 && second === 60;
  return realDate && hour < 24 && minute < 60 && (second < 60 || leap);
}

/** True when two byte arrays are equal. */
function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
  return a.byteLength === b.byteLength && a.every((byte, i) => byte === b[i]);
}

/** True when both versions hold the same paths with the same content and names. */
function sameFiles(a: ReadonlyMap<string, Stored>, b: ReadonlyMap<string, Stored>): boolean {
  if (a.size !== b.size) return false;
  for (const [path, file] of a) {
    const other = b.get(path);
    const same =
      other !== undefined &&
      other.displayName === file.displayName &&
      other.mediaType === file.mediaType &&
      other.bytes.byteLength === file.bytes.byteLength &&
      other.bytes.every((byte, i) => byte === file.bytes[i]);
    if (!same) return false;
  }
  return true;
}

/** The folder entry for `name` below `below` (a prefix with its trailing slash, or empty). */
function folderEntry(below: string, name: string): FolderEntry {
  return { kind: "folder", name, path: `${below}${name}` };
}

/** The file entry of a stored file. */
function fileEntry(file: Stored): FolderEntry {
  const name = lastSegment(file.path);
  return {
    kind: "file",
    name,
    display_name: safeDisplayName(file.displayName),
    path: file.path,
    digest: `fixture-${file.bytes.byteLength}`,
    size: file.bytes.byteLength,
    modified_version: file.modifiedVersion,
    modified: file.modified,
    type_hint: extensionOf(name),
  };
}
