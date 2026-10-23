/**
 * `<fs-file-panel>`: the file panel of the FileSquid default UI (UI-F2.3; UI-S-031, mockups
 * `resources/ui-mockups/v3/35-panel-file-info.svg` and `26-panel-versions.svg`), shown in the
 * shell's 320 px `panel` region on `--fs-surface-card`. Its header holds the file-type badge, the
 * display name (14 px 600, cut with an ellipsis), the history button that opens the Versions view
 * and close. Below it, a tab strip: each tab is a panel registered in a named slot with
 * `registerTab(id, label, create)` (UI-S-003, ADR 028 Decision 6); the Details panel is the
 * File info tab, and UI-F2.15 adds Description and Provenance the same way. The history button
 * shows the view given to `setVersions(create)` in place of the tabs; its Back returns.
 *
 * Panels stay independent: each is created when it is first shown, receives the opened file as
 * its `fileRef` property (a {@link FileRef}, or null) and holds no reference to another panel.
 * Only the panel shown receives a new file, so a hidden tab reads nothing until it is shown.
 *
 * **Events** (bubbling and composed): `fs-file-panel-close` from close. The page decides what
 * closing means (the files route removes the panel from the shell).
 *
 * Accessibility (UI-S-016): the shared stylesheet comes first; the tabs follow the WAI-ARIA
 * tabs pattern (`tablist`, `tab`, `tabpanel`, arrow keys, Home and End, one Tab stop, no
 * positive tabindex); the history button reports `aria-pressed`; leaving the Versions view
 * returns focus to the history button; the layout uses logical properties only.
 */
import type { LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { focusElement } from "../a11y/focus";
import { icon } from "./icons";
import { badgeKind, extensionOf } from "../files/badge";
import type { FileRef } from "../files/model";
import type { FilePanel, PanelFactory } from "./fs-file-browser";
import { VERSIONS_CLOSE_EVENT } from "./fs-versions-panel";

/** What the frame needs. */
export interface FilePanelOptions {
  /** The active label table. */
  labels: LabelTable;
}

/** Name of the event the close button dispatches. */
export const FILE_PANEL_CLOSE_EVENT = "fs-file-panel-close";

/** A panel the frame shows: a tab, or the Versions view. */
interface Entry {
  readonly id: string;
  readonly label: string;
  readonly create: PanelFactory;
  element?: FilePanel;
  /** The file last handed to the element; undefined before the first. */
  delivered?: FileRef | null;
}

const STYLE = `
  :host { display: flex; flex-direction: column; box-sizing: border-box; min-block-size: 100%; background: var(--fs-surface-card); color: var(--fs-on-surface); font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); }
  :host([hidden]) { display: none; }
  .head { display: flex; align-items: center; gap: var(--fs-space-1); box-sizing: border-box; min-block-size: 3.25rem; padding: var(--fs-space-3) var(--fs-space-3) 0; }
  .badge { flex: 0 0 auto; margin-inline: var(--fs-space-1) var(--fs-space-1); padding: 0 var(--fs-space-2); border-radius: var(--fs-radius-badge); font-family: var(--fs-font-mono); font-size: var(--fs-font-size-badge); font-weight: 700; line-height: 1.6; text-transform: uppercase; }
  .badge:empty { display: none; }
  .badge[data-kind="document"] { background: var(--fs-badge-document); color: var(--fs-on-badge-document); }
  .badge[data-kind="data"] { background: var(--fs-badge-data); color: var(--fs-on-badge-data); }
  .badge[data-kind="media"] { background: var(--fs-badge-media); color: var(--fs-on-badge-media); }
  .badge[data-kind="other"] { background: var(--fs-surface-raised); color: var(--fs-on-surface); }
  h2.name { flex: 1 1 auto; min-width: 0; margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); font-weight: 600; color: var(--fs-on-surface-heading); }
  .icon-button { flex: 0 0 auto; display: inline-flex; align-items: center; justify-content: center; inline-size: 1.875rem; block-size: 1.75rem; padding: 0; border: 0; border-radius: var(--fs-radius-input); background: none; color: var(--fs-icon-muted); cursor: pointer; transition: background-color var(--fs-motion-duration), color var(--fs-motion-duration); }
  .icon-button:hover { background: var(--fs-surface-raised); color: var(--fs-icon); }
  .history[aria-pressed="true"], .history[aria-pressed="true"]:hover { background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  .tabs, .versions-view { display: flex; flex-direction: column; flex: 1 1 auto; min-block-size: 0; }
  .tablist { display: flex; gap: var(--fs-space-4); padding: 0 var(--fs-space-3); border-block-end: 1px solid var(--fs-outline); }
  .tab { margin: 0; padding: var(--fs-space-3) 0 var(--fs-space-1); border: 0; border-block-end: 2px solid var(--fs-surface-card); background: none; color: var(--fs-on-surface-variant); font-family: var(--fs-font-sans); font-size: var(--fs-font-size-small); font-weight: 500; line-height: 1.4; cursor: pointer; transition: color var(--fs-motion-duration), border-color var(--fs-motion-duration); }
  .tab:first-child { margin-inline-start: var(--fs-space-1); }
  .tab[aria-selected="true"] { border-block-end-color: var(--fs-primary); color: var(--fs-on-primary-container); font-weight: 600; }
  .tabpanel { display: flex; flex-direction: column; flex: 1 1 auto; min-block-size: 0; }
  .tabpanel > section, .versions-view > section { display: flex; flex-direction: column; flex: 1 1 auto; min-block-size: 0; }
  .tabpanel > section > *, .versions-view > section > * { flex: 1 1 auto; }
  [hidden] { display: none !important; }
  @media (forced-colors: active) {
    .tab[aria-selected="true"] { border-block-end-color: Highlight; }
  }
`;

/** Create an element with a class and optional text. */
function node<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const element = document.createElement(tag);
  element.className = className;
  if (text !== undefined) element.textContent = text;
  return element;
}

