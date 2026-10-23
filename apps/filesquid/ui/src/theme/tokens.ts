/**
 * Design tokens: components are styled only through these CSS custom properties.
 * A distribution MUST define every token for light and dark, and its colour tokens MUST
 * meet WCAG 2.2 AA on their surfaces in both themes (ADR 031, SRS-UI-007), except that the
 * FileSquid default keeps the owner's listed design-guide pairs at exactly their listed values
 * (UI-S-015, Conflict Artifact CA-1 resolved "a"; {@link FILESQUID_CONTRAST_EXEMPTIONS}).
 * The focus ring and the control outline MUST reach 3:1 on every surface with no exception.
 */
import { AA_TEXT_CONTRAST, NON_TEXT_CONTRAST, contrastRatio, parseHexColour } from "./contrast";

/** Tokens every distribution MUST define (UI-S-024). */
export const REQUIRED_TOKENS = [
  "--fs-surface",
  "--fs-surface-card",
  "--fs-surface-raised",
  "--fs-surface-overlay",
  "--fs-surface-container",
  "--fs-on-surface-heading",
  "--fs-on-surface",
  "--fs-on-surface-variant",
  "--fs-on-surface-muted",
  "--fs-on-surface-placeholder",
  "--fs-outline",
  "--fs-outline-strong",
  "--fs-outline-control",
  "--fs-primary",
  "--fs-on-primary",
  "--fs-primary-container",
  "--fs-on-primary-container",
  "--fs-error",
  "--fs-focus-ring",
  "--fs-brand",
  "--fs-icon",
  "--fs-icon-muted",
  "--fs-icon-disabled",
  "--fs-badge-document",
  "--fs-on-badge-document",
  "--fs-badge-data",
  "--fs-on-badge-data",
  "--fs-badge-media",
  "--fs-on-badge-media",
  "--fs-inverse-surface",
  "--fs-on-inverse-surface",
  "--fs-scrim",
  "--fs-scrim-opacity",
  "--fs-font-sans",
  "--fs-font-heading",
  "--fs-font-mono",
  "--fs-font-size-badge",
  "--fs-font-size-eyebrow",
  "--fs-font-size-small",
  "--fs-font-size-ui",
  "--fs-font-size-state",
  "--fs-font-size-body",
  "--fs-font-size-lead",
  "--fs-font-size-title",
  "--fs-font-size-card",
  "--fs-font-size-heading",
  "--fs-font-size-page",
  "--fs-font-size-page-large",
  "--fs-font-size-display",
  "--fs-space-1",
  "--fs-space-2",
  "--fs-space-3",
  "--fs-space-4",
  "--fs-space-5",
  "--fs-space-6",
  "--fs-space-7",
  "--fs-radius-button",
  "--fs-radius-input",
  "--fs-radius-dialog",
  "--fs-radius",
  "--fs-radius-item",
  "--fs-radius-chip",
  "--fs-radius-badge",
  "--fs-radius-check",
  "--fs-radius-pill",
  "--fs-motion-duration",
] as const;

/** One required token name. */
export type TokenName = (typeof REQUIRED_TOKENS)[number];

/**
 * Tokens that do not change between themes (fonts, type sizes, spacing, radii, the scrim
 * opacity and motion), so only the light block must declare them. Every other required token
 * is a colour and MUST be declared in both themes.
 */
export const THEME_INDEPENDENT_TOKENS: readonly TokenName[] = REQUIRED_TOKENS.filter(
  (token) => /^--fs-(?:font|space|radius|motion)/.test(token) || token === "--fs-scrim-opacity",
);

/** The colour tokens: every required token that changes between themes. */
export const COLOUR_TOKENS: readonly TokenName[] = REQUIRED_TOKENS.filter(
  (token) => !THEME_INDEPENDENT_TOKENS.includes(token),
);

/** A colour token whose value is not `#rgb` or `#rrggbb`, so its contrast cannot be checked. */
export interface InvalidColourToken {
  /** Theme the value belongs to. */
  theme: "light" | "dark";
  /** The colour token. */
  token: TokenName;
  /** The declared value. */
  value: string;
}

/** A foreground and background token pair. */
export type TokenPair = readonly [TokenName, TokenName];

/**
 * Text pairs that MUST reach {@link AA_TEXT_CONTRAST} in both themes (UI-S-015): body,
 * secondary and heading text on the surfaces, primary as text or link, text on a primary
 * action, text on a selected item, error text, muted and placeholder text, the file-type badge
 * text and the toast text.
 */
export const CONTRAST_PAIRS: readonly TokenPair[] = [
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
];

/**
 * Pairs the focus ring is drawn on (UI-S-016): the `--fs-focus-ring` outline against the page,
 * the sidebar, a selected item, cards, raised rows and overlays. Each MUST reach
 * {@link NON_TEXT_CONTRAST} in both themes, with no exception.
 */
