/**
 * UI tests for the Logs route (UI-F2.16; UI-S-021, UI-S-032): `#/logs` mounts `<fs-logs>` once,
 * `#/logs?file=<uuid>` and `#/logs?operation_id=<id>` (the Provenance tab's View in Logs links,
 * UI-F2.15) open it filtered, another hash removes it with its detail and the footer sentences,
 * a selected entry's detail stands in the shell's panel slot and widens it to 360 px, and a link
 * the Logs view makes round-trips through the files route's deep link.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  LOGS_ROUTE,
  bindLogsRoute,
  defineElements,
  logsHash,
  type FsAppShell,
  type FsLogEntry,
  type FsLogs,
} from "../../src/index";
import { parseOpenRequest } from "../../src/files/model";
import labels from "../../src/theme/labels.en.json";
import {
  IMG_OPERATION,
  LIBRARY,
  MemoryLogsSource,
  NEWEST_ID,
  NOW,
  PLAN_FILE,
} from "../support/logs-fakes";

let host: HTMLElement & { focusMain: () => void };
let stop: () => void;
let source: MemoryLogsSource;

beforeEach(() => {
  defineElements();
  window.history.replaceState(null, "", "#/");
  host = Object.assign(document.createElement("div"), { focusMain: vi.fn() });
  document.body.replaceChildren(host);
  source = new MemoryLogsSource();
});
afterEach(() => {
  vi.restoreAllMocks();
  stop?.();
  document.body.replaceChildren();
  window.history.replaceState(null, "", "#/");
});

/** Let queued promise callbacks run. */
const settle = async () => {
  for (let i = 0; i < 8; i += 1) await new Promise<void>((resolve) => setTimeout(resolve, 0));
};

/** Bind the route over the fake source. */
function bind(): void {
  stop = bindLogsRoute(host, { labels, source, zone: "Europe/Berlin", now: () => NOW });
}

/** Go to an address and let the view read. */
async function go(hash: string): Promise<void> {
  window.history.replaceState(null, "", hash);
  window.dispatchEvent(new HashChangeEvent("hashchange"));
  await settle();
}

/** The view and the detail in the host. */
const view = () => host.querySelector<FsLogs>("fs-logs");
const detail = () => host.querySelector<FsLogEntry>("fs-log-entry");
/** A footer slot's text. */
const footer = (name: string) =>
  host.querySelector(`:scope > [slot="${name}"]`)?.textContent ?? undefined;

describe("bindLogsRoute", () => {
  it("mounts the view on #/logs once and removes it on any other hash", async () => {
    bind();
    expect(view()).toBeNull();
    await go(LOGS_ROUTE);
    await go(LOGS_ROUTE);
    expect(host.querySelectorAll("fs-logs")).toHaveLength(1);
    expect(host.focusMain).toHaveBeenCalledTimes(2);
    await go("#/monitor");
    expect(view()).toBeNull();
  });

  it("mounts at once, reading once, when the address is already the Logs view", async () => {
    window.history.replaceState(null, "", logsHash({ file: PLAN_FILE }));
    bind();
    await settle();
    expect(view()).not.toBeNull();
    expect(source.pages).toHaveLength(1);
    expect(source.pages[0]?.file).toBe(PLAN_FILE);
    expect(host.focusMain).not.toHaveBeenCalled();
  });

  it("opens filtered to a file or an operation from the Provenance tab's links", async () => {
    bind();
    await go(logsHash({ operation_id: IMG_OPERATION }));
    expect(source.pages.at(-1)?.operation_id).toBe(IMG_OPERATION);
    await go(logsHash({ file: PLAN_FILE }));
    expect(source.pages.at(-1)?.file).toBe(PLAN_FILE);
    expect(source.pages.at(-1)?.operation_id).toBeUndefined();
    await go(LOGS_ROUTE);
    expect(source.pages.at(-1)?.file).toBeUndefined();
    expect(host.querySelectorAll("fs-logs")).toHaveLength(1);
  });
});

