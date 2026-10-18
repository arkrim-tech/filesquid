/**
 * Unit tests for the FileSquidClient I/O interface (UI-S-012): `request()`, `read()`,
 * `command()` and `upload()`, each with the bearer token of UI-S-014 and the command
 * outcomes of CORE-S-005.
 */
import { describe, expect, it, vi } from "vitest";
import { ApiError, FileSquidClient } from "../../src/api/client";
import { commandEnvelope } from "../../src/api/commands";

const TOKEN = "a".repeat(64);

/** One request as the fake fetch saw it. */
interface Seen {
  url: string;
  init: RequestInit;
}

/** A client whose fetch records every call and answers with `answer`. */
function clientWith(answer: () => Response, token: string | null = TOKEN) {
  const seen: Seen[] = [];
  const client = new FileSquidClient({
    baseUrl: "filesquid://localhost",
    token: token ?? undefined,
    fetch: async (input, init) => {
      seen.push({ url: String(input), init: init ?? {} });
      return answer();
    },
  });
  return { client, seen };
}

/** A JSON response. */
function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

/** Read a Blob as text through FileReader (jsdom's Blob has no `text()`). */
function blobText(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? new Error("FileReader failed"));
    reader.readAsText(blob);
  });
}

/** The headers of a recorded request as a Headers object. */
function headersOf(seen: Seen | undefined): Headers {
  return new Headers(seen?.init.headers);
}

const ENVELOPE = commandEnvelope({
  scope: "library:abc",
  expectedRevision: "v3",
  operationId: "op-1",
});

describe("FileSquidClient.request", () => {
  it("GETs JSON under the base URL with the bearer token", async () => {
    const { client, seen } = clientWith(() => json(200, { ok: true }));
    expect(await client.request<{ ok: boolean }>("/api/v1/health")).toEqual({ ok: true });
    expect(seen[0]?.url).toBe("filesquid://localhost/api/v1/health");
    expect(seen[0]?.init.method).toBe("GET");
    expect(headersOf(seen[0]).get("authorization")).toBe(`Bearer ${TOKEN}`);
    expect(seen[0]?.init.body).toBeUndefined();
  });

  it("sends a JSON body with its content type and the chosen method", async () => {
    const { client, seen } = clientWith(() => json(200, {}));
    await client.request("/api/v1/x", { method: "PATCH", body: { name: "a" } });
    expect(seen[0]?.init.method).toBe("PATCH");
    expect(seen[0]?.init.body).toBe('{"name":"a"}');
    expect(headersOf(seen[0]).get("content-type")).toBe("application/json");
  });

  it("returns undefined for 204 No Content", async () => {
    const { client } = clientWith(() => new Response(null, { status: 204 }));
    expect(await client.request("/api/v1/x", { method: "DELETE" })).toBeUndefined();
  });

  it("passes the abort signal to the transport", async () => {
    const { client, seen } = clientWith(() => json(200, {}));
    const controller = new AbortController();
    await client.request("/api/v1/x", { signal: controller.signal });
    expect(seen[0]?.init.signal).toBe(controller.signal);
  });

  it("raises ApiError with the contract's code and reason", async () => {
    const { client } = clientWith(() =>
      json(403, { code: "forbidden", message: "no", retryable: false, reason: "not_permitted" }),
    );
    const error = await client.request("/api/v1/x").catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({ status: 403, code: "forbidden", reason: "not_permitted" });
  });

  it("sends no credential without a token", async () => {
    const { client, seen } = clientWith(() => json(200, {}), null);
    await client.request("/api/v1/x");
    expect(headersOf(seen[0]).has("authorization")).toBe(false);
  });
});

describe("FileSquidClient.read", () => {
  it("sends an inclusive Range and reports a partial answer with its Content-Range", async () => {
    const { client, seen } = clientWith(
      () =>
        new Response("0123456789", {
          status: 206,
          headers: { "content-range": "bytes 0-9/100" },
        }),
    );
    const result = await client.read("/api/v1/files/h/content", { range: { start: 0, end: 9 } });
    expect(headersOf(seen[0]).get("range")).toBe("bytes=0-9");
    expect(headersOf(seen[0]).get("authorization")).toBe(`Bearer ${TOKEN}`);
    expect(result.partial).toBe(true);
    expect(result.contentRange).toEqual({ start: 0, end: 9, size: 100 });
    expect(await result.response.text()).toBe("0123456789");
  });

  it("reads the whole representation without a Range header", async () => {
    const { client, seen } = clientWith(() => new Response("all", { status: 200 }));
    const result = await client.read("/api/v1/files/h/content");
    expect(headersOf(seen[0]).has("range")).toBe(false);
    expect(result.partial).toBe(false);
    expect(result.contentRange).toBeUndefined();
  });

  it("raises ApiError for 416 Range Not Satisfiable", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const { client } = clientWith(() => new Response("", { status: 416 }));
    await expect(client.read("/p", { range: { start: 500 } })).rejects.toMatchObject({
      status: 416,
    });
    warn.mockRestore();
  });

  it("refuses an invalid range before sending anything", async () => {
    const { client, seen } = clientWith(() => new Response(""));
    await expect(client.read("/p", { range: { start: 5, end: 1 } })).rejects.toThrow(RangeError);
    expect(seen).toEqual([]);
  });
});

