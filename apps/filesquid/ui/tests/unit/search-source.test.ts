/**
 * Unit tests for the search sources (UI-F2.9, API-S-012): the client source over
 * `GET /api/v1/files/search` sends exactly the parameters the contract names and turns the answer
 * into the same page the views use, and the in-memory source answers as the operation does for
 * the grammar the UI offers (free text, phrases, exclusion, `type:`, `in:`, `title:`, `after:`,
 * `is:deleted`), refusing what the desktop host refuses.
 */
import { describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import type { DriveRow } from "../../src/drive/model";
import { MemorySearchSource } from "../../src/search/memory-source";
import { ClientSearchSource, type SearchQuery } from "../../src/search/source";

const QUERY: SearchQuery = {
  q: "type:document harbour",
  node: "lib-node",
  sort: "relevance",
  direction: "descending",
  folders: "on_top",
  zone: "Z",
};

const file = (id: string, name: string, extra: Partial<DriveRow> = {}): DriveRow => ({
  key: id,
  kind: "file",
  name,
  fileId: id,
  objectId: `urn:uuid:${id}`,
  libraryId: "L",
  mediaType: "application/pdf",
  size: 10,
  modified: "2026-10-16T10:00:00Z",
  ...extra,
});

describe("ClientSearchSource", () => {
  const body = {
    q: QUERY.q,
    node: "lib-node",
    sort: "relevance",
    direction: "descending",
    folders: "on_top",
    complete: false,
    next_cursor: "next",
    rows: [
      {
        kind: "file",
        file_id: "f1",
        object_id: "urn:uuid:f1",
        display_name: "harbour.pdf",
        media_type: "application/pdf",
        size: 3,
        modified: "2026-10-16T10:00:00Z",
        modified_by_me: null,
        opened_by_me: null,
      },
    ],
  };

  it("sends q, node, folders, zone and limit, and no sort for relevance", async () => {
    const request = vi.fn().mockResolvedValue(body);
    const page = await new ClientSearchSource({ request }, async () => "L").search(QUERY, null, 3);
    const url = new URL(`http://x${request.mock.calls[0]?.[0]}`);
    expect(url.pathname).toBe("/api/v1/files/search");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      q: "type:document harbour",
      node: "lib-node",
      folders: "on_top",
      zone: "Z",
      limit: "3",
    });
    expect(page.rows).toHaveLength(1);
    expect(page.rows[0]).toMatchObject({ name: "harbour.pdf", libraryId: "L" });
    expect(page.nextCursor).toBe("next");
    expect(page.complete).toBe(false);
  });

  it("sends the sort and direction of a view sort key, and the cursor after the first page", async () => {
    const request = vi.fn().mockResolvedValue({ ...body, sort: "size" });
    const source = new ClientSearchSource({ request });
    await source.search({ ...QUERY, sort: "size", direction: "ascending" }, "c1");
    const url = new URL(`http://x${request.mock.calls[0]?.[0]}`);
    expect(url.searchParams.get("sort")).toBe("size");
    expect(url.searchParams.get("direction")).toBe("ascending");
    expect(url.searchParams.get("cursor")).toBe("c1");
    expect(url.searchParams.get("limit")).toBe("200");
  });

  it("omits the node for a search of the whole repository", async () => {
    const request = vi.fn().mockResolvedValue(body);
    await new ClientSearchSource({ request }).search({ ...QUERY, node: undefined });
    expect(request.mock.calls[0]?.[0]).not.toContain("node=");
  });

  it("refuses a page size outside 1 to 1,000 without sending anything", async () => {
    const request = vi.fn();
    const source = new ClientSearchSource({ request });
    await expect(source.search(QUERY, null, 0)).rejects.toBeInstanceOf(RangeError);
    await expect(source.search(QUERY, null, 1001)).rejects.toBeInstanceOf(RangeError);
    expect(request).not.toHaveBeenCalled();
  });

  it("rejects an answer that breaks the contract and passes the host's refusal on", async () => {
    const broken = new ClientSearchSource({ request: vi.fn().mockResolvedValue({ rows: 3 }) });
    await expect(broken.search(QUERY)).rejects.toBeInstanceOf(TypeError);
    const refusal = new ApiError(503, "/api/v1/files/search", { code: "unavailable" });
    const refused = new ClientSearchSource({ request: vi.fn().mockRejectedValue(refusal) });
    await expect(refused.search(QUERY)).rejects.toBe(refusal);
  });

  it("passes the abort signal on", async () => {
    const request = vi.fn().mockResolvedValue(body);
    const controller = new AbortController();
    await new ClientSearchSource({ request }).search(QUERY, null, 3, { signal: controller.signal });
    expect(request.mock.calls[0]?.[1]).toEqual({ signal: controller.signal });
  });
});

