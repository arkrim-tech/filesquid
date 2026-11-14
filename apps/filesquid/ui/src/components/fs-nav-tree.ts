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
 * Window (Task UI-F3.4, UI-S-035): a section's tree is as tall as all its rows but holds only
 * the rows in view plus an overscan of 5 above and below, however many nodes its open levels
 * hold. The element does not scroll itself; a {@link TreeViewport} tells it which part of each
 * tree is in view and when that changed. A level shows at most its first 10,000 nodes and then
 * says so in a line under the last one (UI-S-017).
 *
 * Changes (UI-F2.1b, `navigation.change`): a change naming parents reads only the loaded levels
 * it touches (each named parent, and the shown level holding each named node); `all`, the
 * repository, or a change before the first answer reads the whole tree again.
 *
 * Accessibility (UI-S-016): each section has a heading, Move up and Move down buttons (also
 * Alt+Shift+Up and Alt+Shift+Down anywhere in the section; `aria-disabled` at the ends so focus
 * stays put), a WAI-ARIA treeview with one Tab stop and the treeview keys, and a Pin toggle for
 * the tree's active item. The keys read the rows, not the DOM, so they reach every node of an
 * open level; a node that is not drawn is drawn before it takes focus, and the item that has
 * focus stays drawn until focus leaves it. The Tab stop is the active item when it is drawn,
 * else the current destination, else the first row in view. When a re-render removes the
 * focused control, focus returns to the same control in the new render, else to the first Tab
 * stop; focus never moves when it was outside the tree.
 */
import type { LabelTable } from "../theme/labels";
import {
  isNotFound,
  navigationHref,
  nodeName,
  type NavigationChange,
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
  type NavigationLevel,
  type ResolvedPin,
} from "../navigation/loader";
import { sharedNavigationStore, type NavigationStore } from "../navigation/store";
import { A11Y_STYLE } from "./a11y-style";
import { focusElement, recoverFocus } from "../a11y/focus";
import { failureState, type ViewState } from "./view-state";
import { RETRY_EVENT, type FsStates } from "./fs-states";
import { treeKeyAction } from "./nav-tree-keys";
import { TreeIndex } from "./nav-tree-index";
import { TreeList } from "./nav-tree-list";
import { sectionRows } from "./nav-tree-rows";
import { domViewport, type TreeViewport } from "./nav-tree-viewport";
import { TREE_OVERSCAN, treeWindow } from "./nav-tree-window";
import {
  describePin,
  rowElement,
  sectionBase,
  sectionElement,
  sectionViews,
  treeElement,
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
  /** Where the tree is shown; defaults to the window and the tree's scrolling ancestors. */
  viewport?: TreeViewport;
}

/** Kinds open by default; the owner's collapse of them is kept (UI-S-017). */
const OPEN_BY_DEFAULT: ReadonlySet<NavigationKind> = new Set(["repository", "library"]);

/** Most deeper nodes kept open in one session; the oldest closes first. */
const MAX_SESSION_OPEN = 256;

