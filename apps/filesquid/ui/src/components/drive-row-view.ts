/**
 * The DOM of one row, tile, group heading and column header of the Drive-style list (UI-F2.8,
 * UI-S-026, mockups 05, 06 and 15), with the stylesheet {@link STYLE}. Each function builds
 * elements from a {@link DriveRow} and the active label table; a row shows only the display name
 * (UI-S-019) and puts no name, path or stored name in an attribute. The name is the row's one
 * Tab stop: a link for a folder (so it opens in the usual ways), a button for a file. The row
 * checkbox and the actions button come with it; the list decides what they do.
 */
import {
  badgeClass,
  extensionOf,
  recentGroup,
  recentTimeText,
  relativeTime,
  sizeText,
  activityOf,
  type DriveRow,
} from "../drive/model";
import { formatLabel, type LabelTable } from "../theme/labels";
import type { SortDirection, SortKey } from "../preferences/model";
import { icon } from "./icons";

/** What a row needs to render. */
export interface RowContext {
  /** The active label table. */
  readonly labels: LabelTable;
  /** BCP 47 tag for numbers and dates. */
  readonly locale: string;
  /** The time now, for relative dates. */
  readonly now: () => Date;
  /** The link of a folder row. */
  readonly href: (row: DriveRow) => string;
  /** Whether the Recent layout (activity text, no size or version) is shown. */
  readonly recent: boolean;
}

/** The current sort, or undefined when the view has a fixed order (Recent). */
export interface SortState {
  /** The key. */
  readonly sort: SortKey;
  /** The direction. */
  readonly direction: SortDirection;
}

