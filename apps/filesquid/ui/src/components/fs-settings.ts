/**
 * `<fs-settings>`: the Settings page (UI-S-033, UI-S-015, UI-S-004, UI-S-018; mockup
 * `resources/ui-mockups/v3/32-settings.svg`). Appearance, Privacy and About, and Browsing between Appearance and Privacy when the page is
 * given the shared preferences, each section with a 22 px heading:
 *
 * - **Appearance**: the only `<fs-theme-control>` (owner answer 7, verbatim "Settings ->
 *   Appaerance") and its help text;
 * - **Privacy**: the full host privacy statement of ADR 032 Decision 6, shown in place by an
 *   inline `<fs-host-indicator>`, so it follows the host descriptor exactly as the header badge
 *   does;
 * - **About {product}**: the logo, the product and version, the build, licence and format
 *   notes, and "API: {status} · contract {version}", the only place API information appears
 *   (owner answer 12, verbatim "No, dont show API info on regular UI screens, just in the
 *   Settings"); a denied or failed health read is shown there as an alert with its reason.
 *
 * The Browsing section (UI-F2.8: default layout, sort, folder placement, start view and the
 * hover card, saved through the per-user preferences) is shown when the page is given the
 * shared preference service; the Recently deleted section (UI-F2.17) joins with its task. Words
 * come only from the label table, styling only from tokens.
 */
import { logoSource, type Distribution } from "../distribution";
import type { HealthStatus } from "../api/client";
import { formatLabel, type LabelTable } from "../theme/labels";
import type { ThemeStore } from "../theme/theme-store";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { denialLabel } from "./denial";
import type { FsHostIndicator, HostClient } from "./fs-host-indicator";
import type { FsThemeControl } from "./fs-theme-control";
import { icon } from "./icons";
import type { PreferencesService } from "../preferences/service";
import { STYLE as BROWSING_STYLE, browsingRows, syncBrowsing } from "./settings-browsing";

/** The part of the API client the page needs: the host descriptor and health. */
export interface SettingsClient extends HostClient {
  /** Read API health, for the About section. */
  health(): Promise<HealthStatus>;
}

/** What the page needs. */
export interface SettingsOptions {
  /** Label table for every word shown. */
  labels: LabelTable;
  /** Client that reads the host descriptor and health and delivers events. */
  client: SettingsClient;
  /** Shared theme state, already started, for the Appearance section. */
  theme: ThemeStore;
  /** Brand of the active distribution, for the About section. */
  distribution: Distribution;
  /** The API contract version this UI targets, shown beside the API status. */
  contractVersion: string;
  /** The shared per-user preferences; without them the Browsing section is not shown. */
  preferences?: PreferencesService;
}

const STYLE = `
  :host { display: block; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); color: var(--fs-on-surface); }
  .page { display: grid; gap: var(--fs-space-4); max-inline-size: 72rem; }
  section { display: grid; gap: var(--fs-space-3); padding-block-end: var(--fs-space-4); border-block-end: 1px solid var(--fs-outline); }
  section:last-child { border-block-end: 0; }
  .help { margin: 0; color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  .about { display: flex; flex-wrap: wrap; align-items: start; gap: var(--fs-space-3) var(--fs-space-4); }
  .about img { block-size: var(--fs-space-6); inline-size: auto; }
  .about-text { flex: 1 1 20rem; display: grid; gap: var(--fs-space-1); min-width: 0; }
  .version { margin: 0; font-size: var(--fs-font-size-body); font-weight: 600; color: var(--fs-on-surface-heading); }
  .about-note { margin: 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); overflow-wrap: anywhere; }
  .api { margin: 0; flex: 0 0 auto; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); overflow-wrap: anywhere; }
  .api[role="alert"] { display: flex; align-items: center; gap: var(--fs-space-1); color: var(--fs-on-surface); }
  .api[role="alert"] .icon { color: var(--fs-error); }
  fs-host-indicator { display: block; }
`;

