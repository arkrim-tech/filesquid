/**
 * Fakes of the Logs view's reads (UI-F2.16; API-S-031): the sixteen entries of mockup 38 (their
 * times are 13:05:42 down to 08:55:09 on 19 October 2026 in UTC+2, the zone of the mockup's
 * footer), a pure filter that follows the journal route's tests (time range, library, operation,
 * outcome, level, file, object, operation ID, cursor and order) and a {@link MemoryLogsSource}
 * that answers {@link LogsSource} from them. The journal holds opaque IDs: only the rows that name
 * a file carry a display name, as `names=true` returns it. Test code only.
 */
import type { JournalEntryView, JournalFilters, JournalViewPage } from "../../src/api/client";
import type { LogsEventListener, LogsSource } from "../../src/logs/source";
import type { LogsLibrary } from "../../src/logs/model";

/** The library of the fixture. */
export const LIBRARY = "7c9e6679-7425-40de-944b-e07fc1f90ae7";
/** The installation's agent. */
export const ACTOR = "urn:uuid:5a0c41e2-0000-4000-8000-00000000e19b";
/** The file of the selected row (harbour-masterplan-v7.pdf) and its operation. */
export const PLAN_FILE = "1d0e88b4-0000-4000-8000-00000000a14c";
export const PLAN_OPERATION = "3f9c2a70-5be1-4c0d-9e44-81d7a2c90e21";
/** The file and operation of IMG_4102.jpg. */
export const IMG_FILE = "9a2f5c18-0000-4000-8000-00000000b7e3";
export const IMG_OPERATION = "a71c0d44-8b21-4f6e-b3a9-5e7d1c243e90";
/** The libraries the host opened. */
export const LIBRARIES: readonly LogsLibrary[] = [{ library_id: LIBRARY, name: "Family archive" }];

/** The clock of the tests: 13:10 on 19 October 2026 in UTC+2. */
export const NOW = new Date("2026-10-19T11:10:00.000Z");

/** One entry's fixed fields. */
interface Spec {
  readonly local: string;
  readonly level: JournalEntryView["level"];
  readonly operation: string;
  readonly outcome: JournalEntryView["outcome"];
  readonly name?: string;
  readonly target?: string;
  readonly operationId?: string;
  readonly file?: string;
  readonly version?: number;
  readonly journalOnly?: boolean;
  readonly duration?: number;
  readonly errorCode?: string;
}

/** The mockup's rows, newest first. */
const SPECS: readonly Spec[] = [
  {
    local: "13:05:42",
    level: "info",
    operation: "ocfl.commit",
    outcome: "succeeded",
    name: "IMG_4102.jpg",
    operationId: IMG_OPERATION,
    file: IMG_FILE,
    version: 1,
  },
  {
    local: "13:05:41",
    level: "info",
    operation: "import.item",
    outcome: "succeeded",
    name: "IMG_4102.jpg",
    operationId: IMG_OPERATION,
    file: IMG_FILE,
    version: 1,
  },
  {
    local: "13:05:12",
    level: "warn",
    operation: "process.stalled",
    outcome: "info",
    target: "proc-preview-2",
    journalOnly: true,
  },
  {
    local: "13:04:58",
    level: "error",
    operation: "import.item",
    outcome: "failed",
    name: "IMG_4099.jpg",
    operationId: IMG_OPERATION,
    journalOnly: true,
    errorCode: "unreadable",
  },
  {
    local: "13:04:20",
    level: "info",
    operation: "job.start",
    outcome: "succeeded",
    target: "job-upload-eos",
    operationId: IMG_OPERATION,
  },
  {
    local: "13:03:02",
    level: "info",
    operation: "metadata.set",
    outcome: "succeeded",
    name: "harbour-masterplan-v7.pdf",
    operationId: PLAN_OPERATION,
    file: PLAN_FILE,
    version: 8,
    duration: 38,
  },
  {
    local: "13:01:47",
    level: "warn",
    operation: "api.auth.denied",
    outcome: "failed",
    name: "survey-points.csv",
    operationId: "1b7e44d2-6c05-4a8e-9b13-2f0c5d7609d4",
    journalOnly: true,
    errorCode: "not_permitted",
  },
  {
    local: "12:58:47",
    level: "info",
    operation: "version.restore",
    outcome: "succeeded",
    name: "harbour-masterplan-v7.pdf",
    operationId: "64d0a1f3-77b2-4c19-8e5a-0d3b9c1e2b7a",
    file: PLAN_FILE,
    version: 7,
  },
  {
    local: "12:41:15",
    level: "info",
    operation: "fixity.check",
    outcome: "succeeded",
    target: LIBRARY,
    operationId: "8a22c7d1-0e4f-4b6a-a9d3-7c15e2b6f310",
  },
  {
    local: "12:40:00",
    level: "info",
    operation: "app.start",
    outcome: "succeeded",
    target: "installation",
  },
  {
    local: "12:39:58",
    level: "info",
    operation: "app.stop",
    outcome: "succeeded",
    target: "installation",
  },
  {
    local: "09:17:30",
    level: "info",
    operation: "file.rename",
    outcome: "succeeded",
    name: "meeting-notes-14-oct.docx",
    operationId: "5c10b7e9-3a62-4d8f-8f01-c4a97e5da8f3",
  },
  {
    local: "09:16:02",
    level: "info",
    operation: "file.move",
    outcome: "succeeded",
    name: "tender-response.pdf",
    operationId: "e2b9f04c-d1a7-4e35-b620-9a8c3f7d4d06",
  },
  {
    local: "09:12:44",
    level: "info",
    operation: "export.zip",
    outcome: "succeeded",
    target: "export-4-files",
    operationId: "77aa31e8-c5b0-4f92-a6d4-1e8f0b2dc1d5",
  },
  {
    local: "08:55:10",
    level: "error",
    operation: "preview.render",
    outcome: "failed",
    name: "model-export.blend",
    operationId: "0f4a9c36-2d81-47b5-9e70-b3a6c8d577b2",
    journalOnly: true,
    errorCode: "unsupported",
  },
  {
    local: "08:55:09",
    level: "info",
    operation: "ocfl.commit",
    outcome: "succeeded",
    name: "site-walkthrough.mp4",
    operationId: "9d41e5a7-6b38-4c20-8f94-d02a1c3e7a0c",
    file: "c3d4e5f6-0000-4000-8000-00000000ff01",
    version: 1,
  },
];

