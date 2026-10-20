/**
 * `<fs-settings>`: the Settings page (UI-S-004, UI-S-018; mockup
 * `resources/ui-mockups/v2/32-settings.svg`). This task delivers its Privacy section: the full
 * host privacy statement of ADR 032 Decision 6, shown in place by an inline
 * `<fs-host-indicator>`, so it follows the host descriptor exactly as the header badge does.
 * The appearance, browsing-preference and About sections of the mockup join as further
 * sections in UI-F2.8 and DE-F3.1. Words come only from the label table, styling only from
 * tokens.
 */
import type { LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import type { FsHostIndicator, HostClient } from "./fs-host-indicator";

/** What the page needs: the active label table and the client for the host descriptor. */
export interface SettingsOptions {
  /** Label table for every word shown. */
  labels: LabelTable;
  /** Client that reads the host descriptor and delivers events. */
  client: HostClient;
}

const STYLE = `
  :host { display: block; font-family: var(--fs-font-sans); color: var(--fs-on-surface); }
  .page { display: grid; gap: var(--fs-space-4); max-inline-size: 48rem; }
  h2 { margin: 0; font-family: var(--fs-font-heading); font-size: var(--fs-font-size-title); font-weight: 700; color: var(--fs-on-surface); }
  section { display: grid; gap: var(--fs-space-2); padding: var(--fs-space-3) var(--fs-space-4); border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface-container); }
  h3 { margin: 0; font-size: var(--fs-font-size-body); font-weight: 700; color: var(--fs-on-surface); }
  fs-host-indicator { display: block; }
`;

/** Custom element that shows the Settings page. */
export class FsSettings extends HTMLElement {
  private options?: SettingsOptions;

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the label table and client, then render. Call before or after connecting.
   *
   * @param options The active label table and the client.
   */
  configure(options: SettingsOptions): void {
    this.options = options;
    if (this.isConnected) this.render();
  }

  /** Render when attached. */
  connectedCallback(): void {
    this.render();
  }

  /** Build the page heading and the Privacy section with the inline host statement. */
  private render(): void {
    const root = this.shadowRoot;
    const options = this.options;
    if (!root || !options) return;
    const { labels } = options;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + STYLE;
    const page = document.createElement("div");
    page.className = "page";
    const heading = document.createElement("h2");
    heading.textContent = labels.settings;
    page.append(heading, privacySection(options));
    root.replaceChildren(style, page);
  }
}

/** The Privacy section: a named section holding the inline host statement. */
function privacySection(options: SettingsOptions): HTMLElement {
  const section = document.createElement("section");
  section.dataset.section = "privacy";
  section.setAttribute("aria-labelledby", "privacy-title");
  const title = document.createElement("h3");
  title.id = "privacy-title";
  title.textContent = options.labels.settingsPrivacy;
  const statement = document.createElement("fs-host-indicator") as FsHostIndicator;
  statement.configure({ labels: options.labels, client: options.client, inline: true });
  section.append(title, statement);
  return section;
}
