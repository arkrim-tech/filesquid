/**
 * UI tests for the search route (UI-F2.9, UI-S-007, UI-S-030): `#/search/<query>` shows the
 * results in the drive view over the first library, the bar follows the route (its chips and text
 * come back from the address, and clear when the route is left), `fs-search` from the bar moves
 * the address, and the bar learns which collection is shown for its `in:` suggestion.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { bindDriveRoute, defineElements, type FsAppShell, type FsNewMenu } from "../../src/index";
import type { FsSearchBar } from "../../src/components/fs-search-bar";
import {
  NAVIGATION_ROUTE_PREFIX,
  SEARCH_ROUTE_PREFIX,
  parseSearchHash,
  searchHash,
} from "../../src/app/routes";
import { MemoryNavigationSource } from "../../src/navigation/source";
import labels from "../../src/theme/labels.en.json";
import { preferences, viewSource } from "../support/drive-fakes";
import { SCOPE, TREE, all, one, searchSource, settle } from "../support/search-fakes";

beforeEach(() => {
  defineElements();
  vi.spyOn(console, "warn").mockImplementation(() => undefined);
});
const stops: (() => void)[] = [];
afterEach(() => {
  for (const stop of stops.splice(0)) stop();
  vi.restoreAllMocks();
  document.body.replaceChildren();
  window.location.hash = "";
});

describe("search addresses", () => {
  it("encodes the query after #/search/ and decodes it back", () => {
    const query = 'type:document in:"Harbour project" 100% "a/b" #x';
    const hash = searchHash(query);
    expect(hash.startsWith(SEARCH_ROUTE_PREFIX)).toBe(true);
    expect(hash.slice(1)).not.toMatch(/[ "#]/);
    expect(parseSearchHash(hash)).toBe(query);
  });

  it("reads no query from another address, an empty one or one that does not decode", () => {
    expect(parseSearchHash("#/files")).toBeUndefined();
    expect(parseSearchHash("#/search/")).toBeUndefined();
    expect(parseSearchHash(`${SEARCH_ROUTE_PREFIX}%E0%A4%A`)).toBeUndefined();
    expect(parseSearchHash(`${NAVIGATION_ROUTE_PREFIX}x`)).toBeUndefined();
  });
});

describe("the search route", () => {
  /** Mount the shell, the bar and the route at `hash`. */
  async function mountRoute(hash: string) {
    window.location.hash = hash;
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    document.body.append(shell);
    const { service } = await preferences();
    const search = searchSource();
    const bar = document.createElement("fs-search-bar") as FsSearchBar;
    bar.slot = "search";
    bar.configure({
      labels,
      search,
      navigation: new MemoryNavigationSource(TREE),
      scope: async () => SCOPE,
      debounceMs: 0,
    });
    shell.append(bar);
    const newMenu = document.createElement("fs-new-menu") as FsNewMenu;
    newMenu.configure({ labels });
    shell.append(newMenu);
    stops.push(
      bindDriveRoute(shell, {
        labels,
        navigation: new MemoryNavigationSource(TREE),
        drive: { source: viewSource(), preferences: service, search },
        newMenu,
        searchBar: bar,
      }),
    );
    await settle();
    await settle();
    return { shell, bar, search };
  }

  it("shows the results under the title and the library's name as the scope", async () => {
    const { shell, search } = await mountRoute(searchHash("harbour"));
    const view = shell.querySelector("fs-drive-view") as HTMLElement;
    const root = view.shadowRoot as ShadowRoot;
    expect(root.querySelector(".page-title")?.textContent).toBe(labels.searchResultsTitle);
    expect(root.querySelector(".scope-chip")?.textContent?.trim()).toBe("In: Family archive");
    expect(search.calls[0]?.query).toMatchObject({ q: "harbour", node: SCOPE.node });
    expect(shell.querySelector('[slot="summary"]')?.textContent).toBe("3 results");
    expect(shell.querySelector('[slot="commit-state"]')?.textContent).toBe(labels.fileCommitted);
  });

  it("restores the chips and the text in the bar from the address", async () => {
    const { bar } = await mountRoute(searchHash("type:document harbour"));
    const root = bar.shadowRoot as ShadowRoot;
    expect(
      [...root.querySelectorAll(".frame .chip .chip-label")].map((c) => c.textContent),
    ).toEqual(["Type: Document"]);
    expect(one<HTMLInputElement>(bar, "input.text").value).toBe("harbour");
  });

  it("moves the address when the bar searches", async () => {
    const { bar } = await mountRoute("#/files");
    bar.query = "type:image harbour";
    bar.dispatchEvent(
      new CustomEvent("fs-search", {
        detail: { q: "type:image harbour" },
        bubbles: true,
        composed: true,
      }),
    );
    expect(window.location.hash).toBe(searchHash("type:image harbour"));
  });

  it("changes the query in place when the address changes, without a second view", async () => {
    const { shell, search } = await mountRoute(searchHash("harbour"));
    window.location.hash = searchHash("facade");
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await settle();
    await settle();
    expect(shell.querySelectorAll("fs-drive-view")).toHaveLength(1);
    expect(search.calls.at(-1)?.query.q).toBe("facade");
    expect(shell.querySelector('[slot="summary"]')?.textContent).toBe("1 result");
  });

  it("leaves search mode and clears the bar when the route becomes All files", async () => {
    const { shell, bar, search } = await mountRoute(searchHash("harbour"));
    const before = search.calls.length;
    window.location.hash = "#/files";
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await settle();
    await settle();
    const view = shell.querySelector("fs-drive-view") as HTMLElement;
    expect(view.shadowRoot?.querySelector(".page-title")?.textContent).toBe(labels.fileRoot);
    expect(view.shadowRoot?.querySelector(".scope-chip")).toBeNull();
    expect(search.calls).toHaveLength(before);
    expect(one<HTMLInputElement>(bar, "input.text").value).toBe("");
    expect(all(bar, ".frame .chip")).toHaveLength(0);
  });

  it("tells the bar which collection is shown, for its in: suggestion, and forgets it on a route change", async () => {
    const { shell, bar } = await mountRoute("#/files");
    shell.dispatchEvent(
      new CustomEvent("fs-destination-context", {
        detail: { node: "c-harbour", kind: "collection", name: "Harbour project" },
        bubbles: true,
        composed: true,
      }),
    );
    expect(bar.folder).toEqual({ node: "c-harbour", name: "Harbour project" });
    shell.dispatchEvent(
      new CustomEvent("fs-destination-context", {
        detail: { node: SCOPE.node, kind: "library", name: SCOPE.name },
        bubbles: true,
        composed: true,
      }),
    );
    expect(bar.folder).toBeUndefined();
    shell.dispatchEvent(
      new CustomEvent("fs-destination-context", {
        detail: { node: "c-harbour", kind: "collection", name: "Harbour project" },
        bubbles: true,
        composed: true,
      }),
    );
    window.location.hash = "#/recent";
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    await settle();
    expect(bar.folder).toBeUndefined();
  });

  it("offers no in: collection for a folder, which in: has no title for", async () => {
    const { shell, bar } = await mountRoute("#/files");
    shell.dispatchEvent(
      new CustomEvent("fs-destination-context", {
        detail: { node: "f1", kind: "folder" },
        bubbles: true,
        composed: true,
      }),
    );
    expect(bar.folder).toBeUndefined();
  });

  it("clears the old results at once while the next search's library is looked up", async () => {
    const { shell } = await mountRoute(searchHash("harbour"));
    const view = shell.querySelector("fs-drive-view") as HTMLElement;
    expect(view.shadowRoot?.querySelectorAll(".row").length).toBe(3);
    window.location.hash = searchHash("facade");
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    // No await: the lookup has not answered yet.
    expect(view.shadowRoot?.querySelectorAll(".row").length).toBe(0);
    expect(view.dataset.state).toBe("loading");
    await settle();
    await settle();
    expect(view.shadowRoot?.querySelectorAll(".row").length).toBe(1);
  });

  it("says the node is gone when the host lists no library", async () => {
    window.location.hash = searchHash("harbour");
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
        drive: { source: viewSource(), preferences: service, search: searchSource() },
      }),
    );
    await settle();
    await settle();
    expect(shell.querySelector<HTMLElement>("fs-drive-view")?.dataset.state).toBe("empty");
  });
});
