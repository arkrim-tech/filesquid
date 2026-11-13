/**
 * The scale the preview's zoom chip states (UI-S-031, Task UI-F2.19): "Fit · {percent}%", where
 * the percent is the shown image's fitted width over its natural width. Pure arithmetic; the
 * chip itself is built by the image renderer in `preview.ts`.
 */

/**
 * The fitted scale of an image as a whole percent. Pure.
 *
 * @param shown The width the image is drawn at, in CSS pixels.
 * @param natural The image's natural width, in its own pixels.
 * @returns The rounded percent, or null when either width is unknown, zero or not finite.
 */
export function fitPercent(shown: number, natural: number): number | null {
  const known = Number.isFinite(shown) && Number.isFinite(natural);
  if (!known || shown <= 0 || natural <= 0) return null;
  return Math.round((shown / natural) * 100);
}
