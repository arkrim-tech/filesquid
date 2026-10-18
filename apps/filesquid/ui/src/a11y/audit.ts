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

/** ARIA widget roles that require an accessible name. */
const NAMED_ROLES = [
  "button",
  "link",
  "checkbox",
  "radio",
  "switch",
  "tab",
  "menuitem",
  "menuitemcheckbox",
  "menuitemradio",
  "option",
  "treeitem",
  "combobox",
  "listbox",
  "slider",
  "spinbutton",
  "searchbox",
  "textbox",
];

/** Roles whose name may come from their content (WAI-ARIA "name from content"). */
const CONTENT_ROLES = new Set([
  "button",
  "link",
  "checkbox",
  "radio",
  "switch",
  "tab",
  "menuitem",
  "menuitemcheckbox",
  "menuitemradio",
  "option",
  "treeitem",
]);

/** Elements that need an accessible name when they can be operated. */
const CONTROLS = [
  "a[href]",
  "button",
  "input:not([type='hidden'])",
  "select",
  "textarea",
  ...NAMED_ROLES.map((role) => `[role='${role}']`),
].join(", ");

/** Landmarks that must be named so a screen reader can tell them apart. */
const LANDMARKS = "nav, [role='region'], [role='navigation']";

/** Input types whose `value` is their visible name, with the browser's default when empty. */
const VALUE_NAMED: Record<string, string> = { button: "", submit: "Submit", reset: "Reset" };

/**
 * Compute a simplified accessible name, in the order of the accname rules: `aria-labelledby`
 * (resolved in the element's own document or shadow root), `aria-label`, an associated or
 * wrapping `<label>`, `alt` (only on `img` and image inputs), the `value` of button, submit and
 * reset inputs, `title`, then the content (text plus the `alt` of descendant images) for
 * elements whose role takes its name from content. `select`, `textarea` and other inputs never
 * take a name from their content. Whitespace is collapsed.
 *
 * @param element The element to name.
 * @returns The name, or "" when it has none.
 */
export function accessibleName(element: Element): string {
  const candidates = [
    labelledBy(element),
    element.getAttribute("aria-label") ?? "",
    labelText(element),
    altText(element),
    valueText(element),
    element.getAttribute("title") ?? "",
    namedFromContent(element) ? contentText(element) : "",
  ];
  return candidates.map(collapse).find((name) => name !== "") ?? "";
}

/**
 * Audit a subtree, including every open shadow root inside it.
 *
 * @param root The element, shadow root or document to audit.
 * @returns The violations grouped by rule, in the order positive-tabindex, unnamed-control,
 *   missing-alt, unnamed-landmark. Within a rule: the root's own tree first, then each open
 *   shadow root depth first, each in document order. Empty when the subtree passes.
 */
export function accessibilityViolations(root: ParentNode): A11yViolation[] {
  const trees = scopes(root);
  const all = (selector: string) => trees.flatMap((scope) => matches(scope, selector));
  return [
    ...all("[tabindex]")
      .filter((el) => Number(el.getAttribute("tabindex")) > 0)
      .map((el) => violation("positive-tabindex", el)),
    ...all(CONTROLS)
      .filter((el) => operable(el) && accessibleName(el) === "")
      .map((el) => violation("unnamed-control", el)),
    ...all("img")
      .filter((el) => !el.hasAttribute("alt"))
      .map((el) => violation("missing-alt", el)),
    ...all(LANDMARKS)
      .filter((el) => landmarkName(el) === "")
      .map((el) => violation("unnamed-landmark", el)),
  ];
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

/**
 * True when a control can be reached: not `:disabled` (which includes controls inside a
 * disabled `<fieldset>`) and not inside a `hidden` or `inert` subtree.
 */
function operable(element: Element): boolean {
  if (element.matches(":disabled")) return false;
  return element.closest("[hidden], [inert]") === null;
}

/** A landmark is named only by `aria-labelledby` or `aria-label`, never by its content. */
function landmarkName(element: Element): string {
  return collapse(`${labelledBy(element)} ${element.getAttribute("aria-label") ?? ""}`);
}

/** `alt` names only an `<img>` or an image input. */
function altText(element: Element): string {
  const image =
    element instanceof HTMLImageElement ||
    (element instanceof HTMLInputElement && element.type === "image");
  return image ? (element.getAttribute("alt") ?? "") : "";
}

/** The `value` of a button, submit or reset input, or the browser's default label. */
function valueText(element: Element): string {
  if (!(element instanceof HTMLInputElement) || !(element.type in VALUE_NAMED)) return "";
  return element.value || (VALUE_NAMED[element.type] ?? "");
}

/** True when the element's role takes its name from its content. */
function namedFromContent(element: Element): boolean {
  const role = element.getAttribute("role");
  if (role) return CONTENT_ROLES.has(role);
  return element instanceof HTMLAnchorElement || element instanceof HTMLButtonElement;
}

/** The text of an element's content, with descendant images contributing their `alt`. */
function contentText(element: Element): string {
  return [...element.childNodes]
    .map((node) => {
      if (node instanceof HTMLImageElement) return ` ${node.getAttribute("alt") ?? ""} `;
      if (node instanceof Element) return contentText(node);
      return node.textContent ?? "";
    })
    .join("");
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
