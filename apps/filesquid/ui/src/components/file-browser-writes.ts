/**
 * The write side of `<fs-file-browser>` (UI-F2.2): upload, new folder, rename or move, remove
 * and Undo (restore). Each is one API call through the {@link FileSource} that commits one OCFL
 * version (API-S-008), and only a committed answer is reported as done (never UI state alone). A
 * refusal is shown in the alert region with the label for its reason; a failed call changes
 * nothing on screen. The controller wires the toolbar and the name form and reports in the two
 * live regions of the frame.
 *
 * Remove and restore are the reversible pair the plan names: the Undo offer restores from the
 * version before the removal. There is no control that deletes for good.
 */
import { formatLabel, type LabelTable } from "../theme/labels";
import {
  MAX_DISPLAY_NAME_BYTES,
  MAX_NAME_BYTES,
  MAX_UPLOAD_BYTES,
  formatSize,
  joinPath,
  parentPath,
  previousVersion,
  validateName,
  type FileCommit,
  type FileLocation,
  type FileRow,
  type ObjectRef,
} from "../files/model";
import {
  errorKind,
  failureText,
  folderProblem,
  MAX_UPLOAD_FILES,
  moveTarget,
  problemText,
  renameFailureText,
  renameFileId,
  uploadFiles,
  type UploadFailure,
  type UploadResult,
} from "../files/ops";
import type { FileSource } from "../files/source";
import type { Frame } from "./file-browser-frame";

/** What a write did, for the browser's `fs-files-change` event. */
export type WriteOperation = "upload" | "create-folder" | "move" | "rename" | "remove" | "restore";

/** What the controller needs from the browser. */
export interface WritesHost {
  /** The active label table. */
  readonly labels: LabelTable;
  /** The file source. */
  readonly source: FileSource;
  /** The object and folder shown. */
  location(): FileLocation;
  /**
   * The head version of the object shown, as last listed: the revision every write in it
   * expects (API-F2.13); a stale one is refused as `conflict`.
   */
  revision(): string;
  /** The row the toolbar acts on, or undefined. */
  target(): FileRow | undefined;
  /** Load the listing again after a commit. */
  reload(): Promise<void>;
  /** A write committed `commit`; `changed` are the logical paths it touched. */
  committed(operation: WriteOperation, commit: FileCommit, changed: readonly string[]): void;
  /**
   * The name the listing of `location` shows for the row of `path`, or undefined when that
   * listing is not the one shown and loaded now, or does not list the path.
   */
  nameOf(location: FileLocation, path: string): string | undefined;
  /** Remember a folder created in the client until a file is added beneath it. */
  pendFolder(path: string): void;
  /** Move focus to the row of `path` once it is listed again. */
  focusPath(path: string): void;
}

/** What the name form is for. */
type FormMode = { readonly kind: "folder" } | { readonly kind: "rename"; readonly row: FileRow };

/** What Undo restores. */
interface UndoOffer {
  readonly object: ObjectRef;
  readonly version: string;
  /** The head the removal published, which the restore expects. */
  readonly revision: string;
  readonly paths: readonly string[];
  readonly name: string;
}

/** A button the live regions offer. */
interface Action {
  readonly className: string;
  readonly label: string;
  readonly run: () => void;
}

/** True for a toolbar button that is marked unavailable. */
function unavailable(element: HTMLElement): boolean {
  return element.getAttribute("aria-disabled") === "true";
}

/** The write controller of one browser. */
export class FileWrites {
  private mode?: FormMode;
  private opener?: HTMLElement;
  private busy = false;
  private undoOffer?: UndoOffer;
  private retryable: File[] = [];
  private retryWhere?: FileLocation;

