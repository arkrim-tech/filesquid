/**
 * The model behind the Drive-style views (UI-F2.8, UI-S-006, UI-S-026): the row the list, the
 * grid, the search results of UI-F2.9 and the destination picker of UI-F2.17 all share, the
 * query a view is asked with, the sort rules, relative dates, file-type badge classes, the
 * summary and the effective scope of a selection, the ZIP caps, and the day groups of the Recent
 * view. Every name is the API's display name (UI-S-019). Pure: no I/O, no DOM.
 *
 * Every list and loop is bounded (AGENTS.md section 22) by the `MAX_*` constants.
 */
import type { components } from "../api/schema";
import { formatModified, formatSize } from "../files/model";
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";
import type { FolderPlacement, SortDirection, SortKey, ViewKind } from "../preferences/model";

/** One page of a view, generated from the API contract. */
export type ViewPage = components["schemas"]["ViewPage"];
/** One row of a view page. */
export type ViewRow = components["schemas"]["ViewRow"];

/** Most files a view holds (API-S-011). */
export const MAX_VIEW_FILES = 10_000;
/** Most folder rows a view holds (API-S-011). */
export const MAX_VIEW_FOLDERS = 1_000;
/** Rows asked for per page. */
export const VIEW_PAGE_SIZE = 200;
/** Most pages one view read follows (covers {@link MAX_VIEW_FILES} and the folders). */
export const MAX_VIEW_PAGES = 60;
/** Most members one ZIP archive holds (UI-S-026). */
export const ZIP_MAX_FILES = 1_000;
/** Most bytes one ZIP archive holds: 10 GiB (UI-S-026). */
export const ZIP_MAX_BYTES = 10 * 1024 ** 3;
/** Days the Recent view looks back (UI-S-026). */
export const RECENT_DAYS = 7;

/** The row of a folder or a file in a view. */
export interface DriveRow {
  /** Stable key: the file id, or the folder's node handle. */
  readonly key: string;
  /** Folder (a collection or virtual collection) or file. */
  readonly kind: "folder" | "file";
  /** What users see: the API's display name. */
  readonly name: string;
  /** A folder's node handle, to open it. */
  readonly node?: string;
  /** A folder's kind: collection or virtual collection. */
  readonly nodeKind?: components["schemas"]["ViewFolderKind"];
  /** Files inside a folder, when the host says how many. */
  readonly fileCount?: number;
  /** A file's id. */
  readonly fileId?: string;
  /** A file's own object, `urn:uuid:<file-id>`. */
  readonly objectId?: string;
  /** The library holding the file, when the host says so (API gap: not in the view rows yet). */
  readonly libraryId?: string;
  /** The media type of the original. */
  readonly mediaType?: string;
  /** The original's size in bytes. */
  readonly size?: number;
  /** When the head version was created, RFC 3339. */
  readonly modified?: string;
  /** The head version, `v7`, when the host says so (API gap: not in the view rows yet). */
  readonly version?: string;
  /** When the caller last changed the file, from the caller's own journal only. */
  readonly modifiedByMe?: string | null;
  /** When the caller last opened the file, from the caller's own journal only. */
  readonly openedByMe?: string | null;
  /** The collection the file is shown under, in a cross-collection view. */
  readonly collection?: string;
}

/** What a view is asked for. */
export interface ViewQuery {
  /** The library, collection or virtual-collection node. */
  readonly node: string;
  /** Which files the view shows. */
  readonly view: ViewKind;
  /** The sort key. */
  readonly sort: SortKey;
  /** The sort direction. */
  readonly direction: SortDirection;
  /** Where folder rows go. */
  readonly folders: FolderPlacement;
  /** Only rows whose name contains this text (at most 256 bytes); empty for all. */
  readonly filter: string;
}

/** One page of rows as the UI holds it. */
export interface DrivePage {
  /** The rows, in the API's order. */
  readonly rows: readonly DriveRow[];
  /** The cursor of the next page, or null on the last. */
  readonly nextCursor: string | null;
  /** False when the scope held more than the view bounds, or a file could not be read. */
  readonly complete: boolean;
}

/** Totals of the rows shown. */
export interface DriveSummary {
  /** Number of folders. */
  readonly folders: number;
  /** Number of files. */
  readonly files: number;
  /** Total bytes of the files. */
  readonly bytes: number;
}

