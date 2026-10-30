/**
 * Hash routing shared by every distribution: `#/settings` shows the Settings page,
 * `#/navigation/<handle>` shows a navigation destination with its breadcrumb (UI-F2.1b),
 * `#/monitor` shows the Monitor dashboard inside the shell, and
 * `#/files/<library>/<object>/<folder>/...` shows the file browser (UI-F2.2) until the API lists
 * files by navigation node, with the file panel (UI-F2.3) in the shell's panel slot while a file
 * is open; any other hash shows the shell alone. After a route change focus
 * moves to the shell's main area (UI-S-016), so keyboard and
 * screen-reader users land on the new content instead of on a link that may have been removed.
 * Also provides the in-memory token store used on the server host.
 */
import type { FileBrowserOptions, FsFileBrowser } from "../components/fs-file-browser";
import type { MonitorOptions, TokenStore } from "../components/fs-monitor";
import type { FsNavigationView, NavigationViewOptions } from "../components/fs-navigation-view";
import type { FsSettings, SettingsOptions } from "../components/fs-settings";
import { locationHash, parseLocationHash, type FileLocation } from "../files/model";
import type { HistorySource } from "../files/history-source";
import type { DownloadSource } from "../files/download-source";
import type { MetadataSource } from "../files/metadata-source";
import { bindFilePanel, removeFilePanel } from "./file-panel-route";

/** An element the routed views are mounted in; the app shell also offers `focusMain`. */
export type RouteHost = HTMLElement & {
  /** Move focus to the host's main area (`<fs-app-shell>` implements it). */
  focusMain?: () => void;
};

/** Hash of the Monitor route. */
export const MONITOR_ROUTE = "#/monitor";

/** Hash of the All files view of the sidebar (UI-S-025); `bindDriveRoute` binds it (UI-F2.8). */
export const ALL_FILES_ROUTE = "#/files";

/** Hash of the Recent view of the sidebar (UI-S-025, UI-S-026); `bindDriveRoute` binds it (UI-F2.8). */
export const RECENT_ROUTE = "#/recent";

/** Hash of the Recently deleted view (UI-S-002, UI-S-017); UI-F2.17 binds it. */
export const RECENTLY_DELETED_ROUTE = "#/recently-deleted";

/** Hash of the Logs view (UI-S-025, UI-S-032); UI-F2.16 binds it. */
export const LOGS_ROUTE = "#/logs";

/** Hash prefix of the search results (UI-S-030); the URI-encoded query follows. */
export const SEARCH_ROUTE_PREFIX = "#/search/";

/**
 * The address of the results for a query. Pure.
 *
 * @param query The query the global search runs.
 * @returns `#/search/<encoded query>`.
 */
export function searchHash(query: string): string {
  return `${SEARCH_ROUTE_PREFIX}${encodeURIComponent(query)}`;
}

/**
 * The query of a search address, or undefined for any other hash, an empty query or one whose
 * escapes do not decode (logged). Pure apart from the console message.
 *
 * @param hash A location hash.
 * @returns The query, or undefined.
 */
export function parseSearchHash(hash: string): string | undefined {
  if (!hash.startsWith(SEARCH_ROUTE_PREFIX)) return undefined;
  try {
    const query = decodeURIComponent(hash.slice(SEARCH_ROUTE_PREFIX.length));
    return query.trim() === "" ? undefined : query;
  } catch (error) {
    console.warn("FileSquid ignored a search route whose query does not decode", error);
    return undefined;
  }
}

/**
 * Keep a token typed in at run time (the server host's administrator token) in memory only:
 * no localStorage, no sessionStorage, no URL. It is gone when the page reloads or closes.
 *
 * An empty value (for example a blank form field) is not a token: `get` then returns
 * undefined, so the client sends no credential and the API answers `missing_credentials`.
 *
 * @param onSet Called with no arguments after a token is set, for example to reload the
 *   shell's API status; it never receives the token.
 * @returns A token store for `<fs-monitor>`; give the client its getter as the `token`
 *   function (`() => store.get()`), not the store itself.
 */
export function memoryTokenStore(onSet?: () => void): TokenStore {
  let token: string | undefined;
  return {
    get: () => (token === "" ? undefined : token),
    set(value: string) {
      token = value;
      onSet?.();
    },
  };
}

/**
 * Show or hide an `<fs-monitor>` child of `shell` to match the current hash, now and on
 * every `hashchange`. After a `hashchange` (not on the first apply) focus moves to the shell's
 * main area when it offers `focusMain`. Side effect: adds a window listener, mutates the shell's
 * children and moves focus.
 *
 * @param shell Element the Monitor is slotted into (the app shell).
 * @param options Monitor configuration.
 * @param win Window whose hash is followed.
 * @returns A function that removes the listener.
 */
