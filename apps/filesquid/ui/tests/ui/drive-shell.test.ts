/**
 * UI tests for what surrounds `<fs-drive-view>` (UI-F2.8, UI-S-002, UI-S-006, UI-S-017,
 * UI-S-025, UI-S-033): the destination picker that offers only collections and the folders in
 * them, the New menu with its destination rules, the Browsing section of the Settings page, the
 * Drive routes (`#/files`, `#/recent`) with the footer summary and the sidebar filter, and the
 * file view inside the navigation view of a collection.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  bindDriveRoute,
  defineElements,
  type FsDestinationPicker,
  type FsNewMenu,
  type FsSettings,
  type FsAppShell,
} from "../../src/index";
import { FsNavigationView } from "../../src/components/fs-navigation-view";
import { MemoryNavigationSource, type FixtureNode } from "../../src/navigation/source";
import { ApiError } from "../../src/api/client";
import { PreferencesService } from "../../src/preferences/service";
import { NavigationStore } from "../../src/navigation/store";
import { ROWS, preferences, settle, viewSource, LIBRARY } from "../support/drive-fakes";
import { fakeThemeStore } from "../support/theme-fakes";
import { DESKTOP_DESCRIPTOR } from "../support/host-fakes";
import labels from "../../src/theme/labels.en.json";

beforeEach(() => {
  defineElements();
  vi.spyOn(console, "warn").mockImplementation(() => undefined);
});
/** Route bindings made by a test, undone after it so no `hashchange` listener leaks. */
const stops: (() => void)[] = [];
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  vi.restoreAllMocks();
  document.body.replaceChildren();
  window.location.hash = "";
});

/** One library with a collection holding a folder, a virtual collection and a top-level folder. */
const TREE: FixtureNode = {
  node: "r",
  kind: "repository",
  name: null,
  children: [
    {
      node: "node-library",
      kind: "library",
      name: "Family",
      children: [
        {
          node: "c1",
          kind: "collection",
          name: "Photos",
          children: [{ node: "f1", kind: "folder", name: "2025" }],
        },
        { node: "v1", kind: "virtual_collection", name: "Best of" },
        { node: "f9", kind: "folder", name: "inbox" },
      ],
    },
  ],
};