describe("the detail and the footer", () => {
  it("puts the selected entry's detail in the panel slot and removes it when the view closes it", async () => {
    bind();
    await go(LOGS_ROUTE);
    const root = (view() as FsLogs).shadowRoot as ShadowRoot;
    root.querySelector<HTMLButtonElement>(`tr[data-key="${NEWEST_ID - 5}"] button.select`)?.click();
    await settle();
    expect(detail()?.slot).toBe("panel");
    expect(detail()?.entry?.operation).toBe("metadata.set");
    expect(detail()?.hasAttribute("data-panel-wide")).toBe(true);
    (detail()?.shadowRoot?.querySelector("button.close") as HTMLButtonElement).click();
    expect(detail()).toBeNull();
    expect(root.querySelector('tr[aria-selected="true"]')).toBeNull();
  });

  it("gives the detail the libraries, so its links name the library", async () => {
    bind();
    await go(LOGS_ROUTE);
    const root = (view() as FsLogs).shadowRoot as ShadowRoot;
    root.querySelector<HTMLButtonElement>(`tr[data-key="${NEWEST_ID - 5}"] button.select`)?.click();
    await settle();
    const href = detail()?.shadowRoot?.querySelector("a.view-provenance")?.getAttribute("href");
    expect(href?.startsWith(`#/files/${LIBRARY}/`)).toBe(true);
    expect(parseOpenRequest(href ?? "")).toEqual({ file: PLAN_FILE, tab: "provenance" });
  });

  it("puts the time-zone sentence and the entry count in the footer's slots and clears them on leaving", async () => {
    bind();
    await go(LOGS_ROUTE);
    expect(footer("summary")).toBe(
      "Times in your time zone (UTC+2) · stored as UTC · kept forever",
    );
    expect(footer("commit-state")).toBe("16 entries");
    await go("#/settings");
    expect(footer("summary")).toBe("");
    expect(footer("commit-state")).toBe("");
  });

  it("removes the detail when the route leaves the Logs view", async () => {
    bind();
    await go(LOGS_ROUTE);
    const root = (view() as FsLogs).shadowRoot as ShadowRoot;
    root.querySelector<HTMLButtonElement>(`tr[data-key="${NEWEST_ID}"] button.select`)?.click();
    await settle();
    expect(detail()).not.toBeNull();
    await go("#/monitor");
    expect(detail()).toBeNull();
  });
});

describe("the shell's panel region", () => {
  it("is 320 px for a panel and 360 px while the slotted panel asks for the wide region", async () => {
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    shell.configure({
      distribution: {
        id: "filesquid",
        productName: "FileSquid",
        version: "0",
        logo: { src: "/x.svg", alt: "logo" },
        labels,
      },
      client: { host: async () => ({}) as never, subscribe: () => () => undefined } as never,
      theme: {
        resolved: "light",
        addEventListener: () => undefined,
        removeEventListener: () => undefined,
      } as never,
    });
    document.body.replaceChildren(shell);
    await settle();
    const root = shell.shadowRoot as ShadowRoot;
    const aside = root.querySelector("aside.panel") as HTMLElement;
    const content = document.createElement("div");
    content.slot = "panel";
    shell.append(content);
    root.querySelector('slot[name="panel"]')?.dispatchEvent(new Event("slotchange"));
    expect(aside.hasAttribute("data-wide")).toBe(false);
    content.setAttribute("data-panel-wide", "");
    await settle();
    expect(aside.hasAttribute("data-wide")).toBe(true);
    content.removeAttribute("data-panel-wide");
    await settle();
    expect(aside.hasAttribute("data-wide")).toBe(false);
    const css = root.querySelector("style")?.textContent ?? "";
    expect(css).toMatch(/aside\.panel\[data-wide\] \{ inline-size: 22\.5rem/);
  });
});