  /**
   * @param frame The frame whose controls and live regions this wires.
   * @param host What the browser provides.
   */
  constructor(
    private readonly frame: Frame,
    private readonly host: WritesHost,
  ) {
    const { bar, form } = frame;
    bar.upload.addEventListener("click", () => this.guard(bar.upload, () => bar.input.click()));
    bar.input.addEventListener("change", () => void this.chosen());
    bar.newFolder.addEventListener("click", () =>
      this.guard(bar.newFolder, () => this.open({ kind: "folder" }, bar.newFolder)),
    );
    bar.rename.addEventListener("click", () => this.guard(bar.rename, () => this.rename()));
    bar.remove.addEventListener("click", () => this.guard(bar.remove, () => void this.remove()));
    form.element.addEventListener("submit", (event) => {
      event.preventDefault();
      void this.submit();
    });
    form.cancel.addEventListener("click", () => this.close(true));
    form.element.addEventListener("keydown", (event) => {
      if (event.key === "Escape") this.close(true);
    });
  }

  /** Mark Rename and Remove available only when a row is the target and nothing is running. */
  syncToolbar(): void {
    const { bar } = this.frame;
    const has = this.host.target() !== undefined;
    bar.rename.setAttribute("aria-disabled", String(!has || this.busy));
    bar.remove.setAttribute("aria-disabled", String(!has || this.busy));
    bar.upload.setAttribute("aria-disabled", String(this.busy));
    bar.newFolder.setAttribute("aria-disabled", String(this.busy));
  }

  /** The library and object shown now. */
  private objectNow(): ObjectRef {
    const where = this.host.location();
    return { library_id: where.library_id, object_id: where.object_id };
  }

  /**
   * The browser moved to another folder or object: close the name form, which names the
   * folder it was opened for, without returning focus. A call still in flight answers later
   * without touching the form or focus of the new location.
   */
  leave(): void {
    if (this.mode) this.close(false);
  }

  /** True when `object` is the object shown now. */
  private isShown(object: ObjectRef): boolean {
    const where = this.host.location();
    return where.library_id === object.library_id && where.object_id === object.object_id;
  }

  /** Open the rename or move form for the target row (F2). */
  rename(row: FileRow | undefined = this.host.target()): void {
    if (row) this.open({ kind: "rename", row }, this.focused());
  }

  /**
   * Remove a file or folder with one call and offer Undo (Delete).
   *
   * @param row The row to remove; the target row by default.
   * @returns Resolves when the call and the listing refresh are done.
   */
  async remove(row: FileRow | undefined = this.host.target()): Promise<void> {
    if (!row || this.busy) return;
    const object = this.objectNow();
    const revision = this.host.revision();
    const outcome = await this.run(() => this.host.source.remove(object, row.path, { revision }));
    if (!outcome) return;
    this.host.committed("remove", outcome, [row.path]);
    const version = previousVersion(outcome.version);
    const paths = [row.path];
    this.undoOffer = version
      ? { object, version, revision: outcome.version, paths, name: row.name }
      : undefined;
    const text = formatLabel(this.host.labels.fileRemoved, {
      name: row.name,
      version: outcome.version,
    });
    this.showStatus(
      text,
      this.undoOffer
        ? [
            this.action("undo", this.host.labels.fileUndo, () => void this.undo()),
            this.dismissAction(),
          ]
        : [],
    );
    await this.host.reload();
  }

  /**
   * Upload files into a library, each as its own file (API-S-026): the library shown (the
   * chosen files, or dropped ones), or the one a failed batch was meant for when it is tried
   * again. Each file is one `PUT` that commits one version in that file's own object, so the
   * folder shown gains no version.
   *
   * @param files The files.
   * @param where The location whose library to upload into; the one shown by default.
   * @returns Resolves when every file was tried and the listing refreshed.
   */
  async upload(files: readonly File[], where: FileLocation = this.host.location()): Promise<void> {
    if (files.length === 0 || this.busy) return;
    const { labels, source } = this.host;
    this.setBusy(true);
    const result = await uploadFiles(source, where, files, (current, total, name) =>
      this.showStatus(
        formatLabel(labels.fileUploading, { current: String(current), total: String(total), name }),
      ),
    );
    this.setBusy(false);
    const last = result.committed[result.committed.length - 1];
    if (last) this.host.committed("upload", last, []);
    this.reportUpload(result.committed.length, last, result, where);
    if (last) await this.host.reload();
  }

