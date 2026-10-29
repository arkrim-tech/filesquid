/**
 * A fake of the metadata reads and writes the Description tab uses (UI-F2.4), answering like
 * the metadata property routes of API-F1.4 (API-S-035) for the file of mockup 28,
 * `facade-study.jpg`: eight `dcterms:` and `schema:` statements, one of them extracted, and one
 * `exif:` statement the tab does not list. Each method can be made to fail or to wait for a
 * gate, every set and removal is recorded, and the registry holds what a test puts in it.
 */
import { ApiError } from "../../src/api/client";
import type { FileRef, LibraryRef } from "../../src/files/model";
import { DCTERMS_FILE_TERMS, validTermName } from "../../src/files/metadata";
import type {
  FileStatements,
  MetadataCommit,
  MetadataStatement,
  NamespaceRegistration,
  Namespaces,
  TermCandidate,
  TermCandidates,
} from "../../src/files/metadata";
import type { MetadataSource } from "../../src/files/metadata-source";
import type { SourceOptions } from "../../src/files/source";
import { gate, type Gate } from "./panel-fakes";

/** The object of the fixture file. */
export const METADATA_OBJECT = "urn:uuid:7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8";

/** The fixture file at its head, mockup 28's `facade-study.jpg`. */
export const METADATA_FILE: FileRef = {
  library_id: "lib",
  object_id: METADATA_OBJECT,
  version: "v4",
  path: "original/9f2c41aa.jpg",
  display_name: "facade-study.jpg",
};

/** One statement of the fixture. */
function statement(
  curie: string,
  value: string,
  source: "asserted" | "extracted",
): MetadataStatement {
  const [prefix, term] = curie.split(":") as [string, string];
  return { curie, prefix, term, value, source };
}

/** The statements of mockup 28, in record order, plus one technical `exif:` statement. */
export function metadataStatements(): MetadataStatement[] {
  return [
    statement("dcterms:title", "Facade study, north pier", "asserted"),
    statement("dcterms:creator", "Mara Okafor", "asserted"),
    statement("dcterms:subject", "harbour · facade · restoration", "asserted"),
    statement(
      "dcterms:description",
      "Morning light on the restored north pier facade.",
      "asserted",
    ),
    statement("dcterms:created", "2026-10-12 14:03 (no time zone)", "extracted"),
    statement("dcterms:rights", "© 2026 Mara Okafor", "asserted"),
    statement("schema:contentLocation", "North pier, Harbour", "asserted"),
    statement("schema:copyrightYear", "2026", "asserted"),
    statement("exif:PixelXDimension", "6000", "extracted"),
  ];
}

/** A registered RiC-O extension namespace. */
export const RICO: NamespaceRegistration = {
  prefix: "rico",
  iri: "https://www.ica.org/standards/RiC/ontology#",
  kind: "extension",
  registrant: "owner",
  terms: null,
};

/** An owner-declared custom namespace. */
export const CUSTOM: NamespaceRegistration = {
  prefix: "harbour",
  iri: "https://example.org/harbour#",
  kind: "custom",
  registrant: "owner",
  terms: null,
};

type Method = "statements" | "set" | "remove" | "terms" | "namespaces";

/** The tab's reads and writes over the fixture. */
export class FakeMetadataSource implements MetadataSource {
  statements_: MetadataStatement[] = metadataStatements();
  head = "v4";
  /** The library's registry. */
  registry: NamespaceRegistration[] = [];
  /** Failure of each method, when set. */
  readonly failures: Partial<Record<Method, unknown>> = {};
  /** Gates each method waits on, when set. */
  readonly gates: Partial<Record<Method, Gate>> = {};
  /** Every set, in order. */
  readonly sets: { file: FileRef; curie: string; value: string }[] = [];
  /** Every removal, in order. */
  readonly removals: { file: FileRef; curie: string }[] = [];
  /** Every term lookup, in order. */
  readonly lookups: string[] = [];
  /** Calls per method. */
  readonly calls: Record<Method, number> = {
    statements: 0,
    set: 0,
    remove: 0,
    terms: 0,
    namespaces: 0,
  };
  /** When true, calls ignore their abort signal, as a host that answers anyway would. */
  ignoreAbort = false;

