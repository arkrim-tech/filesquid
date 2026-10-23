/**
 * UI tests for the writes of `<fs-file-browser>` (UI-F2.2): upload (button and drag and drop),
 * new folder, rename or move, remove and restore. Each is one API call that commits one OCFL
 * version, only a committed answer is reported as done, a refusal is shown with its label, and
 * the listing shows display names throughout (UI-S-019).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { MemoryFileSource } from "../../src/files/memory-source";
import type { FsFileBrowser } from "../../src/index";
import { MAX_UPLOAD_BYTES, type FileCommit, type FolderAnswer } from "../../src/files/model";
import {
  FILE_ID,
  FILE_LOCATION,
  LOCATION,
  STORED_NAMES,
  all,
  click,
  control,
  deepText,
  fileObjectFixture,
  fixture,
  idle,
  mount,
  names,
  one,
  rootOf,
  settle,
} from "../support/file-browser-fakes";
import labels from "../../src/theme/labels.en.json";

beforeEach(() => {
  vi.spyOn(console, "warn").mockImplementation(() => undefined);
});
afterEach(() => {
  vi.restoreAllMocks();
  document.body.replaceChildren();
});

/** Choose files in the hidden upload input. */
async function choose(browser: FsFileBrowser, files: File[]): Promise<void> {
  const input = one<HTMLInputElement>(browser, 'input[type="file"]');
  Object.defineProperty(input, "files", { value: files, configurable: true });
  input.dispatchEvent(new Event("change", { bubbles: true }));
  await idle(browser);
}

/** The status and alert texts. */
const status = (browser: FsFileBrowser) => one(browser, ".status").textContent?.trim();
const alert = (browser: FsFileBrowser) => one(browser, ".alert").textContent?.trim();

/** Type into a field of the name form and submit it. */
async function submit(browser: FsFileBrowser, fields: Record<string, string>): Promise<void> {
  for (const [name, value] of Object.entries(fields)) {
    one<HTMLInputElement>(browser, `form.name-form input[name="${name}"]`).value = value;
  }
  one<HTMLFormElement>(browser, "form.name-form").dispatchEvent(
    new Event("submit", { bubbles: true, cancelable: true }),
  );
  await idle(browser);
}

