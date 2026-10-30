/**
 * Unit tests for downloading one version of a file (UI-F2.11, UI-S-009, API-F1.3): the whole
 * file through `GET /api/v1/files/download` up to the desktop scheme's 16 MiB answer cap
 * (DE-S-005), ranged `GET /api/v1/files/content` reads of at most 8 MiB above it, a bounded
 * loop, abort, and the API's refusals surfaced; and `saveFileToDevice`, which hands a Blob to
 * the browser and revokes its object URL.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../../src/api/client";
import {
  ClientDownloadSource,
  DESKTOP_ANSWER_CAP,
  MAX_DOWNLOAD_READS,
  RANGE_CHUNK,
  type DownloadClient,
} from "../../src/files/download-source";
import { saveFileToDevice } from "../../src/components/save-file";
import type { FileRef } from "../../src/files/model";

const FILE: FileRef = {
  library_id: "lib 1",
  object_id: "urn:uuid:abc",
  version: "v5",
  path: "original/a b.pdf",
  display_name: "a b.pdf",
};

/** Bytes of a Blob, read with `FileReader` (jsdom's `Blob` has no `arrayBuffer`). */
function blobBytes(blob: Blob): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer));
    reader.onerror = () => reject(reader.error ?? new Error("FileReader failed"));
    reader.readAsArrayBuffer(blob);
  });
}

/** A client over `bytes`: details report its length, download and ranged reads serve it. */
function clientOver(bytes: Uint8Array) {
  const request = vi.fn(async () => ({ size: bytes.byteLength }));
  const read = vi.fn(
    async (_path: string, options: { range?: { start: number; end?: number } }) => {
      const range = options.range;
      const part = range
        ? bytes.slice(range.start, (range.end ?? bytes.byteLength - 1) + 1)
        : bytes;
      const contentRange = range
        ? { start: range.start, end: range.start + part.byteLength - 1, size: bytes.byteLength }
        : undefined;
      return {
        response: new Response(part as BodyInit),
        partial: range !== undefined,
        contentRange,
      };
    },
  );
  return { request, read } as unknown as DownloadClient & {
    request: typeof request;
    read: typeof read;
  };
}

