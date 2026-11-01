/**
 * The model of the upload flow (UI-1.2, UI-S-028, UI-S-029, API-S-036, API-S-037): the
 * contract's types for a plan and a job, the two ports the elements ask (an {@link ImportSource}
 * for the plan, review, start and discard of an upload, and a {@link JobSource} for the durable
 * jobs and their files), the destination the owner's choice names, and the pure rules over a
 * job: what its files add up to, its phase, the controls it accepts and the files the Jobs
 * panel lists. Nothing here fetches or touches the DOM; a source answers, and the elements
 * never change when the source does (a live one over `FileSquidClient`, a memory one in tests).
 *
 * Every list and page loop is bounded (AGENTS.md section 22) by the `MAX_*` constants.
 */
import type { components } from "../api/schema";
import { formatLabel, type LabelTable } from "../theme/labels";

/** What the owner picks in the host's dialog: `files` or `folder` (API-S-036). */
export type ImportSourceKind = components["schemas"]["ImportSource"];
/** The plan the owner reviews before any byte is staged (API-S-036 **Review**). */
export type ImportPlan = components["schemas"]["ImportPlan"];
/** One planned file: display name (base name only), size, whether it is new and numbered. */
export type PlanItem = components["schemas"]["PlanItem"];
/** One page of planned files. */
export type PlanItemPage = components["schemas"]["PlanItemPage"];
/** One durable job as the job routes show it. */
export type JobView = components["schemas"]["JobView"];
/** The state of a job. */
export type JobState = components["schemas"]["JobState"];
/** One file of an upload job. */
export type JobItem = components["schemas"]["JobItem"];
/** The state of one file of an upload job. */
export type JobItemState = components["schemas"]["JobItemState"];
/** One page of an upload job's files. */
export type JobItemPage = components["schemas"]["JobItemPage"];
/** One listing of jobs. */
export type JobPage = components["schemas"]["JobPage"];

/** Most pages of a plan's files the review reads (1,000 per page): 5,000 files. */
export const MAX_PLAN_ITEM_PAGES = 5;
/** Most pages of a job's files the Jobs panel reads: with the page size, 1,000 files. */
export const MAX_JOB_ITEM_PAGES = 5;
/** Page size of a job's files, 1 to 1,000 (API-S-037). */
export const JOB_ITEM_PAGE_SIZE = 200;
/** Most files of one job the Jobs panel lists at once. */
export const SHOWN_JOB_FILES = 8;
/** Longest wait between two reads of one job's files while it changes. */
export const ITEM_REFRESH_MS = 1000;
/** The cap of numbered names beside a taken one (API-S-011 **Same logical path**). */
export const MAX_NAME_NUMBER = 9999;
/** The kind of job an upload is (API-S-037). */
export const IMPORT_JOB_KIND = "import";

/** What an upload request needs: the open library, the destination and the source. */
export interface PlanRequest {
  /** The open library to import into. */
  readonly libraryId: string;
  /** The destination collection (required, ADR 041). */
  readonly collectionId: string;
  /** The folder inside the collection, as a collection logical path; the root when absent. */
  readonly folder?: string;
  /** Files or a folder. */
  readonly source: ImportSourceKind;
  /** The operation ID to reuse when retrying a request whose answer was lost. */
  readonly operationId?: string;
}

/** What became of a plan request: reviewed, started at once (a host without the review) or closed. */
export type PlanAnswer =
  | { readonly kind: "planned"; readonly plan: ImportPlan }
  | { readonly kind: "started"; readonly jobId: string }
  | { readonly kind: "cancelled" };

/** The plan, review, start and discard of an upload (API-S-036). */
export interface ImportSource {
  /**
   * Show the host's file dialog and plan what the owner picks; nothing is staged.
   *
   * @param request The library, destination and source.
   * @returns The plan, the job of a host without the review, or that the owner closed the dialog.
   */
  plan(request: PlanRequest): Promise<PlanAnswer>;
  /**
   * Read one page of a plan's files.
   *
   * @param planId The plan.
   * @param cursor The previous page's `next_cursor`.
   * @param limit Page size, 1 to 1,000.
   */
  planItems(planId: string, cursor?: string | null, limit?: number): Promise<PlanItemPage>;
  /**
   * Start the job of a plan.
   *
   * @param planId The plan.
   * @returns The job's ID.
   */
  start(planId: string): Promise<string>;
  /**
   * Discard a plan; nothing changes.
   *
   * @param planId The plan.
   */
  discard(planId: string): Promise<void>;
}

/** What the job source tells a follower. */
export type JobChange =
  { readonly kind: "job"; readonly job: JobView } | { readonly kind: "resync" };

