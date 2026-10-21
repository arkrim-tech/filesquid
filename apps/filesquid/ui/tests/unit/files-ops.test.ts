/**
 * Unit tests for the write rules of the file browser (UI-F2.2): the name a file is uploaded
 * under, one commit per file with the 64 MiB limit and a bound on a batch, where a rename or move
 * lands, and the words for a failed call.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  MAX_UPLOAD_FILES,
  failureText,
  fileIdOf,
  folderProblem,
  moveTarget,
  problemText,
  renameFailureText,
  renameFileId,
  nameNormalization,
  reportedMediaType,
  reportedModified,
  uploadFacts,
  uploadFiles,
  uploadName,
} from "../../src/files/ops";
import { MAX_UPLOAD_BYTES, type FileRow } from "../../src/files/model";
import { MemoryFileSource } from "../../src/files/memory-source";
import labels from "../../src/theme/labels.en.json";

const OBJECT = { library_id: "lib", object_id: "obj" };
/** 2026-10-20T08:00:00Z in milliseconds. */
const MODIFIED = Date.UTC(2026, 9, 20, 8, 0, 0);

afterEach(() => vi.restoreAllMocks());

describe("uploadName", () => {
  it.each([
    ["scan.pdf", "scan.pdf"],
    ["a/b\\c.txt", "a_b_c.txt"],
    ["  spaced.txt  ", "spaced.txt"],
    ["tab\there.txt", "tabhere.txt"],
    ["bell\u0007\u0085.txt", "bell.txt"],
    ["", "file"],
    [".", "file"],
    ["..", "file"],
    ["   ", "file"],
  ])("%j is uploaded as %j", (name, expected) => {
    expect(uploadName(name)).toBe(expected);
  });
});

describe("uploadFacts", () => {
  it("states the name, its normalisation form, the media type and the last-modified time", () => {
    const file = new File(["x"], "ignored.txt", { type: "image/jpeg", lastModified: MODIFIED });
    expect(uploadFacts(file, "Holiday 2023.JPG")).toEqual({
      name: "Holiday 2023.JPG",
      name_normalization: "nfc",
      media_type: "image/jpeg",
      last_modified: "2026-10-20T08:00:00.000Z",
    });
  });

  it.each([
    ["ascii.txt", "nfc"],
    ["Caf\u00e9.txt", "nfc"],
    ["Cafe\u0301.txt", "nfd"],
    ["\u00e9e\u0301.txt", "unnormalised"],
  ])("names %j as %s", (name, form) => {
    expect(nameNormalization(name)).toBe(form);
  });

  it.each([
    ["text/plain", "text/plain"],
    ["Image/JPEG", "Image/JPEG"],
    ["application/vnd.ms-excel", "application/vnd.ms-excel"],
    ["text/plain;charset=utf-8", "text/plain"],
    ["", undefined],
    ["text", undefined],
    ["text/", undefined],
    ["text/pl ain", undefined],
    ["+image/jpeg", undefined],
    ["image/.jpeg", undefined],
    [`a/${"b".repeat(128)}`, undefined],
  ])("reports the media type %j as %j", (type, expected) => {
    expect(reportedMediaType(type)).toBe(expected);
  });

  it.each([
    [MODIFIED, "2026-10-20T08:00:00.000Z"],
    [0, "1970-01-01T00:00:00.000Z"],
    [Number.NaN, undefined],
    [8.64e15 + 1, undefined],
    [Date.UTC(10000, 0, 1), undefined],
    [Date.UTC(-1, 0, 1), undefined],
  ])("reports the last-modified time %d as %j", (ms, expected) => {
    expect(reportedModified(ms)).toBe(expected);
  });

  it("leaves out a media type or time it cannot report", () => {
    const file = new File(["x"], "a", { type: "" });
    Object.defineProperty(file, "lastModified", { value: Number.NaN });
    expect(uploadFacts(file, "a")).toEqual({ name: "a", name_normalization: "nfc" });
  });
});

