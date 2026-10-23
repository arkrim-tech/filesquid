// @vitest-environment node
/**
 * The default distribution supplies every required label and design token (light and dark)
 * with the FileSquid Design Guide values of UI-S-024, and meets WCAG 2.2 AA except for the
 * owner's listed design-guide pairs (SRS-UI-007, UI-S-015, Conflict Artifact CA-1 resolved
 * "a"): an exempt pair passes only at exactly its listed colours, every other text pair must
 * reach 4.5:1, and the focus ring and control outlines must reach 3:1 with no exception.
 * Other distributions run the same checks in their own tests, without the exemptions.
 */
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import labels from "../../src/theme/labels.en.json";
import { REQUIRED_LABEL_KEYS, missingLabels } from "../../src/theme/labels";
import {
  CONTRAST_PAIRS,
  CONTROL_CONTRAST_PAIRS,
  FILESQUID_CONTRAST_EXEMPTIONS,
  FOCUS_CONTRAST_PAIRS,
  REQUIRED_TOKENS,
  THEME_INDEPENDENT_TOKENS,
  contrastFailures,
  invalidColourTokens,
  missingTokens,
  tokenValues,
} from "../../src/theme/tokens";
import { NON_TEXT_CONTRAST } from "../../src/theme/contrast";
import * as themeEntry from "../../src/theme/index";

const tokensCss = readFileSync(new URL("../../src/theme/tokens.css", import.meta.url), "utf8");

/** Replace the declaration of `token` in the light block only (the first in the sheet). */
function withLight(css: string, token: string, value: string): string {
  return css.replace(new RegExp(`${token}\\s*:\\s*[^;]+;`), `${token}: ${value};`);
}

