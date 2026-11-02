/**
 * Unit tests for the journal and client-event calls of `FileSquidClient` (UI-F2.16, API-S-030,
 * API-S-031): the journal page with the new filters and `names`, one entry, the CSV export that
 * follows `X-Journal-Next-Cursor` without repeating the header row, the client-events POST with
 * its keep-alive option, and the `Retry-After` of a refusal on `ApiError`.
 */
import { describe, expect, it } from "vitest";
import { ApiError, FileSquidClient } from "../../src/api/client";

const TOKEN = "a".repeat(64);

/** One request as the fake fetch saw it. */
interface Seen {
  url: string;
  init: RequestInit;
}

/** A client whose fetch records every call and answers with the next of `answers`. */
function clientWith(...answers: Response[]) {
  const seen: Seen[] = [];
  let next = 0;
  const client = new FileSquidClient({
    baseUrl: "",
    token: TOKEN,
    fetch: async (input, init) => {
      seen.push({ url: String(input), init: init ?? {} });
      const answer = answers[Math.min(next, answers.length - 1)];
      next += 1;
      return (answer as Response).clone();
    },
  });
  return { client, seen };
}

/** A JSON response. */
function json(status: number, body: unknown, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json", ...headers },
  });
}

describe("the journal page (API-S-031)", () => {
  it("sends the library, object, file, operation id, names and the other filters", async () => {
    const { client, seen } = clientWith(json(200, { entries: [], next_cursor: null }));
    await client.adminJournalPage({
      from: "2026-10-30T00:00:00.000Z",
      library: "lib-1",
      object: "urn:uuid:1d0e88b4-0000-4000-8000-00000000a14c",
      file: "1d0e88b4-0000-4000-8000-00000000a14c",
      operation_id: "3f9c2a70-5be1-4c0d-9e44-81d7a2c90e21",
      operation: "ocfl.commit",
      outcome: "failed",
      level: "error",
      order: "desc",
      limit: 100,
      after: 41,
      names: true,
    });
    const url = new URL(seen[0]?.url ?? "", "http://x");
    expect(url.pathname).toBe("/api/v1/admin/journal");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      from: "2026-10-30T00:00:00.000Z",
      library: "lib-1",
      object: "urn:uuid:1d0e88b4-0000-4000-8000-00000000a14c",
      file: "1d0e88b4-0000-4000-8000-00000000a14c",
      operation_id: "3f9c2a70-5be1-4c0d-9e44-81d7a2c90e21",
      operation: "ocfl.commit",
      outcome: "failed",
      level: "error",
      order: "desc",
      limit: "100",
      after: "41",
      names: "true",
    });
  });

  it("returns the entries with their PROV-O link and display name", async () => {
    const entry = {
      id: 7,
      time: "2026-10-19T11:03:02.418Z",
      host: "filesquid-desktop/0.1.0",
      actor: "urn:uuid:5a0c41e2-0000-4000-8000-00000000e19b",
      operation: "metadata.set",
      outcome: "succeeded",
      level: "info",
      target_ids: [],
      provenance: { journal_only: true },
      display_name: "harbour-masterplan-v7.pdf",
    };
    const { client } = clientWith(json(200, { entries: [entry], next_cursor: 6 }));
    const page = await client.adminJournalPage({ order: "desc" });
    expect(page.next_cursor).toBe(6);
    expect(page.entries[0]?.display_name).toBe("harbour-masterplan-v7.pdf");
    expect(page.entries[0]?.provenance.journal_only).toBe(true);
  });

  it("reads one entry by its id, with names when asked", async () => {
    const { client, seen } = clientWith(
      json(200, {
        id: 12,
        time: "2026-10-19T11:03:02.418Z",
        host: "h",
        actor: "a",
        operation: "x",
        outcome: "info",
        level: "info",
        target_ids: [],
        provenance: { journal_only: true },
      }),
    );
    const entry = await client.adminJournalEntry(12, { names: true });
    expect(seen[0]?.url).toBe("/api/v1/admin/journal/entries/12?names=true");
    expect(entry.id).toBe(12);
  });

  it("answers the denial of a caller that is not the owner as an ApiError", async () => {
    const { client } = clientWith(
      json(403, { code: "forbidden", reason: "not_permitted", retryable: false }),
    );
    const failure = await client.adminJournalPage().catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(ApiError);
    expect((failure as ApiError).reason).toBe("not_permitted");
  });
});

