/**
 * The model behind the Logs view (UI-F2.16; UI-S-021, UI-S-032, API-S-031): the time ranges, the
 * filter and the journal query it makes, the search text and what it names, the row view of an
 * entry (outcome, journal-only mark, target text, short operation ID), the words under the table
 * and in the footer, the rows of the entry detail, the target of View provenance, the choice of
 * the Download dialog and the query it makes. Nothing here fetches or touches the DOM.
 *
 * The journal carries opaque IDs only: a name appears solely where the route resolved it
 * (`display_name`, API-S-031), never from a lookup this module invents.
 */
import type { JournalEntry, JournalEntryView, JournalFilters, Level, Outcome } from "../api/client";
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";

/** The operation of every authorisation refusal; its outcome is `failed` (API-S-031). */
export const DENIED_OPERATION = "api.auth.denied";

/** Entries asked for per page. */
export const PAGE_SIZE = 50;

/** The most pages one list reads before the oldest entries are left to Load older. */
export const MAX_PAGES = 200;

/** The time ranges of the first chip. */
export type RangeId = "hour" | "day" | "week" | "month" | "all";

/** The ranges in menu order. */
export const RANGES: readonly RangeId[] = ["hour", "day", "week", "month", "all"];

/** The range the view opens on (mockup 38: "Last 24 hours"). */
export const DEFAULT_RANGE: RangeId = "day";

/** The outcomes a filter may name: the journal's, and `refused` for an authorisation refusal. */
export type OutcomeFilter = Outcome | "refused";

/** The outcomes in menu order. */
export const OUTCOME_FILTERS: readonly Exclude<OutcomeFilter, "info">[] = [
  "succeeded",
  "failed",
  "refused",
  "started",
  "stalled",
  "resumed",
  "cancelled",
];

/** The levels in menu order. */
export const LEVELS: readonly Level[] = ["info", "warn", "error"];

/** What the view filters the journal by. */
export interface LogsFilterState {
  /** The time range. */
  readonly range: RangeId;
  /** A library ID among the entry's targets. */
  readonly library?: string;
  /** An exact operation name. */
  readonly operation?: string;
  /** An outcome, or `refused`. */
  readonly outcome?: OutcomeFilter;
  /** A level. */
  readonly level?: Level;
  /** A file ID (a UUID). */
  readonly file?: string;
  /** An object ID. */
  readonly object?: string;
  /** An operation ID. */
  readonly operation_id?: string;
}

/** The filter the view opens with: the last 24 hours and nothing else. */
export const EMPTY_FILTER: LogsFilterState = { range: DEFAULT_RANGE };

/** A library as the Library chip lists it. */
export interface LogsLibrary {
  /** The library's ID. */
  readonly library_id: string;
  /** The name the libraries route gave it. */
  readonly name: string;
}

const HOUR_MS = 3_600_000;
const RANGE_MS: Readonly<Record<Exclude<RangeId, "all">, number>> = {
  hour: HOUR_MS,
  day: 24 * HOUR_MS,
  week: 7 * 24 * HOUR_MS,
  month: 30 * 24 * HOUR_MS,
};

/**
 * The start of a range as RFC 3339 UTC. Pure.
 *
 * @param range The range.
 * @param now The time the range ends at.
 * @returns The start, or undefined for all time.
 */
export function rangeFrom(range: RangeId, now: Date): string | undefined {
  if (range === "all") return undefined;
  return new Date(now.getTime() - RANGE_MS[range]).toISOString();
}

/** The `from`, `library`, `level`, `file`, `object` and `operation_id` members of a filter. */
function sharedMembers(filter: LogsFilterState, now: Date): JournalFilters {
  const query: JournalFilters = {};
  const from = rangeFrom(filter.range, now);
  if (from !== undefined) query.from = from;
  if (filter.library !== undefined) query.library = filter.library;
  if (filter.level !== undefined) query.level = filter.level;
  if (filter.file !== undefined) query.file = filter.file;
  if (filter.object !== undefined) query.object = filter.object;
  if (filter.operation_id !== undefined) query.operation_id = filter.operation_id;
  return query;
}

/** The `operation` and `outcome` members: a refusal is the operation of every refusal. */
function operationMembers(filter: LogsFilterState): JournalFilters {
  if (filter.outcome === "refused") return { operation: DENIED_OPERATION };
  const query: JournalFilters = {};
  if (filter.operation !== undefined) query.operation = filter.operation;
  if (filter.outcome !== undefined) query.outcome = filter.outcome;
  return query;
}

