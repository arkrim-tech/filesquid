/**
 * UI tests for UI-S-016 (accessibility contracts in the shell): the shell, the theme control
 * and the Monitor pass the accessibility audit in every state; the skip control, landmarks and
 * names come from the label table; focus is recovered after re-renders and route changes and
 * never moved on first load.
 */
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  MONITOR_ROUTE,
  accessibilityViolations,
  bindMonitorRoute,
  deepActiveElement,
  defineElements,
  type FsAppShell,
  type FsMonitor,
  type MonitorClient,
  type TokenStore,
} from "../../src/index";
import type { Distribution } from "../../src/distribution";
import labels from "../../src/theme/labels.en.json";
import { fakeThemeStore } from "../support/theme-fakes";

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

const distribution: Distribution = {
  id: "filesquid",
  productName: "FileSquid",
  logo: { src: "/filesquid-mark.svg", alt: "FileSquid logo" },
  labels,
};

/** Mount the shell with a healthy API. */
function mountShell(): FsAppShell {
  const shell = document.createElement("fs-app-shell") as FsAppShell;
  shell.configure({
    distribution,
    client: { health: async () => ({ status: "ok", product: "filesquid", schema_version: 1 }) },
    theme: fakeThemeStore().store,
  });
  document.body.replaceChildren(shell);
  return shell;
}

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
  },
};

/** A Monitor client whose admin calls fail with `status` while `failing()` is true. */
function monitorClient(failing: () => number | undefined): MonitorClient {
  const guard = <T>(value: T) => {
    const status = failing();
    return status ? Promise.reject(new ApiError(status, "/api/v1/admin")) : Promise.resolve(value);
  };
  return {
    adminHealth: () => guard(health),
    adminProcesses: () => guard([]),
    adminJournal: () => guard({ entries: [], next_cursor: null }),
    adminMetrics: () => guard({}),
    adminJournalExport: () => guard(""),
    subscribe: () => () => undefined,
  };
}

/** Mount the Monitor. */
function mountMonitor(client: MonitorClient, tokens?: TokenStore): FsMonitor {
  const monitor = document.createElement("fs-monitor") as FsMonitor;
  monitor.configure({ labels, client, tokens, pollMs: 0 });
  document.body.replaceChildren(monitor);
  return monitor;
}

/** Submit the Monitor's token form with `token`, from the keyboard's point of view. */
function submitToken(monitor: FsMonitor, token: string): void {
  const input = monitor.shadowRoot?.querySelector<HTMLInputElement>("#admin-token");
  if (!input) throw new Error("no token field");
  input.focus();
  input.value = token;
  input.form?.requestSubmit();
}

describe("<fs-app-shell> accessibility (UI-S-016)", () => {
  beforeEach(() => defineElements());

  it("passes the accessibility audit, theme control included", async () => {
    const shell = mountShell();
    await settle();
    expect(accessibilityViolations(shell)).toEqual([]);
  });

  it("starts with a skip control worded from the label table", async () => {
    const root = mountShell().shadowRoot;
    const first = root?.querySelector("button, a[href], input");
    expect(first?.classList.contains("skip")).toBe(true);
    expect(first?.textContent).toBe(labels.skipToContent);
    expect(first?.getAttribute("type")).toBe("button");
  });

  it("names the navigation from the label table and has one h1 and a focusable main", () => {
    const root = mountShell().shadowRoot;
    expect(root?.querySelector("nav")?.getAttribute("aria-label")).toBe(labels.navigationMain);
    expect(root?.querySelectorAll("h1")).toHaveLength(1);
    expect(root?.querySelector("header h1")?.textContent).toBe("FileSquid");
    expect(root?.querySelector("main")?.getAttribute("tabindex")).toBe("-1");
    expect(root?.querySelectorAll("[tabindex]:not([tabindex='-1'])")).toHaveLength(0);
  });

  it("the skip control moves focus to main without changing the hash", () => {
    window.location.hash = "#/somewhere";
    const shell = mountShell();
    const skip = shell.shadowRoot?.querySelector<HTMLButtonElement>(".skip");
    skip?.focus();
    skip?.click();
    expect(deepActiveElement(document)).toBe(shell.shadowRoot?.querySelector("main"));
    expect(window.location.hash).toBe("#/somewhere");
    window.location.hash = "";
  });

  it("moves focus to main when a re-render removes the focused element", () => {
    const shell = mountShell();
    shell.shadowRoot?.querySelector<HTMLButtonElement>(".skip")?.focus();
    shell.configure({
      distribution,
      client: { health: async () => ({ status: "ok", product: "x", schema_version: 1 }) },
      theme: fakeThemeStore().store,
    });
    expect(deepActiveElement(document)).toBe(shell.shadowRoot?.querySelector("main"));
  });

  it("does not take focus on first render", () => {
    const outside = document.createElement("button");
    document.body.replaceChildren(outside);
    outside.focus();
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    shell.configure({
      distribution,
      client: { health: async () => ({ status: "ok", product: "x", schema_version: 1 }) },
      theme: fakeThemeStore().store,
    });
    document.body.append(shell);
    expect(document.activeElement).toBe(outside);
  });
});

