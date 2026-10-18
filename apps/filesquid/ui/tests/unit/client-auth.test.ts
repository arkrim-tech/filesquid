/**
 * Unit tests for how FileSquidClient authenticates to the deny-by-default API (API-S-004):
 * the bearer header on every request, the token sources, the WebSocket subprotocol carrier,
 * the typed denial reasons and that the token never appears in an error.
 */
import { describe, expect, it, vi } from "vitest";
import {
  ApiError,
  EVENTS_SUBPROTOCOL,
  FileSquidClient,
  eventProtocols,
  isUsableToken,
} from "../../src/api/client";
import { LAUNCH_TOKEN_GLOBAL, createHostClient, launchToken } from "../../src/api/host";
import { memoryTokenStore } from "../../src/app/routes";

const TOKEN = "0123456789abcdef".repeat(4);

/** A fetch stub that records URL and Authorization header, answering `status` with `body`. */
function recorder(status = 200, body = "{}") {
  const calls: { url: string; auth: string | null }[] = [];
  const fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    calls.push({ url: String(input), auth: new Headers(init?.headers).get("authorization") });
    return new Response(body, { status, headers: { "content-type": "application/json" } });
  };
  return { calls, fetch: fetch as typeof globalThis.fetch };
}

/** A denial body as the API sends it. */
function denial(code: string, reason: string): string {
  return JSON.stringify({ code, message: "Access denied.", retryable: false, reason });
}

/** Capture the arguments the client passes to the WebSocket constructor. */
function socketRecorder() {
  const opened: { url: string; protocols: string[] | string | undefined }[] = [];
  class FakeSocket {
    onmessage: unknown = null;
    constructor(url: string, protocols?: string[] | string) {
      opened.push({ url, protocols });
    }
    close() {}
  }
  return { opened, WebSocket: FakeSocket as unknown as typeof WebSocket };
}

describe("bearer header", () => {
  it("is sent on every route, admin and non-admin alike", async () => {
    const r = recorder(200, JSON.stringify({ entries: [], next_cursor: null }));
    const client = new FileSquidClient({ baseUrl: "", fetch: r.fetch, token: () => TOKEN });
    await client.health();
    await client.adminJournal();
    expect(r.calls.map((c) => c.auth)).toEqual([`Bearer ${TOKEN}`, `Bearer ${TOKEN}`]);
  });

  it("accepts an operator-supplied token string as a constructor option", async () => {
    const r = recorder();
    await new FileSquidClient({ baseUrl: "", fetch: r.fetch, token: TOKEN }).health();
    expect(r.calls[0]?.auth).toBe(`Bearer ${TOKEN}`);
  });

  it("reads the token source on every request, so a token entered later is used", async () => {
    const r = recorder();
    const store = memoryTokenStore();
    const client = new FileSquidClient({ baseUrl: "", fetch: r.fetch, token: () => store.get() });
    await client.health();
    store.set(TOKEN);
    await client.health();
    expect(r.calls.map((c) => c.auth)).toEqual([null, `Bearer ${TOKEN}`]);
  });

  it("is omitted when no token is known, so the API answers missing_credentials", async () => {
    const r = recorder();
    await new FileSquidClient({ baseUrl: "", fetch: r.fetch }).health();
    expect(r.calls[0]?.auth).toBeNull();
  });
});

describe("token sources", () => {
  it("reads the desktop per-launch token from the window global", () => {
    const win = { [LAUNCH_TOKEN_GLOBAL]: TOKEN } as unknown as Window;
    expect(LAUNCH_TOKEN_GLOBAL).toBe("__FILESQUID_LAUNCH_TOKEN__");
    expect(launchToken(win)).toBe(TOKEN);
    expect(launchToken({} as Window)).toBeUndefined();
    expect(launchToken({ [LAUNCH_TOKEN_GLOBAL]: 42 } as unknown as Window)).toBeUndefined();
  });

  it("prefers the launch token on the desktop and the supplied token elsewhere", async () => {
    const calls: (string | null)[] = [];
    const fetch = vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => {
      calls.push(new Headers(init?.headers).get("authorization"));
      return new Response("{}");
    });
    vi.stubGlobal("fetch", fetch);
    const navigator = { userAgent: "Mozilla/5.0 (Macintosh; Mac OS X)" };
    const desktop = { __TAURI_INTERNALS__: {}, navigator, [LAUNCH_TOKEN_GLOBAL]: "launch" };
    await createHostClient(desktop as unknown as Window, () => "typed").health();
    await createHostClient({ navigator } as unknown as Window, () => "typed").health();
    vi.unstubAllGlobals();
    expect(calls).toEqual(["Bearer launch", "Bearer typed"]);
  });

  it("keeps a token entered at run time in memory only, never in Web Storage", () => {
    const setItem = vi.spyOn(Storage.prototype, "setItem");
    const store = memoryTokenStore();
    expect(store.get()).toBeUndefined();
    store.set(TOKEN);
    expect(store.get()).toBe(TOKEN);
    expect(setItem).not.toHaveBeenCalled();
    expect(JSON.stringify({ ...localStorage, ...sessionStorage })).not.toContain(TOKEN);
    setItem.mockRestore();
  });

  it("notifies the listener after a token is set, without passing the token", () => {
    const seen: unknown[][] = [];
    const store = memoryTokenStore((...args: unknown[]) => seen.push(args));
    store.set(TOKEN);
    expect(seen).toEqual([[]]);
  });
});

