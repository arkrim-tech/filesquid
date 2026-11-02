/**
 * UI tests for what the upload flow adds to shared elements (UI-1.2, UI-S-028, mockups 52 and
 * 19): the destination picker's import variant and `preselect`, the New menu's separate switch
 * for New folder, and the app shell's `jobs` slot and upload status in the footer.
 */
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  MemoryNavigationSource,
  defineElements,
  type FixtureNode,
  type FsAppShell,
  type FsDestinationPicker,
  type FsNewMenu,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import type { Distribution } from "../../src/distribution";
import { fakeThemeStore } from "../support/theme-fakes";
import { hostFakes } from "../support/host-fakes";

const distribution: Distribution = {
  id: "filesquid",
  productName: "FileSquid",
  version: "0.1.0",
  logo: { src: "/filesquid-mark.svg", srcDark: "/filesquid-mark-dark.svg", alt: "FileSquid logo" },
  labels,
};

/** The options of a shell over the desktop host fakes. */
const shellOptions = () => ({
  distribution,
  client: hostFakes(),
  theme: fakeThemeStore().store,
});

const tree: FixtureNode = {
  node: "r",
  kind: "repository",
  name: null,
  children: [
    {
      node: "l1",
      kind: "library",
      name: "Family archive",
      children: [
        {
          node: "c1",
          kind: "collection",
          name: "Harbour project",
          children: [
            { node: "f1", kind: "folder", name: "Site photos", children: [] },
            { node: "f2", kind: "folder", name: "Drawings" },
          ],
        },
      ],
    },
    { node: "l2", kind: "library", name: "Studio work", children: [] },
  ],
};

beforeEach(() => defineElements());
afterEach(() => document.body.replaceChildren());

/** Let pending promises and renders finish. */
const settle = async () => {
  for (let i = 0; i < 10; i += 1) await new Promise((resolve) => setTimeout(resolve, 0));
};

describe("<fs-destination-picker> for uploads (mockup 52)", () => {
  /** Mount a picker in the import variant. */
  async function mount() {
    const picker = document.createElement("fs-destination-picker") as FsDestinationPicker;
    picker.configure({ labels, source: new MemoryNavigationSource(tree), variant: "import" });
    document.body.append(picker);
    await settle();
    return picker;
  }

  it("marks the host with the variant and says each library row is not a destination", async () => {
    const picker = await mount();
    expect(picker.dataset.variant).toBe("import");
    const notes = [...(picker.shadowRoot?.querySelectorAll(".not-destination") ?? [])];
    expect(notes.map((note) => note.textContent)).toEqual([
      labels.importNotDestination,
      labels.importNotDestination,
    ]);
    const css = picker.shadowRoot?.querySelector("style")?.textContent ?? "";
    expect(css).toContain("li { min-block-size: 1.9375rem; }");
  });

  it("opens the tree down to a node and chooses it, and announces the choice", async () => {
    const picker = await mount();
    const heard: unknown[] = [];
    picker.addEventListener("fs-destination-change", (event) =>
      heard.push((event as CustomEvent).detail),
    );
    expect(await picker.preselect("f1")).toBe(true);
    expect(picker.selected).toMatchObject({
      node: "f1",
      kind: "folder",
      trail: ["Family archive", "Harbour project", "Site photos"],
    });
    expect(heard).toHaveLength(1);
    const row = picker.shadowRoot?.querySelector('li[data-node="f1"]');
    expect(row?.getAttribute("aria-selected")).toBe("true");
  });

  it("chooses nothing for a node that is not a destination", async () => {
    const picker = await mount();
    expect(await picker.preselect("l2")).toBe(false);
    expect(picker.selected).toBeUndefined();
  });
});

describe("<fs-new-menu> and the shell (UI-1.2)", () => {
  it("keeps New folder off while the upload items are on, with the generic reason", async () => {
    const menu = document.createElement("fs-new-menu") as FsNewMenu;
    menu.configure({ labels });
    menu.destination = "c1";
    menu.uploads = true;
    menu.folders = false;
    document.body.append(menu);
    const button = document.createElement("button");
    document.body.append(button);
    button.dispatchEvent(new CustomEvent("fs-new", { bubbles: true, composed: true }));
    const items = [...(menu.shadowRoot?.querySelectorAll<HTMLElement>("[role=menuitem]") ?? [])];
    expect(items.map((item) => item.getAttribute("aria-disabled"))).toEqual([null, null, "true"]);
    expect(items[2]?.title).toBe(labels.fileActionUnavailable);
  });

  it("shows the upload status in the footer in place of the commit state, and restores it", async () => {
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    shell.configure(shellOptions());
    document.body.append(shell);
    const root = shell.shadowRoot as ShadowRoot;
    const state = root.querySelector<HTMLElement>(".upload-state");
    const commit = root.querySelector<HTMLElement>(".commit-state");
    expect(state?.hidden).toBe(true);
    expect(commit?.hidden).toBe(false);
    shell.uploadStatus = "Uploading · 82 of 196";
    expect(state?.hidden).toBe(false);
    expect(state?.textContent).toBe("Uploading · 82 of 196");
    expect(state?.getAttribute("role")).toBe("status");
    expect(commit?.hidden).toBe(true);
    shell.uploadStatus = undefined;
    expect(state?.hidden).toBe(true);
    expect(commit?.hidden).toBe(false);
  });

  it("offers a jobs slot in the header between the host badge and Monitor", async () => {
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    shell.configure(shellOptions());
    document.body.append(shell);
    const header = shell.shadowRoot?.querySelector("header") as HTMLElement;
    const order = [...header.children].map(
      (child) =>
        child.tagName.toLowerCase() +
        (child.getAttribute("name") ? `:${child.getAttribute("name")}` : ""),
    );
    expect(order.slice(-3)).toEqual(["fs-host-indicator", "slot:jobs", "a"]);
  });
});
