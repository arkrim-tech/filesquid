/**
 * UI tests for `<fs-library-rebuild>` (UI-1.3, UI-S-027, API-S-025, API-S-037, mockup 04): the
 * page that follows the durable rebuild job after Recover (`#/library/<id>/recover/<job>`) or
 * after the library page's Rebuild (`#/library/<id>/rebuild/<job>`): the state chip, the
 * progress in objects, Pause or Resume and Cancel over the job routes, the errors in the
 * report, and the report when the job finishes. Live through `job.update` events and a bounded
 * safety re-read. No path is ever shown.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  accessibilityViolations,
  defineElements,
  MAX_REFRESHES,
  type FsLibraryRebuild,
  type FsStates,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { REBUILD, FakeJobSource, apiError, jobView } from "../support/library-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Mount the page for `job-1` of `lib-1`. */
async function mount(
  jobs = new FakeJobSource(),
  mode: "recover" | "rebuild" = "recover",
  refreshMs = 0,
) {
  const page = document.createElement("fs-library-rebuild") as FsLibraryRebuild;
  page.configure({ labels, jobs, online: () => true, refreshMs, version: "0.1.0" });
  page.mode = mode;
  page.libraryId = "lib-1";
  page.jobId = "job-1";
  document.body.replaceChildren(page);
  await settle();
  return { page, jobs };
}

const rootOf = (page: FsLibraryRebuild) => page.shadowRoot as ShadowRoot;
const text = (page: FsLibraryRebuild, selector: string) =>
  rootOf(page).querySelector(selector)?.textContent ?? "";
const button = (page: FsLibraryRebuild, name: string) =>
  rootOf(page).querySelector(`button[data-action="${name}"]`) as HTMLButtonElement | null;

describe("<fs-library-rebuild> layout (mockup 04)", () => {
  it("shows the recover page: crumb, title, lead, the running job and the note", async () => {
    const { page } = await mount();
    expect(customElements.get("fs-library-rebuild")).toBeDefined();
    expect(text(page, ".crumb a")).toBe(labels.libraries);
    expect(text(page, ".crumb .current")).toBe(labels.libraryRecoverTitle);
    expect(text(page, "h2")).toBe(labels.libraryRecoverTitle);
    expect(text(page, ".lead")).toBe(labels.libraryRecoverHelp);
    expect(text(page, ".job h3")).toBe(labels.libraryRebuild);
    expect(text(page, ".job .chip")).toBe(labels.jobStateRunning);
    expect(rootOf(page).querySelector(".job .chip")?.className).toContain("ok");
    expect(text(page, ".job .progress-text")).toMatch(
      /^1,204 of 2,418 objects read · started \d\d:\d\d · resumes from its checkpoint after you quit$/,
    );
    expect(text(page, ".note")).toBe(labels.libraryRecoverNote);
    expect(text(page, ".back")).toBe(labels.libraryStartBack);
    expect(text(page, ".app-footer")).toContain("Version 0.1.0");
  });

  it("shows the rebuild page for a library's own rebuild with its own title and lead", async () => {
    const { page } = await mount(new FakeJobSource(), "rebuild");
    expect(text(page, "h2")).toBe(labels.libraryRebuild);
    expect(text(page, ".lead")).toBe(labels.libraryRebuildHelp);
    expect(text(page, ".back")).toBe(labels.libraryBack);
    expect((rootOf(page).querySelector(".back") as HTMLAnchorElement).getAttribute("href")).toBe(
      "#/library/lib-1",
    );
  });

  it("draws the progress bar at done over total with an accessible name", async () => {
    const { page } = await mount();
    const bar = rootOf(page).querySelector('[role="progressbar"]') as HTMLElement;
    expect(bar.getAttribute("aria-valuemin")).toBe("0");
    expect(bar.getAttribute("aria-valuemax")).toBe("2418");
    expect(bar.getAttribute("aria-valuenow")).toBe("1204");
    expect(bar.getAttribute("aria-label")).toBe(labels.libraryRebuild);
    expect((bar.querySelector(".fill") as HTMLElement).style.getPropertyValue("--progress")).toBe(
      "49.79%",
    );
    expect(accessibilityViolations(page)).toEqual([]);
  });

  it("leaves the bar indeterminate and the count alone while the total is not known", async () => {
    const jobs = new FakeJobSource();
    jobs.current = jobView({ total: undefined });
    const { page } = await mount(jobs);
    const bar = rootOf(page).querySelector('[role="progressbar"]') as HTMLElement;
    expect(bar.hasAttribute("aria-valuenow")).toBe(false);
    expect(text(page, ".progress-text")).toMatch(/^1,204 objects read · started/);
  });

  it("shows Pause and Cancel while it runs, both secondary", async () => {
    const { page } = await mount();
    expect(button(page, "pause")?.textContent).toBe(labels.jobPause);
    expect(button(page, "cancel")?.textContent).toBe(labels.jobCancel);
    expect(button(page, "resume")).toBeNull();
    for (const name of ["pause", "cancel"]) {
      expect(button(page, name)?.className).toBe("button secondary");
    }
  });

  it("shows no private path and no job or library id", async () => {
    const { page } = await mount();
    const shown = rootOf(page).textContent ?? "";
    expect(shown).not.toContain("job-1");
    expect(shown).not.toMatch(/\/Users|\/home|[A-Z]:\\/);
  });
});

