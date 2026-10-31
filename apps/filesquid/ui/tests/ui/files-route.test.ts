/**
 * UI tests for the files route (UI-F2.2): `#/files/<library>/<object>/<folder>/...` shows the
 * file browser at that folder, a hash change moves it, entering a folder writes the address
 * with `pushState` (so Back works without a second load), and any other hash removes it.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  bindFilesRoute,
  defineElements,
  type FsFileBrowser,
  type FsFilePanel,
} from "../../src/index";
import { fixture, one, settle, control, click } from "../support/file-browser-fakes";
import { FakePanelSource } from "../support/panel-fakes";
import { FakeMetadataSource } from "../support/metadata-fakes";
import { FakeFactsSource, FakeProvenanceSource } from "../support/provenance-fakes";
import labels from "../../src/theme/labels.en.json";

let host: HTMLElement & { focusMain: () => void };
let stop: () => void;

beforeEach(() => {
  defineElements();
  window.history.replaceState(null, "", "#/");
  host = Object.assign(document.createElement("div"), { focusMain: vi.fn() });
  document.body.replaceChildren(host);
});
afterEach(() => {
  vi.restoreAllMocks();
  stop?.();
  document.body.replaceChildren();
  window.history.replaceState(null, "", "#/");
});

/** The browser in the host, if any. */
const browser = () => host.querySelector<FsFileBrowser>("fs-file-browser");

