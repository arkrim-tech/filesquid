/**
 * A navigation source with one very long level, for the tests of the virtualised navigation
 * tree (Task UI-F3.4, UI-S-035): a repository, one library and `total` collections under it,
 * made by arithmetic so a level of 50,000 nodes costs nothing to hold. It answers as the API
 * does (API-S-021): one level per call, `ancestors`, a cursor until the level ends, `not_found`
 * for an unknown node. The memory fixture of UI-F2.1a indexes at most 10,000 nodes, so it
 * cannot stand in for a level over the tree's cap.
 */
import { ApiError } from "../../src/api/client";
import type {
  NavigationChange,
  NavigationNode,
  NavigationPage,
  NavigationSource,
} from "../../src/navigation/model";

/** The repository's handle. */
export const BIG_REPOSITORY = "r";
/** The library's handle. */
export const BIG_LIBRARY = "l1";

/** How the long level is shaped. */
export interface BigTreeOptions {
  /** Collections in the library's level. */
  readonly total: number;
  /** Folders under the collection at an index (0-based); none when omitted. */
  readonly folders?: (index: number) => number;
  /** The name of the collection at an index; `Harbour <index + 1>` when omitted. */
  readonly name?: (index: number) => string;
  /** Nodes a page holds when the caller names no limit; the API's default of 200. */
  readonly pageSize?: number;
}

/** One listing call as the tree made it. */
export interface BigCall {
  /** The listed node, or null for the root level. */
  readonly parent: string | null;
  /** The cursor, or null for a first page. */
  readonly cursor: string | null;
}

/**
 * The handle of the collection at an index.
 *
 * @param index - 0-based place in the library's level.
 * @returns The handle.
 */
export function bigHandle(index: number): string {
  return `c-${index + 1}`;
}

/**
 * The default name of the collection at an index.
 *
 * @param index - 0-based place in the library's level.
 * @returns The name.
 */
export function bigName(index: number): string {
  return `Harbour ${index + 1}`;
}

/** A source over one library with a very long level. */
export class BigNavigationSource implements NavigationSource {
  /** Every listing call, in order. */
  readonly calls: BigCall[] = [];
  private readonly listeners = new Set<(change: NavigationChange) => void>();

  /** @param options The shape of the level. */
  constructor(private readonly options: BigTreeOptions) {}

  /**
   * List one level, as the API does.
   *
   * @param parent - A node handle, or null for the root level.
   * @param cursor - The previous page's cursor.
   * @param limit - Page size; the source's default when omitted.
   * @returns The page.
   * @throws ApiError 404 `not_found` for a node the tree does not hold.
   */
  list(parent: string | null, cursor?: string | null, limit?: number): Promise<NavigationPage> {
    this.calls.push({ parent, cursor: cursor ?? null });
    try {
      return Promise.resolve(this.page(parent, cursor ?? null, limit));
    } catch (error) {
      return Promise.reject(error instanceof Error ? error : new Error(String(error)));
    }
  }

  /**
   * Follow changes pushed with {@link emit}.
   *
   * @param listener - Called with each change.
   * @returns A function that stops following.
   */
  subscribe(listener: (change: NavigationChange) => void): () => void {
    this.listeners.add(listener);
    return () => void this.listeners.delete(listener);
  }

  /**
   * Push a change to every listener.
   *
   * @param change - The change.
   */
  emit(change: NavigationChange): void {
    for (const listener of this.listeners) listener(change);
  }

  /** The repository node. */
  private repository(): NavigationNode {
    return { node: BIG_REPOSITORY, kind: "repository", name: null, has_children: true };
  }

  /** The library node. */
  private library(): NavigationNode {
    return { node: BIG_LIBRARY, kind: "library", name: "Family", has_children: true };
  }

  /** The collection at an index. */
  private collection(index: number): NavigationNode {
    return {
      node: bigHandle(index),
      kind: "collection",
      name: (this.options.name ?? bigName)(index),
      has_children: (this.options.folders?.(index) ?? 0) > 0,
    };
  }

  /** The folder at `at` under the collection at `index`. */
  private folder(index: number, at: number): NavigationNode {
    return {
      node: `f-${index + 1}-${at + 1}`,
      kind: "folder",
      name: `Inner ${at + 1}`,
      has_children: false,
    };
  }

  /** One page of a level. */
  private page(parent: string | null, cursor: string | null, limit?: number): NavigationPage {
    const repository = this.repository();
    const library = this.library();
    if (parent === null) return slice([repository], [], null, limit);
    if (parent === BIG_REPOSITORY) return slice([library], [repository], null, limit);
    const size = limit ?? this.options.pageSize ?? 200;
    const start = Number(cursor ?? 0);
    if (parent === BIG_LIBRARY) {
      return this.span(this.options.total, start, size, [repository, library], (i) =>
        this.collection(i),
      );
    }
    const index = this.indexOf(parent);
    const above = [repository, library, this.collection(index)];
    return this.span(this.options.folders?.(index) ?? 0, start, size, above, (i) =>
      this.folder(index, i),
    );
  }

  /** The 0-based index of a collection handle. */
  private indexOf(handle: string): number {
    const match = /^c-(\d+)$/.exec(handle);
    const index = match ? Number(match[1]) - 1 : -1;
    if (index < 0 || index >= this.options.total) {
      throw new ApiError(404, "/api/v1/navigation", { code: "not_found" });
    }
    return index;
  }

  /** The nodes from `start`, at most `size` of them, with the cursor of the next page. */
  private span(
    count: number,
    start: number,
    size: number,
    ancestors: NavigationNode[],
    at: (index: number) => NavigationNode,
  ): NavigationPage {
    const end = Math.min(count, start + size);
    const nodes = Array.from({ length: Math.max(0, end - start) }, (_, i) => at(start + i));
    return { nodes, ancestors, next_cursor: end < count ? String(end) : null };
  }
}

/** A whole small level as one page. */
function slice(
  nodes: NavigationNode[],
  ancestors: NavigationNode[],
  next: string | null,
  limit?: number,
): NavigationPage {
  return { nodes: nodes.slice(0, limit ?? nodes.length), ancestors, next_cursor: next };
}