/** Replace every declaration of `token` in both dark blocks. */
function withDark(css: string, token: string, value: string): string {
  const pattern = new RegExp(`(${token}\\s*:\\s*)([^;]+);`, "g");
  let index = 0;
  return css.replace(pattern, (whole, head: string) => {
    index += 1;
    return index === 1 ? whole : `${head}${value};`;
  });
}

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
  it("words the theme control and its Settings help (UI-S-033)", () => {
    for (const key of ["theme", "themeSystem", "themeLight", "themeDark", "themeHelp"] as const) {
      expect(REQUIRED_LABEL_KEYS).toContain(key);
    }
    expect([labels.theme, labels.themeSystem, labels.themeLight, labels.themeDark]).toEqual([
      "Theme",
      "System",
      "Light",
      "Dark",
    ]);
    expect(labels.themeHelp).toBe(
      "Follows the macOS appearance. Your choice is kept across launches.",
    );
  });
  it("carries the approved v3 strings with the wording of UI-S-025 to UI-S-033 (owner answer 13)", () => {
    expect(labels).toMatchObject({
      fileNew: "New",
      fileFilterView: "Filter this view",
      fileRecent: "Recent",
      navigationRecentlyDeleted: "Recently deleted",
      fileStorageUsed: "Storage used",
      fileStorageCaptionHistory: "incl. {history} history",
      fileStorageFree: "{free} free on disk",
      logs: "Logs",
      searchPlaceholder: "Search files and folders",
      fileCommitted: "All changes committed to OCFL",
      fileLibraryReady: "Library ready",
      settingsApiStatus: "API: {status} · contract {version}",
      settingsAppearance: "Appearance",
      settingsAbout: "About {product}",
      hostDesktopNoTelemetry:
        "FileSquid sends nothing off this computer. No account, no telemetry.",
      monitorOwnerOnly: "Only you can open this on this device.",
      libraryWelcomeTitle: "Your files. Private.",
      libraryReadOnlyReason: "Their latest version has no version inventory (W010).",
      fileNoPreviewKeep: "FileSquid keeps the file and every version.",
      fileNoPreviewOpenElsewhere: "Open it in the app that made it.",
      fileUploadFiles: "Upload files…",
      fileUploadFolder: "Upload folder…",
    });
  });
  it("words import and export as Upload and Download, keeping the keys (owner answer 8)", () => {
    expect(labels.monitorExport).toBe("Download JSON Lines");
    expect(labels.fileUpload).toBe("Upload files");
    expect(REQUIRED_LABEL_KEYS).toContain("monitorExport");
    expect(REQUIRED_LABEL_KEYS).not.toContain("fileNoPreviewHelp");
    expect(Object.keys(labels)).not.toContain("fileNoPreviewHelp");
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
  it("reads a last declaration without a closing semicolon", () => {
    const css =
      ':root { --fs-surface: #fff; --fs-outline: #e0e0e0 }\n:root[data-theme="dark"] { --fs-outline: #2a2a2a\n}';
    const { light, dark } = tokenValues(css);
    expect(light).toEqual({ "--fs-surface": "#fff", "--fs-outline": "#e0e0e0" });
    expect(dark).toEqual({ "--fs-outline": "#2a2a2a" });
  });
  it("requires the UI-S-024 type-size, spacing and radius tokens, theme-independent", () => {
    const sizes = {
      "--fs-font-size-badge": "0.625rem",
      "--fs-font-size-eyebrow": "0.6875rem",
      "--fs-font-size-small": "0.8125rem",
      "--fs-font-size-ui": "0.875rem",
      "--fs-font-size-state": "0.9375rem",
      "--fs-font-size-body": "1rem",
      "--fs-font-size-lead": "1.0625rem",
      "--fs-font-size-title": "1.25rem",
      "--fs-font-size-card": "1.3125rem",
      "--fs-font-size-heading": "1.375rem",
      "--fs-font-size-page": "1.875rem",
      "--fs-font-size-page-large": "2.125rem",
      "--fs-font-size-display": "3.5rem",
      "--fs-space-1": "4px",
      "--fs-space-2": "8px",
      "--fs-space-3": "16px",
      "--fs-space-4": "24px",
      "--fs-space-5": "40px",
      "--fs-space-6": "64px",
      "--fs-space-7": "96px",
      "--fs-radius-button": "7px",
      "--fs-radius-input": "6px",
      "--fs-radius-dialog": "10px",
      "--fs-radius": "8px",
      "--fs-radius-item": "5px",
      "--fs-radius-chip": "4px",
      "--fs-radius-badge": "3px",
      "--fs-radius-check": "3.5px",
      "--fs-radius-pill": "999px",
      "--fs-scrim-opacity": "0.32",
    };
    for (const token of Object.keys(sizes)) {
      expect(REQUIRED_TOKENS).toContain(token);
      expect(THEME_INDEPENDENT_TOKENS).toContain(token);
    }
    expect(tokenValues(tokensCss).light).toMatchObject(sizes);
  });
  it("does not count commented-out declarations", () => {
    const css = tokensCss.replace(/(--fs-primary\s*:[^;]*;)/g, "/* $1 */");
    expect(missingTokens(css).light).toContain("--fs-primary");
    expect(missingTokens(css).dark).toContain("--fs-primary");
  });
  it("uses the FileSquid Design Guide values of UI-S-024 (owner answers 14 and 15)", () => {
    const { light, dark } = tokenValues(tokensCss);
    expect(light).toMatchObject({
      "--fs-surface": "#ffffff",
      "--fs-surface-card": "#ffffff",
      "--fs-surface-raised": "#f5f5f5",
      "--fs-surface-overlay": "#ffffff",
      "--fs-surface-container": "#f5f5f5",
      "--fs-outline": "#e0e0e0",
      "--fs-outline-strong": "#cccccc",
      "--fs-outline-control": "#8e8e8e",
      "--fs-on-surface-heading": "#000000",
      "--fs-on-surface": "#111111",
      "--fs-on-surface-variant": "#555555",
      "--fs-on-surface-muted": "#777777",
      "--fs-on-surface-placeholder": "#999999",
      "--fs-primary": "#16ac49",
      "--fs-on-primary": "#ffffff",
      "--fs-primary-container": "#f0faf4",
      "--fs-on-primary-container": "#0f8a39",
      "--fs-error": "#e53935",
      "--fs-focus-ring": "#0f8a39",
      "--fs-brand": "#16ac49",
      "--fs-icon": "#111111",
      "--fs-icon-muted": "#777777",
      "--fs-icon-disabled": "#cccccc",
      "--fs-badge-document": "#fd1e23",
      "--fs-on-badge-document": "#ffffff",
      "--fs-badge-data": "#fcd805",
      "--fs-on-badge-data": "#111111",
      "--fs-badge-media": "#007af9",
      "--fs-on-badge-media": "#ffffff",
      "--fs-inverse-surface": "#111111",
      "--fs-on-inverse-surface": "#ffffff",
      "--fs-scrim": "#000000",
    });
    expect(light["--fs-font-heading"]).toMatch(/^Georgia,/);
    expect(light["--fs-font-sans"]).toMatch(/^-apple-system,/);
    expect(light["--fs-font-mono"]).toMatch(/^"SF Mono",/);
    expect(dark).toMatchObject({
      "--fs-surface": "#141414",
      "--fs-surface-card": "#1c1c1c",
      "--fs-surface-raised": "#242424",
      "--fs-surface-overlay": "#2c2c2c",
      "--fs-surface-container": "#1c1c1c",
      "--fs-outline": "#2a2a2a",
      "--fs-outline-strong": "#333333",
      "--fs-outline-control": "#757575",
      "--fs-on-surface-heading": "#f5f5f5",
      "--fs-on-surface": "#e0e0e0",
      "--fs-on-surface-variant": "#a0a0a0",
      "--fs-on-surface-muted": "#6a6a6a",
      "--fs-on-surface-placeholder": "#6a6a6a",
      "--fs-primary": "#16ac49",
      "--fs-on-primary": "#ffffff",
      "--fs-primary-container": "#0d2b18",
      "--fs-on-primary-container": "#4aca74",
      "--fs-error": "#ef5350",
      "--fs-focus-ring": "#16ac49",
      "--fs-brand": "#16ac49",
      "--fs-icon": "#a0a0a0",
      "--fs-icon-muted": "#6a6a6a",
      "--fs-icon-disabled": "#3a3a3a",
      "--fs-badge-document": "#fd1e23",
      "--fs-badge-data": "#fcd805",
      "--fs-badge-media": "#007af9",
      "--fs-inverse-surface": "#2c2c2c",
      "--fs-on-inverse-surface": "#f5f5f5",
      "--fs-scrim": "#000000",
    });
  });
  it("has no warning token: warnings use the error colour (owner answer 14)", () => {
    expect(REQUIRED_TOKENS.some((token) => token.includes("warning"))).toBe(false);
    expect(tokensCss).not.toMatch(/--fs-warning/);
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

describe("WCAG 2.2 AA contrast (UI-S-015)", () => {
  it("checks the UI-S-015 text pairs", () => {
    expect(CONTRAST_PAIRS).toEqual([
      ["--fs-on-surface", "--fs-surface"],
      ["--fs-on-surface", "--fs-surface-container"],
      ["--fs-on-surface-variant", "--fs-surface"],
      ["--fs-on-surface-variant", "--fs-surface-container"],
      ["--fs-on-surface-heading", "--fs-surface"],
      ["--fs-on-surface-heading", "--fs-surface-card"],
      ["--fs-primary", "--fs-surface"],
      ["--fs-primary", "--fs-surface-card"],
      ["--fs-primary", "--fs-surface-overlay"],
      ["--fs-on-primary", "--fs-primary"],
      ["--fs-on-primary-container", "--fs-primary-container"],
      ["--fs-error", "--fs-surface"],
      ["--fs-error", "--fs-surface-container"],
      ["--fs-on-surface-muted", "--fs-surface"],
      ["--fs-on-surface-muted", "--fs-surface-card"],
      ["--fs-on-surface-placeholder", "--fs-surface"],
      ["--fs-on-surface-placeholder", "--fs-surface-card"],
      ["--fs-on-badge-document", "--fs-badge-document"],
      ["--fs-on-badge-data", "--fs-badge-data"],
      ["--fs-on-badge-media", "--fs-badge-media"],
      ["--fs-on-inverse-surface", "--fs-inverse-surface"],
    ]);
  });
  it("passes for the default tokens with the owner's exemptions, in light and dark", () => {
    expect(contrastFailures(tokensCss, FILESQUID_CONTRAST_EXEMPTIONS)).toEqual([]);
  });
  it("reports the exempt pairs when no exemption is given, as another distribution's test does", () => {
    const failures = contrastFailures(tokensCss).map(
      (f) => `${f.theme} ${f.foreground} on ${f.background} ${f.ratio}`,
    );
    expect(failures).toContain("light --fs-on-primary on --fs-primary 2.99");
    expect(failures).toContain("light --fs-primary on --fs-surface 2.99");
    expect(failures).toContain("light --fs-on-primary-container on --fs-primary-container 4.18");
    expect(failures).toContain("light --fs-error on --fs-surface 4.23");
    expect(failures).toContain("light --fs-error on --fs-surface-container 3.88");
    expect(failures).toContain("light --fs-on-surface-muted on --fs-surface 4.48");
    expect(failures).toContain("light --fs-on-surface-placeholder on --fs-surface 2.85");
    expect(failures).toContain("dark --fs-on-primary on --fs-primary 2.99");
    expect(failures).toContain("dark --fs-on-surface-muted on --fs-surface 3.41");
    expect(failures).toContain("dark --fs-on-surface-placeholder on --fs-surface-card 3.15");
    expect(failures).toContain("light --fs-on-badge-document on --fs-badge-document 3.9");
    expect(failures).toContain("dark --fs-on-badge-media on --fs-badge-media 4.08");
    expect(failures.some((f) => f.includes("--fs-focus-ring"))).toBe(false);
    expect(failures.some((f) => f.includes("--fs-outline-control"))).toBe(false);
  });
  it("lists the exempt pairs as data: theme, tokens, both exact values and the measured ratio", () => {
    expect(FILESQUID_CONTRAST_EXEMPTIONS).toContainEqual({
      theme: "light",
      foreground: "--fs-on-primary",
      background: "--fs-primary",
      foregroundValue: "#ffffff",
      backgroundValue: "#16ac49",
      ratio: 2.99,
    });
    expect(FILESQUID_CONTRAST_EXEMPTIONS).toContainEqual({
      theme: "dark",
      foreground: "--fs-on-surface-muted",
      background: "--fs-surface-card",
      foregroundValue: "#6a6a6a",
      backgroundValue: "#1c1c1c",
      ratio: 3.15,
    });
    for (const exemption of FILESQUID_CONTRAST_EXEMPTIONS) {
      expect(exemption.ratio).toBeLessThan(4.5);
      expect(exemption.foreground).not.toBe("--fs-focus-ring");
      expect(exemption.foreground).not.toBe("--fs-outline-control");
    }
  });
  it("accepts an exempt pair only at exactly its listed values: one step off fails", () => {
    const off = withLight(tokensCss, "--fs-primary", "#17ad4a");
    const failures = contrastFailures(off, FILESQUID_CONTRAST_EXEMPTIONS);
    expect(failures).toContainEqual({
      theme: "light",
      foreground: "--fs-on-primary",
      background: "--fs-primary",
      ratio: 2.95,
    });
    expect(failures).toContainEqual(
      expect.objectContaining({
        theme: "light",
        foreground: "--fs-primary",
        background: "--fs-surface",
      }),
    );
  });
  it("fails an unlisted pair below 4.5:1 even for the default", () => {
    const css = withLight(tokensCss, "--fs-on-surface-variant", "#999999");
    expect(contrastFailures(css, FILESQUID_CONTRAST_EXEMPTIONS)).toContainEqual({
      theme: "light",
      foreground: "--fs-on-surface-variant",
      background: "--fs-surface",
      ratio: 2.85,
    });
  });
  it("reports a distribution primary below AA that is not an exempt value", () => {
    const css = withLight(tokensCss, "--fs-primary", "#5cd685");
    expect(contrastFailures(css, FILESQUID_CONTRAST_EXEMPTIONS)).toContainEqual({
      theme: "light",
      foreground: "--fs-on-primary",
      background: "--fs-primary",
      ratio: 1.84,
    });
  });
  it("reports a colour it cannot read instead of passing it", () => {
    const css = withDark(tokensCss, "--fs-on-surface", "white");
    expect(contrastFailures(css, FILESQUID_CONTRAST_EXEMPTIONS)).toContainEqual({
      theme: "dark",
      foreground: "--fs-on-surface",
      background: "--fs-surface",
      ratio: undefined,
    });
  });
  it("finds no unreadable colour token in the default sheet", () => {
    expect(invalidColourTokens(tokensCss)).toEqual([]);
  });
  it("reports a colour token outside the contrast pairs that is not #rgb or #rrggbb", () => {
    const css = withDark(tokensCss, "--fs-outline", "rgb(0 0 0)");
    expect(contrastFailures(css, FILESQUID_CONTRAST_EXEMPTIONS)).toEqual([]);
    expect(invalidColourTokens(css)).toEqual([
      { theme: "dark", token: "--fs-outline", value: "rgb(0 0 0)" },
    ]);
  });
  it("is exported for distributions from @filesquid/ui/theme", () => {
    expect(themeEntry.invalidColourTokens).toBe(invalidColourTokens);
    expect(themeEntry.contrastFailures).toBe(contrastFailures);
    expect(themeEntry.FILESQUID_CONTRAST_EXEMPTIONS).toBe(FILESQUID_CONTRAST_EXEMPTIONS);
    expect(themeEntry.CONTROL_CONTRAST_PAIRS).toBe(CONTROL_CONTRAST_PAIRS);
    expect(typeof themeEntry.contrastRatio).toBe("function");
    expect(typeof themeEntry.tokenValues).toBe("function");
  });
});

describe("UI-S-016 accessibility tokens and labels", () => {
  it("words the skip control and the main navigation from the label table", () => {
    expect(REQUIRED_LABEL_KEYS).toContain("skipToContent");
    expect(REQUIRED_LABEL_KEYS).toContain("navigationMain");
    expect(labels.skipToContent).toBe("Skip to main content");
    expect(labels.navigationMain).toBe("Main navigation");
  });
  it("requires a theme-independent motion duration of 150ms", () => {
    expect(REQUIRED_TOKENS).toContain("--fs-motion-duration");
    expect(THEME_INDEPENDENT_TOKENS).toContain("--fs-motion-duration");
    expect(tokenValues(tokensCss).light["--fs-motion-duration"]).toBe("150ms");
    expect(missingTokens(tokensCss)).toEqual({ light: [], dark: [] });
  });
  it("checks the focus ring token at 3:1 on every surface it is drawn on", () => {
    expect(NON_TEXT_CONTRAST).toBe(3);
    expect(FOCUS_CONTRAST_PAIRS).toEqual([
      ["--fs-focus-ring", "--fs-surface"],
      ["--fs-focus-ring", "--fs-surface-container"],
      ["--fs-focus-ring", "--fs-primary-container"],
      ["--fs-focus-ring", "--fs-surface-card"],
      ["--fs-focus-ring", "--fs-surface-raised"],
      ["--fs-focus-ring", "--fs-surface-overlay"],
    ]);
  });
  it("checks the control outline at 3:1 on every surface, with no exception (owner decision 1. darken them)", () => {
    expect(CONTROL_CONTRAST_PAIRS).toEqual([
      ["--fs-outline-control", "--fs-surface"],
      ["--fs-outline-control", "--fs-surface-card"],
      ["--fs-outline-control", "--fs-surface-container"],
      ["--fs-outline-control", "--fs-surface-raised"],
      ["--fs-outline-control", "--fs-surface-overlay"],
    ]);
    const css = withLight(tokensCss, "--fs-outline-control", "#cccccc");
    expect(contrastFailures(css, FILESQUID_CONTRAST_EXEMPTIONS)).toContainEqual({
      theme: "light",
      foreground: "--fs-outline-control",
      background: "--fs-surface",
      ratio: 1.61,
    });
  });
  it("fails a focus ring below 3:1 with no exception: the guide's green on white is 2.99", () => {
    const css = withLight(tokensCss, "--fs-focus-ring", "#16ac49");
    expect(contrastFailures(css, FILESQUID_CONTRAST_EXEMPTIONS)).toContainEqual({
      theme: "light",
      foreground: "--fs-focus-ring",
      background: "--fs-surface",
      ratio: 2.99,
    });
  });
  it("reports a focus ring below 3:1 on the sidebar, in the theme it fails in", () => {
    const css = withDark(tokensCss, "--fs-surface-container", "#18a04a");
    expect(contrastFailures(css, FILESQUID_CONTRAST_EXEMPTIONS)).toContainEqual({
      theme: "dark",
      foreground: "--fs-focus-ring",
      background: "--fs-surface-container",
      ratio: 1.14,
    });
  });
  it("accepts a focus pair between 3:1 and 4.5:1, which text pairs would reject", () => {
    // #949494 on #ffffff is 3.03:1.
    const css = withLight(tokensCss, "--fs-focus-ring", "#949494");
    const failures = contrastFailures(css, FILESQUID_CONTRAST_EXEMPTIONS).filter(
      (f) => f.theme === "light",
    );
    expect(failures.map((f) => `${f.foreground} on ${f.background}`)).not.toContain(
      "--fs-focus-ring on --fs-surface",
    );
  });
  it("exports the focus pairs for distributions", () => {
    expect(themeEntry.FOCUS_CONTRAST_PAIRS).toBe(FOCUS_CONTRAST_PAIRS);
    expect(themeEntry.NON_TEXT_CONTRAST).toBe(3);
  });
});
