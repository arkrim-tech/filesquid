/**
 * The routes of the Drive-style views and what they share with the shell (UI-F2.8, UI-S-025,
 * UI-S-026): `#/files` (All files) and `#/recent` (Recent) show a `<fs-drive-view>` over the
 * first library the host lists, and the view of any destination (`#/navigation/<node>`) is the
 * navigation view's own. This binder also carries what the views tell the shell:
 *
 * - `fs-summary` fills the footer's summary slot, and the commit state beside it says every
 *   change is committed (UI-S-025);
 * - `fs-filter` from the sidebar's view filter narrows only the current view (UI-S-007);
 * - `fs-destination-context` tells the New menu which collection or folder is shown, the only
 *   places an upload may go (UI-S-002);
 * - `fs-drive-open` opens a file in the file browser at its own object, where the file panel
 *   and the preview are, until the file panel opens from the view itself.
 *
 * Each route change moves focus to the shell's main area (UI-S-016), as every route does.
 */
import { ApiError } from "../api/client";
import type { DriveViewOptions, FsDriveView } from "../components/fs-drive-view";
import type { FsNewMenu } from "../components/fs-new-menu";
import { FILTER_EVENT } from "../components/fs-app-shell";
import { locationHash } from "../files/model";
import type { NavigationSource } from "../navigation/model";
import type { LabelTable } from "../theme/labels";
import type { DriveRow } from "../drive/model";
import { ALL_FILES_ROUTE, RECENT_ROUTE, type RouteHost } from "./routes";

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
  /** Whether the browser is online. */
  readonly online?: () => boolean;
}

/** The view and title each route shows. */
const ROUTES: Readonly<
  Record<string, { view: "all" | "recent"; title: "fileRoot" | "fileRecent" }>
> = {
  [ALL_FILES_ROUTE]: { view: "all", title: "fileRoot" },
  [RECENT_ROUTE]: { view: "recent", title: "fileRecent" },
};

/**
 * The first library the host lists: the node the All files and Recent views show. At most one
 * read per level, so a host with many libraries costs two calls.
 *
 * @param navigation The navigation listing.
 * @returns The library's node handle, or undefined when the host lists none.
 * @throws ApiError or a network error when a listing fails.
 */
export async function firstLibrary(navigation: NavigationSource): Promise<string | undefined> {
  const root = await navigation.list(null);
  for (const repository of root.nodes.filter((node) => node.kind === "repository").slice(0, 1)) {
    const libraries = await navigation.list(repository.node);
    return libraries.nodes.find((node) => node.kind === "library")?.node;
  }
  return undefined;
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

/** Show the library's view for a route, or why there is none. */
async function showLibrary(
  view: FsDriveView,
  options: DriveRouteOptions,
  route: (typeof ROUTES)[string],
): Promise<void> {
  view.heading = options.labels[route.title];
  view.view = route.view;
  try {
    const library = await firstLibrary(options.navigation);
    if (library === undefined) throw new ApiError(404, "/api/v1/navigation", { code: "not_found" });
    view.node = library;
  } catch (error) {
    view.showFailure(error);
  }
}

/** Mount, move or remove the drive view to match the hash. */
function applyRoute(shell: HTMLElement, options: DriveRouteOptions, hash: string): void {
  const route = ROUTES[hash];
  const existing = shell.querySelector<FsDriveView>(":scope > fs-drive-view");
  if (!route) {
    existing?.remove();
    return;
  }
  const view = existing ?? createView(shell, options);
  void showLibrary(view, options, route);
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
    const detail = (event as CustomEvent<{ node: string; kind: string } | undefined>).detail;
    const place = detail && (detail.kind === "collection" || detail.kind === "folder");
    if (options.newMenu) options.newMenu.destination = place ? detail.node : undefined;
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
  ];
  for (const [name, handler] of handlers) shell.addEventListener(name, handler);
  return () => handlers.forEach(([name, handler]) => shell.removeEventListener(name, handler));
}

/**
 * Show the drive view for `#/files` and `#/recent`, now and on every `hashchange`, and carry
 * the footer summary, the sidebar filter, the New menu's destination and file opening between
 * the views and the shell. After a `hashchange` (not on the first apply) focus moves to the
 * shell's main area. Side effect: adds window and shell listeners, mutates the shell's children
 * and moves focus.
 *
 * @param shell Element the view is slotted into (the app shell).
 * @param options Labels, navigation, the view's sources and preferences, and the New menu.
 * @param win Window whose hash is followed.
 * @returns A function that removes the listeners.
 */
export function bindDriveRoute(
  shell: RouteHost,
  options: DriveRouteOptions,
  win: Window = window,
): () => void {
  const unwire = wireEvents(shell, options, win);
  applyRoute(shell, options, win.location.hash);
  const onChange = () => {
    applyRoute(shell, options, win.location.hash);
    if (!ROUTES[win.location.hash]) showFooter(shell, options.labels, undefined);
    shell.focusMain?.();
  };
  win.addEventListener("hashchange", onChange);
  return () => {
    win.removeEventListener("hashchange", onChange);
    unwire();
  };
}
