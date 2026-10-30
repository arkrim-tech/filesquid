/**
 * The routes of the Drive-style views and what they share with the shell (UI-F2.8, UI-S-025,
 * UI-S-026): `#/files` (All files) and `#/recent` (Recent) show a `<fs-drive-view>` over the
 * first library the host lists, `#/search/<query>` shows the same view as the results of the
 * global search in that library (UI-F2.9, UI-S-030), and the view of any destination
 * (`#/navigation/<node>`) is the navigation view's own. This binder also carries what the views
 * and the search bar tell the shell:
 *
 * - `fs-summary` fills the footer's summary slot, and the commit state beside it says every
 *   change is committed (UI-S-025);
 * - `fs-filter` from the sidebar's view filter narrows only the current view (UI-S-007);
 * - `fs-destination-context` tells the New menu which collection or folder is shown, the only
 *   places an upload may go (UI-S-002);
 * - `fs-drive-open` opens a file in the file browser at its own object, where the file panel
 *   and the preview are, until the file panel opens from the view itself;
 * - `fs-search` from the search bar (`detail.q`) moves the address to the results; the bar shows
 *   the address's query (chips and text) while a search is open and is cleared on any other route,
 *   and it is told which collection is shown, for its `in:` suggestion.
 *
 * Each route change moves focus to the shell's main area (UI-S-016), as every route does.
 */
import { ApiError } from "../api/client";
import type { DriveViewOptions, FsDriveView } from "../components/fs-drive-view";
import type { FsNewMenu } from "../components/fs-new-menu";
import type { FsSearchBar } from "../components/fs-search-bar";
import { FILTER_EVENT } from "../components/fs-app-shell";
import { locationHash } from "../files/model";
import { firstLibraryScope } from "../navigation/library-scope";
import type { NavigationSource } from "../navigation/model";
import type { LabelTable } from "../theme/labels";
import type { DriveRow } from "../drive/model";
import {
  ALL_FILES_ROUTE,
  RECENT_ROUTE,
  parseSearchHash,
  searchHash,
  type RouteHost,
} from "./routes";

/** What the Drive routes need. */
export interface DriveRouteOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** The navigation listing that names the library the All files and Recent views show. */
  readonly navigation: NavigationSource;
  /** What every file view needs: its sources and the preferences. */
  readonly drive: Omit<DriveViewOptions, "labels" | "online">;
  /** The New menu, told which collection or folder is shown. */
  readonly newMenu?: FsNewMenu;
  /** The search bar: follows the search address and is told which collection is shown. */
  readonly searchBar?: FsSearchBar;
  /** Whether the browser is online. */
  readonly online?: () => boolean;
}

/** A route that shows a node's view: the view and the label of its title. */
interface ViewRoute {
  readonly view: "all" | "recent";
  readonly title: "fileRoot" | "fileRecent";
}

/** A route that shows the results of a search. */
interface SearchRoute {
  readonly search: string;
}

/** The view and title each route shows. */
const ROUTES: Readonly<Record<string, ViewRoute>> = {
  [ALL_FILES_ROUTE]: { view: "all", title: "fileRoot" },
  [RECENT_ROUTE]: { view: "recent", title: "fileRecent" },
};

/** The route of a hash: a view, a search, or undefined for any other address. */
function routeOf(hash: string): ViewRoute | SearchRoute | undefined {
  const search = parseSearchHash(hash);
  return search === undefined ? ROUTES[hash] : { search };
}

/**
 * The first library the host lists: the node the All files and Recent views show. At most one
 * read per level, so a host with many libraries costs two calls.
 *
 * @param navigation The navigation listing.
 * @returns The library's node handle, or undefined when the host lists none.
 * @throws ApiError or a network error when a listing fails.
 */
export async function firstLibrary(navigation: NavigationSource): Promise<string | undefined> {
  return (await firstLibraryScope(navigation))?.node;
}

/** The slotted `<span>` of a shell slot, created on first use. */
function slotElement(shell: HTMLElement, name: string): HTMLElement {
  const existing = shell.querySelector<HTMLElement>(`:scope > [slot="${name}"]`);
  if (existing) return existing;
  const span = document.createElement("span");
  span.slot = name;
  shell.append(span);
  return span;
}

/** Put the footer's summary and commit state. */
function showFooter(shell: HTMLElement, labels: LabelTable, text: string | undefined): void {
  slotElement(shell, "summary").textContent = text ?? "";
  slotElement(shell, "commit-state").textContent = text === undefined ? "" : labels.fileCommitted;
}

/** Create the drive view for a route and add it to the shell. */
function createView(shell: HTMLElement, options: DriveRouteOptions): FsDriveView {
  const view = document.createElement("fs-drive-view") as unknown as FsDriveView;
  view.configure({ ...options.drive, labels: options.labels, online: options.online });
  shell.append(view);
  return view;
}

/** Show the library's view for a route, or why there is none; dropped when `current` is false. */
async function showLibrary(
  view: FsDriveView,
  options: DriveRouteOptions,
  route: ViewRoute,
  current: () => boolean,
): Promise<void> {
  view.heading = options.labels[route.title];
  view.search = undefined;
  view.view = route.view;
  try {
    const library = await firstLibrary(options.navigation);
    if (library === undefined) throw new ApiError(404, "/api/v1/navigation", { code: "not_found" });
    if (current()) view.node = library;
  } catch (error) {
    if (current()) view.showFailure(error);
    else console.warn("FileSquid dropped a library lookup of a route that was left", error);
  }
}

