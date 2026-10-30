/**
 * Unit tests for the Drive model (UI-F2.8, UI-S-006, UI-S-026): the row mapping, sorting rules,
 * relative dates, file-type badge classes, the summary and effective scope, the ZIP caps and the
 * Recent day groups.
 */
import { describe, expect, it } from "vitest";
import {
  ZIP_MAX_BYTES,
  ZIP_MAX_FILES,
  activityOf,
  badgeClass,
  defaultDirection,
  driveRow,
  driveSummary,
  extensionOf,
  nextSort,
  recentGroup,
  recentTimeText,
  relativeTime,
  sameQuery,
  scopeText,
  sizeText,
  sortOptionLabel,
  zipCheck,
  type DriveRow,
  type ViewQuery,
  type ViewRow,
} from "../../src/drive/model";
import labels from "../../src/theme/labels.en.json";

const NOW = new Date("2026-10-16T12:00:00Z");
const file = (name: string, size: number, extra: Partial<DriveRow> = {}): DriveRow => ({
  key: name,
  kind: "file",
  name,
  size,
  ...extra,
});
const folder = (name: string, fileCount?: number): DriveRow => ({
  key: name,
  kind: "folder",
  name,
  fileCount,
});

describe("driveRow", () => {
  it("maps a file row with its display name, size and own-journal times", () => {
    const row: ViewRow = {
      kind: "file",
      file_id: "f1",
      object_id: "urn:uuid:f1",
      display_name: "a.pdf",
      media_type: "application/pdf",
      size: 5,
      modified: "2026-10-16T10:00:00Z",
      modified_by_me: null,
      opened_by_me: "2026-10-16T11:00:00Z",
    };
    expect(driveRow(row, "lib")).toMatchObject({
      key: "f1",
      kind: "file",
      name: "a.pdf",
      libraryId: "lib",
      openedByMe: "2026-10-16T11:00:00Z",
    });
  });
  it("maps a folder row to its node handle", () => {
    const row: ViewRow = { kind: "folder", node: "c1", node_kind: "collection", name: "Photos" };
    expect(driveRow(row)).toMatchObject({ key: "c1", kind: "folder", node: "c1", name: "Photos" });
  });
});

describe("sorting", () => {
  it("starts names ascending and the other keys descending", () => {
    expect(defaultDirection("name")).toBe("ascending");
    expect(defaultDirection("size")).toBe("descending");
    expect(defaultDirection("modified")).toBe("descending");
  });
  it("reverses the same column and starts another in its default direction", () => {
    const now = { sort: "modified", direction: "descending" } as const;
    expect(nextSort(now, "modified")).toEqual({ sort: "modified", direction: "ascending" });
    expect(nextSort(now, "name")).toEqual({ sort: "name", direction: "ascending" });
  });
  it("words every sort and direction from the label table", () => {
    expect(labels[sortOptionLabel("modified_by_me", "descending")]).toBe(
      "Last modified by me, newest first",
    );
    expect(labels[sortOptionLabel("name", "ascending")]).toBe("Name, A to Z");
  });
  it("compares queries on every member", () => {
    const q: ViewQuery = {
      node: "n",
      view: "all",
      sort: "name",
      direction: "ascending",
      folders: "on_top",
      filter: "",
    };
    expect(sameQuery(q, { ...q })).toBe(true);
    expect(sameQuery(q, { ...q, filter: "x" })).toBe(false);
    expect(sameQuery(q, { ...q, folders: "mixed" })).toBe(false);
  });
});

describe("file-type badges", () => {
  it("takes the extension from the display name, lower case, at most five characters", () => {
    expect(extensionOf("Report.PDF")).toBe("pdf");
    expect(extensionOf(".env")).toBe("env");
    expect(extensionOf("archive.tar.gz")).toBe("gz");
    expect(extensionOf("README")).toBe("");
    expect(extensionOf("trailing.")).toBe("");
    expect(extensionOf("odd.extension1")).toBe("");
  });
  it("colours documents, data and media as UI-S-026 lists them", () => {
    for (const ext of ["pdf", "docx", "txt"]) expect(badgeClass(ext)).toBe("document");
    for (const ext of ["xlsx", "csv", "blend", "env", "gz"]) expect(badgeClass(ext)).toBe("data");
    for (const ext of ["jpg", "m4a", "mp4", "mov", "cr3"]) expect(badgeClass(ext)).toBe("media");
    expect(badgeClass("xyz")).toBe("other");
  });
});

