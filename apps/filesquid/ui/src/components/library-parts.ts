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
import { CONTROL_STYLE } from "./control-style";
import type { FsStates } from "./fs-states";
import type { HostClient } from "./fs-host-indicator";

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
  /**
   * Start page only: the client for the host descriptor, so the welcome screen's privacy box
   * states who can read the files from `GET /api/v1/host` (UI-S-004, UI-S-027). Without it the
   * box is left out rather than guessed.
   */
  host?: HostClient;
  /** Start page only: the release version for the footer line `appFooter`; left out if absent. */
  version?: string;
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

/**
 * The pages' stylesheet (UI-S-027, mockups 01 to 04, 09 and 41 to 43); each page puts
 * {@link A11Y_STYLE} and {@link CONTROL_STYLE} before it. The welcome page centres a 56 px
 * title, a 17 px lead, the LIBRARIES eyebrow, three 332 × 248 px cards (radius 10 px, a 1.5 px
 * `--fs-outline-strong` border, the Create card's 2 px `--fs-primary`) with 21 px 600 heading-font
 * titles, the raised privacy box and the footer line. The library page shows a 30 px title, the
 * facts as 13 px label and value rows (values at 160 px, the ID in mono) on the left and the
 * actions on the right, and the W010 note in a box with a 1 px `--fs-error` border. The repair
 * page shows mono object lines, the consent dialog on the overlay surface over the scrim, and
 * the results and refusals. Eyebrows are 11 px 400 upper case in `--fs-on-surface-muted`.
 */
export const LIBRARY_STYLE = `
  :host { display: block; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); color: var(--fs-on-surface); }
  .page { display: grid; align-content: start; gap: var(--fs-space-3); max-inline-size: 72rem; }
  .page.start { box-sizing: border-box; max-inline-size: 64.75rem; margin: 0 auto; padding: var(--fs-space-7) 0 var(--fs-space-5); gap: var(--fs-space-4); }
  h2 { margin: 0; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-page); font-weight: 700; color: var(--fs-on-surface-heading); overflow-wrap: anywhere; }
  .start h2 { font-size: var(--fs-font-size-display); }
  h3 { margin: 0; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-eyebrow); font-weight: 400; letter-spacing: 0.1em; text-transform: uppercase; color: var(--fs-on-surface-muted); }
  .cards h3 { margin-block-start: var(--fs-space-3); font-family: var(--fs-font-heading); font-size: var(--fs-font-size-card); font-weight: 600; letter-spacing: normal; text-transform: none; color: var(--fs-on-surface-heading); }
  [role="dialog"] h3 { font-family: var(--fs-font-heading); font-size: var(--fs-font-size-heading); font-weight: 700; letter-spacing: normal; text-transform: none; color: var(--fs-on-surface-heading); }
  p { margin: 0; overflow-wrap: anywhere; }
  .intro { color: var(--fs-on-surface); font-size: var(--fs-font-size-ui); }
  .start .intro { max-inline-size: 40rem; font-size: var(--fs-font-size-lead); }
  .help, .note, .truncated { color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  .eyebrow { margin-block-start: var(--fs-space-3); }
  .cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(16rem, 20.75rem)); gap: var(--fs-space-4); }
  .cards section { display: flex; flex-direction: column; align-items: flex-start; gap: var(--fs-space-2); box-sizing: border-box; min-block-size: 15.5rem; padding: var(--fs-space-4); border: 1.5px solid var(--fs-outline-strong); border-radius: var(--fs-radius-dialog); background: var(--fs-surface-card); }
  .cards section[data-flow="create"] { border: 2px solid var(--fs-primary); }
  .cards section .button { margin-block-start: auto; }
  .cards .icon { color: var(--fs-icon); }
  section { display: grid; align-content: start; gap: var(--fs-space-2); min-inline-size: 0; }
  .columns { display: grid; grid-template-columns: minmax(0, 40rem) minmax(0, 28rem); gap: var(--fs-space-6); align-items: start; }
  .side { display: grid; gap: var(--fs-space-4); align-content: start; }
  .privacy { box-sizing: border-box; padding: var(--fs-space-3) var(--fs-space-4); border-radius: var(--fs-radius); background: var(--fs-surface-raised); }
  .app-footer { color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  .crumb { justify-self: start; color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); text-decoration: none; }
  .crumb:hover { color: var(--fs-on-surface); text-decoration: underline; }
  .button[aria-disabled="true"] { border-color: var(--fs-outline); background: var(--fs-surface-raised); color: var(--fs-on-surface-placeholder); cursor: default; }
  dl { display: grid; grid-template-columns: 10rem minmax(0, 1fr); margin: 0; font-size: var(--fs-font-size-small); }
  dt, dd { margin: 0; padding: var(--fs-space-2) 0; border-block-end: 1px solid var(--fs-outline); overflow-wrap: anywhere; }
  dt { color: var(--fs-on-surface-muted); }
  dd { color: var(--fs-on-surface); }
  .mono, .offer li, .results li, [role="dialog"] li, .result li { font-family: var(--fs-font-mono); font-size: var(--fs-font-size-small); }
  ul { display: grid; gap: var(--fs-space-2); margin: 0; padding: 0; list-style: none; overflow-wrap: anywhere; }
  .results li { padding-block: var(--fs-space-2); border-block-end: 1px solid var(--fs-outline); }
  [role="dialog"] li { color: var(--fs-on-surface-variant); }
  .status:empty, .alert:empty { display: none; }
  .status { color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  .alert, section[data-section="repair-note"] { box-sizing: border-box; max-inline-size: 51.25rem; padding: var(--fs-space-3); border: 1px solid var(--fs-error); border-radius: var(--fs-radius); background: var(--fs-surface-card); color: var(--fs-on-surface); font-size: var(--fs-font-size-small); font-weight: 600; }
  section[data-section="repair-note"] { max-inline-size: 28rem; gap: var(--fs-space-1); }
  section[data-section="repair-note"] .reason { color: var(--fs-on-surface-variant); font-weight: 400; }
  section[data-section="repair-note"] .button { justify-self: start; padding-inline: 0; }
  .actions { display: flex; flex-wrap: wrap; gap: var(--fs-space-2); }
  .consent { font-size: var(--fs-font-size-ui); }
`;

/** The stylesheet of a page: the shared accessibility and control rules first, then its own. */
export function pageStyle(): HTMLStyleElement {
  const style = document.createElement("style");
  style.textContent = A11Y_STYLE + CONTROL_STYLE + LIBRARY_STYLE;
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
 * Create a button that names its action in `data-action`: a secondary button, or the screen's
 * one primary button (UI-S-024: at most one filled primary button per screen and per dialog).
 *
 * @param action The action name, for example `create`.
 * @param text The button's label from the label table.
 * @param kind `secondary` (default) or `primary`.
 * @returns The button.
 */
export function actionButton(
  action: string,
  text: string,
  kind: "primary" | "secondary" = "secondary",
): HTMLButtonElement {
  const button = node("button", `button ${kind}`, text);
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
