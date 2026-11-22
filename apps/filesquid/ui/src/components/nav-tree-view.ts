/**
 * Sections and markup of `<fs-nav-tree>` (UI-F2.1a, Task UI-F3.4). The tree groups a library's
 * children by kind into sections, in the API's kind order by default: Collections, Virtual
 * collections and Folders, after a Pinned section when something is pinned. With one library a
 * section lists that library's children of its kind; with several, it lists each library that
 * has some, as an open-by-default parent. The builders create elements from their arguments
 * only; the element wires events, loads data and decides which rows are drawn.
 *
 * Markup: a section's tree is one flat `ul[role=tree]`, as tall as all its rows, that holds
 * only the rows in the window (UI-S-035). Each row is an `a[role=treeitem][href]` inside an
 * `li[role=none]` placed by its row index, and states its own `aria-level`, `aria-setsize` and
 * `aria-posinset`, counted over the whole level. There is no nested `ul[role=group]`: a window
 * cannot hold a group whose parent is not drawn, and the three attributes state the same
 * structure. Every control carries a `data-key` naming it across re-renders, so focus can
 * return to the same control (UI-S-016).
 */
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";
import {
  PINNED_SECTION,
  nodeName,
  sectionOrder,
  unavailableName,
  type NavigationKind,
  type NavigationNode,
  type NavigationPreferences,
  type PinnedDestination,
} from "../navigation/model";
import type { NavigationData, ResolvedPin } from "../navigation/loader";
import { icon, type IconName } from "./icons";
import type { LimitRow, NodeRow, RowContext, TreeRow } from "./nav-tree-rows";

/** A top-level item of a section, with fixed children when the section decides them. */
export interface SectionItem {
  /** The node. */
  readonly node: NavigationNode;
  /** Children decided by the section (a library's nodes of the section's kind; none for pins). */
  readonly fixed?: readonly NavigationNode[];
  /** The nodes shown of the level `fixed` comes from, when it stopped at its cap in this kind. */
  readonly limit?: number;
}

/** A section as shown. */
export interface SectionView {
  /** Section id: `pinned` or a node kind. */
  readonly id: string;
  /** Heading text from the label table. */
  readonly heading: string;
  /** Top-level items. */
  readonly items: readonly SectionItem[];
  /** Pinned destinations the API answers `not_found` for (Pinned section only). */
  readonly unavailable: readonly PinnedDestination[];
  /** The nodes shown of the level the top-level items come from, when it stopped at its cap. */
  readonly limit?: number;
}

/** What the markup asks the element about nodes. */
export interface TreeContext extends RowContext {
  /** The active label table. */
  readonly labels: LabelTable;
  /** Handle of the current destination, if any. */
  readonly current?: string;
  /** The route of a node. */
  href(node: NavigationNode): string;
  /** Whether a node is pinned. */
  isPinned(node: string): boolean;
}

/** Where a section sits and which item is active. */
export interface SectionPlace {
  /** Index in display order. */
  readonly index: number;
  /** Number of sections. */
  readonly count: number;
  /** The section's active item (the one its Pin toggle names), if it has items. */
  readonly active?: NavigationNode;
}

/** What drawing one row of a section needs. */
export interface RowScope {
  /** The section's id. */
  readonly section: string;
  /** The section's id prefix (see {@link sectionBase}), for the ids of its limit lines. */
  readonly base: string;
  /** Labels and node state. */
  readonly ctx: TreeContext;
  /** BCP 47 tag the limit line's count is formatted in. */
  readonly locale: string;
}

/** The kinds that form sections, in default order, with their heading labels. */
const KIND_SECTIONS: readonly (readonly [NavigationKind, LabelKey])[] = [
  ["collection", "collections"],
  ["virtual_collection", "virtualCollections"],
  ["folder", "folders"],
];

/**
 * The sections to show, in the owner's order. Pure.
 *
 * @param data The repository, libraries and loaded children.
 * @param pins The pinned destinations as the API answered them.
 * @param prefs The owner's choices.
 * @param labels The active label table.
 * @returns The sections in display order; only the Pinned section (when something is pinned)
 *   when the repository has no library, so unavailable pins keep their Remove action.
 */
