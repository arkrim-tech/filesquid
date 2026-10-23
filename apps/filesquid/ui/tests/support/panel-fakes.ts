/**
 * A fake of the reads and the restore the file panel uses (UI-F2.3), answering like the routes
 * of API-F1.3 and API-F2.5 for one file of the mockups: `harbour-masterplan-v7.pdf` with seven
 * versions, the newest labelled "Issued for tender". Each method can be made to fail or to wait
 * for a gate, and every restore is recorded.
 */
import { ApiError } from "../../src/api/client";
import type { FileCommit, FileDetails, FileRef, ObjectRef } from "../../src/files/model";
import type {
  FileHistory,
  FileHistoryEntry,
  FileHistorySize,
  ObjectVersions,
} from "../../src/files/history-source";
import type { RestoreOptions, SourceOptions } from "../../src/files/source";

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

/** Make a gate. */
export function gate(): Gate {
  let open: () => void = () => undefined;
  const wait = new Promise<void>((resolve) => {
    open = resolve;
  });
  return { wait, open: () => open() };
}

/** The panel's reads and restore over the fixture. */
export class FakePanelSource {
  entries: FileHistoryEntry[] = panelEntries();
  head = "v7";
  bytes = 100_978_892;
  /** Failure of each method, when set. */
  readonly failures: Partial<
    Record<"details" | "history" | "versions" | "size" | "restore", unknown>
  > = {};
  /** Gates each method waits on, when set. */
  readonly gates: Partial<Record<"details" | "history" | "versions" | "size" | "restore", Gate>> =
    {};
  /** Every restore, in order. */
  readonly restores: {
    object: ObjectRef;
    version: string;
    paths: readonly string[];
    label?: string;
  }[] = [];
  /** Calls per method. */
  readonly calls = { details: 0, history: 0, versions: 0, size: 0, restore: 0 };

  /** Wait for the method's gate, then fail when a failure is set. */
  private async step(name: keyof FakePanelSource["calls"], options?: SourceOptions): Promise<void> {
    this.calls[name] += 1;
    await this.gates[name]?.wait;
    if (options?.signal?.aborted) throw new DOMException("aborted", "AbortError");
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
    return {
      library_id: file.library_id,
      object_id: file.object_id,
      path: file.path,
      versions: this.entries.length,
      contents: this.entries.length,
      bytes: this.bytes,
    };
  }

  /** Record a restore and publish the next version with the restored content. */
  async restore(
    object: ObjectRef,
    version: string,
    paths: readonly string[],
    options: RestoreOptions = {},
  ): Promise<FileCommit> {
    await this.step("restore", options);
    this.restores.push({ object, version, paths, label: options.label });
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
}
