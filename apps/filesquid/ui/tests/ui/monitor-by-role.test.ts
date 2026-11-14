/**
 * UI tests for the Monitor link and route by role (UI-F2.21; UI-S-032, UI-S-026, UI-S-021): with
 * a host that answers the admin health route 403 the header shows no Monitor button, the Logs
 * row is hidden with it, and `#/monitor` shows `stateDenied` with no further admin call; with
 * the owner's token the button and the dashboard are shown as before. Owner decision
 * 2026-10-31, verbatim: "accept recommendations".
 */
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  AdminAccessStore,
  MONITOR_ROUTE,
  bindMonitorRoute,
  defineElements,
  type FsAppShell,
  type FsMonitor,
  type MonitorClient,
  type TokenStore,
} from "../../src/index";
import type { Distribution } from "../../src/distribution";
import labels from "../../src/theme/labels.en.json";
import { fakeThemeStore } from "../support/theme-fakes";
import { hostFakes } from "../support/host-fakes";

const distribution: Distribution = {
  id: "filesquid",
  productName: "FileSquid",
  version: "0.1.0",
  logo: { src: "/filesquid-mark.svg", alt: "FileSquid logo" },
  labels,
};

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** The 403 role refusal the API sends a caller who is not an administrator (API-S-004). */
const refusal = () => new ApiError(403, "/api/v1/admin/health", { reason: "not_permitted" });

const health = {
  status: "ok" as const,
  reasons: [],
  checks: {
    stalled: 0,
    store_ok: true,
    data_dir_writable: true,
    free_bytes: 1024 ** 3,
    queue_fill: 0,
    errors_last_5m: 0,
    ocfl_error_codes: [],
    ocfl_error_objects: 0,
  },
};

/** A Monitor client that counts every admin call and every subscription. */
function countingClient(refuse: boolean) {
  const calls: string[] = [];
  const answer = <T>(name: string, value: T) => {
    calls.push(name);
    return refuse ? Promise.reject(refusal()) : Promise.resolve(value);
  };
  const client: MonitorClient = {
    adminHealth: () => answer("adminHealth", health),
    adminProcesses: () => answer("adminProcesses", []),
    adminJournal: () => answer("adminJournal", { entries: [], next_cursor: null }),
    adminMetrics: () => answer("adminMetrics", {}),
    adminJournalExport: () => answer("adminJournalExport", ""),
    adminOcflFindings: () =>
      answer("adminOcflFindings", {
        findings: [],
        truncated: false,
        settings: { interval_ms: 1, object_cap: 1 },
        libraries: [],
      }),
    startOcflValidation: () => answer("startOcflValidation", { started: [], refused: [] }),
    subscribe: () => {
      calls.push("subscribe");
      return () => undefined;
    },
  };
  return { client, calls };
}

/** Mount a shell on the desktop host fakes with the given access store. */
function mountShell(access: AdminAccessStore): FsAppShell {
  const shell = document.createElement("fs-app-shell") as FsAppShell;
  shell.configure({ distribution, client: hostFakes(), theme: fakeThemeStore().store, access });
  document.body.replaceChildren(shell);
  return shell;
}

/** The header's Monitor button. */
const monitorButton = (shell: FsAppShell) =>
  shell.shadowRoot?.querySelector<HTMLElement>(`header a[href="${MONITOR_ROUTE}"]`);

/** The sidebar's Logs row. */
const logsRow = (shell: FsAppShell) =>
  shell.shadowRoot?.querySelector<HTMLElement>("nav ul.tools li.logs");

