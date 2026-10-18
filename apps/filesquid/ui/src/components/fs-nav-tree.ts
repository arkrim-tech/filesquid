/**
 * `<fs-nav-tree>`: the navigation tree of the FileSquid default UI (F-004 default UI,
 * UI-F2.1a). It reads the tree level by level from an injected {@link NavigationSource} shaped
 * on the API's navigation listing (API-S-021), so the in-memory fixture of UI-F2.1a and the live
 * source of UI-F2.1b plug in without changing the element; it never fetches by itself. It shows
 * a library's collections, virtual collections and folders as sections, with pinned
 * destinations, reorderable sections, and collapse and expand, and keeps those choices in a
 * {@link NavigationStore} (UI-S-017). Until the first answer, and when reading fails,
 * `<fs-states>` shows loading, empty, error, offline or permission denied.
 *
 * Open state: repositories and libraries are open unless the owner collapsed them (kept with the
 * choices); deeper nodes are closed until opened, and their children are read when they open.
 *
 * Accessibility (UI-S-016): each section has a heading, Move up and Move down buttons (also
 * Alt+Shift+Up and Alt+Shift+Down anywhere in the section; `aria-disabled` at the ends so focus
 * stays put), a WAI-ARIA treeview with one Tab stop and the treeview keys, and a Pin toggle for
 * the tree's active item. When a re-render removes the focused control, focus returns to the
 * same control in the new render, else to the first Tab stop; focus never moves when it was
 * outside the tree.
 */
import type { LabelTable } from "../theme/labels";
import {
  navigationHref,
  type NavigationKind,
  type NavigationNode,
  type NavigationSource,
  type PinnedDestination,
} from "../navigation/model";
import {
  readLevel,
  readNavigation,
  readPins,
  type NavigationData,
  type ResolvedPin,
} from "../navigation/loader";
import { sharedNavigationStore, type NavigationStore } from "../navigation/store";
import { A11Y_STYLE } from "./a11y-style";
import { focusElement, recoverFocus } from "../a11y/focus";
import { failureState, type ViewState } from "./view-state";
import { RETRY_EVENT, type FsStates } from "./fs-states";
import { treeKeyAction, type KeyItem } from "./nav-tree-keys";
import {
  describePin,
  sectionElement,
  sectionViews,
  visibleNodes,
  type SectionView,
  type TreeContext,
} from "./nav-tree-view";

/** What the tree needs. */
export interface NavTreeOptions {
  /** The active label table. */
  labels: LabelTable;
  /** Answers the navigation listing; the tree never fetches by itself. */
  source: NavigationSource;
  /** The owner's choices; defaults to the window's shared device-backed store. */
  store?: NavigationStore;
  /** Whether the browser is online; defaults to `navigator.onLine`. */
  online?: () => boolean;
  /** The route of a node; defaults to `navigationHref`. */
  href?: (node: NavigationNode) => string;
}

/** Kinds open by default; the owner's collapse of them is kept (UI-S-017). */
const OPEN_BY_DEFAULT: ReadonlySet<NavigationKind> = new Set(["repository", "library"]);

/** Most deeper nodes kept open in one session; the oldest closes first. */
const MAX_SESSION_OPEN = 256;

