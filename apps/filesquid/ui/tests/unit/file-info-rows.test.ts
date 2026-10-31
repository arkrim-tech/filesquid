/**
 * Unit tests for the File info rows of the facts projection (UI-F2.15; UI-S-022, UI-S-031
 * mockup 35, API-S-027, ADR 038): the original name, extension, format and MIME type and
 * SHA-256 of `GET /files/facts`, the import facts per acquisition (Uploaded, Source, the
 * import-relative path only when known), and the technical rows read from the file's `exif:`
 * statements. The absolute path, the import root, host and account names have no row.
 */
import { describe, expect, it } from "vitest";
import labels from "../../src/theme/labels.en.json";
import { detailRows } from "../../src/files/details";
import type { FileFacts } from "../../src/files/facts-source";
import type { FileDetails } from "../../src/files/model";
import type { ObjectVersions } from "../../src/files/history-source";
import type { MetadataStatement } from "../../src/files/metadata";

const SHA = `9b1f4e${"0".repeat(52)}c07a2d`;

const DETAILS = {
  library_id: "lib",
  object_id: "urn:uuid:7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8",
  version: "v4",
  path: "original/7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8.jpg",
  name: "7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8.jpg",
  display_name: "facade-study.jpg",
  digest: "5a".repeat(64),
  digest_algorithm: "sha512",
  size: 6_481_920,
  fixity: { sha256: [SHA] },
  last_change: { version: "v4", created: "2026-10-16T11:05:00Z", message: null, user: null },
  type_hint: "jpg",
  media_type: "image/jpeg",
} as unknown as FileDetails;

const VERSIONS = {
  library_id: "lib",
  object_id: DETAILS.object_id,
  head: "v4",
  versions: [1, 2, 3, 4].map((n) => ({
    version: `v${n}`,
    created: `2026-10-1${n}T11:05:00Z`,
    message: null,
    user: null,
    label: null,
  })),
} as unknown as ObjectVersions;

function facts(over: Partial<FileFacts> = {}): FileFacts {
  return {
    original_name: "facade-study.jpg",
    display_name: "facade-study.jpg",
    extension: "jpg",
    format: "JPEG image",
    media_type: "image/jpeg",
    size: 6_481_920,
    sha256: SHA,
    imports: [
      {
        imported_at: "2026-10-12T14:20:00Z",
        method: "folder",
        storage: "copied",
        relative_path: null,
        volume_name: "FUJIFILM",
        object_id: DETAILS.object_id,
        object_version: "v1",
      },
    ],
    ...over,
  } as FileFacts;
}

/** The label and value of each row. */
function pairs(rows: ReturnType<typeof detailRows>): [string, string][] {
  return rows.map((r) => [r.label, r.value]);
}