export function bindMonitorRoute(
  shell: RouteHost,
  options: MonitorOptions,
  win: Window = window,
): () => void {
  applyMonitorRoute(shell, options, win.location.hash);
  const onChange = () => {
    applyMonitorRoute(shell, options, win.location.hash);
    shell.focusMain?.();
  };
  win.addEventListener("hashchange", onChange);
  return () => win.removeEventListener("hashchange", onChange);
}

/** Mount the Monitor when `hash` is the Monitor route, otherwise remove it. */
function applyMonitorRoute(shell: HTMLElement, options: MonitorOptions, hash: string): void {
  const existing = shell.querySelector("fs-monitor");
  if (hash !== MONITOR_ROUTE) {
    existing?.remove();
    return;
  }
  if (existing) return;
  const monitor = document.createElement("fs-monitor") as HTMLElement & {
    configure(options: MonitorOptions): void;
  };
  monitor.configure(options);
  shell.append(monitor);
}

/** Hash of the Settings route. */
export const SETTINGS_ROUTE = "#/settings";

/**
 * Show or hide an `<fs-settings>` child of `shell` to match the current hash, now and on
 * every `hashchange`. After a `hashchange` (not on the first apply) focus moves to the shell's
 * main area when it offers `focusMain`. Side effect: adds a window listener, mutates the
 * shell's children and moves focus.
 *
 * @param shell Element the Settings page is slotted into (the app shell).
 * @param options Labels, the client, the theme store, the distribution and the contract version.
 * @param win Window whose hash is followed.
 * @returns A function that removes the listener.
 */
export function bindSettingsRoute(
  shell: RouteHost,
  options: SettingsOptions,
  win: Window = window,
): () => void {
  applySettingsRoute(shell, options, win.location.hash);
  const onChange = () => {
    applySettingsRoute(shell, options, win.location.hash);
    shell.focusMain?.();
  };
  win.addEventListener("hashchange", onChange);
  return () => win.removeEventListener("hashchange", onChange);
}

/** Mount the Settings page when `hash` is the Settings route, otherwise remove it. */
function applySettingsRoute(shell: HTMLElement, options: SettingsOptions, hash: string): void {
  const existing = shell.querySelector("fs-settings");
  if (hash !== SETTINGS_ROUTE) {
    existing?.remove();
    return;
  }
  if (existing) return;
  const settings = document.createElement("fs-settings") as FsSettings;
  settings.configure(options);
  shell.append(settings);
}

/** Hash prefix of a navigation destination; the URI-encoded node handle follows. */
export const NAVIGATION_ROUTE_PREFIX = "#/navigation/";

/** What the navigation route needs. */
export interface NavigationRouteOptions extends NavigationViewOptions {
  /** The navigation tree whose current destination follows the route, when there is one. */
  tree?: HTMLElement & { current?: string };
}

/**
 * The node handle of a navigation route, or undefined for any other hash or a handle whose
 * escapes do not decode (logged). The handle stays opaque: it is decoded, never parsed. Pure
 * apart from the console message.
 *
 * @param hash A location hash.
 * @returns The handle, or undefined.
 */
export function parseNavigationHash(hash: string): string | undefined {
  if (!hash.startsWith(NAVIGATION_ROUTE_PREFIX)) return undefined;
  const encoded = hash.slice(NAVIGATION_ROUTE_PREFIX.length);
  try {
    const handle = decodeURIComponent(encoded);
    return handle === "" ? undefined : handle;
  } catch (error) {
    console.warn("FileSquid ignored a navigation route whose handle does not decode", error);
    return undefined;
  }
}

/**
 * Show or hide an `<fs-navigation-view>` child of `shell` to match the current hash, now and
 * on every `hashchange`: `#/navigation/<handle>` (see `navigationHref`) shows that destination
 * with its breadcrumb and marks it current in the tree; any other hash removes the view and
 * clears the tree's current destination. After a `hashchange` (not on the first apply) focus
 * moves to the shell's main area when it offers `focusMain`. Side effect: adds a window
 * listener, mutates the shell's children and the tree's current destination, and moves focus.
 *
 * @param shell Element the view is slotted into (the app shell).
 * @param options Labels, the navigation source and the tree.
 * @param win Window whose hash is followed.
 * @returns A function that removes the listener.
 */