/** The durable jobs and their files (API-S-037). */
export interface JobSource {
  /** List the upload jobs, newest first, at most 100. */
  list(): Promise<JobPage>;
  /** Read one page of a job's files. */
  items(job: string, cursor?: string | null, limit?: number): Promise<JobItemPage>;
  /** Hold the job; a running job suspends at its next checkpoint. */
  pause(job: string): Promise<JobView>;
  /** Clear a pause. */
  resume(job: string): Promise<JobView>;
  /** Stop the job; committed files stay. */
  cancel(job: string): Promise<JobView>;
  /** Put one failed file back to queued. */
  retryItem(job: string, item: string): Promise<JobView>;
  /** Put every failed file back to queued. */
  retryFailed(job: string): Promise<JobView>;
  /**
   * Follow changes to jobs.
   *
   * @param listener Called with each change.
   * @returns A function that stops following.
   */
  subscribe(listener: (change: JobChange) => void): () => void;
}

/** The library and collection a picker choice names, as the contract wants them. */
export interface ResolvedDestination {
  /** The library the collection is in. */
  readonly libraryId: string;
  /** The collection's ID. */
  readonly collectionId: string;
}

/**
 * Names the destination a choice stands for. Navigation node handles are opaque (API-S-021) and
 * no route yet maps one to a collection ID, so this is a port: a host that can answer supplies a
 * lookup, and the shipped one answers none until the API does.
 */
export interface DestinationDirectory {
  /**
   * The library and collection ID of a chosen collection or folder.
   *
   * @param choice The picker's node handle for it and the names from the library down to it.
   * @returns The IDs, or undefined when this host cannot name them.
   */
  resolve(choice: {
    readonly node: string;
    readonly trail: readonly string[];
  }): Promise<ResolvedDestination | undefined>;
  /** The name of a collection this directory resolved before, for the Jobs panel's titles. */
  collectionName(collectionId: string): string | undefined;
}

/** What a job's files add up to. */
export interface ItemSummary {
  readonly queued: number;
  readonly copying: number;
  readonly copied: number;
  readonly skipped: number;
  readonly failed: number;
  /** The first failed file's error code, when it has one. */
  readonly failureCode?: string;
}

/** What a job is doing, for its chip. */
export type JobPhase =
  | "running"
  | "paused"
  | "pausing"
  | "cancelling"
  | "waiting"
  | "attention"
  | "failed"
  | "cancelled"
  | "completed";

/** The controls a job accepts now (API-S-037). */
export interface JobControls {
  readonly pause: boolean;
  readonly resume: boolean;
  readonly cancel: boolean;
  readonly retry: boolean;
}

/**
 * The folders below the collection of a picker trail, as a collection logical path. Pure.
 *
 * @param trail The names from the library down: library, collection, then folders.
 * @returns The `/`-joined folder names, or undefined for a collection's root.
 */
export function destinationFolder(trail: readonly string[]): string | undefined {
  const folders = trail.slice(2);
  return folders.length === 0 ? undefined : folders.join("/");
}

/**
 * The destination as the dialogs word it: the collection and the folders, joined by an arrow. Pure.
 *
 * @param trail The names from the library down.
 * @returns For example `Harbour project › Site photos`.
 */
export function destinationText(trail: readonly string[]): string {
  return trail.slice(1).join(" › ");
}

/**
 * The name an upload gets beside a taken one: the number goes at the end of the name's first
 * part, before its first dot, and a leading dot belongs to that part (API-S-011 **Same logical
 * path**, `photo-1.jpg`, `archive-1.tar.gz`, `report-1`, `.env-1`). Pure.
 *
 * @param name The display name.
 * @param n The number, 1 to {@link MAX_NAME_NUMBER}.
 * @returns The numbered name.
 */
export function numberedName(name: string, n: number): string {
  const dot = name.indexOf(".", 1);
  return dot < 0 ? `${name}-${n}` : `${name.slice(0, dot)}-${n}${name.slice(dot)}`;
}

/**
 * The first and last numbered name a folder can hold for a file (mockup 48). Pure.
 *
 * @param name The display name.
 * @returns `photo-1.jpg` and `photo-9999.jpg` for `photo.jpg`.
 */
export function pathSuffixRange(name: string): { first: string; last: string } {
  return { first: numberedName(name, 1), last: numberedName(name, MAX_NAME_NUMBER) };
}

/**
 * Add a job's files up. Pure.
 *
 * @param items The files read so far.
 * @returns A count per state and the first failure's code.
 */
export function summariseItems(items: readonly JobItem[]): ItemSummary {
  const sum = { queued: 0, copying: 0, copied: 0, skipped: 0, failed: 0 };
  let failureCode: string | undefined;
  let firstFailure = true;
  for (const item of items) {
    sum[item.state] += 1;
    // The code is the first failed file's own: a later file's code is never borrowed for it.
    if (item.state !== "failed" || !firstFailure) continue;
    failureCode = item.error_code;
    firstFailure = false;
  }
  return failureCode === undefined ? sum : { ...sum, failureCode };
}

