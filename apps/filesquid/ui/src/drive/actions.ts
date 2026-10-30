/**
 * What the Drive-style views do to files (UI-F2.8, UI-S-026): rename, remove with Undo, and
 * move or copy, each through the {@link FileSource} writes of the file routes, never directly.
 * A file of a view is its own object `urn:uuid:<file-id>` (API-S-026): a rename sets the display
 * name through `POST /api/v1/files/renames`, a remove takes the stored original out of the
 * object in one version and Undo restores it from the version before.
 *
 * Each write is a command that expects the object's head as last read, so the head is read just
 * before the write (one listing call). The view rows do not yet carry the library of a file
 * (API gap, reported to the platform engineer), so an action on a row without one is refused
 * with {@link UnavailableActionError} and the menu shows the item disabled with its reason.
 * Moving or copying into a collection needs the destination fields of API-F2.14; until the
 * contract has them the action is refused with {@link DestinationUnavailableError} and nothing is
 * sent, never a copy that no collection would show.
 */
import { ApiError } from "../api/client";
import type { FileCommit, FileLocation, ObjectRef, RenameCommit } from "../files/model";
import { previousVersion } from "../files/model";
import type { FileSource } from "../files/source";
import type { DriveRow } from "./model";

/** The part of the file source the actions use (injectable in tests). */
export type DriveFiles = Pick<FileSource, "list" | "rename" | "remove" | "restore">;

/** Move or copy. */
export type TransferMode = "move" | "copy";

/** Raised when a row cannot be acted on because the host did not name its library. */
export class UnavailableActionError extends Error {
  constructor() {
    super("FileSquid cannot act on this row: the host did not name its library");
    this.name = "UnavailableActionError";
  }
}

/** Raised when the host cannot place a moved or copied file in a collection yet. */
export class DestinationUnavailableError extends Error {
  /**
   * @param mode Whether a move or a copy was asked for.
   * @param count How many files it was asked for.
   */
  constructor(
    readonly mode: TransferMode,
    readonly count: number,
  ) {
    super(`FileSquid cannot ${mode} ${count} file(s) into a collection on this host yet`);
    this.name = "DestinationUnavailableError";
  }
}

/** What a remove leaves behind: the text for the toast and a way to undo it. */
export interface RemovalReceipt {
  /** The removed file's display name. */
  readonly name: string;
  /** The version the removal committed. */
  readonly version: string;
  /** Restore the file from the version before the removal. Network I/O. */
  undo(): Promise<FileCommit>;
}

/**
 * Whether a row can be renamed, removed or moved: a file whose library the host named. Pure.
 *
 * @param row The row.
 * @returns True when the write routes can be addressed.
 */
export function canAct(row: DriveRow): boolean {
  return row.kind === "file" && !!row.libraryId && !!row.fileId && !!row.objectId;
}

/** The file's object, or an error when the row cannot be addressed. */
function objectOf(row: DriveRow): ObjectRef & { fileId: string } {
  if (!canAct(row)) throw new UnavailableActionError();
  return {
    library_id: row.libraryId as string,
    object_id: row.objectId as string,
    fileId: row.fileId as string,
  };
}

/** The Drive view's file actions. */
export class DriveActions {
  /** @param files The file source whose writes the actions use. */
  constructor(private readonly files: DriveFiles) {}

  /**
   * Set a file's display name. Network I/O: one listing read for the head, one write.
   *
   * @param row The file.
   * @param name The new display name (1 to 1,024 bytes; the caller checks first).
   * @returns The version of the file's record that the rename published.
   * @throws UnavailableActionError when the host did not name the file's library.
   * @throws ApiError when the API refuses, `conflict` for the current name included.
   */
  async rename(row: DriveRow, name: string): Promise<RenameCommit> {
    const target = objectOf(row);
    const head = await this.head({ library_id: target.library_id, object_id: target.object_id });
    return this.files.rename({ library_id: target.library_id }, target.fileId, name, {
      revision: head.version,
    });
  }

  /**
   * Remove a file: its stored original leaves the object in one version, the bytes stay in
   * history. Network I/O: reads the original's path, then one write.
   *
   * @param row The file.
   * @returns The receipt, with Undo.
   * @throws UnavailableActionError when the host did not name the file's library.
   * @throws ApiError when the API refuses.
   */
  async remove(row: DriveRow): Promise<RemovalReceipt> {
    const target = objectOf(row);
    const object: ObjectRef = { library_id: target.library_id, object_id: target.object_id };
    const original = await this.original(object);
    const commit = await this.files.remove(object, original.path, { revision: original.version });
    const before = previousVersion(commit.version);
    return {
      name: row.name,
      version: commit.version,
      undo: () => this.restoreOriginal(object, original.path, before, commit.version),
    };
  }

  /**
   * Move or copy rows into a collection or folder. The contract has no destination yet
   * (API-F2.14), so nothing is sent.
   *
   * @param rows The files.
   * @param mode Move or copy.
   * @throws DestinationUnavailableError always, until the contract carries the destination.
   */
  moveOrCopy(rows: readonly DriveRow[], mode: TransferMode): never {
    throw new DestinationUnavailableError(mode, rows.length);
  }

  /** The object's head version, from a one-entry listing of its root. */
  private head(object: ObjectRef): Promise<{ version: string }> {
    return this.files.list({ ...object, prefix: "" }, null, 1);
  }

  /** The path and head of the file's stored original. */
  private async original(object: ObjectRef): Promise<{ path: string; version: string }> {
    const location: FileLocation = { ...object, prefix: "original" };
    const page = await this.files.list(location, null, 1);
    const entry = page.entries.find((candidate) => candidate.kind === "file");
    if (!entry) throw new ApiError(404, "/api/v1/files/listing", { code: "not_found" });
    return { path: entry.path, version: page.version };
  }

  /** Restore the original from the version before the removal. */
  private restoreOriginal(
    object: ObjectRef,
    path: string,
    before: string | undefined,
    revision: string,
  ): Promise<FileCommit> {
    if (before === undefined)
      throw new ApiError(409, "/api/v1/files/restorations", { code: "conflict" });
    return this.files.restore(object, before, [path], { revision });
  }
}
