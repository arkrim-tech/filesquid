/**
 * UI tests for `bindLibraryRoute` and the shell's Libraries link (UI-1.1, holistic-ui): the
 * library start page, the per-library page and the repair offer mount on their hashes, one at a
 * time, and are removed elsewhere; `#/library/<id>/repair` is the address the Monitor's W010
 * note links to through `repairHref`, and it opens that library's repair offer.
 */
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  LIBRARY_ROUTE,
  LibraryRegistry,
  bindLibraryRoute,
  bindMonitorRoute,
  defineElements,
  libraryRepairHref,
  type FsLibraryDetail,
  type FsLibraryRepair,
  type MonitorClient,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { FakeLibrarySource } from "../support/library-fakes";

beforeEach(() => defineElements());

let unbind: () => void = () => undefined;
afterEach(() => {
  unbind();
  window.location.hash = "";
});

/** Let pending work finish. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Bind the route on a fresh host element. */
function setup() {
  const shell = document.createElement("div") as HTMLElement & { focusMain?: () => void };
  const focused: string[] = [];
  shell.focusMain = () => focused.push("main");
  document.body.replaceChildren(shell);
  const source = new FakeLibrarySource();
  unbind = bindLibraryRoute(shell, { labels, source, registry: new LibraryRegistry() });
  return { shell, source, focused };
}

const tags = (shell: HTMLElement) =>
  [...shell.children].map((child) => child.tagName.toLowerCase());

describe("bindLibraryRoute", () => {
  it("mounts nothing on other hashes", () => {
    window.location.hash = "#/monitor";
    const { shell } = setup();
    expect(tags(shell)).toEqual([]);
  });

  it("mounts the start page on #/library", () => {
    window.location.hash = LIBRARY_ROUTE;
    const { shell } = setup();
    expect(tags(shell)).toEqual(["fs-library-start"]);
  });

  it("mounts the library page for #/library/<id> with that ID and swaps pages on hashchange", async () => {
    window.location.hash = "#/library/lib%201";
    const { shell, focused } = setup();
    await settle();
    expect(tags(shell)).toEqual(["fs-library-detail"]);
    expect((shell.firstElementChild as FsLibraryDetail).libraryId).toBe("lib 1");
    window.location.hash = libraryRepairHref("lib 1");
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await settle();
    expect(tags(shell)).toEqual(["fs-library-repair"]);
    expect((shell.firstElementChild as FsLibraryRepair).libraryId).toBe("lib 1");
    window.location.hash = "";
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(tags(shell)).toEqual([]);
    // jsdom also fires its own hashchange, so count at least the two changes made above.
    expect(focused.length).toBeGreaterThanOrEqual(2);
    expect(new Set(focused)).toEqual(new Set(["main"]));
  });

  it("moves the same page to another library without a new element", async () => {
    window.location.hash = "#/library/a";
    const { shell } = setup();
    const first = shell.firstElementChild;
    window.location.hash = "#/library/b";
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(shell.firstElementChild).toBe(first);
    expect((first as FsLibraryDetail).libraryId).toBe("b");
  });

  it("stops following the hash after it is unbound", () => {
    const { shell } = setup();
    unbind();
    window.location.hash = LIBRARY_ROUTE;
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(tags(shell)).toEqual([]);
  });

  it("is the address the Monitor's repairHref points to, and it opens that library's repair offer", async () => {
    const never = () => new Promise<never>(() => undefined);
    const client: MonitorClient = {
      adminHealth: never,
      adminProcesses: never,
      adminJournal: never,
      adminMetrics: never,
      adminJournalExport: never,
      adminOcflFindings: never,
      startOcflValidation: never,
      subscribe: () => () => undefined,
    };
    const shell = document.createElement("div");
    document.body.replaceChildren(shell);
    const repairHref = libraryRepairHref;
    const stopMonitor = bindMonitorRoute(shell, { labels, client, pollMs: 0, repairHref });
    expect(repairHref("5e2e")).toBe("#/library/5e2e/repair");
    stopMonitor();
    window.location.hash = repairHref("5e2e");
    const { shell: host } = setup();
    await settle();
    expect(tags(host)).toEqual(["fs-library-repair"]);
  });
});
