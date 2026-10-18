/**
 * `<fs-app-shell>`: application frame shared by every distribution. Shows the
 * distribution's brand (logo and product name, with a dark-surface logo in the dark theme),
 * the theme control, its vocabulary for the main destinations, and the API status.
 * An authorisation denial is shown with the label for its reason (API-S-004).
 *
 * Accessibility contracts (UI-S-016): a skip control first in the Tab order moves focus to
 * `main`; `header`, a named `nav` and `main` are the landmarks; the layout uses logical
 * properties, wraps at large text sizes and stacks the navigation above `main` below 40rem;
 * when a re-render removes the focused element, focus moves to `main`.
 *
 * Navigation (UI-F2.1a): the `nav` landmark has a `navigation` named slot for the distribution's
 * navigation tree (`<fs-nav-tree slot="navigation">`); until one is slotted it shows the
 * vocabulary of the main destinations as a static list. The Monitor link follows the slot.
 */
import type { Distribution } from "../distribution";
import { logoSource } from "../distribution";
import type { LabelTable } from "../theme/labels";
import type { ThemeStore } from "../theme/theme-store";
import type { HealthStatus } from "../api/client";
import { MONITOR_ROUTE } from "../app/routes";
import { denialLabel } from "./denial";
import type { FsThemeControl } from "./fs-theme-control";
import { A11Y_STYLE, releaseDocumentA11yStyle, retainDocumentA11yStyle } from "./a11y-style";
import { focusElement, recoverFocus } from "../a11y/focus";

/** The part of the API client the shell needs (injectable in tests). */
export interface ShellClient {
  /** Read API health. */
  health(): Promise<HealthStatus>;
}

/** What the shell needs: the distribution's brand and vocabulary, the API client and the theme. */
export interface ShellOptions {
  /** Brand and vocabulary of the active distribution. */
  distribution: Distribution;
  /** API client (only `health()` is used by the shell itself). */
  client: ShellClient;
  /** Shared theme state, already started. */
  theme: ThemeStore;
}

const STYLE = `
  :host { position: relative; display: grid; grid-template-rows: auto 1fr; grid-template-columns: minmax(0, 1fr); min-height: 100vh; background: var(--fs-surface); color: var(--fs-on-surface); font-family: var(--fs-font-sans); }
  .skip { position: absolute; inset-inline-start: var(--fs-space-2); inset-block-start: var(--fs-space-2); z-index: 1; padding: var(--fs-space-2) var(--fs-space-3); border: 0; border-radius: var(--fs-radius); background: var(--fs-primary); color: var(--fs-on-primary); font-family: inherit; font-size: var(--fs-font-size-body); font-weight: 600; cursor: pointer; }
  .skip:not(:focus) { width: 1px; height: 1px; padding: 0; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  header { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-3); padding: var(--fs-space-3) var(--fs-space-4); border-block-end: 1px solid var(--fs-outline); }
  h1 { margin: 0; min-width: 0; overflow-wrap: anywhere; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-title); font-weight: 700; color: var(--fs-on-surface); }
  img { height: 2rem; width: auto; }
  fs-theme-control { margin-inline-start: auto; }
  .body { display: flex; min-width: 0; }
  nav { display: flex; flex-direction: column; gap: var(--fs-space-4); flex: 0 0 15rem; box-sizing: border-box; min-width: 0; padding: var(--fs-space-4); background: var(--fs-surface-container); }
  ul { list-style: none; margin: 0; padding: 0; }
  li { padding: var(--fs-space-2) var(--fs-space-3); border-radius: var(--fs-radius); }
  a { color: inherit; text-decoration: none; border-radius: var(--fs-radius); }
  main { flex: 1 1 auto; min-width: 0; padding: var(--fs-space-4); }
  [role="status"] { font-family: var(--fs-font-mono); color: var(--fs-on-surface-variant); overflow-wrap: anywhere; }
  [role="alert"] { color: var(--fs-error); overflow-wrap: anywhere; }
  @media (max-width: 40rem) {
    .body { flex-direction: column; }
    nav { flex-basis: auto; }
  }
`;

/** Custom element that renders the application frame. */
export class FsAppShell extends HTMLElement {
  private distribution?: Distribution;
  private client?: ShellClient;
  private theme?: ThemeStore;
  /** Number of the newest health read; answers to older reads are discarded. */
  private healthRequest = 0;
  private readonly onThemeChange = () => this.showLogo();
  /** The document holding this shell's reference to the document-level rules, while connected. */
  private styledDocument?: Document;

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
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

  /**
   * Take a reference to the document-level accessibility rules for slotted content, follow
   * the theme and render when attached to the document.
   */
  connectedCallback(): void {
    this.styledDocument = this.ownerDocument;
    retainDocumentA11yStyle(this.styledDocument);
    this.connect();
  }

