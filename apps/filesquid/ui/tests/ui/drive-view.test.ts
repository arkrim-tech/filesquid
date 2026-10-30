/**
 * UI tests for `<fs-drive-view>` (UI-F2.8, UI-S-006, UI-S-026, F-062): the list and grid of a
 * node's view with display names only (UI-S-019), column-header sorting with reverse, the grid
 * sort menu, row and header checkboxes with the bulk bar and its effective scope, the item and
 * right-click menus, rename with its refusals, remove with Undo, the ZIP caps, the move or copy
 * dialog, the Recent view, the view states and the saved preferences.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { formatModified } from "../../src/files/model";
import { MemoryViewSource } from "../../src/drive/memory-source";
import {
  ROWS,
  STORED_NAME,
  all,
  click,
  deepText,
  mount,
  names,
  one,
  rootOf,
  rowNamed,
  settle,
  viewSource,
} from "../support/drive-fakes";
import { MemoryNavigationSource } from "../../src/navigation/source";
import labels from "../../src/theme/labels.en.json";

let errors: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
  vi.spyOn(console, "warn").mockImplementation(() => undefined);
});
afterEach(() => {
  expect(errors).not.toHaveBeenCalled();
  vi.restoreAllMocks();
  document.body.replaceChildren();
});

/** The row `index` of the fixture, which must exist. */
function at<T>(list: readonly T[], index: number): T {
  const found = list[index];
  if (found === undefined) throw new Error(`no fixture row ${index}`);
  return found;
}

/** The menu item whose label is `text`. */
function item(element: Element, text: string): HTMLElement {
  const found = all(element, "[role=menuitem]").find(
    (el) => el.querySelector(".label")?.textContent === text,
  );
  if (!found) throw new Error(`no menu item ${text}`);
  return found;
}

/** Tick the checkbox of the row named `name`. */
async function tick(element: Element, name: string): Promise<void> {
  await click(rowNamed(element, name).querySelector("input.check") as HTMLElement);
}

describe("<fs-drive-view> list", () => {
  it("is a defined custom element that mounts alone and shows the title", async () => {
    const { element } = await mount();
    expect(customElements.get("fs-drive-view")).toBeDefined();
    expect(element.dataset.state).toBe("ready");
    expect(one(element, "h2.page-title").textContent).toBe("Harbour project");
  });

  it("heads the columns Name, Size, Modified and Version from the label table", async () => {
    const { element } = await mount();
    expect(all(element, "[role=columnheader]").map((h) => h.textContent?.trim())).toEqual([
      "",
      labels.fileColumnName,
      labels.fileColumnSize,
      labels.fileColumnModified,
      labels.fileColumnVersion,
    ]);
  });

  it("lists folders first, then files, with display names only", async () => {
    const { element } = await mount();
    expect(names(element)).toEqual([
      "Contracts",
      "Drawings",
      "budget-2026.xlsx",
      "facade-study.jpg",
      "harbour-masterplan-v7.pdf",
      "survey-points.csv",
    ]);
    expect(deepText(element)).not.toContain(STORED_NAME);
  });

  it("words size, folder counts and dates as the mockup does", async () => {
    const { element } = await mount();
    expect(rowNamed(element, "Contracts").querySelector(".size")?.textContent).toBe("14 files");
    expect(rowNamed(element, "harbour-masterplan-v7.pdf").querySelector(".size")?.textContent).toBe(
      "18.4 MB",
    );
    expect(
      rowNamed(element, "harbour-masterplan-v7.pdf").querySelector(".modified")?.textContent,
    ).toBe("2 hours ago");
    expect(rowNamed(element, "budget-2026.xlsx").querySelector(".modified")?.textContent).toBe(
      labels.fileTimeYesterday,
    );
    expect(rowNamed(element, "survey-points.csv").querySelector(".modified")?.textContent).toBe(
      formatModified("2026-10-13T08:00:00Z", "en"),
    );
  });

  it("draws a file-type badge by class and a version chip, and a folder icon for folders", async () => {
    const { element } = await mount();
    const pdf = rowNamed(element, "harbour-masterplan-v7.pdf");
    expect(pdf.querySelector(".badge")?.textContent).toBe("PDF");
    expect(pdf.querySelector(".badge")?.classList.contains("document")).toBe(true);
    expect(rowNamed(element, "budget-2026.xlsx").querySelector(".badge")?.classList).toContain(
      "data",
    );
    expect(rowNamed(element, "facade-study.jpg").querySelector(".badge")?.classList).toContain(
      "media",
    );
    expect(pdf.querySelector(".mono-chip")?.textContent).toBe("v2");
    expect(
      rowNamed(element, "Contracts").querySelector(".icon-folder, .icon-collection"),
    ).not.toBeNull();
  });

  it("names each row's actions button from the label table", async () => {
    const { element } = await mount();
    const button = rowNamed(element, "budget-2026.xlsx").querySelector("button.actions");
    expect(button?.getAttribute("aria-label")).toBe(
      labels.fileActions.replace("{name}", "budget-2026.xlsx"),
    );
  });

  it("summarises folders, files and loaded size for the footer", async () => {
    const { element } = await mount();
    expect(element.summary).toBe("Folders: 2 · Files: 4 · Loaded size: 25.1 MB");
  });

  it("announces the summary to the shell's footer with a bubbling fs-summary event", async () => {
    const { element } = await mount();
    const seen = vi.fn();
    document.body.addEventListener("fs-summary", (event) => seen((event as CustomEvent).detail));
    await element.reload();
    expect(seen).toHaveBeenLastCalledWith({ text: "Folders: 2 · Files: 4 · Loaded size: 25.1 MB" });
  });
});

