/**
 * ImageSquid entry point: FileSquid tokens first, ImageSquid tokens on top, the saved theme
 * applied, then the shared shell configured as ImageSquid and connected to whichever FileSquid
 * host serves it. The FileSquid Monitor is reachable at `#/monitor` (interim entry until the
 * More menu), `#/settings` shows the shared Settings page (Appearance, Privacy, About), and
 * `#/library` creates, opens or recovers a library (`#/library/<id>` shows it, and the
 * Monitor's W010 note links to its repair offer). The desktop host supplies its per-launch
 * token; on the server host the Monitor asks for the administrator token, kept in memory only.
 */
import "@filesquid/ui/theme/tokens.css";
import "./theme/tokens.css";
import {
  CONTRACT_VERSION,
  ClientLibrarySource,
  LibraryRegistry,
  applyLocale,
  bindLibraryRoute,
  bindMonitorRoute,
  bindSettingsRoute,
  createHostClient,
  createThemeStore,
  defineElements,
  detectHost,
  libraryRepairHref,
  memoryTokenStore,
  resolveLocale,
  type FsAppShell,
} from "@filesquid/ui";
import { imagesquid } from "./distribution";

applyLocale(document, resolveLocale(imagesquid));
const theme = createThemeStore(window);
theme.start();
defineElements();
const shell = document.querySelector<FsAppShell>("fs-app-shell");
const tokens = memoryTokenStore(() => void shell?.refreshHealth());
const client = createHostClient(window, () => tokens.get());
const desktop = detectHost(window).isTauri;
shell?.configure({ distribution: imagesquid, client, theme });
if (shell) {
  const settings = { labels: imagesquid.labels, client, theme, distribution: imagesquid };
  bindSettingsRoute(shell, { ...settings, contractVersion: CONTRACT_VERSION });
}
if (shell) {
  const source = new ClientLibrarySource(client);
  const registry = new LibraryRegistry();
  const facts = { host: client, version: imagesquid.version };
  bindLibraryRoute(shell, { labels: imagesquid.labels, source, registry, ...facts });
}
if (shell) {
  bindMonitorRoute(shell, {
    labels: imagesquid.labels,
    client,
    tokens: desktop ? undefined : tokens,
    repairHref: libraryRepairHref,
  });
}
