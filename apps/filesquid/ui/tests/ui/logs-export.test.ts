/**
 * UI tests for the Download logs dialog (UI-F2.16; UI-S-021, UI-S-032, mockup 39): opened from
 * Download…, its scope radios with their lines, the two format cards, the note, the file name that
 * follows the format until it is edited, Choose… (the platform's save dialog, disabled where the
 * browser has none), Download through the export route for the scope (current filter, time
 * range, all entries) in the chosen format, a failure that keeps the dialog open with an alert,
 * and focus: first radio, Tab kept inside, Escape and Cancel close and return focus.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { defineElements, type FsLogs } from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { LIBRARY, MemoryLogsSource, NOW } from "../support/logs-fakes";

beforeEach(() => defineElements());
afterEach(() => {
  vi.restoreAllMocks();
  delete (window as unknown as Record<string, unknown>)["showSaveFilePicker"];
});

/** Let queued promise callbacks run. */
const settle = async () => {
  for (let i = 0; i < 8; i += 1) await new Promise<void>((resolve) => setTimeout(resolve, 0));
};

/** Mount a view and open its Download dialog; `saved` collects what the dialog saves. */
async function open(source = new MemoryLogsSource()) {
  const saved: { blob: Blob; name: string }[] = [];
  const view = document.createElement("fs-logs") as FsLogs;
  view.configure({
    labels,
    source,
    zone: "Europe/Berlin",
    now: () => NOW,
    online: () => true,
    save: (blob, name) => saved.push({ blob, name }),
  });
  document.body.replaceChildren(view);
  await settle();
  const root = view.shadowRoot as ShadowRoot;
  const opener = root.querySelector(".download") as HTMLButtonElement;
  opener.click();
  const dialog = root.querySelector('[role="dialog"]') as HTMLElement;
  return { view, root, source, saved, dialog, opener };
}

/** Choose a radio of a group by its value. */
function pick(dialog: HTMLElement, group: string, value: string): void {
  const radio = dialog.querySelector<HTMLInputElement>(`input[name="${group}"][value="${value}"]`);
  radio?.click();
}

/** The text the dialog saved. */
const textOf = (blob: Blob) =>
  new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsText(blob);
  });

describe("the Download logs dialog", () => {
  it("opens from Download… as a modal dialog named by its title, with focus on the first radio", async () => {
    const { dialog, root } = await open();
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    const title = dialog.querySelector(".dialog-title") as HTMLElement;
    expect(title.textContent).toBe(labels.logsExportTitle);
    expect(dialog.getAttribute("aria-labelledby")).toBe(title.id);
    expect(root.activeElement).toBe(dialog.querySelector('input[name="scope"][value="filter"]'));
    expect(dialog.querySelector(".sub")?.textContent).toBe(labels.logsExportSub);
  });

  it("lists the three scopes with their lines and the two formats with their hints", async () => {
    const { dialog } = await open();
    const names = [...dialog.querySelectorAll(".scope-option .name")].map((n) => n.textContent);
    expect(names).toEqual([
      labels.logsExportScopeFilter,
      labels.logsExportScopeRange,
      labels.logsExportScopeAll,
    ]);
    const lines = [...dialog.querySelectorAll(".scope-option .line")].map((n) => n.textContent);
    expect(lines[0]).toBe("Last 24 hours · all libraries · all operations · 16 entries");
    expect(lines[1]).toMatch(
      /^2026-10-12 \d\d:\d\d to 2026-10-19 \d\d:\d\d, your time \(UTC\+2\)$/,
    );
    expect(lines[2]).toBe("Every entry since Oct 19, 2026");
    const cards = [...dialog.querySelectorAll(".card")].map((c) => c.textContent);
    expect(cards).toEqual([
      `${labels.logsFormatJsonl}${labels.logsFormatJsonlHint}`,
      `${labels.logsFormatCsv}${labels.logsFormatCsvHint}`,
    ]);
    expect(dialog.querySelector(".note-box")?.textContent).toContain(labels.logsExportNote);
    expect(
      dialog.querySelector<HTMLInputElement>('input[name="format"][value="jsonl"]')?.checked,
    ).toBe(true);
  });

  it("proposes the product prefix, the local date and the format, and follows the format until edited", async () => {
    const { dialog } = await open();
    const name = dialog.querySelector("#logs-export-name") as HTMLInputElement;
    expect(name.value).toBe("filesquid-logs-2026-10-19.jsonl");
    pick(dialog, "format", "csv");
    expect(name.value).toBe("filesquid-logs-2026-10-19.csv");
    name.value = "support.csv";
    name.dispatchEvent(new Event("input", { bubbles: true }));
    pick(dialog, "format", "jsonl");
    expect(name.value).toBe("support.csv");
  });

  it("says the save dialog is not available, and disables Choose…, in a browser with none", async () => {
    const { dialog } = await open();
    const choose = dialog.querySelector("button.choose") as HTMLButtonElement;
    expect(choose.disabled).toBe(true);
    expect(dialog.querySelector(".where")?.textContent).toBe(labels.logsExportSavedToOnly);
  });
});