describe("<fs-drive-view> sorting", () => {
  it("sorts by a column head and reverses on a second click", async () => {
    const { element, source } = await mount();
    await click(one(element, "button.sort[data-sort=modified]"));
    expect(source.calls.at(-1)?.query).toMatchObject({ sort: "modified", direction: "descending" });
    expect(
      one(element, "[data-sort=modified]")
        .closest("[role=columnheader]")
        ?.getAttribute("aria-sort"),
    ).toBe("descending");
    await click(one(element, "button.sort[data-sort=modified]"));
    expect(source.calls.at(-1)?.query).toMatchObject({ sort: "modified", direction: "ascending" });
  });

  it("starts in the saved sort and saves a changed sort through the preferences", async () => {
    const { element, saved, prefs } = await mount({
      prefs: { sort_key: "size", sort_direction: "descending" },
    });
    expect(names(element).slice(2)[0]).toBe("harbour-masterplan-v7.pdf");
    await click(one(element, "button.sort[data-sort=name]"));
    await prefs.settled();
    expect(saved.writes.at(-1)).toMatchObject({ sort_key: "name", sort_direction: "ascending" });
  });

  it("offers the sort menu in the grid layout and applies a chosen key and direction", async () => {
    const { element, source } = await mount({ prefs: { layout: "grid" } });
    expect(all(element, ".tile")).toHaveLength(6);
    const sort = one(element, "button.sort-button");
    expect(sort.textContent).toContain("Sort: Name");
    await click(sort);
    const menu = one(element, "[role=menu].sort-menu");
    expect(menu.querySelector(".eyebrow")?.textContent).toBe(labels.fileSortBy);
    expect(
      [...menu.querySelectorAll("[role^=menuitem]")].map((item) => item.textContent?.trim()),
    ).toEqual([
      labels.sortName,
      labels.sortSize,
      labels.sortModified,
      labels.sortModifiedByMe,
      labels.sortOpenedByMe,
      labels.sortAscending,
      labels.sortDescending,
      labels.foldersOnTop,
      labels.foldersMixed,
    ]);
    const items = [...menu.querySelectorAll<HTMLElement>("[role^=menuitem]")];
    await click(items.find((i) => i.textContent?.trim() === labels.sortDescending) as HTMLElement);
    expect(source.calls.at(-1)?.query).toMatchObject({ sort: "name", direction: "descending" });
    expect(names(element).slice(0, 2)).toEqual(["Drawings", "Contracts"]);
  });

  it("places folders mixed with files when asked", async () => {
    const { element } = await mount({
      prefs: {
        layout: "grid",
        folder_placement: "mixed",
        sort_key: "size",
        sort_direction: "descending",
      },
    });
    expect(names(element)[0]).toBe("harbour-masterplan-v7.pdf");
  });

  it("switches layout with the segmented control and saves it", async () => {
    const { element, saved, prefs } = await mount();
    await click(one(element, "button[data-layout=grid]"));
    expect(all(element, ".tile")).toHaveLength(6);
    expect(one(element, "button[data-layout=grid]").getAttribute("aria-pressed")).toBe("true");
    await prefs.settled();
    expect(saved.writes.at(-1)?.layout).toBe("grid");
  });
});

