/**
 * `<fs-app-shell>`: the application frame shared by every distribution, composed as the
 * active mockup suite draws it (UI-S-025, `resources/ui-mockups/v3/05-all-files-list.svg`):
 *
 * - a 240 px sidebar on `--fs-surface-card` with the distribution's 24 px logo mark and product
 *   name (the only `h1`), the New button slot, the view filter slot, the All files and Recent
 *   views, the navigation slot (`<fs-nav-tree slot="navigation">`; the static vocabulary of the
 *   main destinations until one is slotted), then, after a 1 px divider, the single Recently
 *   deleted row as the tree's final row (UI-S-017, owner answer 2; its count once a view sets
 *   `deletedCount`), the storage block, and the library, Settings and Logs rows (Logs on the
 *   desktop host only, UI-S-021);
 * - a 60 px header with the search slot, the host badge (`<fs-host-indicator>`, UI-S-018), the
 *   `jobs` slot (the Jobs button of `<fs-jobs-panel>`, UI-1.2) and the Monitor button, and nothing else: no theme control (it lives in Settings, Appearance,
 *   owner answer 7), no API status (Settings, About only, owner answer 12) and no permanent
 *   privacy text (UI-S-004);
 * - `main` with the default slot; a 40 px footer with the `summary` and `commit-state` slots;
 *   and a 320 px file panel region shown while the `panel` slot is filled, under the header and
 *   beside `main` and the footer (mockups 35 and 26: the header keeps the full width, the footer
 *   ends where the panel starts).
 *
 * The fallback New button dispatches a bubbling, composed `fs-new` event for the menu a later
 * task adds (UI-F2.8); the fallback filter and search fields are the fields those tasks wire.
 * The `frameless` attribute (or property) hides the sidebar, header and footer for the welcome
 * screen (UI-S-027). `storage` shows the library's storage summary once a view supplies one.
 *
 * Accessibility contracts (UI-S-016): a skip control first in the Tab order moves focus to
 * `main`; `header`, a named `nav` and `main` are the landmarks; the layout uses logical
 * properties, wraps at large text sizes and stacks the navigation above `main` below 40rem;
 * when a re-render removes the focused element, focus moves to `main`.
 */
import type { Distribution } from "../distribution";
import { logoSource } from "../distribution";
import { formatLabel, type LabelTable } from "../theme/labels";
import type { ThemeStore } from "../theme/theme-store";
import { formatSize } from "../files/model";
import {
  ALL_FILES_ROUTE,
  LOGS_ROUTE,
  MONITOR_ROUTE,
  RECENT_ROUTE,
  RECENTLY_DELETED_ROUTE,
  SETTINGS_ROUTE,
} from "../app/routes";
import { LIBRARY_ROUTE } from "../libraries/model";
import { A11Y_STYLE, releaseDocumentA11yStyle, retainDocumentA11yStyle } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { focusElement, recoverFocus } from "../a11y/focus";
import { HOST_EVENT, type FsHostIndicator, type HostClient } from "./fs-host-indicator";
import type { FsSettings } from "./fs-settings";
import { icon, type IconName } from "./icons";

/** The part of the API client the shell needs (injectable in tests): the host descriptor. */
export type ShellClient = HostClient;

/** What the shell needs: the distribution's brand and vocabulary, the API client and the theme. */
export interface ShellOptions {
  /** Brand and vocabulary of the active distribution. */
  distribution: Distribution;
  /** API client: `host()` and `subscribe()` for the host badge. */
  client: ShellClient;
  /** Shared theme state, already started; the logo follows the resolved theme. */
  theme: ThemeStore;
}

/** The storage block's facts (API-S-038): bytes used, of which history, and free on the disk. */
export interface StorageSummary {
  /** Bytes the library uses, history included. */
  usedBytes: number;
  /** Bytes of version history inside `usedBytes`; absent when the host does not split it out. */
  historyBytes?: number;
  /** Bytes free on the disk that holds the library; absent when the host cannot say. */
  freeBytes?: number;
}

/** Name of the event the fallback New button dispatches. */
export const NEW_EVENT = "fs-new";

