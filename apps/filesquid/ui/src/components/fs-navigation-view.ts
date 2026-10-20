/**
 * `<fs-navigation-view>`: the main area for one navigation destination (UI-F2.1b part 2,
 * F-004 default UI). It reads one listing of the destination (`list(node, null, 1)`, API-S-021)
 * for its `ancestors` and fills `<fs-breadcrumb>` from them, repository first and the
 * destination last as the current page, and heads the view with the destination's name. It
 * follows the source's changes and reads again when a change names a node of the trail (a
 * rename or move of the destination or of an ancestor), or everything changed.
 *
 * States (`<fs-states>`): loading until the answer; a gone destination for `not_found` (an
 * unknown node, one the caller may not read, or one under an unreadable ancestor); unavailable
 * for a 503 (the host has no navigation service yet); error, offline or denied otherwise. No
 * trail is shown in any of them, so nothing is guessed.
 *
 * Files: the file listing of a destination needs a listing by navigation node that the API
 * does not offer yet (`<fs-file-browser>` lists one OCFL object), so the view says that the
 * files cannot be listed here yet; the browser takes this place when the API offers it.
 */
import type { LabelTable } from "../theme/labels";
import {
  breadcrumbItems,
  isNotFound,
  navigationHref,
  nodeName,
  type NavigationChange,
  type NavigationNode,
  type NavigationSource,
} from "../navigation/model";
import { A11Y_STYLE } from "./a11y-style";
import { failureState, type ViewState } from "./view-state";
import { RETRY_EVENT, type FsStates } from "./fs-states";
import type { FsBreadcrumb } from "./fs-breadcrumb";

/** What the view needs. */
export interface NavigationViewOptions {
  /** The active label table. */
  labels: LabelTable;
  /** Answers the navigation listing; the view never fetches by itself. */
  source: NavigationSource;
  /** Whether the browser is online; defaults to `navigator.onLine`. */
  online?: () => boolean;
  /** The route of a node; defaults to `navigationHref`. */
  href?: (node: NavigationNode) => string;
}

const STYLE = `
  :host { display: block; font-family: var(--fs-font-sans); color: var(--fs-on-surface); }
  section { display: grid; gap: var(--fs-space-3); min-width: 0; }
  h2 { margin: 0; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-title); font-weight: 700; color: var(--fs-on-surface); overflow-wrap: anywhere; }
  .files { margin: 0; color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-body); }
  [hidden] { display: none !important; }
`;