describe("upload", () => {
  it("uploads each chosen file with one PUT as its own file and reports the committed version", async () => {
    const source = fixture();
    const upload = vi.spyOn(source, "upload");
    const browser = await mount(source, { location: { ...LOCATION, prefix: "Contracts" } });
    const events: unknown[] = [];
    browser.addEventListener("fs-files-change", (e) => events.push((e as CustomEvent).detail));
    await choose(browser, [new File(["abc"], "scan.pdf"), new File(["defg"], "memo.pdf")]);
    expect(upload).toHaveBeenCalledTimes(2);
    expect(upload.mock.calls.map((c) => [c[0].library_id, c[1].name])).toEqual([
      ["lib", "scan.pdf"],
      ["lib", "memo.pdf"],
    ]);
    expect(upload.mock.calls.every((c) => !("path" in c[1]) && !("file_id" in c[1]))).toBe(true);
    expect(status(browser)).toBe("Uploaded 2. Version v1 committed.");
    expect(source.head).toBe("v1");
    expect(source.uploads.map((u) => u.name)).toEqual(["scan.pdf", "memo.pdf"]);
    expect(names(browser)).toEqual(["harbour-masterplan-v7.pdf", "tender-response.pdf"]);
    const file = source.uploads[1]?.file_id;
    expect(events).toEqual([
      expect.objectContaining({
        operation: "upload",
        commit: expect.objectContaining({ object_id: `urn:uuid:${file}`, version: "v1" }),
      }),
    ]);
  });

  it("refuses a file over 64 MiB before sending it and says which", async () => {
    const source = fixture();
    const upload = vi.spyOn(source, "upload");
    const browser = await mount(source);
    const big = new File(["x"], "huge.mov");
    Object.defineProperty(big, "size", { value: MAX_UPLOAD_BYTES + 1 });
    await choose(browser, [big]);
    expect(upload).not.toHaveBeenCalled();
    expect(alert(browser)).toContain("huge.mov is larger than 64 MB");
  });

  it("accepts a file of exactly 64 MiB", async () => {
    const source = fixture();
    const upload = vi.spyOn(source, "upload").mockResolvedValue({
      library_id: "lib",
      file_id: "f",
      object_id: "urn:uuid:f",
      version: "v1",
      created: "t",
    });
    const browser = await mount(source);
    const edge = new File(["x"], "edge.bin");
    Object.defineProperty(edge, "size", { value: MAX_UPLOAD_BYTES });
    await choose(browser, [edge]);
    expect(upload).toHaveBeenCalledTimes(1);
  });

  it("names the files that failed, keeps the ones that committed and offers to try again", async () => {
    const source = fixture();
    const real = source.upload.bind(source);
    let calls = 0;
    vi.spyOn(source, "upload").mockImplementation(async (...args) => {
      calls += 1;
      if (calls === 1) throw new ApiError(409, "/api/v1/files/upload", { code: "conflict" });
      return real(...args);
    });
    const browser = await mount(source);
    await choose(browser, [new File(["a"], "one.txt"), new File(["b"], "two.txt")]);
    expect(alert(browser)).toContain("Not uploaded: one.txt");
    expect(alert(browser)).toContain(labels.fileConflict);
    expect(source.uploads.map((u) => u.name)).toEqual(["two.txt"]);
    await click(one(browser, "button.retry"));
    await idle(browser);
    expect(source.uploads.map((u) => u.name)).toEqual(["two.txt", "one.txt"]);
  });

  it("tries the failed files again in the library they were meant for, after moving away", async () => {
    const source = fixture();
    const real = source.upload.bind(source);
    let calls = 0;
    const upload = vi.spyOn(source, "upload").mockImplementation(async (...args) => {
      calls += 1;
      if (calls === 1) throw new ApiError(409, "/api/v1/files/upload", { code: "conflict" });
      return real(...args);
    });
    const browser = await mount(source, { location: { ...LOCATION, prefix: "Contracts" } });
    await choose(browser, [new File(["a"], "one.txt")]);
    browser.location = { ...LOCATION, prefix: "Drawings" };
    await settle();
    await click(one(browser, "button.retry"));
    await idle(browser);
    expect(upload.mock.calls.map((c) => [c[0].library_id, c[1].name])).toEqual([
      ["lib", "one.txt"],
      ["lib", "one.txt"],
    ]);
  });

  it("shows a drop target for dragged files and uploads what is dropped", async () => {
    const source = fixture();
    const upload = vi.spyOn(source, "upload");
    const browser = await mount(source, { location: { ...LOCATION, prefix: "Drawings" } });
    const listing = one(browser, ".listing");
    const drag = (type: string, files: File[]) => {
      const event = new Event(type, { bubbles: true, cancelable: true });
      Object.defineProperty(event, "dataTransfer", {
        value: { types: ["Files"], files },
      });
      listing.dispatchEvent(event);
      return event;
    };
    const over = drag("dragover", []);
    expect(over.defaultPrevented).toBe(true);
    expect(one(browser, ".drop").hidden).toBe(false);
    expect(one(browser, ".drop").textContent).toBe("Drop to upload into Drawings");
    drag("drop", [new File(["z"], "dropped.txt")]);
    await idle(browser);
    expect(upload.mock.calls[0]?.[1].name).toBe("dropped.txt");
    expect(one(browser, ".drop").hidden).toBe(true);
  });

  it("hides the drop target when the drag leaves the listing", async () => {
    const browser = await mount(fixture());
    const listing = one(browser, ".listing");
    const over = new Event("dragover", { bubbles: true, cancelable: true });
    Object.defineProperty(over, "dataTransfer", { value: { types: ["Files"], files: [] } });
    listing.dispatchEvent(over);
    expect(one(browser, ".drop").hidden).toBe(false);
    listing.dispatchEvent(new Event("dragleave", { bubbles: true }));
    expect(one(browser, ".drop").hidden).toBe(true);
  });

  it("ignores a drag that carries no files", async () => {
    const browser = await mount(fixture());
    const event = new Event("dragover", { bubbles: true, cancelable: true });
    Object.defineProperty(event, "dataTransfer", { value: { types: ["text/plain"], files: [] } });
    one(browser, ".listing").dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
    expect(one(browser, ".drop").hidden).toBe(true);
  });

  it("says how many files a batch left out and offers them again", async () => {
    const source = fixture();
    const upload = vi.spyOn(source, "upload").mockResolvedValue({
      library_id: "lib",
      file_id: "f",
      object_id: "urn:uuid:f",
      version: "v1",
      created: "t",
    });
    const browser = await mount(source);
    const files = Array.from({ length: 1002 }, (_, i) => new File(["a"], `f${i}.txt`));
    await choose(browser, files);
    expect(upload).toHaveBeenCalledTimes(1000);
    expect(alert(browser)).toContain(
      "2 files were not uploaded because one batch holds at most 1000.",
    );
    await click(one(browser, "button.retry"));
    await idle(browser);
    expect(upload).toHaveBeenCalledTimes(1002);
  });

  it("refuses a name longer than 255 bytes before sending it", async () => {
    const source = fixture();
    const upload = vi.spyOn(source, "upload");
    const browser = await mount(source);
    await choose(browser, [new File(["a"], `${"n".repeat(256)}.txt`)]);
    expect(upload).not.toHaveBeenCalled();
    expect(alert(browser)).toContain(labels.fileNameTooLong);
  });

  it("strips path separators from the name it uploads under", async () => {
    const source = fixture();
    const upload = vi.spyOn(source, "upload");
    const browser = await mount(source);
    await choose(browser, [new File(["a"], "a/b\\c.txt")]);
    expect(upload.mock.calls[0]?.[1].name).toBe("a_b_c.txt");
  });
});

