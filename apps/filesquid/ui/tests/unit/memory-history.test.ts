/**
 * Unit tests for the history reads `MemoryFileSource` answers for the file panel (UI-F2.3), as
 * `GET /files/versions`, `/files/history` and `/files/history-size` do: every version with the
 * head, only the versions that changed a file, and each distinct content counted once.
 */
import { describe, expect, it } from "vitest";
import { MemoryFileSource } from "../../src/files/memory-source";
import type { FileRef } from "../../src/files/model";

const OBJECT = { library_id: "lib", object_id: "obj" };
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
    await s.remove(OBJECT, "top.png");
    const versions = await s.versions();
    expect(versions.head).toBe("v2");
    expect(versions.versions.map((v) => v.version)).toEqual(["v1", "v2"]);
    expect(versions.versions[1]?.created).toBe("2026-10-16T09:01:00.000Z");
  });

  it("lists only the versions that changed the file: added, removed and restored", async () => {
    const s = source();
    await s.remove(OBJECT, "top.png");
    await s.remove(OBJECT, "Docs/a.txt");
    await s.restore(OBJECT, "v1", ["Docs/a.txt"]);
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
    await s.remove(OBJECT, "Docs/a.txt");
    await s.restore(OBJECT, "v1", ["Docs/a.txt"]);
    expect(await s.historySize(FILE)).toMatchObject({ versions: 3, contents: 1, bytes: 5 });
  });

  it("refuses a file that never existed", async () => {
    const missing = { ...FILE, path: "nope.txt" };
    await expect(source().history(missing)).rejects.toMatchObject({ code: "not_found" });
  });
});
