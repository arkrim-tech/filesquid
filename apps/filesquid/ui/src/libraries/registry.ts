/**
 * An in-memory record of the libraries this page has created or opened (UI-1.1): the library
 * page shows the facts the flow just read, and the revision a copy or rebuild expects, without
 * a second request. A rebuild is a durable job (API-F2.19), so its report is the job's result,
 * read from the job routes (UI-1.3), and is not kept here. It holds only what the API answered (opaque IDs, counts, error codes), never
 * a path, and nothing is written to Web Storage: the library's address `#/library/<id>` is what
 * later pages (such as the import UI) read the open library from.
 */
import type { LibraryView } from "./model";

/** Most libraries kept; the API allows at most 64 open libraries. */
const MAX_LIBRARIES = 64;

/** Libraries seen in this page's session, oldest first, bounded to 64. */
export class LibraryRegistry {
  private readonly views = new Map<string, LibraryView>();

  /** How many libraries are remembered. */
  get size(): number {
    return this.views.size;
  }

  /**
   * Remember a created or opened library, replacing its earlier view and, when the registry is
   * full, forgetting the oldest library.
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
   * The revision a copy or rebuild of the library expects (API-S-019, API-F2.19): the one
   * the API answered when this page created or opened it.
   *
   * @param libraryId The library ID.
   * @returns The expected revision, or undefined when this page did not create or open the
   *   library (the page then reads the current one through `LibrarySource.revision`).
   */
  revisionOf(libraryId: string): string | undefined {
    return this.views.get(libraryId)?.revision;
  }
}
