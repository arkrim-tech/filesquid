/**
 * Public entry point of `@filesquid/ui`: API client, custom elements, theme helpers.
 * Distributions import from here and add their own views, tokens and labels.
 */
import { FsAppShell } from "./components/fs-app-shell";
import { FsMonitor } from "./components/fs-monitor";
import { FsThemeControl } from "./components/fs-theme-control";

export { FsAppShell } from "./components/fs-app-shell";
export type { ShellClient, ShellOptions } from "./components/fs-app-shell";
export { FsThemeControl, type ThemeControlOptions } from "./components/fs-theme-control";
export { FsMonitor, formatGiB, processCells } from "./components/fs-monitor";
export type { MonitorClient, MonitorOptions, TokenStore } from "./components/fs-monitor";
export * from "./api/client";
export {
  commandEnvelope,
  commandOutcome,
  conflictOutcome,
  isCommitted,
  newOperationId,
  parseContentRange,
  rangeHeader,
  type ByteRange,
  type CommandEnvelope,
  type CommandOutcome,
  type CommandOutcomeKind,
  type ContentRange,
  type EnvelopeInput,
  type PortError,
} from "./api/commands";
export { createHostClient, DESKTOP_EVENT, LAUNCH_TOKEN_GLOBAL, launchToken } from "./api/host";
export { MONITOR_ROUTE, bindMonitorRoute, memoryTokenStore, type RouteHost } from "./app/routes";
export {
  accessibilityViolations,
  accessibleName,
  type A11yRule,
  type A11yViolation,
} from "./a11y/audit";
export { deepActiveElement, focusElement, recoverFocus, replaceKeepingFocus } from "./a11y/focus";
export {
  DEFAULT_LOCALE,
  applyLocale,
  resolveLocale,
  type Locale,
  type TextDirection,
} from "./a11y/locale";
export { REDUCED_MOTION_QUERY, prefersReducedMotion, type MotionEnvironment } from "./a11y/motion";
export { A11Y_STYLE } from "./components/a11y-style";
export { denialLabel, type DenialText } from "./components/denial";
export { logoSource, type Distribution, type DistributionLogo } from "./distribution";
export * from "./theme/index";
export {
  DARK_SCHEME_QUERY,
  ThemeStore,
  createThemeStore,
  type ThemeEnvironment,
  type ThemeMedia,
  type ThemeStorage,
} from "./theme/theme-store";

/** Register every FileSquid custom element once. Safe to call repeatedly. */
export function defineElements(): void {
  if (!customElements.get("fs-theme-control")) {
    customElements.define("fs-theme-control", FsThemeControl);
  }
  if (!customElements.get("fs-app-shell")) customElements.define("fs-app-shell", FsAppShell);
  if (!customElements.get("fs-monitor")) customElements.define("fs-monitor", FsMonitor);
}
