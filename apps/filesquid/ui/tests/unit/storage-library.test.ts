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

  it("reads again after the route changes, so a library opened later is found", async () => {
    const events = new EventTarget();
    const request = vi
      .fn()
      .mockResolvedValueOnce({ libraries: [library("a", false)] })
      .mockResolvedValueOnce({ libraries: [library("a", true)] });
    const lookup = singleOpenLibrary({ request } as never, events);
    expect(await lookup()).toBeUndefined();
    expect(await lookup()).toBeUndefined();
    expect(request).toHaveBeenCalledTimes(1);
    events.dispatchEvent(new Event("hashchange"));
    expect(await lookup()).toBe("a");
    expect(request).toHaveBeenCalledTimes(2);
  });

  it("answers nothing when the list is at its cap of 20, since an open library may be out of sight", async () => {
    const full = Array.from({ length: 20 }, (_, i) => library(`l${i}`, i === 0));
    const request = vi.fn().mockResolvedValue({ libraries: full });
    expect(await singleOpenLibrary({ request } as never)()).toBeUndefined();
    const short = vi.fn().mockResolvedValue({ libraries: full.slice(0, 19) });
    expect(await singleOpenLibrary({ request: short } as never)()).toBe("l0");
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

  it("leaves the free space out when the host cannot say, instead of reporting none", async () => {
    const request = vi.fn(async (path: string) =>
      path.startsWith("/api/v1/libraries/recent")
        ? { libraries: [library("lib", true, null)] }
        : { library_id: "lib", objects: 1, contents: 1, bytes: 1234, truncated: false },
    );
    const summary = await readStorage({ request } as never);
    expect(summary).toEqual({ usedBytes: 1234 });
    expect(summary).not.toHaveProperty("freeBytes");
  });

  it("says nothing when the recent list is at its cap of 20", async () => {
    const full = Array.from({ length: 20 }, (_, i) => library(`l${i}`, i === 0));
    const request = vi.fn().mockResolvedValue({ libraries: full });
    expect(await readStorage({ request } as never)).toBeUndefined();
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
    bindStorage(shell, { request } as never, 0);
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

  it("reads the storage once after a burst of writes", async () => {
    defineElements();
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    const request = vi.fn(async (path: string) =>
      path.startsWith("/api/v1/libraries/recent")
        ? { libraries: [library("lib", true)] }
        : { library_id: "lib", objects: 1, contents: 1, bytes: 1, truncated: false },
    );
    vi.useFakeTimers();
    try {
      bindStorage(shell, { request } as never, 300);
      for (let i = 0; i < 5; i += 1) {
        shell.dispatchEvent(new CustomEvent("fs-drive-change"));
        await vi.advanceTimersByTimeAsync(50);
      }
      expect(request).not.toHaveBeenCalled();
      await vi.advanceTimersByTimeAsync(400);
      expect(
        request.mock.calls.filter(([path]) => String(path).includes("history-size")),
      ).toHaveLength(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it("drops a read still in flight when it is stopped", async () => {
    defineElements();
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    let answer: (bytes: number) => void = () => undefined;
    const request = vi.fn(async (path: string) => {
      if (path.startsWith("/api/v1/libraries/recent")) return { libraries: [library("lib", true)] };
      const bytes = await new Promise<number>((resolve) => (answer = resolve));
      return { library_id: "lib", objects: 1, contents: 1, bytes, truncated: false };
    });
    const seen: unknown[] = [];
    Object.defineProperty(shell, "storage", {
      set: (value) => seen.push(value),
      configurable: true,
    });
    const stop = bindStorage(shell, { request } as never, 0);
    shell.dispatchEvent(new CustomEvent("fs-summary", { detail: { text: "x" } }));
    await new Promise((resolve) => setTimeout(resolve, 5));
    stop();
    answer(1000);
    await new Promise((resolve) => setTimeout(resolve, 5));
    expect(seen).toEqual([]);
  });

  it("keeps the newest summary when an older read finishes last", async () => {
    defineElements();
    const shell = document.createElement("fs-app-shell") as FsAppShell;
    const answers: ((bytes: number) => void)[] = [];
    const request = vi.fn(async (path: string) => {
      if (path.startsWith("/api/v1/libraries/recent")) return { libraries: [library("lib", true)] };
      const bytes = await new Promise<number>((resolve) => answers.push(resolve));
      return { library_id: "lib", objects: 1, contents: 1, bytes, truncated: false };
    });
    const seen: unknown[] = [];
    Object.defineProperty(shell, "storage", {
      set: (value) => seen.push(value),
      configurable: true,
    });
    bindStorage(shell, { request } as never, 0);
    shell.dispatchEvent(new CustomEvent("fs-summary", { detail: { text: "x" } }));
    await new Promise((resolve) => setTimeout(resolve, 5));
    shell.dispatchEvent(new CustomEvent("fs-drive-change"));
    await new Promise((resolve) => setTimeout(resolve, 5));
    expect(answers).toHaveLength(2);
    answers[1]?.(2000);
    await new Promise((resolve) => setTimeout(resolve, 0));
    answers[0]?.(1000);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(seen).toEqual([{ usedBytes: 2000, freeBytes: 1000 }]);
  });
});
