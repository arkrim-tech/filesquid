/**
 * Unit tests for the metadata reads and writes of the Description tab (UI-F2.4, UI-S-012,
 * API-S-035): each method of `ClientMetadataSource` is one call of a metadata property route
 * of contract 0.27.0 with the query or the plain JSON body the contract names; the file id
 * comes from the file object's `urn:uuid:` identifier; no envelope is sent today.
 */
import { describe, expect, it, vi } from "vitest";
import { ClientMetadataSource, type MetadataClient } from "../../src/files/metadata-source";
import type { FileRef } from "../../src/files/model";

const FILE: FileRef = {
  library_id: "lib 1",
  object_id: "urn:uuid:7F3C9A1E-0b2c-4d5e-8f90-a1b2c341d2b8",
  version: "v4",
  // A file's stored original is `original/<file-id>.<ext>` (API-S-026).
  path: "original/7F3C9A1E-0b2c-4d5e-8f90-a1b2c341d2b8.jpg",
  display_name: "facade-study.jpg",
};
const FILE_ID = "7F3C9A1E-0b2c-4d5e-8f90-a1b2c341d2b8";

/** A client whose `request` records its calls and answers `answer`. */
function client(answer: unknown = {}) {
  const request = vi.fn(async () => answer);
  return { request } as unknown as MetadataClient & { request: typeof request };
}

/** The path and options of the one request a method sent; a second request fails the test. */
function first(c: ReturnType<typeof client>) {
  expect(c.request).toHaveBeenCalledTimes(1);
  const [path, options] = c.request.mock.calls[0] as unknown as [
    string,
    { method?: string; body?: unknown; signal?: AbortSignal },
  ];
  return { url: new URL(path, "http://x"), options };
}

describe("ClientMetadataSource", () => {
  it("reads the statements of the file at its version", async () => {
    const c = client({ statements: [] });
    const signal = new AbortController().signal;
    await new ClientMetadataSource(c).statements(FILE, { signal });
    const { url, options } = first(c);
    expect(url.pathname).toBe("/api/v1/files/metadata");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      library_id: "lib 1",
      file_id: FILE_ID,
      version: "v4",
    });
    expect(options.method).toBeUndefined();
    expect(options.signal).toBe(signal);
  });

  it("sets one statement with a plain PUT body", async () => {
    const c = client({ version: "v5" });
    await new ClientMetadataSource(c).setStatement(FILE, "dcterms:title", "Facade");
    const { url, options } = first(c);
    expect(url.pathname).toBe("/api/v1/files/metadata/statements");
    expect(url.search).toBe("");
    expect(options.method).toBe("PUT");
    expect(options.body).toEqual({
      library_id: "lib 1",
      file_id: FILE_ID,
      curie: "dcterms:title",
      value: "Facade",
    });
  });

  it("removes one statement with a plain POST body", async () => {
    const c = client({ version: "v5" });
    await new ClientMetadataSource(c).removeStatement(FILE, "schema:copyrightYear");
    const { url, options } = first(c);
    expect(url.pathname).toBe("/api/v1/files/metadata/removals");
    expect(options.method).toBe("POST");
    expect(options.body).toEqual({
      library_id: "lib 1",
      file_id: FILE_ID,
      curie: "schema:copyrightYear",
    });
  });

  it("asks where a bare name can go, in the library", async () => {
    const c = client({ name: "rights", candidates: [] });
    await new ClientMetadataSource(c).placeTerm({ library_id: "lib 1" }, "rights");
    const { url } = first(c);
    expect(url.pathname).toBe("/api/v1/files/metadata/terms");
    expect(Object.fromEntries(url.searchParams)).toEqual({ library_id: "lib 1", name: "rights" });
  });

  it("reads the library's namespaces", async () => {
    const c = client({ namespaces: [] });
    await new ClientMetadataSource(c).namespaces({ library_id: "lib 1" });
    const { url } = first(c);
    expect(url.pathname).toBe("/api/v1/files/metadata/namespaces");
    expect(Object.fromEntries(url.searchParams)).toEqual({ library_id: "lib 1" });
  });

  it("refuses a path that is not the file's own original, without a request", async () => {
    const c = client();
    const source = new ClientMetadataSource(c);
    // A collection snapshot is a `urn:uuid:` object too; only `original/<file-id>` is a file.
    const snapshot = { ...FILE, path: "collection.md" };
    await expect(source.statements(snapshot)).rejects.toBeInstanceOf(TypeError);
    const another = { ...FILE, path: "original/0f0f0f0f-0b2c-4d5e-8f90-a1b2c341d2b8.jpg" };
    await expect(source.setStatement(another, "dcterms:title", "x")).rejects.toBeInstanceOf(
      TypeError,
    );
    await expect(source.removeStatement(another, "dcterms:title")).rejects.toBeInstanceOf(
      TypeError,
    );
    expect(c.request).not.toHaveBeenCalled();
  });

  it("refuses a file whose object is not a file object, without a request", async () => {
    const c = client();
    const source = new ClientMetadataSource(c);
    const other = { ...FILE, object_id: "urn:uuid:not-a-uuid" };
    await expect(source.statements(other)).rejects.toBeInstanceOf(TypeError);
    await expect(source.setStatement(other, "dcterms:title", "x")).rejects.toBeInstanceOf(
      TypeError,
    );
    await expect(source.removeStatement(other, "dcterms:title")).rejects.toBeInstanceOf(TypeError);
    expect(c.request).not.toHaveBeenCalled();
  });
});