describe("<fs-library-rebuild> controls", () => {
  it("pauses over the job route and shows paused with Resume", async () => {
    const { page, jobs } = await mount();
    button(page, "pause")?.click();
    await settle();
    expect(jobs.calls).toEqual(["job:job-1", "pause:job-1"]);
    expect(text(page, ".job .chip")).toBe(labels.jobStatePaused);
    expect(rootOf(page).querySelector(".job .chip")?.className).toContain("muted");
    expect(button(page, "resume")?.textContent).toBe(labels.jobResume);
    expect(button(page, "pause")).toBeNull();
  });

  it("resumes over the job route and shows running again", async () => {
    const jobs = new FakeJobSource();
    jobs.current = jobView({ paused: true });
    const { page } = await mount(jobs);
    expect(text(page, ".job .chip")).toBe(labels.jobStatePaused);
    button(page, "resume")?.click();
    await settle();
    expect(jobs.calls).toEqual(["job:job-1", "resume:job-1"]);
    expect(text(page, ".job .chip")).toBe(labels.jobStateRunning);
    expect(button(page, "pause")).not.toBeNull();
  });

  it("cancels over the job route, drops the controls and says nothing was changed", async () => {
    const { page, jobs } = await mount();
    button(page, "cancel")?.click();
    await settle();
    expect(jobs.calls).toEqual(["job:job-1", "cancel:job-1"]);
    expect(text(page, ".job .chip")).toBe(labels.jobStateCancelled);
    expect(button(page, "pause")).toBeNull();
    expect(button(page, "cancel")).toBeNull();
    expect(text(page, ".report")).toBe(labels.libraryRebuildCancelled);
  });

  it("shows a pause that is asked for but not yet carried out as pausing", async () => {
    const jobs = new FakeJobSource();
    jobs.paused = jobView({ control: "pause" });
    const { page } = await mount(jobs);
    button(page, "pause")?.click();
    await settle();
    expect(text(page, ".job .chip")).toBe(labels.jobStatePausing);
  });

  it("marks the controls busy while one is in flight, so a second click sends nothing", async () => {
    const jobs = new FakeJobSource();
    let release: (view: ReturnType<typeof jobView>) => void = () => undefined;
    jobs.paused = () => new Promise((resolve) => (release = resolve));
    const { page } = await mount(jobs);
    button(page, "pause")?.click();
    button(page, "pause")?.click();
    button(page, "cancel")?.click();
    await settle();
    expect(jobs.calls).toEqual(["job:job-1", "pause:job-1"]);
    expect(button(page, "pause")?.getAttribute("aria-disabled")).toBe("true");
    release(jobView({ paused: true }));
    await settle();
    expect(button(page, "resume")?.hasAttribute("aria-disabled")).toBe(false);
  });

  it("shows a refused control as a sentence and keeps the controls", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const jobs = new FakeJobSource();
    jobs.paused = apiError(409, "conflict");
    const { page } = await mount(jobs);
    button(page, "pause")?.click();
    await settle();
    expect(text(page, ".alert")).toBe(labels.libraryFailed);
    expect(button(page, "pause")).not.toBeNull();
  });
});

