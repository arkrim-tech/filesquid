/**
 * Unit tests for the Versions view model (UI-F2.3, UI-S-009, UI-S-031 26 and 27): rows newest
 * first with the current version marked, the next version label, the versions a restore keeps,
 * relative times, the count and keep wording, and shortened digests and identifiers.
 */
import { describe, expect, it } from "vitest";
import { labelChange } from "../../src/components/version-label-dialog";
import {
  MAX_VERSION_ROWS,
  laterVersions,
  nextVersion,
  relativeTime,
  restoreKeepText,
  shortDigest,
  shortIdentifier,
  versionActions,
  versionCountText,
  versionRows,
  versionTotal,
} from "../../src/files/versions";
import type { FileHistory, FileHistoryEntry } from "../../src/files/history-source";
import labels from "../../src/theme/labels.en.json";

/** One history entry. */
function entry(
  version: string,
  created: string,
  size: number | null,
  label: string | null = null,
  change: FileHistoryEntry["change"] = "changed",
): FileHistoryEntry {
  return {
    version: { version, created, message: null, user: { name: "Owner", address: null }, label },
    change,
    digest: size === null ? null : `d-${version}`,
    size,
  };
}

/** A history of `entries`, oldest first as the API answers. */
function history(entries: FileHistoryEntry[]): FileHistory {
  return { library_id: "lib", object_id: "obj", path: "p", display_name: "a.pdf", entries };
}

describe("versionRows", () => {
  it("lists the API's oldest-first entries newest first and marks only the newest as current", () => {
    const rows = versionRows(
      history([
        entry("v1", "2026-09-27T10:00:00Z", 8, null, "added"),
        entry("v2", "2026-09-28T10:00:00Z", 9, "First draft"),
        entry("v3", "2026-10-02T10:00:00Z", 11),
      ]),
    );
    expect(rows.map((r) => r.version)).toEqual(["v3", "v2", "v1"]);
    expect(rows.map((r) => r.current)).toEqual([true, false, false]);
    expect(rows[1]).toMatchObject({ label: "First draft", size: 9, removed: false });
    expect(rows[0]?.label).toBeNull();
  });

  it("marks a removal and gives no current version when the newest entry removed the file", () => {
    const rows = versionRows(
      history([
        entry("v1", "2026-09-27T10:00:00Z", 8, null, "added"),
        entry("v2", "2026-09-28T10:00:00Z", null, null, "deleted"),
      ]),
    );
    expect(rows[0]).toMatchObject({ version: "v2", removed: true, current: false, size: null });
    expect(rows[1]?.current).toBe(false);
  });

  it("treats a blank label as no label", () => {
    const rows = versionRows(history([entry("v1", "2026-09-27T10:00:00Z", 8, "   ")]));
    expect(rows[0]?.label).toBeNull();
  });

  it("keeps at most MAX_VERSION_ROWS rows, the newest", () => {
    const many = Array.from({ length: MAX_VERSION_ROWS + 3 }, (_, i) =>
      entry(`v${i + 1}`, "2026-09-27T10:00:00Z", 1),
    );
    const rows = versionRows(history(many));
    expect(rows).toHaveLength(MAX_VERSION_ROWS);
    expect(versionTotal(history(many))).toBe(MAX_VERSION_ROWS + 3);
    expect(rows[0]?.version).toBe(`v${MAX_VERSION_ROWS + 3}`);
  });
});

describe("versionActions (UI-F2.11)", () => {
  const base = {
    version: "v3",
    label: null,
    created: "t",
    size: 1,
    removed: false,
    current: false,
  };

  it("offers Restore as new version, Download this version and Edit label, in that order", () => {
    expect(versionActions(base)).toEqual(["restore", "download", "edit-label"]);
  });

  it("does not restore the current version, which is the content shown", () => {
    expect(versionActions({ ...base, current: true })).toEqual(["download", "edit-label"]);
  });

  it("offers only Edit label on a version that removed the file", () => {
    expect(versionActions({ ...base, removed: true, size: null })).toEqual(["edit-label"]);
  });

  it("never offers a delete, expiry or thinning action", () => {
    const all = [base, { ...base, current: true }, { ...base, removed: true }].flatMap(
      versionActions,
    );
    expect(all.join(" ")).not.toMatch(/delete|expir|thin/);
  });
});