describe("new folder", () => {
  it("opens a named form with focus in the field, creates the folder and shows it pending", async () => {
    const source = fixture();
    const create = vi.spyOn(source, "createFolder");
    const browser = await mount(source);
    const opener = one(browser, "button.new-folder");
    await click(opener);
    const form = one(browser, "form.name-form");
    expect(form.hidden).toBe(false);
    expect(form.getAttribute("aria-label")).toBe("New folder in All files");
    expect(form.querySelector(".form-title")?.textContent).toBe("New folder in All files");
    const field = one<HTMLInputElement>(browser, "form.name-form input[name=name]");
    expect(rootOf(browser).activeElement).toBe(field);
    await submit(browser, { name: "Photos" });
    expect(create).toHaveBeenCalledWith(expect.anything(), "Photos");
    expect(status(browser)).toBe("Folder Photos created. It stays pending until a file is added.");
    const pending = control(browser, "Photos").closest(".row") as HTMLElement;
    expect(pending.querySelector(".pending")?.textContent).toBe(labels.fileFolderPending);
    expect(form.hidden).toBe(true);
    expect(source.head).toBe("v1");
  });

  it("opens a pending folder as an empty folder, not an error", async () => {
    const browser = await mount(fixture());
    await click(one(browser, "button.new-folder"));
    await submit(browser, { name: "Photos" });
    await click(control(browser, "Photos"));
    expect(browser.location.prefix).toBe("Photos");
    expect(browser.dataset.state).toBe("empty");
    expect(alert(browser)).toBe("");
  });

  it("says when the folder already exists", async () => {
    const browser = await mount(fixture());
    await click(one(browser, "button.new-folder"));
    await submit(browser, { name: "Contracts" });
    expect(status(browser)).toBe("Folder Contracts already exists.");
  });

  it("refuses an unusable name in the form without calling the API", async () => {
    const source = fixture();
    const create = vi.spyOn(source, "createFolder");
    const browser = await mount(source);
    await click(one(browser, "button.new-folder"));
    await submit(browser, { name: "a/b" });
    expect(one(browser, ".form-error").textContent).toBe(labels.fileNameInvalid);
    await submit(browser, { name: "   " });
    expect(one(browser, ".form-error").textContent).toBe(labels.fileNameEmpty);
    await submit(browser, { name: "x".repeat(256) });
    expect(one(browser, ".form-error").textContent).toBe(labels.fileNameTooLong);
    expect(create).not.toHaveBeenCalled();
    expect(one(browser, "form.name-form").hidden).toBe(false);
  });

  it("cancels with Escape or Cancel and returns focus to the control that opened it", async () => {
    const browser = await mount(fixture());
    const opener = one(browser, "button.new-folder");
    opener.focus();
    await click(opener);
    one(browser, "form.name-form").dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
    );
    await settle();
    expect(one(browser, "form.name-form").hidden).toBe(true);
    expect(rootOf(browser).activeElement).toBe(opener);
    await click(opener);
    await click(one(browser, "form.name-form button.cancel"));
    expect(one(browser, "form.name-form").hidden).toBe(true);
    expect(rootOf(browser).activeElement).toBe(opener);
  });

  it("shows a refusal with its label and keeps the form open", async () => {
    const source = fixture();
    vi.spyOn(source, "createFolder").mockRejectedValue(
      new ApiError(409, "/api/v1/files/folders", { code: "conflict" }),
    );
    const browser = await mount(source);
    await click(one(browser, "button.new-folder"));
    await submit(browser, { name: "x" });
    expect(alert(browser)).toBe(labels.fileConflict);
  });
});

