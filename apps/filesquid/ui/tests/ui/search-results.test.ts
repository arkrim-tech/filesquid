/**
 * UI tests for the search results in `<fs-drive-view>` (UI-F2.9, F-063, UI-S-007, UI-S-030,
 * mockups 21 and 53): the results listed with the Drive list's column heads and a relevance
 * order that a column sort replaces for this search only, the scope and count chips (also for the
 * sidebar filter), the refusal box that names the operator the host cannot answer or does not
 * know, the operators this device answers, the footer count, and the failure states.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { formatSize } from "../../src/files/model";
import type { FsDriveView } from "../../src/components/fs-drive-view";
import { OPERATORS_ON_HOST } from "../../src/search/model";
import type { SearchSource } from "../../src/search/source";
import labels from "../../src/theme/labels.en.json";
import { defineElements } from "../../src/index";
import {
  LIBRARY,
  NODE,
  NOW,
  fakeFiles,
  preferences,
  viewSource,
  all,
  click,
  names,
  one,
  rowNamed,
  settle,
} from "../support/drive-fakes";
import { FILES, searchSource, SCOPE } from "../support/search-fakes";

let errors: ReturnType<typeof vi.spyOn>;
beforeEach(() => {
  errors = vi.spyOn(console, "error").mockImplementation(() => undefined);
  vi.spyOn(console, "warn").mockImplementation(() => undefined);
});
afterEach(() => {
  expect(errors).not.toHaveBeenCalled();
  vi.restoreAllMocks();
  document.body.replaceChildren();
});

/** Mount a view in search mode for `q` over `search` and wait for the answer. */
async function mountSearch(q: string, search: SearchSource = searchSource()) {
  defineElements();
  const { service } = await preferences();
  const element = document.createElement("fs-drive-view") as unknown as FsDriveView;
  element.configure({
    labels,
    source: viewSource(),
    files: fakeFiles(),
    preferences: service,
    search,
    now: () => NOW,
    online: () => true,
  });
  document.body.replaceChildren(element);
  element.heading = labels.searchResultsTitle;
  element.search = { q, node: SCOPE.node, scope: SCOPE.name };
  await settle();
  return { element, prefs: service };
}

const refusal503 = () => new ApiError(503, "/api/v1/files/search", { code: "unavailable" });
const refusal400 = () => new ApiError(400, "/api/v1/files/search", { code: "invalid_request" });

/** A search source that always rejects with `error`. */
function refusing(error: unknown): SearchSource {
  return { search: () => Promise.reject(error) };
}

