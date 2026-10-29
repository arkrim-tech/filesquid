/**
 * UI tests for `<fs-details-panel>`, the File info tab (UI-F2.3, UI-S-003, UI-S-031 35 and 25):
 * the storage facts as property rows with copy, the filter with its no-match state, the
 * loading and error states with Try again, a stale answer ignored, and no stored name.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { accessibilityViolations, defineElements, type FsDetailsPanel } from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import { FakePanelSource, PANEL_FILE, gate } from "../support/panel-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

/** Let queued promise callbacks run. */
const settle = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

/** Mount a details panel over `source` showing the fixture file. */
async function mount(source = new FakePanelSource(), online = true): Promise<FsDetailsPanel> {
  const panel = document.createElement("fs-details-panel") as FsDetailsPanel;
  panel.configure({ labels, source, online: () => online });
  document.body.replaceChildren(panel);
  panel.fileRef = PANEL_FILE;
  await settle();
  return panel;
}

/** The shadow root of a panel. */
function root(panel: HTMLElement): ShadowRoot {
  return panel.shadowRoot as ShadowRoot;
}

/** Type into the filter field. */
function filter(panel: HTMLElement, word: string): void {
  const input = root(panel).querySelector<HTMLInputElement>("input.filter") as HTMLInputElement;
  input.value = word;
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

describe("<fs-details-panel>", () => {
  it("is a defined element with the shared stylesheet first and its tab label", async () => {
    expect(customElements.get("fs-details-panel")).toBeDefined();
    const panel = await mount();
    expect(root(panel).firstElementChild?.tagName).toBe("STYLE");
    expect(root(panel).querySelector("style")?.textContent).toContain(":focus-visible");
    expect(panel.tabLabel).toBe(labels.filePanelInfo);
  });

  it("shows the storage facts as rows of a label over a value, with the count and footer", async () => {
    const panel = await mount();
    expect(panel.dataset.state).toBe("ready");
    const rows = [...root(panel).querySelectorAll(".row")];
    const labelsShown = rows.map((r) => r.querySelector("dt")?.textContent);
    expect(labelsShown).toEqual([
      labels.filePanelOriginalName,
      labels.filePanelExtension,
      labels.filePanelFormat,
      labels.filePanelSize,
      "SHA-256 digest",
      "SHA-512 digest",
      labels.filePanelObject,
      labels.filePanelVersion,
    ]);
    expect(rows[0]?.querySelector(".value")?.textContent).toBe("harbour-masterplan-v7.pdf");
    expect(rows[1]?.querySelector(".value")?.classList.contains("mono")).toBe(true);
    expect(root(panel).querySelector(".count")?.textContent).toBe("8 properties · read only");
    expect(root(panel).querySelector(".footer")?.textContent).toBe(labels.filePanelInfoFooter);
    expect(root(panel).querySelector("input.filter")?.getAttribute("placeholder")).toBe(
      labels.filePanelFilterInfo,
    );
    expect(accessibilityViolations(panel)).toEqual([]);
  });

  it("never puts the stored name or the logical path in the DOM", async () => {
    const panel = await mount();
    expect(root(panel).textContent).not.toContain("9f2c41aa");
    expect(root(panel).innerHTML).not.toContain("original/");
  });

  it("copies the whole value and announces it", async () => {
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const panel = await mount();
    const digest = [...root(panel).querySelectorAll(".row")].find(
      (r) => r.querySelector("dt")?.textContent === "SHA-256 digest",
    );
    const copy = digest?.querySelector<HTMLButtonElement>("button.copy");
    expect(copy?.getAttribute("aria-label")).toBe("Copy SHA-256 digest");
    copy?.click();
    await settle();
    expect(writeText).toHaveBeenCalledWith(`9b1f4e${"0".repeat(52)}c07a2d`);
    expect(root(panel).querySelector(".announce")?.textContent).toBe("SHA-256 digest copied");
  });

  it("logs a refused copy and announces nothing", async () => {
    const writeText = vi.fn(async () => Promise.reject(new Error("denied")));
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const panel = await mount();
    root(panel).querySelector<HTMLButtonElement>("button.copy")?.click();
    await settle();
    expect(warn).toHaveBeenCalled();
    expect(root(panel).querySelector(".announce")?.textContent).toBe("");
  });

  it("filters rows by label or value and shows no match with Clear filter", async () => {
    const panel = await mount();
    filter(panel, "application/pdf");
    expect(root(panel).querySelectorAll(".row")).toHaveLength(1);
    expect(root(panel).querySelector(".count")?.textContent).toBe("1 of 8 properties · read only");
    filter(panel, "ocfl");
    expect(root(panel).querySelectorAll(".row")).toHaveLength(2);
    expect(root(panel).querySelector(".count")?.textContent).toBe("2 of 8 properties · read only");
    filter(panel, "copyright");
    expect(panel.dataset.state).toBe("no-match");
    expect(root(panel).querySelector(".state .title")?.textContent).toBe(
      "No file info matches “copyright”",
    );
    expect(root(panel).querySelector(".state .detail")?.textContent).toBe(
      labels.filePanelNoMatchHint,
    );
    root(panel).querySelector<HTMLButtonElement>(".state button.clear")?.click();
    expect(root(panel).querySelector<HTMLInputElement>("input.filter")?.value).toBe("");
    expect(root(panel).querySelectorAll(".row")).toHaveLength(8);
    expect(panel.dataset.state).toBe("ready");
  });

  it("shows placeholder rows and the busy line while loading, with the filter disabled", async () => {
    const source = new FakePanelSource();
    const wait = gate();
    source.gates.details = wait;
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("loading");
    expect(root(panel).querySelector(".count")?.textContent).toBe(labels.filePanelInfoLoading);
    expect(root(panel).querySelectorAll(".placeholder").length).toBeGreaterThanOrEqual(8);
    expect(root(panel).querySelector<HTMLInputElement>("input.filter")?.disabled).toBe(true);
    wait.open();
    await settle();
    expect(panel.dataset.state).toBe("ready");
  });

  it("shows the error with its reason, Nothing was changed, the code and Try again", async () => {
    const source = new FakePanelSource();
    source.failures.details = new ApiError(503, "/api/v1/files/details", { code: "unavailable" });
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("error");
    const state = root(panel).querySelector(".state") as HTMLElement;
    expect(state.getAttribute("role")).toBe("alert");
    expect(state.querySelector(".title")?.textContent).toBe(labels.filePanelInfoError);
    expect([...state.querySelectorAll(".detail")].map((p) => p.textContent)).toEqual([
      labels.stateUnavailable,
      labels.filePanelNothingChanged,
    ]);
    expect(state.querySelector(".code")?.textContent).toBe("unavailable");
    delete source.failures.details;
    state.querySelector<HTMLButtonElement>("button.retry")?.click();
    await settle();
    expect(panel.dataset.state).toBe("ready");
  });

  it("words a denial with its reason and an offline failure as offline", async () => {
    const source = new FakePanelSource();
    source.failures.versions = new ApiError(403, "/api/v1/files/versions", {
      code: "forbidden",
      reason: "not_permitted",
    });
    const panel = await mount(source);
    expect(root(panel).querySelector(".state .detail")?.textContent).toBe(labels.authNotPermitted);
    const offline = new FakePanelSource();
    offline.failures.details = new TypeError("Failed to fetch");
    const second = await mount(offline, false);
    expect(root(second).querySelector(".state .detail")?.textContent).toBe(labels.stateOffline);
    expect(root(second).querySelector(".state .code")).toBeNull();
  });

  it("ignores the answer for a file that is no longer shown", async () => {
    const source = new FakePanelSource();
    source.ignoreAbort = true;
    const wait = gate();
    source.gates.details = wait;
    const panel = await mount(source);
    panel.fileRef = null;
    wait.open();
    await settle();
    expect(panel.dataset.state).toBe("idle");
    expect(root(panel).querySelectorAll(".row")).toHaveLength(0);
    expect(source.calls.details).toBe(1);
  });

  it("hides Clear filter and forgets the filter while loading, failed or cleared", async () => {
    const source = new FakePanelSource();
    const panel = await mount(source);
    filter(panel, "ocfl");
    const clear = root(panel).querySelector<HTMLButtonElement>(
      ".clear-filter",
    ) as HTMLButtonElement;
    expect(clear.hidden).toBe(false);
    source.failures.details = new ApiError(503, "/api/v1/files/details", { code: "unavailable" });
    panel.fileRef = { ...PANEL_FILE, version: "v6" };
    await settle();
    expect(panel.dataset.state).toBe("error");
    expect(clear.hidden).toBe(true);
    expect(root(panel).querySelector<HTMLInputElement>("input.filter")?.value).toBe("");
    clear.click();
    expect(panel.dataset.state).toBe("error");
  });
});
