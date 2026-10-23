/**
 * UI tests for <fs-app-shell> (UI-S-025, UI-S-015, UI-S-016): strict DOM assertions on the v3
 * frame the shell renders for a distribution, with a fake API client: the 240 px sidebar with
 * the brand, the New, filter, views and navigation slots, the storage block and the Settings
 * and Logs rows; the header with the search slot, the host badge and the Monitor button and
 * without the theme control or any API status (owner answers 7 and 12); the footer slots; the
 * file panel region; and the frameless welcome mode.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  FsAppShell,
  accessibilityViolations,
  defineElements,
  type FsSettings,
} from "../../src/index";
import type { Distribution } from "../../src/distribution";
import labels from "../../src/theme/labels.en.json";
import type { ThemeStore } from "../../src/theme/theme-store";
import { fakeThemeStore } from "../support/theme-fakes";
import { DESKTOP_DESCRIPTOR, hostFakes } from "../support/host-fakes";

const distribution: Distribution = {
  id: "filesquid",
  productName: "FileSquid",
  version: "0.1.0",
  logo: { src: "/filesquid-mark.svg", srcDark: "/filesquid-mark-dark.svg", alt: "FileSquid logo" },
  labels,
};

/** Wait for the shell's asynchronous host read to settle. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Mount a shell with the desktop host fakes. */
function mount(theme: ThemeStore = fakeThemeStore().store, dist: Distribution = distribution) {
  const shell = document.createElement("fs-app-shell") as FsAppShell;
  shell.configure({ distribution: dist, client: hostFakes(), theme });
  document.body.replaceChildren(shell);
  return shell;
}

/** The shell's stylesheet text. */
const styleOf = (shell: FsAppShell) => shell.shadowRoot?.querySelector("style")?.textContent ?? "";

/** The texts of the elements matching `selector` in the shell's shadow root. */
function texts(shell: FsAppShell, selector: string): string[] {
  return [...(shell.shadowRoot?.querySelectorAll(selector) ?? [])].map(
    (el) => el.textContent?.trim() ?? "",
  );
}

