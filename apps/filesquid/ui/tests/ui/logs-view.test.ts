/**
 * UI tests for `<fs-logs>`, the Logs view (UI-F2.16; UI-S-021, UI-S-032, mockup 38): the head and
 * the filter row from the label table, the rows of the journal newest first with their outcome
 * chips and journal-only tags, paging with Load older, every filter (time range, library,
 * operation, outcome, level), the search by file, object, operation ID and operation, the filter
 * a link carries, selection, live entries, a resync, the footer sentences and the loading, empty,
 * no-match, error and denied states. The view reads only through a fake {@link LogsSource}.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { accessibilityViolations, defineElements, type FsLogs } from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import {
  IMG_FILE,
  IMG_OPERATION,
  LIBRARY,
  MemoryLogsSource,
  NEWEST_ID,
  NOW,
  PLAN_FILE,
  PLAN_OPERATION,
  mockupEntries,
  olderEntries,
} from "../support/logs-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

/** Let queued promise callbacks run. */
const settle = async () => {
  for (let i = 0; i < 8; i += 1) await new Promise<void>((resolve) => setTimeout(resolve, 0));
};

/** Mount a view over `source` and wait for its first read. */
async function mount(source = new MemoryLogsSource(), zone = "Europe/Berlin"): Promise<FsLogs> {
  const view = document.createElement("fs-logs") as FsLogs;
  view.configure({ labels, source, zone, now: () => NOW, online: () => true });
  document.body.replaceChildren(view);
  await settle();
  return view;
}

/** The shadow root of a view. */
function root(view: FsLogs): ShadowRoot {
  return view.shadowRoot as ShadowRoot;
}

/** The rows of the table, as text cells. */
function rows(view: FsLogs): string[][] {
  return [...root(view).querySelectorAll("tbody tr[data-key]")].map((row) =>
    [...row.querySelectorAll("td")].map((cell) => cell.textContent?.trim() ?? ""),
  );
}

/** Press a chip and choose a menu item by its label. */
async function choose(view: FsLogs, chip: string, item: string): Promise<void> {
  root(view).querySelector<HTMLButtonElement>(`button[data-chip="${chip}"]`)?.click();
  const entry = [...root(view).querySelectorAll<HTMLElement>(".menu-item")].find(
    (el) => el.textContent?.trim() === item,
  );
  entry?.click();
  await settle();
}

/** Type in the search field and submit. */
async function search(view: FsLogs, text: string): Promise<void> {
  const input = root(view).querySelector("input") as HTMLInputElement;
  input.value = text;
  input.dispatchEvent(new Event("input"));
  root(view)
    .querySelector("form")
    ?.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
  await settle();
}

describe("<fs-logs> head and filter row", () => {
  it("is a defined custom element and speaks the label table", async () => {
    expect(customElements.get("fs-logs")).toBeDefined();
    const view = await mount();
    expect(root(view).querySelector("h2")?.textContent).toBe(labels.logs);
    expect(root(view).querySelector(".intro")?.textContent).toBe(labels.logsIntro);
    expect(root(view).querySelector(".download")?.textContent).toBe(labels.logsDownload);
    const input = root(view).querySelector("input") as HTMLInputElement;
    expect(input.placeholder).toBe(labels.logsSearchPlaceholder);
    const chips = [...root(view).querySelectorAll(".filter-chip")].map((c) =>
      c.textContent?.trim(),
    );
    expect(chips).toEqual([
      "Last 24 hours",
      "Library: all",
      "Operation: all",
      "Outcome: all",
      "Level: all",
    ]);
  });

  it("shows the Live chip while it follows the event channel", async () => {
    const view = await mount();
    const live = root(view).querySelector<HTMLElement>(".chip.live");
    expect(live?.hidden).toBe(false);
    expect(live?.textContent?.trim()).toBe(labels.logsLive);
  });
});

