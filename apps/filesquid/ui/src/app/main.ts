/**
 * The default FileSquid distribution: applies the saved theme, then mounts the shell with
 * FileSquid's official brand and the internal vocabulary, talking to whichever host serves
 * this page. The navigation tree in the shell's navigation slot reads the host's live
 * navigation listing (UI-F2.1b), and `#/navigation/<node>` shows that destination with its
 * breadcrumb; `#/settings` shows the Settings page (Appearance with the theme control, Privacy,
 * About with the API status); `#/monitor` shows the Monitor dashboard, whose W010 note links to the library's repair offer;
 * `#/library` creates, opens or recovers a library and `#/library/<id>` shows it; `#/files/<library>/<object>` shows the
 * file browser until the API lists files by navigation node. On the desktop host the client
 * uses the per-launch token; on the server host the Monitor asks for the administrator token,
 * which is kept in memory only, and entering it reconnects the event channel and reads the
 * host, the health and the tree again.
 */
import "../theme/tokens.css";
import labels from "../theme/labels.en.json";
import pkg from "../../package.json";
import { CONTRACT_VERSION } from "../api/contract-version";
import {
  ClientFileSource,
  ClientLibrarySource,
  ClientNavigationSource,
  LibraryRegistry,
  applyLocale,
  bindFilesRoute,
  bindLibraryRoute,
  bindMonitorRoute,
  bindNavigationRoute,
  bindSettingsRoute,
  createHostClient,
  createThemeStore,
  defineElements,
  detectHost,
  libraryRepairHref,
  memoryTokenStore,
  resolveLocale,
  type Distribution,
  type FsAppShell,
  type FsNavTree,
} from "../index";

/** FileSquid's own brand: the official mark, with a white outline on dark surfaces. */
const filesquid: Distribution = {
  id: "filesquid",
  productName: "FileSquid",
  version: pkg.version,
  logo: { src: "/filesquid-mark.svg", srcDark: "/filesquid-mark-dark.svg", alt: "FileSquid logo" },
  labels,
};

applyLocale(document, resolveLocale(filesquid));
const theme = createThemeStore(window);
theme.start();
defineElements();
const shell = document.querySelector<FsAppShell>("fs-app-shell");
const tree = document.createElement("fs-nav-tree") as FsNavTree;
const tokens = memoryTokenStore(() => {
  void shell?.refreshHealth();
  navigation.reconnect();
  void tree.reload();
});
const client = createHostClient(window, () => tokens.get());
const navigation = new ClientNavigationSource(client);
const desktop = detectHost(window).isTauri;
shell?.configure({ distribution: filesquid, client, theme });
tree.slot = "navigation";
tree.configure({ labels, source: navigation });
shell?.append(tree);
if (shell) bindNavigationRoute(shell, { labels, source: navigation, tree });
if (shell) {
  const settings = { labels, client, theme, distribution: filesquid };
  bindSettingsRoute(shell, { ...settings, contractVersion: CONTRACT_VERSION });
}
const libraries = {
  labels,
  source: new ClientLibrarySource(client),
  registry: new LibraryRegistry(),
};
if (shell) bindLibraryRoute(shell, libraries);
if (shell) {
  const monitor = { labels, client, tokens: desktop ? undefined : tokens };
  bindMonitorRoute(shell, { ...monitor, repairHref: libraryRepairHref });
}
if (shell) bindFilesRoute(shell, { labels, source: new ClientFileSource(client) });
