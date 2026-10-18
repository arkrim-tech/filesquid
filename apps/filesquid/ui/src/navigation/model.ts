/**
 * The navigation model behind `<fs-nav-tree>` and `<fs-breadcrumb>` (F-004 default UI,
 * UI-F2.1a): the typed data interface, shaped on the API's navigation listing (API-S-021: one
 * level per call, opaque node handles, `kind`, `name`, `has_children`, `ancestors`, cursor
 * paging, `not_found`), the owner's navigation choices as UI-S-017 stores them, and the pure
 * rules over both. Nothing here fetches: a {@link NavigationSource} answers the listing (an
 * in-memory fixture in UI-F2.1a, `FileSquidClient` in UI-F2.1b) without the elements changing.
 *
 * Every list, page loop and walk is bounded (AGENTS.md section 22) by the `MAX_*` constants.
 */
import { ApiError } from "../api/client";
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";

/** What a navigation node stands for (API-S-021 `kind`). */
export type NavigationKind =
  "repository" | "library" | "collection" | "virtual_collection" | "folder";

/** Every kind, in the API's listing order. */
export const NAVIGATION_KINDS: readonly NavigationKind[] = [
  "repository",
  "library",
  "collection",
  "virtual_collection",
  "folder",
];

/** One node of the navigation tree, as the API lists it. */
export interface NavigationNode {
  /** Opaque handle (at most 512 bytes); never parsed, never a path. */
  readonly node: string;
  /** What the node stands for. */
  readonly kind: NavigationKind;
  /** The owner's name for it; `null` for a repository without one (its label is shown). */
  readonly name: string | null;
  /** True when the caller may read at least one child. */
  readonly has_children: boolean;
}

/** One page of a level. */
export interface NavigationPage {
  /** The nodes of this page, in the API's stable order. */
  readonly nodes: readonly NavigationNode[];
  /** For a node's children: the nodes from the repository down to that node; else empty. */
  readonly ancestors: readonly NavigationNode[];
  /** Cursor of the next page, or null on the last page. */
  readonly next_cursor: string | null;
}

/** What changed: the parents whose children changed, or everything. */
export type NavigationChange = { readonly parents: readonly string[] } | { readonly all: true };

/**
 * Answers the navigation listing. The elements never fetch; they ask a source, which rejects
 * with an `ApiError` (`not_found` for an unknown or unreadable node, a denial reason when the
 * caller may not list) or a network error.
 */
export interface NavigationSource {
  /**
   * List one level: the repository when `parent` is null, otherwise the children of `parent`.
   *
   * @param parent A node handle, or null for the root level.
   * @param cursor The `next_cursor` of the previous page, or null for the first page.
   * @param limit Page size, 1 to 1,000; the source's default when omitted.
   * @returns One page.
   */
  list(parent: string | null, cursor?: string | null, limit?: number): Promise<NavigationPage>;
  /**
   * Follow changes to the tree (the `navigation.change` event in UI-F2.1b). Optional.
   *
   * @param listener Called with each change.
   * @returns A function that stops following.
   */
  subscribe?(listener: (change: NavigationChange) => void): () => void;
}

/** One link in a breadcrumb trail. */
export interface BreadcrumbItem {
  /** Node handle. */
  readonly node: string;
  /** The text shown. */
  readonly name: string;
  /** Where following it goes. */
  readonly href: string;
}

/** A pinned destination as stored: its handle and kind only, never its name (UI-S-017). */
export interface PinnedDestination {
  /** Node handle. */
  readonly node: string;
  /** Node kind. */
  readonly kind: NavigationKind;
}

/** The owner's navigation choices (UI-S-017). */
export interface NavigationPreferences {
  /** Pinned destinations, oldest first. */
  readonly pinned: readonly PinnedDestination[];
  /** Section ids in the owner's order; sections not listed keep their default place. */
  readonly order: readonly string[];
  /** Handles of collapsed nodes (nodes open by default: repositories and libraries). */
  readonly collapsed: readonly string[];
}

/** Id of the section listing pinned destinations. */
export const PINNED_SECTION = "pinned";
/** Most pinned destinations kept (UI-S-017). */
export const MAX_PINNED = 100;
/** Most collapsed node handles kept (UI-S-017); the oldest is dropped first. */
export const MAX_COLLAPSED = 1000;
/** Longest node handle accepted, in UTF-8 bytes (API-S-021). */
export const MAX_HANDLE_BYTES = 512;
/** Most section ids in the saved order. */
export const MAX_SECTIONS = 16;
/** Most pages read for one level before the listing stops. */
export const MAX_PAGES = 50;
/** Most libraries whose children the tree reads at start (each read runs at the same time). */
export const MAX_LIBRARIES = 64;
/** Deepest level the tree renders and walks. */
export const MAX_DEPTH = 64;