describe("<fs-app-shell>", () => {
  beforeEach(() => defineElements());
  afterEach(() => vi.restoreAllMocks());

  it("is registered as a custom element", () => {
    expect(customElements.get("fs-app-shell")).toBe(FsAppShell);
  });

  it("renders the sidebar brand: the 24 px logo mark and the product name as the only h1 (UI-S-025)", async () => {
    const shell = mount();
    await settle();
    const root = shell.shadowRoot as ShadowRoot;
    const brand = root.querySelector("nav .brand");
    expect(brand?.querySelector("img")?.getAttribute("src")).toBe("/filesquid-mark.svg");
    expect(brand?.querySelector("img")?.getAttribute("alt")).toBe("FileSquid logo");
    expect(root.querySelectorAll("h1")).toHaveLength(1);
    expect(brand?.querySelector("h1")?.textContent).toBe("FileSquid");
    expect(styleOf(shell)).toMatch(/\.brand img \{[^}]*block-size: var\(--fs-space-4\)/);
    expect(styleOf(shell)).toMatch(
      /h1 \{[^}]*font-family: var\(--fs-font-heading\)[^}]*font-size: var\(--fs-font-size-lead\)[^}]*font-weight: 700/,
    );
    expect(styleOf(shell)).toMatch(
      /nav \{[^}]*inline-size: 15rem[^}]*background: var\(--fs-surface-card\)/,
    );
  });

  it("offers the New, filter and views slots with their fallbacks worded from the label table", async () => {
    const shell = mount();
    await settle();
    const root = shell.shadowRoot as ShadowRoot;
    const newButton = root.querySelector<HTMLButtonElement>('slot[name="new"] button.new');
    expect(newButton?.textContent?.trim()).toBe(labels.fileNew);
    expect(newButton?.classList.contains("primary")).toBe(true);
    const filter = root.querySelector<HTMLInputElement>('slot[name="filter"] input.filter');
    expect(filter?.placeholder).toBe(labels.fileFilterView);
    expect(filter?.getAttribute("aria-label")).toBe(labels.fileFilterView);
    expect(texts(shell, 'slot[name="views"] ul.views a')).toEqual([
      labels.fileRoot,
      labels.fileRecent,
    ]);
    const slotted = document.createElement("button");
    slotted.slot = "new";
    shell.append(slotted);
    const slot = root.querySelector<HTMLSlotElement>('slot[name="new"]');
    expect(slot?.assignedElements()).toEqual([slotted]);
  });

  it("dispatches fs-new when the fallback New button is pressed, for the menu a later task adds", async () => {
    const shell = mount();
    await settle();
    const seen = vi.fn();
    shell.addEventListener("fs-new", seen);
    shell.shadowRoot?.querySelector<HTMLButtonElement>("button.new")?.click();
    expect(seen).toHaveBeenCalledOnce();
  });

  it("offers a navigation slot in its nav, with the static destinations as fallback (UI-F2.1a)", async () => {
    const shell = mount();
    await settle();
    const root = shell.shadowRoot as ShadowRoot;
    const slot = root.querySelector<HTMLSlotElement>('nav slot[name="navigation"]');
    expect(slot).not.toBeNull();
    expect([...(slot?.querySelectorAll("li") ?? [])].map((li) => li.textContent)).toEqual([
      "Libraries",
      "Collections",
      "Virtual collections",
      "Virtual exhibits",
    ]);
    const tree = document.createElement("div");
    tree.slot = "navigation";
    shell.append(tree);
    expect(slot?.assignedElements()).toEqual([tree]);
  });

  it("shows the storage block only once a summary is given, worded from the label table", async () => {
    const shell = mount();
    await settle();
    const root = shell.shadowRoot as ShadowRoot;
    const block = root.querySelector<HTMLElement>(".storage");
    expect(block?.querySelector(".storage-title")?.textContent).toBe(labels.fileStorageUsed);
    expect(block?.querySelector<HTMLElement>(".storage-facts")?.hidden).toBe(true);
    shell.storage = {
      usedBytes: 38.4 * 1e9,
      historyBytes: 4.2 * 1e9,
      freeBytes: 1.4 * 1e12,
    };
    expect(block?.querySelector<HTMLElement>(".storage-facts")?.hidden).toBe(false);
    expect(block?.querySelector(".storage-value")?.textContent).toBe("38.4 GB");
    expect(texts(shell, ".storage-caption")).toEqual([
      "incl. 4.2 GB history",
      "1.4 TB free on disk",
    ]);
    const bar = block?.querySelector<HTMLElement>(".storage-fill");
    expect(bar?.style.inlineSize).toBe("2.7%");
    shell.storage = undefined;
    expect(block?.querySelector<HTMLElement>(".storage-facts")?.hidden).toBe(true);
  });

  it("lists the library, Settings and Logs rows after the tree; Logs only on the desktop host", async () => {
    const shell = mount();
    await settle();
    const root = shell.shadowRoot as ShadowRoot;
    expect(texts(shell, "nav ul.tools a")).toEqual([
      labels.libraryNavLink,
      labels.settings,
      labels.logs,
    ]);
    expect(root.querySelector('nav ul.tools a[href="#/settings"]')).not.toBeNull();
    const logs = root.querySelector<HTMLElement>("nav ul.tools li.logs");
    expect(logs?.hidden).toBe(false);
    expect(logs?.querySelector("a")?.getAttribute("href")).toBe("#/logs");
    for (const row of root.querySelectorAll("nav ul.tools a, nav ul.views a")) {
      expect(row.querySelector("svg.icon")).not.toBeNull();
    }
  });

  it("hides the Logs row on a server host (UI-S-021)", async () => {
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    shell.configure({
      distribution,
      client: hostFakes({ ...DESKTOP_DESCRIPTOR, host_type: "server", operator_name: "Op" }),
      theme: fakeThemeStore().store,
    });
    document.body.replaceChildren(shell);
    await settle();
    expect(shell.shadowRoot?.querySelector<HTMLElement>("nav ul.tools li.logs")?.hidden).toBe(true);
  });

  it("marks the row of the current route (Settings) as current", async () => {
    window.location.hash = "#/settings";
    const shell = mount();
    await settle();
    const root = shell.shadowRoot as ShadowRoot;
    expect(root.querySelector('a[href="#/settings"]')?.getAttribute("aria-current")).toBe("page");
    expect(root.querySelector('a[href="#/monitor"]')?.getAttribute("aria-current")).toBeNull();
    window.location.hash = "#/monitor";
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(root.querySelector('a[href="#/settings"]')?.getAttribute("aria-current")).toBeNull();
    expect(root.querySelector('a[href="#/monitor"]')?.getAttribute("aria-current")).toBe("page");
    window.location.hash = "";
  });

  it("puts the search slot, the host badge and the Monitor button in the 60 px header", async () => {
    const shell = mount();
    await settle();
    const root = shell.shadowRoot as ShadowRoot;
    const header = root.querySelector("header");
    const search = header?.querySelector<HTMLInputElement>('slot[name="search"] input.search');
    expect(search?.placeholder).toBe(labels.searchPlaceholder);
    expect(search?.getAttribute("type")).toBe("search");
    expect(header?.querySelector("fs-host-indicator")).not.toBeNull();
    const monitor = header?.querySelector<HTMLAnchorElement>("a.monitor");
    expect(monitor?.textContent?.trim()).toBe(labels.monitor);
    expect(monitor?.getAttribute("href")).toBe("#/monitor");
    expect(monitor?.classList.contains("secondary")).toBe(true);
    expect(styleOf(shell)).toMatch(/header \{[^}]*min-block-size: 3\.75rem/);
    expect(styleOf(shell)).toMatch(/footer \{[^}]*min-block-size: 2\.5rem/);
  });

  it("shows no theme control, no API status and no privacy text in the header (owner answers 7 and 12)", async () => {
    const shell = mount();
    await settle();
    const root = shell.shadowRoot as ShadowRoot;
    expect(root.querySelector("fs-theme-control")).toBeNull();
    expect(root.querySelector(".api")).toBeNull();
    expect(root.querySelector('header [role="status"]')).toBeNull();
    expect(root.querySelector("header")?.textContent).not.toContain(labels.apiStatus);
    expect(root.querySelector("header")?.textContent).not.toContain(labels.hostDesktop);
  });

  it("shows the host privacy indicator in the header on every view", async () => {
    const shell = mount();
    await settle();
    const indicator = shell.shadowRoot?.querySelector("header fs-host-indicator");
    expect(indicator?.getAttribute("data-state")).toBe("ready");
    expect(indicator?.shadowRoot?.querySelector("button.badge .label")?.textContent).toBe(
      labels.hostBadgeDesktop,
    );
    expect(indicator?.shadowRoot?.querySelector<HTMLElement>('[role="dialog"]')?.hidden).toBe(true);
  });

  it("has footer slots for the summary and the commit state", async () => {
    const shell = mount();
    await settle();
    const root = shell.shadowRoot as ShadowRoot;
    expect(root.querySelector('footer slot[name="summary"]')).not.toBeNull();
    expect(root.querySelector('footer slot[name="commit-state"]')).not.toBeNull();
    const state = document.createElement("span");
    state.slot = "commit-state";
    state.textContent = labels.fileCommitted;
    shell.append(state);
    expect(
      root.querySelector<HTMLSlotElement>('footer slot[name="commit-state"]')?.assignedElements(),
    ).toEqual([state]);
  });

  it("shows the 320 px file panel region only while something is slotted into it", async () => {
    const shell = mount();
    await settle();
    const root = shell.shadowRoot as ShadowRoot;
    const panel = root.querySelector<HTMLElement>("aside.panel");
    expect(panel?.hidden).toBe(true);
    expect(styleOf(shell)).toMatch(/aside\.panel \{[^}]*inline-size: 20rem/);
    const content = document.createElement("div");
    content.slot = "panel";
    shell.append(content);
    root.querySelector('slot[name="panel"]')?.dispatchEvent(new Event("slotchange"));
    expect(panel?.hidden).toBe(false);
    content.remove();
    root.querySelector('slot[name="panel"]')?.dispatchEvent(new Event("slotchange"));
    expect(panel?.hidden).toBe(true);
  });

  it("hides the sidebar, header and footer in frameless mode, for the welcome screen (UI-S-027)", async () => {
    const shell = mount();
    await settle();
    expect(styleOf(shell)).toMatch(/:host\(\[frameless\]\) nav[^}]*display: none/);
    shell.frameless = true;
    expect(shell.hasAttribute("frameless")).toBe(true);
    shell.frameless = false;
    expect(shell.hasAttribute("frameless")).toBe(false);
  });

  it("keeps the UI-S-016 contracts: skip control first, named landmarks, no violations", async () => {
    const shell = mount();
    await settle();
    const root = shell.shadowRoot as ShadowRoot;
    expect(root.querySelector("button.skip")?.textContent).toBe(labels.skipToContent);
    expect(root.querySelector("nav")?.getAttribute("aria-label")).toBe(labels.navigationMain);
    expect(root.querySelector("main")?.getAttribute("tabindex")).toBe("-1");
    expect(accessibilityViolations(shell)).toEqual([]);
    expect(styleOf(shell)).toMatch(
      /^\s*:focus-visible \{ outline: 2px solid var\(--fs-focus-ring\)/,
    );
  });

  it("reads the host descriptor again when health is refreshed, for example after sign-in", async () => {
    let reads = 0;
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    shell.configure({
      distribution,
      client: {
        host: async () => {
          reads += 1;
          return hostFakes().host();
        },
        subscribe: () => () => undefined,
      },
      theme: fakeThemeStore().store,
    });
    document.body.replaceChildren(shell);
    await settle();
    expect(reads).toBe(1);
    await shell.refreshHealth();
    expect(reads).toBe(2);
  });

  it("refreshes a Settings page it holds when health is refreshed", async () => {
    let reads = 0;
    const client = {
      health: async () => ({ status: "ok" as const, product: "filesquid", schema_version: 1 }),
      host: async () => {
        reads += 1;
        return hostFakes().host();
      },
      subscribe: () => () => undefined,
    };
    const theme = fakeThemeStore().store;
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    shell.configure({ distribution, client, theme });
    const settings = document.createElement("fs-settings") as FsSettings;
    settings.configure({ labels, client, theme, distribution, contractVersion: "0.24.0" });
    shell.append(settings);
    document.body.replaceChildren(shell);
    await settle();
    expect(reads).toBe(2);
    await shell.refreshHealth();
    expect(reads).toBe(4);
  });

  it("shows the dark-surface logo when the resolved theme is dark", async () => {
    const { store, media } = fakeThemeStore(false);
    const shell = mount(store);
    await settle();
    const logo = () => shell.shadowRoot?.querySelector(".brand img")?.getAttribute("src");
    expect(logo()).toBe("/filesquid-mark.svg");
    media.set(true);
    expect(logo()).toBe("/filesquid-mark-dark.svg");
    store.setChoice("light");
    expect(logo()).toBe("/filesquid-mark.svg");
    store.setChoice("dark");
    expect(logo()).toBe("/filesquid-mark-dark.svg");
  });

  it("keeps the one logo of a distribution without a dark variant", async () => {
    const { store } = fakeThemeStore(true);
    const single: Distribution = { ...distribution, logo: { src: "/mark.svg", alt: "Mark" } };
    const shell = mount(store, single);
    await settle();
    expect(shell.shadowRoot?.querySelector(".brand img")?.getAttribute("src")).toBe("/mark.svg");
  });

  it("stops following the theme once disconnected", async () => {
    const { store } = fakeThemeStore(false);
    const shell = mount(store);
    await settle();
    shell.remove();
    store.setChoice("dark");
    expect(shell.shadowRoot?.querySelector(".brand img")?.getAttribute("src")).toBe(
      "/filesquid-mark.svg",
    );
  });

  it("follows a theme store given after a first configure", async () => {
    const first = fakeThemeStore(false);
    const shell = mount(first.store);
    await settle();
    const second = fakeThemeStore(true);
    shell.configure({ distribution, client: hostFakes(), theme: second.store });
    const src = () => shell.shadowRoot?.querySelector(".brand img")?.getAttribute("src");
    expect(src()).toBe("/filesquid-mark-dark.svg");
    first.store.setChoice("light");
    expect(src()).toBe("/filesquid-mark-dark.svg");
  });
});