describe("rename through the renames route (API-S-027)", () => {
  /** Open the rename form on the one file of the file-object fixture. */
  async function openRename() {
    const source = fileObjectFixture();
    const rename = vi.spyOn(source, "rename");
    const move = vi.spyOn(source, "move");
    const browser = await mount(source, { location: FILE_LOCATION });
    control(browser, "readme.txt").focus();
    await click(one(browser, "button.rename"));
    return { source, rename, move, browser };
  }

  it("renames with one rename call, never a move, and shows the new display name", async () => {
    const { rename, move, browser } = await openRename();
    const form = one(browser, "form.name-form");
    expect(form.getAttribute("aria-label")).toBe("Rename readme.txt");
    expect(one<HTMLInputElement>(browser, "input[name=folder]").closest("label")?.hidden).toBe(
      true,
    );
    await submit(browser, { name: "notes-final.txt" });
    expect(rename).toHaveBeenCalledTimes(1);
    expect(rename).toHaveBeenCalledWith({ library_id: "lib" }, FILE_ID, "notes-final.txt");
    expect(move).not.toHaveBeenCalled();
    expect(status(browser)).toBe("notes-final.txt renamed. Version v2 committed.");
    expect(names(browser)).toEqual(["notes-final.txt"]);
    expect(deepText(rootOf(browser))).not.toContain(`${FILE_ID}.txt`);
  });

  it("shows the display name the reloaded listing returns, not the one typed", async () => {
    const { source, browser } = await openRename();
    const real = MemoryFileSource.prototype.rename.bind(source);
    vi.spyOn(source, "rename").mockImplementation((library, id) =>
      real(library, id, "Canonical name.txt"),
    );
    await submit(browser, { name: "typed.txt" });
    expect(status(browser)).toBe("Canonical name.txt renamed. Version v2 committed.");
    expect(names(browser)).toEqual(["Canonical name.txt"]);
  });

  it("keeps the typed name in the status when the listing could not be read again", async () => {
    const { source, browser } = await openRename();
    const real = MemoryFileSource.prototype.rename.bind(source);
    vi.spyOn(source, "rename").mockImplementation(async (library, id) => {
      const commit = await real(library, id, "Canonical name.txt");
      source.failure = new Error("offline");
      return commit;
    });
    await submit(browser, { name: "typed.txt" });
    expect(status(browser)).toBe("typed.txt renamed. Version v2 committed.");
  });

  it("takes a display name of up to 1,024 bytes and refuses a longer one before the call", async () => {
    const { rename, browser } = await openRename();
    await submit(browser, { name: "x".repeat(1025) });
    expect(one(browser, ".form-error").textContent).toBe(labels.fileDisplayNameTooLong);
    expect(rename).not.toHaveBeenCalled();
    const long = `${"é".repeat(300)}.txt`;
    await submit(browser, { name: long });
    expect(rename).toHaveBeenCalledWith({ library_id: "lib" }, FILE_ID, long);
  });

  it("renames a file in a UUID-named object that is not its own with a move", async () => {
    const other = "0e1d2c3b-4a59-4687-8695-a4b3c2d1e0f9";
    const source = new MemoryFileSource({
      library_id: "lib",
      object_id: `urn:uuid:${FILE_ID}`,
      files: [{ path: `original/${other}.txt`, displayName: "readme.txt", content: "x" }],
    });
    const move = vi.spyOn(source, "move");
    const rename = vi.spyOn(source, "rename");
    const browser = await mount(source, { location: FILE_LOCATION });
    control(browser, "readme.txt").focus();
    await click(one(browser, "button.rename"));
    expect(one(browser, "form.name-form").getAttribute("aria-label")).toBe(
      "Rename or move readme.txt",
    );
    await submit(browser, { name: "notes.txt" });
    expect(rename).not.toHaveBeenCalled();
    expect(move).toHaveBeenCalledTimes(1);
  });

  it.each([
    [400, "invalid_request", labels.fileRenameInvalid],
    [404, "not_found", labels.fileRenameNotFound],
    [409, "conflict", labels.fileRenameConflict],
    [503, "unavailable", labels.fileRenameUnavailable],
  ] as const)("shows the label for a %s refusal and changes nothing", async (code, error, text) => {
    const { source, browser } = await openRename();
    vi.spyOn(source, "rename").mockRejectedValue(
      new ApiError(code, "/api/v1/files/renames", { code: error }),
    );
    await submit(browser, { name: "other.txt" });
    expect(alert(browser)).toBe(text);
    expect(names(browser)).toEqual(["readme.txt"]);
    expect(source.head).toBe("v1");
  });
});

