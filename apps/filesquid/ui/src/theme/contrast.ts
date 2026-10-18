/**
 * WCAG 2.2 contrast helpers (pure, DOM-free). Every distribution's token test uses them to
 * prove that primary actions and body text meet AA on their surfaces (SRS-UI-007, NFR-006).
 */

/** Minimum contrast ratio for normal-size text under WCAG 2.2 AA (success criterion 1.4.3). */
export const AA_TEXT_CONTRAST = 4.5;

/**
 * Minimum contrast ratio for user-interface components and visual focus indicators against the
 * colours next to them under WCAG 2.2 AA (success criterion 1.4.11 Non-text Contrast), used for
 * the focus ring (UI-S-016). The 3:1 figure also appears in 2.4.13 Focus Appearance, which is
 * Level AAA; FileSquid does not claim AAA conformance.
 */
export const NON_TEXT_CONTRAST = 3;

/** An sRGB colour as 0–255 red, green and blue channels. */
export type Rgb = readonly [number, number, number];

/**
 * Read a `#rgb` or `#rrggbb` colour. Other notations are not read, so a token test
 * reports them instead of guessing.
 *
 * @param value CSS colour text, for example a token value.
 * @returns The channels, or undefined when the text is not a 3- or 6-digit hex colour.
 */
export function parseHexColour(value: string): Rgb | undefined {
  const match = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(value.trim());
  const digits = match?.[1];
  if (!digits) return undefined;
  const full = digits.length === 3 ? [...digits].map((d) => d + d).join("") : digits;
  return [0, 2, 4].map((i) => parseInt(full.slice(i, i + 2), 16)) as unknown as Rgb;
}

/** Linearise one sRGB channel given as 0–1 (WCAG 2.2 relative luminance definition). */
function linear(channel: number): number {
  return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
}

/**
 * Compute the WCAG relative luminance of a colour.
 *
 * @param rgb sRGB channels, 0–255.
 * @returns Luminance from 0 (black) to 1 (white).
 */
export function relativeLuminance(rgb: Rgb): number {
  const [r, g, b] = rgb.map((c) => linear(c / 255)) as unknown as Rgb;
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/**
 * Compute the WCAG 2.2 contrast ratio of two colours, in either order.
 *
 * @param foreground First colour (`#rgb` or `#rrggbb`).
 * @param background Second colour (`#rgb` or `#rrggbb`).
 * @returns The ratio from 1 to 21, or undefined when either colour cannot be read.
 */
export function contrastRatio(foreground: string, background: string): number | undefined {
  const a = parseHexColour(foreground);
  const b = parseHexColour(background);
  if (!a || !b) return undefined;
  const [light, dark] = [relativeLuminance(a), relativeLuminance(b)].sort((x, y) => y - x);
  return ((light ?? 0) + 0.05) / ((dark ?? 0) + 0.05);
}
