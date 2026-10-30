/**
 * The drop-down of the global search bar (UI-F2.9, UI-S-030, mockup 20) as data and DOM: the
 * four sections FILTERS, SUGGESTIONS, FILES and OPERATORS and the Enter hint, with the stylesheet
 * {@link STYLE}. {@link DropdownModel} says what to show; {@link renderDropdown} builds the
 * elements. Every option is one `role="option"` element with a global `data-index` (filters, then
 * suggestions, files and operators), so the bar's combobox keys and clicks address them by index.
 * The drop-down is 620 x 520 px, 6 px under the field. Nothing here fetches or decides what an
 * option does.
 */
import { badgeClass, badgeExtensionOf, relativeTime, type DriveRow } from "../drive/model";
import { formatLabel, type LabelTable } from "../theme/labels";
import { BADGE_STYLE } from "./drive-row-view";
import { icon, type IconName } from "./icons";
import type { ChipFilter } from "../search/model";

/** One of the FILTERS chips: opens that filter's choices. */
export interface FilterOption {
  /** Discriminates the options. */
  readonly kind: "filter";
  /** Which filter. */
  readonly filter: "type" | "in" | "modified";
  /** Its name from the label table. */
  readonly text: string;
  /** Whether its choices are shown now. */
  readonly open: boolean;
}

/** One SUGGESTIONS row: an operator, its value and what it does. */
export interface SuggestionOption {
  /** Discriminates the options. */
  readonly kind: "suggestion";
  /** The chip it applies. */
  readonly filter: ChipFilter;
  /** The chip's value (class, title, range or text). */
  readonly chip: string;
  /** The operator as typed, `in:`. */
  readonly op: string;
  /** The value shown after it. */
  readonly value: string;
  /** What it does, at the row's right. */
  readonly hint: string;
}

/** One FILES row: a file the live search found. */
export interface FileOption {
  /** Discriminates the options. */
  readonly kind: "file";
  /** The file. */
  readonly row: DriveRow;
}

/** One OPERATORS chip: text that goes into the field. */
export interface OperatorOption {
  /** Discriminates the options. */
  readonly kind: "operator";
  /** The operator as written. */
  readonly text: string;
}

/** Any option. */
export type DropdownOption = FilterOption | SuggestionOption | FileOption | OperatorOption;

/** What the drop-down shows. */
export interface DropdownModel {
  /** The FILTERS chips. */
  readonly filters: readonly FilterOption[];
  /** The SUGGESTIONS rows; the section is left out when there are none. */
  readonly suggestions: readonly SuggestionOption[];
  /** The FILES rows; the section is left out when there are none. */
  readonly files: readonly FileOption[];
  /** The OPERATORS chips. */
  readonly operators: readonly OperatorOption[];
}

/** The options of a model in index order. */
export function optionsOf(model: DropdownModel): DropdownOption[] {
  return [...model.filters, ...model.suggestions, ...model.files, ...model.operators];
}

/** What the renderer needs besides the model. */
export interface DropdownContext {
  /** The active label table. */
  readonly labels: LabelTable;
  /** BCP 47 tag for dates. */
  readonly locale: string;
  /** The time now, for relative dates. */
  readonly now: () => Date;
  /** The index of the active option, or -1. */
  readonly active: number;
  /** The id prefix of the options, so `aria-activedescendant` can name one. */
  readonly idPrefix: string;
}