/** Custom element that shows the Settings page. */
export class FsSettings extends HTMLElement {
  private options?: SettingsOptions;
  /** Number of the newest health read; answers to older reads are discarded. */
  private healthRequest = 0;
  /** The theme store whose `change` event this page follows for the About logo, if any. */
  private followed?: ThemeStore;
  private readonly onThemeChange = () => this.showLogo();
  /** The browsing section's elements, while it is shown. */
  private browsing?: HTMLElement[];
  /** The preferences whose `change` event the section follows, so that one is the one let go. */
  private followedPreferences?: PreferencesService;
  private readonly onPreferences = () => this.syncPreferences();

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the labels, client, theme, distribution and contract version, then render. Call before
   * or after connecting.
   *
   * @param options See {@link SettingsOptions}.
   */
  configure(options: SettingsOptions): void {
    this.options = options;
    if (this.isConnected) this.render();
  }

  /**
   * Read health and the host descriptor again with the token known now, for example after a
   * sign-in. Network I/O via the client.
   *
   * @returns Resolves once the About and Privacy sections show the new answers.
   */
  async refresh(): Promise<void> {
    const statement = this.shadowRoot?.querySelector<FsHostIndicator>("fs-host-indicator");
    await Promise.all([this.loadHealth(), statement?.refresh()]);
  }

  /** Render, read health and follow the theme for the About logo when attached. */
  connectedCallback(): void {
    this.render();
  }

  /** Stop following the theme once detached. */
  disconnectedCallback(): void {
    this.followed?.removeEventListener("change", this.onThemeChange);
    this.followed = undefined;
    this.followedPreferences?.removeEventListener("change", this.onPreferences);
    this.followedPreferences = undefined;
  }

  /** Show the saved preferences in the Browsing section's controls. */
  private syncPreferences(): void {
    const options = this.options;
    if (this.browsing && options?.preferences) {
      syncBrowsing(this.browsing, options.labels, options.preferences);
    }
  }

  /** Point the About logo at the variant for the theme shown now (UI-S-015). */
  private showLogo(): void {
    const img = this.shadowRoot?.querySelector<HTMLImageElement>(".about img");
    const options = this.options;
    if (!img || !options) return;
    img.src = logoSource(options.distribution.logo, options.theme.resolved);
  }

  /** Follow the configured theme store's changes, and only that one. */
  private follow(theme: ThemeStore): void {
    if (this.followed === theme) return;
    this.followed?.removeEventListener("change", this.onThemeChange);
    theme.addEventListener("change", this.onThemeChange);
    this.followed = theme;
  }

  /** Build the title and the three sections, then read health for About. */
  private render(): void {
    const root = this.shadowRoot;
    const options = this.options;
    if (!root || !options) return;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + CONTROL_STYLE + STYLE + BROWSING_STYLE;
    const page = document.createElement("div");
    page.className = "page";
    const heading = document.createElement("h2");
    heading.className = "page-title";
    heading.textContent = options.labels.settings;
    const browsing = this.browsingSection(options);
    page.append(
      heading,
      appearanceSection(options),
      ...(browsing ? [browsing] : []),
      privacySection(options),
      aboutSection(options),
    );
    root.replaceChildren(style, page);
    this.follow(options.theme);
    this.showLogo();
    void this.loadHealth();
  }

  /** The Browsing section, or undefined when the page has no preferences to show. */
  private browsingSection(options: SettingsOptions): HTMLElement | undefined {
    this.followedPreferences?.removeEventListener("change", this.onPreferences);
    this.followedPreferences = undefined;
    this.browsing = undefined;
    if (!options.preferences) return undefined;
    const { section: element } = section("browsing", options.labels.settingsBrowsing);
    this.browsing = browsingRows(options.labels, options.preferences);
    element.append(...this.browsing);
    options.preferences.addEventListener("change", this.onPreferences);
    this.followedPreferences = options.preferences;
    return element;
  }

