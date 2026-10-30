/**
 * UI tests for the UI-F2.11 additions to `<fs-versions-panel>` (UI-S-009, UI-S-031 mockup 26,
 * PRD F-065): the per-version Download this version and Edit label, as the row menu offers
 * them. Edit label is one command with the head as the expected revision and a null label to
 * clear; an unchanged label sends nothing; a refusal stays in the dialog. Download hands the
 * file of that version to the browser under its display name, busy while it runs, with a
 * visible alert when it fails. No delete, expiry or thinning control exists.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { accessibilityViolations, defineElements, type FsVersionsPanel } from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { FakePanelSource, PANEL_FILE, gate } from "../support/panel-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

/** Let queued promise callbacks run. */
const settle = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

/** The fixed clock of the fixture: two hours after v7. */
const NOW = new Date("2026-10-16T14:00:00Z");

/** The saves the panel made. */
type Save = (blob: Blob, name: string) => void;

/** Mount a versions panel over `source` showing the fixture file. */
async function mount(
  source = new FakePanelSource(),
  save: Save = () => undefined,
): Promise<FsVersionsPanel> {
  const panel = document.createElement("fs-versions-panel") as FsVersionsPanel;
  panel.configure({ labels, source, now: () => NOW, save });
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

/** Open a version's menu and choose `action`. */
async function choose(panel: HTMLElement, version: string, action: string): Promise<void> {
  row(panel, version).querySelector<HTMLButtonElement>("button.more")?.click();
  root(panel)
    .querySelector<HTMLButtonElement>(`[role="menuitem"][data-action="${action}"]`)
    ?.click();
  await settle();
}

/** Open the Edit label dialog of a version. */
async function openLabel(panel: HTMLElement, version: string): Promise<HTMLElement> {
  await choose(panel, version, "edit-label");
  return root(panel).querySelector('[role="dialog"]') as HTMLElement;
}

describe("Download this version", () => {
  it("reads the file of that version and saves it under the display name", async () => {
    const source = new FakePanelSource();
    const saved: [Blob, string][] = [];
    const panel = await mount(source, (blob, name) => saved.push([blob, name]));
    await choose(panel, "v5", "download");
    await settle();
    expect(source.downloads).toEqual([{ ...PANEL_FILE, version: "v5" }]);
    expect(saved).toHaveLength(1);
    expect(saved[0]?.[1]).toBe("harbour-masterplan-v7.pdf");
    expect(root(panel).querySelector('[role="menu"]')).toBeNull();
  });

  it("offers it on the current version too", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    await choose(panel, "v7", "download");
    await settle();
    expect(source.downloads.map((d) => d.version)).toEqual(["v7"]);
  });

  it("announces the download while it runs and marks the menu button busy", async () => {
    const source = new FakePanelSource();
    const wait = gate();
    source.gates.download = wait;
    const panel = await mount(source);
    await choose(panel, "v5", "download");
    expect(root(panel).querySelector(".announce")?.textContent).toBe(
      "Downloading v5 of harbour-masterplan-v7.pdf…",
    );
    expect(row(panel, "v5").querySelector("button.more")?.getAttribute("aria-busy")).toBe("true");
    wait.open();
    await settle();
    expect(row(panel, "v5").querySelector("button.more")?.hasAttribute("aria-busy")).toBe(false);
  });

  it("shows a visible alert when the download fails and nothing is saved", async () => {
    const source = new FakePanelSource();
    source.failures.download = new ApiError(503, "/api/v1/files/download", { code: "unavailable" });
    const saved = vi.fn();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const panel = await mount(source, saved);
    await choose(panel, "v5", "download");
    await settle();
    const alert = root(panel).querySelector('[role="alert"].notice');
    expect(alert?.textContent).toBe("Could not download v5. Nothing was changed.");
    expect(saved).not.toHaveBeenCalled();
    expect(warn).toHaveBeenCalled();
    expect(accessibilityViolations(panel)).toEqual([]);
  });

  it("says a version too large to download here is too large", async () => {
    const source = new FakePanelSource();
    source.failures.download = new ApiError(413, "/api/v1/files/content", {
      code: "payload_too_large",
    });
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const panel = await mount(source);
    await choose(panel, "v5", "download");
    await settle();
    expect(root(panel).querySelector(".notice")?.textContent).toBe(
      `Could not download v5. Nothing was changed. ${labels.versionsDownloadTooLarge}`,
    );
  });

  it("clears the alert on the next download", async () => {
    const source = new FakePanelSource();
    source.failures.download = new ApiError(503, "/api/v1/files/download", { code: "unavailable" });
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const panel = await mount(source);
    await choose(panel, "v5", "download");
    await settle();
    delete source.failures.download;
    await choose(panel, "v4", "download");
    await settle();
    expect(root(panel).querySelector(".notice")).toBeNull();
  });

  it("offers no download for a version that removed the file", async () => {
    const source = new FakePanelSource();
    source.entries = source.entries.map((entry, i) =>
      i === 2 ? { ...entry, change: "deleted", digest: null, size: null } : entry,
    );
    const panel = await mount(source);
    row(panel, "v3").querySelector<HTMLButtonElement>("button.more")?.click();
    expect(root(panel).querySelector('[data-action="download"]')).toBeNull();
  });
});