describe("<fs-drive-view> selection", () => {
  it("opens the bulk bar with the count, the scope and the actions", async () => {
    const { element } = await mount();
    expect(all(element, ".bulk")).toHaveLength(0);
    await tick(element, "Contracts");
    await tick(element, "budget-2026.xlsx");
    expect(one(element, ".bulk-count").textContent).toBe("2 selected");
    expect(one(element, ".bulk-scope").textContent).toBe(
      "1 file and 1 folder (14 files inside) · 412 KB",
    );
    expect(all(element, ".bulk button").map((b) => b.textContent?.trim())).toEqual([
      labels.fileDownloadZip,
      labels.fileMoveOrCopy,
      labels.fileRemove,
    ]);
  });

  it("marks the header checkbox partial, then selects every row", async () => {
    const { element } = await mount();
    await tick(element, "budget-2026.xlsx");
    const header = one<HTMLInputElement>(element, "input.check-all");
    expect(header.indeterminate).toBe(true);
    await click(header);
    expect(all<HTMLInputElement>(element, "input.check").every((box) => box.checked)).toBe(true);
    expect(one(element, ".bulk-count").textContent).toBe("6 selected");
    await click(header);
    expect(all(element, ".bulk")).toHaveLength(0);
  });

  it("names every checkbox from the label table", async () => {
    const { element } = await mount();
    expect(
      rowNamed(element, "budget-2026.xlsx")
        .querySelector("input.check")
        ?.getAttribute("aria-label"),
    ).toBe(labels.fileSelectRow.replace("{name}", "budget-2026.xlsx"));
    expect(one(element, "input.check-all").getAttribute("aria-label")).toBe(labels.fileSelectAll);
  });

  it("clears the selection when the view changes", async () => {
    const { element } = await mount();
    await tick(element, "budget-2026.xlsx");
    await click(one(element, "button.sort[data-sort=size]"));
    expect(all(element, ".bulk")).toHaveLength(1);
    element.filter = "survey";
    await settle();
    expect(all(element, ".bulk")).toHaveLength(0);
  });
});

describe("<fs-drive-view> item menu", () => {
  it("opens from the actions button with Open, Details, Rename, Move or copy, Download, Remove", async () => {
    const { element } = await mount();
    await click(
      rowNamed(element, "budget-2026.xlsx").querySelector("button.actions") as HTMLElement,
    );
    const menu = one(element, "[role=menu].item-menu");
    expect([...menu.querySelectorAll("[role=menuitem] .label")].map((i) => i.textContent)).toEqual([
      labels.fileOpen,
      labels.fileDetails,
      labels.fileRename,
      labels.fileMoveOrCopyItem,
      labels.fileDownload,
      labels.fileRemove,
    ]);
    expect(menu.querySelector(".remove")).not.toBeNull();
  });

  it("offers Open on a file only when the host names its library, and says why otherwise", async () => {
    const rows = ROWS.map((row) => ({ ...row, libraryId: undefined }));
    const source = new MemoryViewSource(new Map([["node-library", { all: rows, deleted: [] }]]));
    const { element } = await mount({ source });
    const opened = vi.fn();
    element.addEventListener("fs-drive-open", opened);
    await click(
      rowNamed(element, "budget-2026.xlsx").querySelector("button.actions") as HTMLElement,
    );
    const open = item(element, labels.fileOpen);
    expect(open.getAttribute("aria-disabled")).toBe("true");
    expect(open.getAttribute("title")).toBe(labels.fileActionUnavailable);
    await click(rowNamed(element, "budget-2026.xlsx").querySelector(".name") as HTMLElement);
    expect(opened).not.toHaveBeenCalled();
    expect(one(element, ".toast[role=alert]").textContent).toBe(labels.fileActionUnavailable);
  });

  it("opens the same menu on a right click", async () => {
    const { element } = await mount();
    const row = rowNamed(element, "budget-2026.xlsx");
    row.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
    await settle();
    expect(all(element, "[role=menu].item-menu [role=menuitem]")).toHaveLength(6);
  });

  it("closes on Escape and returns focus to the control that opened it", async () => {
    const { element } = await mount();
    const button = rowNamed(element, "budget-2026.xlsx").querySelector(
      "button.actions",
    ) as HTMLElement;
    await click(button);
    one(element, "[role=menu]").dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
    );
    await settle();
    expect(all(element, "[role=menu]")).toHaveLength(0);
    expect(rootOf(element).activeElement).toBe(button);
  });
});

