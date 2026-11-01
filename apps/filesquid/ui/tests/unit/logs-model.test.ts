/**
 * Unit tests for the Logs model (UI-F2.16; UI-S-021, UI-S-032, API-S-031): time ranges, the
 * journal query of a filter, the search text, the row view of an entry (outcome kinds, refused,
 * journal-only, target text, short operation IDs), the paging and footer words, the entry detail
 * rows and the export choice. Pure functions only.
 */
import { describe, expect, it } from "vitest";
import type { JournalEntryView } from "../../src/api/client";
import {
  DEFAULT_RANGE,
  DENIED_OPERATION,
  EMPTY_FILTER,
  PAGE_SIZE,
  classifySearch,
  detailRows,
  entryCountText,
  exportFileName,
  exportQuery,
  filterChips,
  footerZoneText,
  isJournalOnlyTagged,
  journalQuery,
  matchesFilter,
  outcomeKind,
  rangeFrom,
  rowView,
  searchedFilter,
  shortId,
  showingText,
  targetText,
  utcOffsetLabel,
  viewProvenanceTarget,
} from "../../src/logs/model";
import labels from "../../src/theme/labels.en.json";

const NOW = new Date("2026-10-19T12:00:00.000Z");
const FILE = "1d0e88b4-0000-4000-8000-00000000a14c";
const OP = "3f9c2a70-5be1-4c0d-9e44-81d7a2c90e21";

/** An entry with the fields a test sets. */
function entry(overrides: Partial<JournalEntryView> = {}): JournalEntryView {
  return {
    id: 10,
    time: "2026-10-19T11:03:02.418Z",
    host: "desktop",
    actor: "urn:uuid:5a0c41e2-0000-4000-8000-00000000e19b",
    operation: "metadata.set",
    outcome: "succeeded",
    level: "info",
    target_ids: [],
    provenance: { journal_only: false, operation_id: OP, object_ref: `urn:uuid:${FILE}@v8` },
    ...overrides,
  };
}

describe("time ranges", () => {
  it("opens on the last 24 hours", () => {
    expect(DEFAULT_RANGE).toBe("day");
    expect(EMPTY_FILTER.range).toBe("day");
  });

  it("gives the start of each range as RFC 3339 UTC, and none for all time", () => {
    expect(rangeFrom("hour", NOW)).toBe("2026-10-19T11:00:00.000Z");
    expect(rangeFrom("day", NOW)).toBe("2026-10-18T12:00:00.000Z");
    expect(rangeFrom("week", NOW)).toBe("2026-10-12T12:00:00.000Z");
    expect(rangeFrom("month", NOW)).toBe("2026-09-19T12:00:00.000Z");
    expect(rangeFrom("all", NOW)).toBeUndefined();
  });
});

describe("the journal query of a filter", () => {
  it("asks newest first, with display names, a page and the range start", () => {
    expect(journalQuery(EMPTY_FILTER, NOW)).toEqual({
      order: "desc",
      names: true,
      limit: PAGE_SIZE,
      from: "2026-10-18T12:00:00.000Z",
    });
  });

  it("carries every filter and the cursor of the next page", () => {
    const query = journalQuery(
      {
        range: "all",
        library: "lib-1",
        operation: "ocfl.commit",
        outcome: "failed",
        level: "error",
        file: FILE,
        object: `urn:uuid:${FILE}`,
        operation_id: OP,
      },
      NOW,
      { after: 99 },
    );
    expect(query).toEqual({
      order: "desc",
      names: true,
      limit: PAGE_SIZE,
      library: "lib-1",
      operation: "ocfl.commit",
      outcome: "failed",
      level: "error",
      file: FILE,
      object: `urn:uuid:${FILE}`,
      operation_id: OP,
      after: 99,
    });
  });

  it("asks a refused outcome as the operation of every refusal", () => {
    const query = journalQuery({ range: "all", outcome: "refused" }, NOW);
    expect(query.operation).toBe(DENIED_OPERATION);
    expect(query.outcome).toBeUndefined();
  });
});

