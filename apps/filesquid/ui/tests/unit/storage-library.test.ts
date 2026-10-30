/**
 * Unit tests for the stop-gap lookup of the open library and the sidebar's storage block
 * (UI-F2.8, SRS-F-062): the library is named only when exactly one is open, the bytes come from
 * the library's history size, the free space from the recent-libraries list, and a failure
 * leaves the block empty without breaking anything.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { singleOpenLibrary } from "../../src/drive/library";
import { ClientViewSource } from "../../src/drive/source";
import { bindStorage, readStorage } from "../../src/app/storage-route";
import { defineElements, type FsAppShell } from "../../src/index";
import { fakeThemeStore } from "../support/theme-fakes";
import { DESKTOP_DESCRIPTOR } from "../support/host-fakes";
import labels from "../../src/theme/labels.en.json";

beforeEach(() => vi.spyOn(console, "warn").mockImplementation(() => undefined));
afterEach(() => vi.restoreAllMocks());

const library = (id: string, open: boolean, free: number | null = 1000) => ({
  library_id: id,
  name: id,
  last_opened: "2026-10-16T00:00:00Z",
  open,
  free_bytes: free,
  free_bytes_reason: null,
});

describe("singleOpenLibrary", () => {
  it("answers the one open library and reads the list once", async () => {
    const request = vi
      .fn()
      .mockResolvedValue({ libraries: [library("a", true), library("b", false)] });
    const lookup = singleOpenLibrary({ request } as never);
    expect(await lookup()).toBe("a");
    expect(await lookup()).toBe("a");
    expect(request).toHaveBeenCalledTimes(1);
  });

  it("answers nothing when several libraries are open or none is", async () => {
    const two = vi.fn().mockResolvedValue({ libraries: [library("a", true), library("b", true)] });
    expect(await singleOpenLibrary({ request: two } as never)()).toBeUndefined();
    const none = vi.fn().mockResolvedValue({ libraries: [library("a", false)] });
    expect(await singleOpenLibrary({ request: none } as never)()).toBeUndefined();
  });

  it("answers nothing, with a console message, on a host without a registry", async () => {
    const request = vi
      .fn()
      .mockRejectedValue(new ApiError(503, "/api/v1/libraries/recent", { code: "unavailable" }));
    expect(await singleOpenLibrary({ request } as never)()).toBeUndefined();
    expect(console.warn).toHaveBeenCalled();
  });

  it("names the library on every row a client view source returns", async () => {
    const page = {
      rows: [
        {
          kind: "file",
          file_id: "f1",
          object_id: "urn:uuid:f1",
          display_name: "a.pdf",
          media_type: "application/pdf",
          size: 1,
          modified: "2026-10-16T10:00:00Z",
          modified_by_me: null,
          opened_by_me: null,
        },
      ],
      next_cursor: null,
      complete: true,
    };
    const request = vi.fn().mockResolvedValue(page);
    const source = new ClientViewSource({ request } as never, async () => "lib-9");
    const answer = await source.view({
      node: "n",
      view: "all",
      sort: "name",
      direction: "ascending",
      folders: "on_top",
      filter: "",
    });
    expect(answer.rows[0]?.libraryId).toBe("lib-9");
  });
});

describe("the storage block", () => {
  it("reads the history size of the open library and the free space of its disk", async () => {
    const request = vi.fn(async (path: string) =>
      path.startsWith("/api/v1/libraries/recent")
        ? { libraries: [library("lib", true, 5000)] }
        : { library_id: "lib", objects: 1, contents: 1, bytes: 1234, truncated: false },
    );
    expect(await readStorage({ request } as never)).toEqual({ usedBytes: 1234, freeBytes: 5000 });
    expect(request).toHaveBeenLastCalledWith("/api/v1/files/library-history-size?library_id=lib");
  });

  it("says nothing with several open libraries, or when a read fails", async () => {
    const two = vi.fn().mockResolvedValue({ libraries: [library("a", true), library("b", true)] });
    expect(await readStorage({ request: two } as never)).toBeUndefined();
    const broken = vi.fn().mockRejectedValue(new Error("down"));
    expect(await readStorage({ request: broken } as never)).toBeUndefined();
  });

  it("fills the shell's block, leaves out the history caption, and refreshes after a write", async () => {
    defineElements();
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    shell.configure({
      distribution: {
        id: "filesquid",
        productName: "FileSquid",
        version: "0.1.0",
        logo: { src: "/a.svg", alt: "x" },
        labels,
      },
      client: { host: async () => DESKTOP_DESCRIPTOR, subscribe: () => () => undefined } as never,
      theme: fakeThemeStore().store,
    });
    document.body.append(shell);
    let bytes = 1024;
    const request = vi.fn(async (path: string) =>
      path.startsWith("/api/v1/libraries/recent")
        ? { libraries: [library("lib", true, 4096)] }
        : { library_id: "lib", objects: 1, contents: 1, bytes, truncated: false },
    );
    bindStorage(shell, { request } as never);
    expect(request).not.toHaveBeenCalled();
    shell.dispatchEvent(
      new CustomEvent("fs-summary", { detail: { text: "x" }, bubbles: true, composed: true }),
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
    const root = shell.shadowRoot as ShadowRoot;
    expect(root.querySelector(".storage-value")?.textContent).toBe("1 KB");
    expect(root.querySelector<HTMLElement>(".storage-history")?.hidden).toBe(true);
    expect(root.querySelector(".storage-free")?.textContent).toBe("4 KB free on disk");
    bytes = 2048;
    shell.dispatchEvent(new CustomEvent("fs-drive-change", { bubbles: true, composed: true }));
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(root.querySelector(".storage-value")?.textContent).toBe("2 KB");
    shell.remove();
  });
});
