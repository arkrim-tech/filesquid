/**
 * The model behind `<fs-file-browser>` (UI-F2.2): the file-route types from the generated API
 * contract, the limits of those routes, and the pure rules over them: the row model that
 * carries only display names (UI-S-019), path and name handling, size and date text from the
 * label table, the route form of a location, and which preview a media type gets. Nothing here
 * fetches or touches the DOM.
 *
 * Every list is bounded (AGENTS.md section 22) by the `MAX_*` constants.
 */
import type { components } from "../api/schema";
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";

/** One entry of a folder listing, generated from the API contract. */
export type FolderEntry = components["schemas"]["FolderEntry"];
/** One page of a folder listing. */
export type FolderPage = components["schemas"]["FolderPage"];
/** Storage facts about one file version. */
export type FileDetails = components["schemas"]["FileDetails"];
/** The one OCFL version a write published. */
export type FileCommit = components["schemas"]["FileCommit"];
/** The answer to creating a folder. */
export type FolderAnswer = components["schemas"]["FolderAnswer"];
/** The object whose files are listed: a library and one OCFL object in it. */
export interface ObjectRef {
  /** The library. */
  readonly library_id: string;
  /** The OCFL object identifier. */
  readonly object_id: string;
}

/** Where the browser is: an object and a folder (a logical-path prefix, empty for the root). */
export interface FileLocation extends ObjectRef {
  /** The folder, a logical-path prefix without a trailing `/`. */
  readonly prefix: string;
}

/**
 * The file a panel or preview works on (ADR 028 Decision 6): a file at one concrete version.
 * `display_name` is the name users see; the stored name is never part of it (UI-S-019).
 */
export interface FileRef extends ObjectRef {
  /** The concrete version, `v1`, `v2`, ...; never `head`. */
  readonly version: string;
  /** The file's logical path. */
  readonly path: string;
  /** The name users see. */
  readonly display_name: string;
}

/** What the listing shows for one folder or file (UI-S-019: no stored name in it). */
export interface FileRow {
  /** Stable key: the logical path. */
  readonly key: string;
  /** Folder or file. */
  readonly kind: "folder" | "file";
  /** What users see: a file's display name, a folder's last path segment. */
  readonly name: string;
  /** The logical path. */
  readonly path: string;
  /** True for a folder that exists only in the client until a file is added beneath it. */
  readonly pending: boolean;
  /** Size in bytes (files). */
  readonly size?: number;
  /** Lowercase extension, a display hint only (files). */
  readonly typeHint: string | null;
  /** When the last change to the file was committed, RFC 3339 (files). */
  readonly modified?: string;
  /** The version that last set the file's content (files). */
  readonly version?: string;
}

/** Totals of the rows shown. */
export interface RowSummary {
  /** Number of folders. */
  readonly folders: number;
  /** Number of files. */
  readonly files: number;
  /** Total bytes of the files. */
  readonly bytes: number;
}

/** Largest upload the API accepts, in bytes (64 MiB). */
export const MAX_UPLOAD_BYTES = 64 * 1024 * 1024;
/** Bytes of a text file the preview reads, in one ranged read (64 KiB). */
export const TEXT_PREVIEW_BYTES = 64 * 1024;
/** Entries asked for per listing page. */
export const PAGE_SIZE = 200;
/** Most listing pages read for one folder (100,000 entries). */
export const MAX_LISTING_PAGES = 500;
/** Longest file or folder name, in UTF-8 bytes. */
export const MAX_NAME_BYTES = 255;
/** Rows kept in the DOM above and below the visible ones. */
export const OVERSCAN_ROWS = 5;
/** Height of a list row, in rem (so the window follows the text size). */
export const LIST_ROW_REM = 3;
/** Height of a grid row of tiles, in rem. */
export const GRID_ROW_REM = 9;
/** Least width of a grid tile, in rem. */
export const GRID_TILE_MIN_REM = 10;
/** Rows assumed visible when the scrolling area has no measured height yet. */
export const FALLBACK_VIEWPORT_ROWS = 20;

/**
 * Build the rows of a folder from a listing: entries keep the API's order (folders first), and
 * folders still pending in the client follow the listed folders. Pure.
 *
 * @param entries The entries loaded so far.
 * @param pending Paths of folders created in the client and not yet in a listing.
 * @returns One row per entry, carrying display names only.
 */
export function fileRows(entries: readonly FolderEntry[], pending: readonly string[]): FileRow[] {
  const listed = new Set(entries.map((entry) => entry.path));
  const folders = entries.filter((entry) => entry.kind === "folder");
  const files = entries.filter((entry) => entry.kind === "file");
  const extra = pending.filter((path) => !listed.has(path));
  return [
    ...folders.map(folderRow),
    ...extra.map((path) => pendingRow(path)),
    ...files.map(fileRow),
  ];
}