export function sectionViews(
  data: NavigationData,
  pins: readonly ResolvedPin[],
  prefs: NavigationPreferences,
  labels: LabelTable,
): SectionView[] {
  if (data.libraries.length === 0) return pins.length > 0 ? [pinnedSection(pins, labels)] : [];
  const kinds = KIND_SECTIONS.map(([kind, label]) => kindSection(data, kind, labels[label]));
  const all = pins.length > 0 ? [pinnedSection(pins, labels), ...kinds] : kinds;
  const order = sectionOrder(
    prefs.order,
    all.map((view) => view.id),
  );
  return order.map((id) => all.find((view) => view.id === id) as SectionView);
}

/**
 * The id prefix of the section at a place in display order.
 *
 * @param index The section's index.
 * @returns The prefix of the ids inside it.
 */
export function sectionBase(index: number): string {
  return `fs-nav-${index}`;
}

/**
 * Build a section: its heading and Move buttons, then its tree (or an empty note), the
 * unavailable pins with their Remove buttons, and the Pin toggle.
 *
 * @param view The section.
 * @param ctx Labels and node state.
 * @param place The section's position and active item.
 * @param tree The section's tree from {@link treeElement}, when the section has items.
 * @returns The `<section>` element.
 */
export function sectionElement(
  view: SectionView,
  ctx: TreeContext,
  place: SectionPlace,
  tree?: HTMLElement,
): HTMLElement {
  const section = document.createElement("section");
  section.dataset.section = view.id;
  const base = sectionBase(place.index);
  const heading = document.createElement("h2");
  heading.id = `${base}-heading`;
  heading.textContent = view.heading;
  const head = document.createElement("div");
  head.className = "head";
  head.append(heading, moveButton(view, ctx.labels, place, -1));
  head.append(moveButton(view, ctx.labels, place, 1));
  section.append(head);
  if (tree) section.append(tree);
  if (view.unavailable.length > 0) section.append(unavailableList(view.unavailable, ctx.labels));
  if (!tree && view.unavailable.length === 0) section.append(emptyNote(ctx.labels));
  if (tree) section.append(pinButton(view, ctx, place));
  return section;
}

/**
 * The `ul[role=tree]` of a section, named by the section's heading, as tall as all its rows and
 * with no row drawn yet.
 *
 * @param rows Rows of the section's tree, drawn or not.
 * @param base The section's id prefix (see {@link sectionBase}).
 * @returns The list.
 */
export function treeElement(rows: number, base: string): HTMLElement {
  const tree = document.createElement("ul");
  tree.setAttribute("role", "tree");
  tree.setAttribute("aria-labelledby", `${base}-heading`);
  tree.style.setProperty("--rows", String(rows));
  return tree;
}

/**
 * Build the element of one row of a section's tree, placed by its row index.
 *
 * @param rows The section's rows.
 * @param index The row to build.
 * @param scope The section, labels and node state.
 * @returns The `li`, or undefined for a row that draws nothing (the second row of a limit line).
 */
export function rowElement(
  rows: readonly TreeRow[],
  index: number,
  scope: RowScope,
): HTMLLIElement | undefined {
  const row = rows[index];
  if (!row || row.type === "pad") return undefined;
  const li = document.createElement("li");
  li.setAttribute("role", "none");
  li.dataset.index = String(index);
  li.style.setProperty("--r", String(index));
  if (row.type === "limit") fillLimit(li, row, limitId(scope.base, index), scope);
  else li.append(itemLink(row, scope));
  return li;
}

/**
 * Word the Pin toggle for a section's active item and show whether it is pinned.
 *
 * @param button The Pin toggle.
 * @param labels The active label table.
 * @param node The active item.
 * @param pinned Whether it is pinned.
 */
