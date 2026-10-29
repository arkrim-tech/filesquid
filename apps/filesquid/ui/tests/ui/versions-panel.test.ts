/**
 * UI tests for `<fs-versions-panel>` (UI-F2.3, UI-S-009, UI-S-031 26 and 27): the Back row and
 * count, the history eyebrow, rows newest first with the current version marked and no menu on
 * it, the row menu, the restore dialog (one call with the optional label, a refusal kept in the
 * dialog, Escape and Tab inside), the change event, and the loading and error states.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  MAX_VERSION_ROWS,
  accessibilityViolations,
  defineElements,
  type FsVersionsPanel,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { FakePanelSource, PANEL_FILE, gate } from "../support/panel-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

/** Let queued promise callbacks run. */
const settle = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

/** The fixed clock of the fixture: two hours after v7. */
const NOW = new Date("2026-10-16T14:00:00Z");

/** Mount a versions panel over `source` showing the fixture file. */
async function mount(source = new FakePanelSource()): Promise<FsVersionsPanel> {
  const panel = document.createElement("fs-versions-panel") as FsVersionsPanel;
  panel.configure({ labels, source, now: () => NOW });
  document.body.replaceChildren(panel);
  panel.fileRef = PANEL_FILE;
  await settle();
  return panel;
}

/** The shadow root of a panel. */
function root(panel: HTMLElement): ShadowRoot {
  return panel.shadowRoot as ShadowRoot;
}

/** The row of a version. */
function row(panel: HTMLElement, version: string): HTMLElement {
  return root(panel).querySelector(`li.version[data-version="${version}"]`) as HTMLElement;
}

/** Open the menu of a version and choose Restore as new version. */
async function openRestore(panel: HTMLElement, version: string): Promise<HTMLElement> {
  row(panel, version).querySelector<HTMLButtonElement>("button.more")?.click();
  root(panel).querySelector<HTMLButtonElement>('[role="menuitem"][data-action="restore"]')?.click();
  await settle();
  return root(panel).querySelector('[role="dialog"]') as HTMLElement;
}