describe("<fs-logs> rows", () => {
  it("lists the journal newest first, with the cells of mockup 38", async () => {
    const view = await mount();
    const list = rows(view);
    expect(list).toHaveLength(16);
    expect(list[0]).toEqual(["13:05:42", "info", "ocfl.commit", "ok", "IMG_4102.jpg", "a71c…3e90"]);
    expect(list[5]?.slice(0, 5)).toEqual([
      "13:03:02",
      "info",
      "metadata.set",
      "ok",
      "harbour-masterplan-v7.pdf",
    ]);
    expect(list[5]?.[5]).toBe("3f9c…0e21");
  });

  it("draws failed and refused outcomes in the alert tone and tags them journal-only", async () => {
    const view = await mount();
    const failed = root(view).querySelector(`tr[data-key="${NEWEST_ID - 3}"]`) as HTMLElement;
    expect(failed.querySelector(".chip.alert")?.textContent).toBe("failed");
    expect(failed.querySelector(".journal-only")?.textContent).toBe(labels.logsJournalOnly);
    expect(failed.querySelector("td.level")?.classList.contains("alert")).toBe(true);
    const refused = root(view).querySelector(`tr[data-key="${NEWEST_ID - 6}"]`) as HTMLElement;
    expect(refused.querySelector(".chip.alert")?.textContent).toBe("refused");
    expect(refused.querySelector(".journal-only")).not.toBeNull();
  });

  it("shows a dash for an entry with no outcome and for no operation id", async () => {
    const view = await mount();
    const stalled = rows(view)[2] ?? [];
    expect(stalled[3]).toBe(labels.logsNone);
    expect(stalled[5]).toBe(labels.logsNone);
    expect(root(view).querySelector(`tr[data-key="${NEWEST_ID - 2}"] .journal-only`)).toBeNull();
  });

  it("shows the opaque ID where no display name was resolved", async () => {
    const view = await mount();
    expect(rows(view)[4]?.[4]).toBe("job-upload-eos");
  });

  it("names a library target by the name the libraries route gave", async () => {
    const view = await mount();
    expect(rows(view)[8]?.[4]).toBe("Family archive");
  });

  it("says how many are shown of how many when the whole range is read", async () => {
    const view = await mount();
    expect(root(view).querySelector(".showing")?.textContent).toBe(
      "Showing 16 of 16 entries in the last 24 hours",
    );
    expect(root(view).querySelector<HTMLElement>("button.older")?.hidden).toBe(true);
  });

  it("asks newest first, with names, one page, from 24 hours ago", async () => {
    const source = new MemoryLogsSource();
    await mount(source);
    expect(source.pages[0]).toEqual({
      order: "desc",
      names: true,
      limit: 50,
      from: "2026-10-18T11:10:00.000Z",
    });
  });
});

describe("<fs-logs> paging", () => {
  it("loads older entries with the cursor and says more are available until the end", async () => {
    const source = new MemoryLogsSource([...mockupEntries(), ...olderEntries(60)]);
    const view = await mount(source);
    expect(rows(view)).toHaveLength(50);
    expect(root(view).querySelector(".showing")?.textContent).toBe(
      "Showing 50 entries in the last 24 hours · more are available",
    );
    const older = root(view).querySelector("button.older") as HTMLButtonElement;
    expect(older.hidden).toBe(false);
    older.click();
    await settle();
    expect(source.pages[1]?.after).toBe(NEWEST_ID - 49);
    expect(rows(view)).toHaveLength(76);
    expect(root(view).querySelector(".showing")?.textContent).toBe(
      "Showing 76 of 76 entries in the last 24 hours",
    );
    expect(older.hidden).toBe(true);
  });

  it("keeps the rows and says so when older entries cannot be read", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new MemoryLogsSource([...mockupEntries(), ...olderEntries(60)]);
    const view = await mount(source);
    source.pageError = new Error("down");
    (root(view).querySelector("button.older") as HTMLButtonElement).click();
    await settle();
    expect(rows(view)).toHaveLength(50);
    expect(root(view).querySelector(".announce")?.textContent).toBe(labels.logsErrorTitle);
    expect((root(view).querySelector("button.older") as HTMLButtonElement).disabled).toBe(false);
  });
});

