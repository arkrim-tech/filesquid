/**
 * Reading the navigation tree through a {@link NavigationSource} (UI-F2.1a): every page of a
 * level, the first two levels plus each library's children at start, and the name and kind of
 * pinned destinations. Each function is named for the reads it makes; every loop is bounded by
 * {@link MAX_PAGES} pages and {@link MAX_PINNED} pins (AGENTS.md section 22).
 */
import {
  MAX_PAGES,
  MAX_PINNED,
  isNotFound,
  type NavigationNode,
  type NavigationSource,
  type PinnedDestination,
} from "./model";

/** A whole level: its nodes and the `ancestors` of the listed node. */
export interface NavigationLevel {
  /** Every node of the level, up to {@link MAX_PAGES} pages. */
  readonly nodes: readonly NavigationNode[];
  /** The listed node's ancestors, the node itself last; empty for the root level. */
  readonly ancestors: readonly NavigationNode[];
}

/** What the tree shows at start: the repository, its libraries and each library's children. */
export interface NavigationData {
  /** The connected host's repository, or undefined when the root level is empty. */
  readonly repository?: NavigationNode;
  /** The repository's libraries. */
  readonly libraries: readonly NavigationNode[];
  /** Loaded children by parent handle (each library's at start, more as nodes open). */
  readonly children: ReadonlyMap<string, readonly NavigationNode[]>;
}

/** A pinned destination with what the API answered for it: the node, or null for `not_found`. */
export interface ResolvedPin {
  /** The stored pin. */
  readonly pin: PinnedDestination;
  /** The node now, or null when the API answers `not_found`. */
  readonly node: NavigationNode | null;
}

/**
 * Read every page of one level. Stops after {@link MAX_PAGES} pages with a console message.
 * I/O: source reads.
 *
 * @param source The navigation source.
 * @param parent A node handle, or null for the root level.
 * @returns The level.
 */
export async function readLevel(
  source: NavigationSource,
  parent: string | null,
): Promise<NavigationLevel> {
  const nodes: NavigationNode[] = [];
  let ancestors: readonly NavigationNode[] = [];
  let cursor: string | null = null;
  for (let page = 0; page < MAX_PAGES; page += 1) {
    const answer = await source.list(parent, cursor);
    nodes.push(...answer.nodes);
    ancestors = answer.ancestors;
    cursor = answer.next_cursor;
    if (cursor === null) return { nodes, ancestors };
  }
  console.warn(`FileSquid showed the first ${MAX_PAGES} pages of a navigation level only`);
  return { nodes, ancestors };
}

/**
 * Read the repository, its libraries and every library's children. I/O: source reads.
 *
 * @param source The navigation source.
 * @returns The data the tree starts from.
 */
export async function readNavigation(source: NavigationSource): Promise<NavigationData> {
  const repository = (await readLevel(source, null)).nodes[0];
  if (!repository) return { libraries: [], children: new Map() };
  const libraries = (await readLevel(source, repository.node)).nodes;
  const levels = await Promise.all(libraries.map((lib) => readLevel(source, lib.node)));
  const children = new Map(libraries.map((lib, i) => [lib.node, levels[i]?.nodes ?? []]));
  return { repository, libraries, children };
}

/**
 * Read each pinned destination's node (its name and kind now) from the last of its
 * `ancestors`. A `not_found` answer marks the pin unavailable; any other failure is logged and
 * the pin is left out until the next load. At most {@link MAX_PINNED} pins are read. I/O:
 * source reads.
 *
 * @param source The navigation source.
 * @param pins The stored pins.
 * @returns The pins that were answered, in stored order.
 */
export async function readPins(
  source: NavigationSource,
  pins: readonly PinnedDestination[],
): Promise<ResolvedPin[]> {
  const answers = await Promise.all(pins.slice(0, MAX_PINNED).map((pin) => readPin(source, pin)));
  return answers.filter((answer): answer is ResolvedPin => answer !== undefined);
}

/** Read one pin: its node, null for `not_found`, undefined (logged) for any other failure. */
async function readPin(
  source: NavigationSource,
  pin: PinnedDestination,
): Promise<ResolvedPin | undefined> {
  try {
    const page = await source.list(pin.node, null, 1);
    return { pin, node: page.ancestors.at(-1) ?? null };
  } catch (error) {
    if (isNotFound(error)) return { pin, node: null };
    console.warn("FileSquid could not read a pinned destination; it is left out for now", error);
    return undefined;
  }
}
