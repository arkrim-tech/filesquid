/**
 * FileSquid API client.
 *
 * Every UI reaches FileSquid only through this client (FileSquid U-16, UI-S-012). It hides
 * which host answers: `filesquid-server` over HTTP and WebSocket, or the desktop host through
 * the `filesquid://` custom URI scheme and Tauri events (ADR 030). {@link selectHostAdapter}
 * picks the adapter at run time. Its interface is `request()` (JSON), `read()` (bytes, with
 * ranges), `command()` and `upload()` (mutations that return a `CommandOutcome`) and
 * `subscribe()` (push events), plus typed shortcuts for existing routes.
 *
 * Authorisation is deny-by-default (API-S-004): every request made with a known token carries
 * `Authorization: Bearer <token>`, and the event channel WebSocket offers that token as the
 * subprotocol `filesquid.bearer.<token>`. The client keeps no copy of the token: it asks its
 * token source on every call, never stores, logs or renders it, and never puts it in an error.
 */
import {
  commandOutcome,
  conflictOutcome,
  parseContentRange,
  rangeHeader,
  type ByteRange,
  type CommandEnvelope,
  type CommandOutcome,
  type ContentRange,
} from "./commands";
import type { CompleteList } from "./complete-list";
import type { components } from "./schema";

/** Process health payload, generated from the API contract. */
export type HealthStatus = components["schemas"]["HealthStatus"];

/** One push event from the API event channel. */
export type ApiEvent = components["schemas"]["ApiEvent"];

/** The host descriptor, generated from the API contract (`GET /api/v1/host`, API-S-020). */
export type HostDescriptor = components["schemas"]["HostDescriptor"];

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
/** HTTP status of a `conflict` answer: the command's expected revision did not match. */
const HTTP_CONFLICT = 409;
/** HTTP status of a partial (ranged) answer. */
const HTTP_PARTIAL = 206;
/** HTTP status of an answer with no body. */
const HTTP_NO_CONTENT = 204;

/** Literal {@link DenialReason} values; only checked, then used through {@link DENIAL_REASONS}. */
const DENIAL_REASON_VALUES = [
  "missing_credentials",
  "invalid_credentials",
  "not_permitted",
] as const satisfies readonly DenialReason[];
/** Every {@link DenialReason} the contract defines, for validating response bodies at run time. */
const DENIAL_REASONS: CompleteList<DenialReason, typeof DENIAL_REASON_VALUES> =
  DENIAL_REASON_VALUES;
/** Literal {@link ErrorCode} values; only checked, then used through {@link ERROR_CODES}. */
const ERROR_CODE_VALUES = [
  "invalid_request",
  "unauthorized",
  "forbidden",
  "not_found",
  "method_not_allowed",
  "conflict",
  "internal",
  "unavailable",
] as const satisfies readonly ErrorCode[];
/** Every {@link ErrorCode} the contract defines, for validating response bodies at run time. */
const ERROR_CODES: CompleteList<ErrorCode, typeof ERROR_CODE_VALUES> = ERROR_CODE_VALUES;
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
 * @returns A code and a reason from the contract's enums (others are dropped) and the
 *   retryable flag.
 */
export function errorBody(value: unknown): Partial<ApiErrorBody> {
  if (typeof value !== "object" || value === null) return {};
  const record = value as Record<string, unknown>;
  const reason = DENIAL_REASONS.find((known) => known === record.reason);
  const code = ERROR_CODES.find((known) => known === record.code);
  return {
    code,
    retryable: typeof record.retryable === "boolean" ? record.retryable : undefined,
    reason,
  };
}

/**
 * Read the contract error body of a failed response. A body that is not JSON (for example a
 * proxy's HTML page) yields no fields; that case is logged as a warning with the status only,
 * never the body or the parser's message (which can quote the body).
 * Network I/O: consumes the response body.
 *
 * @param response A non-2xx response.
 * @returns The contract fields found.
 */
async function readErrorBody(response: Response): Promise<Partial<ApiErrorBody>> {
  try {
    return errorBody(JSON.parse(await response.text()));
  } catch (error) {
    // The parser's message can quote the body, so only its kind and the status are logged.
    const kind = error instanceof Error ? error.name : typeof error;
    console.warn(`FileSquid API error body (HTTP ${response.status}) was not JSON (${kind})`);
    return {};
  }
}

