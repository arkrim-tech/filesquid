/**
 * The parts of `<fs-drive-view>` that search results and the sidebar filter add (UI-F2.9, F-063,
 * UI-S-007, UI-S-030; mockups 21 and 53): the scope and count chips under the title, the box
 * that says a search was refused and names the operator, the line "Nothing was searched", and the
 * operators this device answers, with the stylesheet {@link STYLE}. Each function builds elements
 * from the active label table; no name, path or id reaches an attribute.
 */
import { driveSummary, fileCountText, type DriveRow } from "../drive/model";
import { formatSize } from "../files/model";
import { OPERATORS_ON_HOST, type SearchRefusal } from "../search/model";
import { formatLabel, type LabelTable } from "../theme/labels";
import { icon } from "./icons";

/** The stylesheet of the chips, the refusal box and the operator list, tokens only (UI-S-024). */
export const STYLE = `
  .chips { display: flex; flex-wrap: wrap; align-items: center; gap: var(--fs-space-2); margin-block-end: var(--fs-space-3); }
  .chips:empty { display: none; }
  .scope-chip, .count-chip { display: inline-flex; align-items: center; box-sizing: border-box; block-size: 1.625rem; padding: 0 var(--fs-space-2); border: 1px solid var(--fs-surface-raised); border-radius: var(--fs-radius-item); background: var(--fs-surface-raised); font-size: var(--fs-font-size-small); font-weight: 500; color: var(--fs-on-surface-variant); }
  .refused[hidden] { display: none; }
  .refusal { position: relative; inset-inline-start: calc(-1 * var(--fs-space-2)); display: flex; align-items: flex-start; gap: var(--fs-space-2); box-sizing: border-box; inline-size: calc(100% + var(--fs-space-2)); min-block-size: 4rem; padding: var(--fs-space-2) var(--fs-space-3); border: 1px solid var(--fs-error); border-radius: var(--fs-radius); background: var(--fs-surface-card); }
  .refusal .icon { margin-block-start: var(--fs-space-2); color: var(--fs-error); }
  .refusal-text { display: grid; min-width: 0; margin-block-start: var(--fs-space-1); }
  .refusal-title, .refusal-help, .nothing-searched { margin: 0; font-size: var(--fs-font-size-small); line-height: 1.25rem; overflow-wrap: anywhere; }
  .refusal-title { font-weight: 600; color: var(--fs-on-surface); }
  .refusal-help, .nothing-searched { font-weight: 400; color: var(--fs-on-surface-variant); }
  .nothing-searched { box-sizing: border-box; min-block-size: 3.625rem; margin-block-start: var(--fs-space-4); padding-block-start: var(--fs-space-1); }
  .operators-host .eyebrow { block-size: 1.5rem; }
  .operator-list { display: flex; flex-wrap: wrap; gap: var(--fs-space-2); margin: 0; padding: 0; list-style: none; }
  .operator-chip { display: inline-flex; align-items: center; box-sizing: border-box; block-size: 1.375rem; padding: 0 var(--fs-space-1); border: 1px solid var(--fs-outline); border-radius: var(--fs-radius-chip); background: var(--fs-surface-raised); font-family: var(--fs-font-mono); font-size: var(--fs-font-size-small); font-weight: 400; color: var(--fs-on-surface); }
`;

/** The count of files with their total size: "3 files · 20.4 MB", or "0 files". */
export function countChipText(
  rows: readonly DriveRow[],
  labels: LabelTable,
  locale: string,
): string {
  const summary = driveSummary(rows);
  const count = fileCountText(summary.files, labels, locale);
  if (summary.files === 0) return count;
  return formatLabel(labels.searchCountSize, {
    count,
    size: formatSize(summary.bytes, labels, locale),
  });
}

/** The footer's count of results: "3 results", "1 result". */
export function resultCountText(count: number, labels: LabelTable, locale: string): string {
  const template = count === 1 ? labels.searchResultCountOne : labels.searchResultCountOther;
  return formatLabel(template, { count: count.toLocaleString(locale) });
}

/** A chip span with a class. */
function chip(className: string, text: string): HTMLSpanElement {
  const span = document.createElement("span");
  span.className = className;
  span.textContent = text;
  return span;
}

/**
 * The chips under the title: the active scope and the result count. Pure DOM.
 *
 * @param scope The scope chip's text, "In: Family archive".
 * @param count The count chip's text, or undefined while the rows are not an answer yet.
 * @returns The chips, in order.
 */
export function chipNodes(scope: string, count: string | undefined): HTMLElement[] {
  const chips = [chip("scope-chip", scope)];
  return count === undefined ? chips : [...chips, chip("count-chip", count)];
}

/** The refusal box: the alert icon, the operator named in the title and the help line. */
function refusalBox(labels: LabelTable, refusal: SearchRefusal): HTMLElement {
  const unavailable = refusal.kind === "unavailable";
  const values = { operator: refusal.operator };
  const box = document.createElement("div");
  box.className = "refusal";
  box.setAttribute("role", "alert");
  const text = document.createElement("div");
  text.className = "refusal-text";
  const title = document.createElement("p");
  title.className = "refusal-title";
  title.textContent = formatLabel(
    unavailable ? labels.searchUnavailable : labels.searchUnknownOperator,
    values,
  );
  const help = document.createElement("p");
  help.className = "refusal-help";
  help.textContent = formatLabel(
    unavailable ? labels.searchUnavailableHelp : labels.searchUnknownOperatorHelp,
    values,
  );
  text.append(title, help);
  box.append(icon("alert"), text);
  return box;
}

/**
 * The operators the desktop host answers, as chips under the eyebrow OPERATORS ON THIS DEVICE.
 *
 * @param labels The active label table.
 * @returns The block.
 */
export function operatorsHost(labels: LabelTable): HTMLElement {
  const block = document.createElement("div");
  block.className = "operators-host";
  const eyebrow = document.createElement("h3");
  eyebrow.className = "eyebrow";
  eyebrow.textContent = labels.searchOperatorsOnHost;
  const list = document.createElement("ul");
  list.className = "operator-list";
  for (const operator of OPERATORS_ON_HOST) {
    const item = document.createElement("li");
    item.append(chip("operator-chip", operator));
    list.append(item);
  }
  block.append(eyebrow, list);
  return block;
}

/**
 * The content that stands in place of the list when a search was refused: the box naming the
 * operator, "Nothing was searched", and the operators this device answers (mockups 21 and 53).
 *
 * @param labels The active label table.
 * @param refusal What was refused and the operator it named.
 * @returns The nodes, in order.
 */
export function refusalNodes(labels: LabelTable, refusal: SearchRefusal): HTMLElement[] {
  const nothing = document.createElement("p");
  nothing.className = "nothing-searched";
  nothing.textContent = labels.searchNothingSearched;
  return [refusalBox(labels, refusal), nothing, operatorsHost(labels)];
}
