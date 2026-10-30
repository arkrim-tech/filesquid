/**
 * The seeded catalogue behind the performance suite (Task UI-F3.1, NFR-002 profiles): the
 * Small, Medium and Large profiles as the UI sees them through the API's bounds (10,000 files
 * and 1,000 folders per view, 200 rows per page, 1,000 navigation nodes per page), the content
 * mix NFR-002 names (Unicode, duplicate names, pairs, sizes up to 250 GB), determinism (a seed,
 * so every run is identical), and the host answers (view, navigation, search) the production
 * build is measured against: stable ordering and uniqueness across pages, cursors bound to
 * their query, the refusals the API gives.
 */
import { describe, expect, it } from "vitest";
import {
  BIG_NODE,
  Catalogue,
  FixtureError,
  fold,
  LIBRARY_NODE,
  PROFILES,
  VIEW_CAPS,
  type ViewParams,
} from "../performance/fixtures";
import { textContent } from "../performance/preview-fixtures";

const query: ViewParams = {
  node: BIG_NODE,
  view: "all",
  sort: "name",
  direction: "ascending",
  folders: "on_top",
  filter: "",
};

/** Read every page of a view, returning all rows and the final `complete`. */
function readAll(catalogue: Catalogue, params: ViewParams) {
  const rows: ReturnType<Catalogue["viewPage"]>["rows"] = [];
  let cursor: string | null = null;
  let complete = true;
  let pages = 0;
  do {
    const page = catalogue.viewPage(params, cursor, VIEW_CAPS.pageSize);
    rows.push(...page.rows);
    complete = page.complete;
    cursor = page.next_cursor;
    pages += 1;
  } while (cursor !== null && pages < VIEW_CAPS.maxPages + 5);
  return { rows, complete, pages };
}

describe("the profiles as the UI sees them", () => {
  it("bounds every view at 10,000 files, 1,000 folders, 200 rows a page and 60 pages", () => {
    expect(VIEW_CAPS).toEqual({ files: 10_000, folders: 1_000, pageSize: 200, maxPages: 60 });
  });

  it("gives each profile the navigation size of its album count", () => {
    expect(PROFILES.small.navCollections).toBe(100);
    expect(PROFILES.medium.navCollections).toBe(5_000);
    expect(PROFILES.large.navCollections).toBe(50_000);
  });

  it("Small: one collection of 10,000 files and 100 folders, answered whole", () => {
    const { rows, complete } = readAll(new Catalogue(PROFILES.small), query);
    expect(rows.filter((r) => r.kind === "file")).toHaveLength(10_000);
    expect(rows.filter((r) => r.kind === "folder")).toHaveLength(100);
    expect(complete).toBe(true);
  });

  it("Medium: the caps exactly, answered whole", () => {
    const { rows, complete } = readAll(new Catalogue(PROFILES.medium), query);
    expect(rows.filter((r) => r.kind === "file")).toHaveLength(10_000);
    expect(rows.filter((r) => r.kind === "folder")).toHaveLength(1_000);
    expect(complete).toBe(true);
  });

  it("Large: a scope over the caps answers the first 10,000 files and 1,000 folders, not whole", () => {
    expect(PROFILES.large.viewFiles).toBe(12_500);
    expect(PROFILES.large.viewFolders).toBe(1_200);
    const { rows, complete, pages } = readAll(new Catalogue(PROFILES.large), query);
    expect(rows.filter((r) => r.kind === "file")).toHaveLength(10_000);
    expect(rows.filter((r) => r.kind === "folder")).toHaveLength(1_000);
    expect(complete).toBe(false);
    expect(pages).toBeLessThanOrEqual(VIEW_CAPS.maxPages);
  });
});