  /** Files chosen in the hidden input: upload them and clear the input. */
  private async chosen(): Promise<void> {
    const input = this.frame.bar.input;
    const files = Array.from(input.files ?? []);
    input.value = "";
    await this.upload(files);
  }

  /** Report a finished upload batch into `where`: committed, or what failed. */
  private reportUpload(
    count: number,
    last: FileCommit | undefined,
    result: UploadResult,
    where: FileLocation,
  ): void {
    const { labels } = this.host;
    const failed = result.failed;
    const retriable = failed.filter((f) => typeof f.reason !== "string").map((f) => f.file);
    this.retryable = [...retriable, ...result.left];
    this.retryWhere = where;
    if (failed.length === 0 && result.skipped === 0 && last) {
      this.showStatus(
        formatLabel(labels.fileUploaded, { count: String(count), version: last.version }),
      );
      return;
    }
    const lines = failed.map((f) => this.failureLine(f));
    if (result.skipped > 0) {
      const text = formatLabel(labels.fileUploadSkipped, {
        count: String(result.skipped),
        limit: String(MAX_UPLOAD_FILES),
      });
      lines.push(text);
    }
    const retry =
      this.retryable.length > 0
        ? [this.action("retry", labels.stateRetry, () => void this.retry())]
        : [];
    this.showAlert(lines, retry);
  }

  /** Try the files of the last failed batch again, in the library they were meant for. */
  private async retry(): Promise<void> {
    await this.upload(this.retryable, this.retryWhere);
  }

  /** One line for a file that was not uploaded. */
  private failureLine(failure: UploadFailure): string {
    const { labels } = this.host;
    if (failure.reason === "too_large") {
      const limit = formatSize(MAX_UPLOAD_BYTES, labels);
      return formatLabel(labels.fileTooLarge, { name: failure.name, limit });
    }
    if (failure.reason === "name_too_long") {
      return `${formatLabel(labels.fileUploadFailed, { name: failure.name })} ${labels.fileNameTooLong}`;
    }
    const head = formatLabel(labels.fileUploadFailed, { name: failure.name });
    return `${head} ${failureText(failure.reason.error, labels)}`;
  }

  /** Restore what the last removal took away, from the version before it. */
  private async undo(): Promise<void> {
    const offer = this.undoOffer;
    if (!offer || this.busy) return;
    const outcome = await this.run(() =>
      this.host.source.restore(offer.object, offer.version, offer.paths, {
        revision: offer.revision,
      }),
    );
    if (!outcome) return;
    this.undoOffer = undefined;
    this.host.committed("restore", outcome, offer.paths);
    const text = formatLabel(this.host.labels.fileRestored, {
      name: offer.name,
      version: outcome.version,
    });
    this.showStatus(text);
    await this.host.reload();
    if (this.isShown(offer.object)) this.host.focusPath(offer.paths[0] ?? "");
  }

  /** Open the name form for `mode`, remembering where focus returns. */
  private open(mode: FormMode, opener: HTMLElement | undefined): void {
    const { form } = this.frame;
    const { labels } = this.host;
    this.mode = mode;
    this.opener = opener;
    const rename = mode.kind === "rename";
    const where = this.host.location();
    const folder =
      where.prefix === "" ? labels.fileRoot : (where.prefix.split("/").pop() ?? labels.fileRoot);
    const only = rename && this.renameOnly(mode.row);
    const renameTitle = only ? labels.fileRenameOnlyTitle : labels.fileRenameTitle;
    const title = rename
      ? formatLabel(renameTitle, { name: mode.row.name })
      : formatLabel(labels.fileNewFolderTitle, { folder });
    form.element.setAttribute("aria-label", title);
    this.setFieldLabel(rename ? labels.fileFieldName : labels.fileFolderName);
    form.submit.textContent = rename ? labels.fileSave : labels.fileCreate;
    form.folderField.hidden = !rename || only;
    form.name.value = rename ? mode.row.name : "";
    form.folder.value = rename ? parentPath(mode.row.path) : "";
    this.formError("");
    form.element.hidden = false;
    form.name.focus();
    form.name.select();
  }