/**
 * Name of the event the fallback view filter dispatches on every input (`detail.value`): the
 * filter narrows only the current view (UI-S-007), so whoever owns the view listens for it.
 */
export const FILTER_EVENT = "fs-filter";

const STYLE = `
  :host { position: relative; display: grid; grid-template-columns: auto minmax(0, 1fr); block-size: 100vh; overflow: hidden; background: var(--fs-surface); color: var(--fs-on-surface); font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); }
  :host([frameless]) { grid-template-columns: minmax(0, 1fr); }
  :host([frameless]) nav, :host([frameless]) header, :host([frameless]) footer, :host([frameless]) aside.panel { display: none; }
  .skip { position: absolute; inset-inline-start: var(--fs-space-2); inset-block-start: var(--fs-space-2); z-index: 1; padding: var(--fs-space-2) var(--fs-space-3); border: 0; border-radius: var(--fs-radius-button); background: var(--fs-primary); color: var(--fs-on-primary); font-family: inherit; font-size: var(--fs-font-size-ui); font-weight: 600; cursor: pointer; }
  .skip:not(:focus) { width: 1px; height: 1px; padding: 0; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  nav { display: flex; flex-direction: column; gap: var(--fs-space-2); box-sizing: border-box; inline-size: 15rem; min-width: 0; min-height: 0; overflow: auto; padding: var(--fs-space-3) 0; border-inline-end: 1px solid var(--fs-outline); background: var(--fs-surface-card); }
  .brand { display: flex; align-items: center; gap: var(--fs-space-2); min-width: 0; padding: 0 var(--fs-space-3); }
  .brand img { block-size: var(--fs-space-4); inline-size: auto; }
  h1 { margin: 0; min-width: 0; overflow-wrap: anywhere; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-lead); font-weight: 700; color: var(--fs-on-surface-heading); }
  .new, .filter { box-sizing: border-box; inline-size: calc(100% - 2 * var(--fs-space-3)); margin: 0 var(--fs-space-3); }
  .new { margin-block-start: var(--fs-space-2); }
  .filter { min-block-size: 2.125rem; }
  ul { list-style: none; margin: 0; padding: 0; }
  .views, .tools, .destinations { display: flex; flex-direction: column; }
  .destinations li, .views a, .tools a, .deleted a { display: flex; align-items: center; gap: var(--fs-space-2); min-block-size: 1.875rem; padding: 0 var(--fs-space-3); color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); text-decoration: none; overflow-wrap: anywhere; transition: background-color var(--fs-motion-duration), color var(--fs-motion-duration); }
  .views a:hover, .tools a:hover, .deleted a:hover { background: var(--fs-surface-raised); color: var(--fs-on-surface); }
  .views a, .tools a, .deleted a { position: relative; }
  .views a[aria-current="page"], .tools a[aria-current="page"], .deleted a[aria-current="page"] { position: relative; background: var(--fs-primary-container); color: var(--fs-on-primary-container); font-weight: 500; }
  .views a[aria-current="page"]::before, .tools a[aria-current="page"]::before, .deleted a[aria-current="page"]::before { content: ""; position: absolute; inset-block: 0; inset-inline-start: 0; inline-size: 2.5px; background: var(--fs-brand); }
  .tree { flex: 1 1 auto; min-height: 0; overflow: auto; contain: layout paint style; }
  .deleted { margin-block-start: var(--fs-space-2); padding-block-start: var(--fs-space-2); border-block-start: 1px solid var(--fs-outline); }
  .views a[aria-current="page"] .icon, .tools a[aria-current="page"] .icon, .deleted a[aria-current="page"] .icon { color: var(--fs-primary); }
  .deleted .count { margin-inline-start: auto; color: var(--fs-on-surface-muted); }
  .storage { display: grid; gap: var(--fs-space-1); padding: var(--fs-space-3) var(--fs-space-3) 0; border-block-start: 1px solid var(--fs-outline); font-size: var(--fs-font-size-small); }
  .storage-head { display: flex; justify-content: space-between; gap: var(--fs-space-2); }
  .storage-title { margin: 0; font-weight: 500; color: var(--fs-on-surface-variant); }
  .storage-value { margin: 0; font-weight: 600; color: var(--fs-on-surface); }
  .storage-bar { block-size: 0.25rem; border-radius: var(--fs-radius-check); background: var(--fs-outline); overflow: hidden; }
  .storage-fill { block-size: 100%; background: var(--fs-brand); }
  .storage-caption { margin: 0; color: var(--fs-on-surface-muted); }
  .storage-facts { display: grid; gap: var(--fs-space-1); }
  .content { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
  .body { display: flex; flex: 1 1 auto; min-width: 0; min-height: 0; }
  .column { display: flex; flex-direction: column; flex: 1 1 auto; min-width: 0; min-height: 0; }
  header { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-3); box-sizing: border-box; min-block-size: 3.75rem; padding: var(--fs-space-2) var(--fs-space-4); border-block-end: 1px solid var(--fs-outline); }
  .search { flex: 0 1 32.5rem; min-width: 0; }
  .search-slot { flex: 0 1 auto; display: flex; min-width: 0; }
  .spacer { flex: 1 1 auto; }
  fs-host-indicator { flex: 0 0 auto; }
  .upload-state { display: inline-flex; align-items: center; gap: var(--fs-space-2); }
  .upload-state .icon { color: var(--fs-icon-muted); }
  .monitor { min-inline-size: 6rem; }
  main { flex: 1 1 auto; min-width: 0; min-height: 0; overflow: auto; padding: var(--fs-space-4); }
  footer { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: var(--fs-space-2) var(--fs-space-3); box-sizing: border-box; min-block-size: 2.5rem; padding: var(--fs-space-1) var(--fs-space-3); border-block-start: 1px solid var(--fs-outline); color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  footer p { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  aside.panel { box-sizing: border-box; flex: 0 0 auto; display: flex; flex-direction: column; inline-size: 20rem; min-width: 0; min-height: 0; overflow: auto; border-inline-start: 1px solid var(--fs-outline); background: var(--fs-surface-card); }
  aside.panel ::slotted(*) { flex: 1 0 auto; }
  [hidden] { display: none !important; }
  @media (max-width: 40rem) {
    :host { grid-template-columns: minmax(0, 1fr); block-size: auto; min-height: 100vh; overflow: visible; }
    main { overflow: visible; }
    .body { flex-direction: column; }
    nav { inline-size: auto; border-inline-end: 0; border-block-end: 1px solid var(--fs-outline); }
    aside.panel { inline-size: auto; }
  }
  @media (forced-colors: active) {
    .views a[aria-current="page"], .tools a[aria-current="page"] { forced-color-adjust: none; background: Highlight; color: HighlightText; }
  }
`;