/** The stylesheet of rows, tiles, headers and the checkbox, tokens only (UI-S-024). */
export const STYLE = `
  .table { --cols: 1.5rem 0.5rem 3.25rem minmax(0, 1fr) 6.25rem 1.875rem 6.875rem 5rem 0.375rem 1.875rem; display: flex; flex-direction: column; flex: 1 1 auto; min-width: 0; min-height: 0; background: var(--fs-surface); }
    .head { display: grid; grid-template-columns: var(--cols); align-items: center; box-sizing: border-box; padding-inline-end: var(--fs-space-2); block-size: 2.375rem; border-block-end: 1px solid var(--fs-outline-strong); font-size: var(--fs-font-size-small); font-weight: 500; color: var(--fs-on-surface-variant); }
  .head .col-check { grid-column: 1; display: flex; justify-content: center; }
  .head .col-name { grid-column: 4; } .head .col-size { grid-column: 5; text-align: end; }
  .head .col-modified { grid-column: 7; } .head .col-version { grid-column: 8; }
  .head [aria-sort] { font-weight: 600; color: var(--fs-on-surface); }
  button.sort { display: inline-flex; align-items: center; gap: var(--fs-space-1); padding: 0; border: 0; background: none; color: inherit; font: inherit; cursor: pointer; }
  .scroller { position: relative; flex: 1 1 auto; min-width: 0; min-height: 0; overflow: auto; }
  .space { position: relative; inline-size: 100%; block-size: calc(var(--rows, 0) * var(--row-height)); }
  .row { position: absolute; inset-inline: 0; inset-block-start: calc(var(--r) * var(--row-height)); box-sizing: border-box; display: grid; grid-template-columns: var(--cols); align-items: center; padding-inline-end: var(--fs-space-2); block-size: var(--row-height); border-block-end: 1px solid var(--fs-outline); font-size: var(--fs-font-size-ui); }
  .row:hover, .row:focus-within, .row[data-selected], .row[data-menu] { background: var(--fs-surface-raised); }
  .row .check-cell { grid-column: 1; display: flex; justify-content: center; }
  .row .badge-cell { grid-column: 3; display: flex; align-items: center; }
  .row .name-cell { grid-column: 4; min-width: 0; }
  .row .size { grid-column: 5; text-align: end; }
  .row .modified { grid-column: 7; } .row .version { grid-column: 8; }
  .row .actions-cell { grid-column: 10; display: flex; justify-content: center; }
  .row .cell { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .row .name-cell { font-size: var(--fs-font-size-ui); color: var(--fs-on-surface); }
  .name { display: block; max-inline-size: 100%; padding: var(--fs-space-1) 0; border: 0; background: none; color: inherit; font: inherit; text-align: start; text-decoration: none; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; cursor: pointer; }
  .name:hover { text-decoration: underline; }
  .row .collection { grid-column: 5 / 7; text-align: start; }
  .recent .row .activity { grid-column: 7 / 11; text-align: end; }
  .group { position: absolute; inset-inline: 0; inset-block-start: calc(var(--r) * var(--row-height)); box-sizing: border-box; display: flex; align-items: end; block-size: var(--row-height); padding: 0 var(--fs-space-3) var(--fs-space-2); }
  .group .eyebrow { margin: 0; }
  .badge { display: inline-flex; align-items: center; justify-content: center; box-sizing: border-box; min-inline-size: 1.875rem; block-size: 1rem; padding: 0 var(--fs-space-1); border: 1px solid var(--fs-surface-raised); border-radius: var(--fs-radius-badge); font-family: var(--fs-font-mono); font-size: var(--fs-font-size-badge); font-weight: 700; text-transform: uppercase; background: var(--fs-surface-raised); color: var(--fs-on-surface-variant); }
  .badge.document { border-color: var(--fs-badge-document); background: var(--fs-badge-document); color: var(--fs-on-badge-document); }
  .badge.data { border-color: var(--fs-badge-data); background: var(--fs-badge-data); color: var(--fs-on-badge-data); }
  .badge.media { border-color: var(--fs-badge-media); background: var(--fs-badge-media); color: var(--fs-on-badge-media); }
  .row .icon-folder, .row .icon-collection, .row .icon-layers, .tile .thumb .icon { color: var(--fs-icon); }
  .actions { box-sizing: border-box; inline-size: 1.875rem; block-size: 1.875rem; display: inline-flex; align-items: center; justify-content: center; padding: 0; border: 1px solid var(--fs-outline-strong); border-radius: var(--fs-radius-input); background: var(--fs-surface-card); color: var(--fs-icon); cursor: pointer; }
  .row:not(:hover):not(:focus-within):not([data-menu]) .actions, .tile:not(:hover):not(:focus-within):not([data-menu]) .actions { visibility: hidden; }
  input.check, input.check-all { appearance: none; box-sizing: border-box; inline-size: 0.90625rem; block-size: 0.90625rem; margin: 0; border: 1.5px solid var(--fs-outline-control); border-radius: var(--fs-radius-check); background: var(--fs-surface-card); cursor: pointer; }
  input.check:checked, input.check-all:checked, input.check-all:indeterminate { border-color: var(--fs-primary); background: var(--fs-primary) center / 0.75rem no-repeat; }
  input.check:checked, input.check-all:checked { background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23fff' stroke-width='3' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M5 12.5l4.5 4.5L19 7.5'/%3E%3C/svg%3E"); }
  input.check-all:indeterminate { background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%23fff' stroke-width='3' stroke-linecap='round'%3E%3Cpath d='M6 12h12'/%3E%3C/svg%3E"); }
  .tile { position: absolute; box-sizing: border-box; inline-size: 13.375rem; block-size: 13.625rem; inset-inline-start: calc(var(--c) * 14.5rem); inset-block-start: calc(var(--r) * var(--row-height)); display: flex; flex-direction: column; align-items: center; gap: var(--fs-space-1); padding: 0; border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface); }
  .tile:hover, .tile:focus-within, .tile[data-selected] { background: var(--fs-surface-raised); }
  .tile .thumb { position: relative; margin-block-start: var(--fs-space-1); box-sizing: border-box; display: grid; place-items: center; inline-size: 12.625rem; block-size: 9.625rem; border-radius: var(--fs-radius-item); background: var(--fs-surface-raised); }
  .tile .thumb .badge { min-inline-size: 2.5rem; block-size: 1.5rem; font-size: var(--fs-font-size-small); }
  .tile .check { position: absolute; inset-block-start: var(--fs-space-2); inset-inline-start: var(--fs-space-2); }
  .tile .actions { position: absolute; inset-block-start: var(--fs-space-2); inset-inline-end: var(--fs-space-2); }
  .tile .name { padding: 0; max-inline-size: 12.625rem; inline-size: 100%; font-size: var(--fs-font-size-small); font-weight: 500; color: var(--fs-on-surface); }
  .tile .meta { font-size: var(--fs-font-size-small); color: var(--fs-on-surface-muted); }
  .placeholder-row { position: relative; block-size: var(--row-height); border-block-end: 1px solid var(--fs-outline); }
  .placeholder-row::after { content: ""; position: absolute; inset-block: 0.875rem; inset-inline: 3.25rem 40%; border-radius: var(--fs-radius-item); background: var(--fs-surface-raised); }
  @media (max-width: 40rem) {
    .table { --cols: 1.5rem 0.5rem 3.25rem minmax(0, 1fr) 5rem 0 0 0 0 1.875rem; }
    .head .col-modified, .head .col-version, .row .modified, .row .version { display: none; }
  }
`;

