/**
 * Public entry point of `@filesquid/ui`: API client, custom elements, theme helpers.
 * Distributions import from here and add their own views, tokens and labels.
 */
import { FsAppShell } from "./components/fs-app-shell";
import { FsMonitor } from "./components/fs-monitor";

export { FsAppShell } from "./components/fs-app-shell";
export type { ShellClient } from "./components/fs-app-shell";
export { FsMonitor, formatGiB, processCells } from "./components/fs-monitor";
export type { MonitorClient, MonitorOptions, TokenStore } from "./components/fs-monitor";
export * from "./api/client";
export { createHostClient, DESKTOP_EVENT, LAUNCH_TOKEN_GLOBAL, launchToken } from "./api/host";
export { MONITOR_ROUTE, bindMonitorRoute, memoryTokenStore } from "./app/routes";
export { denialLabel, type DenialText } from "./components/denial";
export type { Distribution } from "./distribution";
export { REQUIRED_LABEL_KEYS, missingLabels, type LabelKey, type LabelTable } from "./theme/labels";
export { REQUIRED_TOKENS, missingTokens } from "./theme/tokens";

/** Register every FileSquid custom element once. Safe to call repeatedly. */
export function defineElements(): void {
  if (!customElements.get("fs-app-shell")) customElements.define("fs-app-shell", FsAppShell);
  if (!customElements.get("fs-monitor")) customElements.define("fs-monitor", FsMonitor);
}
