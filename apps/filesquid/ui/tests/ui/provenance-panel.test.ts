/**
 * UI tests for `<fs-provenance-panel>`, the Provenance tab (UI-F2.15; UI-S-020, UI-S-022,
 * UI-S-031 mockups 36 and 25): the activities newest first with their `prov:` attributes, the
 * latest expanded, a record read when another is expanded, the reconstructed mark and the
 * unrecorded agent, the in-tab filter with its no-match state, copy, the loading, empty and
 * error states, View in Logs for the owner only, the JSON-LD export, and a tab that mounts
 * alone with nothing but its own source.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { accessibilityViolations, defineElements, type FsProvenancePanel } from "../../src/index";
import labels from "../../src/theme/labels.en.json";
import {
  FakeProvenanceSource,
  PROV_FILE,
  provEntries,
  provGate,
} from "../support/provenance-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

/** Let queued promise callbacks run. */
const settle = async () => {
  for (let i = 0; i < 6; i += 1) await new Promise<void>((resolve) => setTimeout(resolve, 0));
};

/** Mount a panel over `source` showing the fixture file. */
async function mount(
  source = new FakeProvenanceSource(),
  extra: {
    canViewLogs?: boolean;
    save?: (blob: Blob, name: string) => void;
    online?: boolean;
  } = {},
): Promise<FsProvenancePanel> {
  const panel = document.createElement("fs-provenance-panel") as FsProvenancePanel;
  panel.configure({
    labels,
    source,
    canViewLogs: () => extra.canViewLogs ?? false,
    save: extra.save ?? (() => undefined),
    online: () => extra.online ?? true,
  });
  document.body.replaceChildren(panel);
  panel.fileRef = PROV_FILE;
  await settle();
  return panel;
}

const root = (panel: HTMLElement) => panel.shadowRoot as ShadowRoot;
const activities = (panel: HTMLElement) => [
  ...root(panel).querySelectorAll<HTMLElement>("li.activity"),
];

/** The text of a blob (jsdom's Blob has no `text()`). */
function readBlob(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsText(blob);
  });
}

