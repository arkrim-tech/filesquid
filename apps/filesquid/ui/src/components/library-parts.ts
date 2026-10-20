/**
 * Parts the three library pages share (UI-1.1): the stylesheet, small DOM builders and the
 * {@link Feedback} block that shows a step's progress, outcome or failure. Every word is set
 * with `textContent` from the label table; every colour, size and duration comes from a token.
 */
import type { Failure, LibraryAction } from "../libraries/errors";
import type { LibrarySource } from "../libraries/model";
import type { LibraryRegistry } from "../libraries/registry";
import type { LabelTable } from "../theme/labels";
import { focusElement } from "../a11y/focus";
import { A11Y_STYLE } from "./a11y-style";
import type { FsStates } from "./fs-states";

/** What every library page needs. */
export interface LibraryPageOptions {
  /** Label table for every word shown. */
  labels: LabelTable;
  /** The library routes. */
  source: LibrarySource;
  /** Libraries seen this session; the pages share one. */
  registry: LibraryRegistry;
  /** Go to a hash address; sets `location.hash` by default. */
  navigate?: (hash: string) => void;
  /** Whether the browser reports a network connection; `navigator.onLine` by default. */
  online?: () => boolean;
}

/** A step of a flow that failed, with the step it was and what it rejected with. */
export class StepFailure extends Error {
  /**
   * @param action The step that failed.
   * @param reason What it rejected with.
   */
  constructor(
    readonly action: LibraryAction,
    readonly reason: unknown,
  ) {
    super(`FileSquid library step ${action} failed`);
    this.name = "StepFailure";
  }
}

/**
 * Run one step of a flow and tag its failure with the step, so the page can word it.
 *
 * @param action The step.
 * @param call The step's work.
 * @returns What the step answered.
 * @throws StepFailure when the step rejects.
 */
export async function step<T>(action: LibraryAction, call: () => Promise<T>): Promise<T> {
  try {
    return await call();
  } catch (reason) {
    throw new StepFailure(action, reason);
  }
}

/** The pages' stylesheet; each page puts {@link A11Y_STYLE} before it. */
export const LIBRARY_STYLE = `
  :host { display: block; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-body); color: var(--fs-on-surface); }
  .page { display: grid; gap: var(--fs-space-4); max-inline-size: 56rem; }
  h2 { margin: 0; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-title); font-weight: 700; }
  h3 { margin: 0; font-size: var(--fs-font-size-body); font-weight: 700; }
  p { margin: 0; overflow-wrap: anywhere; }
  .intro, .help, .note { color: var(--fs-on-surface-variant); }
  .cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr)); gap: var(--fs-space-3); }
  section { display: grid; align-content: start; gap: var(--fs-space-2); box-sizing: border-box; min-inline-size: 0; padding: var(--fs-space-3) var(--fs-space-4); border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface-container); }
  button { justify-self: start; padding: var(--fs-space-1) var(--fs-space-3); border: 1px solid var(--fs-primary); border-radius: var(--fs-radius); background: var(--fs-surface); color: var(--fs-primary); font: inherit; font-weight: 600; cursor: pointer; transition: background-color var(--fs-motion-duration), color var(--fs-motion-duration); }
  button:hover { background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  button.primary { background: var(--fs-primary); color: var(--fs-on-primary); }
  button[aria-disabled="true"] { opacity: 0.5; cursor: default; }
  a { color: var(--fs-primary); }
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: var(--fs-space-1) var(--fs-space-4); margin: 0; }
  dt { color: var(--fs-on-surface-variant); }
  dd { margin: 0; overflow-wrap: anywhere; }
  ul { display: grid; gap: var(--fs-space-1); margin: 0; padding-inline-start: var(--fs-space-4); overflow-wrap: anywhere; }
  .status:empty, .alert:empty { display: none; }
  .alert { color: var(--fs-error); font-weight: 600; }
  .actions { display: flex; flex-wrap: wrap; gap: var(--fs-space-2); }
  .scrim { position: fixed; inset: 0; z-index: 3; display: grid; place-items: center; padding: var(--fs-space-4); background: color-mix(in srgb, var(--fs-on-surface) 45%, transparent); }
  [role="dialog"] { display: grid; gap: var(--fs-space-3); box-sizing: border-box; inline-size: min(36rem, 100%); max-block-size: 100%; padding: var(--fs-space-4); border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface); color: var(--fs-on-surface); overflow: auto; }
  .consent { font-weight: 600; }
`;