describe("detailRows with the facts projection", () => {
  it("draws the rows of mockup 35 in order", () => {
    const rows = detailRows(DETAILS, VERSIONS, labels, "en", undefined, { facts: facts() });
    expect(rows.map((r) => r.key)).toEqual([
      "name",
      "extension",
      "format",
      "size",
      "digest-sha256",
      "digest-sha512",
      "uploaded",
      "source",
      "object",
      "version",
    ]);
    const shown = Object.fromEntries(pairs(rows));
    expect(shown["Original file name"]).toBe("facade-study.jpg");
    expect(shown["Format and MIME type"]).toBe("JPEG image · image/jpeg");
    expect(shown["Source"]).toBe("Folder upload · volume FUJIFILM");
    expect(shown["Uploaded"]).toMatch(/^Oct 12, 2026, \d\d:\d\d \(14:20 UTC\)$/);
  });

  it("takes the SHA-256 row from the facts and keeps its whole value for copy", () => {
    const rows = detailRows(DETAILS, VERSIONS, labels, "en", undefined, { facts: facts() });
    const sha = rows.find((r) => r.key === "digest-sha256");
    expect(sha).toMatchObject({ value: "9b1f4e…c07a2d", copy: SHA, mono: true });
  });

  it("shows the import-relative path only when the API knows it", () => {
    const withPath = facts({
      imports: [
        { ...(facts().imports[0] as object), relative_path: "Harbour project/a.jpg" } as never,
      ],
    });
    const rows = detailRows(DETAILS, VERSIONS, labels, "en", undefined, { facts: withPath });
    expect(rows.find((r) => r.key === "import-path")).toMatchObject({
      label: "Imported from",
      value: "Harbour project/a.jpg",
      mono: true,
    });
    expect(
      detailRows(DETAILS, VERSIONS, labels, "en", undefined, { facts: facts() }).some(
        (r) => r.key === "import-path",
      ),
    ).toBe(false);
  });

  it("leaves the volume out of the Source row when it is unknown, and words every method", () => {
    const base = facts().imports[0] as never;
    const one = (method: string, volume: string | null) =>
      detailRows(DETAILS, VERSIONS, labels, "en", undefined, {
        facts: facts({ imports: [{ ...(base as object), method, volume_name: volume } as never] }),
      }).find((r) => r.key === "source")?.value;
    expect(one("file_picker", null)).toBe("Files upload");
    expect(one("drag_and_drop", "Disk")).toBe("Files upload · dragged and dropped · volume Disk");
    expect(one("other_application", null)).toBe("Shared from another application");
    expect(one("api_upload", null)).toBe("Files upload");
  });

  it("draws one Uploaded and Source pair per acquisition, at most ten", () => {
    const base = facts().imports[0] as never;
    const many = Array.from({ length: 12 }, () => base);
    const rows = detailRows(DETAILS, VERSIONS, labels, "en", undefined, {
      facts: facts({ imports: many }),
    });
    expect(rows.filter((r) => r.key.startsWith("uploaded"))).toHaveLength(10);
    expect(rows.filter((r) => r.key.startsWith("source"))).toHaveLength(10);
  });

  it("shows a name that is not UTF-8 as its base64 bytes", () => {
    const rows = detailRows(DETAILS, VERSIONS, labels, "en", undefined, {
      facts: facts({ original_name: { encoding: "base64", value: "/w==" } }),
    });
    expect(rows[0]).toMatchObject({ value: "base64: /w==", mono: true });
  });

  it("falls back to the storage facts when there is no facts projection, with no import rows", () => {
    const rows = detailRows(DETAILS, VERSIONS, labels, "en");
    expect(rows.map((r) => r.key)).toEqual([
      "name",
      "extension",
      "format",
      "size",
      "digest-sha256",
      "digest-sha512",
      "object",
      "version",
    ]);
  });

  it("never carries an absolute path, an import root or a host or account name", () => {
    const hostile = facts() as unknown as Record<string, unknown>;
    hostile.import_root = "/Users/jane/Pictures";
    hostile.host = "jane-macbook";
    const text = JSON.stringify(
      detailRows(DETAILS, VERSIONS, labels, "en", undefined, { facts: hostile as never }),
    );
    for (const secret of ["/Users/jane", "jane-macbook"]) expect(text).not.toContain(secret);
  });
});

describe("technical rows", () => {
  const statement = (
    curie: string,
    value: string,
    source: "extracted" | "asserted" = "extracted",
  ): MetadataStatement =>
    ({
      curie,
      prefix: curie.split(":")[0],
      term: curie.split(":")[1],
      value,
      source,
    }) as MetadataStatement;

  it("lists the exif statements in their own group, after the properties, and not in the count", () => {
    const rows = detailRows(DETAILS, VERSIONS, labels, "en", undefined, {
      facts: facts(),
      technical: [statement("exif:Duration", "6:18"), statement("exif:PixelXDimension", "6000")],
    });
    const technical = rows.filter((r) => r.group);
    expect(technical.map((r) => r.label)).toEqual(["Duration", "Pixel X Dimension"]);
    expect(technical.every((r) => r.group === "Technical · read from the file")).toBe(true);
    expect(technical[0]).toMatchObject({ value: "6:18", key: "technical-Duration" });
    expect(rows.slice(-2)).toEqual(technical);
  });

  it("says nothing could be read when the file has no exif statements", () => {
    const rows = detailRows(DETAILS, VERSIONS, labels, "en", undefined, {
      facts: facts(),
      technical: [],
    });
    const note = rows.at(-1);
    expect(note).toMatchObject({
      key: "technical-none",
      value: "Nothing could be read from this format.",
      display: "note",
      group: "Technical · read from the file",
    });
  });

  it("draws no technical group when the statements could not be read at all", () => {
    const rows = detailRows(DETAILS, VERSIONS, labels, "en", undefined, { facts: facts() });
    expect(rows.some((r) => r.group)).toBe(false);
  });

  it("ignores statements of other prefixes and bounds the rows", () => {
    const many = Array.from({ length: 80 }, (_, i) => statement(`exif:Tag${i}`, String(i)));
    const rows = detailRows(DETAILS, VERSIONS, labels, "en", undefined, {
      technical: [statement("dcterms:title", "x", "asserted"), ...many],
    });
    expect(rows.filter((r) => r.group)).toHaveLength(50);
    expect(rows.some((r) => r.label === "title")).toBe(false);
  });
});
