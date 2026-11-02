/**
 * The DOM of the Jobs panel's blocks (UI-1.2, UI-S-028, mockup 19): one block per upload job with
 * its title, state chip, detail line, progress bar, controls and files. Each builder turns a
 * {@link JobEntry} and the label table into elements and leaves what a button does to the panel,
 * which hands the callbacks in. Only the controls the job accepts are drawn (API-S-037), a job
 * waiting for the owner after a failed file (DE-1.8) gets no bar and no motion, and no source
 * path appears: a file shows its display name and its destination folder.
 */
import { badgeClass, badgeExtensionOf } from "../drive/model";
import { fileRefusal } from "../imports/failure";
import type { JobEntry } from "../imports/jobs-store";
import {
  SHOWN_JOB_FILES,
  allowedControls,
  itemPercent,
  itemStatus,
  jobPhase,
  shownItems,
  type DestinationDirectory,
  type JobControls,
  type JobItem,
  type JobPhase,
} from "../imports/model";
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";
import { icon, type IconName } from "./icons";
import { actionButton, node } from "./library-parts";
import { refusalCard } from "./refusal-card";

/** What a block needs to draw and to act. */
export interface JobViewContext {
  readonly labels: LabelTable;
  readonly directory: DestinationDirectory;
  readonly locale: string;
  /** Runs a control of a job; the panel words a refusal. */
  readonly act: (action: JobAction) => void;
}

/** A control the owner chose. */
export type JobAction =
  | { readonly kind: "pause" | "resume" | "cancel" | "retry-failed"; readonly job: string }
  | { readonly kind: "retry"; readonly job: string; readonly item: string };

/** The chip text and class of each phase. */
const CHIPS: Readonly<Record<JobPhase, { key: LabelKey; className: string }>> = {
  running: { key: "jobsStateRunning", className: "chip ok" },
  paused: { key: "jobsStatePaused", className: "chip muted" },
  pausing: { key: "jobsStatePausing", className: "chip muted" },
  cancelling: { key: "jobsStateCancelling", className: "chip muted" },
  waiting: { key: "jobsStateWaiting", className: "chip muted" },
  attention: { key: "jobsStateAttention", className: "chip alert" },
  failed: { key: "jobsStateFailed", className: "chip alert" },
  cancelled: { key: "jobsStateCancelled", className: "chip muted" },
  completed: { key: "jobsStateCompleted", className: "chip muted" },
};

/** A folder path as the panel shows it: its segments joined by an arrow. */
function folderText(folder: string): string {
  return folder.split("/").join(" › ");
}

/**
 * The title of a job: "Upload into {collection} › {folder}" from the job's destination.
 *
 * @param entry The job.
 * @param ctx Labels and the directory that names collections.
 * @returns The title; a collection the directory does not know is called by the vocabulary's
 *   word for it.
 */
export function jobTitle(entry: JobEntry, ctx: JobViewContext): string {
  const { labels } = ctx;
  const destination = entry.job.destination;
  if (destination === undefined) return labels.jobsUploadIntoUnknown;
  const collection = ctx.directory.collectionName(destination.collection_id) ?? labels.collection;
  if (destination.folder === undefined) {
    return formatLabel(labels.jobsUploadIntoCollection, { collection });
  }
  return formatLabel(labels.jobsUploadInto, { collection, folder: folderText(destination.folder) });
}

