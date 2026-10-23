/**
 * `<fs-theme-control>`: the theme control of Settings, Appearance (SRS-UI-007, UI-S-015,
 * UI-S-033; owner answer 7, verbatim "Settings -> Appaerance"; mockup
 * `resources/ui-mockups/v3/32-settings.svg`). A labelled group of three native radio buttons
 * (System, Light, Dark) drawn as a segmented control, the chosen one at weight 600 on the
 * active tint, so it works with the keyboard (arrow keys) and screen readers without custom key
 * handling. It reads and changes the shared {@link ThemeStore}; it keeps no theme state of its
 * own.
 *
 * Accessibility (UI-S-016): the radio group is one Tab stop; the focus ring is drawn on the
 * label of the focused radio; colour changes take `--fs-motion-duration` (none under reduced
 * motion); under forced colours the chosen option uses the system highlight colours.
 */
import type { LabelTable } from "../theme/labels";
import {
  THEME_CHOICES,
  parseThemeChoice,
  type ThemeChoice,
  type ThemeStore,
} from "../theme/theme-store";
import { A11Y_STYLE } from "./a11y-style";
import { replaceKeepingFocus } from "../a11y/focus";

/** What the control needs: the shared theme store and the active label table. */
export interface ThemeControlOptions {
  /** Shared theme state. */
  store: ThemeStore;
  /** Label table for the legend and the three options. */
  labels: LabelTable;
}

const STYLE = `
  :host { display: inline-block; max-width: 100%; font-family: var(--fs-font-sans); color: var(--fs-on-surface); }
  fieldset { position: relative; display: flex; flex-wrap: wrap; gap: var(--fs-space-2); margin: 0; padding: 0; border: 0; }
  legend { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  label { position: relative; display: inline-flex; align-items: center; justify-content: center; box-sizing: border-box; min-inline-size: 6.375rem; min-block-size: 2.125rem; padding: 0 var(--fs-space-3); border: 1.5px solid var(--fs-outline-control); border-radius: var(--fs-radius-button); font-size: var(--fs-font-size-small); font-weight: 500; color: var(--fs-on-surface); background: var(--fs-surface-card); cursor: pointer; transition: color var(--fs-motion-duration), background-color var(--fs-motion-duration), border-color var(--fs-motion-duration); }
  label:hover { background: var(--fs-surface-raised); }
  label:has(input:checked) { border-color: var(--fs-primary); background: var(--fs-primary-container); color: var(--fs-on-primary-container); font-size: var(--fs-font-size-ui); font-weight: 600; }
  label:has(input:focus-visible) { outline: 2px solid var(--fs-focus-ring); outline-offset: 2px; }
  input { position: absolute; opacity: 0; width: 1px; height: 1px; margin: 0; }
  input:focus-visible { outline: none; }
  @media (forced-colors: active) {
    label:has(input:checked) { forced-color-adjust: none; color: HighlightText; background: Highlight; }
    label:has(input:focus-visible) { outline-color: Highlight; }
  }
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

  /**
   * Build the group of radios from the label table. When this replaced a focused radio, focus
   * moves to the chosen one.
   */
  private render(): void {
    const root = this.shadowRoot;
    if (!root || !this.options) return;
    const { labels, store } = this.options;
    const fieldset = document.createElement("fieldset");
    const legend = document.createElement("legend");
    legend.textContent = labels.theme;
    const radios = THEME_CHOICES.map((choice) => option(choice, labels));
    fieldset.append(legend, ...radios);
    fieldset.addEventListener("change", this.onInput);
    const chosen = fieldset.querySelector<HTMLInputElement>(`input[value="${store.choice}"]`);
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + STYLE;
    replaceKeepingFocus(root, [style, fieldset], () => chosen);
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
