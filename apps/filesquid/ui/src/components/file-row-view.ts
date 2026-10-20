/**
 * The DOM of one row or tile of the file listing, and the listing's header (UI-F2.2). Each
 * function builds elements from a {@link FileRow} and the active label table; the stylesheet
 * for them is {@link STYLE}, which the browser puts in its shadow root. A row shows only the
 * display name (UI-S-019) and puts no path or stored name in an attribute. The name is the row's
 * one interactive control: a link for a folder (so it can be opened in the usual ways), a
 * button for a file. Columns are data (`COLUMNS`), so sorting (UI-F2.8) can attach to a header
 * without changing a row.
 */
import type { LabelKey, LabelTable } from "../theme/labels";
import { formatLabel } from "../theme/labels";
import { formatModified, formatSize, type FileRow } from "../files/model";

/** What a row needs to render. */
export interface RowContext {
  /** The active label table. */
  readonly labels: LabelTable;
  /** BCP 47 tag for numbers and dates. */
  readonly locale: string;
  /** The link of a folder row. */
  readonly href: (row: FileRow) => string;
}

/** One column of the list layout: its class and the label of its header. */
interface Column {
  /** Class of its header and cells. */
  readonly id: "name" | "size" | "type" | "modified" | "version";
  /** Label key of its header. */
  readonly label: LabelKey;
}

/** The columns of the list layout, in order. */
export const COLUMNS: readonly Column[] = [
  { id: "name", label: "fileColumnName" },
  { id: "size", label: "fileColumnSize" },
  { id: "type", label: "fileColumnType" },
  { id: "modified", label: "fileColumnModified" },
  { id: "version", label: "fileColumnVersion" },
];

export const STYLE = `
  .table { --fs-file-columns: minmax(0, 1fr) 6rem 5rem 8rem 4rem; display: flex; flex-direction: column; min-width: 0; border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface); }
  .head { display: grid; grid-template-columns: var(--fs-file-columns); align-items: center; gap: var(--fs-space-2); padding: var(--fs-space-2) var(--fs-space-3); border-block-end: 1px solid var(--fs-outline); color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); font-weight: 700; }
  .scroller { position: relative; min-width: 0; overflow: auto; block-size: min(60vh, 40rem); }
  .space { position: relative; inline-size: 100%; block-size: calc(var(--rows) * var(--fs-file-row-height)); }
  .row, .tile { position: absolute; box-sizing: border-box; display: grid; align-items: center; gap: var(--fs-space-2); min-width: 0; padding: var(--fs-space-1) var(--fs-space-3); inset-block-start: calc(var(--r) * var(--fs-file-row-height)); block-size: var(--fs-file-row-height); }
  .row { inset-inline: 0; grid-template-columns: var(--fs-file-columns); border-block-end: 1px solid var(--fs-outline); }
  .tile { inset-inline-start: calc(var(--c) * 100% / var(--cols)); inline-size: calc(100% / var(--cols)); grid-template-rows: 1fr auto auto; justify-items: start; border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); background: var(--fs-surface-container); }
  .row[data-active], .tile[data-active] { background: var(--fs-primary-container); color: var(--fs-on-primary-container); }
  .cell { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .name-cell { display: flex; align-items: center; gap: var(--fs-space-2); font-size: var(--fs-font-size-body); color: var(--fs-on-surface); }
  .name { min-width: 0; padding: var(--fs-space-1) var(--fs-space-2); border: 0; border-radius: var(--fs-radius); background: none; color: inherit; font: inherit; text-align: start; text-decoration: none; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; cursor: pointer; }
  .name:hover { text-decoration: underline; }
  .type { flex: 0 0 auto; padding: 0 var(--fs-space-2); border: 1px solid var(--fs-outline); border-radius: var(--fs-radius); font-size: var(--fs-font-size-small); font-weight: 700; color: var(--fs-on-surface-variant); }
  .pending { flex: 0 1 auto; overflow: hidden; text-overflow: ellipsis; font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .meta { font-size: var(--fs-font-size-small); color: var(--fs-on-surface-variant); }
  .tile .name { max-inline-size: 100%; }
  @media (max-width: 40rem) {
    .table { --fs-file-columns: minmax(0, 1fr) 5rem; }
    .head .type, .head .modified, .head .version, .row .type-cell, .row .modified, .row .version { display: none; }
  }
`;

