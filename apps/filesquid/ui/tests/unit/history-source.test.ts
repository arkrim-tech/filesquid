/**
 * Unit tests for the history reads the Details and Versions panels use (UI-F2.3): each method
 * of `ClientHistorySource` is one GET of an existing route with the query the contract names,
 * and `ClientFileSource.restore` sends the optional label member of `POST
 * /api/v1/files/restorations` (API-F2.5) in the command's input, with the head it read as the
 * envelope's expected revision (API-F2.13).
 */
import { describe, expect, it, vi } from "vitest";
import { ClientHistorySource } from "../../src/files/history-source";
import { ClientFileSource, type FileClient } from "../../src/files/source";
import type { FileRef } from "../../src/files/model";

const FILE: FileRef = {
  library_id: "lib 1",
  object_id: "urn:uuid:abc",
  version: "v4",
  path: "original/a b.pdf",
  display_name: "a b.pdf",
};

/** A client whose `request` records its calls and answers `answer`. */
function client(answer: unknown = {}) {
  const request = vi.fn(async () => answer);
  return { request } as unknown as FileClient & { request: typeof request };
}

describe("ClientHistorySource", () => {
  it("reads the file's history by library, object and path", async () => {
    const c = client({ entries: [] });
    await new ClientHistorySource(c).history(FILE);
    const [path] = c.request.mock.calls[0] as unknown as [string];
    const url = new URL(path, "http://x");
    expect(url.pathname).toBe("/api/v1/files/history");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      library_id: "lib 1",
      object_id: "urn:uuid:abc",
      path: "original/a b.pdf",
    });
  });

  it("reads every version of the object", async () => {
    const c = client({ versions: [] });
    await new ClientHistorySource(c).versions(FILE);
    const url = new URL((c.request.mock.calls[0] as unknown as [string])[0], "http://x");
    expect(url.pathname).toBe("/api/v1/files/versions");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      library_id: "lib 1",
      object_id: "urn:uuid:abc",
    });
  });

  it("reads the bytes the file's history keeps", async () => {
    const c = client({ bytes: 1 });
    await new ClientHistorySource(c).historySize(FILE);
    const url = new URL((c.request.mock.calls[0] as unknown as [string])[0], "http://x");
    expect(url.pathname).toBe("/api/v1/files/history-size");
    expect(url.searchParams.get("path")).toBe("original/a b.pdf");
  });

  it("passes the abort signal on", async () => {
    const c = client();
    const abort = new AbortController();
    await new ClientHistorySource(c).history(FILE, { signal: abort.signal });
    expect(c.request.mock.calls[0]).toEqual([expect.any(String), { signal: abort.signal }]);
  });
});

/** A client whose `command` records its calls and answers a committed restore. */
function commandClient() {
  const result = { library_id: "lib", object_id: "obj", version: "v8", created: "t" };
  const command = vi.fn(async () => ({
    operation_id: "op",
    error: null,
    kind: "committed",
    result,
  }));
  return { command } as unknown as FileClient & { command: typeof command };
}

describe("ClientFileSource.restore", () => {
  it("sends the label in the command's input and the head as its expected revision", async () => {
    const c = commandClient();
    const object = { library_id: "lib", object_id: "obj" };
    const commit = await new ClientFileSource(c).restore(object, "v5", ["original/a.pdf"], {
      revision: "v7",
      label: "Back to client review",
    });
    expect(commit.version).toBe("v8");
    expect(c.command).toHaveBeenCalledWith(
      "/api/v1/files/restorations",
      expect.objectContaining({ expected_revision: "v7", scope: "library:lib" }),
      {
        library_id: "lib",
        object_id: "obj",
        version: "v5",
        paths: ["original/a.pdf"],
        label: "Back to client review",
      },
      { signal: undefined },
    );
  });

  it("sends no label member without one", async () => {
    const c = commandClient();
    await new ClientFileSource(c).restore({ library_id: "l", object_id: "o" }, "v1", ["p"], {
      revision: "v2",
    });
    const [, , input] = c.command.mock.calls[0] as unknown as [string, object, object];
    expect(input).not.toHaveProperty("label");
  });
});
