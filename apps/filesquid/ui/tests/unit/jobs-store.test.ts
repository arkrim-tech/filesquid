/**
 * Unit tests for `JobsStore` and the in-memory sources (UI-1.2, UI-S-028, API-S-037): the job
 * list newest first, one event subscription, a job's files read at most once per interval and
 * through at most five pages, the count of jobs still open, the footer's progress, and the
 * controls that apply the view the API answers.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { JobsStore } from "../../src/imports/jobs-store";
import { MAX_JOB_ITEM_PAGES } from "../../src/imports/model";
import { MemoryJobSource } from "../../src/imports/memory-source";
import { jobItem, jobView } from "../support/import-fakes";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

/** Let pending promises finish (timers stay faked). */
const settle = async () => {
  for (let i = 0; i < 8; i += 1) await Promise.resolve();
};

describe("JobsStore (API-S-037)", () => {
  it("reads the jobs newest first and counts the ones still open", async () => {
    const source = new MemoryJobSource([
      jobView({ id: "old", state: "completed", created: "2026-10-31T09:00:00Z" }),
      jobView({ id: "new", state: "running", created: "2026-10-31T11:00:00Z" }),
      jobView({ id: "held", state: "pending", paused: true, created: "2026-10-31T10:00:00Z" }),
    ]);
    // The store sorts, whatever order the API lists them in.
    const listed = [...source.jobs];
    source.list = async () => ({ jobs: listed, truncated: false });
    const store = new JobsStore({ source });
    await store.start();
    expect(store.entries.map((entry) => entry.job.id)).toEqual(["new", "held", "old"]);
    expect(store.activeCount).toBe(2);
  });

  it("follows job.update: a changed job replaces its entry and a new job is added, each announced", async () => {
    const source = new MemoryJobSource([jobView({ id: "a", done: 1 })]);
    const store = new JobsStore({ source });
    const changes = vi.fn();
    store.addEventListener("change", changes);
    await store.start();
    changes.mockClear();
    source.emit(jobView({ id: "a", done: 5 }));
    source.emit(jobView({ id: "b", done: 0, created: "2026-10-31T12:00:00Z" }));
    expect(store.entries.map((entry) => [entry.job.id, entry.job.done])).toEqual([
      ["b", 0],
      ["a", 5],
    ]);
    expect(changes).toHaveBeenCalledTimes(2);
  });

  it("reads the whole list again after a resync", async () => {
    const source = new MemoryJobSource([jobView({ id: "a" })]);
    const store = new JobsStore({ source });
    await store.start();
    source.jobs.splice(0, 1, jobView({ id: "z", created: "2026-10-31T13:00:00Z" }));
    source.resync();
    await settle();
    expect(store.entries.map((entry) => entry.job.id)).toEqual(["z"]);
  });

  it("reads the files of an open job and adds them up, but not those of a completed job", async () => {
    const source = new MemoryJobSource(
      [jobView({ id: "a" }), jobView({ id: "b", state: "completed" })],
      {
        a: [
          jobItem("x", { state: "copied" }),
          jobItem("y", { state: "failed", error_code: "source_changed" }),
        ],
      },
    );
    const store = new JobsStore({ source });
    await store.start();
    await settle();
    const entry = store.entries.find((candidate) => candidate.job.id === "a");
    expect(entry?.summary).toMatchObject({ copied: 1, failed: 1, failureCode: "source_changed" });
    expect(entry?.items).toHaveLength(2);
    expect(source.itemReads).toEqual(["a"]);
  });

  it("reads a job's files at most once per interval however many updates arrive", async () => {
    const source = new MemoryJobSource([jobView({ id: "a" })], { a: [jobItem("x")] });
    const store = new JobsStore({ source, itemRefreshMs: 1000 });
    await store.start();
    await settle();
    expect(source.itemReads).toEqual(["a"]);
    for (let done = 2; done < 40; done += 1) source.emit(jobView({ id: "a", done }));
    await settle();
    expect(source.itemReads).toEqual(["a"]);
    await vi.advanceTimersByTimeAsync(1000);
    await settle();
    expect(source.itemReads).toEqual(["a", "a"]);
    await vi.advanceTimersByTimeAsync(5000);
    await settle();
    expect(source.itemReads).toEqual(["a", "a"]);
  });

  it("reads at most five pages of a job's files and says the list is cut", async () => {
    const source = new MemoryJobSource([jobView({ id: "a" })], {
      a: Array.from({ length: 12 }, (_, i) => jobItem(`f${i}`)),
    });
    source.pageSize = 2;
    const store = new JobsStore({ source });
    await store.start();
    await settle();
    const entry = store.entries[0];
    expect(source.itemReads).toHaveLength(MAX_JOB_ITEM_PAGES);
    expect(entry?.items).toHaveLength(MAX_JOB_ITEM_PAGES * 2);
    expect(entry?.itemsCut).toBe(true);
  });

  it("sums the progress of the running jobs for the footer, and none when nothing runs", async () => {
    const source = new MemoryJobSource([
      jobView({ id: "a", done: 82, total: 196 }),
      jobView({ id: "b", done: 10, total: 40 }),
      jobView({ id: "c", state: "pending", paused: true, done: 14, total: 40 }),
    ]);
    const store = new JobsStore({ source });
    await store.start();
    expect(store.progress).toEqual({ done: 92, total: 236 });
    source.emit(jobView({ id: "a", state: "completed", done: 196, total: 196 }));
    source.emit(jobView({ id: "b", state: "completed", done: 40, total: 40 }));
    expect(store.progress).toBeUndefined();
  });

  it("applies the view a control answers and reads the files again after a retry", async () => {
    const source = new MemoryJobSource(
      [jobView({ id: "a", state: "pending", done: 2, total: 2 })],
      { a: [jobItem("x", { state: "failed", error_code: "source_changed", item_id: "ix" })] },
    );
    const store = new JobsStore({ source, itemRefreshMs: 0 });
    await store.start();
    await settle();
    await store.pause("a");
    expect(store.entries[0]?.job.paused).toBe(true);
    await store.resume("a");
    expect(store.entries[0]?.job.paused).toBe(false);
    await store.retryItem("a", "ix");
    await settle();
    expect(source.calls).toEqual(["pause a", "resume a", "retry a ix"]);
    expect(store.entries[0]?.summary.queued).toBe(1);
    await store.cancel("a");
    expect(store.entries[0]?.job.state).toBe("cancelled");
  });

  it("retries every failed file of a job", async () => {
    const source = new MemoryJobSource([jobView({ id: "a", state: "pending" })], {
      a: [jobItem("x", { state: "failed", error_code: "e" }), jobItem("y", { state: "failed" })],
    });
    const store = new JobsStore({ source, itemRefreshMs: 0 });
    await store.start();
    await settle();
    await store.retryFailed("a");
    await settle();
    expect(source.calls).toEqual(["retry-failed a"]);
    expect(store.entries[0]?.summary.failed).toBe(0);
  });

  it("lets a refused control reject so the panel can word it", async () => {
    const source = new MemoryJobSource([jobView({ id: "a", state: "completed" })]);
    const store = new JobsStore({ source });
    await store.start();
    await expect(store.pause("a")).rejects.toMatchObject({ status: 409 });
  });

  it("stops following events and drops its timers when stopped", async () => {
    const source = new MemoryJobSource([jobView({ id: "a" })], { a: [jobItem("x")] });
    const store = new JobsStore({ source });
    await store.start();
    await settle();
    store.stop();
    expect(source.listening).toBe(0);
    source.emit(jobView({ id: "a", done: 99 }));
    expect(store.entries[0]?.job.done).toBe(82);
  });

  it("tracks a job the owner just started even before its event arrives", async () => {
    const source = new MemoryJobSource([]);
    const store = new JobsStore({ source });
    await store.start();
    source.jobs.push(jobView({ id: "fresh" }));
    await store.track("fresh");
    expect(store.entries.map((entry) => entry.job.id)).toEqual(["fresh"]);
  });
});

