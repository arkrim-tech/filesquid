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
 * Files (UI-F2.8): a library, a collection or a virtual collection shows its files in a
 * `<fs-drive-view>` over the view operation of API-S-011 when the host gives the view its
 * `drive` options; the page title is then the drive view's, and a `fs-destination-context`
 * event (`detail.node`, `detail.kind`, or no detail when none is shown) tells the shell's New
 * menu where uploads would go. A folder, which the view operation does not list, and a view
 * without `drive` options say that the files cannot be listed here yet.
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
import type { DriveViewOptions, FsDriveView } from "./fs-drive-view";
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
  /** What the file view of a library, collection or virtual collection needs; none, no view. */
  drive?: Omit<DriveViewOptions, "labels" | "online">;
}

/** The kinds of node the view operation lists (API-S-011). */
const VIEWABLE: ReadonlySet<string> = new Set(["library", "collection", "virtual_collection"]);

const STYLE = `
  :host { display: block; font-family: var(--fs-font-sans); color: var(--fs-on-surface); }
  section { display: grid; gap: var(--fs-space-3); min-width: 0; }
  section.drive { gap: 0; }
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
  private drive?: FsDriveView;
  private filterText = "";

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
    this.trail = undefined;
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

  /** The sidebar filter text, handed to the file view (UI-S-007). */
  get filter(): string {
    return this.filterText;
  }

  /** Narrow the file view to rows whose name contains `text`. */
  set filter(text: string) {
    this.filterText = text;
    if (this.drive) this.drive.filter = text;
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
    const current = trail.at(-1);
    const viewable = !state && current !== undefined && options.drive && VIEWABLE.has(current.kind);
    root.querySelector("section")?.classList.toggle("drive", viewable === true);
    if (viewable) this.showDrive(body, current, options);
    else {
      this.drive = undefined;
      body.replaceChildren(...(state ? [] : destination(current, options.labels)));
    }
    this.announceContext(state ? undefined : current);
    this.dataset.state = state?.kind ?? "ready";
  }

  /** Show the file view of a node, keeping the one on screen when it is the same element. */
  private showDrive(body: HTMLElement, node: NavigationNode, options: NavigationViewOptions): void {
    if (!this.drive) {
      const drive = document.createElement("fs-drive-view") as unknown as FsDriveView;
      drive.configure({
        ...(options.drive as NonNullable<NavigationViewOptions["drive"]>),
        labels: options.labels,
        online: options.online,
      });
      drive.filter = this.filterText;
      this.drive = drive;
      body.replaceChildren(drive);
    }
    this.drive.heading = nodeName(node, options.labels);
    this.drive.node = node.node;
  }

  /** Tell the shell which node is shown, or that none is. */
  private announceContext(node: NavigationNode | undefined): void {
    const detail = node ? { node: node.node, kind: node.kind } : undefined;
    this.dispatchEvent(
      new CustomEvent("fs-destination-context", { detail, bubbles: true, composed: true }),
    );
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
