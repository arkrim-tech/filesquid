/**
 * An in-memory record of the libraries this page has created, opened or rebuilt (UI-1.1): the
 * library page shows the facts and the last rebuild report the flow just produced without a
 * second request. It holds only what the API answered (opaque IDs, counts, error codes), never
 * a path, and nothing is written to Web Storage: the library's address `#/library/<id>` is what
 * later pages (such as the import UI) read the open library from.
 */
import type { LibraryView, RebuildReport } from "./model";

/** Most libraries kept; the API allows at most 64 open libraries. */
const MAX_LIBRARIES = 64;

/** Libraries seen in this page's session, oldest first, bounded to 64. */
export class LibraryRegistry {
  private readonly views = new Map<string, LibraryView>();
  private readonly rebuilds = new Map<string, RebuildReport>();

  /** How many libraries are remembered. */
  get size(): number {
    return this.views.size;
  }

  /**
   * Remember a created or opened library, replacing its earlier view and, when the registry is
   * full, forgetting the oldest library and its rebuild report.
   *
   * @param view The library as the API answered it.
   */
  remember(view: LibraryView): void {
    this.views.delete(view.library_id);
    this.views.set(view.library_id, view);
    while (this.views.size > MAX_LIBRARIES) {
      const oldest = this.views.keys().next().value;
      if (oldest === undefined) return;
      this.views.delete(oldest);
      this.rebuilds.delete(oldest);
    }
  }

  /**
   * The library's view.
   *
   * @param libraryId The library ID.
   * @returns The view, or undefined when this page did not create or open it.
   */
  get(libraryId: string): LibraryView | undefined {
    return this.views.get(libraryId);
  }

  /**
   * Keep the last rebuild report of a library.
   *
   * @param libraryId The library ID.
   * @param report What the rebuild found.
   */
  rememberRebuild(libraryId: string, report: RebuildReport): void {
    this.rebuilds.set(libraryId, report);
  }

  /**
   * The last rebuild report of a library.
   *
   * @param libraryId The library ID.
   * @returns The report, or undefined when none was made in this session.
   */
  lastRebuild(libraryId: string): RebuildReport | undefined {
    return this.rebuilds.get(libraryId);
  }
}