/** Make a `div` with a class and role. */
function cell(className: string, role: string, text = ""): HTMLElement {
  const div = document.createElement("div");
  div.className = className;
  div.setAttribute("role", role);
  div.textContent = text;
  return div;
}

/** The row's checkbox, named from the label table. */
function checkbox(row: DriveRow, context: RowContext, checked: boolean): HTMLInputElement {
  const input = document.createElement("input");
  input.type = "checkbox";
  input.className = "check";
  input.checked = checked;
  input.tabIndex = -1;
  input.setAttribute("aria-label", formatLabel(context.labels.fileSelectRow, { name: row.name }));
  return input;
}

/** The icon of a folder row: layers for a virtual collection, else the collection folder. */
function folderIcon(row: DriveRow): SVGSVGElement {
  return icon(row.nodeKind === "virtual_collection" ? "layers" : "collection");
}

/** The file-type badge of a file, or the folder icon of a folder. */
function badgeOf(row: DriveRow): SVGSVGElement | HTMLElement {
  if (row.kind === "folder") return folderIcon(row);
  const ext = extensionOf(row.name);
  const badge = document.createElement("span");
  badge.className = `badge ${badgeClass(ext)}`;
  badge.textContent = ext === "" ? "FILE" : ext.toUpperCase();
  return badge;
}

/** The row's name control: a link for a folder, a button for a file. */
function nameControl(row: DriveRow, context: RowContext): HTMLElement {
  if (row.kind === "folder") {
    const link = document.createElement("a");
    link.className = "name";
    link.href = context.href(row);
    link.textContent = row.name;
    return link;
  }
  const button = document.createElement("button");
  button.type = "button";
  button.className = "name";
  button.textContent = row.name;
  return button;
}

/** The actions button of a row or tile. */
function actionsButton(row: DriveRow, context: RowContext): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "actions";
  button.tabIndex = -1;
  button.setAttribute("aria-haspopup", "menu");
  button.setAttribute("aria-label", formatLabel(context.labels.fileActions, { name: row.name }));
  button.append(icon("more"));
  return button;
}

/** The text at the right of a Recent row: "You changed it · 2 hours ago". */
function activityText(row: DriveRow, context: RowContext): string {
  const activity = activityOf(row);
  if (!activity) return "";
  const now = context.now();
  const group = recentGroup(activity.at, now) ?? "earlier";
  const time = recentTimeText(activity.at, group, now, context.locale, context.labels);
  const template =
    activity.kind === "changed"
      ? context.labels.fileRecentChanged
      : context.labels.fileRecentOpened;
  return formatLabel(template, { time });
}

/** The modified cell: a `<time>` with the relative date. */
function modifiedCell(row: DriveRow, context: RowContext): HTMLElement {
  const out = cell("cell modified", "cell");
  if (row.modified === undefined) return out;
  const time = document.createElement("time");
  time.dateTime = row.modified;
  time.textContent = relativeTime(row.modified, context.now(), context.locale, context.labels);
  out.append(time);
  return out;
}

/** The version chip cell of a file. */
function versionCell(row: DriveRow): HTMLElement {
  const out = cell("cell version", "cell");
  if (row.kind === "file" && row.version) {
    const chip = document.createElement("span");
    chip.className = "mono-chip";
    chip.textContent = row.version;
    out.append(chip);
  }
  return out;
}

/**
 * One row of the list layout (`role="row"`): checkbox, folder icon or file-type badge, name,
 * size, modified, version and the actions button; the Recent layout has the collection and the
 * activity text instead of size, modified and version.
 *
 * @param row The row.
 * @param context Labels, locale, clock, links and layout.
 * @param selected Whether the row is ticked.
 * @returns The row element.
 */
