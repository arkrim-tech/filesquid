/**
 * The model behind the global search bar and the search results (UI-F2.9, F-063, UI-S-007,
 * UI-S-030, API-S-012): the chips the bar applies and how chips plus free text become the query
 * the search operation takes (and back, so an address restores the bar), the date ranges of the
 * Modified chip in the caller's zone, how the host's refusals are told apart and name their
 * operator, and the suggestions a typed word brings. Pure: no I/O and no DOM. The grammar is the
 * published one (`GET /api/v1/files/search`); nothing here invents an operator.
 *
 * Every list and loop is bounded (AGENTS.md section 22) by the `MAX_*` constants.
 */
import { ApiError } from "../api/client";
import { defaultDirection } from "../drive/model";
import type { SortDirection, SortKey } from "../preferences/model";
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";
import type { SearchSort } from "./source";

/** Most bytes of query the search operation accepts (API-S-012 `MAX_QUERY_BYTES`). */
export const MAX_QUERY_BYTES = 1024;
/** Most terms the search operation accepts (API-S-012 `MAX_QUERY_TERMS`). */
export const MAX_QUERY_TERMS = 32;
/** Most chips the bar holds at once. */
export const MAX_CHIPS = 8;
/** Matching files the drop-down lists (mockup 20). */
export const SUGGESTED_FILES = 3;
/** Milliseconds the bar waits after a key before it asks for live suggestions. */
export const SUGGEST_DEBOUNCE_MS = 250;
/** Most collections the Location filter lists (one page, bounded). */
export const MAX_LOCATIONS = 50;

/** The classes `type:` takes (API-S-012); `folder` stands for collections. */
export const TYPE_CLASSES = ["document", "image", "video", "audio", "text", "folder"] as const;
/** One class of `type:`. */
export type TypeClass = (typeof TYPE_CLASSES)[number];

/** The ranges of the Modified chip. */
export const MODIFIED_RANGES = ["today", "week", "month", "year"] as const;
/** One range of the Modified chip. */
export type ModifiedRange = (typeof MODIFIED_RANGES)[number];

/** Days each Modified range reaches back from the caller's calendar date. */
const RANGE_DAYS: Readonly<Record<ModifiedRange, number>> = {
  today: 0,
  week: 7,
  month: 30,
  year: 365,
};

/** What a chip filters on. */
export type ChipFilter = "type" | "in" | "modified" | "title";

/**
 * One applied chip. `value` is the class (`type`), the collection's title (`in`), the range
 * (`modified`) or the text (`title`).
 */
export interface SearchChip {
  /** What the chip filters on. */
  readonly filter: ChipFilter;
  /** The chip's value. */
  readonly value: string;
}

/**
 * The operators the desktop host answers, as the drop-down and the refusal page list them
 * (mockups 20, 21 and 53, UI-S-030). The operators of `filesquid-server` join with SV-F1.4.
 */
export const OPERATORS_ON_HOST: readonly string[] = [
  '"exact phrase"',
  "-word",
  "type:",
  "in:",
  "title:",
  "creator:",
  "before:",
  "after:",
  "size>",
  "is:deleted",
];

/** Operator names the grammar answers (folded; `size>` and `size<` carry their sign). */
const ANSWERED = new Set([
  "type",
  "in",
  "title",
  "owner",
  "creator",
  "tag",
  "is",
  "before",
  "after",
  "createdbefore",
  "createdafter",
  "source",
  "size>",
  "size<",
]);

/** Operators of the grammar that only `filesquid-server` answers (refused as unavailable here). */
const SERVER_OPERATORS = new Set(["sharedwith", "to", "from", "pendingowner"]);

/** The part of a query the bar and the refusal page look at. */
export interface QueryToken {
  /** The token as typed, quotes included. */
  readonly raw: string;
  /** The text with its quotes removed. */
  readonly text: string;
  /** Whether a leading minus excludes what the term matches. */
  readonly negated: boolean;
  /** The operator's folded name (`size>` and `size<` with their sign), when the term is one. */
  readonly operator?: string;
  /** The operator's value, or the term's text when it is no operator. */
  readonly value: string;
}