/** A sidebar row: its label key, route and icon. */
interface Row {
  readonly key: keyof LabelTable;
  readonly href: string;
  readonly icon: IconName;
  readonly cls?: string;
}

/** The views above the tree. */
const VIEWS: readonly Row[] = [
  { key: "fileRoot", href: ALL_FILES_ROUTE, icon: "files" },
  { key: "fileRecent", href: RECENT_ROUTE, icon: "clock" },
];

/** The rows below the storage block. */
const TOOLS: readonly Row[] = [
  { key: "libraryNavLink", href: LIBRARY_ROUTE, icon: "library", cls: "library" },
  { key: "settings", href: SETTINGS_ROUTE, icon: "settings", cls: "settings" },
  { key: "logs", href: LOGS_ROUTE, icon: "logs", cls: "logs" },
];

/** Custom element that renders the application frame. */
export class FsAppShell extends HTMLElement {
  private distribution?: Distribution;
  private client?: ShellClient;
  private theme?: ThemeStore;
  private summary?: StorageSummary;
  private deleted?: number;
  private uploading?: string;
  private readonly onThemeChange = () => this.showLogo();
  private readonly onHashChange = () => this.syncCurrent();
  /** The document holding this shell's reference to the document-level rules, while connected. */
  private styledDocument?: Document;

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.addEventListener(HOST_EVENT, (event) => this.onHost(event as CustomEvent));
  }

  /**
   * Set the distribution, API client and theme store, then render. Call before or after
   * connecting.
   *
   * @param options Distribution brand and vocabulary, the API client and the theme store.
   */
  configure(options: ShellOptions): void {
    this.theme?.removeEventListener("change", this.onThemeChange);
    this.distribution = options.distribution;
    this.client = options.client;
    this.theme = options.theme;
    if (this.isConnected) this.connect();
  }

  /** Whether the sidebar, header and footer are hidden (the welcome screen, UI-S-027). */
  get frameless(): boolean {
    return this.hasAttribute("frameless");
  }

  /** Hide or show the frame around `main`. */
  set frameless(value: boolean) {
    this.toggleAttribute("frameless", value);
  }

  /** The storage summary shown in the sidebar, or undefined while none is known. */
  get storage(): StorageSummary | undefined {
    return this.summary;
  }

  /** Show a storage summary in the sidebar, or hide the facts with undefined. */
  set storage(summary: StorageSummary | undefined) {
    this.summary = summary;
    this.showStorage();
  }

  /** The footer's upload status ("Uploading · 82 of 196"), or undefined while nothing uploads. */
  get uploadStatus(): string | undefined {
    return this.uploading;
  }

  /**
   * Show the upload status in the footer, in place of the commit state, or restore the commit
   * state with undefined (UI-1.2, mockup 19).
   */
  set uploadStatus(text: string | undefined) {
    this.uploading = text;
    this.showUploadStatus();
  }

  /** The number of files in Recently deleted, or undefined while none is known. */
  get deletedCount(): number | undefined {
    return this.deleted;
  }

  /** Show the Recently deleted count at the end of its row, or hide it with undefined. */
  set deletedCount(count: number | undefined) {
    this.deleted = count;
    this.showDeletedCount();
  }

  /**
   * Take a reference to the document-level accessibility rules for slotted content, follow
   * the theme and the route, and render when attached to the document.
   */
  connectedCallback(): void {
    this.styledDocument = this.ownerDocument;
    retainDocumentA11yStyle(this.styledDocument);
    window.addEventListener("hashchange", this.onHashChange);
    this.connect();
  }

  /**
   * Stop following the theme and the route, and release the document-level rules once
   * detached; the last shell to leave removes them.
   */
  disconnectedCallback(): void {
    this.theme?.removeEventListener("change", this.onThemeChange);
    window.removeEventListener("hashchange", this.onHashChange);
    if (this.styledDocument) releaseDocumentA11yStyle(this.styledDocument);
    this.styledDocument = undefined;
  }

  /** Subscribe to the theme store and render. */
  private connect(): void {
    this.theme?.addEventListener("change", this.onThemeChange);
    this.render();
  }

  /**
   * Move focus to the main area, for the skip control and after a route change. Side effect:
   * changes focus.
   */
  focusMain(): void {
    const main = this.shadowRoot?.querySelector("main");
    if (main) focusElement(main);
  }

  /**
   * Read the host descriptor again and let a Settings page the shell holds read health and the
   * host again, for example after a token was entered (the event channel is reopened with it).
   * Network I/O via the client.
   *
   * @returns Resolves once the host badge and the Settings page show the new answers.
   */
  async refreshHealth(): Promise<void> {
    const indicator = this.shadowRoot?.querySelector<FsHostIndicator>("fs-host-indicator");
    const settings = [...this.querySelectorAll<FsSettings>(":scope > fs-settings")];
    await Promise.all([indicator?.refresh(), ...settings.map((page) => page.refresh())]);
  }

  /**
   * Build the frame. When the re-render removed the focused element, focus moves to `main`.
   */
  private render(): void {
    const root = this.shadowRoot;
    const dist = this.distribution;
    if (!root || !dist) return;
    const hadFocus = root.activeElement !== null;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + CONTROL_STYLE + STYLE;
    root.replaceChildren(
      style,
      this.skipControl(dist),
      this.sidebar(dist),
      this.contentColumn(dist),
    );
    this.showLogo();
    this.showStorage();
    this.showDeletedCount();
    this.showUploadStatus();
    this.syncCurrent();
    recoverFocus(root, hadFocus, () => root.querySelector("main"));
  }

  /** The skip control, first in the Tab order. */
  private skipControl(dist: Distribution): HTMLButtonElement {
    const skip = document.createElement("button");
    skip.type = "button";
    skip.className = "skip";
    skip.textContent = dist.labels.skipToContent;
    skip.addEventListener("click", () => this.focusMain());
    return skip;
  }

  /** The sidebar: brand, New, filter, views, tree, storage and the tool rows. */
  private sidebar(dist: Distribution): HTMLElement {
    const { labels } = dist;
    const nav = document.createElement("nav");
    nav.setAttribute("aria-label", labels.navigationMain);
    const tree = document.createElement("div");
    tree.className = "tree";
    tree.append(namedSlot("navigation", destinations(labels)), deletedRow(labels));
    nav.append(
      brand(dist),
      namedSlot("new", this.newButton(labels)),
      namedSlot("filter", this.filterField(labels)),
      namedSlot("views", rows("views", VIEWS, labels)),
      tree,
      storageBlock(labels),
      rows("tools", TOOLS, labels),
    );
    return nav;
  }

  /** The fallback view filter: dispatches `fs-filter` with the text on every input. */
  private filterField(labels: LabelTable): HTMLInputElement {
    const input = textField("filter", labels.fileFilterView, "text");
    input.addEventListener("input", () =>
      this.dispatchEvent(
        new CustomEvent(FILTER_EVENT, {
          detail: { value: input.value },
          bubbles: true,
          composed: true,
        }),
      ),
    );
    return input;
  }

  /** The fallback New button: dispatches `fs-new` for the New menu (`<fs-new-menu>`). */
  private newButton(labels: LabelTable): HTMLButtonElement {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "button primary large new";
    button.setAttribute("aria-haspopup", "menu");
    button.append(icon("plus"), document.createTextNode(labels.fileNew));
    // Sent from the button, so a menu opens below it and returns focus to it.
    button.addEventListener("click", () =>
      button.dispatchEvent(new CustomEvent(NEW_EVENT, { bubbles: true, composed: true })),
    );
    return button;
  }

  /** The header over the body: the main and footer column beside the file panel region. */
  private contentColumn(dist: Distribution): HTMLElement {
    const { labels } = dist;
    const content = document.createElement("div");
    content.className = "content";
    const main = document.createElement("main");
    main.tabIndex = -1;
    main.append(document.createElement("slot"));
    const column = document.createElement("div");
    column.className = "column";
    column.append(main, footerBar());
    const body = document.createElement("div");
    body.className = "body";
    body.append(column, this.panelRegion());
    content.append(this.header(labels), body);
    return content;
  }

  /** The header: search slot, host badge and the Monitor button. */
  private header(labels: LabelTable): HTMLElement {
    const header = document.createElement("header");
    const search = namedSlot("search", textField("search", labels.searchPlaceholder, "search"));
    search.className = "search-slot";
    const spacer = document.createElement("span");
    spacer.className = "spacer";
    const monitor = document.createElement("a");
    monitor.className = "button secondary monitor";
    monitor.href = MONITOR_ROUTE;
    monitor.append(icon("monitor"), document.createTextNode(labels.monitor));
    header.append(search, spacer, this.hostIndicator(labels), namedSlot("jobs"), monitor);
    return header;
  }

  /** The host privacy indicator, bound to the client. */
  private hostIndicator(labels: LabelTable): HTMLElement {
    const indicator = document.createElement("fs-host-indicator") as FsHostIndicator;
    if (this.client) indicator.configure({ labels, client: this.client });
    return indicator;
  }

  /** The file panel region, shown while something is slotted into it. */
  private panelRegion(): HTMLElement {
    const aside = document.createElement("aside");
    aside.className = "panel";
    aside.hidden = true;
    const slot = namedSlot("panel");
    slot.addEventListener("slotchange", () => {
      aside.hidden = slot.assignedNodes().length === 0;
    });
    aside.append(slot);
    return aside;
  }

  /** Point the sidebar logo at the variant for the theme shown now. */
  private showLogo(): void {
    const img = this.shadowRoot?.querySelector(".brand img");
    const logo = this.distribution?.logo;
    if (!img || !logo) return;
    img.setAttribute("src", logoSource(logo, this.theme?.resolved ?? "light"));
  }

  /** Fill the storage block from the summary, or hide its facts. */
  private showStorage(): void {
    const root = this.shadowRoot;
    const labels = this.distribution?.labels;
    const facts = root?.querySelector<HTMLElement>(".storage-facts");
    if (!root || !labels || !facts) return;
    const summary = this.summary;
    facts.hidden = summary === undefined;
    if (!summary) return;
    const lang = document.documentElement.lang || "en";
    const size = (bytes: number) => formatSize(bytes, labels, lang);
    setText(root, ".storage-value", size(summary.usedBytes));
    const history = root.querySelector<HTMLElement>(".storage-history");
    if (history) history.hidden = summary.historyBytes === undefined;
    if (summary.historyBytes !== undefined) {
      setText(
        root,
        ".storage-history",
        formatLabel(labels.fileStorageCaptionHistory, { history: size(summary.historyBytes) }),
      );
    }
    const known = summary.freeBytes !== undefined;
    for (const part of root.querySelectorAll<HTMLElement>(".storage-free, .storage-bar")) {
      part.hidden = !known;
    }
    if (summary.freeBytes !== undefined) {
      const free = formatLabel(labels.fileStorageFree, { free: size(summary.freeBytes) });
      setText(root, ".storage-free", free);
    }
    const fill = root.querySelector<HTMLElement>(".storage-fill");
    if (fill) fill.style.inlineSize = `${usedFraction(summary).toFixed(1)}%`;
  }

  /** Show the upload status instead of the commit state, or the commit state again. */
  private showUploadStatus(): void {
    const root = this.shadowRoot;
    const state = root?.querySelector<HTMLElement>(".upload-state");
    const commit = root?.querySelector<HTMLElement>(".commit-state");
    if (!state || !commit) return;
    state.hidden = this.uploading === undefined;
    commit.hidden = this.uploading !== undefined;
    const text = state.querySelector("span");
    if (text) text.textContent = this.uploading ?? "";
  }

  /** Fill or hide the Recently deleted count. */
  private showDeletedCount(): void {
    const count = this.shadowRoot?.querySelector<HTMLElement>(".deleted .count");
    if (!count) return;
    count.hidden = this.deleted === undefined;
    const lang = document.documentElement.lang || "en";
    count.textContent = this.deleted === undefined ? "" : this.deleted.toLocaleString(lang);
  }

  /** Mark the sidebar and header rows of the current route. */
  private syncCurrent(): void {
    // A filtered address (`#/logs?file=<uuid>`) is still the view its row names; a query
    // inside a search address is percent-encoded, so only a real query is cut off.
    const hash = window.location.hash.split("?")[0] ?? "";
    for (const link of this.shadowRoot?.querySelectorAll<HTMLAnchorElement>("a[href]") ?? []) {
      if (link.getAttribute("href") === hash && hash !== "")
        link.setAttribute("aria-current", "page");
      else link.removeAttribute("aria-current");
    }
  }

  /** Show the Logs row on the desktop host only (UI-S-021). */
  private onHost(event: CustomEvent<{ hostType?: string }>): void {
    const logs = this.shadowRoot?.querySelector<HTMLElement>("li.logs");
    if (logs) logs.hidden = event.detail?.hostType !== "desktop";
  }
}