/** Custom element that shows one navigation destination with its breadcrumb. */
export class FsNavigationView extends HTMLElement {
  private options?: NavigationViewOptions;
  private handle?: string;
  /** The trail shown now, repository first; undefined while there is none. */
  private trail?: readonly NavigationNode[];
  private generation = 0;
  private stopFollowing?: () => void;

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.addEventListener(RETRY_EVENT, (event) => {
      event.stopPropagation();
      void this.reload();
    });
  }

  /**
   * Set the labels and source, then read the destination. Call before or after connecting.
   *
   * @param options Labels, source, and the optional online check and route.
   */
  configure(options: NavigationViewOptions): void {
    this.disconnect();
    this.options = options;
    if (this.isConnected) this.connect();
  }

  /** The handle of the destination shown. */
  get node(): string | undefined {
    return this.handle;
  }

  /** Show another destination; its trail is read again. */
  set node(handle: string | undefined) {
    this.handle = handle;
    if (this.isConnected) void this.reload();
  }

  /** Follow the source and read the destination while attached. */
  connectedCallback(): void {
    this.connect();
  }

  /** Stop following the source once detached; answers still in flight are dropped. */
  disconnectedCallback(): void {
    this.disconnect();
    this.generation += 1;
  }

  /**
   * Read the destination's trail again and show it, or the state for why it cannot be read.
   * Shows loading meanwhile only when no trail of this destination is on screen, so a refresh
   * after a change keeps the breadcrumb (and focus in it) in place. An answer for an older read
   * is dropped. I/O: one source read.
   *
   * @returns Resolves once the answer or the failure is shown.
   */
  async reload(): Promise<void> {
    const source = this.options?.source;
    const handle = this.handle;
    if (!source || handle === undefined) return;
    const generation = ++this.generation;
    if (this.trail?.at(-1)?.node !== handle) {
      this.trail = undefined;
      this.show({ kind: "loading" });
    }
    try {
      const page = await source.list(handle, null, 1);
      if (generation === this.generation) this.accept(page.ancestors);
    } catch (error) {
      if (generation === this.generation) this.fail(error);
    }
  }

  /** Subscribe to the source and read the destination. */
  private connect(): void {
    const source = this.options?.source;
    if (!source) return;
    this.disconnect();
    this.stopFollowing = source.subscribe?.((change) => this.onChange(change));
    void this.reload();
  }

  /** Stop following the source. */
  private disconnect(): void {
    this.stopFollowing?.();
    this.stopFollowing = undefined;
  }

  /**
   * Read again when everything changed, nothing is shown, or the children of a node above the
   * destination changed (the destination or an ancestor may have been renamed or moved). A
   * change below the destination cannot change its trail.
   */
  private onChange(change: NavigationChange): void {
    const above = this.trail?.slice(0, -1);
    const touched = "all" in change || !above || above.some((n) => change.parents.includes(n.node));
    if (touched) void this.reload();
  }

  /**
   * Show a trail. One that does not start at a repository or does not end at the destination
   * breaks the contract and is shown as an error, never as a partial breadcrumb.
   */
  private accept(ancestors: readonly NavigationNode[]): void {
    const current = ancestors.at(-1);
    const rooted = ancestors[0]?.kind === "repository";
    if (!current || !rooted || current.node !== this.handle) {
      this.trail = undefined;
      console.error("FileSquid navigation answer has no trail for the destination");
      this.show({ kind: "error" });
      return;
    }
    this.trail = ancestors;
    this.show(undefined);
  }

  /** Show why the destination cannot be read: gone for `not_found`, else the failure state. */
  private fail(error: unknown): void {
    this.trail = undefined;
    console.warn("FileSquid could not read a navigation destination", error);
    const labels = (this.options as NavigationViewOptions).labels;
    const online = this.options?.online ?? (() => navigator.onLine);
    const gone: ViewState = { kind: "empty", message: labels.navigationGone };
    this.show(isNotFound(error) ? gone : failureState(error, online()));
  }

  /** The stylesheet, breadcrumb, heading, files note and states, created once. */
  private skeleton(root: ShadowRoot, labels: LabelTable) {
    let section = root.querySelector("section");
    if (!section) {
      const style = document.createElement("style");
      style.textContent = A11Y_STYLE + STYLE;
      section = document.createElement("section");
      section.append(
        document.createElement("fs-breadcrumb"),
        document.createElement("fs-states"),
        document.createElement("div"),
      );
      root.replaceChildren(style, section);
    }
    const [crumb, states, body] = [...section.children] as [FsBreadcrumb, FsStates, HTMLElement];
    crumb.configure({ labels });
    states.configure({ labels });
    return { crumb, states, body };
  }

  /** Show the trail (state undefined) or a state with no trail. */
  private show(state: ViewState | undefined): void {
    const root = this.shadowRoot;
    const options = this.options;
    if (!root || !options) return;
    const { crumb, states, body } = this.skeleton(root, options.labels);
    const trail = state ? [] : (this.trail ?? []);
    const href = options.href ?? navigationHref;
    crumb.items = breadcrumbItems(trail, options.labels, href);
    states.hidden = state === undefined;
    if (state) states.state = state;
    body.replaceChildren(...(state ? [] : destination(trail.at(-1), options.labels)));
    this.dataset.state = state?.kind ?? "ready";
  }
}

/** The heading and files note of a destination that was read. */
function destination(node: NavigationNode | undefined, labels: LabelTable): HTMLElement[] {
  if (!node) return [];
  const heading = document.createElement("h2");
  heading.textContent = nodeName(node, labels);
  const files = document.createElement("p");
  files.className = "files";
  files.textContent = labels.navigationFilesPending;
  return [heading, files];
}