/**
 * The journal query of a filter: newest first, with display names, one page. Pure.
 *
 * @param filter The filter.
 * @param now The time a range ends at.
 * @param paging The cursor of the next page, from the page before.
 * @returns The filters for `GET /api/v1/admin/journal`.
 */
export function journalQuery(
  filter: LogsFilterState,
  now: Date,
  paging: { after?: number } = {},
): JournalFilters {
  const query: JournalFilters = {
    order: "desc",
    names: true,
    limit: PAGE_SIZE,
    ...sharedMembers(filter, now),
    ...operationMembers(filter),
  };
  if (paging.after !== undefined) query.after = paging.after;
  return query;
}

/** What the search text names. */
export type SearchKind =
  | { readonly kind: "empty" }
  | { readonly kind: "none" }
  | { readonly kind: "object"; readonly value: string }
  | { readonly kind: "uuid"; readonly value: string }
  | { readonly kind: "operation"; readonly value: string };

const UUID = "[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}";
const OBJECT_TEXT = new RegExp(`^(urn:uuid:${UUID})(?:@v\\d+)?$`, "i");
const UUID_TEXT = new RegExp(`^${UUID}$`, "i");
/** An operation name: lower-case dotted words such as `ocfl.commit` or `cli.library.open`. */
const OPERATION_TEXT = /^[a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_-]*)+$/;

/**
 * Tell what the search text names: an object (`urn:uuid:` with or without a version), a bare
 * UUID (an operation ID or a file ID, told apart by the view), an operation name, or nothing the
 * journal can be searched by. Pure.
 *
 * @param text The text typed in the search field.
 * @returns The kind and the value to filter by.
 */
export function classifySearch(text: string): SearchKind {
  const trimmed = text.trim();
  if (trimmed === "") return { kind: "empty" };
  const object = OBJECT_TEXT.exec(trimmed);
  if (object?.[1]) return { kind: "object", value: object[1] };
  if (UUID_TEXT.test(trimmed)) return { kind: "uuid", value: trimmed };
  if (OPERATION_TEXT.test(trimmed)) return { kind: "operation", value: trimmed };
  return { kind: "none" };
}

/**
 * The filter with a search in it: the file, object, operation ID and operation members (the ones
 * the search field names, one at a time) are replaced by what the search names, and every other
 * filter is kept. A search that names nothing clears the file, object and operation ID members.
 * Pure.
 *
 * @param filter The filter before.
 * @param search What the text names.
 * @param uuidAs What a bare UUID is filtered as (the view asks the journal which it is).
 * @returns The new filter.
 */
export function searchedFilter(
  filter: LogsFilterState,
  search: SearchKind,
  uuidAs: "operation_id" | "file" = "operation_id",
): LogsFilterState {
  const base: { -readonly [K in keyof LogsFilterState]: LogsFilterState[K] } = { ...filter };
  delete base.file;
  delete base.object;
  delete base.operation_id;
  // The search field names one of the file, object, operation ID and operation: a new search
  // replaces whichever it held.
  if (search.kind === "operation" || search.kind === "object" || search.kind === "uuid") {
    delete base.operation;
  }
  if (search.kind === "object") return { ...base, object: search.value };
  if (search.kind === "operation") return { ...base, operation: search.value };
  if (search.kind === "uuid") return { ...base, [uuidAs]: search.value };
  return base;
}

/** The kind of outcome an entry shows. */
export type OutcomeKind =
  "ok" | "failed" | "refused" | "started" | "stalled" | "resumed" | "cancelled" | "none";

const OUTCOME_KINDS: Readonly<Record<Outcome, OutcomeKind>> = {
  succeeded: "ok",
  failed: "failed",
  stalled: "stalled",
  started: "started",
  resumed: "resumed",
  cancelled: "cancelled",
  info: "none",
};

/**
 * The outcome an entry shows: an authorisation refusal is `refused`, whatever its outcome says.
 * Pure.
 *
 * @param entry The entry.
 * @returns The kind.
 */
export function outcomeKind(entry: JournalEntryView): OutcomeKind {
  if (entry.operation === DENIED_OPERATION) return "refused";
  return OUTCOME_KINDS[entry.outcome] ?? "none";
}

/**
 * True for an entry the view tags journal-only: a failed or refused entry, or a single-file
 * download, that the API marks as having no provenance activity. Pure.
 *
 * @param entry The entry.
 */
