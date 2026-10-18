/**
 * UI tests for the `#/monitor` hash route and the in-memory token store: the Monitor is
 * mounted only on its route and removed when leaving it.
 */
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  MONITOR_ROUTE,
  bindMonitorRoute,
  defineElements,
  memoryTokenStore,
  type MonitorClient,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";

const never = () => new Promise<never>(() => undefined);
const client: MonitorClient = {
  adminHealth: never,
  adminProcesses: never,
  adminJournal: never,
  adminMetrics: never,
  adminJournalExport: never,
  subscribe: () => () => undefined,
};

describe("bindMonitorRoute", () => {
  let unbind: () => void = () => undefined;
  beforeEach(() => defineElements());
  afterEach(() => {
    unbind();
    window.location.hash = "";
  });

  it("mounts <fs-monitor> on #/monitor and removes it on other hashes", () => {
    const shell = document.createElement("div");
    document.body.replaceChildren(shell);
    window.location.hash = "";
    unbind = bindMonitorRoute(shell, { labels, client, pollMs: 0 });
    expect(shell.querySelector("fs-monitor")).toBeNull();
    window.location.hash = MONITOR_ROUTE;
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(shell.querySelectorAll("fs-monitor")).toHaveLength(1);
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(shell.querySelectorAll("fs-monitor")).toHaveLength(1);
    window.location.hash = "#/libraries";
    window.dispatchEvent(new HashChangeEvent("hashchange"));
    expect(shell.querySelector("fs-monitor")).toBeNull();
  });
});

describe("memoryTokenStore", () => {
  it("round-trips a token and keeps separate stores apart", () => {
    const store = memoryTokenStore();
    const other = memoryTokenStore();
    store.set("abc");
    expect(store.get()).toBe("abc");
    expect(other.get()).toBeUndefined();
  });
});