/** The stylesheet of the drop-down, tokens only (UI-S-024). */
export const STYLE = `
  .dropdown { position: absolute; z-index: 5; inset-block-start: calc(100% + 6px); inset-inline-start: 0; box-sizing: border-box; inline-size: 38.75rem; max-inline-size: 100vw; block-size: 32.5rem; max-block-size: calc(100vh - 6rem); overflow: auto; border: 1px solid var(--fs-outline-strong); border-radius: var(--fs-radius); background: var(--fs-surface-overlay); color: var(--fs-on-surface); }
  .dropdown[hidden] { display: none; }
  .section { padding: var(--fs-space-3) 0 0; }
  .section .eyebrow { box-sizing: border-box; block-size: 1.3125rem; padding: 0 var(--fs-space-3); }
  .filters { padding-block-end: var(--fs-space-1); }
  .suggestions { padding-block-end: var(--fs-space-1); }
  .files, .operators { border-block-start: 1px solid var(--fs-outline); }
  .filter-row { display: flex; flex-wrap: wrap; gap: var(--fs-space-2); margin: 0 var(--fs-space-3); }
  .filter-chip { display: inline-flex; align-items: center; gap: var(--fs-space-1); box-sizing: border-box; block-size: 1.75rem; padding: 0 var(--fs-space-2); border: 1px solid var(--fs-outline-strong); border-radius: var(--fs-radius-chip); background: var(--fs-surface-card); color: var(--fs-on-surface); font-family: inherit; font-size: var(--fs-font-size-small); font-weight: 500; cursor: pointer; }
  .filter-chip[aria-selected="true"], .filter-chip[data-open] { background: var(--fs-surface-raised); }
  .rows { margin: 0 var(--fs-space-1); }
  .suggestion, .file-option { display: flex; align-items: center; gap: var(--fs-space-2); box-sizing: border-box; padding: 0 var(--fs-space-2); border-radius: var(--fs-radius-item); cursor: pointer; }
  .suggestion { block-size: 2.125rem; }
  .suggestion > .icon { margin-inline-start: var(--fs-space-1); }
  .file-option > .badge { align-self: flex-start; margin: var(--fs-space-2) var(--fs-space-1) 0; }
  .suggestion[aria-selected="true"], .suggestion:hover, .file-option[aria-selected="true"], .file-option:hover { background: var(--fs-surface-raised); }
  .suggestion .op { font-family: var(--fs-font-mono); font-size: var(--fs-font-size-small); font-weight: 400; color: var(--fs-primary); }
  .suggestion .words { display: flex; flex: 1 1 auto; gap: var(--fs-space-1); min-width: 0; }
  .suggestion .value { font-size: var(--fs-font-size-small); font-weight: 500; color: var(--fs-on-surface); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .suggestion .hint { margin-inline: auto var(--fs-space-1); font-size: var(--fs-font-size-small); font-weight: 400; color: var(--fs-on-surface-muted); white-space: nowrap; }
  .files .rows { margin-block-start: var(--fs-space-2); }
  .file-option { block-size: 2.75rem; }
  .file-option .file-text { display: grid; min-width: 0; }
  .file-option .file-name { font-size: var(--fs-font-size-small); font-weight: 500; color: var(--fs-on-surface); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .file-option .file-meta { font-size: var(--fs-font-size-small); font-weight: 400; color: var(--fs-on-surface-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .operator-row { display: flex; flex-wrap: wrap; gap: var(--fs-space-1); margin: 0 var(--fs-space-3); }
  .operator-chip { box-sizing: border-box; block-size: 1.375rem; padding: 0 var(--fs-space-2); border: 1px solid var(--fs-surface-raised); border-radius: var(--fs-radius-chip); background: var(--fs-surface-raised); color: var(--fs-on-surface-variant); font-family: var(--fs-font-mono); font-size: var(--fs-font-size-eyebrow); font-weight: 500; cursor: pointer; }
  .operator-chip[aria-selected="true"], .operator-chip:hover { border-color: var(--fs-outline-strong); }
  .enter-hint { margin: var(--fs-space-3) var(--fs-space-3) 0; font-size: var(--fs-font-size-small); font-weight: 400; color: var(--fs-on-surface-muted); }
  ${BADGE_STYLE}
`;

/** The icon of a suggestion row's filter. */
const ROW_ICON: Readonly<Record<ChipFilter, IconName>> = {
  type: "files",
  in: "collection",
  modified: "calendar",
  title: "search",
};

/** The icon of a FILTERS chip. */
const FILTER_ICON: Readonly<Record<FilterOption["filter"], IconName>> = {
  type: "files",
  in: "collection",
  modified: "calendar",
};

/** A text element. */
function textElement(tag: string, className: string, text: string): HTMLElement {
  const element = document.createElement(tag);
  element.className = className;
  element.textContent = text;
  return element;
}

/** The eyebrow heading of a section. */
function eyebrow(text: string, id: string): HTMLElement {
  const heading = textElement("h3", "eyebrow", text);
  heading.id = id;
  return heading;
}

/** Mark an element as an option at `index`. */
function asOption(element: HTMLElement, index: number, context: DropdownContext): HTMLElement {
  element.id = `${context.idPrefix}${index}`;
  element.dataset.index = String(index);
  element.setAttribute("role", "option");
  element.setAttribute("aria-selected", String(index === context.active));
  return element;
}

/** A section: its eyebrow and body, grouped for assistive technology. */
function section(className: string, title: string, id: string, body: HTMLElement): HTMLElement {
  const out = document.createElement("div");
  out.className = `section ${className}`;
  out.setAttribute("role", "group");
  out.setAttribute("aria-labelledby", id);
  out.append(eyebrow(title, id), body);
  return out;
}

