/**
 * FileSquid API client.
 *
 * Every UI reaches FileSquid only through this client (FileSquid U-16). It hides which
 * host answers: `filesquid-server` over HTTP and WebSocket, or the desktop host through
 * the `filesquid://` custom URI scheme and Tauri events (ADR 030).
 *
 * Authorisation is deny-by-default (API-S-004): every request carries
 * `Authorization: Bearer <token>`, and the event channel WebSocket offers the token as the
 * subprotocol `filesquid.bearer.<token>`. The client keeps no copy of the token: it asks its
 * token source on every call, never stores, logs or renders it, and never puts it in an error.
 */
import type { components } from "./schema";

/** Process health payload, generated from the API contract. */
export type HealthStatus = components["schemas"]["HealthStatus"];

/** One push event from the API event channel. */
export type ApiEvent = components["schemas"]["ApiEvent"];

/** Error code of a failed request, generated from the API contract. */
export type ErrorCode = components["schemas"]["ErrorCode"];

/** Why the authorisation layer refused a call, generated from the API contract. */
export type DenialReason = components["schemas"]["DenialReason"];

/** Body of every failed API request, generated from the API contract. */
export type ApiErrorBody = components["schemas"]["ApiError"];

/** Callback for push events. */
export type EventListener = (event: ApiEvent) => void;

/** Monitor health report (`GET /api/v1/admin/health`). */
export type HealthReport = components["schemas"]["HealthReport"];
/** One live or recently finished process. */
export type ProcessView = components["schemas"]["ProcessView"];
/** One activity journal entry. */
export type JournalEntry = components["schemas"]["JournalEntry"];
/** One page of journal entries. */
export type JournalPage = components["schemas"]["JournalPage"];
/** Journal entry outcome. */
export type Outcome = components["schemas"]["Outcome"];
/** Journal entry level. */
export type Level = components["schemas"]["Level"];
/** Metric name to value (same values as Prometheus `/metrics`). */
export type MetricsSnapshot = Record<string, number>;

/** Filters for journal queries and exports; every field is optional. */
export interface JournalFilters {
  /** Earliest time, RFC 3339. */
  from?: string;
  /** Latest time, RFC 3339. */
  to?: string;
  /** Exact actor. */
  actor?: string;
  /** Exact operation, for example `ocfl.commit`. */
  operation?: string;
  /** Exact outcome. */
  outcome?: Outcome;
  /** Exact level. */
  level?: Level;
  /** Exact process id. */
  process?: string;
  /** Cursor from the previous page. */
  after?: number;
  /** Page size. */
  limit?: number;
  /** `desc` for newest first (the Monitor dashboard); `asc` (default) otherwise. */
  order?: "asc" | "desc";
}

/** Response header naming the cursor to continue a truncated journal export. */
const NEXT_CURSOR_HEADER = "X-Journal-Next-Cursor";
/** Export requests followed at most (each up to 1 000 000 entries on the server). */
const MAX_EXPORT_REQUESTS = 10;

/** Subprotocol of the event channel; the server selects it on a successful upgrade. */
export const EVENTS_SUBPROTOCOL = "filesquid.events.v1";
/** Prefix of the subprotocol that carries the bearer token on a browser WebSocket. */
const BEARER_SUBPROTOCOL_PREFIX = "filesquid.bearer.";
/** Every {@link DenialReason} the contract defines, for validating response bodies. */
const DENIAL_REASONS: readonly DenialReason[] = [
  "missing_credentials",
  "invalid_credentials",
  "not_permitted",
];
/** RFC 7230 `token`: the only characters both a header value and a subprotocol accept here. */
const TOKEN_CHARS = /^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/;

/** Build a query string from defined, non-empty filter values (in declaration order). */
function queryString(filters: JournalFilters): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(filters)) {
    if (value !== undefined && value !== "") params.set(key, String(value));
  }
  const text = params.toString();
  return text ? `?${text}` : "";
}

