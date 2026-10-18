/**
 * UI tests for <fs-app-shell>: strict DOM assertions on what the shell renders
 * for a distribution, with a fake API client.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FsAppShell, defineElements } from "../../src/index";
import type { Distribution } from "../../src/distribution";
import { ApiError, type HealthStatus } from "../../src/api/client";
import labels from "../../src/theme/labels.en.json";

const distribution: Distribution = {
  id: "filesquid",
  productName: "FileSquid",
  logo: { src: "/filesquid-logo.png", alt: "FileSquid logo" },
  labels,
};

/** Wait for the shell's asynchronous health request to settle. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Mount a shell with a client whose health() resolves or rejects. */
function mount(health: () => Promise<HealthStatus>) {
  const shell = document.createElement("fs-app-shell") as FsAppShell;
  shell.configure({ distribution, client: { health } });
  document.body.replaceChildren(shell);
  return shell;
}

describe("<fs-app-shell>", () => {
  beforeEach(() => defineElements());
  // Restore spies even when an assertion fails mid-test.
  afterEach(() => vi.restoreAllMocks());

  it("is registered as a custom element", () => {
    expect(customElements.get("fs-app-shell")).toBe(FsAppShell);
  });

  it("renders the distribution's product name, logo and labels in its shadow DOM", async () => {
    const shell = mount(async () => ({ status: "ok", product: "filesquid", schema_version: 1 }));
    await settle();
    const root = shell.shadowRoot;
    expect(root).not.toBeNull();
    expect(root?.querySelector("h1")?.textContent).toBe("FileSquid");
    const logo = root?.querySelector("img");
    expect(logo?.getAttribute("src")).toBe("/filesquid-logo.png");
    expect(logo?.getAttribute("alt")).toBe("FileSquid logo");
    const nav = [...(root?.querySelectorAll("nav li") ?? [])].map((li) => li.textContent);
    expect(nav).toEqual([
      "Libraries",
      "Collections",
      "Virtual collections",
      "Virtual exhibits",
      "Monitor",
    ]);
  });

  it("shows the API health status once the client answers", async () => {
    const shell = mount(async () => ({ status: "ok", product: "filesquid", schema_version: 1 }));
    await settle();
    const status = shell.shadowRoot?.querySelector('[role="status"]');
    expect(status?.textContent).toBe("API: ok (filesquid)");
    expect(shell.shadowRoot?.querySelector('[role="alert"]')).toBeNull();
  });

  it("shows an alert when the API is unreachable", async () => {
    const shell = mount(async () => {
      throw new Error("offline");
    });
    await settle();
    const alert = shell.shadowRoot?.querySelector('[role="alert"]');
    expect(alert?.textContent).toBe("API unavailable: offline");
  });

  it.each([
    [401, "unauthorized", "missing_credentials", "API unavailable: Missing credentials"],
    [403, "forbidden", "invalid_credentials", "API unavailable: Invalid credentials"],
    [403, "forbidden", "not_permitted", "API unavailable: Not permitted"],
  ] as const)("shows HTTP %i %s (%s) with its own label", async (status, code, reason, text) => {
    const shell = mount(async () => {
      throw new ApiError(status, "/api/v1/health", { code, reason, retryable: false });
    });
    await settle();
    const alert = shell.shadowRoot?.querySelector('[role="alert"]');
    expect(alert?.textContent).toBe(text);
    expect(alert?.getAttribute("data-reason")).toBe(reason);
  });

  it("reloads the API status on refreshHealth (after a token is entered)", async () => {
    let calls = 0;
    const shell = mount(async () => {
      calls += 1;
      if (calls === 1) throw new ApiError(401, "/api/v1/health", { reason: "missing_credentials" });
      return { status: "ok", product: "filesquid", schema_version: 1 };
    });
    await settle();
    expect(shell.shadowRoot?.querySelector('[role="alert"]')).not.toBeNull();
    await shell.refreshHealth();
    expect(shell.shadowRoot?.querySelector('[role="status"]')?.textContent).toBe(
      "API: ok (filesquid)",
    );
    expect(shell.shadowRoot?.querySelector('[role="alert"]')).toBeNull();
  });

  it("ignores a stale pre-sign-in answer that arrives after the signed-in one", async () => {
    let rejectFirst: (error: unknown) => void = () => undefined;
    const first = new Promise<HealthStatus>((_, reject) => (rejectFirst = reject));
    let calls = 0;
    const shell = mount(() => {
      calls += 1;
      if (calls === 1) return first;
      return Promise.resolve({ status: "ok", product: "filesquid", schema_version: 1 });
    });
    await shell.refreshHealth();
    const debug = vi.spyOn(console, "debug").mockImplementation(() => undefined);
    rejectFirst(new ApiError(401, "/api/v1/health", { reason: "missing_credentials" }));
    await settle();
    expect(calls).toBe(2);
    expect(shell.shadowRoot?.querySelector('[role="status"]')?.textContent).toBe(
      "API: ok (filesquid)",
    );
    expect(shell.shadowRoot?.querySelector('[role="alert"]')).toBeNull();
    expect(debug).toHaveBeenCalledOnce();
  });
});
