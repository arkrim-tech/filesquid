/**
 * Unit tests for the history reads `MemoryFileSource` answers for the file panel (UI-F2.3), as
 * `GET /files/versions`, `/files/history` and `/files/history-size` do: every version with the
 * head, only the versions that changed a file, and each distinct content counted once.
 */
import { describe, expect, it } from "vitest";
import { MemoryFileSource } from "../../src/files/memory-source";
import type { FileRef } from "../../src/files/model";

const OBJECT = { library_id: "lib", object_id: "obj" };
const FILE_ID = "5b1d2c3e-7a4f-4e8b-9c6d-0a1b2c3d4e5f";
const FILE: FileRef = { ...OBJECT, version: "v1", path: "Docs/a.txt", display_name: "" };

/** A source holding a small tree at v1. */
function source(): MemoryFileSource {
  return new MemoryFileSource({
    ...OBJECT,
    files: [
      { path: "Docs/a.txt", displayName: "Alpha notes.txt", content: "alpha" },
      { path: "top.png", content: "png", mediaType: "image/png" },
    ],
  });
}

describe("MemoryFileSource history reads", () => {
  it("lists every version of the object, oldest first, with the head", async () => {
    const s = source();
    await s.remove(OBJECT, "top.png", { revision: "v1" });
    const versions = await s.versions();
    expect(versions.head).toBe("v2");
    expect(versions.versions.map((v) => v.version)).toEqual(["v1", "v2"]);
    expect(versions.versions[1]?.created).toBe("2026-10-16T09:01:00.000Z");
  });

  it("lists only the versions that changed the file: added, removed and restored", async () => {
    const s = source();
    await s.remove(OBJECT, "top.png", { revision: "v1" });
    await s.remove(OBJECT, "Docs/a.txt", { revision: "v2" });
    await s.restore(OBJECT, "v1", ["Docs/a.txt"], { revision: "v3" });
    const history = await s.history(FILE);
    expect(history.display_name).toBe("Alpha notes.txt");
    expect(history.entries.map((e) => [e.version.version, e.change, e.size])).toEqual([
      ["v1", "added", 5],
      ["v3", "deleted", null],
      ["v4", "added", 5],
    ]);
    expect(history.entries[1]?.digest).toBeNull();
  });

  it("counts each distinct content of the history once", async () => {
    const s = source();
    await s.remove(OBJECT, "Docs/a.txt", { revision: "v1" });
    await s.restore(OBJECT, "v1", ["Docs/a.txt"], { revision: "v2" });
    expect(await s.historySize(FILE)).toMatchObject({ versions: 3, contents: 1, bytes: 5 });
  });

  it("keeps a restore's label on its version, in the versions and the history", async () => {
    const s = source();
    await s.remove(OBJECT, "Docs/a.txt", { revision: "v1" });
    await s.restore(OBJECT, "v1", ["Docs/a.txt"], { revision: "v2", label: "Back again" });
    expect((await s.versions()).versions.map((v) => v.label)).toEqual([null, null, "Back again"]);
    expect((await s.history(FILE)).entries.at(-1)?.version.label).toBe("Back again");
  });

  it("refuses a restore label the restorations route refuses, before the revision check", async () => {
    const s = source();
    await s.remove(OBJECT, "Docs/a.txt", { revision: "v1" });
    for (const label of ["   ", "a\u0007b", "x".repeat(513)]) {
      await expect(
        s.restore(OBJECT, "v1", ["Docs/a.txt"], { revision: "stale", label }),
      ).rejects.toMatchObject({ code: "invalid_request" });
    }
    expect((await s.versions()).head).toBe("v2");
    const longest = "x".repeat(512);
    const ok = await s.restore(OBJECT, "v1", ["Docs/a.txt"], { revision: "v2", label: longest });
    expect(ok.version).toBe("v3");
    expect((await s.versions()).versions.at(-1)?.label).toBe(longest);
  });

  it("lists no version that only changed the display name, as the API compares content", async () => {
    const s = new MemoryFileSource({
      library_id: "lib",
      object_id: `urn:uuid:${FILE_ID}`,
      files: [{ path: `original/${FILE_ID}.txt`, displayName: "a.txt", content: "a" }],
    });
    await s.rename({ library_id: "lib" }, FILE_ID, "b.txt", { revision: "v1" });
    const own = { ...FILE, object_id: `urn:uuid:${FILE_ID}`, path: `original/${FILE_ID}.txt` };
    expect((await s.history(own)).entries.map((e) => e.version.version)).toEqual(["v1"]);
    expect((await s.versions()).display_name).toBe("b.txt");
  });

  it("names no file for an object that is not a file object", async () => {
    expect((await source().versions()).display_name).toBeNull();
  });

  it("counts two different contents of the same size as two", async () => {
    const s = new MemoryFileSource({
      ...OBJECT,
      files: [
        { path: "x.txt", content: "ab" },
        { path: "y.txt", content: "zz" },
      ],
    });
    await s.remove(OBJECT, "x.txt", { revision: "v1" });
    await s.move(OBJECT, "y.txt", "x.txt", { revision: "v2" });
    const size = await s.historySize({ ...FILE, path: "x.txt" });
    expect(size).toMatchObject({ contents: 2, bytes: 4 });
  });

  it("refuses a file that never existed", async () => {
    const missing = { ...FILE, path: "nope.txt" };
    await expect(source().history(missing)).rejects.toMatchObject({ code: "not_found" });
  });
});
