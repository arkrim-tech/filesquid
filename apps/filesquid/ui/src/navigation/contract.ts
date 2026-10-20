/**
 * The navigation contract check (UI-F2.1b part 2, API-S-021): what `GET /api/v1/navigation`
 * answers and what a `navigation.change` event carries are checked here before the tree or the
 * breadcrumb shows any of it, so a host that breaks the contract shows the error state instead
 * of a guessed tree. A node keeps only its four contract fields, so nothing else the host sends
 * reaches the DOM. Pure: no I/O, no DOM.
 */
import {
  NAVIGATION_KINDS,
  isHandle,
  type NavigationChange,
  type NavigationKind,
  type NavigationNode,
  type NavigationPage,
} from "./model";

/** Most nodes one page may hold: the largest `limit` the API accepts. */
export const MAX_PAGE_NODES = 1000;

/** Most parents one `navigation.change` event names; more is sent as `all`. */
export const MAX_CHANGE_PARENTS = 64;

/** True for a plain object (not null, not an array). */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** True for a name the kind allows: a string, or `null` only for a repository. */
function validName(kind: NavigationKind, name: unknown): boolean {
  return typeof name === "string" || (name === null && kind === "repository");
}

/**
 * Check one node and keep only its contract fields. Pure.
 *
 * @param value A node from an answer.
 * @returns The node, or undefined when it breaks the contract.
 */
function parseNode(value: unknown): NavigationNode | undefined {
  if (!isRecord(value) || !isHandle(value.node)) return undefined;
  const kind = value.kind as NavigationKind;
  if (!NAVIGATION_KINDS.includes(kind) || !validName(kind, value.name)) return undefined;
  if (typeof value.has_children !== "boolean") return undefined;
  const name = value.name as string | null;
  return { node: value.node, kind, name, has_children: value.has_children };
}

/** Check a list of nodes of at most {@link MAX_PAGE_NODES}; undefined when any breaks it. */
function parseNodes(value: unknown): NavigationNode[] | undefined {
  if (!Array.isArray(value) || value.length > MAX_PAGE_NODES) return undefined;
  const nodes = value.map(parseNode);
  return nodes.every((node) => node !== undefined) ? (nodes as NavigationNode[]) : undefined;
}

/** True for a page cursor: a non-empty string, or `null` on the last page. */
function validCursor(value: unknown): value is string | null {
  return value === null || (typeof value === "string" && value !== "");
}

/**
 * Check one listing answer against the contract: `nodes` and `ancestors` are lists of valid
 * nodes (handles of 1 to 512 bytes, a known kind, a string name or `null` for a repository,
 * boolean `has_children`), at most {@link MAX_PAGE_NODES} each, and `next_cursor` is a
 * non-empty string or `null`. Pure.
 *
 * @param value The parsed answer.
 * @returns The page with contract fields only, or undefined when it breaks the contract.
 */
export function parseNavigationPage(value: unknown): NavigationPage | undefined {
  if (!isRecord(value) || !validCursor(value.next_cursor)) return undefined;
  const nodes = parseNodes(value.nodes);
  const ancestors = parseNodes(value.ancestors);
  if (!nodes || !ancestors) return undefined;
  return { nodes, ancestors, next_cursor: value.next_cursor };
}

/** True for a list of 1 to {@link MAX_CHANGE_PARENTS} usable handles. */
function validParents(value: unknown): value is string[] {
  if (!Array.isArray(value) || value.length < 1 || value.length > MAX_CHANGE_PARENTS) {
    return false;
  }
  return value.every(isHandle);
}

/**
 * Check a `navigation.change` payload: exactly one of `parents` (1 to 64 handles) and
 * `all: true`. Pure.
 *
 * @param value The event's `data`.
 * @returns The change, or undefined when it breaks the contract.
 */
export function parseNavigationChange(value: unknown): NavigationChange | undefined {
  if (!isRecord(value)) return undefined;
  const hasParents = value.parents !== undefined;
  if (hasParents === (value.all !== undefined)) return undefined;
  if (!hasParents) return value.all === true ? { all: true } : undefined;
  return validParents(value.parents) ? { parents: [...value.parents] } : undefined;
}