/** Format version of the stored value. */
const VERSION = 1;

/** No saved choices. */
export const EMPTY_NAVIGATION_PREFERENCES: NavigationPreferences = Object.freeze({
  pinned: [],
  order: [],
  collapsed: [],
});

/** The label-table key of each kind's singular noun. */
export const KIND_LABEL: Readonly<Record<NavigationKind, LabelKey>> = {
  repository: "repository",
  library: "library",
  collection: "collection",
  virtual_collection: "virtualCollection",
  folder: "folder",
};

/** Result of reading a stored value. */
export type StoredPreferences =
  | { readonly ok: true; readonly prefs: NavigationPreferences }
  | { readonly ok: false; readonly problem: string };

/**
 * Read a stored value (already parsed from JSON). A value with another `version`, an unknown
 * field, a malformed entry or more entries than a bound allows is refused as a whole, with the
 * reason, so the caller can say why on the console and use the defaults (UI-S-017). Pure.
 *
 * @param value The parsed stored value.
 * @returns The choices, or why the value was refused.
 */
export function readStoredPreferences(value: unknown): StoredPreferences {
  if (!isRecord(value)) return refuse("it is not an object");
  if (value.version !== VERSION) return refuse(`its version is not ${VERSION}`);
  const unknown = Object.keys(value).filter((k) => !STORED_KEYS.includes(k));
  if (unknown.length > 0) return refuse(`it has unknown fields (${unknown.join(", ")})`);
  const pinned = pinList(value.pinned);
  const order = stringList(absentAsEmpty(value.order), MAX_SECTIONS, isSectionId);
  const collapsed = stringList(absentAsEmpty(value.collapsed), MAX_COLLAPSED, isHandle);
  if (!pinned) return refuse(`its pins are not at most ${MAX_PINNED} {node, kind} entries`);
  if (!order) return refuse(`its section order is not at most ${MAX_SECTIONS} section ids`);
  if (!collapsed) return refuse(`its collapsed nodes are not at most ${MAX_COLLAPSED} handles`);
  return { ok: true, prefs: { pinned, order, collapsed } };
}

/**
 * Write choices as the stored JSON text, with its format version. Pure.
 *
 * @param prefs The choices.
 * @returns JSON text for Web Storage.
 */
export function serializeNavigationPreferences(prefs: NavigationPreferences): string {
  const pinned = prefs.pinned.map(({ node, kind }) => ({ node, kind }));
  return JSON.stringify({
    version: VERSION,
    pinned,
    order: prefs.order,
    collapsed: prefs.collapsed,
  });
}

/**
 * The display order of sections: saved ids that are shown, in saved order; a section that is
 * not saved keeps its default place relative to them, so one that comes before every saved
 * section by default (such as Pinned, when the first pin is made after a reorder) leads, and
 * any other follows. Pure.
 *
 * @param saved The saved order.
 * @param ids The section ids shown now, in default order.
 * @returns Every id in `ids`, once, in display order.
 */
export function sectionOrder(saved: readonly string[], ids: readonly string[]): string[] {
  const present = new Set(ids);
  const kept = [...new Set(saved)].filter((id) => present.has(id));
  const placed = new Set(kept);
  const firstSaved = ids.findIndex((id) => placed.has(id));
  const unsaved = ids.filter((id) => !placed.has(id));
  const leading = unsaved.filter((id) => ids.indexOf(id) < firstSaved);
  const trailing = unsaved.filter((id) => ids.indexOf(id) >= firstSaved);
  return [...leading, ...kept, ...trailing];
}

/**
 * Move one section a step up (-1) or down (+1). A move past either end, or of an unknown id,
 * returns the order unchanged. Pure.
 *
 * @param order The display order.
 * @param id The section to move.
 * @param delta -1 for up, +1 for down.
 * @returns The new order.
 */
export function moveSection(order: readonly string[], id: string, delta: -1 | 1): string[] {
  const from = order.indexOf(id);
  const to = from + delta;
  if (from < 0 || to < 0 || to >= order.length) return [...order];
  const next = [...order];
  next[from] = next[to] as string;
  next[to] = id;
  return next;
}

/**
 * Add an id at the end of a list or remove it, keeping the newest `cap` entries. Pure.
 *
 * @param list The current list.
 * @param id The id to add or remove.
 * @param present True to add, false to remove.
 * @param cap The most entries kept.
 * @returns The new list (a copy, equal to `list` when nothing changed).
 */