describe("search results", () => {
  it("lists the matches under the title, with the column heads of the Drive list", async () => {
    const { element } = await mountSearch("harbour");
    expect(one(element, "h2.page-title").textContent).toBe(labels.searchResultsTitle);
    expect(names(element)).toEqual([
      "harbour-2025-report.pdf",
      "harbour-masterplan-v7.pdf",
      "harbour-walk-permit.pdf",
    ]);
    expect(
      all(element, "[role=columnheader] button.sort").map((b) => b.textContent?.trim()),
    ).toEqual([labels.fileColumnName, labels.fileColumnSize, labels.fileColumnModified]);
    expect(element.dataset.state).toBe("ready");
  });

  it("shows the scope chip and the count chip with the total size", async () => {
    const { element } = await mountSearch("harbour");
    const bytes = FILES.filter((r) => r.name.startsWith("harbour")).reduce(
      (sum, r) => sum + (r.size ?? 0),
      0,
    );
    expect(all(element, ".scope-chip").map((c) => c.textContent?.trim())).toEqual([
      "In: Family archive",
    ]);
    expect(one(element, ".count-chip").textContent?.trim()).toBe(
      `3 files · ${formatSize(bytes, labels, "en")}`,
    );
  });

  it("says how many results there are in the footer, in the label table's words", async () => {
    const { element } = await mountSearch("harbour");
    expect(element.summary).toBe("3 results");
    const one1 = await mountSearch("masterplan");
    expect(one1.element.summary).toBe("1 result");
  });

  it("asks the search operation for the query in the library's scope, by relevance", async () => {
    const search = searchSource();
    await mountSearch("type:document harbour", search);
    expect(search.calls[0]?.query).toMatchObject({
      q: "type:document harbour",
      node: SCOPE.node,
      sort: "relevance",
      folders: "on_top",
      zone: "Z",
    });
  });

  it("shows no sort mark while the order is relevance", async () => {
    const { element } = await mountSearch("harbour");
    expect(all(element, "[aria-sort]")).toHaveLength(0);
  });

  it("sorts by a column, reverses on a second click and returns to relevance on a third", async () => {
    const search = searchSource();
    const { element, prefs } = await mountSearch("harbour", search);
    const before = structuredClone(prefs.preferences);
    const head = () => one(element, "button.sort[data-sort=size]");
    await click(head());
    expect(search.calls.at(-1)?.query).toMatchObject({ sort: "size", direction: "descending" });
    expect(one(element, "[aria-sort]").getAttribute("aria-sort")).toBe("descending");
    expect(names(element)[0]).toBe("harbour-masterplan-v7.pdf");
    await click(head());
    expect(search.calls.at(-1)?.query).toMatchObject({ sort: "size", direction: "ascending" });
    expect(names(element)[0]).toBe("harbour-walk-permit.pdf");
    await click(head());
    expect(search.calls.at(-1)?.query).toMatchObject({ sort: "relevance" });
    expect(all(element, "[aria-sort]")).toHaveLength(0);
    // The browsing preferences of the Drive views are left alone, every field of them.
    expect(prefs.preferences).toEqual(before);
  });

  it("asks for 200 rows a page and starts without a cursor", async () => {
    const search = searchSource();
    await mountSearch("harbour", search);
    expect(search.calls[0]?.limit).toBe(200);
    expect(search.calls[0]?.cursor).toBeNull();
  });

  it("shows the empty state when nothing matches, and 0 results in the footer", async () => {
    const { element } = await mountSearch("zebra");
    expect(element.dataset.state).toBe("empty");
    expect(one(element, "fs-states").shadowRoot?.textContent).toContain(labels.searchNoResults);
    expect(element.summary).toBe("0 results");
    expect(one(element, ".count-chip").textContent?.trim()).toBe("0 files");
  });

  it("marks a search the host cut short, as the views do", async () => {
    const rows = FILES.slice(0, 1);
    const incomplete: SearchSource = {
      search: async () => ({ rows, nextCursor: null, complete: false }),
    };
    const { element } = await mountSearch("harbour", incomplete);
    expect(element.summary).toContain(labels.fileLimitReached.replace("{count}", "1"));
  });

  it("offers the item menu and selection of the Drive list on a result", async () => {
    const { element } = await mountSearch("harbour");
    const row = rowNamed(element, "harbour-masterplan-v7.pdf");
    await click(row.querySelector("input.check") as HTMLElement);
    expect(one(element, ".bulk-count").textContent).toBe(
      labels.fileSelected.replace("{count}", "1"),
    );
  });

  it("opens the item menu of a result with the Drive list's items", async () => {
    const { element } = await mountSearch("harbour");
    const row = rowNamed(element, "harbour-masterplan-v7.pdf");
    await click(row.querySelector("button.actions") as HTMLElement);
    const items = all(element, "[role=menuitem] .label").map((el) => el.textContent);
    expect(items).toEqual(
      expect.arrayContaining([labels.fileOpen, labels.fileDetails, labels.fileRename]),
    );
  });

  it("narrows the results by the sidebar filter, without asking the search again", async () => {
    const search = searchSource();
    const { element } = await mountSearch("harbour", search);
    element.filter = "masterplan";
    await settle();
    expect(names(element)).toEqual(["harbour-masterplan-v7.pdf"]);
    expect(element.summary).toBe("1 result");
    expect(search.calls.at(-1)?.query.q).toBe("harbour");
    element.filter = "";
    await settle();
    expect(names(element)).toHaveLength(3);
  });

  it("keeps reading while a filter has no match on a page but the results go on", async () => {
    const many = [
      ...Array.from({ length: 450 }, (_, index) => ({
        ...FILES[0],
        key: "m" + index,
        fileId: "m" + index,
        name: "harbour-" + String(index).padStart(3, "0") + ".pdf",
      })),
      { ...FILES[0], key: "z", fileId: "z", name: "harbour-zzz-final.pdf" },
    ];
    const search = searchSource(many as never);
    const { element } = await mountSearch("harbour", search);
    element.filter = "zzz";
    await settle();
    await settle();
    expect(names(element)).toEqual(["harbour-zzz-final.pdf"]);
    expect(search.calls.filter((call) => call.cursor !== null)).toHaveLength(2);
  });

  it("matches the filter as search matches names, across Unicode normalisation", async () => {
    const decomposed = { ...FILES[0], key: "c", fileId: "c", name: "harbour-cafe\u0301.pdf" };
    const { element } = await mountSearch("harbour", searchSource([decomposed] as never));
    element.filter = "CAF\u00c9";
    await settle();
    expect(names(element)).toEqual(["harbour-cafe\u0301.pdf"]);
  });

  it("reads again when the same search is shown after the view was cleared", async () => {
    const search = searchSource();
    const { element } = await mountSearch("harbour", search);
    element.showPending();
    expect(names(element)).toHaveLength(0);
    element.search = { q: "harbour", node: SCOPE.node, scope: SCOPE.name };
    await settle();
    expect(names(element)).toHaveLength(3);
  });

  it("starts a new scope in relevance order even for the same query", async () => {
    const search = searchSource();
    const { element } = await mountSearch("harbour", search);
    await click(one(element, "button.sort[data-sort=size]"));
    expect(search.calls.at(-1)?.query.sort).toBe("size");
    element.search = { q: "harbour", node: SCOPE.node, scope: "Another library" };
    await settle();
    expect(search.calls.at(-1)?.query.sort).toBe("relevance");
  });

  it("drops the old answer while the route finds the next search's scope", async () => {
    const { element } = await mountSearch("harbour");
    expect(names(element)).toHaveLength(3);
    element.showPending();
    expect(names(element)).toHaveLength(0);
    expect(element.dataset.state).toBe("loading");
    expect(all(element, ".count-chip")).toHaveLength(0);
  });
});

