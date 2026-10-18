/**
 * ImageSquid entry point: FileSquid tokens first, ImageSquid tokens on top, the saved theme
 * applied, then the shared shell configured as ImageSquid and connected to whichever FileSquid
 * host serves it. The FileSquid Monitor is reachable at `#/monitor` (interim entry until the
 * More menu). The desktop host supplies its per-launch token; on the server host the Monitor
 * asks for the administrator token, kept in memory only.
 */
import "@filesquid/ui/theme/tokens.css";
import "./theme/tokens.css";
import {
  applyLocale,
  bindMonitorRoute,
  createHostClient,
  createThemeStore,
  defineElements,
  detectHost,
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
  bindMonitorRoute(shell, {
    labels: imagesquid.labels,
    client,
    tokens: desktop ? undefined : tokens,
  });
}
