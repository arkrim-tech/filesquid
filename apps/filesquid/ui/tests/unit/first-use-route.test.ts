/**
 * Unit tests for the launch redirect to first use (UI-1.3, UI-S-034): with no library known to
 * the host the app opens the welcome screen, whose dialog creates the first library. A host
 * that knows a library, has no registry (503) or fails leaves the launch address alone, and an
 * address the owner already chose is never replaced.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { redirectToFirstUse } from "../../src/app/first-use-route";
import { ApiError } from "../../src/api/client";
import type { RecentLibraries } from "../../src/libraries/model";

const KNOWN: RecentLibraries = {
  libraries: [
    {
      library_id: "lib-1",
      name: "Family archive",
      last_opened: "2026-10-31T09:30:00Z",
      open: true,
      free_bytes: null,
      free_bytes_reason: "volume_unreadable",
    },
  ],
};

/** A window stand-in that records `location.replace`. */
function fakeWindow(hash: string) {
  const replaced: string[] = [];
  const win = {
    location: {
      hash,
      replace: (to: string) => {
        replaced.push(to);
        win.location.hash = to;
      },
    },
  } as unknown as Window;
  return { win, replaced };
}

afterEach(() => vi.restoreAllMocks());

describe("redirectToFirstUse", () => {
  it.each(["", "#", "#/"])(
    "goes to the welcome screen from the launch address %j when no library is known",
    async (hash) => {
      const { win, replaced } = fakeWindow(hash);
      const recent = vi.fn().mockResolvedValue({ libraries: [] });
      await expect(redirectToFirstUse(win, { recent })).resolves.toBe(true);
      expect(replaced).toEqual(["#/library"]);
    },
  );

  it("stays when the host knows a library", async () => {
    const { win, replaced } = fakeWindow("");
    await expect(redirectToFirstUse(win, { recent: async () => KNOWN })).resolves.toBe(false);
    expect(replaced).toEqual([]);
  });

  it("asks nothing when the owner already chose an address", async () => {
    const { win, replaced } = fakeWindow("#/settings");
    const recent = vi.fn();
    await expect(redirectToFirstUse(win, { recent })).resolves.toBe(false);
    expect(recent).not.toHaveBeenCalled();
    expect(replaced).toEqual([]);
  });

  it("does not replace an address the owner chose while the host was answering", async () => {
    const { win, replaced } = fakeWindow("");
    const recent = vi.fn().mockImplementation(async () => {
      win.location.hash = "#/monitor";
      return { libraries: [] };
    });
    await expect(redirectToFirstUse(win, { recent })).resolves.toBe(false);
    expect(replaced).toEqual([]);
  });

  it("stays quietly when the host keeps no library registry (503), and logs another failure", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const unavailable = new ApiError(503, "/api/v1/libraries/recent", {
      code: "unavailable",
      retryable: true,
    });
    const quiet = fakeWindow("");
    await expect(
      redirectToFirstUse(quiet.win, { recent: vi.fn().mockRejectedValue(unavailable) }),
    ).resolves.toBe(false);
    expect(quiet.replaced).toEqual([]);
    expect(warn).not.toHaveBeenCalled();
    const broken = new TypeError("bad answer");
    const loud = fakeWindow("");
    await expect(
      redirectToFirstUse(loud.win, { recent: vi.fn().mockRejectedValue(broken) }),
    ).resolves.toBe(false);
    expect(loud.replaced).toEqual([]);
    expect(warn).toHaveBeenCalledTimes(1);
    expect(warn).toHaveBeenCalledWith(expect.stringContaining("recent libraries"), broken);
  });
});