describe("<fs-versions-panel>", () => {
  it("is a defined element with the shared stylesheet first", async () => {
    expect(customElements.get("fs-versions-panel")).toBeDefined();
    const panel = await mount();
    expect(root(panel).firstElementChild?.tagName).toBe("STYLE");
  });

  it("shows Back, the count, the history eyebrow and the note", async () => {
    const panel = await mount();
    expect(panel.dataset.state).toBe("ready");
    expect(root(panel).querySelector("button.back")?.textContent).toBe(labels.versionsBack);
    expect(root(panel).querySelector(".count")?.textContent).toBe("7 versions");
    expect(root(panel).querySelector(".eyebrow")?.textContent).toBe(
      "History 96.3 MB · newest first",
    );
    expect(root(panel).querySelector(".note .note-title")?.textContent).toBe(
      labels.versionsKeepEvery,
    );
    expect(root(panel).querySelector(".note .note-detail")?.textContent).toBe(
      labels.versionsKeepDetail,
    );
    expect(accessibilityViolations(panel)).toEqual([]);
  });

  it("lists every version newest first with chip, label or No label, and time and size", async () => {
    const panel = await mount();
    const rows = [...root(panel).querySelectorAll<HTMLElement>("li.version")];
    expect(rows.map((r) => r.dataset.version)).toEqual(["v7", "v6", "v5", "v4", "v3", "v2", "v1"]);
    expect(rows[0]?.querySelector(".mono-chip")?.textContent).toBe("v7");
    expect(rows[0]?.querySelector(".label")?.textContent).toBe("Issued for tender");
    expect(rows[0]?.querySelector(".meta")?.textContent).toBe("2 hours ago · 18.4 MB");
    expect(rows[1]?.querySelector(".no-label")?.textContent).toBe(labels.versionsNoLabel);
    expect(rows[1]?.querySelector(".meta")?.textContent).toBe("Yesterday · 18.1 MB");
    expect(rows[2]?.querySelector(".meta time")?.getAttribute("datetime")).toBe(
      "2026-10-09T10:00:00Z",
    );
    expect(root(panel).querySelector("ul.versions")?.getAttribute("aria-label")).toBe(
      "Versions of harbour-masterplan-v7.pdf",
    );
  });

  it("marks the current version with a chip and offers no menu on it; every other row has one", async () => {
    const panel = await mount();
    expect(row(panel, "v7").querySelector(".current")?.textContent).toBe(labels.versionsCurrent);
    expect(row(panel, "v7").querySelector("button.more")).toBeNull();
    const more = row(panel, "v5").querySelector<HTMLButtonElement>("button.more");
    expect(more?.getAttribute("aria-label")).toBe("Actions for v5");
    expect(more?.getAttribute("aria-haspopup")).toBe("menu");
    expect(root(panel).querySelectorAll("button.more")).toHaveLength(6);
  });

  it("opens the row menu with Restore as new version and no delete, expiry or thinning", async () => {
    const panel = await mount();
    const more = row(panel, "v5").querySelector<HTMLButtonElement>(
      "button.more",
    ) as HTMLButtonElement;
    more.click();
    expect(more.getAttribute("aria-expanded")).toBe("true");
    const items = [...root(panel).querySelectorAll('[role="menu"] [role="menuitem"]')];
    expect(items.map((i) => i.textContent)).toEqual([labels.versionsRestoreAction]);
    const words = items.map((i) => i.textContent?.toLowerCase() ?? "").join(" ");
    expect(words).not.toMatch(/delete|expir|thin/);
    expect(root(panel).activeElement).toBe(items[0]);
    items[0]?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(root(panel).querySelector('[role="menu"]')).toBeNull();
    expect(more.getAttribute("aria-expanded")).toBe("false");
    expect(root(panel).activeElement).toBe(more);
  });

  it("opens the restore dialog naming the file, the source and the new version", async () => {
    const panel = await mount();
    const dialog = await openRestore(panel, "v5");
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(dialog.querySelector(".dialog-title")?.textContent).toBe("Restore v5 as a new version?");
    expect(dialog.querySelector(".file-name")?.textContent).toBe("harbour-masterplan-v7.pdf");
    expect(dialog.querySelector(".source-line .mono-chip")?.textContent).toBe("v5");
    expect(dialog.querySelector(".source-line .line-text")?.textContent).toBe(
      "Client review · Oct 9, 2026 · 17.9 MB",
    );
    expect(dialog.querySelector(".new-line .mono-chip")?.textContent).toBe("v8");
    expect(dialog.querySelector(".new-line .line-text")?.textContent).toBe(
      "New current version, same content as v5",
    );
    expect(dialog.querySelector("label.label-field")?.textContent).toBe("Label for v8 (optional)");
    expect(dialog.querySelector(".keep")?.textContent).toBe(
      "v6 and v7 stay in the history. You can restore them later.",
    );
    expect(dialog.querySelector("button.cancel")?.textContent).toBe(labels.fileCancel);
    expect(dialog.querySelector("button.restore")?.textContent).toBe(labels.versionsRestore);
    expect(dialog.querySelectorAll("button.primary")).toHaveLength(1);
    expect(root(panel).activeElement).toBe(dialog.querySelector("input.label-input"));
    expect(accessibilityViolations(panel)).toEqual([]);
  });

  it("restores with one call carrying the label, announces it and dispatches the change", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    const changes: unknown[] = [];
    document.addEventListener("fs-files-change", (e) => changes.push((e as CustomEvent).detail), {
      once: true,
    });
    const dialog = await openRestore(panel, "v5");
    const input = dialog.querySelector<HTMLInputElement>("input.label-input") as HTMLInputElement;
    input.value = "  Back to client review version  ";
    dialog.querySelector<HTMLButtonElement>("button.restore")?.click();
    await settle();
    await settle();
    expect(source.restores).toEqual([
      {
        object: { library_id: "lib", object_id: PANEL_FILE.object_id },
        version: "v5",
        paths: [PANEL_FILE.path],
        revision: "v7",
        label: "Back to client review version",
      },
    ]);
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
    expect(root(panel).querySelector(".announce")?.textContent).toBe(
      "harbour-masterplan-v7.pdf restored. Version v8 committed.",
    );
    expect(changes).toEqual([
      { operation: "restore", commit: expect.objectContaining({ version: "v8" }) },
    ]);
    expect(row(panel, "v8").querySelector(".current")).not.toBeNull();
  });

  it("restores without a label when the field is left empty", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    const dialog = await openRestore(panel, "v2");
    dialog.querySelector<HTMLButtonElement>("button.restore")?.click();
    await settle();
    expect(source.restores[0]?.label).toBeUndefined();
  });

  it("refuses a label with a control character or over 512 bytes before calling the API", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    const dialog = await openRestore(panel, "v5");
    const input = dialog.querySelector<HTMLInputElement>("input.label-input") as HTMLInputElement;
    input.value = "é".repeat(257);
    dialog.querySelector<HTMLButtonElement>("button.restore")?.click();
    await settle();
    expect(source.calls.restore).toBe(0);
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(dialog.querySelector(".error")?.textContent).toBe(labels.versionsRestoreLabelInvalid);
    input.value = "tab\there";
    dialog.querySelector<HTMLButtonElement>("button.restore")?.click();
    await settle();
    expect(source.calls.restore).toBe(0);
  });

  it("sends the head it read as the revision, so a head moved meanwhile is refused", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    const dialog = await openRestore(panel, "v5");
    source.head = "v9";
    dialog.querySelector<HTMLButtonElement>("button.restore")?.click();
    await settle();
    expect(source.restores).toEqual([]);
    expect(dialog.querySelector(".error")?.textContent).toBe(labels.fileConflict);
  });

  it("keeps the dialog open with the refusal's words when the restore is refused", async () => {
    const source = new FakePanelSource();
    source.failures.restore = new ApiError(409, "/api/v1/files/restorations", { code: "conflict" });
    const panel = await mount(source);
    const dialog = await openRestore(panel, "v5");
    dialog.querySelector<HTMLButtonElement>("button.restore")?.click();
    await settle();
    expect(root(panel).querySelector('[role="dialog"]')).toBe(dialog);
    expect(dialog.querySelector(".error")?.textContent).toBe(labels.fileConflict);
    expect(dialog.querySelector<HTMLButtonElement>("button.restore")?.disabled).toBe(false);
  });

  it("disables the buttons while the restore runs", async () => {
    const source = new FakePanelSource();
    const wait = gate();
    source.gates.restore = wait;
    const panel = await mount(source);
    const dialog = await openRestore(panel, "v5");
    dialog.querySelector<HTMLButtonElement>("button.restore")?.click();
    expect(dialog.querySelector<HTMLButtonElement>("button.restore")?.disabled).toBe(true);
    expect(dialog.querySelector<HTMLButtonElement>("button.cancel")?.disabled).toBe(true);
    wait.open();
    await settle();
  });

  it("closes the dialog with Escape or Cancel without a call and returns focus to the menu button", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    const more = row(panel, "v5").querySelector<HTMLButtonElement>("button.more");
    let dialog = await openRestore(panel, "v5");
    dialog.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
    expect(root(panel).activeElement).toBe(more);
    dialog = await openRestore(panel, "v5");
    dialog.querySelector<HTMLButtonElement>("button.cancel")?.click();
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
    expect(source.calls.restore).toBe(0);
  });

  it("keeps Tab inside the dialog", async () => {
    const panel = await mount();
    const dialog = await openRestore(panel, "v5");
    const buttons = [...dialog.querySelectorAll<HTMLElement>("*")].filter(
      (el) => el instanceof HTMLButtonElement || el instanceof HTMLInputElement,
    );
    expect(buttons.map((el) => el.className)).toEqual([
      "icon-button close",
      "input label-input",
      "button secondary large cancel",
      "button primary large restore",
    ]);
    const last = buttons[buttons.length - 1] as HTMLElement;
    last.focus();
    const tab = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
    dialog.dispatchEvent(tab);
    expect(tab.defaultPrevented).toBe(true);
    expect(root(panel).activeElement).toBe(buttons[0]);
  });

  it("dispatches fs-versions-close from Back", async () => {
    const panel = await mount();
    const closed = vi.fn();
    panel.addEventListener("fs-versions-close", closed);
    root(panel).querySelector<HTMLButtonElement>("button.back")?.click();
    expect(closed).toHaveBeenCalledTimes(1);
  });

  it("words the eyebrow without a size when the history size cannot be read", async () => {
    const source = new FakePanelSource();
    source.failures.size = new ApiError(500, "/api/v1/files/history-size", { code: "internal" });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("ready");
    expect(root(panel).querySelector(".eyebrow")?.textContent).toBe(labels.versionsEyebrowNoSize);
    expect(warn).toHaveBeenCalled();
  });

  it("shows loading, then the error with Try again when the history cannot be read", async () => {
    const source = new FakePanelSource();
    const wait = gate();
    source.gates.history = wait;
    source.failures.history = new ApiError(500, "/api/v1/files/history", { code: "internal" });
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("loading");
    expect(root(panel).querySelector(".busy")?.textContent).toBe(labels.versionsLoading);
    wait.open();
    await settle();
    expect(panel.dataset.state).toBe("error");
    expect(root(panel).querySelector(".state .title")?.textContent).toBe(labels.versionsError);
    expect(root(panel).querySelector(".state .code")?.textContent).toBe("internal");
    delete source.failures.history;
    root(panel).querySelector<HTMLButtonElement>(".state button.retry")?.click();
    await settle();
    expect(panel.dataset.state).toBe("ready");
  });

  it("counts every version even when it lists only the newest MAX_VERSION_ROWS", async () => {
    const source = new FakePanelSource();
    const one = source.entries[0] as (typeof source.entries)[number];
    source.entries = Array.from({ length: MAX_VERSION_ROWS + 2 }, (_, i) => ({
      ...one,
      version: { ...one.version, version: `v${i + 1}` },
    }));
    source.head = `v${MAX_VERSION_ROWS + 2}`;
    const panel = await mount(source);
    expect(root(panel).querySelectorAll("li.version")).toHaveLength(MAX_VERSION_ROWS);
    expect(root(panel).querySelector(".count")?.textContent).toBe("10,002 versions");
  }, 60_000);

  it("drops an open dialog when the file, its version or the document changes", async () => {
    const panel = await mount();
    await openRestore(panel, "v5");
    panel.fileRef = { ...PANEL_FILE, version: "v8" };
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
    await settle();
    await openRestore(panel, "v5");
    panel.remove();
    document.body.append(panel);
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
  });

  it("applies nothing from a restore answered after the file changed, but still reports it", async () => {
    const source = new FakePanelSource();
    const wait = gate();
    source.gates.restore = wait;
    const panel = await mount(source);
    const changes: unknown[] = [];
    document.addEventListener("fs-files-change", (e) => changes.push((e as CustomEvent).detail));
    const dialog = await openRestore(panel, "v5");
    dialog.querySelector<HTMLButtonElement>("button.restore")?.click();
    panel.fileRef = { ...PANEL_FILE, path: "original/other.pdf", display_name: "other.pdf" };
    wait.open();
    await settle();
    expect(root(panel).querySelector(".announce")?.textContent).toBe("");
    expect(changes).toHaveLength(1);
  });

  it("refuses a restore of a version the history does not hold", async () => {
    const source = new FakePanelSource();
    const object = { library_id: "lib", object_id: PANEL_FILE.object_id };
    await expect(
      source.restore(object, "v99", [PANEL_FILE.path], { revision: "v7" }),
    ).rejects.toMatchObject({ code: "not_found" });
    await expect(
      source.restore(object, "v2", ["original/nope.pdf"], { revision: "v7" }),
    ).rejects.toMatchObject({ code: "not_found" });
  });

  it("marks a removal and offers no restore from it", async () => {
    const source = new FakePanelSource();
    source.entries = source.entries.map((entry, i) =>
      i === 2 ? { ...entry, change: "deleted", digest: null, size: null } : entry,
    );
    const panel = await mount(source);
    expect(row(panel, "v3").querySelector(".meta")?.textContent).toBe(
      `Oct 2, 2026 · ${labels.versionsRemoved}`,
    );
    expect(row(panel, "v3").querySelector("button.more")).toBeNull();
  });
});