describe("MemorySearchSource", () => {
  const rows = [
    file("f1", "harbour-masterplan-v7.pdf", { modified: "2026-10-16T10:00:00Z", size: 30 }),
    file("f2", "harbour-2025-report.pdf", { modified: "2026-09-02T10:00:00Z", size: 20 }),
    file("f3", "facade-study.jpg", { mediaType: "image/jpeg", size: 10 }),
    file("f4", "old-draft.pdf"),
    { key: "c1", kind: "folder", name: "Harbour project", node: "c1", nodeKind: "collection" },
  ] as DriveRow[];
  const source = () =>
    new MemorySearchSource(new Map([["lib-node", rows]]), { deleted: new Set(["f4"]) });
  const names = async (q: string, sort: SearchQuery["sort"] = "relevance") =>
    (
      await source().search({
        ...QUERY,
        q,
        sort,
        direction: sort === "relevance" ? "descending" : "ascending",
      })
    ).rows.map((row) => row.name);

  it("matches free text in the name, without case, and every term must match", async () => {
    expect(await names("HARBOUR")).toEqual([
      "Harbour project",
      "harbour-2025-report.pdf",
      "harbour-masterplan-v7.pdf",
    ]);
    expect(await names("harbour report")).toEqual(["harbour-2025-report.pdf"]);
  });

  it("matches a quoted phrase exactly and a leading minus excludes", async () => {
    expect(await names('"masterplan-v7"')).toEqual(["harbour-masterplan-v7.pdf"]);
    expect(await names("harbour -report -project")).toEqual(["harbour-masterplan-v7.pdf"]);
  });

  it("applies type:, in:, title: and after:", async () => {
    expect(await names("type:image")).toEqual(["facade-study.jpg"]);
    expect(await names("type:folder")).toEqual(["Harbour project"]);
    expect(await names("type:document harbour")).toEqual([
      "harbour-2025-report.pdf",
      "harbour-masterplan-v7.pdf",
    ]);
    expect(await names("title:report")).toEqual(["harbour-2025-report.pdf"]);
    expect(await names("harbour after:2026-10-01")).toEqual(["harbour-masterplan-v7.pdf"]);
  });

  it("hides deleted files unless is:deleted asks for them", async () => {
    expect(await names("draft")).toEqual([]);
    expect(await names("is:deleted")).toEqual(["old-draft.pdf"]);
  });

  it("orders by relevance or by a view sort key in the direction asked", async () => {
    expect(await names("type:document", "size")).toEqual([
      "harbour-2025-report.pdf",
      "harbour-masterplan-v7.pdf",
    ]);
    const descending = await source().search({
      ...QUERY,
      q: "type:document",
      sort: "size",
      direction: "descending",
    });
    expect(descending.rows.map((row) => row.name)).toEqual([
      "harbour-masterplan-v7.pdf",
      "harbour-2025-report.pdf",
    ]);
  });

  it("refuses what the desktop host refuses, as the API does", async () => {
    await expect(names("sharedwith:me")).rejects.toMatchObject({
      status: 503,
      code: "unavailable",
    });
    await expect(names("is:favorite")).rejects.toMatchObject({ status: 503 });
    await expect(names("colour:red")).rejects.toMatchObject({
      status: 400,
      code: "invalid_request",
    });
    await expect(names('"open')).rejects.toMatchObject({ status: 400 });
  });

  it("answers 404 for a scope it does not know and pages with a stable cursor", async () => {
    await expect(source().search({ ...QUERY, node: "other" })).rejects.toMatchObject({
      status: 404,
    });
    const first = await source().search({ ...QUERY, q: "harbour" }, null, 2);
    expect(first.rows).toHaveLength(2);
    expect(first.nextCursor).not.toBeNull();
    const second = await source().search({ ...QUERY, q: "harbour" }, first.nextCursor, 2);
    expect(second.rows).toHaveLength(1);
    expect(second.nextCursor).toBeNull();
  });
});
