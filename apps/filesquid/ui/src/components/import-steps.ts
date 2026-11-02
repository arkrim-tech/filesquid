/**
 * The DOM of the two steps of the upload dialog (UI-1.2, UI-S-028): the destination step of
 * mockup 52 (680 px wide) with its planning state, and the review of mockup 18 (860 px wide).
 * Each builder only builds and wires the DOM it owns; what a click does (the plan request, the
 * start, the discard) is the dialog element's decision. Every word comes from the label table,
 * every colour and size from a token, and no source path appears anywhere: the plan carries only
 * counts, sizes, display names and the volume's label (API-S-036).
 */
import { badgeClass, badgeExtensionOf } from "../drive/model";
import { formatSize } from "../files/model";
import {
  numberedName,
  type ImportPlan,
  type ImportSourceKind,
  type PlanItem,
} from "../imports/model";
import { formatLabel, type LabelTable } from "../theme/labels";
import { destinationText } from "../imports/model";
import type { FsDestinationPicker } from "./fs-destination-picker";
import { icon } from "./icons";
import { actionButton, node } from "./library-parts";

/** The parts of the destination step the dialog drives. */
export interface DestinationStep {
  readonly dialog: HTMLElement;
  readonly body: HTMLElement;
  readonly picker: FsDestinationPicker;
  readonly field: HTMLInputElement;
  readonly start: HTMLButtonElement;
  readonly cancel: HTMLButtonElement;
  readonly close: HTMLButtonElement;
  readonly reason: HTMLElement;
  readonly radios: Readonly<Record<ImportSourceKind, HTMLInputElement>>;
}

/** The dialog frame: role, name and the head with its title, close control and intro. */
function frame(
  className: string,
  labels: LabelTable,
  title: string,
  subtitle: string | undefined,
  subtitleClass: string,
): { dialog: HTMLElement; head: HTMLElement; close: HTMLButtonElement } {
  const dialog = node("div", `dialog ${className}`);
  dialog.setAttribute("role", "dialog");
  dialog.setAttribute("aria-modal", "true");
  const heading = node("h2", "dialog-title", title);
  heading.id = `dialog-title-${className}`;
  dialog.setAttribute("aria-labelledby", heading.id);
  const close = node("button", "button plain close");
  close.type = "button";
  close.dataset.action = "close";
  close.setAttribute("aria-label", labels.importClose);
  close.append(icon("close", 20));
  const head = node("div", "head");
  head.append(heading, close);
  if (subtitle !== undefined) head.append(node("p", subtitleClass, subtitle));
  dialog.append(head);
  return { dialog, head, close };
}

/** One radio of the source group. */
function sourceRadio(kind: ImportSourceKind, text: string, checked: boolean): HTMLLabelElement {
  const label = node("label");
  const input = node("input");
  input.type = "radio";
  input.name = "import-source";
  input.value = kind;
  input.checked = checked;
  label.append(input, document.createTextNode(text));
  return label;
}

/** The Upload radio group: Files or a folder with its structure. */
function sourceGroup(labels: LabelTable, source: ImportSourceKind): HTMLFieldSetElement {
  const group = node("fieldset", "source");
  group.append(node("legend", "", labels.importSourceLegend));
  const radios = node("div", "radios");
  radios.append(
    sourceRadio("files", labels.importSourceFiles, source === "files"),
    sourceRadio("folder", labels.importSourceFolder, source === "folder"),
  );
  group.append(radios);
  return group;
}

/**
 * Build the destination step (mockup 52): the title, the intro, the source group, the required
 * destination field, the tree of collections and folders, its note, and the footer with Cancel
 * and the start control, which is disabled until a destination is chosen.
 *
 * @param labels The active label table.
 * @param source The source the Upload group starts on.
 * @returns The step's dialog and the parts the dialog element wires.
 */
export function destinationStep(labels: LabelTable, source: ImportSourceKind): DestinationStep {
  const { dialog, close } = frame(
    "import",
    labels,
    labels.importTitle,
    labels.importIntro,
    "intro",
  );
  const body = node("div", "body");
  const head = node("div", "field-head");
  head.append(node("span", "field-label", labels.importDestination));
  head.append(node("span", "tag", labels.importRequired));
  const field = node("input", "input destination-field");
  field.type = "text";
  field.readOnly = true;
  field.placeholder = labels.importDestinationPlaceholder;
  field.setAttribute("aria-label", labels.importDestination);
  field.setAttribute("aria-required", "true");
  const picker = document.createElement("fs-destination-picker") as FsDestinationPicker;
  const tree = node("div", "picker");
  tree.append(picker);
  body.append(
    sourceGroup(labels, source),
    head,
    field,
    tree,
    node("p", "help", labels.importDestinationNote),
  );
  const foot = node("div", "foot");
  const reason = node("p", "reason", labels.importChooseDestinationFirst);
  const cancel = actionButton("cancel", labels.fileCancel);
  const start = actionButton("start", "", "primary");
  foot.append(reason, cancel, start);
  dialog.append(body, foot);
  const radios = {
    files: dialog.querySelector<HTMLInputElement>('input[value="files"]') as HTMLInputElement,
    folder: dialog.querySelector<HTMLInputElement>('input[value="folder"]') as HTMLInputElement,
  };
  const step = { dialog, body, picker, field, start, cancel, close, reason, radios };
  showStart(step, labels, source, undefined);
  return step;
}

