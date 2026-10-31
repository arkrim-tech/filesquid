/**
 * The facts read of the File info tab (UI-F2.15; UI-S-022, API-S-027, MD-S-013, ADR 038): the
 * {@link FactsSource} interface over `GET /api/v1/files/facts` and {@link ClientFactsSource},
 * which answers it through `FileSquidClient` (UI-S-012). The route is the read-only projection
 * of the file's `<file-uuid>.metadata.md` record: original and display name, extension, format
 * and media type, size, SHA-256 and one summary per acquisition. It never answers an absolute
 * path, an import root, a host name or an account name, and no route edits it. The tab never
 * fetches by itself, so a fixture stands in for the API in tests. Every method only reads.
 */
import type { FileSquidClient } from "../api/client";
import type { components } from "../api/schema";
import type { FileRef } from "./model";
import { fileIdOf, isOriginalPath } from "./ops";
import type { SourceOptions } from "./source";

/** The facts projection of one file, generated from the API contract. */
export type FileFacts = components["schemas"]["FileFacts"];
/** The facts of one file at one version, with the library and file that were read. */
export type FileFactsAnswer = components["schemas"]["FileFactsAnswer"];
/** One acquisition of a file: when, how, and from which volume. */
export type ImportSummary = components["schemas"]["ImportSummary"];
/** A name as the record holds it: text, or the bytes as base64 when it is not UTF-8. */
export type RawName = components["schemas"]["RawName"];

/** What the File info tab asks of the API for a file's record. */
export interface FactsSource {
  /** The facts of the file at its version. Reads. */
  facts(file: FileRef, options?: SourceOptions): Promise<FileFactsAnswer>;
}

/** The part of `FileSquidClient` the source needs (injectable in tests). */
export type FactsClient = Pick<FileSquidClient, "request">;

/** The facts route. */
const FACTS_PATH = "/api/v1/files/facts";

/**
 * The file id of a reference that is a file's own stored original (`original/<file-id>[.<ext>]`
 * in the object `urn:uuid:<file-id>`, API-S-026), the only reference with a file record. Pure.
 *
 * @param file The file.
 * @returns The id, or undefined when the reference has no file record.
 */
function recordFileId(file: FileRef): string | undefined {
  const id = fileIdOf(file.object_id);
  return id !== undefined && isOriginalPath(file.path, id) ? id : undefined;
}

/**
 * Whether the facts route can answer for a reference: it is a file's own original, so a file
 * record exists. A tab reads the facts only for such a reference. Pure.
 *
 * @param file The file.
 * @returns True when the facts route takes the reference.
 */
export function hasFileRecord(file: FileRef): boolean {
  return recordFileId(file) !== undefined;
}

/** A {@link FactsSource} over `FileSquidClient` and the facts route. */
export class ClientFactsSource implements FactsSource {
  /** @param client The API client (its transport adapter and token are already chosen). */
  constructor(private readonly client: FactsClient) {}

  /**
   * Read the facts of the file at its version with `GET /api/v1/files/facts`. Network I/O.
   *
   * @param file The file at one version.
   * @param options Abort signal.
   * @returns The facts projection.
   * @throws TypeError for a reference that is not a file's own original (nothing is sent).
   * @throws ApiError when the API refuses (a file with no record is 404).
   */
  async facts(file: FileRef, options: SourceOptions = {}): Promise<FileFactsAnswer> {
    const file_id = recordFileId(file);
    if (file_id === undefined) throw new TypeError("The reference has no file record");
    const query = new URLSearchParams({
      library_id: file.library_id,
      file_id,
      version: file.version,
    });
    return this.client.request<FileFactsAnswer>(`${FACTS_PATH}?${query.toString()}`, {
      signal: options.signal,
    });
  }
}