describe("<fs-logs> filters", () => {
  it("filters by time range", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    await choose(view, "range", "Last hour");
    expect(source.pages.at(-1)?.from).toBe("2026-10-19T10:10:00.000Z");
    expect(root(view).querySelector(".filter-chip.range")?.textContent?.trim()).toBe("Last hour");
    await choose(view, "range", "All time");
    expect(source.pages.at(-1)?.from).toBeUndefined();
    expect(root(view).querySelector(".showing")?.textContent).toContain("since the first entry");
  });

  it("filters by library, naming it by the name the libraries route gave", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    await choose(view, "library", "Family archive");
    expect(source.pages.at(-1)?.library).toBe(LIBRARY);
    expect(root(view).querySelector(".filter-chip.library")?.textContent?.trim()).toBe(
      "Library: Family archive",
    );
    await choose(view, "library", labels.logsMenuAllLibraries);
    expect(source.pages.at(-1)?.library).toBeUndefined();
  });

  it("filters by operation, offering the operations it has read", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    root(view).querySelector<HTMLButtonElement>('button[data-chip="operation"]')?.click();
    const items = [...root(view).querySelectorAll(".menu-item")].map((i) => i.textContent?.trim());
    expect(items).toContain(labels.logsMenuAllOperations);
    expect(items).toContain("metadata.set");
    expect(items).toContain("ocfl.commit");
    expect(new Set(items).size).toBe(items.length);
    root(view).querySelector<HTMLElement>('[data-id="metadata.set"]')?.click();
    await settle();
    expect(source.pages.at(-1)?.operation).toBe("metadata.set");
    expect(rows(view).map((r) => r[2])).toEqual(["metadata.set"]);
  });

  it("filters by outcome, a refusal being the operation of every refusal", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    await choose(view, "outcome", "refused");
    expect(source.pages.at(-1)?.operation).toBe("api.auth.denied");
    expect(source.pages.at(-1)?.outcome).toBeUndefined();
    expect(rows(view)).toHaveLength(1);
    await choose(view, "outcome", "failed");
    expect(source.pages.at(-1)?.outcome).toBe("failed");
  });

  it("filters by level", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    await choose(view, "level", "error");
    expect(source.pages.at(-1)?.level).toBe("error");
    expect(rows(view).map((r) => r[1])).toEqual(["error", "error"]);
    expect(root(view).querySelector(".filter-chip.level")?.getAttribute("data-active")).toBe(
      "true",
    );
  });

  it("opens a chip's menu as a named menu that closes on Escape and returns focus", async () => {
    const view = await mount();
    const chip = root(view).querySelector<HTMLButtonElement>(
      'button[data-chip="level"]',
    ) as HTMLButtonElement;
    chip.click();
    const menu = root(view).querySelector('[role="menu"]');
    expect(menu?.getAttribute("aria-label")).toBe(labels.logsMenuLevel);
    expect(chip.getAttribute("aria-expanded")).toBe("true");
    menu?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(root(view).querySelector('[role="menu"]')).toBeNull();
    expect(root(view).activeElement).toBe(chip);
  });
});