describe("FileSquidClient.command", () => {
  it("POSTs the envelope and input as JSON and returns a committed outcome", async () => {
    const outcome = { operation_id: "op-1", kind: "committed", error: null };
    const { client, seen } = clientWith(() => json(200, outcome));
    expect(await client.command("/api/v1/x", ENVELOPE, { name: "b" })).toEqual(outcome);
    expect(seen[0]?.init.method).toBe("POST");
    expect(JSON.parse(String(seen[0]?.init.body))).toEqual({
      envelope: ENVELOPE,
      input: { name: "b" },
    });
    expect(headersOf(seen[0]).get("content-type")).toBe("application/json");
    expect(headersOf(seen[0]).get("authorization")).toBe(`Bearer ${TOKEN}`);
  });

  it("returns pending for a 202 answer, which is not completion", async () => {
    const pending = { operation_id: "op-1", kind: "pending", error: null };
    const { client, seen } = clientWith(() => json(202, pending));
    expect(await client.command("/api/v1/x", ENVELOPE, undefined, { method: "DELETE" })).toEqual(
      pending,
    );
    expect(seen[0]?.init.method).toBe("DELETE");
  });

  it("rejects a 2xx answer that is not JSON with a TypeError", async () => {
    const { client } = clientWith(() => new Response("<html>proxy</html>", { status: 200 }));
    const error = await client.command("/api/v1/x", ENVELOPE).catch((e: unknown) => e);
    expect(error).toEqual(new TypeError("command answer is not a CommandOutcome"));
  });

  it("returns a module error with its PortError", async () => {
    const failed = {
      operation_id: "op-1",
      kind: "error",
      error: { code: "disk_full", retryable: true },
    };
    const { client } = clientWith(() => json(200, failed));
    expect(await client.command("/api/v1/x", ENVELOPE)).toEqual(failed);
  });

  it("maps HTTP 409 conflict to a conflict outcome for the sent operation", async () => {
    const { client } = clientWith(() =>
      json(409, { code: "conflict", message: "stale", retryable: false }),
    );
    expect(await client.command("/api/v1/x", ENVELOPE)).toEqual({
      operation_id: "op-1",
      kind: "conflict",
      error: null,
    });
  });

  it("raises ApiError for a denial instead of an outcome", async () => {
    const { client } = clientWith(() =>
      json(401, {
        code: "unauthorized",
        message: "no",
        retryable: false,
        reason: "missing_credentials",
      }),
    );
    await expect(client.command("/api/v1/x", ENVELOPE)).rejects.toMatchObject({
      status: 401,
      reason: "missing_credentials",
    });
  });

  it("rejects an answer for another operation id", async () => {
    const { client } = clientWith(() =>
      json(200, { operation_id: "op-9", kind: "committed", error: null }),
    );
    await expect(client.command("/api/v1/x", ENVELOPE)).rejects.toThrow(TypeError);
  });

  it("refuses an unusable token before sending, without echoing it", async () => {
    const bad = "bad token\n";
    const { client, seen } = clientWith(() => json(200, {}), bad);
    const error = await client.command("/api/v1/x", ENVELOPE).catch((e: unknown) => e);
    expect(error).toMatchObject({ status: 0, reason: "invalid_credentials" });
    expect(String((error as Error).message)).not.toContain("bad token");
    expect(seen).toEqual([]);
  });
});

describe("FileSquidClient.upload", () => {
  it("POSTs multipart form data with an envelope part and a file part", async () => {
    const outcome = { operation_id: "op-1", kind: "committed", error: null };
    const { client, seen } = clientWith(() => json(201, outcome));
    const file = new Blob(["pixels"], { type: "image/png" });
    expect(await client.upload("/api/v1/x", file, ENVELOPE, { filename: "a.png" })).toEqual(
      outcome,
    );
    const body = seen[0]?.init.body;
    expect(body).toBeInstanceOf(FormData);
    const form = body as FormData;
    expect(JSON.parse(await blobText(form.get("envelope") as Blob))).toEqual(ENVELOPE);
    const part = form.get("file") as File;
    expect(part.name).toBe("a.png");
    expect(await blobText(part)).toBe("pixels");
    expect(seen[0]?.init.method).toBe("POST");
    // The transport sets the multipart boundary itself.
    expect(headersOf(seen[0]).has("content-type")).toBe(false);
    expect(headersOf(seen[0]).get("authorization")).toBe(`Bearer ${TOKEN}`);
  });

  it("keeps a File's own name when no filename is given", async () => {
    const outcome = { operation_id: "op-1", kind: "committed", error: null };
    const { client, seen } = clientWith(() => json(201, outcome));
    await client.upload("/api/v1/x", new File(["x"], "holiday.jpg"), ENVELOPE);
    expect(((seen[0]?.init.body as FormData).get("file") as File).name).toBe("holiday.jpg");
  });

  it("maps HTTP 409 conflict to a conflict outcome", async () => {
    const { client } = clientWith(() =>
      json(409, { code: "conflict", message: "stale", retryable: false }),
    );
    const outcome = await client.upload("/api/v1/x", new Blob(["x"]), ENVELOPE);
    expect(outcome.kind).toBe("conflict");
  });
});
