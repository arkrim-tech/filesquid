/**
 * A fake of the reads and the restore the file panel uses (UI-F2.3), answering like the routes
 * of API-F1.3 and API-F2.5 for one file of the mockups: `harbour-masterplan-v7.pdf` with seven
 * versions, the newest labelled "Issued for tender". Each method can be made to fail or to wait
 * for a gate, and every restore is recorded.
 */
import { ApiError } from "../../src/api/client";
import { validLabel } from "../../src/files/model";
import type { FileCommit, FileDetails, FileRef, ObjectRef } from "../../src/files/model";
import type {
  FileHistory,
  FileHistoryEntry,
  FileHistorySize,
  ObjectVersions,
} from "../../src/files/history-source";
import type { RestoreOptions, SourceOptions, WriteOptions } from "../../src/files/source";

/** The object of the fixture file. */
export const PANEL_OBJECT = "urn:uuid:7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8";

/** The fixture file at its head. */
export const PANEL_FILE: FileRef = {
  library_id: "lib",
  object_id: PANEL_OBJECT,
  version: "v7",
  path: "original/9f2c41aa.pdf",
  display_name: "harbour-masterplan-v7.pdf",
};

/** Label, ISO time and size of each version, oldest first. */
const VERSIONS: readonly [string | null, string, number][] = [
  [null, "2026-09-27T10:00:00Z", 8_703_180],
  ["First draft", "2026-09-28T10:00:00Z", 10_066_330],
  [null, "2026-10-02T10:00:00Z", 12_373_197],
  [null, "2026-10-07T10:00:00Z", 14_889_779],
  ["Client review", "2026-10-09T10:00:00Z", 18_769_510],
  [null, "2026-10-15T10:00:00Z", 18_979_225],
  ["Issued for tender", "2026-10-16T12:00:00Z", 19_293_798],
];

/** The history entries of the fixture, oldest first. */
export function panelEntries(): FileHistoryEntry[] {
  return VERSIONS.map(([label, created, size], i) => ({
    version: {
      version: `v${i + 1}`,
      created,
      message: null,
      user: { name: "Owner", address: null },
      label,
    },
    change: i === 0 ? "added" : "changed",
    digest: `digest-${i + 1}`,
    size,
  }));
}

/** A gate a fake call waits on until the test opens it. */
export interface Gate {
  /** Promise the call awaits. */
  readonly wait: Promise<void>;
  /** Let the call go on. */
  open(): void;
}

/** A promise that settles when `signal` aborts; never without one. */
function aborted(signal?: AbortSignal): Promise<void> {
  if (!signal) return new Promise<void>(() => undefined);
  if (signal.aborted) return Promise.resolve();
  return new Promise<void>((resolve) =>
    signal.addEventListener("abort", () => resolve(), { once: true }),
  );
}

/** Make a gate. */
export function gate(): Gate {
  let open: () => void = () => undefined;
  const wait = new Promise<void>((resolve) => {
    open = resolve;
  });
  return { wait, open: () => open() };
}

/** The methods a fake call can fail or wait on. */
type StepName = "details" | "history" | "versions" | "size" | "restore" | "label" | "download";

/** The panel's reads, restore, label edit and download over the fixture. */
export class FakePanelSource {
  entries: FileHistoryEntry[] = panelEntries();
  head = "v7";
  bytes = 100_978_892;
  /** Failure of each method, when set. */
  readonly failures: Partial<Record<StepName, unknown>> = {};
  /** Gates each method waits on, when set. */
  readonly gates: Partial<Record<StepName, Gate>> = {};
  /** Every label edit, in order, with the revision it expected. */
  readonly labelEdits: {
    version: string;
    label: string | null;
    revision: string;
  }[] = [];
  /** Every download, in order: the file version asked for. */
  readonly downloads: FileRef[] = [];
  /** Every restore, in order, with the revision it expected. */
  readonly restores: {
    object: ObjectRef;
    version: string;
    paths: readonly string[];
    revision: string;
    label?: string;
  }[] = [];
  /** Calls per method. */
  readonly calls: Record<StepName, number> = {
    details: 0,
    history: 0,
    versions: 0,
    size: 0,
    restore: 0,
    label: 0,
    download: 0,
  };
  /** When true, calls ignore their abort signal, as a host that answers anyway would. */
  ignoreAbort = false;

  /**
   * Wait for the method's gate (an abort ends the wait at once, unless `ignoreAbort`), then
   * fail when a failure is set.
   */
  private async step(name: StepName, options?: SourceOptions): Promise<void> {
    this.calls[name] += 1;
    const signal = this.ignoreAbort ? undefined : options?.signal;
    await Promise.race([this.gates[name]?.wait, aborted(signal)]);
    if (signal?.aborted) throw new DOMException("aborted", "AbortError");
    if (this.failures[name] !== undefined) throw this.failures[name];
  }