describe("<fs-library-rebuild> live updates and the finished report", () => {
  it("follows job.update events for its own job and ignores another job", async () => {
    const { page, jobs } = await mount();
    jobs.listener?.({ kind: "update", job: jobView({ id: "other", done: 5 }) });
    expect(text(page, ".progress-text")).toContain("1,204 of 2,418");
    jobs.listener?.({ kind: "update", job: jobView({ done: 2000 }) });
    expect(text(page, ".progress-text")).toContain("2,000 of 2,418");
  });

  it("rereads the job on a resync", async () => {
    const { page, jobs } = await mount();
    jobs.current = jobView({ done: 2400 });
    jobs.listener?.({ kind: "resync" });
    await settle();
    expect(jobs.calls).toEqual(["job:job-1", "job:job-1"]);
    expect(text(page, ".progress-text")).toContain("2,400 of 2,418");
  });

  it("reads the finished job again for its report, because the event carries none", async () => {
    const { page, jobs } = await mount();
    jobs.current = jobView({ state: "completed", done: 2418, result: REBUILD });
    jobs.listener?.({ kind: "update", job: jobView({ state: "completed", done: 2418 }) });
    await settle();
    expect(jobs.calls).toEqual(["job:job-1", "job:job-1"]);
    expect(text(page, ".job .chip")).toBe(labels.jobStateCompleted);
    expect(rootOf(page).querySelector(".job .chip")?.className).toContain("ok");
    expect(text(page, ".report")).toContain(
      "Read 2 object(s) from files. Not readable: 1. With validation errors: 1.",
    );
    expect(text(page, ".report")).toContain(labels.libraryRecoveredOpen);
    expect(button(page, "pause")).toBeNull();
    expect(button(page, "cancel")).toBeNull();
    expect(
      (rootOf(page).querySelector(".report a") as HTMLAnchorElement).getAttribute("href"),
    ).toBe("#/library/lib-1");
  });

  it("lists the objects with validation errors, mono, with an error icon, and the unreadable ones", async () => {
    const jobs = new FakeJobSource();
    jobs.current = jobView({ state: "completed", done: 2418, result: REBUILD });
    const { page } = await mount(jobs);
    expect(text(page, ".errors .eyebrow")).toBe(labels.libraryRebuildErrorsTitle);
    const lines = [...rootOf(page).querySelectorAll(".errors li")].map((li) => li.textContent);
    expect(lines).toEqual([
      "urn:uuid:obj-c: validation errors E023",
      "An object could not be opened (E001).",
    ]);
    expect(rootOf(page).querySelectorAll(".errors li svg")).toHaveLength(2);
  });

  it("shows no error section while the report is not there or when it holds no errors", async () => {
    const { page } = await mount();
    expect(rootOf(page).querySelector(".errors")).toBeNull();
    const clean = new FakeJobSource();
    clean.current = jobView({
      state: "completed",
      result: { objects: [REBUILD.objects[0] as never], unreadable: [] },
    });
    const done = await mount(clean);
    expect(rootOf(done.page).querySelector(".errors")).toBeNull();
  });

  it("words a rebuild from the library page without the recovered-copy sentence", async () => {
    const jobs = new FakeJobSource();
    jobs.current = jobView({ state: "completed", result: REBUILD });
    const { page } = await mount(jobs, "rebuild");
    expect(text(page, ".report")).not.toContain(labels.libraryRecoveredOpen);
    expect(text(page, ".report")).toContain("Read 2 object(s)");
  });

  it("shows a failed job as failed with a sentence, logging its code", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const jobs = new FakeJobSource();
    jobs.current = jobView({ state: "failed", error_code: "storage_gone" });
    const { page } = await mount(jobs);
    expect(text(page, ".job .chip")).toBe(labels.jobStateFailed);
    expect(rootOf(page).querySelector(".job .chip")?.className).toContain("alert");
    expect(text(page, ".report")).toBe(labels.libraryFailed);
    expect(warn).toHaveBeenCalled();
    expect(rootOf(page).textContent).not.toContain("storage_gone");
  });

  it("stops watching when the page is removed", async () => {
    const { page, jobs } = await mount();
    page.remove();
    expect(jobs.stopped).toBe(1);
  });

  it("re-reads on its own while the job is active, bounded, and stops once it is final", async () => {
    const jobs = new FakeJobSource();
    const { page } = await mount(jobs, "recover", 5);
    jobs.current = jobView({ done: 2000 });
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(text(page, ".progress-text")).toContain("2,000 of 2,418");
    jobs.current = jobView({ state: "completed", result: REBUILD });
    await new Promise((resolve) => setTimeout(resolve, 30));
    const settled = jobs.calls.length;
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(jobs.calls.length).toBe(settled);
  });
});

