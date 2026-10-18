/**
 * `<fs-app-shell>`: application frame shared by every distribution. Shows the
 * distribution's brand, its vocabulary for the main destinations, and the API status.
 * An authorisation denial is shown with the label for its reason (API-S-004).
 */
import type { Distribution } from "../distribution";
import type { LabelTable } from "../theme/labels";
import type { HealthStatus } from "../api/client";
import { MONITOR_ROUTE } from "../app/routes";
import { denialLabel } from "./denial";

/** The part of the API client the shell needs (injectable in tests). */
export interface ShellClient {
  /** Read API health. */
  health(): Promise<HealthStatus>;
}

const STYLE = `
  :host { display: block; min-height: 100vh; background: var(--fs-surface); color: var(--fs-on-surface); font-family: var(--fs-font-sans); }
  header { display: flex; align-items: center; gap: 12px; padding: 12px 16px; border-bottom: 1px solid var(--fs-outline); }
  h1 { margin: 0; font-size: 1.25rem; font-weight: 600; }
  img { width: 32px; height: 32px; }
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

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the distribution and API client, then render. Call before or after connecting.
   *
   * @param options Distribution brand and vocabulary, and the API client.
   */
  configure(options: { distribution: Distribution; client: ShellClient }): void {
    this.distribution = options.distribution;
    this.client = options.client;
    if (this.isConnected) this.render();
  }

  /** Render when attached to the document. */
  connectedCallback(): void {
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
    const img = root.querySelector("img");
    img?.setAttribute("src", dist.logo.src);
    img?.setAttribute("alt", dist.logo.alt);
    const h1 = root.querySelector("h1");
    if (h1) h1.textContent = dist.productName;
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

  /**
   * Read API health again and update the status line, for example after a token was entered.
   * Network I/O via the client.
   *
   * @returns Resolves once the status line shows the new answer.
   */
  async refreshHealth(): Promise<void> {
    await this.loadHealth();
  }

  /** Read API health and show it as a status line or an alert. Network I/O via the client. */
  private async loadHealth(): Promise<void> {
    const target = this.shadowRoot?.querySelector<HTMLElement>(".api");
    const labels = this.distribution?.labels;
    if (!target || !labels || !this.client) return;
    try {
      const health = await this.client.health();
      target.setAttribute("role", "status");
      delete target.dataset.reason;
      target.textContent = `${labels.apiStatus}: ${health.status} (${health.product})`;
    } catch (error) {
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