/** The FILTERS chips. */
function filtersSection(model: DropdownModel, context: DropdownContext): HTMLElement {
  const row = document.createElement("div");
  row.className = "filter-row";
  model.filters.forEach((filter, at) => {
    const button = document.createElement("button");
    button.type = "button";
    button.tabIndex = -1;
    button.className = "filter-chip";
    button.dataset.filter = filter.filter;
    if (filter.open) button.dataset.open = "";
    button.append(icon(FILTER_ICON[filter.filter]), document.createTextNode(filter.text));
    row.append(asOption(button, at, context));
  });
  return section("filters", context.labels.searchFilters, `${context.idPrefix}h-filters`, row);
}

/** One SUGGESTIONS row. */
function suggestionRow(option: SuggestionOption, index: number, context: DropdownContext) {
  const row = document.createElement("div");
  row.className = "suggestion";
  row.dataset.filter = option.filter;
  const words = document.createElement("span");
  words.className = "words";
  words.append(textElement("span", "op", option.op), textElement("span", "value", option.value));
  row.append(icon(ROW_ICON[option.filter]), words, textElement("span", "hint", option.hint));
  return asOption(row, index, context);
}

/** The SUGGESTIONS rows. */
function suggestionsSection(model: DropdownModel, start: number, context: DropdownContext) {
  const rows = document.createElement("div");
  rows.className = "rows";
  model.suggestions.forEach((option, at) => {
    rows.append(suggestionRow(option, start + at, context));
  });
  const title = context.labels.searchSuggestions;
  return section("suggestions", title, `${context.idPrefix}h-suggestions`, rows);
}

/** The file-type badge of a file. */
function badge(row: DriveRow): HTMLElement {
  const ext = badgeExtensionOf(row.name);
  return textElement("span", `badge ${badgeClass(ext)}`, ext === "" ? "FILE" : ext.toUpperCase());
}

/** "{collection} · {time}", or just the time when the host names no collection. */
function fileMeta(row: DriveRow, context: DropdownContext): string {
  const time =
    row.modified === undefined
      ? ""
      : relativeTime(row.modified, context.now(), context.locale, context.labels);
  if (!row.collection) return time;
  return formatLabel(context.labels.searchFileMeta, { collection: row.collection, time });
}

/** The FILES rows: the file's badge, display name and where and when. */
function filesSection(model: DropdownModel, start: number, context: DropdownContext) {
  const rows = document.createElement("div");
  rows.className = "rows";
  model.files.forEach(({ row }, at) => {
    const out = document.createElement("div");
    out.className = "file-option";
    const text = document.createElement("div");
    text.className = "file-text";
    text.append(
      textElement("span", "file-name", row.name),
      textElement("span", "file-meta", fileMeta(row, context)),
    );
    out.append(badge(row), text);
    rows.append(asOption(out, start + at, context));
  });
  return section("files", context.labels.searchFiles, `${context.idPrefix}h-files`, rows);
}

/** The OPERATORS chips and the Enter hint. */
function operatorsSection(model: DropdownModel, start: number, context: DropdownContext) {
  const row = document.createElement("div");
  row.className = "operator-row";
  model.operators.forEach((operator, at) => {
    const chip = document.createElement("button");
    chip.type = "button";
    chip.tabIndex = -1;
    chip.className = "operator-chip";
    chip.textContent = operator.text;
    row.append(asOption(chip, start + at, context));
  });
  const body = document.createElement("div");
  body.append(row, textElement("p", "enter-hint", context.labels.searchEnterHint));
  const title = context.labels.searchOperators;
  return section("operators", title, `${context.idPrefix}h-operators`, body);
}

/**
 * Build the drop-down's sections from a model. Pure DOM: no listeners, no reads.
 *
 * @param model What to show.
 * @param context Labels, locale, clock, the active option and the id prefix.
 * @returns The section elements, in order.
 */
export function renderDropdown(model: DropdownModel, context: DropdownContext): HTMLElement[] {
  const sections = [filtersSection(model, context)];
  let start = model.filters.length;
  if (model.suggestions.length > 0) {
    sections.push(suggestionsSection(model, start, context));
    start += model.suggestions.length;
  }
  if (model.files.length > 0) {
    sections.push(filesSection(model, start, context));
    start += model.files.length;
  }
  sections.push(operatorsSection(model, start, context));
  return sections;
}