export function bindNavigationRoute(
  shell: RouteHost,
  options: NavigationRouteOptions,
  win: Window = window,
): () => void {
  applyNavigationRoute(shell, options, win.location.hash);
  const onChange = () => {
    applyNavigationRoute(shell, options, win.location.hash);
    shell.focusMain?.();
  };
  win.addEventListener("hashchange", onChange);
  return () => win.removeEventListener("hashchange", onChange);
}

/** Mount the view for a navigation route, move it to the route's node, or remove it. */
function applyNavigationRoute(shell: HTMLElement, options: NavigationRouteOptions, hash: string) {
  const handle = parseNavigationHash(hash);
  const existing = shell.querySelector<FsNavigationView>("fs-navigation-view");
  if (options.tree && options.tree.current !== handle) options.tree.current = handle;
  if (handle === undefined) {
    existing?.remove();
    return;
  }
  const view = existing ?? createNavigationView(shell, options);
  if (view.node !== handle) view.node = handle;
}

/** Create the view and add it to the shell. */
function createNavigationView(shell: HTMLElement, options: NavigationRouteOptions) {
  const view = document.createElement("fs-navigation-view") as FsNavigationView;
  const { labels, source, online, href, drive } = options;
  view.configure({ labels, source, online, href, drive });
  shell.append(view);
  return view;
}

/** The sources the file panel reads beyond the browser's file source (UI-F2.3, UI-F2.11). */
export interface FilePanelSources {
  /** Reads histories, versions and history sizes. */
  readonly history: HistorySource;
  /** Reads the whole file of one version, for Download this version. */
  readonly download: DownloadSource;
}

/**
 * What the files route needs: the labels and the file source of the browser, and optionally the
 * {@link FilePanelSources} of the file panel; without them the route mounts no file panel. The
 * two sources are one option, so the types never allow a panel with only one of them.
 */
export interface FilesRouteOptions extends Pick<FileBrowserOptions, "labels" | "source"> {
  /** The history and download sources of the file panel. */
  readonly filePanel?: FilePanelSources;
  /** Reads and writes statements for the file panel's Description tab (UI-F2.4). */
  readonly metadata?: MetadataSource;
}

/**
 * Show or hide an `<fs-file-browser>` child of `shell` to match the current hash, now and on
 * every `hashchange`: `#/files/<library>/<object>/<folder>/...` (see `locationHash`) shows the
 * browser at that folder. When the browser enters a folder itself, the address follows with
 * `history.pushState`, so the Back button returns to the folder before without a second load.
 * After a `hashchange` (not on the first apply) focus moves to the shell's main area when it
 * offers `focusMain`. Side effect: adds window and element listeners, mutates the shell's
 * children, writes history entries and moves focus.
 *
 * @param shell Element the browser is slotted into (the app shell).
 * @param options Labels and the file source.
 * @param win Window whose hash is followed.
 * @returns A function that removes the window listener.
 */
export function bindFilesRoute(
  shell: RouteHost,
  options: FilesRouteOptions,
  win: Window = window,
): () => void {
  applyFilesRoute(shell, options, win);
  const onChange = () => {
    applyFilesRoute(shell, options, win);
    shell.focusMain?.();
  };
  win.addEventListener("hashchange", onChange);
  return () => win.removeEventListener("hashchange", onChange);
}

/** Mount the file browser for a files route, move it to the route's folder, or remove it. */
function applyFilesRoute(shell: HTMLElement, options: FilesRouteOptions, win: Window): void {
  const location = parseLocationHash(win.location.hash);
  const existing = shell.querySelector<FsFileBrowser>("fs-file-browser");
  if (!location) {
    existing?.remove();
    removeFilePanel(shell);
    return;
  }
  const browser = existing ?? createFileBrowser(shell, options, win);
  browser.location = location;
}

/** Create the browser, keep the address in step with it and add it to the shell. */
function createFileBrowser(
  shell: HTMLElement,
  options: FilesRouteOptions,
  win: Window,
): FsFileBrowser {
  const browser = document.createElement("fs-file-browser") as FsFileBrowser;
  browser.configure({ labels: options.labels, source: options.source, href: locationHash });
  if (options.filePanel) bindFilePanel(shell, browser, { ...options, ...options.filePanel });
  browser.addEventListener("fs-location-change", (event) => {
    const { location } = (event as CustomEvent<{ location: FileLocation }>).detail;
    const hash = locationHash(location);
    if (win.location.hash !== hash) win.history.pushState(null, "", hash);
  });
  shell.append(browser);
  return browser;
}