describe("labelChange (UI-F2.11)", () => {
  it("sets a trimmed label", () => {
    expect(labelChange("  Client review ", null)).toEqual({ kind: "set", label: "Client review" });
  });

  it("clears the label for blank input when there is one", () => {
    expect(labelChange("   ", "Client review")).toEqual({ kind: "set", label: null });
  });

  it("is unchanged for the same label, or blank when none was set, as the route refuses those", () => {
    expect(labelChange("Client review", "Client review")).toEqual({ kind: "unchanged" });
    expect(labelChange("  Client review  ", "Client review")).toEqual({ kind: "unchanged" });
    expect(labelChange("", null)).toEqual({ kind: "unchanged" });
    expect(labelChange("", "   ")).toEqual({ kind: "unchanged" });
  });

  it("is invalid over 512 bytes or with a control character", () => {
    expect(labelChange("é".repeat(257), null)).toEqual({ kind: "invalid" });
    expect(labelChange("a\u0007b", null)).toEqual({ kind: "invalid" });
  });
});

describe("nextVersion and laterVersions", () => {
  it("names the version after the head", () => {
    expect(nextVersion("v7")).toBe("v8");
    expect(nextVersion("v1")).toBe("v2");
    expect(nextVersion("head")).toBeUndefined();
    expect(nextVersion("v9007199254740993")).toBe("v9007199254740994");
    expect(nextVersion("v0")).toBeUndefined();
  });

  it("lists the file's versions newer than the one restored, oldest first", () => {
    const rows = versionRows(
      history([
        entry("v1", "2026-09-27T10:00:00Z", 1),
        entry("v5", "2026-10-09T10:00:00Z", 2),
        entry("v6", "2026-10-15T10:00:00Z", 3),
        entry("v7", "2026-10-16T10:00:00Z", 4),
      ]),
    );
    expect(laterVersions(rows, "v5")).toEqual(["v6", "v7"]);
    expect(laterVersions(rows, "v7")).toEqual([]);
    expect(laterVersions(rows, "v1")).toEqual(["v5", "v6", "v7"]);
  });

  it("orders version numbers beyond the safe integer range exactly", () => {
    const big = versionRows(
      history([
        entry("v9007199254740992", "2026-09-27T10:00:00Z", 1),
        entry("v9007199254740993", "2026-09-28T10:00:00Z", 1),
      ]),
    );
    expect(laterVersions(big, "v9007199254740992")).toEqual(["v9007199254740993"]);
  });
});

describe("restoreKeepText and versionCountText", () => {
  it("words one, two and more kept versions from the label table", () => {
    expect(restoreKeepText(["v7"], labels)).toBe(
      "v7 stays in the history. You can restore it later.",
    );
    expect(restoreKeepText(["v6", "v7"], labels)).toBe(
      "v6 and v7 stay in the history. You can restore them later.",
    );
    expect(restoreKeepText(["v3", "v4", "v7"], labels)).toBe(
      "v3 to v7 stay in the history. You can restore them later.",
    );
    expect(restoreKeepText([], labels)).toBeUndefined();
  });

  it("counts versions in the singular and plural", () => {
    expect(versionCountText(1, labels, "en")).toBe("1 version");
    expect(versionCountText(7, labels, "en")).toBe("7 versions");
    expect(versionCountText(1200, labels, "en")).toBe("1,200 versions");
  });
});

describe("relativeTime", () => {
  const now = new Date("2026-10-16T14:00:00Z");

  it("says now, minutes and hours within a day", () => {
    expect(relativeTime("2026-10-16T13:59:40Z", now, "en")).toBe("Now");
    expect(relativeTime("2026-10-16T13:15:00Z", now, "en")).toBe("45 minutes ago");
    expect(relativeTime("2026-10-16T12:00:00Z", now, "en")).toBe("2 hours ago");
  });

  it("says yesterday between one and two days, then the date", () => {
    expect(relativeTime("2026-10-15T10:00:00Z", now, "en")).toBe("Yesterday");
    expect(relativeTime("2026-10-09T10:00:00Z", now, "en-GB")).toBe("9 Oct 2026");
  });

  it("treats a time in the future as now and leaves an unreadable time unchanged", () => {
    expect(relativeTime("2026-10-16T15:00:00Z", now, "en")).toBe("Now");
    expect(relativeTime("not a time", now, "en")).toBe("not a time");
  });
});

describe("shortDigest and shortIdentifier", () => {
  it("keeps the first and last six characters of a long digest", () => {
    expect(shortDigest("9b1f4e0123456789abcdefc07a2d")).toBe("9b1f4e…c07a2d");
    expect(shortDigest("abc123")).toBe("abc123");
  });

  it("shortens a urn:uuid identifier to its first group and last six characters", () => {
    expect(shortIdentifier("urn:uuid:7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8")).toBe(
      "urn:uuid:7f3c9a1e-…-41d2b8",
    );
    expect(shortIdentifier("obj-1")).toBe("obj-1");
    expect(shortIdentifier("x".repeat(40))).toBe(`${"x".repeat(14)}…${"x".repeat(6)}`);
  });
});
