/**
 * UI tests for the files route (UI-F2.2): `#/files/<library>/<object>/<folder>/...` shows the
 * file browser at that folder, a hash change moves it, entering a folder writes the address
 * with `pushState` (so Back works without a second load), and any other hash removes it.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { bindFilesRoute, defineElements, type FsFileBrowser } from "../../src/index";
import { fixture, one, settle, control, click } from "../support/file-browser-fakes";
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

  it("stops following the hash once unbound", async () => {
    stop = bindFilesRoute(host, { labels, source: fixture() });
    stop();
    window.history.replaceState(null, "", "#/files/lib/obj");
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(browser()).toBeNull();
  });
});
