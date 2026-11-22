/**
 * Reading the navigation tree through a {@link NavigationSource} (UI-F2.1a): every page of a
 * level, the first two levels plus each library's children at start, and the name and kind of
 * pinned destinations. Each function is named for the reads it makes; every loop is bounded by
 * {@link MAX_PAGES} pages and {@link MAX_LEVEL_NODES} nodes a level, {@link MAX_LIBRARIES}
 * libraries and {@link MAX_PINNED} pins (AGENTS.md section 22). A level that stops at its cap
 * says so on the console and is marked, so the tree can say so on screen (UI-S-017).
 */
import {
  MAX_LEVEL_NODES,
  MAX_LIBRARIES,
  MAX_PAGES,
  MAX_PINNED,
  isNotFound,
  type NavigationNode,
  type NavigationSource,
  type PinnedDestination,
} from "./model";

/** A whole level: its nodes and the `ancestors` of the listed node. */
export interface NavigationLevel {
  /** The nodes of the level, up to {@link MAX_PAGES} pages and {@link MAX_LEVEL_NODES} nodes. */
  readonly nodes: readonly NavigationNode[];
  /** The listed node's ancestors, the node itself last; empty for the root level. */
  readonly ancestors: readonly NavigationNode[];
  /** True when the level holds more nodes than were read: it stopped at its cap. */
  readonly capped: boolean;
}

/** What the tree shows at start: the repository, its libraries and each library's children. */
export interface NavigationData {
  /** The connected host's repository, or undefined when the root level is empty. */
  readonly repository?: NavigationNode;
  /** The repository's libraries. */
  readonly libraries: readonly NavigationNode[];
  /** Loaded children by parent handle (each library's at start, more as nodes open). */
  readonly children: ReadonlyMap<string, readonly NavigationNode[]>;
  /** The parents among `children` whose level stopped at its cap. */
  readonly capped: ReadonlySet<string>;
}

/** A pinned destination with what the API answered for it: the node, or null for `not_found`. */
export interface ResolvedPin {
  /** The stored pin. */
  readonly pin: PinnedDestination;
  /** The node now, or null when the API answers `not_found`. */
  readonly node: NavigationNode | null;
}

/**
 * Read one level page by page. Stops after {@link MAX_PAGES} pages or {@link MAX_LEVEL_NODES}
 * nodes, whichever comes first, with a console message, and marks the level as capped.
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
  let pages = 0;
  while (pages < MAX_PAGES && nodes.length < MAX_LEVEL_NODES) {
    const answer = await source.list(parent, cursor);
    pages += 1;
    nodes.push(...answer.nodes);
    ancestors = answer.ancestors;
    cursor = answer.next_cursor;
    if (cursor === null) break;
  }
  const capped = cursor !== null || nodes.length > MAX_LEVEL_NODES;
  if (capped) console.warn(capMessage(pages));
  return { nodes: nodes.slice(0, MAX_LEVEL_NODES), ancestors, capped };
}

/** What the console says when a level stopped at its cap: by pages, else by nodes. */
function capMessage(pages: number): string {
  const shown = pages === MAX_PAGES ? `${MAX_PAGES} pages` : `${MAX_LEVEL_NODES} nodes`;
  return `FileSquid showed the first ${shown} of a navigation level only`;
}

/**
 * Read the repository, its first {@link MAX_LIBRARIES} libraries and each one's children. A
 * library whose children cannot be read is logged and shown without them, so one failure does
 * not fail the tree; the repository or library listing failing still rejects. I/O: source
 * reads, at most {@link MAX_LIBRARIES} at once.
 *
 * @param source The navigation source.
 * @returns The data the tree starts from.
 */
export async function readNavigation(source: NavigationSource): Promise<NavigationData> {
  const repository = (await readLevel(source, null)).nodes[0];
  if (!repository) return { libraries: [], children: new Map(), capped: new Set() };
  const listed = (await readLevel(source, repository.node)).nodes;
  if (listed.length > MAX_LIBRARIES) {
    console.warn(`FileSquid shows the first ${MAX_LIBRARIES} libraries in the navigation only`);
  }
  const libraries = listed.slice(0, MAX_LIBRARIES);
  const levels = await Promise.allSettled(libraries.map((lib) => readLevel(source, lib.node)));
  const children = new Map<string, readonly NavigationNode[]>();
  const capped = new Set<string>();
  levels.forEach((level, i) => {
    const lib = libraries[i] as NavigationNode;
    if (level.status === "rejected") {
      console.warn("FileSquid could not read a library's navigation children", level.reason);
      return;
    }
    children.set(lib.node, level.value.nodes);
    if (level.value.capped) capped.add(lib.node);
  });
  return { repository, libraries, children, capped };
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
