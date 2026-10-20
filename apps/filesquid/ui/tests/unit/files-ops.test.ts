/**
 * Unit tests for the write rules of the file browser (UI-F2.2): the name a file is uploaded
 * under, one commit per file with the 64 MiB limit and a bound on a batch, where a rename or move
 * lands, and the words for a failed call.
 */
import { describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  MAX_UPLOAD_FILES,
  failureText,
  folderProblem,
  moveTarget,
  problemText,
  uploadFiles,
  uploadName,
} from "../../src/files/ops";
import { MAX_UPLOAD_BYTES, type FileRow } from "../../src/files/model";
import { MemoryFileSource } from "../../src/files/memory-source";
import labels from "../../src/theme/labels.en.json";

const OBJECT = { library_id: "lib", object_id: "obj" };

describe("uploadName", () => {
  it.each([
    ["scan.pdf", "scan.pdf"],
    ["a/b\\c.txt", "a_b_c.txt"],
    ["  spaced.txt  ", "spaced.txt"],
    ["tab\there.txt", "tab_here.txt"],
    ["", "file"],
    [".", "file"],
    ["..", "file"],
    ["   ", "file"],
  ])("%j is uploaded as %j", (name, expected) => {
    expect(uploadName(name)).toBe(expected);
  });
});

describe("uploadFiles", () => {
  /** A source holding nothing, with a spy on upload. */
  function empty() {
    const source = new MemoryFileSource({ ...OBJECT, files: [{ path: "seed.txt", content: "s" }] });
    return { source, upload: vi.spyOn(source, "upload") };
  }

  it("uploads one file at a time into the prefix, in order, reporting progress", async () => {
    const { source, upload } = empty();
    const seen: string[] = [];
    const files = [new File(["a"], "one.txt"), new File(["b"], "two.txt")];
    const result = await uploadFiles(source, OBJECT, "Docs", files, (i, n, name) =>
      seen.push(`${i}/${n} ${name}`),
    );
    expect(upload.mock.calls.map((c) => c[1])).toEqual(["Docs/one.txt", "Docs/two.txt"]);
    expect(seen).toEqual(["1/2 one.txt", "2/2 two.txt"]);
    expect(result.committed.map((c) => c.version)).toEqual(["v2", "v3"]);
    expect(result.failed).toEqual([]);
  });

  it("refuses a file over 64 MiB without sending it and goes on with the rest", async () => {
    const { source, upload } = empty();
    const huge = new File(["x"], "huge.bin");
    Object.defineProperty(huge, "size", { value: MAX_UPLOAD_BYTES + 1 });
    const result = await uploadFiles(source, OBJECT, "", [huge, new File(["y"], "ok.txt")]);
    expect(upload).toHaveBeenCalledTimes(1);
    expect(result.failed).toMatchObject([{ name: "huge.bin", reason: "too_large" }]);
    expect(result.committed).toHaveLength(1);
  });

  it("records a rejected upload and goes on with the rest", async () => {
    const { source } = empty();
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const error = new ApiError(409, "/api/v1/files/upload", { code: "conflict" });
    vi.spyOn(source, "upload").mockRejectedValueOnce(error);
    const result = await uploadFiles(source, OBJECT, "", [
      new File(["a"], "a.txt"),
      new File(["b"], "b.txt"),
    ]);
    expect(result.failed).toHaveLength(1);
    expect(result.failed[0]?.reason).toEqual({ error });
    expect(result.committed).toHaveLength(1);
    vi.restoreAllMocks();
  });

  it("tries at most MAX_UPLOAD_FILES files and reports how many it left", async () => {
    const { source, upload } = empty();
    upload.mockResolvedValue({ ...OBJECT, version: "v2", created: "t" });
    const files = Array.from(
      { length: MAX_UPLOAD_FILES + 3 },
      (_, i) => new File(["a"], `f${i}.txt`),
    );
    const result = await uploadFiles(source, OBJECT, "", files);
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
