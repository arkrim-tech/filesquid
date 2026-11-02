/**
 * UI tests for the recent libraries and the first-use trigger of `<fs-library-start>`
 * (UI-1.3, UI-S-027, UI-S-034, API-S-038, mockups 01 and 57): the RECENT LIBRARIES column with
 * each library's name, "Opened {when} · {free} free on its disk" and a ghost Open; a library
 * whose volume cannot be read shows no free figure; an empty list opens the first-use dialog
 * over the welcome screen; a host without a registry (503) shows neither.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  LibraryRegistry,
  accessibilityViolations,
  defineElements,
  type FsLibraryStart,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { FakeLibrarySource, LIBRARY, RECENT, apiError } from "../support/library-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));
const NOW = new Date("2026-10-31T12:00:00Z");

/** Mount the start page with a recording source and a fixed clock. */
async function mount(source = new FakeLibrarySource()) {
  const registry = new LibraryRegistry();
  const navigated: string[] = [];
  const page = document.createElement("fs-library-start") as FsLibraryStart;
  page.configure({
    labels,
    source,
    registry,
    navigate: (hash) => navigated.push(hash),
    online: () => true,
    now: () => NOW,
  });
  document.body.replaceChildren(page);
  await settle();
  return { page, source, registry, navigated };
}

const rootOf = (page: FsLibraryStart) => page.shadowRoot as ShadowRoot;
const rows = (page: FsLibraryStart) => [
  ...rootOf(page).querySelectorAll<HTMLElement>(".recent li"),
];

describe("recent libraries on the welcome screen", () => {
  it("reads the recent libraries once and shows the eyebrow and one row each", async () => {
    const { page, source } = await mount();
    expect(source.calls).toEqual(["recent"]);
    expect(rootOf(page).querySelector(".recent .eyebrow")?.textContent).toBe(
      labels.libraryRecentTitle,
    );
    expect(rows(page)).toHaveLength(2);
  });

  it("words each row: the name, when it was opened and the free space of its disk, and Open", async () => {
    const { page } = await mount();
    const [first, second] = rows(page);
    expect(first?.querySelector(".name")?.textContent).toBe("Family archive");
    expect(first?.querySelector(".meta")?.textContent).toBe(
      "Opened today · 1.3 TB free on its disk",
    );
    expect(second?.querySelector(".meta")?.textContent).toBe(
      "Opened 20 Oct 2026 · free space unknown",
    );
    for (const row of rows(page)) {
      expect(row.querySelector("button")?.textContent).toBe(labels.libraryOpenRecent);
      expect(row.querySelector("button")?.className).toBe("button ghost");
    }
  });

  it("names each Open button after its library for assistive technology", async () => {
    const { page } = await mount();
    const button = rows(page)[0]?.querySelector("button") as HTMLButtonElement;
    expect(button.getAttribute("aria-label")).toBe(`${labels.libraryOpenRecent} Family archive`);
    expect(accessibilityViolations(page)).toEqual([]);
  });

  it("shows no path, location or library id", async () => {
    const { page } = await mount();
    const shown = rootOf(page).querySelector(".recent")?.textContent ?? "";
    expect(shown).not.toContain("lib-1");
    expect(shown).not.toMatch(/\/Users|\/home|[A-Z]:\\/);
  });

  it("opens the page of a library that is open now", async () => {
    const { page, source, navigated } = await mount();
    (rows(page)[0]?.querySelector("button") as HTMLButtonElement).click();
    await settle();
    expect(navigated).toEqual(["#/library/lib-1"]);
    expect(source.calls).toEqual(["recent"]);
  });

  it("opens a remembered library that is not open now through the Open flow, by its folder", async () => {
    const source = new FakeLibrarySource();
    const { page, navigated } = await mount(source);
    (rows(page)[1]?.querySelector("button") as HTMLButtonElement).click();
    await settle();
    expect(source.calls).toEqual(["recent", "choose:open", "open:loc-1"]);
    expect(navigated).toEqual(["#/library/lib-1"]);
  });

  it("shows at most 20 libraries", async () => {
    const source = new FakeLibrarySource();
    const library = RECENT.libraries[0] as (typeof RECENT.libraries)[number];
    source.recentAnswer = {
      libraries: Array.from({ length: 20 }, (_, index) => ({
        ...library,
        library_id: `lib-${index}`,
      })),
    };
    const { page } = await mount(source);
    expect(rows(page)).toHaveLength(20);
  });
});

describe("first use", () => {
  it("opens the first-use dialog over the welcome screen when no library is known", async () => {
    const source = new FakeLibrarySource();
    source.recentAnswer = { libraries: [] };
    const { page } = await mount(source);
    expect(rootOf(page).querySelector("fs-first-use")).not.toBeNull();
    expect(rootOf(page).querySelector(".recent")).toBeNull();
    expect(rootOf(page).querySelectorAll("section[data-flow]")).toHaveLength(3);
  });

  it("opens no dialog when a library is known", async () => {
    const { page } = await mount();
    expect(rootOf(page).querySelector("fs-first-use")).toBeNull();
  });

  it("leaves the three choices and no dialog when the host keeps no registry (503)", async () => {
    const source = new FakeLibrarySource();
    source.recentAnswer = apiError(503, "unavailable");
    const { page } = await mount(source);
    expect(rootOf(page).querySelector("fs-first-use")).toBeNull();
    expect(rootOf(page).querySelector(".recent")).toBeNull();
    expect(rootOf(page).querySelectorAll("section[data-flow]")).toHaveLength(3);
  });

  it("logs another failure and leaves the choices usable", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.recentAnswer = new TypeError("bad answer");
    const { page } = await mount(source);
    expect(warn).toHaveBeenCalled();
    expect(rootOf(page).querySelector("fs-first-use")).toBeNull();
    expect(rootOf(page).querySelectorAll("section[data-flow]")).toHaveLength(3);
  });

  it("reopens the dialog from the Create card after it was cancelled, while no library is known", async () => {
    const source = new FakeLibrarySource();
    source.recentAnswer = { libraries: [] };
    const { page } = await mount(source);
    const dialog = rootOf(page).querySelector("fs-first-use") as HTMLElement;
    (dialog.shadowRoot?.querySelector('button[data-action="cancel"]') as HTMLButtonElement).click();
    expect(rootOf(page).querySelector("fs-first-use")).toBeNull();
    (rootOf(page).querySelector('button[data-action="create"]') as HTMLButtonElement).click();
    await settle();
    expect(rootOf(page).querySelector("fs-first-use")).not.toBeNull();
    expect(source.calls).toEqual(["recent"]);
  });

  it("keeps the immediate folder dialog of Create when a library is known", async () => {
    const { page, source, navigated } = await mount();
    (rootOf(page).querySelector('button[data-action="create"]') as HTMLButtonElement).click();
    await settle();
    expect(source.calls).toEqual(["recent", "choose:create", "create:loc-1"]);
    expect(navigated).toEqual([`#/library/${LIBRARY.library_id}`]);
  });
});