/** The label key of a type class's name. */
const TYPE_LABEL: Readonly<Record<TypeClass, LabelKey>> = {
  document: "searchTypeDocument",
  image: "searchTypeImage",
  video: "searchTypeVideo",
  audio: "searchTypeAudio",
  text: "searchTypeText",
  folder: "searchTypeFolder",
};

/** The label key of a Modified range's name. */
export const RANGE_LABEL: Readonly<Record<ModifiedRange, LabelKey>> = {
  today: "searchModifiedToday",
  week: "searchModifiedWeek",
  month: "searchModifiedMonth",
  year: "searchModifiedYear",
};

/** The label key of a filter's name. */
export const FILTER_LABEL: Readonly<Record<ChipFilter, LabelKey>> = {
  type: "searchFilterType",
  in: "searchFilterLocation",
  modified: "searchFilterModified",
  title: "searchFilterTitle",
};

/** Whether `value` is a class of `type:`. */
export function isTypeClass(value: string): value is TypeClass {
  return (TYPE_CLASSES as readonly string[]).includes(value);
}

/** The text of `value` cut to `limit` bytes of UTF-8 at a character boundary. */
function cutToBytes(value: string, limit: number): string {
  const encoder = new TextEncoder();
  if (encoder.encode(value).length <= limit) return value;
  let bytes = 0;
  let out = "";
  for (const character of value) {
    bytes += encoder.encode(character).length;
    if (bytes > limit) break;
    out += character;
  }
  return out;
}

/** `value` as one query term: quoted when it holds a space, a colon or a quote. */
function termValue(value: string): string {
  const plain = value.replaceAll('"', "");
  return /[\s:]/.test(plain) ? `"${plain}"` : plain;
}

/**
 * The date `days` before `now` in the caller's calendar, as `YYYY-MM-DD`. Pure.
 *
 * @param now The time now.
 * @param days Days back, from 0.
 * @returns The date.
 */
