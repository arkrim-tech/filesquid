/**
 * An in-memory {@link FileSource} over a fixture object, answering as the file routes do
 * (API-F1.3): folders first then files in code-point order, a display name apart from the
 * stored name, opaque cursors with a bounded page, ranged reads, one new OCFL version per
 * write, restore from an earlier version, a folder that stays pending until a file is added,
 * and the API's refusals as `ApiError` (`not_found`, `conflict`, `invalid_request`,
 * `range_not_satisfiable`). Tests and demos use it; the live source is `ClientFileSource`.
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
  ObjectRef,
} from "./model";
import { lastSegment } from "./model";
import type { FileSource } from "./source";

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
}

/** Route named in the fixture's errors. */
const ROUTE = "/api/v1/files";
/** Default page size, as the API's. */
const DEFAULT_LIMIT = 200;
/** Largest page size the API accepts. */
const MAX_LIMIT = 1000;
/** Most cursors remembered; the oldest is forgotten first. */
const MAX_CURSORS = 1000;
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
  return a < b ? -1 : a > b ? 1 : 0;
}

/** A file source answering from memory. */
export class MemoryFileSource implements FileSource {
  private readonly snapshots: Snapshot[] = [];
  private readonly cursors = new Map<string, { prefix: string; offset: number }>();
  /** When set, every call rejects with this value (a network error, a denial, ...). */
  failure: unknown = undefined;
  /** Every handle URL minted, in order, with the file it was minted for. */
  readonly mints: { readonly file: FileRef; readonly url: string }[] = [];

  /** @param options The object and the files of `v1`. */
  constructor(private readonly options: MemoryOptions) {
    const created = new Date(BASE_TIME).toISOString();
    const files = new Map<string, Stored>();
    for (const file of options.files) files.set(file.path, initialFile(file, created));
    this.snapshots.push({ created, files });
  }

  /** The head version label. */
  get head(): string {
    return `v${this.snapshots.length}`;
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
  ): Promise<FolderPage> {
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
      display_name: stored.displayName,
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

  /** Add or replace one file. */
  async upload(_object: ObjectRef, path: string, body: Blob): Promise<FileCommit> {
    this.check();
    const bytes = await readBlob(body);
    return this.commit((files, version, created) => {
      const name = lastSegment(path);
      const same = files.get(path);
      const type = TYPES[extensionOf(name) ?? ""] ?? "application/octet-stream";
      const stored = { path, displayName: same?.displayName ?? name, bytes, mediaType: type };
      files.set(path, { ...stored, modified: created, modifiedVersion: version });
    });
  }

  /** Create a folder: pending when no file lies below it yet. */
  async createFolder(_object: ObjectRef, path: string): Promise<FolderAnswer> {
    this.check();
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

  /** Move or rename one file or folder. */
  async move(_object: ObjectRef, from: string, to: string): Promise<FileCommit> {
    this.check();
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
  async remove(_object: ObjectRef, path: string): Promise<FileCommit> {
    this.check();
    return this.commit((files) => {
      const removing = [...files.keys()].filter((candidate) => within(candidate, path));
      if (removing.length === 0) throw refusal(404, "not_found");
      for (const candidate of removing) files.delete(candidate);
    });
  }

  /** Restore files or folders from an earlier version as a new version. */
  async restore(
    _object: ObjectRef,
    version: string,
    paths: readonly string[],
  ): Promise<FileCommit> {
    this.check();
    const earlier = this.snapshot(version);
    return this.commit((files, label, created) => {
      for (const path of paths) {
        const found = [...earlier.files.values()].filter((file) => within(file.path, path));
        if (found.length === 0) throw refusal(404, "not_found");
        for (const file of found)
          files.set(file.path, { ...file, modified: created, modifiedVersion: label });
      }
    });
  }

  /** Reject with the configured failure. */
  private check(): void {
    if (this.failure !== undefined) throw this.failure;
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
  ): FileCommit {
    const files = new Map(this.latest().files);
    const version = `v${this.snapshots.length + 1}`;
    const created = new Date(BASE_TIME + this.snapshots.length * 60_000).toISOString();
    change(files, version, created);
    if (sameFiles(files, this.latest().files)) throw refusal(409, "conflict");
    this.snapshots.push({ created, files });
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
    display_name: file.displayName,
    path: file.path,
    digest: `fixture-${file.bytes.byteLength}`,
    size: file.bytes.byteLength,
    modified_version: file.modifiedVersion,
    modified: file.modified,
    type_hint: extensionOf(name),
  };
}