describe("uploadFiles", () => {
  /** A source holding nothing, with a spy on upload. */
  function empty() {
    const source = new MemoryFileSource({ ...OBJECT, files: [{ path: "seed.txt", content: "s" }] });
    return { source, upload: vi.spyOn(source, "upload") };
  }

  it("uploads one file at a time as its own file, in order, with its facts, reporting progress", async () => {
    const { source, upload } = empty();
    const seen: string[] = [];
    const files = [
      new File(["a"], "one.txt", { type: "text/plain", lastModified: MODIFIED }),
      new File(["b"], "two.txt"),
    ];
    const result = await uploadFiles(source, OBJECT, files, (i, n, name) =>
      seen.push(`${i}/${n} ${name}`),
    );
    expect(upload.mock.calls.map((c) => c[0].library_id)).toEqual(["lib", "lib"]);
    expect(upload.mock.calls.map((c) => c[1].name)).toEqual(["one.txt", "two.txt"]);
    expect(upload.mock.calls[0]?.[1]).toEqual({
      name: "one.txt",
      name_normalization: "nfc",
      media_type: "text/plain",
      last_modified: "2026-10-20T08:00:00.000Z",
    });
    expect(upload.mock.calls[0]?.[2]).toBe(files[0]);
    expect(seen).toEqual(["1/2 one.txt", "2/2 two.txt"]);
    expect(result.committed.map((c) => c.version)).toEqual(["v1", "v1"]);
    expect(new Set(result.committed.map((c) => c.file_id)).size).toBe(2);
    expect(result.failed).toEqual([]);
    expect(source.head).toBe("v1");
  });

  it("sends the cleaned display name, never a path", async () => {
    const { source, upload } = empty();
    await uploadFiles(source, OBJECT, [new File(["a"], "a/b\\c.txt")]);
    expect(upload.mock.calls[0]?.[1].name).toBe("a_b_c.txt");
  });

  it("refuses a file over 64 MiB without sending it and goes on with the rest", async () => {
    const { source, upload } = empty();
    const huge = new File(["x"], "huge.bin");
    Object.defineProperty(huge, "size", { value: MAX_UPLOAD_BYTES + 1 });
    const result = await uploadFiles(source, OBJECT, [huge, new File(["y"], "ok.txt")]);
    expect(upload).toHaveBeenCalledTimes(1);
    expect(result.failed).toMatchObject([{ name: "huge.bin", reason: "too_large" }]);
    expect(result.committed).toHaveLength(1);
  });

  it("records a rejected upload and goes on with the rest", async () => {
    const { source } = empty();
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const error = new ApiError(409, "/api/v1/files/upload", { code: "conflict" });
    vi.spyOn(source, "upload").mockRejectedValueOnce(error);
    const result = await uploadFiles(source, OBJECT, [
      new File(["a"], "a.txt"),
      new File(["b"], "b.txt"),
    ]);
    expect(result.failed).toHaveLength(1);
    expect(result.failed[0]?.reason).toEqual({ error });
    expect(result.committed).toHaveLength(1);
  });

  it("refuses a name longer than 255 bytes without sending it", async () => {
    const { source, upload } = empty();
    const long = new File(["x"], `${"a".repeat(256)}.txt`);
    const result = await uploadFiles(source, OBJECT, [long]);
    expect(upload).not.toHaveBeenCalled();
    expect(result.failed).toMatchObject([{ reason: "name_too_long" }]);
  });

  it("hands back the files it did not try", async () => {
    const { source, upload } = empty();
    upload.mockResolvedValue({ ...OBJECT, file_id: "f", version: "v1", created: "t" });
    const files = Array.from(
      { length: MAX_UPLOAD_FILES + 2 },
      (_, i) => new File(["a"], `f${i}.txt`),
    );
    const result = await uploadFiles(source, OBJECT, files);
    expect(result.left.map((f) => f.name)).toEqual([
      `f${MAX_UPLOAD_FILES}.txt`,
      `f${MAX_UPLOAD_FILES + 1}.txt`,
    ]);
  });

  it("tries at most MAX_UPLOAD_FILES files and reports how many it left", async () => {
    const { source, upload } = empty();
    upload.mockResolvedValue({ ...OBJECT, file_id: "f", version: "v1", created: "t" });
    const files = Array.from(
      { length: MAX_UPLOAD_FILES + 3 },
      (_, i) => new File(["a"], `f${i}.txt`),
    );
    const result = await uploadFiles(source, OBJECT, files);
    expect(upload).toHaveBeenCalledTimes(MAX_UPLOAD_FILES);
    expect(result.skipped).toBe(3);
  });
});