describe("<fs-logs> search", () => {
  it("filters by an operation name", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    await search(view, "ocfl.commit");
    expect(source.pages.at(-1)?.operation).toBe("ocfl.commit");
    expect(rows(view)).toHaveLength(2);
    expect(root(view).querySelector(".hint")?.textContent).toBe("Searching operation: ocfl.commit");
  });

  it("filters by an object ID, with or without its version", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    await search(view, `urn:uuid:${PLAN_FILE}@v8`);
    expect(source.pages.at(-1)?.object).toBe(`urn:uuid:${PLAN_FILE}`);
    expect(rows(view).map((r) => r[2])).toEqual(["metadata.set", "version.restore"]);
    expect(root(view).querySelector(".filter-chip.object")?.textContent?.trim()).toBe(
      `Object: urn:uuid:${PLAN_FILE}`,
    );
  });

  it("filters by an operation ID, and when no entry has it, tries the UUID as a file ID", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    await search(view, PLAN_OPERATION);
    expect(source.pages.at(-1)?.operation_id).toBe(PLAN_OPERATION);
    expect(rows(view)).toHaveLength(1);
    await search(view, IMG_FILE);
    const queries = source.pages.slice(-2);
    expect(queries[0]?.operation_id).toBe(IMG_FILE);
    expect(queries[1]?.file).toBe(IMG_FILE);
    expect(queries[1]?.operation_id).toBeUndefined();
    expect(rows(view)).toHaveLength(2);
  });

  it("searches an opaque operation id, and says what text it cannot match", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    await search(view, "harbour-masterplan-v7.pdf");
    expect(source.pages.at(-1)?.operation_id).toBe("harbour-masterplan-v7.pdf");
    const before = source.pages.length;
    await search(view, "not an id");
    expect(source.pages).toHaveLength(before);
    const hint = root(view).querySelector(".hint") as HTMLElement;
    expect(hint.hidden).toBe(false);
    expect(hint.textContent).toBe(labels.logsSearchHint);
    expect(hint.classList.contains("invalid")).toBe(true);
  });

  it("removes a file filter with its chip and clears the search with its button", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    await search(view, `urn:uuid:${PLAN_FILE}`);
    (root(view).querySelector(".filter-chip.object") as HTMLButtonElement).click();
    await settle();
    expect(source.pages.at(-1)?.object).toBeUndefined();
    expect((root(view).querySelector("input") as HTMLInputElement).value).toBe("");
    await search(view, "ocfl.commit");
    (root(view).querySelector(".search .clear") as HTMLButtonElement).click();
    await settle();
    expect(source.pages.at(-1)?.operation).toBeUndefined();
  });
});

describe("<fs-logs> opened from a link", () => {
  it("opens filtered to a file, over all time, with the ID in the search field and a chip", async () => {
    const source = new MemoryLogsSource();
    const view = document.createElement("fs-logs") as FsLogs;
    view.configure({ labels, source, zone: "UTC", now: () => NOW, online: () => true });
    view.routeFilter = { file: PLAN_FILE };
    document.body.replaceChildren(view);
    await settle();
    expect(source.pages).toHaveLength(1);
    expect(source.pages[0]?.file).toBe(PLAN_FILE);
    expect(source.pages[0]?.from).toBeUndefined();
    expect((root(view).querySelector("input") as HTMLInputElement).value).toBe(PLAN_FILE);
    expect(root(view).querySelector(".filter-chip.file")?.textContent?.trim()).toBe(
      `File: ${PLAN_FILE}`,
    );
    expect(rows(view).map((r) => r[2])).toEqual(["metadata.set", "version.restore"]);
  });

  it("opens filtered to an operation, and returns to the default filter for the bare route", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    view.routeFilter = { operation_id: IMG_OPERATION };
    await settle();
    expect(source.pages.at(-1)?.operation_id).toBe(IMG_OPERATION);
    view.routeFilter = undefined;
    await settle();
    expect(source.pages.at(-1)?.operation_id).toBeUndefined();
    expect(source.pages.at(-1)?.from).toBe("2026-10-18T11:10:00.000Z");
  });

  it("does not read again for the address it already shows", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    view.routeFilter = { file: PLAN_FILE };
    await settle();
    const reads = source.pages.length;
    view.routeFilter = { file: PLAN_FILE };
    await settle();
    expect(source.pages).toHaveLength(reads);
  });
});

