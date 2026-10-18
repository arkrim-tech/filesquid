/**
 * Design tokens: components are styled only through these CSS custom properties.
 * A distribution MUST define every token for light and dark, and its colour tokens MUST
 * meet WCAG 2.2 AA on their surfaces in both themes (ADR 031, SRS-UI-007).
 */
import { AA_TEXT_CONTRAST, contrastRatio } from "./contrast";

/** Tokens every distribution MUST define. */
export const REQUIRED_TOKENS = [
  "--fs-surface",
  "--fs-surface-container",
  "--fs-on-surface",
  "--fs-on-surface-variant",
  "--fs-outline",
  "--fs-primary",
  "--fs-on-primary",
  "--fs-primary-container",
  "--fs-on-primary-container",
  "--fs-error",
  "--fs-font-sans",
  "--fs-font-heading",
  "--fs-font-mono",
  "--fs-radius",
] as const;

/** One required token name. */
export type TokenName = (typeof REQUIRED_TOKENS)[number];

/** Tokens that do not change between themes, so the dark block need not repeat them. */
const THEME_INDEPENDENT: readonly TokenName[] = [
  "--fs-font-sans",
  "--fs-font-heading",
  "--fs-font-mono",
  "--fs-radius",
];

/**
 * Foreground and background token pairs that MUST reach {@link AA_TEXT_CONTRAST} in both
 * themes: body text and secondary text on the surfaces, primary as text or link on the
 * surface, text on a primary action, text on a selected item, and error text.
 */
export const CONTRAST_PAIRS: readonly (readonly [TokenName, TokenName])[] = [
  ["--fs-on-surface", "--fs-surface"],
  ["--fs-on-surface", "--fs-surface-container"],
  ["--fs-on-surface-variant", "--fs-surface"],
  ["--fs-on-surface-variant", "--fs-surface-container"],
  ["--fs-primary", "--fs-surface"],
  ["--fs-on-primary", "--fs-primary"],
  ["--fs-on-primary-container", "--fs-primary-container"],
  ["--fs-error", "--fs-surface"],
  ["--fs-error", "--fs-surface-container"],
];

/** A token pair below AA in one theme; `ratio` is undefined when a colour cannot be read. */
export interface ContrastFailure {
  /** Theme the pair fails in. */
  theme: "light" | "dark";
  /** Text or foreground token. */
  foreground: TokenName;
  /** Surface or background token. */
  background: TokenName;
  /** Contrast ratio rounded to two decimals, or undefined for an unreadable colour. */
  ratio: number | undefined;
}

/** Remove CSS comments so commented-out declarations do not count as defined. */
function stripComments(css: string): string {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** Return the declared custom properties and their values in one CSS block body. */
function declarations(block: string): Record<string, string> {
  const pairs = [...block.matchAll(/(--fs-[a-z0-9-]+)\s*:\s*([^;]+);/g)];
  return Object.fromEntries(pairs.map((m) => [m[1] ?? "", (m[2] ?? "").trim()]));
}

/** Extract the bodies of the light (`:root`) and dark blocks of a token stylesheet. */
function blocks(css: string): { light: string; dark: string } {
  const light = [...css.matchAll(/:root\s*\{([^}]*)\}/g)].map((m) => m[1]).join("\n");
  const dark = [...css.matchAll(/\[data-theme="dark"\]\s*\{([^}]*)\}/g)]
    .map((m) => m[1])
    .join("\n");
  return { light, dark };
}

/**
 * Read the token values a stylesheet declares for each theme.
 *
 * Light tokens live in `:root { … }`; dark tokens in `:root[data-theme="dark"] { … }`
 * (the same values are also applied for `prefers-color-scheme: dark`).
 *
 * @param css Token stylesheet text.
 * @returns Declared values by token name, per theme (dark holds only what the dark block sets).
 */
export function tokenValues(css: string): {
  light: Record<string, string>;
  dark: Record<string, string>;
} {
  const { light, dark } = blocks(stripComments(css));
  return { light: declarations(light), dark: declarations(dark) };
}

/**
 * Return required tokens missing from the light and dark blocks of a stylesheet.
 * Fonts and radius are theme-independent, so only the light block must declare them.
 *
 * @param css Token stylesheet text.
 * @returns Missing token names per theme; both lists are empty for a complete sheet.
 */
export function missingTokens(css: string): { light: string[]; dark: string[] } {
  const { light, dark } = tokenValues(css);
  return {
    light: REQUIRED_TOKENS.filter((t) => !(t in light)),
    dark: REQUIRED_TOKENS.filter((t) => !THEME_INDEPENDENT.includes(t)).filter((t) => !(t in dark)),
  };
}

/** Check every contrast pair against the values of one theme. */
function themeFailures(
  theme: ContrastFailure["theme"],
  values: Record<string, string>,
): ContrastFailure[] {
  return CONTRAST_PAIRS.flatMap(([foreground, background]) => {
    const ratio = contrastRatio(values[foreground] ?? "", values[background] ?? "");
    if (ratio !== undefined && ratio >= AA_TEXT_CONTRAST) return [];
    const rounded = ratio === undefined ? undefined : Math.round(ratio * 100) / 100;
    return [{ theme, foreground, background, ratio: rounded }];
  });
}

/**
 * Return every contrast pair below WCAG 2.2 AA in the light or dark theme of a stylesheet.
 * Colour tokens MUST be `#rgb` or `#rrggbb`; any other value is reported with an undefined ratio.
 *
 * @param css Token stylesheet text.
 * @returns The failing pairs; empty when the stylesheet meets AA in both themes.
 */
export function contrastFailures(css: string): ContrastFailure[] {
  const { light, dark } = tokenValues(css);
  return [...themeFailures("light", light), ...themeFailures("dark", { ...light, ...dark })];
}
