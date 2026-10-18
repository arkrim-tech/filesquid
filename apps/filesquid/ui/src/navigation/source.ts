/**
 * An in-memory {@link NavigationSource} over a fixture tree, answering exactly as the API's
 * navigation listing does (API-S-021): the repository at the root level, one level of children
 * per call in the given order, `ancestors` from the repository down to the listed node, opaque
 * cursors with a bounded page size, and `not_found` for an unknown node. Tests and demos use
 * it; UI-F2.1b swaps in a source over `FileSquidClient` without changing the elements.
 */
import { ApiError } from "../api/client";
import type {
  NavigationChange,
  NavigationKind,
  NavigationNode,
  NavigationPage,
  NavigationSource,
} from "./model";

/** One node of a fixture tree. */
export interface FixtureNode {
  /** Node handle. */
  readonly node: string;
  /** Node kind. */
  readonly kind: NavigationKind;
  /** Name, or null for an unnamed repository. */
  readonly name: string | null;
  /** Children, in listing order. */
  readonly children?: readonly FixtureNode[];
}

/** The listing route the fixture stands in for, used in its `not_found` errors. */
const ROUTE = "/api/v1/navigation";

/** Default page size, as the API's. */
const DEFAULT_LIMIT = 200;

/** A navigation source answering from a fixture tree held in memory. */
export class MemoryNavigationSource implements NavigationSource {
  private readonly listeners = new Set<(change: NavigationChange) => void>();
  private index = new Map<string, { item: FixtureNode; path: FixtureNode[] }>();
  /** When set, every `list` call rejects with this value (a network error, a denial, …). */
  failure: unknown = undefined;

  /**
   * @param root The repository node and everything below it.
   */
  constructor(private root: FixtureNode) {
    this.reindex();
  }

  /**
   * List one level, as the API does. Rejects with {@link failure} when set, with a
   * `not_found` `ApiError` for an unknown node, and with `invalid_request` for a malformed
   * cursor.
   *
   * @param parent A node handle, or null for the repository.
   * @param cursor The previous page's `next_cursor`, or null.
   * @param limit Page size, 1 to 1,000 (default 200).
   * @returns One page.
   */
  async list(
    parent: string | null,
    cursor?: string | null,
    limit?: number,
  ): Promise<NavigationPage> {
    if (this.failure !== undefined) throw this.failure;
    const entry = parent === null ? undefined : this.index.get(parent);
    if (parent !== null && !entry) throw new ApiError(404, ROUTE, { code: "not_found" });
    const level = entry ? (entry.item.children ?? []) : [this.root];
    const ancestors = entry ? [...entry.path, entry.item].map(toNode) : [];
    if (cursor && !/^\d+$/.test(cursor)) {
      throw new ApiError(400, ROUTE, { code: "invalid_request" });
    }
    const start = cursor ? Number(cursor) : 0;
    const size = Math.min(Math.max(limit ?? DEFAULT_LIMIT, 1), 1000);
    const end = start + size;
    const next = end < level.length ? String(end) : null;
    return { nodes: level.slice(start, end).map(toNode), ancestors, next_cursor: next };
  }

  /**
   * Follow changes announced with {@link replace} or {@link emit}.
   *
   * @param listener Called with each change.
   * @returns A function that stops following.
   */
  subscribe(listener: (change: NavigationChange) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /**
   * Replace the whole tree and announce that everything changed.
   *
   * @param root The new repository node.
   */
  replace(root: FixtureNode): void {
    this.root = root;
    this.reindex();
    this.emit({ all: true });
  }

  /**
   * Announce a change to every subscriber.
   *
   * @param change What changed.
   */
  emit(change: NavigationChange): void {
    for (const listener of [...this.listeners]) listener(change);
  }

  /** Index every node by handle with its ancestors, depth first, at most 10,000 nodes. */
  private reindex(): void {
    const index = new Map<string, { item: FixtureNode; path: FixtureNode[] }>();
    const stack: { item: FixtureNode; path: FixtureNode[] }[] = [{ item: this.root, path: [] }];
    for (let guard = 0; guard < 10_000 && stack.length > 0; guard += 1) {
      const entry = stack.pop() as { item: FixtureNode; path: FixtureNode[] };
      index.set(entry.item.node, entry);
      const path = [...entry.path, entry.item];
      for (const child of entry.item.children ?? []) stack.push({ item: child, path });
    }
    this.index = index;
  }
}

/** The listed form of a fixture node. */
function toNode(item: FixtureNode): NavigationNode {
  const hasChildren = (item.children ?? []).length > 0;
  return { node: item.node, kind: item.kind, name: item.name, has_children: hasChildren };
}
