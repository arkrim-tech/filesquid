/**
 * UI tests for <fs-monitor> (SRS-F-061, Monitor dashboard): strict DOM assertions on
 * health, processes, storage, journal, live updates, the admin sign-in and safe rendering.
 */
import { beforeEach, describe, expect, it } from "vitest";
import { ApiError, type ApiEvent } from "../../src/api/client";
import { FsMonitor, defineElements, type MonitorClient, type TokenStore } from "../../src/index";
import labels from "../../src/theme/labels.en.json";

const GIB = 1024 ** 3;
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

const health = {
  status: "degraded" as const,
  reasons: [
    { code: "process_stalled", severity: "degraded" as const, detail: "1 process(es) stalled" },
  ],
  checks: {
    stalled: 1,
    store_ok: true,
    data_dir_writable: true,
    free_bytes: 10 * GIB,
    queue_fill: 0.1,
    errors_last_5m: 0,
    ocfl_error_codes: [],
    ocfl_error_objects: 0,
  },
};
const importProcess = {
  id: "import-1",
  kind: "import",
  state: "stalled" as const,
  done: 5,
  total: 10,
  started: "2026-10-15T10:00:00.000Z",
  last_heartbeat: "2026-10-15T10:00:05.000Z",
  heartbeat_timeout_ms: 30000,
  rate_per_second: 0.5,
  eta_seconds: 10,
  error_code: null,
};
const entry = (id: number, operation: string) => ({
  id,
  time: "2026-10-15T10:00:00.000Z",
  host: "filesquid-server",
  actor: "system",
  operation,
  outcome: "succeeded" as const,
  level: "info" as const,
  target_ids: [],
});

/** Fake client; `fail` makes every admin call reject with that status once. */
function fakeClient(fail?: number) {
  let listener: ((e: ApiEvent) => void) | undefined;
  let failing = fail;
  const guard = <T>(value: T) =>
    failing ? Promise.reject(new ApiError(failing, "/api/v1/admin")) : Promise.resolve(value);
  const client: MonitorClient = {
    adminHealth: () => guard(health),
    adminProcesses: () => guard([importProcess]),
    adminJournal: () =>
      guard({
        entries: [entry(1, "host.started"), entry(2, "<img src=x onerror=alert(1)>")],
        next_cursor: null,
      }),
    adminMetrics: () =>
      guard({
        filesquid_journal_entries_total: 2,
        filesquid_errors_total: 0,
        filesquid_journal_queue_depth: 0,
      }),
    adminJournalExport: () => guard('{"operation":"host.started"}\n'),
    subscribe: (l) => {
      listener = l;
      return () => (listener = undefined);
    },
  };
  return { client, emit: (e: ApiEvent) => listener?.(e), unfail: () => (failing = undefined) };
}

function mount(client: MonitorClient, tokens?: TokenStore) {
  const el = document.createElement("fs-monitor") as FsMonitor;
  el.configure({ labels, client, tokens, pollMs: 0 });
  document.body.replaceChildren(el);
  return el;
}

