/**
 * The words the library screens build from the host's facts and a job's view (UI-1.3,
 * UI-S-027, mockups 01, 04 and 41): when a library was opened, its free space, the rebuild's
 * progress line, the state chip and the progress fraction. Every word comes from the label
 * table; a figure the host did not give (an unreadable volume, an unknown total) is left out,
 * never invented. Pure.
 */
import { formatSize } from "../files/model";
import { formatLabel, type LabelTable } from "../theme/labels";
import type { JobView, RebuildReport, RecentLibrary } from "./model";

/** How a status chip is drawn: on the active tint, in the secondary colour, or outlined. */
export type ChipTone = "ok" | "muted" | "alert";

/** A state chip: its word and its treatment. */
export interface Chip {
  /** The word from the label table. */
  readonly text: string;
  /** The treatment (UI-S-024: `ok` states on the active tint, errors outlined in `--fs-error`). */
  readonly tone: ChipTone;
}

/**
 * The locale to write dates, times and counts in. English is written in the day-month-year
 * order of the mockups ("20 Oct 2026"); any other tag is used as declared. Pure.
 *
 * @param lang The page's language tag, or blank.
 * @returns A BCP 47 tag.
 */
export function formatLocale(lang: string | undefined): string {
  const tag = (lang ?? "").trim();
  return tag === "" || tag === "en" ? "en-GB" : tag;
}

/** The page's language tag, read from the document. */
export function pageLocale(): string {
  return formatLocale(typeof document === "undefined" ? undefined : document.documentElement.lang);
}

/** True when two instants fall on the same calendar day in the viewer's zone. */
function sameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

/**
 * When a library was opened: the word for today, else the short date. Pure.
 *
 * @param iso The last-opened time, RFC 3339.
 * @param now The current time.
 * @param labels The active label table.
 * @param locale BCP 47 tag; the page's language by default.
 * @returns `today` or a date such as `20 Oct 2026`; the text as given when it is not a time.
 */
export function openedWhen(
  iso: string,
  now: Date,
  labels: LabelTable,
  locale: string = pageLocale(),
): string {
  const opened = new Date(iso);
  if (Number.isNaN(opened.getTime())) return iso;
  if (sameDay(opened, now)) return labels.libraryRecentToday;
  const format = new Intl.DateTimeFormat(locale, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
  return format.format(opened);
}

/**
 * The meta line of a recent library: "Opened {when} · {free} free on its disk", or without the
 * free figure when its volume cannot be read. Pure.
 *
 * @param library The recent library.
 * @param now The current time.
 * @param labels The active label table.
 * @param locale BCP 47 tag; the page's language by default.
 * @returns The line.
 */
export function recentMeta(
  library: RecentLibrary,
  now: Date,
  labels: LabelTable,
  locale: string = pageLocale(),
): string {
  const when = openedWhen(library.last_opened, now, labels, locale);
  if (library.free_bytes === null) return formatLabel(labels.libraryRecentMetaNoFree, { when });
  const free = formatSize(library.free_bytes, labels, locale);
  return formatLabel(labels.libraryRecentMeta, { when, free });
}

/**
 * The value of the Free space fact: "{free} on the disk that holds this library", or that it is
 * not known when the volume cannot be read. Pure.
 *
 * @param freeBytes The free bytes the host gave, or null.
 * @param labels The active label table.
 * @param locale BCP 47 tag; the page's language by default.
 * @returns The value.
 */
export function freeSpaceText(
  freeBytes: number | null,
  labels: LabelTable,
  locale: string = pageLocale(),
): string {
  if (freeBytes === null) return labels.libraryFactFreeUnknown;
  return formatLabel(labels.libraryFactFree, { free: formatSize(freeBytes, labels, locale) });
}

/**
 * The rebuild's progress line: "{done} of {total} objects read · started {time} · resumes from
 * its checkpoint after you quit", or the count alone while the total is not known. Pure.
 *
 * @param job The rebuild job.
 * @param labels The active label table.
 * @param locale BCP 47 tag; the page's language by default.
 * @returns The line.
 */
export function rebuildProgressText(
  job: JobView,
  labels: LabelTable,
  locale: string = pageLocale(),
): string {
  const number = new Intl.NumberFormat(locale);
  const clock = new Intl.DateTimeFormat(locale, {
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  });
  const values = {
    done: number.format(job.done),
    time: clock.format(new Date(job.created)),
  };
  if (job.total === undefined) return formatLabel(labels.libraryRebuildProgressNoTotal, values);
  return formatLabel(labels.libraryRebuildProgress, { ...values, total: number.format(job.total) });
}

/**
 * How far a job is, for the progress bar. Pure.
 *
 * @param job The job.
 * @returns Done over total held between 0 and 1; 1 for a finished job with nothing to do;
 *   undefined while the total is not known.
 */
export function progressFraction(job: JobView): number | undefined {
  if (job.total === undefined) return undefined;
  if (job.total === 0) return job.state === "completed" ? 1 : 0;
  return Math.min(1, Math.max(0, job.done / job.total));
}

/**
 * The state chip of a job. Pure.
 *
 * @param job The job.
 * @param labels The active label table.
 * @returns The word and its treatment: a requested control shows as pausing or cancelling, a
 *   paused job as paused, and the final states as finished, failed or cancelled.
 */
export function jobChip(job: JobView, labels: LabelTable): Chip {
  if (job.state === "completed") return { text: labels.jobStateCompleted, tone: "ok" };
  if (job.state === "failed") return { text: labels.jobStateFailed, tone: "alert" };
  if (job.state === "cancelled") return { text: labels.jobStateCancelled, tone: "muted" };
  if (job.control === "cancel") return { text: labels.jobStateCancelling, tone: "muted" };
  if (job.control === "pause") return { text: labels.jobStatePausing, tone: "muted" };
  if (job.paused) return { text: labels.jobStatePaused, tone: "muted" };
  if (job.state === "pending") return { text: labels.jobStatePending, tone: "muted" };
  return { text: labels.jobStateRunning, tone: "ok" };
}

/**
 * The sentence that reports a finished rebuild: the objects read, those not readable and those
 * with validation errors. Pure.
 *
 * @param report The job's result.
 * @param labels The active label table.
 * @returns The sentence.
 */
export function rebuildSummary(report: RebuildReport, labels: LabelTable): string {
  const invalid = report.objects.filter((object) => object.error_codes.length > 0).length;
  return formatLabel(labels.libraryRebuilt, {
    objects: String(report.objects.length),
    unreadable: String(report.unreadable.length),
    invalid: String(invalid),
  });
}
