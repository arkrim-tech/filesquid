/**
 * Unit tests for the sources of the File info and Provenance tabs (UI-F2.15; UI-S-012,
 * API-S-027, API-S-029): every method is one call of a published route through
 * `FileSquidClient`, with the query or path the contract names; a file that is not in an object
 * named by a UUID is refused before anything is sent; the export keeps the truncation header.
 */
import { describe, expect, it, vi } from "vitest";
import { ClientFactsSource, hasFileRecord, type FactsClient } from "../../src/files/facts-source";
import {
  ClientProvenanceSource,
  HISTORY_PAGE_SIZE,
  type ProvenanceClient,
} from "../../src/files/provenance-source";
import type { FileRef } from "../../src/files/model";

const FILE_ID = "7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8";
const FILE: FileRef = {
  library_id: "lib 1",
  object_id: `urn:uuid:${FILE_ID}`,
  version: "v4",
  path: `original/${FILE_ID}.jpg`,
  display_name: "facade-study.jpg",
};

/** A client with `request` and `read` recording their calls. */
function client(answer: unknown = {}, response?: Response) {
  const request = vi.fn(async () => answer);
  const read = vi.fn(async () => ({ response: response as Response, partial: false }));
  return { request, read } as unknown as ProvenanceClient &
    FactsClient & { request: typeof request; read: typeof read };
}

/** The text of a blob (jsdom's Blob has no `text()`). */
function readBlob(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsText(blob);
  });
}

/** The URL of the first call of a mock. */
function urlOf(mock: ReturnType<typeof vi.fn>): URL {
  return new URL((mock.mock.calls[0] as unknown as [string])[0], "http://x");
}

describe("ClientFactsSource", () => {
  it("reads the facts of the file at its version", async () => {
    const c = client({ facts: {} });
    const signal = new AbortController().signal;
    await new ClientFactsSource(c).facts(FILE, { signal });
    const url = urlOf(c.request);
    expect(url.pathname).toBe("/api/v1/files/facts");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      library_id: "lib 1",
      file_id: FILE_ID,
      version: "v4",
    });
    expect(
      (c.request.mock.calls[0] as unknown as [string, { signal?: AbortSignal }])[1].signal,
    ).toBe(signal);
  });

  it("refuses a reference that is not a file's own original before sending anything", async () => {
    const c = client();
    const other = { ...FILE, path: "Holiday/photo.jpg" };
    await expect(new ClientFactsSource(c).facts(other)).rejects.toThrow(TypeError);
    expect(c.request).not.toHaveBeenCalled();
    expect(hasFileRecord(other)).toBe(false);
    expect(hasFileRecord(FILE)).toBe(true);
  });
});

describe("ClientProvenanceSource", () => {
  it("reads one page of the file's history by the file id, with a bounded page size and the cursor", async () => {
    const c = client({ items: [], next_cursor: null, truncated: false });
    await new ClientProvenanceSource(c).history(FILE, { cursor: "abc def" });
    const url = urlOf(c.request);
    expect(url.pathname).toBe(`/api/v1/files/${FILE_ID}/history`);
    expect(Object.fromEntries(url.searchParams)).toEqual({
      limit: String(HISTORY_PAGE_SIZE),
      cursor: "abc def",
    });
  });

  it("reads the first page without a cursor", async () => {
    const c = client({ items: [], next_cursor: null, truncated: false });
    await new ClientProvenanceSource(c).history(FILE);
    expect(urlOf(c.request).searchParams.has("cursor")).toBe(false);
  });

  it("reads the record of one version", async () => {
    const c = client({ "fsq:profile": "filesquid-prov/1" });
    const record = await new ClientProvenanceSource(c).record(FILE, "v3");
    expect(urlOf(c.request).pathname).toBe(`/api/v1/files/${FILE_ID}/versions/v3/prov`);
    expect(record).toEqual({ "fsq:profile": "filesquid-prov/1" });
  });

  it("exports the history as a JSON-LD blob and reports a truncated export", async () => {
    const response = new Response('[{"@id":"x"}]', {
      headers: { "content-type": "application/ld+json", "X-History-Truncated": "true" },
    });
    const c = client({}, response);
    const exported = await new ClientProvenanceSource(c).exportHistory(FILE);
    expect(urlOf(c.read).pathname).toBe(`/api/v1/files/${FILE_ID}/history/export`);
    expect(urlOf(c.read).searchParams.has("redact")).toBe(false);
    expect(exported.truncated).toBe(true);
    expect(exported.blob.type).toContain("application/ld+json");
    expect(await readBlob(exported.blob)).toBe('[{"@id":"x"}]');
  });

  it("refuses an object that is not named by a UUID before sending anything", async () => {
    const c = client();
    const source = new ClientProvenanceSource(c);
    const odd = { ...FILE, object_id: "ark:/12345/x" };
    await expect(source.history(odd)).rejects.toThrow(TypeError);
    await expect(source.record(odd, "v1")).rejects.toThrow(TypeError);
    await expect(source.exportHistory(odd)).rejects.toThrow(TypeError);
    expect(c.request).not.toHaveBeenCalled();
    expect(c.read).not.toHaveBeenCalled();
  });

  it("refuses a path that is not the file's own stored original, so another file's history is never shown", async () => {
    const c = client();
    const source = new ClientProvenanceSource(c);
    const other = { ...FILE, path: "Harbour project/facade-study.jpg" };
    await expect(source.history(other)).rejects.toThrow(TypeError);
    await expect(source.record(other, "v1")).rejects.toThrow(TypeError);
    await expect(source.exportHistory(other)).rejects.toThrow(TypeError);
    expect(c.request).not.toHaveBeenCalled();
    expect(c.read).not.toHaveBeenCalled();
  });
});