describe("the search text", () => {
  it("is empty for blank text", () => {
    expect(classifySearch("   ")).toEqual({ kind: "empty" });
  });

  it("takes urn:uuid and version references as an object", () => {
    expect(classifySearch(`urn:uuid:${FILE}`)).toEqual({
      kind: "object",
      value: `urn:uuid:${FILE}`,
    });
    expect(classifySearch(`urn:uuid:${FILE}@v8`)).toEqual({
      kind: "object",
      value: `urn:uuid:${FILE}`,
    });
  });

  it("takes a bare UUID as an operation ID or a file ID, to be told apart by the view", () => {
    expect(classifySearch(` ${OP} `)).toEqual({ kind: "uuid", value: OP });
  });

  it("takes a dotted name as an operation", () => {
    expect(classifySearch("ocfl.commit")).toEqual({ kind: "operation", value: "ocfl.commit" });
    expect(classifySearch("cli.library.open")).toEqual({
      kind: "operation",
      value: "cli.library.open",
    });
  });

  it("does not search the journal for text it cannot match to an ID or an operation", () => {
    expect(classifySearch("harbour-masterplan-v7.pdf")).toEqual({ kind: "none" });
    expect(classifySearch("hello")).toEqual({ kind: "none" });
  });

  it("puts the searched value in the member the journal route filters on", () => {
    expect(searchedFilter(EMPTY_FILTER, { kind: "object", value: "urn:uuid:x" }).object).toBe(
      "urn:uuid:x",
    );
    expect(searchedFilter(EMPTY_FILTER, { kind: "operation", value: "a.b" }).operation).toBe("a.b");
    const op = searchedFilter(EMPTY_FILTER, { kind: "uuid", value: OP }, "operation_id");
    expect(op.operation_id).toBe(OP);
    const file = searchedFilter(EMPTY_FILTER, { kind: "uuid", value: FILE }, "file");
    expect(file.file).toBe(FILE);
  });

  it("replaces the earlier search: one search member at a time, other filters kept", () => {
    const before = { ...EMPTY_FILTER, file: FILE, level: "warn" as const, library: "lib-1" };
    const after = searchedFilter(before, { kind: "operation", value: "ocfl.commit" });
    expect(after.file).toBeUndefined();
    expect(after.operation).toBe("ocfl.commit");
    expect(after.level).toBe("warn");
    expect(after.library).toBe("lib-1");
  });
});

describe("outcomes", () => {
  it("names the outcome of an entry", () => {
    expect(outcomeKind(entry())).toBe("ok");
    expect(outcomeKind(entry({ outcome: "failed" }))).toBe("failed");
    expect(outcomeKind(entry({ outcome: "stalled" }))).toBe("stalled");
    expect(outcomeKind(entry({ outcome: "started" }))).toBe("started");
    expect(outcomeKind(entry({ outcome: "resumed" }))).toBe("resumed");
    expect(outcomeKind(entry({ outcome: "cancelled" }))).toBe("cancelled");
    expect(outcomeKind(entry({ outcome: "info" }))).toBe("none");
  });

  it("calls an authorisation refusal refused, whatever its outcome says", () => {
    expect(outcomeKind(entry({ operation: DENIED_OPERATION, outcome: "failed" }))).toBe("refused");
  });

  it("tags failed and refused entries and file downloads journal-only, no other", () => {
    const only = { journal_only: true };
    expect(isJournalOnlyTagged(entry({ outcome: "failed", provenance: only }))).toBe(true);
    expect(isJournalOnlyTagged(entry({ operation: DENIED_OPERATION, provenance: only }))).toBe(
      true,
    );
    expect(isJournalOnlyTagged(entry({ operation: "file.download", provenance: only }))).toBe(true);
    expect(isJournalOnlyTagged(entry({ operation: "app.start", provenance: only }))).toBe(false);
    expect(isJournalOnlyTagged(entry({ outcome: "failed" }))).toBe(false);
  });
});

