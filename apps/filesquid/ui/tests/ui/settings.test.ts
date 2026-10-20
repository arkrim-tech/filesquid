/**
 * UI tests for `<fs-settings>` and `bindSettingsRoute` (UI-S-004, UI-S-018): the Settings page's
 * Privacy section shows the full host privacy statement in place, from the host descriptor
 * through an inline `<fs-host-indicator>` (no badge, no popover), and follows `host.change`;
 * `#/settings` mounts the page and any other hash removes it.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  accessibilityViolations,
  bindSettingsRoute,
  defineElements,
  type ApiEvent,
  type FsHostIndicator,
  type FsSettings,
  type HostClient,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { DESKTOP_DESCRIPTOR } from "../support/host-fakes";

beforeEach(() => defineElements());
afterEach(() => {
  vi.restoreAllMocks();
  window.location.hash = "";
});

/** Let pending reads and renders finish. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** A client answering the desktop descriptor, with an event channel the test drives. */
function fakeClient() {
  let listener: ((event: ApiEvent) => void) | undefined;
  const client: HostClient = {
    host: async () => DESKTOP_DESCRIPTOR,
    subscribe: (l) => {
      listener = l;
      return () => undefined;
    },
  };
  return { client, emit: (event: ApiEvent) => listener?.(event) };
}

/** Mount a Settings page. */
async function mount(client: HostClient): Promise<FsSettings> {
  const settings = document.createElement("fs-settings") as FsSettings;
  settings.configure({ labels, client });
  document.body.replaceChildren(settings);
  await settle();
  return settings;
}

/** The inline statement inside the Privacy section. */
function statementOf(settings: FsSettings): FsHostIndicator {
  return settings.shadowRoot?.querySelector(
    '[data-section="privacy"] fs-host-indicator',
  ) as FsHostIndicator;
}

describe("<fs-settings>", () => {
  it("is a defined custom element with the Settings heading and a named Privacy section", async () => {
    expect(customElements.get("fs-settings")).toBeDefined();
    const settings = await mount(fakeClient().client);
    const root = settings.shadowRoot as ShadowRoot;
    expect(root.querySelector("h2")?.textContent).toBe(labels.settings);
    const section = root.querySelector('[data-section="privacy"]');
    expect(section?.getAttribute("aria-labelledby")).toBe(root.querySelector("h3")?.id);
    expect(root.querySelector("h3")?.textContent).toBe(labels.settingsPrivacy);
    expect(accessibilityViolations(settings)).toEqual([]);
  });

  it("shows the full statement in place, with no badge, popover or link back to Settings", async () => {
    const statement = statementOf(await mount(fakeClient().client));
    const root = statement.shadowRoot as ShadowRoot;
    expect(statement.dataset.state).toBe("ready");
    expect(root.querySelector<HTMLElement>("button.badge")?.hidden).toBe(true);
    expect(root.querySelector<HTMLElement>(".settings-link")?.hidden).toBe(true);
    const region = root.querySelector<HTMLElement>('[role="region"]');
    expect(region?.hidden).toBe(false);
    expect(root.querySelector('[role="dialog"]')).toBeNull();
    expect([...root.querySelectorAll(".statement")].map((p) => p.textContent)).toEqual([
      labels.hostDesktop,
    ]);
    expect(root.querySelector(".not-enrolled")?.textContent).toBe(labels.hostNotEnrolled);
  });

  it("follows host.change and does not close on Escape", async () => {
    const fake = fakeClient();
    const statement = statementOf(await mount(fake.client));
    const root = statement.shadowRoot as ShadowRoot;
    fake.emit({ topic: "host.change", data: { ...DESKTOP_DESCRIPTOR, host_type: "browser" } });
    expect(root.querySelector(".statement")?.textContent).toBe(labels.hostBrowser);
    const region = root.querySelector<HTMLElement>('[role="region"]');
    region?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(region?.hidden).toBe(false);
  });
});

describe("bindSettingsRoute", () => {
  it("mounts the page at once when bound while the hash is #/settings", () => {
    const shell = document.createElement("div");
    document.body.replaceChildren(shell);
    window.location.hash = "#/settings";
    const stop = bindSettingsRoute(shell, { labels, client: fakeClient().client });
    expect(shell.querySelector("fs-settings")).not.toBeNull();
    stop();
  });

  it("mounts the page on #/settings, moves focus to main, and removes it on another route", async () => {
    const shell = document.createElement("div") as HTMLDivElement & { focusMain: () => void };
    shell.focusMain = vi.fn();
    document.body.replaceChildren(shell);
    const stop = bindSettingsRoute(shell, { labels, client: fakeClient().client });
    expect(shell.querySelector("fs-settings")).toBeNull();
    window.location.hash = "#/settings";
    await settle();
    expect(shell.querySelector("fs-settings")).not.toBeNull();
    expect(shell.focusMain).toHaveBeenCalled();
    window.location.hash = "#/monitor";
    await settle();
    expect(shell.querySelector("fs-settings")).toBeNull();
    stop();
  });
});
