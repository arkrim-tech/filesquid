/**
 * Sections and markup of `<fs-nav-tree>` (UI-F2.1a). The tree groups a library's children by
 * kind into sections, in the API's kind order by default: Collections, Virtual collections and
 * Folders, after a Pinned section when something is pinned. With one library a section lists
 * that library's children of its kind; with several, it lists each library that has some, as an
 * open-by-default parent. The builders create elements from their arguments only; the element
 * wires events and loads data.
 *
 * Markup follows the WAI-ARIA navigation treeview example: each item is an
 * `a[role=treeitem][href]` inside an `li[role=none]`, and an open parent owns its
 * `ul[role=group]` through `aria-owns`. Every control carries a `data-key` naming it across
 * re-renders, so focus can return to the same control (UI-S-016).
 */
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";
import {
  MAX_DEPTH,
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

/** A top-level item of a section, with fixed children when the section decides them. */
export interface SectionItem {
  /** The node. */
  readonly node: NavigationNode;
  /** Children decided by the section (a library's nodes of the section's kind; none for pins). */
  readonly fixed?: readonly NavigationNode[];
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
}

/** What the markup asks the element about nodes. */
export interface TreeContext {
  /** The active label table. */
  readonly labels: LabelTable;
  /** Handle of the current destination, if any. */
  readonly current?: string;
  /** The route of a node. */
  href(node: NavigationNode): string;
  /** Whether a parent is open. */
  isOpen(node: NavigationNode): boolean;
  /** A node's loaded children, or undefined while they are not loaded. */
  childrenOf(node: NavigationNode): readonly NavigationNode[] | undefined;
  /** Whether a node's children are loading. */
  isBusy(node: NavigationNode): boolean;
  /** Whether a node is pinned. */
  isPinned(node: string): boolean;
}

/** Where a section sits and which item is its Tab stop. */
export interface SectionPlace {
  /** Index in display order. */
  readonly index: number;
  /** Number of sections. */
  readonly count: number;
  /** The section's active item (its Tab stop), if it has items. */
  readonly active?: NavigationNode;
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
 * The items a section shows now, in document order, down to {@link MAX_DEPTH} levels. Pure
 * apart from the context's questions.
 *
 * @param view The section.
 * @param ctx Open state and loaded children.
 * @returns The visible nodes.
 */
export function visibleNodes(view: SectionView, ctx: TreeContext): NavigationNode[] {
  const walk = (items: readonly SectionItem[], level: number): NavigationNode[] =>
    items.flatMap((item) => {
      const open = level < MAX_DEPTH && expandable(item) && ctx.isOpen(item.node);
      const below = open ? walk(childItems(item, ctx), level + 1) : [];
      return [item.node, ...below];
    });
  return walk(view.items, 1);
}

/**
 * Build a section: its heading and Move buttons, then its tree (or an empty note), the
 * unavailable pins with their Remove buttons, and the Pin toggle.
 *
 * @param view The section.
 * @param ctx Labels and node state.
 * @param place The section's position and active item.
 * @returns The `<section>` element.
 */
export function sectionElement(
  view: SectionView,
  ctx: TreeContext,
  place: SectionPlace,
): HTMLElement {
  const section = document.createElement("section");
  section.dataset.section = view.id;
  const base = `fs-nav-${place.index}`;
  const heading = document.createElement("h2");
  heading.id = `${base}-heading`;
  heading.textContent = view.heading;
  const head = document.createElement("div");
  head.className = "head";
  head.append(heading, moveButton(view, ctx.labels, place, -1));
  head.append(moveButton(view, ctx.labels, place, 1));
  section.append(head);
  const hasItems = view.items.length > 0;
  if (hasItems) section.append(treeElement(view, ctx, place, base));
  if (view.unavailable.length > 0) section.append(unavailableList(view.unavailable, ctx.labels));
  if (!hasItems && view.unavailable.length === 0) section.append(emptyNote(ctx.labels));
  if (hasItems) section.append(pinButton(view, ctx, place));
  return section;
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
  const items: SectionItem[] =
    data.libraries.length === 1 && only
      ? ofKind(only).map((node) => ({ node }))
      : data.libraries
          .map((lib) => ({ node: lib, fixed: ofKind(lib) }))
          .filter((item) => item.fixed.length > 0);
  return { id: kind, heading, items, unavailable: [] };
}

/** The Pinned section: answered pins as flat items, `not_found` pins as unavailable. */
function pinnedSection(pins: readonly ResolvedPin[], labels: LabelTable): SectionView {
  const items = pins.flatMap(({ node }) => (node ? [{ node, fixed: [] }] : []));
  const unavailable = pins.flatMap(({ pin, node }) => (node ? [] : [pin]));
  return { id: PINNED_SECTION, heading: labels.navigationPinned, items, unavailable };
}

/** True when an item can open. */
function expandable(item: SectionItem): boolean {
  return item.fixed ? item.fixed.length > 0 : item.node.has_children;
}

/** The items under an open item: its fixed children, or its loaded children. */
function childItems(item: SectionItem, ctx: TreeContext): SectionItem[] {
  return (item.fixed ?? ctx.childrenOf(item.node) ?? []).map((node) => ({ node }));
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
  name.textContent = unavailableName(pin, labels);
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

/** What building one item needs. */
interface ItemScope {
  readonly view: SectionView;
  readonly ctx: TreeContext;
  readonly place: SectionPlace;
  readonly counter: { next: number };
  readonly base: string;
}

/** The `ul[role=tree]` of a section, named by its heading. */
function treeElement(
  view: SectionView,
  ctx: TreeContext,
  place: SectionPlace,
  base: string,
): HTMLElement {
  const tree = document.createElement("ul");
  tree.setAttribute("role", "tree");
  tree.setAttribute("aria-labelledby", `${base}-heading`);
  const scope: ItemScope = { view, ctx, place, counter: { next: 0 }, base };
  tree.append(...view.items.map((item) => itemElement(item, 1, undefined, scope)));
  return tree;
}

/** `<li role=none><a role=treeitem>…</a><ul role=group>…</ul></li>` for one item. */
function itemElement(
  item: SectionItem,
  level: number,
  parent: string | undefined,
  scope: ItemScope,
): HTMLLIElement {
  const li = document.createElement("li");
  li.setAttribute("role", "none");
  const link = itemLink(item.node, level, parent, scope);
  li.append(link);
  if (!expandable(item) || level >= MAX_DEPTH) return li;
  const open = scope.ctx.isOpen(item.node);
  link.setAttribute("aria-expanded", String(open));
  link.querySelector(".twisty")?.classList.add("parent");
  if (scope.ctx.isBusy(item.node)) link.setAttribute("aria-busy", "true");
  const children = open ? childItems(item, scope.ctx) : [];
  if (children.length === 0) return li;
  const group = document.createElement("ul");
  group.setAttribute("role", "group");
  group.id = `${scope.base}-group-${scope.counter.next++}`;
  link.setAttribute("aria-owns", group.id);
  group.append(...children.map((child) => itemElement(child, level + 1, item.node.node, scope)));
  li.append(group);
  return li;
}

/** The `a[role=treeitem]` of one node. */
function itemLink(
  node: NavigationNode,
  level: number,
  parent: string | undefined,
  scope: ItemScope,
): HTMLAnchorElement {
  const link = document.createElement("a");
  link.setAttribute("role", "treeitem");
  link.href = scope.ctx.href(node);
  link.dataset.node = node.node;
  link.dataset.kind = node.kind;
  link.dataset.key = `item:${scope.view.id}:${node.node}`;
  if (parent !== undefined) link.dataset.parent = parent;
  link.setAttribute("aria-level", String(level));
  link.tabIndex = node.node === scope.place.active?.node ? 0 : -1;
  if (node.node === scope.ctx.current) link.setAttribute("aria-current", "page");
  const twisty = document.createElement("span");
  twisty.className = "twisty";
  twisty.setAttribute("aria-hidden", "true");
  const name = document.createElement("span");
  name.className = "name";
  name.textContent = nodeName(node, scope.ctx.labels);
  link.append(twisty, name);
  return link;
}

/** A `<button type=button>` with a class and focus key. */
function control(cls: string, key: string): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = cls;
  button.dataset.key = key;
  return button;
}
