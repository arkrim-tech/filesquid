/**
 * Unit tests for the pure file-browser rules (UI-F2.2): the row model that carries only display
 * names (UI-S-019), paths and names, size and date text from the label table, the route form of
 * a location, preview kinds and the limits of the file routes.
 */
import { describe, expect, it } from "vitest";
import {
  MAX_NAME_BYTES,
  MAX_PAGES,
  MAX_UPLOAD_BYTES,
  OVERSCAN_ROWS,
  PAGE_SIZE,
  TEXT_PREVIEW_BYTES,
  affectsPath,
  fileRows,
  formatModified,
  formatSize,
  joinPath,
  lastSegment,
  locationHash,
  parentPath,
  parseLocationHash,
  previewKind,
  previousVersion,
  prefixTrail,
  rowSummary,
  validateName,
  type FolderEntry,
} from "../../src/files/model";
import labels from "../../src/theme/labels.en.json";

const FILE: FolderEntry = {
  kind: "file",
  name: "0f8e9c1a-5b7d-4e21-9c3a-aa11bb22cc33.pdf",
  display_name: "harbour-masterplan-v7.pdf",
  path: "Contracts/0f8e9c1a-5b7d-4e21-9c3a-aa11bb22cc33.pdf",
  digest: "abc",
  size: 19_293_798,
  modified_version: "v7",
  modified: "2026-10-16T09:12:00Z",
  type_hint: "pdf",
};
const FOLDER: FolderEntry = { kind: "folder", name: "Drawings", path: "Drawings" };

describe("limits", () => {
  it("match the file routes", () => {
    expect(MAX_UPLOAD_BYTES).toBe(67_108_864);
    expect(TEXT_PREVIEW_BYTES).toBe(65_536);
    expect(PAGE_SIZE).toBe(200);
    expect(MAX_PAGES).toBe(500);
    expect(MAX_NAME_BYTES).toBe(255);
    expect(OVERSCAN_ROWS).toBe(5);
  });
});

describe("fileRows", () => {
  it("shows the display name of a file and never its stored name", () => {
    const [row] = fileRows([FILE], []);
    expect(row?.name).toBe("harbour-masterplan-v7.pdf");
    // The logical path is an identifier the UI sends back; only it and the key may hold the
    // stored name, and the UI renders neither (UI-S-019).
    const { key, path, ...shown } = row as NonNullable<typeof row>;
    expect([key, path]).toEqual([FILE.path, FILE.path]);
    expect(JSON.stringify(shown)).not.toContain("0f8e9c1a");
    expect(row).toMatchObject({
      key: FILE.path,
      kind: "file",
      size: 19_293_798,
      typeHint: "pdf",
      version: "v7",
      modified: "2026-10-16T09:12:00Z",
      pending: false,
    });
  });

  it("keeps the API order, with a folder named by its last segment", () => {
    const rows = fileRows([FOLDER, FILE], []);
    expect(rows.map((r) => [r.kind, r.name])).toEqual([
      ["folder", "Drawings"],
      ["file", "harbour-masterplan-v7.pdf"],
    ]);
  });

  it("adds pending folders after the listed folders and drops one the listing now holds", () => {
    const rows = fileRows([FOLDER, FILE], ["Drawings", "Photos"]);
    expect(rows.map((r) => [r.kind, r.name, r.pending])).toEqual([
      ["folder", "Drawings", false],
      ["folder", "Photos", true],
      ["file", "harbour-masterplan-v7.pdf", false],
    ]);
  });
});

describe("rowSummary", () => {
  it("counts folders, files and bytes", () => {
    expect(rowSummary(fileRows([FOLDER, FILE, FILE], []))).toEqual({
      folders: 1,
      files: 2,
      bytes: 38_587_596,
    });
  });
});

describe("paths", () => {
  it("join, split and walk", () => {
    expect(joinPath("", "a.txt")).toBe("a.txt");
    expect(joinPath("A/B", "a.txt")).toBe("A/B/a.txt");
    expect(parentPath("A/B/a.txt")).toBe("A/B");
    expect(parentPath("a.txt")).toBe("");
    expect(lastSegment("A/B/a.txt")).toBe("a.txt");
    expect(prefixTrail("A/B")).toEqual([
      { name: "A", prefix: "A" },
      { name: "B", prefix: "A/B" },
    ]);
    expect(prefixTrail("")).toEqual([]);
  });

  it("a change affects the path itself and everything below it", () => {
    expect(affectsPath("A", "A")).toBe(true);
    expect(affectsPath("A", "A/b.txt")).toBe(true);
    expect(affectsPath("A", "AB/b.txt")).toBe(false);
    expect(affectsPath("A/b.txt", "A")).toBe(false);
  });
});