export const FOCUS_CONTRAST_PAIRS: readonly TokenPair[] = [
  ["--fs-focus-ring", "--fs-surface"],
  ["--fs-focus-ring", "--fs-surface-container"],
  ["--fs-focus-ring", "--fs-primary-container"],
  ["--fs-focus-ring", "--fs-surface-card"],
  ["--fs-focus-ring", "--fs-surface-raised"],
  ["--fs-focus-ring", "--fs-surface-overlay"],
];

/**
 * Pairs the control outline is drawn on (UI-S-024, WCAG 2.2 success criterion 1.4.11; owner
 * decision 2026-10-23 "1. darken them"): the boundary of an input, select, checkbox or radio
 * against every surface it sits on. Each MUST reach {@link NON_TEXT_CONTRAST} in both themes,
 * with no exception.
 */
export const CONTROL_CONTRAST_PAIRS: readonly TokenPair[] = [
  ["--fs-outline-control", "--fs-surface"],
  ["--fs-outline-control", "--fs-surface-card"],
  ["--fs-outline-control", "--fs-surface-container"],
  ["--fs-outline-control", "--fs-surface-raised"],
  ["--fs-outline-control", "--fs-surface-overlay"],
];

/**
 * A text pair the owner keeps below 4.5:1 at exactly the listed colours (UI-S-015). It is
 * skipped by {@link contrastFailures} only when the theme and both tokens match and both
 * declared values equal the listed values; at any other value the pair is reported.
 */
export interface ContrastExemption {
  /** Theme the exemption applies to. */
  theme: "light" | "dark";
  /** Text or foreground token. */
  foreground: TokenName;
  /** Surface or background token. */
  background: TokenName;
  /** The listed `#rrggbb` value of the foreground. */
  foregroundValue: string;
  /** The listed `#rrggbb` value of the background. */
  backgroundValue: string;
  /** The ratio measured at the listed values, rounded to two decimals. */
  ratio: number;
}

/** Build the exemptions of one foreground at one value on several backgrounds. */
function exempt(
  theme: ContrastExemption["theme"],
  foreground: TokenName,
  foregroundValue: string,
  backgrounds: readonly (readonly [TokenName, string, number])[],
): ContrastExemption[] {
  return backgrounds.map(([background, backgroundValue, ratio]) => ({
    theme,
    foreground,
    background,
    foregroundValue,
    backgroundValue,
    ratio,
  }));
}

/** The three light surfaces that are `#ffffff`. */
const WHITE: readonly (readonly [TokenName, string])[] = [
  ["--fs-surface", "#ffffff"],
  ["--fs-surface-card", "#ffffff"],
  ["--fs-surface-overlay", "#ffffff"],
];

/** `backgrounds` at one ratio. */
const at = (backgrounds: readonly (readonly [TokenName, string])[], ratio: number) =>
  backgrounds.map(([token, value]) => [token, value, ratio] as const);

/**
 * The owner's listed design-guide pairs of SRS-UI-007 and UI-S-015 (Conflict Artifact CA-1
 * resolved 2026-10-22, verbatim: "a"), for the FileSquid default's token file only. ImageSquid
 * and every other distribution pass no exemptions, so every text pair must reach 4.5:1 there.
 */