describe("<fs-destination-picker>", () => {
  /** Mount a picker over the fixture tree. */
  async function mountPicker(current?: string): Promise<FsDestinationPicker> {
    const picker = document.createElement("fs-destination-picker") as FsDestinationPicker;
    picker.configure({ labels, source: new MemoryNavigationSource(TREE), current });
    document.body.append(picker);
    await settle();
    return picker;
  }
  const rows = (picker: Element) => [
    ...(picker.shadowRoot?.querySelectorAll<HTMLElement>("li[data-node]") ?? []),
  ];

  it("offers only collections and the folders inside them, never a library top level or a virtual collection", async () => {
    const picker = await mountPicker();
    const shown = rows(picker).map((li) => [li.textContent?.trim(), li.dataset.selectable]);
    expect(shown).toEqual([
      ["Family", "false"],
      ["Photos", "true"],
    ]);
    (rows(picker)[1]?.querySelector(".chevron") as HTMLElement).click();
    await settle();
    expect(rows(picker).map((li) => li.dataset.selectable)).toEqual(["false", "true", "true"]);
    expect(rows(picker).map((li) => li.textContent)).not.toContain("inbox");
    expect(rows(picker).map((li) => li.textContent)).not.toContain("Best of");
  });

  it("chooses a place, announces it with its trail and marks it selected", async () => {
    const picker = await mountPicker();
    const seen = vi.fn();
    picker.addEventListener("fs-destination-change", (e) => seen((e as CustomEvent).detail));
    rows(picker)[1]?.click();
    expect(seen).toHaveBeenCalledWith({
      node: "c1",
      kind: "collection",
      trail: ["Family", "Photos"],
    });
    expect(rows(picker)[1]?.getAttribute("aria-selected")).toBe("true");
    expect(picker.selected?.node).toBe("c1");
  });

  it("does not choose a library", async () => {
    const picker = await mountPicker();
    const seen = vi.fn();
    picker.addEventListener("fs-destination-change", seen);
    rows(picker)[0]?.click();
    expect(seen).not.toHaveBeenCalled();
  });

  it("marks the current folder from the label table", async () => {
    const picker = await mountPicker("c1");
    expect(rows(picker)[1]?.textContent).toContain(labels.fileCurrentFolder);
  });

  it("follows the tree keys: Down moves, Enter chooses, Right expands", async () => {
    const picker = await mountPicker();
    const list = picker.shadowRoot?.querySelector("ul") as HTMLElement;
    rows(picker)[0]?.focus();
    list.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    expect(picker.shadowRoot?.activeElement).toBe(rows(picker)[1]);
    list.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    await settle();
    expect(rows(picker)).toHaveLength(3);
    list.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    expect(picker.selected?.node).toBe("c1");
  });

  it("has one Tab stop, on the chosen row or else the first, and moves it with focus", async () => {
    const picker = await mountPicker();
    expect(rows(picker).map((li) => li.tabIndex)).toEqual([0, -1]);
    rows(picker)[1]?.focus();
    expect(rows(picker).map((li) => li.tabIndex)).toEqual([-1, 0]);
  });

  it("clears the choice, and says so, when it is configured again", async () => {
    const picker = await mountPicker();
    rows(picker)[1]?.click();
    const seen = vi.fn();
    picker.addEventListener("fs-destination-change", (e) => seen((e as CustomEvent).detail));
    picker.configure({ labels, source: new MemoryNavigationSource(TREE) });
    await settle();
    expect(picker.selected).toBeUndefined();
    expect(seen).toHaveBeenCalledWith(null);
    expect(rows(picker).every((li) => li.getAttribute("aria-selected") === "false")).toBe(true);
  });

  it("does not expand a row that has no children when Right is pressed", async () => {
    const source = new MemoryNavigationSource(TREE);
    const list = vi.spyOn(source, "list");
    const picker = document.createElement("fs-destination-picker") as FsDestinationPicker;
    picker.configure({ labels, source });
    document.body.append(picker);
    await settle();
    (rows(picker)[1]?.querySelector(".chevron") as HTMLElement).click();
    await settle();
    const leaf = rows(picker)[2] as HTMLElement;
    leaf.focus();
    const before = list.mock.calls.length;
    picker.shadowRoot
      ?.querySelector("ul")
      ?.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
    await settle();
    expect(list.mock.calls.length).toBe(before);
    expect(leaf.hasAttribute("aria-expanded")).toBe(false);
  });

  it("keeps the children of every node when two expands overlap", async () => {
    const two: FixtureNode = {
      node: "r",
      kind: "repository",
      name: null,
      children: [
        {
          node: "lib",
          kind: "library",
          name: "Family",
          children: [
            {
              node: "a",
              kind: "collection",
              name: "A",
              children: [{ node: "a1", kind: "folder", name: "A1" }],
            },
            {
              node: "b",
              kind: "collection",
              name: "B",
              children: [{ node: "b1", kind: "folder", name: "B1" }],
            },
          ],
        },
      ],
    };
    const source = new MemoryNavigationSource(two);
    const list = source.list.bind(source);
    const gates = new Map<string, () => void>();
    let hold = false;
    source.list = async (parent, cursor, limit) => {
      if (hold && (parent === "a" || parent === "b")) {
        await new Promise<void>((resolve) => gates.set(parent, resolve));
      }
      return list(parent, cursor, limit);
    };
    const picker = document.createElement("fs-destination-picker") as FsDestinationPicker;
    picker.configure({ labels, source });
    document.body.append(picker);
    await settle();
    hold = true;
    for (const li of rows(picker).slice(1)) (li.querySelector(".chevron") as HTMLElement).click();
    await settle();
    gates.get("a")?.();
    await settle();
    gates.get("b")?.();
    await settle();
    expect(rows(picker).map((li) => li.textContent?.trim())).toEqual([
      "Family",
      "A",
      "A1",
      "B",
      "B1",
    ]);
  });

  it("reads a node's children once when it is expanded twice before the first read ends", async () => {
    const source = new MemoryNavigationSource(TREE);
    const list = source.list.bind(source);
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    let hold = false;
    let reads = 0;
    source.list = async (parent, cursor, limit) => {
      if (hold && parent === "c1") {
        reads += 1;
        await gate;
      }
      return list(parent, cursor, limit);
    };
    const picker = document.createElement("fs-destination-picker") as FsDestinationPicker;
    picker.configure({ labels, source });
    document.body.append(picker);
    await settle();
    hold = true;
    const chevron = rows(picker)[1]?.querySelector(".chevron") as HTMLElement;
    chevron.click();
    chevron.click();
    await settle();
    release();
    await settle();
    expect(reads).toBe(1);
    expect(rows(picker).map((li) => li.textContent?.trim())).toEqual(["Family", "Photos", "2025"]);
  });

  it("shows the error state with Try again when a lazy read of a node fails", async () => {
    const source = new MemoryNavigationSource(TREE);
    const list = source.list.bind(source);
    let fail = false;
    source.list = (parent, cursor, limit) =>
      fail && parent === "c1" ? Promise.reject(new Error("down")) : list(parent, cursor, limit);
    const picker = document.createElement("fs-destination-picker") as FsDestinationPicker;
    picker.configure({ labels, source, online: () => true });
    document.body.append(picker);
    await settle();
    fail = true;
    (rows(picker)[1]?.querySelector(".chevron") as HTMLElement).click();
    await settle();
    expect(picker.dataset.state).toBe("error");
  });

  it("ignores a read that finishes after a newer one started", async () => {
    const stale = new MemoryNavigationSource({
      ...TREE,
      children: [{ node: "node-library", kind: "library", name: "Old", children: [] }],
    });
    const fresh = new MemoryNavigationSource(TREE);
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    let gated = false;
    const source = {
      ...fresh,
      list: async (parent: string | null, cursor?: string | null, limit?: number) => {
        if (parent !== "r" || gated) return fresh.list(parent, cursor, limit);
        gated = true;
        await gate;
        return stale.list(parent, cursor, limit);
      },
    };
    const picker = document.createElement("fs-destination-picker") as FsDestinationPicker;
    picker.configure({ labels, source: source as never });
    document.body.append(picker);
    await picker.load();
    release();
    await settle();
    (rows(picker)[1]?.querySelector(".chevron") as HTMLElement).click();
    await settle();
    expect(rows(picker).map((li) => li.textContent?.trim())).toEqual(["Family", "Photos", "2025"]);
  });

  it("shows the error state with Try again when the listing fails", async () => {
    const source = new MemoryNavigationSource(TREE);
    source.failure = new Error("down");
    const picker = document.createElement("fs-destination-picker") as FsDestinationPicker;
    picker.configure({ labels, source, online: () => true });
    document.body.append(picker);
    await settle();
    expect(picker.dataset.state).toBe("error");
  });
});