describe("the journal export (API-S-031)", () => {
  it("asks for the format and the filters, and keeps one CSV header over several responses", async () => {
    const first = new Response("id,time\r\n1,a\r\n2,b\r\n", {
      headers: { "X-Journal-Next-Cursor": "2" },
    });
    const second = new Response("id,time\r\n3,c\r\n");
    const { client, seen } = clientWith(first, second);
    const text = await client.adminJournalExport({
      format: "csv",
      level: "warn",
      library: "lib-1",
      order: "desc",
    });
    expect(text).toBe("id,time\r\n1,a\r\n2,b\r\n3,c\r\n");
    const urls = seen.map((call) => new URL(call.url, "http://x"));
    expect(urls[0]?.searchParams.get("format")).toBe("csv");
    expect(urls[0]?.searchParams.get("library")).toBe("lib-1");
    expect(urls[0]?.searchParams.get("order")).toBe("desc");
    expect(urls[1]?.searchParams.get("after")).toBe("2");
  });

  it("joins JSON Lines responses as they are", async () => {
    const first = new Response('{"id":1}\n', { headers: { "X-Journal-Next-Cursor": "1" } });
    const second = new Response('{"id":2}\n');
    const { client } = clientWith(first, second);
    expect(await client.adminJournalExport({ format: "jsonl" })).toBe('{"id":1}\n{"id":2}\n');
  });
});

describe("client events (API-S-030)", () => {
  const request = {
    events: [
      {
        kind: "console" as const,
        id: "6f1c1c0e-6f1c-4f1c-8f1c-6f1c1c0e6f1c",
        level: "warn" as const,
        component: "fs-monitor",
        error_code: "load_failed",
      },
    ],
  };

  it("posts the batch as JSON and returns the acknowledgement", async () => {
    const ack = { acknowledged: [request.events[0]?.id], journaled: 1, principal: "urn:uuid:p" };
    const { client, seen } = clientWith(json(200, ack));
    const answer = await client.postClientEvents(request);
    expect(answer).toEqual(ack);
    expect(seen[0]?.url).toBe("/api/v1/client-events");
    expect(seen[0]?.init.method).toBe("POST");
    expect(JSON.parse(String(seen[0]?.init.body))).toEqual(request);
    expect(new Headers(seen[0]?.init.headers).get("content-type")).toBe("application/json");
    expect(seen[0]?.init.keepalive).toBeUndefined();
  });

  it("sends a keep-alive request when asked (the page-close overflow count)", async () => {
    const { client, seen } = clientWith(
      json(200, { acknowledged: [], journaled: 0, principal: "p" }),
    );
    await client.postClientEvents(request, { keepalive: true });
    expect(seen[0]?.init.keepalive).toBe(true);
  });

  it("reads Retry-After in whole seconds from a refusal", async () => {
    const { client } = clientWith(
      json(429, { code: "rate_limited", retryable: true }, { "Retry-After": "7" }),
    );
    const failure = (await client.postClientEvents(request).catch((e: unknown) => e)) as ApiError;
    expect(failure.status).toBe(429);
    expect(failure.retryAfterMs).toBe(7000);
  });

  it("has no retryAfterMs when the header is absent or is not whole seconds", async () => {
    const absent = clientWith(json(503, { code: "unavailable", retryable: true }));
    const odd = clientWith(
      json(429, { code: "rate_limited", retryable: true }, { "Retry-After": "soon" }),
    );
    const a = (await absent.client.postClientEvents(request).catch((e: unknown) => e)) as ApiError;
    const b = (await odd.client.postClientEvents(request).catch((e: unknown) => e)) as ApiError;
    expect(a.retryAfterMs).toBeUndefined();
    expect(b.retryAfterMs).toBeUndefined();
  });
});
