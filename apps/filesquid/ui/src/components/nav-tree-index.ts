/**
 * An index of the navigation tree as drawn (UI-F2.1a, Task UI-F3.1). Moving the current
 * destination changes three things only: which items carry `aria-current`, which item is each
 * section's Tab stop and what the Pin toggle names. Finding those items by searching the DOM
 * walks every element of the tree (a level of 10,000 nodes is about 80,000 elements, 5 ms a
 * search), so the tree indexes its items once, when it is drawn, and these moves touch only the
 * items involved.
 */

/** The drawn treeitems, by section and node handle, with each section's Tab stop and marks. */
export class TreeIndex {
  /** Section id to the section element. */
  private readonly sections = new Map<string, HTMLElement>();
  /** Section id to its treeitems by node handle (the first item of a handle). */
  private readonly items = new Map<string, Map<string, HTMLElement>>();
  /** Section id to its first treeitem. */
  private readonly firsts = new Map<string, HTMLElement>();
  /** Section id to the treeitem that is its Tab stop now. */
  private readonly stops = new Map<string, HTMLElement>();
  /** The treeitems that carry `aria-current` now. */
  private marked: HTMLElement[] = [];

  /** True when no tree is drawn. */
  get empty(): boolean {
    return this.sections.size === 0;
  }

  /** The ids of the drawn sections, in order. */
  sectionIds(): string[] {
    return [...this.sections.keys()];
  }

  /**
   * Index a drawn tree in one pass, remembering which items already carry `aria-current` and
   * which are Tab stops. Replaces the earlier index.
   *
   * @param box The element that holds the `section[data-section]` elements.
   */
  build(box: ParentNode): void {
    this.clear();
    for (const section of box.querySelectorAll<HTMLElement>("section[data-section]")) {
      const id = section.dataset.section ?? "";
      const byNode = new Map<string, HTMLElement>();
      for (const item of section.querySelectorAll<HTMLElement>('[role="treeitem"]')) {
        const node = item.dataset.node ?? "";
        if (!byNode.has(node)) byNode.set(node, item);
        if (!this.firsts.has(id)) this.firsts.set(id, item);
        if (item.getAttribute("tabindex") === "0") this.stops.set(id, item);
        if (item.getAttribute("aria-current") === "page") this.marked.push(item);
      }
      this.sections.set(id, section);
      this.items.set(id, byNode);
    }
  }

  /** Forget the drawn tree (it was replaced by a state, or is being drawn again). */
  clear(): void {
    this.sections.clear();
    this.items.clear();
    this.firsts.clear();
    this.stops.clear();
    this.marked = [];
  }

  /** The section element, or undefined. */
  section(id: string): HTMLElement | undefined {
    return this.sections.get(id);
  }

  /** The treeitem of a node in a section, or undefined when the section does not draw it. */
  itemIn(id: string, node: string | undefined): HTMLElement | undefined {
    return node === undefined ? undefined : this.items.get(id)?.get(node);
  }

  /** The first treeitem of a section. */
  first(id: string): HTMLElement | undefined {
    return this.firsts.get(id);
  }

  /**
   * Move `aria-current` to the items of a node, in every section that draws it, and take it from
   * the items that had it. Side effects: sets and removes attributes of those items only.
   *
   * @param node The current destination, or undefined for none.
   */
  mark(node: string | undefined): void {
    for (const item of this.marked) item.removeAttribute("aria-current");
    this.marked = [...this.items.values()].flatMap((byNode) => {
      const item = node === undefined ? undefined : byNode.get(node);
      return item ? [item] : [];
    });
    for (const item of this.marked) item.setAttribute("aria-current", "page");
  }

  /**
   * Make an item its section's only Tab stop. Side effects: sets `tabindex` of the old and the
   * new stop.
   *
   * @param id The section.
   * @param item The treeitem that becomes the stop.
   */
  setStop(id: string, item: HTMLElement): void {
    const old = this.stops.get(id);
    if (old && old !== item) old.tabIndex = -1;
    if (item.tabIndex !== 0) item.tabIndex = 0;
    this.stops.set(id, item);
  }
}
