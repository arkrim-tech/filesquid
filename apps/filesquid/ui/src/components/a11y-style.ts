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

/** Id of the document-level copy of the stylesheet. */
export const A11Y_DOCUMENT_STYLE_ID = "fs-a11y-style";

/**
 * Add one document-level copy of the shared rules to `doc.head`, so light-DOM content that a
 * distribution slots into a component (which shadow-root styles cannot reach beyond the
 * slotted element itself) gets the same focus ring, motion and contrast rules. Idempotent.
 * Side effect: mutates the document head.
 *
 * @param doc The document the component is connected to.
 */
export function installDocumentA11yStyle(doc: Document): void {
  if (doc.getElementById(A11Y_DOCUMENT_STYLE_ID)) return;
  const style = doc.createElement("style");
  style.id = A11Y_DOCUMENT_STYLE_ID;
  style.textContent = STYLE;
  doc.head.append(style);
}
