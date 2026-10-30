/**
 * The sidebar's storage block (UI-F2.8, UI-S-025, SRS-F-062 "storage used is reported per
 * library"): "Storage used" with the bytes the open library keeps, version history included
 * (`GET /api/v1/files/library-history-size`), a bar against the free space of the disk that
 * holds it (`GET /api/v1/libraries/recent`, API-S-038) and the free-space caption. The API
 * counts every distinct content once and does not split the history out of the total, so the
 * "incl. {history} history" caption is left out rather than guessed. With no open library, or
 * more than one (the library of the shown node is not known to the page yet), the block says
 * nothing. Failures are logged and leave the block empty; they never block the page.
 */
import type { FileSquidClient } from "../api/client";
import type { components } from "../api/schema";
import type { FsAppShell, StorageSummary } from "../components/fs-app-shell";

/** The part of `FileSquidClient` the block needs (injectable in tests). */
export type StorageClient = Pick<FileSquidClient, "request">;

type RecentLibraries = components["schemas"]["RecentLibraries"];
type LibraryHistorySize = components["schemas"]["LibraryHistorySize"];

/**
 * Read the storage facts of the one open library.
 *
 * @param client The API client.
 * @returns The summary, or undefined when there is not exactly one open library or the host
 *   could not say. Network I/O: two reads.
 */
export async function readStorage(client: StorageClient): Promise<StorageSummary | undefined> {
  try {
    const recent = await client.request<RecentLibraries>("/api/v1/libraries/recent");
    const open = recent.libraries.filter((library) => library.open);
    const only = open.length === 1 ? open[0] : undefined;
    if (!only) return undefined;
    const path = `/api/v1/files/library-history-size?library_id=${encodeURIComponent(only.library_id)}`;
    const size = await client.request<LibraryHistorySize>(path);
    // A null free space (volume unreadable, or not local) is unknown, never zero.
    if (only.free_bytes === null || only.free_bytes === undefined) return { usedBytes: size.bytes };
    return { usedBytes: size.bytes, freeBytes: only.free_bytes };
  } catch (error) {
    console.warn("FileSquid could not read the storage used", error);
    return undefined;
  }
}

/**
 * Fill the shell's storage block when the first view reports (`fs-summary`, so a page that
 * shows no files asks nothing) and again after each committed write (`fs-drive-change`). Side
 * effect: reads the API and sets `shell.storage`.
 *
 * @param shell The app shell.
 * @param client The API client.
 * @returns A function that stops following writes.
 */
export function bindStorage(shell: FsAppShell, client: StorageClient): () => void {
  let started = false;
  let latest = 0;
  const refresh = () => {
    const mine = ++latest;
    // Only the newest read may set the block; an older one that finishes later is dropped.
    void readStorage(client).then((summary) => {
      if (mine === latest) shell.storage = summary;
    });
  };
  const first = () => {
    if (started) return;
    started = true;
    refresh();
  };
  shell.addEventListener("fs-summary", first);
  shell.addEventListener("fs-drive-change", refresh);
  return () => {
    shell.removeEventListener("fs-summary", first);
    shell.removeEventListener("fs-drive-change", refresh);
  };
}
