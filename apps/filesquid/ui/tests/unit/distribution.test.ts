// @vitest-environment node
/**
 * Brand slots (UI-S-015): the logo file shown for each resolved theme.
 */
import { describe, expect, it } from "vitest";
import { logoSource } from "../../src/distribution";

describe("logoSource", () => {
  const logo = { src: "/mark.svg", srcDark: "/mark-dark.svg", alt: "Mark" };

  it("uses the dark-surface variant in the dark theme", () => {
    expect(logoSource(logo, "dark")).toBe("/mark-dark.svg");
  });
  it("uses the main logo in the light theme", () => {
    expect(logoSource(logo, "light")).toBe("/mark.svg");
  });
  it("falls back to the main logo in the dark theme when there is no variant", () => {
    expect(logoSource({ src: "/mark.svg", alt: "Mark" }, "dark")).toBe("/mark.svg");
    expect(logoSource({ src: "/mark.svg", srcDark: "", alt: "Mark" }, "dark")).toBe("/mark.svg");
  });
});