/** The stylesheet of a page: the shared accessibility rules first, then the pages' own. */
export function pageStyle(): HTMLStyleElement {
  const style = document.createElement("style");
  style.textContent = A11Y_STYLE + LIBRARY_STYLE;
  return style;
}

/**
 * Create an element with a class and text. Pure DOM construction.
 *
 * @param tag The tag name.
 * @param className Its class, if any.
 * @param text Its text content, if any.
 * @returns The element.
 */
export function node<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  if (className) element.className = className;
  if (text !== undefined) element.textContent = text;
  return element;
}

/**
 * Create a button that names its action in `data-action`.
 *
 * @param action The action name, for example `create`.
 * @param text The button's label from the label table.
 * @returns The button.
 */
export function actionButton(action: string, text: string): HTMLButtonElement {
  const button = node("button", undefined, text);
  button.type = "button";
  button.dataset.action = action;
  return button;
}

/**
 * Create a link.
 *
 * @param className Its class.
 * @param text Its label from the label table.
 * @param href Its hash address.
 * @returns The link.
 */
export function link(className: string, text: string, href: string): HTMLAnchorElement {
  const anchor = node("a", className, text);
  anchor.href = href;
  return anchor;
}

/** Whether the browser reports a network connection. */
export function browserOnline(): boolean {
  return typeof navigator === "undefined" ? true : navigator.onLine;
}

/**
 * The progress, outcome or failure of one step, as persistent live regions so assistive
 * technology announces each change: a polite status line, an assertive alert line and an
 * `<fs-states>` for the states that offer Retry. Created once; updated in place.
 */
export class Feedback {
  /** The block to add to a page. */
  readonly root: HTMLElement = node("div", "feedback");
  /** The state element, hidden unless a state is shown. */
  readonly states: FsStates;
  private readonly status = node("p", "status");
  private readonly alert = node("p", "alert");

  /**
   * @param labels The active label table.
   * @param name What the block is for, written to `data-for` (for example `actions`).
   */
  constructor(labels: LabelTable, name?: string) {
    this.states = document.createElement("fs-states") as FsStates;
    this.states.configure({ labels });
    this.states.hidden = true;
    this.status.setAttribute("role", "status");
    this.alert.setAttribute("role", "alert");
    if (name) this.root.dataset.for = name;
    this.root.append(this.states, this.status, this.alert);
  }

  /** Empty every region. */
  clear(): void {
    this.states.hidden = true;
    this.status.textContent = "";
    this.alert.textContent = "";
  }

  /**
   * Show a progress or outcome sentence.
   *
   * @param text The sentence from the label table.
   */
  note(text: string): void {
    const retryHadFocus = this.states.shadowRoot?.activeElement != null;
    this.clear();
    if (retryHadFocus) focusElement(this.status);
    this.status.textContent = text;
  }

  /**
   * Show the empty state with a sentence of the view's own.
   *
   * @param text The sentence from the label table.
   */
  empty(text: string): void {
    this.clear();
    this.states.state = { kind: "empty", message: text };
    this.states.hidden = false;
  }

  /** Show the loading state. */
  loading(): void {
    this.clear();
    this.states.state = { kind: "loading" };
    this.states.hidden = false;
  }

  /**
   * Show a failure: a sentence in the alert line, or a state with its Retry.
   *
   * @param failure What {@link libraryFailure} decided.
   */
  fail(failure: Failure): void {
    this.clear();
    if (failure.kind === "message") {
      this.alert.textContent = failure.text;
      return;
    }
    this.states.state = failure.state;
    this.states.hidden = false;
  }
}
