/**
 * UI tests for the files route's deep link (UI-F2.16; UI-S-021, UI-S-020): the Logs view's View
 * provenance and Show file open one file's panel on a tab through
 * `#/files/<library>/urn:uuid:<file>/original?open=<file>&tab=<tab>`. The browser lists the
 * file's own original, opens the file whose original that ID is, and the panel shows the tab
 * asked for; an address that names no such file opens none, and a later listing never opens a
 * file by itself.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  bindFilesRoute,
  defineElements,
  openFileHash,
  type FsFileBrowser,
  type FsFilePanel,
} from "../../src/index";
import { FILE_ID, fileObjectFixture, settle } from "../support/file-browser-fakes";
import { FakePanelSource } from "../support/panel-fakes";
import { FakeMetadataSource } from "../support/metadata-fakes";
import { FakeFactsSource, FakeProvenanceSource } from "../support/provenance-fakes";
import labels from "../../src/theme/labels.en.json";

let host: HTMLElement & { focusMain: () => void };
let stop: () => void;

beforeEach(() => {
  defineElements();
  window.history.replaceState(null, "", "#/");
  host = Object.assign(document.createElement("div"), { focusMain: vi.fn() });
  document.body.replaceChildren(host);
});
afterEach(() => {
  vi.restoreAllMocks();
  stop?.();
  document.body.replaceChildren();
  window.history.replaceState(null, "", "#/");
});

/** Bind the route over the file object fixture with every tab's source. */
function bind(): void {
  const panels = new FakePanelSource();
  stop = bindFilesRoute(host, {
    labels,
    source: fileObjectFixture(),
    metadata: new FakeMetadataSource(),
    filePanel: {
      history: panels,
      download: panels,
      facts: new FakeFactsSource(),
      provenance: new FakeProvenanceSource(),
    },
  });
}

/** Go to an address as a link does, and let the listing and the panel load. */
async function go(hash: string): Promise<void> {
  window.history.replaceState(null, "", hash);
  window.dispatchEvent(new HashChangeEvent("hashchange"));
  await settle();
  await settle();
}

/** The browser in the host. */
const browser = () => host.querySelector("fs-file-browser") as FsFileBrowser;
/** The file panel in the host, if open. */
const panel = () => host.querySelector<FsFilePanel>("fs-file-panel");
/** The id of the tab selected in a panel. */
const selectedTab = (p: FsFilePanel) =>
  p.shadowRoot?.querySelector('[role="tab"][aria-selected="true"]')?.id;

describe("the open-file deep link", () => {
  it("opens the file whose own original the ID names, on the Provenance tab", async () => {
    bind();
    await go(openFileHash("lib", FILE_ID, "provenance"));
    expect(browser().location).toEqual({
      library_id: "lib",
      object_id: `urn:uuid:${FILE_ID}`,
      prefix: "original",
    });
    expect(browser().file?.display_name).toBe("readme.txt");
    expect(panel()?.fileRef?.display_name).toBe("readme.txt");
    expect(selectedTab(panel() as FsFilePanel)).toBe("tab-provenance");
  });

  it("opens it on File info for Show file", async () => {
    bind();
    await go(openFileHash("lib", FILE_ID, "details"));
    expect(selectedTab(panel() as FsFilePanel)).toBe("tab-details");
  });

  it("opens the file with the panel's first tab when the link names none", async () => {
    bind();
    await go(openFileHash("lib", FILE_ID));
    expect(browser().file?.display_name).toBe("readme.txt");
    expect(selectedTab(panel() as FsFilePanel)).toBe("tab-details");
  });

  it("opens the file again when the folder is already shown, as a second link does", async () => {
    bind();
    await go(openFileHash("lib", FILE_ID, "details"));
    (host.querySelector("fs-file-browser") as FsFileBrowser).shadowRoot
      ?.querySelector<HTMLButtonElement>("button.back")
      ?.click();
    expect(browser().file).toBeNull();
    await go(openFileHash("lib", FILE_ID, "provenance"));
    expect(browser().file?.display_name).toBe("readme.txt");
    expect(selectedTab(panel() as FsFilePanel)).toBe("tab-provenance");
  });

  it("opens no file when the address names none that is in the folder, and none by itself later", async () => {
    bind();
    const other = "00000000-0000-4000-8000-000000000001";
    await go(`#/files/lib/urn%3Auuid%3A${FILE_ID}/original?open=${other}&tab=provenance`);
    expect(browser().file).toBeNull();
    expect(panel()).toBeNull();
    await go(`#/files/lib/urn%3Auuid%3A${FILE_ID}/original`);
    expect(browser().file).toBeNull();
  });

  it("shows the folder only for an address with no open request", async () => {
    bind();
    await go(`#/files/lib/urn%3Auuid%3A${FILE_ID}/original`);
    expect(browser().file).toBeNull();
    expect(browser().location.prefix).toBe("original");
  });
});

describe("the file panel's tab selection", () => {
  it("selects a registered tab, and leaves the panel alone for an unknown one", async () => {
    bind();
    await go(openFileHash("lib", FILE_ID, "details"));
    const p = panel() as FsFilePanel;
    expect(p.selectTab("provenance")).toBe(true);
    expect(selectedTab(p)).toBe("tab-provenance");
    expect(p.selectTab("nothing")).toBe(false);
    expect(selectedTab(p)).toBe("tab-provenance");
  });
});