export function isJournalOnlyTagged(entry: JournalEntryView): boolean {
  if (!entry.provenance?.journal_only) return false;
  const kind = outcomeKind(entry);
  return kind === "failed" || kind === "refused" || entry.operation === "file.download";
}

/** The characters of an opaque ID kept at each end of its short form. */
const SHORT_KEEP = 4;

/**
 * An opaque ID as the table shows it: its first and last four characters around an ellipsis, or
 * a dash when there is none. Pure.
 *
 * @param id The ID.
 */
export function shortId(id: string | undefined): string {
  if (id === undefined || id === "") return "—";
  if (id.length <= 2 * SHORT_KEEP + 1) return id;
  return `${id.slice(0, SHORT_KEEP)}…${id.slice(-SHORT_KEEP)}`;
}

/**
 * The text of an entry's target: the display name the route resolved, else the first target ID
 * (opaque) that is not a library, else the name the libraries route gave the library the entry
 * is about, else the first target, else a dash. Pure.
 *
 * @param entry The entry.
 * @param libraries The libraries the host opened, by ID (the names the libraries route gave).
 */
export function targetText(
  entry: JournalEntryView,
  libraries: ReadonlyMap<string, string> = new Map(),
): string {
  if (entry.display_name !== undefined && entry.display_name !== "") return entry.display_name;
  const own = entry.target_ids.find((id) => !libraries.has(id));
  if (own !== undefined) return own;
  const first = entry.target_ids[0];
  return (first === undefined ? undefined : libraries.get(first)) ?? first ?? "—";
}

/** How the outcome chip of a row is drawn. */
export type OutcomeTone = "ok" | "alert" | "muted" | "none";

/** The cells of one row of the table. */
export interface RowView {
  /** Stable key: the entry ID. */
  readonly key: string;
  /** The local time of day, `HH:MM:SS`. */
  readonly time: string;
  /** The full local time, for the cell's tooltip. */
  readonly timeTitle: string;
  /** The level word. */
  readonly level: string;
  /** `alert` for warn and error. */
  readonly levelTone: "plain" | "alert";
  /** The operation. */
  readonly operation: string;
  /** The outcome chip. */
  readonly outcome: {
    readonly kind: OutcomeKind;
    readonly text: string;
    readonly tone: OutcomeTone;
  };
  /** The display name, or the opaque ID. */
  readonly target: string;
  /** The short operation ID, or a dash. */
  readonly operationId: string;
  /** Whether the row carries the journal-only tag. */
  readonly journalOnly: boolean;
}

const OUTCOME_LABEL: Readonly<Record<OutcomeKind, LabelKey>> = {
  ok: "logsOutcomeOk",
  failed: "logsOutcomeFailed",
  refused: "logsOutcomeRefused",
  started: "logsOutcomeStarted",
  stalled: "logsOutcomeStalled",
  resumed: "logsOutcomeResumed",
  cancelled: "logsOutcomeCancelled",
  none: "logsNone",
};

const OUTCOME_TONE: Readonly<Record<OutcomeKind, OutcomeTone>> = {
  ok: "ok",
  failed: "alert",
  refused: "alert",
  stalled: "alert",
  started: "muted",
  resumed: "muted",
  cancelled: "muted",
  none: "none",
};

const LEVEL_LABEL: Readonly<Record<Level, LabelKey>> = {
  info: "logsLevelInfo",
  warn: "logsLevelWarn",
  error: "logsLevelError",
};

/** The language numbers and dates are written in. */
function language(): string {
  return (typeof document !== "undefined" && document.documentElement.lang) || "en";
}

/** The local time of day of an instant, `HH:MM:SS`, in `zone` (the viewer's zone when absent). */
function timeOfDay(time: string, zone?: string): string {
  const format = new Intl.DateTimeFormat("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
    ...(zone ? { timeZone: zone } : {}),
  });
  return format.format(new Date(time));
}

/** The long local time of an instant: `Oct 19, 2026, 11:03:02`, in `zone`. */
function localDateTime(time: string, lang: string, zone?: string): string {
  const format = new Intl.DateTimeFormat(lang, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
    ...(zone ? { timeZone: zone } : {}),
  });
  return format.format(new Date(time));
}