describe("a refused search", () => {
  it("names an operator the device cannot answer and lists nothing (mockup 21)", async () => {
    const { element } = await mountSearch("harbour sharedwith:me", refusing(refusal503()));
    const box = one(element, ".refusal");
    expect(box.getAttribute("role")).toBe("alert");
    expect(one(element, ".refusal-title").textContent).toBe(
      labels.searchUnavailable.replace("{operator}", "sharedwith:"),
    );
    expect(one(element, ".refusal-help").textContent).toBe(
      labels.searchUnavailableHelp.replaceAll("{operator}", "sharedwith:"),
    );
    expect(one(element, ".nothing-searched").textContent).toBe(labels.searchNothingSearched);
    expect(all(element, ".row, .tile")).toHaveLength(0);
    expect(element.summary).toBe("0 results");
    expect(all(element, ".scope-chip, .count-chip").map((c) => c.textContent?.trim())).toEqual([
      "In: Family archive",
      "0 files",
    ]);
  });

  it.each(["to:ben", "from:ben", "pendingowner:me", "is:favorite", "owner:ben"])(
    "names the unavailable operator of %s in the box the user reads",
    async (term) => {
      const { element } = await mountSearch("harbour " + term, refusing(refusal503()));
      const operator = term.startsWith("is:") ? "is:favorite" : term.split(":")[0] + ":";
      expect(one(element, ".refusal-title").textContent).toBe(
        labels.searchUnavailable.replace("{operator}", operator),
      );
    },
  );

  it("names an operator that is not in the grammar as an invalid request (mockup 53)", async () => {
    const { element } = await mountSearch("harbour colour:red", refusing(refusal400()));
    expect(one(element, ".refusal-title").textContent).toBe(
      labels.searchUnknownOperator.replace("{operator}", "colour:"),
    );
    expect(one(element, ".refusal-help").textContent).toBe(labels.searchUnknownOperatorHelp);
  });

  it("lists the operators this device answers, and only those", async () => {
    const { element } = await mountSearch("harbour colour:red", refusing(refusal400()));
    expect(one(element, ".operators-host .eyebrow").textContent).toBe(labels.searchOperatorsOnHost);
    expect(all(element, ".operators-host .operator-chip").map((c) => c.textContent)).toEqual([
      ...OPERATORS_ON_HOST,
    ]);
  });

  it("clears the refusal when the next search runs", async () => {
    const real = searchSource();
    const search: SearchSource = {
      search: vi
        .fn()
        .mockRejectedValueOnce(refusal400())
        .mockImplementation((query, cursor, limit, call) =>
          real.search(query, cursor, limit, call),
        ),
    };
    const { element } = await mountSearch("harbour colour:red", search);
    expect(all(element, ".refusal")).toHaveLength(1);
    element.search = { q: "harbour", node: SCOPE.node, scope: SCOPE.name };
    await settle();
    expect(all(element, ".refusal")).toHaveLength(0);
    expect(names(element)).toHaveLength(3);
  });

  it("shows the unavailable state, not an operator refusal, when no operator is named", async () => {
    const { element } = await mountSearch("harbour", refusing(refusal503()));
    expect(all(element, ".refusal")).toHaveLength(0);
    expect(element.dataset.state).toBe("unavailable");
  });

  it("shows the error state with Try again for any other failure", async () => {
    const search = refusing(new ApiError(500, "/api/v1/files/search", { code: "internal" }));
    const { element } = await mountSearch("harbour", search);
    expect(element.dataset.state).toBe("error");
    expect(all(element, ".refusal")).toHaveLength(0);
    const retry = one(element, "fs-states").shadowRoot?.querySelector("button");
    expect(retry?.textContent?.trim()).toBe(labels.stateRetry);
  });

  it("does not take a failed scope lookup for a refusal of the old query", async () => {
    const { element } = await mountSearch("harbour colour:red", refusing(refusal400()));
    expect(all(element, ".refusal")).toHaveLength(1);
    element.showFailure(refusal400());
    expect(all(element, ".refusal")).toHaveLength(0);
  });
});

