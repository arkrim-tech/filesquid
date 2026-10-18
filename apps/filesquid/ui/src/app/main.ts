/**
 * The default FileSquid distribution: applies the saved theme, then mounts the shell with
 * FileSquid's official brand and the internal vocabulary, talking to whichever host serves
 * this page. `#/monitor` shows the Monitor dashboard. On the desktop host the client uses the
 * per-launch token; on the server host the Monitor asks for the administrator token, which is
 * kept in memory only.
 */
import "../theme/tokens.css";
import labels from "../theme/labels.en.json";
import {
  applyLocale,
  bindMonitorRoute,
  createHostClient,
  createThemeStore,
  defineElements,
  detectHost,
  memoryTokenStore,
  resolveLocale,
  type Distribution,
  type FsAppShell,
} from "../index";

/** FileSquid's own brand: the official mark, with a white outline on dark surfaces. */
const filesquid: Distribution = {
  id: "filesquid",
  productName: "FileSquid",
  logo: { src: "/filesquid-mark.svg", srcDark: "/filesquid-mark-dark.svg", alt: "FileSquid logo" },
  labels,
};

applyLocale(document, resolveLocale(filesquid));
const theme = createThemeStore(window);
theme.start();
defineElements();
const shell = document.querySelector<FsAppShell>("fs-app-shell");
const tokens = memoryTokenStore(() => void shell?.refreshHealth());
const client = createHostClient(window, () => tokens.get());
const desktop = detectHost(window).isTauri;
shell?.configure({ distribution: filesquid, client, theme });
if (shell) bindMonitorRoute(shell, { labels, client, tokens: desktop ? undefined : tokens });