/** The row of a listed folder. */
function folderRow(entry: FolderEntry): FileRow {
  return {
    key: entry.path,
    kind: "folder",
    name: entry.name,
    path: entry.path,
    pending: false,
    typeHint: null,
  };
}

/** The row of a folder that exists only in the client. */
function pendingRow(path: string): FileRow {
  return {
    key: path,
    kind: "folder",
    name: lastSegment(path),
    path,
    pending: true,
    typeHint: null,
  };
}

/** The row of a listed file: its display name, never its stored name. */
function fileRow(entry: FolderEntry): FileRow {
  if (entry.kind !== "file") throw new TypeError("fileRow needs a file entry");
  return {
    key: entry.path,
    kind: "file",
    name: entry.display_name,
    path: entry.path,
    pending: false,
    size: entry.size,
    typeHint: entry.type_hint,
    modified: entry.modified,
    version: entry.modified_version,
  };
}

/**
 * Count the rows. Pure.
 *
 * @param rows The rows shown.
 * @returns Folder and file counts and the files' total size.
 */
export function rowSummary(rows: readonly FileRow[]): RowSummary {
  let folders = 0;
  let files = 0;
  let bytes = 0;
  for (const row of rows) {
    if (row.kind === "folder") folders += 1;
    else {
      files += 1;
      bytes += row.size ?? 0;
    }
  }
  return { folders, files, bytes };
}

/** Join a folder prefix and a name into a logical path. Pure. */
export function joinPath(prefix: string, name: string): string {
  return prefix === "" ? name : `${prefix}/${name}`;
}

/** The folder a logical path lies in; the empty root for a top-level path. Pure. */
export function parentPath(path: string): string {
  const cut = path.lastIndexOf("/");
  return cut < 0 ? "" : path.slice(0, cut);
}

/** The last segment of a logical path. Pure. */
export function lastSegment(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1);
}

/** One step of a folder trail. */
export interface PrefixStep {
  /** The folder's name. */
  readonly name: string;
  /** Its logical-path prefix. */
  readonly prefix: string;
}

/** The folders from the root down to `prefix`, for a breadcrumb. Pure. */
export function prefixTrail(prefix: string): PrefixStep[] {
  if (prefix === "") return [];
  const names = prefix.split("/");
  return names.map((name, i) => ({ name, prefix: names.slice(0, i + 1).join("/") }));
}

/** True when a change to `changed` touches `path`: the path itself or anything below it. Pure. */
export function affectsPath(changed: string, path: string): boolean {
  return path === changed || path.startsWith(`${changed}/`);
}

/** Why a name is refused. */
export type NameProblem = "empty" | "separator" | "reserved" | "control" | "long";

/**
 * True for a control character: C0 (below space), DEL and C1 (U+0080 to U+009F). Pure.
 *
 * @param character One character.
 */
export function isControl(character: string): boolean {
  const code = character.codePointAt(0) ?? 0;
  return code < 0x20 || (code >= 0x7f && code <= 0x9f);
}

/**
 * Check a file or folder name as typed (surrounding spaces are ignored). The API stays the
 * authority; this only spares a round trip for an obviously unusable name. Pure.
 *
 * @param raw The name as typed.
 * @returns The problem, or undefined when the name is usable.
 */
export function validateName(raw: string): NameProblem | undefined {
  const name = raw.trim();
  if (name === "") return "empty";
  if (Array.from(name).some(isControl)) return "control";
  if (name.includes("/") || name.includes("\\")) return "separator";
  if (name === "." || name === "..") return "reserved";
  if (new TextEncoder().encode(name).length > MAX_NAME_BYTES) return "long";
  return undefined;
}

/**
 * The version before `version` (`v5` is `v4`), the one an Undo restores from. Pure.
 *
 * @param version An OCFL version label.
 * @returns The previous label, or undefined for `v1` or a label that is not `v<n>`.
 */
export function previousVersion(version: string): string | undefined {
  const match = /^v([1-9][0-9]*)$/.exec(version);
  const number = match ? Number(match[1]) : 0;
  return number > 1 ? `v${number - 1}` : undefined;
}

/** Binary size units, smallest first, with the label key of each. */
const SIZE_UNITS: readonly LabelKey[] = ["sizeB", "sizeKB", "sizeMB", "sizeGB", "sizeTB"];

/** The value as it will be shown in `unit`, rounded, so 1,023.96 KB steps up to the next unit. */
function roundedSize(value: number, unit: number): number {
  const digits = unit === 0 || value >= 100 ? 0 : 1;
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
}