/** Facts about the host the UI is running in. */
export interface HostEnvironment {
  /** True inside a Tauri desktop webview. */
  isTauri: boolean;
  /** Operating system family; only Windows changes the scheme URL form. */
  platform: "windows" | "android" | "macos" | "linux" | "other";
}

/**
 * Error raised when the API answers with a non-2xx status, or when the client refuses to send
 * an unusable credential (`status` 0). Carries the contract's `code`, `retryable` and, for
 * authorisation denials, `reason`, so the UI can tell missing credentials, invalid credentials
 * and not permitted apart. The message names the path, status and reason only: never the
 * token, and never the server's free-text message.
 */
export class ApiError extends Error {
  /** Error code from the response body, when the body was a contract `ApiError`. */
  public readonly code?: ErrorCode;
  /** Authorisation denial reason (401 and 403 only). */
  public readonly reason?: DenialReason;
  /** True when repeating the same request unchanged may succeed. */
  public readonly retryable: boolean;

  /**
   * @param status HTTP status code returned by the API, or 0 when nothing was sent.
   * @param path API path that failed.
   * @param body Parsed contract error body, when there was one.
   */
  constructor(
    public readonly status: number,
    public readonly path: string,
    body: Partial<ApiErrorBody> = {},
  ) {
    const detail = body.reason ?? body.code;
    super(`FileSquid API ${path} failed with HTTP ${status}${detail ? ` (${detail})` : ""}`);
    this.name = "ApiError";
    this.code = body.code;
    this.reason = body.reason;
    this.retryable = body.retryable ?? false;
  }
}

/**
 * Supplies the bearer token: a fixed string configured by the operator, or a function asked
 * on every request (the desktop launch token, or a token typed in at run time).
 */
export type TokenSource = string | (() => string | undefined);

/**
 * True when `token` can be sent as a bearer header value and as a WebSocket subprotocol
 * (RFC 7230 `token` characters, non-empty). Pure.
 *
 * @param token Candidate credential.
 * @returns Whether the client may send it.
 */
export function isUsableToken(token: string): boolean {
  return TOKEN_CHARS.test(token);
}

/**
 * The subprotocols a browser offers on the event channel: `filesquid.events.v1` first, then
 * `filesquid.bearer.<token>` when a token is known (API-S-004). Pure.
 *
 * @param token The bearer token, if any; callers check it with {@link isUsableToken} first.
 * @returns The list passed to the `WebSocket` constructor.
 */
export function eventProtocols(token: string | undefined): string[] {
  return token
    ? [EVENTS_SUBPROTOCOL, `${BEARER_SUBPROTOCOL_PREFIX}${token}`]
    : [EVENTS_SUBPROTOCOL];
}

/**
 * Keep only the contract fields of a parsed error body; anything else is dropped. Pure.
 *
 * @param value Parsed JSON of a failed response.
 * @returns The code, retryable flag and a reason from the contract's enum.
 */
export function errorBody(value: unknown): Partial<ApiErrorBody> {
  if (typeof value !== "object" || value === null) return {};
  const record = value as Record<string, unknown>;
  const reason = DENIAL_REASONS.find((known) => known === record.reason);
  return {
    code: typeof record.code === "string" ? (record.code as ErrorCode) : undefined,
    retryable: typeof record.retryable === "boolean" ? record.retryable : undefined,
    reason,
  };
}

/**
 * Read the contract error body of a failed response. A body that is not JSON (for example a
 * proxy's HTML page) yields no fields; that case is logged as a warning without the body.
 * Network I/O: consumes the response body.
 *
 * @param response A non-2xx response.
 * @returns The contract fields found.
 */
async function readErrorBody(response: Response): Promise<Partial<ApiErrorBody>> {
  try {
    return errorBody(JSON.parse(await response.text()));
  } catch (error) {
    console.warn(`FileSquid API error body (HTTP ${response.status}) was not JSON`, error);
    return {};
  }
}