describe("<fs-new-menu>", () => {
  /** Mount a menu and a stand-in for the shell's New button. */
  function mountMenu(): { menu: FsNewMenu; button: HTMLButtonElement } {
    const menu = document.createElement("fs-new-menu") as FsNewMenu;
    menu.configure({ labels });
    const button = document.createElement("button");
    document.body.append(menu, button);
    return { menu, button };
  }
  const items = (menu: Element) => [
    ...(menu.shadowRoot?.querySelectorAll<HTMLElement>("[role=menuitem]") ?? []),
  ];
  const pressNew = (button: Element) =>
    button.dispatchEvent(new CustomEvent("fs-new", { bubbles: true, composed: true }));

  it("opens below the New button with Upload files…, Upload folder… and New folder, shortcuts at the right", async () => {
    const { menu, button } = mountMenu();
    pressNew(button);
    expect(items(menu).map((i) => i.querySelector(".label")?.textContent)).toEqual([
      labels.fileUploadFiles,
      labels.fileUploadFolder,
      labels.fileNewFolder,
    ]);
    expect(items(menu).map((i) => i.querySelector(".shortcut")?.textContent ?? "")).toEqual([
      "⌘O",
      "",
      "⇧⌘N",
    ]);
    expect(menu.shadowRoot?.querySelector("[role=menu]")?.getAttribute("aria-label")).toBe(
      labels.fileNewMenu,
    );
  });

  it("says the host cannot place uploads, not where to go, while uploads are not possible anywhere", () => {
    const { menu, button } = mountMenu();
    pressNew(button);
    expect(items(menu).every((i) => i.title === labels.fileActionUnavailable)).toBe(true);
  });

  it("disables every item with the label-table reason where no collection or folder is shown", () => {
    const { menu, button } = mountMenu();
    menu.uploads = true;
    pressNew(button);
    expect(items(menu).every((i) => i.getAttribute("aria-disabled") === "true")).toBe(true);
    expect(items(menu)[0]?.title).toBe(labels.fileUploadDisabledReason);
  });

  it("disables the items with the generic reason in a collection while the host cannot place uploads", () => {
    const { menu, button } = mountMenu();
    menu.destination = "c1";
    pressNew(button);
    expect(items(menu)[0]?.getAttribute("aria-disabled")).toBe("true");
    expect(items(menu)[0]?.title).toBe(labels.fileActionUnavailable);
  });

  it("enables the items in a collection once uploads are possible and announces the choice", () => {
    const { menu, button } = mountMenu();
    menu.destination = "c1";
    menu.uploads = true;
    const seen = vi.fn();
    menu.addEventListener("fs-new-action", (e) => seen((e as CustomEvent).detail));
    pressNew(button);
    items(menu)[2]?.click();
    expect(seen).toHaveBeenCalledWith({ action: "new-folder", destination: "c1" });
    expect(menu.shadowRoot?.querySelector("[role=menu]")).toBeNull();
  });

  it("closes on a second press of New", () => {
    const { menu, button } = mountMenu();
    pressNew(button);
    pressNew(button);
    expect(menu.shadowRoot?.querySelector("[role=menu]")).toBeNull();
  });
});