export function withMember(
  list: readonly string[],
  id: string,
  present: boolean,
  cap: number,
): string[] {
  if (list.includes(id) === present) return [...list];
  if (!present) return list.filter((entry) => entry !== id);
  return [...list, id].slice(-cap);
}

/**
 * The text shown for a node: its name, or the label of its kind when it has none. Pure.
 *
 * @param node The node.
 * @param labels The active label table.
 * @returns The display name.
 */
export function nodeName(node: NavigationNode, labels: LabelTable): string {
  return node.name ?? labels[KIND_LABEL[node.kind]];
}

/**
 * The default in-app route of a node: `#/navigation/` and the URI-encoded handle. Pure.
 *
 * @param node The node.
 * @returns The route.
 */
export function navigationHref(node: Pick<NavigationNode, "node">): string {
  return `#/navigation/${encodeURIComponent(node.node)}`;
}

/**
 * A breadcrumb trail from a listing's `ancestors` (repository first, the current node last).
 * Pure.
 *
 * @param ancestors The `ancestors` of a listing.
 * @param labels The active label table, for nodes without a name.
 * @param href The route of a node; {@link navigationHref} by default.
 * @returns The trail, at most {@link MAX_DEPTH} long; a deeper trail keeps its last
 *   {@link MAX_DEPTH} nodes, so the current node stays last.
 */
export function breadcrumbItems(
  ancestors: readonly NavigationNode[],
  labels: LabelTable,
  href: (node: NavigationNode) => string = navigationHref,
): BreadcrumbItem[] {
  return ancestors
    .slice(-MAX_DEPTH)
    .map((node) => ({ node: node.node, name: nodeName(node, labels), href: href(node) }));
}

/**
 * The label of a pinned destination the API no longer answers for, for example
 * "Collection (unavailable)". Pure.
 *
 * @param pin The pinned destination.
 * @param labels The active label table.
 * @returns The text shown.
 */
export function unavailableName(pin: PinnedDestination, labels: LabelTable): string {
  return formatLabel(labels.navigationUnavailable, { kind: labels[KIND_LABEL[pin.kind]] });
}

/**
 * True when a listing was answered `not_found` (an unknown node, or one the caller may not
 * read). Pure.
 *
 * @param error What a source rejected with.
 * @returns Whether it is a `not_found` answer.
 */
export function isNotFound(error: unknown): boolean {
  return error instanceof ApiError && error.code === "not_found";
}

/**
 * True for a usable node handle: a non-empty string of at most {@link MAX_HANDLE_BYTES} UTF-8
 * bytes. Pure.
 *
 * @param value Candidate handle.
 * @returns Whether it is usable.
 */
export function isHandle(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value !== "" &&
    new TextEncoder().encode(value).length <= MAX_HANDLE_BYTES
  );
}

/** Fields a stored value may have. */
const STORED_KEYS = ["version", "pinned", "order", "collapsed"];

/** A refusal with its reason. */
function refuse(problem: string): StoredPreferences {
  return { ok: false, problem };
}

/** True for a plain object (not null, not an array). */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** True for a section id: a short, non-empty string. */
function isSectionId(value: unknown): value is string {
  return typeof value === "string" && value !== "" && value.length <= 64;
}

/**
 * An absent stored list means an empty one; anything else, `null` included, is checked as
 * given, so an explicit `null` is refused.
 */
function absentAsEmpty(value: unknown): unknown {
  return value === undefined ? [] : value;
}

/** A bounded list of valid strings (duplicates dropped), or undefined when malformed. */
function stringList(
  value: unknown,
  cap: number,
  valid: (entry: unknown) => entry is string,
): string[] | undefined {
  if (!Array.isArray(value) || value.length > cap || !value.every(valid)) return undefined;
  return [...new Set(value)];
}

/** A bounded list of `{node, kind}` pins (duplicates dropped), or undefined when malformed. */
function pinList(value: unknown): PinnedDestination[] | undefined {
  const list = absentAsEmpty(value);
  if (!Array.isArray(list) || list.length > MAX_PINNED || !list.every(isPin)) return undefined;
  const seen = new Set<string>();
  return list.filter((pin) => !seen.has(pin.node) && Boolean(seen.add(pin.node)));
}

/** True for exactly `{ node, kind }` with a usable handle and a known kind. */
function isPin(value: unknown): value is PinnedDestination {
  if (!isRecord(value) || Object.keys(value).length !== 2) return false;
  return isHandle(value.node) && NAVIGATION_KINDS.includes(value.kind as NavigationKind);
}
