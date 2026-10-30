/**
 * Hand a downloaded file to the browser to save (UI-F2.11): the one place the Versions panel
 * touches the DOM to save a file. The side effect is in the name: {@link saveFileToDevice}
 * clicks a temporary download link.
 */

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