describe("moveTarget", () => {
  const file: FileRow = {
    key: "Docs/3f9a.pdf",
    kind: "file",
    name: "plan.pdf",
    path: "Docs/3f9a.pdf",
    pending: false,
    typeHint: "pdf",
  };

  it("keeps the stored name when the name the user sees is unchanged", () => {
    expect(moveTarget(file, "plan.pdf", "Archive")).toBe("Archive/3f9a.pdf");
    expect(moveTarget(file, " plan.pdf ", "Archive")).toBe("Archive/3f9a.pdf");
  });
  it("uses the typed name when it differs", () => {
    expect(moveTarget(file, "plan-final.pdf", "Docs")).toBe("Docs/plan-final.pdf");
    expect(moveTarget(file, "plan-final.pdf", "")).toBe("plan-final.pdf");
  });
  it("answers nothing when the path would not change", () => {
    expect(moveTarget(file, "plan.pdf", "Docs")).toBeUndefined();
    expect(moveTarget(file, "3f9a.pdf", "Docs")).toBeUndefined();
  });
  it("moves a folder by name", () => {
    const folder: FileRow = { ...file, kind: "folder", name: "Docs", path: "Docs", key: "Docs" };
    expect(moveTarget(folder, "Papers", "")).toBe("Papers");
  });
});

describe("folderProblem and problemText", () => {
  it("accepts the root and a path of usable names", () => {
    expect(folderProblem("")).toBeUndefined();
    expect(folderProblem("  ")).toBeUndefined();
    expect(folderProblem("A/B c")).toBeUndefined();
  });
  it("names the first unusable segment", () => {
    expect(folderProblem("A//B")).toBe("empty");
    expect(folderProblem("A/../B")).toBe("reserved");
    expect(folderProblem("A\\B")).toBe("separator");
  });
  it("words each problem from the label table", () => {
    expect(problemText("empty", labels)).toBe(labels.fileNameEmpty);
    expect(problemText("long", labels)).toBe(labels.fileNameTooLong);
    expect(problemText("separator", labels)).toBe(labels.fileNameInvalid);
    expect(problemText("reserved", labels)).toBe(labels.fileNameInvalid);
    expect(problemText("control", labels)).toBe(labels.fileNameInvalid);
  });
});

describe("failureText", () => {
  it("words a denial, a conflict and anything else from the label table", () => {
    const denied = new ApiError(403, "/x", { code: "forbidden", reason: "invalid_credentials" });
    expect(failureText(denied, labels)).toBe(labels.authInvalidCredentials);
    expect(failureText(new ApiError(409, "/x", { code: "conflict" }), labels)).toBe(
      labels.fileConflict,
    );
    expect(failureText(new ApiError(500, "/x", { code: "internal" }), labels)).toBe(
      labels.fileActionFailed,
    );
    expect(failureText(new Error("boom"), labels)).toBe(labels.fileActionFailed);
  });
});

describe("renaming a file (API-S-027)", () => {
  const ID = "5b1d2c3e-7a4f-4e8b-9c6d-0a1b2c3d4e5f";
  const row = (path: string, kind: "file" | "folder" = "file"): FileRow => ({
    key: path,
    kind,
    name: "readme.txt",
    path,
    pending: false,
    typeHint: "txt",
  });

  it("fileIdOf reads the file's id from its own object's identifier", () => {
    expect(fileIdOf(`urn:uuid:${ID}`)).toBe(ID);
    expect(fileIdOf("obj")).toBeUndefined();
    expect(fileIdOf("urn:uuid:not-a-uuid")).toBeUndefined();
  });

  it("renameFileId answers only for a file's original in its own object", () => {
    const object = `urn:uuid:${ID}`;
    expect(renameFileId(row(`original/${ID}.txt`), object)).toBe(ID);
    expect(renameFileId(row("original", "folder"), object)).toBeUndefined();
    expect(renameFileId(row("Docs/a.txt"), object)).toBeUndefined();
    expect(renameFileId(row(`original/${ID}.txt`), "obj")).toBeUndefined();
  });

  it("renameFailureText words each refusal of the route from the label table", () => {
    const refuse = (status: number, code: string) =>
      renameFailureText(new ApiError(status, "/x", { code: code as never }), labels);
    expect(refuse(400, "invalid_request")).toBe(labels.fileRenameInvalid);
    expect(refuse(404, "not_found")).toBe(labels.fileRenameNotFound);
    expect(refuse(409, "conflict")).toBe(labels.fileRenameConflict);
    expect(refuse(503, "unavailable")).toBe(labels.fileRenameUnavailable);
    expect(refuse(500, "internal")).toBe(labels.fileActionFailed);
    const denied = new ApiError(403, "/x", { code: "forbidden", reason: "not_permitted" });
    expect(renameFailureText(denied, labels)).toBe(labels.authNotPermitted);
  });
});
