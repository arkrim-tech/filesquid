/**
 * Unit tests for the words the library screens build from facts and job views (UI-1.3,
 * UI-S-027, UI-S-034): when a library was opened, its free space, the rebuild's progress line,
 * the state chip and the progress fraction. Every word comes from the label table; nothing
 * invents a figure the host did not give.
 */
import { describe, expect, it } from "vitest";
import labels from "../../src/theme/labels.en.json";
import {
  freeSpaceText,
  jobChip,
  openedWhen,
  progressFraction,
  rebuildProgressText,
  recentMeta,
  rebuildSummary,
} from "../../src/libraries/format";
import { REBUILD, REBUILD_JOB } from "../support/library-fakes";

const NOW = new Date("2026-10-31T12:00:00Z");
const RECENT = {
  library_id: "lib-1",
  name: "Family archive",
  last_opened: "2026-10-31T08:00:00Z",
  open: true,
  free_bytes: 1_400_000_000_000 as number | null,
  free_bytes_reason: null as "volume_unreadable" | "not_local" | null,
};

describe("openedWhen", () => {
  it("says today for the same calendar day", () => {
    expect(openedWhen("2026-10-31T00:01:00Z", NOW, labels)).toBe(labels.libraryRecentToday);
  });

  it("says the date for an earlier day", () => {
    expect(openedWhen("2026-10-20T10:00:00Z", NOW, labels, "en-GB")).toBe("20 Oct 2026");
  });

  it("falls back to the date text for a time that does not parse", () => {
    expect(openedWhen("not a time", NOW, labels)).toBe("not a time");
  });
});

describe("recentMeta", () => {
  it("words the opened time and the free space of its disk", () => {
    expect(recentMeta(RECENT, NOW, labels)).toBe("Opened today · 1.3 TB free on its disk");
  });

  it("leaves out the free space when the volume cannot be read, never a figure", () => {
    const unreadable = {
      ...RECENT,
      free_bytes: null,
      free_bytes_reason: "volume_unreadable" as const,
    };
    const text = recentMeta(unreadable, NOW, labels);
    expect(text).toBe("Opened today · free space unknown");
    expect(text).not.toMatch(/\d+ ?(B|KB|MB|GB|TB)/);
  });

  it("shows zero free bytes as a figure, not as unknown", () => {
    expect(recentMeta({ ...RECENT, free_bytes: 0 }, NOW, labels)).toBe(
      "Opened today · 0 B free on its disk",
    );
  });
});

describe("freeSpaceText", () => {
  it("words the free space of the disk that holds the library", () => {
    expect(freeSpaceText(1_400_000_000_000, labels)).toBe(
      "1.3 TB on the disk that holds this library",
    );
  });

  it("says it is not known when the volume cannot be read", () => {
    expect(freeSpaceText(null, labels)).toBe(labels.libraryFactFreeUnknown);
  });
});

describe("rebuildProgressText", () => {
  const job = { ...REBUILD_JOB.job, state: "running" as const, done: 1204, total: 2418 };

  it("words objects read, the start time and the checkpoint", () => {
    const running = { ...job, created: "2026-10-31T10:41:09Z" };
    expect(rebuildProgressText(running, labels, "en-GB")).toBe(
      "1,204 of 2,418 objects read · started 10:41 · resumes from its checkpoint after you quit",
    );
  });

  it("words the count alone while the total is not known", () => {
    const unknown = { ...job, total: undefined, created: "2026-10-31T10:41:09Z" };
    expect(rebuildProgressText(unknown, labels, "en-GB")).toBe(
      "1,204 objects read · started 10:41 · resumes from its checkpoint after you quit",
    );
  });
});

describe("progressFraction", () => {
  const base = REBUILD_JOB.job;

  it("is done over total, held between 0 and 1", () => {
    expect(progressFraction({ ...base, done: 1204, total: 2418 })).toBeCloseTo(0.4979, 3);
    expect(progressFraction({ ...base, done: 9, total: 3 })).toBe(1);
  });

  it("is undefined while the total is not known and 1 for an empty finished library", () => {
    expect(progressFraction({ ...base, done: 5 })).toBeUndefined();
    expect(progressFraction({ ...base, state: "completed", done: 0, total: 0 })).toBe(1);
    expect(progressFraction({ ...base, state: "running", done: 0, total: 0 })).toBe(0);
  });
});

describe("jobChip", () => {
  const base = REBUILD_JOB.job;

  it.each([
    [{ state: "running" }, "running", "ok"],
    [{ state: "pending" }, "pending", "muted"],
    [{ state: "pending", paused: true }, "paused", "muted"],
    [{ state: "running", paused: true }, "paused", "muted"],
    [{ state: "running", control: "pause" }, "pausing", "muted"],
    [{ state: "running", control: "cancel" }, "cancelling", "muted"],
    [{ state: "completed" }, "finished", "ok"],
    [{ state: "failed" }, "failed", "alert"],
    [{ state: "cancelled" }, "cancelled", "muted"],
  ] as const)("shows %j as %s on the %s treatment", (change, text, tone) => {
    expect(jobChip({ ...base, ...change } as typeof base, labels)).toEqual({ text, tone });
  });
});

describe("rebuildSummary", () => {
  it("counts the objects read, the unreadable ones and those with validation errors", () => {
    expect(rebuildSummary(REBUILD, labels)).toBe(
      "Read 2 object(s) from files. Not readable: 1. With validation errors: 1.",
    );
  });
});