describe("the row of an entry", () => {
  it("shortens an opaque ID to its first and last four characters, or shows a dash", () => {
    expect(shortId(OP)).toBe("3f9c…0e21");
    expect(shortId("abc")).toBe("abc");
    expect(shortId(undefined)).toBe("—");
  });

  it("shows the display name beside the object, else the first target, else a dash", () => {
    expect(targetText(entry({ display_name: "IMG_4102.jpg" }))).toBe("IMG_4102.jpg");
    expect(targetText(entry({ target_ids: ["proc-1", "other"] }))).toBe("proc-1");
    expect(targetText(entry())).toBe("—");
  });

  it("builds the cells: local time of day, level, operation, outcome, target, short operation id", () => {
    const view = rowView(
      entry({ display_name: "harbour-masterplan-v7.pdf", level: "warn" }),
      labels,
      "UTC",
    );
    expect(view).toMatchObject({
      key: "10",
      time: "11:03:02",
      level: "warn",
      levelTone: "alert",
      operation: "metadata.set",
      outcome: { kind: "ok", text: "ok", tone: "ok" },
      target: "harbour-masterplan-v7.pdf",
      operationId: "3f9c…0e21",
      journalOnly: false,
    });
  });

  it("marks failed and refused rows with the error tone and the journal-only tag", () => {
    const failed = rowView(
      entry({ outcome: "failed", level: "error", provenance: { journal_only: true } }),
      labels,
      "UTC",
    );
    expect(failed.outcome).toEqual({ kind: "failed", text: "failed", tone: "alert" });
    expect(failed.levelTone).toBe("alert");
    expect(failed.journalOnly).toBe(true);
    const refused = rowView(
      entry({ operation: DENIED_OPERATION, outcome: "failed", provenance: { journal_only: true } }),
      labels,
      "UTC",
    );
    expect(refused.outcome).toEqual({ kind: "refused", text: "refused", tone: "alert" });
  });

  it("shows a dash for the outcome of an entry that has none and for a missing operation id", () => {
    const view = rowView(entry({ outcome: "info", provenance: { journal_only: true } }), labels);
    expect(view.outcome).toEqual({ kind: "none", text: "—", tone: "none" });
    expect(rowView(entry({ provenance: { journal_only: true } }), labels).operationId).toBe("—");
  });

  it("formats times in the given time zone", () => {
    const view = rowView(entry({ time: "2026-10-19T11:03:02.418Z" }), labels, "Europe/Berlin");
    expect(view.time).toBe("13:03:02");
  });
});

describe("the words under the table and in the footer", () => {
  it("says how many entries are shown, of how many when the whole range is loaded", () => {
    expect(showingText(labels, 16, false, "day")).toBe(
      "Showing 16 of 16 entries in the last 24 hours",
    );
    expect(showingText(labels, 50, true, "week")).toBe(
      "Showing 50 entries in the last 7 days · more are available",
    );
    expect(showingText(labels, 3, false, "all")).toBe(
      "Showing 3 of 3 entries since the first entry",
    );
  });

  it("counts entries in the singular and the plural", () => {
    expect(entryCountText(labels, 1)).toBe("1 entry");
    expect(entryCountText(labels, 12408)).toMatch(/^12.408 entries$/);
  });

  it("names the time zone as an offset from UTC", () => {
    expect(utcOffsetLabel(NOW, "UTC")).toBe("UTC");
    expect(utcOffsetLabel(NOW, "Europe/Berlin")).toBe("UTC+2");
    expect(utcOffsetLabel(NOW, "America/New_York")).toBe("UTC-4");
    expect(utcOffsetLabel(NOW, "Asia/Kolkata")).toBe("UTC+5:30");
    expect(footerZoneText(labels, "UTC+2")).toBe(
      "Times in your time zone (UTC+2) · stored as UTC · kept forever",
    );
  });
});