/**
 * The offset of a time zone from UTC as the footer names it: `UTC`, `UTC+2`, `UTC-4`,
 * `UTC+5:30`. Pure.
 *
 * @param at The instant the offset is read at (daylight saving changes it).
 * @param zone The IANA zone, or the viewer's zone when absent.
 * @returns The label; the zone's own name when the platform gives no offset.
 */
export function utcOffsetLabel(at: Date, zone?: string): string {
  const options: Intl.DateTimeFormatOptions = {
    timeZoneName: "longOffset",
    ...(zone ? { timeZone: zone } : {}),
  };
  const part = new Intl.DateTimeFormat("en-US", options)
    .formatToParts(at)
    .find((p) => p.type === "timeZoneName");
  const match = /^GMT(?:([+-])(\d{2}):(\d{2}))?$/.exec(part?.value ?? "");
  if (!match) return zone ?? "UTC";
  if (!match[1] || (match[2] === "00" && match[3] === "00")) return "UTC";
  const hours = String(Number(match[2]));
  const minutes = match[3] === "00" ? "" : `:${match[3]}`;
  return `UTC${match[1]}${hours}${minutes}`;
}

/**
 * The cells of one row. Pure.
 *
 * @param entry The entry.
 * @param labels The active label table.
 * @param zone The IANA zone times are shown in; the viewer's when absent.
 * @param libraries The libraries the host opened, by ID, for naming a library target.
 * @returns The row view.
 */
export function rowView(
  entry: JournalEntryView,
  labels: LabelTable,
  zone?: string,
  libraries?: ReadonlyMap<string, string>,
): RowView {
  const kind = outcomeKind(entry);
  const operationId = entry.operation_id ?? entry.provenance?.operation_id;
  return {
    key: String(entry.id ?? `${entry.time}-${entry.operation}`),
    time: timeOfDay(entry.time, zone),
    timeTitle: `${localDateTime(entry.time, language(), zone)} (${utcOffsetLabel(new Date(entry.time), zone)})`,
    level: labels[LEVEL_LABEL[entry.level]] ?? entry.level,
    levelTone: entry.level === "info" ? "plain" : "alert",
    operation: entry.operation,
    outcome: { kind, text: labels[OUTCOME_LABEL[kind]], tone: OUTCOME_TONE[kind] },
    target: targetText(entry, libraries),
    operationId: shortId(operationId),
    journalOnly: isJournalOnlyTagged(entry),
  };
}

/** The phrase that ends the "Showing" line for a range. */
const RANGE_PHRASE: Readonly<Record<RangeId, LabelKey>> = {
  hour: "logsRangePhraseHour",
  day: "logsRangePhraseDay",
  week: "logsRangePhraseWeek",
  month: "logsRangePhraseMonth",
  all: "logsRangePhraseAll",
};

/** The chip word of a range. */
const RANGE_LABEL: Readonly<Record<RangeId, LabelKey>> = {
  hour: "logsRangeHour",
  day: "logsRangeDay",
  week: "logsRangeWeek",
  month: "logsRangeMonth",
  all: "logsRangeAll",
};

/** A whole number written for the page's language. */
function whole(count: number): string {
  return count.toLocaleString(language());
}

/**
 * The line under the table. The API has no count route, so "of {total}" is said only when the
 * whole range is loaded (the total is then the entries shown); otherwise it says more are
 * available. Pure.
 *
 * @param labels The active label table.
 * @param shown How many entries are listed.
 * @param more Whether the journal has older entries in the range that are not listed.
 * @param range The time range.
 */
export function showingText(
  labels: LabelTable,
  shown: number,
  more: boolean,
  range: RangeId,
): string {
  const phrase = labels[RANGE_PHRASE[range]];
  const values = { shown: whole(shown), total: whole(shown), range: phrase };
  return formatLabel(more ? labels.logsShowingMore : labels.logsShowing, values);
}

/**
 * The count at the right of the footer. Pure.
 *
 * @param labels The active label table.
 * @param count How many entries are listed.
 */
export function entryCountText(labels: LabelTable, count: number): string {
  const text = count === 1 ? labels.logsEntryCountOne : labels.logsEntryCountOther;
  return formatLabel(text, { count: whole(count) });
}

/**
 * The time-zone sentence at the left of the footer. Pure.
 *
 * @param labels The active label table.
 * @param zone The zone as `utcOffsetLabel` names it.
 */
export function footerZoneText(labels: LabelTable, zone: string): string {
  return formatLabel(labels.logsFooterZone, { zone });
}