describe("bindFilesRoute", () => {
  it("shows the browser at the folder the hash names, now and after a hash change", async () => {
    window.history.replaceState(null, "", "#/files/lib/obj/Contracts");
    stop = bindFilesRoute(host, { labels, source: fixture() });
    await settle();
    expect(browser()?.location).toEqual({
      library_id: "lib",
      object_id: "obj",
      prefix: "Contracts",
    });
    window.history.replaceState(null, "", "#/files/lib/obj");
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await settle();
    expect(browser()?.location.prefix).toBe("");
    expect(host.focusMain).toHaveBeenCalledTimes(1);
  });

  it("removes the browser for any other route and mounts it once for a files route", async () => {
    stop = bindFilesRoute(host, { labels, source: fixture() });
    expect(browser()).toBeNull();
    window.history.replaceState(null, "", "#/files/lib/obj");
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(host.querySelectorAll("fs-file-browser")).toHaveLength(1);
    window.history.replaceState(null, "", "#/monitor");
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(browser()).toBeNull();
  });

  it("does not take focus on the first apply", () => {
    window.history.replaceState(null, "", "#/files/lib/obj");
    stop = bindFilesRoute(host, { labels, source: fixture() });
    expect(host.focusMain).not.toHaveBeenCalled();
  });

  it("writes the address when the browser enters a folder, without a second load", async () => {
    const source = fixture();
    const list = vi.spyOn(source, "list");
    window.history.replaceState(null, "", "#/files/lib/obj");
    stop = bindFilesRoute(host, { labels, source });
    await settle();
    const push = vi.spyOn(window.history, "pushState");
    await click(control(browser() as FsFileBrowser, "Contracts"));
    expect(push).toHaveBeenCalledTimes(1);
    expect(push).toHaveBeenCalledWith(null, "", "#/files/lib/obj/Contracts");
    expect(window.location.hash).toBe("#/files/lib/obj/Contracts");
    await settle();
    expect(list).toHaveBeenCalledTimes(2);
    expect(one(browser() as FsFileBrowser, "fs-breadcrumb")).toBeDefined();
  });

  it("does not push a second entry for the folder that is already open", async () => {
    window.history.replaceState(null, "", "#/files/lib/obj/Contracts");
    stop = bindFilesRoute(host, { labels, source: fixture() });
    await settle();
    const push = vi.spyOn(window.history, "pushState");
    const crumb = (browser() as FsFileBrowser).shadowRoot?.querySelector("fs-breadcrumb");
    const links = crumb?.shadowRoot?.querySelectorAll("a") ?? [];
    (links[links.length - 1] as HTMLAnchorElement).click();
    await settle();
    expect(push).not.toHaveBeenCalled();
  });

  it("mounts the file panel in the panel slot for the opened file and removes it on close (UI-F2.3)", async () => {
    window.history.replaceState(null, "", "#/files/lib/obj");
    stop = bindFilesRoute(host, {
      labels,
      source: fixture(),
      filePanel: { history: new FakePanelSource(), download: new FakePanelSource() },
    });
    await settle();
    expect(host.querySelector("fs-file-panel")).toBeNull();
    await click(control(browser() as FsFileBrowser, "readme.txt"));
    await settle();
    const panel = host.querySelector<FsFilePanel>("fs-file-panel") as FsFilePanel;
    expect(panel.slot).toBe("panel");
    expect(panel.fileRef).toMatchObject({ display_name: "readme.txt", version: "v1" });
    const tabs = panel.shadowRoot?.querySelectorAll('[role="tab"]') ?? [];
    expect([...tabs].map((t) => t.textContent)).toEqual([labels.filePanelInfo]);
    expect(panel.shadowRoot?.querySelector("fs-details-panel")).not.toBeNull();
    expect(panel.shadowRoot?.querySelector("fs-versions-panel")).toBeNull();
    panel.shadowRoot?.querySelector<HTMLButtonElement>("button.history")?.click();
    expect(panel.shadowRoot?.querySelector("fs-versions-panel")).not.toBeNull();
    panel.dispatchEvent(new CustomEvent("fs-file-panel-close", { bubbles: true, composed: true }));
    expect(host.querySelector("fs-file-panel")).toBeNull();
    expect((browser() as FsFileBrowser).file).not.toBeNull();
  });

  it("removes the file panel when the viewer closes or the route leaves the files", async () => {
    window.history.replaceState(null, "", "#/files/lib/obj");
    stop = bindFilesRoute(host, {
      labels,
      source: fixture(),
      filePanel: { history: new FakePanelSource(), download: new FakePanelSource() },
    });
    await settle();
    await click(control(browser() as FsFileBrowser, "readme.txt"));
    await settle();
    expect(host.querySelector("fs-file-panel")).not.toBeNull();
    const back = (browser() as FsFileBrowser).shadowRoot?.querySelector<HTMLButtonElement>(
      "button.back",
    );
    back?.click();
    expect(host.querySelector("fs-file-panel")).toBeNull();
    await click(control(browser() as FsFileBrowser, "readme.txt"));
    await settle();
    window.history.replaceState(null, "", "#/monitor");
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(host.querySelector("fs-file-panel")).toBeNull();
  });

  it("reloads the listing after the panel commits a restore", async () => {
    window.history.replaceState(null, "", "#/files/lib/obj");
    stop = bindFilesRoute(host, {
      labels,
      source: fixture(),
      filePanel: { history: new FakePanelSource(), download: new FakePanelSource() },
    });
    await settle();
    await click(control(browser() as FsFileBrowser, "readme.txt"));
    await settle();
    const reload = vi.spyOn(browser() as FsFileBrowser, "reload");
    const panel = host.querySelector("fs-file-panel") as HTMLElement;
    const commit = { library_id: "lib", object_id: "obj", version: "v2", created: "" };
    panel.dispatchEvent(
      new CustomEvent("fs-files-change", {
        detail: { operation: "restore", commit },
        bubbles: true,
        composed: true,
      }),
    );
    expect(reload).toHaveBeenCalledTimes(1);
  });

  it("registers the Description tab when a metadata source is given (UI-F2.4)", async () => {
    window.history.replaceState(null, "", "#/files/lib/obj");
    const metadata = new FakeMetadataSource();
    const history = new FakePanelSource();
    stop = bindFilesRoute(host, {
      labels,
      source: fixture(),
      filePanel: { history, download: history },
      metadata,
    });
    await settle();
    await click(control(browser() as FsFileBrowser, "readme.txt"));
    await settle();
    const panel = host.querySelector<FsFilePanel>("fs-file-panel") as FsFilePanel;
    const tabs = [...(panel.shadowRoot?.querySelectorAll<HTMLElement>('[role="tab"]') ?? [])];
    expect(tabs.map((t) => t.textContent)).toEqual([
      labels.filePanelInfo,
      labels.filePanelDescription,
    ]);
    expect(panel.shadowRoot?.querySelector("fs-metadata-panel")).toBeNull();
    tabs[1]?.click();
    await settle();
    const description = panel.shadowRoot?.querySelector("fs-metadata-panel") as HTMLElement;
    expect(description.closest("section")?.dataset.slot).toBe("description");
    expect(description.dataset.state).toBe("ready");
    expect(metadata.calls.statements).toBe(1);
    expect(description.shadowRoot?.querySelectorAll(".row")).toHaveLength(8);
  });

  it("registers exactly the three tabs of UI-S-022 and no audit log tab (UI-F2.15)", async () => {
    window.history.replaceState(null, "", "#/files/lib/obj");
    const history = new FakePanelSource();
    const provenance = new FakeProvenanceSource();
    stop = bindFilesRoute(host, {
      labels,
      source: fixture(),
      filePanel: { history, download: history, facts: new FakeFactsSource(), provenance },
      metadata: new FakeMetadataSource(),
    });
    await settle();
    await click(control(browser() as FsFileBrowser, "readme.txt"));
    await settle();
    const panel = host.querySelector<FsFilePanel>("fs-file-panel") as FsFilePanel;
    const tabs = [...(panel.shadowRoot?.querySelectorAll<HTMLElement>('[role="tab"]') ?? [])];
    expect(tabs.map((t) => t.textContent)).toEqual([
      labels.filePanelInfo,
      labels.filePanelDescription,
      labels.filePanelProvenance,
    ]);
    expect(tabs.map((t) => t.textContent?.toLowerCase()).join(" ")).not.toMatch(/audit|log/);
    // The Provenance tab reads nothing until it is shown, and it mounts in its own slot.
    expect(panel.shadowRoot?.querySelector("fs-provenance-panel")).toBeNull();
    expect(provenance.historyCalls).toHaveLength(0);
    tabs[2]?.click();
    await settle();
    const tab = panel.shadowRoot?.querySelector("fs-provenance-panel") as HTMLElement;
    expect(tab.closest("section")?.dataset.slot).toBe("provenance");
    expect(tab.dataset.state).toBe("ready");
    expect(provenance.historyCalls).toHaveLength(1);
  });

  it("offers View in Logs only once the host is known to be the desktop (UI-F2.15)", async () => {
    window.history.replaceState(null, "", "#/files/lib/obj");
    const history = new FakePanelSource();
    stop = bindFilesRoute(host, {
      labels,
      source: fixture(),
      filePanel: { history, download: history, provenance: new FakeProvenanceSource() },
    });
    host.dispatchEvent(new CustomEvent("fs-host", { detail: { hostType: "desktop" } }));
    await settle();
    await click(control(browser() as FsFileBrowser, "readme.txt"));
    await settle();
    const panel = host.querySelector<FsFilePanel>("fs-file-panel") as FsFilePanel;
    panel.shadowRoot?.querySelectorAll<HTMLElement>('[role="tab"]')[1]?.click();
    await settle();
    const tab = panel.shadowRoot?.querySelector("fs-provenance-panel") as HTMLElement;
    expect(tab.shadowRoot?.querySelector("a.logs")).not.toBeNull();
    host.dispatchEvent(new CustomEvent("fs-host", { detail: { hostType: "server" } }));
    panel.fileRef = { ...(panel.fileRef as NonNullable<typeof panel.fileRef>), version: "v2" };
    await settle();
    expect(tab.shadowRoot?.querySelector("a.logs")).toBeNull();
  });

  it("mounts no file panel without a history source", async () => {
    window.history.replaceState(null, "", "#/files/lib/obj");
    stop = bindFilesRoute(host, { labels, source: fixture() });
    await settle();
    await click(control(browser() as FsFileBrowser, "readme.txt"));
    await settle();
    expect(host.querySelector("fs-file-panel")).toBeNull();
  });

  it("stops following the hash once unbound", async () => {
    stop = bindFilesRoute(host, { labels, source: fixture() });
    stop();
    window.history.replaceState(null, "", "#/files/lib/obj");
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(browser()).toBeNull();
  });
});