describe("ClientDownloadSource", () => {
  it("reads a small version through the download route with the file's query", async () => {
    const bytes = new Uint8Array([1, 2, 3, 4]);
    const c = clientOver(bytes);
    const blob = await new ClientDownloadSource(c).readVersionFile(FILE);
    expect([...(await blobBytes(blob))]).toEqual([1, 2, 3, 4]);
    expect(c.read).toHaveBeenCalledTimes(1);
    const [path, options] = c.read.mock.calls[0] as unknown as [string, { range?: unknown }];
    const url = new URL(path, "http://x");
    expect(url.pathname).toBe("/api/v1/files/download");
    expect(Object.fromEntries(url.searchParams)).toEqual({
      library_id: "lib 1",
      object_id: "urn:uuid:abc",
      version: "v5",
      path: "original/a b.pdf",
    });
    expect(options.range).toBeUndefined();
  });

  it("asks the details route for the size of that version first", async () => {
    const c = clientOver(new Uint8Array(1));
    await new ClientDownloadSource(c).readVersionFile(FILE);
    const [path] = c.request.mock.calls[0] as unknown as [string];
    const url = new URL(path, "http://x");
    expect(url.pathname).toBe("/api/v1/files/details");
    expect(url.searchParams.get("version")).toBe("v5");
  });

  it("reads a version above the desktop cap with ranged reads of at most 8 MiB", async () => {
    const size = DESKTOP_ANSWER_CAP + 5;
    const bytes = Uint8Array.from({ length: size }, (_, i) => i % 251);
    const c = clientOver(bytes);
    const blob = await new ClientDownloadSource(c).readVersionFile(FILE);
    expect(blob.size).toBe(size);
    expect(Buffer.from(await blobBytes(blob)).equals(Buffer.from(bytes))).toBe(true);
    const ranges = c.read.mock.calls.map(
      (call) => (call[1] as { range: { start: number; end: number } }).range,
    );
    expect(ranges.length).toBe(Math.ceil(size / RANGE_CHUNK));
    expect(ranges[0]).toEqual({ start: 0, end: RANGE_CHUNK - 1 });
    expect(ranges.at(-1)).toEqual({ start: RANGE_CHUNK * (ranges.length - 1), end: size - 1 });
    const path = c.read.mock.calls[0]?.[0] as string;
    expect(new URL(path, "http://x").pathname).toBe("/api/v1/files/content");
  });

  it("refuses a version beyond the bound of reads instead of looping", async () => {
    const size = RANGE_CHUNK * MAX_DOWNLOAD_READS + 1;
    const c = clientOver(new Uint8Array(1));
    c.request.mockResolvedValue({ size });
    await expect(new ClientDownloadSource(c).readVersionFile(FILE)).rejects.toMatchObject({
      code: "payload_too_large",
    });
    expect(c.read).not.toHaveBeenCalled();
  });

  it.each([
    ["a whole 200 answer that ignored the Range", { partial: false, contentRange: undefined }],
    ["a 206 without a Content-Range", { partial: true, contentRange: undefined }],
    ["a 206 for another range", { partial: true, contentRange: { start: 1, end: RANGE_CHUNK } }],
    [
      "a 206 for a shorter range",
      { partial: true, contentRange: { start: 0, end: RANGE_CHUNK - 2 } },
    ],
  ])("refuses %s instead of saving a corrupted file", async (_name, answer) => {
    const size = DESKTOP_ANSWER_CAP + 5;
    const c = clientOver(new Uint8Array(size));
    c.read.mockResolvedValueOnce({
      response: new Response(new Uint8Array(RANGE_CHUNK)),
      ...answer,
    } as never);
    await expect(new ClientDownloadSource(c).readVersionFile(FILE)).rejects.toBeInstanceOf(
      TypeError,
    );
    expect(c.read).toHaveBeenCalledTimes(1);
  });

  it("refuses a 206 whose body is not the length its Content-Range names", async () => {
    const size = DESKTOP_ANSWER_CAP + 5;
    const c = clientOver(new Uint8Array(size));
    const contentRange = { start: 0, end: RANGE_CHUNK - 1, size };
    c.read.mockResolvedValueOnce({
      response: new Response(new Uint8Array(RANGE_CHUNK - 1)),
      partial: true,
      contentRange,
    });
    await expect(new ClientDownloadSource(c).readVersionFile(FILE)).rejects.toBeInstanceOf(
      TypeError,
    );
  });

  it("passes the abort signal on and stops reading when it aborts", async () => {
    const size = DESKTOP_ANSWER_CAP + 5;
    const c = clientOver(new Uint8Array(size));
    const abort = new AbortController();
    const seen: (AbortSignal | undefined)[] = [];
    c.read.mockImplementationOnce(async (_path, options) => {
      seen.push((options as { signal?: AbortSignal }).signal);
      abort.abort();
      const contentRange = { start: 0, end: RANGE_CHUNK - 1, size };
      return { response: new Response(new Uint8Array(RANGE_CHUNK)), partial: true, contentRange };
    });
    await expect(
      new ClientDownloadSource(c).readVersionFile(FILE, { signal: abort.signal }),
    ).rejects.toMatchObject({ name: "AbortError" });
    expect(c.read).toHaveBeenCalledTimes(1);
    expect(seen).toEqual([abort.signal]);
  });

  it("surfaces the API's refusal", async () => {
    const c = clientOver(new Uint8Array(1));
    c.read.mockRejectedValue(new ApiError(404, "/api/v1/files/download", { code: "not_found" }));
    await expect(new ClientDownloadSource(c).readVersionFile(FILE)).rejects.toMatchObject({
      status: 404,
    });
  });
});

describe("saveFileToDevice", () => {
  afterEach(() => vi.restoreAllMocks());

  it("clicks a download link named for the file and revokes its object URL", () => {
    const create = vi.fn(() => "blob:fs-test");
    const revoke = vi.fn();
    Object.assign(URL, { createObjectURL: create, revokeObjectURL: revoke });
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (
      this: HTMLAnchorElement,
    ) {
      expect(this.download).toBe("a b.pdf");
      expect(this.href).toBe("blob:fs-test");
    });
    saveFileToDevice(new Blob(["x"]), "a b.pdf");
    expect(click).toHaveBeenCalledTimes(1);
    expect(revoke).toHaveBeenCalledWith("blob:fs-test");
    expect(document.querySelector("a[download]")).toBeNull();
  });
});
