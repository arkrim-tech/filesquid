/**
 * Unit tests for the view sources and the file actions (UI-F2.8, API-S-011): the client source
 * over the view operation, the in-memory source that answers as the API does (stable order, a
 * missing value last in both directions, folder placement, filter, cursors bound to the query and
 * the rows), and rename, remove with Undo and the honest refusal of a move or copy.
 */
import { describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  DestinationUnavailableError,
  DriveActions,
  UnavailableActionError,
  canAct,
} from "../../src/drive/actions";
import { MemoryViewSource } from "../../src/drive/memory-source";
import type { DriveRow, ViewQuery } from "../../src/drive/model";
import { ClientViewSource, parseViewPage } from "../../src/drive/source";

const QUERY: ViewQuery = {
  node: "lib",
  view: "all",
  sort: "name",
  direction: "ascending",
  folders: "on_top",
  filter: "",
};

const file = (id: string, name: string, extra: Partial<DriveRow> = {}): DriveRow => ({
  key: id,
  kind: "file",
  name,
  fileId: id,
  objectId: `urn:uuid:${id}`,
  libraryId: "L",
  size: 1,
  modified: "2026-10-16T10:00:00Z",
  ...extra,
});
const folder = (node: string, name: string): DriveRow => ({
  key: node,
  kind: "folder",
  name,
  node,
});

describe("ClientViewSource", () => {
  const body = {
    node: "lib",
    view: "all",
    sort: "name",
    direction: "ascending",
    folders: "on_top",
    complete: true,
    next_cursor: "next",
    rows: [
      { kind: "folder", node: "c1", node_kind: "collection", name: "Photos" },
      {
        kind: "file",
        file_id: "f1",
        object_id: "urn:uuid:f1",
        display_name: "a.pdf",
        media_type: "application/pdf",
        size: 3,
        modified: "2026-10-16T10:00:00Z",
        modified_by_me: null,
        opened_by_me: null,
      },
    ],
  };

  it("asks GET /api/v1/files/view with the node, view, sort, placement, filter, cursor and limit", async () => {
    const request = vi.fn().mockResolvedValue(body);
    const page = await new ClientViewSource({ request } as never).view(
      { ...QUERY, sort: "modified", direction: "descending", filter: "tax" },
      "cur",
      50,
    );
    const url = new URL(`http://x${request.mock.calls[0]?.[0]}`);
    expect(url.pathname).toBe("/api/v1/files/view");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      node: "lib",
      view: "all",
      sort: "modified",
      direction: "descending",
      folders: "on_top",
      filter: "tax",
      cursor: "cur",
      limit: "50",
    });
    expect(page.nextCursor).toBe("next");
    expect(page.rows.map((r) => r.name)).toEqual(["Photos", "a.pdf"]);
  });

  it("leaves out an empty filter and a missing cursor", async () => {
    const request = vi.fn().mockResolvedValue({ ...body, next_cursor: null });
    await new ClientViewSource({ request } as never).view(QUERY);
    const url = new URL(`http://x${request.mock.calls[0]?.[0]}`);
    expect(url.searchParams.has("filter")).toBe(false);
    expect(url.searchParams.has("cursor")).toBe(false);
  });

  it("rejects an answer or a row that breaks the contract", () => {
    expect(() => parseViewPage({ rows: "no" })).toThrow(TypeError);
    expect(() => parseViewPage({ ...body, rows: [{ kind: "file", file_id: 1 }] })).toThrow(
      TypeError,
    );
  });

  it("rejects a page whose completeness or cursor is missing or mistyped, so a truncated view never looks whole", () => {
    const withoutComplete: Record<string, unknown> = { ...body };
    delete withoutComplete.complete;
    expect(() => parseViewPage(withoutComplete)).toThrow(TypeError);
    expect(() => parseViewPage({ ...body, complete: "yes" })).toThrow(TypeError);
    expect(() => parseViewPage({ ...body, next_cursor: 7 })).toThrow(TypeError);
    expect(parseViewPage({ ...body, complete: false }).complete).toBe(false);
  });

  it("rejects a folder row whose kind is not one of the contract's two", () => {
    const folderRow = (node_kind: unknown) => ({ kind: "folder", node: "c", node_kind, name: "x" });
    expect(() => parseViewPage({ ...body, rows: [folderRow("library")] })).toThrow(TypeError);
    expect(() => parseViewPage({ ...body, rows: [folderRow("nonsense")] })).toThrow(TypeError);
    expect(parseViewPage({ ...body, rows: [folderRow("collection")] }).rows).toHaveLength(1);
    expect(parseViewPage({ ...body, rows: [folderRow("virtual_collection")] }).rows).toHaveLength(
      1,
    );
  });

  it("refuses a page size outside 1 to 1,000 before calling the API", async () => {
    const request = vi.fn().mockResolvedValue(body);
    const source = new ClientViewSource({ request } as never);
    for (const limit of [0, -1, 1001, 1.5, Number.NaN]) {
      await expect(source.view(QUERY, null, limit)).rejects.toThrow(RangeError);
    }
    expect(request).not.toHaveBeenCalled();
    await source.view(QUERY, null, 1000);
    expect(request).toHaveBeenCalledTimes(1);
  });

  it("carries the API's own refusals", async () => {
    const request = vi
      .fn()
      .mockRejectedValue(new ApiError(400, "/api/v1/files/view", { code: "invalid_request" }));
    await expect(new ClientViewSource({ request } as never).view(QUERY)).rejects.toMatchObject({
      status: 400,
    });
  });
});

