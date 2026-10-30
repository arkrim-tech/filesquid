/**
 * `<fs-details-panel>`: the Details panel (UI-F2.3, ADR 028 Decision 5), shown as the File info
 * tab of the file panel (UI-S-031, mockup `resources/ui-mockups/v3/35-panel-file-info.svg`). It
 * shows the storage facts of the opened file version, each owned by the OCFL module (UI-S-003):
 * the original file name, extension, MIME type, size, digests, OCFL object and version, read
 * through `GET /api/v1/files/details` and `GET /api/v1/files/versions`, and the bytes its
 * history keeps through `GET /api/v1/files/history-size` (UI-F2.11; the row is left out when that
 * read fails). The stored name, the logical path and any content location never reach the DOM (UI-S-019, API-S-005).
 *
 * It is a panel of the file panel's named slots: it receives the file as its `fileRef`
 * property and holds no reference to any other panel, so it mounts alone. Its table is the
 * shared {@link PropertyTable} (filter, count, rows with copy, the loading, no-match and error
 * states of mockup 25, the footer), which UI-F2.15 reuses for the other tabs.
 *
 * Accessibility (UI-S-016): the shared stylesheet comes first; the rows are a description list;
 * copy buttons are named by their property; a copy is announced in a polite status region;
 * the error state is an alert; focus is never moved by a load.
 */
import { formatLabel, type LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { PROPERTY_TABLE_STYLE, PropertyTable } from "./property-table";
import { detailRows } from "../files/details";
import type { FileRef } from "../files/model";
import type { FileSource } from "../files/source";
import type { HistorySource } from "../files/history-source";

/** The reads the panel needs: the storage facts, the object's versions and the history size. */
export type DetailsSource = Pick<FileSource, "details"> &
  Pick<HistorySource, "versions" | "historySize">;

/** What the panel needs. */
export interface DetailsPanelOptions {
  /** The active label table. */
  labels: LabelTable;
  /** Reads the storage facts and versions. */
  source: DetailsSource;
  /** Whether the browser is online; `navigator.onLine` when absent. */
  online?: () => boolean;
}

const STYLE = `
  :host { display: flex; flex-direction: column; min-block-size: 100%; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-ui); color: var(--fs-on-surface); }
  :host([hidden]) { display: none; }
`;

/** Custom element that shows the storage facts of one file version. */
export class FsDetailsPanel extends HTMLElement {
  private options?: DetailsPanelOptions;
  private table?: PropertyTable;
  private file: FileRef | null = null;
  private generation = 0;
  private abort?: AbortController;

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the labels and source, render, and load the file already given, if any.
   *
   * @param options Labels, the source and an optional online check.
   */
  configure(options: DetailsPanelOptions): void {
    this.options = options;
    this.build(options);
    void this.load();
  }

  /** The tab label this panel asks for ("File info"). */
  get tabLabel(): string {
    return this.options?.labels.filePanelInfo ?? "";
  }

  /** The file version shown, or null. */
  get fileRef(): FileRef | null {
    return this.file;
  }

  /** Show another file version (null clears the panel). */
  set fileRef(file: FileRef | null) {
    this.file = file;
    void this.load();
  }

  /** Cancel a read in flight when the element leaves the document. */
  disconnectedCallback(): void {
    this.abort?.abort();
  }

  /** Build the stylesheet and the table. */
  private build(options: DetailsPanelOptions): void {
    const labels = options.labels;
    const table = new PropertyTable({
      labels,
      filterPlaceholder: labels.filePanelFilterInfo,
      loadingText: labels.filePanelInfoLoading,
      errorTitle: labels.filePanelInfoError,
      footer: labels.filePanelInfoFooter,
      countText: (shown, total) => countText(labels, shown, total),
      noMatchTitle: (word) => formatLabel(labels.filePanelInfoNoMatch, { word }),
      onRetry: () => void this.load(),
      online: options.online,
      onState: (state) => void (this.dataset.state = state),
    });
    this.table = table;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + CONTROL_STYLE + STYLE + PROPERTY_TABLE_STYLE;
    this.shadowRoot?.replaceChildren(style, table.element);
  }

  /** Read the facts and versions of the file shown and fill the table; ignore stale answers. */
  private async load(): Promise<void> {
    const { options, table, file } = this;
    if (!options || !table) return;
    this.abort?.abort();
    const generation = ++this.generation;
    if (!file) return table.clear();
    const abort = new AbortController();
    this.abort = abort;
    table.loading();
    try {
      const signal = abort.signal;
      const [details, versions, history] = await Promise.all([
        options.source.details(file, { signal }),
        options.source.versions(file, { signal }),
        readHistorySize(options.source, file, signal),
      ]);
      if (generation !== this.generation) return;
      const locale = document.documentElement.lang || "en";
      table.rows(detailRows(details, versions, options.labels, locale, history));
    } catch (error) {
      if (generation !== this.generation || abort.signal.aborted) return;
      console.warn("FileSquid could not read file details", errorName(error));
      table.error(error);
    }
  }
}

/** The file's history size; one that cannot be read leaves its row out and is logged. */
async function readHistorySize(
  source: DetailsSource,
  file: FileRef,
  signal: AbortSignal,
): Promise<{ versions: number; bytes: number } | undefined> {
  try {
    return await source.historySize(file, { signal });
  } catch (error) {
    // A read this panel cancelled itself (its next load, or leaving the document) is not a failure.
    if (!signal.aborted)
      console.warn("FileSquid could not read the history size", errorName(error));
    return undefined;
  }
}

/** The count eyebrow: all rows, or how many of them match the filter. */
function countText(labels: LabelTable, shown: number, total: number): string {
  const locale = document.documentElement.lang || "en";
  const number = (n: number) => new Intl.NumberFormat(locale).format(n);
  if (shown === total) return formatLabel(labels.filePanelInfoCount, { count: number(total) });
  return formatLabel(labels.filePanelInfoCountFiltered, {
    shown: number(shown),
    count: number(total),
  });
}

/** The kind of an error, without its message. */
function errorName(error: unknown): string {
  return error instanceof Error ? error.name : typeof error;
}
