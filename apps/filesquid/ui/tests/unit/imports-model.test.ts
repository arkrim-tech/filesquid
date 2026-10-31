/**
 * Unit tests for the pure rules of the upload flow (UI-1.2, UI-S-028, UI-S-029, API-S-036,
 * API-S-037, DE-1.8): the destination the picker's trail names, the numbered name an upload gets
 * beside a taken one, what a job's files add up to, the phase and the controls a job offers,
 * the files the Jobs panel lists, and how one file's status reads.
 */
import { describe, expect, it } from "vitest";
import {
  allowedControls,
  destinationFolder,
  destinationText,
  itemPercent,
  itemStatus,
  jobPhase,
  numberedName,
  pathSuffixRange,
  shownItems,
  summariseItems,
  type JobItem,
  type JobView,
} from "../../src/imports/model";
import labels from "../../src/theme/labels.en.json";

/** A job view with the fields the rules read; each test overrides what it needs. */
function job(over: Partial<JobView> = {}): JobView {
  return {
    id: "j1",
    kind: "import",
    operation_id: "op-1",
    state: "running",
    paused: false,
    done: 3,
    total: 10,
    attempts: 1,
    created: "2026-10-31T11:02:00Z",
    updated: "2026-10-31T11:03:00Z",
    result: null,
    ...over,
  };
}

/** A job file in a state. */
function item(name: string, state: JobItem["state"], over: Partial<JobItem> = {}): JobItem {
  return { item_id: `id-${name}`, name, state, bytes_done: 0, bytes_total: 100, ...over };
}

describe("destination of a picker choice (UI-S-028)", () => {
  it("names the folders below the collection, joined by a slash, and none for a collection", () => {
    expect(destinationFolder(["Family archive", "Harbour project"])).toBeUndefined();
    expect(destinationFolder(["Family archive", "Harbour project", "Site photos"])).toBe(
      "Site photos",
    );
    expect(destinationFolder(["Family archive", "Harbour project", "Drawings", "Sheets"])).toBe(
      "Drawings/Sheets",
    );
  });

  it("shows the collection and the folders after the library, joined by an arrow", () => {
    expect(destinationText(["Family archive", "Harbour project", "Site photos"])).toBe(
      "Harbour project › Site photos",
    );
    expect(destinationText(["Family archive", "Harbour project"])).toBe("Harbour project");
  });
});

describe("numbered names (API-S-011 Same logical path)", () => {
  it("puts the number at the end of the first part of the name, before the first dot", () => {
    expect(numberedName("IMG_4102.JPG", 1)).toBe("IMG_4102-1.JPG");
    expect(numberedName("archive.tar.gz", 1)).toBe("archive-1.tar.gz");
    expect(numberedName("report", 2)).toBe("report-2");
    expect(numberedName(".env", 1)).toBe(".env-1");
    expect(numberedName("photo.jpg", 9999)).toBe("photo-9999.jpg");
  });

  it("states the first and last name a folder can hold, as the cap of 9,999 does", () => {
    expect(pathSuffixRange("photo.jpg")).toEqual({ first: "photo-1.jpg", last: "photo-9999.jpg" });
  });
});

describe("a job's files added up (API-S-037)", () => {
  it("counts each state and keeps the first failure's code", () => {
    const sum = summariseItems([
      item("a", "copied"),
      item("b", "copying"),
      item("c", "queued"),
      item("d", "failed", { error_code: "source_changed" }),
      item("e", "failed", { error_code: "source_missing" }),
      item("f", "skipped"),
    ]);
    expect(sum).toEqual({
      queued: 1,
      copying: 1,
      copied: 1,
      skipped: 1,
      failed: 2,
      failureCode: "source_changed",
    });
  });
});