describe("Settings, Browsing", () => {
  /** Mount a Settings page with preferences. */
  async function mountSettings(prefs?: PreferencesService): Promise<FsSettings> {
    const settings = document.createElement("fs-settings") as FsSettings;
    settings.configure({
      labels,
      theme: fakeThemeStore().store,
      distribution: {
        id: "filesquid",
        productName: "FileSquid",
        version: "0.1.0",
        logo: { src: "/a.svg", alt: "x" },
        labels,
      },
      contractVersion: "0.30.0",
      client: {
        health: async () => ({ schema_version: 1, status: "ok", product: "filesquid" }),
        host: async () => DESKTOP_DESCRIPTOR,
        subscribe: () => () => undefined,
      },
      preferences: prefs,
    } as never);
    document.body.append(settings);
    await settle();
    return settings;
  }

  it("has no Browsing section without preferences", async () => {
    const settings = await mountSettings();
    expect(settings.shadowRoot?.querySelector("[data-section=browsing]")).toBeNull();
  });

  it("shows Browsing between Appearance and Privacy with the saved choices", async () => {
    const { service } = await preferences({
      layout: "grid",
      sort_key: "size",
      sort_direction: "descending",
      hover_card: false,
    });
    const settings = await mountSettings(service);
    const headings = [...(settings.shadowRoot?.querySelectorAll("h3.section-title") ?? [])].map(
      (h) => h.textContent,
    );
    expect(headings.slice(0, 3)).toEqual([
      labels.settingsAppearance,
      labels.settingsBrowsing,
      labels.settingsPrivacy,
    ]);
    const select = (id: string) => settings.shadowRoot?.querySelector<HTMLSelectElement>(`#${id}`);
    expect(select("pref-layout")?.value).toBe("grid");
    expect(select("pref-sort")?.value).toBe("size:descending");
    expect(settings.shadowRoot?.querySelector<HTMLInputElement>("#pref-hover")?.checked).toBe(
      false,
    );
    expect(settings.shadowRoot?.querySelector(".pref-note.help")?.textContent).toBe(
      labels.settingsBrowsingHelp,
    );
  });

  it("saves a changed choice through the preferences at once", async () => {
    const { service, source } = await preferences();
    const settings = await mountSettings(service);
    const sort = settings.shadowRoot?.querySelector<HTMLSelectElement>(
      "#pref-sort",
    ) as HTMLSelectElement;
    sort.value = "modified:descending";
    sort.dispatchEvent(new Event("change", { bubbles: true }));
    await service.settled();
    expect(source.writes.at(-1)).toMatchObject({
      sort_key: "modified",
      sort_direction: "descending",
    });
    const start = settings.shadowRoot?.querySelector<HTMLSelectElement>(
      "#pref-start",
    ) as HTMLSelectElement;
    start.value = "personal_root";
    start.dispatchEvent(new Event("change", { bubbles: true }));
    await service.settled();
    expect(source.writes.at(-1)?.start_view).toBe("personal_root");
  });

  it("stops following the old preferences when it is configured with others", async () => {
    const { service: first } = await preferences();
    const { service: second } = await preferences({ layout: "grid" });
    const settings = await mountSettings(first);
    const release = vi.spyOn(first, "removeEventListener");
    settings.configure({
      ...(settings as unknown as { options: object }).options,
      preferences: second,
    } as never);
    expect(release).toHaveBeenCalledWith("change", expect.any(Function));
    const layout = settings.shadowRoot?.querySelector<HTMLSelectElement>("#pref-layout");
    expect(layout?.value).toBe("grid");
  });

  it("says the choices apply until the page closes when the host stores none", async () => {
    const service = new PreferencesService({
      source: {
        read: () =>
          Promise.reject(new ApiError(503, "/api/v1/preferences", { code: "unavailable" })),
        write: () =>
          Promise.reject(new ApiError(503, "/api/v1/preferences", { code: "unavailable" })),
      },
      navigation: new NavigationStore({}),
    });
    await service.start();
    const settings = await mountSettings(service);
    expect(settings.shadowRoot?.querySelector(".pref-status")?.textContent).toBe(
      labels.settingsBrowsingDeviceOnly,
    );
  });

  it("says the preferences could not be read, not that the host keeps none, after another failure", async () => {
    const service = new PreferencesService({
      source: {
        read: () =>
          Promise.reject(new ApiError(401, "/api/v1/preferences", { code: "unauthorized" })),
        write: () => Promise.reject(new Error("no")),
      },
      navigation: new NavigationStore({}),
    });
    await service.start();
    const settings = await mountSettings(service);
    expect(settings.shadowRoot?.querySelector(".pref-status")?.textContent).toBe(
      labels.settingsBrowsingUnreadable,
    );
  });

  it("uses the saved sort labels in every option", async () => {
    const { service } = await preferences();
    const settings = await mountSettings(service);
    const options = [
      ...(settings.shadowRoot?.querySelectorAll<HTMLOptionElement>("#pref-sort option") ?? []),
    ].map((o) => o.textContent);
    expect(options).toHaveLength(10);
    expect(options[0]).toBe("Name, A to Z");
  });
});