/**
 * Word the start control and the footer reason for the source and the chosen destination: the
 * control reads "Choose files…" or "Choose folder…", is disabled until a destination is chosen,
 * and the footer says so; the destination field shows the choice. Side effect: changes the DOM.
 *
 * @param step The destination step.
 * @param labels The active label table.
 * @param source The source chosen in the Upload group.
 * @param trail The names from the library down to the chosen destination, or undefined.
 */
export function showStart(
  step: DestinationStep,
  labels: LabelTable,
  source: ImportSourceKind,
  trail: readonly string[] | undefined,
): void {
  step.start.textContent =
    source === "files" ? labels.importChooseFiles : labels.importChooseFolder;
  step.start.disabled = trail === undefined;
  step.reason.textContent = trail === undefined ? labels.importChooseDestinationFirst : "";
  step.field.value = trail === undefined ? "" : destinationText(trail);
}

/**
 * The planning state: the host's file dialog is open, then the picked files are checked. There is
 * no percentage, because the host cannot tell how long checking a large pick takes (DE-1.8).
 *
 * @param labels The active label table.
 * @returns A polite status block.
 */
export function planningBlock(labels: LabelTable): HTMLElement {
  const block = node("div", "planning", labels.importPlanning);
  block.setAttribute("role", "status");
  return block;
}

/** What the review needs. */
export interface ReviewContext {
  readonly labels: LabelTable;
  readonly plan: ImportPlan;
  /** The names from the library down to the destination the owner chose. */
  readonly trail: readonly string[];
  readonly locale: string;
}

/** The parts of the review the dialog drives. */
export interface ReviewStep {
  readonly dialog: HTMLElement;
  readonly body: HTMLElement;
  readonly list: HTMLElement;
  readonly note: HTMLElement;
  readonly start: HTMLButtonElement;
  readonly cancel: HTMLButtonElement;
  readonly change: HTMLButtonElement;
  readonly close: HTMLButtonElement;
}

/** One count card: the number in the heading font and its caption. */
function countCard(count: string, caption: string): HTMLElement {
  const card = node("div", "card");
  card.append(node("p", "count", count), node("p", "caption", caption));
  return card;
}

/** The four count cards of the plan. */
function countCards(ctx: ReviewContext): HTMLElement {
  const { labels, plan, locale } = ctx;
  const count = (n: number) => n.toLocaleString(locale);
  const cards = node("div", "cards");
  cards.append(
    countCard(count(plan.files_found), labels.importCountFound),
    countCard(count(plan.files_new), labels.importCountNew),
    countCard(count(plan.files_present), labels.importCountPresent),
    countCard(formatSize(plan.bytes_to_copy, labels, locale), labels.importCountToCopy),
  );
  return cards;
}

/** The destination column: the required field, Change… and the numbering note. */
function destinationColumn(ctx: ReviewContext): {
  column: HTMLElement;
  note: HTMLElement;
  change: HTMLButtonElement;
} {
  const { labels } = ctx;
  const column = node("div", "destination");
  const row = node("div", "chosen");
  const field = node("input", "input destination-field");
  field.type = "text";
  field.readOnly = true;
  field.value = destinationText(ctx.trail);
  field.setAttribute("aria-label", labels.importDestination);
  const change = node("button", "button ghost change", labels.importChange);
  change.type = "button";
  change.dataset.action = "change";
  row.append(field, change);
  const note = node("p", "note");
  note.hidden = true;
  column.append(node("div", "field-label", labels.importDestination), row);
  return { column, note, change };
}

/** The Space column: free space after the upload, when known, and the staging limit. */
function spaceColumn(ctx: ReviewContext): HTMLElement {
  const { labels, plan, locale } = ctx;
  const column = node("div", "space");
  column.append(node("div", "field-label", labels.importSpace));
  if (plan.free_bytes_after !== null) {
    const free = formatSize(plan.free_bytes_after, labels, locale);
    column.append(node("p", "free", formatLabel(labels.importAfterUpload, { free })));
  }
  const limit = formatSize(plan.staging_limit_bytes, labels, locale);
  column.append(node("p", "staging", formatLabel(labels.importStagingLimit, { limit })));
  return column;
}

