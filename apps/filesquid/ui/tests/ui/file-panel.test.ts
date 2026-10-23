/**
 * UI tests for `<fs-file-panel>`, the file panel frame (UI-F2.3, UI-S-003, UI-S-031): the header
 * with the file-type badge, the display name, the history button and close; tabs that are
 * registered panels in named slots, independent of each other, with the WAI-ARIA tabs keys; the
 * Versions view shown in place of the tabs; and the file handed to every panel.
 */
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  accessibilityViolations,
  defineElements,
  type FilePanel,
  type FsFilePanel,
} from "../../src/index";
import type { FileRef } from "../../src/files/model";
import labels from "../../src/theme/labels.en.json";
import { PANEL_FILE } from "../support/panel-fakes";

beforeEach(() => defineElements());

/** A stand-in panel that records the file it is given. */
function stub(name: string): FilePanel & { seen: (FileRef | null | undefined)[] } {
  const element = document.createElement("div") as unknown as FilePanel & {
    seen: (FileRef | null | undefined)[];
  };
  element.className = name;
  element.textContent = name;
  element.seen = [];
  Object.defineProperty(element, "fileRef", {
    set(value: FileRef | null) {
      element.seen.push(value);
    },
    get() {
      return element.seen[element.seen.length - 1];
    },
  });
  return element;
}

/** Mount a file panel with two tabs and a Versions view. */
function mount() {
  const panel = document.createElement("fs-file-panel") as FsFilePanel;
  panel.configure({ labels });
  const info = stub("info");
  const other = stub("other");
  const versions = stub("versions");
  panel.registerTab("details", labels.filePanelInfo, () => info);
  panel.registerTab("other", "Other", () => other);
  panel.setVersions(() => versions);
  document.body.replaceChildren(panel);
  panel.fileRef = PANEL_FILE;
  return { panel, info, other, versions, root: panel.shadowRoot as ShadowRoot };
}

