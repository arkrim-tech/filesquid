/**
 * The library the Drive views and the global search work in (UI-F2.8, UI-F2.9): the first library
 * the host lists, as its node handle and its name. Until the view operation names its library
 * (the stop-gap of `singleOpenLibrary`), All files, Recent and the search scope all mean this one
 * library, so the scope chip "In: {library}" and the node a search is sent to cannot disagree.
 */
import {
  MAX_PAGES,
  type NavigationNode,
  type NavigationPage,
  type NavigationSource,
} from "./model";

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
 * The first child of `parent` of one kind, following the listing's cursor (at most
 * {@link MAX_PAGES} pages), so a node on a later page is found too. Network I/O.
 */
async function firstOfKind(
  navigation: NavigationSource,
  parent: string | null,
  kind: NavigationNode["kind"],
): Promise<NavigationNode | undefined> {
  let cursor: string | null = null;
  for (let pages = 0; pages < MAX_PAGES; pages += 1) {
    const page: NavigationPage = await navigation.list(parent, cursor);
    const found = page.nodes.find((node) => node.kind === kind);
    if (found) return found;
    cursor = page.next_cursor;
    if (cursor === null) return undefined;
  }
  return undefined;
}

/**
 * The first library the host lists. It reads the root and then the repository, and follows each
 * listing's cursor until the first node of the kind is found, at most {@link MAX_PAGES} reads per
 * level: two calls when the first pages hold it, and a bounded number when they do not. Network
 * I/O via the navigation source.
 *
 * @param navigation The navigation listing.
 * @returns The library, or undefined when the host lists none.
 * @throws ApiError or a network error when a listing fails.
 */
export async function firstLibraryScope(
  navigation: NavigationSource,
): Promise<LibraryScope | undefined> {
  const repository = await firstOfKind(navigation, null, "repository");
  if (!repository) return undefined;
  const library = await firstOfKind(navigation, repository.node, "library");
  return library ? { node: library.node, name: library.name } : undefined;
}