/**
 * The error raised when the configured token cannot be sent. It names neither the token nor
 * the characters that made it unusable.
 *
 * @param path API path the call was for.
 */
function unusableToken(path: string): ApiError {
  return new ApiError(0, path, {
    code: "forbidden",
    reason: "invalid_credentials",
    retryable: false,
  });
}

/** Options for {@link FileSquidClient}. Transports are injectable for tests. */
export interface ClientOptions {
  /** Prefix for API paths: "" (same origin), "filesquid://localhost" or "http://filesquid.localhost" (Windows, Android). */
  baseUrl: string;
  /** WebSocket URL of the event channel (server host only). */
  eventsUrl?: string;
  /** Subscribe function for desktop events (Tauri). When set, WebSocket is not used. */
  subscribeHost?: (listener: EventListener) => () => void;
  /** fetch implementation; defaults to the global fetch. */
  fetch?: typeof fetch;
  /** WebSocket constructor; defaults to the global WebSocket. */
  WebSocket?: typeof WebSocket;
  /**
   * Bearer token sent on every request and offered on the event channel. On the desktop host
   * {@link createHostClient} supplies the per-launch token; on `filesquid-server` the operator
   * passes the administrator token (or a function that returns it). Kept in memory only.
   */
  token?: TokenSource;
}

/**
 * Return the API base URL for a host environment.
 *
 * @param env Host facts from {@link detectHost}.
 * Tauri serves custom schemes as `http://<scheme>.localhost` on Windows and Android,
 * and as `<scheme>://localhost` elsewhere.
 *
 * @returns "" for same-origin HTTP, or the custom-scheme origin inside the Tauri app.
 */
export function apiBaseUrl(env: HostEnvironment): string {
  if (!env.isTauri) return "";
  const httpForm = env.platform === "windows" || env.platform === "android";
  return httpForm ? "http://filesquid.localhost" : "filesquid://localhost";
}

/**
 * Return the WebSocket URL of the event channel for a page served by `filesquid-server`.
 *
 * @param location The page URL.
 */
export function eventsUrl(location: URL): string {
  const scheme = location.protocol === "https:" ? "wss:" : "ws:";
  return `${scheme}//${location.host}/api/v1/events`;
}

/**
 * Detect the host environment from a window object.
 *
 * @param win The browser window (or a stand-in in tests).
 */
export function detectHost(win: Window): HostEnvironment {
  const isTauri = "__TAURI_INTERNALS__" in win;
  const agent = win.navigator.userAgent.toLowerCase();
  // Android user agents also contain "linux", so test for Android first.
  const platform = agent.includes("windows")
    ? "windows"
    : agent.includes("android")
      ? "android"
      : agent.includes("mac os")
        ? "macos"
        : agent.includes("linux")
          ? "linux"
          : "other";
  return { isTauri, platform };
}

/** Typed client for the FileSquid API. */
export class FileSquidClient {
  private readonly options: ClientOptions;

  /** @param options Base URL and injectable transports. */
  constructor(options: ClientOptions) {
    this.options = options;
  }

  /** Read process health (`GET /api/v1/health`). Network I/O. */
  async health(): Promise<HealthStatus> {
    return this.getJson<HealthStatus>("/api/v1/health");
  }

  /**
   * Subscribe to push events. Opens a WebSocket on the server host, or uses the
   * host-supplied subscription on the desktop host.
   *
   * The browser WebSocket offers the subprotocols from {@link eventProtocols}, because a
   * browser cannot set an `Authorization` header on an upgrade.
   *
   * @param listener Called once per event.
   * @returns A function that ends the subscription and releases its connection.
   * @throws ApiError (status 0, `invalid_credentials`) when the token cannot be sent.
   */
  subscribe(listener: EventListener): () => void {
    if (this.options.subscribeHost) return this.options.subscribeHost(listener);
    const path = "/api/v1/events";
    const token = this.currentToken(path);
    const Socket = this.options.WebSocket ?? globalThis.WebSocket;
    const url = this.options.eventsUrl ?? eventsUrl(new URL(globalThis.location.href));
    const socket = new Socket(url, eventProtocols(token));
    socket.onmessage = (message: MessageEvent<string>) => {
      try {
        listener(JSON.parse(message.data) as ApiEvent);
      } catch (error) {
        console.error("FileSquid event was not valid JSON", error);
      }
    };
    return () => socket.close();
  }

