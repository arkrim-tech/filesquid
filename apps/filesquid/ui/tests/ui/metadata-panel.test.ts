/**
 * UI tests for `<fs-metadata-panel>`, the Description tab (UI-F2.4; UI-S-003, UI-S-022,
 * UI-S-031 mockups 28, 29 and 25; API-S-035): the dcterms and schema rows keyed by their CURIE
 * with the owner and extracted tones and source chips, edit and remove only on asserted rows,
 * the empty, loading, no-match and error states, the Add property dialog with its groups in
 * the API's add order, and each edit as one source call that is announced and reported.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { accessibilityViolations, defineElements, type FsMetadataPanel } from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { CUSTOM, FakeMetadataSource, METADATA_FILE, RICO, gate } from "../support/metadata-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

/** Let queued promise callbacks run. */
const settle = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

/** Mount a metadata panel over `source` showing the fixture file. */
async function mount(source = new FakeMetadataSource(), online = true): Promise<FsMetadataPanel> {
  const panel = document.createElement("fs-metadata-panel") as FsMetadataPanel;
  panel.configure({ labels, source, online: () => online });
  document.body.replaceChildren(panel);
  panel.fileRef = METADATA_FILE;
  await settle();
  return panel;
}

/** The shadow root of a panel. */
function root(panel: HTMLElement): ShadowRoot {
  return panel.shadowRoot as ShadowRoot;
}

/** The row of a CURIE. */
function row(panel: HTMLElement, curie: string): HTMLElement {
  return root(panel).querySelector(`.row[data-key="${curie}"]`) as HTMLElement;
}

