// @vitest-environment node
/**
 * Distribution completeness (UI-S-005, SRS-UI-007): every distribution's label table MUST
 * define every key of the default table, and its token file every default token, in light
 * and dark; this test fails otherwise. Its colour tokens MUST also meet WCAG 2.2 AA with no
 * exemption (UI-S-015): FileSquid's listed design-guide pairs apply to the default only.
 *
 * The test reads each distribution's tables straight from its directory, so it needs no
 * install of that distribution. During the MVP the pre-commit hook runs no ImageSquid suite
 * (owner decision 2026-10-23, `.agents/rules/local-verification.md`), and a change confined
 * to `apps/imagesquid` runs this package's suite instead, so this file is what guards
 * ImageSquid's tables. A missing table file fails the test; it is never skipped. ImageSquid's
 * own vocabulary and brand checks stay in `apps/imagesquid/tests/unit/theme.test.ts`.
 */
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import defaultLabels from "../../src/theme/labels.en.json";
import { missingLabels } from "../../src/theme/labels";
import {
  contrastFailures,
  invalidColourTokens,
  missingTokens,
  tokenValues,
} from "../../src/theme/tokens";

/** A distribution checked here: its display name and the directory of its theme tables. */
interface DistributionTables {
  readonly name: string;
  readonly themeDir: URL;
}

/** Every distribution in this repository whose tables must stay complete. */
const DISTRIBUTIONS: readonly DistributionTables[] = [
  { name: "ImageSquid", themeDir: new URL("../../../../imagesquid/src/theme/", import.meta.url) },
];

const defaultTokensCss = readFileSync(
  new URL("../../src/theme/tokens.css", import.meta.url),
  "utf8",
);

/**
 * Read a distribution's label table.
 * @param dir - The distribution's theme directory.
 * @returns The parsed `labels.en.json` table.
 */
function readLabels(dir: URL): Record<string, unknown> {
  return JSON.parse(readFileSync(new URL("labels.en.json", dir), "utf8")) as Record<
    string,
    unknown
  >;
}

/**
 * Read a distribution's token file.
 * @param dir - The distribution's theme directory.
 * @returns The text of `tokens.css`.
 */
function readTokens(dir: URL): string {
  return readFileSync(new URL("tokens.css", dir), "utf8");
}

/**
 * List the keys of `expected` that `table` lacks or leaves empty. Pure.
 * @param expected - The keys the table must define.
 * @param table - The table under test.
 * @returns The missing or empty keys, in `expected` order.
 */
function absentKeys(expected: readonly string[], table: Record<string, unknown>): string[] {
  return expected.filter((key) => {
    const value = table[key];
    return typeof value !== "string" || value.trim() === "";
  });
}

describe.each(DISTRIBUTIONS)("$name label table (UI-S-005)", ({ themeDir }) => {
  const labels = readLabels(themeDir);

  it("defines every required key", () => {
    expect(missingLabels(labels)).toEqual([]);
  });
  it("defines every key of the default table, none empty", () => {
    expect(absentKeys(Object.keys(defaultLabels), labels)).toEqual([]);
  });
});

describe.each(DISTRIBUTIONS)("$name design tokens (UI-S-005, UI-S-015)", ({ themeDir }) => {
  const tokensCss = readTokens(themeDir);

  it("define every required token for light and dark", () => {
    expect(missingTokens(tokensCss)).toEqual({ light: [], dark: [] });
  });
  it("declare every token the default token file declares, in light and in dark", () => {
    const defaults = tokenValues(defaultTokensCss);
    const own = tokenValues(tokensCss);
    expect(Object.keys(defaults.light).filter((token) => !(token in own.light))).toEqual([]);
    expect(Object.keys(defaults.dark).filter((token) => !(token in own.dark))).toEqual([]);
  });
  it("write every colour token as #rgb or #rrggbb in both themes", () => {
    expect(invalidColourTokens(tokensCss)).toEqual([]);
  });
  it("meet WCAG 2.2 AA for every checked pair in light and dark, with no exemption", () => {
    expect(contrastFailures(tokensCss)).toEqual([]);
    expect(contrastFailures(tokensCss, [])).toEqual([]);
  });
  it("apply the same dark values for the OS preference and for data-theme=dark", () => {
    const media = tokensCss.match(
      /@media \(prefers-color-scheme: dark\)\s*\{\s*:root:not\(\[data-theme="light"\]\)\s*\{([^}]*)\}/,
    );
    expect(media).not.toBeNull();
    const fromMedia = tokenValues(`:root[data-theme="dark"] {${media?.[1] ?? ""}}`).dark;
    expect(fromMedia).toEqual(tokenValues(tokensCss).dark);
  });
});