/** The id the newest fixture entry has. */
export const NEWEST_ID = 112;

/** The UTC time of a local time of day on 19 October 2026 in UTC+2. */
function utc(local: string): string {
  const [h = "0", m = "0", s = "0"] = local.split(":");
  const at = new Date(Date.UTC(2026, 9, 19, Number(h) - 2, Number(m), Number(s), 418));
  return at.toISOString();
}

/** The entry of one spec. */
function entryOf(spec: Spec, id: number): JournalEntryView {
  const object = spec.file ? `urn:uuid:${spec.file}` : undefined;
  const targets = [LIBRARY, ...(object ? [object] : []), ...(spec.target ? [spec.target] : [])];
  const reference = object && spec.version ? `${object}@v${spec.version}` : undefined;
  return {
    id,
    time: utc(spec.local),
    host: "desktop",
    actor: ACTOR,
    operation: spec.operation,
    outcome: spec.outcome,
    level: spec.level,
    target_ids: spec.target === "installation" ? [spec.target] : targets,
    ...(spec.duration === undefined ? {} : { duration_ms: spec.duration }),
    ...(spec.errorCode === undefined ? {} : { error_code: spec.errorCode }),
    ...(spec.operationId === undefined ? {} : { operation_id: spec.operationId }),
    ...(reference === undefined ? {} : { object_ref: reference }),
    provenance: {
      journal_only: spec.journalOnly === true || spec.operationId === undefined,
      ...(spec.journalOnly || spec.operationId === undefined || !reference
        ? {}
        : { operation_id: spec.operationId, object_ref: reference }),
    },
    ...(spec.name === undefined ? {} : { display_name: spec.name }),
  };
}

/**
 * The entries of mockup 38, newest first, ids {@link NEWEST_ID} down. Pure.
 *
 * @returns Sixteen entries.
 */
export function mockupEntries(): JournalEntryView[] {
  return SPECS.map((spec, index) => entryOf(spec, NEWEST_ID - index));
}

/**
 * Older entries for paging tests: `count` entries of the plain `info` kind, older than the mockup's
 * rows, ids below the mockup's. Pure.
 *
 * @param count How many.
 */
export function olderEntries(count: number): JournalEntryView[] {
  const oldest = NEWEST_ID - SPECS.length;
  return Array.from({ length: count }, (_, i) => ({
    id: oldest - i,
    time: new Date(Date.UTC(2026, 9, 18, 20, 0, 0) - i * 60_000).toISOString(),
    host: "desktop",
    actor: ACTOR,
    operation: "ocfl.commit",
    outcome: "succeeded" as const,
    level: "info" as const,
    target_ids: [LIBRARY],
    operation_id: `00000000-0000-4000-8000-${String(i).padStart(12, "0")}`,
    provenance: { journal_only: true },
  }));
}

/** The file ID of an object or file filter value. */
function fileOf(value: string): string {
  return value.replace(/^urn:uuid:/, "").toLowerCase();
}

/** True when the entry names the file among its targets or its object. */
function namesFile(entry: JournalEntryView, file: string): boolean {
  const wanted = fileOf(file);
  const ids = [...entry.target_ids, entry.object_ref?.split("@v", 1)[0] ?? ""];
  return ids.some((id) => fileOf(id) === wanted);
}