describe("rename or move", () => {
  it("moves a file in a plain object in place with one move that keeps its folder", async () => {
    const source = fixture();
    const move = vi.spyOn(source, "move");
    const browser = await mount(source);
    control(browser, "readme.txt").focus();
    await click(one(browser, "button.rename"));
    const form = one(browser, "form.name-form");
    expect(form.getAttribute("aria-label")).toBe("Rename or move readme.txt");
    expect(one<HTMLInputElement>(browser, "input[name=name]").value).toBe("readme.txt");
    expect(one<HTMLInputElement>(browser, "input[name=folder]").value).toBe("");
    await submit(browser, { name: "notes-final.txt" });
    expect(move).toHaveBeenCalledWith(expect.anything(), "3f9a0005.txt", "notes-final.txt");
    expect(status(browser)).toBe("notes-final.txt moved. Version v2 committed.");
    expect(deepText(rootOf(browser))).not.toContain("3f9a0005");
  });

  it("moves a file to another folder by naming the folder", async () => {
    const source = fixture();
    const move = vi.spyOn(source, "move");
    const browser = await mount(source);
    control(browser, "readme.txt").focus();
    await click(one(browser, "button.rename"));
    await submit(browser, { name: "readme.txt", folder: "Drawings" });
    // Only the folder changed, so the stored name is kept (and never shown).
    expect(move).toHaveBeenCalledWith(expect.anything(), "3f9a0005.txt", "Drawings/3f9a0005.txt");
    expect(names(browser)).not.toContain("readme.txt");
  });

  it("renames a folder, moving everything below it", async () => {
    const source = fixture();
    const move = vi.spyOn(source, "move");
    const browser = await mount(source);
    control(browser, "Drawings").focus();
    await click(one(browser, "button.rename"));
    await submit(browser, { name: "Plans" });
    expect(move).toHaveBeenCalledWith(expect.anything(), "Drawings", "Plans");
    expect(names(browser)).toContain("Plans");
  });

  it("refuses an unusable name or folder segment without calling the API", async () => {
    const source = fixture();
    const move = vi.spyOn(source, "move");
    const browser = await mount(source);
    control(browser, "readme.txt").focus();
    await click(one(browser, "button.rename"));
    await submit(browser, { name: "../x" });
    expect(one(browser, ".form-error").textContent).toBe(labels.fileNameInvalid);
    await submit(browser, { name: "ok.txt", folder: "A//B" });
    expect(one(browser, ".form-error").textContent).toBe(labels.fileNameEmpty);
    expect(move).not.toHaveBeenCalled();
  });

  it("does nothing when no row is chosen", async () => {
    const browser = await mount(fixture());
    await click(one(browser, "button.rename"));
    expect(one(browser, "form.name-form").hidden).toBe(true);
  });

  it("opens from F2 on a row", async () => {
    const browser = await mount(fixture());
    const row = control(browser, "readme.txt");
    row.focus();
    row.dispatchEvent(new KeyboardEvent("keydown", { key: "F2", bubbles: true }));
    await settle();
    expect(one(browser, "form.name-form").hidden).toBe(false);
  });
});