describe("<fs-monitor>", () => {
  beforeEach(() => defineElements());

  it("is registered and shows health with its reasons", async () => {
    expect(customElements.get("fs-monitor")).toBe(FsMonitor);
    const el = mount(fakeClient().client);
    await settle();
    const section = el.shadowRoot?.querySelector('[data-section="health"]');
    expect(section?.getAttribute("data-status")).toBe("degraded");
    expect(section?.querySelector("h2")?.textContent).toBe(labels.monitorHealth);
    expect(section?.querySelector(".status")?.textContent).toBe(labels.statusDegraded);
    expect([...(section?.querySelectorAll("li") ?? [])].map((li) => li.textContent)).toEqual([
      "1 process(es) stalled",
    ]);
  });

  it("lists processes with state, progress, rate and ETA", async () => {
    const el = mount(fakeClient().client);
    await settle();
    const row = el.shadowRoot?.querySelector('tr[data-process-id="import-1"]');
    const cells = [...(row?.querySelectorAll("td") ?? [])].map((td) => td.textContent);
    expect(cells).toEqual(["import", "stalled", "5 / 10", "0.5/s", "10 s"]);
    expect(row?.getAttribute("data-state")).toBe("stalled");
  });

  it("shows storage and queue facts from the health checks and metrics", async () => {
    const el = mount(fakeClient().client);
    await settle();
    const storage = el.shadowRoot?.querySelector('[data-section="storage"]')?.textContent ?? "";
    expect(storage).toContain("10.0 GiB");
    const rates = el.shadowRoot?.querySelector('[data-section="rates"]')?.textContent ?? "";
    expect(rates).toContain("2");
  });

  it("renders journal entries as text, never as markup", async () => {
    const el = mount(fakeClient().client);
    await settle();
    const rows = [...(el.shadowRoot?.querySelectorAll('[data-section="journal"] tbody tr') ?? [])];
    expect(rows.map((r) => r.querySelectorAll("td")[1]?.textContent)).toEqual([
      "host.started",
      "<img src=x onerror=alert(1)>",
    ]);
    expect(el.shadowRoot?.querySelector('[data-section="journal"] img')).toBeNull();
  });

  it("applies live process, health and journal events without reloading", async () => {
    const fake = fakeClient();
    const el = mount(fake.client);
    await settle();
    fake.emit({ topic: "process.update", data: { ...importProcess, state: "running" } });
    fake.emit({ topic: "health.change", data: { ...health, status: "ok", reasons: [] } });
    fake.emit({ topic: "journal.entry", data: entry(3, "ocfl.commit") });
    const root = el.shadowRoot;
    expect(root?.querySelector('tr[data-process-id="import-1"]')?.getAttribute("data-state")).toBe(
      "running",
    );
    expect(root?.querySelector('[data-section="health"]')?.getAttribute("data-status")).toBe("ok");
    const first = root?.querySelector('[data-section="journal"] tbody tr td:nth-child(2)');
    expect(first?.textContent).toBe("ocfl.commit");
  });

  it("asks for the admin token on 401 and retries with it", async () => {
    const fake = fakeClient(401);
    let stored: string | undefined;
    const tokens: TokenStore = { get: () => stored, set: (t) => (stored = t) };
    const el = mount(fake.client, tokens);
    await settle();
    const form = el.shadowRoot?.querySelector("form[data-form='token']");
    expect(form).not.toBeNull();
    const input = form?.querySelector("input") as HTMLInputElement;
    expect(input.type).toBe("password");
    expect(form?.querySelector("label")?.textContent).toBe(labels.monitorAdminToken);
    fake.unfail();
    input.value = "s3cret";
    form?.dispatchEvent(new Event("submit", { cancelable: true }));
    await settle();
    expect(stored).toBe("s3cret");
    expect(el.shadowRoot?.querySelector('[data-section="health"]')).not.toBeNull();
  });

  it.each([
    ["missing_credentials", "status", "Missing credentials"],
    ["invalid_credentials", "alert", "Invalid credentials"],
    ["not_permitted", "alert", "Not permitted"],
  ] as const)("shows %s distinctly above the token form", async (reason, role, text) => {
    const status = reason === "missing_credentials" ? 401 : 403;
    const client = fakeClient().client;
    client.adminHealth = () => Promise.reject(new ApiError(status, "/api/v1/admin", { reason }));
    const tokens: TokenStore = { get: () => undefined, set: () => undefined };
    const el = mount(client, tokens);
    await settle();
    const note = el.shadowRoot?.querySelector(`[data-reason="${reason}"]`);
    expect(note?.textContent).toBe(text);
    expect(note?.getAttribute("role")).toBe(role);
    expect(el.shadowRoot?.querySelector("form[data-form='token']")).not.toBeNull();
  });

  it("shows the reason label, not a token form, without a token store (desktop)", async () => {
    const client = fakeClient().client;
    const reason = "invalid_credentials";
    client.adminHealth = () => Promise.reject(new ApiError(403, "/api/v1/admin", { reason }));
    const el = mount(client);
    await settle();
    const alert = el.shadowRoot?.querySelector('[role="alert"]');
    expect(alert?.textContent).toBe("Monitor unavailable: Invalid credentials");
    expect(el.shadowRoot?.querySelector("form[data-form='token']")).toBeNull();
  });

  it("shows a refused subscription as a denial instead of throwing", async () => {
    const client = fakeClient().client;
    client.subscribe = () => {
      throw new ApiError(0, "/api/v1/events", { reason: "invalid_credentials" });
    };
    const tokens: TokenStore = { get: () => undefined, set: () => undefined };
    const el = mount(client, tokens);
    await settle();
    expect(el.shadowRoot?.querySelector('[data-reason="invalid_credentials"]')).not.toBeNull();
  });

  it("clears the typed token from the form after submitting", async () => {
    const fake = fakeClient(401);
    const tokens: TokenStore = { get: () => undefined, set: () => undefined };
    const el = mount(fake.client, tokens);
    await settle();
    const form = el.shadowRoot?.querySelector("form[data-form='token']");
    const input = form?.querySelector("input") as HTMLInputElement;
    input.value = "s3cret";
    form?.dispatchEvent(new Event("submit", { cancelable: true }));
    expect(input.value).toBe("");
    await settle();
    expect(el.shadowRoot?.innerHTML).not.toContain("s3cret");
  });

  it("keeps journal entries published while the first page loads, each once", async () => {
    const fake = fakeClient();
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const load = fake.client.adminJournal;
    fake.client.adminJournal = async (f) => {
      await gate;
      return load(f);
    };
    const el = mount(fake.client);
    await settle();
    fake.emit({ topic: "journal.entry", data: entry(3, "ocfl.commit") });
    fake.emit({ topic: "journal.entry", data: entry(2, "<img src=x onerror=alert(1)>") });
    release();
    await settle();
    const ops = [
      ...(el.shadowRoot?.querySelectorAll('[data-section="journal"] tbody tr') ?? []),
    ].map((r) => r.querySelectorAll("td")[1]?.textContent);
    expect(ops).toEqual(["ocfl.commit", "<img src=x onerror=alert(1)>", "host.started"]);
  });

  it("sets nothing up when removed before the first load finishes", async () => {
    const fake = fakeClient();
    let subscriptions = 0;
    let open = 0;
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const health = fake.client.adminHealth;
    fake.client.adminHealth = async () => {
      await gate;
      return health();
    };
    fake.client.subscribe = () => {
      subscriptions += 1;
      open += 1;
      return () => (open -= 1);
    };
    const el = document.createElement("fs-monitor") as FsMonitor;
    el.configure({ labels, client: fake.client, pollMs: 10 });
    document.body.replaceChildren(el);
    await settle();
    el.remove();
    release();
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(subscriptions).toBe(1);
    expect(open).toBe(0);
    expect(el.shadowRoot?.querySelector('[data-section="health"]')).toBeNull();
  });
});