describe("Download", () => {
  it("exports the current filter as JSON Lines through the route and saves it, then closes", async () => {
    const { dialog, source, saved, root, opener } = await open();
    (dialog.querySelector("button.submit") as HTMLButtonElement).click();
    await settle();
    expect(source.exports).toEqual([{ format: "jsonl", from: "2026-10-18T11:10:00.000Z" }]);
    expect(saved).toHaveLength(1);
    expect(saved[0]?.name).toBe("filesquid-logs-2026-10-19.jsonl");
    const text = await textOf(saved[0]?.blob as Blob);
    expect(text.trim().split("\n")).toHaveLength(16);
    expect(JSON.parse(text.split("\n")[0] ?? "{}").operation).toBe("ocfl.commit");
    expect(text).not.toContain("display_name");
    expect(root.querySelector('[role="dialog"]')).toBeNull();
    expect(root.activeElement).toBe(opener);
    expect(root.querySelector(".announce")?.textContent).toBe(
      "Downloaded filesquid-logs-2026-10-19.jsonl.",
    );
  });

  it("exports the CSV format with the chosen name", async () => {
    const { dialog, source, saved } = await open();
    pick(dialog, "format", "csv");
    (dialog.querySelector("button.submit") as HTMLButtonElement).click();
    await settle();
    expect(source.exports[0]?.format).toBe("csv");
    expect(saved[0]?.name).toBe("filesquid-logs-2026-10-19.csv");
    expect(saved[0]?.blob.type).toContain("text/csv");
  });

  it("exports all entries with nothing but the format", async () => {
    const { dialog, source } = await open();
    pick(dialog, "scope", "all");
    (dialog.querySelector("button.submit") as HTMLButtonElement).click();
    await settle();
    expect(source.exports).toEqual([{ format: "jsonl" }]);
  });

  it("exports a time range between the two times chosen, in the viewer's time", async () => {
    const { dialog, source } = await open();
    pick(dialog, "scope", "range");
    const fields = dialog.querySelector(".range-fields") as HTMLElement;
    expect(fields.hidden).toBe(false);
    const [from, to] = [...fields.querySelectorAll<HTMLInputElement>("input")] as [
      HTMLInputElement,
      HTMLInputElement,
    ];
    from.value = "2026-10-01T00:00";
    from.dispatchEvent(new Event("input", { bubbles: true }));
    to.value = "2026-10-02T12:30";
    to.dispatchEvent(new Event("input", { bubbles: true }));
    expect(dialog.querySelectorAll(".scope-option .line")[1]?.textContent).toBe(
      "2026-10-01 00:00 to 2026-10-02 12:30, your time (UTC+2)",
    );
    (dialog.querySelector("button.submit") as HTMLButtonElement).click();
    await settle();
    const query = source.exports[0];
    expect(query?.format).toBe("jsonl");
    expect(query?.from).toBe(new Date("2026-10-01T00:00").toISOString());
    expect(query?.to).toBe(new Date("2026-10-02T12:30").toISOString());
  });

  it("refuses a range that does not start before it ends, and a name with a slash, without sending", async () => {
    const { dialog, source } = await open();
    pick(dialog, "scope", "range");
    const [from, to] = [...dialog.querySelectorAll<HTMLInputElement>(".range-fields input")] as [
      HTMLInputElement,
      HTMLInputElement,
    ];
    from.value = "2026-10-05T00:00";
    from.dispatchEvent(new Event("input", { bubbles: true }));
    to.value = "2026-10-04T00:00";
    to.dispatchEvent(new Event("input", { bubbles: true }));
    (dialog.querySelector("button.submit") as HTMLButtonElement).click();
    await settle();
    expect(dialog.querySelector(".failed")?.textContent).toBe(labels.logsExportRangeInvalid);
    pick(dialog, "scope", "filter");
    const name = dialog.querySelector("#logs-export-name") as HTMLInputElement;
    name.value = "a/b.jsonl";
    name.dispatchEvent(new Event("input", { bubbles: true }));
    (dialog.querySelector("button.submit") as HTMLButtonElement).click();
    await settle();
    expect(dialog.querySelector(".failed")?.textContent).toBe(labels.logsExportNameInvalid);
    expect(source.exports).toHaveLength(0);
  });

  it("keeps the dialog open with an alert, saving nothing, when the export fails", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new MemoryLogsSource();
    source.exportError = new ApiError(500, "/api/v1/admin/journal/export", { code: "internal" });
    const { dialog, saved, root } = await open(source);
    (dialog.querySelector("button.submit") as HTMLButtonElement).click();
    expect(dialog.getAttribute("aria-busy")).toBe("true");
    expect((dialog.querySelector("button.submit") as HTMLButtonElement).disabled).toBe(true);
    expect(dialog.querySelector(".busy")?.textContent).toBe(labels.logsExportBusy);
    await settle();
    const alert = dialog.querySelector('[role="alert"]') as HTMLElement;
    expect(alert.textContent).toBe(labels.logsExportFailed);
    expect(saved).toHaveLength(0);
    expect(root.querySelector('[role="dialog"]')).toBe(dialog);
    expect((dialog.querySelector("button.submit") as HTMLButtonElement).disabled).toBe(false);
    // Another try, once the route answers, saves.
    source.exportError = undefined;
    (dialog.querySelector("button.submit") as HTMLButtonElement).click();
    await settle();
    expect(saved).toHaveLength(1);
  });
});