describe("stable paging", () => {
  const catalogue = new Catalogue(PROFILES.medium);

  it.each([
    ["name", "ascending"],
    ["name", "descending"],
    ["size", "ascending"],
    ["size", "descending"],
    ["modified", "ascending"],
    ["modified", "descending"],
  ] as const)("lists every row once, in %s %s order, across pages", (sort, direction) => {
    const { rows } = readAll(catalogue, { ...query, sort, direction });
    const keys = rows.map((r) => (r.kind === "file" ? r.file_id : r.node));
    expect(new Set(keys).size).toBe(keys.length);
    expect(rows).toHaveLength(11_000);
    const files = rows.filter((r) => r.kind === "file");
    const values = files.map((r) =>
      sort === "name" ? fold(r.display_name) : sort === "size" ? r.size : r.modified,
    );
    for (let i = 1; i < values.length; i += 1) {
      const a = values[i - 1] as string | number;
      const b = values[i] as string | number;
      expect(direction === "ascending" ? a <= b : a >= b).toBe(true);
    }
  });

  it("puts folders on top, or mixes them with the files", () => {
    const onTop = catalogue.viewPage(query, null, 200).rows;
    // Medium has 1,000 folders, so the first page of 200 is all folders when they are on top.
    expect(onTop.every((r) => r.kind === "folder")).toBe(true);
    const mixed = readAll(catalogue, { ...query, folders: "mixed" }).rows;
    const firstFile = mixed.findIndex((r) => r.kind === "file");
    const lastFolder = mixed.map((r) => r.kind).lastIndexOf("folder");
    expect(firstFile).toBeGreaterThanOrEqual(0);
    expect(firstFile).toBeLessThan(lastFolder);
  });

  it("narrows by a name filter and keeps paging stable", () => {
    const { rows } = readAll(catalogue, { ...query, filter: "masterplan" });
    expect(rows.length).toBeGreaterThan(10);
    for (const row of rows) {
      const name = row.kind === "file" ? row.display_name : row.name;
      expect(name.toLowerCase()).toContain("masterplan");
    }
  });

  it("refuses an unknown node (404), a page size over 1,000 (400) and a foreign cursor (400)", () => {
    const refusal = (action: () => unknown) => {
      try {
        action();
      } catch (error) {
        return error as FixtureError;
      }
      throw new Error("no refusal");
    };
    expect(refusal(() => catalogue.viewPage({ ...query, node: "nope" }, null, 200)).status).toBe(
      404,
    );
    expect(refusal(() => catalogue.viewPage(query, null, 1_001)).status).toBe(400);
    const cursor = catalogue.viewPage(query, null, 200).next_cursor as string;
    expect(refusal(() => catalogue.viewPage({ ...query, sort: "size" }, cursor, 200)).status).toBe(
      400,
    );
  });
});

describe("the content mix", () => {
  const catalogue = new Catalogue(PROFILES.medium);
  const files = readAll(catalogue, query).rows.filter((r) => r.kind === "file");
  const names = files.map((r) => r.display_name);

  it("has Unicode names in several scripts, and accents both composed and decomposed", () => {
    expect(names.some((n) => /[぀-ヿ一-鿿]/.test(n))).toBe(true);
    expect(names.some((n) => /[؀-ۿ]/.test(n))).toBe(true);
    expect(names.some((n) => /\p{Extended_Pictographic}/u.test(n))).toBe(true);
    expect(names.some((n) => n !== n.normalize("NFC"))).toBe(true);
    expect(names.some((n) => n === n.normalize("NFC") && n !== n.normalize("NFD"))).toBe(true);
  });

  it("repeats one display name on many rows", () => {
    const counts = new Map<string, number>();
    for (const name of names) counts.set(name, (counts.get(name) ?? 0) + 1);
    expect(Math.max(...counts.values())).toBeGreaterThan(100);
  });

  it("has pairs that share a stem, such as a JPEG beside its RAW", () => {
    const stems = new Map<string, Set<string>>();
    for (const name of names) {
      const dot = name.lastIndexOf(".");
      const set = stems.get(name.slice(0, dot)) ?? new Set<string>();
      set.add(name.slice(dot + 1));
      stems.set(name.slice(0, dot), set);
    }
    expect([...stems.values()].some((set) => set.has("jpg") && set.has("dng"))).toBe(true);
  });

  it("has sizes up to a 250 GB archive part, and a 100 GB single resource", () => {
    const sizes = files.map((r) => r.size);
    expect(Math.max(...sizes)).toBe(250_000_000_000);
    expect(sizes).toContain(100_000_000_000);
  });

  it("covers the document, data and media badge classes", () => {
    const extensions = new Set(names.map((n) => n.slice(n.lastIndexOf(".") + 1)));
    for (const extension of ["pdf", "docx", "xlsx", "csv", "jpg", "mp4", "m4a"]) {
      expect(extensions.has(extension)).toBe(true);
    }
  });

  it("is the same on every run (a seed, not a clock or Math.random)", () => {
    const again = readAll(new Catalogue(PROFILES.medium), query).rows;
    expect(JSON.stringify(again)).toBe(JSON.stringify(readAll(catalogue, query).rows));
  });
});