  /** Wait for the method's gate, then fail when a failure is set. */
  private async step(name: Method, options?: SourceOptions): Promise<void> {
    this.calls[name] += 1;
    const signal = this.ignoreAbort ? undefined : options?.signal;
    const aborted = signal
      ? new Promise<void>((resolve) => {
          if (signal.aborted) resolve();
          else signal.addEventListener("abort", () => resolve(), { once: true });
        })
      : new Promise<void>(() => undefined);
    await Promise.race([this.gates[name]?.wait, aborted]);
    if (signal?.aborted) throw new DOMException("aborted", "AbortError");
    if (this.failures[name] !== undefined) throw this.failures[name];
  }

  /** The statements of the fixture at `file.version`. */
  async statements(file: FileRef, options?: SourceOptions): Promise<FileStatements> {
    await this.step("statements", options);
    return {
      library_id: file.library_id,
      file_id: "7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8",
      version: file.version,
      statements: this.statements_.map((s) => ({ ...s })),
    };
  }

  /** Publish the next version with the statement set or replaced. */
  async setStatement(
    file: FileRef,
    curie: string,
    value: string,
    options?: SourceOptions,
  ): Promise<MetadataCommit> {
    await this.step("set", options);
    const route = "/api/v1/files/metadata/statements";
    const [prefix, term] = curie.split(":");
    const known =
      (prefix === "dcterms" && DCTERMS_FILE_TERMS.includes(term ?? "")) ||
      prefix === "schema" ||
      this.registry.some((r) => r.prefix === prefix);
    if (!known || !validTermName(term ?? "")) {
      throw new ApiError(400, route, { code: "invalid_request" });
    }
    const existing = this.statements_.find((s) => s.curie === curie);
    if (existing?.value === value) throw new ApiError(409, route, { code: "conflict" });
    this.sets.push({ file, curie, value });
    if (existing) existing.value = value;
    else this.statements_.push(statement(curie, value, "asserted"));
    return this.commit(file);
  }

  /** Publish the next version without the statement. */
  async removeStatement(
    file: FileRef,
    curie: string,
    options?: SourceOptions,
  ): Promise<MetadataCommit> {
    await this.step("remove", options);
    const route = "/api/v1/files/metadata/removals";
    const before = this.statements_.length;
    this.statements_ = this.statements_.filter((s) => s.curie !== curie);
    if (this.statements_.length === before) {
      throw new ApiError(404, route, { code: "not_found" });
    }
    this.removals.push({ file, curie });
    return this.commit(file);
  }

  /** Where a bare name can go, as the route answers it. */
  async placeTerm(
    library: LibraryRef,
    name: string,
    options?: SourceOptions,
  ): Promise<TermCandidates> {
    await this.step("terms", options);
    this.lookups.push(name);
    const candidates: TermCandidate[] = [];
    if (DCTERMS_FILE_TERMS.includes(name))
      candidates.push({ curie: `dcterms:${name}`, kind: "dcterms" });
    if (validTermName(name)) candidates.push({ curie: `schema:${name}`, kind: "schema" });
    for (const r of this.registry) {
      if (r.terms === null || r.terms.includes(name)) {
        candidates.push({ curie: `${r.prefix}:${name}`, kind: r.kind });
      }
    }
    return { name, candidates };
  }

  /** The library's registry. */
  async namespaces(library: LibraryRef, options?: SourceOptions): Promise<Namespaces> {
    await this.step("namespaces", options);
    return { library_id: library.library_id, namespaces: this.registry.map((r) => ({ ...r })) };
  }

  /** Advance the head and answer the commit. */
  private commit(file: FileRef): MetadataCommit {
    const next = `v${Number(this.head.slice(1)) + 1}`;
    this.head = next;
    return {
      library_id: file.library_id,
      object_id: file.object_id,
      version: next,
      created: "2026-10-16T13:00:00Z",
    };
  }
}

export { gate };
