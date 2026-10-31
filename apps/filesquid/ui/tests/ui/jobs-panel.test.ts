/**
 * UI tests for `<fs-jobs-panel>` (UI-1.2, UI-S-028, API-S-037, DE-1.8, mockups 19 and 48): the
 * header button with its count badge, one block per upload job with the chip, detail line and
 * bar of its phase, only the controls the job accepts, the running job's files with each file's
 * status and error code, Retry of one file and Retry failed, a job waiting for the owner after a
 * failed file (no bar, no motion), a refused control worded under its job, and the footer status
 * and New menu that `mountImports` wires.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  IMPORT_STARTED_EVENT,
  JobsStore,
  MemoryJobSource,
  NamedDestinations,
  accessibilityViolations,
  defineElements,
  mountImports,
  type FsJobsPanel,
  type FsNewMenu,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import {
  COLLECTION_ID,
  FakeClient,
  LIBRARY_ID,
  MOCKUP_ITEMS,
  jobItem,
  jobView,
} from "../support/import-fakes";

beforeEach(() => defineElements());
afterEach(() => {
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

/** Let pending promises and renders finish. */
const settle = async () => {
  for (let i = 0; i < 12; i += 1) await new Promise((resolve) => setTimeout(resolve, 0));
};

/** A directory that knows the collection of the fixtures by name. */
async function directory() {
  const known = new NamedDestinations(async () => ({
    libraryId: LIBRARY_ID,
    collectionId: COLLECTION_ID,
  }));
  await known.resolve({ node: "c1", trail: ["Family archive", "Harbour project"] });
  return known;
}

/** Mount a panel over a memory job source. */
async function mount(source: MemoryJobSource, open = true) {
  const store = new JobsStore({ source, itemRefreshMs: 0 });
  const panel = document.createElement("fs-jobs-panel") as FsJobsPanel;
  panel.configure({ labels, store, directory: await directory() });
  document.body.append(panel);
  await settle();
  if (open) panel.open();
  return { panel, store, source };
}

/** The shadow root of the panel. */
const rootOf = (panel: FsJobsPanel) => panel.shadowRoot as ShadowRoot;

/** The block of a job. */
const block = (panel: FsJobsPanel, id: string) =>
  rootOf(panel).querySelector<HTMLElement>(`section[data-job-id="${id}"]`) as HTMLElement;

/** The mockup's four jobs: running, paused, failed and cancelled. */
function mockupJobs() {
  return new MemoryJobSource(
    [
      jobView({ id: "run", created: "2026-10-31T11:02:00Z", done: 82, total: 196 }),
      jobView({
        id: "pause",
        state: "pending",
        paused: true,
        done: 14,
        total: 40,
        destination: { collection_id: COLLECTION_ID },
        created: "2026-10-31T10:00:00Z",
      }),
      jobView({
        id: "fail",
        state: "failed",
        done: 120,
        total: 300,
        error_code: "source_changed",
        created: "2026-10-31T09:00:00Z",
      }),
      jobView({
        id: "cancel",
        state: "cancelled",
        done: 3,
        total: 12,
        created: "2026-10-31T08:00:00Z",
      }),
    ],
    {
      run: [...MOCKUP_ITEMS],
      fail: [
        jobItem("a.jpg", { state: "failed", error_code: "source_changed" }),
        jobItem("b.jpg", { state: "failed", error_code: "source_changed" }),
        jobItem("c.jpg", { state: "failed", error_code: "source_changed" }),
      ],
    },
  );
}

