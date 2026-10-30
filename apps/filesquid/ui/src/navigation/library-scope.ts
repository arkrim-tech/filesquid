/**
 * The library the Drive views and the global search work in (UI-F2.8, UI-F2.9): the first library
 * the host lists, as its node handle and its name. Until the view operation names its library
 * (the stop-gap of `singleOpenLibrary`), All files, Recent and the search scope all mean this one
 * library, so the scope chip "In: {library}" and the node a search is sent to cannot disagree.
 */
import type { NavigationNode, NavigationSource } from "./model";

/** The library a view or a search is scoped to. */
export interface LibraryScope {
  /** The library's node handle. */
  readonly node: string;
  /** Its name, or null when the host gives it none (the caller words it from the label table). */
  readonly name: string | null;
}

/**
 * A lookup of the library scope that reads once per route: the answer is kept until `hashchange`
 * on `events`, because creating or opening a library is a route change, or until the navigation
 * source reports a change to the tree; a failed read is not kept, so the next call tries again.
 *
 * @param navigation The navigation listing.
 * @param events Where `hashchange` is announced (the window); without it the first answer is
 *   kept for the page's lifetime.
 * @returns A function resolving to the library, or undefined when the host lists none. Network
 *   I/O on the first call after each route change.
 */
export function cachedLibraryScope(
  navigation: NavigationSource,
  events?: EventTarget,
): () => Promise<LibraryScope | undefined> {
  let cached: Promise<LibraryScope | undefined> | undefined;
  events?.addEventListener("hashchange", () => {
    cached = undefined;
  });
  // A change to the tree (a library added or renamed) invalidates the answer too.
  navigation.subscribe?.(() => {
    cached = undefined;
  });
  return () => {
    if (cached) return cached;
    const read = firstLibraryScope(navigation);
    cached = read;
    read.catch(() => {
      if (cached === read) cached = undefined;
    });
    return read;
  };
}

/**
 * The first library the host lists. At most one read per level, so a host with many libraries
 * costs two calls. Network I/O via the navigation source.
 *
 * @param navigation The navigation listing.
 * @returns The library, or undefined when the host lists none.
 * @throws ApiError or a network error when a listing fails.
 */
export async function firstLibraryScope(
  navigation: NavigationSource,
): Promise<LibraryScope | undefined> {
  const root = await navigation.list(null);
  const repository: NavigationNode | undefined = root.nodes.find((n) => n.kind === "repository");
  if (!repository) return undefined;
  const libraries = await navigation.list(repository.node);
  const library = libraries.nodes.find((node) => node.kind === "library");
  return library ? { node: library.node, name: library.name } : undefined;
}