  /** Overall health with reasons (`GET /api/v1/admin/health`). Network I/O. */
  async adminHealth(): Promise<HealthReport> {
    return this.getJson<HealthReport>("/api/v1/admin/health");
  }

  /** Live and recently finished processes (`GET /api/v1/admin/processes`). Network I/O. */
  async adminProcesses(): Promise<ProcessView[]> {
    return this.getJson<ProcessView[]>("/api/v1/admin/processes");
  }

  /** One page of the activity journal (`GET /api/v1/admin/journal`). Network I/O. */
  async adminJournal(filters: JournalFilters = {}): Promise<JournalPage> {
    return this.getJson<JournalPage>(`/api/v1/admin/journal${queryString(filters)}`);
  }

  /** Counters and gauges (`GET /api/v1/admin/metrics`). Network I/O. */
  async adminMetrics(): Promise<MetricsSnapshot> {
    return this.getJson<MetricsSnapshot>("/api/v1/admin/metrics");
  }

  /**
   * All matching journal entries as JSON Lines text (`GET /api/v1/admin/journal/export`),
   * oldest first. A response cut at the server's size cap carries `X-Journal-Next-Cursor`;
   * this follows it for at most {@link MAX_EXPORT_REQUESTS} requests and throws if entries
   * still remain, rather than returning an incomplete export. Network I/O.
   */
  async adminJournalExport(
    filters: Omit<JournalFilters, "after" | "limit" | "order"> = {},
  ): Promise<string> {
    let text = "";
    let after: number | undefined;
    for (let i = 0; i < MAX_EXPORT_REQUESTS; i++) {
      const path = `/api/v1/admin/journal/export${queryString({ ...filters, after })}`;
      const response = await this.request(path);
      text += await response.text();
      const next = response.headers.get(NEXT_CURSOR_HEADER);
      if (next === null) return text;
      after = Number(next);
    }
    throw new Error(`journal export exceeds ${MAX_EXPORT_REQUESTS} requests; narrow the filters`);
  }

  /** GET a JSON resource under the base URL; throws {@link ApiError} on non-2xx. */
  private async getJson<T>(path: string): Promise<T> {
    const response = await this.request(path);
    return (await response.json()) as T;
  }

  /**
   * The current token from the configured source, checked before it is sent anywhere.
   *
   * @param path API path the token is for (named in the error).
   * @returns The token, or undefined when none is known.
   * @throws ApiError (status 0, `invalid_credentials`) when the token cannot be sent.
   */
  private currentToken(path: string): string | undefined {
    const source = this.options.token;
    const token = typeof source === "function" ? source() : source;
    if (token === undefined || token === "") return undefined;
    if (!isUsableToken(token)) throw unusableToken(path);
    return token;
  }

  /**
   * GET `path` with the bearer token, when one is known. Network I/O.
   *
   * @param path API path under the base URL.
   * @returns The 2xx response.
   * @throws ApiError on a non-2xx answer, with the body's code, reason and retryable flag.
   */
  private async request(path: string): Promise<Response> {
    const doFetch = this.options.fetch ?? globalThis.fetch.bind(globalThis);
    const token = this.currentToken(path);
    const init: RequestInit | undefined = token
      ? { headers: { Authorization: `Bearer ${token}` } }
      : undefined;
    const response = await doFetch(`${this.options.baseUrl}${path}`, init);
    if (!response.ok) throw new ApiError(response.status, path, await readErrorBody(response));
    return response;
  }
}
