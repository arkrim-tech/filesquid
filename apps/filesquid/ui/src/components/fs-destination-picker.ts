/**
 * `<fs-destination-picker>`: the tree of places a file can be moved or copied into (UI-F2.8,
 * UI-S-002 **Destination**, UI-S-026, mockup 13, ADR 041). It reads the navigation listing
 * (API-S-021) and offers only collections and the folders inside them: never a library's top
 * level, a virtual collection or Recently deleted, because those are not destinations. UI-F2.17
 * reuses it for uploads and imports, so it knows nothing about move or copy; it announces the
 * choice with a bubbling, composed `fs-destination-change` event whose `detail` is the
 * {@link DestinationChoice}, or `null` when the choice is cleared.
 *
 * A level is read page by page through `next_cursor`, at most {@link MAX_PAGES} pages, and the
 * tree is read no deeper than {@link MAX_DEPTH}. Keys follow the ARIA tree pattern: Up and Down
 * move, Right expands, Left collapses, Enter and Space choose. A node whose read fails shows the
 * error state of `<fs-states>` for the whole tree with Try again.
 */
import { failureState, type ViewState } from "./view-state";
import {
  MAX_DEPTH,
  MAX_PAGES,
  type NavigationKind,
  type NavigationNode,
  type NavigationSource,
  nodeName,
} from "../navigation/model";
import type { LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { RETRY_EVENT, type FsStates } from "./fs-states";
import { icon } from "./icons";

/** A place chosen as a destination. */
export interface DestinationChoice {
  /** The node handle of the collection or folder. */
  readonly node: string;
  /** Its kind: `collection` or `folder`. */
  readonly kind: NavigationKind;
  /** The names from the library down to it, for the Destination line. */
  readonly trail: readonly string[];
}

/** What the picker needs. */
export interface DestinationPickerOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** Answers the navigation listing. */
  readonly source: NavigationSource;
  /** The node the files are in now, marked "Current folder". */
  readonly current?: string;
  /** Whether the browser is online; defaults to `navigator.onLine`. */
  readonly online?: () => boolean;
  /**
   * `import` draws the tree as the upload dialog does (UI-1.2, mockup 52): rows 31 px high, a
   * 1.5 px border and 8 px radius, and "Not a destination" at the right of each library row.
   */
  readonly variant?: "import";
}

/** One visible row of the tree. */
interface Row {
  readonly node: NavigationNode;
  readonly depth: number;
  readonly trail: readonly string[];
  readonly selectable: boolean;
}

const STYLE = `
  :host { display: block; min-height: 0; color: var(--fs-on-surface); font-family: var(--fs-font-sans); font-size: var(--fs-font-size-small); }
  ul { list-style: none; margin: 0; padding: var(--fs-space-1); border: 1px solid var(--fs-outline-strong); border-radius: var(--fs-radius); background: var(--fs-surface-card); overflow: auto; max-block-size: 100%; }
  li { display: flex; align-items: center; gap: var(--fs-space-2); box-sizing: border-box; min-block-size: 2.125rem; padding: 0 var(--fs-space-2); border-radius: var(--fs-radius-item); cursor: pointer; }
  li:hover { background: var(--fs-surface-raised); }
  li[aria-selected="true"] { background: var(--fs-primary-container); color: var(--fs-on-primary-container); font-weight: 500; }
  li[data-selectable="false"] { color: var(--fs-on-surface-variant); cursor: default; font-weight: 500; }
  li:focus-visible { outline: 2px solid var(--fs-focus-ring); outline-offset: -2px; }
  .chevron { display: inline-flex; inline-size: 1rem; flex: 0 0 auto; color: var(--fs-icon-muted); }
  li[aria-expanded="true"] .chevron .icon { transform: rotate(90deg); }
  .name { flex: 1 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .current { color: var(--fs-on-surface-muted); }
  .empty { margin: 0; padding: var(--fs-space-3); color: var(--fs-on-surface-variant); }
  .not-destination { margin-inline-start: auto; font-weight: 400; color: var(--fs-on-surface-muted); }
  :host([data-variant="import"]) { block-size: 100%; }
  :host([data-variant="import"]) ul { box-sizing: border-box; block-size: 100%; border-width: 1.5px; }
  :host([data-variant="import"]) li { min-block-size: 1.9375rem; }
  :host([data-variant="import"]) li[data-selectable="false"] { color: var(--fs-on-surface-muted); }
  [hidden] { display: none !important; }
`;

