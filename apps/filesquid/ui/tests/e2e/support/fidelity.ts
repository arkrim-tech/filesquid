/**
 * Shared helpers of the mockup fidelity proofs (UI-S-024, UI-S-013): the light and dark hex
 * values of UI-S-024, the computed look of an element, a bounding-box check within 2 px of a
 * mockup's region, and the width this browser renders for an authored border. Pure page reads.
 */
import { expect, type Locator, type Page } from "@playwright/test";

/** The light and dark hex values of UI-S-024 that the tests read back. */
export const COLOURS = {
  light: {
    surface: "#ffffff",
    card: "#ffffff",
    raised: "#f5f5f5",
    overlay: "#ffffff",
    outline: "#e0e0e0",
    strong: "#cccccc",
    control: "#8e8e8e",
    heading: "#000000",
    body: "#111111",
    variant: "#555555",
    muted: "#777777",
    placeholder: "#999999",
    tint: "#f0faf4",
    active: "#0f8a39",
    primary: "#16ac49",
    onPrimary: "#ffffff",
    error: "#e53935",
    focus: "#0f8a39",
    icon: "#111111",
  },
  dark: {
    surface: "#141414",
    card: "#1c1c1c",
    raised: "#242424",
    overlay: "#2c2c2c",
    outline: "#2a2a2a",
    strong: "#333333",
    control: "#757575",
    heading: "#f5f5f5",
    body: "#e0e0e0",
    variant: "#a0a0a0",
    muted: "#6a6a6a",
    placeholder: "#6a6a6a",
    tint: "#0d2b18",
    active: "#4aca74",
    primary: "#16ac49",
    onPrimary: "#ffffff",
    error: "#ef5350",
    focus: "#16ac49",
    icon: "#a0a0a0",
  },
} as const;

/** The computed styles a test compares. */
export interface Look {
  family: string;
  size: string;
  weight: string;
  colour: string;
  background: string;
  borderWidth: string;
  borderColour: string;
  radius: string;
  transform: string;
  spacing: string;
}

/** Read the computed look of the first element a locator matches. */
export function look(locator: Locator): Promise<Look> {
  return locator.first().evaluate((el) => {
    const css = getComputedStyle(el);
    return {
      family: css.fontFamily,
      size: css.fontSize,
      weight: css.fontWeight,
      colour: css.color,
      background: css.backgroundColor,
      borderWidth: css.borderTopWidth,
      borderColour: css.borderTopColor,
      radius: css.borderTopLeftRadius,
      transform: css.textTransform,
      spacing: css.letterSpacing,
    };
  });
}

/** Assert a box within 2 px of the mockup's x, y, width and height (each optional). */
export async function expectBox(
  locator: Locator,
  expected: { x?: number; y?: number; width?: number; height?: number },
): Promise<void> {
  const box = await locator.first().boundingBox();
  expect(box, "the region is rendered").not.toBeNull();
  for (const key of ["x", "y", "width", "height"] as const) {
    const want = expected[key];
    if (want === undefined) continue;
    const got = box?.[key] ?? Number.NaN;
    expect(Math.abs(got - want), `${key} ${got} is within 2 px of ${want}`).toBeLessThanOrEqual(2);
  }
}

/**
 * The computed width this browser gives a border authored at `width`. The guide's 1.5 px
 * borders are snapped by some Chromium builds (to 1px here), so a 1.5 px border is compared with
 * the same browser's rendering of 1.5 px rather than a literal; the component tests assert the
 * authored `1.5px` in each stylesheet.
 */
export function renderedBorder(page: Page, width: string): Promise<string> {
  return page.evaluate((value) => {
    const probe = document.createElement("div");
    probe.style.border = `${value} solid`;
    document.body.append(probe);
    const computed = getComputedStyle(probe).borderTopWidth;
    probe.remove();
    return computed;
  }, width);
}
