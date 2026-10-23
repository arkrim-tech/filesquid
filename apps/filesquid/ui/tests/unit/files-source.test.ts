/**
 * Unit tests for the file routes as the UI uses them (UI-F2.2): `FileSquidClient`'s content
 * handle mint (UI-S-014), and `ClientFileSource`, which turns each file-browser operation into
 * exactly one API call with the contract's query and body; every write is a command with an
 * envelope and the expected revision (API-F2.13, UI-S-012).
 */
import { describe, expect, it, vi } from "vitest";
import { ApiError, FileSquidClient } from "../../src/api/client";
import type { CommandEnvelope } from "../../src/api/commands";
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

/** The text of `blob`, read with `FileReader` (jsdom's `Blob` has no `text()`). */
function blobText(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? new Error("FileReader failed"));
    reader.readAsText(blob);
  });
}

/** The JSON part `name` of a multipart body, or member `name` of a JSON body. */
async function sentPart(seen: Seen, name: "envelope" | "input"): Promise<unknown> {
  const body = seen.init.body;
  if (body instanceof FormData) return JSON.parse(await blobText(body.get(name) as Blob));
  return (JSON.parse(String(body)) as Record<string, unknown>)[name];
}

/** A committed answer, with `result`, of the command `seen` carried. */
async function committed(seen: Seen, result: unknown): Promise<Response> {
  const { operation_id } = (await sentPart(seen, "envelope")) as CommandEnvelope;
  return json(200, { operation_id, kind: "committed", error: null, result });
}