/**
 * Turn one contract row into the row the UI holds. Pure.
 *
 * @param row The row of a view page.
 * @param libraryId The library, when the host reported it.
 * @returns The row.
 */
export function driveRow(row: ViewRow, libraryId?: string): DriveRow {
  if (row.kind === "folder") {
    return {
      key: row.node,
      kind: "folder",
      name: row.name,
      node: row.node,
      nodeKind: row.node_kind,
    };
  }
  return {
    key: row.file_id,
    kind: "file",
    name: row.display_name,
    fileId: row.file_id,
    objectId: row.object_id,
    libraryId,
    mediaType: row.media_type,
    size: row.size,
    modified: row.modified,
    modifiedByMe: row.modified_by_me,
    openedByMe: row.opened_by_me,
  };
}

/** Whether two queries ask for the same rows (everything but nothing is ignored). */
export function sameQuery(a: ViewQuery, b: ViewQuery): boolean {
  return (
    a.node === b.node &&
    a.view === b.view &&
    a.sort === b.sort &&
    a.direction === b.direction &&
    a.folders === b.folders &&
    a.filter === b.filter
  );
}

/** The direction a sort key starts in: dates newest first, names and sizes as a user expects. */
export function defaultDirection(sort: SortKey): SortDirection {
  return sort === "name" ? "ascending" : "descending";
}

/**
 * What a click on a column head asks for: the same column reverses, another column starts in
 * its default direction. Pure.
 *
 * @param current The sort now.
 * @param clicked The column clicked.
 * @returns The new sort key and direction.
 */
export function nextSort(
  current: { sort: SortKey; direction: SortDirection },
  clicked: SortKey,
): { sort: SortKey; direction: SortDirection } {
  if (current.sort !== clicked) return { sort: clicked, direction: defaultDirection(clicked) };
  const direction = current.direction === "ascending" ? "descending" : "ascending";
  return { sort: clicked, direction };
}

/** The label key of a sort key's name. */
export const SORT_LABEL: Readonly<Record<SortKey, LabelKey>> = {
  name: "sortName",
  size: "sortSize",
  modified: "sortModified",
  modified_by_me: "sortModifiedByMe",
  opened_by_me: "sortOpenedByMe",
};

/** The label key of a sort direction's name. */
export const DIRECTION_LABEL: Readonly<Record<SortDirection, LabelKey>> = {
  ascending: "sortAscending",
  descending: "sortDescending",
};

/** The label key of a folder placement's name in the sort menu. */
export const PLACEMENT_LABEL: Readonly<Record<FolderPlacement, LabelKey>> = {
  on_top: "foldersOnTop",
  mixed: "foldersMixed",
};

/** The label key of a sort key and direction together, as the Settings page words them. */
export function sortOptionLabel(sort: SortKey, direction: SortDirection): LabelKey {
  const stem: Record<SortKey, string> = {
    name: "Name",
    size: "Size",
    modified: "Modified",
    modified_by_me: "ModifiedByMe",
    opened_by_me: "OpenedByMe",
  };
  const end = direction === "ascending" ? "Ascending" : "Descending";
  return `sortOption${stem[sort]}${end}` as LabelKey;
}

/** The columns a header click sorts by. */
export const SORTABLE_COLUMNS: readonly SortKey[] = ["name", "size", "modified"];

/** What a file-type badge's colour says (UI-S-026): documents red, data yellow, media blue. */
export type BadgeClass = "document" | "data" | "media" | "other";

const DOCUMENTS = new Set(["pdf", "docx", "doc", "txt", "rtf", "odt", "md"]);
const DATA = new Set(["xlsx", "xls", "csv", "blend", "env", "gz", "json", "xml", "zip", "tar"]);
const MEDIA = new Set([
  "jpg",
  "jpeg",
  "png",
  "gif",
  "webp",
  "avif",
  "heic",
  "m4a",
  "mp3",
  "wav",
  "flac",
  "mp4",
  "mov",
  "webm",
  "cr3",
  "raw",
]);

/** The longest filter text the API takes, in UTF-8 bytes (API-S-011). */
export const FILTER_MAX_BYTES = 256;

/**
 * Cut a filter to at most {@link FILTER_MAX_BYTES} UTF-8 bytes without splitting a character.
 * Pure.
 *
 * @param text What the user typed.
 * @returns `text`, or its longest prefix that fits.
 */