describe("Monitor link and route by role (UI-S-032)", () => {
  let unbind: () => void = () => undefined;
  beforeEach(() => defineElements());
  afterEach(() => {
    unbind();
    unbind = () => undefined;
    window.location.hash = "";
  });

  it("shows no Monitor button and no Logs row when the admin health route answers 403", async () => {
    const fake = countingClient(true);
    const access = new AdminAccessStore(fake.client, () => true);
    const shell = mountShell(access);
    await settle();
    expect(monitorButton(shell)?.hidden).toBe(false);
    await access.probe();
    expect(monitorButton(shell)?.hidden).toBe(true);
    expect(logsRow(shell)?.hidden).toBe(true);
  });

  it("shows the Monitor button and the Logs row to the desktop owner", async () => {
    const fake = countingClient(false);
    const access = new AdminAccessStore(fake.client, () => true);
    const shell = mountShell(access);
    await access.probe();
    await settle();
    expect(monitorButton(shell)?.hidden).toBe(false);
    expect(monitorButton(shell)?.textContent).toBe(labels.monitor);
    expect(logsRow(shell)?.hidden).toBe(false);
  });

  it("shows the button again once a new credential is granted", async () => {
    let refuse = true;
    const access = new AdminAccessStore(
      { adminHealth: () => (refuse ? Promise.reject(refusal()) : Promise.resolve(health)) },
      () => true,
    );
    const shell = mountShell(access);
    await access.probe();
    expect(monitorButton(shell)?.hidden).toBe(true);
    refuse = false;
    await access.probe();
    expect(monitorButton(shell)?.hidden).toBe(false);
  });

  it("shows stateDenied on a direct visit to #/monitor, with no further admin call", async () => {
    const fake = countingClient(true);
    const access = new AdminAccessStore(fake.client, () => true);
    const shell = document.createElement("div");
    document.body.replaceChildren(shell);
    window.location.hash = MONITOR_ROUTE;
    void access.probe();
    unbind = bindMonitorRoute(shell, { labels, client: fake.client, pollMs: 0, access });
    await settle();
    await settle();
    const monitor = shell.querySelector<FsMonitor>("fs-monitor");
    const states = monitor?.shadowRoot?.querySelector<HTMLElement>("fs-states");
    expect(states?.dataset.state).toBe("denied");
    expect(states?.shadowRoot?.querySelector(".title")?.textContent).toBe(labels.stateDenied);
    expect(states?.shadowRoot?.querySelector(".detail")?.textContent).toBe(labels.authNotPermitted);
    expect(monitor?.shadowRoot?.querySelector("form[data-form='token']")).toBeNull();
    expect(monitor?.shadowRoot?.querySelector("[data-section]")).toBeNull();
    expect(fake.calls).toEqual(["adminHealth"]);
  });

  it("shows stateDenied, not the sign-in form, when its own read is refused on the server host", async () => {
    const fake = countingClient(true);
    const access = new AdminAccessStore(fake.client, () => false);
    const tokens: TokenStore = { get: () => "t", set: () => undefined };
    const shell = mountShell(access);
    window.location.hash = MONITOR_ROUTE;
    unbind = bindMonitorRoute(shell, { labels, client: fake.client, pollMs: 0, access, tokens });
    await settle();
    await settle();
    const monitor = shell.querySelector<FsMonitor>("fs-monitor");
    expect(monitor?.shadowRoot?.querySelector("fs-states")?.getAttribute("data-state")).toBe(
      "denied",
    );
    expect(monitor?.shadowRoot?.querySelector("form[data-form='token']")).toBeNull();
    expect(access.state).toBe("denied");
    expect(monitorButton(shell)?.hidden).toBe(true);
  });

  it("shows the dashboard as before with the owner's token", async () => {
    const fake = countingClient(false);
    const access = new AdminAccessStore(fake.client, () => true);
    const shell = document.createElement("div");
    document.body.replaceChildren(shell);
    window.location.hash = MONITOR_ROUTE;
    void access.probe();
    unbind = bindMonitorRoute(shell, { labels, client: fake.client, pollMs: 0, access });
    await settle();
    await settle();
    const monitor = shell.querySelector<FsMonitor>("fs-monitor");
    expect(monitor?.shadowRoot?.querySelector('[data-section="health"]')).not.toBeNull();
    expect(monitor?.shadowRoot?.querySelector("fs-states[data-state='denied']")).toBeNull();
  });

  it("keeps the sign-in form for missing credentials on the server host", async () => {
    const fake = countingClient(false);
    fake.client.adminHealth = () =>
      Promise.reject(new ApiError(401, "/api/v1/admin/health", { reason: "missing_credentials" }));
    const access = new AdminAccessStore(fake.client, () => false);
    const tokens: TokenStore = { get: () => undefined, set: () => undefined };
    const shell = mountShell(access);
    window.location.hash = MONITOR_ROUTE;
    unbind = bindMonitorRoute(shell, { labels, client: fake.client, pollMs: 0, access, tokens });
    await settle();
    await settle();
    const monitor = shell.querySelector<FsMonitor>("fs-monitor");
    expect(monitor?.shadowRoot?.querySelector("form[data-form='token']")).not.toBeNull();
    expect(monitorButton(shell)?.hidden).toBe(false);
  });
});