/** The share of the disk the library uses, as a percentage, 0 when nothing is known. */
function usedFraction(summary: StorageSummary): number {
  const total = summary.usedBytes + (summary.freeBytes ?? 0);
  return total > 0 ? Math.min(100, (summary.usedBytes / total) * 100) : 0;
}

/** Set the text of the first element matching `selector`. */
function setText(root: ShadowRoot, selector: string, text: string): void {
  const target = root.querySelector(selector);
  if (target) target.textContent = text;
}

/** A named slot with optional fallback content. */
function namedSlot(name: string, ...fallback: Node[]): HTMLSlotElement {
  const slot = document.createElement("slot");
  slot.name = name;
  slot.append(...fallback);
  return slot;
}

/** The logo mark and product name. */
function brand(dist: Distribution): HTMLElement {
  const box = document.createElement("div");
  box.className = "brand";
  const img = document.createElement("img");
  img.alt = dist.logo.alt;
  const h1 = document.createElement("h1");
  h1.textContent = dist.productName;
  box.append(img, h1);
  return box;
}

/** A fallback text or search field named by its placeholder. */
function textField(cls: string, placeholder: string, type: "text" | "search"): HTMLInputElement {
  const input = document.createElement("input");
  input.type = type;
  input.className = `input ${cls}`;
  input.placeholder = placeholder;
  input.setAttribute("aria-label", placeholder);
  input.autocomplete = "off";
  return input;
}

