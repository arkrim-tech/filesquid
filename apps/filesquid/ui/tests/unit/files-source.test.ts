/**
 * Unit tests for the file routes as the UI uses them (UI-F2.2): `FileSquidClient`'s content
 * handle mint and raw upload (UI-S-014, UI-S-012), and `ClientFileSource`, which turns each
 * file-browser operation into exactly one API call with the contract's query and body.
 */
import { describe, expect, it, vi } from "vitest";
import { ApiError, FileSquidClient } from "../../src/api/client";
import { ClientFileSource } from "../../src/files/source";
import type { FileLocation, FileRef } from "../../src/files/model";

const TOKEN = "t".repeat(48);
const HANDLE = "H".repeat(43);

/** One request as the fake fetch saw it. */
interface Seen {
  url: string;
  init: RequestInit;
}

/** A JSON response. */
function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

/** A client over a fetch that records calls and answers with `answer`. */
function clientWith(answer: (seen: Seen) => Response, baseUrl = "filesquid://localhost") {
  const seen: Seen[] = [];
  const client = new FileSquidClient({
    baseUrl,
    token: TOKEN,
    fetch: async (input, init) => {
      const call = { url: String(input), init: init ?? {} };
      seen.push(call);
      return answer(call);
    },
  });
  return { client, seen };
}

const LOCATION: FileLocation = { library_id: "lib-1", object_id: "urn:uuid:o1", prefix: "A b" };
const REF: FileRef = {
  library_id: "lib-1",
  object_id: "urn:uuid:o1",
  version: "v3",
  path: "A b/x.png",
  display_name: "photo.png",
};
const COMMIT = { library_id: "lib-1", object_id: "urn:uuid:o1", version: "v4", created: "t" };

describe("FileSquidClient.mintContentHandle", () => {
  const request = { library_id: "lib-1", object_id: "o", version: "v3", path: "x.png" };
  const answer = {
    handle: HANDLE,
    url: `/api/v1/content/${HANDLE}`,
    expires_at: "2026-10-20T10:00:00Z",
  };

  it("posts the one file version with the bearer token and returns the URL on the active host", async () => {
    const { client, seen } = clientWith(() => json(200, answer));
    const minted = await client.mintContentHandle(request);
    expect(seen).toHaveLength(1);
    expect(seen[0]?.url).toBe("filesquid://localhost/api/v1/content-handles");
    expect(seen[0]?.init.method).toBe("POST");
    expect(new Headers(seen[0]?.init.headers).get("authorization")).toBe(`Bearer ${TOKEN}`);
    expect(JSON.parse(String(seen[0]?.init.body))).toEqual(request);
    expect(minted).toEqual({
      url: `filesquid://localhost/api/v1/content/${HANDLE}`,
      expires_at: "2026-10-20T10:00:00Z",
    });
  });

  it("uses a same-origin path on the server host", async () => {
    const { client } = clientWith(() => json(200, answer), "");
    expect((await client.mintContentHandle(request)).url).toBe(`/api/v1/content/${HANDLE}`);
  });

  it("refuses an answer that is not a handle, without quoting it", async () => {
    const bad = { handle: "short", url: "https://evil.example/x", expires_at: "t" };
    const { client } = clientWith(() => json(200, bad));
    const error = await client.mintContentHandle(request).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(TypeError);
    expect(String((error as Error).message)).not.toContain("evil");
    expect(String((error as Error).message)).not.toContain("short");
  });

  it("refuses a URL that is not the content route, even with a valid handle", async () => {
    const { client } = clientWith(() => json(200, { ...answer, url: `//evil.example/${HANDLE}` }));
    await expect(client.mintContentHandle(request)).rejects.toBeInstanceOf(TypeError);
  });

  it("rejects with ApiError when the mint is refused", async () => {
    const { client } = clientWith(() =>
      json(403, { code: "forbidden", reason: "not_permitted", retryable: false }),
    );
    const error = await client.mintContentHandle(request).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).reason).toBe("not_permitted");
  });
});

describe("FileSquidClient.putBytes", () => {
  it("sends the bytes with PUT, the bearer token and an octet-stream type, and parses the answer", async () => {
    const { client, seen } = clientWith(() => json(200, COMMIT));
    const body = new Blob([new Uint8Array([1, 2, 3])]);
    const answer = await client.putBytes<typeof COMMIT>("/api/v1/files/upload?path=x", body);
    expect(answer).toEqual(COMMIT);
    expect(seen[0]?.url).toBe("filesquid://localhost/api/v1/files/upload?path=x");
    expect(seen[0]?.init.method).toBe("PUT");
    expect(seen[0]?.init.body).toBe(body);
    const headers = new Headers(seen[0]?.init.headers);
    expect(headers.get("authorization")).toBe(`Bearer ${TOKEN}`);
    expect(headers.get("content-type")).toBe("application/octet-stream");
  });

  it("rejects with ApiError on a conflict", async () => {
    const { client } = clientWith(() => json(409, { code: "conflict", retryable: false }));
    const error = await client
      .putBytes("/api/v1/files/upload", new Blob([]))
      .catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).code).toBe("conflict");
  });
});