/** A chip of the filter row. */
export type ChipId =
  "range" | "library" | "operation" | "outcome" | "level" | "file" | "object" | "operation_id";

/** One chip: its words and whether a filter is set on it. */
export interface FilterChip {
  /** Which filter. */
  readonly id: ChipId;
  /** The chip's text. */
  readonly text: string;
  /** True when the filter is set (the chip is not the "all" form). */
  readonly active: boolean;
  /** True for the chips of a file, object or operation ID, which are removed with a button. */
  readonly removable: boolean;
}

/** The word of an outcome filter. */
function outcomeWord(labels: LabelTable, outcome: OutcomeFilter): string {
  const kind: OutcomeKind = outcome === "refused" ? "refused" : OUTCOME_KINDS[outcome];
  return labels[OUTCOME_LABEL[kind]];
}

/** The name of a library: the name the route gave, else its ID. */
function libraryName(id: string, libraries: readonly LogsLibrary[]): string {
  return libraries.find((library) => library.library_id === id)?.name ?? id;
}

/**
 * The chips of the filter row: the five of mockup 38, then one for each file, object or
 * operation ID filter in force. Pure.
 *
 * @param filter The filter.
 * @param labels The active label table.
 * @param libraries The libraries the host opened, for the Library chip's name.
 */
export function filterChips(
  filter: LogsFilterState,
  labels: LabelTable,
  libraries: readonly LogsLibrary[],
): FilterChip[] {
  const all = labels.logsChipAll;
  const value = (set: string | undefined) => set ?? all;
  const chip = (id: ChipId, text: string, set: boolean, removable = false): FilterChip => ({
    id,
    text,
    active: set,
    removable,
  });
  const chips = [
    chip("range", labels[RANGE_LABEL[filter.range]], filter.range !== DEFAULT_RANGE),
    chip(
      "library",
      formatLabel(labels.logsChipLibrary, {
        value: value(
          filter.library === undefined ? undefined : libraryName(filter.library, libraries),
        ),
      }),
      filter.library !== undefined,
    ),
    chip(
      "operation",
      formatLabel(labels.logsChipOperation, { value: value(filter.operation) }),
      filter.operation !== undefined,
    ),
    chip(
      "outcome",
      formatLabel(labels.logsChipOutcome, {
        value: value(
          filter.outcome === undefined ? undefined : outcomeWord(labels, filter.outcome),
        ),
      }),
      filter.outcome !== undefined,
    ),
    chip(
      "level",
      formatLabel(labels.logsChipLevel, {
        value: value(filter.level === undefined ? undefined : labels[LEVEL_LABEL[filter.level]]),
      }),
      filter.level !== undefined,
    ),
  ];
  const extras: [ChipId, LabelKey, string | undefined][] = [
    ["file", "logsChipFile", filter.file],
    ["object", "logsChipObject", filter.object],
    ["operation_id", "logsChipOperationId", filter.operation_id],
  ];
  for (const [id, key, set] of extras) {
    if (set !== undefined)
      chips.push(chip(id, formatLabel(labels[key], { value: set }), true, true));
  }
  return chips;
}

/** The ID a journal filter names for a file: the UUID of its object (`urn:uuid:<file>`). */
function fileIdOf(text: string): string | undefined {
  const match = new RegExp(`^(?:urn:uuid:)?(${UUID})(?:@v\\d+)?$`, "i").exec(text);
  return match?.[1]?.toLowerCase();
}

/** The object part of an object reference (`urn:uuid:<file>@v8` is `urn:uuid:<file>`). */
function objectOf(reference: string | undefined): string | undefined {
  return reference?.split("@v", 1)[0];
}

/** The identifiers an entry names: its targets and the object of its object reference. */
function namedIds(entry: JournalEntryView): string[] {
  const ids = [...entry.target_ids];
  const object = objectOf(entry.object_ref ?? entry.provenance?.object_ref);
  if (object !== undefined) ids.push(object);
  return ids;
}

/** True when the entry names the file `file` among its identifiers. */
function namesFile(entry: JournalEntryView, file: string): boolean {
  const wanted = file.toLowerCase();
  return namedIds(entry).some((id) => fileIdOf(id) === wanted);
}

/** True when the entry satisfies the outcome filter. */
function matchesOutcome(entry: JournalEntryView, outcome: OutcomeFilter | undefined): boolean {
  if (outcome === undefined) return true;
  if (outcome === "refused") return entry.operation === DENIED_OPERATION;
  return entry.outcome === outcome;
}