/** True for a job that has ended. */
function finished(job: JobView): boolean {
  return job.state === "completed" || job.state === "failed" || job.state === "cancelled";
}

/**
 * What a job is doing. A pending job whose files all ended and at least one failed is waiting
 * for the owner (DE-1.8): API-S-037 refuses to retry a finished job, so the host leaves it
 * pending with its failed files retryable, and the panel shows no running state for it. Pure.
 *
 * @param job The job view.
 * @param files What its files add up to.
 * @returns The phase.
 */
export function jobPhase(job: JobView, files: ItemSummary): JobPhase {
  if (finished(job)) return job.state as "failed" | "cancelled" | "completed";
  if (job.control === "cancel") return "cancelling";
  if (job.paused) return "paused";
  if (job.control === "pause") return "pausing";
  if (job.state === "running") return "running";
  const idle = files.queued === 0 && files.copying === 0;
  return (files.failed > 0 || checkpointEnded(job)) && idle ? "attention" : "waiting";
}

/**
 * Whether a pending job's checkpoint has reached its end. DE-1.8 leaves a job `pending` only
 * when files ended and some failed (otherwise it completes), so this holds even when the files
 * read are a bounded part of a large job and show no failure.
 */
function checkpointEnded(job: JobView): boolean {
  return (
    job.state === "pending" && job.total !== undefined && job.total > 0 && job.done >= job.total
  );
}

/**
 * The controls a job accepts now (API-S-037): a finished job and one under a cancel request
 * accept none; a pause already asked leaves only Cancel; a paused job accepts Resume and Cancel
 * and refuses a retry until it is resumed; a job waiting for the owner after a failed file
 * (DE-1.8) accepts a retry and Cancel; otherwise Pause and Cancel, and a retry while a file has
 * failed. Pure.
 *
 * @param job The job view.
 * @param files What its files add up to.
 * @returns Which buttons to offer.
 */
export function allowedControls(job: JobView, files: ItemSummary): JobControls {
  const none = { pause: false, resume: false, cancel: false, retry: false };
  if (finished(job) || job.control === "cancel") return none;
  if (job.paused) return { ...none, resume: true, cancel: true };
  if (job.control === "pause") return { ...none, cancel: true };
  // A pending job whose files all ended has nothing to hold: it waits for a retry or a cancel.
  const waiting = jobPhase(job, files) === "attention";
  return { pause: !waiting, resume: false, cancel: true, retry: files.failed > 0 || waiting };
}

/** The rank of a state in the list the panel shows: failures first, then what is moving. */
const SHOWN_RANK: Readonly<Record<JobItemState, number>> = {
  failed: 0,
  copying: 1,
  queued: 2,
  copied: 3,
  skipped: 3,
};

/**
 * The files the Jobs panel lists: at most `limit`, chosen by what the owner needs first (a failed
 * file that needs Retry, then the file copying, then those waiting, then the finished ones) and
 * shown in the job's own order. Pure.
 *
 * @param items The files read so far, in the job's order.
 * @param limit The most to show.
 * @returns The shown files.
 */
export function shownItems(items: readonly JobItem[], limit: number): JobItem[] {
  const ranked = items
    .map((item, index) => ({ item, index }))
    .sort((a, b) => SHOWN_RANK[a.item.state] - SHOWN_RANK[b.item.state] || a.index - b.index)
    .slice(0, limit);
  return ranked.sort((a, b) => a.index - b.index).map((entry) => entry.item);
}

/**
 * A copying file's progress as a whole percentage, rounded down, 0 for an empty file. Pure.
 *
 * @param item The file.
 * @returns 0 to 100.
 */
export function itemPercent(item: JobItem): number {
  if (item.bytes_total <= 0) return 0;
  return Math.min(100, Math.floor((item.bytes_done * 100) / item.bytes_total));
}

/**
 * One file's status as the Jobs panel words it (UI-S-028). Pure.
 *
 * @param item The file.
 * @param labels The active label table.
 * @returns Copied, "Copying · {percent}%", Queued, "Already uploaded · skipped", or
 *   "Failed · {error_code}".
 */
export function itemStatus(item: JobItem, labels: LabelTable): string {
  switch (item.state) {
    case "copied":
      return labels.jobsItemCopied;
    case "copying":
      return formatLabel(labels.jobsItemCopying, { percent: String(itemPercent(item)) });
    case "queued":
      return labels.jobsItemQueued;
    case "skipped":
      return labels.importItemSkipped;
    case "failed":
      return item.error_code === undefined
        ? labels.jobsItemFailedPlain
        : formatLabel(labels.jobsItemFailed, { error_code: item.error_code });
  }
}
