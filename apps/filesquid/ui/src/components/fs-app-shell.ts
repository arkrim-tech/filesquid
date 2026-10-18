/**
 * `<fs-app-shell>`: application frame shared by every distribution. Shows the
 * distribution's brand (logo and product name, with a dark-surface logo in the dark theme),
 * the theme control, its vocabulary for the main destinations, and the API status.
 * An authorisation denial is shown with the label for its reason (API-S-004).
 */
import type { Distribution } from "../distribution";
import { logoSource } from "../distribution";
import type { LabelTable } from "../theme/labels";
import type { ThemeStore } from "../theme/theme-store";
import type { HealthStatus } from "../api/client";
import { MONITOR_ROUTE } from "../app/routes";
import { denialLabel } from "./denial";
import type { FsThemeControl } from "./fs-theme-control";

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
  :host { display: block; min-height: 100vh; background: var(--fs-surface); color: var(--fs-on-surface); font-family: var(--fs-font-sans); }
  header { display: flex; align-items: center; gap: 12px; padding: 12px 16px; border-bottom: 1px solid var(--fs-outline); }
  h1 { margin: 0; font-family: var(--fs-font-heading); font-size: 1.25rem; font-weight: 700; color: var(--fs-on-surface); }
  img { height: 32px; width: auto; }
  fs-theme-control { margin-left: auto; }
  .body { display: flex; }
  nav { width: 240px; padding: 16px; background: var(--fs-surface-container); min-height: calc(100vh - 57px); box-sizing: border-box; }
  ul { list-style: none; margin: 0; padding: 0; }
  li { padding: 8px 12px; border-radius: var(--fs-radius); }
  a { color: inherit; text-decoration: none; }
  main { flex: 1; padding: 16px; }
  [role="status"] { font-family: var(--fs-font-mono); color: var(--fs-on-surface-variant); }
  [role="alert"] { color: var(--fs-error); }
`;

/** Custom element that renders the application frame. */
export class FsAppShell extends HTMLElement {
  private distribution?: Distribution;
  private client?: ShellClient;
  private theme?: ThemeStore;
  /** Number of the newest health read; answers to older reads are discarded. */
  private healthRequest = 0;
  private readonly onThemeChange = () => this.showLogo();

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

  /** Follow the theme and render when attached to the document. */
  connectedCallback(): void {
    this.connect();
  }

  /** Stop following the theme once detached. */
  disconnectedCallback(): void {
    this.theme?.removeEventListener("change", this.onThemeChange);
  }

  /** Subscribe to the theme store and render. */
  private connect(): void {
    this.theme?.addEventListener("change", this.onThemeChange);
    this.render();
  }

  /** Build the frame, then load API status. */
  private render(): void {
    const root = this.shadowRoot;
    const dist = this.distribution;
    if (!root || !dist) return;
    const { labels } = dist;
    root.innerHTML = `
      <style>${STYLE}</style>
      <header>
        <img alt="">
        <h1></h1>
      </header>
      <div class="body">
        <nav aria-label="Main"><ul></ul></nav>
        <main><p class="api"></p><slot></slot></main>
      </div>`;
    root.querySelector("img")?.setAttribute("alt", dist.logo.alt);
    this.showLogo();
    const h1 = root.querySelector("h1");
    if (h1) h1.textContent = dist.productName;
    this.mountThemeControl(root, labels);
    const items = [
      labels.libraries,
      labels.collections,
      labels.virtualCollections,
      labels.virtualExhibits,
    ];
    root
      .querySelector("ul")
      ?.replaceChildren(...items.map((text) => li(text)), li(labels.monitor, MONITOR_ROUTE));
    void this.loadHealth();
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