describe("<fs-jobs-panel> button and panel (mockup 19)", () => {
  it("is a defined element with no button until a job is known", async () => {
    expect(customElements.get("fs-jobs-panel")).toBeDefined();
    const { panel } = await mount(new MemoryJobSource([]), false);
    expect(rootOf(panel).querySelector<HTMLElement>(".jobs-button")?.hidden).toBe(true);
    expect(rootOf(panel).querySelector<HTMLElement>(".panel")?.hidden).toBe(true);
  });

  it("shows the button with the number of jobs not ended, named from the label table", async () => {
    const { panel } = await mount(mockupJobs(), false);
    const button = rootOf(panel).querySelector<HTMLButtonElement>(".jobs-button");
    expect(button?.hidden).toBe(false);
    expect(button?.getAttribute("aria-label")).toBe("Jobs, 2 open");
    expect(button?.querySelector(".badge-count")?.textContent).toBe("2");
    expect(button?.getAttribute("aria-expanded")).toBe("false");
    button?.click();
    expect(button && rootOf(panel).querySelector<HTMLElement>(".panel")?.hidden).toBe(false);
  });

  it("titles the panel and each block from the label table, newest first, with the note", async () => {
    const { panel } = await mount(mockupJobs());
    const root = rootOf(panel);
    expect(root.querySelector(".panel-title")?.textContent).toBe(labels.jobsTitle);
    expect(root.querySelector(".panel")?.getAttribute("role")).toBe("dialog");
    const ids = [...root.querySelectorAll<HTMLElement>("section.job")].map((b) => b.dataset.jobId);
    expect(ids).toEqual(["run", "pause", "fail", "cancel"]);
    expect(block(panel, "run").querySelector(".job-title")?.textContent).toBe(
      "Upload into Harbour project › Site photos",
    );
    expect(block(panel, "pause").querySelector(".job-title")?.textContent).toBe(
      "Upload into Harbour project",
    );
    expect(root.querySelector(".panel-note")?.textContent).toBe(labels.jobsNote);
  });

  it("draws the running job: chip, detail, 4 px bar, Pause and Cancel, and its files with their statuses", async () => {
    const { panel } = await mount(mockupJobs());
    const run = block(panel, "run");
    expect(run.querySelector(".chip")?.className).toBe("chip ok");
    expect(run.querySelector(".chip")?.textContent).toBe("running");
    expect(run.querySelector(".detail")?.textContent).toBe("82 of 196 files · started 11:02");
    const bar = run.querySelector<HTMLElement>(".job > .bar, :scope > .bar");
    expect(bar?.getAttribute("role")).toBe("progressbar");
    expect(bar?.querySelector<HTMLElement>(".fill")?.style.inlineSize).toBe("41.84%");
    expect([...run.querySelectorAll(".actions button")].map((b) => b.textContent)).toEqual([
      "Pause",
      "Cancel",
    ]);
    expect(run.querySelector(".eyebrow")?.textContent).toBe("FILES · 4 OF 196 SHOWN");
    const files = [...run.querySelectorAll<HTMLElement>(".job-file")];
    expect(files.map((file) => file.querySelector(".name")?.textContent)).toEqual([
      "IMG_4101.JPG",
      "IMG_4102.CR3",
      "IMG_4103.JPG",
      "IMG_4099.JPG",
    ]);
    expect(files.map((file) => file.querySelector(".state")?.textContent)).toEqual([
      "Copied",
      "Copying · 62%",
      "Queued",
      "Failed · source_changed",
    ]);
    expect(files[0]?.querySelector(".where")?.textContent).toBe("→ Site photos");
    expect(files[1]?.querySelector(".bar.small .fill")?.getAttribute("style")).toContain("62%");
    expect(files[0]?.querySelector(".badge")?.textContent).toBe("jpg");
  });

  it("offers Resume and Cancel to a paused job and no Retry, and says it resumes from its checkpoint", async () => {
    const { panel } = await mount(mockupJobs());
    const paused = block(panel, "pause");
    expect(paused.querySelector(".chip")?.textContent).toBe("paused");
    expect(paused.querySelector(".chip")?.className).toBe("chip muted");
    expect(paused.querySelector(".detail")?.textContent).toBe(
      "14 of 40 files · resumes from its checkpoint",
    );
    expect([...paused.querySelectorAll("button")].map((b) => b.textContent)).toEqual([
      "Resume",
      "Cancel",
    ]);
  });

  it("shows a failed job and a cancelled job with no control (a finished job refuses them)", async () => {
    const { panel } = await mount(mockupJobs());
    const failed = block(panel, "fail");
    expect(failed.querySelector(".chip")?.className).toBe("chip alert");
    expect(failed.querySelector(".detail")?.textContent).toBe(
      "120 of 300 files · 3 failed: source_changed",
    );
    expect(failed.querySelectorAll("button")).toHaveLength(0);
    const cancelled = block(panel, "cancel");
    expect(cancelled.querySelector(".chip")?.textContent).toBe("cancelled");
    expect(cancelled.querySelector(".detail")?.textContent).toBe(
      "3 of 12 files · committed files stay",
    );
    expect(cancelled.querySelectorAll("button")).toHaveLength(0);
  });

  it("meets the accessibility contracts", async () => {
    const { panel } = await mount(mockupJobs());
    expect(accessibilityViolations(rootOf(panel))).toEqual([]);
  });

  it("closes on Escape and gives focus back to the button", async () => {
    const { panel } = await mount(mockupJobs());
    rootOf(panel)
      .querySelector<HTMLElement>(".panel-close")
      ?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true, composed: true }),
      );
    expect(panel.isOpen).toBe(false);
    expect(rootOf(panel).activeElement?.className).toBe("jobs-button");
  });
});

