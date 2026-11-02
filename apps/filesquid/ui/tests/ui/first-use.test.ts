/**
 * UI tests for `<fs-first-use>` (UI-1.3, UI-S-034, mockup 57): the dialog over the welcome
 * screen that asks for the first library's name and folder, creates the library and then its
 * first collection, keeps the library when only the collection fails and offers Try again for
 * that step alone, never shows a path, and announces `fs-first-use-done` as the hook for the
 * import dialog of UI-1.2.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  LibraryRegistry,
  accessibilityViolations,
  defineElements,
  type FsFirstUse,
  type FsStates,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { COLLECTION, FakeLibrarySource, LIBRARY, apiError } from "../support/library-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Mount the dialog with a recording source and navigation. */
async function mount(source = new FakeLibrarySource()) {
  const registry = new LibraryRegistry();
  const navigated: string[] = [];
  const dialog = document.createElement("fs-first-use") as FsFirstUse;
  dialog.configure({
    labels,
    source,
    registry,
    navigate: (hash) => navigated.push(hash),
    online: () => true,
  });
  const done: CustomEvent[] = [];
  const cancelled: Event[] = [];
  dialog.addEventListener("fs-first-use-done", (event) => done.push(event as CustomEvent));
  dialog.addEventListener("fs-first-use-cancel", (event) => cancelled.push(event));
  document.body.replaceChildren(dialog);
  await settle();
  return { dialog, source, registry, navigated, done, cancelled };
}

const rootOf = (dialog: FsFirstUse) => dialog.shadowRoot as ShadowRoot;
const button = (dialog: FsFirstUse, name: string) =>
  rootOf(dialog).querySelector(`button[data-action="${name}"]`) as HTMLButtonElement;
const nameInput = (dialog: FsFirstUse) =>
  rootOf(dialog).querySelector("input.input") as HTMLInputElement;
const text = (dialog: FsFirstUse, selector: string) =>
  rootOf(dialog).querySelector(selector)?.textContent ?? "";
const type = (input: HTMLInputElement, value: string) => {
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
};