describe("remove and restore", () => {
  it("removes with one call, reports the version and offers Undo, which restores from the version before", async () => {
    const source = fixture();
    const remove = vi.spyOn(source, "remove");
    const restore = vi.spyOn(source, "restore");
    const browser = await mount(source);
    control(browser, "readme.txt").focus();
    await click(one(browser, "button.remove"));
    expect(remove).toHaveBeenCalledWith(expect.anything(), "3f9a0005.txt");
    expect(status(browser)).toContain("readme.txt removed. Version v2 committed.");
    expect(names(browser)).not.toContain("readme.txt");
    await click(one(browser, "button.undo"));
    expect(restore).toHaveBeenCalledWith(expect.anything(), "v1", ["3f9a0005.txt"]);
    expect(status(browser)).toBe("readme.txt restored. Version v3 committed.");
    expect(names(browser)).toContain("readme.txt");
  });

  it("removes a folder and everything in it", async () => {
    const source = fixture();
    const browser = await mount(source);
    control(browser, "Contracts").focus();
    await click(one(browser, "button.remove"));
    expect(names(browser)).not.toContain("Contracts");
    expect(source.paths.some((p) => p.startsWith("Contracts"))).toBe(false);
  });

  it("offers no delete-forever control", async () => {
    const browser = await mount(fixture());
    const words = all(browser, "button").map((b) => (b.textContent ?? "").toLowerCase());
    expect(words.some((w) => /forever|permanent|purge/.test(w))).toBe(false);
  });

  it("removes with the Delete key on a row", async () => {
    const source = fixture();
    const remove = vi.spyOn(source, "remove");
    const browser = await mount(source);
    const row = control(browser, "readme.txt");
    row.focus();
    row.dispatchEvent(new KeyboardEvent("keydown", { key: "Delete", bubbles: true }));
    await settle();
    expect(remove).toHaveBeenCalledTimes(1);
  });

  it("shows a refusal, and does not claim the removal, when the API refuses", async () => {
    const source = fixture();
    vi.spyOn(source, "remove").mockRejectedValue(
      new ApiError(403, "/api/v1/files/removals", { code: "forbidden", reason: "not_permitted" }),
    );
    const browser = await mount(source);
    control(browser, "readme.txt").focus();
    await click(one(browser, "button.remove"));
    expect(alert(browser)).toBe(labels.authNotPermitted);
    expect(names(browser)).toContain("readme.txt");
    expect(status(browser)).toBe("");
  });

  it("dismisses the Undo offer", async () => {
    const browser = await mount(fixture());
    control(browser, "readme.txt").focus();
    await click(one(browser, "button.remove"));
    await click(one(browser, "button.dismiss"));
    expect(status(browser)).toBe("");
    expect(rootOf(browser).querySelector("button.undo")).toBeNull();
  });

  it("restores into the object the file was removed from, even after another object is opened", async () => {
    const source = fixture();
    const restore = vi.spyOn(source, "restore");
    const browser = await mount(source);
    control(browser, "readme.txt").focus();
    await click(one(browser, "button.remove"));
    browser.location = { library_id: "other-lib", object_id: "other-obj", prefix: "" };
    await click(one(browser, "button.undo"));
    expect(restore).toHaveBeenCalledWith({ library_id: "lib", object_id: "obj" }, "v1", [
      "3f9a0005.txt",
    ]);
  });

  it("does not move focus into another object's listing when Undo restores into the first", async () => {
    const browser = await mount(fixture());
    control(browser, "readme.txt").focus();
    await click(one(browser, "button.remove"));
    browser.location = { library_id: "other-lib", object_id: "other-obj", prefix: "" };
    await settle();
    (rootOf(browser).activeElement as HTMLElement | null)?.blur();
    await click(one(browser, "button.undo"));
    await idle(browser);
    const active = rootOf(browser).activeElement;
    expect(active === null || !one(browser, ".table").contains(active)).toBe(true);
  });

  it("keeps a file of another object open when a removal from the first commits late", async () => {
    const source = fixture();
    let answer: (commit: FileCommit) => void = () => undefined;
    vi.spyOn(source, "remove").mockImplementation(
      () => new Promise<FileCommit>((resolve) => (answer = resolve)),
    );
    const browser = await mount(source);
    control(browser, "readme.txt").focus();
    one(browser, "button.remove").click();
    browser.location = { library_id: "other-lib", object_id: "other-obj", prefix: "" };
    await settle();
    await click(control(browser, "readme.txt"));
    await settle();
    answer({ library_id: "lib", object_id: "obj", version: "v2", created: "t" });
    await idle(browser);
    expect(one(browser, ".viewer").hidden).toBe(false);
    expect(one(browser, "h2.viewer-title").textContent).toBe("readme.txt");
  });

  it("never shows a stored name after a move, a remove and an undo", async () => {
    const browser = await mount(fixture());
    control(browser, "readme.txt").focus();
    await click(one(browser, "button.rename"));
    await submit(browser, { name: "renamed.txt", folder: "Drawings" });
    control(browser, "Drawings");
    control(browser, "facade-study.png").focus();
    await click(one(browser, "button.remove"));
    await click(one(browser, "button.undo"));
    for (const stored of STORED_NAMES) expect(deepText(rootOf(browser))).not.toContain(stored);
  });
});