export function listRow(row: DriveRow, context: RowContext, selected: boolean): HTMLElement {
  const out = document.createElement("div");
  out.className = "row";
  out.dataset.kind = row.kind;
  out.setAttribute("role", "row");
  if (selected) out.dataset.selected = "";
  const check = cell("cell check-cell", "cell");
  check.append(checkbox(row, context, selected));
  const badge = cell("cell badge-cell", "cell");
  badge.append(badgeOf(row));
  const name = cell("cell name-cell", "cell");
  name.append(nameControl(row, context));
  const actions = cell("cell actions-cell", "cell");
  actions.append(actionsButton(row, context));
  out.append(check, badge, name);
  if (context.recent) {
    out.append(cell("cell collection", "cell", row.collection ?? ""));
    out.append(cell("cell activity", "cell", activityText(row, context)));
  } else {
    out.append(cell("cell size", "cell", sizeText(row, context.labels, context.locale)));
    out.append(modifiedCell(row, context), versionCell(row));
  }
  out.append(actions);
  return out;
}

/** The size, a separator and the date of a tile, or only what is known. */
function tileMeta(row: DriveRow, context: RowContext): string {
  const size = sizeText(row, context.labels, context.locale);
  const date =
    row.modified === undefined
      ? ""
      : relativeTime(row.modified, context.now(), context.locale, context.labels);
  if (size === "" || date === "") return size || date;
  return formatLabel(context.labels.fileTileMeta, { size, date });
}

/**
 * One tile of the grid layout (`role="listitem"`): a thumbnail area with the badge or folder
 * icon, the checkbox and the actions button, the name and "{size} · {date}".
 *
 * @param row The row.
 * @param context Labels, locale, clock, links and layout.
 * @param selected Whether the row is ticked.
 * @returns The tile element.
 */
export function gridTile(row: DriveRow, context: RowContext, selected: boolean): HTMLElement {
  const out = document.createElement("div");
  out.className = "tile";
  out.dataset.kind = row.kind;
  out.setAttribute("role", "listitem");
  if (selected) out.dataset.selected = "";
  const thumb = document.createElement("div");
  thumb.className = "thumb";
  thumb.append(badgeOf(row), checkbox(row, context, selected), actionsButton(row, context));
  const meta = document.createElement("span");
  meta.className = "meta";
  meta.textContent = tileMeta(row, context);
  out.append(thumb, nameControl(row, context), meta);
  return out;
}

/**
 * A day heading of the Recent view, an eyebrow.
 *
 * @param text The group's name from the label table.
 * @returns The heading element.
 */
export function groupHeading(text: string): HTMLElement {
  const out = document.createElement("div");
  out.className = "group";
  const eyebrow = document.createElement("h3");
  eyebrow.className = "eyebrow";
  eyebrow.textContent = text;
  out.append(eyebrow);
  return out;
}

/** The sortable heads: key, class and label key suffix. */
const HEADS = [
  { id: "name", sort: "name" as SortKey, labelKey: "fileColumnName" as const },
  { id: "size", sort: "size" as SortKey, labelKey: "fileColumnSize" as const },
  { id: "modified", sort: "modified" as SortKey, labelKey: "fileColumnModified" as const },
  { id: "version", sort: undefined, labelKey: "fileColumnVersion" as const },
];

/** One column head; a sortable one is a button that carries `aria-sort` when it is the sort. */
function head(
  entry: (typeof HEADS)[number],
  labels: LabelTable,
  sort: SortState | undefined,
): HTMLElement {
  const out = cell(`col col-${entry.id}`, "columnheader");
  const label = labels[entry.labelKey];
  if (!entry.sort || !sort) {
    out.textContent = label;
    return out;
  }
  const button = document.createElement("button");
  button.type = "button";
  button.className = "sort";
  button.dataset.sort = entry.sort;
  button.append(document.createTextNode(label));
  if (sort.sort === entry.sort) {
    out.setAttribute("aria-sort", sort.direction);
    button.append(icon(sort.direction === "ascending" ? "arrow-up" : "arrow-down"));
  }
  out.append(button);
  return out;
}

/**
 * The header row of the list layout: the header checkbox and the column heads Name, Size,
 * Modified and Version; the Recent layout has none (its order is fixed).
 *
 * @param labels The active label table.
 * @param sort The current sort, or undefined for a fixed order.
 * @returns The header element.
 */
export function headerRow(labels: LabelTable, sort: SortState | undefined): HTMLElement {
  const out = document.createElement("div");
  out.className = "head";
  out.setAttribute("role", "row");
  const checkCell = cell("col col-check", "columnheader");
  const all = document.createElement("input");
  all.type = "checkbox";
  all.className = "check-all";
  all.tabIndex = 0;
  all.setAttribute("aria-label", labels.fileSelectAll);
  checkCell.append(all);
  out.append(checkCell, ...HEADS.map((entry) => head(entry, labels, sort)));
  return out;
}