/** Type into an input and dispatch `input`. */
function type(input: HTMLInputElement | null, text: string): void {
  if (!input) throw new Error("no input");
  input.value = text;
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

/** Open the Add property dialog and return it. */
async function openAdd(panel: HTMLElement): Promise<HTMLElement> {
  root(panel).querySelector<HTMLButtonElement>("button.add")?.click();
  await settle();
  return root(panel).querySelector('[role="dialog"]') as HTMLElement;
}

describe("<fs-metadata-panel>", () => {
  it("is a defined element with the shared stylesheet first and its tab label", async () => {
    expect(customElements.get("fs-metadata-panel")).toBeDefined();
    const panel = await mount();
    expect(root(panel).firstElementChild?.tagName).toBe("STYLE");
    expect(root(panel).querySelector("style")?.textContent).toContain(":focus-visible");
    expect(panel.tabLabel).toBe(labels.filePanelDescription);
  });

  it("lists the dcterms and schema rows keyed by their CURIE, with the count and no footer", async () => {
    const panel = await mount();
    expect(panel.dataset.state).toBe("ready");
    const keys = [...root(panel).querySelectorAll(".row")].map((r) => r.getAttribute("data-key"));
    expect(keys).toEqual([
      "dcterms:title",
      "dcterms:creator",
      "dcterms:subject",
      "dcterms:description",
      "dcterms:created",
      "dcterms:rights",
      "schema:contentLocation",
      "schema:copyrightYear",
    ]);
    expect(row(panel, "dcterms:title").querySelector("dt .key")?.textContent).toBe("dcterms:title");
    expect(row(panel, "dcterms:title").querySelector(".value")?.textContent).toBe(
      "Facade study, north pier",
    );
    expect(root(panel).querySelector(".count")?.textContent).toBe("Dublin Core and Schema.org · 8");
    expect(root(panel).querySelector("input.filter")?.getAttribute("placeholder")).toBe(
      labels.metadataFilter,
    );
    expect(root(panel).querySelector(".footer")).toBeNull();
    expect(root(panel).textContent).not.toContain("exif:");
    expect(accessibilityViolations(panel)).toEqual([]);
  });

  it("marks owner values with the You chip and edit and remove; extracted values with File only", async () => {
    const panel = await mount();
    const owner = row(panel, "dcterms:subject");
    expect(owner.querySelector("dt .key")?.classList.contains("key-owner")).toBe(true);
    expect(owner.querySelector(".chip.source")?.textContent).toBe(labels.metadataSourceOwner);
    expect(owner.querySelector(".chip.source")?.classList.contains("ok")).toBe(true);
    expect(owner.querySelector("button.edit")?.getAttribute("aria-label")).toBe(
      "Edit dcterms:subject",
    );
    expect(owner.querySelector("button.remove")?.getAttribute("aria-label")).toBe(
      "Remove dcterms:subject",
    );
    expect(owner.querySelector("button.copy")?.getAttribute("aria-label")).toBe(
      "Copy dcterms:subject",
    );
    const extracted = row(panel, "dcterms:created");
    expect(extracted.querySelector("dt .key")?.classList.contains("key-extracted")).toBe(true);
    expect(extracted.querySelector(".chip.source")?.textContent).toBe(labels.metadataSourceFile);
    expect(extracted.querySelector(".chip.source")?.classList.contains("muted")).toBe(true);
    expect(extracted.querySelector("button.edit")).toBeNull();
    expect(extracted.querySelector("button.remove")).toBeNull();
    expect(extracted.querySelector("button.copy")).not.toBeNull();
  });

  it("shows Add property below the rows and the empty state when the record holds none", async () => {
    const panel = await mount();
    const add = root(panel).querySelector<HTMLButtonElement>(".body > button.add");
    expect(add?.textContent).toBe(labels.metadataAdd);
    expect(add?.classList.contains("ghost")).toBe(true);
    const source = new FakeMetadataSource();
    source.statements_ = [];
    const empty = await mount(source);
    expect(empty.dataset.state).toBe("empty");
    expect(root(empty).querySelector(".count")?.textContent).toBe(labels.metadataCountEmpty);
    const state = root(empty).querySelector(".state.empty") as HTMLElement;
    expect(state.querySelector(".title")?.textContent).toBe(labels.metadataEmpty);
    expect(state.querySelector(".detail")?.textContent).toBe(labels.metadataEmptyHint);
    expect(state.querySelector("button.add")?.textContent).toBe(labels.metadataAdd);
    expect(root(empty).querySelector<HTMLInputElement>("input.filter")?.disabled).toBe(true);
  });

  it("filters rows by CURIE or value and shows no match with Clear filter", async () => {
    const panel = await mount();
    type(root(panel).querySelector("input.filter"), "schema:");
    expect(root(panel).querySelectorAll(".row")).toHaveLength(2);
    expect(root(panel).querySelector(".count")?.textContent).toBe(
      "Dublin Core and Schema.org · 2 of 8",
    );
    type(root(panel).querySelector("input.filter"), "Okafor");
    expect(root(panel).querySelectorAll(".row")).toHaveLength(2);
    type(root(panel).querySelector("input.filter"), "copyright");
    expect(root(panel).querySelectorAll(".row")).toHaveLength(1);
    type(root(panel).querySelector("input.filter"), "zzz");
    expect(panel.dataset.state).toBe("no-match");
    expect(root(panel).querySelector(".state .title")?.textContent).toBe(
      "No properties match “zzz”",
    );
    root(panel).querySelector<HTMLButtonElement>(".state button.clear")?.click();
    expect(root(panel).querySelectorAll(".row")).toHaveLength(8);
  });

  it("shows placeholder rows while loading and the error state with Try again", async () => {
    const source = new FakeMetadataSource();
    const wait = gate();
    source.gates.statements = wait;
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("loading");
    expect(root(panel).querySelector(".count")?.textContent).toBe(labels.metadataLoading);
    expect(root(panel).querySelectorAll(".placeholder").length).toBeGreaterThanOrEqual(8);
    wait.open();
    await settle();
    expect(panel.dataset.state).toBe("ready");
    const failing = new FakeMetadataSource();
    failing.failures.statements = new ApiError(503, "/api/v1/files/metadata", {
      code: "unavailable",
    });
    const second = await mount(failing);
    expect(second.dataset.state).toBe("error");
    const state = root(second).querySelector('.state[role="alert"]') as HTMLElement;
    expect(state.querySelector(".title")?.textContent).toBe(labels.metadataError);
    expect([...state.querySelectorAll(".detail")].map((p) => p.textContent)).toEqual([
      labels.stateUnavailable,
      labels.filePanelNothingChanged,
    ]);
    expect(state.querySelector(".code")?.textContent).toBe("unavailable");
    delete failing.failures.statements;
    state.querySelector<HTMLButtonElement>("button.retry")?.click();
    await settle();
    expect(second.dataset.state).toBe("ready");
  });

  it("shows a file that is not in its own object as an error without a code", async () => {
    const source = new FakeMetadataSource();
    source.failures.statements = new TypeError("not a file object");
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("error");
    expect(root(panel).querySelector(".state .code")).toBeNull();
  });

  it("removes an owner value in one call, announces it, reports the commit and lists again", async () => {
    const source = new FakeMetadataSource();
    const panel = await mount(source);
    const changes: unknown[] = [];
    panel.addEventListener("fs-files-change", (e) => changes.push((e as CustomEvent).detail));
    row(panel, "schema:copyrightYear").querySelector<HTMLButtonElement>("button.remove")?.click();
    await settle();
    await settle();
    expect(source.removals).toEqual([{ file: METADATA_FILE, curie: "schema:copyrightYear" }]);
    expect(changes).toEqual([
      {
        operation: "metadata.remove",
        commit: expect.objectContaining({ version: "v5", object_id: METADATA_FILE.object_id }),
      },
    ]);
    expect(root(panel).querySelector(".announce")?.textContent).toBe(
      "schema:copyrightYear removed. Version v5 committed.",
    );
    expect(root(panel).querySelectorAll(".row")).toHaveLength(7);
    expect(root(panel).querySelector(".count")?.textContent).toBe("Dublin Core and Schema.org · 7");
    expect(source.calls.statements).toBe(2);
  });

  it("words a refused removal and keeps the rows", async () => {
    const source = new FakeMetadataSource();
    source.failures.remove = new ApiError(409, "/api/v1/files/metadata/removals", {
      code: "conflict",
    });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const panel = await mount(source);
    row(panel, "dcterms:rights").querySelector<HTMLButtonElement>("button.remove")?.click();
    await settle();
    expect(root(panel).querySelector(".announce")?.textContent).toBe(labels.fileConflict);
    expect(root(panel).querySelectorAll(".row")).toHaveLength(8);
    expect(warn).toHaveBeenCalled();
  });

  it("edits an owner value through the dialog with the property fixed and Save", async () => {
    const source = new FakeMetadataSource();
    const panel = await mount(source);
    const edit = row(panel, "dcterms:title").querySelector<HTMLButtonElement>("button.edit");
    edit?.click();
    await settle();
    const dialog = root(panel).querySelector('[role="dialog"]') as HTMLElement;
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(dialog.querySelector(".dialog-title")?.textContent).toBe("Edit dcterms:title");
    expect(dialog.querySelector(".file-name")?.textContent).toBe("facade-study.jpg");
    expect(dialog.querySelector("input.search")).toBeNull();
    const value = dialog.querySelector<HTMLInputElement>("input.value-input");
    expect(value?.value).toBe("Facade study, north pier");
    expect(root(panel).activeElement).toBe(value);
    expect(dialog.querySelector("button.confirm")?.textContent).toBe(labels.fileSave);
    type(value, "Facade study, north pier (revised)");
    dialog.querySelector<HTMLButtonElement>("button.confirm")?.click();
    await settle();
    await settle();
    expect(source.sets).toEqual([
      { file: METADATA_FILE, curie: "dcterms:title", value: "Facade study, north pier (revised)" },
    ]);
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
    expect(root(panel).querySelector(".announce")?.textContent).toBe(
      "dcterms:title set. Version v5 committed.",
    );
    expect(row(panel, "dcterms:title").querySelector(".value")?.textContent).toBe(
      "Facade study, north pier (revised)",
    );
    expect(root(panel).activeElement).toBe(
      row(panel, "dcterms:title").querySelector("button.edit"),
    );
  });

  it("refuses a blank or oversized value before sending, and words a refused set in the dialog", async () => {
    const source = new FakeMetadataSource();
    const panel = await mount(source);
    row(panel, "dcterms:title").querySelector<HTMLButtonElement>("button.edit")?.click();
    await settle();
    const dialog = root(panel).querySelector('[role="dialog"]') as HTMLElement;
    const value = dialog.querySelector<HTMLInputElement>("input.value-input");
    type(value, "   ");
    dialog.querySelector<HTMLButtonElement>("button.confirm")?.click();
    await settle();
    expect(source.sets).toEqual([]);
    expect(value?.getAttribute("aria-invalid")).toBe("true");
    expect(dialog.querySelector(".error")?.textContent).toBe(labels.metadataValueInvalid);
    // 32,769 two-byte characters are 65,538 UTF-8 bytes, over the route's 65,536.
    type(value, "é".repeat(32_769));
    dialog.querySelector<HTMLButtonElement>("button.confirm")?.click();
    await settle();
    expect(source.sets).toEqual([]);
    expect(value?.getAttribute("aria-invalid")).toBe("true");
    expect(dialog.querySelector(".error")?.textContent).toBe(labels.metadataValueInvalid);
    expect(root(panel).querySelector('[role="dialog"]')).not.toBeNull();
    type(value, "Facade study, north pier");
    source.failures.set = new ApiError(400, "/api/v1/files/metadata/statements", {
      code: "invalid_request",
    });
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    dialog.querySelector<HTMLButtonElement>("button.confirm")?.click();
    await settle();
    expect(root(panel).querySelector('[role="dialog"]')).not.toBeNull();
    expect(dialog.querySelector(".error")?.textContent).toBe(labels.metadataRefused);
    delete source.failures.set;
    source.failures.set = new ApiError(409, "/api/v1/files/metadata/statements", {
      code: "conflict",
    });
    dialog.querySelector<HTMLButtonElement>("button.confirm")?.click();
    await settle();
    expect(dialog.querySelector(".error")?.textContent).toBe(labels.fileConflict);
  });

  it("opens Add property with the search, the groups in the add order, and a custom key only with a custom namespace", async () => {
    const source = new FakeMetadataSource();
    source.registry = [RICO, CUSTOM];
    const panel = await mount(source);
    const dialog = await openAdd(panel);
    expect(dialog.querySelector(".dialog-title")?.textContent).toBe(labels.metadataAdd);
    const search = dialog.querySelector<HTMLInputElement>("input.search");
    expect(root(panel).activeElement).toBe(search);
    expect(search?.getAttribute("aria-label")).toBe(labels.metadataSearch);
    expect(dialog.querySelector<HTMLButtonElement>("button.confirm")?.disabled).toBe(true);
    type(search, "righ");
    await settle();
    expect(source.lookups).toEqual(["righ"]);
    const groups = [...dialog.querySelectorAll(".group")].map((g) => g.getAttribute("data-group"));
    expect(groups).toEqual(["dcterms", "schema", "registered"]);
    expect(
      [...dialog.querySelectorAll('.group[data-group="dcterms"] .candidate')].map((c) =>
        c.getAttribute("data-curie"),
      ),
    ).toEqual([
      "dcterms:rights",
      "dcterms:rightsHolder",
      "dcterms:accessRights",
      "dcterms:dateCopyrighted",
    ]);
    expect(
      dialog.querySelector('.group[data-group="dcterms"] .candidate .description')?.textContent,
    ).toBe(labels.dctermsRights);
    expect(dialog.querySelector('.group[data-group="dcterms"] .eyebrow')?.textContent).toBe(
      labels.metadataGroupDcterms,
    );
    expect(
      dialog.querySelector('.group[data-group="schema"] .candidate')?.getAttribute("data-curie"),
    ).toBe("schema:righ");
    expect(dialog.querySelector('.group[data-group="schema"] .description')?.textContent).toBe(
      labels.metadataSchemaNote,
    );
    expect(
      [...dialog.querySelectorAll('.group[data-group="registered"] .candidate')].map((c) =>
        c.getAttribute("data-curie"),
      ),
    ).toEqual(["rico:righ", "harbour:righ"]);
    expect(
      dialog.querySelector('.group[data-group="registered"] .candidate .description')?.textContent,
    ).toBe(RICO.iri);
    expect(dialog.querySelector("button.custom-key")?.textContent).toBe(labels.metadataCustomKey);
    expect(accessibilityViolations(panel)).toEqual([]);

    const plain = new FakeMetadataSource();
    const second = await mount(plain);
    const other = await openAdd(second);
    expect(other.querySelector("button.custom-key")).toBeNull();
  });

  it("adds a dcterms property in one call and lists it", async () => {
    const source = new FakeMetadataSource();
    const panel = await mount(source);
    const changes: unknown[] = [];
    panel.addEventListener("fs-files-change", (e) => changes.push((e as CustomEvent).detail));
    const dialog = await openAdd(panel);
    type(dialog.querySelector("input.search"), "rightsHolder");
    await settle();
    const candidate = dialog.querySelector<HTMLButtonElement>(
      '.candidate[data-curie="dcterms:rightsHolder"]',
    );
    candidate?.click();
    expect(candidate?.getAttribute("aria-pressed")).toBe("true");
    expect(dialog.querySelector(".value-label")?.textContent).toBe(
      "Value for dcterms:rightsHolder",
    );
    const value = dialog.querySelector<HTMLInputElement>("input.value-input");
    expect(root(panel).activeElement).toBe(value);
    expect(dialog.querySelector<HTMLButtonElement>("button.confirm")?.disabled).toBe(false);
    type(value, "Mara Okafor");
    dialog.querySelector<HTMLButtonElement>("button.confirm")?.click();
    await settle();
    await settle();
    expect(source.sets).toEqual([
      { file: METADATA_FILE, curie: "dcterms:rightsHolder", value: "Mara Okafor" },
    ]);
    expect(changes).toEqual([
      { operation: "metadata.set", commit: expect.objectContaining({ version: "v5" }) },
    ]);
    expect(root(panel).querySelectorAll(".row")).toHaveLength(9);
    expect(row(panel, "dcterms:rightsHolder").querySelector(".value")?.textContent).toBe(
      "Mara Okafor",
    );
    expect(root(panel).activeElement).toBe(root(panel).querySelector(".body > button.add"));
  });

  it("adds a key in a custom namespace after checking it", async () => {
    const source = new FakeMetadataSource();
    source.registry = [CUSTOM];
    const panel = await mount(source);
    const dialog = await openAdd(panel);
    dialog.querySelector<HTMLButtonElement>("button.custom-key")?.click();
    const key = dialog.querySelector<HTMLInputElement>("input.custom-input");
    expect(root(panel).activeElement).toBe(key);
    type(key, "berth");
    expect(dialog.querySelector(".error")?.textContent).toBe(labels.metadataCustomKeyInvalid);
    expect(dialog.querySelector<HTMLButtonElement>("button.confirm")?.disabled).toBe(true);
    type(key, "harbour:berth");
    expect(dialog.querySelector(".error")?.textContent).toBe("");
    expect(dialog.querySelector(".value-label")?.textContent).toBe("Value for harbour:berth");
    type(dialog.querySelector("input.value-input"), "North pier, berth 4");
    dialog.querySelector<HTMLButtonElement>("button.confirm")?.click();
    await settle();
    await settle();
    expect(source.sets).toEqual([
      { file: METADATA_FILE, curie: "harbour:berth", value: "North pier, berth 4" },
    ]);
  });

  it("shows no candidates for an unknown name and asks the API only for a valid name", async () => {
    const source = new FakeMetadataSource();
    const panel = await mount(source);
    const dialog = await openAdd(panel);
    type(dialog.querySelector("input.search"), "a b");
    await settle();
    expect(source.lookups).toEqual([]);
    expect(dialog.querySelector(".no-candidates")?.textContent).toBe("No property matches “a b”.");
    type(dialog.querySelector("input.search"), "");
    await settle();
    expect(dialog.querySelector(".no-candidates")).toBeNull();
    expect(dialog.querySelectorAll(".group")).toHaveLength(0);
  });

  it("keeps Tab inside the dialog, and Escape cancels and returns focus to the opener", async () => {
    const panel = await mount();
    const add = root(panel).querySelector<HTMLButtonElement>(".body > button.add");
    add?.click();
    await settle();
    const dialog = root(panel).querySelector('[role="dialog"]') as HTMLElement;
    const close = dialog.querySelector<HTMLButtonElement>("button.close") as HTMLButtonElement;
    close.focus();
    close.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Tab", shiftKey: true, bubbles: true }),
    );
    expect(root(panel).activeElement).toBe(dialog.querySelector("button.cancel"));
    dialog.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
    expect(root(panel).activeElement).toBe(add);
  });

  it("drops the dialog and ignores stale answers when the file changes", async () => {
    const source = new FakeMetadataSource();
    source.ignoreAbort = true;
    const panel = await mount(source);
    await openAdd(panel);
    const wait = gate();
    source.gates.statements = wait;
    panel.fileRef = { ...METADATA_FILE, version: "v5" };
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
    panel.fileRef = null;
    wait.open();
    await settle();
    expect(panel.dataset.state).toBe("idle");
    expect(root(panel).querySelectorAll(".row")).toHaveLength(0);
  });

  it("does not revert to the old file when a write commits after another file opened", async () => {
    const source = new FakeMetadataSource();
    const panel = await mount(source);
    const changes: unknown[] = [];
    panel.addEventListener("fs-files-change", (e) => changes.push((e as CustomEvent).detail));
    row(panel, "dcterms:title").querySelector<HTMLButtonElement>("button.edit")?.click();
    await settle();
    const dialog = root(panel).querySelector('[role="dialog"]') as HTMLElement;
    const wait = gate();
    source.gates.set = wait;
    type(dialog.querySelector("input.value-input"), "Facade study, revised");
    dialog.querySelector<HTMLButtonElement>("button.confirm")?.click();
    await settle();
    const other = { ...METADATA_FILE, path: "original/9f2c41ab.pdf", display_name: "other.pdf" };
    panel.fileRef = other;
    await settle();
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
    const reads = source.calls.statements;
    wait.open();
    await settle();
    await settle();
    expect(changes).toEqual([
      { operation: "metadata.set", commit: expect.objectContaining({ version: "v5" }) },
    ]);
    expect(panel.fileRef).toEqual(other);
    expect(source.calls.statements).toBe(reads);
  });

  it("focuses the empty state's Add property after removing the last statement", async () => {
    const source = new FakeMetadataSource();
    source.statements_ = source.statements_.filter((s) => s.curie === "dcterms:title");
    const panel = await mount(source);
    row(panel, "dcterms:title").querySelector<HTMLButtonElement>("button.remove")?.click();
    await settle();
    await settle();
    expect(panel.dataset.state).toBe("empty");
    expect(root(panel).activeElement).toBe(root(panel).querySelector(".state.empty button.add"));
  });

  it("disables a row's actions while its removal runs", async () => {
    const source = new FakeMetadataSource();
    const wait = gate();
    source.gates.remove = wait;
    const panel = await mount(source);
    const subject = row(panel, "dcterms:subject");
    const remove = subject.querySelector<HTMLButtonElement>("button.remove") as HTMLButtonElement;
    remove.click();
    await settle();
    expect(remove.disabled).toBe(true);
    expect(subject.querySelector<HTMLButtonElement>("button.edit")?.disabled).toBe(true);
    remove.click();
    wait.open();
    await settle();
    await settle();
    expect(source.removals).toHaveLength(1);
    expect(root(panel).querySelectorAll(".row")).toHaveLength(7);
  });

  it("re-enables a row's actions when its removal is refused", async () => {
    const source = new FakeMetadataSource();
    source.failures.remove = new ApiError(409, "/api/v1/files/metadata/removals", {
      code: "conflict",
    });
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const panel = await mount(source);
    const remove = row(panel, "dcterms:subject").querySelector<HTMLButtonElement>("button.remove");
    remove?.click();
    await settle();
    expect(remove?.disabled).toBe(false);
  });

  it("clears a custom key's invalid mark when a candidate is chosen instead", async () => {
    const source = new FakeMetadataSource();
    source.registry = [CUSTOM];
    const panel = await mount(source);
    const dialog = await openAdd(panel);
    dialog.querySelector<HTMLButtonElement>("button.custom-key")?.click();
    const key = dialog.querySelector<HTMLInputElement>("input.custom-input");
    type(key, "nope");
    expect(key?.getAttribute("aria-invalid")).toBe("true");
    type(dialog.querySelector("input.search"), "title");
    await settle();
    dialog.querySelector<HTMLButtonElement>('.candidate[data-curie="dcterms:title"]')?.click();
    expect(key?.value).toBe("");
    expect(key?.hasAttribute("aria-invalid")).toBe(false);
    expect(dialog.querySelector(".error")?.textContent).toBe("");
  });

  it("names extensions in the count when a registered row is listed", async () => {
    const source = new FakeMetadataSource();
    source.registry = [RICO];
    source.statements_.push({
      curie: "rico:hasOrHadRightsHolder",
      prefix: "rico",
      term: "hasOrHadRightsHolder",
      value: "Mara Okafor",
      source: "asserted",
    });
    const panel = await mount(source);
    expect(root(panel).querySelector(".count")?.textContent).toBe(
      "Dublin Core, Schema.org and extensions · 9",
    );
    type(root(panel).querySelector("input.filter"), "rico");
    expect(root(panel).querySelector(".count")?.textContent).toBe(
      "Dublin Core, Schema.org and extensions · 1 of 9",
    );
  });

  it("re-reads the statements of a new version of the same file", async () => {
    const source = new FakeMetadataSource();
    const panel = await mount(source);
    panel.fileRef = { ...METADATA_FILE, version: "v5" };
    await settle();
    expect(source.calls.statements).toBe(2);
    expect(panel.dataset.state).toBe("ready");
  });
});