describe("filter chips", () => {
  it("lists the five chips with their words, all when nothing is chosen", () => {
    const chips = filterChips(EMPTY_FILTER, labels, []);
    expect(chips.map((chip) => chip.text)).toEqual([
      "Last 24 hours",
      "Library: all",
      "Operation: all",
      "Outcome: all",
      "Level: all",
    ]);
    expect(chips.map((chip) => chip.id)).toEqual([
      "range",
      "library",
      "operation",
      "outcome",
      "level",
    ]);
  });

  it("shows the chosen value, naming a library by the name the libraries route gave", () => {
    const chips = filterChips(
      {
        range: "week",
        library: "lib-1",
        operation: "ocfl.commit",
        outcome: "refused",
        level: "warn",
      },
      labels,
      [{ library_id: "lib-1", name: "Family archive" }],
    );
    expect(chips.map((chip) => chip.text)).toEqual([
      "Last 7 days",
      "Library: Family archive",
      "Operation: ocfl.commit",
      "Outcome: refused",
      "Level: warn",
    ]);
  });
});

describe("live entries", () => {
  const since = NOW;
  it("matches an entry to the filter it would have been read with", () => {
    expect(matchesFilter(entry(), EMPTY_FILTER, since)).toBe(true);
    expect(
      matchesFilter(entry({ operation: "x.y" }), { ...EMPTY_FILTER, operation: "a.b" }, since),
    ).toBe(false);
    expect(
      matchesFilter(entry({ level: "warn" }), { ...EMPTY_FILTER, level: "error" }, since),
    ).toBe(false);
    expect(
      matchesFilter(entry({ outcome: "failed" }), { ...EMPTY_FILTER, outcome: "failed" }, since),
    ).toBe(true);
    expect(
      matchesFilter(entry({ outcome: "failed" }), { ...EMPTY_FILTER, outcome: "stalled" }, since),
    ).toBe(false);
  });

  it("matches the library, file, object and operation id the route filters on", () => {
    const e = entry({ target_ids: ["lib-1", `urn:uuid:${FILE}`], operation_id: OP });
    expect(matchesFilter(e, { ...EMPTY_FILTER, library: "lib-1" }, since)).toBe(true);
    expect(matchesFilter(e, { ...EMPTY_FILTER, library: "lib-2" }, since)).toBe(false);
    expect(matchesFilter(e, { ...EMPTY_FILTER, file: FILE }, since)).toBe(true);
    expect(matchesFilter(e, { ...EMPTY_FILTER, object: `urn:uuid:${FILE}` }, since)).toBe(true);
    expect(matchesFilter(e, { ...EMPTY_FILTER, operation_id: OP }, since)).toBe(true);
    expect(matchesFilter(e, { ...EMPTY_FILTER, operation_id: "other" }, since)).toBe(false);
  });

  it("matches an object through the object reference of the entry", () => {
    const e = entry({ object_ref: `urn:uuid:${FILE}@v8` });
    expect(matchesFilter(e, { ...EMPTY_FILTER, file: FILE }, since)).toBe(true);
    expect(matchesFilter(e, { ...EMPTY_FILTER, object: `urn:uuid:${FILE}x` }, since)).toBe(false);
  });

  it("matches a refused filter to refusals only", () => {
    const refusal = entry({ operation: DENIED_OPERATION, outcome: "failed" });
    expect(matchesFilter(refusal, { ...EMPTY_FILTER, outcome: "refused" }, since)).toBe(true);
    expect(matchesFilter(entry(), { ...EMPTY_FILTER, outcome: "refused" }, since)).toBe(false);
  });
});

