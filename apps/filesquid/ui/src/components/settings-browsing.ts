/**
 * The Browsing section of the Settings page (UI-F2.8, UI-S-033, mockup 32): label and value rows
 * for the default layout, the sort, the folder placement, the start view and the hover card,
 * with "These preferences are stored in your library and come back on every launch." A choice is
 * applied and saved at once through the {@link PreferencesService}; the section says so when the
 * host stores no preferences (they apply until the page closes) or a save failed. Words come
 * from the label table, styling only from tokens.
 */
import {
  SORT_KEYS,
  START_VIEWS,
  LAYOUTS,
  FOLDER_PLACEMENTS,
  type SortKey,
} from "../preferences/model";
import { sortOptionLabel } from "../drive/model";
import type { PreferencesService } from "../preferences/service";
import type { LabelTable } from "../theme/labels";

/** The stylesheet of the section's rows, tokens only; added after the page's own. */
export const STYLE = `
  .pref-rows { display: grid; gap: var(--fs-space-2); max-inline-size: 40rem; }
  .pref-row { display: grid; grid-template-columns: 12rem minmax(0, 1fr); align-items: center; gap: var(--fs-space-3); }
  .pref-row label { font-size: var(--fs-font-size-ui); color: var(--fs-on-surface); }
  .pref-row select.input { inline-size: 100%; }
  .pref-row input[type="checkbox"] { appearance: auto; inline-size: 0.90625rem; block-size: 0.90625rem; margin: 0; accent-color: var(--fs-primary); }
  .pref-note { margin: 0; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .pref-note:empty { display: none; }
`;

/** A `<select>` of `options` (value, text) with the id its label points at. */
function select(id: string, options: readonly (readonly [string, string])[]): HTMLSelectElement {
  const element = document.createElement("select");
  element.id = id;
  element.className = "input";
  for (const [value, text] of options) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = text;
    element.append(option);
  }
  return element;
}

/** A row: the label and its control. */
function row(id: string, text: string, control: HTMLElement): HTMLElement {
  const out = document.createElement("div");
  out.className = "pref-row";
  const label = document.createElement("label");
  label.htmlFor = id;
  label.textContent = text;
  out.append(label, control);
  return out;
}

/** The sort options: each key in each direction, worded by the label table. */
function sortOptions(labels: LabelTable): (readonly [string, string])[] {
  return SORT_KEYS.flatMap((key) =>
    (["ascending", "descending"] as const).map(
      (direction) => [`${key}:${direction}`, labels[sortOptionLabel(key, direction)]] as const,
    ),
  );
}

/**
 * Build the Browsing section's body: the five rows, the help line and the status note.
 *
 * @param labels The active label table.
 * @param service The shared preferences; each control saves through it.
 * @returns The elements to put in the section.
 */
export function browsingRows(labels: LabelTable, service: PreferencesService): HTMLElement[] {
  const layout = select(
    "pref-layout",
    LAYOUTS.map(
      (value) => [value, value === "list" ? labels.fileViewList : labels.fileViewGrid] as const,
    ),
  );
  const sort = select("pref-sort", sortOptions(labels));
  const folders = select(
    "pref-folders",
    FOLDER_PLACEMENTS.map(
      (value) =>
        [
          value,
          value === "on_top" ? labels.foldersPlacementTop : labels.foldersPlacementMixed,
        ] as const,
    ),
  );
  const start = select(
    "pref-start",
    START_VIEWS.map(
      (value) =>
        [value, value === "home" ? labels.startViewHome : labels.startViewPersonalRoot] as const,
    ),
  );
  const hover = document.createElement("input");
  hover.type = "checkbox";
  hover.id = "pref-hover";
  layout.addEventListener("change", () =>
    service.update({ layout: layout.value as "list" | "grid" }),
  );
  sort.addEventListener("change", () => {
    const [key, direction] = sort.value.split(":") as [SortKey, "ascending" | "descending"];
    service.update({ sort_key: key, sort_direction: direction });
  });
  folders.addEventListener("change", () =>
    service.update({ folder_placement: folders.value as "on_top" | "mixed" }),
  );
  start.addEventListener("change", () =>
    service.update({ start_view: start.value as "home" | "personal_root" }),
  );
  hover.addEventListener("change", () => service.update({ hover_card: hover.checked }));
  const rows = document.createElement("div");
  rows.className = "pref-rows";
  rows.append(
    row("pref-layout", labels.settingsDefaultLayout, layout),
    row("pref-sort", labels.settingsSortBy, sort),
    row("pref-folders", labels.settingsFolders, folders),
    row("pref-start", labels.settingsStartView, start),
    row("pref-hover", labels.settingsHoverCard, hover),
  );
  const help = document.createElement("p");
  help.className = "pref-note help";
  help.textContent = labels.settingsBrowsingHelp;
  const status = document.createElement("p");
  status.className = "pref-note pref-status";
  status.setAttribute("role", "status");
  const out = [rows, help, status];
  syncBrowsing(out, labels, service);
  return out;
}

/**
 * Show the current preferences in the section's controls and its status note.
 *
 * @param parts The elements {@link browsingRows} returned.
 * @param labels The active label table.
 * @param service The shared preferences.
 */
export function syncBrowsing(
  parts: readonly HTMLElement[],
  labels: LabelTable,
  service: PreferencesService,
): void {
  const prefs = service.preferences;
  const set = (id: string, value: string) => {
    const control = parts[0]?.querySelector<HTMLSelectElement>(`#${id}`);
    if (control) control.value = value;
  };
  set("pref-layout", prefs.layout);
  set("pref-sort", `${prefs.sort_key}:${prefs.sort_direction}`);
  set("pref-folders", prefs.folder_placement);
  set("pref-start", prefs.start_view);
  const hover = parts[0]?.querySelector<HTMLInputElement>("#pref-hover");
  if (hover) hover.checked = prefs.hover_card;
  const status = parts[2];
  if (!status) return;
  const failed = service.saveFailed;
  const deviceOnly = service.mode === "device";
  status.textContent = failed
    ? labels.settingsBrowsingSaveFailed
    : deviceOnly
      ? labels.settingsBrowsingDeviceOnly
      : "";
}