const STYLE = `
  :host { display: block; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-body); color: var(--fs-on-surface); }
  .sections { display: flex; flex-direction: column; gap: var(--fs-space-4); }
  section { display: flex; flex-direction: column; gap: var(--fs-space-1); min-width: 0; }
  .head { display: flex; align-items: center; gap: var(--fs-space-1); }
  h2 { flex: 1 1 auto; min-width: 0; margin: 0; padding: var(--fs-space-1) var(--fs-space-2); font-size: var(--fs-font-size-small); font-weight: 700; color: var(--fs-on-surface-variant); overflow-wrap: anywhere; }
  button { border: 0; border-radius: var(--fs-radius); background: none; color: var(--fs-on-surface-variant); font: inherit; cursor: pointer; transition: background-color var(--fs-motion-duration), color var(--fs-motion-duration); }
  button:hover { background: var(--fs-surface); color: var(--fs-on-surface); }
  .up, .down { flex: 0 0 auto; padding: var(--fs-space-1) var(--fs-space-2); }
  .up::before { content: "\\2191" / ""; }
  .down::before { content: "\\2193" / ""; }
  button[aria-disabled="true"] { opacity: 0.4; cursor: default; }
  .pin, .remove { align-self: flex-start; padding: var(--fs-space-1) var(--fs-space-3); border: 1px solid var(--fs-outline); font-size: var(--fs-font-size-small); }
  .pin[aria-pressed="true"] { border-color: var(--fs-primary); background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  ul { list-style: none; margin: 0; padding: 0; }
  ul[role="group"] { padding-inline-start: var(--fs-space-4); }
  .unavailable li { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-2); padding: var(--fs-space-1) var(--fs-space-2); color: var(--fs-on-surface-variant); overflow-wrap: anywhere; }
  [hidden] { display: none !important; }
  a { display: flex; align-items: center; gap: var(--fs-space-1); padding: var(--fs-space-1) var(--fs-space-2); border-radius: var(--fs-radius); color: inherit; text-decoration: none; overflow-wrap: anywhere; transition: background-color var(--fs-motion-duration); }
  a:hover { background: var(--fs-surface); }
  a[aria-current="page"] { background: var(--fs-primary-container); color: var(--fs-on-primary-container); font-weight: 600; }
  a[aria-busy="true"] { color: var(--fs-on-surface-variant); }
  .name { min-width: 0; }
  .twisty { flex: 0 0 auto; inline-size: 1em; color: var(--fs-on-surface-variant); text-align: center; }
  .twisty.parent { cursor: pointer; }
  .twisty.parent::before { content: "\\25B8" / ""; }
  .twisty.parent:dir(rtl)::before { content: "\\25C2" / ""; }
  [aria-expanded="true"] > .twisty.parent::before { content: "\\25BE" / ""; }
  .empty { margin: 0; padding: var(--fs-space-1) var(--fs-space-2); color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  @media (forced-colors: active) {
    a[aria-current="page"], .pin[aria-pressed="true"] { forced-color-adjust: none; background: Highlight; color: HighlightText; }
  }
`;

