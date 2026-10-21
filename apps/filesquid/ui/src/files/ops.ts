/**
 * The write operations of the file browser (UI-F2.2) that need more than one call or some
 * rules of their own: uploading a batch of files one commit at a time, the name a file is
 * uploaded under and the facts sent with it (API-S-035), where a rename or move lands, and the
 * words for a failed call. Everything
 * here is either pure or named for the calls it makes (AGENTS.md section 26); `FileSource`
 * performs the I/O and each call commits one OCFL version.
 */
import { ApiError } from "../api/client";
import { denialLabel } from "../components/denial";
import type { LabelTable } from "../theme/labels";
import {
  MAX_NAME_BYTES,
  MAX_UPLOAD_BYTES,
  isControl,
  isMediaType,
  joinPath,
  lastSegment,
  validateName,
  type FileRow,
  type LibraryRef,
  type NameNormalization,
  type NameProblem,
  type UploadCommit,
  type UploadFacts,
} from "./model";
import type { FileSource } from "./source";

/** Most files one batch uploads; further files are left for another batch. */
export const MAX_UPLOAD_FILES = 1000;

/** One file that was not uploaded, and why. */
export interface UploadFailure {
  /** The file. */
  readonly file: File;
  /** The name it would have been uploaded under. */
  readonly name: string;
  /** `too_large`, or what the call rejected with. */
  readonly reason: "too_large" | "name_too_long" | { readonly error: unknown };
}

/** What a batch upload did. */
export interface UploadResult {
  /** The commits, one per file uploaded, each in that file's own object. */
  readonly committed: readonly UploadCommit[];
  /** The files that were not uploaded. */
  readonly failed: readonly UploadFailure[];
  /** Files beyond {@link MAX_UPLOAD_FILES} that were not tried. */
  readonly skipped: number;
  /** Those files themselves, so they can be offered again. */
  readonly left: readonly File[];
}

/**
 * The name a file is uploaded under: its own name with path separators replaced by `_`, control
 * characters removed (as the API does for display names) and surrounding spaces trimmed (a name left empty, `.` or `..` becomes
 * `file`). Pure.
 *
 * @param name The file's name as the browser gives it.
 * @returns The name to upload under.
 */
export function uploadName(name: string): string {
  const clean = Array.from(name, (ch) => (ch === "/" || ch === "\\" ? "_" : ch))
    .filter((ch) => !isControl(ch))
    .join("")
    .trim();
  return clean === "" || clean === "." || clean === ".." ? "file" : clean;
}

/**
 * Upload files one at a time, each one `PUT` that creates the file's own object in one version
 * (API-S-026). A file over 64 MiB or with a name over 255 bytes is refused without being sent;
 * a failed file is recorded and the rest continue (files are never sent together). At most
 * {@link MAX_UPLOAD_FILES} files are tried. Network I/O through `source`.
 *
 * @param source The file source.
 * @param library The library to upload into.
 * @param files The files.
 * @param progress Called before each file with its place (1-based), the total and its name.
 * @returns The commits, the failures and the count not tried.
 */
export async function uploadFiles(
  source: FileSource,
  library: LibraryRef,
  files: readonly File[],
  progress: (current: number, total: number, name: string) => void = () => undefined,
): Promise<UploadResult> {
  const batch = files.slice(0, MAX_UPLOAD_FILES);
  const committed: UploadCommit[] = [];
  const failed: UploadFailure[] = [];
  for (const [i, file] of batch.entries()) {
    const name = uploadName(file.name);
    progress(i + 1, batch.length, name);
    if (file.size > MAX_UPLOAD_BYTES) {
      failed.push({ file, name, reason: "too_large" });
      continue;
    }
    if (new TextEncoder().encode(name).length > MAX_NAME_BYTES) {
      failed.push({ file, name, reason: "name_too_long" });
      continue;
    }
    try {
      committed.push(await source.upload(library, uploadFacts(file, name), file));
    } catch (error) {
      console.warn("FileSquid could not upload a file", errorKind(error));
      failed.push({ file, name, reason: { error } });
    }
  }
  return {
    committed,
    failed,
    skipped: files.length - batch.length,
    left: files.slice(batch.length),
  };
}

