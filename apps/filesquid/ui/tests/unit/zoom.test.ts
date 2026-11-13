/**
 * UI-S-031 (Task UI-F2.19): the zoom chip's "Fit · {percent}%" is the shown image's fitted width
 * over its natural width, rounded to a whole percent; without a known width there is no percent.
 */
import { describe, expect, it } from "vitest";
import { fitPercent } from "../../src/files/zoom";

describe("fitPercent", () => {
  it("is the fitted width over the natural width, rounded to a whole percent", () => {
    expect(fitPercent(640, 1684)).toBe(38);
    expect(fitPercent(640, 1280)).toBe(50);
    expect(fitPercent(426.67, 1280)).toBe(33);
    expect(fitPercent(1, 3)).toBe(33);
    expect(fitPercent(2, 3)).toBe(67);
  });

  it("is 100 for an image shown at its natural width and never more than the image allows", () => {
    expect(fitPercent(16, 16)).toBe(100);
    expect(fitPercent(1200, 600)).toBe(200);
  });

  it("is null when either width is unknown, zero, negative or not a number", () => {
    expect(fitPercent(640, 0)).toBeNull();
    expect(fitPercent(0, 1280)).toBeNull();
    expect(fitPercent(-1, 1280)).toBeNull();
    expect(fitPercent(Number.NaN, 1280)).toBeNull();
    expect(fitPercent(640, Number.POSITIVE_INFINITY)).toBeNull();
  });
});
