/**
 * ImageSquid entry point: FileSquid tokens first, ImageSquid tokens on top, then the
 * shared shell configured as ImageSquid and connected to whichever FileSquid host serves it.
 * The FileSquid Monitor is reachable at `#/monitor` (interim entry until the More menu).
 * The desktop host supplies its per-launch token; on the server host the Monitor asks for the
 * administrator token, kept in memory only.
 */
import "@filesquid/ui/theme/tokens.css";
import "./theme/tokens.css";
import {
  bindMonitorRoute,
  createHostClient,
  defineElements,
  detectHost,
  memoryTokenStore,
  type FsAppShell,
} from "@filesquid/ui";
import { imagesquid } from "./distribution";

defineElements();
const shell = document.querySelector<FsAppShell>("fs-app-shell");
const tokens = memoryTokenStore(() => void shell?.refreshHealth());
const client = createHostClient(window, () => tokens.get());
const desktop = detectHost(window).isTauri;
shell?.configure({ distribution: imagesquid, client });
if (shell) {
  bindMonitorRoute(shell, {
    labels: imagesquid.labels,
    client,
    tokens: desktop ? undefined : tokens,
  });
}
