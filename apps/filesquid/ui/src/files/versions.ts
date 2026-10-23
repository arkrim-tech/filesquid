/**
 * The model behind the Versions view (UI-F2.3, UI-S-009, UI-S-031 mockups 26 and 27): rows
 * newest first with the current version marked, the label of the version a restore publishes,
 * the versions it keeps, relative times, the count and keep wording from the label table, and
 * the shortened forms of digests and identifiers the file panel shows. Pure: nothing here
 * fetches or touches the DOM. Every list is bounded (AGENTS.md section 22).
 */
import { formatLabel, type LabelTable } from "../theme/labels";
import type { FileHistory } from "./history-source";
import { formatModified } from "./model";

/** Most versions the view lists: the newest are kept. */
export const MAX_VERSION_ROWS = 10_000;

/** One row of the Versions view. */
export interface VersionRow {
  /** The version: `v1`, `v2`, ... */
  readonly version: string;
  /** Its label, or null for none (a blank label counts as none). */
  readonly label: string | null;
  /** When it was created, RFC 3339 as recorded. */
  readonly created: string;
  /** The file's size after it, or null when it removed the file. */
  readonly size: number | null;
  /** True when this version removed the file. */
  readonly removed: boolean;
  /** True for the newest version, when it holds the file: the content shown now. */
  readonly current: boolean;
}

/**
 * The rows of a file's history, newest first; the newest is current unless it removed the
 * file. Pure.
 *
 * @param history The API's history, oldest first.
 * @returns At most {@link MAX_VERSION_ROWS} rows, the newest.
 */
export function versionRows(history: FileHistory): VersionRow[] {
  const newest = history.entries.slice(-MAX_VERSION_ROWS).reverse();
  return newest.map((entry, index) => {
    const removed = entry.change === "deleted";
    const label = entry.version.label?.trim() ? entry.version.label : null;
    return {
      version: entry.version.version,
      label,
      created: entry.version.created,
      size: removed ? null : (entry.size ?? null),
      removed,
      current: index === 0 && !removed,
    };
  });
}

/**
 * The number of a `v<n>` label, or undefined. OCFL puts no cap on version numbers, so it is a
 * `bigint`: exact beyond `Number.MAX_SAFE_INTEGER`.
 */
function versionNumber(version: string): bigint | undefined {
  const match = /^v([1-9][0-9]*)$/.exec(version);
  return match ? BigInt(match[1] as string) : undefined;
}

/**
 * The number of versions that changed the file, all of them, though {@link versionRows} lists
 * at most {@link MAX_VERSION_ROWS}. Pure.
 *
 * @param history The API's history.
 * @returns The count of its entries.
 */
export function versionTotal(history: FileHistory): number {
  return history.entries.length;
}

/**
 * The version after `head`, the one a restore publishes. Pure.
 *
 * @param head The object's head, `v<n>`.
 * @returns `v<n+1>`, or undefined for a label that is not `v<n>`.
 */
export function nextVersion(head: string): string | undefined {
  const number = versionNumber(head);
  return number === undefined ? undefined : `v${number + 1n}`;
}

/**
 * The file's versions newer than `version`, oldest first: the ones a restore of `version`
 * keeps in the history. Pure.
 *
 * @param rows The rows, newest first.
 * @param version The version restored.
 * @returns Their labels.
 */
export function laterVersions(rows: readonly VersionRow[], version: string): string[] {
  const from = versionNumber(version);
  if (from === undefined) return [];
  return rows
    .filter((row) => (versionNumber(row.version) ?? 0n) > from)
    .map((row) => row.version)
    .reverse();
}

/**
 * The note a restore dialog shows about the versions it keeps. Pure.
 *
 * @param later The versions newer than the one restored, oldest first.
 * @param labels The active label table.
 * @returns The note, or undefined when no newer version exists.
 */
export function restoreKeepText(later: readonly string[], labels: LabelTable): string | undefined {
  const first = later[0];
  const last = later[later.length - 1];
  if (first === undefined || last === undefined) return undefined;
  if (later.length === 1) return formatLabel(labels.versionsRestoreKeepOne, { version: first });
  if (later.length === 2) {
    return formatLabel(labels.versionsRestoreKeepTwo, { first, second: last });
  }
  return formatLabel(labels.versionsRestoreKeepRange, { first, last });
}

/**
 * The count of versions, in the singular for one. Pure.
 *
 * @param count The number of versions.
 * @param labels The active label table.
 * @param locale BCP 47 tag for the number format.
 * @returns For example `7 versions`.
 */
export function versionCountText(count: number, labels: LabelTable, locale?: string): string {
  const template = count === 1 ? labels.versionsCountOne : labels.versionsCount;
  return formatLabel(template, { count: new Intl.NumberFormat(locale).format(count) });
}

/** A text with its first character in upper case for the locale. */
function capitalise(text: string, locale?: string): string {
  const first = text.charAt(0);
  return first.toLocaleUpperCase(locale) + text.slice(1);
}

/** Milliseconds per minute, hour and day. */
const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/**
 * A version's time relative to `now` (mockup 26): "Now" within a minute, minutes and hours
 * within a day, "Yesterday" within two days, then the short date. A time in the future reads
 * as now. Pure.
 *
 * @param iso An RFC 3339 time.
 * @param now The present.
 * @param locale BCP 47 tag.
 * @returns The text, or `iso` unchanged when it is not a time.
 */
export function relativeTime(iso: string, now: Date, locale?: string): string {
  const time = new Date(iso);
  if (Number.isNaN(time.getTime())) return iso;
  const elapsed = Math.max(0, now.getTime() - time.getTime());
  const words = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  if (elapsed < MINUTE) return capitalise(words.format(0, "second"), locale);
  if (elapsed < HOUR) return words.format(-Math.floor(elapsed / MINUTE), "minute");
  if (elapsed < DAY) return words.format(-Math.floor(elapsed / HOUR), "hour");
  if (elapsed < 2 * DAY) return capitalise(words.format(-1, "day"), locale);
  return formatModified(iso, locale);
}

/** Characters kept at each end of a shortened digest. */
const DIGEST_END = 6;

/**
 * A long digest as its first and last six characters around an ellipsis (`9b1f4e…c07a2d`). Pure.
 *
 * @param digest A hex digest.
 * @returns The short form, or the digest when it is short already.
 */
export function shortDigest(digest: string): string {
  if (digest.length <= DIGEST_END * 2 + 1) return digest;
  return `${digest.slice(0, DIGEST_END)}…${digest.slice(-DIGEST_END)}`;
}

/** A `urn:uuid:` identifier: its first group and the rest. */
const URN_UUID = /^(urn:uuid:[0-9a-f]{8})-[0-9a-f-]+$/i;
/** Longest identifier shown whole. */
const MAX_WHOLE_IDENTIFIER = 24;

/**
 * An object identifier as the file panel shows it: a `urn:uuid:` as its first group and last
 * six characters (`urn:uuid:7f3c9a1e-…-41d2b8`), any other long identifier as its first 14 and
 * last 6 characters. Pure.
 *
 * @param identifier The identifier.
 * @returns The short form, or the identifier when it is short already.
 */
export function shortIdentifier(identifier: string): string {
  const urn = URN_UUID.exec(identifier);
  if (urn) return `${urn[1]}-…-${identifier.slice(-DIGEST_END)}`;
  if (identifier.length <= MAX_WHOLE_IDENTIFIER) return identifier;
  return `${identifier.slice(0, 14)}…${identifier.slice(-DIGEST_END)}`;
}
