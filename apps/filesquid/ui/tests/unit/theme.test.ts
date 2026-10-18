// @vitest-environment node
/**
 * The default distribution supplies every required label and design token
 * (light and dark) with the official FileSquid values, and meets WCAG 2.2 AA.
 * Other distributions run the same checks in their own tests.
 */
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import labels from "../../src/theme/labels.en.json";
import { REQUIRED_LABEL_KEYS, missingLabels } from "../../src/theme/labels";
import {
  CONTRAST_PAIRS,
  REQUIRED_TOKENS,
  contrastFailures,
  missingTokens,
  tokenValues,
} from "../../src/theme/tokens";
import * as themeEntry from "../../src/theme/index";

const tokensCss = readFileSync(new URL("../../src/theme/tokens.css", import.meta.url), "utf8");

describe("default label table", () => {
  it("covers every internal name", () => {
    expect(missingLabels(labels)).toEqual([]);
  });
  it("uses the internal names as FileSquid nouns", () => {
    expect(labels.collection).toBe("Collection");
    expect(labels.virtualCollection).toBe("Virtual collection");
  });
  it("lists required keys for repository through file", () => {
    expect(REQUIRED_LABEL_KEYS).toContain("repository");
    expect(REQUIRED_LABEL_KEYS).toContain("file");
  });
  it("words the theme control", () => {
    for (const key of ["theme", "themeSystem", "themeLight", "themeDark"] as const) {
      expect(REQUIRED_LABEL_KEYS).toContain(key);
    }
    expect([labels.theme, labels.themeSystem, labels.themeLight, labels.themeDark]).toEqual([
      "Theme",
      "System",
      "Light",
      "Dark",
    ]);
  });
});

describe("default design tokens", () => {
  it("defines every required token for light and dark", () => {
    expect(missingTokens(tokensCss)).toEqual({ light: [], dark: [] });
  });
  it("uses the --fs- prefix and requires a heading font", () => {
    expect(REQUIRED_TOKENS.every((token) => token.startsWith("--fs-"))).toBe(true);
    expect(REQUIRED_TOKENS).toContain("--fs-font-heading");
  });
  it("reports tokens a distribution forgot", () => {
    expect(missingTokens(":root { --fs-surface: #fff; }").light).toContain("--fs-primary");
    expect(REQUIRED_TOKENS.length).toBeGreaterThan(5);
  });
  it("reports a colour token missing from the dark theme only", () => {
    const css = tokensCss.replace(/(:root\[data-theme="dark"\]\s*\{[^}]*?)--fs-error:[^;]*;/, "$1");
    expect(missingTokens(css)).toEqual({ light: [], dark: ["--fs-error"] });
  });
  it("does not count commented-out declarations", () => {
    const css = tokensCss.replace(/(--fs-primary\s*:[^;]*;)/g, "/* $1 */");
    expect(missingTokens(css).light).toContain("--fs-primary");
    expect(missingTokens(css).dark).toContain("--fs-primary");
  });
  it("uses the official FileSquid values, adjusted for AA", () => {
    const { light, dark } = tokenValues(tokensCss);
    expect(light).toMatchObject({
      "--fs-surface": "#ffffff",
      "--fs-surface-container": "#f5f5f5",
      "--fs-on-surface": "#111111",
      "--fs-on-surface-variant": "#555555",
      "--fs-outline": "#e0e0e0",
      "--fs-primary": "#0b7a32",
      "--fs-on-primary": "#ffffff",
      "--fs-primary-container": "#f0faf4",
      "--fs-on-primary-container": "#0b7a32",
      "--fs-error": "#c62828",
      "--fs-radius": "8px",
    });
    expect(light["--fs-font-heading"]).toMatch(/^Georgia,/);
    expect(light["--fs-font-sans"]).toMatch(/^-apple-system,/);
    expect(light["--fs-font-mono"]).toMatch(/^"SF Mono",/);
    expect(dark).toMatchObject({
      "--fs-surface": "#141414",
      "--fs-surface-container": "#1c1c1c",
      "--fs-on-surface": "#e0e0e0",
      "--fs-on-surface-variant": "#a0a0a0",
      "--fs-outline": "#2a2a2a",
      "--fs-primary": "#16ac49",
      "--fs-on-primary": "#0d2b18",
      "--fs-primary-container": "#0d2b18",
      "--fs-on-primary-container": "#4aca74",
      "--fs-error": "#ef5350",
    });
  });
  it("applies the same dark values for the OS preference and for data-theme=dark", () => {
    const media = tokensCss.match(
      /@media \(prefers-color-scheme: dark\)\s*\{\s*:root:not\(\[data-theme="light"\]\)\s*\{([^}]*)\}/,
    );
    expect(media).not.toBeNull();
    const fromMedia = tokenValues(`:root[data-theme="dark"] {${media?.[1] ?? ""}}`).dark;
    expect(fromMedia).toEqual(tokenValues(tokensCss).dark);
  });
});

describe("WCAG 2.2 AA contrast", () => {
  it("checks primary actions and body text on their surfaces", () => {
    expect(CONTRAST_PAIRS).toEqual(
      expect.arrayContaining([
        ["--fs-on-surface", "--fs-surface"],
        ["--fs-on-primary", "--fs-primary"],
        ["--fs-primary", "--fs-surface"],
      ]),
    );
  });
  it("passes for the default tokens in light and dark", () => {
    expect(contrastFailures(tokensCss)).toEqual([]);
  });
  it("reports the official brand green with white text (2.99:1) as a failure", () => {
    const css = tokensCss.replace(/--fs-primary: #0b7a32;/, "--fs-primary: #16ac49;");
    const failures = contrastFailures(css);
    expect(failures).toContainEqual({
      theme: "light",
      foreground: "--fs-on-primary",
      background: "--fs-primary",
      ratio: 2.99,
    });
  });
  it("reports a colour it cannot read instead of passing it", () => {
    const css = tokensCss.replace(/--fs-on-surface: #e0e0e0;/g, "--fs-on-surface: white;");
    expect(contrastFailures(css)).toContainEqual({
      theme: "dark",
      foreground: "--fs-on-surface",
      background: "--fs-surface",
      ratio: undefined,
    });
  });
  it("is exported for distributions from @filesquid/ui/theme", () => {
    expect(themeEntry.contrastFailures).toBe(contrastFailures);
    expect(typeof themeEntry.contrastRatio).toBe("function");
    expect(typeof themeEntry.tokenValues).toBe("function");
  });
});
