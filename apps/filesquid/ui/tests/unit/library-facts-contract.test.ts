/**
 * Unit tests for the checks of the library facts, the recent libraries, the first collection
 * and the job views (UI-1.3, API-S-038, API-S-037, UI-S-027, UI-S-034): an answer outside the
 * contract is a `TypeError` and never shown, and each parser builds its result from the fields
 * it checked.
 */
import { describe, expect, it } from "vitest";
import {
  parseCollection,
  parseJobControl,
  parseJobView,
  parseLibraryFacts,
  parseRecentLibraries,
} from "../../src/libraries/contract";
import { REBUILD_JOB } from "../support/library-fakes";

const FACTS = {
  library_id: "lib-1",
  name: "Family archive",
  free_bytes: 1_400_000_000_000,
  free_bytes_reason: null,
  revision: "1",
};

const RECENT = {
  library_id: "lib-1",
  name: "Family archive",
  last_opened: "2026-10-31T09:30:00Z",
  open: true,
  free_bytes: 1_400_000_000_000,
  free_bytes_reason: null,
};

describe("parseLibraryFacts", () => {
  it("accepts the facts of an open library", () => {
    expect(parseLibraryFacts(FACTS)).toEqual(FACTS);
  });

  it("accepts an unreadable volume with its reason and a library that is not open", () => {
    const unreadable = {
      ...FACTS,
      free_bytes: null,
      free_bytes_reason: "volume_unreadable",
      revision: null,
    };
    expect(parseLibraryFacts(unreadable)).toEqual(unreadable);
  });

  it.each([
    ["a non-object", "facts"],
    ["a missing name", { ...FACTS, name: undefined }],
    ["a negative free space", { ...FACTS, free_bytes: -1 }],
    ["a fractional free space", { ...FACTS, free_bytes: 1.5 }],
    ["an unknown reason", { ...FACTS, free_bytes: null, free_bytes_reason: "gone" }],
    ["a revision that is not text", { ...FACTS, revision: 1 }],
  ])("refuses %s", (_what, value) => {
    expect(() => parseLibraryFacts(value)).toThrow(TypeError);
  });

  it("never passes on a field outside the contract, such as a path", () => {
    const parsed = parseLibraryFacts({ ...FACTS, path: "/Users/someone/archive" });
    expect(Object.keys(parsed).sort()).toEqual(Object.keys(FACTS).sort());
  });
});

describe("parseRecentLibraries", () => {
  it("accepts the list and keeps its order", () => {
    const second = { ...RECENT, library_id: "lib-2", name: "Studio work", open: false };
    const parsed = parseRecentLibraries({ libraries: [RECENT, second] });
    expect(parsed.libraries.map((library) => library.library_id)).toEqual(["lib-1", "lib-2"]);
  });

  it("accepts the empty list that means first use", () => {
    expect(parseRecentLibraries({ libraries: [] })).toEqual({ libraries: [] });
  });

  it("accepts a library whose volume cannot be read", () => {
    const parsed = parseRecentLibraries({
      libraries: [{ ...RECENT, free_bytes: null, free_bytes_reason: "not_local" }],
    });
    expect(parsed.libraries[0]).toMatchObject({ free_bytes: null, free_bytes_reason: "not_local" });
  });

  it.each([
    ["a non-object", 4],
    ["no list", {}],
    ["more than 20 libraries", { libraries: Array.from({ length: 21 }, () => RECENT) }],
    ["a date that is not RFC 3339", { libraries: [{ ...RECENT, last_opened: "yesterday" }] }],
    ["an open flag that is not a boolean", { libraries: [{ ...RECENT, open: "yes" }] }],
    ["a blank library id", { libraries: [{ ...RECENT, library_id: "" }] }],
  ])("refuses %s", (_what, value) => {
    expect(() => parseRecentLibraries(value)).toThrow(TypeError);
  });
});

describe("parseCollection", () => {
  it("accepts the collection the create command made", () => {
    expect(parseCollection({ collection_id: "col-1", title: "untitled collection" })).toEqual({
      collection_id: "col-1",
      title: "untitled collection",
    });
  });

  it.each([
    ["a missing id", { title: "untitled collection" }],
    ["a missing title", { collection_id: "col-1" }],
    ["a non-object", null],
  ])("refuses %s", (_what, value) => {
    expect(() => parseCollection(value)).toThrow(TypeError);
  });
});

describe("parseJobView and parseJobControl", () => {
  const job = REBUILD_JOB.job;

  it("accepts a running job with progress", () => {
    const running = { ...job, state: "running", done: 1204, total: 2418 };
    expect(parseJobView(running)).toMatchObject({ state: "running", done: 1204, total: 2418 });
  });

  it("accepts a paused job and a requested control", () => {
    const paused = { ...job, state: "running", paused: true, control: "pause" };
    expect(parseJobView(paused)).toMatchObject({ paused: true, control: "pause" });
  });

  it("accepts a finished job with its report and a failed one with its code", () => {
    const report = { objects: [], unreadable: [] };
    expect(parseJobView({ ...job, state: "completed", result: report })).toMatchObject({
      state: "completed",
      result: report,
    });
    expect(parseJobView({ ...job, state: "failed", error_code: "storage_gone" })).toMatchObject({
      error_code: "storage_gone",
    });
  });

  it.each([
    ["an unknown state", { ...job, state: "sleeping" }],
    ["a missing id", { ...job, id: undefined }],
    ["a negative count", { ...job, done: -1 }],
    ["a date that is not RFC 3339", { ...job, created: "today" }],
    ["a report that breaks the contract", { ...job, state: "completed", result: { objects: 3 } }],
  ])("refuses a job view with %s", (_what, value) => {
    expect(() => parseJobView(value)).toThrow(TypeError);
  });

  it("reads a control answer: the job and whether it was handed to its worker", () => {
    expect(parseJobControl({ job, scheduled: true })).toEqual({ job, scheduled: true });
    expect(() => parseJobControl({ job })).toThrow(TypeError);
    expect(() => parseJobControl({ job: { ...job, state: "x" }, scheduled: false })).toThrow(
      TypeError,
    );
  });
});