describe("<fs-jobs-panel> controls (API-S-037)", () => {
  it("sends Pause, Resume and Cancel through the job routes and shows the answered job", async () => {
    const { panel, source } = await mount(mockupJobs());
    block(panel, "run").querySelector<HTMLButtonElement>('button[data-action="pause"]')?.click();
    await settle();
    block(panel, "pause").querySelector<HTMLButtonElement>('button[data-action="resume"]')?.click();
    await settle();
    expect(source.calls).toEqual(["pause run", "resume pause"]);
    expect(block(panel, "run").dataset.phase).toBe("paused");
    block(panel, "run").querySelector<HTMLButtonElement>('button[data-action="cancel"]')?.click();
    await settle();
    expect(block(panel, "run").querySelector(".chip")?.textContent).toBe("cancelled");
  });

  it("shows a pending job whose files ended with a failure as needing attention: no bar, Retry failed, Retry and Cancel", async () => {
    const source = new MemoryJobSource(
      [jobView({ id: "wait", state: "pending", done: 2, total: 2 })],
      {
        wait: [
          jobItem("ok.jpg", { state: "copied", bytes_done: 1000 }),
          jobItem("bad.jpg", { state: "failed", error_code: "source_missing" }),
        ],
      },
    );
    const { panel } = await mount(source);
    const wait = block(panel, "wait");
    expect(wait.dataset.phase).toBe("attention");
    expect(wait.querySelector(".chip")?.textContent).toBe(labels.jobsStateAttention);
    expect(wait.querySelector(".chip")?.className).toBe("chip alert");
    expect(wait.querySelector(".detail")?.textContent).toBe(
      "2 of 2 files · 1 failed: source_missing",
    );
    expect(wait.querySelector("[role=progressbar]")).toBeNull();
    expect([...wait.querySelectorAll(".actions button")].map((b) => b.textContent)).toEqual([
      "Retry failed",
      "Cancel",
    ]);
    const files = [...wait.querySelectorAll(".job-file")];
    expect(files).toHaveLength(1);
    expect(files[0]?.querySelector(".state")?.textContent).toBe("Failed · source_missing");
    expect(files[0]?.querySelector(".retry")?.textContent).toBe("Retry");
  });

  it("retries one failed file, then every failed file, by the file's own item ID", async () => {
    const source = new MemoryJobSource(
      [jobView({ id: "wait", state: "pending", done: 3, total: 3 })],
      {
        wait: [
          jobItem("a.jpg", { state: "failed", error_code: "e1", item_id: "ia" }),
          jobItem("b.jpg", { state: "failed", error_code: "e2", item_id: "ib" }),
          jobItem("c.jpg", { state: "failed", error_code: "e3", item_id: "ic" }),
        ],
      },
    );
    const { panel } = await mount(source);
    block(panel, "wait").querySelector<HTMLButtonElement>(".job-file .retry")?.click();
    await settle();
    expect(source.calls).toEqual(["retry wait ia"]);
    expect(block(panel, "wait").querySelectorAll(".job-file .retry")).toHaveLength(2);
    block(panel, "wait")
      .querySelector<HTMLButtonElement>('button[data-action="retry-failed"]')
      ?.click();
    await settle();
    expect(source.calls).toEqual(["retry wait ia", "retry-failed wait"]);
    expect(block(panel, "wait").dataset.phase).toBe("waiting");
  });

  it("words a refused control under its job and changes nothing", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = mockupJobs();
    const { panel } = await mount(source);
    const pause = block(panel, "run").querySelector<HTMLButtonElement>(
      'button[data-action="pause"]',
    );
    source.jobs[0] = jobView({ id: "run", state: "completed", created: "2026-10-31T11:02:00Z" });
    pause?.click();
    await settle();
    expect(block(panel, "run").querySelector(".alert")?.textContent).toBe(labels.jobsRefused);
  });

  it("gives a file that failed with path_suffix_exhausted its error card with the first and last name", async () => {
    const source = new MemoryJobSource(
      [jobView({ id: "wait", state: "pending", done: 1, total: 1 })],
      {
        wait: [jobItem("photo.jpg", { state: "failed", error_code: "path_suffix_exhausted" })],
      },
    );
    const { panel } = await mount(source);
    const card = block(panel, "wait").querySelector(".refusal") as HTMLElement;
    expect(card.querySelector(".title")?.textContent).toBe("Not uploaded: photo.jpg");
    expect(card.querySelector(".reason")?.textContent).toBe(
      "Every name from photo-1.jpg to photo-9999.jpg is taken in this folder. Nothing was changed. Rename or remove one, then try again.",
    );
    card.querySelector("button")?.click();
    expect(block(panel, "wait").querySelector(".refusal")).toBeNull();
  });

  it("hides the button on a host that cannot report jobs", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new MemoryJobSource([jobView()]);
    source.list = async () => {
      throw new TypeError("unavailable");
    };
    const { panel } = await mount(source, false);
    expect(rootOf(panel).querySelector<HTMLElement>(".jobs-button")?.hidden).toBe(true);
  });
});

