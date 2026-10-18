// @vitest-environment node
/**
 * The ImageSquid distribution MUST supply every label and design token FileSquid
 * requires (ADR 031), MUST use the ImageSquid vocabulary (ImageSquid U-05), MUST use the
 * Arkrim role values for primary (IS-S-008) and MUST meet WCAG 2.2 AA in both themes.
 */
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  REQUIRED_LABEL_KEYS,
  contrastFailures,
  invalidColourTokens,
  missingLabels,
  missingTokens,
  tokenValues,
} from "@filesquid/ui/theme";
import labels from "../../src/theme/labels.en.json";

const tokensCss = readFileSync(new URL("../../src/theme/tokens.css", import.meta.url), "utf8");

describe("ImageSquid label table", () => {
  it("defines every required key", () => {
    expect(missingLabels(labels)).toEqual([]);
  });
  it("uses repo, album, gallery and exhibit for the internal nouns", () => {
    expect(labels.repository).toBe("Repo");
    expect(labels.collections).toBe("Albums");
    expect(labels.virtualCollections).toBe("Galleries");
    expect(labels.virtualExhibits).toBe("Exhibits");
    const nounKeys = REQUIRED_LABEL_KEYS.filter((key) => !key.startsWith("api"));
    const nouns = nounKeys
      .map((key) => labels[key])
      .join(" ")
      .toLowerCase();
    for (const internal of ["repository", "virtual collection", "virtual exhibit", "collection"]) {
      expect(nouns).not.toContain(internal);
    }
  });
  it("carries the screenshot wording (IS-S-004)", () => {
    expect(labels.automaticGalleriesHeading).toBe("Collections");
    expect(labels.searchPlaceholder).toBe("Search photos, collections, dates…");
  });
  it("words the theme control", () => {
    expect([labels.theme, labels.themeSystem, labels.themeLight, labels.themeDark]).toEqual([
      "Theme",
      "System",
      "Light",
      "Dark",
    ]);
  });
});

describe("ImageSquid design tokens", () => {
  it("define every required token for light and dark", () => {
    expect(missingTokens(tokensCss)).toEqual({ light: [], dark: [] });
  });
  it("use the Arkrim primary role values (IS-S-008)", () => {
    const { light, dark } = tokenValues(tokensCss);
    expect([light["--fs-primary"], light["--fs-on-primary"]]).toEqual(["#005db6", "#ffffff"]);
    expect([dark["--fs-primary"], dark["--fs-on-primary"]]).toEqual(["#a9c7ff", "#003063"]);
  });
  it("meet WCAG 2.2 AA for primary actions and body text in light and dark", () => {
    expect(contrastFailures(tokensCss)).toEqual([]);
  });
  it("write every colour token as #rgb or #rrggbb in both themes", () => {
    expect(invalidColourTokens(tokensCss)).toEqual([]);
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