/** True when the entry satisfies the filters of the journal route (without paging). */
export function matchesRoute(entry: JournalEntryView, f: JournalFilters): boolean {
  if (f.from !== undefined && entry.time < f.from) return false;
  if (f.to !== undefined && entry.time > f.to) return false;
  if (f.operation !== undefined && entry.operation !== f.operation) return false;
  if (f.outcome !== undefined && entry.outcome !== f.outcome) return false;
  if (f.level !== undefined && entry.level !== f.level) return false;
  if (f.library !== undefined && !entry.target_ids.includes(f.library)) return false;
  if (f.file !== undefined && !namesFile(entry, f.file)) return false;
  if (f.object !== undefined && !namesFile(entry, f.object)) return false;
  return f.operation_id === undefined || entry.operation_id === f.operation_id;
}

/**
 * The page the journal route answers: filters, order, cursor and limit (default 100). Pure.
 *
 * @param all Every entry, in any order.
 * @param f The query.
 */
export function journalPage(all: readonly JournalEntryView[], f: JournalFilters): JournalViewPage {
  const desc = f.order === "desc";
  const sorted = all.filter((e) => matchesRoute(e, f)).sort((a, b) => (a.id ?? 0) - (b.id ?? 0));
  const ordered = desc ? sorted.reverse() : sorted;
  const afterId = f.after;
  const rest =
    afterId === undefined
      ? ordered
      : ordered.filter((e) => (desc ? (e.id ?? 0) < afterId : (e.id ?? 0) > afterId));
  const limit = f.limit ?? 100;
  const entries = rest.slice(0, limit).map((e) => (f.names ? e : withoutName(e)));
  const more = rest.length > limit;
  return { entries, next_cursor: more ? (entries[entries.length - 1]?.id ?? null) : null };
}

/** An entry without its display name. */
function withoutName(entry: JournalEntryView): JournalEntryView {
  const copy = { ...entry };
  delete copy.display_name;
  return copy;
}

/** The JSON Lines of the entries the filters select, oldest first, opaque IDs only. */
export function exportJsonl(all: readonly JournalEntryView[], f: JournalFilters): string {
  const page = journalPage(all, { ...f, order: "asc", limit: 50_000, names: false });
  return (
    page.entries.map((entry) => JSON.stringify(entry)).join("\n") +
    (page.entries.length ? "\n" : "")
  );
}

/** A {@link LogsSource} over entries held in memory, recording every call. */
export class MemoryLogsSource implements LogsSource {
  /** The journal, in any order. */
  entries: JournalEntryView[];
  /** The libraries the host opened. */
  known: LogsLibrary[] = [...LIBRARIES];
  /** Every `page` query. */
  readonly pages: JournalFilters[] = [];
  /** Every `export` query. */
  readonly exports: Omit<JournalFilters, "after" | "limit" | "names">[] = [];
  /** Every `entry` id asked for. */
  readonly entryReads: number[] = [];
  /** When set, `page` rejects with it. */
  pageError?: Error;
  /** When set, `export` rejects with it. */
  exportError?: Error;
  /** When set, `entry` rejects with it. */
  entryError?: Error;
  private listener?: LogsEventListener;
  private open?: () => void;
  /** True while a listener is subscribed. */
  subscribed = false;

  /** @param entries The journal. */
  constructor(entries: JournalEntryView[] = mockupEntries()) {
    this.entries = entries;
  }

  async page(filters: JournalFilters): Promise<JournalViewPage> {
    this.pages.push(filters);
    if (this.pageError) throw this.pageError;
    return journalPage(this.entries, filters);
  }

  async entry(id: number): Promise<JournalEntryView> {
    this.entryReads.push(id);
    if (this.entryError) throw this.entryError;
    const found = this.entries.find((e) => e.id === id);
    if (!found) throw new Error("no such entry");
    return found;
  }

  async export(filters: Omit<JournalFilters, "after" | "limit" | "names">): Promise<string> {
    this.exports.push(filters);
    if (this.exportError) throw this.exportError;
    return exportJsonl(this.entries, filters);
  }

  async oldest(): Promise<JournalEntryView | undefined> {
    return [...this.entries].sort((a, b) => a.time.localeCompare(b.time))[0];
  }

  async libraries(): Promise<LogsLibrary[]> {
    return this.known;
  }

  subscribe(listener: LogsEventListener, onOpen?: () => void): () => void {
    this.listener = listener;
    this.open = onOpen;
    this.subscribed = true;
    return () => {
      this.listener = undefined;
      this.subscribed = false;
    };
  }

  /** Deliver a live entry as a `journal.entry` event. */
  emit(entry: JournalEntryView): void {
    const { provenance: _provenance, display_name: _name, ...live } = entry;
    void _provenance;
    void _name;
    this.listener?.({ topic: "journal.entry", data: live } as never);
  }

  /** Deliver an event of another topic. */
  emitTopic(topic: string): void {
    this.listener?.({ topic, data: {} } as never);
  }

  /** The channel (re)opened. */
  reopen(): void {
    this.open?.();
  }
}
