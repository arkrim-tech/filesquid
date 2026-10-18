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
 * @throws Error when the direction is not `ltr` or `rtl`, or the language is blank or not a
 *   well-formed BCP 47 tag. The returned language is trimmed and canonical ("en-US").
 */
export function resolveLocale(distribution: Distribution): Locale {
  const locale = distribution.locale ?? DEFAULT_LOCALE;
  if (locale.dir !== "ltr" && locale.dir !== "rtl") {
    throw new Error(
      `Distribution ${distribution.id}: direction "${String(locale.dir)}" is not ltr or rtl`,
    );
  }
  return { lang: canonicalLanguage(distribution.id, locale.lang), dir: locale.dir };
}

/**
 * Trim a BCP 47 language tag and return its canonical form. Pure.
 *
 * @param id The distribution's id, for the error message.
 * @param lang The declared tag, for example " en-us ".
 * @returns The canonical tag, for example "en-US".
 * @throws Error when the tag is blank or not a well-formed BCP 47 tag.
 */
function canonicalLanguage(id: string, lang: string): string {
  const tag = lang.trim();
  if (tag === "") throw new Error(`Distribution ${id}: the locale needs a language tag`);
  try {
    return Intl.getCanonicalLocales(tag)[0] ?? tag;
  } catch (error) {
    throw new Error(`Distribution ${id}: language "${tag}" is not a BCP 47 tag`, {
      cause: error,
    });
  }
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