describe("the sidebar filter's scope and summary chips", () => {
  /** Mount an ordinary view over the fixture library. */
  async function mountView() {
    defineElements();
    const { service } = await preferences();
    const element = document.createElement("fs-drive-view") as unknown as FsDriveView;
    element.configure({
      labels,
      source: viewSource(),
      files: fakeFiles(),
      preferences: service,
      now: () => NOW,
      online: () => true,
    });
    document.body.replaceChildren(element);
    element.heading = "Harbour project";
    element.node = NODE;
    await settle();
    return element;
  }

  it("shows no chips until the filter holds text", async () => {
    const element = await mountView();
    expect(all(element, ".scope-chip, .count-chip")).toHaveLength(0);
  });

  it("reads the current view's name as the scope, never search everywhere", async () => {
    const element = await mountView();
    element.filter = "harbour";
    await settle();
    expect(all(element, ".scope-chip").map((c) => c.textContent?.trim())).toEqual([
      "In: Harbour project",
    ]);
    expect(one(element, ".count-chip").textContent?.trim()).toMatch(/^1 file · /);
    element.filter = "";
    await settle();
    expect(all(element, ".scope-chip, .count-chip")).toHaveLength(0);
  });

  it("narrows only this view: the library is still the node asked for", async () => {
    const element = await mountView();
    element.filter = "harbour";
    await settle();
    expect(names(element)).toEqual(["harbour-masterplan-v7.pdf"]);
    expect(element.node).toBe(NODE);
    expect(LIBRARY).toBeDefined();
  });
});