/** True when the entry satisfies the filters on what it names. */
function matchesTargets(entry: JournalEntryView, filter: LogsFilterState): boolean {
  if (filter.library !== undefined && !entry.target_ids.includes(filter.library)) return false;
  if (filter.file !== undefined && !namesFile(entry, filter.file)) return false;
  if (filter.object !== undefined && !namedIds(entry).includes(filter.object)) return false;
  if (filter.operation_id === undefined) return true;
  return (entry.operation_id ?? entry.provenance?.operation_id) === filter.operation_id;
}

/**
 * Whether a live entry would have been read with the filter: the same tests the journal route
 * makes, so an entry that arrives over the event channel is counted only when the list would
 * have held it. Pure.
 *
 * @param entry The entry.
 * @param filter The filter.
 * @param now The time a range ends at.
 */
export function matchesFilter(
  entry: JournalEntryView,
  filter: LogsFilterState,
  now: Date,
): boolean {
  const from = rangeFrom(filter.range, now);
  if (from !== undefined && entry.time < from) return false;
  if (filter.operation !== undefined && entry.operation !== filter.operation) return false;
  if (filter.level !== undefined && entry.level !== filter.level) return false;
  return matchesOutcome(entry, filter.outcome) && matchesTargets(entry, filter);
}

/** One row of the entry detail. */
export interface DetailRow {
  /** Stable key, for tests and `data-key`. */
  readonly key: string;
  /** The row's label. */
  readonly label: string;
  /** The value. */
  readonly value: string;
  /** True for identifiers, times and codes. */
  readonly mono?: boolean;
  /** A quieter line after the value (the actor's role, "looked up in your library"). */
  readonly note?: string;
  /** True when the note follows the value on its line. */
  readonly noteInline?: boolean;
}

/** What the detail rows need besides the entry. */
export interface DetailContext {
  /** The IANA zone local times are shown in. */
  readonly zone?: string;
  /** The libraries the host opened. */
  readonly libraries: readonly LogsLibrary[];
  /** The page's language. */
  readonly lang: string;
  /** The library the entry belongs to, when the caller knows it. */
  readonly libraryId?: string;
}

/** The library of an entry: the one the caller names, else the first target the host knows. */
function libraryOf(entry: JournalEntryView, ctx: DetailContext): string | undefined {
  if (ctx.libraryId !== undefined) return ctx.libraryId;
  return entry.target_ids.find((id) => ctx.libraries.some((l) => l.library_id === id));
}

/** `urn:uuid:<file>@v8` as `urn:uuid:<file> → v8`. */
function objectText(reference: string): string {
  const at = reference.lastIndexOf("@v");
  if (at < 0) return reference;
  return `${reference.slice(0, at)} → v${reference.slice(at + 2)}`;
}

/**
 * The rows of the entry detail, in the order of mockup 38, with a row only where the entry has
 * a value (the library, the object, the file name and the duration are not always known), and
 * the error code and process the entry has. Pure.
 *
 * @param entry The entry.
 * @param labels The active label table.
 * @param ctx The zone, the libraries and the language.
 */
export function detailRows(
  entry: JournalEntryView,
  labels: LabelTable,
  ctx: DetailContext,
): DetailRow[] {
  const at = new Date(entry.time);
  const rows: (DetailRow | undefined)[] = [
    { key: "time-utc", label: labels.logsDetailTimeUtc, value: entry.time, mono: true },
    {
      key: "time-local",
      label: labels.logsDetailTimeLocal,
      value: `${localDateTime(entry.time, ctx.lang, ctx.zone)} (${utcOffsetLabel(at, ctx.zone)})`,
    },
    {
      key: "level",
      label: labels.logsDetailLevel,
      value: labels[LEVEL_LABEL[entry.level]],
      mono: true,
    },
    { key: "outcome", label: labels.logsDetailOutcome, value: rowView(entry, labels).outcome.text },
    optional("error", labels.logsDetailError, entry.error_code, { mono: true }),
    actorRow(entry, labels),
    { key: "software", label: labels.logsDetailSoftware, value: entry.host },
    optional("process", labels.logsDetailProcess, entry.process_id, { mono: true }),
    libraryRow(entry, labels, ctx),
    optional("object", labels.logsDetailObject, objectRef(entry), { mono: true, text: objectText }),
    optional("file", labels.logsDetailFile, entry.display_name, {
      note: labels.logsLookedUp,
      inline: true,
    }),
    optional(
      "operation-id",
      labels.logsDetailOperationId,
      entry.operation_id ?? entry.provenance?.operation_id,
      {
        mono: true,
      },
    ),
    optional(
      "duration",
      labels.logsDetailDuration,
      entry.duration_ms === undefined ? undefined : String(entry.duration_ms),
      { text: (ms) => formatLabel(labels.logsDuration, { ms }) },
    ),
  ];
  return rows.filter((row): row is DetailRow => row !== undefined);
}