describe("<fs-first-use>", () => {
  it("draws the dialog of mockup 57 from the label table", async () => {
    const { dialog } = await mount();
    expect(customElements.get("fs-first-use")).toBeDefined();
    const box = rootOf(dialog).querySelector('[role="dialog"]') as HTMLElement;
    expect(box.getAttribute("aria-modal")).toBe("true");
    const title = text(dialog, ".dialog-title");
    expect(title).toBe(labels.libraryFirstUseTitle);
    expect(box.getAttribute("aria-labelledby")).toBe(
      rootOf(dialog).querySelector(".dialog-title")?.id,
    );
    expect(text(dialog, ".lead")).toContain(labels.libraryFirstUseLead);
    expect(text(dialog, ".lead")).toContain(labels.libraryFirstUseCollectionLead);
    expect(text(dialog, 'label[for="first-use-name"]')).toBe(labels.libraryFirstUseName);
    expect(text(dialog, ".location-label")).toBe(labels.libraryFirstUseLocation);
    expect(text(dialog, ".location-value")).toBe(labels.libraryFolderNone);
    expect(button(dialog, "choose").textContent).toBe(labels.libraryChooseFolder);
    expect(text(dialog, ".rule")).toBe(labels.libraryFolderRule);
    expect(text(dialog, ".collection-note strong")).toBe(labels.libraryFirstCollection);
    expect(text(dialog, ".collection-note span")).toBe(labels.libraryFirstCollectionNote);
    expect(button(dialog, "cancel").textContent).toBe(labels.libraryFirstUseCancel);
    expect(button(dialog, "create").textContent).toBe(labels.libraryCreate);
  });

  it("has one primary button, secondary Choose folder and Cancel, and focuses the name", async () => {
    const { dialog } = await mount();
    expect(button(dialog, "create").className).toBe("button primary large");
    expect(button(dialog, "cancel").className).toBe("button secondary large");
    expect(button(dialog, "choose").className).toBe("button secondary");
    expect(rootOf(dialog).activeElement).toBe(nameInput(dialog));
    expect(accessibilityViolations(dialog)).toEqual([]);
  });

  it("keeps Create disabled until a folder is chosen", async () => {
    const { dialog, source } = await mount();
    expect(button(dialog, "create").getAttribute("aria-disabled")).toBe("true");
    button(dialog, "create").click();
    await settle();
    expect(source.calls).toEqual([]);
  });

  it("chooses the folder with purpose create and shows the choice without any path", async () => {
    const { dialog, source } = await mount();
    button(dialog, "choose").click();
    await settle();
    expect(source.calls).toEqual(["choose:create"]);
    expect(text(dialog, ".location-value")).toBe(labels.libraryFolderChosen);
    expect(button(dialog, "create").hasAttribute("aria-disabled")).toBe(false);
    expect(rootOf(dialog).textContent).not.toContain("loc-1");
  });

  it("says nothing changed when the owner cancels the folder dialog", async () => {
    const source = new FakeLibrarySource();
    source.choice = { outcome: "cancelled" };
    const { dialog } = await mount(source);
    button(dialog, "choose").click();
    await settle();
    expect(text(dialog, ".status")).toBe(labels.libraryCancelled);
    expect(button(dialog, "create").getAttribute("aria-disabled")).toBe("true");
  });

  it("creates the library with its name and then its first collection, then opens the library page", async () => {
    const { dialog, source, registry, navigated, done } = await mount();
    type(nameInput(dialog), "  Family archive ");
    button(dialog, "choose").click();
    await settle();
    button(dialog, "create").click();
    await settle();
    expect(source.calls).toEqual([
      "choose:create",
      "create:loc-1:Family archive",
      `collection:lib-1:${labels.libraryFirstCollectionTitle}`,
    ]);
    expect(source.revisions).toEqual([LIBRARY.revision]);
    expect(registry.get("lib-1")).toEqual(LIBRARY);
    // The first collection may have advanced the revision: later commands read it afresh.
    expect(registry.revisionOf("lib-1")).toBeUndefined();
    expect(navigated).toEqual(["#/library/lib-1"]);
    expect(done).toHaveLength(1);
    expect(done[0]?.detail).toEqual({
      libraryId: "lib-1",
      collectionId: COLLECTION.collection_id,
    });
    expect(done[0]?.bubbles && done[0]?.composed).toBe(true);
  });

  it("refuses a name the API would refuse, says so under the field and sends nothing", async () => {
    const { dialog, source } = await mount();
    button(dialog, "choose").click();
    await settle();
    source.calls.length = 0;
    type(nameInput(dialog), "bad\u0007name");
    button(dialog, "create").click();
    await settle();
    expect(source.calls).toEqual([]);
    expect(nameInput(dialog).getAttribute("aria-invalid")).toBe("true");
    const problem = rootOf(dialog).querySelector("#first-use-name-error") as HTMLElement;
    expect(problem.hidden).toBe(false);
    expect(problem.textContent).toBe(labels.libraryFirstUseNameInvalid);
    expect(rootOf(dialog).activeElement).toBe(nameInput(dialog));
    type(nameInput(dialog), "Good name");
    expect(nameInput(dialog).hasAttribute("aria-invalid")).toBe(false);
    expect(problem.hidden).toBe(true);
    button(dialog, "create").click();
    await settle();
    expect(source.calls[0]).toBe("create:loc-1:Good name");
  });

  it("accepts a name longer than 256 characters when it is within 1,024 bytes", async () => {
    const { dialog, source } = await mount();
    button(dialog, "choose").click();
    await settle();
    type(nameInput(dialog), "n".repeat(900));
    button(dialog, "create").click();
    await settle();
    expect(source.calls).toContain(`create:loc-1:${"n".repeat(900)}`);
  });

  it("sends no name when the field is blank, so the host names the library", async () => {
    const { dialog, source } = await mount();
    type(nameInput(dialog), "   ");
    button(dialog, "choose").click();
    await settle();
    button(dialog, "create").click();
    await settle();
    expect(source.calls).toContain("create:loc-1");
  });

  it("keeps the library when only the collection fails and retries that step alone", async () => {
    const source = new FakeLibrarySource();
    source.collection = apiError(500, "internal");
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { dialog, registry, navigated, done } = await mount(source);
    button(dialog, "choose").click();
    await settle();
    button(dialog, "create").click();
    await settle();
    expect(registry.get("lib-1")).toEqual(LIBRARY);
    expect(navigated).toEqual([]);
    expect(done).toEqual([]);
    expect(text(dialog, ".kept")).toBe(labels.libraryFirstUseCollectionFailed);
    const states = rootOf(dialog).querySelector("fs-states") as FsStates;
    const retry = states.shadowRoot?.querySelector("button") as HTMLButtonElement;
    expect(retry.textContent).toBe(labels.stateRetry);
    expect(nameInput(dialog).getAttribute("aria-disabled")).toBe("true");
    expect(button(dialog, "choose").getAttribute("aria-disabled")).toBe("true");
    source.calls.length = 0;
    source.collection = COLLECTION;
    retry.click();
    await settle();
    expect(source.calls).toEqual([`collection:lib-1:${labels.libraryFirstCollectionTitle}`]);
    expect(navigated).toEqual(["#/library/lib-1"]);
    expect(done).toHaveLength(1);
  });

  it("goes to the library page, not back, when Cancel is pressed after the library exists", async () => {
    const source = new FakeLibrarySource();
    source.collection = apiError(500, "internal");
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { dialog, navigated, cancelled } = await mount(source);
    button(dialog, "choose").click();
    await settle();
    button(dialog, "create").click();
    await settle();
    button(dialog, "cancel").click();
    expect(navigated).toEqual(["#/library/lib-1"]);
    expect(cancelled).toHaveLength(1);
    expect(dialog.isConnected).toBe(false);
  });

  it("shows a refused create as a sentence, keeps the folder and lets Create run again", async () => {
    const source = new FakeLibrarySource();
    source.created = apiError(409, "conflict");
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { dialog, registry } = await mount(source);
    button(dialog, "choose").click();
    await settle();
    button(dialog, "create").click();
    await settle();
    expect(text(dialog, ".alert")).toBe(labels.libraryConflictCreate);
    expect(registry.size).toBe(0);
    source.created = LIBRARY;
    button(dialog, "create").click();
    await settle();
    expect(source.calls.filter((call) => call.startsWith("choose"))).toHaveLength(1);
    expect(source.calls.filter((call) => call.startsWith("create"))).toHaveLength(2);
  });

  it("closes without any call when Cancel is pressed first", async () => {
    const { dialog, source, navigated, cancelled } = await mount();
    button(dialog, "cancel").click();
    expect(source.calls).toEqual([]);
    expect(navigated).toEqual([]);
    expect(cancelled).toHaveLength(1);
    expect(dialog.isConnected).toBe(false);
  });

  it("cancels on Escape and keeps Tab inside the dialog", async () => {
    const { dialog, cancelled } = await mount();
    const box = rootOf(dialog).querySelector('[role="dialog"]') as HTMLElement;
    const create = button(dialog, "create");
    const name = nameInput(dialog);
    create.focus();
    const tab = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
    box.dispatchEvent(tab);
    expect(tab.defaultPrevented).toBe(true);
    expect(rootOf(dialog).activeElement).toBe(name);
    box.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
    );
    expect(cancelled).toHaveLength(1);
  });

  it("marks the buttons busy while the library is being created", async () => {
    const source = new FakeLibrarySource();
    let release: (view: typeof LIBRARY) => void = () => undefined;
    source.created = () => new Promise((resolve) => (release = resolve));
    const { dialog } = await mount(source);
    button(dialog, "choose").click();
    await settle();
    button(dialog, "create").click();
    await settle();
    expect(button(dialog, "create").getAttribute("aria-disabled")).toBe("true");
    expect(button(dialog, "cancel").getAttribute("aria-disabled")).toBe("true");
    release(LIBRARY);
    await settle();
    expect(source.calls.some((call) => call.startsWith("collection"))).toBe(true);
  });
});
