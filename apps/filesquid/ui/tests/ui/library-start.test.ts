/**
 * UI tests for `<fs-library-start>` (UI-1.1, API-S-034, API-S-025, SRS-F-001): create, open and
 * recover each ask the host for a folder (purpose create, open, open), pass the answered
 * location ID on, and go to the library page; a cancelled choice changes nothing; a 503 is the
 * unavailable state with its reason and a Retry while the three choices stay offered; no path is
 * ever rendered.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  LibraryRegistry,
  accessibilityViolations,
  defineElements,
  type FsLibraryStart,
  type FsStates,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { FakeLibrarySource, LIBRARY, REBUILD, apiError } from "../support/library-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

/** Let pending promises and renders finish. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Mount a start page over a fake source; `navigated` collects the hashes it navigates to. */
async function mount(source = new FakeLibrarySource(), online = () => true) {
  const registry = new LibraryRegistry();
  const navigated: string[] = [];
  const page = document.createElement("fs-library-start") as FsLibraryStart;
  page.configure({ labels, source, registry, navigate: (hash) => navigated.push(hash), online });
  document.body.replaceChildren(page);
  await settle();
  return { page, source, registry, navigated };
}

/** The shadow root of the page. */
const rootOf = (page: FsLibraryStart) => page.shadowRoot as ShadowRoot;

/** A flow's button. */
function button(page: FsLibraryStart, flow: string): HTMLButtonElement {
  return rootOf(page).querySelector(`button[data-action="${flow}"]`) as HTMLButtonElement;
}