describe("<fs-file-panel>", () => {
  it("is a defined element with the shared stylesheet first", () => {
    expect(customElements.get("fs-file-panel")).toBeDefined();
    const { root } = mount();
    expect(root.firstElementChild?.tagName).toBe("STYLE");
  });

  it("shows the badge, the display name, the history button and close in the header", () => {
    const { root } = mount();
    const badge = root.querySelector(".badge") as HTMLElement;
    expect(badge.textContent).toBe("PDF");
    expect(badge.dataset.kind).toBe("document");
    const name = root.querySelector("h2.name") as HTMLElement;
    expect(name.textContent).toBe("harbour-masterplan-v7.pdf");
    expect(name.getAttribute("title")).toBe("harbour-masterplan-v7.pdf");
    expect(root.querySelector("button.history")?.getAttribute("aria-label")).toBe(
      labels.filePanelHistory,
    );
    expect(root.querySelector("button.history")?.getAttribute("aria-pressed")).toBe("false");
    expect(root.querySelector("button.close")?.getAttribute("aria-label")).toBe(
      labels.filePanelClose,
    );
    expect(root.querySelector("[role='complementary'], aside")).toBeNull();
    expect(accessibilityViolations(root.host as HTMLElement)).toEqual([]);
  });

  it("puts each registered panel in its own named slot and tab, the first selected", () => {
    const { root, info, other } = mount();
    const tablist = root.querySelector('[role="tablist"]') as HTMLElement;
    expect(tablist.getAttribute("aria-label")).toBe(labels.filePanelTabs);
    const tabs = [...root.querySelectorAll<HTMLElement>('[role="tab"]')];
    expect(tabs.map((t) => t.textContent)).toEqual([labels.filePanelInfo, "Other"]);
    expect(tabs.map((t) => t.getAttribute("aria-selected"))).toEqual(["true", "false"]);
    expect(tabs.map((t) => t.tabIndex)).toEqual([0, -1]);
    const panels = [...root.querySelectorAll<HTMLElement>('[role="tabpanel"]')];
    expect(panels.map((p) => p.dataset.slot)).toEqual(["details", "other"]);
    expect(panels.map((p) => p.hidden)).toEqual([false, true]);
    expect(panels[0]?.getAttribute("aria-labelledby")).toBe(tabs[0]?.id);
    expect(tabs[0]?.getAttribute("aria-controls")).toBe(panels[0]?.id);
    expect(panels[0]?.querySelector('section[data-slot="details"]')?.contains(info)).toBe(true);
    expect(other.isConnected).toBe(false);
  });

  it("creates and hands the file only to the panel shown, and no panel a reference to another", () => {
    const { root, info, other, versions } = mount();
    expect(info.seen).toEqual([PANEL_FILE]);
    expect(other.seen).toEqual([]);
    expect(versions.seen).toEqual([]);
    root.querySelectorAll<HTMLElement>('[role="tab"]')[1]?.click();
    expect(other.seen).toEqual([PANEL_FILE]);
    const second = { ...PANEL_FILE, version: "v8" };
    (root.host as FsFilePanel).fileRef = second;
    expect(other.seen).toEqual([PANEL_FILE, second]);
    expect(info.seen).toEqual([PANEL_FILE]);
    root.querySelectorAll<HTMLElement>('[role="tab"]')[0]?.click();
    expect(info.seen).toEqual([PANEL_FILE, second]);
    root.querySelector<HTMLButtonElement>("button.history")?.click();
    expect(versions.seen).toEqual([second]);
    expect(Object.values(info)).not.toContain(other);
  });

  it("moves between tabs with the arrow, Home and End keys and selects on click", () => {
    const { root } = mount();
    const tabs = [...root.querySelectorAll<HTMLElement>('[role="tab"]')];
    tabs[0]?.focus();
    tabs[0]?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    expect(tabs[1]?.getAttribute("aria-selected")).toBe("true");
    expect(root.activeElement).toBe(tabs[1]);
    tabs[1]?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    expect(tabs[0]?.getAttribute("aria-selected")).toBe("true");
    tabs[0]?.dispatchEvent(new KeyboardEvent("keydown", { key: "End", bubbles: true }));
    expect(tabs[1]?.getAttribute("aria-selected")).toBe("true");
    tabs[1]?.dispatchEvent(new KeyboardEvent("keydown", { key: "Home", bubbles: true }));
    expect(tabs[0]?.getAttribute("aria-selected")).toBe("true");
    tabs[1]?.click();
    expect(tabs[1]?.getAttribute("aria-selected")).toBe("true");
    expect(
      [...root.querySelectorAll<HTMLElement>('[role="tabpanel"]')].map((p) => p.hidden),
    ).toEqual([true, false]);
  });

  it("shows the Versions view in place of the tabs from the history button, and Back returns", () => {
    const { root, versions } = mount();
    const history = root.querySelector<HTMLButtonElement>("button.history") as HTMLButtonElement;
    history.click();
    expect(history.getAttribute("aria-pressed")).toBe("true");
    expect((root.querySelector(".tabs") as HTMLElement).hidden).toBe(true);
    const view = root.querySelector(".versions-view") as HTMLElement;
    expect(view.hidden).toBe(false);
    expect(view.contains(versions)).toBe(true);
    versions.dispatchEvent(new CustomEvent("fs-versions-close", { bubbles: true, composed: true }));
    expect(history.getAttribute("aria-pressed")).toBe("false");
    expect((root.querySelector(".tabs") as HTMLElement).hidden).toBe(false);
    expect(view.hidden).toBe(true);
    expect(root.activeElement).toBe(history);
  });

  it("dispatches fs-file-panel-close from close", () => {
    const { panel, root } = mount();
    const closed = vi.fn();
    document.addEventListener("fs-file-panel-close", closed, { once: true });
    root.querySelector<HTMLButtonElement>("button.close")?.click();
    expect(closed).toHaveBeenCalledTimes(1);
    expect(panel.isConnected).toBe(true);
  });

  it("replaces a tab registered again under the same id", () => {
    const { panel, root } = mount();
    const again = stub("again");
    panel.registerTab("details", "Again", () => again);
    const tabs = [...root.querySelectorAll<HTMLElement>('[role="tab"]')];
    expect(tabs.map((t) => t.textContent)).toEqual(["Again", "Other"]);
    expect(again.seen).toContainEqual(PANEL_FILE);
  });

  it("follows a new file and clears the header with none", () => {
    const { panel, root, info } = mount();
    panel.fileRef = { ...PANEL_FILE, version: "v8", display_name: "facade-study.jpg" };
    expect(root.querySelector("h2.name")?.textContent).toBe("facade-study.jpg");
    expect((root.querySelector(".badge") as HTMLElement).dataset.kind).toBe("media");
    panel.fileRef = null;
    expect(root.querySelector("h2.name")?.textContent).toBe("");
    expect(info.seen[info.seen.length - 1]).toBeNull();
  });
});