export function clampFilter(text: string): string {
  const encoder = new TextEncoder();
  if (encoder.encode(text).length <= FILTER_MAX_BYTES) return text;
  let bytes = 0;
  let out = "";
  for (const character of text) {
    bytes += encoder.encode(character).length;
    if (bytes > FILTER_MAX_BYTES) break;
    out += character;
  }
  return out;
}

/**
 * The extension to show in a list badge: the last dot-separated part of the display name, lower
 * case, at most 5 characters; a leading-dot name such as `.env` is its own extension, because
 * UI-S-026 lists ENV among the data badges. This differs on purpose from `files/badge.ts`
 * `extensionOf` (the preview and the file panel), which gives a leading-dot name none. Pure.
 *
 * @param name The display name.
 * @returns The extension in lower case, or an empty string when there is none.
 */
export function badgeExtensionOf(name: string): string {
  const dot = name.lastIndexOf(".");
  if (dot < 0 || dot === name.length - 1) return "";
  const ext = name.slice(dot + 1).toLowerCase();
  return /^[a-z0-9]{1,5}$/.test(ext) ? ext : "";
}

/**
 * The badge class of an extension. Pure.
 *
 * @param ext A lower-case extension.
 * @returns Document, data, media or other.
 */
export function badgeClass(ext: string): BadgeClass {
  if (DOCUMENTS.has(ext)) return "document";
  if (DATA.has(ext)) return "data";
  if (MEDIA.has(ext)) return "media";
  return "other";
}

/** Whole local calendar days between two times (positive when `then` is earlier). */
function dayGap(then: Date, now: Date): number {
  const a = Date.UTC(then.getFullYear(), then.getMonth(), then.getDate());
  const b = Date.UTC(now.getFullYear(), now.getMonth(), now.getDate());
  return Math.round((b - a) / 86_400_000);
}

/**
 * Write a time as the list shows it (UI-S-026): "2 hours ago" earlier today (minutes within the
 * hour, even across midnight), "Yesterday" for any time on the calendar day before, otherwise
 * the date ("9 Oct 2026"). A value that is not a time is shown as given,
 * never as a wrong date. Pure.
 *
 * @param iso An RFC 3339 time.
 * @param now The time now.
 * @param locale BCP 47 tag.
 * @param labels The active label table.
 * @returns The text.
 */
export function relativeTime(iso: string, now: Date, locale: string, labels: LabelTable): string {
  const then = new Date(iso);
  if (Number.isNaN(then.getTime())) return iso;
  const seconds = Math.floor((now.getTime() - then.getTime()) / 1000);
  if (seconds < 60) return labels.fileTimeJustNow;
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "always" });
  if (seconds < 3600) return rtf.format(-Math.floor(seconds / 60), "minute");
  const gap = dayGap(then, now);
  if (gap === 0) return rtf.format(-Math.floor(seconds / 3600), "hour");
  return gap === 1 ? labels.fileTimeYesterday : formatModified(iso, locale);
}

/** The count of files as the label table words it ("1 file", "62 files"). */
export function fileCountText(count: number, labels: LabelTable, locale: string): string {
  const key = count === 1 ? labels.fileCountOne : labels.fileCountOther;
  return formatLabel(key, { count: count.toLocaleString(locale) });
}

/** The count of folders as the label table words it. */
export function folderCountText(count: number, labels: LabelTable, locale: string): string {
  const key = count === 1 ? labels.folderCountOne : labels.folderCountOther;
  return formatLabel(key, { count: count.toLocaleString(locale) });
}

/**
 * The size cell of a row: a file's size, a folder's "{count} files" when the host says how
 * many, otherwise nothing. Pure.
 *
 * @param row The row.
 * @param labels The active label table.
 * @param locale BCP 47 tag.
 * @returns The text, possibly empty.
 */
export function sizeText(row: DriveRow, labels: LabelTable, locale: string): string {
  if (row.kind === "folder") {
    return row.fileCount === undefined ? "" : fileCountText(row.fileCount, labels, locale);
  }
  return row.size === undefined ? "" : formatSize(row.size, labels, locale);
}

/**
 * Count the rows. Pure.
 *
 * @param rows The rows shown.
 * @returns Folder and file counts and the files' total size.
 */