export function describePin(
  button: HTMLButtonElement,
  labels: LabelTable,
  node: NavigationNode,
  pinned: boolean,
): void {
  button.dataset.node = node.node;
  button.dataset.kind = node.kind;
  const name = nodeName(node, labels);
  button.setAttribute("aria-label", formatLabel(labels.navigationPinItem, { name }));
  button.setAttribute("aria-pressed", String(pinned));
}

/** One kind's section. */
function kindSection(data: NavigationData, kind: NavigationKind, heading: string): SectionView {
  const ofKind = (lib: NavigationNode) =>
    (data.children.get(lib.node) ?? []).filter((node) => node.kind === kind);
  const [only] = data.libraries;
  if (data.libraries.length === 1 && only) {
    const items = ofKind(only).map((node) => ({ node }));
    return { id: kind, heading, items, unavailable: [], limit: levelLimit(data, only, kind) };
  }
  const items: SectionItem[] = data.libraries
    .map((lib) => ({ node: lib, fixed: ofKind(lib), limit: levelLimit(data, lib, kind) }))
    .filter((item) => item.fixed.length > 0);
  return { id: kind, heading, items, unavailable: [] };
}

/**
 * The nodes shown of a library's level when it stopped at its cap and its last node is of
 * `kind`: the limit line goes under the last node the level shows (UI-S-017), and that node
 * sits in the section of its kind.
 */
function levelLimit(
  data: NavigationData,
  library: NavigationNode,
  kind: NavigationKind,
): number | undefined {
  const nodes = data.children.get(library.node) ?? [];
  return data.capped.has(library.node) && nodes.at(-1)?.kind === kind ? nodes.length : undefined;
}

/** The Pinned section: answered pins as flat items, `not_found` pins as unavailable. */
function pinnedSection(pins: readonly ResolvedPin[], labels: LabelTable): SectionView {
  const items = pins.flatMap(({ node }) => (node ? [{ node, fixed: [] }] : []));
  const unavailable = pins.flatMap(({ pin, node }) => (node ? [] : [pin]));
  return { id: PINNED_SECTION, heading: labels.navigationPinned, items, unavailable };
}

/** A Move up (-1) or Move down (+1) button, `aria-disabled` at the end it cannot pass. */
function moveButton(
  view: SectionView,
  labels: LabelTable,
  place: SectionPlace,
  delta: -1 | 1,
): HTMLButtonElement {
  const up = delta < 0;
  const button = control(up ? "up" : "down", `${up ? "up" : "down"}:${view.id}`);
  button.append(icon(up ? "arrow-up" : "arrow-down", 16));
  button.dataset.delta = String(delta);
  const template = up ? labels.navigationMoveUp : labels.navigationMoveDown;
  button.setAttribute("aria-label", formatLabel(template, { name: view.heading }));
  if ((up && place.index === 0) || (!up && place.index === place.count - 1)) {
    button.setAttribute("aria-disabled", "true");
  }
  return button;
}

/** The Pin toggle for the section's active item. */
function pinButton(view: SectionView, ctx: TreeContext, place: SectionPlace): HTMLElement {
  const button = control("pin", `pin:${view.id}`);
  button.textContent = ctx.labels.navigationPin;
  const target = place.active ?? (view.items[0] as SectionItem).node;
  describePin(button, ctx.labels, target, ctx.isPinned(target.node));
  return button;
}

/** The unavailable pins, each with a Remove button. */
function unavailableList(pins: readonly PinnedDestination[], labels: LabelTable): HTMLElement {
  const list = document.createElement("ul");
  list.className = "unavailable";
  list.append(...pins.map((pin) => unavailableItem(pin, labels)));
  return list;
}

/** One unavailable pin: its kind's label and a Remove button. */
function unavailableItem(pin: PinnedDestination, labels: LabelTable): HTMLLIElement {
  const li = document.createElement("li");
  const name = document.createElement("span");
  name.className = "name";
  name.append(kindIcon(pin.kind), document.createTextNode(unavailableName(pin, labels)));
  const remove = control("remove", `remove:${pin.node}`);
  remove.dataset.node = pin.node;
  remove.dataset.kind = pin.kind;
  remove.textContent = labels.navigationRemove;
  const label = formatLabel(labels.navigationRemoveItem, { name: name.textContent });
  remove.setAttribute("aria-label", label);
  li.append(name, remove);
  return li;
}