  /** Set the text of the name field's label. */
  private setFieldLabel(text: string): void {
    const label = this.frame.form.name.parentElement?.querySelector(".field-label");
    if (label) label.textContent = text;
  }

  /** Show or clear the form's own error. */
  private formError(text: string): void {
    this.frame.form.error.textContent = text;
  }

  /** Close the form; when `restore`, return focus to the control that opened it. */
  private close(restore: boolean): void {
    const opener = this.opener;
    this.mode = undefined;
    this.opener = undefined;
    this.frame.form.element.hidden = true;
    this.formError("");
    if (restore && opener?.isConnected) opener.focus();
  }

  /** Validate and run the form. */
  private async submit(): Promise<void> {
    const mode = this.mode;
    if (!mode || this.busy) return;
    const { form } = this.frame;
    const moves = mode.kind === "rename" && !this.renameOnly(mode.row);
    const displayName = mode.kind === "rename" && !moves;
    const limit = displayName ? MAX_DISPLAY_NAME_BYTES : MAX_NAME_BYTES;
    const problem =
      validateName(form.name.value, limit) ??
      (moves ? folderProblem(form.folder.value) : undefined);
    if (problem) return this.formError(problemText(problem, this.host.labels, displayName));
    this.formError("");
    if (mode.kind === "folder") await this.createFolder(mode, form.name.value.trim());
    else if (moves) await this.move(mode, form.name.value, form.folder.value);
    else await this.renameFile(mode, form.name.value);
  }

  /**
   * Whether the row is renamed by the renames route: the stored original of a file's own
   * object, which never moves and has no folder to choose (API-S-027).
   */
  private renameOnly(row: FileRow): boolean {
    return renameFileId(row, this.host.location().object_id) !== undefined;
  }

  /**
   * Set a file's display name with one call to the renames route. The name shown afterwards is
   * the one the reloaded listing carries. A refusal is shown with the label for its reason.
   * Focus returns to the opener only while the form that asked is still open.
   */
  private async renameFile(
    mode: Extract<FormMode, { kind: "rename" }>,
    name: string,
  ): Promise<void> {
    const row = mode.row;
    const typed = name.trim();
    const where = this.host.location();
    const fileId = renameFileId(row, where.object_id);
    if (fileId === undefined || typed === row.name) return this.close(true);
    const options = { revision: this.host.revision() };
    const commit = await this.run(
      () => this.host.source.rename({ library_id: where.library_id }, fileId, typed, options),
      renameFailureText,
    );
    if (!commit) return;
    if (this.mode === mode) this.close(true);
    this.host.committed("rename", commit, [row.path]);
    await this.host.reload();
    // Only the listing of the place the rename was asked in, once loaded, names the file.
    const shown = this.host.nameOf(where, row.path) ?? typed;
    this.showStatus(
      formatLabel(this.host.labels.fileRenamed, { name: shown, version: commit.version }),
    );
  }