describe("<fs-logs> selection and the footer", () => {
  it("announces the selected entry with the libraries and marks its row", async () => {
    const view = await mount();
    const heard: { entry: { id: number } | null; libraries: unknown[] }[] = [];
    view.addEventListener("fs-logs-select", (event) =>
      heard.push((event as CustomEvent).detail as never),
    );
    const row = root(view).querySelector(`tr[data-key="${NEWEST_ID - 5}"]`) as HTMLElement;
    row.querySelector<HTMLButtonElement>("button.select")?.click();
    expect(heard[0]?.entry?.id).toBe(NEWEST_ID - 5);
    expect(heard[0]?.libraries).toHaveLength(1);
    expect(row.getAttribute("aria-selected")).toBe("true");
    expect(view.selectedEntry?.operation).toBe("metadata.set");
    view.select(undefined);
    expect(row.getAttribute("aria-selected")).toBe("false");
    expect(heard[1]?.entry).toBeNull();
  });

  it("names a row's button by its time, operation and outcome, and moves by arrow keys", async () => {
    const view = await mount();
    const buttons = [...root(view).querySelectorAll<HTMLButtonElement>("button.select")];
    expect(buttons[0]?.getAttribute("aria-label")).toBe("13:05:42, ocfl.commit, ok");
    expect(buttons.filter((b) => b.tabIndex === 0)).toHaveLength(1);
    buttons[0]?.focus();
    buttons[0]?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    expect(root(view).activeElement).toBe(buttons[1]);
    buttons[1]?.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true }));
    expect(root(view).activeElement).toBe(buttons.at(-1));
  });

  it("announces the time-zone sentence and the entry count for the shell footer", async () => {
    const view = document.createElement("fs-logs") as FsLogs;
    const heard: { zone: string; count: string }[] = [];
    view.addEventListener("fs-logs-footer", (event) => heard.push((event as CustomEvent).detail));
    view.configure({
      labels,
      source: new MemoryLogsSource(),
      zone: "Europe/Berlin",
      now: () => NOW,
    });
    document.body.replaceChildren(view);
    await settle();
    expect(heard.at(-1)).toEqual({
      zone: "Times in your time zone (UTC+2) · stored as UTC · kept forever",
      count: "16 entries",
    });
  });
});

describe("<fs-logs> live entries", () => {
  /** A live entry newer than the list, with the fields an event carries. */
  const arrival = (id: number, operation = "ocfl.commit") => ({
    ...mockupEntries()[0],
    id,
    operation,
    time: "2026-10-19T11:09:00.000Z",
  });

  it("counts entries that match the filter and adds them when the chip is chosen", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    source.emit(arrival(200) as never);
    source.emit(arrival(201) as never);
    const live = root(view).querySelector<HTMLElement>(".chip.live") as HTMLElement;
    expect(live.textContent?.trim()).toBe("Live · 2 new");
    expect(root(view).querySelector(".announce")?.textContent).toBe(
      "2 new entries arrived. Choose Live to show them.",
    );
    expect(rows(view)).toHaveLength(16);
    live.click();
    expect(rows(view)).toHaveLength(18);
    expect(rows(view)[0]?.[2]).toBe("ocfl.commit");
    expect(live.textContent?.trim()).toBe(labels.logsLive);
  });

  it("ignores an entry the filter would not have read, a repeat and an event of another kind", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    await choose(view, "level", "error");
    source.emit(arrival(200) as never);
    source.emit({ ...arrival(201), level: "error" } as never);
    source.emit({ ...arrival(201), level: "error" } as never);
    source.emitTopic("job.update");
    expect(root(view).querySelector(".chip.live")?.textContent?.trim()).toBe("Live · 1 new");
  });

  it("reads again after events.resync and when the channel reopens, without placeholder rows", async () => {
    const source = new MemoryLogsSource();
    await mount(source);
    source.emitTopic("events.resync");
    await settle();
    // The first open is the channel starting; the second is a reconnection.
    source.reopen();
    await settle();
    expect(source.pages).toHaveLength(2);
    source.reopen();
    await settle();
    expect(source.pages).toHaveLength(3);
  });

  it("stops following the channel when it leaves the document", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    expect(source.subscribed).toBe(true);
    view.remove();
    expect(source.subscribed).toBe(false);
  });
});