/** Largest time value a `Date` holds, in milliseconds either side of 1970 (ECMA-262). */
const MAX_TIME_MS = 8.64e15;

/**
 * The facts an upload states about a file (API-S-035): the name it is uploaded under with its
 * Unicode normalisation form, and the media type and last-modified time the platform reported
 * when the API can take them. A fact the platform did not give in a usable form is left out, so
 * the API records it as unavailable. Pure.
 *
 * @param file The file as the browser gives it.
 * @param name The name to upload under (see {@link uploadName}).
 * @returns The facts to send.
 */
export function uploadFacts(file: File, name: string): UploadFacts {
  const mediaType = reportedMediaType(file.type);
  const modified = reportedModified(file.lastModified);
  return {
    name,
    name_normalization: nameNormalization(name),
    ...(mediaType === undefined ? {} : { media_type: mediaType }),
    ...(modified === undefined ? {} : { last_modified: modified }),
  };
}

/**
 * The Unicode normalisation form a name is in: `nfc` (every ASCII name among them), else `nfd`,
 * else `unnormalised`. NFKC text is always NFC and NFKD text always NFD, so those two are never
 * the answer. Pure.
 *
 * @param name The name.
 * @returns Its form, as the upload route names it.
 */
export function nameNormalization(name: string): NameNormalization {
  if (name === name.normalize("NFC")) return "nfc";
  if (name === name.normalize("NFD")) return "nfd";
  return "unnormalised";
}

/**
 * The media type to report for a platform type: its `type/subtype` without parameters, when
 * both are RFC 6838 restricted names; otherwise none, and the API derives one from the name.
 * Pure.
 *
 * @param type The type the platform gave (`File.type`), possibly empty.
 * @returns The media type, or undefined.
 */
export function reportedMediaType(type: string): string | undefined {
  const bare = type.split(";", 1)[0]?.trim() ?? "";
  return isMediaType(bare) ? bare : undefined;
}

/**
 * The last-modified time to report: RFC 3339 UTC with milliseconds, for a time whose year has
 * four digits (0000 to 9999); otherwise none. Pure.
 *
 * @param ms The time the platform gave (`File.lastModified`), in milliseconds since 1970.
 * @returns The time, or undefined.
 */
export function reportedModified(ms: number): string | undefined {
  if (!Number.isFinite(ms) || Math.abs(ms) > MAX_TIME_MS) return undefined;
  const text = new Date(ms).toISOString();
  return /^\d{4}-/.test(text) ? text : undefined;
}

/** The kind of an error, without its message. */
export function errorKind(error: unknown): string {
  return error instanceof Error ? error.name : typeof error;
}

/**
 * The words for a failed call: the denial's label for an authorisation denial, the conflict
 * label for a `conflict`, otherwise the generic failure label. Pure.
 *
 * @param error What the call rejected with.
 * @param labels The active label table.
 * @returns The text to show.
 */
export function failureText(error: unknown, labels: LabelTable): string {
  const denial = denialLabel(error, labels);
  if (denial) return denial.text;
  if (error instanceof ApiError && error.code === "conflict") return labels.fileConflict;
  return labels.fileActionFailed;
}

/** A UUID in its canonical hex form. */
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** The folder of a file object that holds the stored original (API-S-026). */
const ORIGINAL_FOLDER = "original/";

/**
 * The file a file object holds, from the object's identifier `urn:uuid:<file-id>` (API-S-026).
 * Pure.
 *
 * @param objectId The OCFL object identifier shown.
 * @returns The file's id, or undefined when the object is not named by a UUID.
 */
export function fileIdOf(objectId: string): string | undefined {
  const id = objectId.startsWith("urn:uuid:") ? objectId.slice("urn:uuid:".length) : "";
  return UUID.test(id) ? id : undefined;
}

/**
 * The file to rename through `POST /api/v1/files/renames` for a row, when the row is the stored
 * original of a file's own object. Such an original never moves (API-S-008), so a rename of it
 * changes only the display name (API-S-027). Any other row (a folder, or a path in an object
 * that is not a file's own) is renamed or moved with the moves route. Pure.
 *
 * @param row The file or folder.
 * @param objectId The OCFL object identifier shown.
 * @returns The file's id, or undefined when the row is renamed by a move.
 */