export const FILESQUID_CONTRAST_EXEMPTIONS: readonly ContrastExemption[] = [
  ...exempt("light", "--fs-on-primary", "#ffffff", [["--fs-primary", "#16ac49", 2.99]]),
  ...exempt("light", "--fs-primary", "#16ac49", at(WHITE, 2.99)),
  ...exempt("light", "--fs-on-primary-container", "#0f8a39", [
    ["--fs-primary-container", "#f0faf4", 4.18],
  ]),
  ...exempt("light", "--fs-error", "#e53935", [
    ...at(WHITE, 4.23),
    ["--fs-surface-container", "#f5f5f5", 3.88],
    ["--fs-surface-raised", "#f5f5f5", 3.88],
  ]),
  ...exempt("light", "--fs-on-surface-muted", "#777777", at(WHITE, 4.48)),
  ...exempt("light", "--fs-on-surface-placeholder", "#999999", at(WHITE, 2.85)),
  ...exempt("dark", "--fs-on-primary", "#ffffff", [["--fs-primary", "#16ac49", 2.99]]),
  ...exempt("dark", "--fs-on-surface-muted", "#6a6a6a", [
    ["--fs-surface", "#141414", 3.41],
    ["--fs-surface-card", "#1c1c1c", 3.15],
  ]),
  ...exempt("dark", "--fs-on-surface-placeholder", "#6a6a6a", [
    ["--fs-surface", "#141414", 3.41],
    ["--fs-surface-card", "#1c1c1c", 3.15],
  ]),
  ...exempt("light", "--fs-on-badge-document", "#ffffff", [
    ["--fs-badge-document", "#fd1e23", 3.9],
  ]),
  ...exempt("dark", "--fs-on-badge-document", "#ffffff", [["--fs-badge-document", "#fd1e23", 3.9]]),
  ...exempt("light", "--fs-on-badge-media", "#ffffff", [["--fs-badge-media", "#007af9", 4.08]]),
  ...exempt("dark", "--fs-on-badge-media", "#ffffff", [["--fs-badge-media", "#007af9", 4.08]]),
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
  // The last declaration of a block may omit its semicolon, as CSS allows.
  const pairs = [...block.matchAll(/(--fs-[a-z0-9-]+)\s*:\s*([^;]+?)\s*(?:;|$)/g)];
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
 * Fonts, type sizes, spacing, radii and motion are theme-independent, so only the light block
 * must declare them; every colour token must be in both blocks.
 *
 * @param css Token stylesheet text.
 * @returns Missing token names per theme; both lists are empty for a complete sheet.
 */
export function missingTokens(css: string): { light: string[]; dark: string[] } {
  const { light, dark } = tokenValues(css);
  return {
    light: REQUIRED_TOKENS.filter((t) => !(t in light)),
    dark: COLOUR_TOKENS.filter((t) => !(t in dark)),
  };
}

/**
 * Return every declared colour token, in either theme, whose value is not `#rgb` or
 * `#rrggbb`, including tokens such as `--fs-outline` that no contrast pair reads.
 *
 * @param css Token stylesheet text.
 * @returns The unreadable colour values; empty when every colour token is hex.
 */
export function invalidColourTokens(css: string): InvalidColourToken[] {
  const values = tokenValues(css);
  return (["light", "dark"] as const).flatMap((theme) =>
    COLOUR_TOKENS.filter((token) => token in values[theme])
      .filter((token) => parseHexColour(values[theme][token] ?? "") === undefined)
      .map((token) => ({ theme, token, value: values[theme][token] ?? "" })),
  );
}

/** What one theme's check needs. */
interface ThemeCheck {
  readonly theme: ContrastFailure["theme"];
  readonly values: Record<string, string>;
  readonly exemptions: readonly ContrastExemption[];
}

/** Check text pairs at 4.5:1 and focus and control pairs at 3:1 against one theme's values. */
function themeFailures(check: ThemeCheck): ContrastFailure[] {
  return [
    ...pairFailures(check, CONTRAST_PAIRS, AA_TEXT_CONTRAST),
    ...pairFailures({ ...check, exemptions: [] }, FOCUS_CONTRAST_PAIRS, NON_TEXT_CONTRAST),
    ...pairFailures({ ...check, exemptions: [] }, CONTROL_CONTRAST_PAIRS, NON_TEXT_CONTRAST),
  ];
}

/** True when a listed exemption names this pair and both declared values equal the listed ones. */
function isExempt(check: ThemeCheck, [foreground, background]: TokenPair): boolean {
  const fg = (check.values[foreground] ?? "").toLowerCase();
  const bg = (check.values[background] ?? "").toLowerCase();
  return check.exemptions.some(
    (e) =>
      e.theme === check.theme &&
      e.foreground === foreground &&
      e.background === background &&
      e.foregroundValue.toLowerCase() === fg &&
      e.backgroundValue.toLowerCase() === bg,
  );
}

/** Check each pair against `minimum`, skipping only the pairs exempt at their listed values. */
function pairFailures(
  check: ThemeCheck,
  pairs: readonly TokenPair[],
  minimum: number,
): ContrastFailure[] {
  return pairs.flatMap((pair) => {
    const [foreground, background] = pair;
    const ratio = contrastRatio(check.values[foreground] ?? "", check.values[background] ?? "");
    if (ratio !== undefined && ratio >= minimum) return [];
    if (isExempt(check, pair)) return [];
    const rounded = ratio === undefined ? undefined : Math.round(ratio * 100) / 100;
    return [{ theme: check.theme, foreground, background, ratio: rounded }];
  });
}

/**
 * Return every contrast pair below WCAG 2.2 AA in the light or dark theme of a stylesheet:
 * text pairs ({@link CONTRAST_PAIRS}) below 4.5:1, and focus-ring ({@link FOCUS_CONTRAST_PAIRS})
 * and control-outline ({@link CONTROL_CONTRAST_PAIRS}) pairs below 3:1. A text pair named in
 * `exemptions` is skipped only when both its colours equal the listed values exactly; a listed
 * pair at any other value, every unlisted pair, and every focus or control pair is reported.
 * Colour tokens MUST be `#rgb` or `#rrggbb`; any other value is reported with an undefined ratio.
 *
 * @param css Token stylesheet text.
 * @param exemptions The owner's listed pairs; the FileSquid default passes
 *   {@link FILESQUID_CONTRAST_EXEMPTIONS}, other distributions pass nothing.
 * @returns The failing pairs; empty when the stylesheet meets AA in both themes.
 */
export function contrastFailures(
  css: string,
  exemptions: readonly ContrastExemption[] = [],
): ContrastFailure[] {
  const { light, dark } = tokenValues(css);
  return [
    ...themeFailures({ theme: "light", values: light, exemptions }),
    ...themeFailures({ theme: "dark", values: { ...light, ...dark }, exemptions }),
  ];
}
