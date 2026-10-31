/**
 * UI tests for the File info tab with the facts projection (UI-F2.15; UI-S-022, UI-S-031
 * mockups 35 and 25, API-S-027, ADR 038): the import facts rows, the technical group from the
 * `exif:` statements or "Nothing could be read from this format.", the filter over every row
 * with the count of the properties only, the soft failure of each optional read, and the tab
 * mounted with no source but its own.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { accessibilityViolations, defineElements, type FsDetailsPanel } from "../../src/index";
import type { MetadataStatement } from "../../src/files/metadata";
import labels from "../../src/theme/labels.en.json";
import { FakePanelSource } from "../support/panel-fakes";
import { FakeFactsSource, PROV_FILE, provFacts } from "../support/provenance-fakes";

beforeEach(() => defineElements());
afterEach(() => vi.restoreAllMocks());

const settle = async () => {
  for (let i = 0; i < 6; i += 1) await new Promise<void>((resolve) => setTimeout(resolve, 0));
};

/** An `exif:` statement read from the file. */
function exif(term: string, value: string): MetadataStatement {
  return {
    curie: `exif:${term}`,
    prefix: "exif",
    term,
    value,
    source: "extracted",
  } as MetadataStatement;
}

/** Mount a File info tab. */
async function mount(
  facts = new FakeFactsSource(),
  statements: MetadataStatement[] | Error = [exif("Duration", "6:18")],
): Promise<FsDetailsPanel> {
  const panel = document.createElement("fs-details-panel") as FsDetailsPanel;
  panel.configure({
    labels,
    source: new FakePanelSource(),
    facts,
    statements: {
      statements: async () => {
        if (statements instanceof Error) throw statements;
        return { library_id: "lib", file_id: "f", version: "v4", statements };
      },
    },
    online: () => true,
  });
  document.body.replaceChildren(panel);
  panel.fileRef = PROV_FILE;
  await settle();
  return panel;
}

const root = (panel: HTMLElement) => panel.shadowRoot as ShadowRoot;
const labelsShown = (panel: HTMLElement) =>
  [...root(panel).querySelectorAll(".row dt")].map((dt) => dt.textContent);

describe("<fs-details-panel> as the File info tab with facts", () => {
  it("shows the import facts rows of mockup 35 between the digests and the OCFL object", async () => {
    const panel = await mount();
    expect(panel.dataset.state).toBe("ready");
    expect(labelsShown(panel)).toEqual([
      "Original file name",
      "Extension",
      "Format and MIME type",
      "Size",
      "History size",
      "SHA-256 digest",
      "SHA-512 digest",
      "Uploaded",
      "Source",
      "OCFL object",
      "OCFL version",
      "Duration",
    ]);
    const values = (key: string) =>
      root(panel).querySelector(`.row[data-key="${key}"] .value`)?.textContent;
    expect(values("format")).toBe("JPEG image · image/jpeg");
    expect(values("source")).toBe("Folder upload · volume FUJIFILM");
    expect(values("uploaded")).toMatch(/\(14:20 UTC\)$/);
  });

  it("counts the properties only, with the technical group under its own eyebrow", async () => {
    const panel = await mount();
    expect(root(panel).querySelector(".count")?.textContent).toBe("11 properties · read only");
    const eyebrows = [...root(panel).querySelectorAll(".group-title")].map((e) => e.textContent);
    expect(eyebrows).toEqual(["Technical · read from the file"]);
    expect(
      root(panel).querySelector('.row[data-key="technical-Duration"] .value')?.textContent,
    ).toBe("6:18");
  });

  it("says nothing could be read from a format with no technical statements", async () => {
    const panel = await mount(new FakeFactsSource(), []);
    const note = root(panel).querySelector(".note-row");
    expect(note?.textContent).toBe(labels.filePanelTechnicalNone);
    expect(root(panel).querySelector(".group-title")?.textContent).toBe(
      labels.filePanelTechnicalGroup,
    );
  });

  it("filters the technical rows too, and the no-match state shows when nothing matches", async () => {
    const panel = await mount();
    const input = root(panel).querySelector<HTMLInputElement>("input.filter") as HTMLInputElement;
    input.value = "duration";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    expect(labelsShown(panel)).toEqual(["Duration"]);
    expect(root(panel).querySelector(".count")?.textContent).toBe("0 of 11 properties · read only");
    input.value = "zzz";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    expect(panel.dataset.state).toBe("no-match");
  });

  it("leaves the import rows out and keeps the tab when the facts cannot be read", async () => {
    const facts = new FakeFactsSource();
    facts.error = new ApiError(404, "/x", { code: "not_found" });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const panel = await mount(facts);
    expect(panel.dataset.state).toBe("ready");
    expect(labelsShown(panel)).not.toContain("Uploaded");
    expect(labelsShown(panel)).toContain("SHA-256 digest");
    expect(warn).toHaveBeenCalled();
  });

  it("leaves the technical group out when the statements cannot be read", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const panel = await mount(new FakeFactsSource(), new ApiError(500, "/x", { code: "internal" }));
    expect(panel.dataset.state).toBe("ready");
    expect(root(panel).querySelector(".group-title")).toBeNull();
    expect(labelsShown(panel)).toContain("Uploaded");
  });

  it("reads no facts for a file that is not a file's own original", async () => {
    const facts = new FakeFactsSource();
    const panel = document.createElement("fs-details-panel") as FsDetailsPanel;
    panel.configure({ labels, source: new FakePanelSource(), facts, online: () => true });
    document.body.replaceChildren(panel);
    panel.fileRef = { ...PROV_FILE, path: "Holiday/photo.jpg" };
    await settle();
    expect(facts.calls).toHaveLength(0);
    expect(panel.dataset.state).toBe("ready");
  });

  it("never shows an absolute path, an import root, a host or an account name", async () => {
    const facts = new FakeFactsSource();
    const hostile = provFacts() as unknown as Record<string, unknown>;
    Object.assign(hostile, {
      import_root: "/Users/jane/Pictures",
      host: "jane-macbook",
      account: "jane",
    });
    facts.answer = hostile as never;
    const panel = await mount(facts);
    const text = root(panel).textContent ?? "";
    for (const secret of ["/Users/jane", "jane-macbook"]) expect(text).not.toContain(secret);
  });

  it("is still the one File info tab for the audit: no violations", async () => {
    const panel = await mount();
    expect(accessibilityViolations(panel)).toEqual([]);
  });
});
