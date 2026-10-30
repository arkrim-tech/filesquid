/**
 * Which library the rows of a view belong to, until the view operation says so (UI-F2.8): the
 * rows of `GET /api/v1/files/view` name a file and its object but not the library, and every
 * file route needs one. When the host has exactly one library open, every node belongs to it, so
 * the answer is unambiguous; with several (or none, or a host that keeps no registry, which
 * answers 503) the library stays unknown and the actions that need it are offered disabled with
 * their reason. This is a stop-gap over `GET /api/v1/libraries/recent` that the platform
 * engineer replaces by naming the library in the view page; the source then uses that.
 */
import type { FileSquidClient } from "../api/client";
import type { components } from "../api/schema";

/** The libraries this host opened, as the API lists them. */
type RecentLibraries = components["schemas"]["RecentLibraries"];

/** The part of `FileSquidClient` the lookup needs (injectable in tests). */
export type LibraryLookupClient = Pick<FileSquidClient, "request">;

/** The route (API-S-038). */
const PATH = "/api/v1/libraries/recent";

/**
 * Build a lookup that reads the host's libraries and answers the one open library. The answer is
 * kept until the route changes (`hashchange` on `events`), because opening or closing a library
 * is a route change; the next call after that reads again.
 *
 * @param client The API client.
 * @param events Where `hashchange` is announced (the window); without it the first answer is
 *   kept for the page's lifetime.
 * @returns A function resolving to the library's id, or undefined when there is not exactly one
 *   open library or the host could not say. Network I/O on the first call after each route
 *   change.
 */
export function singleOpenLibrary(
  client: LibraryLookupClient,
  events?: EventTarget,
): () => Promise<string | undefined> {
  let cached: Promise<string | undefined> | undefined;
  events?.addEventListener("hashchange", () => {
    cached = undefined;
  });
  return () => {
    cached ??= read(client);
    return cached;
  };
}

/** The most libraries `GET /api/v1/libraries/recent` lists (API-S-038). */
export const RECENT_LIBRARIES_CAP = 20;

/**
 * The one open library of a recent-libraries answer. A list at its cap may hide further open
 * libraries, so it proves nothing about "only one" and answers undefined. Pure.
 *
 * @param answer The answer of `GET /api/v1/libraries/recent`.
 * @returns The open library, or undefined when none, several or possibly more are open.
 */
export function onlyOpenLibrary(
  answer: RecentLibraries,
): RecentLibraries["libraries"][number] | undefined {
  const libraries = Array.isArray(answer.libraries) ? answer.libraries : [];
  if (libraries.length >= RECENT_LIBRARIES_CAP) return undefined;
  const open = libraries.filter((library) => library.open);
  return open.length === 1 ? open[0] : undefined;
}

/** Read the libraries and pick the only open one; a failure is logged and answers undefined. */
async function read(client: LibraryLookupClient): Promise<string | undefined> {
  try {
    return onlyOpenLibrary(await client.request<RecentLibraries>(PATH))?.library_id;
  } catch (error) {
    console.warn("FileSquid could not tell which library is open; file actions stay off", error);
    return undefined;
  }
}
