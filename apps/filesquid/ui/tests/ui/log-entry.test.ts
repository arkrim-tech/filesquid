/**
 * UI tests for `<fs-log-entry>`, the detail of one Logs entry (UI-F2.16; UI-S-021, UI-S-032,
 * mockup 38): the rows of the journal's fields drawn by the shared property table (copy buttons,
 * no filter), the operation and outcome chip, View provenance for a committed operation only, the
 * journal-only note, Show file, Copy as JSON without the display name, the opaque-IDs note, the
 * second read for what a live event did not carry, and a read that fails.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { accessibilityViolations, defineElements, type FsLogEntry } from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import {
  ACTOR,
  LIBRARIES,
  LIBRARY,
  MemoryLogsSource,
  NEWEST_ID,
  PLAN_FILE,
  PLAN_OPERATION,
  mockupEntries,
} from "../support/logs-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

/** Let queued promise callbacks run. */
const settle = async () => {
  for (let i = 0; i < 8; i += 1) await new Promise<void>((resolve) => setTimeout(resolve, 0));
};

/** The entry of mockup 38's selected row (metadata.set on the plan). */
const planEntry = () => mockupEntries()[5] as ReturnType<typeof mockupEntries>[number];

/** Mount a detail panel over `source` showing `index` of the fixture. */
async function mount(
  source = new MemoryLogsSource(),
  entry: ReturnType<typeof mockupEntries>[number] | null = planEntry(),
): Promise<FsLogEntry> {
  const panel = document.createElement("fs-log-entry") as FsLogEntry;
  panel.configure({ labels, source, zone: "Europe/Berlin", online: () => true });
  panel.libraries = LIBRARIES;
  panel.entry = entry;
  document.body.replaceChildren(panel);
  await settle();
  return panel;
}

/** The root of a panel. */
function root(panel: FsLogEntry): ShadowRoot {
  return panel.shadowRoot as ShadowRoot;
}

/** The label and value of every row, keyed by the row's key. */
function rowsOf(panel: FsLogEntry): Record<string, { label: string; value: string }> {
  const out: Record<string, { label: string; value: string }> = {};
  for (const row of root(panel).querySelectorAll<HTMLElement>(".row")) {
    out[row.dataset.key ?? ""] = {
      label: row.querySelector("dt")?.textContent?.trim() ?? "",
      value: row.querySelector("dd")?.textContent?.trim() ?? "",
    };
  }
  return out;
}