describe("Edit label", () => {
  it("opens a dialog for the version with its label in the field and one filled primary button", async () => {
    const panel = await mount();
    const dialog = await openLabel(panel, "v5");
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(dialog.querySelector(".dialog-title")?.textContent).toBe("Edit label for v5");
    expect(dialog.querySelector(".file-name")?.textContent).toBe("harbour-masterplan-v7.pdf");
    expect(dialog.querySelector("label.label-field")?.textContent).toBe(
      labels.versionsEditLabelField,
    );
    expect(dialog.querySelector<HTMLInputElement>("input.label-input")?.value).toBe(
      "Client review",
    );
    expect(dialog.querySelector(".help")?.textContent).toBe(labels.versionsEditLabelHelp);
    expect(dialog.querySelector("button.cancel")?.textContent).toBe(labels.fileCancel);
    expect(dialog.querySelector("button.save")?.textContent).toBe(labels.versionsEditLabelSave);
    expect(dialog.querySelectorAll("button.primary")).toHaveLength(1);
    expect(root(panel).activeElement).toBe(dialog.querySelector("input.label-input"));
    expect(accessibilityViolations(panel)).toEqual([]);
  });

  it("opens with an empty field for a version without a label", async () => {
    const panel = await mount();
    const dialog = await openLabel(panel, "v6");
    expect(dialog.querySelector<HTMLInputElement>("input.label-input")?.value).toBe("");
  });

  it("saves the trimmed label with one call carrying the head, announces it and dispatches the change", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    const changes: unknown[] = [];
    document.addEventListener("fs-files-change", (e) => changes.push((e as CustomEvent).detail), {
      once: true,
    });
    const dialog = await openLabel(panel, "v6");
    const input = dialog.querySelector<HTMLInputElement>("input.label-input") as HTMLInputElement;
    input.value = "  Sent to the client  ";
    dialog.querySelector<HTMLButtonElement>("button.save")?.click();
    await settle();
    await settle();
    expect(source.labelEdits).toEqual([
      { version: "v6", label: "Sent to the client", revision: "v7" },
    ]);
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
    expect(root(panel).querySelector(".announce")?.textContent).toBe("Label of v6 saved.");
    expect(changes).toEqual([
      { operation: "label", commit: expect.objectContaining({ version: "v8" }) },
    ]);
    expect(row(panel, "v6").querySelector(".label")?.textContent).toBe("Sent to the client");
  });

  it("clears the label when the field is emptied", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    const dialog = await openLabel(panel, "v5");
    const input = dialog.querySelector<HTMLInputElement>("input.label-input") as HTMLInputElement;
    input.value = "   ";
    dialog.querySelector<HTMLButtonElement>("button.save")?.click();
    await settle();
    await settle();
    expect(source.labelEdits).toEqual([{ version: "v5", label: null, revision: "v7" }]);
    expect(root(panel).querySelector(".announce")?.textContent).toBe("Label of v5 removed.");
    expect(row(panel, "v5").querySelector(".no-label")).not.toBeNull();
  });

  it("closes without a call when the label is unchanged, as the route would refuse it", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    let dialog = await openLabel(panel, "v5");
    dialog.querySelector<HTMLButtonElement>("button.save")?.click();
    await settle();
    expect(source.calls.label).toBe(0);
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
    dialog = await openLabel(panel, "v6");
    dialog.querySelector<HTMLButtonElement>("button.save")?.click();
    await settle();
    expect(source.calls.label).toBe(0);
  });

  it("refuses a label with a control character or over 512 bytes before calling the API", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    const dialog = await openLabel(panel, "v5");
    const input = dialog.querySelector<HTMLInputElement>("input.label-input") as HTMLInputElement;
    input.value = "é".repeat(257);
    dialog.querySelector<HTMLButtonElement>("button.save")?.click();
    await settle();
    expect(source.calls.label).toBe(0);
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(dialog.querySelector(".error")?.textContent).toBe(labels.versionsRestoreLabelInvalid);
    input.value = "tab\there";
    dialog.querySelector<HTMLButtonElement>("button.save")?.click();
    await settle();
    expect(source.calls.label).toBe(0);
  });

  it("keeps the dialog open with the refusal's words when the head moved meanwhile", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    const dialog = await openLabel(panel, "v5");
    source.head = "v9";
    const input = dialog.querySelector<HTMLInputElement>("input.label-input") as HTMLInputElement;
    input.value = "Reviewed";
    dialog.querySelector<HTMLButtonElement>("button.save")?.click();
    await settle();
    expect(root(panel).querySelector('[role="dialog"]')).toBe(dialog);
    expect(dialog.querySelector(".error")?.textContent).toBe(labels.fileConflict);
    expect(dialog.querySelector<HTMLButtonElement>("button.save")?.disabled).toBe(false);
    expect(source.labelEdits).toEqual([]);
  });

  it("disables the buttons while the label is saved", async () => {
    const source = new FakePanelSource();
    const wait = gate();
    source.gates.label = wait;
    const panel = await mount(source);
    const dialog = await openLabel(panel, "v6");
    (dialog.querySelector("input.label-input") as HTMLInputElement).value = "x";
    dialog.querySelector<HTMLButtonElement>("button.save")?.click();
    expect(dialog.querySelector<HTMLButtonElement>("button.save")?.disabled).toBe(true);
    expect(dialog.querySelector<HTMLButtonElement>("button.cancel")?.disabled).toBe(true);
    wait.open();
    await settle();
  });

  it("closes with Escape or Cancel without a call and returns focus to the menu button", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    const more = row(panel, "v5").querySelector<HTMLButtonElement>("button.more");
    let dialog = await openLabel(panel, "v5");
    dialog.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
    expect(root(panel).activeElement).toBe(more);
    dialog = await openLabel(panel, "v5");
    dialog.querySelector<HTMLButtonElement>("button.cancel")?.click();
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
    expect(source.calls.label).toBe(0);
  });

  it("keeps Tab inside the dialog", async () => {
    const panel = await mount();
    const dialog = await openLabel(panel, "v5");
    const stops = [...dialog.querySelectorAll<HTMLElement>("*")].filter(
      (el) => el instanceof HTMLButtonElement || el instanceof HTMLInputElement,
    );
    expect(stops.map((el) => el.className)).toEqual([
      "icon-button close",
      "input label-input",
      "button secondary large cancel",
      "button primary large save",
    ]);
    (stops[stops.length - 1] as HTMLElement).focus();
    const tab = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
    dialog.dispatchEvent(tab);
    expect(tab.defaultPrevented).toBe(true);
    expect(root(panel).activeElement).toBe(stops[0]);
  });

  it("drops an open label dialog when the file changes", async () => {
    const panel = await mount();
    await openLabel(panel, "v5");
    panel.fileRef = { ...PANEL_FILE, version: "v8" };
    expect(root(panel).querySelector('[role="dialog"]')).toBeNull();
  });

  it("restores afterwards against the new head a label edit published", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    const dialog = await openLabel(panel, "v6");
    (dialog.querySelector("input.label-input") as HTMLInputElement).value = "Sent";
    dialog.querySelector<HTMLButtonElement>("button.save")?.click();
    await settle();
    await settle();
    await choose(panel, "v5", "restore");
    root(panel).querySelector<HTMLButtonElement>("button.restore")?.click();
    await settle();
    expect(source.restores[0]?.revision).toBe("v8");
  });
});