  /** Storage facts of the fixture file at `file.version`. */
  async details(file: FileRef, options?: SourceOptions): Promise<FileDetails> {
    await this.step("details", options);
    const index = Number(file.version.slice(1)) - 1;
    const found = this.entries[index];
    if (!found) throw new ApiError(404, "/api/v1/files/details", { code: "not_found" });
    return {
      library_id: file.library_id,
      object_id: file.object_id,
      version: file.version,
      path: file.path,
      name: "9f2c41aa.pdf",
      display_name: PANEL_FILE.display_name,
      digest: "5a".repeat(64),
      digest_algorithm: "sha512",
      size: found.size ?? 0,
      fixity: { sha256: [`9b1f4e${"0".repeat(52)}c07a2d`] },
      last_change: found.version,
      type_hint: "pdf",
      media_type: "application/pdf",
    };
  }

  /** The file's history, oldest first. */
  async history(file: FileRef, options?: SourceOptions): Promise<FileHistory> {
    await this.step("history", options);
    return {
      library_id: file.library_id,
      object_id: file.object_id,
      path: file.path,
      display_name: PANEL_FILE.display_name,
      entries: this.entries,
    };
  }

  /** Every version of the object. */
  async versions(object: ObjectRef, options?: SourceOptions): Promise<ObjectVersions> {
    await this.step("versions", options);
    return {
      library_id: object.library_id,
      object_id: object.object_id,
      head: this.head,
      display_name: PANEL_FILE.display_name,
      versions: this.entries.map((e) => e.version),
    };
  }

  /** The bytes the history keeps. */
  async historySize(file: FileRef, options?: SourceOptions): Promise<FileHistorySize> {
    await this.step("size", options);
    const contents = new Set(this.entries.map((e) => e.digest).filter((d) => d !== null));
    return {
      library_id: file.library_id,
      object_id: file.object_id,
      path: file.path,
      versions: this.entries.length,
      contents: contents.size,
      bytes: this.bytes,
    };
  }

  /**
   * Record a restore and publish the next version with the restored content. As the API does,
   * a revision that is not the head is refused as `conflict` (API-F2.13), and a version or path
   * the history does not hold as `not_found`.
   */
  async restore(
    object: ObjectRef,
    version: string,
    paths: readonly string[],
    options: RestoreOptions,
  ): Promise<FileCommit> {
    await this.step("restore", options);
    const route = "/api/v1/files/restorations";
    if (options.revision !== this.head) throw new ApiError(409, route, { code: "conflict" });
    const held = this.entries.some((e) => e.version.version === version && e.digest !== null);
    if (!held || paths.some((path) => path !== PANEL_FILE.path)) {
      throw new ApiError(404, route, { code: "not_found" });
    }
    this.restores.push({
      object,
      version,
      paths,
      revision: options.revision,
      label: options.label,
    });
    const source = this.entries.find((e) => e.version.version === version);
    const next = `v${Number(this.head.slice(1)) + 1}`;
    const created = "2026-10-16T13:00:00Z";
    this.entries = [
      ...this.entries,
      {
        version: {
          version: next,
          created,
          message: null,
          user: null,
          label: options.label ?? null,
        },
        change: "changed",
        digest: source?.digest ?? null,
        size: source?.size ?? null,
      },
    ];
    this.head = next;
    return { library_id: object.library_id, object_id: object.object_id, version: next, created };
  }
  /**
   * Record a label edit as `PUT /api/v1/files/labels` answers it: an invalid label is
   * `invalid_request`, a revision that is not the head `conflict`, an unknown version
   * `not_found`, and a label that changes nothing `conflict`. Otherwise the version's label
   * changes and the labels record is committed as the next version.
   */
  async setVersionLabel(
    object: ObjectRef,
    version: string,
    label: string | null,
    options: WriteOptions,
  ): Promise<FileCommit> {
    await this.step("label", options);
    const route = "/api/v1/files/labels";
    if (label !== null && !validLabel(label)) {
      throw new ApiError(400, route, { code: "invalid_request" });
    }
    if (options.revision !== this.head) throw new ApiError(409, route, { code: "conflict" });
    const at = this.entries.findIndex((e) => e.version.version === version);
    const entry = this.entries[at];
    if (!entry) throw new ApiError(404, route, { code: "not_found" });
    if ((entry.version.label ?? null) === label) {
      throw new ApiError(409, route, { code: "conflict" });
    }
    this.labelEdits.push({ version, label, revision: options.revision });
    this.entries = this.entries.map((e, i) =>
      i === at ? { ...e, version: { ...e.version, label } } : e,
    );
    const next = `v${Number(this.head.slice(1)) + 1}`;
    this.head = next;
    const created = "2026-10-16T13:00:00Z";
    return { library_id: object.library_id, object_id: object.object_id, version: next, created };
  }

  /** The whole file of the version asked for; a version that removed the file is `not_found`. */
  async readVersionFile(file: FileRef, options?: SourceOptions): Promise<Blob> {
    await this.step("download", options);
    this.downloads.push(file);
    const entry = this.entries.find((e) => e.version.version === file.version);
    if (!entry || entry.digest === null) {
      throw new ApiError(404, "/api/v1/files/download", { code: "not_found" });
    }
    return new Blob([`bytes of ${file.version}`]);
  }
}