describe("<fs-drive-view> rename and remove", () => {
  /** Open the rename dialog of the row named `name`. */
  async function openRename(element: Element, name: string): Promise<void> {
    await click(rowNamed(element, name).querySelector("button.actions") as HTMLElement);
    await click(item(element, labels.fileRename));
  }

  it("renames through the renames route with the file's head revision", async () => {
    const { element, files } = await mount();
    await openRename(element, "budget-2026.xlsx");
    const dialog = one(element, "[role=dialog]");
    expect(dialog.querySelector(".dialog-title")?.textContent).toBe("Rename budget-2026.xlsx");
    const input = dialog.querySelector("input") as HTMLInputElement;
    expect(rootOf(element).activeElement).toBe(input);
    input.value = "budget-2026-final.xlsx";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await click(dialog.querySelector("button[type=submit]") as HTMLElement);
    expect(files.rename).toHaveBeenCalledWith(
      { library_id: "lib-1" },
      "f-xlsx",
      "budget-2026-final.xlsx",
      expect.objectContaining({ revision: "v2" }),
    );
    expect(all(element, "[role=dialog]")).toHaveLength(0);
  });

  it("refuses a name over 1,024 bytes before sending and keeps Save disabled", async () => {
    const { element, files } = await mount();
    await openRename(element, "budget-2026.xlsx");
    const dialog = one(element, "[role=dialog]");
    const input = dialog.querySelector("input") as HTMLInputElement;
    input.value = "x".repeat(1025);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    expect(dialog.querySelector(".form-error")?.textContent).toBe(labels.fileDisplayNameTooLong);
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(dialog.querySelector("button[type=submit]")?.hasAttribute("disabled")).toBe(true);
    expect(files.rename).not.toHaveBeenCalled();
  });

  it("counts the name's bytes, not its characters: 513 two-byte characters are 1,026 bytes", async () => {
    const { element, files } = await mount();
    await openRename(element, "budget-2026.xlsx");
    const dialog = one(element, "[role=dialog]");
    const input = dialog.querySelector("input") as HTMLInputElement;
    input.value = "é".repeat(513);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    expect(dialog.querySelector(".form-error")?.textContent).toBe(labels.fileDisplayNameTooLong);
    expect(files.rename).not.toHaveBeenCalled();
    input.value = "é".repeat(512);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    expect(dialog.querySelector(".form-error")?.textContent).toBe("");
  });

  it("shows a refusal under the field and keeps the form open with the typed name", async () => {
    const { element, files } = await mount();
    (files.rename as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new ApiError(409, "/api/v1/files/renames", { code: "conflict" }),
    );
    await openRename(element, "budget-2026.xlsx");
    const dialog = one(element, "[role=dialog]");
    const input = dialog.querySelector("input") as HTMLInputElement;
    input.value = "taken.xlsx";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await click(dialog.querySelector("button[type=submit]") as HTMLElement);
    expect(dialog.querySelector(".form-error")?.textContent).toBe(labels.fileRenameConflict);
    expect(dialog.querySelector<HTMLInputElement>("input")?.value).toBe("taken.xlsx");
    expect(input.getAttribute("aria-invalid")).toBe("true");
  });

  it("removes a file, shows the toast with the version and undoes it", async () => {
    const { element, files } = await mount();
    await click(
      rowNamed(element, "budget-2026.xlsx").querySelector("button.actions") as HTMLElement,
    );
    await click(item(element, labels.fileRemove));
    expect(files.remove).toHaveBeenCalled();
    const toast = one(element, ".toast");
    expect(toast.textContent).toContain("budget-2026.xlsx removed. Version v3 committed.");
    await click(toast.querySelector("button.undo") as HTMLElement);
    expect(files.restore).toHaveBeenCalledWith(
      expect.objectContaining({ object_id: "urn:uuid:f-xlsx" }),
      "v2",
      [expect.stringContaining("original/")],
      expect.objectContaining({ revision: "v3" }),
    );
    expect(all(element, ".toast")).toHaveLength(0);
  });

  it("removes the selected files in turn and stops at the first failure, keeping the selection", async () => {
    const { element, files } = await mount();
    const remove = files.remove as unknown as ReturnType<typeof vi.fn>;
    const receipt = { library_id: "lib-1", object_id: "x", version: "v3", created: "t" };
    remove.mockReset();
    remove.mockResolvedValueOnce(receipt).mockRejectedValueOnce(new Error("refused"));
    await tick(element, "budget-2026.xlsx");
    await tick(element, "facade-study.jpg");
    await tick(element, "survey-points.csv");
    await click(one(element, ".bulk-remove"));
    expect(remove).toHaveBeenCalledTimes(2);
    expect(remove.mock.calls[0]?.[0]).toMatchObject({ object_id: "urn:uuid:f-xlsx" });
    expect(remove.mock.calls[1]?.[0]).toMatchObject({ object_id: "urn:uuid:f-jpg" });
    expect(one(element, ".toast[role=alert]").textContent).toBe(labels.fileActionFailed);
    expect(one(element, ".bulk-count").textContent).toBe("3 selected");
  });

  it("clears the selection once every selected file is removed", async () => {
    const { element, files } = await mount();
    await tick(element, "budget-2026.xlsx");
    await tick(element, "facade-study.jpg");
    await click(one(element, ".bulk-remove"));
    expect(files.remove).toHaveBeenCalledTimes(2);
    expect(all(element, ".bulk-count")).toHaveLength(0);
  });

  it("says why an action is not available when the host does not name the library", async () => {
    const rows = ROWS.map((row) => ({ ...row, libraryId: undefined }));
    const source = new MemoryViewSource(new Map([["node-library", { all: rows, deleted: [] }]]));
    const { element } = await mount({ source });
    await click(
      rowNamed(element, "budget-2026.xlsx").querySelector("button.actions") as HTMLElement,
    );
    const rename = item(element, labels.fileRename);
    expect(rename.getAttribute("aria-disabled")).toBe("true");
    expect(rename.getAttribute("title")).toBe(labels.fileActionUnavailable);
  });
});