  /**
   * Read API health and show it in About as the status line or an alert. Only the newest read
   * updates the line, so a slow answer from before a sign-in cannot overwrite the signed-in
   * status. Network I/O via the client.
   */
  private async loadHealth(): Promise<void> {
    const target = this.shadowRoot?.querySelector<HTMLElement>(".api");
    const options = this.options;
    if (!target || !options) return;
    const request = ++this.healthRequest;
    try {
      const health = await options.client.health();
      if (request !== this.healthRequest) return;
      showHealth(target, options, health);
    } catch (error) {
      if (request !== this.healthRequest) {
        console.debug("FileSquid discarded a superseded API health answer", error);
        return;
      }
      showHealthError(target, options.labels, error);
    }
  }
}

/** A section with a heading the section is named by. */
function section(name: string, title: string): { section: HTMLElement; body: HTMLElement[] } {
  const element = document.createElement("section");
  element.dataset.section = name;
  element.setAttribute("aria-labelledby", `${name}-title`);
  const heading = document.createElement("h3");
  heading.id = `${name}-title`;
  heading.className = "section-title";
  heading.textContent = title;
  element.append(heading);
  return { section: element, body: [] };
}

/** The Appearance section: the theme control and its help text. */
function appearanceSection(options: SettingsOptions): HTMLElement {
  const { section: element } = section("appearance", options.labels.settingsAppearance);
  const control = document.createElement("fs-theme-control") as FsThemeControl;
  control.configure({ store: options.theme, labels: options.labels });
  const help = document.createElement("p");
  help.className = "help";
  help.textContent = options.labels.themeHelp;
  element.append(control, help);
  return element;
}

/** The Privacy section: the inline host statement in a raised box. */
function privacySection(options: SettingsOptions): HTMLElement {
  const { section: element } = section("privacy", options.labels.settingsPrivacy);
  const box = document.createElement("div");
  box.className = "note-box raised";
  const statement = document.createElement("fs-host-indicator") as FsHostIndicator;
  statement.configure({ labels: options.labels, client: options.client, inline: true });
  box.append(statement);
  element.append(box);
  return element;
}

/** The About section: logo, product and version, notes and the API status line. */
function aboutSection(options: SettingsOptions): HTMLElement {
  const { labels, distribution } = options;
  const title = formatLabel(labels.settingsAbout, { product: distribution.productName });
  const { section: element } = section("about", title);
  const box = document.createElement("div");
  box.className = "note-box raised about";
  const img = document.createElement("img");
  img.alt = distribution.logo.alt;
  const text = document.createElement("div");
  text.className = "about-text";
  const version = document.createElement("p");
  version.className = "version";
  version.textContent = formatLabel(labels.settingsVersion, {
    product: distribution.productName,
    version: distribution.version ?? "",
  }).trim();
  text.append(
    version,
    ...[labels.settingsBuildNote, labels.settingsLicence, labels.settingsFormats].map(note),
  );
  const api = document.createElement("p");
  api.className = "api";
  api.setAttribute("role", "status");
  box.append(img, text, api);
  element.append(box);
  return element;
}

/** One note line of the About box. */
function note(text: string): HTMLParagraphElement {
  const p = document.createElement("p");
  p.className = "about-note";
  p.textContent = text;
  return p;
}

/** Show a health answer as the status line. */
function showHealth(target: HTMLElement, options: SettingsOptions, health: HealthStatus): void {
  target.setAttribute("role", "status");
  delete target.dataset.reason;
  target.textContent = formatLabel(options.labels.settingsApiStatus, {
    status: health.status,
    version: options.contractVersion,
  });
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
  // The words in the body colour on the raised box (error text there would be below 4.5:1 in
  // dark), marked by the alert icon in the error colour.
  target.replaceChildren(
    icon("alert"),
    document.createTextNode(`${labels.apiUnavailable}: ${message}`),
  );
}
