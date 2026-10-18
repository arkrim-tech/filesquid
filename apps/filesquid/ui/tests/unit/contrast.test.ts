// @vitest-environment node
/**
 * WCAG 2.2 contrast helpers used by every distribution's token test (SRS-UI-007, NFR-006).
 */
import { describe, expect, it } from "vitest";
import {
  AA_TEXT_CONTRAST,
  contrastRatio,
  parseHexColour,
  relativeLuminance,
} from "../../src/theme/contrast";

describe("parseHexColour", () => {
  it("reads #rrggbb and #rgb in any case", () => {
    expect(parseHexColour("#16AC49")).toEqual([0x16, 0xac, 0x49]);
    expect(parseHexColour("#fff")).toEqual([255, 255, 255]);
    expect(parseHexColour("  #0b7a32 ")).toEqual([0x0b, 0x7a, 0x32]);
  });
  it.each(["16ac49", "#12345", "#gggggg", "rgb(0, 0, 0)", "", "#1234"])("refuses %j", (value) => {
    expect(parseHexColour(value)).toBeUndefined();
  });
});

describe("relativeLuminance", () => {
  it("is 0 for black and 1 for white", () => {
    expect(relativeLuminance([0, 0, 0])).toBe(0);
    expect(relativeLuminance([255, 255, 255])).toBeCloseTo(1, 10);
  });
});

describe("contrastRatio", () => {
  it("is 21 for black on white and symmetric", () => {
    expect(contrastRatio("#000000", "#ffffff")).toBeCloseTo(21, 5);
    expect(contrastRatio("#ffffff", "#000000")).toBeCloseTo(21, 5);
  });
  it("is 1 for a colour on itself", () => {
    expect(contrastRatio("#16ac49", "#16ac49")).toBe(1);
  });
  it("matches known WCAG values", () => {
    expect(contrastRatio("#16ac49", "#ffffff")).toBeCloseTo(2.99, 2);
    expect(contrastRatio("#0b7a32", "#ffffff")).toBeCloseTo(5.46, 2);
    expect(contrastRatio("#005db6", "#ffffff")).toBeCloseTo(6.48, 2);
  });
  it("is undefined when either colour cannot be read", () => {
    expect(contrastRatio("red", "#ffffff")).toBeUndefined();
    expect(contrastRatio("#ffffff", "var(--x)")).toBeUndefined();
  });
  it("names the AA threshold for normal text", () => {
    expect(AA_TEXT_CONTRAST).toBe(4.5);
  });
});