describe("relativeTime", () => {
  const rel = (iso: string) => relativeTime(iso, NOW, "en", labels);
  it("says just now, minutes and hours ago within a day", () => {
    expect(rel("2026-10-16T11:59:50Z")).toBe(labels.fileTimeJustNow);
    expect(rel("2026-10-16T11:30:00Z")).toBe("30 minutes ago");
    expect(rel("2026-10-16T10:00:00Z")).toBe("2 hours ago");
  });
  it("says Yesterday for the day before, then the date", () => {
    expect(rel("2026-10-15T12:00:00Z")).toBe(labels.fileTimeYesterday);
    expect(rel("2026-10-09T12:00:00Z")).toMatch(/9 Oct 2026|Oct 9, 2026/);
  });
  it("shows a value that is not a time as given, never a wrong date", () => {
    expect(rel("not a time")).toBe("not a time");
  });
});

describe("summary, scope and sizes", () => {
  const rows = [folder("A", 14), folder("B", 62), file("a.pdf", 1024), file("b.csv", 2048)];
  it("counts folders, files and bytes", () => {
    expect(driveSummary(rows)).toEqual({ folders: 2, files: 2, bytes: 3072 });
  });
  it("words a folder's size as its file count and a file's as its size", () => {
    expect(sizeText(rows[0] as DriveRow, labels, "en")).toBe("14 files");
    expect(sizeText(folder("C", 1), labels, "en")).toBe("1 file");
    expect(sizeText(folder("D"), labels, "en")).toBe("");
    expect(sizeText(rows[2] as DriveRow, labels, "en")).toBe("1 KB");
  });
  it("states the effective scope with the files inside when every folder's count is known", () => {
    expect(scopeText(rows, labels, "en")).toBe("2 files and 2 folders (76 files inside) · 3 KB");
    expect(scopeText([file("a", 1)], labels, "en")).toBe("1 file · 1 B");
  });
  it("leaves the inside count out when a selected folder's count is unknown", () => {
    expect(scopeText([folder("X"), file("a", 1)], labels, "en")).toBe("1 file and 1 folder · 1 B");
  });
});

describe("ZIP caps", () => {
  it("allows a selection within 1,000 members and 10 GiB", () => {
    expect(zipCheck([file("a", 5), file("b", 6)])).toEqual({ ok: true, count: 2, bytes: 11 });
  });
  it("refuses more than 1,000 members, counting files inside folders", () => {
    const many = Array.from({ length: ZIP_MAX_FILES + 1 }, (_, i) => file(`f${i}`, 1));
    expect(zipCheck(many)).toEqual({ ok: false, reason: "too_many" });
    expect(zipCheck([folder("big", ZIP_MAX_FILES + 1)])).toEqual({ ok: false, reason: "too_many" });
    expect(zipCheck(many.slice(0, ZIP_MAX_FILES)).ok).toBe(true);
  });
  it("refuses more than 10 GiB", () => {
    expect(zipCheck([file("a", ZIP_MAX_BYTES + 1)])).toEqual({ ok: false, reason: "too_big" });
    expect(zipCheck([file("a", ZIP_MAX_BYTES)]).ok).toBe(true);
  });
  it("refuses an empty selection", () => {
    expect(zipCheck([])).toEqual({ ok: false, reason: "empty" });
  });
});

describe("Recent", () => {
  it("takes the later of the caller's own change and open times, and nothing else", () => {
    expect(
      activityOf(
        file("a", 1, { modifiedByMe: "2026-10-16T10:00:00Z", openedByMe: "2026-10-16T09:00:00Z" }),
      ),
    ).toEqual({ kind: "changed", at: "2026-10-16T10:00:00Z" });
    expect(activityOf(file("a", 1, { openedByMe: "2026-10-16T09:00:00Z" }))).toEqual({
      kind: "opened",
      at: "2026-10-16T09:00:00Z",
    });
    expect(activityOf(file("a", 1))).toBeUndefined();
  });
  it("groups into today, yesterday and earlier within seven days", () => {
    expect(recentGroup("2026-10-16T01:00:00Z", NOW)).toBe("today");
    expect(recentGroup("2026-10-15T12:00:00Z", NOW)).toBe("yesterday");
    expect(recentGroup("2026-10-13T12:00:00Z", NOW)).toBe("earlier");
    expect(recentGroup("2026-10-01T12:00:00Z", NOW)).toBeUndefined();
    expect(recentGroup("garbage", NOW)).toBeUndefined();
  });
  it("words the time per group", () => {
    const text = (iso: string, group: "today" | "yesterday" | "earlier") =>
      recentTimeText(iso, group, NOW, "en", labels);
    expect(text("2026-10-16T09:00:00Z", "today")).toBe("3 hours ago");
    expect(text("2026-10-13T12:00:00Z", "earlier")).toMatch(/13 Oct 2026|Oct 13, 2026/);
  });
});
