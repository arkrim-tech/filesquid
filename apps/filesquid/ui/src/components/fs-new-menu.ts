/**
 * `<fs-new-menu>`: the menu of the sidebar's one New control (UI-F2.8, UI-S-006, UI-S-026,
 * mockup 10): Upload files… (⌘O), Upload folder… and, after a divider, New folder (⇧⌘N). It is
 * headless: the visible control is the app shell's New button, which dispatches a bubbling,
 * composed `fs-new` event; this element listens for it on the document and opens the menu below
 * the button that sent it. Choosing an item dispatches `fs-new-action` (`detail.action`:
 * `upload-files`, `upload-folder` or `new-folder`, `detail.destination`: the node the files go
 * into), so UI-F2.17 handles the uploads and the folder creation without a second control.
 *
 * Every item is enabled only while a collection or a folder inside one is shown (`destination`),
 * because there is no upload at a library's top level, in a virtual collection or in a view that
 * is not a collection (UI-S-002 **Destination**, ADR 041); elsewhere it is disabled and says why
 * in its tooltip. While a host cannot place uploads yet (`uploads` false) the items are disabled
 * with the generic "not available here yet" reason. Words from the label table, tokens only.
 */
import type { LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { newEntries } from "./drive-entries";
import { STYLE as MENU_STYLE, openMenu, type OpenMenu } from "./drive-menu";
import { NEW_EVENT } from "./fs-app-shell";

/** Name of the event a chosen item dispatches. */
export const NEW_ACTION_EVENT = "fs-new-action";

/** What a chosen New item asks for. */
export type NewAction = "upload-files" | "upload-folder" | "new-folder";

/** What the menu needs. */
export interface NewMenuOptions {
  /** The active label table. */
  readonly labels: LabelTable;
}

const STYLE = `
  :host { display: contents; }
`;

/** Custom element that opens the New menu for the shell's New button. */
export class FsNewMenu extends HTMLElement {
  private options?: NewMenuOptions;
  private place?: string;
  private ready = false;
  private menu?: OpenMenu;
  private readonly onNew = (event: Event) => this.open(event);

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the labels and render the menu layer. Call before or after connecting.
   *
   * @param options The active label table.
   */
  configure(options: NewMenuOptions): void {
    this.options = options;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + CONTROL_STYLE + STYLE + MENU_STYLE;
    const layer = document.createElement("div");
    layer.className = "layer";
    (this.shadowRoot as ShadowRoot).replaceChildren(style, layer);
  }

  /** The collection or folder shown, the destination of an upload; undefined elsewhere. */
  get destination(): string | undefined {
    return this.place;
  }

  /** Set the collection or folder shown; undefined where there is no upload (UI-S-002). */
  set destination(node: string | undefined) {
    this.place = node;
  }

  /** Whether this host can place uploads yet; false until UI-F2.17 and API-F2.14. */
  get uploads(): boolean {
    return this.ready;
  }

  /** Say whether this host can place uploads. */
  set uploads(value: boolean) {
    this.ready = value;
  }

  /** Listen for the shell's New button. */
  connectedCallback(): void {
    document.addEventListener(NEW_EVENT, this.onNew);
  }

  /** Stop listening and close an open menu. */
  disconnectedCallback(): void {
    document.removeEventListener(NEW_EVENT, this.onNew);
    this.menu?.close(false);
  }

  /** Open the menu below the control that dispatched `fs-new`. */
  private open(event: Event): void {
    const labels = this.options?.labels;
    const layer = this.shadowRoot?.querySelector<HTMLElement>(".layer");
    const opener = event.composedPath()[0];
    if (!labels || !layer || !(opener instanceof HTMLElement)) return;
    if (this.menu?.element.isConnected) {
      this.menu.close(true);
      return;
    }
    const enabled = this.place !== undefined && this.ready;
    const entries = newEntries(labels, enabled);
    const reasoned =
      enabled || this.place === undefined
        ? entries
        : withReason(entries, labels.fileActionUnavailable);
    this.menu = openMenu(layer, { element: opener }, opener, {
      className: "new-menu",
      label: labels.fileNewMenu,
      entries: reasoned,
      onChoose: (id) => this.choose(id),
    });
  }

  /** Dispatch the chosen action. */
  private choose(id: string): void {
    const detail = { action: id as NewAction, destination: this.place };
    this.dispatchEvent(
      new CustomEvent(NEW_ACTION_EVENT, { detail, bubbles: true, composed: true }),
    );
  }
}

/** The same entries with another reason on each disabled item. */
function withReason(
  entries: ReturnType<typeof newEntries>,
  reason: string,
): ReturnType<typeof newEntries> {
  return entries.map((entry) =>
    entry.kind === "item" && entry.disabled ? { ...entry, reason } : entry,
  );
}
