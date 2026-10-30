/**
 * The Edit label dialog of the Versions view (UI-F2.11, UI-S-009, UI-S-031, PRD F-065): "Edit
 * label for v{n}" naming the file, one label field holding the version's label, a help line
 * ("Leave it empty to remove the label."), Cancel and **Save**, the dialog's one filled primary
 * button. It is built like the restore dialog and shares its head, footer and styles
 * ({@link dialogHead}, {@link dialogActions}, `RESTORE_DIALOG_STYLE`); {@link buildLabelDialog}
 * returns typed references so the Versions panel never queries it by selector, and the panel
 * performs the command. {@link labelChange} decides what a typed label means.
 *
 * The dialog sits on `--fs-surface-overlay`, so it draws no muted, placeholder or active text:
 * only body and secondary text, and the refusal message on the card-coloured message box
 * (SRS-UI-007, UI-S-015).
 */
import { formatLabel, type LabelTable } from "../theme/labels";
import type { FileRef } from "../files/model";
import {
  labelProblem,
  dialogActions,
  dialogHead,
  node,
  type DialogParts,
} from "./version-restore-dialog";

/** What the dialog names. */
export interface LabelDialogParams {
  /** The active label table. */
  readonly labels: LabelTable;
  /** The file whose version is labelled. */
  readonly file: FileRef;
  /** The version labelled: `v1`, `v2`, ... */
  readonly version: string;
  /** The version's label now, or null for none. */
  readonly current: string | null;
}

/** What saving a typed label does. */
export type LabelChange =
  | { readonly kind: "invalid" }
  | { readonly kind: "unchanged" }
  | { readonly kind: "set"; readonly label: string | null };

/**
 * Decide what a typed label means for a version. Blank clears the label; the route refuses a
 * label that would change nothing (409), so an unchanged label sends nothing. Pure.
 *
 * @param raw The label as typed.
 * @param current The version's label now, or null for none.
 * @returns `invalid` when the route would refuse the label, `unchanged` when saving would
 *   change nothing, otherwise the label to send (null to clear).
 */
export function labelChange(raw: string, current: string | null): LabelChange {
  if (labelProblem(raw)) return { kind: "invalid" };
  const label = raw.trim() === "" ? null : raw.trim();
  if (label === (current?.trim() ? current : null)) return { kind: "unchanged" };
  return { kind: "set", label };
}

/**
 * Build the Edit label dialog for one version.
 *
 * @param params The file, the version, its label now and the words.
 * @returns The dialog and its references; nothing is attached or sent.
 */
export function buildLabelDialog(params: LabelDialogParams): DialogParts {
  const { labels, file, version, current } = params;
  const top = dialogHead(
    labels,
    formatLabel(labels.versionsEditLabelTitle, { version }),
    "label-title",
  );
  const name = node("p", "file-name", file.display_name);
  name.id = "label-file";
  const field = node("label", "label-field", labels.versionsEditLabelField);
  field.htmlFor = "label-input";
  const input = node("input", "input label-input");
  input.id = "label-input";
  input.type = "text";
  input.autocomplete = "off";
  input.value = current ?? "";
  const help = node("p", "help", labels.versionsEditLabelHelp);
  const error = node("p", "error");
  error.setAttribute("role", "alert");
  const buttons = dialogActions(labels, labels.versionsEditLabelSave, "save");
  const dialog = node("form", "dialog restore-dialog label-dialog");
  dialog.noValidate = true;
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  dialog.setAttribute("aria-labelledby", "label-title");
  dialog.setAttribute("aria-describedby", "label-file");
  const body = node("div", "restore-body");
  body.append(top.element, name, field, input, help, error);
  dialog.append(body, buttons.element);
  const scrim = node("div", "scrim");
  scrim.append(dialog);
  return { scrim, dialog, input, close: top.close, error, ...buttons };
}