/** The header row of the list layout. */
export function headerRow(labels: LabelTable): HTMLElement {
  const head = document.createElement("div");
  head.className = "head";
  head.setAttribute("role", "row");
  head.setAttribute("aria-rowindex", "1");
  for (const column of COLUMNS) {
    const cell = document.createElement("div");
    cell.className = column.id;
    cell.setAttribute("role", "columnheader");
    cell.textContent = labels[column.label];
    head.append(cell);
  }
  return head;
}

/** A `<div>` cell with a class, a role and text. */
function cell(className: string, role: string, text = ""): HTMLElement {
  const div = document.createElement("div");
  div.className = className;
  div.setAttribute("role", role);
  div.textContent = text;
  return div;
}

/** The type badge text: the extension hint in capitals, or the folder or generic label. */
function typeText(row: FileRow, labels: LabelTable): string {
  if (row.kind === "folder") return labels.folder;
  return row.typeHint ? row.typeHint.toUpperCase() : labels.fileTypeUnknown;
}

/** The row's name control: a link for a folder, a button for a file. */
function nameControl(row: FileRow, context: RowContext): HTMLElement {
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

/** The `<time>` of a file's modified date. */
function modifiedCell(row: FileRow, context: RowContext): HTMLElement {
  const out = cell("cell modified", "cell");
  if (row.modified === undefined) return out;
  const time = document.createElement("time");
  time.dateTime = row.modified;
  time.textContent = formatModified(row.modified, context.locale);
  out.append(time);
  return out;
}

/** One row of the list layout (`role="row"`). */
export function listRow(row: FileRow, context: RowContext): HTMLElement {
  const out = document.createElement("div");
  out.className = "row";
  out.dataset.kind = row.kind;
  out.setAttribute("role", "row");
  const name = cell("cell name-cell", "cell");
  name.append(nameControl(row, context));
  if (row.pending) name.append(pendingNote(context.labels));
  const size = row.size === undefined ? "" : formatSize(row.size, context.labels, context.locale);
  const type = cell("cell type-cell", "cell");
  const badge = document.createElement("span");
  badge.className = "type";
  badge.textContent = typeText(row, context.labels);
  type.append(badge);
  out.append(
    name,
    cell("cell size", "cell", size),
    type,
    modifiedCell(row, context),
    cell("cell version", "cell", row.version ?? ""),
  );
  return out;
}

/** One tile of the grid layout (`role="listitem"`). */
export function gridTile(row: FileRow, context: RowContext): HTMLElement {
  const out = document.createElement("div");
  out.className = "tile";
  out.dataset.kind = row.kind;
  out.setAttribute("role", "listitem");
  const badge = document.createElement("span");
  badge.className = "type";
  badge.textContent = typeText(row, context.labels);
  const meta = document.createElement("span");
  meta.className = "meta";
  meta.textContent = tileMeta(row, context);
  out.append(badge, nameControl(row, context), meta);
  if (row.pending) out.append(pendingNote(context.labels));
  return out;
}

/** Size and date of a file tile, or the pending note's place for a folder (empty). */
function tileMeta(row: FileRow, context: RowContext): string {
  if (row.size === undefined || row.modified === undefined) return "";
  const size = formatSize(row.size, context.labels, context.locale);
  return formatLabel(context.labels.fileTileMeta, {
    size,
    date: formatModified(row.modified, context.locale),
  });
}

/** The marker of a folder that is pending until a file is added. */
function pendingNote(labels: LabelTable): HTMLElement {
  const note = document.createElement("span");
  note.className = "pending";
  note.textContent = labels.fileFolderPending;
  return note;
}