function daysBack(now: Date, days: number): string {
  const date = new Date(now.getFullYear(), now.getMonth(), now.getDate() - days);
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

/**
 * The `after:` date of a Modified range: the caller's calendar date that many days back. Pure.
 *
 * @param range The range.
 * @param now The time now.
 * @returns The date as `YYYY-MM-DD`.
 */
export function modifiedAfter(range: ModifiedRange, now: Date): string {
  return daysBack(now, RANGE_DAYS[range]);
}

/**
 * The caller's zone as the search operation takes it: `Z`, or a fixed offset from -14:00 to
 * +14:00 (API-S-012). Pure.
 *
 * @param now The time now (its zone offset is read).
 * @returns The zone.
 */
export function zoneOf(now: Date): string {
  const minutes = -now.getTimezoneOffset();
  if (minutes === 0) return "Z";
  const clamped = Math.max(-14 * 60, Math.min(14 * 60, minutes));
  const sign = clamped < 0 ? "-" : "+";
  const hours = String(Math.floor(Math.abs(clamped) / 60)).padStart(2, "0");
  const rest = String(Math.abs(clamped) % 60).padStart(2, "0");
  return `${sign}${hours}:${rest}`;
}

/**
 * The query term a chip stands for. Pure.
 *
 * @param chip The chip.
 * @param now The time now, for the Modified ranges.
 * @returns The term, for example `type:document` or `after:2026-09-16`.
 */
export function chipTerm(chip: SearchChip, now: Date): string {
  if (chip.filter === "modified") {
    return `after:${modifiedAfter(chip.value as ModifiedRange, now)}`;
  }
  return `${chip.filter}:${termValue(chip.value)}`;
}

/**
 * The query for chips and free text: the chip terms, then the text, cut at the API's byte limit
 * at a character boundary. Pure.
 *
 * @param chips The applied chips.
 * @param text The free text typed after them.
 * @param now The time now.
 * @returns The query, or an empty string when there is neither a chip nor text.
 */
export function composeQuery(chips: readonly SearchChip[], text: string, now: Date): string {
  const terms = chips.slice(0, MAX_CHIPS).map((chip) => chipTerm(chip, now));
  const query = [...terms, text.trim()].filter((part) => part !== "").join(" ");
  return cutToBytes(query, MAX_QUERY_BYTES);
}

/**
 * Add a chip. Type, Location and Modified keep one chip each (the newest replaces the older);
 * Title chips accumulate but never repeat; the list never exceeds {@link MAX_CHIPS}. Pure.
 *
 * @param chips The chips now.
 * @param chip The chip to add.
 * @returns The new list.
 */
export function addChip(chips: readonly SearchChip[], chip: SearchChip): SearchChip[] {
  if (chip.filter === "title") {
    const repeated = chips.some((c) => c.filter === "title" && c.value === chip.value);
    return repeated || chips.length >= MAX_CHIPS ? [...chips] : [...chips, chip];
  }
  const index = chips.findIndex((c) => c.filter === chip.filter);
  if (index >= 0) return chips.map((c, at) => (at === index ? chip : c));
  return chips.length >= MAX_CHIPS ? [...chips] : [...chips, chip];
}

/**
 * Remove the chip at `index`. Pure.
 *
 * @param chips The chips now.
 * @param index Its place.
 * @returns The new list; the same chips when there is none at `index`.
 */
export function removeChip(chips: readonly SearchChip[], index: number): SearchChip[] {
  return chips.filter((_, at) => at !== index);
}

/**
 * The words of a chip: "Type: Document", "Modified: last 30 days", "Location: {title}". Pure.
 *
 * @param chip The chip.
 * @param labels The active label table.
 * @returns The chip's text.
 */
export function chipLabel(chip: SearchChip, labels: LabelTable): string {
  return formatLabel(labels.searchChip, {
    filter: labels[FILTER_LABEL[chip.filter]],
    value: chipValueLabel(chip, labels),
  });
}

/** The value of a chip as the label table words it. */
export function chipValueLabel(chip: SearchChip, labels: LabelTable): string {
  if (chip.filter === "type" && isTypeClass(chip.value)) return labels[TYPE_LABEL[chip.value]];
  if (chip.filter === "modified" && chip.value in RANGE_LABEL) {
    return labels[RANGE_LABEL[chip.value as ModifiedRange]];
  }
  return chip.value;
}

/** The index of the first sign in `body` that makes it an operator, or -1. */
function signAt(body: string): number {
  return body.search(/[:<>]/);
}

/** The folded operator name and value of `body`, when it is an operator (API-S-012). */
function operatorOf(
  body: string,
  quotedAt: number | undefined,
): { name: string; value: string } | undefined {
  const split = signAt(body);
  if (split < 0 || (quotedAt !== undefined && split >= quotedAt)) return undefined;
  const name = body.slice(0, split);
  if (!/^[A-Za-z]+$/.test(name)) return undefined;
  const sign = body.charAt(split);
  const rest = body.slice(split + 1);
  if (sign === ":") return { name: name.toLowerCase(), value: rest };
  return name.toLowerCase() === "size" ? { name: `size${sign}`, value: rest } : undefined;
}

/** One token of the query with its quotes handled as the API handles them. */
function tokenOf(raw: string, text: string, quotedFrom: number | undefined): QueryToken {
  const negated = text.startsWith("-") && quotedFrom !== 0;
  const body = negated ? text.slice(1) : text;
  const quotedAt =
    quotedFrom === undefined ? undefined : Math.max(0, quotedFrom - (negated ? 1 : 0));
  const operator = operatorOf(body, quotedAt);
  return operator
    ? { raw, text, negated, operator: operator.name, value: operator.value }
    : { raw, text, negated, value: body };
}

/**
 * Split a query into its terms as the search operation does: runs of non-space characters, where
 * a quoted part may hold spaces, a leading minus excludes, and an operator is letters before a
 * colon outside quotes (or `size` before `>` or `<`). At most {@link MAX_QUERY_TERMS} tokens are
 * returned. Pure.
 *
 * @param query The query.
 * @returns The tokens, in order.
 */
export function tokenizeQuery(query: string): QueryToken[] {
  const tokens: QueryToken[] = [];
  let start = -1;
  let text = "";
  let quotedFrom: number | undefined;
  let inQuotes = false;
  const flush = (end: number) => {
    if (start >= 0 && tokens.length < MAX_QUERY_TERMS) {
      tokens.push(tokenOf(query.slice(start, end), text, quotedFrom));
    }
    start = -1;
    text = "";
    quotedFrom = undefined;
  };
  for (let at = 0; at < query.length; at += 1) {
    const character = query.charAt(at);
    if (!inQuotes && /\s/.test(character)) {
      flush(at);
      continue;
    }
    if (start < 0) start = at;
    if (character === '"') {
      inQuotes = !inQuotes;
      quotedFrom ??= text.length;
    } else text += character;
  }
  flush(query.length);
  return tokens;
}

/** The chip a term stands for, when it is exactly the term a chip makes. */
function chipOf(token: QueryToken, now: Date): SearchChip | undefined {
  if (token.negated || token.operator === undefined) return undefined;
  const value = token.value;
  if (token.operator === "type") {
    const folded = value.toLowerCase();
    return isTypeClass(folded) ? { filter: "type", value: folded } : undefined;
  }
  if (token.operator === "in") return value.trim() === "" ? undefined : { filter: "in", value };
  if (token.operator === "title")
    return value.trim() === "" ? undefined : { filter: "title", value };
  if (token.operator !== "after") return undefined;
  const range = MODIFIED_RANGES.find((candidate) => modifiedAfter(candidate, now) === value);
  return range ? { filter: "modified", value: range } : undefined;
}

/** Whether a chip of this kind is already applied (Title chips may repeat). */
function taken(chips: readonly SearchChip[], chip: SearchChip): boolean {
  if (chips.length >= MAX_CHIPS) return true;
  if (chip.filter === "title")
    return chips.some((c) => c.filter === "title" && c.value === chip.value);
  return chips.some((c) => c.filter === chip.filter);
}

/**
 * Read a query back into chips and free text, so an address restores the bar. A term becomes a
 * chip only when it is exactly the term a chip makes (`type:` of a class, `in:`, `title:`, and
 * `after:` of a Modified range's date); everything else stays in the text as typed, in order, so
 * composing the result asks for the same terms. Pure.
 *
 * @param query The query.
 * @param now The time now.
 * @returns The chips and the text.
 */
export function parseQuery(query: string, now: Date): { chips: SearchChip[]; text: string } {
  const chips: SearchChip[] = [];
  const rest: string[] = [];
  for (const token of tokenizeQuery(query)) {
    const chip = chipOf(token, now);
    if (chip && !taken(chips, chip)) chips.push(chip);
    else rest.push(token.raw);
  }
  return { chips, text: rest.join(" ") };
}

/** Why a search was refused, naming the operator. */
export interface SearchRefusal {
  /** `unavailable`: the host cannot answer the operator; `unknown`: it is not in the grammar. */
  readonly kind: "unavailable" | "unknown";
  /** The operator as the page names it, for example `sharedwith:`, `is:favorite` or `colour:`. */
  readonly operator: string;
}

/** The operator of a token as the refusal page names it. */
function operatorName(token: QueryToken): string {
  const name = token.operator ?? "";
  if (name === "is") return `is:${token.value.toLowerCase()}`;
  return name.endsWith(">") || name.endsWith("<") ? name : `${name}:`;
}

/** Whether the host refuses this operator as one it cannot answer. */
function unanswerable(token: QueryToken): boolean {
  const name = token.operator;
  if (name === undefined) return false;
  if (SERVER_OPERATORS.has(name)) return true;
  if (name === "is") return token.value.toLowerCase() === "favorite";
  return name === "owner" && token.value.trim().toLowerCase() !== "me";
}

/** Whether the grammar has no such operator (or no such value of `is:`). */
function unknownOperator(token: QueryToken): boolean {
  const name = token.operator;
  if (name === undefined) return false;
  if (name === "is") return !["deleted", "favorite"].includes(token.value.toLowerCase());
  return !ANSWERED.has(name) && !SERVER_OPERATORS.has(name);
}

/**
 * Tell why the search operation refused a query. An `unavailable` answer (503) names the first
 * operator the host cannot answer; an `invalid_request` (400) names the first operator that is
 * not in the grammar. A refusal that names no operator (a bound, an unclosed quote) and any other
 * failure give undefined, so the view shows its ordinary failure state. Pure.
 *
 * @param error What the search rejected with.
 * @param query The query that was sent.
 * @returns The refusal, or undefined.
 */
export function refusalOf(error: unknown, query: string): SearchRefusal | undefined {
  if (!(error instanceof ApiError)) return undefined;
  const tokens = tokenizeQuery(query);
  if (error.status === 503 && error.code === "unavailable") {
    const token = tokens.find(unanswerable);
    return token ? { kind: "unavailable", operator: operatorName(token) } : undefined;
  }
  if (error.status === 400 && error.code === "invalid_request") {
    const token = tokens.find(unknownOperator);
    return token ? { kind: "unknown", operator: operatorName(token) } : undefined;
  }
  return undefined;
}

/**
 * The class of `type:` a media type belongs to (API-S-012: PDF, RTF and the Microsoft Office,
 * OOXML and OpenDocument formats are documents). Pure.
 *
 * @param mediaType The media type of a file.
 * @returns The class, or undefined when none of the classes names it.
 */
export function typeClassOf(mediaType: string): TypeClass | undefined {
  const type = mediaType.toLowerCase();
  const documents = /^application\/(pdf|rtf|msword|vnd\.(ms-|openxmlformats|oasis\.opendocument))/;
  if (documents.test(type) || type === "text/rtf") return "document";
  for (const prefix of ["image", "video", "audio", "text"] as const) {
    if (type.startsWith(`${prefix}/`)) return prefix;
  }
  return undefined;
}

/** Words that name a type class, matched as prefixes of the last word typed. */
const TYPE_WORDS: Readonly<Record<TypeClass, readonly string[]>> = {
  document: ["document", "doc", "pdf", "presentation", "slides", "spreadsheet", "sheet", "report"],
  image: ["image", "photo", "picture", "pic", "screenshot"],
  video: ["video", "movie", "film", "clip"],
  audio: ["audio", "music", "song", "sound", "recording"],
  text: ["text", "txt", "note"],
  folder: ["folder", "collection", "directory"],
};

/** The classes a typed word names, by prefix of one of their words. */
function namedClasses(text: string): TypeClass[] {
  const word = text.trim().toLowerCase().split(/\s+/).at(-1) ?? "";
  if (word.length < 2) return [];
  return TYPE_CLASSES.filter((cls) => TYPE_WORDS[cls].some((name) => name.startsWith(word)));
}

/** The classes of the files found, most common first. */
function foundClasses(mediaTypes: readonly string[]): TypeClass[] {
  const counts = new Map<TypeClass, number>();
  for (const mediaType of mediaTypes.slice(0, 1000)) {
    const cls = typeClassOf(mediaType);
    if (cls) counts.set(cls, (counts.get(cls) ?? 0) + 1);
  }
  return [...counts.entries()].sort((a, b) => b[1] - a[1]).map(([cls]) => cls);
}

/**
 * The Type chips a typed text brings: the classes its last word names ("pres" suggests
 * documents, "photo" images), else the classes of the files the live search found. At most two.
 * Pure.
 *
 * @param text The free text typed.
 * @param mediaTypes The media types of the files found.
 * @returns The classes, best first.
 */
export function typeSuggestions(text: string, mediaTypes: readonly string[]): TypeClass[] {
  const named = namedClasses(text);
  return (named.length > 0 ? named : foundClasses(mediaTypes)).slice(0, 2);
}

/** The label key of a type class's name, for rows that word the class. */
export function typeLabelKey(cls: TypeClass): LabelKey {
  return TYPE_LABEL[cls];
}

/** The order of a search: relevance, or a view sort key with its direction. */
export interface SearchOrder {
  /** Relevance, or the view sort key. */
  readonly sort: SearchSort;
  /** The direction. */
  readonly direction: SortDirection;
}

/**
 * The order a click on a column head asks for in search results: another column starts in its
 * default direction, the same column reverses, and a third click returns to relevance. Pure.
 *
 * @param current The order now.
 * @param clicked The column clicked.
 * @returns The new order.
 */
export function nextSearchSort(current: SearchOrder, clicked: SortKey): SearchOrder {
  const first = defaultDirection(clicked);
  if (current.sort !== clicked) return { sort: clicked, direction: first };
  if (current.direction !== first) return { sort: "relevance", direction: "descending" };
  return { sort: clicked, direction: first === "ascending" ? "descending" : "ascending" };
}