/** A clock time of an RFC 3339 instant in the viewer's zone, for "started 11:02". */
function clockTime(iso: string, locale: string): string {
  const time = new Date(iso);
  if (Number.isNaN(time.getTime())) return iso;
  return time.toLocaleTimeString(locale, { hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
}

/** The detail line of a job, by phase (mockup 19). */
function detailText(entry: JobEntry, phase: JobPhase, ctx: JobViewContext): string {
  const { labels, locale } = ctx;
  const { job } = entry;
  const done = job.done.toLocaleString(locale);
  const total = (job.total ?? job.done).toLocaleString(locale);
  const time = clockTime(job.created, locale);
  if (phase === "paused") return formatLabel(labels.jobsDetailPaused, { done, total });
  if (phase === "cancelled") return formatLabel(labels.jobsDetailCancelled, { done, total });
  if (phase === "completed") return formatLabel(labels.jobsDetailCompleted, { done, total, time });
  if (phase === "failed" || phase === "attention") return failedText(entry, { done, total }, ctx);
  if (job.total === undefined) return formatLabel(labels.jobsDetailNoTotal, { done, time });
  return formatLabel(labels.jobsDetail, { done, total, time });
}

/** The detail of a job with failed files: how many and the first one's code. */
function failedText(
  entry: JobEntry,
  counts: { done: string; total: string },
  ctx: JobViewContext,
): string {
  const { labels } = ctx;
  const code = entry.summary.failureCode ?? entry.job.error_code;
  // A job that failed as a whole, or whose failed files were not read, states no file count.
  if (entry.summary.failed === 0) {
    if (code === undefined) return formatLabel(labels.jobsDetailJobFailed, counts);
    return formatLabel(labels.jobsDetailJobFailedCode, { ...counts, error_code: code });
  }
  const n = String(entry.summary.failed);
  if (code === undefined) return formatLabel(labels.jobsDetailFailedPlain, { ...counts, n });
  return formatLabel(labels.jobsDetailFailed, { ...counts, n, error_code: code });
}

/** One control button, with its icon. */
function control(
  ctx: JobViewContext,
  text: string,
  glyph: IconName | undefined,
  action: JobAction,
): HTMLButtonElement {
  const button = actionButton(action.kind, text);
  if (glyph) button.prepend(icon(glyph));
  button.addEventListener("click", () => ctx.act(action));
  return button;
}

/** The buttons the job accepts, in the mockup's order: Pause or Resume, Cancel, Retry failed. */
function controlButtons(
  entry: JobEntry,
  controls: JobControls,
  phase: JobPhase,
  ctx: JobViewContext,
): HTMLButtonElement[] {
  const { labels } = ctx;
  const job = entry.job.id;
  const buttons: HTMLButtonElement[] = [];
  if (controls.pause) buttons.push(control(ctx, labels.jobsPause, "pause", { kind: "pause", job }));
  if (controls.resume)
    buttons.push(control(ctx, labels.jobsResume, "play", { kind: "resume", job }));
  // While the job still runs, one failed file has its own Retry (mockup 19); Retry failed is for
  // several, and for a job waiting for the owner (DE-1.8).
  const bulk = phase === "attention" || entry.summary.failed > 1;
  if (controls.retry && bulk) {
    buttons.push(control(ctx, labels.jobsRetryFailed, "retry", { kind: "retry-failed", job }));
  }
  if (controls.cancel)
    buttons.push(control(ctx, labels.jobsCancel, undefined, { kind: "cancel", job }));
  return buttons;
}

/** The 4 px progress bar of a running job. */
function progressBar(entry: JobEntry, labels: LabelTable): HTMLElement {
  const { job } = entry;
  const total = job.total ?? 0;
  const percent = total > 0 ? Math.min(100, (job.done / total) * 100) : 0;
  const bar = node("div", "bar");
  bar.setAttribute("role", "progressbar");
  bar.setAttribute("aria-label", labels.jobsTitle);
  // Without a known total the bar is indeterminate: it states no range and no value.
  if (total > 0) {
    bar.setAttribute("aria-valuemin", "0");
    bar.setAttribute("aria-valuemax", String(total));
    bar.setAttribute("aria-valuenow", String(job.done));
  }
  const fill = node("div", "fill");
  fill.style.inlineSize = `${percent.toFixed(2)}%`;
  bar.append(fill);
  return bar;
}

/** The files a job lists: failed ones only for a job that is waiting or failed, else the busiest. */
function listedFiles(entry: JobEntry, phase: JobPhase): JobItem[] {
  const failedOnly = phase === "attention" || phase === "failed";
  const pool = failedOnly ? entry.items.filter((item) => item.state === "failed") : entry.items;
  if (phase === "paused" || phase === "completed" || phase === "cancelled") return [];
  return shownItems(pool, SHOWN_JOB_FILES);
}

/** One file row: badge, name, destination, status, a bar while copying, Retry, and its card. */
function fileRow(
  item: JobItem,
  entry: JobEntry,
  controls: JobControls,
  ctx: JobViewContext,
): HTMLElement {
  const { labels } = ctx;
  const row = node("li", "job-file");
  row.dataset.state = item.state;
  const extension = badgeExtensionOf(item.name);
  const badge = node(
    "span",
    `badge ${badgeClass(extension)}`,
    extension === "" ? "FILE" : extension,
  );
  const folder = item.folder ?? entry.job.destination?.folder;
  const where =
    folder === undefined ? "" : formatLabel(labels.jobsFolderArrow, { folder: folderText(folder) });
  row.append(
    badge,
    node("span", "name", item.name),
    node("span", "where", where),
    node("span", "state", itemStatus(item, labels)),
  );
  if (item.state === "copying") row.append(copyBar(item));
  if (item.state === "failed" && controls.retry) {
    const retry = control(ctx, labels.jobsRetry, "retry", {
      kind: "retry",
      job: entry.job.id,
      item: item.item_id,
    });
    retry.classList.add("retry");
    retry.dataset.item = item.item_id;
    row.append(retry);
  }
  const refusal = fileRefusal(item, labels);
  if (refusal) row.append(refusalCard(labels, refusal.title, refusal.reason));
  return row;
}

/** The 3 px bar under a copying file's name. */
function copyBar(item: JobItem): HTMLElement {
  const bar = node("div", "bar small");
  bar.setAttribute("role", "progressbar");
  bar.setAttribute("aria-valuemin", "0");
  bar.setAttribute("aria-valuemax", "100");
  bar.setAttribute("aria-valuenow", String(itemPercent(item)));
  const fill = node("div", "fill");
  fill.style.inlineSize = `${itemPercent(item)}%`;
  bar.append(fill);
  return bar;
}

/** The files section: the eyebrow with the count shown and the rows. */
function filesSection(
  entry: JobEntry,
  files: readonly JobItem[],
  controls: JobControls,
  ctx: JobViewContext,
): HTMLElement[] {
  const { labels, locale } = ctx;
  const total = (entry.job.total ?? entry.items.length).toLocaleString(locale);
  const shown = files.length.toLocaleString(locale);
  const eyebrow = node("p", "eyebrow", formatLabel(labels.jobsFilesShown, { shown, total }));
  const list = node("ul", "job-files");
  list.append(...files.map((item) => fileRow(item, entry, controls, ctx)));
  return [eyebrow, list];
}

/**
 * Build the block of one job (mockup 19).
 *
 * @param entry The job, its files and what they add up to.
 * @param ctx Labels, the directory, the locale and the callback for a control.
 * @returns The block: title, chip and detail, a bar while it runs, the controls it accepts and
 *   its files.
 */
export function jobBlock(entry: JobEntry, ctx: JobViewContext): HTMLElement {
  const phase = jobPhase(entry.job, entry.summary);
  const controls = allowedControls(entry.job, entry.summary);
  const files = listedFiles(entry, phase);
  const block = node("section", "job");
  block.dataset.jobId = entry.job.id;
  block.dataset.phase = phase;
  const chip = CHIPS[phase];
  const line = node("div", "job-line");
  line.append(
    node("span", chip.className, ctx.labels[chip.key]),
    node("span", "detail", detailText(entry, phase, ctx)),
  );
  const buttons = node("div", "actions");
  buttons.append(...controlButtons(entry, controls, phase, ctx));
  const top = node("div", "job-top");
  top.append(node("p", "job-title", jobTitle(entry, ctx)));
  const below = files.length > 0;
  if (!below && buttons.children.length > 0) top.append(buttons);
  block.append(top, line);
  if (phase === "running" || phase === "pausing") block.append(progressBar(entry, ctx.labels));
  if (below && buttons.children.length > 0) block.append(buttons);
  if (below) block.append(...filesSection(entry, files, controls, ctx));
  return block;
}