  /**
   * Create a folder in the current folder (one call; nothing is written to the object). An
   * answer that arrives after the browser left that object marks nothing pending in the new one.
   */
  private async createFolder(mode: FormMode, name: string): Promise<void> {
    const where = this.host.location();
    const path = joinPath(where.prefix, name);
    const revision = this.host.revision();
    const answer = await this.run(() => this.host.source.createFolder(where, path, { revision }));
    if (!answer) return;
    if (answer.pending && this.isShown(where)) this.host.pendFolder(path);
    const text = answer.pending
      ? this.host.labels.fileFolderCreated
      : this.host.labels.fileFolderExists;
    if (this.mode === mode) this.close(true);
    this.showStatus(formatLabel(text, { name }));
    await this.host.reload();
  }

  /**
   * Rename or move the form's row with one call. Focus returns to the opener only while the
   * form that asked is still open, so a late answer leaves a new location alone.
   */
  private async move(
    mode: Extract<FormMode, { kind: "rename" }>,
    name: string,
    folder: string,
  ): Promise<void> {
    const row = mode.row;
    const to = moveTarget(row, name, folder);
    if (to === undefined) return this.close(true);
    const where = this.host.location();
    const revision = this.host.revision();
    const commit = await this.run(() => this.host.source.move(where, row.path, to, { revision }));
    if (!commit) return;
    if (this.mode === mode) this.close(true);
    this.host.committed("move", commit, [row.path, to]);
    const shown = name.trim() === "" ? row.name : name.trim();
    this.showStatus(
      formatLabel(this.host.labels.fileMoved, { name: shown, version: commit.version }),
    );
    await this.host.reload();
  }

  /**
   * Run one API call with the controller marked busy. A rejection is shown in the alert region
   * (worded by `words`, the generic failure text by default) and logged by kind only; nothing
   * else changes.
   */
  private async run<T>(
    call: () => Promise<T>,
    words: (error: unknown, labels: LabelTable) => string = failureText,
  ): Promise<T | undefined> {
    this.setBusy(true);
    try {
      return await call();
    } catch (error) {
      console.warn("FileSquid file action failed", errorKind(error));
      this.showAlert([words(error, this.host.labels)], []);
      return undefined;
    } finally {
      this.setBusy(false);
    }
  }

  /** Mark the controller busy or idle and refresh the toolbar. */
  private setBusy(busy: boolean): void {
    this.busy = busy;
    this.frame.root.toggleAttribute("aria-busy", busy);
    this.syncToolbar();
  }

  /** Run `action` unless the control is marked unavailable. */
  private guard(control: HTMLElement, action: () => void): void {
    if (!unavailable(control)) action();
  }

  /** The focused element of the browser's shadow root, if any. */
  private focused(): HTMLElement | undefined {
    const root = this.frame.root.getRootNode();
    const active = root instanceof ShadowRoot ? root.activeElement : null;
    return active instanceof HTMLElement ? active : undefined;
  }

  /** A button for the live regions. */
  private action(className: string, label: string, run: () => void): Action {
    return { className, label, run };
  }

  /** The Dismiss button of a status. */
  private dismissAction(): Action {
    return this.action("dismiss", this.host.labels.fileDismiss, () => this.dismiss());
  }

  /** Clear the status and its Undo offer. */
  private dismiss(): void {
    this.undoOffer = undefined;
    this.frame.status.replaceChildren();
  }

  /** Show a polite status (and clear any alert). */
  private showStatus(text: string, actions: readonly Action[] = []): void {
    this.frame.alert.replaceChildren();
    this.frame.status.replaceChildren(paragraph(text), ...actions.map(actionButton));
  }

  /** Show an alert (and clear any status). */
  private showAlert(lines: readonly string[], actions: readonly Action[]): void {
    this.frame.status.replaceChildren();
    this.frame.alert.replaceChildren(...lines.map(paragraph), ...actions.map(actionButton));
  }
}

/** A `<p>` with text. */
function paragraph(text: string): HTMLParagraphElement {
  const p = document.createElement("p");
  p.textContent = text;
  return p;
}

/** The button of an {@link Action}. */
function actionButton(action: Action): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = action.className;
  button.textContent = action.label;
  button.addEventListener("click", action.run);
  return button;
}
