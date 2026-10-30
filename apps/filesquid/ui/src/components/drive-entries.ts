/**
 * The entries of the Drive-style menus (UI-F2.8, UI-S-026, mockups 06 and 10), as pure
 * functions of the label table and the state: the item menu of a row, the grid's sort menu and
 * the New menu. Keeping them pure means the words, the order and the disabled reasons are tested
 * without a DOM, and `drive-menu.ts` only draws them.
 */
import { canAct } from "../drive/actions";
import { DIRECTION_LABEL, PLACEMENT_LABEL, SORT_LABEL, type DriveRow } from "../drive/model";
import {
  FOLDER_PLACEMENTS,
  SORT_DIRECTIONS,
  SORT_KEYS,
  type FolderPlacement,
  type SortDirection,
  type SortKey,
} from "../preferences/model";
import type { LabelTable } from "../theme/labels";
import type { MenuEntry } from "./drive-menu";

/** The sort and folder placement a sort menu shows as chosen. */
export interface SortChoice {
  /** The sort key. */
  readonly sort: SortKey;
  /** The direction. */
  readonly direction: SortDirection;
  /** Where folders go. */
  readonly folders: FolderPlacement;
}

/** The id of a sort-menu item: `sort:name`, `direction:ascending`, `folders:on_top`. */
export function sortItemId(group: "sort" | "direction" | "folders", value: string): string {
  return `${group}:${value}`;
}

/**
 * The grid's sort menu: the eyebrow SORT BY, the five keys, Ascending and Descending, Folders on
 * top and Folders mixed with files, the chosen items checked (UI-S-026).
 *
 * @param labels The active label table.
 * @param chosen What is chosen now.
 * @returns The entries.
 */
export function sortEntries(labels: LabelTable, chosen: SortChoice): MenuEntry[] {
  const radio = (
    group: "sort" | "direction" | "folders",
    value: string,
    text: string,
    on: boolean,
  ) => ({ kind: "radio", id: sortItemId(group, value), label: text, checked: on }) as const;
  return [
    { kind: "eyebrow", text: labels.fileSortBy },
    ...SORT_KEYS.map((key) => radio("sort", key, labels[SORT_LABEL[key]], key === chosen.sort)),
    { kind: "divider" },
    ...SORT_DIRECTIONS.map((d) =>
      radio("direction", d, labels[DIRECTION_LABEL[d]], d === chosen.direction),
    ),
    { kind: "divider" },
    ...FOLDER_PLACEMENTS.map((p) =>
      radio("folders", p, labels[PLACEMENT_LABEL[p]], p === chosen.folders),
    ),
  ];
}

/**
 * The item menu of a row (the actions button and the right click): for a file Open, Details,
 * Rename (↩), Move or copy…, Download and Remove in the error colour, with dividers as mockup 10
 * draws them; for a folder Open only. A write the host cannot address yet (no library named for
 * the file) is shown disabled with the reason in its tooltip.
 *
 * @param labels The active label table.
 * @param row The row.
 * @param canDownload Whether this host can download a single file now.
 * @returns The entries.
 */
export function itemEntries(labels: LabelTable, row: DriveRow, canDownload: boolean): MenuEntry[] {
  const open: MenuEntry = { kind: "item", id: "open", label: labels.fileOpen };
  if (row.kind === "folder") return [open];
  const acts = canAct(row);
  const reason = labels.fileActionUnavailable;
  const gated = (disabled: boolean) => (disabled ? { disabled: true, reason } : {});
  return [
    open,
    { kind: "item", id: "details", label: labels.fileDetails, ...gated(!acts) },
    { kind: "divider" },
    { kind: "item", id: "rename", label: labels.fileRename, shortcut: "↩", ...gated(!acts) },
    { kind: "item", id: "move", label: labels.fileMoveOrCopyItem, ...gated(!acts) },
    { kind: "item", id: "download", label: labels.fileDownload, ...gated(!acts || !canDownload) },
    { kind: "divider" },
    { kind: "item", id: "remove", label: labels.fileRemove, destructive: true, ...gated(!acts) },
  ];
}

/**
 * The New menu (mockup 10): Upload files… (⌘O), Upload folder…, a divider and New folder (⇧⌘N).
 * The upload items are enabled only while a collection or a folder inside one is shown (UI-S-002
 * **Destination**) and say why when they are not.
 *
 * @param labels The active label table.
 * @param canUpload Whether a collection or folder is shown and the host can place uploads.
 * @returns The entries.
 */
export function newEntries(labels: LabelTable, canUpload: boolean): MenuEntry[] {
  const gate = canUpload ? {} : { disabled: true, reason: labels.fileUploadDisabledReason };
  return [
    { kind: "item", id: "upload-files", label: labels.fileUploadFiles, shortcut: "⌘O", ...gate },
    { kind: "item", id: "upload-folder", label: labels.fileUploadFolder, ...gate },
    { kind: "divider" },
    { kind: "item", id: "new-folder", label: labels.fileNewFolder, shortcut: "⇧⌘N", ...gate },
  ];
}