/** The integrity note: copied and checked, only read, and the file record. */
function integrityNote(labels: LabelTable): HTMLElement {
  const box = node("div", "integrity");
  box.append(
    icon("shield", 20),
    node("p", "title", labels.importIntegrityTitle),
    node("p", "", labels.importIntegrityDetail),
  );
  return box;
}

/**
 * Build the review (mockup 18): "Upload {count} files" with the volume, the four count cards, the
 * Files table (filled by {@link fillPlanItems} once the planned files are read), the required
 * destination with Change…, the Space lines, the integrity note, Cancel and the start control.
 * The job starts only on the start control; Cancel discards the plan.
 *
 * @param ctx Labels, the plan, the chosen destination's trail and the locale.
 * @returns The review's dialog and the parts the dialog element wires.
 */
export function reviewStep(ctx: ReviewContext): ReviewStep {
  const { labels, plan } = ctx;
  const volume =
    plan.source_volume === undefined
      ? undefined
      : formatLabel(labels.importReviewVolume, { volume: plan.source_volume });
  const title = formatLabel(labels.importReviewTitle, {
    count: plan.files_found.toLocaleString(ctx.locale),
  });
  const { dialog, close } = frame("review", labels, title, volume, "volume");
  const body = node("div", "body");
  const list = node("ul", "files");
  list.setAttribute("aria-label", labels.importFiles);
  list.setAttribute("aria-busy", "true");
  const destination = destinationColumn(ctx);
  const two = node("div", "two");
  two.append(destination.column, spaceColumn(ctx));
  body.append(
    countCards(ctx),
    node("p", "field-label files-head", labels.importFiles),
    list,
    two,
    destination.note,
    integrityNote(labels),
  );
  const foot = node("div", "foot");
  const cancel = actionButton("cancel", labels.fileCancel);
  const count = plan.files_new.toLocaleString(ctx.locale);
  const start = actionButton("start", formatLabel(labels.importStart, { count }), "primary");
  start.disabled = plan.files_new === 0;
  const reason = node("p", "reason", plan.files_new === 0 ? labels.importNothingNew : "");
  foot.append(reason, cancel, start);
  dialog.append(body, foot);
  return {
    dialog,
    body,
    list,
    note: destination.note,
    start,
    cancel,
    change: destination.change,
    close,
  };
}

/** One row of the Files table. */
function planRow(item: PlanItem, labels: LabelTable, locale: string): HTMLElement {
  const row = node("li");
  row.dataset.present = String(item.present);
  const mark = node("span", "mark");
  if (!item.present) mark.append(icon("check", 12));
  const extension = badgeExtensionOf(item.name);
  const badge = node(
    "span",
    `badge ${badgeClass(extension)}`,
    extension === "" ? "FILE" : extension,
  );
  const status = item.present ? labels.importItemSkipped : labels.importItemNew;
  row.append(
    mark,
    badge,
    node("span", "name", item.name),
    node("span", "size", formatSize(item.bytes, labels, locale)),
    node("span", "status", status),
  );
  return row;
}

/** The name the numbering note gives as its example: a numbered file, else a new one. */
function exampleName(items: readonly PlanItem[]): string | undefined {
  const pick = items.find((item) => item.numbered) ?? items.find((item) => !item.present);
  return pick === undefined ? undefined : numberedName(pick.name, 1);
}

/**
 * Fill the Files table with the planned files and show the numbering note and, when the plan
 * holds more files than were read, how many are shown. Side effect: changes the DOM.
 *
 * @param step The review.
 * @param items The planned files read, in the plan's order.
 * @param ctx Labels, the plan, the trail and the locale.
 */
export function fillPlanItems(
  step: ReviewStep,
  items: readonly PlanItem[],
  ctx: ReviewContext,
): void {
  const { labels, plan, locale } = ctx;
  step.list.replaceChildren(...items.map((item) => planRow(item, labels, locale)));
  step.list.removeAttribute("aria-busy");
  // The plan counts numbered names across every file; the note needs one name to show, from the
  // files read, and is still shown when the numbered file was beyond the pages read.
  const fallback = plan.names_numbered > 0 && items[0] ? numberedName(items[0].name, 1) : undefined;
  const example = exampleName(items) ?? fallback;
  const folder = ctx.trail.at(-1) ?? "";
  step.note.hidden = example === undefined;
  if (example !== undefined) {
    step.note.textContent = formatLabel(labels.importNumberedNote, { folder, example });
  }
  const cut = plan.files_found > items.length;
  const existing = step.body.querySelector(".files-note");
  existing?.remove();
  if (!cut) return;
  const shown = items.length.toLocaleString(locale);
  const total = plan.files_found.toLocaleString(locale);
  step.list.after(node("p", "files-note", formatLabel(labels.importFilesCut, { shown, total })));
}
