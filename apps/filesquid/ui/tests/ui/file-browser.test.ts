/**
 * UI tests for `<fs-file-browser>` (UI-F2.2): the Drive-style listing in list and grid layouts,
 * display names only (UI-S-019), folder navigation, the preview viewer with panel and renderer
 * slots, the five view states, the virtualised window with stable keys, and the accessibility
 * contracts of UI-S-016.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  accessibilityViolations,
  type FileRef,
  type FsFileBrowser,
  type PreviewRenderer,
} from "../../src/index";
import { MemoryFileSource } from "../../src/files/memory-source";
import {
  LOCATION,
  STORED_NAMES,
  all,
  click,
  control,
  deepText,
  fixture,
  mount,
  names,
  one,
  rootOf,
  settle,
} from "../support/file-browser-fakes";
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

describe("<fs-file-browser> listing", () => {
  it("is a defined custom element that mounts alone, without panels or a distribution", async () => {
    const browser = await mount(fixture());
    expect(customElements.get("fs-file-browser")).toBeDefined();
    expect(browser.dataset.state).toBe("ready");
    expect(one(browser, "aside.panels").hidden).toBe(true);
  });

  it("does not take focus on first load", async () => {
    await mount(fixture());
    expect(document.activeElement).toBe(document.body);
  });

  it("starts in the list layout, with the layout toggle pressed on List", async () => {
    const browser = await mount(fixture());
    expect(one(browser, ".table").getAttribute("role")).toBe("table");
    const buttons = all(browser, "button.layout");
    expect(buttons.map((b) => [b.textContent, b.getAttribute("aria-pressed")])).toEqual([
      [labels.fileViewList, "true"],
      [labels.fileViewGrid, "false"],
    ]);
  });

  it("heads the columns name, size, type, modified and version from the label table", async () => {
    const browser = await mount(fixture());
    expect(all(browser, "[role=columnheader]").map((h) => h.textContent)).toEqual([
      labels.fileColumnName,
      labels.fileColumnSize,
      labels.fileColumnType,
      labels.fileColumnModified,
      labels.fileColumnVersion,
    ]);
  });

  it("lists folders first and shows display names, never stored names, anywhere", async () => {
    const browser = await mount(fixture());
    expect(names(browser)).toEqual([
      "Contracts",
      "Drawings",
      "facade-study.png",
      "readme.txt",
      "model-export.blend",
    ]);
    const everything = deepText(rootOf(browser));
    for (const stored of STORED_NAMES) expect(everything).not.toContain(stored);
  });

  it("shows size, type hint, modified date and version in each file row", async () => {
    const browser = await mount(fixture());
    const row = control(browser, "readme.txt").closest(".row") as HTMLElement;
    expect(row.querySelector(".size")?.textContent).toBe("11 B");
    expect(row.querySelector(".type")?.textContent).toBe("TXT");
    expect(row.querySelector(".modified time")?.getAttribute("datetime")).toBe(
      "2026-10-16T09:00:00.000Z",
    );
    expect(row.querySelector(".modified time")?.textContent).toMatch(/2026/);
    expect(row.querySelector(".version")?.textContent).toBe("v1");
    const folder = control(browser, "Contracts").closest(".row") as HTMLElement;
    expect(folder.dataset.kind).toBe("folder");
    expect(folder.querySelector(".size")?.textContent).toBe("");
  });

  it("summarises folders, files and total size in the footer", async () => {
    const browser = await mount(fixture());
    expect(one(browser, ".table").getAttribute("aria-rowcount")).toBe("6");
    expect(one(browser, ".summary").textContent).toBe("Folders: 2 · Files: 3 · Loaded size: 19 B");
  });

  it("switches to the grid layout and back, keeping display names", async () => {
    const browser = await mount(fixture());
    await click(one(browser, 'button.layout[data-layout="grid"]'));
    expect(one(browser, ".table").getAttribute("role")).toBe("list");
    expect(all(browser, ".tile")).toHaveLength(5);
    expect(names(browser)).toContain("facade-study.png");
    expect(one(browser, 'button.layout[data-layout="grid"]').getAttribute("aria-pressed")).toBe(
      "true",
    );
    await click(one(browser, 'button.layout[data-layout="list"]'));
    expect(one(browser, ".table").getAttribute("role")).toBe("table");
    expect(all(browser, ".row")).toHaveLength(5);
  });

  it("shows an empty folder as empty, not as an error", async () => {
    const browser = await mount(fixture([]));
    expect(browser.dataset.state).toBe("empty");
    expect(one(browser, "fs-states").dataset.state).toBe("empty");
  });
});

describe("<fs-file-browser> navigation", () => {
  it("opens a folder, shows its files, updates the breadcrumb and announces the location", async () => {
    const browser = await mount(fixture());
    const seen: unknown[] = [];
    browser.addEventListener("fs-location-change", (e) => seen.push((e as CustomEvent).detail));
    await click(control(browser, "Contracts"));
    expect(names(browser)).toEqual(["harbour-masterplan-v7.pdf", "tender-response.pdf"]);
    expect(browser.location).toEqual({ ...LOCATION, prefix: "Contracts" });
    expect(seen).toEqual([{ location: { ...LOCATION, prefix: "Contracts" } }]);
    const crumbs = one<HTMLElement & { items: { name: string }[] }>(browser, "fs-breadcrumb");
    expect(crumbs.items.map((i) => i.name)).toEqual([labels.fileRoot, "Contracts"]);
  });

  it("returns to the parent from the breadcrumb", async () => {
    const browser = await mount(fixture(), { location: { ...LOCATION, prefix: "Contracts" } });
    const crumbs = one(browser, "fs-breadcrumb");
    const root = crumbs.shadowRoot?.querySelector("a") as HTMLAnchorElement;
    root.click();
    await settle();
    expect(browser.location.prefix).toBe("");
    expect(names(browser)).toContain("Contracts");
  });

  it("moves focus into the new folder's list after opening a folder that has focus", async () => {
    const browser = await mount(fixture());
    control(browser, "Contracts").focus();
    await click(control(browser, "Contracts"));
    expect(rootOf(browser).activeElement?.classList.contains("name")).toBe(true);
  });

  it("links each folder to its location route and leaves modified clicks to the browser", async () => {
    const browser = await mount(fixture());
    const link = control(browser, "Contracts") as HTMLAnchorElement;
    expect(link.getAttribute("href")).toBe("#/files/lib/obj/Contracts");
    const event = new MouseEvent("click", { bubbles: true, cancelable: true, ctrlKey: true });
    link.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
    expect(browser.location.prefix).toBe("");
  });
});

describe("<fs-file-browser> viewer", () => {
  it("opens a file in the viewer with its display name, a handle preview and an announced file", async () => {
    const source = fixture();
    const browser = await mount(source);
    const opened: (FileRef | null)[] = [];
    browser.addEventListener("fs-file-open", (e) => opened.push((e as CustomEvent).detail.file));
    await click(control(browser, "facade-study.png"));
    await settle();
    expect(one(browser, ".viewer").hidden).toBe(false);
    expect(one(browser, ".listing").hidden).toBe(true);
    expect(one(browser, "h2.viewer-title").textContent).toBe("facade-study.png");
    expect(one(browser, ".position").textContent).toBe("1 of 3");
    expect(opened).toEqual([
      {
        library_id: "lib",
        object_id: "obj",
        version: "v1",
        path: "3f9a0004.png",
        display_name: "facade-study.png",
      },
    ]);
    const preview = one(browser, "fs-file-preview");
    expect(preview.shadowRoot?.querySelector("img")?.getAttribute("src")).toBe(
      source.mints[0]?.url,
    );
    for (const stored of STORED_NAMES) expect(deepText(rootOf(browser))).not.toContain(stored);
  });

  it("closes with Back or Escape and returns focus to the row that opened it", async () => {
    const browser = await mount(fixture());
    await click(control(browser, "readme.txt"));
    await click(one(browser, "button.back"));
    expect(one(browser, ".viewer").hidden).toBe(true);
    expect(rootOf(browser).activeElement?.textContent?.trim()).toBe("readme.txt");
    await click(control(browser, "readme.txt"));
    one(browser, ".viewer").dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
    );
    await settle();
    expect(one(browser, ".viewer").hidden).toBe(true);
    expect(rootOf(browser).activeElement?.textContent?.trim()).toBe("readme.txt");
  });

  it("steps to the previous and next file of the folder and stops at the ends", async () => {
    const browser = await mount(fixture());
    await click(control(browser, "facade-study.png"));
    const prev = one(browser, "button.previous");
    const next = one(browser, "button.next");
    expect(prev.getAttribute("aria-disabled")).toBe("true");
    await click(next);
    expect(one(browser, "h2.viewer-title").textContent).toBe("readme.txt");
    expect(one(browser, ".position").textContent).toBe("2 of 3");
    await click(next);
    expect(one(browser, "h2.viewer-title").textContent).toBe("model-export.blend");
    expect(next.getAttribute("aria-disabled")).toBe("true");
    await click(prev);
    expect(one(browser, "h2.viewer-title").textContent).toBe("readme.txt");
  });

  it("says there is no preview for a format it cannot show", async () => {
    const browser = await mount(fixture());
    await click(control(browser, "model-export.blend"));
    await settle();
    const preview = one(browser, "fs-file-preview");
    expect(preview.dataset.state).toBe("unsupported");
  });

  it("mounts a registered panel in its slot, hands it the file and nothing else, and clears it on close", async () => {
    const browser = await mount(fixture());
    const received: (FileRef | null)[] = [];
    const makePanel = (id: string) => () => {
      const panel = document.createElement("div");
      panel.id = id;
      Object.defineProperty(panel, "fileRef", {
        set: (value: FileRef | null) => received.push(value),
        get: () => null,
      });
      return panel;
    };
    browser.registerPanel("details", makePanel("details-panel"));
    browser.registerPanel("versions", makePanel("versions-panel"));
    expect(one(browser, "aside.panels").hidden).toBe(true);
    await click(control(browser, "readme.txt"));
    expect(one(browser, "aside.panels").hidden).toBe(false);
    expect(all(browser, "aside.panels section").map((s) => s.dataset.slot)).toEqual([
      "details",
      "versions",
    ]);
    expect(rootOf(browser).querySelector("#details-panel")).not.toBeNull();
    const shown = received.filter((r): r is FileRef => r !== null);
    expect(shown).toHaveLength(2);
    expect(shown[0]).toEqual({
      library_id: "lib",
      object_id: "obj",
      version: "v1",
      path: "3f9a0005.txt",
      display_name: "readme.txt",
    });
    await click(one(browser, "button.back"));
    expect(received.slice(-2)).toEqual([null, null]);
    expect(one(browser, "aside.panels").hidden).toBe(true);
  });

  it("replaces a panel registered again under the same slot name", async () => {
    const browser = await mount(fixture());
    browser.registerPanel("details", () => Object.assign(document.createElement("i"), { id: "a" }));
    browser.registerPanel("details", () => Object.assign(document.createElement("i"), { id: "b" }));
    await click(control(browser, "readme.txt"));
    expect(rootOf(browser).querySelector("#a")).toBeNull();
    expect(rootOf(browser).querySelector("#b")).not.toBeNull();
  });

  it("tries a registered preview renderer before the built-ins", async () => {
    const browser = await mount(fixture());
    const renderer: PreviewRenderer = {
      id: "notes",
      accepts: (d) => d.media_type === "text/plain",
      render: (c) =>
        Object.assign(document.createElement("section"), {
          textContent: `custom ${c.file.display_name}`,
        }),
    };
    browser.registerPreviewRenderer(renderer);
    await click(control(browser, "readme.txt"));
    await settle();
    const preview = one(browser, "fs-file-preview");
    expect(preview.shadowRoot?.textContent).toContain("custom readme.txt");
  });

  it("closes the viewer when the opened file is removed", async () => {
    const browser = await mount(fixture());
    await click(control(browser, "readme.txt"));
    await click(one(browser, "button.remove"));
    expect(one(browser, ".viewer").hidden).toBe(true);
    expect(names(browser)).not.toContain("readme.txt");
  });
});

describe("<fs-file-browser> loads and objects", () => {
  it("does not report a load that was aborted by leaving the document as a failure", async () => {
    const source = fixture();
    const real = source.list.bind(source);
    const gate = deferred();
    vi.spyOn(source, "list").mockImplementation(async (location, cursor, limit, options) => {
      await gate.promise;
      if (options?.signal?.aborted) throw new DOMException("aborted", "AbortError");
      return real(location, cursor, limit, options);
    });
    const browser = await mount(source);
    browser.remove();
    gate.resolve();
    await settle();
    expect(browser.dataset.state).not.toBe("error");
  });

  it("forgets folders created in one object when another object is opened", async () => {
    const browser = await mount(fixture());
    await click(one(browser, "button.new-folder"));
    one<HTMLInputElement>(browser, "form.name-form input[name=name]").value = "Photos";
    one<HTMLFormElement>(browser, "form.name-form").dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await settle();
    await settle();
    expect(names(browser)).toContain("Photos");
    browser.location = { library_id: "lib", object_id: "obj-two", prefix: "" };
    await settle();
    expect(names(browser)).not.toContain("Photos");
  });
});

describe("<fs-file-browser> states", () => {
  it("shows loading first, then the listing", async () => {
    const source = fixture();
    const gate = deferred();
    const real = source.list.bind(source);
    vi.spyOn(source, "list").mockImplementation(async (...args) => {
      await gate.promise;
      return real(...args);
    });
    const browser = await mount(source);
    expect(browser.dataset.state).toBe("loading");
    expect(one(browser, "fs-states").dataset.state).toBe("loading");
    gate.resolve();
    await settle();
    expect(browser.dataset.state).toBe("ready");
  });

  it("shows an error with Try again and loads again on retry", async () => {
    const source = fixture();
    source.failure = new Error("down");
    const browser = await mount(source);
    expect(browser.dataset.state).toBe("error");
    source.failure = undefined;
    one(browser, "fs-states").dispatchEvent(
      new CustomEvent("fs-retry", { bubbles: true, composed: true }),
    );
    await settle();
    expect(browser.dataset.state).toBe("ready");
    expect(names(browser)).toContain("Contracts");
  });

  it("tells offline from an error and permission denied from both", async () => {
    const offline = fixture();
    offline.failure = new Error("x");
    vi.spyOn(navigator, "onLine", "get").mockReturnValue(false);
    expect((await mount(offline)).dataset.state).toBe("offline");
    const denied = fixture();
    denied.failure = new ApiError(403, "/api/v1/files/listing", {
      code: "forbidden",
      reason: "not_permitted",
    });
    expect((await mount(denied)).dataset.state).toBe("denied");
  });
});

describe("<fs-file-browser> virtual window", () => {
  /** 1,000 files in one folder. */
  function many(): MemoryFileSource {
    const files = Array.from({ length: 1000 }, (_, i) => ({
      path: `f${String(i).padStart(4, "0")}.txt`,
      displayName: `Document ${String(i).padStart(4, "0")}.txt`,
      content: "x",
    }));
    return fixture(files);
  }

  /** Give the scroller a height and scroll it. */
  async function scrollTo(browser: FsFileBrowser, top: number): Promise<void> {
    const scroller = one(browser, ".scroller");
    Object.defineProperty(scroller, "clientHeight", { value: 480, configurable: true });
    scroller.scrollTop = top;
    scroller.dispatchEvent(new Event("scroll"));
    await settle();
  }

  it("keeps only the rows near the viewport in the DOM and describes the whole list", async () => {
    const browser = await mount(many());
    expect(all(browser, ".row").length).toBeLessThanOrEqual(40);
    expect(one(browser, ".table").getAttribute("aria-rowcount")).toBe("-1");
    expect(all(browser, ".row")[0]?.getAttribute("aria-rowindex")).toBe("2");
  });

  it("loads the next page of 200 when the window nears the loaded end, and renders the right rows", async () => {
    const source = many();
    const list = vi.spyOn(source, "list");
    const browser = await mount(source);
    expect(list).toHaveBeenCalledTimes(1);
    await scrollTo(browser, 48 * 190);
    expect(list).toHaveBeenCalledTimes(2);
    expect(list.mock.calls[1]?.[1]).not.toBeNull();
    await scrollTo(browser, 48 * 500);
    expect(names(browser)).toContain("Document 0500.txt");
    expect(all(browser, ".row").length).toBeLessThanOrEqual(40);
  });

  it("reuses the DOM node of a row that stays in the window (stable keys)", async () => {
    const browser = await mount(many());
    await scrollTo(browser, 0);
    const before = control(browser, "Document 0005.txt").closest(".row");
    await scrollTo(browser, 48 * 3);
    const after = control(browser, "Document 0005.txt").closest(".row");
    expect(after).toBe(before);
  });

  it("keeps the focused row in the DOM when it scrolls out of the window", async () => {
    const browser = await mount(many());
    const focused = control(browser, "Document 0001.txt");
    focused.focus();
    await scrollTo(browser, 48 * 500);
    expect(rootOf(browser).activeElement).toBe(focused);
    expect(focused.isConnected).toBe(true);
  });

  it("keeps one Tab stop when the chosen row has scrolled out of the window", async () => {
    const browser = await mount(many());
    control(browser, "Document 0001.txt").focus();
    (rootOf(browser).activeElement as HTMLElement).blur();
    await scrollTo(browser, 48 * 150);
    const stops = all(browser, ".name").filter((el) => el.getAttribute("tabindex") === "0");
    expect(stops).toHaveLength(1);
  });

  it("stops after the page limit and says how many items it shows", async () => {
    const source = many();
    const browser = await mount(source, { pageSize: 100, maxPages: 2 });
    await scrollTo(browser, 48 * 190);
    await scrollTo(browser, 48 * 195);
    expect(names(browser).length).toBeLessThanOrEqual(40);
    expect(one(browser, ".limit").textContent).toBe("Showing the first 200 items only");
  });
});

