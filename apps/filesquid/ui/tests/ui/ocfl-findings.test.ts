/**
 * UI tests for <fs-ocfl-findings> (UI-F2.14, API-S-022, SRS-F-061): the Monitor's OCFL
 * validation section. Strict DOM assertions on the open findings (errors apart from warnings),
 * a withheld location, the last and next run, the schedule, the Validate now control, the
 * read-only note and repair link for W010, coalesced refreshes and the failure state.
 */
import { beforeEach, describe, expect, it } from "vitest";
import {
  ApiError,
  type FindingView,
  type FindingsView,
  type RunAccepted,
  type RunRequest,
  type RunView,
} from "../../src/api/client";
import {
  FsOcflFindings,
  defineElements,
  type FindingsClient,
  type FindingsOptions,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const LIB = "11111111-1111-4111-8111-111111111111";

/** One finding of the fixture library. */
function finding(code: string, overrides: Partial<FindingView> = {}): FindingView {
  return {
    library_id: LIB,
    object_id: "urn:uuid:aaaa",
    version: "v2",
    code,
    severity: code.startsWith("E") ? "error" : "warning",
    location: "objects/aa/v2/content/stray.bin",
    ...overrides,
  };
}

/** A findings answer with `findings` and one library that ran once. */
function view(findings: FindingView[], extra: Partial<FindingsView> = {}): FindingsView {
  return {
    findings,
    truncated: false,
    settings: { interval_ms: 7 * 24 * 3_600_000, object_cap: 100_000 },
    libraries: [
      {
        library_id: LIB,
        running: false,
        last_run: {
          trigger: "requested",
          started: "2026-10-20T10:00:00Z",
          ended: "2026-10-20T10:00:05Z",
          outcome: "completed",
          checked: 12,
          not_checked: 0,
          error_code: null,
        },
        next_run: "2026-10-27T10:00:00Z",
      },
    ],
    ...extra,
  };
}

/** A fake client answering `answers` in turn (the last one repeats) and recording calls. */
function fakeClient(...answers: (FindingsView | Error)[]) {
  const runs: RunRequest[] = [];
  let reads = 0;
  let runAnswer: RunAccepted | Error = { started: [LIB], refused: [] };
  const client: FindingsClient = {
    adminOcflFindings: async () => {
      const answer = answers[Math.min(reads, answers.length - 1)];
      reads += 1;
      if (answer instanceof Error) throw answer;
      return answer as FindingsView;
    },
    startOcflValidation: async (request: RunRequest = {}) => {
      runs.push(request);
      if (runAnswer instanceof Error) throw runAnswer;
      return runAnswer;
    },
  };
  return {
    client,
    runs,
    reads: () => reads,
    answerRun: (answer: RunAccepted | Error) => (runAnswer = answer),
  };
}

/** Mount the element with `client`. */
function mount(client: FindingsClient, extra: Partial<FindingsOptions> = {}): FsOcflFindings {
  const el = document.createElement("fs-ocfl-findings") as FsOcflFindings;
  el.configure({ labels, client, ...extra });
  document.body.replaceChildren(el);
  return el;
}

/** Text of each cell of each body row of one part's table. */
function rows(el: FsOcflFindings, part: string): string[][] {
  const body = el.shadowRoot?.querySelectorAll(`[data-part="${part}"] tbody tr`) ?? [];
  return [...body].map((row) => [...row.querySelectorAll("td")].map((td) => td.textContent ?? ""));
}

describe("<fs-ocfl-findings>", () => {
  beforeEach(() => defineElements());

  it("is registered and lists an E023 error with its object, version, code and location", async () => {
    expect(customElements.get("fs-ocfl-findings")).toBe(FsOcflFindings);
    const el = mount(fakeClient(view([finding("E023")])).client);
    await settle();
    expect(el.dataset.state).toBe("ready");
    expect(rows(el, "errors")).toEqual([
      [LIB, "urn:uuid:aaaa", "v2", "E023", "objects/aa/v2/content/stray.bin"],
    ]);
    expect(rows(el, "warnings")).toEqual([[labels.findingsNoWarnings]]);
    const heads = el.shadowRoot?.querySelectorAll('[data-part="errors"] th') ?? [];
    expect([...heads].map((th) => th.textContent)).toEqual([
      labels.library,
      labels.findingsObject,
      labels.findingsVersion,
      labels.findingsCode,
      labels.findingsLocation,
    ]);
  });

  it("lists a W010 warning apart from errors, says its files are read-only and links to repair", async () => {
    const fake = fakeClient(view([finding("W010", { location: null })]));
    const el = mount(fake.client, { repairHref: (id) => `#/repair/${id}` });
    await settle();
    expect(rows(el, "errors")).toEqual([[labels.findingsNoErrors]]);
    expect(rows(el, "warnings").map((cells) => cells[3])).toEqual(["W010"]);
    const note = el.shadowRoot?.querySelector('[data-note="read-only"]');
    expect(note?.textContent).toContain(
      labels.findingsReadOnly.replace("{count}", "1").replace("{name}", LIB),
    );
    const link = note?.querySelector<HTMLAnchorElement>('a[data-action="repair"]');
    expect(link?.textContent).toBe(labels.findingsRepair);
    expect(link?.getAttribute("href")).toBe(`#/repair/${LIB}`);
  });

  it("shows the read-only note without a link when the distribution offers no repair address", async () => {
    const el = mount(fakeClient(view([finding("W010")])).client);
    await settle();
    expect(el.shadowRoot?.querySelector('[data-note="read-only"]')).not.toBeNull();
    expect(el.shadowRoot?.querySelector('a[data-action="repair"]')).toBeNull();
  });

  it("renders no location the API withholds", async () => {
    const withheld = finding("E023", { location: null });
    const el = mount(fakeClient(view([withheld])).client);
    await settle();
    const cell = el.shadowRoot?.querySelector('[data-part="errors"] tbody td[data-withheld]');
    expect(cell?.textContent).toBe(labels.findingsLocationHidden);
    expect(el.shadowRoot?.textContent).not.toContain("objects/");
    expect(el.shadowRoot?.textContent).not.toContain("stray.bin");
  });

  it("shows the last and the next run, the outcome and the counts from the findings route", async () => {
    const el = mount(fakeClient(view([])).client);
    await settle();
    expect(rows(el, "runs")).toEqual([
      [
        LIB,
        labels.findingsIdle,
        "2026-10-20T10:00:00Z",
        "2026-10-20T10:00:05Z",
        labels.runCompleted,
        "12",
        "0",
        "2026-10-27T10:00:00Z",
      ],
    ]);
  });

  it("says Never and Not scheduled for a library that has not run, Running while it runs", async () => {
    const libraries = [{ library_id: LIB, running: true, last_run: null, next_run: null }];
    const el = mount(fakeClient(view([], { libraries })).client);
    await settle();
    const [cells] = rows(el, "runs");
    expect(cells?.[1]).toBe(labels.findingsRunning);
    expect(cells?.[2]).toBe(labels.findingsNever);
    expect(cells?.[7]).toBe(labels.findingsNotScheduled);
  });

  it("names a failed run's error code and a capped run's unchecked objects", async () => {
    const run = (outcome: RunView["outcome"], error_code: string | null, not_checked: number) => ({
      library_id: `lib-${outcome}`,
      running: false,
      next_run: null,
      last_run: {
        trigger: "scheduled" as const,
        started: "2026-10-20T10:00:00Z",
        ended: "2026-10-20T10:01:00Z",
        outcome,
        checked: 100_000,
        not_checked,
        error_code,
      },
    });
    const libraries = [run("failed", "unavailable", 0), run("capped", null, 250)];
    const el = mount(fakeClient(view([], { libraries })).client);
    await settle();
    const [failed, capped] = rows(el, "runs");
    expect(failed?.[4]).toBe(`${labels.runFailed} (unavailable)`);
    expect(capped?.slice(4, 7)).toEqual([labels.runCapped, "100000", "250"]);
  });

  it("says nothing is open to validate when the host has no library open", async () => {
    const el = mount(fakeClient(view([], { libraries: [] })).client);
    await settle();
    expect(rows(el, "runs")).toEqual([[labels.findingsNoLibraries]]);
  });

  it("shows the schedule in force", async () => {
    const el = mount(fakeClient(view([])).client);
    await settle();
    const schedule = el.shadowRoot?.querySelector('[data-part="schedule"]')?.textContent ?? "";
    expect(schedule).toContain(labels.findingsInterval.replace("{hours}", "168"));
    expect(schedule).toContain("100000");
  });

  it("says when more findings are open than listed", async () => {
    const el = mount(fakeClient(view([finding("E023")], { truncated: true })).client);
    await settle();
    const note = el.shadowRoot?.querySelector<HTMLElement>('[data-note="truncated"]');
    expect(note?.hidden).toBe(false);
    expect(note?.textContent).toBe(labels.findingsTruncated);
  });

  it("drops a finding once a refresh no longer reports it", async () => {
    const el = mount(fakeClient(view([finding("E023")]), view([])).client);
    await settle();
    expect(rows(el, "errors")).toHaveLength(1);
    await el.refresh();
    expect(rows(el, "errors")).toEqual([[labels.findingsNoErrors]]);
  });

  it("coalesces refreshes: one request in flight and one queued behind it", async () => {
    const fake = fakeClient(view([]));
    const el = mount(fake.client);
    await settle();
    const before = fake.reads();
    const all = [el.refresh(), el.refresh(), el.refresh(), el.refresh()];
    await Promise.all(all);
    await settle();
    expect(fake.reads() - before).toBe(2);
  });

  it("starts a run of every open library and reports what started", async () => {
    const fake = fakeClient(view([]));
    const el = mount(fake.client);
    await settle();
    const button = el.shadowRoot?.querySelector<HTMLButtonElement>(
      'button[data-action="validate"]',
    );
    expect(button?.textContent).toBe(labels.findingsValidate);
    const reads = fake.reads();
    button?.click();
    await settle();
    await settle();
    expect(fake.runs).toEqual([{}]);
    const result = el.shadowRoot?.querySelector('[data-part="run-result"]');
    expect(result?.getAttribute("role")).toBe("status");
    expect(result?.textContent).toBe(
      labels.findingsRunResult.replace("{started}", "1").replace("{refused}", "0"),
    );
    expect(fake.reads()).toBeGreaterThan(reads);
  });

  it("words a refused start from the label table", async () => {
    const fake = fakeClient(view([]));
    fake.answerRun(
      new ApiError(403, "/api/v1/admin/ocfl/validation-runs", { reason: "not_permitted" }),
    );
    const el = mount(fake.client);
    await settle();
    el.shadowRoot?.querySelector<HTMLButtonElement>('button[data-action="validate"]')?.click();
    await settle();
    const result = el.shadowRoot?.querySelector<HTMLElement>('[data-part="run-result"]');
    expect(result?.textContent).toBe(
      labels.findingsRunFailed.replace("{reason}", labels.authNotPermitted),
    );
    expect(result?.dataset.outcome).toBe("failed");
  });

  it("shows the unavailable state with Retry when the findings route answers 503", async () => {
    const fake = fakeClient(new ApiError(503, "/api/v1/admin/ocfl/findings"), view([]));
    const el = mount(fake.client);
    await settle();
    expect(el.dataset.state).toBe("failed");
    const states = el.shadowRoot?.querySelector("fs-states") as HTMLElement;
    expect(states.hidden).toBe(false);
    expect(states.dataset.state).toBe("unavailable");
    expect(el.shadowRoot?.querySelector<HTMLElement>(".content")?.hidden).toBe(true);
    states.shadowRoot?.querySelector("button")?.click();
    await settle();
    await settle();
    expect(el.dataset.state).toBe("ready");
    expect(states.hidden).toBe(true);
  });

  it("names every table by its heading inside a focusable scroll region (UI-S-016)", async () => {
    const el = mount(fakeClient(view([finding("E023"), finding("W010")])).client);
    await settle();
    const tables = [...(el.shadowRoot?.querySelectorAll("table") ?? [])];
    expect(tables).toHaveLength(3);
    for (const table of tables) {
      const id = table.getAttribute("aria-labelledby") ?? "";
      expect(el.shadowRoot?.getElementById(id)?.textContent).not.toBe("");
      const region = table.parentElement;
      expect(region?.getAttribute("role")).toBe("region");
      expect(region?.getAttribute("tabindex")).toBe("0");
      expect(region?.getAttribute("aria-labelledby")).toBe(id);
    }
  });

  it("keeps focus on Validate now across a refresh", async () => {
    const fake = fakeClient(view([finding("E023")]), view([]));
    const el = mount(fake.client);
    await settle();
    const button = el.shadowRoot?.querySelector<HTMLButtonElement>(
      'button[data-action="validate"]',
    );
    button?.focus();
    await el.refresh();
    expect(el.shadowRoot?.activeElement).toBe(button);
  });

  it("renders finding values as text, never as markup", async () => {
    const hostile = finding("E023", { object_id: "<img src=x onerror=alert(1)>" });
    const el = mount(fakeClient(view([hostile])).client);
    await settle();
    expect(el.shadowRoot?.querySelector("img")).toBeNull();
    expect(rows(el, "errors")[0]?.[1]).toBe("<img src=x onerror=alert(1)>");
  });

  it("starts no queued read after it is removed", async () => {
    const fake = fakeClient(view([]));
    const el = mount(fake.client);
    await settle();
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const read = fake.client.adminOcflFindings;
    fake.client.adminOcflFindings = async () => {
      await gate;
      return read();
    };
    const before = fake.reads();
    void el.refresh();
    void el.refresh();
    el.remove();
    release();
    await settle();
    await settle();
    expect(fake.reads() - before).toBe(1);
  });

  it("says at least how many objects are read-only when the list is truncated", async () => {
    const el = mount(fakeClient(view([finding("W010")], { truncated: true })).client);
    await settle();
    const note = el.shadowRoot?.querySelector('[data-note="read-only"]');
    expect(note?.textContent).toBe(
      labels.findingsReadOnlyPartial.replace("{count}", "1").replace("{name}", LIB),
    );
  });

  it("names each library whose run was not started and why", async () => {
    const fake = fakeClient(view([]));
    fake.answerRun({ started: [], refused: [{ library_id: LIB, error_code: "already_running" }] });
    const el = mount(fake.client);
    await settle();
    el.shadowRoot?.querySelector<HTMLButtonElement>('button[data-action="validate"]')?.click();
    await settle();
    const result = el.shadowRoot?.querySelector('[data-part="run-result"]');
    const summary = labels.findingsRunResult.replace("{started}", "0").replace("{refused}", "1");
    const refused = labels.findingsRunRefused
      .replace("{name}", LIB)
      .replace("{code}", "already_running");
    expect(result?.textContent).toBe(`${summary} ${refused}`);
  });
});