/** The static vocabulary of the main destinations, the navigation slot's fallback. */
function destinations(labels: LabelTable): HTMLUListElement {
  const list = document.createElement("ul");
  list.className = "destinations";
  const items = [
    labels.libraries,
    labels.collections,
    labels.virtualCollections,
    labels.virtualExhibits,
  ];
  for (const text of items) {
    const li = document.createElement("li");
    li.textContent = text;
    list.append(li);
  }
  return list;
}

/** A list of sidebar rows, each an icon and a link. */
function rows(cls: string, items: readonly Row[], labels: LabelTable): HTMLUListElement {
  const list = document.createElement("ul");
  list.className = cls;
  for (const row of items) {
    const li = document.createElement("li");
    if (row.cls) li.className = row.cls;
    const link = document.createElement("a");
    link.href = row.href;
    link.append(icon(row.icon), document.createTextNode(labels[row.key]));
    li.append(link);
    list.append(li);
  }
  return list;
}

/** The single Recently deleted row, the tree's final row, with its count hidden until known. */
function deletedRow(labels: LabelTable): HTMLUListElement {
  const list = document.createElement("ul");
  list.className = "deleted";
  const li = document.createElement("li");
  const link = document.createElement("a");
  link.href = RECENTLY_DELETED_ROUTE;
  const count = document.createElement("span");
  count.className = "count";
  count.hidden = true;
  link.append(icon("trash"), document.createTextNode(labels.navigationRecentlyDeleted), count);
  li.append(link);
  list.append(li);
  return list;
}