describe("<fs-log-entry> detail", () => {
  it("is a defined custom element that asks for the wide panel region", async () => {
    expect(customElements.get("fs-log-entry")).toBeDefined();
    const panel = await mount();
    expect(panel.hasAttribute("data-panel-wide")).toBe(true);
    expect(root(panel).querySelector(".head-top h2")?.textContent).toBe(labels.logsDetailTitle);
  });

  it("shows the operation in mono and the outcome chip", async () => {
    const panel = await mount();
    expect(root(panel).querySelector(".operation")?.textContent).toBe("metadata.set");
    const chip = root(panel).querySelector(".title .chip") as HTMLElement;
    expect(chip.textContent).toBe("ok");
    expect(chip.classList.contains("ok")).toBe(true);
  });

  it("draws the rows of mockup 38 from what the journal carries", async () => {
    const panel = await mount();
    const rows = rowsOf(panel);
    expect(Object.keys(rows)).toEqual([
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
    expect(rows["time-utc"]).toEqual({ label: "Time (UTC)", value: "2026-10-19T11:03:02.418Z" });
    expect(rows["time-local"]?.value).toBe("Oct 19, 2026, 13:03:02 (UTC+2)");
    expect(rows["actor"]?.value).toContain(ACTOR);
    expect(rows["actor"]?.value).toContain("Owner agent · this installation");
    expect(rows["library"]?.value).toBe("Family archive");
    expect(rows["object"]?.value).toBe(`urn:uuid:${PLAN_FILE} → v8`);
    expect(rows["file"]?.value).toBe("harbour-masterplan-v7.pdf· looked up in your library");
    expect(rows["operation-id"]?.value).toBe(PLAN_OPERATION);
    expect(rows["duration"]?.value).toBe("38 ms");
  });

  it("has no filter field, and a copy button for each row, named by its property", async () => {
    const panel = await mount();
    expect(root(panel).querySelector("input")).toBeNull();
    const copy = root(panel).querySelector('.row[data-key="operation-id"] button.copy');
    expect(copy?.getAttribute("aria-label")).toBe("Copy Operation id");
  });

  it("copies a row's value and announces it", async () => {
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const panel = await mount();
    (root(panel).querySelector('.row[data-key="operation-id"] button.copy') as HTMLElement).click();
    await settle();
    expect(writeText).toHaveBeenCalledWith(PLAN_OPERATION);
    expect(root(panel).querySelector(".announce")?.textContent).toBe("Operation id copied");
  });

  it("says the entry is opaque ids only, in the raised note", async () => {
    const panel = await mount();
    expect(root(panel).querySelector(".note-box.raised")?.textContent).toContain(
      labels.logsOpaqueNote,
    );
  });
});

describe("<fs-log-entry> links", () => {
  it("links View provenance and Show file to the file's panel on its own tabs", async () => {
    const panel = await mount();
    const provenance = root(panel).querySelector<HTMLAnchorElement>("a.view-provenance");
    const show = root(panel).querySelector<HTMLAnchorElement>("a.show-file");
    expect(provenance?.hidden).toBe(false);
    expect(provenance?.textContent).toBe(labels.logsViewProvenance);
    expect(provenance?.getAttribute("href")).toBe(
      `#/files/${LIBRARY}/urn%3Auuid%3A${PLAN_FILE}/original?open=${PLAN_FILE}&tab=provenance`,
    );
    expect(show?.getAttribute("href")).toBe(
      `#/files/${LIBRARY}/urn%3Auuid%3A${PLAN_FILE}/original?open=${PLAN_FILE}&tab=details`,
    );
    expect(root(panel).querySelector<HTMLElement>(".journal-only")?.hidden).toBe(true);
  });

  it("offers no View provenance for a journal-only entry, and says it is recorded in the logs only", async () => {
    const refused = mockupEntries()[6] as ReturnType<typeof mockupEntries>[number];
    const panel = await mount(new MemoryLogsSource(), refused);
    expect(root(panel).querySelector<HTMLElement>("a.view-provenance")?.hidden).toBe(true);
    const only = root(panel).querySelector<HTMLElement>(".journal-only");
    expect(only?.hidden).toBe(false);
    expect(only?.textContent).toBe(labels.logsJournalOnlyNote);
    expect(root(panel).querySelector(".title .chip")?.textContent).toBe("refused");
  });

  it("offers neither link for an entry that names no file", async () => {
    const stalled = mockupEntries()[2] as ReturnType<typeof mockupEntries>[number];
    const panel = await mount(new MemoryLogsSource(), stalled);
    expect(root(panel).querySelector<HTMLElement>(".links")?.hidden).toBe(true);
    expect(root(panel).querySelector(".title .chip")?.hasAttribute("hidden")).toBe(true);
  });

  it("offers no link while the host's libraries are not known", async () => {
    const panel = await mount();
    panel.libraries = [];
    expect(root(panel).querySelector<HTMLElement>(".links")?.hidden).toBe(true);
    expect(rowsOf(panel)["library"]).toBeUndefined();
  });
});

describe("<fs-log-entry> Copy as JSON", () => {
  it("puts the entry as stored on the clipboard, without the display name", async () => {
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const panel = await mount();
    (root(panel).querySelector("button.copy-json") as HTMLButtonElement).click();
    await settle();
    const copied = JSON.parse((writeText.mock.calls[0] as unknown as [string])[0]) as Record<
      string,
      unknown
    >;
    expect(copied["id"]).toBe(NEWEST_ID - 5);
    expect(copied["operation"]).toBe("metadata.set");
    expect("display_name" in copied).toBe(false);
    expect(root(panel).querySelector(".announce")?.textContent).toBe(labels.logsCopied);
  });

  it("says so when the clipboard refuses", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const writeText = vi.fn(async () => {
      throw new DOMException("no", "NotAllowedError");
    });
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const panel = await mount();
    (root(panel).querySelector("button.copy-json") as HTMLButtonElement).click();
    await settle();
    expect(root(panel).querySelector(".announce")?.textContent).toBe(labels.logsCopyFailed);
  });
});

describe("<fs-log-entry> second read", () => {
  it("reads the entry again for its display name and shows what it returns", async () => {
    const source = new MemoryLogsSource();
    const live = { ...planEntry() } as Partial<ReturnType<typeof planEntry>>;
    delete live.display_name;
    const panel = await mount(source, live as ReturnType<typeof planEntry>);
    expect(source.entryReads).toEqual([NEWEST_ID - 5]);
    expect(rowsOf(panel)["file"]?.value).toContain("harbour-masterplan-v7.pdf");
  });

  it("keeps what the list gave and says so when the entry cannot be read", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new MemoryLogsSource();
    source.entryError = new Error("down");
    const panel = await mount(source);
    expect(root(panel).querySelector<HTMLElement>(".read-failed")?.hidden).toBe(false);
    expect(root(panel).querySelector(".read-failed")?.textContent).toBe(labels.logsEntryLoadFailed);
    expect(rowsOf(panel)["operation-id"]?.value).toBe(PLAN_OPERATION);
  });

  it("clears the panel for no entry, and closes on the close button", async () => {
    const panel = await mount();
    const closed = vi.fn();
    panel.addEventListener("fs-log-entry-close", closed);
    (root(panel).querySelector("button.close") as HTMLButtonElement).click();
    expect(closed).toHaveBeenCalledTimes(1);
    expect(root(panel).querySelector("button.close")?.getAttribute("aria-label")).toBe(
      labels.logsDetailClose,
    );
    panel.entry = null;
    expect(panel.hidden).toBe(true);
  });
});

describe("<fs-log-entry> accessibility", () => {
  it("has no accessibility violations and is a named region", async () => {
    const panel = await mount();
    expect(accessibilityViolations(root(panel))).toEqual([]);
    expect(root(panel).querySelector("section")?.getAttribute("aria-label")).toBe(
      labels.logsDetailTitle,
    );
  });
});