/** True for the kinds a file can be placed in. */
function isPlace(kind: NavigationKind): boolean {
  return kind === "collection" || kind === "folder";
}

/** Custom element that offers collections and folders as destinations. */
export class FsDestinationPicker extends HTMLElement {
  private options?: DestinationPickerOptions;
  private levels = new Map<string, readonly NavigationNode[]>();
  private expanded = new Set<string>();
  private readonly reading = new Map<string, Promise<void>>();
  private roots: readonly NavigationNode[] = [];
  private chosen?: DestinationChoice;
  private generation = 0;
  private loading: Promise<void> = Promise.resolve();

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.addEventListener(RETRY_EVENT, (event) => {
      event.stopPropagation();
      void this.load();
    });
  }

  /**
   * Set the labels and source and read the tree. Call before or after connecting.
   *
   * @param options Labels, source and the current node.
   */
  configure(options: DestinationPickerOptions): void {
    this.options = options;
    if (options.variant) this.dataset.variant = options.variant;
    else delete this.dataset.variant;
    this.clearChoice();
    if (this.isConnected) this.loading = this.load();
  }

  /** Forget the chosen destination (the new tree may not hold it) and announce that. */
  private clearChoice(): void {
    if (!this.chosen) return;
    this.chosen = undefined;
    this.announce(undefined);
  }

  /** Announce the choice; the event's `detail` is null when it is cleared. */
  private announce(choice: DestinationChoice | undefined): void {
    this.dispatchEvent(
      new CustomEvent("fs-destination-change", {
        detail: choice ?? null,
        bubbles: true,
        composed: true,
      }),
    );
  }

  /** The chosen destination, or undefined. */
  get selected(): DestinationChoice | undefined {
    return this.chosen;
  }

  /**
   * Open the tree down to a collection or folder and choose it, so a dialog opened from a view of
   * that place starts with it as the destination (UI-1.2). Network I/O via the source for the
   * levels on the way.
   *
   * @param node The node handle of the collection or folder.
   * @returns Whether the node was found and chosen; false leaves the choice empty.
   */
  async preselect(node: string): Promise<boolean> {
    const source = this.options?.source;
    if (!source) return false;
    await this.loading;
    const generation = this.generation;
    try {
      const { ancestors } = await source.list(node);
      for (const ancestor of ancestors) {
        if (ancestor.node === node || ancestor.kind === "repository") continue;
        if (ancestor.has_children) await this.expand(ancestor.node);
      }
    } catch (error) {
      if (generation === this.generation)
        this.showFailure("FileSquid could not read a destination", error);
      return false;
    }
    if (generation !== this.generation) return false;
    this.choose(node);
    return this.chosen?.node === node;
  }

  /** Read the tree when attached. */
  connectedCallback(): void {
    if (this.options && this.roots.length === 0) this.loading = this.load();
  }

  /** Drop answers still in flight once detached. */
  disconnectedCallback(): void {
    this.generation += 1;
  }

  /**
   * Read the top of the tree: the repository, its libraries and the first library's
   * collections (opened by default). Network I/O via the source.
   *
   * @returns Resolves once the tree or its failure is shown.
   */
  async load(): Promise<void> {
    const source = this.options?.source;
    if (!source) return;
    const generation = ++this.generation;
    this.reading.clear();
    this.show({ kind: "loading" });
    try {
      const staged = new Map<string, readonly NavigationNode[]>();
      const repositories = await this.level(null, staged);
      const libraries = (
        await Promise.all(repositories.map((node) => this.level(node.node, staged)))
      ).flat();
      const first = libraries[0];
      if (first) await this.level(first.node, staged);
      if (generation !== this.generation) return;
      this.levels = staged;
      this.roots = libraries;
      this.expanded = new Set(first ? [first.node] : []);
      this.show(undefined);
    } catch (error) {
      if (generation !== this.generation) return;
      this.showFailure("FileSquid could not read the destinations", error);
    }
  }

  /** Log a failed read and show the error state of the whole tree, with Try again. */
  private showFailure(message: string, error: unknown): void {
    console.warn(message, error);
    const online = this.options?.online ?? (() => navigator.onLine);
    this.show(failureState(error, online()));
  }

  /** Read one level through every page, at most {@link MAX_PAGES}, into `into`. */
  private async level(
    parent: string | null,
    into: Map<string, readonly NavigationNode[]>,
  ): Promise<readonly NavigationNode[]> {
    const source = (this.options as DestinationPickerOptions).source;
    const nodes: NavigationNode[] = [];
    let cursor: string | null = null;
    for (let page = 0; page < MAX_PAGES; page += 1) {
      const answer = await source.list(parent, cursor);
      nodes.push(...answer.nodes);
      cursor = answer.next_cursor;
      if (cursor === null) break;
    }
    if (parent !== null) into.set(parent, nodes);
    return nodes;
  }

  /** Read the children of a node, if not read yet, and show it open. */
  private async expand(node: string): Promise<void> {
    const generation = this.generation;
    if (!this.levels.has(node)) await this.readChildren(node);
    if (generation !== this.generation) return;
    this.expanded.add(node);
  }

  /** Read a node's children; a read already running for the node is joined, not repeated. */
  private readChildren(node: string): Promise<void> {
    let running = this.reading.get(node);
    if (!running) {
      const read: Promise<void> = this.fetchChildren(node).finally(() => {
        if (this.reading.get(node) === read) this.reading.delete(node);
      });
      running = read;
      this.reading.set(node, read);
    }
    return running;
  }

  /** One read of a node's children, kept only if the tree was not read again meanwhile. */
  private async fetchChildren(node: string): Promise<void> {
    const generation = this.generation;
    const staged = new Map<string, readonly NavigationNode[]>();
    await this.level(node, staged);
    if (generation !== this.generation) return;
    // Merge into the current map: another node's read may have finished while this one ran.
    this.levels = new Map([...this.levels, ...staged]);
  }

  /** The visible rows: libraries (not places), then collections and folders below them. */
  private rows(): Row[] {
    const out: Row[] = [];
    const walk = (
      nodes: readonly NavigationNode[],
      depth: number,
      trail: readonly string[],
      inside: boolean,
    ) => {
      for (const node of nodes) {
        if (node.kind === "virtual_collection" || node.kind === "repository") continue;
        const place = isPlace(node.kind) && (inside || node.kind === "collection");
        const path = [...trail, nodeName(node, (this.options as DestinationPickerOptions).labels)];
        if (node.kind === "folder" && !inside) continue;
        out.push({ node, depth, trail: path, selectable: place });
        if (this.expanded.has(node.node) && depth < MAX_DEPTH) {
          walk(
            this.levels.get(node.node) ?? [],
            depth + 1,
            path,
            inside || node.kind === "collection",
          );
        }
      }
    };
    walk(this.roots, 0, [], false);
    return out;
  }

  /** Render the tree, or a state instead of it. */
  private show(state: ViewState | undefined): void {
    const root = this.shadowRoot;
    const options = this.options;
    if (!root || !options) return;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + CONTROL_STYLE + STYLE;
    const states = document.createElement("fs-states") as FsStates;
    states.configure({ labels: options.labels });
    states.hidden = state === undefined;
    if (state) states.state = state;
    const list = document.createElement("ul");
    list.setAttribute("role", "tree");
    list.setAttribute("aria-label", options.labels.fileDestinations);
    list.hidden = state !== undefined;
    if (!state) list.append(...this.rows().map((row) => this.rowElement(row)));
    if (!state && list.children.length === 0) list.append(this.emptyNote());
    const focused = (root.activeElement as HTMLElement | null)?.dataset?.node;
    root.replaceChildren(style, states, list);
    list.addEventListener("click", (event) => this.onClick(event));
    list.addEventListener("keydown", (event) => this.onKey(event));
    list.addEventListener("focusin", (event) => this.moveTabStop(list, event.target as Element));
    this.setTabStop(list);
    this.dataset.state = state?.kind ?? "ready";
    // The tree is drawn again after each change; focus stays on the row it was on (UI-S-016).
    const again = [...list.querySelectorAll<HTMLElement>("li[data-node]")].find(
      (li) => li.dataset.node === focused,
    );
    again?.focus();
  }

  /** Give the tree its one Tab stop: the chosen row, else the first (ARIA tree pattern). */
  private setTabStop(list: HTMLElement): void {
    const items = [...list.querySelectorAll<HTMLElement>("li[data-node]")];
    const stop = items.find((li) => li.getAttribute("aria-selected") === "true") ?? items[0];
    if (stop) stop.tabIndex = 0;
  }

  /** Move the tree's Tab stop to the row that took focus. */
  private moveTabStop(list: HTMLElement, target: Element): void {
    const row = target.closest<HTMLElement>("li[data-node]");
    if (!row) return;
    for (const li of list.querySelectorAll<HTMLElement>("li[data-node]")) li.tabIndex = -1;
    row.tabIndex = 0;
  }

  /** The note shown when no destination exists. */
  private emptyNote(): HTMLElement {
    const note = document.createElement("p");
    note.className = "empty";
    note.textContent = (this.options as DestinationPickerOptions).labels.fileDestinationNone;
    return note;
  }

  /** The element of one row. */
  private rowElement(row: Row): HTMLElement {
    const options = this.options as DestinationPickerOptions;
    const labels = options.labels;
    const li = document.createElement("li");
    li.setAttribute("role", "treeitem");
    li.tabIndex = -1;
    li.dataset.node = row.node.node;
    li.dataset.selectable = String(row.selectable);
    li.style.setProperty(
      "padding-inline-start",
      `calc(var(--fs-space-2) + ${row.depth} * var(--fs-space-3))`,
    );
    li.setAttribute("aria-level", String(row.depth + 1));
    li.setAttribute("aria-selected", String(this.chosen?.node === row.node.node));
    if (row.node.has_children)
      li.setAttribute("aria-expanded", String(this.expanded.has(row.node.node)));
    const chevron = document.createElement("span");
    chevron.className = "chevron";
    if (row.node.has_children) chevron.append(icon("chevron"));
    const name = document.createElement("span");
    name.className = "name";
    name.textContent = row.trail.at(-1) ?? "";
    li.append(chevron, icon(row.node.kind === "library" ? "library" : "collection"), name);
    if (options.variant === "import" && row.node.kind === "library") {
      const note = document.createElement("span");
      note.className = "not-destination";
      note.textContent = labels.importNotDestination;
      li.append(note);
    }
    if (row.node.node === this.options?.current) {
      const current = document.createElement("span");
      current.className = "current";
      current.textContent = labels.fileCurrentFolder;
      li.append(current);
    }
    return li;
  }

  /** A click chooses a place or toggles a parent. */
  private onClick(event: Event): void {
    const li = (event.target as Element).closest<HTMLElement>("li[data-node]");
    if (!li) return;
    const node = li.dataset.node as string;
    const toggler = (event.target as Element).closest(".chevron");
    if (toggler || li.dataset.selectable === "false") void this.toggle(node);
    else this.choose(node);
  }

  /** Expand or collapse a node; a failed read shows the error state with Try again. */
  private async toggle(node: string): Promise<void> {
    if (this.expanded.has(node)) {
      this.expanded.delete(node);
      this.show(undefined);
      return;
    }
    const generation = this.generation;
    try {
      await this.expand(node);
    } catch (error) {
      if (generation === this.generation) {
        this.showFailure("FileSquid could not read a destination", error);
      }
      return;
    }
    if (generation === this.generation) this.show(undefined);
  }

  /** Make the node the destination and announce it. */
  private choose(node: string): void {
    const row = this.rows().find((candidate) => candidate.node.node === node);
    if (!row?.selectable) return;
    this.chosen = { node, kind: row.node.kind, trail: row.trail };
    this.show(undefined);
    this.announce(this.chosen);
  }

  /** Tree keys: Up, Down, Right, Left, Enter and Space. */
  private onKey(event: KeyboardEvent): void {
    const items = [...(this.shadowRoot?.querySelectorAll<HTMLElement>("li[data-node]") ?? [])];
    const here = items.indexOf((this.shadowRoot?.activeElement ?? null) as HTMLElement);
    const move = { ArrowDown: 1, ArrowUp: -1 }[event.key as "ArrowDown" | "ArrowUp"];
    if (move !== undefined) {
      event.preventDefault();
      items[Math.min(items.length - 1, Math.max(0, here + move))]?.focus();
      return;
    }
    const node = items[here]?.dataset.node;
    if (!node) return;
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      if (items[here]?.dataset.selectable === "false") void this.toggle(node);
      else this.choose(node);
    }
    const state = items[here]?.getAttribute("aria-expanded");
    if (event.key === "ArrowRight" && state === "false") void this.toggle(node);
    if (event.key === "ArrowLeft" && state === "true") void this.toggle(node);
  }
}
