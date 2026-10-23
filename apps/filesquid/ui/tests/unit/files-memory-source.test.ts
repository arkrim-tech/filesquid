/**
 * Unit tests for the in-memory file source (UI-F2.2): it answers like the file routes so the
 * element tests run without a server: folders first, display names apart from stored names,
 * opaque cursors with a bounded page, one new version per write, restore from an earlier
 * version, and the API's refusals as `ApiError`.
 */
import { describe, expect, it } from "vitest";
import { ApiError } from "../../src/api/client";
import { MemoryFileSource } from "../../src/files/memory-source";
import { MAX_UPLOAD_BYTES, type FileLocation, type FileRef } from "../../src/files/model";

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
    const file = { ...LOCATION, version: "v1", path: "top.png", display_name: "top.png" };
    await expect(s.list(LOCATION)).rejects.toThrow("boom");
    await expect(s.details(file)).rejects.toThrow("boom");
    await expect(s.remove(LOCATION, "top.png", { revision: s.head })).rejects.toThrow("boom");
  });

  it("orders names by Unicode code point, not by UTF-16 unit", async () => {
    const s = new MemoryFileSource({
      library_id: "lib",
      object_id: "obj",
      files: [
        { path: "\u{1F600}.txt", content: "a" },
        { path: "\uFF21.txt", content: "b" },
      ],
    });
    const page = await s.list(LOCATION);
    expect(page.entries.map((e) => e.name)).toEqual(["\uFF21.txt", "\u{1F600}.txt"]);
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
  it("an upload creates the file's own object at v1 and leaves the object shown as it was", async () => {
    const s = source();
    const facts = { name: "new.txt", name_normalization: "nfc", media_type: "text/plain" } as const;
    const commit = await s.upload(LOCATION, facts, new Blob(["hello"]));
    expect(commit.file_id).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    );
    expect(commit).toMatchObject({
      library_id: "lib",
      object_id: `urn:uuid:${commit.file_id}`,
      version: "v1",
    });
    expect(s.head).toBe("v1");
    expect((await s.list(LOCATION)).entries.map((e) => e.name)).not.toContain("new.txt");
    expect(s.uploads).toEqual([
      expect.objectContaining({
        file_id: commit.file_id,
        name: "new.txt",
        name_normalization: "nfc",
        media_type: "text/plain",
        size: 5,
        version: "v1",
      }),
    ]);
  });

  it("an upload naming a file_id is a new version of that file; unchanged bytes are a conflict", async () => {
    const s = source();
    const first = await s.upload(LOCATION, { name: "a.txt" }, new Blob(["one"]));
    const again = { name: "a.txt", file_id: first.file_id };
    const second = await s.upload(LOCATION, again, new Blob(["two"]), { revision: "v1" });
    expect(second).toMatchObject({ file_id: first.file_id, version: "v2" });
    await expect(
      s.upload(LOCATION, again, new Blob(["two"]), { revision: "v2" }),
    ).rejects.toMatchObject({ code: "conflict" });
    const unknown = { name: "a.txt", file_id: "0b7e2c1a-3d4f-4a5b-8c6d-7e8f9a0b1c2d" };
    await expect(s.upload(LOCATION, unknown, new Blob(["x"]))).rejects.toMatchObject({
      code: "not_found",
    });
  });

  it.each([
    ["an empty name", { name: "" }],
    ["a name with a slash", { name: "a/b.txt" }],
    ["a name with NUL", { name: "a\u0000b" }],
    ["a name over 1,024 bytes", { name: "n".repeat(1025) }],
    ["a name that is not ASCII without its form", { name: "Café.txt" }],
    ["a media type that is not type/subtype", { name: "a.txt", media_type: "text" }],
    ["a time that is not RFC 3339 UTC", { name: "a.txt", last_modified: "2026-10-20 08:00" }],
    [
      "a date that is not in the calendar",
      { name: "a.txt", last_modified: "2026-02-29T00:00:00Z" },
    ],
    ["a clock that is out of range", { name: "a.txt", last_modified: "2026-10-20T25:99:99Z" }],
    [
      "a leap second that is not 23:59:60",
      { name: "a.txt", last_modified: "2026-10-20T08:00:60Z" },
    ],
    ["a file_id that is not a UUID v4", { name: "a.txt", file_id: "not-a-uuid" }],
  ])("refuses %s as invalid_request without a version", async (_case, facts) => {
    const s = source();
    await expect(s.upload(LOCATION, facts, new Blob(["x"]))).rejects.toMatchObject({
      code: "invalid_request",
    });
    expect(s.uploads).toEqual([]);
  });

  it("accepts a leap second and a leap day as the API does", async () => {
    const s = source();
    const times = ["2024-02-29T00:00:00Z", "0004-02-29T12:00:00Z", "2016-12-31T23:59:60.5Z"];
    for (const [i, last_modified] of times.entries()) {
      await s.upload(LOCATION, { name: `t${i}.txt`, last_modified }, new Blob(["x"]));
    }
    expect(s.uploads.map((u) => u.last_modified)).toEqual(times);
  });

  it("refuses a body over 64 MiB as invalid_request without reading it", async () => {
    const s = source();
    const body = new Blob(["x"]);
    Object.defineProperty(body, "size", { value: MAX_UPLOAD_BYTES + 1 });
    await expect(s.upload(LOCATION, { name: "big.bin" }, body)).rejects.toMatchObject({
      code: "invalid_request",
    });
    expect(s.uploads).toEqual([]);
  });

  it("gives every upload a later created time, a new version of a file included", async () => {
    const s = source();
    const first = await s.upload(LOCATION, { name: "a.txt" }, new Blob(["one"]));
    const again = { name: "a.txt", file_id: first.file_id };
    const second = await s.upload(LOCATION, again, new Blob(["two"]), { revision: "v1" });
    const third = await s.upload(LOCATION, again, new Blob(["three"]), { revision: "v2" });
    expect(Date.parse(second.created)).toBeGreaterThan(Date.parse(first.created));
    expect(Date.parse(third.created)).toBeGreaterThan(Date.parse(second.created));
  });

  it("a folder stays pending until a file is added beneath it", async () => {
    const s = source();
    expect(await s.createFolder(LOCATION, "Empty", { revision: s.head })).toMatchObject({
      path: "Empty",
      pending: true,
    });
    expect(await s.createFolder(LOCATION, "Docs", { revision: s.head })).toMatchObject({
      path: "Docs",
      pending: false,
    });
    await expect(s.createFolder(LOCATION, "top.png", { revision: s.head })).rejects.toMatchObject({
      code: "conflict",
    });
    expect(s.head).toBe("v1");
  });

  it("a move renames a file or moves a folder with everything below it", async () => {
    const s = source();
    expect((await s.move(LOCATION, "top.png", "picture.png", { revision: s.head })).version).toBe(
      "v2",
    );
    await s.move(LOCATION, "Docs", "Papers", { revision: s.head });
    const paths = (await s.list({ ...LOCATION, prefix: "Papers" })).entries.map((e) => e.path);
    expect(paths).toEqual(["Papers/a.txt", "Papers/b.txt"]);
  });

  it("refuses a write whose revision is not the head as a conflict (API-F2.13)", async () => {
    const s = source();
    const stale = { revision: "v9" };
    await expect(s.move(LOCATION, "top.png", "p.png", stale)).rejects.toMatchObject({
      code: "conflict",
      status: 409,
    });
    await expect(s.remove(LOCATION, "top.png", stale)).rejects.toMatchObject({ status: 409 });
    await expect(s.restore(LOCATION, "v1", ["top.png"], stale)).rejects.toMatchObject({
      status: 409,
    });
    await expect(s.createFolder(LOCATION, "New", stale)).rejects.toMatchObject({ status: 409 });
    const upload = s.upload(LOCATION, { name: "n.txt" }, new Blob(["n"]), { revision: "v1" });
    await expect(upload).rejects.toMatchObject({ status: 409 });
    expect(s.head).toBe("v1");
  });

  it("a move to the same place is refused as a conflict and commits nothing", async () => {
    const s = source();
    await expect(
      s.move(LOCATION, "top.png", "top.png", { revision: s.head }),
    ).rejects.toMatchObject({
      code: "conflict",
    });
    expect(s.head).toBe("v1");
  });

  describe("rename (API-S-027)", () => {
    const FILE_ID = "5b1d2c3e-7a4f-4e8b-9c6d-0a1b2c3d4e5f";
    const WHERE: FileLocation = {
      library_id: "lib",
      object_id: `urn:uuid:${FILE_ID}`,
      prefix: "original",
    };
    /** One file's own object, as the API stores it. */
    const fileObject = () =>
      new MemoryFileSource({
        library_id: "lib",
        object_id: `urn:uuid:${FILE_ID}`,
        files: [{ path: `original/${FILE_ID}.txt`, displayName: "readme.txt", content: "hi" }],
      });

    it("changes only the display name, as a new version", async () => {
      const s = fileObject();
      const commit = await s.rename({ library_id: "lib" }, FILE_ID, "notes.txt", {
        revision: s.head,
      });
      expect(commit).toMatchObject({ object_id: WHERE.object_id, version: "v2" });
      const entry = (await s.list(WHERE)).entries[0];
      expect(entry).toMatchObject({ name: `${FILE_ID}.txt`, display_name: "notes.txt" });
      expect(entry).toMatchObject({ path: `original/${FILE_ID}.txt` });
    });

    it("refuses a bad name (400), the current name (409) and an unknown file (404)", async () => {
      const s = fileObject();
      const rename = (id: string, name: string) =>
        s.rename({ library_id: "lib" }, id, name, { revision: s.head });
      for (const bad of ["", "a/b", "a\0b", "x".repeat(1025)]) {
        await expect(rename(FILE_ID, bad)).rejects.toMatchObject({
          code: "invalid_request",
          status: 400,
        });
      }
      await expect(rename(FILE_ID, "readme.txt")).rejects.toMatchObject({
        code: "conflict",
        status: 409,
      });
      await expect(rename("ffffffff-ffff-4fff-8fff-ffffffffffff", "n")).rejects.toMatchObject({
        code: "not_found",
        status: 404,
      });
      expect(s.head).toBe("v1");
    });

    it("shows the safe form of a stored name, as the API does (API-S-005)", async () => {
      const s = fileObject();
      await s.rename({ library_id: "lib" }, FILE_ID, "  a\\b\u0007c.txt ", { revision: s.head });
      const entry = (await s.list(WHERE)).entries[0];
      expect(entry).toMatchObject({ display_name: "a_bc.txt" });
      const ref = { ...WHERE, version: "v2", path: `original/${FILE_ID}.txt`, display_name: "" };
      expect((await s.details(ref)).display_name).toBe("a_bc.txt");
      await s.rename({ library_id: "lib" }, FILE_ID, "   ", { revision: s.head });
      expect((await s.list(WHERE)).entries[0]).toMatchObject({ display_name: "file" });
    });

    it("refuses a move from or to original, or below it, with 400 (use renames)", async () => {
      const s = fileObject();
      const original = `original/${FILE_ID}.txt`;
      for (const [from, to] of [
        [original, "original/other.txt"],
        [original, "elsewhere.txt"],
        ["original", "moved"],
        ["elsewhere.txt", "original/in.txt"],
      ] as const) {
        await expect(s.move(WHERE, from, to, { revision: s.head })).rejects.toMatchObject({
          code: "invalid_request",
          status: 400,
        });
      }
      expect(s.head).toBe("v1");
    });

    it("does not treat another UUID-named object as a file object", async () => {
      const collection = new MemoryFileSource({
        library_id: "lib",
        object_id: `urn:uuid:${FILE_ID}`,
        files: [{ path: "original/notes.txt", content: "n" }],
      });
      await expect(
        collection.move(WHERE, "original/notes.txt", "original/b.txt", {
          revision: collection.head,
        }),
      ).resolves.toMatchObject({ version: "v2" });
      await expect(
        collection.rename({ library_id: "lib" }, FILE_ID, "x.txt", { revision: collection.head }),
      ).rejects.toMatchObject({ code: "not_found" });
    });
  });

  it("removes keep history and restore brings a file back as a new version", async () => {
    const s = source();
    const removed = await s.remove(LOCATION, "Docs/a.txt", { revision: s.head });
    expect(removed.version).toBe("v2");
    const docs = await s.list({ ...LOCATION, prefix: "Docs" });
    expect(docs.entries.map((e) => e.name)).toEqual(["b.txt"]);
    const restored = await s.restore(LOCATION, "v1", ["Docs/a.txt"], { revision: s.head });
    expect(restored.version).toBe("v3");
    const again = await s.list({ ...LOCATION, prefix: "Docs" });
    expect(again.entries.map((e) => e.name)).toEqual(["a.txt", "b.txt"]);
  });

  it("refuses to remove or restore what is not there", async () => {
    const s = source();
    await expect(s.remove(LOCATION, "ghost.txt", { revision: s.head })).rejects.toMatchObject({
      code: "not_found",
    });
    await expect(
      s.restore(LOCATION, "v9", ["top.png"], { revision: s.head }),
    ).rejects.toMatchObject({
      code: "not_found",
    });
  });
});
