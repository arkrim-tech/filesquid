/**
 * The accessibility stylesheet every `@filesquid/ui` custom element puts first in its shadow
 * root (UI-S-016), so the contracts hold in every component and every distribution inherits
 * them without code:
 *
 * - a visible focus ring on everything in the Tab order: 2px solid `--fs-primary`, offset 2px
 *   (the token contrast test keeps it at 3:1 on the page, sidebar and selected item);
 * - no focus ring on elements focused only by script (`tabindex="-1"` targets of focus
 *   recovery), which are not in the Tab order;
 * - no motion under `prefers-reduced-motion: reduce`;
 * - stronger borders under `prefers-contrast: more`;
 * - system colours for the focus ring under `forced-colors: active`.
 */

const STYLE = `
  :focus-visible { outline: 2px solid var(--fs-primary); outline-offset: 2px; }
  [tabindex="-1"]:focus { outline: none; }
  @media (prefers-reduced-motion: reduce) {
    *, *::before, *::after { transition-duration: 0s !important; animation-duration: 0s !important; animation-iteration-count: 1 !important; scroll-behavior: auto !important; }
  }
  @media (prefers-contrast: more) {
    *, *::before, *::after { border-color: var(--fs-on-surface-variant) !important; }
  }
  @media (forced-colors: active) {
    :focus-visible { outline-color: Highlight; }
  }
`;

/** CSS text of the shared accessibility stylesheet; components prepend it to their own. */
export const A11Y_STYLE = STYLE;

/** Id of the document-level copy of the stylesheet (internal to `@filesquid/ui`). */
export const A11Y_DOCUMENT_STYLE_ID = "fs-a11y-style";

/** How many connected shells currently use the document-level copy, per document. */
const documentUsers = new WeakMap<Document, number>();

/** The document-level copy this module installed in `doc.head`, if present. */
function ownDocumentStyle(doc: Document): HTMLStyleElement | null {
  return doc.head.querySelector<HTMLStyleElement>(`:scope > style#${A11Y_DOCUMENT_STYLE_ID}`);
}

/**
 * Take a reference to the document-level copy of the shared rules, adding it to `doc.head`
 * when absent, so light-DOM content that a distribution slots into a component (which
 * shadow-root styles cannot reach beyond the slotted element itself) gets the same focus ring,
 * motion and contrast rules. Only this module's own `<style id="fs-a11y-style">` in the head
 * counts as present; an unrelated element with that id does not. Internal: `<fs-app-shell>`
 * calls it when connected and calls {@link releaseDocumentA11yStyle} when disconnected.
 * Side effect: may mutate the document head.
 *
 * @param doc The document the component is connected to.
 */
export function retainDocumentA11yStyle(doc: Document): void {
  documentUsers.set(doc, (documentUsers.get(doc) ?? 0) + 1);
  if (ownDocumentStyle(doc)) return;
  const style = doc.createElement("style");
  style.id = A11Y_DOCUMENT_STYLE_ID;
  style.textContent = STYLE;
  doc.head.append(style);
}

/**
 * Drop a reference taken with {@link retainDocumentA11yStyle}; the last release removes the
 * document-level copy, so no global `!important` rule outlives the shells that need it.
 * Side effect: may mutate the document head.
 *
 * @param doc The document the component was connected to.
 */
export function releaseDocumentA11yStyle(doc: Document): void {
  const users = Math.max((documentUsers.get(doc) ?? 0) - 1, 0);
  documentUsers.set(doc, users);
  if (users === 0) ownDocumentStyle(doc)?.remove();
}
