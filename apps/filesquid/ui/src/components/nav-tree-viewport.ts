/**
 * The viewport of the virtualised navigation tree (Task UI-F3.4, UI-S-035): what part of each
 * section's tree is in view, and when that may have changed. `<fs-nav-tree>` does not scroll
 * itself. In the shell the sidebar's tree area scrolls it (UI-S-025), in the stacked layout
 * below 40 rem the page does, and a distribution may place it in a scrolling area of its own.
 * So the part in view is what the window and every clipping ancestor leave of a tree, and the
 * tree follows the scrolls and size changes of all of them.
 *
 * Side effects are named: {@link TreeViewport.bands} reads layout, {@link TreeViewport.follow}
 * adds listeners and returns the function that removes them.
 */
import type { TreeBand } from "./nav-tree-window";

/** Row height in rem: 30 px at the default text size (mockup 55, UI-S-024). */
export const TREE_ROW_REM = 1.875;

/** Rows assumed in view when the tree has no measured view yet (no layout, or hidden). */
const FALLBACK_ROWS = 20;

/** Most ancestors walked from the tree to the document; bounds a malformed tree. */
const MAX_ANCESTORS = 64;

/** What the tree asks about where it is shown. Injectable, so tests need no layout. */
export interface TreeViewport {
  /**
   * The height of one row.
   *
   * @returns Pixels.
   */
  rowHeight(): number;
  /**
   * The part of each tree that is in view. Reads layout.
   *
   * @param lists The `ul[role=tree]` of each section, in document order.
   * @returns One band per list, in pixels from that list's top edge.
   */
  bands(lists: readonly HTMLElement[]): TreeBand[];
  /**
   * Follow what may move a band: scrolls and size changes. Adds listeners.
   *
   * @param changed Called when the bands may have changed.
   * @returns A function that stops following.
   */
  follow(changed: () => void): () => void;
}

/** Pixels per rem, from the root font size. */
function remPixels(): number {
  const size = parseFloat(getComputedStyle(document.documentElement).fontSize);
  return Number.isFinite(size) && size > 0 ? size : 16;
}

/**
 * The elements around `host` in the flattened tree, nearest first, up to the document: the slot
 * it is assigned to, parents, and the hosts of the shadow roots on the way.
 */
function composedAncestors(host: Element): Element[] {
  const out: Element[] = [];
  let at: Element | null = host;
  for (let depth = 0; depth < MAX_ANCESTORS && at; depth += 1) {
    const root: Node = at.getRootNode();
    const above: Element | null =
      at.assignedSlot ?? at.parentElement ?? (root instanceof ShadowRoot ? root.host : null);
    if (above) out.push(above);
    at = above;
  }
  return out;
}

/** True for an element that cuts off what overflows it vertically. */
function clips(element: Element): boolean {
  if (element === document.documentElement || element === document.body) return false;
  const overflow = getComputedStyle(element).overflowY;
  return overflow !== "" && overflow !== "visible";
}

/** The vertical span of the window that every clipping ancestor leaves, in client pixels. */
function visibleSpan(ancestors: readonly Element[]): { top: number; bottom: number } {
  let top = 0;
  let bottom = window.innerHeight || document.documentElement.clientHeight;
  for (const element of ancestors) {
    if (!clips(element)) continue;
    const edge = element.getBoundingClientRect().top + element.clientTop;
    top = Math.max(top, edge);
    bottom = Math.min(bottom, edge + element.clientHeight);
  }
  return { top, bottom };
}

/**
 * The viewport of a tree in the document: 1.875 rem rows, and the bands the window and the
 * tree's clipping ancestors leave in view. Without a measured view (no layout engine, or a
 * hidden sidebar) it assumes 20 rows, so the tree still draws a small, bounded window.
 *
 * @param host The `<fs-nav-tree>` element.
 * @returns The viewport.
 */
export function domViewport(host: HTMLElement): TreeViewport {
  const rowHeight = () => TREE_ROW_REM * remPixels();
  return {
    rowHeight,
    bands(lists) {
      const span = visibleSpan(composedAncestors(host));
      const bottom = span.bottom > span.top ? span.bottom : span.top + FALLBACK_ROWS * rowHeight();
      return lists.map((list) => {
        const edge = list.getBoundingClientRect().top;
        return { top: span.top - edge, bottom: bottom - edge };
      });
    },
    follow(changed) {
      const targets: EventTarget[] = [window, ...composedAncestors(host)];
      const passive: AddEventListenerOptions = { passive: true };
      for (const target of targets) target.addEventListener("scroll", changed, passive);
      window.addEventListener("resize", changed, passive);
      const observer =
        typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(changed);
      for (const target of [host, ...composedAncestors(host)]) observer?.observe(target);
      return () => {
        for (const target of targets) target.removeEventListener("scroll", changed, passive);
        window.removeEventListener("resize", changed, passive);
        observer?.disconnect();
      };
    },
  };
}