describe("MemoryViewSource", () => {
  const rows = [
    file("f2", "beta.txt", { size: 20, modifiedByMe: "2026-10-15T00:00:00Z" }),
    file("f1", "Alpha.txt", { size: 10 }),
    file("f3", "gamma.txt", { size: 5, modifiedByMe: "2026-10-16T00:00:00Z" }),
    folder("c1", "Zeta"),
    folder("c2", "Delta"),
  ];
  const source = () => new MemoryViewSource(new Map([["lib", { all: rows }]]));
  const names = async (patch: Partial<ViewQuery>) =>
    (await source().view({ ...QUERY, ...patch })).rows.map((r) => r.name);

  it("puts folders on top by default and mixes them on request", async () => {
    expect(await names({})).toEqual(["Delta", "Zeta", "Alpha.txt", "beta.txt", "gamma.txt"]);
    expect(await names({ folders: "mixed" })).toEqual([
      "Alpha.txt",
      "beta.txt",
      "Delta",
      "gamma.txt",
      "Zeta",
    ]);
  });

  it("sorts by size in both directions with folders (no size) after every file", async () => {
    expect(await names({ sort: "size", folders: "mixed" })).toEqual([
      "gamma.txt",
      "Alpha.txt",
      "beta.txt",
      "Delta",
      "Zeta",
    ]);
    expect(await names({ sort: "size", direction: "descending", folders: "mixed" })).toEqual([
      "beta.txt",
      "Alpha.txt",
      "gamma.txt",
      "Delta",
      "Zeta",
    ]);
  });

  it("sorts by the caller's own times only, with a missing time last in both directions", async () => {
    expect((await names({ sort: "modified_by_me", folders: "mixed" })).slice(0, 2)).toEqual([
      "beta.txt",
      "gamma.txt",
    ]);
    expect(
      (await names({ sort: "modified_by_me", direction: "descending", folders: "mixed" })).slice(
        0,
        2,
      ),
    ).toEqual(["gamma.txt", "beta.txt"]);
  });

  it("filters by name without case", async () => {
    expect(await names({ filter: "ALPHA" })).toEqual(["Alpha.txt"]);
  });

  it("pages through a cursor without repeating or skipping a row", async () => {
    const s = source();
    const first = await s.view(QUERY, null, 2);
    const second = await s.view(QUERY, first.nextCursor, 2);
    const third = await s.view(QUERY, second.nextCursor, 2);
    expect([...first.rows, ...second.rows, ...third.rows].map((r) => r.key)).toEqual(
      (await s.view(QUERY, null, 10)).rows.map((r) => r.key),
    );
    expect(third.nextCursor).toBeNull();
  });

  it("refuses a cursor of other parameters (400) and of changed rows (409)", async () => {
    const s = source();
    const page = await s.view(QUERY, null, 2);
    await expect(s.view({ ...QUERY, sort: "size" }, page.nextCursor, 2)).rejects.toMatchObject({
      status: 400,
    });
    s.replace("lib", "all", rows.slice(0, 3));
    await expect(s.view(QUERY, page.nextCursor, 2)).rejects.toMatchObject({
      status: 409,
      code: "conflict",
    });
  });

  it("answers 404 for an unknown node and 400 for a view the host does not serve", async () => {
    await expect(source().view({ ...QUERY, node: "nope" })).rejects.toMatchObject({ status: 404 });
    await expect(source().view({ ...QUERY, view: "favorites" })).rejects.toMatchObject({
      status: 400,
    });
  });

  it("reports the library of its rows", async () => {
    const lib = new MemoryViewSource(
      new Map([["n", { all: [{ ...file("f", "a"), libraryId: undefined }] }]]),
      "LIB",
    );
    expect((await lib.view({ ...QUERY, node: "n" })).rows[0]?.libraryId).toBe("LIB");
  });
});