/** Show the results of a search in the first library, or why there is none. */
async function showSearch(
  view: FsDriveView,
  options: DriveRouteOptions,
  q: string,
  current: () => boolean,
): Promise<void> {
  view.heading = options.labels.searchResultsTitle;
  if (view.search?.q !== q) view.showPending();
  try {
    const library = await firstLibraryScope(options.navigation);
    if (library === undefined) throw new ApiError(404, "/api/v1/navigation", { code: "not_found" });
    const scope = library.name ?? options.labels.library;
    if (current()) view.search = { q, node: library.node, scope };
  } catch (error) {
    if (current()) view.showFailure(error);
    else console.warn("FileSquid dropped a library lookup of a route that was left", error);
  }
}

/**
 * Mount, move or remove the drive view to match the hash, and make the search bar show the
 * address's query (cleared on any other route). Each call starts a new route generation, so a
 * library lookup of an earlier route cannot change the view after a later one.
 */
function applyRoute(
  shell: HTMLElement,
  options: DriveRouteOptions,
  hash: string,
  generation: { n: number },
): void {
  const mine = ++generation.n;
  const route = routeOf(hash);
  const existing = shell.querySelector<FsDriveView>(":scope > fs-drive-view");
  if (options.searchBar) options.searchBar.query = route && "search" in route ? route.search : "";
  if (!route) {
    existing?.remove();
    return;
  }
  const view = existing ?? createView(shell, options);
  const current = () => mine === generation.n;
  if ("search" in route) void showSearch(view, options, route.search, current);
  else void showLibrary(view, options, route, current);
}

/** What the views tell the shell. */
function wireEvents(shell: RouteHost, options: DriveRouteOptions, win: Window): () => void {
  const onSummary = (event: Event) => {
    const text = (event as CustomEvent<{ text: string }>).detail?.text;
    showFooter(shell, options.labels, text);
  };
  const onFilter = (event: Event) => {
    const value = (event as CustomEvent<{ value: string }>).detail?.value ?? "";
    for (const view of shell.querySelectorAll<HTMLElement & { filter: string }>(
      ":scope > fs-drive-view, :scope > fs-navigation-view",
    )) {
      view.filter = value;
    }
  };
  const onContext = (event: Event) => {
    const detail = (event as CustomEvent<{ node: string; kind: string; name?: string } | undefined>)
      .detail;
    const place = detail && (detail.kind === "collection" || detail.kind === "folder");
    if (options.newMenu) options.newMenu.destination = place ? detail.node : undefined;
    if (options.searchBar) {
      const named = place && detail.name !== undefined;
      options.searchBar.folder = named
        ? { node: detail.node, name: detail.name as string }
        : undefined;
    }
  };
  const onSearch = (event: Event) => {
    const q = (event as CustomEvent<{ q: string }>).detail?.q ?? "";
    if (q.trim() === "") return;
    const hash = searchHash(q);
    if (win.location.hash !== hash) win.location.hash = hash;
  };
  const onOpen = (event: Event) => {
    const row = (event as CustomEvent<{ row: DriveRow }>).detail?.row;
    if (!row?.libraryId || !row.objectId) return;
    win.location.hash = locationHash({
      library_id: row.libraryId,
      object_id: row.objectId,
      prefix: "original",
    });
  };
  const handlers: [string, (event: Event) => void][] = [
    ["fs-summary", onSummary],
    [FILTER_EVENT, onFilter],
    ["fs-destination-context", onContext],
    ["fs-drive-open", onOpen],
    ["fs-search", onSearch],
  ];
  for (const [name, handler] of handlers) shell.addEventListener(name, handler);
  return () => handlers.forEach(([name, handler]) => shell.removeEventListener(name, handler));
}

/**
 * Show the drive view for `#/files`, `#/recent` and `#/search/<query>`, now and on every
 * `hashchange`, and carry the footer summary, the sidebar filter, the New menu's destination, the
 * search bar's query and folder, and file opening between the views and the shell. After a
 * `hashchange` (not on the first apply) focus moves to the shell's main area. Side effect: adds
 * window and shell listeners, mutates the shell's children and moves focus.
 *
 * @param shell Element the view is slotted into (the app shell).
 * @param options Labels, navigation, the view's sources and preferences, the New menu and the
 *   search bar.
 * @param win Window whose hash is followed.
 * @returns A function that removes the listeners.
 */
export function bindDriveRoute(
  shell: RouteHost,
  options: DriveRouteOptions,
  win: Window = window,
): () => void {
  const unwire = wireEvents(shell, options, win);
  const generation = { n: 0 };
  applyRoute(shell, options, win.location.hash, generation);
  const onChange = () => {
    // The destination of the last route is not a place on this one; a navigation view that is
    // shown now announces its own once it has read the node.
    if (options.newMenu) options.newMenu.destination = undefined;
    if (options.searchBar) options.searchBar.folder = undefined;
    applyRoute(shell, options, win.location.hash, generation);
    if (!routeOf(win.location.hash)) showFooter(shell, options.labels, undefined);
    shell.focusMain?.();
  };
  win.addEventListener("hashchange", onChange);
  return () => {
    win.removeEventListener("hashchange", onChange);
    unwire();
  };
}