describe("the folders under the big view", () => {
  const catalogue = new Catalogue(PROFILES.small);

  it("answers the view of a folder with a few hundred files at most, whole", () => {
    const folder = catalogue.viewPage(query, null, 200).rows[0];
    expect(folder?.kind).toBe("folder");
    const node = (folder as { node: string }).node;
    const { rows, complete } = readAll(catalogue, { ...query, node });
    expect(complete).toBe(true);
    expect(rows.length).toBeGreaterThanOrEqual(30);
    expect(rows.length).toBeLessThanOrEqual(410);
  });

  it("gives every folder its own view, so each open is a cache miss", () => {
    const names = ["f-1", "f-2", "f-3"].map(
      (node) => catalogue.viewPage({ ...query, node }, null, 5).rows[0],
    );
    expect(new Set(names.map((r) => JSON.stringify(r))).size).toBe(3);
  });
});

describe("the navigation listing", () => {
  const catalogue = new Catalogue(PROFILES.large);

  it("lists the repository, one library, then the collections in pages of 1,000", () => {
    expect(catalogue.navigationPage(null, null, 200).nodes[0]).toMatchObject({
      node: "r",
      kind: "repository",
    });
    const library = catalogue.navigationPage("r", null, 200);
    expect(library.nodes).toHaveLength(1);
    expect(library.nodes[0]).toMatchObject({ node: LIBRARY_NODE, kind: "library" });
    let cursor: string | null = null;
    let count = 0;
    let pages = 0;
    do {
      const page = catalogue.navigationPage(LIBRARY_NODE, cursor, 1_000);
      count += page.nodes.length;
      cursor = page.next_cursor;
      pages += 1;
    } while (cursor !== null && pages < 60);
    expect(count).toBe(50_000);
    expect(pages).toBe(50);
  });

  it("names the ancestors of a listed node, the node itself last", () => {
    const page = catalogue.navigationPage(BIG_NODE, null, 200);
    expect(page.ancestors.map((n) => n.node)).toEqual(["r", LIBRARY_NODE, BIG_NODE]);
    expect(page.nodes).toHaveLength(200);
  });

  it("refuses an unknown node with 404", () => {
    expect(() => catalogue.navigationPage("zzz", null, 200)).toThrow(FixtureError);
  });
});

describe("the text preview file", () => {
  it("is about 150 KiB, so only its first 64 KiB is read, and names itself on its first line", () => {
    const bytes = Buffer.byteLength(textContent(2));
    expect(bytes).toBeGreaterThanOrEqual(150 * 1024);
    expect(bytes).toBeLessThan(160 * 1024);
    expect(textContent(2).startsWith("NOTE 2\n")).toBe(true);
    expect(textContent(2).endsWith("TAIL-MARKER")).toBe(true);
  });
});

