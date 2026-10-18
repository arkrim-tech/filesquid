/**
 * Public entry point of `@filesquid/ui`: API client, custom elements, theme helpers.
 * Distributions import from here and add their own views, tokens and labels.
 */
import { FsAppShell } from "./components/fs-app-shell";
import { FsBreadcrumb } from "./components/fs-breadcrumb";
import { FsHostIndicator } from "./components/fs-host-indicator";
import { FsMonitor } from "./components/fs-monitor";
import { FsNavTree } from "./components/fs-nav-tree";
import { FsStates } from "./components/fs-states";
import { FsThemeControl } from "./components/fs-theme-control";

export { FsAppShell } from "./components/fs-app-shell";
export type { ShellClient, ShellOptions } from "./components/fs-app-shell";
export { FsThemeControl, type ThemeControlOptions } from "./components/fs-theme-control";
export {
  FsHostIndicator,
  type HostClient,
  type HostIndicatorOptions,
} from "./components/fs-host-indicator";
export {
  HOST_TYPES,
  describeHost,
  parseHostDescriptor,
  type HostDescriptor,
  type HostType,
  type HostView,
  type ServiceView,
} from "./host/descriptor";
export { FsMonitor, formatGiB, processCells } from "./components/fs-monitor";
export { FsStates, RETRY_EVENT, type StatesOptions } from "./components/fs-states";
export { failureState, type ViewState, type ViewStateKind } from "./components/view-state";
export { FsNavTree, type NavTreeOptions } from "./components/fs-nav-tree";
export { FsBreadcrumb, type BreadcrumbOptions } from "./components/fs-breadcrumb";
export { treeKeyAction, type KeyItem, type TreeKeyAction } from "./components/nav-tree-keys";
export {
  EMPTY_NAVIGATION_PREFERENCES,
  KIND_LABEL,
  MAX_COLLAPSED,
  MAX_DEPTH,
  MAX_HANDLE_BYTES,
  MAX_LIBRARIES,
  MAX_PAGES,
  MAX_PINNED,
  MAX_SECTIONS,
  NAVIGATION_KINDS,
  PINNED_SECTION,
  breadcrumbItems,
  isHandle,
  isNotFound,
  moveSection,
  navigationHref,
  nodeName,
  readStoredPreferences,
  sectionOrder,
  serializeNavigationPreferences,
  unavailableName,
  withMember,
  type BreadcrumbItem,
  type NavigationChange,
  type NavigationKind,
  type NavigationNode,
  type NavigationPage,
  type NavigationPreferences,
  type NavigationSource,
  type PinnedDestination,
  type StoredPreferences,
} from "./navigation/model";
export {
  readLevel,
  readNavigation,
  readPins,
  type NavigationData,
  type NavigationLevel,
  type ResolvedPin,
} from "./navigation/loader";
export {
  NAVIGATION_STORAGE_KEY,
  NavigationStore,
  createNavigationStore,
  sharedNavigationStore,
  type NavigationEnvironment,
} from "./navigation/store";
export { MemoryNavigationSource, type FixtureNode } from "./navigation/source";
export { deviceStorage, type DeviceStorage } from "./app/device-storage";
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
export { denialLabel, reasonLabel, type DenialText } from "./components/denial";
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

/**
 * Register every FileSquid custom element once. Safe to call repeatedly. Elements that other
 * elements create (the theme control, the states element) are defined first.
 */
export function defineElements(): void {
  if (!customElements.get("fs-theme-control")) {
    customElements.define("fs-theme-control", FsThemeControl);
  }
  if (!customElements.get("fs-states")) customElements.define("fs-states", FsStates);
  if (!customElements.get("fs-nav-tree")) customElements.define("fs-nav-tree", FsNavTree);
  if (!customElements.get("fs-breadcrumb")) customElements.define("fs-breadcrumb", FsBreadcrumb);
  if (!customElements.get("fs-host-indicator")) {
    customElements.define("fs-host-indicator", FsHostIndicator);
  }
  if (!customElements.get("fs-app-shell")) customElements.define("fs-app-shell", FsAppShell);
  if (!customElements.get("fs-monitor")) customElements.define("fs-monitor", FsMonitor);
}