/** The object reference of an entry. */
function objectRef(entry: JournalEntryView): string | undefined {
  return entry.object_ref ?? entry.provenance?.object_ref;
}

/** A row that exists only when the entry has a value for it. */
function optional(
  key: string,
  label: string,
  value: string | undefined,
  style: {
    mono?: boolean;
    note?: string;
    inline?: boolean;
    text?: (value: string) => string;
  } = {},
): DetailRow | undefined {
  if (value === undefined || value === "") return undefined;
  const text = style.text ? style.text(value) : value;
  return {
    key,
    label,
    value: text,
    ...(style.mono ? { mono: true } : {}),
    ...(style.note ? { note: style.note } : {}),
    ...(style.inline ? { noteInline: true } : {}),
  };
}

/** The actor row: the installation's agent, named as the owner's. */
function actorRow(entry: JournalEntryView, labels: LabelTable): DetailRow {
  return {
    key: "actor",
    label: labels.logsDetailActor,
    value: entry.actor,
    mono: true,
    ...(entry.actor.startsWith("urn:uuid:") ? { note: labels.logsActorOwner } : {}),
  };
}

/** The library row: its name, or its ID when the host did not name it. */
function libraryRow(
  entry: JournalEntryView,
  labels: LabelTable,
  ctx: DetailContext,
): DetailRow | undefined {
  const id = libraryOf(entry, ctx);
  if (id === undefined) return undefined;
  return { key: "library", label: labels.logsDetailLibrary, value: libraryName(id, ctx.libraries) };
}

/** What View provenance opens: the file and the operation whose activities it shows. */
export interface ProvenanceTarget {
  /** The file's UUID. */
  readonly file: string;
  /** The operation ID the activities share. */
  readonly operation_id: string;
}

/**
 * The target of View provenance. Only an entry of a committed operation has activities: a
 * journal-only entry, or one without an operation ID or a file object, has none. Pure.
 *
 * @param entry The entry.
 * @returns The target, or undefined when the entry has no provenance to open.
 */
export function viewProvenanceTarget(entry: JournalEntryView): ProvenanceTarget | undefined {
  const link = entry.provenance;
  if (!link || link.journal_only) return undefined;
  const operation = link.operation_id ?? entry.operation_id;
  const file = fileIdOf(objectOf(link.object_ref ?? entry.object_ref) ?? "");
  if (operation === undefined || file === undefined) return undefined;
  return { file, operation_id: operation };
}

/** The scopes of the Download dialog. */
export type ExportScope = "filter" | "range" | "all";

/** The formats of the Download dialog. */
export type ExportFormat = "jsonl" | "csv";

/** What the Download dialog chose. */
export interface ExportChoice {
  /** Which entries. */
  readonly scope: ExportScope;
  /** The format. */
  readonly format: ExportFormat;
  /** The start of a time range, RFC 3339 UTC. */
  readonly from?: string;
  /** The end of a time range, RFC 3339 UTC. */
  readonly to?: string;
}

/** The date `YYYY-MM-DD` of an instant in `zone`. */
function localDate(at: Date, zone?: string): string {
  const format = new Intl.DateTimeFormat("en-CA", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    ...(zone ? { timeZone: zone } : {}),
  });
  return format.format(at);
}

/**
 * The file name the dialog proposes: the product prefix, the local date and the format. Pure.
 *
 * @param labels The active label table.
 * @param format The format.
 * @param now The time of the download.
 * @param zone The IANA zone the date is read in.
 */
export function exportFileName(
  labels: LabelTable,
  format: ExportFormat,
  now: Date,
  zone?: string,
): string {
  return `${labels.logsExportFilePrefix}-${localDate(now, zone)}.${format}`;
}

