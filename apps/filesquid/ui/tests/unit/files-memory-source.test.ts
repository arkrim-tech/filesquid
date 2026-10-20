/**
 * Unit tests for the in-memory file source (UI-F2.2): it answers like the file routes so the
 * element tests run without a server: folders first, display names apart from stored names,
 * opaque cursors with a bounded page, one new version per write, restore from an earlier
 * version, and the API's refusals as `ApiError`.
 */
import { describe, expect, it } from "vitest";
import { ApiError } from "../../src/api/client";
import { MemoryFileSource } from "../../src/files/memory-source";
import type { FileLocation, FileRef } from "../../src/files/model";

const LOCATION: FileLocation = { library_id: "lib", object_id: "obj", prefix: "" };

/** A source holding a small tree at v1. */
function source(): MemoryFileSource {
  return new MemoryFileSource({
    library_id: "lib",
    object_id: "obj",
    files: [
      { path: "Docs/a.txt", displayName: "Alpha notes.txt", content: "alpha" },
      { path: "Docs/b.txt", content: "bravo" },
      { path: "top.png", content: "png", mediaType: "image/png" },
    ],
  });
}

describe("MemoryFileSource listing", () => {
  it("lists folders first, then files, with display names apart from stored names", async () => {
    const page = await source().list(LOCATION);
    expect(page.version).toBe("v1");
    expect(page.entries.map((e) => [e.kind, e.name])).toEqual([
      ["folder", "Docs"],
      ["file", "top.png"],
    ]);
    const docs = await source().list({ ...LOCATION, prefix: "Docs" });
    const alpha = docs.entries[0];
    expect(alpha).toMatchObject({ kind: "file", name: "a.txt", display_name: "Alpha notes.txt" });
    expect(docs.entries[1]).toMatchObject({ name: "b.txt", display_name: "b.txt" });
  });

  it("pages with opaque cursors and refuses one it did not issue", async () => {
    const s = source();
    const first = await s.list(LOCATION, null, 1);
    expect(first.entries).toHaveLength(1);
    expect(first.next_cursor).not.toBeNull();
    const second = await s.list(LOCATION, first.next_cursor, 1);
    expect(second.entries[0]).toMatchObject({ name: "top.png" });
    expect(second.next_cursor).toBeNull();
    await expect(s.list(LOCATION, "bogus", 1)).rejects.toMatchObject({ code: "invalid_request" });
  });

  it("answers not_found for an unknown folder and invalid_request for a bad limit", async () => {
    await expect(source().list({ ...LOCATION, prefix: "Nope" })).rejects.toBeInstanceOf(ApiError);
    await expect(source().list(LOCATION, null, 0)).rejects.toMatchObject({
      code: "invalid_request",
    });
  });

  it("rejects every call with the failure it was told to raise", async () => {
    const s = source();
    s.failure = new Error("boom");
    await expect(s.list(LOCATION)).rejects.toThrow("boom");
  });
});

describe("MemoryFileSource files", () => {
  const ref: FileRef = {
    library_id: "lib",
    object_id: "obj",
    version: "v1",
    path: "Docs/a.txt",
    display_name: "Alpha notes.txt",
  };

  it("answers details with the media type, size and display name", async () => {
    const details = await source().details(ref);
    expect(details).toMatchObject({
      path: "Docs/a.txt",
      name: "a.txt",
      display_name: "Alpha notes.txt",
      size: 5,
      media_type: "text/plain",
      version: "v1",
    });
  });

  it("reads an inclusive byte range", async () => {
    const bytes = await source().readRange(ref, { start: 1, end: 3 });
    expect(new TextDecoder().decode(bytes)).toBe("lph");
  });

  it("mints a distinct URL per call and records each mint", async () => {
    const s = source();
    const a = await s.mintContent(ref);
    const b = await s.mintContent(ref);
    expect(a).not.toBe(b);
    expect(s.mints).toHaveLength(2);
  });
});

describe("MemoryFileSource writes", () => {
  it("an upload commits one new version holding the file", async () => {
    const s = source();
    const commit = await s.upload(LOCATION, "new.txt", new Blob(["hello"]));
    expect(commit.version).toBe("v2");
    expect(s.head).toBe("v2");
    const page = await s.list(LOCATION);
    expect(page.entries.map((e) => e.name)).toContain("new.txt");
  });

  it("a folder stays pending until a file is added beneath it", async () => {
    const s = source();
    expect(await s.createFolder(LOCATION, "Empty")).toMatchObject({ path: "Empty", pending: true });
    expect(await s.createFolder(LOCATION, "Docs")).toMatchObject({ path: "Docs", pending: false });
    await expect(s.createFolder(LOCATION, "top.png")).rejects.toMatchObject({ code: "conflict" });
    expect(s.head).toBe("v1");
  });

  it("a move renames a file or moves a folder with everything below it", async () => {
    const s = source();
    expect((await s.move(LOCATION, "top.png", "picture.png")).version).toBe("v2");
    await s.move(LOCATION, "Docs", "Papers");
    const paths = (await s.list({ ...LOCATION, prefix: "Papers" })).entries.map((e) => e.path);
    expect(paths).toEqual(["Papers/a.txt", "Papers/b.txt"]);
  });

  it("a move to the same place is refused as a conflict and commits nothing", async () => {
    const s = source();
    await expect(s.move(LOCATION, "top.png", "top.png")).rejects.toMatchObject({
      code: "conflict",
    });
    expect(s.head).toBe("v1");
  });

  it("removes keep history and restore brings a file back as a new version", async () => {
    const s = source();
    const removed = await s.remove(LOCATION, "Docs/a.txt");
    expect(removed.version).toBe("v2");
    const docs = await s.list({ ...LOCATION, prefix: "Docs" });
    expect(docs.entries.map((e) => e.name)).toEqual(["b.txt"]);
    const restored = await s.restore(LOCATION, "v1", ["Docs/a.txt"]);
    expect(restored.version).toBe("v3");
    const again = await s.list({ ...LOCATION, prefix: "Docs" });
    expect(again.entries.map((e) => e.name)).toEqual(["a.txt", "b.txt"]);
  });

  it("refuses to remove or restore what is not there", async () => {
    const s = source();
    await expect(s.remove(LOCATION, "ghost.txt")).rejects.toMatchObject({ code: "not_found" });
    await expect(s.restore(LOCATION, "v9", ["top.png"])).rejects.toMatchObject({
      code: "not_found",
    });
  });
});