describe("<fs-library-start>", () => {
  it("is a defined element with the heading, the intro and three flows from the label table", async () => {
    expect(customElements.get("fs-library-start")).toBeDefined();
    const { page } = await mount();
    const root = rootOf(page);
    expect(root.querySelector("h2")?.textContent).toBe(labels.libraries);
    expect(root.querySelector(".intro")?.textContent).toBe(labels.libraryIntro);
    const flows = [...root.querySelectorAll("section[data-flow]")].map((section) => [
      (section as HTMLElement).dataset.flow,
      section.querySelector("h3")?.textContent,
      section.querySelector("p")?.textContent,
      section.querySelector("button")?.textContent,
    ]);
    expect(flows).toEqual([
      ["create", labels.libraryCreateTitle, labels.libraryCreateHelp, labels.libraryCreate],
      ["open", labels.libraryOpenTitle, labels.libraryOpenHelp, labels.libraryOpen],
      ["recover", labels.libraryRecoverTitle, labels.libraryRecoverHelp, labels.libraryRecover],
    ]);
  });

  it("puts the shared accessibility stylesheet first and passes the accessibility audit", async () => {
    const { page } = await mount();
    expect(rootOf(page).firstElementChild?.tagName).toBe("STYLE");
    expect(accessibilityViolations(page)).toEqual([]);
  });

  it("create: asks for a folder with purpose create, passes the location ID on and opens the library page", async () => {
    const { page, source, registry, navigated } = await mount();
    button(page, "create").click();
    await settle();
    expect(source.calls).toEqual(["choose:create", "create:loc-1"]);
    expect(registry.get("lib-1")).toEqual(LIBRARY);
    expect(navigated).toEqual(["#/library/lib-1"]);
  });

  it("open: asks with purpose open and passes the location ID to the open route", async () => {
    const { page, source, navigated } = await mount();
    button(page, "open").click();
    await settle();
    expect(source.calls).toEqual(["choose:open", "open:loc-1"]);
    expect(navigated).toEqual(["#/library/lib-1"]);
  });

  it("recover: opens the chosen copy, rebuilds its catalog from files and keeps the report for the library page", async () => {
    const { page, source, registry, navigated } = await mount();
    button(page, "recover").click();
    await settle();
    expect(source.calls).toEqual(["choose:open", "open:loc-1", "rebuild:lib-1"]);
    expect(registry.lastRebuild("lib-1")).toEqual(REBUILD);
    expect(navigated).toEqual(["#/library/lib-1"]);
  });

  it("answers an already open library again without an error", async () => {
    const source = new FakeLibrarySource();
    source.opened = LIBRARY;
    const { page, navigated } = await mount(source);
    button(page, "open").click();
    await settle();
    button(page, "open").click();
    await settle();
    expect(navigated).toEqual(["#/library/lib-1", "#/library/lib-1"]);
    expect(rootOf(page).querySelector(".alert")?.textContent).toBe("");
  });

  it("cancel: changes nothing, sends no library request and says so", async () => {
    const source = new FakeLibrarySource();
    source.choice = { outcome: "cancelled" };
    const { page, registry, navigated } = await mount(source);
    button(page, "create").click();
    await settle();
    expect(source.calls).toEqual(["choose:create"]);
    expect(navigated).toEqual([]);
    expect(registry.size).toBe(0);
    const status = rootOf(page).querySelector('[role="status"].status');
    expect(status?.textContent).toBe(labels.libraryCancelled);
    expect(button(page, "create").getAttribute("aria-disabled")).not.toBe("true");
  });

  it("503: shows the unavailable state with its reason and a Retry, and keeps all three choices", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.choice = apiError(503, "unavailable");
    const { page, navigated } = await mount(source);
    button(page, "create").click();
    await settle();
    const states = rootOf(page).querySelector("fs-states") as FsStates;
    expect(states.hidden).toBe(false);
    expect(states.state).toEqual({
      kind: "unavailable",
      reason: labels.libraryChooserUnavailable,
    });
    const text = states.shadowRoot?.textContent ?? "";
    expect(text).toContain(labels.stateUnavailable);
    expect(text).toContain(labels.libraryChooserUnavailable);
    expect(text).toContain(labels.stateRetry);
    for (const flow of ["create", "open", "recover"]) {
      expect(button(page, flow).hidden, flow).toBe(false);
      expect(button(page, flow).getAttribute("aria-disabled"), flow).not.toBe("true");
    }
    expect(navigated).toEqual([]);
  });

  it("Retry asks for the folder again and clears the state when it succeeds", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.choice = apiError(503, "unavailable");
    const { page, navigated } = await mount(source);
    button(page, "open").click();
    await settle();
    source.choice = { outcome: "chosen", location_id: "loc-2" };
    const states = rootOf(page).querySelector("fs-states") as FsStates;
    states.dispatchEvent(new CustomEvent("fs-retry", { bubbles: true, composed: true }));
    await settle();
    expect(source.calls).toEqual(["choose:open", "choose:open", "open:loc-2"]);
    expect(states.hidden).toBe(true);
    expect(navigated).toEqual(["#/library/lib-1"]);
  });

  it("Retry after a failed open reuses the chosen folder and asks for none again", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.opened = apiError(503, "unavailable");
    const { page, navigated } = await mount(source);
    button(page, "open").click();
    await settle();
    expect(source.calls).toEqual(["choose:open", "open:loc-1"]);
    source.opened = LIBRARY;
    const states = rootOf(page).querySelector("fs-states") as FsStates;
    states.dispatchEvent(new CustomEvent("fs-retry", { bubbles: true, composed: true }));
    await settle();
    expect(source.calls).toEqual(["choose:open", "open:loc-1", "open:loc-1"]);
    expect(navigated).toEqual(["#/library/lib-1"]);
  });

  it("Retry after a failed rebuild of a recovered copy rebuilds the opened library only", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.rebuilt = apiError(503, "unavailable");
    const { page, registry, navigated } = await mount(source);
    button(page, "recover").click();
    await settle();
    expect(navigated).toEqual([]);
    expect(registry.get("lib-1")).toEqual(LIBRARY);
    source.rebuilt = REBUILD;
    const states = rootOf(page).querySelector("fs-states") as FsStates;
    states.dispatchEvent(new CustomEvent("fs-retry", { bubbles: true, composed: true }));
    await settle();
    expect(source.calls).toEqual(["choose:open", "open:loc-1", "rebuild:lib-1", "rebuild:lib-1"]);
    expect(registry.lastRebuild("lib-1")).toEqual(REBUILD);
    expect(navigated).toEqual(["#/library/lib-1"]);
  });

  it("a choice clicked after a failure starts afresh with a new folder", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.opened = apiError(503, "unavailable");
    const { page, navigated } = await mount(source);
    button(page, "open").click();
    await settle();
    source.opened = LIBRARY;
    source.choice = { outcome: "chosen", location_id: "loc-2" };
    button(page, "open").click();
    await settle();
    expect(source.calls).toEqual(["choose:open", "open:loc-1", "choose:open", "open:loc-2"]);
    expect(navigated).toEqual(["#/library/lib-1"]);
  });

  it("words a refused create (409) as a message, writes nothing and offers the choices again", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.created = apiError(409, "conflict");
    const { page, navigated } = await mount(source);
    button(page, "create").click();
    await settle();
    const alert = rootOf(page).querySelector('[role="alert"].alert');
    expect(alert?.textContent).toBe(labels.libraryConflictCreate);
    expect(navigated).toEqual([]);
    expect(button(page, "create").getAttribute("aria-disabled")).not.toBe("true");
  });

  it("shows a denial with the label of its reason", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.choice = apiError(403, "forbidden", "not_permitted");
    const { page } = await mount(source);
    button(page, "open").click();
    await settle();
    const states = rootOf(page).querySelector("fs-states") as FsStates;
    expect(states.state).toEqual({ kind: "denied", reason: "not_permitted" });
  });

  it("disables the choices while a folder is being chosen so one dialog is open at a time", async () => {
    const source = new FakeLibrarySource();
    let answer: (value: { outcome: "cancelled" }) => void = () => undefined;
    source.choice = () => new Promise((resolve) => (answer = resolve));
    const { page } = await mount(source);
    button(page, "create").click();
    await settle();
    expect(button(page, "open").getAttribute("aria-disabled")).toBe("true");
    expect(rootOf(page).querySelector(".status")?.textContent).toBe(labels.libraryChoosing);
    button(page, "open").click();
    await settle();
    expect(source.calls).toEqual(["choose:create"]);
    answer({ outcome: "cancelled" });
    await settle();
    expect(button(page, "open").getAttribute("aria-disabled")).not.toBe("true");
  });

  it("is offline when the browser is offline and the load failed without an answer", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const source = new FakeLibrarySource();
    source.choice = new TypeError("fetch failed");
    const { page } = await mount(source, () => false);
    button(page, "create").click();
    await settle();
    expect((rootOf(page).querySelector("fs-states") as FsStates).state).toEqual({
      kind: "offline",
    });
  });

  it("does not navigate when the page was removed while the flow was pending", async () => {
    const source = new FakeLibrarySource();
    let answer: (value: { outcome: "chosen"; location_id: string }) => void = () => undefined;
    source.choice = () => new Promise((resolve) => (answer = resolve));
    const { page, navigated } = await mount(source);
    button(page, "create").click();
    await settle();
    page.remove();
    answer({ outcome: "chosen", location_id: "loc-1" });
    await settle();
    expect(source.calls).toEqual(["choose:create"]);
    expect(navigated).toEqual([]);
  });

  it("accepts new clicks after being removed and attached again mid-flow", async () => {
    const source = new FakeLibrarySource();
    source.choice = () => new Promise(() => undefined);
    const { page } = await mount(source);
    button(page, "create").click();
    await settle();
    page.remove();
    document.body.append(page);
    await settle();
    button(page, "open").click();
    await settle();
    expect(source.calls).toEqual(["choose:create", "choose:open"]);
  });

  it("never renders a filesystem path", async () => {
    const { page } = await mount();
    button(page, "create").click();
    await settle();
    expect(rootOf(page).textContent).not.toMatch(/[A-Za-z]:\\|\/Users\/|\/home\/|\/Volumes\//);
  });
});