  /**
   * Stop following the theme and release the document-level rules once detached; the last
   * shell to leave removes them.
   */
  disconnectedCallback(): void {
    this.theme?.removeEventListener("change", this.onThemeChange);
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
   * Build the frame, then load API status. When the re-render removed the focused element,
   * focus moves to `main`.
   */
  private render(): void {
    const root = this.shadowRoot;
    const dist = this.distribution;
    if (!root || !dist) return;
    const { labels } = dist;
    const hadFocus = root.activeElement !== null;
    root.innerHTML = `
      <style>${A11Y_STYLE}${STYLE}</style>
      <button class="skip" type="button"></button>
      <header>
        <img alt="">
        <h1></h1>
      </header>
      <div class="body">
        <nav><slot name="navigation"><ul class="destinations"></ul></slot><ul class="tools"></ul></nav>
        <main tabindex="-1"><p class="api"></p><slot></slot></main>
      </div>`;
    this.fillText(root, dist);
    this.showLogo();
    this.mountThemeControl(root, labels);
    const items = [
      labels.libraries,
      labels.collections,
      labels.virtualCollections,
      labels.virtualExhibits,
    ];
    root.querySelector("ul.destinations")?.replaceChildren(...items.map((text) => li(text)));
    root.querySelector("ul.tools")?.replaceChildren(li(labels.monitor, MONITOR_ROUTE));
    recoverFocus(root, hadFocus, () => root.querySelector("main"));
    void this.loadHealth();
  }

  /** Word the frame from the distribution: logo alt, product name, skip control and nav name. */
  private fillText(root: ShadowRoot, dist: Distribution): void {
    root.querySelector("img")?.setAttribute("alt", dist.logo.alt);
    const h1 = root.querySelector("h1");
    if (h1) h1.textContent = dist.productName;
    const skip = root.querySelector<HTMLButtonElement>(".skip");
    if (skip) {
      skip.textContent = dist.labels.skipToContent;
      skip.addEventListener("click", () => this.focusMain());
    }
    root.querySelector("nav")?.setAttribute("aria-label", dist.labels.navigationMain);
  }

  /** Point the header logo at the variant for the theme shown now. */
  private showLogo(): void {
    const img = this.shadowRoot?.querySelector("header img");
    const logo = this.distribution?.logo;
    if (!img || !logo) return;
    img.setAttribute("src", logoSource(logo, this.theme?.resolved ?? "light"));
  }

  /** Add the theme control to the header, bound to the shared store. */
  private mountThemeControl(root: ShadowRoot, labels: LabelTable): void {
    if (!this.theme) return;
    const control = document.createElement("fs-theme-control") as FsThemeControl;
    control.configure({ store: this.theme, labels });
    root.querySelector("header")?.append(control);
  }

  /**
   * Read API health again and update the status line, for example after a token was entered.
   * Network I/O via the client.
   *
   * @returns Resolves once the status line shows the new answer.
   */
  async refreshHealth(): Promise<void> {
    await this.loadHealth();
  }

  /**
   * Read API health and show it as a status line or an alert. Reads may overlap (the first
   * render and a refresh after sign-in); only the newest one updates the line, so a slow
   * answer from before sign-in cannot overwrite the signed-in status. Network I/O via the client.
   */
  private async loadHealth(): Promise<void> {
    const target = this.shadowRoot?.querySelector<HTMLElement>(".api");
    const labels = this.distribution?.labels;
    if (!target || !labels || !this.client) return;
    const request = ++this.healthRequest;
    try {
      const health = await this.client.health();
      if (request !== this.healthRequest) return;
      target.setAttribute("role", "status");
      delete target.dataset.reason;
      target.textContent = `${labels.apiStatus}: ${health.status} (${health.product})`;
    } catch (error) {
      if (request !== this.healthRequest) {
        console.debug("FileSquid discarded a superseded API health answer", error);
        return;
      }
      showHealthError(target, labels, error);
    }
  }
}

/**
 * Show a failed health read as an alert: the label of a denial reason, or the error message.
 *
 * @param target The status paragraph.
 * @param labels The distribution's label table.
 * @param error What the client rejected with.
 */
function showHealthError(target: HTMLElement, labels: LabelTable, error: unknown): void {
  const denial = denialLabel(error, labels);
  target.setAttribute("role", "alert");
  if (denial) target.dataset.reason = denial.reason;
  else delete target.dataset.reason;
  const message = denial?.text ?? (error instanceof Error ? error.message : String(error));
  target.textContent = `${labels.apiUnavailable}: ${message}`;
}

/** Create a list item with text content, wrapped in a link when `href` is given. */
function li(text: string, href?: string): HTMLLIElement {
  const item = document.createElement("li");
  if (!href) {
    item.textContent = text;
    return item;
  }
  const link = document.createElement("a");
  link.href = href;
  link.textContent = text;
  item.append(link);
  return item;
}