describe("<fs-drive-view> order", () => {
  it("keeps the nodes in item order after the sort is reversed", async () => {
    const { element, prefs } = await mount();
    prefs.update({ sort_direction: "descending" });
    await settle();
    await settle();
    expect(names(element)).toEqual([
      "Drawings",
      "Contracts",
      "survey-points.csv",
      "harbour-masterplan-v7.pdf",
      "facade-study.jpg",
      "budget-2026.xlsx",
    ]);
    const indexes = all(element, ".row").map((row) => Number(row.dataset.index));
    expect(indexes).toEqual([...indexes].sort((a, b) => a - b));
  });
});

describe("<fs-drive-view> reads", () => {
  /** A source whose first page has a next page that never finishes until it is aborted. */
  function pagedSource() {
    return {
      view: vi.fn(
        async (
          _query: unknown,
          cursor?: string | null,
          _limit?: number,
          call?: { signal?: AbortSignal },
        ) => {
          if (!cursor) return { rows: [...ROWS], nextCursor: "page-2", complete: true };
          return new Promise<never>((_, reject) =>
            call?.signal?.addEventListener("abort", () =>
              reject(new DOMException("aborted", "AbortError")),
            ),
          );
        },
      ),
    };
  }

  it("keeps the rows and shows no error when the next page is aborted by leaving the page", async () => {
    const source = pagedSource();
    const { element } = await mount({ source: source as never });
    // The list asks for the next page by itself because the first page is short.
    expect(source.view).toHaveBeenCalledTimes(2);
    element.remove();
    await settle();
    expect(element.dataset.state).toBe("ready");
    expect(names(element)).toHaveLength(6);
  });

  it("limits the sidebar filter to the 256 bytes the API takes, never splitting a character", async () => {
    const source = viewSource();
    const { element } = await mount({ source });
    element.filter = "é".repeat(200);
    await settle();
    const sent = new TextEncoder().encode(source.calls.at(-1)?.query.filter ?? "").length;
    expect(sent).toBe(256);
    expect(source.calls.at(-1)?.query.filter).toBe("é".repeat(128));
  });
});