/** Custom element that shows the navigation tree. */
export class FsNavTree extends HTMLElement {
  private options?: NavTreeOptions;
  private store?: NavigationStore;
  private currentId?: string;
  private data?: NavigationData;
  private failure?: { error: unknown };
  private readonly loaded = new Map<string, readonly NavigationNode[]>();
  private readonly pinCache = new Map<string, ResolvedPin>();
  private pins: ResolvedPin[] = [];
  /** Deeper nodes opened this session, oldest first. */
  private readonly opened = new Set<string>();
  private readonly busy = new Set<string>();
  /** Each section's active item handle (its Tab stop), kept across re-renders. */
  private readonly active = new Map<string, string>();
  /** The nodes in the last render, by handle. */
  private rendered = new Map<string, NavigationNode>();
  private generation = 0;
  private stopFollowing?: () => void;
  /**
   * Pins already read since the last load or store change. A pin whose read failed (other than
   * `not_found`) is read again only after the next load or store change, never in a loop.
   */
  private readonly pinsTried = new Set<string>();
  private readonly onStoreChange = () => {
    this.pinsTried.clear();
    this.syncPins();
  };

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.addEventListener("click", (event) => this.onClick(event));
    root.addEventListener("keydown", (event) => this.onKeydown(event as KeyboardEvent));
    root.addEventListener("focusin", (event) => this.onFocusin(event));
    root.addEventListener(RETRY_EVENT, () => void this.reload());
  }

  /**
   * Set the labels, source and choices store, then read the tree. Call before or after
   * connecting. Without a store, the window's shared device-backed store is used.
   *
   * @param options Labels, source, and the optional store, online check and route.
   */
  configure(options: NavTreeOptions): void {
    this.disconnect();
    this.options = options;
    this.store = options.store ?? sharedNavigationStore(window);
    this.data = undefined;
    if (this.isConnected) this.connect();
  }

  /** Handle of the current destination, marked with `aria-current="page"`. */
  get current(): string | undefined {
    return this.currentId;
  }

  /** Mark another destination as current. */
  set current(node: string | undefined) {
    this.currentId = node;
    this.render();
  }

  /** Follow the store and source, and read the tree, while attached. */
  connectedCallback(): void {
    this.connect();
  }

  /** Stop following the store and source once detached. */
  disconnectedCallback(): void {
    this.disconnect();
  }

  /**
   * Read the repository, libraries, their children, the pins and the children of every node
   * opened this session again. Until the first answer the loading state shows; later reloads
   * keep showing the last answer until the new one arrives. An answer that arrives after a newer
   * reload started is dropped. I/O: source reads.
   *
   * @returns Resolves once the new answer (or failure) is shown.
   */
  async reload(): Promise<void> {
    const source = this.options?.source;
    if (!source || !this.store) return;
    const generation = ++this.generation;
    this.failure = undefined;
    this.render();
    try {
      const data = await readNavigation(source);
      const asked = this.store.preferences.pinned;
      const pins = await readPins(source, asked);
      const reopened = await this.readOpened(source);
      if (generation !== this.generation || source !== this.options?.source) return;
      this.accept(data, pins, reopened, asked);
    } catch (error) {
      if (generation !== this.generation || source !== this.options?.source) return;
      this.fail(error);
    }
    this.render();
  }

  /** Subscribe to the store and source, then read the tree. */
  private connect(): void {
    const options = this.options;
    if (!options || !this.store) return;
    this.disconnect();
    this.store.addEventListener("change", this.onStoreChange);
    this.stopFollowing = options.source.subscribe?.(() => void this.reload());
    void this.reload();
  }

  /** Stop following the store and source. */
  private disconnect(): void {
    this.store?.removeEventListener("change", this.onStoreChange);
    this.stopFollowing?.();
    this.stopFollowing = undefined;
  }

  /** Keep a new answer; the pins it asked for count as tried until the next store change. */
  private accept(
    data: NavigationData,
    pins: readonly ResolvedPin[],
    reopened: ReadonlyMap<string, readonly NavigationNode[]>,
    asked: readonly PinnedDestination[],
  ): void {
    this.data = data;
    this.loaded.clear();
    for (const [parent, nodes] of [...data.children, ...reopened]) this.loaded.set(parent, nodes);
    for (const parent of this.opened) if (!reopened.has(parent)) this.opened.delete(parent);
    this.pinCache.clear();
    for (const pin of pins) this.pinCache.set(pin.pin.node, pin);
    this.pinsTried.clear();
    for (const pin of asked) this.pinsTried.add(pin.node);
    this.syncPins(false);
  }

  /** Keep a failed answer and log it. */
  private fail(error: unknown): void {
    console.warn("FileSquid could not read the navigation", error);
    this.failure = { error };
  }

  /** Read the children of nodes opened this session; nodes that fail stay closed. */
  private async readOpened(
    source: NavigationSource,
  ): Promise<Map<string, readonly NavigationNode[]>> {
    const opened = [...this.opened];
    const levels = await Promise.allSettled(opened.map((node) => readLevel(source, node)));
    const result = new Map<string, readonly NavigationNode[]>();
    levels.forEach((level, i) => {
      if (level.status === "fulfilled") result.set(opened[i] as string, level.value.nodes);
      else console.warn("FileSquid closed a navigation node it could not read", level.reason);
    });
    return result;
  }

  /**
   * Line the shown pins up with the stored ones: known answers are reused, a node already on
   * screen is taken from there, and the rest are read from the source. Then render.
   */
  private syncPins(renderNow = true): void {
    const stored = this.store?.preferences.pinned ?? [];
    const missing: PinnedDestination[] = [];
    this.pins = stored.flatMap((pin) => {
      const known = this.pinCache.get(pin.node) ?? this.renderedPin(pin);
      if (known) this.pinCache.set(pin.node, known);
      else missing.push(pin);
      return known ? [known] : [];
    });
    const untried = missing.filter((pin) => !this.pinsTried.has(pin.node));
    for (const pin of untried) this.pinsTried.add(pin.node);
    if (untried.length > 0 && this.data) void this.readMissingPins(untried);
    if (renderNow) this.render();
  }

  /** A pin whose node is on screen. */
  private renderedPin(pin: PinnedDestination): ResolvedPin | undefined {
    const node = this.rendered.get(pin.node);
    return node ? { pin, node } : undefined;
  }

  /**
   * Read pins that are neither known nor on screen, then show them. An answer that arrives
   * after a newer load started, or from a source that was replaced, is dropped. I/O: source
   * reads.
   */
  private async readMissingPins(pins: readonly PinnedDestination[]): Promise<void> {
    const source = this.options?.source;
    if (!source) return;
    const generation = this.generation;
    const answers = await readPins(source, pins);
    if (generation !== this.generation || source !== this.options?.source) return;
    for (const answer of answers) this.pinCache.set(answer.pin.node, answer);
    this.syncPins();
  }

  /** The stylesheet, states element and sections container, created once. */
  private skeleton(root: ShadowRoot, labels: LabelTable): { states: FsStates; box: HTMLElement } {
    let states = root.querySelector<FsStates>("fs-states");
    let box = root.querySelector<HTMLElement>(".sections");
    if (!states || !box) {
      const style = document.createElement("style");
      style.textContent = A11Y_STYLE + STYLE;
      states = document.createElement("fs-states") as FsStates;
      states.dataset.key = "states";
      box = document.createElement("div");
      box.className = "sections";
      root.replaceChildren(style, states, box);
    }
    states.configure({ labels });
    return { states, box };
  }

  /** Show the sections when there is an answer, otherwise the state; recover focus if needed. */
  private render(): void {
    const root = this.shadowRoot;
    const options = this.options;
    if (!root || !options || !this.store || !this.isConnected) return;
    const { states, box } = this.skeleton(root, options.labels);
    const hadFocus = root.activeElement !== null;
    const key = root.activeElement?.closest<HTMLElement>("[data-key]")?.dataset.key;
    const views = this.failure || !this.data ? undefined : this.views(this.data);
    const state = this.viewState(views);
    states.hidden = state === undefined;
    if (state) states.state = state;
    box.hidden = state !== undefined;
    box.replaceChildren(...(state || !views ? [] : this.sections(views)));
    recoverFocus(root, hadFocus, () => this.focusTarget(root, key, states));
  }

  /** The sections of an answer. */
  private views(data: NavigationData): SectionView[] {
    const { labels } = this.options as NavTreeOptions;
    return sectionViews(data, this.pins, (this.store as NavigationStore).preferences, labels);
  }

  /** The state to show instead of sections, or undefined to show them. */
  private viewState(views: readonly SectionView[] | undefined): ViewState | undefined {
    if (this.failure) {
      const online = this.options?.online ?? (() => navigator.onLine);
      return failureState(this.failure.error, online());
    }
    if (!views) return { kind: "loading" };
    const empty = views.every((v) => v.items.length === 0 && v.unavailable.length === 0);
    return empty ? { kind: "empty" } : undefined;
  }

  /** Build every section element and remember the rendered nodes. */
  private sections(views: readonly SectionView[]): HTMLElement[] {
    const ctx = this.context();
    this.rendered = new Map();
    return views.map((view, index) => {
      const visible = visibleNodes(view, ctx);
      for (const node of visible) this.rendered.set(node.node, node);
      const active = this.activeNode(view.id, visible);
      return sectionElement(view, ctx, { index, count: views.length, active });
    });
  }

  /** What the markup asks about nodes. */
  private context(): TreeContext {
    const options = this.options as NavTreeOptions;
    const pinned = new Set((this.store as NavigationStore).preferences.pinned.map((p) => p.node));
    return {
      labels: options.labels,
      current: this.currentId,
      href: options.href ?? navigationHref,
      isOpen: (node) => this.isOpen(node),
      childrenOf: (node) => this.loaded.get(node.node),
      isBusy: (node) => this.busy.has(node.node),
      isPinned: (node) => pinned.has(node),
    };
  }

  /** Whether a parent is open now. */
  private isOpen(node: NavigationNode): boolean {
    if (OPEN_BY_DEFAULT.has(node.kind)) {
      return !(this.store as NavigationStore).preferences.collapsed.includes(node.node);
    }
    return this.opened.has(node.node) && this.loaded.has(node.node);
  }

  /** The section's Tab stop: the remembered item, else the current one, else the first. */
  private activeNode(section: string, visible: readonly NavigationNode[]) {
    const remembered = this.active.get(section);
    return (
      visible.find((n) => n.node === remembered) ??
      visible.find((n) => n.node === this.currentId) ??
      visible[0]
    );
  }

  /** Where focus goes after a re-render removed it: the same control, else a fallback. */
  private focusTarget(root: ShadowRoot, key: string | undefined, states: FsStates): HTMLElement {
    const same = [...root.querySelectorAll<HTMLElement>("[data-key]")].find(
      (el) => el.dataset.key === key && !el.closest("[hidden]"),
    );
    const firstStop = root.querySelector<HTMLElement>('[role="treeitem"][tabindex="0"]');
    return same ?? firstStop ?? root.querySelector<HTMLElement>("button") ?? states;
  }

  /**
   * Open or close a parent. Repositories and libraries change the kept choices; deeper nodes
   * open for this session and have their children read when first opened.
   */
  private setOpen(node: NavigationNode, open: boolean): void {
    if (OPEN_BY_DEFAULT.has(node.kind)) {
      this.store?.setCollapsed(node.node, !open);
      return;
    }
    this.opened.delete(node.node);
    if (open) this.opened.add(node.node);
    if (this.opened.size > MAX_SESSION_OPEN)
      this.opened.delete(this.opened.values().next().value as string);
    if (open && !this.loaded.has(node.node)) void this.readChildren(node);
    else this.render();
  }

  /**
   * Read a node's children, marking it busy meanwhile. An answer that arrives after a newer
   * load started, or from a source that was replaced, is dropped. I/O: source reads.
   */
  private async readChildren(node: NavigationNode): Promise<void> {
    const source = this.options?.source;
    if (!source || this.busy.has(node.node)) return;
    const generation = this.generation;
    this.busy.add(node.node);
    this.render();
    try {
      const nodes = (await readLevel(source, node.node)).nodes;
      if (generation === this.generation && source === this.options?.source) {
        this.loaded.set(node.node, nodes);
      }
    } catch (error) {
      console.warn("FileSquid could not open a navigation node", error);
      this.opened.delete(node.node);
    } finally {
      this.busy.delete(node.node);
      this.render();
    }
  }

  /** Clicks on a twisty, a Move button, the Pin toggle or a Remove button. */
  private onClick(event: Event): void {
    const target = event.target as Element;
    const twisty = target.closest(".twisty.parent");
    if (twisty) {
      event.preventDefault();
      this.toggle(twisty.closest<HTMLElement>('[role="treeitem"]'));
      return;
    }
    const button = target.closest<HTMLButtonElement>("button");
    const section = button?.closest<HTMLElement>("section")?.dataset.section;
    if (button && section !== undefined) this.press(button, section);
  }

  /** Apply a pressed section control. */
  private press(button: HTMLButtonElement, section: string): void {
    const pin = pinOf(button);
    if (button.classList.contains("pin") && pin) {
      this.store?.setPinned(pin, button.getAttribute("aria-pressed") !== "true");
    } else if (button.classList.contains("remove") && pin) {
      this.store?.setPinned(pin, false);
    } else if (button.dataset.delta && button.getAttribute("aria-disabled") !== "true") {
      this.moveSection(section, Number(button.dataset.delta) as -1 | 1);
    }
  }

  /** Move a section among the sections shown now. */
  private moveSection(section: string, delta: -1 | 1): void {
    if (!this.data || !this.store) return;
    const { labels } = this.options as NavTreeOptions;
    const defaults = { ...this.store.preferences, order: [] };
    const ids = sectionViews(this.data, this.pins, defaults, labels).map((view) => view.id);
    this.store.moveSection(section, delta, ids);
  }

  /** Open a closed parent item, or close an open one. */
  private toggle(item: HTMLElement | null): void {
    const node = item?.dataset.node ? this.rendered.get(item.dataset.node) : undefined;
    if (item && node) this.setOpen(node, item.getAttribute("aria-expanded") !== "true");
  }

  /** Alt+Shift+Up/Down moves the section; unmodified treeview keys move in the tree. */
  private onKeydown(event: KeyboardEvent): void {
    const target = event.target as Element;
    const section = target.closest<HTMLElement>("section")?.dataset.section;
    const vertical = event.key === "ArrowUp" || event.key === "ArrowDown";
    if (event.altKey && event.shiftKey && vertical && section !== undefined) {
      event.preventDefault();
      this.moveSection(section, event.key === "ArrowUp" ? -1 : 1);
      return;
    }
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    const item = target.closest<HTMLElement>('[role="treeitem"]');
    if (item) this.treeKey(event, item);
  }

  /** Apply one treeview key on an item. */
  private treeKey(event: KeyboardEvent, item: HTMLElement): void {
    const tree = item.closest('[role="tree"]');
    if (!tree) return;
    const items = [...tree.querySelectorAll<HTMLElement>('[role="treeitem"]')];
    const rtl = getComputedStyle(this).direction === "rtl";
    const action = treeKeyAction(event.key, items.indexOf(item), keyItems(items), rtl);
    if (action.type === "none") return;
    event.preventDefault();
    const nodeAt = (i: number) => this.rendered.get(items[i]?.dataset.node ?? "");
    if (action.type === "focus") this.focusItem(items[action.index]);
    const targets = action.type === "expand" ? action.indexes : [];
    for (const node of targets.map(nodeAt)) if (node) this.setOpen(node, true);
    const closing = action.type === "collapse" ? nodeAt(action.index) : undefined;
    if (closing) this.setOpen(closing, false);
  }

  /** Remember a focused item as its section's Tab stop. */
  private onFocusin(event: Event): void {
    const item = (event.target as Element).closest?.<HTMLElement>('[role="treeitem"]');
    if (item) this.makeActive(item);
  }

  /** Move focus to an item and make it its section's Tab stop. Side effect: changes focus. */
  private focusItem(item: HTMLElement | undefined): void {
    if (!item) return;
    this.makeActive(item);
    focusElement(item);
  }

  /** Give `item` the section's only `tabindex="0"` and word the Pin toggle for it. */
  private makeActive(item: HTMLElement): void {
    const section = item.closest<HTMLElement>("section");
    const node = item.dataset.node ? this.rendered.get(item.dataset.node) : undefined;
    if (!section?.dataset.section || !node) return;
    this.active.set(section.dataset.section, node.node);
    for (const other of section.querySelectorAll<HTMLElement>('[role="treeitem"]')) {
      other.tabIndex = other === item ? 0 : -1;
    }
    const pin = section.querySelector<HTMLButtonElement>("button.pin");
    const pinned = this.store?.preferences.pinned.some((p) => p.node === node.node) ?? false;
    if (pin) describePin(pin, (this.options as NavTreeOptions).labels, node, pinned);
  }
}

/** The handle and kind a Pin or Remove button acts on. */
function pinOf(button: HTMLButtonElement): PinnedDestination | undefined {
  const { node, kind } = button.dataset;
  return node && kind ? { node, kind: kind as NavigationKind } : undefined;
}

/** Describe rendered items for the key rules: name, parent index and open state. */
function keyItems(items: readonly HTMLElement[]): KeyItem[] {
  const index = new Map(items.map((item, i) => [item.dataset.node, i]));
  return items.map((item) => {
    const expanded = item.getAttribute("aria-expanded");
    return {
      name: item.querySelector(".name")?.textContent ?? "",
      parent: item.dataset.parent === undefined ? -1 : (index.get(item.dataset.parent) ?? -1),
      expanded: expanded === null ? undefined : expanded === "true",
    };
  });
}