describe("<fs-library-rebuild> recovery from a failed read", () => {
  it("drops the error once a later read or an event shows the job again", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const jobs = new FakeJobSource();
    const { page } = await mount(jobs);
    jobs.current = new TypeError("bad answer");
    jobs.listener?.({ kind: "resync" });
    await settle();
    expect(rootOf(page).querySelector("fs-states")?.hasAttribute("hidden")).toBe(false);
    jobs.listener?.({ kind: "update", job: jobView({ done: 1500 }) });
    expect(rootOf(page).querySelector("fs-states")?.hasAttribute("hidden")).toBe(true);
    expect(text(page, ".progress-text")).toContain("1,500 of 2,418");
  });
});

describe("<fs-library-rebuild> safety re-read bound", () => {
  it("re-reads an active job at most MAX_REFRESHES times, then leaves it to the events", async () => {
    const jobs = new FakeJobSource();
    await mount(jobs, "recover", 1);
    await vi.waitFor(() => expect(jobs.calls.length).toBeGreaterThan(MAX_REFRESHES), {
      timeout: 30_000,
      interval: 50,
    });
    await new Promise((resolve) => setTimeout(resolve, 100));
    // The first read plus the capped re-reads, and not one more.
    expect(jobs.calls.length).toBe(1 + MAX_REFRESHES);
  }, 40_000);

  it("keeps the waiting re-read when updates arrive, so busy events cannot postpone it", async () => {
    const jobs = new FakeJobSource();
    const { page } = await mount(jobs, "recover", 40);
    for (let i = 0; i < 8; i += 1) {
      jobs.listener?.({ kind: "update", job: jobView({ done: 1300 + i }) });
      await new Promise((resolve) => setTimeout(resolve, 10));
    }
    expect(jobs.calls.length).toBeGreaterThan(1);
    expect(text(page, ".progress-text")).toContain("objects read");
  });
});

describe("<fs-library-rebuild> failures to read the job", () => {
  it("shows the error state with Try again that reads the job again", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const jobs = new FakeJobSource();
    jobs.current = new TypeError("bad answer");
    const { page } = await mount(jobs);
    const states = rootOf(page).querySelector("fs-states") as FsStates;
    const retry = states.shadowRoot?.querySelector("button") as HTMLButtonElement;
    expect(retry.textContent).toBe(labels.stateRetry);
    jobs.current = jobView();
    retry.click();
    await settle();
    expect(text(page, ".job .chip")).toBe(labels.jobStateRunning);
  });

  it("says the job is gone for a 404 and for a job of another kind", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const gone = new FakeJobSource();
    gone.current = apiError(404, "not_found");
    const first = await mount(gone);
    expect(text(first.page, ".alert")).toBe(labels.libraryNotFound);
    const other = new FakeJobSource();
    other.current = jobView({ kind: "import" });
    const second = await mount(other);
    expect(text(second.page, ".alert")).toBe(labels.libraryNotFound);
    expect(rootOf(second.page).querySelector(".job")).toBeNull();
  });

  it("shows unavailable when no job source is given", async () => {
    const page = document.createElement("fs-library-rebuild") as FsLibraryRebuild;
    page.configure({ labels });
    page.libraryId = "lib-1";
    page.jobId = "job-1";
    document.body.replaceChildren(page);
    await settle();
    expect(rootOf(page).querySelector("fs-states")).not.toBeNull();
  });
});