describe("<fs-drive-view> ZIP and move or copy", () => {
  it("refuses more than 1,000 members, states the cap and offers to narrow the selection", async () => {
    const rows = [{ ...at(ROWS, 0), fileCount: 1001 }, at(ROWS, 2)];
    const { element } = await mount({ rows });
    await tick(element, "Contracts");
    await click(one(element, ".bulk-zip"));
    const dialog = one(element, "[role=dialog]");
    expect(dialog.textContent).toContain(labels.fileZipTooMany);
    expect(dialog.querySelector("button.start")?.hasAttribute("disabled")).toBe(true);
  });

  it("refuses more than 10 GiB with the cap stated", async () => {
    const big = ROWS.map((row) => (row.kind === "file" ? { ...row, size: 3 * 1024 ** 3 } : row));
    const { element } = await mount({ rows: big });
    await click(one(element, "input.check-all"));
    await click(one(element, ".bulk-zip"));
    expect(one(element, "[role=dialog]").textContent).toContain(labels.fileZipTooBig);
  });

  it("says an empty archive is the problem, not a cap", async () => {
    const rows = [{ ...at(ROWS, 0), fileCount: 0 }, at(ROWS, 2)];
    const { element } = await mount({ rows });
    await tick(element, "Contracts");
    await click(one(element, ".bulk-zip"));
    const dialog = one(element, "[role=dialog]");
    expect(dialog.querySelector(".form-error")?.textContent).toBe(labels.fileZipEmpty);
    expect(dialog.textContent).not.toContain(labels.fileZipTooMany);
    expect(dialog.querySelector("button.start")?.hasAttribute("disabled")).toBe(true);
  });

  it("words a one-file archive in the singular", async () => {
    const { element } = await mount();
    await tick(element, "budget-2026.xlsx");
    await click(one(element, ".bulk-zip"));
    expect(one(element, "[role=dialog]").textContent).toContain("1 file · 412 KB · saved to");
  });

  it("describes an allowed ZIP and never shows a save path", async () => {
    const { element } = await mount();
    await tick(element, "budget-2026.xlsx");
    await tick(element, "facade-study.jpg");
    await click(one(element, ".bulk-zip"));
    const dialog = one(element, "[role=dialog]");
    expect(dialog.querySelector(".dialog-title")?.textContent).toBe(labels.fileZipTitle);
    expect(dialog.textContent).toContain("2 files · 6.6 MB · saved to a file you choose");
    expect(dialog.textContent).toContain(labels.fileZipPolicyChip);
    expect(dialog.textContent).toContain(labels.fileZipLimits);
    expect(dialog.textContent).not.toMatch(/\/Users|\/home|C:\\/);
  });

  it("says the ZIP download is not available on a host without it, and changes nothing", async () => {
    const { element } = await mount();
    await tick(element, "budget-2026.xlsx");
    await click(one(element, ".bulk-zip"));
    await click(one(element, "[role=dialog] button.start"));
    expect(one(element, "[role=dialog] .form-error").textContent).toBe(labels.fileZipUnavailable);
  });

  it("refuses honestly, changing nothing, when the host cannot place the files in a collection", async () => {
    const navigation = new MemoryNavigationSource({
      node: "r",
      kind: "repository",
      name: null,
      children: [
        {
          node: "node-library",
          kind: "library",
          name: "Family",
          children: [{ node: "c1", kind: "collection", name: "Photos" }],
        },
      ],
    });
    const { element, files } = await mount({ navigation });
    await tick(element, "budget-2026.xlsx");
    await click(one(element, ".bulk-move"));
    const picker = one(element, "fs-destination-picker");
    await settle();
    const photos = [
      ...(picker.shadowRoot?.querySelectorAll<HTMLElement>("li[data-node]") ?? []),
    ][1];
    await click(photos as HTMLElement);
    await click(one(element, "[role=dialog] button.start"));
    expect(one(element, "[role=dialog] .form-error").textContent).toBe(
      labels.fileMoveCopyUnavailable,
    );
    expect(files.rename).not.toHaveBeenCalled();
    expect(files.remove).not.toHaveBeenCalled();
  });

  it("says the move is not available, and keeps Move here disabled, when there is no destination source", async () => {
    const { element } = await mount();
    await tick(element, "budget-2026.xlsx");
    await click(one(element, ".bulk-move"));
    const dialog = one(element, "[role=dialog]");
    expect(dialog.querySelector(".form-error")?.textContent).toBe(labels.fileMoveCopyUnavailable);
    expect(dialog.querySelector("button.start")?.hasAttribute("disabled")).toBe(true);
  });

  it("opens the move or copy dialog with the mode control and the mode's explanation", async () => {
    const { element } = await mount();
    await tick(element, "budget-2026.xlsx");
    await click(one(element, ".bulk-move"));
    const dialog = one(element, "[role=dialog]");
    expect(dialog.querySelector(".dialog-title")?.textContent).toBe(
      "Move or copy budget-2026.xlsx",
    );
    expect([...dialog.querySelectorAll("[role=radio]")].map((r) => r.textContent?.trim())).toEqual([
      labels.fileMoveMode,
      labels.fileCopyMode,
    ]);
    expect(dialog.textContent).toContain(labels.fileMoveExplain);
    await click([...dialog.querySelectorAll<HTMLElement>("[role=radio]")][1] as HTMLElement);
    expect(dialog.textContent).toContain(labels.fileCopyExplain);
    expect(dialog.querySelector("button.start")?.textContent).toBe(labels.fileCopyHere);
  });
});