describe("event channel subprotocols", () => {
  it("offers filesquid.events.v1 and filesquid.bearer.<token>, events first", () => {
    const s = socketRecorder();
    const client = new FileSquidClient({
      baseUrl: "",
      eventsUrl: "ws://127.0.0.1:4173/api/v1/events",
      WebSocket: s.WebSocket,
      token: TOKEN,
    });
    client.subscribe(() => undefined)();
    expect(EVENTS_SUBPROTOCOL).toBe("filesquid.events.v1");
    expect(s.opened).toEqual([
      {
        url: "ws://127.0.0.1:4173/api/v1/events",
        protocols: ["filesquid.events.v1", `filesquid.bearer.${TOKEN}`],
      },
    ]);
    expect(s.opened[0]?.url).not.toContain(TOKEN);
  });

  it("offers only filesquid.events.v1 without a token", () => {
    expect(eventProtocols(undefined)).toEqual(["filesquid.events.v1"]);
  });

  it("refuses an unusable token before opening a socket, without echoing it", () => {
    const s = socketRecorder();
    const bad = "bad token\nwith spaces";
    const client = new FileSquidClient({
      baseUrl: "",
      eventsUrl: "ws://h/api/v1/events",
      WebSocket: s.WebSocket,
      token: bad,
    });
    let thrown: unknown;
    try {
      client.subscribe(() => undefined);
    } catch (error) {
      thrown = error;
    }
    expect(thrown).toBeInstanceOf(ApiError);
    expect((thrown as ApiError).reason).toBe("invalid_credentials");
    expect(String((thrown as ApiError).message)).not.toContain("bad token");
    expect(s.opened).toEqual([]);
  });
});

describe("denial reasons", () => {
  it.each([
    [401, "unauthorized", "missing_credentials"],
    [403, "forbidden", "invalid_credentials"],
    [403, "forbidden", "not_permitted"],
  ])("HTTP %i %s carries reason %s in the typed error", async (status, code, reason) => {
    const r = recorder(status, denial(code, reason));
    const error = await new FileSquidClient({ baseUrl: "", fetch: r.fetch, token: TOKEN })
      .adminHealth()
      .catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({ status, code, reason, retryable: false });
    expect((error as ApiError).message).toBe(
      `FileSquid API /api/v1/admin/health failed with HTTP ${status} (${reason})`,
    );
  });

  it("leaves reason undefined for errors that are not denials", async () => {
    const body = JSON.stringify({ code: "unavailable", message: "Busy.", retryable: true });
    const error = (await new FileSquidClient({ baseUrl: "", fetch: recorder(503, body).fetch })
      .health()
      .catch((e: unknown) => e)) as ApiError;
    expect(error).toMatchObject({ status: 503, code: "unavailable", retryable: true });
    expect(error.reason).toBeUndefined();
  });

  it("ignores a reason outside the contract's DenialReason enum", async () => {
    const r = recorder(403, denial("forbidden", "something_else"));
    const error = (await new FileSquidClient({ baseUrl: "", fetch: r.fetch })
      .health()
      .catch((e: unknown) => e)) as ApiError;
    expect(error.code).toBe("forbidden");
    expect(error.reason).toBeUndefined();
  });

  it("never puts the token into the error, even when the server echoes it", async () => {
    const echo = JSON.stringify({ code: "forbidden", message: TOKEN, reason: "not_permitted" });
    const error = (await new FileSquidClient({
      baseUrl: "",
      fetch: recorder(403, echo).fetch,
      token: TOKEN,
    })
      .health()
      .catch((e: unknown) => e)) as ApiError;
    expect(JSON.stringify(error)).not.toContain(TOKEN);
    expect(error.message).not.toContain(TOKEN);
    expect(String(error.stack)).not.toContain(TOKEN);
  });

  it("refuses an unusable HTTP token before sending (status 0, invalid_credentials)", async () => {
    const r = recorder();
    const bad = "line\nbreak";
    const error = (await new FileSquidClient({ baseUrl: "", fetch: r.fetch, token: bad })
      .health()
      .catch((e: unknown) => e)) as ApiError;
    expect(r.calls).toEqual([]);
    expect(error).toMatchObject({ status: 0, reason: "invalid_credentials", retryable: false });
    expect(error.message).not.toContain("line");
  });
});

describe("isUsableToken", () => {
  it("accepts RFC 7230 token characters and rejects anything else", () => {
    expect(isUsableToken(TOKEN)).toBe(true);
    expect(isUsableToken("a.b-c_d~e")).toBe(true);
    expect(isUsableToken("")).toBe(false);
    expect(isUsableToken("has space")).toBe(false);
    expect(isUsableToken("comma,list")).toBe(false);
    expect(isUsableToken("ünicode")).toBe(false);
  });
});
