/**
 * File-type badges of the active mockup suite (UI-S-024, UI-S-026): the upper-case extension on
 * one of three colours, red for documents, yellow for data and blue for media, as the mockups
 * draw them. Anything else is shown on the raised surface. Pure rules; the components draw the
 * badge through `--fs-badge-*` tokens.
 */

/** The colour family of a badge. */
export type BadgeKind = "document" | "data" | "media" | "other";

/** Extensions the mockups draw in red. */
const DOCUMENT = new Set(["pdf", "docx", "doc", "txt", "md", "rtf", "odt", "pages"]);
/** Extensions the mockups draw in yellow. */
const DATA = new Set(["xlsx", "xls", "csv", "blend", "env", "gz", "zip", "json", "xml", "tar"]);
/** Extensions the mockups draw in blue. */
const MEDIA = new Set([
  "jpg",
  "jpeg",
  "png",
  "gif",
  "heic",
  "tif",
  "tiff",
  "webp",
  "m4a",
  "mp3",
  "wav",
  "mp4",
  "mov",
  "mkv",
  "cr3",
  "cr2",
  "dng",
  "raf",
  "nef",
  "arw",
]);

/**
 * The upper-case extension of a display name, without its dot. Pure.
 *
 * @param name A display name such as `budget-2026.xlsx`.
 * @returns `XLSX`, or an empty string for a name with no extension or a leading-dot name.
 */
export function extensionOf(name: string): string {
  const dot = name.lastIndexOf(".");
  if (dot <= 0 || dot === name.length - 1) return "";
  return name.slice(dot + 1).toUpperCase();
}

/**
 * The colour family of an extension. Pure.
 *
 * @param extension The extension in any case, with or without its dot.
 * @returns `document`, `data`, `media` or `other`.
 */
export function badgeKind(extension: string): BadgeKind {
  const key = extension.replace(/^\./, "").toLowerCase();
  if (DOCUMENT.has(key)) return "document";
  if (DATA.has(key)) return "data";
  if (MEDIA.has(key)) return "media";
  return "other";
}
