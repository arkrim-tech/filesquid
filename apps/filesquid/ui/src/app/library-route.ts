/**
 * Hash routing of the library flow (UI-1.1): `#/library` shows the start page (create, open,
 * recover) without the shell's frame (UI-S-027: the welcome screen has no sidebar, header or
 * footer), `#/library/<id>` shows that library's page, and `#/library/<id>/repair` shows its
 * repair offer, the address the Monitor's W010 note links to through `repairHref`. UI-1.3 adds
 * `#/library/<id>/recover/<job>` (the page that follows a recovered copy's rebuild job, drawn
 * without the shell like mockup 04) and `#/library/<id>/rebuild/<job>` (the same page for a
 * library's own rebuild, inside the shell). One page is
 * mounted at a time; any other hash removes it and restores the frame. Later tasks, such as the import UI, read the
 * open library's ID from this route and need not ask for a folder again.
 */
import type { FsLibraryDetail } from "../components/fs-library-detail";
import type { FsLibraryRebuild } from "../components/fs-library-rebuild";
import type { FsLibraryRepair } from "../components/fs-library-repair";
import type { FsLibraryStart } from "../components/fs-library-start";
import type { LibraryPageOptions } from "../components/library-parts";
import { parseLibraryHash, type LibraryRoute } from "../libraries/model";
import type { RouteHost } from "./routes";

/** The tags of the four pages. */
const PAGE_TAG = {
  start: "fs-library-start",
  library: "fs-library-detail",
  repair: "fs-library-repair",
  job: "fs-library-rebuild",
} as const;

/** All four page tags, for removing the ones that do not match the route. */
const PAGE_TAGS = Object.values(PAGE_TAG);

/** A page of the flow. */
type Page = FsLibraryStart | FsLibraryDetail | FsLibraryRepair | FsLibraryRebuild;

/** True for the pages drawn without the shell's frame: the welcome screen and Recover. */
function isFrameless(route: LibraryRoute | undefined): boolean {
  if (route?.kind === "start") return true;
  return route?.kind === "job" && route.mode === "recover";
}

/**
 * Show the library page the current hash names, now and on every `hashchange`. After a
 * `hashchange` (not on the first apply) focus moves to the shell's main area when it offers
 * `focusMain`. Side effect: adds a window listener, mutates the shell's children and moves
 * focus.
 *
 * @param shell Element the pages are slotted into (the app shell).
 * @param options Labels, the library source, the registry and the optional online check.
 * @param win Window whose hash is followed.
 * @returns A function that removes the listener.
 */
export function bindLibraryRoute(
  shell: RouteHost,
  options: LibraryPageOptions,
  win: Window = window,
): () => void {
  applyLibraryRoute(shell, options, parseLibraryHash(win.location.hash));
  const onChange = () => {
    applyLibraryRoute(shell, options, parseLibraryHash(win.location.hash));
    shell.focusMain?.();
  };
  win.addEventListener("hashchange", onChange);
  return () => win.removeEventListener("hashchange", onChange);
}

/** Mount the page for a route, move it to the route's library, or remove every page. */
function applyLibraryRoute(
  shell: HTMLElement,
  options: LibraryPageOptions,
  route: LibraryRoute | undefined,
): void {
  const wanted = route ? PAGE_TAG[route.kind] : undefined;
  shell.toggleAttribute("frameless", isFrameless(route));
  for (const tag of PAGE_TAGS) {
    if (tag !== wanted) shell.querySelector(`:scope > ${tag}`)?.remove();
  }
  if (!route || !wanted) return;
  const existing = shell.querySelector<HTMLElement>(`:scope > ${wanted}`);
  const page = existing ?? createPage(shell, options, route);
  pointPage(page as Page, route);
}

/** Move a page to the library, and for the job page the job and flow, a route names. */
function pointPage(page: Page, route: LibraryRoute): void {
  if (route.kind === "start") return;
  if (route.kind === "job") {
    const job = page as FsLibraryRebuild;
    if (job.mode !== route.mode) job.mode = route.mode;
    if (job.jobId !== route.job) job.jobId = route.job;
  }
  const withLibrary = page as FsLibraryDetail | FsLibraryRepair | FsLibraryRebuild;
  if (withLibrary.libraryId !== route.id) withLibrary.libraryId = route.id;
}

/** Create the page for a route, configure it and add it to the shell. */
function createPage(shell: HTMLElement, options: LibraryPageOptions, route: LibraryRoute): Page {
  const page = document.createElement(PAGE_TAG[route.kind]) as Page;
  page.configure(options);
  shell.append(page);
  return page;
}
