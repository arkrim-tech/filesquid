/**
 * `<fs-breadcrumb>`: where the current destination sits (UI-F2.1a). A `nav` landmark named by
 * the `navigationBreadcrumb` label holds an ordered list of links from the top-level
 * destination down to the current one, which carries `aria-current="page"` (WAI-ARIA
 * breadcrumb pattern). The trail is data: set `items`, for example from `breadcrumbTrail()`
 * over the navigation data. The element is hidden while the trail is empty.
 *
 * Accessibility (UI-S-016): separators are drawn by CSS and hidden from assistive technology;
 * the trail wraps instead of scrolling; at most `MAX_DEPTH` links are shown (the last ones, so
 * the current page stays last); after a new trail, focus stays on the link with the same
 * `href` when it survives, and otherwise moves to the current page's link.
 */
import type { LabelTable } from "../theme/labels";
import { MAX_DEPTH, type BreadcrumbItem } from "../navigation/model";
import { A11Y_STYLE } from "./a11y-style";
import { replaceKeepingFocus } from "../a11y/focus";

/** What the element needs: the active label table. */
export interface BreadcrumbOptions {
  /** Label table for the landmark name. */
  labels: LabelTable;
}

const STYLE = `
  :host { display: block; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  :host([hidden]) { display: none; }
  ol { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-1) var(--fs-space-2); margin: 0; padding: 0; list-style: none; }
  li { display: inline-flex; align-items: center; gap: var(--fs-space-2); min-width: 0; }
  li + li::before { content: "/" / ""; color: var(--fs-on-surface-variant); }
  a { color: var(--fs-primary); text-decoration: none; border-radius: var(--fs-radius); overflow-wrap: anywhere; transition: color var(--fs-motion-duration); }
  a:hover { text-decoration: underline; }
  a[aria-current="page"] { color: var(--fs-on-surface); font-weight: 600; }
`;

/** Custom element that shows a breadcrumb trail. */
export class FsBreadcrumb extends HTMLElement {
  private labels?: LabelTable;
  private trail: readonly BreadcrumbItem[] = [];

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the label table, then render. Call before or after connecting.
   *
   * @param options The active label table.
   */
  configure(options: BreadcrumbOptions): void {
    this.labels = options.labels;
    this.render();
  }

  /** The trail shown now, top-level destination first. */
  get items(): readonly BreadcrumbItem[] {
    return this.trail;
  }

  /** Show another trail, top-level destination first; the last item is the current page. */
  set items(items: readonly BreadcrumbItem[]) {
    this.trail = items;
    this.render();
  }

  /** Build the landmark and the list; hide the element while the trail is empty. */
  private render(): void {
    const root = this.shadowRoot;
    if (!root || !this.labels) return;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + STYLE;
    const nav = document.createElement("nav");
    nav.setAttribute("aria-label", this.labels.navigationBreadcrumb);
    const list = document.createElement("ol");
    const trail = this.trail.slice(-MAX_DEPTH);
    list.append(...trail.map((item, i) => crumb(item, i === trail.length - 1)));
    nav.append(list);
    this.hidden = trail.length === 0;
    const focusedHref = root.activeElement?.getAttribute("href");
    replaceKeepingFocus(root, [style, nav], () => surviving(root, focusedHref));
  }
}

/**
 * The link to focus after a re-render: the one with the previously focused `href` when it is
 * still in the trail, otherwise the current page's link.
 */
function surviving(root: ShadowRoot, href: string | null | undefined): HTMLElement | null {
  const links = [...root.querySelectorAll<HTMLAnchorElement>("a")];
  const same = links.find((link) => href != null && link.getAttribute("href") === href);
  return same ?? root.querySelector<HTMLElement>('a[aria-current="page"]');
}

/** One `<li><a href>name</a></li>`, marked as the current page when `current`. */
function crumb(item: BreadcrumbItem, current: boolean): HTMLLIElement {
  const li = document.createElement("li");
  const link = document.createElement("a");
  link.href = item.href;
  link.textContent = item.name;
  if (current) link.setAttribute("aria-current", "page");
  li.append(link);
  return li;
}