describe("<fs-file-browser> keyboard and accessibility", () => {
  it("has one Tab stop in the listing and moves it with the arrow, Home and End keys", async () => {
    const browser = await mount(fixture());
    const stops = () => all(browser, ".name").filter((el) => el.getAttribute("tabindex") === "0");
    expect(stops()).toHaveLength(1);
    expect(stops()[0]?.textContent).toBe("Contracts");
    control(browser, "Contracts").focus();
    const press = async (key: string) => {
      (rootOf(browser).activeElement as HTMLElement).dispatchEvent(
        new KeyboardEvent("keydown", { key, bubbles: true }),
      );
      await settle();
    };
    await press("ArrowDown");
    expect(rootOf(browser).activeElement?.textContent).toBe("Drawings");
    expect(stops().map((s) => s.textContent)).toEqual(["Drawings"]);
    await press("End");
    expect(rootOf(browser).activeElement?.textContent).toBe("model-export.blend");
    await press("Home");
    expect(rootOf(browser).activeElement?.textContent).toBe("Contracts");
    await press("ArrowUp");
    expect(rootOf(browser).activeElement?.textContent).toBe("Contracts");
  });

  it("uses native controls, no positive tabindex, named table and toolbar, and passes the audit", async () => {
    const browser = await mount(fixture());
    expect(accessibilityViolations(browser)).toEqual([]);
    expect(one(browser, ".table").getAttribute("aria-label")).toBe(labels.files);
    expect(one(browser, "[role=toolbar]").getAttribute("aria-label")).toBe(labels.fileToolbar);
    for (const el of all(browser, "[tabindex]")) {
      expect(Number(el.getAttribute("tabindex"))).toBeLessThanOrEqual(0);
    }
    await click(control(browser, "readme.txt"));
    await settle();
    expect(accessibilityViolations(browser)).toEqual([]);
  });

  it("puts the shared accessibility stylesheet first in its shadow root", async () => {
    const browser = await mount(fixture());
    expect(rootOf(browser).firstElementChild?.tagName).toBe("STYLE");
    expect(rootOf(browser).firstElementChild?.textContent).toContain(":focus-visible");
  });

  it("disables Rename and Remove until a row is chosen, without removing them from the Tab order", async () => {
    const browser = await mount(fixture());
    expect(one(browser, "button.rename").hasAttribute("disabled")).toBe(false);
    expect(one(browser, "button.remove").hasAttribute("disabled")).toBe(false);
    expect(one(browser, "button.rename").getAttribute("aria-disabled")).toBe("true");
    expect(one(browser, "button.remove").getAttribute("aria-disabled")).toBe("true");
    control(browser, "readme.txt").focus();
    expect(one(browser, "button.rename").getAttribute("aria-disabled")).toBe("false");
    expect(one(browser, "button.remove").getAttribute("aria-disabled")).toBe("false");
    expect(one(browser, "button.rename").hasAttribute("disabled")).toBe(false);
    expect(one(browser, "button.remove").hasAttribute("disabled")).toBe(false);
  });
});

/** A promise resolved from outside. */
function deferred() {
  let resolve: () => void = () => undefined;
  const promise = new Promise<void>((r) => (resolve = r));
  return { promise, resolve };
}
