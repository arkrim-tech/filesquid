/**
 * The default FileSquid distribution: mounts the shell with FileSquid's brand and the
 * internal vocabulary, talking to whichever host serves this page. `#/monitor` shows the
 * Monitor dashboard. On the desktop host the client uses the per-launch token; on the server
 * host the Monitor asks for the administrator token, which is kept in memory only.
 */
import "../theme/tokens.css";
import labels from "../theme/labels.en.json";
import {
  bindMonitorRoute,
  createHostClient,
  defineElements,
  detectHost,
  memoryTokenStore,
  type FsAppShell,
} from "../index";

defineElements();
const shell = document.querySelector<FsAppShell>("fs-app-shell");
const tokens = memoryTokenStore(() => void shell?.refreshHealth());
const client = createHostClient(window, () => tokens.get());
const desktop = detectHost(window).isTauri;
shell?.configure({
  distribution: {
    id: "filesquid",
    productName: "FileSquid",
    logo: { src: "/filesquid-logo.png", alt: "FileSquid logo" },
    labels,
  },
  client,
});
if (shell) bindMonitorRoute(shell, { labels, client, tokens: desktop ? undefined : tokens });
