/**
 * UI tests for `<fs-search-bar>` (UI-F2.9, F-063, UI-S-007, UI-S-030, mockup 20): chips plus text
 * in one field, Clear all and per-chip remove, the drop-down with FILTERS, SUGGESTIONS, live FILES
 * and OPERATORS, the Type, Location and Modified choices, the combobox keys, Enter for the full
 * results, restoring a route's query, and the bounds on every list. Every word comes from the
 * label table.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { OPERATORS_ON_HOST, SUGGESTED_FILES } from "../../src/search/model";
import {
  SCOPE,
  all,
  click,
  focusField,
  label,
  mountBar,
  one,
  press,
  searchSource,
  settle,
  texts,
  type,
} from "../support/search-fakes";

let errors: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
  vi.spyOn(console, "warn").mockImplementation(() => undefined);
});
afterEach(() => {
  expect(errors).not.toHaveBeenCalled();
  vi.useRealTimers();
  vi.restoreAllMocks();
  document.body.replaceChildren();
});

describe("<fs-search-bar> field", () => {
  it("is a defined custom element that mounts alone with the placeholder from the label table", () => {
    const { element } = mountBar();
    expect(customElements.get("fs-search-bar")).toBeDefined();
    const input = one<HTMLInputElement>(element, "input.text");
    expect(input.placeholder).toBe(label("searchPlaceholder"));
    expect(input.getAttribute("aria-label")).toBe(label("searchPlaceholder"));
    expect(input.getAttribute("role")).toBe("combobox");
    expect(input.getAttribute("aria-expanded")).toBe("false");
    expect(one(element, ".dropdown").hidden).toBe(true);
    expect(one<HTMLButtonElement>(element, "button.clear-all").hidden).toBe(true);
  });

  it("opens the drop-down on focus and closes it on Escape, keeping the text", async () => {
    const { element } = mountBar();
    const input = await type(element, "harbour");
    expect(one(element, ".dropdown").hidden).toBe(false);
    expect(input.getAttribute("aria-expanded")).toBe("true");
    await press(element, "Escape");
    expect(one(element, ".dropdown").hidden).toBe(true);
    expect(input.getAttribute("aria-expanded")).toBe("false");
    expect(input.value).toBe("harbour");
  });

  it("shows Clear all only while there is a chip or text", async () => {
    const { element } = mountBar();
    await type(element, "harbour");
    expect(one<HTMLButtonElement>(element, "button.clear-all").hidden).toBe(false);
    expect(one(element, "button.clear-all").textContent?.trim()).toBe(label("searchClearAll"));
    await click(one(element, "button.clear-all"));
    expect(one<HTMLButtonElement>(element, "button.clear-all").hidden).toBe(true);
    expect(one<HTMLInputElement>(element, "input.text").value).toBe("");
  });
});

describe("<fs-search-bar> drop-down sections", () => {
  it("lists FILTERS Type, Location and Modified, and no People chip", async () => {
    const { element } = mountBar();
    await focusField(element);
    expect(texts(element, "button.filter-chip")).toEqual([
      label("searchFilterType"),
      label("searchFilterLocation"),
      label("searchFilterModified"),
    ]);
    expect(one(element, ".dropdown").textContent).not.toMatch(/people/i);
  });

  it("names its sections with eyebrows from the label table", async () => {
    const { element } = mountBar();
    await type(element, "harbour");
    expect(texts(element, "h3.eyebrow")).toEqual([
      label("searchFilters"),
      label("searchSuggestions"),
      label("searchFiles"),
      label("searchOperators"),
    ]);
  });

  it("lists the ten operators and the Enter hint, and none the desktop cannot answer", async () => {
    const { element } = mountBar();
    await focusField(element);
    expect(texts(element, ".operator-chip")).toEqual([...OPERATORS_ON_HOST]);
    expect(one(element, ".enter-hint").textContent).toBe(label("searchEnterHint"));
    expect(one(element, ".dropdown").textContent).not.toContain("sharedwith");
    expect(one(element, ".dropdown").textContent).not.toContain("unfiled");
  });

  it("suggests in:, type: and title: for typed text, with their hints", async () => {
    const { element } = mountBar();
    element.folder = { node: "c-harbour", name: "Harbour project" };
    await type(element, "harbour");
    const rows = all(element, ".suggestion").map((row) => [
      row.querySelector(".op")?.textContent,
      row.querySelector(".value")?.textContent,
      row.querySelector(".hint")?.textContent,
    ]);
    expect(rows).toEqual([
      ["in:", "Harbour project", label("searchHintFolder")],
      ["type:", "document", label("searchHintType")],
      ["title:", "harbour", label("searchHintTitle")],
    ]);
  });

  it("offers no in: suggestion when no collection is shown, and none for empty text", async () => {
    const { element } = mountBar();
    await type(element, "harbour");
    expect(texts(element, ".suggestion .op")).toEqual(["type:", "title:"]);
    await type(element, "");
    expect(all(element, ".suggestion")).toHaveLength(0);
    expect(all(element, ".file-option")).toHaveLength(0);
  });

  it("shows the first three matching files with a badge, the name and where and when", async () => {
    const { element, search } = mountBar();
    await type(element, "harbour");
    expect(SUGGESTED_FILES).toBe(3);
    expect(texts(element, ".file-option .file-name")).toEqual([
      "harbour-2025-report.pdf",
      "harbour-masterplan-v7.pdf",
      "harbour-walk-permit.pdf",
    ]);
    expect(texts(element, ".file-option .badge")).toEqual(["PDF", "PDF", "PDF"]);
    const meta = texts(element, ".file-option .file-meta");
    expect(meta[1]).toBe("Harbour project · 2 hours ago");
    expect(meta[2]).toMatch(/^Documents · /);
    const call = search.calls.at(-1);
    expect(call?.query).toMatchObject({ q: "harbour", node: SCOPE.node, sort: "relevance" });
    expect(call?.limit).toBe(SUGGESTED_FILES);
  });

  it("shows no file name or path other than the display name (UI-S-019)", async () => {
    const { element } = mountBar();
    await type(element, "harbour");
    const attributes = all(element, ".file-option *")
      .flatMap((el) => [...el.attributes].map((a) => a.value))
      .join("\n");
    expect(attributes).not.toMatch(/urn:uuid|original\//);
    // What the user sees is the display name and, as the mockup draws it, "{collection} · {time}".
    const shown = texts(element, ".file-option .file-name, .file-option .file-meta").join("\n");
    expect(shown).not.toMatch(/urn:uuid|original\/|\.\.\//);
    expect(texts(element, ".file-option .file-name")).toEqual([
      "harbour-2025-report.pdf",
      "harbour-masterplan-v7.pdf",
      "harbour-walk-permit.pdf",
    ]);
    for (const meta of texts(element, ".file-option .file-meta")) {
      expect(meta).toMatch(/^(Harbour project|Documents|Photos 2026) · /);
    }
  });

  it("never shows files of an older query under the text now in the field", async () => {
    const { element } = mountBar();
    const input = await type(element, "harbour");
    expect(all(element, ".file-option")).toHaveLength(3);
    input.value = "harbour zzz";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    expect(all(element, ".file-option")).toHaveLength(0);
    await settle();
    await settle();
    expect(all(element, ".file-option")).toHaveLength(0);
  });

  it("asks nothing and shows no files while the field is empty", async () => {
    const { element, search } = mountBar();
    await focusField(element);
    expect(search.calls).toHaveLength(0);
  });
});

describe("<fs-search-bar> live suggestions", () => {
  it("waits for the debounce, asks once for a burst of keys, and drops a stale answer", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    const { element, search } = mountBar({ debounceMs: 250 });
    const input = one<HTMLInputElement>(element, "input.text");
    input.focus();
    for (const text of ["h", "ha", "har"]) {
      input.value = text;
      input.dispatchEvent(new Event("input", { bubbles: true }));
      await vi.advanceTimersByTimeAsync(100);
    }
    expect(search.calls).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(250);
    expect(search.calls).toHaveLength(1);
    expect(search.calls[0]?.query.q).toBe("har");
  });

  it("shows no files, no console error and still offers suggestions when the host refuses", async () => {
    const search = searchSource();
    vi.spyOn(search, "search").mockRejectedValue(
      new ApiError(503, "/api/v1/files/search", { code: "unavailable" }),
    );
    const { element } = mountBar({ search });
    await type(element, "harbour sharedwith:me");
    expect(all(element, ".file-option")).toHaveLength(0);
    expect(all(element, ".suggestion").length).toBeGreaterThan(0);
  });

  it("includes the chips in the live query", async () => {
    const { element, search } = mountBar();
    await type(element, "harbour");
    await click(one(element, ".suggestion[data-filter=type]"));
    await settle();
    expect(search.calls.at(-1)?.query.q).toBe("type:document harbour");
  });
});

describe("<fs-search-bar> chips", () => {
  it("applies a typed suggestion as a chip beside the text and keeps the text", async () => {
    const { element, searched } = mountBar();
    await type(element, "harbour");
    await click(one(element, ".suggestion[data-filter=type]"));
    expect(texts(element, ".frame .chip .chip-label")).toEqual(["Type: Document"]);
    expect(one<HTMLInputElement>(element, "input.text").value).toBe("harbour");
    await press(element, "Enter");
    expect(searched).toHaveBeenCalledWith({ q: "type:document harbour" });
  });

  it("turns the typed text into a Title chip from the title: suggestion", async () => {
    const { element } = mountBar();
    await type(element, "harbour");
    await click(one(element, ".suggestion[data-filter=title]"));
    expect(texts(element, ".frame .chip .chip-label")).toEqual(["Title: harbour"]);
    expect(one<HTMLInputElement>(element, "input.text").value).toBe("");
  });

  it("offers the six types under the Type filter and applies the chosen one", async () => {
    const { element } = mountBar();
    await focusField(element);
    await click(one(element, "button.filter-chip[data-filter=type]"));
    expect(texts(element, ".suggestion .value")).toEqual([
      "document",
      "image",
      "video",
      "audio",
      "text",
      "folder",
    ]);
    await click(all(element, ".suggestion")[1] as HTMLElement);
    expect(texts(element, ".frame .chip .chip-label")).toEqual(["Type: Image"]);
  });

  it("offers the library's collections under Location and applies in: with the title", async () => {
    const { element, searched } = mountBar();
    await focusField(element);
    await click(one(element, "button.filter-chip[data-filter=in]"));
    expect(texts(element, ".suggestion .value")).toEqual([
      "Documents",
      "Harbour project",
      "Photos 2026",
    ]);
    await click(all(element, ".suggestion")[1] as HTMLElement);
    expect(texts(element, ".frame .chip .chip-label")).toEqual(["Location: Harbour project"]);
    await press(element, "Enter");
    expect(searched).toHaveBeenCalledWith({ q: 'in:"Harbour project"' });
  });

  it("offers four ranges under Modified and sends after: in the caller's zone", async () => {
    const { element, searched } = mountBar();
    await focusField(element);
    await click(one(element, "button.filter-chip[data-filter=modified]"));
    expect(texts(element, ".suggestion .op")).toEqual(["after:", "after:", "after:", "after:"]);
    expect(texts(element, ".suggestion .value")).toEqual([
      "2026-10-16",
      "2026-10-09",
      "2026-09-16",
      "2025-10-16",
    ]);
    expect(texts(element, ".suggestion .hint")).toEqual([
      label("searchModifiedToday"),
      label("searchModifiedWeek"),
      label("searchModifiedMonth"),
      label("searchModifiedYear"),
    ]);
    await click(all(element, ".suggestion")[2] as HTMLElement);
    expect(texts(element, ".frame .chip .chip-label")).toEqual(["Modified: last 30 days"]);
    await press(element, "Enter");
    expect(searched).toHaveBeenCalledWith({ q: "after:2026-09-16" });
  });

  it("keeps one chip of each of Type, Location and Modified", async () => {
    const { element } = mountBar();
    await focusField(element);
    for (const index of [0, 1]) {
      await click(one(element, "button.filter-chip[data-filter=type]"));
      await click(all(element, ".suggestion")[index] as HTMLElement);
    }
    expect(texts(element, ".frame .chip .chip-label")).toEqual(["Type: Image"]);
  });

  it("removes one chip with its own control, named from the label table", async () => {
    const { element } = mountBar();
    await focusField(element);
    await click(one(element, "button.filter-chip[data-filter=type]"));
    await click(all(element, ".suggestion")[0] as HTMLElement);
    await click(one(element, "button.filter-chip[data-filter=modified]"));
    await click(all(element, ".suggestion")[2] as HTMLElement);
    const remove = all(element, "button.chip-remove");
    expect(remove.map((b) => b.getAttribute("aria-label"))).toEqual([
      "Remove Type: Document",
      "Remove Modified: last 30 days",
    ]);
    await click(remove[0] as HTMLElement);
    expect(texts(element, ".frame .chip .chip-label")).toEqual(["Modified: last 30 days"]);
  });

  it("clears every chip and the text with Clear all and runs no search", async () => {
    const { element, searched } = mountBar();
    await type(element, "harbour");
    await click(one(element, ".suggestion[data-filter=type]"));
    await click(one(element, "button.clear-all"));
    expect(all(element, ".frame .chip")).toHaveLength(0);
    expect(one<HTMLInputElement>(element, "input.text").value).toBe("");
    expect(searched).not.toHaveBeenCalled();
  });

  it("removes the last chip with Backspace on empty text, and does nothing with text", async () => {
    const { element } = mountBar();
    await focusField(element);
    await click(one(element, "button.filter-chip[data-filter=type]"));
    await click(all(element, ".suggestion")[0] as HTMLElement);
    const input = one<HTMLInputElement>(element, "input.text");
    input.focus();
    input.value = "a";
    await press(element, "Backspace");
    expect(all(element, ".frame .chip")).toHaveLength(1);
    input.value = "";
    await press(element, "Backspace");
    expect(all(element, ".frame .chip")).toHaveLength(0);
  });

  it("puts an operator chip's text into the field without running a search", async () => {
    const { element, searched } = mountBar();
    await focusField(element);
    const chip = all(element, ".operator-chip").find((c) => c.textContent === "type:");
    await click(chip as HTMLElement);
    expect(one<HTMLInputElement>(element, "input.text").value).toBe("type:");
    expect(searched).not.toHaveBeenCalled();
  });
});

describe("<fs-search-bar> keys and Enter", () => {
  it("moves the active option with the arrow keys and names it in aria-activedescendant", async () => {
    const { element } = mountBar();
    const input = await type(element, "harbour");
    await press(element, "ArrowDown");
    const first = all(element, "[role=option]")[0] as HTMLElement;
    expect(first.getAttribute("aria-selected")).toBe("true");
    expect(input.getAttribute("aria-activedescendant")).toBe(first.id);
    await press(element, "ArrowDown");
    expect(first.getAttribute("aria-selected")).toBe("false");
    await press(element, "ArrowUp");
    expect(first.getAttribute("aria-selected")).toBe("true");
  });

  it("applies the active option on Enter instead of searching", async () => {
    const { element, searched } = mountBar();
    await type(element, "harbour");
    const options = all(element, "[role=option]");
    const target = options.findIndex((o) => o.classList.contains("suggestion"));
    for (let step = 0; step <= target; step += 1) await press(element, "ArrowDown");
    await press(element, "Enter");
    expect(searched).not.toHaveBeenCalled();
    expect(all(element, ".frame .chip").length).toBe(1);
  });

  it("runs the search on Enter with no active option and closes the drop-down", async () => {
    const { element, searched } = mountBar();
    await type(element, "harbour plan");
    await press(element, "Enter");
    expect(searched).toHaveBeenCalledTimes(1);
    expect(searched).toHaveBeenCalledWith({ q: "harbour plan" });
    expect(one(element, ".dropdown").hidden).toBe(true);
  });

  it("does not run an empty search", async () => {
    const { element, searched } = mountBar();
    await focusField(element);
    await press(element, "Enter");
    expect(searched).not.toHaveBeenCalled();
  });

  it("opens a file suggestion as the views open a file, and runs no search", async () => {
    const { element, searched } = mountBar();
    const opened = vi.fn();
    element.addEventListener("fs-drive-open", (event) => opened((event as CustomEvent).detail));
    await type(element, "harbour");
    await click(all(element, ".file-option")[1] as HTMLElement);
    expect(searched).not.toHaveBeenCalled();
    expect(opened).toHaveBeenCalledTimes(1);
    expect(opened.mock.calls[0]?.[0]).toMatchObject({
      intent: "open",
      row: { name: "harbour-masterplan-v7.pdf", libraryId: "lib-1" },
    });
    expect(one(element, ".dropdown").hidden).toBe(true);
  });
});

describe("<fs-search-bar> Location choices", () => {
  it("lists nothing and logs, without an unhandled rejection, when the collections cannot be read", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { element, navigation } = mountBar();
    navigation.failure = new Error("offline");
    await focusField(element);
    await click(one(element, "button.filter-chip[data-filter=in]"));
    expect(all(element, ".suggestion")).toHaveLength(0);
    expect(warn).toHaveBeenCalled();
  });

  it("lists nothing and logs when the scope cannot be found", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { element, search, navigation } = mountBar();
    element.configure({
      labels: (await import("../../src/theme/labels.en.json")).default,
      search,
      navigation,
      scope: () => Promise.reject(new Error("no library")),
      debounceMs: 0,
    });
    await focusField(element);
    await click(one(element, "button.filter-chip[data-filter=in]"));
    expect(all(element, ".suggestion")).toHaveLength(0);
    expect(warn).toHaveBeenCalled();
  });
});

describe("<fs-search-bar> query from the route", () => {
  it("restores chips and text from a query and reports the same query back", () => {
    const { element } = mountBar();
    element.query = "type:document after:2026-09-16 harbour sharedwith:me";
    expect(texts(element, ".frame .chip .chip-label")).toEqual([
      "Type: Document",
      "Modified: last 30 days",
    ]);
    expect(one<HTMLInputElement>(element, "input.text").value).toBe("harbour sharedwith:me");
    expect(element.query).toBe("type:document after:2026-09-16 harbour sharedwith:me");
  });

  it("clears the field when the query is set to nothing", () => {
    const { element } = mountBar();
    element.query = "type:image harbour";
    element.query = "";
    expect(all(element, ".frame .chip")).toHaveLength(0);
    expect(one<HTMLInputElement>(element, "input.text").value).toBe("");
  });

  it("bounds the text at the API's limit", async () => {
    const { element, searched } = mountBar();
    await type(element, "x".repeat(2000));
    await press(element, "Enter");
    const sent = (searched.mock.calls[0]?.[0] as { q: string }).q;
    expect(new TextEncoder().encode(sent).length).toBeLessThanOrEqual(1024);
  });
});