describe("ClientFileSource", () => {
  it("lists a folder with the contract's query", async () => {
    const page = { entries: [], next_cursor: null };
    const { client, seen } = clientWith(() => json(200, page));
    const source = new ClientFileSource(client);
    expect(await source.list(LOCATION, "cur", 200)).toEqual(page);
    const url = new URL(String(seen[0]?.url));
    expect(url.pathname).toBe("/api/v1/files/listing");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      library_id: "lib-1",
      object_id: "urn:uuid:o1",
      prefix: "A b",
      cursor: "cur",
      limit: "200",
    });
  });

  it("leaves the prefix and cursor out for the first page of the root", async () => {
    const { client, seen } = clientWith(() => json(200, { entries: [], next_cursor: null }));
    await new ClientFileSource(client).list({ ...LOCATION, prefix: "" });
    const params = new URL(String(seen[0]?.url)).searchParams;
    expect(params.has("prefix")).toBe(false);
    expect(params.has("cursor")).toBe(false);
    expect(params.get("limit")).toBe("200");
  });

  it("reads the details of one file version", async () => {
    const { client, seen } = clientWith(() => json(200, { media_type: "image/png" }));
    await new ClientFileSource(client).details(REF);
    const url = new URL(String(seen[0]?.url));
    expect(url.pathname).toBe("/api/v1/files/details");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      library_id: "lib-1",
      object_id: "urn:uuid:o1",
      version: "v3",
      path: "A b/x.png",
    });
  });

  it("reads a range of the content with one inclusive Range header", async () => {
    const { client, seen } = clientWith(
      () => new Response(new Uint8Array([104, 105]), { status: 206 }),
    );
    const bytes = await new ClientFileSource(client).readRange(REF, { start: 0, end: 65_535 });
    expect([...bytes]).toEqual([104, 105]);
    const url = new URL(String(seen[0]?.url));
    expect(url.pathname).toBe("/api/v1/files/content");
    expect(url.searchParams.get("path")).toBe("A b/x.png");
    expect(new Headers(seen[0]?.init.headers).get("range")).toBe("bytes=0-65535");
  });

  it("never returns more than the range it asked for", async () => {
    const { client } = clientWith(() => new Response(new Uint8Array(100), { status: 200 }));
    const bytes = await new ClientFileSource(client).readRange(REF, { start: 0, end: 9 });
    expect(bytes.byteLength).toBe(10);
  });

  it("uploads one file as one PUT with the path in the query", async () => {
    const { client, seen } = clientWith(() => json(200, COMMIT));
    const body = new Blob(["abc"]);
    expect(await new ClientFileSource(client).upload(LOCATION, "A b/new.txt", body)).toEqual(
      COMMIT,
    );
    const url = new URL(String(seen[0]?.url));
    expect(url.pathname).toBe("/api/v1/files/upload");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      library_id: "lib-1",
      object_id: "urn:uuid:o1",
      path: "A b/new.txt",
    });
    expect(seen[0]?.init.method).toBe("PUT");
  });

  it.each([
    [
      "createFolder",
      (s: ClientFileSource) => s.createFolder(LOCATION, "A b/New"),
      "/api/v1/files/folders",
      { library_id: "lib-1", object_id: "urn:uuid:o1", path: "A b/New" },
    ],
    [
      "move",
      (s: ClientFileSource) => s.move(LOCATION, "A b/a.txt", "C/a.txt"),
      "/api/v1/files/moves",
      { library_id: "lib-1", object_id: "urn:uuid:o1", from: "A b/a.txt", to: "C/a.txt" },
    ],
    [
      "remove",
      (s: ClientFileSource) => s.remove(LOCATION, "A b/a.txt"),
      "/api/v1/files/removals",
      { library_id: "lib-1", object_id: "urn:uuid:o1", path: "A b/a.txt" },
    ],
    [
      "restore",
      (s: ClientFileSource) => s.restore(LOCATION, "v3", ["A b/a.txt"]),
      "/api/v1/files/restorations",
      { library_id: "lib-1", object_id: "urn:uuid:o1", version: "v3", paths: ["A b/a.txt"] },
    ],
  ])("%s is one JSON POST with the contract's body", async (_name, call, path, body) => {
    const { client, seen } = clientWith(() => json(200, COMMIT));
    await call(new ClientFileSource(client));
    expect(seen).toHaveLength(1);
    expect(new URL(String(seen[0]?.url)).pathname).toBe(path);
    expect(seen[0]?.init.method).toBe("POST");
    expect(JSON.parse(String(seen[0]?.init.body))).toEqual(body);
  });

  it("mints a content handle for one file version and returns only its URL", async () => {
    const answer = { handle: HANDLE, url: `/api/v1/content/${HANDLE}`, expires_at: "t" };
    const { client, seen } = clientWith(() => json(200, answer));
    const url = await new ClientFileSource(client).mintContent(REF);
    expect(url).toBe(`filesquid://localhost/api/v1/content/${HANDLE}`);
    expect(JSON.parse(String(seen[0]?.init.body))).toEqual({
      library_id: "lib-1",
      object_id: "urn:uuid:o1",
      version: "v3",
      path: "A b/x.png",
    });
  });

  it("lets an API error through unchanged", async () => {
    const { client } = clientWith(() => json(404, { code: "not_found", retryable: false }));
    const error = await new ClientFileSource(client).list(LOCATION).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect(vi.isMockFunction(error)).toBe(false);
  });
});
