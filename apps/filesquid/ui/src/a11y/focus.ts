/**
 * Focus helpers for UI-S-016 focus recovery: when a component re-renders and removes the
 * element that had focus, the browser drops focus to `<body>` and a keyboard or screen-reader
 * user loses their place. Components render through {@link replaceKeepingFocus} so focus moves
 * to a defined target instead. Focus is moved only when it was inside the re-rendered root, so
 * a first load or a background refresh never steals it.
 */

/**
 * The element that really has focus, following `activeElement` into open shadow roots.
 *
 * @param doc The document to look in.
 * @returns The innermost focused element, or `<body>` (or null) when nothing has focus.
 */
export function deepActiveElement(doc: Document): Element | null {
  let active = doc.activeElement;
  // Shadow roots nest a handful of levels at most; 32 bounds a malformed tree.
  for (let depth = 0; depth < 32 && active?.shadowRoot?.activeElement; depth += 1) {
    active = active.shadowRoot.activeElement;
  }
  return active;
}

/**
 * Move focus to `element`. An element that is not focusable by itself (a heading, `main`, an
 * alert) gets `tabindex="-1"` first, so it can take focus without joining the Tab order.
 * Side effect: changes focus.
 *
 * @param element The new focus target.
 */
export function focusElement(element: HTMLElement): void {
  if (!focusable(element)) element.setAttribute("tabindex", "-1");
  element.focus();
}

/**
 * Replace the children of a shadow root and, when that removed the focused element, move
 * focus to `target()`. Re-inserting the focused element counts as removing it, as it does in the
 * browser, so the target may be that element again. When focus was outside the root, focus is
 * left alone. Side effect: mutates the root and may change focus.
 *
 * @param root The component's shadow root.
 * @param children The new children.
 * @param target Called after the replacement to pick the focus target; null leaves focus unset.
 */
export function replaceKeepingFocus(
  root: ShadowRoot,
  children: readonly Node[],
  target: () => HTMLElement | null | undefined,
): void {
  const hadFocus = root.activeElement !== null;
  root.replaceChildren(...children);
  recoverFocus(root, hadFocus, target);
}

/**
 * After a re-render that may have removed the focused element, move focus to `target()` when
 * focus was inside `root` before and is no longer. Side effect: may change focus.
 *
 * @param root The component's shadow root.
 * @param hadFocus Whether focus was inside `root` before the re-render.
 * @param target Called to pick the focus target; null leaves focus unset.
 */
export function recoverFocus(
  root: ShadowRoot,
  hadFocus: boolean,
  target: () => HTMLElement | null | undefined,
): void {
  if (!hadFocus || root.activeElement !== null) return;
  const element = target();
  if (element) focusElement(element);
}

/** True for elements the browser focuses without a `tabindex`. */
function focusable(element: HTMLElement): boolean {
  if (element.hasAttribute("tabindex")) return true;
  if (element instanceof HTMLAnchorElement) return element.hasAttribute("href");
  return (
    element instanceof HTMLButtonElement ||
    element instanceof HTMLInputElement ||
    element instanceof HTMLSelectElement ||
    element instanceof HTMLTextAreaElement
  );
}