describe("mountImports (UI-1.2)", () => {
  /** A shell stand-in with the footer property, and a New menu. */
  function setup(hostType: "desktop" | "server") {
    const client = new FakeClient({
      "GET /api/v1/jobs?kind=import&limit=100": {
        jobs: [jobView({ id: "run", done: 82, total: 196 })],
        truncated: false,
      },
      "GET /api/v1/jobs/run/items?limit=200": { items: [], next_cursor: null },
    });
    const shell = Object.assign(document.createElement("div"), {
      uploadStatus: undefined as string | undefined,
    });
    document.body.append(shell);
    const newMenu = document.createElement("fs-new-menu") as FsNewMenu;
    const navigation = { list: async () => ({ nodes: [], ancestors: [], next_cursor: null }) };
    const mounted = mountImports(shell as never, {
      labels,
      client: { request: client.request, subscribe: client.subscribe },
      navigation,
      newMenu,
    });
    shell.dispatchEvent(new CustomEvent("fs-host", { detail: { hostType }, bubbles: true }));
    return { client, shell, newMenu, mounted };
  }

  it("enables the New menu's upload items on the desktop host only, and leaves New folder off", async () => {
    const desktop = setup("desktop");
    await settle();
    expect(desktop.newMenu.uploads).toBe(true);
    expect(desktop.newMenu.folders).toBe(false);
    document.body.replaceChildren();
    const server = setup("server");
    await settle();
    expect(server.newMenu.uploads).toBe(false);
  });

  it("shows 'Uploading · {done} of {total}' in the footer while a job runs, and clears it after", async () => {
    const { shell, client } = setup("desktop");
    await settle();
    expect(shell.uploadStatus).toBe("Uploading · 82 of 196");
    client.emit({
      topic: "job.update",
      data: jobView({ id: "run", state: "completed", done: 196 }),
    });
    await settle();
    expect(shell.uploadStatus).toBeUndefined();
  });

  it("opens the Jobs panel on a job the dialog started", async () => {
    const { mounted } = setup("desktop");
    await settle();
    mounted.dialog.dispatchEvent(
      new CustomEvent(IMPORT_STARTED_EVENT, { detail: { jobId: "run" }, bubbles: true }),
    );
    await settle();
    expect(mounted.panel.isOpen).toBe(true);
  });
});
