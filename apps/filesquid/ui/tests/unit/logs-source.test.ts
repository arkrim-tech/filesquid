/**
 * Unit tests for the Logs source (UI-F2.16, UI-S-012): every method is one call through
 * `FileSquidClient`, the libraries come from the recent-libraries route, a host with no library
 * registry answers an empty list, and the oldest entry is read with one ascending query.
 */
import { describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import { ClientLogsSource, type LogsClient } from "../../src/logs/source";

/** A client whose methods answer `overrides`; the rest fail loudly. */
function fakeClient(overrides: Partial<LogsClient>): LogsClient {
  const fail = () => {
    throw new Error("unexpected call");
  };
  return {
    adminJournalPage: fail,
    adminJournalEntry: fail,
    adminJournalExport: fail,
    request: fail,
    subscribe: fail,
    ...overrides,
  } as LogsClient;
}

const ENTRY = {
  id: 1,
  time: "2026-10-01T00:00:00.000Z",
  host: "h",
  actor: "a",
  operation: "app.start",
  outcome: "succeeded" as const,
  level: "info" as const,
  target_ids: [],
  provenance: { journal_only: true },
};

describe("ClientLogsSource", () => {
  it("reads a page and one entry through the client", async () => {
    const adminJournalPage = vi.fn(async () => ({ entries: [ENTRY], next_cursor: null }));
    const adminJournalEntry = vi.fn(async () => ENTRY);
    const source = new ClientLogsSource(fakeClient({ adminJournalPage, adminJournalEntry }));
    expect((await source.page({ order: "desc" })).entries).toHaveLength(1);
    expect(adminJournalPage).toHaveBeenCalledWith({ order: "desc" });
    expect(await source.entry(1)).toBe(ENTRY);
    expect(adminJournalEntry).toHaveBeenCalledWith(1, { names: true });
  });

  it("exports through the client with the query it was given", async () => {
    const adminJournalExport = vi.fn(async () => "id\r\n1\r\n");
    const source = new ClientLogsSource(fakeClient({ adminJournalExport }));
    expect(await source.export({ format: "csv", level: "warn" })).toBe("id\r\n1\r\n");
    expect(adminJournalExport).toHaveBeenCalledWith({ format: "csv", level: "warn" });
  });

  it("reads the oldest entry with one ascending query of one entry", async () => {
    const adminJournalPage = vi.fn(async () => ({ entries: [ENTRY], next_cursor: 1 }));
    const source = new ClientLogsSource(fakeClient({ adminJournalPage }));
    expect(await source.oldest()).toBe(ENTRY);
    expect(adminJournalPage).toHaveBeenCalledWith({ order: "asc", limit: 1 });
  });

  it("answers no oldest entry for an empty journal", async () => {
    const adminJournalPage = vi.fn(async () => ({ entries: [], next_cursor: null }));
    const source = new ClientLogsSource(fakeClient({ adminJournalPage }));
    expect(await source.oldest()).toBeUndefined();
  });

  it("lists the libraries the host opened, with their names", async () => {
    const request = vi.fn(async () => ({
      libraries: [
        { library_id: "lib-1", name: "Family archive", open: true },
        { library_id: "lib-2", name: "Work", open: false },
      ],
    }));
    const source = new ClientLogsSource(fakeClient({ request: request as LogsClient["request"] }));
    expect(await source.libraries()).toEqual([
      { library_id: "lib-1", name: "Family archive" },
      { library_id: "lib-2", name: "Work" },
    ]);
    expect(request).toHaveBeenCalledWith("/api/v1/libraries/recent");
  });

  it("answers no libraries when the host keeps no registry (503), and logs it", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const request = vi.fn(async () => {
      throw new ApiError(503, "/api/v1/libraries/recent", { code: "unavailable" });
    });
    const source = new ClientLogsSource(fakeClient({ request: request as LogsClient["request"] }));
    expect(await source.libraries()).toEqual([]);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });

  it("does not hide any other failure of the libraries route", async () => {
    const request = vi.fn(async () => {
      throw new ApiError(500, "/api/v1/libraries/recent", { code: "internal" });
    });
    const source = new ClientLogsSource(fakeClient({ request: request as LogsClient["request"] }));
    await expect(source.libraries()).rejects.toBeInstanceOf(ApiError);
  });

  it("subscribes to the event channel and passes the unsubscribe through", () => {
    const stop = vi.fn();
    const subscribe = vi.fn(() => stop);
    const source = new ClientLogsSource(fakeClient({ subscribe }));
    const listener = () => undefined;
    const onOpen = () => undefined;
    const end = source.subscribe(listener, onOpen);
    expect(subscribe).toHaveBeenCalledWith(listener, onOpen);
    end();
    expect(stop).toHaveBeenCalled();
  });
});