describe("a job's phase (API-S-037, DE-1.8)", () => {
  const none = summariseItems([]);

  it("tells running, paused, finished and requested states apart", () => {
    expect(jobPhase(job(), none)).toBe("running");
    expect(jobPhase(job({ state: "pending", paused: true }), none)).toBe("paused");
    expect(jobPhase(job({ state: "running", control: "pause" }), none)).toBe("pausing");
    expect(jobPhase(job({ state: "running", control: "cancel" }), none)).toBe("cancelling");
    expect(jobPhase(job({ state: "completed" }), none)).toBe("completed");
    expect(jobPhase(job({ state: "cancelled" }), none)).toBe("cancelled");
    expect(jobPhase(job({ state: "failed", error_code: "too_many_files" }), none)).toBe("failed");
    expect(jobPhase(job({ state: "pending" }), none)).toBe("waiting");
  });

  it("shows a pending job whose files all ended with a failure as needing attention", () => {
    const failed = summariseItems([item("a", "copied"), item("b", "failed", { error_code: "x" })]);
    expect(jobPhase(job({ state: "pending", done: 2, total: 2 }), failed)).toBe("attention");
  });

  it("keeps a pending job with a failed file but a queued one waiting", () => {
    const mixed = summariseItems([item("a", "queued"), item("b", "failed", { error_code: "x" })]);
    expect(jobPhase(job({ state: "pending" }), mixed)).toBe("waiting");
  });
});

describe("the controls a job offers (API-S-037)", () => {
  const failed = summariseItems([item("a", "failed", { error_code: "x" })]);
  const clean = summariseItems([item("a", "copying")]);

  it("offers Pause and Cancel to a running job, and Retry only for a failed file", () => {
    expect(allowedControls(job(), clean)).toEqual({
      pause: true,
      resume: false,
      cancel: true,
      retry: false,
    });
    expect(allowedControls(job(), failed).retry).toBe(true);
  });

  it("offers Resume and Cancel, and no Retry, to a paused job (a paused job refuses a retry)", () => {
    expect(allowedControls(job({ state: "pending", paused: true }), failed)).toEqual({
      pause: false,
      resume: true,
      cancel: true,
      retry: false,
    });
  });

  it("offers Retry and Cancel to a pending job waiting for the owner", () => {
    expect(allowedControls(job({ state: "pending" }), failed)).toEqual({
      pause: false,
      resume: false,
      cancel: true,
      retry: true,
    });
  });

  it("offers nothing to a finished job or one under a cancel request", () => {
    const off = { pause: false, resume: false, cancel: false, retry: false };
    for (const state of ["completed", "failed", "cancelled"] as const) {
      expect(allowedControls(job({ state }), failed)).toEqual(off);
    }
    expect(allowedControls(job({ control: "cancel" }), failed)).toEqual(off);
  });

  it("offers only Cancel while a pause is requested but not yet taken", () => {
    expect(allowedControls(job({ control: "pause" }), failed)).toEqual({
      pause: false,
      resume: false,
      cancel: true,
      retry: false,
    });
  });
});

describe("the files the Jobs panel lists (UI-S-028)", () => {
  it("lists failed files first within the limit, copying next, then the rest, in job order", () => {
    const files = [
      item("a", "copied"),
      item("b", "copying"),
      item("c", "queued"),
      item("d", "queued"),
      item("e", "failed", { error_code: "x" }),
      item("f", "queued"),
    ];
    expect(shownItems(files, 3).map((file) => file.name)).toEqual(["b", "c", "e"]);
  });

  it("keeps every file when there are fewer than the limit", () => {
    const files = [item("a", "copied"), item("b", "queued")];
    expect(shownItems(files, 8)).toEqual(files);
  });
});

describe("one file's status (UI-S-028)", () => {
  it("words each state from the label table", () => {
    expect(itemStatus(item("a", "copied"), labels)).toBe("Copied");
    expect(itemStatus(item("a", "queued"), labels)).toBe("Queued");
    expect(itemStatus(item("a", "skipped"), labels)).toBe("Already uploaded · skipped");
    expect(itemStatus(item("a", "copying", { bytes_done: 62, bytes_total: 100 }), labels)).toBe(
      "Copying · 62%",
    );
    expect(itemStatus(item("a", "failed", { error_code: "source_changed" }), labels)).toBe(
      "Failed · source_changed",
    );
    expect(itemStatus(item("a", "failed"), labels)).toBe("Failed");
  });

  it("rounds a copy's progress down and never passes 100 or divides by zero", () => {
    expect(itemPercent(item("a", "copying", { bytes_done: 1, bytes_total: 3 }))).toBe(33);
    expect(itemPercent(item("a", "copying", { bytes_done: 5, bytes_total: 3 }))).toBe(100);
    expect(itemPercent(item("a", "copying", { bytes_done: 0, bytes_total: 0 }))).toBe(0);
  });
});