describe("<fs-logs> states", () => {
  it("shows placeholder rows while loading", async () => {
    const source = new MemoryLogsSource();
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const page = source.page.bind(source);
    source.page = async (filters) => {
      await gate;
      return page(filters);
    };
    const view = await mount(source);
    expect(view.dataset.state).toBe("loading");
    expect(root(view).querySelectorAll("tbody tr[aria-hidden='true']")).toHaveLength(10);
    expect(root(view).querySelector("tbody")?.getAttribute("aria-busy")).toBe("true");
    release();
    await settle();
    expect(view.dataset.state).toBe("ready");
  });

  it("says there are no entries yet for an empty journal", async () => {
    const view = await mount(new MemoryLogsSource([]));
    expect(view.dataset.state).toBe("empty");
    expect(root(view).querySelector(".state .title")?.textContent).toBe(labels.logsEmptyTitle);
    expect(root(view).querySelector(".state button")).toBeNull();
  });

  it("says no entries match and offers Clear filters", async () => {
    const source = new MemoryLogsSource();
    const view = await mount(source);
    await choose(view, "operation", "app.start");
    await choose(view, "level", "error");
    expect(root(view).querySelector(".state .title")?.textContent).toBe(labels.logsNoMatchTitle);
    (root(view).querySelector(".state button") as HTMLButtonElement).click();
    await settle();
    expect(source.pages.at(-1)).toEqual({
      order: "desc",
      names: true,
      limit: 50,
      from: "2026-10-18T11:10:00.000Z",
    });
    expect(rows(view)).toHaveLength(16);
  });

  it("shows the reason and code of a failed read with Try again, and recovers", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new MemoryLogsSource();
    source.pageError = new ApiError(500, "/api/v1/admin/journal", { code: "internal" });
    const view = await mount(source);
    expect(view.dataset.state).toBe("error");
    const state = root(view).querySelector(".state") as HTMLElement;
    expect(state.getAttribute("role")).toBe("alert");
    expect(state.querySelector(".title")?.textContent).toBe(labels.logsErrorTitle);
    expect(state.querySelector(".code")?.textContent).toBe("internal");
    source.pageError = undefined;
    (state.querySelector("button") as HTMLButtonElement).click();
    await settle();
    expect(view.dataset.state).toBe("ready");
    expect(rows(view)).toHaveLength(16);
  });

  it("shows a permission state with no entries for a caller who is not the owner", async () => {
    const source = new MemoryLogsSource();
    source.pageError = new ApiError(403, "/api/v1/admin/journal", { code: "forbidden" });
    const view = await mount(source);
    expect(view.dataset.state).toBe("denied");
    expect(root(view).querySelector(".state .title")?.textContent).toBe(labels.logsDeniedTitle);
    expect(root(view).querySelector(".state .detail")?.textContent).toBe(labels.logsDeniedDetail);
    expect(rows(view)).toHaveLength(0);
    expect(root(view).querySelector<HTMLElement>(".chip.live")?.hidden).toBe(true);
    expect(root(view).querySelector("tbody tr[data-key]")).toBeNull();
  });
});

describe("<fs-logs> accessibility", () => {
  it("has no accessibility violations and names its table region", async () => {
    const view = await mount();
    expect(accessibilityViolations(view.shadowRoot as ShadowRoot)).toEqual([]);
    const region = root(view).querySelector('[role="region"]');
    expect(region?.getAttribute("aria-label")).toBe(labels.logsTableLabel);
    expect(region?.getAttribute("tabindex")).toBe("0");
  });
});