export function renameFileId(row: FileRow, objectId: string): string | undefined {
  const id = fileIdOf(objectId);
  if (row.kind !== "file" || id === undefined) return undefined;
  return isOriginalPath(row.path, id) ? id : undefined;
}

/**
 * Whether `path` is the stored original of file `fileId`: `original/<file-id>`, then a dot and
 * 1 to 16 ASCII letters or digits when the file has an extension (API-S-026). A collection
 * snapshot is named `urn:uuid:` too, so the object's identifier alone does not say it holds a
 * file's original. Pure.
 *
 * @param path A logical path.
 * @param fileId The file's id.
 * @returns True for that file's stored original.
 */
export function isOriginalPath(path: string, fileId: string): boolean {
  const stored = path.startsWith(ORIGINAL_FOLDER) ? path.slice(ORIGINAL_FOLDER.length) : "";
  const id = fileId.toLowerCase();
  const name = stored.toLowerCase();
  if (name === id) return true;
  return name.startsWith(`${id}.`) && /^[a-z0-9]{1,16}$/.test(name.slice(id.length + 1));
}

/**
 * Whether `path` is the reserved `original` folder of a file object or lies below it. The API
 * refuses a move whose source or destination is. Pure.
 *
 * @param path A logical path.
 * @returns True for `original` and anything below it.
 */
export function inOriginalFolder(path: string): boolean {
  return path === "original" || path.startsWith(ORIGINAL_FOLDER);
}

/**
 * The words for a failed rename: the denial's label for an authorisation denial, otherwise the
 * label for the route's refusal (`invalid_request`, `not_found`, `conflict`, `unavailable`),
 * otherwise the generic failure label. Pure.
 *
 * @param error What the call rejected with.
 * @param labels The active label table.
 * @returns The text to show.
 */
export function renameFailureText(error: unknown, labels: LabelTable): string {
  const denial = denialLabel(error, labels);
  if (denial) return denial.text;
  if (!(error instanceof ApiError)) return labels.fileActionFailed;
  switch (error.code) {
    case "invalid_request":
      return labels.fileRenameInvalid;
    case "not_found":
      return labels.fileRenameNotFound;
    case "conflict":
      return labels.fileRenameConflict;
    case "unavailable":
      return labels.fileRenameUnavailable;
    default:
      return labels.fileActionFailed;
  }
}

/**
 * The label for a name problem. Pure.
 *
 * @param problem What is wrong with the name.
 * @param labels The active label table.
 * @param displayName True for a display name (the renames route's 1,024-byte limit) rather
 *   than a file or folder name (255 bytes).
 * @returns The text to show.
 */
export function problemText(problem: NameProblem, labels: LabelTable, displayName = false): string {
  switch (problem) {
    case "empty":
      return labels.fileNameEmpty;
    case "long":
      return displayName ? labels.fileDisplayNameTooLong : labels.fileNameTooLong;
    default:
      return labels.fileNameInvalid;
  }
}

/**
 * Check the folder a file or folder is moved into: empty is the root, otherwise each segment
 * must be a usable name (so `A//B` is refused as an empty name). Pure.
 *
 * @param folder The folder as typed.
 * @returns The first problem, or undefined.
 */
export function folderProblem(folder: string): NameProblem | undefined {
  const path = folder.trim();
  if (path === "") return undefined;
  for (const segment of path.split("/")) {
    const problem = validateName(segment);
    if (problem) return problem;
  }
  return undefined;
}

/**
 * Where a rename or move lands. Typing the name the user already sees keeps the stored last
 * segment (so a move to another folder never renames the file); a different name becomes the
 * new last segment. Pure.
 *
 * @param row The file or folder.
 * @param name The name as typed.
 * @param folder The destination folder as typed (empty for the root).
 * @returns The new logical path, or undefined when nothing would change.
 */
export function moveTarget(row: FileRow, name: string, folder: string): string | undefined {
  const typed = name.trim();
  const segment = typed === row.name ? lastSegment(row.path) : typed;
  const to = joinPath(folder.trim(), segment);
  return to === row.path ? undefined : to;
}