describe("route focus (UI-S-016)", () => {
  let unbind: () => void = () => undefined;
  beforeEach(() => defineElements());
  afterEach(() => {
    unbind();
    window.location.hash = "";
  });

  it("focuses main after a hashchange but not on the first apply", () => {
    window.location.hash = MONITOR_ROUTE;
    const shell = mountShell();
    const client = monitorClient(() => undefined);
    unbind = bindMonitorRoute(shell, { labels, client, pollMs: 0 });
    const main = shell.shadowRoot?.querySelector("main");
    expect(deepActiveElement(document)).not.toBe(main);
    window.location.hash = "";
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(shell.querySelector("fs-monitor")).toBeNull();
    expect(deepActiveElement(document)).toBe(main);
  });

  it("works with a plain host element that has no focusMain", () => {
    const host = document.createElement("div");
    document.body.replaceChildren(host);
    unbind = bindMonitorRoute(host, { labels, client: monitorClient(() => undefined), pollMs: 0 });
    window.location.hash = MONITOR_ROUTE;
    expect(() => window.dispatchEvent(new HashChangeEvent("hashchange"))).not.toThrow();
    expect(host.querySelector("fs-monitor")).not.toBeNull();
  });
});

describe("<fs-monitor> accessibility (UI-S-016)", () => {
  beforeEach(() => defineElements());

  it("passes the audit when loaded and names each table by its section heading", async () => {
    const monitor = mountMonitor(monitorClient(() => undefined));
    await settle();
    expect(accessibilityViolations(monitor)).toEqual([]);
    const root = monitor.shadowRoot;
    for (const name of ["processes", "journal"]) {
      const section = root?.querySelector(`[data-section="${name}"]`);
      const heading = section?.querySelector("h2");
      const region = section?.querySelector(".scroll");
      expect(heading?.id).not.toBe("");
      expect(region?.getAttribute("role")).toBe("region");
      expect(region?.getAttribute("tabindex")).toBe("0");
      expect(region?.getAttribute("aria-labelledby")).toBe(heading?.id);
      expect(region?.querySelector("table")?.getAttribute("aria-labelledby")).toBe(heading?.id);
    }
  });

  it("passes the audit with the token form", async () => {
    const tokens: TokenStore = { get: () => undefined, set: () => undefined };
    const monitor = mountMonitor(
      monitorClient(() => 401),
      tokens,
    );
    await settle();
    expect(accessibilityViolations(monitor)).toEqual([]);
  });

  it("moves focus to the first section heading after a keyboard sign-in", async () => {
    let status: number | undefined = 401;
    let stored: string | undefined;
    const tokens: TokenStore = { get: () => stored, set: (t) => (stored = t) };
    const monitor = mountMonitor(
      monitorClient(() => status),
      tokens,
    );
    await settle();
    status = undefined;
    submitToken(monitor, "secret");
    await settle();
    const heading = monitor.shadowRoot?.querySelector('[data-section="health"] h2');
    expect(heading?.textContent).toBe(labels.monitorHealth);
    expect(deepActiveElement(document)).toBe(heading);
  });

  it("puts focus back on the new token field when the token is refused", async () => {
    const tokens: TokenStore = { get: () => undefined, set: () => undefined };
    const monitor = mountMonitor(
      monitorClient(() => 401),
      tokens,
    );
    await settle();
    submitToken(monitor, "wrong");
    await settle();
    const field = monitor.shadowRoot?.querySelector("#admin-token");
    expect(field).not.toBeNull();
    expect(deepActiveElement(document)).toBe(field);
  });

  it("moves focus to the alert when a search fails without a token store", async () => {
    let status = 0; // 0: every call succeeds
    const monitor = mountMonitor(monitorClient(() => status));
    await settle();
    const search = monitor.shadowRoot?.querySelector<HTMLButtonElement>(
      'form[data-form="journal"] button[type="submit"]',
    );
    search?.focus();
    status = 500;
    search?.click();
    await settle();
    const alert = monitor.shadowRoot?.querySelector('[role="alert"]');
    expect(alert).not.toBeNull();
    expect(deepActiveElement(document)).toBe(alert);
  });

  it("does not take focus when it loads in the background", async () => {
    const outside = document.createElement("button");
    const monitor = document.createElement("fs-monitor") as FsMonitor;
    monitor.configure({ labels, client: monitorClient(() => undefined), pollMs: 0 });
    document.body.replaceChildren(outside, monitor);
    outside.focus();
    await settle();
    expect(document.activeElement).toBe(outside);
  });
});