export function driveSummary(rows: readonly DriveRow[]): DriveSummary {
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

/** Files inside the selected folders, or undefined when any selected folder's count is unknown. */
function filesInside(rows: readonly DriveRow[]): number | undefined {
  let inside = 0;
  for (const row of rows) {
    if (row.kind !== "folder") continue;
    if (row.fileCount === undefined) return undefined;
    inside += row.fileCount;
  }
  return inside;
}

/**
 * The effective scope of a selection (UI-S-026): "{files} and {folders} ({inside} inside) ·
 * {size}", with the inside count only when the host says how many files each folder holds. Pure.
 *
 * @param rows The selected rows.
 * @param labels The active label table.
 * @param locale BCP 47 tag.
 * @returns The sentence.
 */
export function scopeText(rows: readonly DriveRow[], labels: LabelTable, locale: string): string {
  const sum = driveSummary(rows);
  const inside = filesInside(rows);
  const files = fileCountText(sum.files, labels, locale);
  const size = formatSize(sum.bytes, labels, locale);
  if (sum.folders === 0) return formatLabel(labels.fileSelectionScopeFiles, { files, size });
  const folders = folderCountText(sum.folders, labels, locale);
  if (inside === undefined) return formatLabel(labels.fileSelectionScope, { files, folders, size });
  const insideText = fileCountText(inside, labels, locale);
  return formatLabel(labels.fileSelectionScopeInside, { files, folders, inside: insideText, size });
}

/** The result of checking a selection against the ZIP caps. */
export type ZipCheck =
  | { readonly ok: true; readonly count: number; readonly bytes: number }
  | { readonly ok: false; readonly reason: "too_many" | "too_big" | "empty" };

/**
 * Check a selection against the ZIP caps of 1,000 members and 10 GiB (UI-S-026). Files inside
 * selected folders count when the host says how many. Pure.
 *
 * @param rows The selected rows.
 * @returns The member count and bytes, or the cap that refuses the selection.
 */
export function zipCheck(rows: readonly DriveRow[]): ZipCheck {
  const sum = driveSummary(rows);
  const count = sum.files + (filesInside(rows) ?? 0);
  if (count === 0) return { ok: false, reason: "empty" };
  if (count > ZIP_MAX_FILES) return { ok: false, reason: "too_many" };
  if (sum.bytes > ZIP_MAX_BYTES) return { ok: false, reason: "too_big" };
  return { ok: true, count, bytes: sum.bytes };
}

/** A day group of the Recent view. */
export type RecentGroup = "today" | "yesterday" | "earlier";

/** What the caller last did to a file, and when (the later of the two own-journal times). */
export interface Activity {
  /** Changed or opened. */
  readonly kind: "changed" | "opened";
  /** When, RFC 3339. */
  readonly at: string;
}

/**
 * The caller's latest activity on a file, from the caller's own journal fields only. Pure.
 *
 * @param row The row.
 * @returns The later of the two times, or undefined when the caller did neither.
 */
export function activityOf(row: DriveRow): Activity | undefined {
  const changed = row.modifiedByMe ?? undefined;
  const opened = row.openedByMe ?? undefined;
  if (changed && (!opened || changed >= opened)) return { kind: "changed", at: changed };
  return opened ? { kind: "opened", at: opened } : undefined;
}

/**
 * The day group of a time: today, yesterday, or earlier within the last {@link RECENT_DAYS}
 * days; undefined for anything older or not a time. Pure.
 *
 * @param iso An RFC 3339 time.
 * @param now The time now.
 * @returns The group, or undefined.
 */
export function recentGroup(iso: string, now: Date): RecentGroup | undefined {
  const then = new Date(iso);
  if (Number.isNaN(then.getTime())) return undefined;
  const gap = dayGap(then, now);
  if (gap < 0 || gap >= RECENT_DAYS) return undefined;
  if (gap === 0) return "today";
  return gap === 1 ? "yesterday" : "earlier";
}

/**
 * Write the time of a Recent row (mockup 15): relative in the Today group, the clock time in
 * Yesterday, the date after that. Pure.
 *
 * @param iso An RFC 3339 time.
 * @param group Its day group.
 * @param now The time now.
 * @param locale BCP 47 tag.
 * @param labels The active label table.
 * @returns The text.
 */
export function recentTimeText(
  iso: string,
  group: RecentGroup,
  now: Date,
  locale: string,
  labels: LabelTable,
): string {
  if (group === "today") return relativeTime(iso, now, locale, labels);
  if (group === "earlier") return formatModified(iso, locale);
  const time = new Date(iso);
  return new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(time);
}