/**
 * Write a size as the label table words it: whole bytes below 1 KB, otherwise one decimal (none
 * at 100 and above) in KB, MB, GB or TB of 1,024 units. Pure.
 *
 * @param bytes The size in bytes.
 * @param labels The active label table (units come from it).
 * @param locale BCP 47 tag for the number format.
 * @returns For example `6.2 MB`.
 */
export function formatSize(bytes: number, labels: LabelTable, locale?: string): string {
  let value = bytes;
  let unit = 0;
  while (unit < SIZE_UNITS.length - 1 && roundedSize(value, unit) >= 1024) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 || value >= 100 ? 0 : 1;
  const text = new Intl.NumberFormat(locale, { maximumFractionDigits: digits }).format(value);
  return formatLabel(labels[SIZE_UNITS[unit] as LabelKey], { value: text });
}

/**
 * Write a commit time as a short date in the user's locale. Pure.
 *
 * @param iso An RFC 3339 time.
 * @param locale BCP 47 tag.
 * @param timeZone IANA zone; the viewer's own when omitted.
 * @returns The date, or `iso` unchanged when it is not a time (never a wrong date).
 */
export function formatModified(iso: string, locale?: string, timeZone?: string): string {
  const time = new Date(iso);
  if (Number.isNaN(time.getTime())) return iso;
  const options: Intl.DateTimeFormatOptions = {
    year: "numeric",
    month: "short",
    day: "numeric",
    timeZone,
  };
  return new Intl.DateTimeFormat(locale, options).format(time);
}

/** Hash prefix of the files route. */
const FILES_ROUTE = "#/files/";

/**
 * The hash route of a location: `#/files/<library>/<object>/<folder>/...`, each segment
 * percent-encoded. Pure.
 *
 * @param location The location.
 * @returns The route.
 */
export function locationHash(location: FileLocation): string {
  const segments = [location.library_id, location.object_id, ...prefixSegments(location.prefix)];
  return `${FILES_ROUTE}${segments.map(encodeURIComponent).join("/")}`;
}

/** The segments of a prefix; none for the root. */
function prefixSegments(prefix: string): string[] {
  return prefix === "" ? [] : prefix.split("/");
}

/**
 * Read a location from a hash route. A route with fewer than two segments, a `.` or `..`
 * segment, an empty segment in the middle or a bad percent escape is not a location. Pure.
 *
 * @param hash The `location.hash` value.
 * @returns The location, or undefined.
 */
export function parseLocationHash(hash: string): FileLocation | undefined {
  if (!hash.startsWith(FILES_ROUTE)) return undefined;
  const raw = hash.slice(FILES_ROUTE.length).replace(/\/$/, "").split("/");
  let segments: string[];
  try {
    segments = raw.map(decodeURIComponent);
  } catch {
    return undefined;
  }
  const [library_id, object_id, ...folders] = segments;
  const usable = segments.every(
    (s) => s !== "" && s !== "." && s !== ".." && !s.includes("/") && !s.includes("\\"),
  );
  if (!usable || !library_id || !object_id) return undefined;
  return { library_id, object_id, prefix: folders.join("/") };
}

/** What a preview shows a file as. */
export type PreviewKind = "image" | "video" | "audio" | "text";

/** Image types a browser decodes natively. */
const IMAGE_TYPES: ReadonlySet<string> = new Set([
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/webp",
  "image/avif",
  "image/svg+xml",
  "image/bmp",
]);
/** Video types a browser plays natively. */
const VIDEO_TYPES: ReadonlySet<string> = new Set(["video/mp4", "video/webm", "video/ogg"]);
/** Audio types a browser plays natively. */
const AUDIO_TYPES: ReadonlySet<string> = new Set([
  "audio/mpeg",
  "audio/ogg",
  "audio/wav",
  "audio/x-wav",
  "audio/flac",
  "audio/mp4",
  "audio/aac",
  "audio/webm",
]);
/** Non-`text/` types shown as text. */
const TEXT_TYPES: ReadonlySet<string> = new Set(["application/json", "application/xml"]);

/**
 * Which built-in preview, if any, shows a media type. The API's media type comes from the
 * extension and is never sniffed, so this is the only gate between a file and a native viewer.
 * Pure.
 *
 * @param mediaType A media type, with or without parameters.
 * @returns The preview kind, or undefined when there is none.
 */
export function previewKind(mediaType: string): PreviewKind | undefined {
  const type = (mediaType.split(";")[0] ?? "").trim().toLowerCase();
  if (IMAGE_TYPES.has(type)) return "image";
  if (VIDEO_TYPES.has(type)) return "video";
  if (AUDIO_TYPES.has(type)) return "audio";
  if (type.startsWith("text/") || TEXT_TYPES.has(type)) return "text";
  return undefined;
}
