/**
 * DOM-free theme helpers (`@filesquid/ui/theme`): label, token and contrast contracts that
 * distributions check in their own tests without a browser, and the theme-choice rules.
 */
export {
  REQUIRED_LABEL_KEYS,
  formatLabel,
  missingLabels,
  type LabelKey,
  type LabelTable,
} from "./labels";
export {
  COLOUR_TOKENS,
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
  type ContrastExemption,
  type ContrastFailure,
  type InvalidColourToken,
  type TokenName,
  type TokenPair,
} from "./tokens";
export {
  AA_TEXT_CONTRAST,
  NON_TEXT_CONTRAST,
  contrastRatio,
  parseHexColour,
  relativeLuminance,
  type Rgb,
} from "./contrast";
export {
  THEME_CHOICES,
  THEME_STORAGE_KEY,
  parseThemeChoice,
  resolveTheme,
  type ResolvedTheme,
  type ThemeChoice,
} from "./theme-store";
