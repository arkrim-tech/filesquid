/**
 * A distribution is a UI layer over the FileSquid API: brand, vocabulary and views
 * (ADR 029, ADR 031). It contains no backend code.
 */
import type { LabelTable } from "./theme/labels";
import type { ResolvedTheme } from "./theme/theme-store";

/** A distribution's logo, with an optional variant for dark surfaces. */
export interface DistributionLogo {
  /** Logo for light surfaces (and for both themes when `srcDark` is absent). */
  src: string;
  /** Logo for dark surfaces, for a mark whose outline or wordmark must turn light. */
  srcDark?: string;
  /** Text alternative. */
  alt: string;
}

/** Brand and vocabulary a distribution passes to FileSquid components. */
export interface Distribution {
  /** Stable identifier, for example "filesquid" or "imagesquid". */
  id: string;
  /** Product name shown in the shell header and window title. */
  productName: string;
  /** Logo image shown next to the product name. */
  logo: DistributionLogo;
  /** Visible nouns keyed by internal names. */
  labels: LabelTable;
}

/**
 * Pick the logo file for the theme shown.
 *
 * @param logo The distribution's logo.
 * @param theme The resolved theme.
 * @returns `srcDark` in the dark theme when the distribution has one, otherwise `src`.
 */
export function logoSource(logo: DistributionLogo, theme: ResolvedTheme): string {
  return theme === "dark" && logo.srcDark ? logo.srcDark : logo.src;
}