/**
 * Parse the JSON body of a 2xx command answer. A body that is not JSON (for example a proxy
 * page) is not an outcome, so it is reported as the same `TypeError` as any other invalid
 * answer, with the parser's error as its `cause`. Reading the body happens outside that
 * conversion, so an abort or transport failure rejects with its own error.
 * Network I/O: consumes the response body.
 *
 * @param response A 2xx command answer.
 * @returns The parsed JSON value.
 * @throws TypeError when the body is not JSON.
 */
async function readJsonAnswer(response: Response): Promise<unknown> {
  const text = await response.text();
  try {
    return JSON.parse(text) as unknown;
  } catch (error) {
    throw new TypeError("command answer is not a CommandOutcome", { cause: error });
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
 * Which adapter answers the client: the desktop custom URI scheme with Tauri events, or
 * same-origin HTTP with the WebSocket event channel. The encrypted browser host (UI-F5.1)
 * adds its own kind here; views never see the difference.
 */
export interface HostAdapter {
  /** Adapter name. */
  kind: "desktop-scheme" | "server-http";
  /** Prefix for API paths, from {@link apiBaseUrl}. */
  baseUrl: string;
  /** Event transport: Tauri events on the desktop, the WebSocket channel on the server. */
  events: "tauri" | "websocket";
}

/**
 * Select the adapter for a host environment (ADR 030 Decision 3). Pure.
 *
 * @param env Host facts from {@link detectHost}.
 * @returns The adapter kind, base URL and event transport.
 */
export function selectHostAdapter(env: HostEnvironment): HostAdapter {
  const baseUrl = apiBaseUrl(env);
  return env.isTauri
    ? { kind: "desktop-scheme", baseUrl, events: "tauri" }
    : { kind: "server-http", baseUrl, events: "websocket" };
}

/** HTTP methods the API uses. */
export type HttpMethod = "GET" | "HEAD" | "POST" | "PUT" | "PATCH" | "DELETE";

/** Options for {@link FileSquidClient.request}. */
export interface RequestOptions {
  /** HTTP method; `GET` by default. */
  method?: HttpMethod;
  /** JSON-serialisable body; sent as `application/json` when given. */
  body?: unknown;
  /** Cancels the request, for example when a view is left. */
  signal?: AbortSignal;
}

/** Options for {@link FileSquidClient.read}. */
export interface ReadOptions {
  /** Inclusive byte range; the whole representation when omitted. */
  range?: ByteRange;
  /** Cancels the read. */
  signal?: AbortSignal;
}

/** Result of {@link FileSquidClient.read}. */
export interface ReadResult {
  /** The 2xx response; its body is not consumed yet. */
  response: Response;
  /** True for a 206 answer (part of the representation), false for the whole of it. */
  partial: boolean;
  /** The returned range of a partial answer, when the server named it. */
  contentRange?: ContentRange;
}

/** Options for {@link FileSquidClient.command}. */
export interface CommandOptions {
  /** HTTP method; `POST` by default. */
  method?: Exclude<HttpMethod, "GET" | "HEAD">;
  /** Cancels the request. The command may still apply; retry with the same operation id. */
  signal?: AbortSignal;
}

/** Options for {@link FileSquidClient.upload}. */
export interface UploadOptions {
  /** File name sent with the `file` part. */
  filename?: string;
  /** Cancels the upload. The command may still apply; retry with the same operation id. */
  signal?: AbortSignal;
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
    return this.request<HealthStatus>("/api/v1/health");
  }

  /**
   * Subscribe to push events. Opens a WebSocket on the server host, or uses the
   * host-supplied subscription on the desktop host.
   *
   * The browser WebSocket offers the subprotocols from {@link eventProtocols}, because a
   * browser cannot set an `Authorization` header on an upgrade.
   *
   * @param listener Called once per event.
   * @param onOpen Called each time the WebSocket channel opens, so a subscriber can read the
   *   state it shows again (events may have been missed while it was closed). Never called on
   *   the desktop host, whose event channel is in-process and does not disconnect.
   * @returns A function that ends the subscription and releases its connection.
   * @throws ApiError (status 0, `invalid_credentials`) when the token cannot be sent.
   */
  subscribe(listener: EventListener, onOpen?: () => void): () => void {
    if (this.options.subscribeHost) return this.options.subscribeHost(listener);
    const path = "/api/v1/events";
    const token = this.currentToken(path);
    const Socket = this.options.WebSocket ?? globalThis.WebSocket;
    const url = this.options.eventsUrl ?? eventsUrl(new URL(globalThis.location.href));
    const socket = new Socket(url, eventProtocols(token));
    if (onOpen) socket.onopen = () => onOpen();
    socket.onmessage = (message: MessageEvent<string>) => {
      try {
        listener(JSON.parse(message.data) as ApiEvent);
      } catch (error) {
        console.error("FileSquid event was not valid JSON", error);
      }
    };
    return () => socket.close();
  }

  /**
   * Read the host descriptor (`GET /api/v1/host`, API-S-020): host type, operator name and
   * the hosted backup and sync state. Network I/O.
   *
   * @returns The descriptor as the host sent it; callers validate it before showing it.
   * @throws ApiError when the host refuses the call or has not described itself yet.
   */
  async host(): Promise<HostDescriptor> {
    return this.request<HostDescriptor>("/api/v1/host");
  }

  /** Overall health with reasons (`GET /api/v1/admin/health`). Network I/O. */
  async adminHealth(): Promise<HealthReport> {
    return this.request<HealthReport>("/api/v1/admin/health");
  }

  /** Live and recently finished processes (`GET /api/v1/admin/processes`). Network I/O. */
  async adminProcesses(): Promise<ProcessView[]> {
    return this.request<ProcessView[]>("/api/v1/admin/processes");
  }

  /** One page of the activity journal (`GET /api/v1/admin/journal`). Network I/O. */
  async adminJournal(filters: JournalFilters = {}): Promise<JournalPage> {
    return this.request<JournalPage>(`/api/v1/admin/journal${queryString(filters)}`);
  }

  /** Counters and gauges (`GET /api/v1/admin/metrics`). Network I/O. */
  async adminMetrics(): Promise<MetricsSnapshot> {
    return this.request<MetricsSnapshot>("/api/v1/admin/metrics");
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
      const response = await this.send(path);
      text += await response.text();
      const next = response.headers.get(NEXT_CURSOR_HEADER);
      if (next === null) return text;
      after = Number(next);
    }
    throw new Error(`journal export exceeds ${MAX_EXPORT_REQUESTS} requests; narrow the filters`);
  }

  /**
   * Call a JSON route under the base URL. Network I/O.
   *
   * @param path API path, for example `/api/v1/health`.
   * @param options Method (`GET` by default), JSON body and abort signal.
   * @returns The parsed JSON body, or undefined for `204 No Content`.
   * @throws ApiError on a non-2xx answer, or (status 0) when the token cannot be sent.
   */
  async request<T>(path: string, options: RequestOptions = {}): Promise<T> {
    const hasBody = options.body !== undefined;
    const response = await this.send(path, {
      method: options.method ?? "GET",
      headers: hasBody ? { "Content-Type": "application/json" } : undefined,
      body: hasBody ? JSON.stringify(options.body) : undefined,
      signal: options.signal,
    });
    if (response.status === HTTP_NO_CONTENT) return undefined as T;
    return (await response.json()) as T;
  }

  /**
   * Read bytes, the whole representation or an inclusive range. Network I/O; the body is
   * left unread for the caller to stream.
   *
   * @param path API path of the content.
   * @param options Optional byte range and abort signal.
   * @returns The response, whether it is partial (206) and its `Content-Range`.
   * @throws RangeError for an invalid range, before anything is sent.
   * @throws ApiError on a non-2xx answer (for example 416), or when the token cannot be sent.
   */
  async read(path: string, options: ReadOptions = {}): Promise<ReadResult> {
    const range = options.range ? rangeHeader(options.range) : undefined;
    const response = await this.send(path, {
      method: "GET",
      headers: range ? { Range: range } : undefined,
      signal: options.signal,
    });
    const partial = response.status === HTTP_PARTIAL;
    const contentRange = partial
      ? parseContentRange(response.headers.get("Content-Range"))
      : undefined;
    return { response, partial, contentRange };
  }

  /**
   * Send a command: JSON body `{ envelope, input }`. Network I/O.
   *
   * @param path API path of the command.
   * @param envelope Envelope from `commandEnvelope`; reuse it to retry.
   * @param input Command input, omitted from the body when undefined.
   * @param options Method (`POST` by default) and abort signal.
   * @returns The outcome; only `committed` is durable completion evidence.
   * @throws ApiError on a non-2xx answer other than 409, or when the token cannot be sent.
   * @throws TypeError when the answer is not an outcome for this operation.
   */
  async command(
    path: string,
    envelope: CommandEnvelope,
    input?: unknown,
    options: CommandOptions = {},
  ): Promise<CommandOutcome> {
    const response = await this.transmit(path, {
      method: options.method ?? "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ envelope, input }),
      signal: options.signal,
    });
    return this.outcome(path, envelope.operation_id, response);
  }

  /**
   * Upload one file as `multipart/form-data` with an `envelope` part (JSON) and a `file`
   * part. The transport sets the multipart boundary. Network I/O.
   *
   * @param path API path of the upload.
   * @param file File bytes.
   * @param envelope Envelope from `commandEnvelope`; reuse it to retry.
   * @param options File name and abort signal.
   * @returns The outcome; only `committed` is durable completion evidence.
   * @throws ApiError on a non-2xx answer other than 409, or when the token cannot be sent.
   * @throws TypeError when the answer is not an outcome for this operation.
   */
  async upload(
    path: string,
    file: Blob,
    envelope: CommandEnvelope,
    options: UploadOptions = {},
  ): Promise<CommandOutcome> {
    const form = new FormData();
    form.append("envelope", new Blob([JSON.stringify(envelope)], { type: "application/json" }));
    form.append("file", file, options.filename ?? (file instanceof File ? file.name : "file"));
    const response = await this.transmit(path, {
      method: "POST",
      body: form,
      signal: options.signal,
    });
    return this.outcome(path, envelope.operation_id, response);
  }

  /**
   * Turn a command answer into its outcome: 2xx bodies are validated, 409 becomes
   * `conflict`, anything else is an {@link ApiError}. Network I/O: consumes the body.
   *
   * @param path API path, named in an error.
   * @param operationId The id the command was sent with.
   * @param response The raw answer.
   * @returns The validated outcome.
   */
  private async outcome(
    path: string,
    operationId: string,
    response: Response,
  ): Promise<CommandOutcome> {
    if (response.ok) return commandOutcome(await readJsonAnswer(response), operationId);
    if (response.status !== HTTP_CONFLICT) {
      throw new ApiError(response.status, path, await readErrorBody(response));
    }
    // The conflict body adds nothing the outcome needs; release it.
    await response.body?.cancel();
    return conflictOutcome(operationId);
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
    if (token === undefined) return undefined;
    if (!isUsableToken(token)) throw unusableToken(path);
    return token;
  }

  /**
   * Send `init` to `path` and require a 2xx answer. Network I/O.
   *
   * @param path API path under the base URL.
   * @param init Method, headers, body and signal.
   * @returns The 2xx response.
   * @throws ApiError on a non-2xx answer, with the body's code, reason and retryable flag.
   */
  private async send(path: string, init: RequestInit = {}): Promise<Response> {
    const response = await this.transmit(path, init);
    if (!response.ok) throw new ApiError(response.status, path, await readErrorBody(response));
    return response;
  }

  /**
   * Send `init` to `path` with the bearer token, when one is known, whatever the status of
   * the answer. Network I/O.
   *
   * @param path API path under the base URL.
   * @param init Method, headers, body and signal; `Authorization` is added here only.
   * @returns The response.
   * @throws ApiError (status 0, `invalid_credentials`) when the token cannot be sent.
   */
  private async transmit(path: string, init: RequestInit): Promise<Response> {
    const doFetch = this.options.fetch ?? globalThis.fetch.bind(globalThis);
    const token = this.currentToken(path);
    const headers = new Headers(init.headers);
    if (token) headers.set("Authorization", `Bearer ${token}`);
    return doFetch(`${this.options.baseUrl}${path}`, { ...init, headers });
  }
}