/**
 * The export query of a choice, oldest first: the current filter (no paging), a time range, or
 * everything. The route adds no names. Pure.
 *
 * @param choice The dialog's choice.
 * @param filter The view's filter.
 * @param now The time a range ends at.
 */
export function exportQuery(
  choice: ExportChoice,
  filter: LogsFilterState,
  now: Date,
): Omit<JournalFilters, "after" | "limit" | "names"> {
  if (choice.scope === "all") return { format: choice.format };
  if (choice.scope === "range") {
    return {
      format: choice.format,
      ...(choice.from !== undefined ? { from: choice.from } : {}),
      ...(choice.to !== undefined ? { to: choice.to } : {}),
    };
  }
  const query = journalQuery(filter, now);
  delete query.order;
  delete query.names;
  delete query.limit;
  delete query.after;
  return { ...query, format: choice.format };
}

/** How many entries a count says the filter matches, as far as the view has read them. */
export interface LoadedCount {
  /** Entries read so far. */
  readonly count: number;
  /** Whether older entries remain unread. */
  readonly more: boolean;
}

/**
 * The line under the "Current filter" radio: the range, the library, the operation and any other
 * filter in words, and the count when the view has read the whole range. Pure.
 *
 * @param filter The filter.
 * @param labels The active label table.
 * @param libraries The libraries, for a library's name.
 * @param loaded What the view has read, when it has read something.
 */
export function exportFilterLine(
  filter: LogsFilterState,
  labels: LabelTable,
  libraries: readonly LogsLibrary[],
  loaded?: LoadedCount,
): string {
  const parts = [
    labels[RANGE_LABEL[filter.range]],
    filter.library === undefined
      ? labels.logsExportAllLibraries
      : libraryName(filter.library, libraries),
    filter.operation === undefined ? labels.logsExportAllOperations : filter.operation,
  ];
  if (filter.outcome !== undefined) parts.push(outcomeWord(labels, filter.outcome));
  if (filter.level !== undefined) parts.push(labels[LEVEL_LABEL[filter.level]]);
  for (const id of [filter.file, filter.object, filter.operation_id])
    if (id !== undefined) parts.push(id);
  const text = parts.join(" · ");
  if (loaded === undefined) return text;
  const count = loaded.more
    ? formatLabel(labels.logsExportCountAtLeast, { count: whole(loaded.count) })
    : entryCountText(labels, loaded.count);
  return formatLabel(labels.logsExportFilterLine, { filter: text, count });
}

/**
 * Whether the data of an event is a journal entry: an object with a time, an operation and an
 * outcome. Pure.
 *
 * @param data The `data` of a `journal.entry` event.
 */
export function isLiveEntry(data: unknown): data is JournalEntry {
  if (typeof data !== "object" || data === null) return false;
  const entry = data as Partial<JournalEntry>;
  return (
    typeof entry.time === "string" &&
    typeof entry.operation === "string" &&
    typeof entry.outcome === "string" &&
    Array.isArray(entry.target_ids)
  );
}

/**
 * The entry the list shows for a live event. The event has no PROV-O link, so one is built from
 * what the entry carries: a failed or refused entry and a single-file download have no activity
 * (journal-only by definition, UI-S-021), any other entry names its operation and object. The
 * entry detail reads the entry again for its display name. Pure.
 *
 * @param entry The data of a `journal.entry` event.
 */
export function liveEntryView(entry: JournalEntry): JournalEntryView {
  const view = { ...entry, provenance: { journal_only: false } } as JournalEntryView;
  const kind = outcomeKind(view);
  const none = kind === "failed" || kind === "refused" || entry.operation === "file.download";
  return {
    ...view,
    provenance: {
      journal_only: none,
      ...(none || entry.operation_id === undefined ? {} : { operation_id: entry.operation_id }),
      ...(none || entry.object_ref === undefined ? {} : { object_ref: entry.object_ref }),
    },
  };
}

/**
 * The list with newer entries put in front, newest first, each ID once (the entry already
 * listed wins, because the list read it with its display name). Pure.
 *
 * @param list The entries listed, newest first.
 * @param newer Entries that arrived since.
 */
export function mergeNewest(
  list: readonly JournalEntryView[],
  newer: readonly JournalEntryView[],
): JournalEntryView[] {
  const known = new Set(list.map((entry) => entry.id));
  const fresh = newer.filter((entry) => entry.id === undefined || !known.has(entry.id));
  return [...fresh.sort((a, b) => (b.id ?? 0) - (a.id ?? 0)), ...list];
}
