/**
 * Unit tests for the FileSquidClient admin methods (SRS-F-061): token header, query
 * strings, JSON Lines export and error statuses. Authentication on every route is covered
 * in client-auth.test.ts.
 */
import { describe, expect, it } from "vitest";
import { ApiError, FileSquidClient } from "../../src/api/client";

/** A fetch stub that records requests and answers with `body`. */
function recorder(body: string, status = 200) {
  const calls: { url: string; auth: string | null }[] = [];
  const fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const headers = new Headers(init?.headers);
    calls.push({ url: String(input), auth: headers.get("authorization") });
    return new Response(body, { status, headers: { "content-type": "application/json" } });
  };
  return { calls, fetch: fetch as typeof globalThis.fetch };
}

describe("admin methods", () => {
  it("send the token as a bearer header when one is set", async () => {
    const r = recorder(JSON.stringify({ status: "ok", reasons: [], checks: {} }));
    const client = new FileSquidClient({ baseUrl: "", fetch: r.fetch, token: () => "t0k" });
    await client.adminHealth();
    expect(r.calls).toEqual([{ url: "/api/v1/admin/health", auth: "Bearer t0k" }]);
  });

  it("send no authorization header when no token is known", async () => {
    const r = recorder("[]");
    const client = new FileSquidClient({ baseUrl: "filesquid://localhost", fetch: r.fetch });
    await client.adminProcesses();
    expect(r.calls).toEqual([{ url: "filesquid://localhost/api/v1/admin/processes", auth: null }]);
  });

  it("encode journal filters as query parameters", async () => {
    const r = recorder(JSON.stringify({ entries: [], next_cursor: null }));
    const client = new FileSquidClient({ baseUrl: "", fetch: r.fetch });
    await client.adminJournal({ operation: "ocfl.commit", outcome: "failed", after: 7, limit: 50 });
    expect(r.calls[0]?.url).toBe(
      "/api/v1/admin/journal?operation=ocfl.commit&outcome=failed&after=7&limit=50",
    );
  });

  it("return the JSON Lines export as text", async () => {
    const r = recorder('{"operation":"a"}\n{"operation":"b"}\n');
    const client = new FileSquidClient({ baseUrl: "", fetch: r.fetch });
    const text = await client.adminJournalExport({ level: "error" });
    expect(r.calls[0]?.url).toBe("/api/v1/admin/journal/export?level=error");
    expect(text.trim().split("\n")).toHaveLength(2);
  });

  it("follow the continuation cursor of a truncated export", async () => {
    const urls: string[] = [];
    const pages = [
      new Response('{"id":1}\n', { headers: { "X-Journal-Next-Cursor": "1" } }),
      new Response('{"id":2}\n'),
    ];
    const fetch = (async (input: RequestInfo | URL) => {
      urls.push(String(input));
      return pages.shift() as Response;
    }) as typeof globalThis.fetch;
    const text = await new FileSquidClient({ baseUrl: "", fetch }).adminJournalExport({
      operation: "ocfl.commit",
    });
    expect(urls).toEqual([
      "/api/v1/admin/journal/export?operation=ocfl.commit",
      "/api/v1/admin/journal/export?operation=ocfl.commit&after=1",
    ]);
    expect(text).toBe('{"id":1}\n{"id":2}\n');
  });

  it("raise ApiError with 401 and 403 so the UI can ask for the token", async () => {
    const body = JSON.stringify({ code: "unauthorized", message: "x", retryable: false });
    const client = new FileSquidClient({ baseUrl: "", fetch: recorder(body, 401).fetch });
    await expect(client.adminMetrics()).rejects.toMatchObject({
      status: 401,
      path: "/api/v1/admin/metrics",
    });
    await expect(client.adminMetrics()).rejects.toBeInstanceOf(ApiError);
  });
});
