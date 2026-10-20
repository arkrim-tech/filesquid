/**
 * The write operations of the file browser (UI-F2.2) that need more than one call or some
 * rules of their own: uploading a batch of files one commit at a time, the name a file is
 * uploaded under, where a rename or move lands, and the words for a failed call. Everything
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
  joinPath,
  lastSegment,
  validateName,
  type FileCommit,
  type FileRow,
  type NameProblem,
  type ObjectRef,
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
  /** The commits, one per file uploaded. */
  readonly committed: readonly FileCommit[];
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
 * Upload files one at a time, each one `PUT` that commits one version, into `prefix`. A file
 * over 64 MiB is refused without being sent; a failed file is recorded and the rest continue
 * (the API refuses a change while another is staged, so files are never sent together). At most
 * {@link MAX_UPLOAD_FILES} files are tried. Network I/O through `source`.
 *
 * @param source The file source.
 * @param object The library and object.
 * @param prefix The folder to upload into.
 * @param files The files.
 * @param progress Called before each file with its place (1-based), the total and its name.
 * @returns The commits, the failures and the count not tried.
 */
export async function uploadFiles(
  source: FileSource,
  object: ObjectRef,
  prefix: string,
  files: readonly File[],
  progress: (current: number, total: number, name: string) => void = () => undefined,
): Promise<UploadResult> {
  const batch = files.slice(0, MAX_UPLOAD_FILES);
  const committed: FileCommit[] = [];
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
      committed.push(await source.upload(object, joinPath(prefix, name), file));
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

/** The label for a name problem. */
export function problemText(problem: NameProblem, labels: LabelTable): string {
  switch (problem) {
    case "empty":
      return labels.fileNameEmpty;
    case "long":
      return labels.fileNameTooLong;
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
