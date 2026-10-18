/**
 * `<fs-states>`: what a view shows instead of its content (UI-F2.1a). Five distinct states,
 * each worded only from the active label table: loading and empty in a polite `role="status"`
 * region, error, offline and permission denied in a `role="alert"` region. Error and offline
 * offer Retry, which dispatches a bubbling, composed `fs-retry` event (ADR 031 Decision 6:
 * changes flow up as events); the view that owns the data decides what to reload.
 *
 * Accessibility (UI-S-016): both live regions exist from the first render and are updated in
 * place, so screen readers announce every change; when focus was inside the element and the
 * state moves to the other region (loading after Retry, or error after a failed Retry), focus
 * moves to the region that now shows the state; focus is never moved when it was outside the
 * element.
 */
import type { LabelTable } from "../theme/labels";
import { reasonLabel } from "./denial";
import { A11Y_STYLE } from "./a11y-style";
import { focusElement } from "../a11y/focus";
import type { ViewState } from "./view-state";

/** What the element needs: the active label table. */
export interface StatesOptions {
  /** Label table for every message and the Retry button. */
  labels: LabelTable;
}

/** Name of the event Retry dispatches. */
export const RETRY_EVENT = "fs-retry";

const STYLE = `
  :host { display: block; color: var(--fs-on-surface); font-family: var(--fs-font-sans); font-size: var(--fs-font-size-body); }
  :host([hidden]) { display: none; }
  .region { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-2) var(--fs-space-3); padding: var(--fs-space-3) var(--fs-space-4); border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface-container); }
  .region:empty { padding: 0; border: 0; }
  p { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  .status { color: var(--fs-on-surface-variant); }
  .alert .title { color: var(--fs-error); font-weight: 600; }
  .detail { color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  button { padding: var(--fs-space-1) var(--fs-space-3); border: 1px solid var(--fs-primary); border-radius: var(--fs-radius); background: var(--fs-surface); color: var(--fs-primary); font: inherit; font-weight: 600; cursor: pointer; transition: background-color var(--fs-motion-duration), color var(--fs-motion-duration); }
  button:hover { background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
`;

/** Custom element that shows one {@link ViewState}. */
export class FsStates extends HTMLElement {
  private labels?: LabelTable;
  private current: ViewState = { kind: "loading" };

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the label table, then render. Call before or after connecting.
   *
   * @param options The active label table.
   */
  configure(options: StatesOptions): void {
    this.labels = options.labels;
    this.update();
  }

  /** The state shown now. */
  get state(): ViewState {
    return this.current;
  }

  /** Show another state, updating the live regions in place. */
  set state(state: ViewState) {
    this.current = state;
    this.update();
  }

  /** Build the stylesheet and the two persistent live regions once. */
  private skeleton(root: ShadowRoot): void {
    if (root.querySelector(".region")) return;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + STYLE;
    const status = region("status");
    const alert = region("alert");
    root.replaceChildren(style, status, alert);
  }

  /** Fill the region for the current state and empty the other; recover focus if needed. */
  private update(): void {
    const root = this.shadowRoot;
    const labels = this.labels;
    if (!root || !labels) return;
    this.skeleton(root);
    const hadFocus = root.activeElement !== null;
    const state = this.current;
    const alerting = isAlert(state);
    const status = root.querySelector<HTMLElement>(".status");
    const alert = root.querySelector<HTMLElement>(".alert");
    status?.replaceChildren(...(alerting ? [] : statusContent(state, labels)));
    alert?.replaceChildren(...(alerting ? alertContent(state, labels, () => this.retry()) : []));
    this.dataset.state = state.kind;
    const inUse = alerting ? alert : status;
    if (hadFocus && inUse && !inUse.contains(root.activeElement)) focusElement(inUse);
  }

  /** Ask the owning view to load again. */
  private retry(): void {
    this.dispatchEvent(new CustomEvent(RETRY_EVENT, { bubbles: true, composed: true }));
  }
}

/** A persistent live region with the given role. */
function region(role: "status" | "alert"): HTMLDivElement {
  const div = document.createElement("div");
  div.className = `region ${role}`;
  div.setAttribute("role", role);
  return div;
}

/** True for the states announced as alerts. */
function isAlert(state: ViewState): boolean {
  return state.kind === "error" || state.kind === "offline" || state.kind === "denied";
}

/** The message of a loading or empty state. */
function statusContent(state: ViewState, labels: LabelTable): Node[] {
  const text = state.kind === "empty" ? (state.message ?? labels.stateEmpty) : labels.stateLoading;
  return [paragraph("title", text)];
}

/** The message, detail and Retry of an error, offline or denied state. */
function alertContent(state: ViewState, labels: LabelTable, retry: () => void): Node[] {
  if (state.kind === "denied") {
    const detail = state.reason ? reasonLabel(state.reason, labels) : undefined;
    const title = paragraph("title", labels.stateDenied);
    return detail ? [title, paragraph("detail", detail)] : [title];
  }
  const title = paragraph(
    "title",
    state.kind === "offline" ? labels.stateOffline : labels.stateError,
  );
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = labels.stateRetry;
  button.addEventListener("click", retry);
  return [title, button];
}

/** A `<p>` with a class and text. */
function paragraph(cls: string, text: string): HTMLParagraphElement {
  const p = document.createElement("p");
  p.className = cls;
  p.textContent = text;
  return p;
}