describe("JobsStore races and timers (API-S-037)", () => {
  it("drops a list answer that a later read overtook, and keeps a job an event updated meanwhile", async () => {
    const source = new MemoryJobSource([
      jobView({ id: "a", done: 1, updated: "2026-10-31T11:00:00Z" }),
    ]);
    const answers: Array<() => void> = [];
    source.list = () =>
      new Promise((resolve) => {
        const jobs = [
          jobView({ id: "a", done: answers.length + 1, updated: "2026-10-31T11:00:00Z" }),
        ];
        answers.push(() => resolve({ jobs, truncated: false }));
      });
    const store = new JobsStore({ source });
    const first = store.start();
    const second = store.refresh();
    answers[1]?.();
    await second;
    answers[0]?.();
    await first;
    expect(store.entries[0]?.job.done).toBe(2);
    source.emit(jobView({ id: "a", done: 9, updated: "2026-10-31T12:00:00Z" }));
    const late = store.refresh();
    answers[2]?.();
    await late;
    expect(store.entries[0]?.job.done).toBe(9);
  });

  it("drops its timer when a job ends, so no file read follows", async () => {
    const source = new MemoryJobSource([jobView({ id: "a" })], { a: [jobItem("x")] });
    const store = new JobsStore({ source, itemRefreshMs: 1000 });
    await store.start();
    await settle();
    source.emit(jobView({ id: "a", done: 3 }));
    source.emit(jobView({ id: "a", state: "completed", done: 196 }));
    await vi.advanceTimersByTimeAsync(5000);
    await settle();
    expect(source.itemReads).toEqual(["a"]);
  });

  it("drops its timers when stopped, so no file read follows", async () => {
    const source = new MemoryJobSource([jobView({ id: "a" })], { a: [jobItem("x")] });
    const store = new JobsStore({ source, itemRefreshMs: 1000 });
    await store.start();
    await settle();
    source.emit(jobView({ id: "a", done: 3 }));
    store.stop();
    await vi.advanceTimersByTimeAsync(5000);
    await settle();
    expect(source.itemReads).toEqual(["a"]);
  });

  it("reads the files at once after a retry even while an interval timer waits", async () => {
    const source = new MemoryJobSource(
      [jobView({ id: "a", state: "pending", done: 2, total: 2 })],
      { a: [jobItem("x", { state: "failed", error_code: "e", item_id: "ix" })] },
    );
    const store = new JobsStore({ source, itemRefreshMs: 60_000 });
    await store.start();
    await settle();
    source.emit(
      jobView({ id: "a", state: "pending", done: 2, total: 2, updated: "2026-10-31T11:04:00Z" }),
    );
    await store.retryItem("a", "ix");
    await settle();
    expect(source.itemReads).toEqual(["a", "a"]);
    expect(store.entries[0]?.summary.queued).toBe(1);
  });

  it("follows the events over a new subscription after a reconnect", async () => {
    const source = new MemoryJobSource([jobView({ id: "a" })]);
    const store = new JobsStore({ source });
    await store.start();
    expect(source.listening).toBe(1);
    await store.reconnect();
    expect(source.listening).toBe(1);
    source.emit(jobView({ id: "a", done: 50, updated: "2026-10-31T12:30:00Z" }));
    expect(store.entries[0]?.job.done).toBe(50);
  });
});

describe("MemoryJobSource retry rules (API-S-037)", () => {
  it("refuses a retry of a finished or paused job before changing any file, and an unknown file as not found", async () => {
    const files = [jobItem("x", { state: "failed", error_code: "e", item_id: "ix" })];
    const source = new MemoryJobSource(
      [
        jobView({ id: "done", state: "completed" }),
        jobView({ id: "held", state: "pending", paused: true }),
        jobView({ id: "open", state: "pending" }),
      ],
      { done: [...files], held: [...files], open: [...files] },
    );
    await expect(source.retryFailed("done")).rejects.toMatchObject({ status: 409 });
    await expect(source.retryItem("held", "ix")).rejects.toMatchObject({ status: 409 });
    await expect(source.retryItem("open", "nope")).rejects.toMatchObject({ status: 404 });
    expect(source.itemsByJob.done?.[0]?.state).toBe("failed");
    expect(source.itemsByJob.held?.[0]?.state).toBe("failed");
    expect(source.itemsByJob.open?.[0]?.state).toBe("failed");
  });
});
