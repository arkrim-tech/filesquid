/**
 * UI tests for `<fs-settings>` and `bindSettingsRoute` (UI-S-033, UI-S-015, UI-S-004,
 * UI-S-018): the Settings page's Appearance section holds the only theme control (owner
 * answer 7), its Privacy section shows the full host privacy statement in place through an
 * inline `<fs-host-indicator>` and follows `host.change`, and its About section is the only
 * place API information appears, as "API: {status} · contract {version}" (owner answer 12);
 * `#/settings` mounts the page and any other hash removes it.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  ApiError,
  accessibilityViolations,
  bindSettingsRoute,
  defineElements,
  type ApiEvent,
  type Distribution,
  type FsHostIndicator,
  type FsSettings,
  type HealthStatus,
  type SettingsClient,
} from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { DESKTOP_DESCRIPTOR } from "../support/host-fakes";
import { fakeThemeStore } from "../support/theme-fakes";

const distribution: Distribution = {
  id: "filesquid",
  productName: "FileSquid",
  version: "0.1.0",
  logo: { src: "/filesquid-mark.svg", srcDark: "/filesquid-mark-dark.svg", alt: "FileSquid logo" },
  labels,
};

beforeEach(() => defineElements());
afterEach(() => {
  vi.restoreAllMocks();
  window.location.hash = "";
});

/** Let pending reads and renders finish. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** A healthy answer. */
const ok = async (): Promise<HealthStatus> => ({
  schema_version: 1,
  status: "ok",
  product: "filesquid",
});

/** A client answering the desktop descriptor and health, with an event channel the test drives. */
function fakeClient(health: () => Promise<HealthStatus> = ok) {
  let listener: ((event: ApiEvent) => void) | undefined;
  const reads = { host: 0 };
  const client: SettingsClient = {
    health,
    host: async () => {
      reads.host += 1;
      return DESKTOP_DESCRIPTOR;
    },
    subscribe: (l) => {
      listener = l;
      return () => undefined;
    },
  };
  return { client, reads, emit: (event: ApiEvent) => listener?.(event) };
}

