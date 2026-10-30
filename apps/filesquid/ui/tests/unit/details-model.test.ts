/**
 * Unit tests for the File info rows of the Details panel (UI-F2.3, UI-S-003, UI-S-031 35):
 * storage facts only, from `GET /files/details` and the version total of `GET /files/versions`,
 * worded from the label table; the stored name and the logical path never appear (UI-S-019).
 */
import { describe, expect, it } from "vitest";
import { detailRows, digestRows } from "../../src/files/details";
import type { FileDetails } from "../../src/files/model";
import type { ObjectVersions } from "../../src/files/history-source";
import labels from "../../src/theme/labels.en.json";

const OBJECT = "urn:uuid:7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8";

/** Details of the mockup's facade-study.jpg. */
function details(overrides: Partial<FileDetails> = {}): FileDetails {
  return {
    library_id: "lib",
    object_id: OBJECT,
    version: "v4",
    path: "original/9f2c41aa.jpg",
    name: "9f2c41aa.jpg",
    display_name: "facade-study.jpg",
    digest: "5a".repeat(64),
    digest_algorithm: "sha512",
    size: 6481920,
    fixity: { sha256: ["9b1f4e" + "0".repeat(52) + "c07a2d"] },
    last_change: { version: "v4", created: "2026-10-16T11:05:00Z", message: null, user: null },
    type_hint: "jpg",
    media_type: "image/jpeg",
    ...overrides,
  };
}

/** Four versions, the head v4. */
const VERSIONS: ObjectVersions = {
  library_id: "lib",
  object_id: OBJECT,
  head: "v4",
  display_name: "facade-study.jpg",
  versions: ["v1", "v2", "v3", "v4"].map((version, i) => ({
    version,
    created: `2026-10-1${3 + i}T11:05:00Z`,
    message: null,
    user: null,
  })),
};

describe("detailRows", () => {
  it("words the storage facts in the mockup's order", () => {
    const rows = detailRows(details(), VERSIONS, labels, "en");
    expect(rows.map((r) => r.label)).toEqual([
      "Original file name",
      "Extension",
      "Format and MIME type",
      "Size",
      "SHA-256 digest",
      "SHA-512 digest",
      "OCFL object",
      "OCFL version",
    ]);
    expect(rows.map((r) => r.value)).toEqual([
      "facade-study.jpg",
      ".jpg",
      "image/jpeg",
      "6.2 MB · 6,481,920 bytes",
      "9b1f4e…c07a2d",
      "5a5a5a…5a5a5a",
      "urn:uuid:7f3c9a1e-…-41d2b8",
      "v4 of 4 · Oct 16, 2026, 11:05 UTC",
    ]);
  });

  it("copies the whole digest and identifier, and marks digests, identifiers and the extension mono", () => {
    const rows = detailRows(details(), VERSIONS, labels, "en");
    const byLabel = new Map(rows.map((r) => [r.label, r]));
    expect(byLabel.get("SHA-256 digest")?.copy).toBe("9b1f4e" + "0".repeat(52) + "c07a2d");
    expect(byLabel.get("OCFL object")?.copy).toBe(OBJECT);
    expect(rows.filter((r) => r.mono).map((r) => r.label)).toEqual([
      "Extension",
      "SHA-256 digest",
      "SHA-512 digest",
      "OCFL object",
    ]);
    expect(rows.every((r) => r.key !== "")).toBe(true);
  });

  it("never shows the stored name or the logical path", () => {
    const text = JSON.stringify(detailRows(details(), VERSIONS, labels, "en"));
    expect(text).not.toContain("9f2c41aa");
    expect(text).not.toContain("original/");
  });

  it("leaves out the extension row when the file has no extension hint", () => {
    const rows = detailRows(details({ type_hint: null }), VERSIONS, labels, "en");
    expect(rows.some((r) => r.label === "Extension")).toBe(false);
  });

  it("dates the version from the object's versions in UTC", () => {
    const rows = detailRows(details({ version: "v2" }), VERSIONS, labels, "en-GB");
    expect(rows.find((r) => r.label === "OCFL version")?.value).toBe(
      "v2 of 4 · 14 Oct 2026, 11:05 UTC",
    );
  });
});

describe("digestRows", () => {
  it("shows SHA-256 first, then the content digest, then other fixity values, each once", () => {
    const rows = digestRows(
      details({
        digest_algorithm: "sha256",
        digest: "ab".repeat(32),
        fixity: { md5: ["cd".repeat(16)], sha256: ["ab".repeat(32)] },
      }),
      labels,
    );
    expect(rows.map((r) => r.label)).toEqual(["SHA-256 digest", "MD5 digest"]);
  });

  it("keeps at most eight fixity algorithms besides the content digest, SHA-256 among them", () => {
    const fixity = Object.fromEntries(
      Array.from({ length: 20 }, (_, i) => [`alg${i}`, [`${i}`.repeat(20)]]),
    );
    expect(digestRows(details({ fixity }), labels).length).toBeLessThanOrEqual(9);
    const withSha = { ...fixity, sha256: ["ab".repeat(32)] };
    const rows = digestRows(details({ fixity: withSha }), labels);
    expect(rows.length).toBe(9);
    expect(rows[0]?.label).toBe("SHA-256 digest");
  });
});

describe("detailRows history size (UI-F2.11, PRD F-065)", () => {
  it("adds a History size row after Size with the bytes the history keeps and the versions", () => {
    const rows = detailRows(details(), VERSIONS, labels, "en", { versions: 4, bytes: 25_165_824 });
    expect(rows.map((r) => r.label).slice(3, 5)).toEqual(["Size", "History size"]);
    expect(rows.find((r) => r.key === "history-size")?.value).toBe("24 MB · 4 versions");
  });

  it("leaves the row out when the history size could not be read", () => {
    const rows = detailRows(details(), VERSIONS, labels, "en", undefined);
    expect(rows.some((r) => r.key === "history-size")).toBe(false);
  });
});