describe("Drive routes", () => {
  /** A shell with the route bound over the fixture library. */
  async function mountRoute(hash: string) {
    window.location.hash = hash;
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    shell.configure({
      distribution: {
        id: "filesquid",
        productName: "FileSquid",
        version: "0.1.0",
        logo: { src: "/a.svg", alt: "x" },
        labels,
      },
      client: { host: async () => DESKTOP_DESCRIPTOR, subscribe: () => () => undefined } as never,
      theme: fakeThemeStore().store,
    });
    document.body.append(shell);
    const { service } = await preferences();
    const source = viewSource();
    const newMenu = document.createElement("fs-new-menu") as FsNewMenu;
    newMenu.configure({ labels });
    shell.append(newMenu);
    const stop = bindDriveRoute(shell, {
      labels,
      navigation: new MemoryNavigationSource(TREE),
      drive: { source, preferences: service },
      newMenu,
    });
    stops.push(stop);
    await settle();
    await settle();
    return { shell, source, stop, newMenu };
  }

  it("shows All files over the first library, titled from the label table, with the footer summary", async () => {
    const { shell, source } = await mountRoute("#/files");
    const view = shell.querySelector("fs-drive-view");
    expect(view?.shadowRoot?.querySelector(".page-title")?.textContent).toBe(labels.fileRoot);
    expect(source.calls[0]?.query).toMatchObject({ node: "node-library", view: "all" });
    expect(shell.querySelector('[slot="summary"]')?.textContent).toBe(
      "Folders: 2 · Files: 4 · Loaded size: 25.1 MB",
    );
    expect(shell.querySelector('[slot="commit-state"]')?.textContent).toBe(labels.fileCommitted);
  });

  it("shows Recent with its fixed order, which says so when the host serves no recent view", async () => {
    const { shell } = await mountRoute("#/recent");
    const view = shell.querySelector("fs-drive-view") as HTMLElement;
    expect(view.shadowRoot?.querySelector(".page-title")?.textContent).toBe(labels.fileRecent);
    expect(view.dataset.state).toBe("unavailable");
  });

  it("narrows only the current view with the sidebar filter", async () => {
    const { shell, source } = await mountRoute("#/files");
    const filter = shell.shadowRoot?.querySelector<HTMLInputElement>(
      "input.filter",
    ) as HTMLInputElement;
    filter.value = "survey";
    filter.dispatchEvent(new Event("input", { bubbles: true }));
    await settle();
    expect(source.calls.at(-1)?.query.filter).toBe("survey");
  });

  it("removes the view and clears the footer when the route is left", async () => {
    const { shell } = await mountRoute("#/files");
    window.location.hash = "#/settings";
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await settle();
    expect(shell.querySelector("fs-drive-view")).toBeNull();
    expect(shell.querySelector('[slot="summary"]')?.textContent).toBe("");
  });

  it("forgets the New menu's destination on every route change, so Upload cannot target a place no longer shown", async () => {
    const { newMenu } = await mountRoute("#/files");
    newMenu.destination = "c1";
    window.location.hash = "#/recent";
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(newMenu.destination).toBeUndefined();
  });

  it("ignores a library lookup that finishes after the route changed", async () => {
    window.location.hash = "#/files";
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    document.body.append(shell);
    const { service } = await preferences();
    const navigation = new MemoryNavigationSource(TREE);
    const list = navigation.list.bind(navigation);
    let fail: (reason: unknown) => void = () => undefined;
    let first = true;
    navigation.list = (parent, cursor, limit) => {
      if (parent !== null || !first) return list(parent, cursor, limit);
      first = false;
      return new Promise((_, reject) => (fail = reject));
    };
    stops.push(
      bindDriveRoute(shell, {
        labels,
        navigation,
        drive: { source: viewSource(), preferences: service },
      }),
    );
    window.location.hash = "#/recent";
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await settle();
    await settle();
    const view = shell.querySelector<HTMLElement>("fs-drive-view");
    const before = view?.dataset.state;
    fail(new Error("late"));
    await settle();
    expect(view?.dataset.state).toBe(before);
    expect(view?.dataset.state).not.toBe("error");
  });

  it("says the node is gone when the host lists no library", async () => {
    window.location.hash = "#/files";
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    document.body.append(shell);
    const { service } = await preferences();
    stops.push(
      bindDriveRoute(shell, {
        labels,
        navigation: new MemoryNavigationSource({
          node: "r",
          kind: "repository",
          name: null,
          children: [],
        }),
        drive: { source: viewSource(), preferences: service },
      }),
    );
    await settle();
    await settle();
    expect(shell.querySelector<HTMLElement>("fs-drive-view")?.dataset.state).toBe("empty");
  });

  it("opens a file in the file browser at its own object", async () => {
    const { shell } = await mountRoute("#/files");
    const view = shell.querySelector("fs-drive-view") as HTMLElement;
    view.dispatchEvent(
      new CustomEvent("fs-drive-open", {
        detail: { row: { ...ROWS[3], libraryId: LIBRARY }, intent: "open" },
        bubbles: true,
        composed: true,
      }),
    );
    expect(window.location.hash).toBe(`#/files/${LIBRARY}/urn%3Auuid%3Af-xlsx/original`);
  });
});