/** Mount a Settings page. */
async function mount(client: SettingsClient): Promise<FsSettings> {
  const settings = document.createElement("fs-settings") as FsSettings;
  settings.configure({
    labels,
    client,
    theme: fakeThemeStore().store,
    distribution,
    contractVersion: "0.24.0",
  });
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

/** Text of the first element matching `selector`. */
const text = (settings: FsSettings, selector: string) =>
  settings.shadowRoot?.querySelector(selector)?.textContent?.trim();

describe("<fs-settings>", () => {
  it("is a defined custom element with the 30 px title and the Appearance, Privacy and About sections", async () => {
    expect(customElements.get("fs-settings")).toBeDefined();
    const settings = await mount(fakeClient().client);
    const root = settings.shadowRoot as ShadowRoot;
    expect(root.querySelector("h2")?.textContent).toBe(labels.settings);
    expect(root.querySelector("h2")?.classList.contains("page-title")).toBe(true);
    expect([...root.querySelectorAll("section")].map((s) => s.dataset.section)).toEqual([
      "appearance",
      "privacy",
      "about",
    ]);
    for (const section of root.querySelectorAll("section")) {
      const heading = section.querySelector("h3");
      expect(section.getAttribute("aria-labelledby")).toBe(heading?.id);
      expect(heading?.classList.contains("section-title")).toBe(true);
    }
    expect(text(settings, '[data-section="appearance"] h3')).toBe(labels.settingsAppearance);
    expect(text(settings, '[data-section="privacy"] h3')).toBe(labels.settingsPrivacy);
    expect(text(settings, '[data-section="about"] h3')).toBe("About FileSquid");
    expect(accessibilityViolations(settings)).toEqual([]);
  });

  it("holds the theme control in Appearance with its help text (owner answer 7)", async () => {
    const settings = await mount(fakeClient().client);
    const control = settings.shadowRoot?.querySelector(
      '[data-section="appearance"] fs-theme-control',
    );
    expect(control).not.toBeNull();
    expect(control?.shadowRoot?.querySelector("legend")?.textContent).toBe(labels.theme);
    expect(
      [...(control?.shadowRoot?.querySelectorAll("label") ?? [])].map((l) => l.textContent),
    ).toEqual([labels.themeSystem, labels.themeLight, labels.themeDark]);
    expect(text(settings, '[data-section="appearance"] .help')).toBe(labels.themeHelp);
  });

  it("shows the full statement in place, with the no-telemetry line, no badge, popover or link back", async () => {
    const statement = statementOf(await mount(fakeClient().client));
    const root = statement.shadowRoot as ShadowRoot;
    expect(statement.dataset.state).toBe("ready");
    expect(root.querySelector<HTMLElement>("button.badge")?.hidden).toBe(true);
    expect(root.querySelector<HTMLElement>(".settings-link")?.hidden).toBe(true);
    const region = root.querySelector<HTMLElement>('[role="region"]');
    expect(region?.hidden).toBe(false);
    expect(root.querySelector('[role="dialog"]')).toBeNull();
    expect(root.querySelector("h2")?.textContent).toBe(labels.hostBadgeDesktop);
    expect([...root.querySelectorAll(".statement")].map((p) => p.textContent)).toEqual([
      labels.hostDesktop,
    ]);
    expect(root.querySelector(".no-telemetry")?.textContent).toBe(labels.hostDesktopNoTelemetry);
    expect(root.querySelector(".not-enrolled")?.textContent).toBe(labels.hostNotEnrolled);
  });

  it("follows host.change and does not close on Escape", async () => {
    const fake = fakeClient();
    const statement = statementOf(await mount(fake.client));
    const root = statement.shadowRoot as ShadowRoot;
    fake.emit({ topic: "host.change", data: { ...DESKTOP_DESCRIPTOR, host_type: "browser" } });
    expect(root.querySelector(".statement")?.textContent).toBe(labels.hostBrowser);
    expect(root.querySelector(".no-telemetry")).toBeNull();
    const region = root.querySelector<HTMLElement>('[role="region"]');
    region?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    expect(region?.hidden).toBe(false);
  });

  it("shows the product, version, notes and the API status in About (owner answer 12)", async () => {
    const settings = await mount(fakeClient().client);
    const about = settings.shadowRoot?.querySelector('[data-section="about"]');
    expect(about?.querySelector("img")?.getAttribute("src")).toBe("/filesquid-mark.svg");
    expect(about?.querySelector(".version")?.textContent).toBe("FileSquid 0.1.0");
    expect([...(about?.querySelectorAll(".about-note") ?? [])].map((p) => p.textContent)).toEqual([
      labels.settingsBuildNote,
      labels.settingsLicence,
      labels.settingsFormats,
    ]);
    const status = about?.querySelector('.api[role="status"]');
    expect(status?.textContent).toBe("API: ok · contract 0.24.0");
    expect(about?.querySelector('[role="alert"]')).toBeNull();
  });

  it("shows a failed health read as an alert in About, with the denial reason's label", async () => {
    const settings = await mount(
      fakeClient(async () => {
        throw new ApiError(401, "/api/v1/health", {
          code: "unauthorized",
          reason: "missing_credentials",
          retryable: false,
        });
      }).client,
    );
    const alert = settings.shadowRoot?.querySelector('[data-section="about"] .api[role="alert"]');
    expect(alert?.textContent).toBe(`${labels.apiUnavailable}: ${labels.authMissingCredentials}`);
    expect(alert?.getAttribute("data-reason")).toBe("missing_credentials");
  });

  it("reads health and the host again on refresh, so a sign-in replaces the alert", async () => {
    let calls = 0;
    const fake = fakeClient(async () => {
      calls += 1;
      if (calls === 1) throw new Error("offline");
      return ok();
    });
    const settings = await mount(fake.client);
    expect(text(settings, '.api[role="alert"]')).toBe(`${labels.apiUnavailable}: offline`);
    const hostReads = fake.reads.host;
    expect(hostReads).toBeGreaterThanOrEqual(1);
    await settings.refresh();
    expect(fake.reads.host).toBe(hostReads + 1);
    expect(text(settings, '.api[role="status"]')).toBe("API: ok · contract 0.24.0");
    expect(settings.shadowRoot?.querySelector('.api[role="alert"]')).toBeNull();
  });

  it("ignores a stale answer that arrives after a newer one", async () => {
    let rejectFirst: (error: unknown) => void = () => undefined;
    const first = new Promise<HealthStatus>((_, reject) => (rejectFirst = reject));
    let calls = 0;
    const settings = await mount(
      fakeClient(() => {
        calls += 1;
        return calls === 1 ? first : ok();
      }).client,
    );
    await settings.refresh();
    const debug = vi.spyOn(console, "debug").mockImplementation(() => undefined);
    rejectFirst(new Error("late"));
    await settle();
    expect(text(settings, '.api[role="status"]')).toBe("API: ok · contract 0.24.0");
    expect(debug).toHaveBeenCalledOnce();
  });
});

describe("bindSettingsRoute", () => {
  const options = () => ({
    labels,
    client: fakeClient().client,
    theme: fakeThemeStore().store,
    distribution,
    contractVersion: "0.24.0",
  });

  it("mounts the page at once when bound while the hash is #/settings", () => {
    const shell = document.createElement("div");
    document.body.replaceChildren(shell);
    window.location.hash = "#/settings";
    const stop = bindSettingsRoute(shell, options());
    expect(shell.querySelector("fs-settings")).not.toBeNull();
    stop();
  });

  it("mounts the page on #/settings, moves focus to main, and removes it on another route", async () => {
    const shell = document.createElement("div") as HTMLDivElement & { focusMain: () => void };
    shell.focusMain = vi.fn();
    document.body.replaceChildren(shell);
    const stop = bindSettingsRoute(shell, options());
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