describe("DriveActions", () => {
  const files = () => ({
    list: vi.fn(async (location: { prefix: string }) => ({
      version: "v4",
      entries: location.prefix === "original" ? [{ kind: "file", path: "original/3f9a.pdf" }] : [],
    })),
    rename: vi.fn(async () => ({ version: "v5" })),
    remove: vi.fn(async () => ({ version: "v5" })),
    restore: vi.fn(async () => ({ version: "v6" })),
  });

  it("only acts on a file whose library the host named", () => {
    expect(canAct(file("f", "a"))).toBe(true);
    expect(canAct({ ...file("f", "a"), libraryId: undefined })).toBe(false);
    expect(canAct(folder("c", "x"))).toBe(false);
  });

  it("renames with the file object's head as the expected revision", async () => {
    const f = files();
    await new DriveActions(f as never).rename(file("f1", "a"), "b");
    expect(f.rename).toHaveBeenCalledWith({ library_id: "L" }, "f1", "b", { revision: "v4" });
  });

  it("removes the stored original and undoes it from the version before", async () => {
    const f = files();
    const receipt = await new DriveActions(f as never).remove(file("f1", "a.pdf"));
    expect(f.remove).toHaveBeenCalledWith(
      { library_id: "L", object_id: "urn:uuid:f1" },
      "original/3f9a.pdf",
      { revision: "v4" },
    );
    expect(receipt).toMatchObject({ name: "a.pdf", version: "v5" });
    await receipt.undo();
    expect(f.restore).toHaveBeenCalledWith(
      { library_id: "L", object_id: "urn:uuid:f1" },
      "v4",
      ["original/3f9a.pdf"],
      { revision: "v5" },
    );
  });

  it("refuses a row without a library and sends nothing", async () => {
    const f = files();
    await expect(
      new DriveActions(f as never).rename({ ...file("f1", "a"), libraryId: undefined }, "b"),
    ).rejects.toBeInstanceOf(UnavailableActionError);
    expect(f.rename).not.toHaveBeenCalled();
  });

  it("says a file has no original when the object lists none", async () => {
    const f = files();
    f.list.mockResolvedValueOnce({ version: "v1", entries: [] });
    await expect(new DriveActions(f as never).remove(file("f1", "a"))).rejects.toMatchObject({
      status: 404,
    });
  });

  it("refuses a move or copy into a collection until the contract carries the destination", () => {
    expect(() => new DriveActions(files() as never).moveOrCopy([file("f", "a")], "copy")).toThrow(
      DestinationUnavailableError,
    );
  });
});
