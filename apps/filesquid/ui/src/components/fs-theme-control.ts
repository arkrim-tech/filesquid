/**
 * `<fs-theme-control>`: the theme control every distribution's shell shows (SRS-UI-007).
 * A labelled group of three native radio buttons (System, Light, Dark), so it works with the
 * keyboard (arrow keys) and screen readers without custom key handling. It reads and changes
 * the shared {@link ThemeStore}; it keeps no theme state of its own.
 */
import type { LabelTable } from "../theme/labels";
import {
  THEME_CHOICES,
  parseThemeChoice,
  type ThemeChoice,
  type ThemeStore,
} from "../theme/theme-store";

/** What the control needs: the shared theme store and the active label table. */
export interface ThemeControlOptions {
  /** Shared theme state. */
  store: ThemeStore;
  /** Label table for the legend and the three options. */
  labels: LabelTable;
}

const STYLE = `
  :host { display: inline-block; font-family: var(--fs-font-sans); color: var(--fs-on-surface); }
  fieldset { position: relative; display: flex; gap: 2px; margin: 0; padding: 2px; border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface-container); }
  legend { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  label { position: relative; display: inline-flex; align-items: center; padding: 4px 10px; border-radius: calc(var(--fs-radius) - 2px); font-size: 0.8125rem; font-weight: 600; color: var(--fs-on-surface-variant); background: var(--fs-surface-container); cursor: pointer; }
  label:has(input:checked) { color: var(--fs-on-primary-container); background: var(--fs-primary-container); }
  label:has(input:focus-visible) { outline: 2px solid var(--fs-primary); outline-offset: 1px; }
  input { position: absolute; opacity: 0; width: 1px; height: 1px; margin: 0; }
`;

/** Label-table key for each choice. */
const OPTION_LABEL: Record<ThemeChoice, "themeSystem" | "themeLight" | "themeDark"> = {
  system: "themeSystem",
  light: "themeLight",
  dark: "themeDark",
};

/** Custom element that switches between the system, light and dark themes. */
export class FsThemeControl extends HTMLElement {
  private options?: ThemeControlOptions;
  private readonly onStoreChange = () => this.reflect();
  private readonly onInput = (event: Event) => this.choose(event);

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the theme store and labels, then render. Call before or after connecting.
   *
   * @param options Shared theme store and active label table.
   */
  configure(options: ThemeControlOptions): void {
    this.options?.store.removeEventListener("change", this.onStoreChange);
    this.options = options;
    if (this.isConnected) this.connect();
  }

  /** Render and follow the store while attached. */
  connectedCallback(): void {
    this.connect();
  }

  /** Stop following the store once detached. */
  disconnectedCallback(): void {
    this.options?.store.removeEventListener("change", this.onStoreChange);
  }

  /** Subscribe to the store and render. */
  private connect(): void {
    if (!this.options) return;
    this.options.store.addEventListener("change", this.onStoreChange);
    this.render();
  }

  /** Build the group of radios from the label table. */
  private render(): void {
    const root = this.shadowRoot;
    if (!root || !this.options) return;
    const { labels } = this.options;
    const fieldset = document.createElement("fieldset");
    const legend = document.createElement("legend");
    legend.textContent = labels.theme;
    fieldset.append(legend, ...THEME_CHOICES.map((choice) => option(choice, labels)));
    fieldset.addEventListener("change", this.onInput);
    const style = document.createElement("style");
    style.textContent = STYLE;
    root.replaceChildren(style, fieldset);
    this.reflect();
  }

  /** Check the radio of the store's choice and expose the resolved theme as `data-resolved`. */
  private reflect(): void {
    const store = this.options?.store;
    if (!store) return;
    for (const input of this.shadowRoot?.querySelectorAll("input") ?? []) {
      input.checked = input.value === store.choice;
    }
    this.dataset.resolved = store.resolved;
  }

  /** Pass a chosen radio to the store, which applies and saves it. */
  private choose(event: Event): void {
    const input = event.target;
    if (!(input instanceof HTMLInputElement) || !input.checked) return;
    this.options?.store.setChoice(parseThemeChoice(input.value));
    this.reflect();
  }
}

/** Build one `<label><input type="radio"> text</label>` option. */
function option(choice: ThemeChoice, labels: LabelTable): HTMLLabelElement {
  const label = document.createElement("label");
  const input = document.createElement("input");
  input.type = "radio";
  input.name = "theme";
  input.value = choice;
  const text = document.createElement("span");
  text.textContent = labels[OPTION_LABEL[choice]];
  label.append(input, text);
  return label;
}
