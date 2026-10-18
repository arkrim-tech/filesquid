/**
 * Locale hooks (UI-S-016, PRD NFR-007): a distribution declares the language and writing
 * direction of its label table, and the page's root element carries them, so screen readers
 * pick the right voice and right-to-left scripts mirror the layout. Component stylesheets use
 * logical properties only, so `dir="rtl"` is all a right-to-left distribution needs.
 */
import type { Distribution } from "../distribution";

/** Writing directions a distribution can declare. */
export type TextDirection = "ltr" | "rtl";

/** The language and direction of a distribution's label table. */
export interface Locale {
  /** BCP 47 language tag, for example "en" or "ar". */
  lang: string;
  /** Writing direction. */
  dir: TextDirection;
}

/** The locale of a distribution that declares none: English, left to right. */
export const DEFAULT_LOCALE: Readonly<Locale> = Object.freeze({ lang: "en", dir: "ltr" });

/**
 * The locale a distribution declares, or {@link DEFAULT_LOCALE}. Pure.
 *
 * @param distribution The active distribution.
 * @returns Its language and direction.
 * @throws Error when the direction is not `ltr` or `rtl`, or the language is blank.
 */
export function resolveLocale(distribution: Distribution): Locale {
  const locale = distribution.locale ?? DEFAULT_LOCALE;
  if (locale.dir !== "ltr" && locale.dir !== "rtl") {
    throw new Error(
      `Distribution ${distribution.id}: direction "${String(locale.dir)}" is not ltr or rtl`,
    );
  }
  if (locale.lang.trim() === "") {
    throw new Error(`Distribution ${distribution.id}: the locale needs a language tag`);
  }
  return { lang: locale.lang, dir: locale.dir };
}

/**
 * Set `lang` and `dir` on the document's root element. Side effect: mutates the document.
 *
 * @param doc The page's document.
 * @param locale The language and direction to apply.
 */
export function applyLocale(doc: Document, locale: Locale): void {
  doc.documentElement.setAttribute("lang", locale.lang);
  doc.documentElement.setAttribute("dir", locale.dir);
}