describe("validateName", () => {
  it("accepts an ordinary name, trimmed", () => {
    expect(validateName("  report 2026.pdf ")).toBeUndefined();
  });
  it.each([
    ["", "empty"],
    ["   ", "empty"],
    ["a/b", "separator"],
    ["a\\b", "separator"],
    [".", "reserved"],
    ["..", "reserved"],
    ["a\u0000b", "control"],
    ["a\u001fb", "control"],
    ["é".repeat(128), "long"],
  ])("refuses %j as %s", (name, problem) => {
    expect(validateName(name)).toBe(problem);
  });
  it("counts bytes, not characters: 255 bytes pass and 256 do not", () => {
    expect(validateName("a".repeat(255))).toBeUndefined();
    expect(validateName("a".repeat(256))).toBe("long");
  });
});

describe("previousVersion", () => {
  it("steps back one OCFL version", () => {
    expect(previousVersion("v5")).toBe("v4");
    expect(previousVersion("v10")).toBe("v9");
    expect(previousVersion("v1")).toBeUndefined();
    expect(previousVersion("head")).toBeUndefined();
    expect(previousVersion("v0")).toBeUndefined();
  });
});

describe("formatSize", () => {
  it.each([
    [0, "0 B"],
    [999, "999 B"],
    [2048, "2 KB"],
    [421_888, "412 KB"],
    [6_481_920, "6.2 MB"],
    [19_293_798, "18.4 MB"],
    [1_288_490_189, "1.2 GB"],
    [2 ** 40 * 3, "3 TB"],
  ])("%d bytes is %s", (bytes, text) => {
    expect(formatSize(bytes, labels, "en")).toBe(text);
  });
  it("takes its words from the label table", () => {
    const own = { ...labels, sizeMB: "{value} Mo" };
    expect(formatSize(6_481_920, own, "en")).toBe("6.2 Mo");
  });
});

describe("formatModified", () => {
  it("writes the date the way the locale does", () => {
    expect(formatModified("2026-10-16T12:00:00Z", "en", "UTC")).toBe("Oct 16, 2026");
  });
  it("shows an unparseable time as given instead of a wrong date", () => {
    expect(formatModified("not a time", "en", "UTC")).toBe("not a time");
  });
});

describe("location route", () => {
  it("round-trips a location with folders", () => {
    const location = { library_id: "lib-1", object_id: "urn:uuid:1", prefix: "A b/c#d" };
    const hash = locationHash(location);
    expect(hash).toBe("#/files/lib-1/urn%3Auuid%3A1/A%20b/c%23d");
    expect(parseLocationHash(hash)).toEqual(location);
  });
  it("the root has no folder segments", () => {
    const location = { library_id: "l", object_id: "o", prefix: "" };
    expect(locationHash(location)).toBe("#/files/l/o");
    expect(parseLocationHash("#/files/l/o/")).toEqual(location);
  });
  it.each([
    "",
    "#/monitor",
    "#/files",
    "#/files/only-library",
    "#/files/l/o/../x",
    "#/files/l/o/%E0%A4%A",
  ])("does not read %j", (hash) => {
    expect(parseLocationHash(hash)).toBeUndefined();
  });
});

describe("previewKind", () => {
  it.each([
    ["image/png", "image"],
    ["image/jpeg", "image"],
    ["image/svg+xml", "image"],
    ["video/mp4", "video"],
    ["video/webm", "video"],
    ["audio/mpeg", "audio"],
    ["audio/wav", "audio"],
    ["text/plain; charset=utf-8", "text"],
    ["text/csv", "text"],
    ["application/json", "text"],
    ["application/pdf", undefined],
    ["application/x-blender", undefined],
    ["image/tiff", undefined],
    ["application/octet-stream", undefined],
  ])("%s previews as %s", (type, kind) => {
    expect(previewKind(type)).toBe(kind);
  });
});
