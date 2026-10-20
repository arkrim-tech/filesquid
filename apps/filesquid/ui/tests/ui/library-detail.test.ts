/**
 * UI tests for `<fs-library-detail>` (UI-1.1, API-S-025, API-S-023, SRS-F-001): the library's
 * facts without any path, copy to a chosen destination (purpose copy_destination, then the
 * snapshot route with `destination_location_id`), rebuild from files, the W010 note "can be read
 * but not changed" with its link to the repair offer, and the recover report kept by the
 * recover flow.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  LibraryRegistry,
  accessibilityViolations,
  defineElements,
  formatSize,
  type FsLibraryDetail,
  type FsStates,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import {
  CANDIDATES,
  COPY_REPORT,
  FakeLibrarySource,
  LIBRARY,
  REBUILD,
  apiError,
} from "../support/library-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

/** Let pending promises and renders finish. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Mount a detail page for `lib-1`. */
async function mount(
  source = new FakeLibrarySource(),
  registry = new LibraryRegistry(),
  id = "lib-1",
) {
  const page = document.createElement("fs-library-detail") as FsLibraryDetail;
  page.configure({ labels, source, registry, online: () => true });
  page.libraryId = id;
  document.body.replaceChildren(page);
  await settle();
  return { page, source, registry };
}

const rootOf = (page: FsLibraryDetail) => page.shadowRoot as ShadowRoot;
const action = (page: FsLibraryDetail, name: string) =>
  rootOf(page).querySelector(`button[data-action="${name}"]`) as HTMLButtonElement;
const text = (page: FsLibraryDetail, selector: string) =>
  rootOf(page).querySelector(selector)?.textContent ?? "";