const STYLE = `
  :host { display: block; --row-height: 1.875rem; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .sections { display: flex; flex-direction: column; gap: var(--fs-space-3); }
  section { display: flex; flex-direction: column; gap: var(--fs-space-2); min-width: 0; }
  .head { display: flex; align-items: center; gap: var(--fs-space-1); padding: 0 var(--fs-space-2) 0 var(--fs-space-3); }
  h2 { flex: 1 1 auto; min-width: 0; margin: 0; font-size: var(--fs-font-size-eyebrow); font-weight: 400; letter-spacing: 0.1em; text-transform: uppercase; color: var(--fs-on-surface-muted); overflow-wrap: anywhere; }
  button { border: 0; border-radius: var(--fs-radius-item); background: none; color: var(--fs-icon-muted); font: inherit; cursor: pointer; transition: background-color var(--fs-motion-duration), color var(--fs-motion-duration); }
  button:hover { background: var(--fs-surface-raised); color: var(--fs-icon); }
  .up, .down { display: inline-flex; flex: 0 0 auto; padding: var(--fs-space-1); }
  button[aria-disabled="true"] { color: var(--fs-icon-disabled); cursor: default; }
  button[aria-disabled="true"]:hover { background: none; color: var(--fs-icon-disabled); }
  .pin, .remove { align-self: flex-start; padding: 0 var(--fs-space-3); color: var(--fs-primary); font-size: var(--fs-font-size-ui); font-weight: 600; line-height: 1.875rem; }
  .pin:hover, .remove:hover { background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  .pin[aria-pressed="true"] { background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  ul { list-style: none; margin: 0; padding: 0; }
  [role="tree"] { position: relative; block-size: calc(var(--rows, 0) * var(--row-height)); }
  [role="tree"] > li { position: absolute; inset-inline: 0; inset-block-start: calc(var(--r, 0) * var(--row-height)); block-size: var(--row-height); }
  [role="tree"] > li.limit { display: flex; align-items: center; box-sizing: border-box; block-size: calc(2 * var(--row-height)); padding: 0 var(--fs-space-2) 0 var(--fs-space-3); overflow: hidden; color: var(--fs-on-surface-muted); line-height: 1.3; }
  .unavailable li { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: var(--fs-space-2); min-block-size: 1.875rem; padding: 0 var(--fs-space-2) 0 var(--fs-space-3); color: var(--fs-on-surface-muted); overflow-wrap: anywhere; }
  .unavailable .kind-icon { color: var(--fs-icon-muted); }
  .unavailable .name { display: inline-flex; align-items: center; gap: var(--fs-space-2); }
  [hidden] { display: none !important; }
  a { display: flex; align-items: center; gap: var(--fs-space-2); box-sizing: border-box; block-size: 100%; min-block-size: 1.875rem; padding: 0 var(--fs-space-2) 0 var(--fs-space-3); color: var(--fs-on-surface-variant); font-weight: 400; text-decoration: none; white-space: nowrap; transition: background-color var(--fs-motion-duration), color var(--fs-motion-duration); }
  a[data-kind="library"], a[data-kind="repository"] { color: var(--fs-on-surface); font-weight: 500; }
  a:hover { background: var(--fs-surface-raised); }
  a[aria-current="page"] { position: relative; background: var(--fs-primary-container); color: var(--fs-on-primary-container); font-weight: 500; }
  a[aria-current="page"]::before { content: ""; position: absolute; inset-block: 0; inset-inline-start: 0; inline-size: 2.5px; background: var(--fs-brand); }
  a[aria-current="page"] .kind-icon { color: var(--fs-primary); }
  a[aria-busy="true"] { color: var(--fs-on-surface-muted); }
  .kind-icon { flex: 0 0 auto; color: var(--fs-icon); }
  .name { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .twisty { display: inline-flex; flex: 0 0 auto; inline-size: 0.875rem; color: var(--fs-icon-muted); }
  .twisty svg { display: none; inline-size: 0.875rem; block-size: 0.875rem; transition: transform var(--fs-motion-duration); }
  .twisty.parent { cursor: pointer; }
  .twisty.parent svg { display: block; }
  .twisty.parent:dir(rtl) svg { transform: scaleX(-1); }
  [aria-expanded="true"] > .twisty.parent svg { transform: rotate(90deg); }
  .empty { margin: 0; padding: 0 var(--fs-space-3); color: var(--fs-on-surface-muted); font-size: var(--fs-font-size-small); }
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
  /** The parents among `loaded` whose level stopped at its cap (UI-S-017). */
  private readonly capped = new Set<string>();
  private readonly pinCache = new Map<string, ResolvedPin>();
  private pins: ResolvedPin[] = [];
  /** Deeper nodes opened this session, oldest first. */
  private readonly opened = new Set<string>();
  private readonly busy = new Set<string>();
  /** Each section's active item handle (its Tab stop), kept across re-renders. */
  private readonly active = new Map<string, string>();
  /** The nodes the last render shows, drawn or not, by handle. */
  private rendered = new Map<string, NavigationNode>();
  /** Each section's tree of the last render: its rows and the ones drawn now. */
  private readonly lists = new Map<string, TreeList>();
  /** Where the tree is shown, and the function that stops following it. */
  private viewport?: TreeViewport;
  private stopViewport?: () => void;
  /** The drawn treeitems, so the current destination moves without a search. */
  private readonly index = new TreeIndex();
  private generation = 0;
  /** The generation of the last whole-tree read that finished; a read is in flight otherwise. */
  private settled = 0;
  /** The newest read token of each level re-read for a change. */
  private readonly levelTokens = new Map<string, number>();
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
    this.viewport = options.viewport ?? domViewport(this);
    this.data = undefined;
    if (this.isConnected) this.connect();
  }

  /** Handle of the current destination, marked with `aria-current="page"`. */
  get current(): string | undefined {
    return this.currentId;
  }

  /**
   * Mark another destination as current. The tree is not rebuilt: only `aria-current`, each
   * section's Tab stop and its Pin toggle depend on the current destination, so they move in
   * the rows already drawn (Task UI-F3.1). A row drawn later takes `aria-current` as it is
   * built. The tree is drawn from scratch only when none is drawn yet.
   */
  set current(node: string | undefined) {
    this.currentId = node;
    if (!this.markCurrentInPlace()) this.render();
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
    this.settled = generation;
    this.render();
  }

  /** Subscribe to the store, the source and the viewport, then read the tree. */
  private connect(): void {
    const options = this.options;
    if (!options || !this.store) return;
    this.disconnect();
    this.store.addEventListener("change", this.onStoreChange);
    this.stopFollowing = options.source.subscribe?.((change) => this.onChange(change));
    this.stopViewport = this.viewport?.follow(() => this.drawWindows());
    void this.reload();
  }

  /**
   * Follow one change: `all`, a change naming the repository (its libraries changed), a change
   * before the first answer, or one while a whole-tree read is in flight (its answer may predate
   * the change) reads the whole tree; otherwise only the levels the named parents touch are read
   * again.
   */
  private onChange(change: NavigationChange): void {
    const repository = this.data?.repository?.node;
    if (
      "all" in change ||
      !this.data ||
      this.failure ||
      this.settled !== this.generation ||
      change.parents.includes(repository ?? "")
    ) {
      void this.reload();
      return;
    }
    void this.refreshParents(change.parents);
  }

  /**
   * The loaded levels a change touches: each named parent whose children are loaded, and the
   * level that shows each named node (its `has_children` or name may have changed). A named
   * node that is not on screen touches nothing.
   */
  private levelsTouched(parents: readonly string[]): string[] {
    const named = new Set(parents);
    const levels = new Set<string>();
    for (const [parent, children] of this.loaded) {
      if (named.has(parent) || children.some((child) => named.has(child.node))) levels.add(parent);
    }
    return [...levels];
  }

  /**
   * Read the levels a change touches again and show them. An answer that arrives after a
   * newer load started, or from a source that was replaced, is dropped. I/O: source reads, at
   * most one per loaded level.
   */
  private async refreshParents(parents: readonly string[]): Promise<void> {
    const source = this.options?.source;
    const levels = this.levelsTouched(parents);
    if (!source || levels.length === 0) return;
    const generation = this.generation;
    const tokens = levels.map((parent) => this.nextLevelToken(parent));
    const answers = await Promise.allSettled(levels.map((parent) => readLevel(source, parent)));
    if (generation !== this.generation || source !== this.options?.source) return;
    answers.forEach((answer, i) => {
      const parent = levels[i] as string;
      if (this.levelTokens.get(parent) === tokens[i]) this.acceptLevel(parent, answer);
    });
    this.syncLibraryChildren();
    this.syncPins();
  }

  /** Take a new read token for one level; only the newest read of a level is kept. */
  private nextLevelToken(parent: string): number {
    const token = (this.levelTokens.get(parent) ?? 0) + 1;
    this.levelTokens.set(parent, token);
    return token;
  }

  /** Give the sections each library's children as loaded now. */
  private syncLibraryChildren(): void {
    const data = this.data;
    if (!data) return;
    const children = new Map(data.children);
    for (const library of data.libraries) {
      const nodes = this.loaded.get(library.node);
      if (nodes) children.set(library.node, nodes);
      else children.delete(library.node);
    }
    const capped = new Set([...this.capped].filter((parent) => children.has(parent)));
    this.data = { ...data, children, capped };
  }

  /**
   * Keep one re-read level: new children replace the old, a `not_found` level closes (its
   * node is gone or no longer readable), any other failure is logged and keeps the old
   * children. Pins among the children take the new names; pins that left the level are read
   * again.
   */
  private acceptLevel(parent: string, answer: PromiseSettledResult<NavigationLevel>): void {
    const before = this.loaded.get(parent) ?? [];
    if (answer.status === "rejected" && !isNotFound(answer.reason)) {
      console.warn("FileSquid could not read a changed navigation level", answer.reason);
      return;
    }
    const after = answer.status === "fulfilled" ? answer.value.nodes : [];
    if (answer.status === "fulfilled") this.keepLevel(parent, answer.value);
    else this.closeLevel(parent);
    const kept = new Map(after.map((node) => [node.node, node]));
    for (const node of before) if (!kept.has(node.node)) this.forgetPin(node.node);
    for (const node of after) this.renamePin(node);
  }

  /** Keep a level as read: its nodes, and whether it stopped at its cap. */
  private keepLevel(parent: string, level: NavigationLevel): void {
    this.loaded.set(parent, level.nodes);
    if (level.capped) this.capped.add(parent);
    else this.capped.delete(parent);
  }

  /** Close a level whose node is gone. */
  private closeLevel(parent: string): void {
    this.loaded.delete(parent);
    this.capped.delete(parent);
    this.opened.delete(parent);
  }

  /** Forget what is known of a node that left its level, so a pin of it is read again. */
  private forgetPin(handle: string): void {
    this.pinCache.delete(handle);
    this.pinsTried.delete(handle);
    this.rendered.delete(handle);
  }

  /** Give a known pin the node as it was just listed. */
  private renamePin(node: NavigationNode): void {
    const known = this.pinCache.get(node.node);
    if (known) this.pinCache.set(node.node, { pin: known.pin, node });
  }

  /** Stop following the store, the source and the viewport. */
  private disconnect(): void {
    this.store?.removeEventListener("change", this.onStoreChange);
    this.stopFollowing?.();
    this.stopFollowing = undefined;
    this.stopViewport?.();
    this.stopViewport = undefined;
  }

  /** Keep a new answer; the pins it asked for count as tried until the next store change. */
  private accept(
    data: NavigationData,
    pins: readonly ResolvedPin[],
    reopened: ReadonlyMap<string, NavigationLevel>,
    asked: readonly PinnedDestination[],
  ): void {
    this.data = data;
    this.loaded.clear();
    this.capped.clear();
    for (const [parent, nodes] of data.children) this.loaded.set(parent, nodes);
    for (const parent of data.capped) this.capped.add(parent);
    for (const [parent, level] of reopened) this.keepLevel(parent, level);
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
  private async readOpened(source: NavigationSource): Promise<Map<string, NavigationLevel>> {
    const opened = [...this.opened];
    const levels = await Promise.allSettled(opened.map((node) => readLevel(source, node)));
    const result = new Map<string, NavigationLevel>();
    levels.forEach((level, i) => {
      if (level.status === "fulfilled") result.set(opened[i] as string, level.value);
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

  /**
   * Show the sections when there is an answer, otherwise the state; draw each tree's window;
   * recover focus if needed. Side effects: replaces the sections and may move focus.
   */
  private render(): void {
    const root = this.shadowRoot;
    const options = this.options;
    if (!root || !options || !this.store || !this.isConnected) return;
    const { states, box } = this.skeleton(root, options.labels);
    const hadFocus = root.activeElement !== null;
    const key = focusedKey(root);
    const parked = this.focusParked(root);
    const views = this.failure || !this.data ? undefined : this.views(this.data);
    const state = this.viewState(views);
    states.hidden = state === undefined;
    if (state) states.state = state;
    box.hidden = state !== undefined;
    this.lists.clear();
    box.replaceChildren(...(state || !views ? [] : this.sections(views)));
    if (state || !views) this.index.clear();
    else this.drawWindows(key);
    // An item that was scrolled out of view keeps focus without scrolling the tree back to it.
    if (parked && root.activeElement === null)
      sameControl(root, key)?.focus({ preventScroll: true });
    recoverFocus(root, hadFocus, () => this.focusTarget(root, key, states));
  }

  /**
   * Draw the window of every section's tree: the rows in view plus the overscan, and the row
   * that has focus (or had it before a re-render, named by `keep`). Then place each tree's Tab
   * stop and index the drawn items. Side effects: reads layout, then adds and removes rows.
   *
   * @param keep The `data-key` of the control that had focus before the sections were replaced.
   */
  private drawWindows(keep?: string): void {
    const root = this.shadowRoot;
    const box = root?.querySelector<HTMLElement>(".sections");
    if (!root || !box || !this.viewport || this.lists.size === 0) return;
    const lists = [...this.lists.values()];
    const rowHeight = this.viewport.rowHeight();
    const bands = this.viewport.bands(lists.map((list) => list.element));
    const focused = focusedKey(root) ?? keep;
    lists.forEach((list, at) => {
      const band = bands[at] ?? { top: 0, bottom: 0 };
      const count = list.rows.length;
      const range = treeWindow({ count, rowHeight, ...band, overscan: TREE_OVERSCAN });
      list.draw(range, [list.rowOf(itemHandle(list.id, focused))]);
    });
    this.index.build(box);
    for (const list of lists) this.placeStop(list);
  }

  /**
   * Whether focus is on a tree item that was scrolled out of its window and is drawn only
   * because it has focus.
   */
  private focusParked(root: ShadowRoot): boolean {
    const item = root.activeElement?.closest<HTMLElement>('[role="treeitem"]');
    const list = item ? this.listOf(item) : undefined;
    return (
      item !== undefined && item !== null && list !== undefined && !list.inWindow(rowIndex(item))
    );
  }

  /** The tree that holds an item. */
  private listOf(item: Element): TreeList | undefined {
    return this.lists.get(item.closest<HTMLElement>("section")?.dataset.section ?? "");
  }

  /**
   * Move `aria-current`, the Tab stops and the Pin toggles to follow the current destination,
   * in the rows already drawn. Side effects: sets attributes of a few drawn nodes.
   *
   * @returns False when no tree is drawn (the caller renders), true when the drawn one follows.
   */
  private markCurrentInPlace(): boolean {
    if (this.index.empty || !this.store) return false;
    this.index.mark(this.currentId);
    for (const id of this.index.sectionIds()) this.followCurrent(id);
    return true;
  }

  /** Place a section's Tab stop and word its Pin toggle for the section's active item. */
  private followCurrent(id: string): void {
    const list = this.lists.get(id);
    if (!list) return;
    this.placeStop(list);
    const node = this.activeNode(list);
    const pin = this.index.section(id)?.querySelector<HTMLButtonElement>("button.pin");
    if (!node || !pin) return;
    const pinned = this.store?.preferences.pinned.some((p) => p.node === node.node) ?? false;
    describePin(pin, (this.options as NavTreeOptions).labels, node, pinned);
  }

  /**
   * Make one drawn item the tree's Tab stop: the item the person moved to, else the current
   * destination, else the first row in view, which stands in while neither is drawn. Side
   * effect: sets `tabindex` of the old and the new stop.
   */
  private placeStop(list: TreeList): void {
    const moved = list.item(list.rowOf(this.active.get(list.id)));
    const stop = moved ?? list.item(list.rowOf(this.currentId)) ?? list.leadItem();
    if (stop) this.index.setStop(list.id, stop);
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

  /**
   * Build every section element with its tree still empty, and remember each tree's rows and
   * the nodes they show. The rows are drawn by {@link drawWindows}.
   */
  private sections(views: readonly SectionView[]): HTMLElement[] {
    const ctx = this.context();
    this.rendered = new Map();
    return views.map((view, index) => {
      const list = this.treeList(view, index, ctx);
      if (list) this.lists.set(view.id, list);
      const place = { index, count: views.length, active: list && this.activeNode(list) };
      return sectionElement(view, ctx, place, list?.element);
    });
  }

  /** The tree of a section that has items: its rows and its still empty `ul[role=tree]`. */
  private treeList(view: SectionView, index: number, ctx: TreeContext): TreeList | undefined {
    if (view.items.length === 0) return undefined;
    const rows = sectionRows(view, ctx);
    for (const row of rows) if (row.type === "node") this.rendered.set(row.node.node, row.node);
    const base = sectionBase(index);
    const scope = { section: view.id, base, locale: documentLocale() };
    // A row is built when it comes into the window, so it reads the tree's state as it is then.
    const build = (at: number) => rowElement(rows, at, { ...scope, ctx: this.context() });
    return new TreeList(view.id, treeElement(rows.length, base), rows, build);
  }

  /** A section's active item: the one the person moved to, else the current one, else the first. */
  private activeNode(list: TreeList): NavigationNode | undefined {
    const moved = list.rowOf(this.active.get(list.id));
    const current = list.rowOf(this.currentId);
    return list.nodeAt(moved >= 0 ? moved : Math.max(0, current));
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
      limitOf: (node) =>
        this.capped.has(node.node) ? this.loaded.get(node.node)?.length : undefined,
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

  /** Where focus goes after a re-render removed it: the same control, else a fallback. */
  private focusTarget(root: ShadowRoot, key: string | undefined, states: FsStates): HTMLElement {
    const same = sameControl(root, key);
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
    if (open) this.markOpened(node);
    else this.opened.delete(node.node);
    if (open && !this.loaded.has(node.node)) void this.readChildren(node);
    else this.render();
  }

  /**
   * Open several parents at once (the `*` key): the kept choices change for repositories and
   * libraries, at most {@link MAX_SESSION_OPEN} deeper nodes open for this session, the tree is
   * drawn once, and the children of the unread ones are read together and drawn once more when
   * all have answered. I/O: source reads.
   */
  private async openAll(nodes: readonly NavigationNode[]): Promise<void> {
    const opening = nodes.filter((node) => !OPEN_BY_DEFAULT.has(node.kind));
    const kept = opening.slice(0, MAX_SESSION_OPEN);
    for (const node of nodes) {
      if (OPEN_BY_DEFAULT.has(node.kind)) this.store?.setCollapsed(node.node, false);
    }
    for (const node of kept) this.markOpened(node);
    const unread = kept.filter((node) => !this.loaded.has(node.node) && !this.busy.has(node.node));
    for (const node of unread) this.busy.add(node.node);
    this.render();
    if (unread.length === 0) return;
    await Promise.allSettled(unread.map((node) => this.fetchChildren(node)));
    this.render();
  }

  /** Open a deeper node for this session; past {@link MAX_SESSION_OPEN} the oldest closes. */
  private markOpened(node: NavigationNode): void {
    this.opened.delete(node.node);
    this.opened.add(node.node);
    if (this.opened.size > MAX_SESSION_OPEN)
      this.opened.delete(this.opened.values().next().value as string);
  }

  /**
   * Read a node's children, marking it busy meanwhile, and draw the tree before and after.
   * I/O: source reads.
   */
  private async readChildren(node: NavigationNode): Promise<void> {
    if (!this.options?.source || this.busy.has(node.node)) return;
    this.busy.add(node.node);
    this.render();
    await this.fetchChildren(node);
    this.render();
  }

  /**
   * Read the children of a node the caller marked busy, and keep them. An answer that arrives
   * after a newer load started, or from a source that was replaced, is dropped; a failure is
   * logged and closes the node. Draws nothing. I/O: source reads.
   */
  private async fetchChildren(node: NavigationNode): Promise<void> {
    const source = this.options?.source;
    const generation = this.generation;
    try {
      if (!source) return;
      const level = await readLevel(source, node.node);
      if (generation === this.generation && source === this.options?.source) {
        this.keepLevel(node.node, level);
      }
    } catch (error) {
      console.warn("FileSquid could not open a navigation node", error);
      this.opened.delete(node.node);
    } finally {
      this.busy.delete(node.node);
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

  /**
   * Apply one treeview key on an item. The key rules read the section's rows, so a move, an
   * expand or a collapse reaches a node whether it is drawn or not (UI-S-035).
   */
  private treeKey(event: KeyboardEvent, item: HTMLElement): void {
    const list = this.listOf(item);
    if (!list) return;
    const { labels } = this.options as NavTreeOptions;
    const keys = list.keys((node) => nodeName(node, labels));
    const rtl = getComputedStyle(this).direction === "rtl";
    const action = treeKeyAction(event.key, keys.at(rowIndex(item)), keys.items, rtl);
    if (action.type === "none") return;
    event.preventDefault();
    const nodeAt = (i: number) => list.nodeAt(keys.rows[i] ?? -1);
    if (action.type === "focus") this.focusRow(list, keys.rows[action.index] ?? -1);
    const targets = action.type === "expand" ? action.indexes : [];
    const opening = targets.flatMap((i) => nodeAt(i) ?? []);
    if (opening.length === 1) this.setOpen(opening[0] as NavigationNode, true);
    else if (opening.length > 1) void this.openAll(opening);
    const closing = action.type === "collapse" ? nodeAt(action.index) : undefined;
    if (closing) this.setOpen(closing, false);
  }

  /** Remember a focused item as its section's Tab stop. */
  private onFocusin(event: Event): void {
    const item = (event.target as Element).closest?.<HTMLElement>('[role="treeitem"]');
    if (item) this.makeActive(item);
  }

  /**
   * Move focus to a row and make it its section's Tab stop. A row outside the window is drawn
   * first; the browser scrolls the focused item into view, and the window is drawn again where
   * the view is then. Side effects: may add a row, changes focus, redraws the windows.
   */
  private focusRow(list: TreeList, row: number): void {
    const item = list.ensure(row);
    if (!item) return;
    this.makeActive(item);
    focusElement(item);
    this.drawWindows();
  }

  /** Give `item` the section's only `tabindex="0"` and word the Pin toggle for it. */
  private makeActive(item: HTMLElement): void {
    const section = item.closest<HTMLElement>("section");
    const node = item.dataset.node ? this.rendered.get(item.dataset.node) : undefined;
    if (!section?.dataset.section || !node) return;
    this.active.set(section.dataset.section, node.node);
    this.index.setStop(section.dataset.section, item);
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

/** The `data-key` of the control that has focus inside the tree, if any. */
function focusedKey(root: ShadowRoot): string | undefined {
  return root.activeElement?.closest<HTMLElement>("[data-key]")?.dataset.key;
}

/** The shown control that carries a `data-key`, if any. */
function sameControl(root: ShadowRoot, key: string | undefined): HTMLElement | undefined {
  if (key === undefined) return undefined;
  return [...root.querySelectorAll<HTMLElement>("[data-key]")].find(
    (el) => el.dataset.key === key && !el.closest("[hidden]"),
  );
}

/** The node handle in the `data-key` of a section's tree item, else undefined. */
function itemHandle(section: string, key: string | undefined): string | undefined {
  const prefix = `item:${section}:`;
  return key?.startsWith(prefix) ? key.slice(prefix.length) : undefined;
}

/** The row index of a drawn tree item, or -1. */
function rowIndex(item: Element): number {
  const index = Number(item.closest<HTMLElement>("li")?.dataset.index ?? -1);
  return Number.isInteger(index) ? index : -1;
}

/** The BCP 47 tag numbers are formatted in: the document's language, else English. */
function documentLocale(): string {
  return document.documentElement.lang || "en";
}
