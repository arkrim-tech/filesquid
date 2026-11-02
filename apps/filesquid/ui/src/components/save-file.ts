/**
 * Hand a downloaded file to the browser to save (UI-F2.11, UI-F2.16): the one place the Versions
 * panel and the Logs download touch the DOM to save a file. The side effect is in the name:
 * {@link saveFileToDevice} clicks a temporary download link, and {@link writeToPickedFile} writes
 * to a file the person chose in the platform's save dialog, where the browser has one.
 */

/** The part of a file handle the save dialog gives that the download writes through. */
export interface SaveTarget {
  /** The name the person chose. */
  readonly name: string;
  /** Open the file for writing. */
  createWritable(): Promise<{
    write(data: Blob): Promise<void>;
    close(): Promise<void>;
    abort(): Promise<void>;
  }>;
}

/** What a page may have: the platform's save dialog (`showSaveFilePicker`). */
type PickerWindow = {
  showSaveFilePicker?: (options: {
    suggestedName: string;
    types?: { description: string; accept: Record<string, string[]> }[];
  }) => Promise<SaveTarget>;
};

/**
 * Save a Blob as a file through the browser's own download. Side effect: adds a hidden link to
 * the page, clicks it and removes it, and revokes the object URL it made, also when the click
 * throws.
 *
 * @param blob The file's bytes.
 * @param name The file name to save under (the file's display name).
 * @throws Error when the browser refuses to click the link; the object URL is revoked first.
 */
export function saveFileToDevice(blob: Blob, name: string): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = name;
  link.hidden = true;
  document.body.append(link);
  try {
    link.click();
  } finally {
    link.remove();
    URL.revokeObjectURL(url);
  }
}

/**
 * Whether this page can open the platform's save dialog.
 *
 * @param win The window.
 */
export function canPickSaveFile(win: Window): boolean {
  return typeof (win as PickerWindow).showSaveFilePicker === "function";
}

/**
 * Open the platform's save dialog. Side effect: shows the dialog.
 *
 * @param win The window.
 * @param suggestedName The name the dialog starts with.
 * @param description The file type's words, from the label table.
 * @param extension The extension with its dot (`.csv`).
 * @param mime The media type.
 * @returns The chosen target, or undefined when the person cancelled the dialog.
 * @throws Error when the page has no save dialog or the dialog failed for another reason.
 */
export async function pickSaveFile(
  win: Window,
  suggestedName: string,
  description: string,
  extension: string,
  mime: string,
): Promise<SaveTarget | undefined> {
  const picker = (win as PickerWindow).showSaveFilePicker;
  if (typeof picker !== "function") throw new Error("This page has no save dialog");
  try {
    return await picker.call(win, {
      suggestedName,
      types: [{ description, accept: { [mime]: [extension] } }],
    });
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") return undefined;
    throw error;
  }
}

/**
 * Write a Blob to a file the person chose. Side effect: writes the file; on a failed write the
 * writer is aborted, so no half-written file is left.
 *
 * @param target The chosen file.
 * @param blob The bytes.
 * @throws Error when the write fails.
 */
export async function writeToPickedFile(target: SaveTarget, blob: Blob): Promise<void> {
  const writer = await target.createWritable();
  try {
    await writer.write(blob);
    await writer.close();
  } catch (error) {
    await writer.abort().catch((abortError: unknown) => {
      console.warn("FileSquid could not abort a failed write", abortError);
    });
    throw error;
  }
}