describe("the detail of an entry", () => {
  it("lists the rows of mockup 38 in order, from what the journal carries", () => {
    const rows = detailRows(
      entry({
        id: 12,
        display_name: "harbour-masterplan-v7.pdf",
        duration_ms: 38,
        target_ids: ["lib-1"],
        object_ref: `urn:uuid:${FILE}@v8`,
        operation_id: OP,
        host: "desktop",
      }),
      labels,
      { zone: "UTC", libraries: [{ library_id: "lib-1", name: "Family archive" }], lang: "en-US" },
    );
    const byKey = Object.fromEntries(rows.map((row) => [row.key, row]));
    expect(rows.map((row) => row.key)).toEqual([
      "time-utc",
      "time-local",
      "level",
      "outcome",
      "actor",
      "software",
      "library",
      "object",
      "file",
      "operation-id",
      "duration",
    ]);
    expect(byKey["time-utc"]).toMatchObject({ value: "2026-10-19T11:03:02.418Z", mono: true });
    expect(byKey["time-local"]?.value).toBe("Oct 19, 2026, 11:03:02 (UTC)");
    expect(byKey["level"]).toMatchObject({ value: "info", mono: true });
    expect(byKey["outcome"]?.value).toBe("ok");
    expect(byKey["actor"]).toMatchObject({
      value: "urn:uuid:5a0c41e2-0000-4000-8000-00000000e19b",
      mono: true,
      note: "Owner agent · this installation",
    });
    expect(byKey["software"]?.value).toBe("desktop");
    expect(byKey["library"]?.value).toBe("Family archive");
    expect(byKey["object"]).toMatchObject({ value: `urn:uuid:${FILE} → v8`, mono: true });
    expect(byKey["file"]).toMatchObject({
      value: "harbour-masterplan-v7.pdf",
      note: "· looked up in your library",
    });
    expect(byKey["operation-id"]).toMatchObject({ value: OP, mono: true });
    expect(byKey["duration"]?.value).toBe("38 ms");
  });

  it("leaves out the rows an entry has no value for, and adds the error code and process it has", () => {
    const rows = detailRows(
      entry({ error_code: "not_found", process_id: "proc-1", provenance: { journal_only: true } }),
      labels,
      { zone: "UTC", libraries: [], lang: "en-US" },
    );
    const keys = rows.map((row) => row.key);
    expect(keys).not.toContain("file");
    expect(keys).not.toContain("duration");
    expect(keys).not.toContain("object");
    expect(keys).toContain("error");
    expect(keys).toContain("process");
  });

  it("names the library by its id when the libraries route did not name it", () => {
    const rows = detailRows(entry({ target_ids: ["lib-9"] }), labels, {
      zone: "UTC",
      libraries: [{ library_id: "lib-1", name: "Family archive" }],
      lang: "en-US",
      libraryId: "lib-9",
    });
    expect(rows.find((row) => row.key === "library")?.value).toBe("lib-9");
  });
});

describe("View provenance", () => {
  it("is offered for an entry of a committed operation with a UUID object", () => {
    const target = viewProvenanceTarget(entry());
    expect(target).toEqual({ file: FILE, operation_id: OP });
  });

  it("is not offered for a journal-only entry or one without an operation id or object", () => {
    expect(viewProvenanceTarget(entry({ provenance: { journal_only: true } }))).toBeUndefined();
    expect(
      viewProvenanceTarget(entry({ provenance: { journal_only: false, object_ref: "x@v1" } })),
    ).toBeUndefined();
    expect(
      viewProvenanceTarget(entry({ provenance: { journal_only: false, operation_id: OP } })),
    ).toBeUndefined();
  });
});

describe("the export choice", () => {
  it("names the file with the product prefix, the local date and the format", () => {
    expect(exportFileName(labels, "jsonl", NOW, "UTC")).toBe("filesquid-logs-2026-10-19.jsonl");
    expect(exportFileName(labels, "csv", NOW, "UTC")).toBe("filesquid-logs-2026-10-19.csv");
  });

  it("exports the current filter, oldest first, with no names and no paging", () => {
    const query = exportQuery(
      { scope: "filter", format: "csv" },
      { range: "day", level: "warn", library: "lib-1" },
      NOW,
    );
    expect(query).toEqual({
      format: "csv",
      from: "2026-10-18T12:00:00.000Z",
      level: "warn",
      library: "lib-1",
    });
  });

  it("exports a time range between two instants, whatever the filter", () => {
    const query = exportQuery(
      {
        scope: "range",
        format: "jsonl",
        from: "2026-10-01T00:00:00.000Z",
        to: "2026-10-19T23:59:59.999Z",
      },
      { range: "hour", level: "warn" },
      NOW,
    );
    expect(query).toEqual({
      format: "jsonl",
      from: "2026-10-01T00:00:00.000Z",
      to: "2026-10-19T23:59:59.999Z",
    });
  });

  it("exports every entry with nothing but the format", () => {
    expect(
      exportQuery({ scope: "all", format: "jsonl" }, { range: "hour", level: "warn" }, NOW),
    ).toEqual({
      format: "jsonl",
    });
  });
});