/** Custom element that frames the panels of one opened file. */
export class FsFilePanel extends HTMLElement {
  private options?: FilePanelOptions;
  private file: FileRef | null = null;
  private tabs: Entry[] = [];
  private versions?: Entry;
  private selected = 0;
  private versionsOpen = false;

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.addEventListener(VERSIONS_CLOSE_EVENT, () => this.showVersions(false, true));
  }

  /**
   * Set the label table and render. Call before or after registering panels.
   *
   * @param options The active label table.
   */
  configure(options: FilePanelOptions): void {
    this.options = options;
    this.render();
  }

  /** The opened file, or null. */
  get fileRef(): FileRef | null {
    return this.file;
  }

  /** Show another file (null clears the header and the panel shown). */
  set fileRef(file: FileRef | null) {
    this.file = file;
    this.showHeader();
    this.deliver();
  }

  /** Whether the Versions view is shown in place of the tabs. */
  get versionsShown(): boolean {
    return this.versionsOpen;
  }

  /**
   * Add a tab: a panel in the named slot `id`, labelled `label`. A tab registered again under
   * the same id replaces the old one. The panel is created when its tab is first shown.
   *
   * @param id The slot name, for example `details`.
   * @param label The tab's words, from the label table.
   * @param create Builds the panel element.
   */
  registerTab(id: string, label: string, create: PanelFactory): void {
    const entry: Entry = { id, label, create };
    const at = this.tabs.findIndex((tab) => tab.id === id);
    if (at >= 0) this.tabs[at] = entry;
    else this.tabs.push(entry);
    this.render();
  }

  /**
   * Set the view the history button shows in place of the tabs (the Versions view). It is
   * created when first shown and dispatches `fs-versions-close` to return.
   *
   * @param create Builds the view element.
   */
  setVersions(create: PanelFactory): void {
    this.versions = { id: "versions", label: "", create };
    this.render();
  }

  /** Build the header, the tab strip and panels, and the Versions container. */
  private render(): void {
    const root = this.shadowRoot;
    const labels = this.options?.labels;
    if (!root || !labels) return;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + CONTROL_STYLE + STYLE;
    for (const entry of [...this.tabs, this.versions]) entry?.element?.remove();
    root.replaceChildren(style, this.header(labels), this.tabArea(labels), this.versionsArea());
    this.selected = Math.min(this.selected, Math.max(0, this.tabs.length - 1));
    this.showHeader();
    this.applyMode();
  }

  /** The header: badge, name, history button and close. */
  private header(labels: LabelTable): HTMLElement {
    const head = node("header", "head");
    const history = node("button", "icon-button history");
    history.type = "button";
    history.setAttribute("aria-label", labels.filePanelHistory);
    history.setAttribute("aria-pressed", "false");
    history.append(icon("history", 20));
    history.addEventListener("click", () => this.showVersions(!this.versionsOpen, false));
    history.hidden = this.versions === undefined;
    const close = node("button", "icon-button close");
    close.type = "button";
    close.setAttribute("aria-label", labels.filePanelClose);
    close.append(icon("close", 20));
    close.addEventListener("click", () =>
      this.dispatchEvent(
        new CustomEvent(FILE_PANEL_CLOSE_EVENT, { bubbles: true, composed: true }),
      ),
    );
    head.append(node("span", "badge"), node("h2", "name"), history, close);
    return head;
  }

  /** The tab strip and one tab panel per registered tab. */
  private tabArea(labels: LabelTable): HTMLElement {
    const area = node("div", "tabs");
    const list = node("div", "tablist");
    list.setAttribute("role", "tablist");
    list.setAttribute("aria-label", labels.filePanelTabs);
    list.addEventListener("keydown", (event) => this.onTabKey(event));
    const panels = this.tabs.map((entry, index) => {
      const tab = node("button", "tab", entry.label);
      tab.type = "button";
      tab.id = `tab-${entry.id}`;
      tab.setAttribute("role", "tab");
      tab.setAttribute("aria-controls", `panel-${entry.id}`);
      tab.addEventListener("click", () => this.select(index, false));
      list.append(tab);
      const panel = node("div", "tabpanel");
      panel.id = `panel-${entry.id}`;
      panel.dataset.slot = entry.id;
      panel.setAttribute("role", "tabpanel");
      panel.setAttribute("aria-labelledby", tab.id);
      return panel;
    });
    area.append(list, ...panels);
    return area;
  }

  /** The container of the Versions view. */
  private versionsArea(): HTMLElement {
    const area = node("div", "versions-view");
    area.hidden = true;
    return area;
  }

  /** Fill the badge and the name from the file. */
  private showHeader(): void {
    const root = this.shadowRoot;
    const badge = root?.querySelector<HTMLElement>(".badge");
    const name = root?.querySelector<HTMLElement>("h2.name");
    if (!badge || !name) return;
    const display = this.file?.display_name ?? "";
    const extension = extensionOf(display);
    badge.textContent = extension;
    badge.dataset.kind = badgeKind(extension);
    name.textContent = display;
    if (display) name.title = display;
    else name.removeAttribute("title");
  }

  /** Select a tab; with `focus`, move focus to it (keyboard). */
  private select(index: number, focus: boolean): void {
    this.selected = index;
    this.applyMode();
    const tab = this.shadowRoot?.querySelectorAll<HTMLElement>('[role="tab"]')[index];
    if (focus && tab) focusElement(tab);
  }

  /** Arrow keys, Home and End move between tabs and select (following the writing direction). */
  private onTabKey(event: KeyboardEvent): void {
    const count = this.tabs.length;
    if (count === 0) return;
    const rtl = getComputedStyle(this).direction === "rtl";
    const forward = rtl ? "ArrowLeft" : "ArrowRight";
    const backward = rtl ? "ArrowRight" : "ArrowLeft";
    const targets: Record<string, number> = {
      [forward]: (this.selected + 1) % count,
      [backward]: (this.selected - 1 + count) % count,
      Home: 0,
      End: count - 1,
    };
    const target = targets[event.key];
    if (target === undefined) return;
    event.preventDefault();
    this.select(target, true);
  }

  /** Show or hide the Versions view; when leaving it with `focus`, focus the history button. */
  private showVersions(open: boolean, focus: boolean): void {
    if (!this.versions) return;
    this.versionsOpen = open;
    this.applyMode();
    const history = this.shadowRoot?.querySelector<HTMLElement>("button.history");
    if (focus && history) focusElement(history);
  }

  /** Show the selected tab or the Versions view, and hand the file to the panel shown. */
  private applyMode(): void {
    const root = this.shadowRoot;
    if (!root) return;
    const tabs = [...root.querySelectorAll<HTMLElement>('[role="tab"]')];
    const panels = [...root.querySelectorAll<HTMLElement>('[role="tabpanel"]')];
    tabs.forEach((tab, index) => {
      tab.setAttribute("aria-selected", String(index === this.selected));
      tab.tabIndex = index === this.selected ? 0 : -1;
    });
    panels.forEach((panel, index) => void (panel.hidden = index !== this.selected));
    const area = root.querySelector<HTMLElement>(".tabs");
    const view = root.querySelector<HTMLElement>(".versions-view");
    if (area) area.hidden = this.versionsOpen;
    if (view) view.hidden = !this.versionsOpen;
    root.querySelector("button.history")?.setAttribute("aria-pressed", String(this.versionsOpen));
    this.deliver();
  }

  /** The entry shown now and its container. */
  private active(): { entry: Entry; host: HTMLElement } | undefined {
    const root = this.shadowRoot;
    const entry = this.versionsOpen ? this.versions : this.tabs[this.selected];
    const host = this.versionsOpen
      ? root?.querySelector<HTMLElement>(".versions-view")
      : root?.querySelectorAll<HTMLElement>('[role="tabpanel"]')[this.selected];
    return entry && host ? { entry, host } : undefined;
  }

  /** Create the panel shown if needed, mount it in its slot and hand it the file if it changed. */
  private deliver(): void {
    const shown = this.active();
    if (!shown) return;
    const { entry, host } = shown;
    if (!entry.element && this.file === null) return;
    if (!entry.element) entry.element = entry.create();
    if (entry.element.parentElement?.parentElement !== host) {
      const section = node("section", "");
      section.dataset.slot = entry.id;
      section.append(entry.element);
      host.replaceChildren(section);
    }
    if (entry.delivered === this.file) return;
    entry.delivered = this.file;
    entry.element.fileRef = this.file;
  }
}
