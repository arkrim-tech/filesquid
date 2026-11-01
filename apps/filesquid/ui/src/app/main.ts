/**
 * The default FileSquid distribution: applies the saved theme, then mounts the shell with
 * FileSquid's official brand and the internal vocabulary, talking to whichever host serves
 * this page. The navigation tree in the shell's navigation slot reads the host's live
 * navigation listing (UI-F2.1b), and `#/navigation/<node>` shows that destination with its
 * breadcrumb; the header's search bar (`<fs-search-bar>`, UI-F2.9) opens `#/search/<query>`,
 * the results in the Drive list; `#/settings` shows the Settings page (Appearance with the theme control, Privacy,
 * About with the API status); `#/monitor` shows the Monitor dashboard, whose W010 note links to the library's repair offer;
 * `#/library` creates, opens or recovers a library and `#/library/<id>` shows it; `#/files/<library>/<object>` shows the
 * file browser until the API lists files by navigation node, with the file panel (File info and Versions) for the open file. On the desktop host the client
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
  ClientPreferencesSource,
  ClientViewSource,
  ClientSearchSource,
  singleOpenLibrary,
  ClientDownloadSource,
  ClientFactsSource,
  ClientHistorySource,
  ClientProvenanceSource,
  ClientMetadataSource,
  ClientLibrarySource,
  ClientNavigationSource,
  LibraryRegistry,
  applyLocale,
  PreferencesService,
  bindDriveRoute,
  bindFilesRoute,
  bindStorage,
  bindLibraryRoute,
  bindMonitorRoute,
  bindNavigationRoute,
  bindSettingsRoute,
  createHostClient,
  cachedLibraryScope,
  createThemeStore,
  defineElements,
  detectHost,
  libraryRepairHref,
  mountImports,
  clientDestinationLookup,
  memoryTokenStore,
  resolveLocale,
  sharedNavigationStore,
  type Distribution,
  type FsAppShell,
  type MountedImports,
  type FsNavTree,
  type FsNewMenu,
  type FsSearchBar,
} from "../index";
import { deviceStorage } from "./device-storage";

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
let imports: MountedImports | undefined;
const tokens = memoryTokenStore(() => {
  void shell?.refreshHealth();
  void imports?.panel.reconnect(); // the Jobs panel's first read may have been refused
  navigation.reconnect();
  void tree.reload();
  void preferences.start(); // reads again when the first read was refused before the token
});
const client = createHostClient(window, () => tokens.get());
const navigation = new ClientNavigationSource(client);
const desktop = detectHost(window).isTauri;
// Per-user preferences (UI-F2.8, UI-S-017): read once through the API; the navigation tree's
// pins, order and collapsed nodes move there too, from the device key, once.
const preferences = new PreferencesService({
  source: new ClientPreferencesSource(client),
  navigation: sharedNavigationStore(window),
  storage: deviceStorage(window, "the navigation choices"),
});
void preferences.start();
const files = new ClientFileSource(client);
const viewSource = new ClientViewSource(client, singleOpenLibrary(client, window));
// The global search (UI-F2.9): one source for the results and the bar's live files, in the one
// open library until the rows name theirs; the bar sits in the shell's search slot.
const search = new ClientSearchSource(client, singleOpenLibrary(client, window));
const drive = { source: viewSource, preferences, files, navigation, search };
const searchBar = document.createElement("fs-search-bar") as FsSearchBar;
searchBar.slot = "search";
searchBar.configure({
  labels,
  search,
  navigation,
  scope: cachedLibraryScope(navigation, window),
});
shell?.append(searchBar);
const newMenu = document.createElement("fs-new-menu") as FsNewMenu;
newMenu.configure({ labels });
shell?.append(newMenu);
shell?.configure({ distribution: filesquid, client, theme });
tree.slot = "navigation";
tree.configure({ labels, source: navigation });
shell?.append(tree);
if (shell) bindNavigationRoute(shell, { labels, source: navigation, tree, drive });
if (shell) bindDriveRoute(shell, { labels, navigation, drive, newMenu, searchBar });
if (shell) bindStorage(shell, client);
// The upload dialog, the Jobs panel and the footer status (UI-1.2). The API gives no collection ID
// for a navigation node yet; the lookup asks the route the platform engineer is to add, and the
// dialog says it cannot name the collection, sending nothing, until a host answers it.
if (shell) {
  const lookup = clientDestinationLookup(client);
  imports = mountImports(shell, { labels, client, navigation, newMenu, lookup });
}
if (shell) {
  const settings = { labels, client, theme, distribution: filesquid };
  bindSettingsRoute(shell, { ...settings, contractVersion: CONTRACT_VERSION, preferences });
}
const libraries = {
  labels,
  source: new ClientLibrarySource(client),
  registry: new LibraryRegistry(),
  host: client,
  version: pkg.version,
};
if (shell) bindLibraryRoute(shell, libraries);
if (shell) {
  const monitor = { labels, client, tokens: desktop ? undefined : tokens };
  bindMonitorRoute(shell, { ...monitor, repairHref: libraryRepairHref });
}
if (shell) {
  const history = new ClientHistorySource(client);
  const filePanel = {
    history,
    download: new ClientDownloadSource(client),
    facts: new ClientFactsSource(client),
    provenance: new ClientProvenanceSource(client),
  };
  const browser = { labels, source: files, filePanel, metadata: new ClientMetadataSource(client) };
  bindFilesRoute(shell, browser);
}
