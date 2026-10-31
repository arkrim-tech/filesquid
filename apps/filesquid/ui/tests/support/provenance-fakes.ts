/**
 * Fakes of the sources the File info and Provenance tabs read (UI-F2.15): the history of one
 * file as `GET /files/{id}/history` answers it (oldest first, in pages), the JSON-LD record of
 * each version, the JSON-LD export and the facts projection, answering the data of mockups 35
 * and 36 (`facade-study.jpg`, four versions, the head v4 a metadata edit). Each call is
 * recorded; the history can be made to fail or to wait (every call, or only the next one), and
 * record, export and facts reads can be made to fail.
 */
import { ApiError } from "../../src/api/client";
import type { FactsSource, FileFacts } from "../../src/files/facts-source";
import type { HistoryEntry, HistoryPage } from "../../src/files/provenance";
import type { ExportedHistory, ProvenanceSource } from "../../src/files/provenance-source";
import type { FileRef } from "../../src/files/model";
import type { SourceOptions } from "../../src/files/source";

/** The object of the fixture file. */
export const PROV_OBJECT = "urn:uuid:7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8";
/** The installation agent of mockup 36. */
export const PROV_INSTALLATION = "urn:uuid:5a0c41e2-1111-4222-8333-44445555e19b";
/** The host software agent of mockup 36. */
export const PROV_HOST = "urn:filesquid:software:FileSquid:desktop:0.1.0";
/** The whole SHA-256 of the fixture's entities. */
export const PROV_SHA = `9b1f4e${"0".repeat(52)}c07a2d`;

/** The fixture file at its head. */
export const PROV_FILE: FileRef = {
  library_id: "lib",
  object_id: PROV_OBJECT,
  version: "v4",
  path: `original/${PROV_OBJECT.slice("urn:uuid:".length)}.jpg`,
  display_name: "facade-study.jpg",
};

/** Kind and ISO time of each version of the fixture, oldest first (mockup 36). */
const STEPS: readonly (readonly [HistoryEntry["kind"], string])[] = [
  ["import", "2026-10-12T14:20:00Z"],
  ["delete", "2026-10-13T09:12:00Z"],
  ["revert", "2026-10-13T09:40:00Z"],
  ["metadata.edit", "2026-10-16T11:05:02Z"],
];

/** The history entries of the fixture, oldest first. */
export function provEntries(): HistoryEntry[] {
  return STEPS.map(([kind, time], i) => ({
    kind,
    operation_id: `op-${i + 1}`,
    version: { object_id: PROV_OBJECT, version: `v${i + 1}` },
    agents: [PROV_INSTALLATION, PROV_HOST],
    started_at: time,
    ended_at: time,
    journal: { operation_id: `op-${i + 1}`, journal_entry_ids: [String(i + 1)] },
    label: null,
    reconstructed: false,
  }));
}

/** The `filesquid-prov/1` record of version `n`: it used v(n-1) and generated and derived v(n). */
export function provRecord(n: number): unknown {
  const path = "Harbour project/facade-study.jpg";
  const entity = (v: number) =>
    `urn:filesquid:entity:${PROV_OBJECT}:v${v}:${encodeURIComponent(path)}`;
  const activity = `urn:filesquid:activity:op-${n}:${PROV_OBJECT}:v${n}`;
  const node = (v: number, extra: object = {}) => ({
    "@id": entity(v),
    "@type": "prov:Entity",
    "fsq:objectId": PROV_OBJECT,
    "fsq:version": `v${v}`,
    "fsq:path": path,
    "fsq:sha256": PROV_SHA,
    ...extra,
  });
  const used = n > 1 ? [{ "@id": entity(n - 1) }] : [];
  return {
    "@context": {},
    "fsq:profile": "filesquid-prov/1",
    "@graph": [
      {
        "@id": activity,
        "@type": "prov:Activity",
        "fsq:operation": `op-${n}`,
        "fsq:kind": STEPS[n - 1]?.[0],
        "prov:used": used,
      },
      ...(n > 1 ? [node(n - 1)] : []),
      node(n, {
        "prov:wasGeneratedBy": { "@id": activity },
        ...(n > 1 ? { "prov:wasDerivedFrom": [{ "@id": entity(n - 1) }] } : {}),
      }),
    ],
  };
}

/** A gate a fake call waits on until the test opens it. */
export interface ProvGate {
  /** Promise the call awaits. */
  readonly wait: Promise<void>;
  /** Let the call go on. */
  open(): void;
}