describe("the search operation", () => {
  const catalogue = new Catalogue(PROFILES.medium);
  const terms = catalogue.searchTerms(120);

  it("offers 120 distinct varied queries that each match at least one file", () => {
    expect(new Set(terms).size).toBe(120);
    for (const q of terms) {
      expect(catalogue.searchPage({ q, sort: "relevance" }, null, 200).rows.length).toBeGreaterThan(
        0,
      );
    }
  });

  it("answers the files whose name holds the query, in relevance order, paged without repeats", () => {
    const q = "masterplan";
    const keys = new Set<string>();
    let cursor: string | null = null;
    let total = 0;
    do {
      const page = catalogue.searchPage({ q, sort: "relevance" }, cursor, 200);
      for (const row of page.rows) {
        if (row.kind !== "file") throw new Error("a folder in the search results");
        expect(row.display_name.toLowerCase()).toContain(q);
        keys.add(row.file_id);
      }
      total += page.rows.length;
      cursor = page.next_cursor;
    } while (cursor !== null);
    expect(keys.size).toBe(total);
    expect(total).toBeGreaterThan(10);
  });

  it("refuses an operator, an open quote and an empty query, as the API does (400 invalid_request)", () => {
    for (const q of [
      "colour:blue",
      'generator:"Camera App"',
      "type:spreadsheet",
      '"open',
      "",
      "  ",
    ]) {
      expect(() => catalogue.searchPage({ q, sort: "relevance" }, null, 200), q).toThrow(
        FixtureError,
      );
    }
  });

  it("refuses a query over 1,024 bytes or 32 terms, as the API does, and takes one at the limits", () => {
    const refused = (q: string) => () => catalogue.searchPage({ q, sort: "relevance" }, null, 200);
    expect(refused("a".repeat(1_025))).toThrow(FixtureError);
    expect(refused(Array.from({ length: 33 }, () => "x").join(" "))).toThrow(FixtureError);
    expect(refused("é".repeat(513))).toThrow(FixtureError);
    expect(refused("a".repeat(1_024))).not.toThrow();
    expect(refused(Array.from({ length: 32 }, () => "x").join(" "))).not.toThrow();
  });

  it("searches the node it is narrowed to, never the big collection", () => {
    const names = catalogue
      .viewPage(
        {
          node: "c-5",
          view: "all",
          sort: "name",
          direction: "ascending",
          folders: "mixed",
          filter: "",
        },
        null,
        200,
      )
      .rows.map((row) => (row.kind === "file" ? row.display_name : row.name));
    const stem = (names.find((name) => name.includes(".")) ?? "").split(".")[0] ?? "";
    const narrowed = catalogue.searchPage({ q: stem, node: "c-5", sort: "relevance" }, null, 200);
    expect(narrowed.rows.length).toBeGreaterThan(0);
    for (const row of narrowed.rows) {
      expect(names).toContain(row.kind === "file" ? row.display_name : row.name);
    }
    const everywhere = catalogue.searchPage({ q: stem, sort: "relevance" }, null, 200);
    expect(everywhere.rows.map((r) => (r.kind === "file" ? r.display_name : r.name))).not.toEqual(
      narrowed.rows.map((r) => (r.kind === "file" ? r.display_name : r.name)),
    );
    expect(() =>
      catalogue.searchPage({ q: stem, node: "zzz", sort: "relevance" }, null, 200),
    ).toThrow(FixtureError);
  });

  it("returns the folders whose name matches, placed on top or mixed in", () => {
    const onTop = catalogue.searchPage({ q: "album", sort: "name", folders: "on_top" }, null, 200);
    const kinds = onTop.rows.map((row) => row.kind === "file");
    expect(kinds).toContain(false);
    expect(kinds.indexOf(true) === -1 || kinds.lastIndexOf(false) < kinds.indexOf(true)).toBe(true);
    // Mixed: the order is the sort's alone. Names descending put the files ("scan.pdf",
    // "IMG_...") before the folders ("Album ...") that "0" also matches, which on top reverses.
    const query = { q: "0", sort: "name", direction: "descending" } as const;
    const top = catalogue.searchPage({ ...query, folders: "on_top" }, null, 200).rows[0];
    const mixed = catalogue.searchPage({ ...query, folders: "mixed" }, null, 200).rows[0];
    expect(top?.kind).toBe("folder");
    expect(mixed?.kind).toBe("file");
  });

  it("folds names and queries alike: canonically equivalent Unicode finds the same rows", () => {
    const composed = catalogue.searchPage({ q: "álbum", sort: "name" }, null, 200).rows;
    const decomposed = catalogue.searchPage({ q: "álbum", sort: "name" }, null, 200).rows;
    expect(composed.length).toBeGreaterThan(0);
    expect(decomposed.map((r) => (r.kind === "file" ? r.file_id : r.node))).toEqual(
      composed.map((r) => (r.kind === "file" ? r.file_id : r.node)),
    );
  });
});