describe("answers that arrive after another object is opened", () => {
  const OTHER = { library_id: "other-lib", object_id: "other-obj", prefix: "" };

  /** Submit the name form without waiting for the call to answer. */
  function send(browser: FsFileBrowser, name: string): void {
    one<HTMLInputElement>(browser, 'form.name-form input[name="name"]').value = name;
    one<HTMLFormElement>(browser, "form.name-form").dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
  }

  it("closes an open name form when another object is opened", async () => {
    const browser = await mount(fixture());
    await click(one(browser, "button.new-folder"));
    browser.location = OTHER;
    await settle();
    expect(one(browser, "form.name-form").hidden).toBe(true);
  });

  it("does not return focus to the opener when a rename answers late", async () => {
    const source = fixture();
    let answer: (commit: FileCommit) => void = () => undefined;
    vi.spyOn(source, "move").mockImplementation(
      () => new Promise<FileCommit>((resolve) => (answer = resolve)),
    );
    const browser = await mount(source);
    control(browser, "readme.txt").focus();
    await click(one(browser, "button.rename"));
    send(browser, "renamed.txt");
    browser.location = OTHER;
    await settle();
    (rootOf(browser).activeElement as HTMLElement | null)?.blur();
    answer({ library_id: "lib", object_id: "obj", version: "v2", created: "t" });
    await idle(browser);
    expect(rootOf(browser).activeElement).toBeNull();
  });

  it("does not show a folder created late in the object opened since", async () => {
    const source = fixture();
    let answer: (folder: FolderAnswer) => void = () => undefined;
    vi.spyOn(source, "createFolder").mockImplementation(
      () => new Promise<FolderAnswer>((resolve) => (answer = resolve)),
    );
    const browser = await mount(source);
    await click(one(browser, "button.new-folder"));
    send(browser, "Photos");
    browser.location = OTHER;
    await settle();
    answer({ library_id: "lib", object_id: "obj", path: "Photos", pending: true });
    await idle(browser);
    expect(names(browser)).not.toContain("Photos");
  });
});

