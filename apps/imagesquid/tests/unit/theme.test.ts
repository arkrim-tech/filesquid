// @vitest-environment node
/**
 * The ImageSquid distribution MUST use the ImageSquid vocabulary (ImageSquid U-05), MUST use
 * the Arkrim role values for primary (IS-S-008) and MUST keep the 8 px spacing scale.
 *
 * That it supplies every label and design token FileSquid requires (UI-S-005, ADR 031) and
 * meets WCAG 2.2 AA in both themes with no contrast exemption (UI-S-015) is checked by
 * `apps/filesquid/ui/tests/unit/distribution-tables.test.ts`, which reads this package's
 * label table and token file. That test runs in the pre-commit hook during the MVP, when the
 * hook runs no ImageSquid suite (owner decision 2026-10-23); this file runs by hand with
 * `npm test`.
 */
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { REQUIRED_LABEL_KEYS, tokenValues } from "@filesquid/ui/theme";
import labels from "../../src/theme/labels.en.json";

const tokensCss = readFileSync(new URL("../../src/theme/tokens.css", import.meta.url), "utf8");

describe("ImageSquid label table", () => {
  it("uses repo, album, gallery and exhibit for the internal nouns", () => {
    expect(labels.repository).toBe("Repo");
    expect(labels.collections).toBe("Albums");
    expect(labels.virtualCollections).toBe("Galleries");
    expect(labels.virtualExhibits).toBe("Exhibits");
    // The search placeholder keeps the screenshot wording of IS-S-004, whose "collections" are
    // ImageSquid's automatic Collections heading, not the internal noun.
    const nounKeys = REQUIRED_LABEL_KEYS.filter(
      (key) => !key.startsWith("api") && key !== "searchPlaceholder",
    );
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
  it("words navigation and view states without the internal nouns (UI-F2.1a)", () => {
    expect(labels.folders).toBe("Folders");
    expect(labels.navigationPinned).toBe("Pinned");
    for (const template of [
      labels.navigationMoveUp,
      labels.navigationMoveDown,
      labels.navigationPinItem,
    ]) {
      expect(template).toContain("{name}");
    }
    expect(labels.stateRetry).toBe("Try again");
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
  it("use the Arkrim primary role values (IS-S-008)", () => {
    const { light, dark } = tokenValues(tokensCss);
    expect([light["--fs-primary"], light["--fs-on-primary"]]).toEqual(["#005db6", "#ffffff"]);
    expect([dark["--fs-primary"], dark["--fs-on-primary"]]).toEqual(["#a9c7ff", "#003063"]);
  });
  it("use the 8 px spacing scale of UI-S-024", () => {
    const { light } = tokenValues(tokensCss);
    const scale = [1, 2, 3, 4, 5, 6, 7].map((step) => light[`--fs-space-${step}`]);
    expect(scale).toEqual(["4px", "8px", "16px", "24px", "40px", "64px", "96px"]);
  });
});