describe("<fs-drive-view> Recent", () => {
  it("groups by day, shows the note instead of a sort control and the footer count", async () => {
    const { element } = await mount({ view: "recent" });
    // The fixture serves no recent view: the host answers 400 and the view says so.
    expect(element.dataset.state).toBe("unavailable");
    expect(one(element, "fs-states").shadowRoot?.textContent).toContain(labels.fileViewUnavailable);
  });

  it("groups the rows of a served recent view under TODAY, YESTERDAY and EARLIER THIS WEEK", async () => {
    const recent = ROWS.filter((r) => r.kind === "file").map((row) =>
      row.key === "f-csv" ? { ...row, modifiedByMe: "2026-10-13T08:00:00Z" } : row,
    );
    const source = new MemoryViewSource(
      new Map([
        [
          "node-library",
          { all: ROWS, recent: recent.slice(0, 1).concat(recent.slice(1, 2), recent.slice(3)) },
        ],
      ]),
      "lib-1",
    );
    const { element } = await mount({ source, view: "recent" });
    expect(all(element, "h3.eyebrow").map((h) => h.textContent)).toEqual([
      labels.fileRecentToday,
      labels.fileRecentYesterday,
      labels.fileRecentEarlier,
    ]);
    expect(one(element, ".recent-note").textContent).toBe(labels.fileRecentNote);
    expect(all(element, "button.sort-button, button.sort")).toHaveLength(0);
    expect(
      rowNamed(element, "harbour-masterplan-v7.pdf").querySelector(".activity")?.textContent,
    ).toBe("You opened it · 3 hours ago");
    expect(element.summary).toBe("3 files in the last 7 days");
  });
});

describe("<fs-drive-view> states", () => {
  it("shows placeholder rows and the loading state while the first page loads", async () => {
    const source = new MemoryViewSource(new Map([["node-library", { all: ROWS }]]));
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const slow = source.view.bind(source);
    source.view = async (...args) => {
      await gate;
      return slow(...args);
    };
    const mounted = mount({ source });
    await settle();
    const element = document.querySelector("fs-drive-view") as HTMLElement;
    expect(element.dataset.state).toBe("loading");
    expect(all(element, ".placeholder-row").length).toBeGreaterThan(0);
    release();
    await mounted;
  });

  it("shows the empty state for a view without rows", async () => {
    const { element } = await mount({ rows: [] });
    expect(element.dataset.state).toBe("empty");
  });

  it("shows the error state with Try again, which reads again", async () => {
    const source = new MemoryViewSource(new Map([["node-library", { all: ROWS }]]));
    const read = vi.spyOn(source, "view");
    read.mockRejectedValueOnce(new Error("boom"));
    const { element } = await mount({ source });
    expect(element.dataset.state).toBe("error");
    const states = one(element, "fs-states").shadowRoot as ShadowRoot;
    (states.querySelector("button") as HTMLElement).click();
    await settle();
    expect(element.dataset.state).toBe("ready");
  });

  it("shows a gone node as the empty state", async () => {
    const { element } = await mount({ source: new MemoryViewSource(new Map()) });
    expect(element.dataset.state).toBe("empty");
  });

  it("narrows only the current view with the sidebar filter and sends it to the API", async () => {
    const { element, source } = await mount();
    element.filter = "survey";
    await settle();
    expect(source.calls.at(-1)?.query.filter).toBe("survey");
    expect(names(element)).toEqual(["survey-points.csv"]);
  });
});