/** The note shown for a section without items. */
function emptyNote(labels: LabelTable): HTMLElement {
  const note = document.createElement("p");
  note.className = "empty";
  note.textContent = labels.stateEmpty;
  return note;
}

/** The id of the limit line at a row index of a section. */
function limitId(base: string, index: number): string {
  return `${base}-limit-${index}`;
}

/** Each level indents by 20 px (16 + 4, mockup 55), so hovered and active rows stay full width. */
function indent(level: number): string {
  return `calc(var(--fs-space-3) + ${level - 1} * (var(--fs-space-3) + var(--fs-space-1)))`;
}

/** Make `li` the line that says a level stopped at its cap (UI-S-017). */
function fillLimit(li: HTMLLIElement, row: LimitRow, id: string, scope: RowScope): void {
  li.className = "limit";
  li.style.setProperty("padding-inline-start", indent(row.level));
  const text = document.createElement("span");
  text.id = id;
  const count = row.count.toLocaleString(scope.locale);
  text.textContent = formatLabel(scope.ctx.labels.navigationLevelLimit, { count });
  // The line is two rows tall; the tooltip carries it whole where a deep indent cuts it short.
  li.title = text.textContent;
  li.append(text);
}

/** The `a[role=treeitem]` of one node, stating its place in the whole level. */
function itemLink(row: NodeRow, scope: RowScope): HTMLAnchorElement {
  const { node } = row;
  const link = document.createElement("a");
  link.setAttribute("role", "treeitem");
  link.href = scope.ctx.href(node);
  link.dataset.node = node.node;
  link.dataset.kind = node.kind;
  link.dataset.key = `item:${scope.section}:${node.node}`;
  link.setAttribute("aria-level", String(row.level));
  link.setAttribute("aria-setsize", String(row.setsize));
  link.setAttribute("aria-posinset", String(row.posinset));
  link.style.setProperty("padding-inline-start", indent(row.level));
  link.tabIndex = -1;
  if (node.node === scope.ctx.current) link.setAttribute("aria-current", "page");
  if (row.limit !== undefined) {
    link.setAttribute("aria-describedby", limitId(scope.base, row.limit));
  }
  const twisty = document.createElement("span");
  twisty.className = "twisty";
  twisty.setAttribute("aria-hidden", "true");
  const name = document.createElement("span");
  name.className = "name";
  name.textContent = nodeName(node, scope.ctx.labels);
  // Rows are one line (the window needs one row height); the tooltip carries a name cut short.
  link.title = name.textContent;
  link.append(twisty, kindIcon(node.kind), name);
  if (row.expanded !== undefined) markParent(link, twisty, row);
  return link;
}

/** Give a parent its open state, its chevron and, while its children load, `aria-busy`. */
function markParent(link: HTMLAnchorElement, twisty: HTMLElement, row: NodeRow): void {
  link.setAttribute("aria-expanded", String(row.expanded));
  twisty.classList.add("parent");
  // Only a parent draws the chevron: a leaf keeps the empty slot, so names align.
  twisty.append(icon("chevron", 16));
  if (row.busy) link.setAttribute("aria-busy", "true");
}

/** The 16 px line icon of a node kind (mockup 55). */
function kindIcon(kind: NavigationKind): SVGSVGElement {
  const names: Record<NavigationKind, IconName> = {
    repository: "library",
    library: "library",
    collection: "collection",
    virtual_collection: "layers",
    recently_deleted: "trash",
    folder: "folder",
  };
  const svg = icon(names[kind] ?? "folder", 16);
  svg.classList.add("kind-icon");
  return svg;
}

/** A `<button type=button>` with a class and focus key. */
function control(cls: string, key: string): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = cls;
  button.dataset.key = key;
  return button;
}