describe("<fs-library-detail>", () => {
  it("shows the library's facts from the label table, and no path", async () => {
    const registry = new LibraryRegistry();
    registry.remember(LIBRARY);
    const { page } = await mount(new FakeLibrarySource(), registry);
    expect(customElements.get("fs-library-detail")).toBeDefined();
    expect(text(page, "h2")).toBe(labels.library);
    const facts = [
      ...rootOf(page).querySelectorAll('[data-section="facts"] dt, [data-section="facts"] dd'),
    ].map((node) => node.textContent);
    expect(facts).toEqual([
      labels.libraryFactId,
      "lib-1",
      labels.libraryFactFormat,
      labels.libraryFactFormatValue.replace("{version}", "1.1"),
      labels.libraryFactObjects,
      "3",
    ]);
    expect(rootOf(page).textContent).not.toMatch(/[A-Za-z]:\\|\/Users\/|\/home\/|\/Volumes\//);
    expect(rootOf(page).firstElementChild?.tagName).toBe("STYLE");
    expect(accessibilityViolations(page)).toEqual([]);
  });

  it("says what it has not read when the library was not opened in this session", async () => {
    const { page } = await mount();
    const facts = [...rootOf(page).querySelectorAll('[data-section="facts"] dd')].map(
      (node) => node.textContent,
    );
    expect(facts).toEqual(["lib-1", labels.libraryFactUnknown, labels.libraryFactUnknown]);
  });

  it("links back to all libraries", async () => {
    const { page } = await mount();
    const link = rootOf(page).querySelector("a.all") as HTMLAnchorElement;
    expect(link.textContent).toBe(labels.libraryStartBack);
    expect(link.getAttribute("href")).toBe("#/library");
  });

  it("says that files can be read but not changed (W010) and links to the repair offer", async () => {
    const { page, source } = await mount();
    expect(source.calls).toEqual(["candidates:lib-1"]);
    const note = rootOf(page).querySelector('[data-section="repair-note"]');
    expect(note?.querySelector("p")?.textContent).toBe(
      labels.libraryReadOnly.replace("{count}", "2"),
    );
    const link = note?.querySelector("a") as HTMLAnchorElement;
    expect(link.textContent).toBe(labels.libraryReviewRepair);
    expect(link.getAttribute("href")).toBe("#/library/lib-1/repair");
  });

  it("words a truncated candidate list as at least", async () => {
    const source = new FakeLibrarySource();
    source.candidates = { ...CANDIDATES, truncated: true };
    const { page } = await mount(source);
    expect(text(page, '[data-section="repair-note"] p')).toBe(
      labels.libraryReadOnlyPartial.replace("{count}", "2"),
    );
  });

  it("shows no repair note when no object needs the repair", async () => {
    const source = new FakeLibrarySource();
    source.candidates = { candidates: [], truncated: false };
    const { page } = await mount(source);
    expect(rootOf(page).querySelector('[data-section="repair-note"]')).toBeNull();
  });

  it("copy: asks for a destination with purpose copy_destination and passes destination_location_id", async () => {
    const { page, source } = await mount();
    action(page, "copy").click();
    await settle();
    expect(source.calls).toEqual([
      "candidates:lib-1",
      "choose:copy_destination",
      "copy:lib-1:loc-1",
    ]);
    const result = rootOf(page).querySelector('[data-result="copy"]');
    expect(result?.getAttribute("role")).toBe("status");
    const lines = [...(result?.querySelectorAll("p, li") ?? [])].map((node) => node.textContent);
    expect(lines).toEqual([
      labels.libraryCopied
        .replace("{objects}", "2")
        .replace("{files}", "5")
        .replace("{size}", formatSize(2048, labels)),
      labels.libraryCopyInvalidTitle.replace("{count}", "1"),
      labels.libraryCopyInvalid
        .replace("{object}", "urn:uuid:obj-c")
        .replace("{codes}", "E023, E040"),
    ]);
  });

  it("copy: says a clean copy validated without errors", async () => {
    const source = new FakeLibrarySource();
    source.copied = { objects: 1, files: 1, bytes: 10, invalid_objects: [] };
    const { page } = await mount(source);
    action(page, "copy").click();
    await settle();
    expect(text(page, '[data-result="copy"]')).toContain(labels.libraryCopyClean);
  });

  it("copy: a cancelled destination changes nothing", async () => {
    const source = new FakeLibrarySource();
    source.choice = { outcome: "cancelled" };
    const { page } = await mount(source);
    action(page, "copy").click();
    await settle();
    expect(source.calls).toEqual(["candidates:lib-1", "choose:copy_destination"]);
    expect(text(page, '[data-for="actions"] .status')).toBe(labels.libraryCancelled);
  });

  it("copy: a 503 from the chooser is the unavailable state with its reason", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.choice = apiError(503, "unavailable");
    const { page } = await mount(source);
    action(page, "copy").click();
    await settle();
    const states = rootOf(page).querySelector('[data-for="actions"] fs-states') as FsStates;
    expect(states.state).toEqual({ kind: "unavailable", reason: labels.libraryChooserUnavailable });
    expect(action(page, "copy").hidden).toBe(false);
  });

  it("copy: a refused destination is worded and writes nothing", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.copied = apiError(409, "conflict");
    const { page } = await mount(source);
    action(page, "copy").click();
    await settle();
    expect(text(page, '[data-for="actions"] .alert')).toBe(labels.libraryConflictCopy);
  });

  it("rebuild: reads the library from its files and reports each count, then reads the candidates again", async () => {
    const { page, source, registry } = await mount();
    action(page, "rebuild").click();
    await settle();
    expect(source.calls).toEqual(["candidates:lib-1", "rebuild:lib-1", "candidates:lib-1"]);
    expect(registry.lastRebuild("lib-1")).toEqual(REBUILD);
    const result = rootOf(page).querySelector('[data-result="rebuild"]');
    const lines = [...(result?.querySelectorAll("p, li") ?? [])].map((node) => node.textContent);
    expect(lines).toEqual([
      labels.libraryRebuilt
        .replace("{objects}", "2")
        .replace("{unreadable}", "1")
        .replace("{invalid}", "1"),
      labels.libraryRebuiltInvalid.replace("{object}", "urn:uuid:obj-c").replace("{codes}", "E023"),
      labels.libraryUnreadable.replace("{code}", "E001"),
    ]);
  });

  it("shows the report the recover flow kept for this library", async () => {
    const registry = new LibraryRegistry();
    registry.remember(LIBRARY);
    registry.rememberRebuild("lib-1", REBUILD);
    const { page } = await mount(new FakeLibrarySource(), registry);
    expect(rootOf(page).querySelector('[data-result="rebuild"]')).not.toBeNull();
  });

  it("shows an error state with Retry when the candidates cannot be read", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.candidates = apiError(404, "not_found");
    const { page } = await mount(source);
    expect(text(page, '[data-for="repair-note"] .alert')).toBe(labels.libraryNotFound);
  });

  it("an unreachable library service is the unavailable state, and Retry reads again", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.candidates = apiError(503, "unavailable");
    const { page } = await mount(source);
    const states = rootOf(page).querySelector('[data-for="repair-note"] fs-states') as FsStates;
    expect(states.state.kind).toBe("unavailable");
    source.candidates = CANDIDATES;
    states.dispatchEvent(new CustomEvent("fs-retry", { bubbles: true, composed: true }));
    await settle();
    expect(rootOf(page).querySelector('[data-section="repair-note"]')).not.toBeNull();
  });

  it("a copy that finishes after the page moved to another library shows nothing there", async () => {
    const source = new FakeLibrarySource();
    let finish: (value: typeof COPY_REPORT) => void = () => undefined;
    source.copied = () => new Promise((resolve) => (finish = resolve));
    const { page } = await mount(source);
    action(page, "copy").click();
    await settle();
    page.libraryId = "lib-2";
    await settle();
    expect(action(page, "copy").getAttribute("aria-disabled")).not.toBe("true");
    expect(source.calls).toContain("copy:lib-1:loc-1");
    finish(COPY_REPORT);
    await settle();
    expect(rootOf(page).querySelector('[data-result="copy"]')?.children.length).toBe(0);
  });

  it("follows a changed library ID", async () => {
    const { page, source } = await mount();
    page.libraryId = "lib-2";
    await settle();
    expect(source.calls.at(-1)).toBe("candidates:lib-2");
    expect(text(page, '[data-section="facts"] dd')).toBe("lib-2");
  });
});