/** Type into the filter field. */
function filter(panel: HTMLElement, word: string): void {
  const input = root(panel).querySelector<HTMLInputElement>("input.filter") as HTMLInputElement;
  input.value = word;
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

describe("<fs-provenance-panel>", () => {
  it("is a defined element with the shared stylesheet first and its tab label", async () => {
    expect(customElements.get("fs-provenance-panel")).toBeDefined();
    const panel = await mount();
    expect(root(panel).firstElementChild?.tagName).toBe("STYLE");
    expect(root(panel).querySelector("style")?.textContent).toContain(":focus-visible");
    expect(panel.tabLabel).toBe(labels.filePanelProvenance);
  });

  it("mounts alone: only its own source is read, once for the history and once for the head", async () => {
    const source = new FakeProvenanceSource();
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("ready");
    expect(source.historyCalls).toEqual([undefined]);
    expect(source.recordCalls).toEqual(["v4"]);
  });

  it("lists the activities newest first with the kind, prov:Activity and the version chip", async () => {
    const panel = await mount();
    const rows = activities(panel);
    expect(rows.map((r) => r.querySelector(".kind")?.textContent)).toEqual([
      "metadata edit",
      "restore",
      "remove",
      "import",
    ]);
    expect(rows.map((r) => r.querySelector(".chip.version")?.textContent)).toEqual([
      "v4",
      "v3",
      "v2",
      "v1",
    ]);
    expect(rows[0]?.querySelector(".type")?.textContent).toBe("prov:Activity");
    expect(root(panel).querySelector(".count")?.textContent).toBe(labels.provenanceCount);
    expect(root(panel).querySelector("input.filter")?.getAttribute("placeholder")).toBe(
      labels.provenanceFilter,
    );
  });

  it("expands the latest activity with its prov attributes and values", async () => {
    const panel = await mount();
    const [latest, older] = activities(panel);
    expect(latest?.querySelector("button.toggle")?.getAttribute("aria-expanded")).toBe("true");
    expect(older?.querySelector("button.toggle")?.getAttribute("aria-expanded")).toBe("false");
    const names = [...(latest?.querySelectorAll(".attr .name") ?? [])].map((n) => n.textContent);
    expect(names).toEqual([
      "prov:startedAtTime",
      "prov:endedAtTime",
      "prov:wasAssociatedWith",
      "prov:used",
      "prov:wasGeneratedBy (entity v4)",
      "prov:wasDerivedFrom",
    ]);
    const text = latest?.textContent ?? "";
    for (const part of [
      "2026-10-16T11:05:02Z",
      "urn:uuid:5a0c41e2-…-55e19b",
      "owner agent · this installation",
      "FileSquid 0.1.0",
      "prov:SoftwareAgent",
      "urn:uuid:7f3c9a1e-…-41d2b8@v3",
      "Harbour project/facade-study.jpg",
      "sha256 9b1f4e…c07a2d",
      "…41d2b8@v3",
      "Oct 16, 2026, 11:05 UTC",
    ]) {
      expect(text).toContain(part);
    }
  });

  it("reads the record of an activity when it is expanded, once, and collapses it again", async () => {
    const source = new FakeProvenanceSource();
    const panel = await mount(source);
    const toggle = activities(panel)[1]?.querySelector<HTMLButtonElement>(
      "button.toggle",
    ) as HTMLButtonElement;
    toggle.click();
    await settle();
    expect(source.recordCalls).toEqual(["v4", "v3"]);
    expect(
      activities(panel)[1]?.querySelector("button.toggle")?.getAttribute("aria-expanded"),
    ).toBe("true");
    expect(activities(panel)[1]?.textContent).toContain("prov:wasDerivedFrom");
    (activities(panel)[1]?.querySelector("button.toggle") as HTMLButtonElement).click();
    await settle();
    expect(
      activities(panel)[1]?.querySelector("button.toggle")?.getAttribute("aria-expanded"),
    ).toBe("false");
    (activities(panel)[1]?.querySelector("button.toggle") as HTMLButtonElement).click();
    await settle();
    expect(source.recordCalls).toEqual(["v4", "v3"]);
  });

  it("keeps focus on the toggle that was pressed after the activity re-renders", async () => {
    const panel = await mount();
    const toggle = activities(panel)[2]?.querySelector<HTMLButtonElement>(
      "button.toggle",
    ) as HTMLButtonElement;
    toggle.focus();
    toggle.click();
    await settle();
    expect(root(panel).activeElement?.closest("li.activity")?.getAttribute("data-key")).toBe(
      "v2:op-2",
    );
  });

  it("marks a reconstructed activity and shows its unrecorded agent as unrecorded", async () => {
    const source = new FakeProvenanceSource();
    source.entries = provEntries().map((e, i) =>
      i === 3
        ? {
            ...e,
            reconstructed: true,
            started_at: null,
            agents: ["urn:filesquid:agent:unrecorded:installation"],
          }
        : e,
    );
    const panel = await mount(source);
    const latest = activities(panel)[0] as HTMLElement;
    expect(latest.querySelector(".chip.reconstructed")?.textContent).toBe(
      labels.provenanceReconstructed,
    );
    expect(latest.textContent).toContain(labels.provenanceNotRecorded);
    expect(latest.textContent).toContain(labels.provenanceAgentUnrecordedInstallation);
    expect(activities(panel)[1]?.querySelector(".chip.reconstructed")).toBeNull();
  });

  it("filters by any word of an activity and shows the no-match state with Clear filter", async () => {
    const panel = await mount();
    filter(panel, "restore");
    expect(activities(panel)).toHaveLength(1);
    expect(root(panel).querySelector(".count")?.textContent).toBe("1 of 4 activities");
    filter(panel, "copyright");
    expect(panel.dataset.state).toBe("no-match");
    expect(root(panel).querySelector(".state .title")?.textContent).toBe(
      "No activities match “copyright”",
    );
    expect(root(panel).querySelector(".count")?.textContent).toBe("0 of 4 activities");
    root(panel).querySelector<HTMLButtonElement>(".state .button.clear")?.click();
    expect(activities(panel)).toHaveLength(4);
    expect(panel.dataset.state).toBe("ready");
  });

  it("filters by an operation identifier and by an entity path", async () => {
    const panel = await mount();
    filter(panel, "op-2");
    expect(activities(panel).map((r) => r.getAttribute("data-key"))).toEqual(["v2:op-2"]);
    filter(panel, "");
    expect(activities(panel)).toHaveLength(4);
    // The latest activity's record is read, so its entity path matches; the others are not read.
    filter(panel, "facade-study.jpg");
    expect(activities(panel).map((r) => r.getAttribute("data-key"))).toEqual(["v4:op-4"]);
    filter(panel, "sha256 9b1f4e");
    expect(activities(panel)).toHaveLength(0);
    filter(panel, "9b1f4e");
    expect(activities(panel).map((r) => r.getAttribute("data-key"))).toEqual(["v4:op-4"]);
  });

  it("copies the whole value of an attribute and announces it", async () => {
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
    const panel = await mount();
    const copy = [...root(panel).querySelectorAll<HTMLButtonElement>("button.copy")].find((b) =>
      b.getAttribute("aria-label")?.includes("prov:wasAssociatedWith"),
    );
    copy?.click();
    await settle();
    expect(writeText).toHaveBeenCalledWith("urn:uuid:5a0c41e2-1111-4222-8333-44445555e19b");
    expect(root(panel).querySelector("[role=status]")?.textContent).toContain(
      "prov:wasAssociatedWith copied",
    );
  });

  it("shows loading placeholder rows with the busy line, then the activities", async () => {
    const source = new FakeProvenanceSource();
    source.historyGate = provGate();
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("loading");
    expect(root(panel).querySelector(".count")?.textContent).toBe(labels.provenanceLoading);
    expect(root(panel).querySelectorAll(".placeholder").length).toBeGreaterThan(0);
    expect(root(panel).querySelector("input.filter")?.hasAttribute("disabled")).toBe(true);
    source.historyGate.open();
    await settle();
    expect(panel.dataset.state).toBe("ready");
  });

  it("shows the empty state for a file with no recorded activity", async () => {
    const source = new FakeProvenanceSource();
    source.entries = [];
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("empty");
    expect(root(panel).querySelector(".state .title")?.textContent).toBe(labels.provenanceEmpty);
    expect(root(panel).querySelector(".state .detail")?.textContent).toBe(
      labels.provenanceEmptyHint,
    );
  });

  it("shows a distinct error state with the code and Try again, and retries", async () => {
    const source = new FakeProvenanceSource();
    source.historyError = new ApiError(503, "/x", { code: "unavailable", retryable: true });
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("error");
    const state = root(panel).querySelector(".state") as HTMLElement;
    expect(state.getAttribute("role")).toBe("alert");
    expect(state.querySelector(".title")?.textContent).toBe(labels.provenanceError);
    expect(state.querySelector(".code")?.textContent).toBe("unavailable");
    expect(state.textContent).toContain(labels.filePanelNothingChanged);
    source.historyError = undefined;
    state.querySelector<HTMLButtonElement>("button.retry")?.click();
    await settle();
    expect(panel.dataset.state).toBe("ready");
  });

  it("shows the owner a View in Logs link for the file and for each expanded activity", async () => {
    const panel = await mount(new FakeProvenanceSource(), { canViewLogs: true });
    const fileLink = root(panel).querySelector<HTMLAnchorElement>(
      ".footer a.logs",
    ) as HTMLAnchorElement;
    expect(fileLink.textContent).toBe(labels.provenanceViewInLogs);
    expect(fileLink.getAttribute("href")).toBe("#/logs?file=7f3c9a1e-0b2c-4d5e-8f90-a1b2c341d2b8");
    const own = activities(panel)[0]?.querySelector<HTMLAnchorElement>("a.logs");
    expect(own?.getAttribute("href")).toBe("#/logs?operation_id=op-4");
    expect(own?.getAttribute("aria-label")).toBe("View metadata edit v4 in Logs");
  });

  it("links every activity to the Logs view for its own operation once it is expanded", async () => {
    const panel = await mount(new FakeProvenanceSource(), { canViewLogs: true });
    for (const index of [1, 2, 3]) {
      (activities(panel)[index]?.querySelector("button.toggle") as HTMLButtonElement).click();
      await settle();
    }
    const links = activities(panel).map((row) => ({
      key: row.dataset.key,
      href: row.querySelector("a.logs")?.getAttribute("href"),
    }));
    expect(links).toEqual([
      { key: "v4:op-4", href: "#/logs?operation_id=op-4" },
      { key: "v3:op-3", href: "#/logs?operation_id=op-3" },
      { key: "v2:op-2", href: "#/logs?operation_id=op-2" },
      { key: "v1:op-1", href: "#/logs?operation_id=op-1" },
    ]);
  });

  it("draws or removes View in Logs when the host becomes known or changes", async () => {
    let owner = false;
    const source = new FakeProvenanceSource();
    const panel = document.createElement("fs-provenance-panel") as FsProvenancePanel;
    panel.configure({ labels, source, canViewLogs: () => owner });
    document.body.replaceChildren(panel);
    panel.fileRef = PROV_FILE;
    await settle();
    expect(root(panel).querySelector("a.logs")).toBeNull();
    owner = true;
    document.dispatchEvent(new CustomEvent("fs-host", { detail: { hostType: "desktop" } }));
    expect(root(panel).querySelector(".footer a.logs")).not.toBeNull();
    owner = false;
    document.dispatchEvent(new CustomEvent("fs-host", { detail: { hostType: "server" } }));
    expect(root(panel).querySelector("a.logs")).toBeNull();
  });

  it("offers no View in Logs to anyone who is not the owner or an administrator", async () => {
    const panel = await mount(new FakeProvenanceSource(), { canViewLogs: false });
    expect(root(panel).querySelector("a.logs")).toBeNull();
  });

  it("has no per-file audit log control of any kind", async () => {
    const panel = await mount(new FakeProvenanceSource(), { canViewLogs: true });
    const text = (root(panel).textContent ?? "").toLowerCase();
    expect(text).not.toContain("audit");
    expect(root(panel).querySelector('[role="tab"]')).toBeNull();
  });

  it("exports the file's history as JSON-LD, saves it under a name and announces it", async () => {
    const saved: { blob: Blob; name: string }[] = [];
    const source = new FakeProvenanceSource();
    const panel = await mount(source, { save: (blob, name) => saved.push({ blob, name }) });
    const button = root(panel).querySelector<HTMLButtonElement>(
      ".footer button.export",
    ) as HTMLButtonElement;
    expect(button.textContent).toBe(labels.provenanceExport);
    expect(button.getAttribute("aria-label")).toBe(labels.provenanceExportName);
    button.click();
    await settle();
    expect(source.exports).toBe(1);
    expect(saved).toHaveLength(1);
    expect(saved[0]?.name).toBe("facade-study.jpg.history.jsonld");
    const blob = saved[0]?.blob as Blob;
    expect(blob.type).toBe("application/ld+json");
    expect(await readBlob(blob)).toBe('[{"@id":"x"}]');
    expect(root(panel).querySelector(".footer [role=status]")?.textContent).toBe(
      "Exported the history of facade-study.jpg.",
    );
  });

  it("does not carry an export's announcement or failure over to the next file", async () => {
    const source = new FakeProvenanceSource();
    const panel = await mount(source);
    const exportButton = () =>
      root(panel).querySelector<HTMLButtonElement>(".footer button.export") as HTMLButtonElement;
    exportButton().click();
    await settle();
    expect(root(panel).querySelector(".footer [role=status]")?.textContent).not.toBe("");
    panel.fileRef = { ...PROV_FILE, object_id: "urn:uuid:11111111-2222-4333-8444-555555555555" };
    await settle();
    expect(root(panel).querySelector(".footer [role=status]")?.textContent).toBe("");
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    source.exportError = new ApiError(500, "/x", { code: "internal" });
    exportButton().click();
    await settle();
    expect((root(panel).querySelector(".footer [role=alert]") as HTMLElement).hidden).toBe(false);
    panel.fileRef = PROV_FILE;
    await settle();
    expect((root(panel).querySelector(".footer [role=alert]") as HTMLElement).hidden).toBe(true);
  });

  it("ignores an export that finishes after another file opened", async () => {
    const saved: string[] = [];
    const source = new FakeProvenanceSource();
    const panel = await mount(source, { save: (_blob, name) => saved.push(name) });
    const slow = provGate();
    source.exportGate = slow;
    (root(panel).querySelector(".footer button.export") as HTMLButtonElement).click();
    panel.fileRef = { ...PROV_FILE, object_id: "urn:uuid:11111111-2222-4333-8444-555555555555" };
    await settle();
    slow.open();
    await settle();
    expect(saved).toEqual([]);
    expect(root(panel).querySelector(".footer [role=status]")?.textContent).toBe("");
  });

  it("says when the export left records out, and when it failed, and changes nothing", async () => {
    const source = new FakeProvenanceSource();
    source.exportTruncated = true;
    const panel = await mount(source);
    const button = () =>
      root(panel).querySelector<HTMLButtonElement>(".footer button.export") as HTMLButtonElement;
    button().click();
    await settle();
    expect(root(panel).querySelector(".footer [role=status]")?.textContent).toContain(
      labels.provenanceExportPartial,
    );
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    source.exportError = new ApiError(500, "/x", { code: "internal" });
    button().click();
    await settle();
    expect(root(panel).querySelector(".footer [role=alert]")?.textContent).toBe(
      labels.provenanceExportFailed,
    );
  });

  it("shows a record that cannot be read inside its activity, with Try again, and keeps the list", async () => {
    const source = new FakeProvenanceSource();
    source.failingRecords.add("v4");
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("ready");
    const latest = activities(panel)[0] as HTMLElement;
    expect(latest.querySelector("[role=alert]")?.textContent).toContain(
      "Could not read the record of v4.",
    );
    source.failingRecords.clear();
    latest.querySelector<HTMLButtonElement>("button.retry-record")?.click();
    await settle();
    expect(activities(panel)[0]?.textContent).toContain("prov:used");
  });

  it("says when the list is cut at its bound or the index left activities out", async () => {
    const source = new FakeProvenanceSource();
    source.truncated = true;
    const panel = await mount(source);
    expect(root(panel).querySelector(".after-rows")?.textContent).toContain(
      labels.provenanceIndexTruncated,
    );
  });

  it("follows the history pages to the end", async () => {
    const source = new FakeProvenanceSource();
    source.pageSize = 2;
    const panel = await mount(source);
    expect(source.historyCalls).toEqual([undefined, "page-2"]);
    expect(activities(panel)).toHaveLength(4);
  });

  it("ignores a stale answer after another file opens, and clears for null", async () => {
    const other = "urn:uuid:11111111-2222-4333-8444-555555555555";
    const source = new FakeProvenanceSource();
    source.entriesByObject.set(other, provEntries().slice(0, 1));
    const first = provGate();
    source.historyGateOnce = first;
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("loading");
    panel.fileRef = { ...PROV_FILE, object_id: other };
    await settle();
    // The second file's one activity is shown while the first file's answer is still out.
    expect(activities(panel).map((r) => r.dataset.key)).toEqual(["v1:op-1"]);
    first.open();
    await settle();
    expect(activities(panel).map((r) => r.dataset.key)).toEqual(["v1:op-1"]);
    expect(source.historyCalls).toHaveLength(2);
    panel.fileRef = null;
    await settle();
    expect(panel.dataset.state).toBe("idle");
    expect(activities(panel)).toHaveLength(0);
  });

  it("shows a file with no file history as having no activities, not as a failure", async () => {
    const source = new FakeProvenanceSource();
    source.historyError = new TypeError("The reference has no file history");
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("empty");
  });

  it("limits the list to the newest 500 activities and says so", async () => {
    const source = new FakeProvenanceSource();
    const base = provEntries()[0] as ReturnType<typeof provEntries>[number];
    source.entries = Array.from({ length: 520 }, (_, i) => ({
      ...base,
      operation_id: `op-${i + 1}`,
      version: { ...base.version, version: `v${i + 1}` },
    }));
    source.pageSize = 200;
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const panel = await mount(source);
    expect(activities(panel)).toHaveLength(500);
    expect(activities(panel)[0]?.dataset.key).toBe("v520:op-520");
    expect(root(panel).querySelector(".after-rows")?.textContent).toBe(
      "The list is limited to 500 activities.",
    );
  });

  it("starts the read again when it was cut short by leaving the document", async () => {
    const source = new FakeProvenanceSource();
    const gate = provGate();
    source.historyGate = gate;
    const panel = await mount(source);
    expect(panel.dataset.state).toBe("loading");
    panel.remove();
    source.historyGate = undefined;
    gate.open();
    await settle();
    document.body.append(panel);
    await settle();
    expect(panel.dataset.state).toBe("ready");
    expect(activities(panel)).toHaveLength(4);
  });

  it("uses native controls only, no positive tabindex, and passes the accessibility audit", async () => {
    const panel = await mount(new FakeProvenanceSource(), { canViewLogs: true });
    expect(
      root(panel).querySelectorAll("[tabindex]:not([tabindex='-1']):not([tabindex='0'])"),
    ).toHaveLength(0);
    for (const control of root(panel).querySelectorAll("button, a")) {
      expect(control.getAttribute("aria-label") ?? control.textContent).toBeTruthy();
    }
    expect(accessibilityViolations(panel)).toEqual([]);
  });
});