describe("rename form look (UI-S-026, mockups 14 and 44)", () => {
  it("draws the form, fields and buttons through the shared v3 controls", async () => {
    const browser = await mount(fixture());
    const css = rootOf(browser).querySelector("style")?.textContent ?? "";
    expect(css).toMatch(
      /\.name-form \{[^}]*border: 1px solid var\(--fs-outline-strong\); border-radius: var\(--fs-radius-dialog\); background: var\(--fs-surface-card\)/,
    );
    expect(css).toMatch(
      /\.form-title \{[^}]*font-family: var\(--fs-font-heading\); font-size: var\(--fs-font-size-title\); font-weight: 700/,
    );
    expect(css).toMatch(
      /\.input \{[^}]*min-block-size: 2\.25rem;[^}]*border: 1\.5px solid var\(--fs-outline-control\); border-radius: var\(--fs-radius-input\)/,
    );
    expect(css).toMatch(/\.input:focus-visible \{[^}]*border: 2px solid var\(--fs-focus-ring\)/);
    expect(css).toMatch(
      /\.input\[aria-invalid="true"\] \{ border: 2px solid var\(--fs-error\); \}/,
    );
    expect(css).toMatch(/\.form-error \{[^}]*color: var\(--fs-error\)/);
    expect(one(browser, "form.name-form button.submit").className).toBe("submit button primary");
    expect(one(browser, "button.new-folder").className).toBe("new-folder button secondary");
    expect(one(browser, "button.upload").className).toBe("upload button primary");
    for (const name of ["name", "folder"]) {
      expect(one(browser, `form.name-form input[name="${name}"]`).className).toBe("input");
    }
  });

  it("marks only the field a refusal is about with aria-invalid, and clears it", async () => {
    const browser = await mount(fixture());
    await click(one(browser, "button.new-folder"));
    await submit(browser, { name: "a/b" });
    const name = one<HTMLInputElement>(browser, 'form.name-form input[name="name"]');
    const folder = one<HTMLInputElement>(browser, 'form.name-form input[name="folder"]');
    expect(name.getAttribute("aria-invalid")).toBe("true");
    expect(folder.hasAttribute("aria-invalid")).toBe(false);
    await submit(browser, { name: "Photos" });
    expect(name.hasAttribute("aria-invalid")).toBe(false);
  });
});