describe("<fs-navigation-view> with the file view", () => {
  /** A navigation view for `node` over the fixture tree with drive options. */
  async function mountView(node: string, withDrive = true): Promise<FsNavigationView> {
    const view = document.createElement("fs-navigation-view") as FsNavigationView;
    const { service } = await preferences();
    view.configure({
      labels,
      source: new MemoryNavigationSource(TREE),
      drive: withDrive ? { source: viewSource(), preferences: service } : undefined,
    });
    document.body.append(view);
    view.node = node;
    await settle();
    await settle();
    return view;
  }
  const driveOf = (view: Element) =>
    view.shadowRoot?.querySelector("fs-drive-view") as HTMLElement | null;

  it("shows the file view with the node's name for a library", async () => {
    const view = await mountView("node-library");
    expect(driveOf(view)?.shadowRoot?.querySelector(".page-title")?.textContent).toBe("Family");
    expect(driveOf(view)?.dataset.state).toBe("ready");
  });

  it("tells the shell which node is shown, so the New menu knows where uploads would go", async () => {
    const seen = vi.fn();
    document.body.addEventListener("fs-destination-context", (e) =>
      seen((e as CustomEvent).detail),
    );
    await mountView("c1");
    expect(seen).toHaveBeenLastCalledWith({ node: "c1", kind: "collection", name: "Photos" });
  });

  it("keeps the files note for a folder, which the view operation does not list", async () => {
    const view = await mountView("f1");
    expect(driveOf(view)).toBeNull();
    expect(view.shadowRoot?.textContent).toContain(labels.navigationFilesPending);
  });

  it("keeps the files note when the host gives no file view options", async () => {
    const view = await mountView("node-library", false);
    expect(driveOf(view)).toBeNull();
    expect(view.shadowRoot?.textContent).toContain(labels.navigationFilesPending);
  });

  it("hands the sidebar filter to the file view", async () => {
    const view = (await mountView("node-library")) as FsNavigationView;
    view.filter = "budget";
    await settle();
    expect((driveOf(view) as unknown as { filter: string }).filter).toBe("budget");
  });
});
