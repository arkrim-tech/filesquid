/**
 * Accessibility audit for UI-S-016: a small, dependency-free DOM check that every
 * `@filesquid/ui` element and every distribution can run in its own tests. It walks open shadow
 * roots and reports positive `tabindex`, unnamed controls, images without a text alternative
 * and unnamed landmarks. It is a floor, not a full WCAG audit: the built-artifact Playwright
 * tests check the browser's own accessibility tree on top of it. Pure apart from DOM reads.
 */

/** The rules the audit checks. */
export type A11yRule = "positive-tabindex" | "unnamed-control" | "missing-alt" | "unnamed-landmark";

/** One finding: the rule broken and the lower-case tag of the element that broke it. */
export interface A11yViolation {
  /** The rule broken. */
  rule: A11yRule;
  /** Lower-case tag name of the element. */
  element: string;
}

/** Elements that need an accessible name when they can be operated. */
const CONTROLS =
  "a[href], button, input:not([type='hidden']), select, textarea, [role='button'], [role='link'], [role='radio'], [role='checkbox'], [role='tab'], [role='menuitem']";

/** Landmarks that must be named so a screen reader can tell them apart. */
const LANDMARKS = "nav, [role='region'], [role='navigation']";

/**
 * Compute a simplified accessible name: `aria-labelledby` (resolved in the element's own
 * document or shadow root), `aria-label`, an associated or wrapping `<label>`, `alt`, `title`,
 * then the text content. Whitespace is collapsed.
 *
 * @param element The element to name.
 * @returns The name, or "" when it has none.
 */
export function accessibleName(element: Element): string {
  const candidates = [
    labelledBy(element),
    element.getAttribute("aria-label") ?? "",
    labelText(element),
    element.getAttribute("alt") ?? "",
    element.getAttribute("title") ?? "",
    element instanceof HTMLInputElement ? "" : (element.textContent ?? ""),
  ];
  return candidates.map(collapse).find((name) => name !== "") ?? "";
}

/**
 * Audit a subtree, including every open shadow root inside it.
 *
 * @param root The element, shadow root or document to audit.
 * @returns The violations, grouped by rule and in document order within a rule; empty when
 *   the subtree passes.
 */
export function accessibilityViolations(root: ParentNode): A11yViolation[] {
  return scopes(root).flatMap((scope) => [
    ...matches(scope, "[tabindex]")
      .filter((el) => Number(el.getAttribute("tabindex")) > 0)
      .map((el) => violation("positive-tabindex", el)),
    ...matches(scope, CONTROLS)
      .filter((el) => operable(el) && accessibleName(el) === "")
      .map((el) => violation("unnamed-control", el)),
    ...matches(scope, "img")
      .filter((el) => !el.hasAttribute("alt"))
      .map((el) => violation("missing-alt", el)),
    ...matches(scope, LANDMARKS)
      .filter((el) => landmarkName(el) === "")
      .map((el) => violation("unnamed-landmark", el)),
  ]);
}

/** The root and every open shadow root below it, depth first. */
function scopes(root: ParentNode): ParentNode[] {
  const hosts = [root, ...root.querySelectorAll("*")].filter(
    (node): node is Element => node instanceof Element && node.shadowRoot !== null,
  );
  return [root, ...hosts.flatMap((host) => (host.shadowRoot ? scopes(host.shadowRoot) : []))];
}

/** Elements of `scope` matching `selector`, the scope itself included when it is an element. */
function matches(scope: ParentNode, selector: string): Element[] {
  const self = scope instanceof Element && scope.matches(selector) ? [scope] : [];
  return [...self, ...scope.querySelectorAll(selector)];
}

/** True when a control can be reached: not disabled and not inside a `hidden` subtree. */
function operable(element: Element): boolean {
  if (element.hasAttribute("disabled")) return false;
  return element.closest("[hidden]") === null;
}

/** A landmark is named only by `aria-labelledby` or `aria-label`, never by its content. */
function landmarkName(element: Element): string {
  return collapse(`${labelledBy(element)} ${element.getAttribute("aria-label") ?? ""}`);
}

/** The text of the elements named by `aria-labelledby`, looked up in the element's own root. */
function labelledBy(element: Element): string {
  const ids = (element.getAttribute("aria-labelledby") ?? "").split(/\s+/).filter(Boolean);
  const root = element.getRootNode() as ParentNode;
  return ids.map((id) => byId(root, id)?.textContent ?? "").join(" ");
}

/** The text of the `<label>` wrapping the element or pointing at it with `for`. */
function labelText(element: Element): string {
  const wrapping = element.closest("label");
  if (wrapping && wrapping !== element) return wrapping.textContent ?? "";
  if (!element.id) return "";
  const root = element.getRootNode() as ParentNode;
  return root.querySelector(`label[for="${quoted(element.id)}"]`)?.textContent ?? "";
}

/** Find an element by id in a document or shadow root. */
function byId(root: ParentNode, id: string): Element | null {
  return root.querySelector(`[id="${quoted(id)}"]`);
}

/** Escape a value for use inside a double-quoted CSS attribute selector. */
function quoted(value: string): string {
  return value.replace(/["\\]/g, "\\$&");
}

/** Collapse runs of whitespace and trim. */
function collapse(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

/** Build a violation for `element`. */
function violation(rule: A11yRule, element: Element): A11yViolation {
  return { rule, element: element.tagName.toLowerCase() };
}
