/**
 * UI-S-016 / NFR-007 locale hooks: a distribution may declare its language and writing
 * direction; the default is English, left to right; anything but `ltr` or `rtl` is refused.
 */
import { describe, expect, it } from "vitest";
import { DEFAULT_LOCALE, applyLocale, resolveLocale } from "../../src/a11y/locale";
import { prefersReducedMotion, REDUCED_MOTION_QUERY } from "../../src/a11y/motion";
import type { Distribution } from "../../src/distribution";
import labels from "../../src/theme/labels.en.json";
import * as entry from "../../src/index";

const base: Distribution = {
  id: "filesquid",
  productName: "FileSquid",
  logo: { src: "/filesquid-mark.svg", alt: "FileSquid logo" },
  labels,
};

describe("resolveLocale", () => {
  it("defaults to English, left to right", () => {
    expect(DEFAULT_LOCALE).toEqual({ lang: "en", dir: "ltr" });
    expect(resolveLocale(base)).toEqual({ lang: "en", dir: "ltr" });
  });
  it("uses the distribution's language and direction", () => {
    expect(resolveLocale({ ...base, locale: { lang: "ar", dir: "rtl" } })).toEqual({
      lang: "ar",
      dir: "rtl",
    });
  });
  it("refuses a direction other than ltr or rtl, and an empty language", () => {
    const bad = { ...base, locale: { lang: "en", dir: "auto" } } as unknown as Distribution;
    expect(() => resolveLocale(bad)).toThrow(/direction "auto"/);
    expect(() => resolveLocale({ ...base, locale: { lang: " ", dir: "ltr" } })).toThrow(/language/);
  });
});

describe("applyLocale", () => {
  it("sets lang and dir on the root element", () => {
    const doc = document.implementation.createHTMLDocument("t");
    applyLocale(doc, { lang: "he", dir: "rtl" });
    expect(doc.documentElement.getAttribute("lang")).toBe("he");
    expect(doc.documentElement.getAttribute("dir")).toBe("rtl");
  });
  it("is exported from @filesquid/ui", () => {
    expect(entry.applyLocale).toBe(applyLocale);
    expect(entry.resolveLocale).toBe(resolveLocale);
  });
});

describe("prefersReducedMotion", () => {
  it("reads the prefers-reduced-motion media query", () => {
    const queries: string[] = [];
    const win = (matches: boolean) => ({
      matchMedia: (query: string) => {
        queries.push(query);
        return { matches } as MediaQueryList;
      },
    });
    expect(prefersReducedMotion(win(true))).toBe(true);
    expect(prefersReducedMotion(win(false))).toBe(false);
    expect(queries).toEqual([REDUCED_MOTION_QUERY, REDUCED_MOTION_QUERY]);
    expect(REDUCED_MOTION_QUERY).toBe("(prefers-reduced-motion: reduce)");
  });
  it("treats a window without matchMedia as no preference", () => {
    expect(prefersReducedMotion({})).toBe(false);
  });
  it("is exported from @filesquid/ui", () => {
    expect(entry.prefersReducedMotion).toBe(prefersReducedMotion);
  });
});