/** The storage block: title, value, bar and the two captions (facts hidden until known). */
function storageBlock(labels: LabelTable): HTMLElement {
  const block = document.createElement("div");
  block.className = "storage";
  const head = document.createElement("div");
  head.className = "storage-head";
  const title = document.createElement("p");
  title.className = "storage-title";
  title.textContent = labels.fileStorageUsed;
  const value = document.createElement("p");
  value.className = "storage-value";
  head.append(title, value);
  const facts = document.createElement("div");
  facts.className = "storage-facts";
  facts.hidden = true;
  const bar = document.createElement("div");
  bar.className = "storage-bar";
  const fill = document.createElement("div");
  fill.className = "storage-fill";
  bar.append(fill);
  facts.append(bar, caption("storage-history"), caption("storage-free"));
  block.append(head, facts);
  return block;
}

/** One storage caption line. */
function caption(cls: string): HTMLParagraphElement {
  const p = document.createElement("p");
  p.className = `storage-caption ${cls}`;
  return p;
}

/** The footer with its summary and commit-state slots. */
function footerBar(): HTMLElement {
  const footer = document.createElement("footer");
  const summary = document.createElement("p");
  summary.className = "summary";
  summary.append(namedSlot("summary"));
  const state = document.createElement("p");
  state.className = "commit-state";
  state.append(namedSlot("commit-state"));
  const uploading = document.createElement("p");
  uploading.className = "upload-state";
  uploading.hidden = true;
  uploading.setAttribute("role", "status");
  uploading.append(icon("refresh"), document.createElement("span"));
  footer.append(summary, state, uploading);
  return footer;
}