/** A client over a fetch that records calls and answers with `answer`. */
function clientWith(
  answer: (seen: Seen) => Response | Promise<Response>,
  baseUrl = "filesquid://localhost",
) {
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
const UPLOAD = {
  library_id: "lib-1",
  file_id: "0b7e2c1a-3d4f-4a5b-8c6d-7e8f9a0b1c2d",
  object_id: "urn:uuid:0b7e2c1a-3d4f-4a5b-8c6d-7e8f9a0b1c2d",
  version: "v1",
  created: "2026-10-20T08:00:01Z",
};

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

  it("takes the bytes at the range start when the host ignores Range and answers 200", async () => {
    const whole = Uint8Array.from({ length: 100 }, (_, i) => i);
    const { client } = clientWith(() => new Response(whole, { status: 200 }));
    const bytes = await new ClientFileSource(client).readRange(REF, { start: 20, end: 24 });
    expect([...bytes]).toEqual([20, 21, 22, 23, 24]);
  });

  it("stops reading a body that is longer than the range", async () => {
    let pulled = 0;
    const body = new ReadableStream<Uint8Array>({
      pull(controller) {
        pulled += 1;
        controller.enqueue(new Uint8Array(1024));
      },
    });
    const { client } = clientWith(() => new Response(body, { status: 200 }));
    const bytes = await new ClientFileSource(client).readRange(REF, { start: 0, end: 1500 });
    expect(bytes.byteLength).toBe(1501);
    expect(pulled).toBeLessThan(10);
  });

  it("answers a non-JSON mint answer with a TypeError", async () => {
    const { client } = clientWith(() => new Response("<html>", { status: 200 }));
    await expect(
      client.mintContentHandle({ library_id: "l", object_id: "o", version: "v1", path: "p" }),
    ).rejects.toBeInstanceOf(TypeError);
  });

  it("uploads one file as one multipart PUT: envelope, the client's facts as input, no path", async () => {
    const { client, seen } = clientWith((call) => committed(call, UPLOAD));
    const body = new Blob(["abc"]);
    const facts = {
      name: "Café 2023.txt",
      name_normalization: "nfc",
      media_type: "text/plain",
      last_modified: "2026-10-20T08:00:00.000Z",
    } as const;
    expect(await new ClientFileSource(client).upload(LOCATION, facts, body)).toEqual(UPLOAD);
    const [call] = seen as [Seen];
    expect(new URL(call.url).pathname).toBe("/api/v1/files/upload");
    expect(new URL(call.url).search).toBe("");
    expect(call.init.method).toBe("PUT");
    expect(await sentPart(call, "input")).toEqual({
      library_id: "lib-1",
      name: "Café 2023.txt",
      size: 3,
      media_type: "text/plain",
      last_modified: "2026-10-20T08:00:00.000Z",
      name_normalization: "nfc",
    });
    expect(await sentPart(call, "envelope")).toMatchObject({
      schema_version: 1,
      expected_revision: "0",
      scope: "library:lib-1",
    });
    expect(await blobText((call.init.body as FormData).get("file") as Blob)).toBe("abc");
    expect(seen).toHaveLength(1);
  });

  it("always uploads into the library it is given, whatever the facts hold", async () => {
    const { client, seen } = clientWith((call) => committed(call, UPLOAD));
    const facts = { name: "a.txt", library_id: "other" } as unknown as { name: string };
    await new ClientFileSource(client).upload(LOCATION, facts, new Blob(["x"]));
    expect(await sentPart(seen[0] as Seen, "input")).toMatchObject({ library_id: "lib-1" });
  });

  it("names the file an upload replaces with file_id and expects that file's head", async () => {
    const { client, seen } = clientWith((call) => committed(call, { ...UPLOAD, version: "v2" }));
    const facts = { name: "a.txt", name_normalization: "nfc", file_id: UPLOAD.file_id } as const;
    const options = { revision: "v1" };
    await new ClientFileSource(client).upload(LOCATION, facts, new Blob(["new"]), options);
    const input = (await sentPart(seen[0] as Seen, "input")) as Record<string, unknown>;
    expect(input.file_id).toBe(UPLOAD.file_id);
    expect(Object.keys(input).sort()).toEqual([
      "file_id",
      "library_id",
      "name",
      "name_normalization",
      "size",
    ]);
    expect(await sentPart(seen[0] as Seen, "envelope")).toMatchObject({ expected_revision: "v1" });
  });

  it("rename is one command POST to the renames route with the contract's input", async () => {
    const { client, seen } = clientWith((call) => committed(call, COMMIT));
    const commit = await new ClientFileSource(client).rename(
      { library_id: "lib-1" },
      "5b1d2c3e-7a4f-4e8b-9c6d-0a1b2c3d4e5f",
      "Holiday photo.png",
      { revision: "v3" },
    );
    expect(commit).toEqual(COMMIT);
    expect(seen).toHaveLength(1);
    const [call] = seen as [Seen];
    expect(new URL(call.url).pathname).toBe("/api/v1/files/renames");
    expect(call.init.method).toBe("POST");
    expect(await sentPart(call, "input")).toEqual({
      library_id: "lib-1",
      file_id: "5b1d2c3e-7a4f-4e8b-9c6d-0a1b2c3d4e5f",
      display_name: "Holiday photo.png",
    });
    expect(await sentPart(call, "envelope")).toMatchObject({
      schema_version: 1,
      expected_revision: "v3",
      scope: "library:lib-1",
    });
  });

  it("rejects a conflict outcome (HTTP 409) as ApiError conflict and commits nothing", async () => {
    const { client } = clientWith(() => json(409, { code: "conflict", retryable: false }));
    const source = new ClientFileSource(client);
    await expect(source.move(LOCATION, "a", "b", { revision: "v1" })).rejects.toMatchObject({
      status: 409,
      code: "conflict",
    });
    await expect(source.upload(LOCATION, { name: "a.txt" }, new Blob(["x"]))).rejects.toMatchObject(
      { status: 409, code: "conflict" },
    );
  });

  it("refuses an answer that is not committed or carries no result", async () => {
    const pending = (call: Seen) =>
      sentPart(call, "envelope").then((e) =>
        json(200, {
          operation_id: (e as CommandEnvelope).operation_id,
          kind: "pending",
          error: null,
        }),
      );
    const { client } = clientWith(pending);
    const source = new ClientFileSource(client);
    await expect(source.remove(LOCATION, "a", { revision: "v1" })).rejects.toBeInstanceOf(
      TypeError,
    );
  });

  it.each([
    [400, "invalid_request"],
    [404, "not_found"],
    [409, "conflict"],
    [503, "unavailable"],
  ])("rename rejects with the route's %s refusal (%s)", async (status, code) => {
    const retryable = status === 503; // only `unavailable` is retryable (the contract)
    const { client } = clientWith(() => json(status, { code, message: "x", retryable }));
    await expect(
      new ClientFileSource(client).rename({ library_id: "lib-1" }, "f", "n", { revision: "v1" }),
    ).rejects.toMatchObject({ status, code, retryable });
  });

  it.each([
    [
      "createFolder",
      (s: ClientFileSource) => s.createFolder(LOCATION, "A b/New", { revision: "v3" }),
      "/api/v1/files/folders",
      { library_id: "lib-1", object_id: "urn:uuid:o1", path: "A b/New" },
    ],
    [
      "move",
      (s: ClientFileSource) => s.move(LOCATION, "A b/a.txt", "C/a.txt", { revision: "v3" }),
      "/api/v1/files/moves",
      { library_id: "lib-1", object_id: "urn:uuid:o1", from: "A b/a.txt", to: "C/a.txt" },
    ],
    [
      "remove",
      (s: ClientFileSource) => s.remove(LOCATION, "A b/a.txt", { revision: "v3" }),
      "/api/v1/files/removals",
      { library_id: "lib-1", object_id: "urn:uuid:o1", path: "A b/a.txt" },
    ],
    [
      "restore",
      (s: ClientFileSource) => s.restore(LOCATION, "v3", ["A b/a.txt"], { revision: "v3" }),
      "/api/v1/files/restorations",
      { library_id: "lib-1", object_id: "urn:uuid:o1", version: "v3", paths: ["A b/a.txt"] },
    ],
  ])("%s is one command POST with the contract's input", async (_name, call, path, body) => {
    const { client, seen } = clientWith((seen) => committed(seen, COMMIT));
    await call(new ClientFileSource(client));
    expect(seen).toHaveLength(1);
    expect(new URL(String(seen[0]?.url)).pathname).toBe(path);
    expect(seen[0]?.init.method).toBe("POST");
    expect(await sentPart(seen[0] as Seen, "input")).toEqual(body);
    expect(await sentPart(seen[0] as Seen, "envelope")).toMatchObject({
      schema_version: 1,
      expected_revision: "v3",
      scope: "library:lib-1",
    });
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