describe("Choose…", () => {
  it("writes to the file chosen in the platform's save dialog instead of downloading", async () => {
    const written: Blob[] = [];
    const target = {
      name: "from-the-dialog.jsonl",
      createWritable: async () => ({
        write: async (blob: Blob) => void written.push(blob),
        close: async () => undefined,
        abort: async () => undefined,
      }),
    };
    const picker = vi.fn(async () => target);
    (window as unknown as Record<string, unknown>)["showSaveFilePicker"] = picker;
    const { dialog, saved } = await open();
    const choose = dialog.querySelector("button.choose") as HTMLButtonElement;
    expect(choose.disabled).toBe(false);
    expect(dialog.querySelector(".where")?.textContent).toBe(labels.logsExportSavedTo);
    choose.click();
    await settle();
    expect(picker).toHaveBeenCalledWith(
      expect.objectContaining({ suggestedName: "filesquid-logs-2026-10-19.jsonl" }),
    );
    expect((dialog.querySelector("#logs-export-name") as HTMLInputElement).value).toBe(
      "from-the-dialog.jsonl",
    );
    (dialog.querySelector("button.submit") as HTMLButtonElement).click();
    await settle();
    expect(written).toHaveLength(1);
    expect(saved).toHaveLength(0);
  });

  it("keeps the name when the person cancels the save dialog", async () => {
    const abort = new DOMException("cancelled", "AbortError");
    (window as unknown as Record<string, unknown>)["showSaveFilePicker"] = vi.fn(async () => {
      throw abort;
    });
    const { dialog } = await open();
    (dialog.querySelector("button.choose") as HTMLButtonElement).click();
    await settle();
    expect((dialog.querySelector("#logs-export-name") as HTMLInputElement).value).toBe(
      "filesquid-logs-2026-10-19.jsonl",
    );
    expect(dialog.querySelector(".failed")?.textContent).toBe("");
  });
});

describe("focus", () => {
  it("closes on Escape and on Cancel and returns focus to Download…", async () => {
    const first = await open();
    first.dialog.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(first.root.querySelector('[role="dialog"]')).toBeNull();
    expect(first.root.activeElement).toBe(first.opener);
    first.opener.click();
    const dialog = first.root.querySelector('[role="dialog"]') as HTMLElement;
    (dialog.querySelector("button.cancel") as HTMLButtonElement).click();
    expect(first.root.querySelector('[role="dialog"]')).toBeNull();
    expect(first.source.exports).toHaveLength(0);
  });

  it("keeps Tab inside the dialog", async () => {
    const { dialog, root } = await open();
    const submit = dialog.querySelector("button.submit") as HTMLButtonElement;
    submit.focus();
    submit.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true }),
    );
    expect(root.activeElement).toBe(dialog.querySelector('input[name="scope"]'));
  });

  it("names the library of the filter in the Current filter line", async () => {
    const source = new MemoryLogsSource();
    const view = document.createElement("fs-logs") as FsLogs;
    view.configure({ labels, source, zone: "UTC", now: () => NOW });
    document.body.replaceChildren(view);
    await settle();
    const root = view.shadowRoot as ShadowRoot;
    root.querySelector<HTMLButtonElement>('button[data-chip="library"]')?.click();
    root.querySelector<HTMLElement>(`[data-id="${LIBRARY}"]`)?.click();
    await settle();
    (root.querySelector(".download") as HTMLButtonElement).click();
    expect(root.querySelector(".scope-option .line")?.textContent).toBe(
      "Last 24 hours · Family archive · all operations · 14 entries",
    );
  });
});