/** Make a gate. */
export function provGate(): ProvGate {
  let open: () => void = () => undefined;
  const wait = new Promise<void>((resolve) => {
    open = resolve;
  });
  return { wait, open };
}

/** A fake {@link ProvenanceSource}. */
export class FakeProvenanceSource implements ProvenanceSource {
  /** The entries `history` answers, oldest first. */
  entries: HistoryEntry[] = provEntries();
  /** The page size the fake uses; a smaller one makes the page cursor visible. */
  pageSize = 100;
  /** Whether the index says it left activities out. */
  truncated = false;
  /** The error `history` rejects with, when set. */
  historyError?: unknown;
  /** The versions whose record read rejects with an ApiError 500. */
  failingRecords = new Set<string>();
  /** The record each version answers; the fixture's by default. */
  records = new Map<string, unknown>();
  /** A gate `history` waits on, when set. */
  historyGate?: ProvGate;
  /** A gate only the next `history` call waits on; the call that takes it clears it. */
  historyGateOnce?: ProvGate;
  /** The entries of a file other than the fixture's, by object ID (`entries` for any other). */
  entriesByObject = new Map<string, HistoryEntry[]>();
  /** The error `exportHistory` rejects with, when set. */
  exportError?: unknown;
  /** Whether `exportHistory` reports a truncated export. */
  exportTruncated = false;
  /** A gate `exportHistory` waits on, when set. */
  exportGate?: ProvGate;
  /** Every history call's cursor (undefined for the first page). */
  readonly historyCalls: (string | undefined)[] = [];
  /** Every record read's version. */
  readonly recordCalls: string[] = [];
  /** Every export call. */
  exports = 0;

  async history(
    file: FileRef,
    options: SourceOptions & { cursor?: string } = {},
  ): Promise<HistoryPage> {
    this.historyCalls.push(options.cursor);
    const once = this.historyGateOnce;
    this.historyGateOnce = undefined;
    await once?.wait;
    await this.historyGate?.wait;
    if (this.historyError) throw this.historyError;
    // The cursor is an opaque token to the client; the fake maps it back to an offset.
    const start = options.cursor ? Number(/^page-(\d+)$/.exec(options.cursor)?.[1]) : 0;
    const all = this.entriesByObject.get(file.object_id) ?? this.entries;
    const items = all.slice(start, start + this.pageSize);
    const more = start + this.pageSize < all.length;
    const next = more ? `page-${start + this.pageSize}` : null;
    return { items, next_cursor: next, truncated: this.truncated };
  }

  async record(file: FileRef, version: string): Promise<unknown> {
    void file;
    this.recordCalls.push(version);
    if (this.failingRecords.has(version)) {
      throw new ApiError(500, `/api/v1/files/x/versions/${version}/prov`, { code: "internal" });
    }
    return this.records.get(version) ?? provRecord(Number(version.slice(1)));
  }

  async exportHistory(file: FileRef): Promise<ExportedHistory> {
    void file;
    this.exports += 1;
    await this.exportGate?.wait;
    if (this.exportError) throw this.exportError;
    const blob = new Blob(['[{"@id":"x"}]'], { type: "application/ld+json" });
    return { blob, truncated: this.exportTruncated };
  }
}

/** The facts of mockup 35 for the fixture file. */
export function provFacts(over: Partial<FileFacts> = {}): FileFacts {
  return {
    original_name: "facade-study.jpg",
    display_name: "facade-study.jpg",
    extension: "jpg",
    format: "JPEG image",
    media_type: "image/jpeg",
    size: 6_481_920,
    sha256: PROV_SHA,
    imports: [
      {
        imported_at: "2026-10-12T14:20:00Z",
        method: "folder",
        storage: "copied",
        relative_path: null,
        volume_name: "FUJIFILM",
        object_id: PROV_OBJECT,
        object_version: "v1",
      },
    ],
    ...over,
  } as FileFacts;
}

/** A fake {@link FactsSource}. */
export class FakeFactsSource implements FactsSource {
  /** What `facts` answers. */
  answer: FileFacts = provFacts();
  /** The error `facts` rejects with, when set. */
  error?: unknown;
  /** Every call's file. */
  readonly calls: FileRef[] = [];

  async facts(
    file: FileRef,
  ): Promise<{ library_id: string; file_id: string; version: string; facts: FileFacts }> {
    this.calls.push(file);
    if (this.error) throw this.error;
    const file_id = file.object_id.slice("urn:uuid:".length);
    return { library_id: file.library_id, file_id, version: file.version, facts: this.answer };
  }
}
