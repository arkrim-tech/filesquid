/**
 * The file panel of the files route (UI-F2.3, UI-F2.4, UI-F2.15, UI-S-031): composes
 * `<fs-file-panel>` with the Details panel as its File info tab (with the facts and technical
 * rows when their sources are given), the Metadata panel as its Description tab (when a metadata
 * source is given), the Provenance panel as its Provenance tab (when a provenance source is
 * given) and the Versions view behind its history button, puts it in the
 * shell's `panel` slot while a file is open in the browser, and reloads the browser's listing
 * after a panel commits a version (a restore, or a metadata edit). The panels read and write
 * only through the file source, the history source, the download source and the metadata
 * source (UI-S-012, ADR 030).
 */
import type { FsDetailsPanel } from "../components/fs-details-panel";
import type { FsMetadataPanel } from "../components/fs-metadata-panel";
import type { FsProvenancePanel } from "../components/fs-provenance-panel";
import type { FsFileBrowser } from "../components/fs-file-browser";
import { HOST_EVENT } from "../components/fs-host-indicator";
import { FILE_PANEL_CLOSE_EVENT, type FsFilePanel } from "../components/fs-file-panel";
import type { FsVersionsPanel, VersionsSource } from "../components/fs-versions-panel";
import type { FileRef } from "../files/model";
import type { FileSource } from "../files/source";
import type { HistorySource } from "../files/history-source";
import type { DownloadSource } from "../files/download-source";
import type { MetadataSource } from "../files/metadata-source";
import type { FactsSource } from "../files/facts-source";
import type { ProvenanceSource } from "../files/provenance-source";
import type { LabelTable } from "../theme/labels";

/**
 * What the file panel needs: the labels, the file source, the history source and the
 * download source.
 */
export interface FilePanelRouteOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** Reads storage facts, restores and sets version labels. */
  readonly source: FileSource;
  /** Reads histories, versions and history sizes. */
  readonly history: HistorySource;
  /** Reads the whole file of one version for saving (Download this version). */
  readonly download: DownloadSource;
  /**
   * Reads and writes statements; without it the panel has no Description tab, and the File info
   * tab has no technical rows.
   */
  readonly metadata?: MetadataSource;
  /** Reads the facts projection of the file's record; without it File info has no import rows. */
  readonly facts?: FactsSource;
  /** Reads the history, its records and its export; without it the panel has no Provenance tab. */
  readonly provenance?: ProvenanceSource;
  /** Whether the viewer may open the Logs view (View in Logs); never when absent. */
  readonly canViewLogs?: () => boolean;
}

/**
 * Follow which host the shell shows, from its host badge's `fs-host` events, from now on: the
 * owner is the person on the desktop host, the only host with a Logs view (UI-S-021). Call it
 * when the route is bound, before the panel exists, so a host that is known by the time a file
 * opens is not missed. Side effect: adds an event listener to the shell.
 *
 * @param shell The app shell, whose host badge dispatches the events.
 * @returns A function that tells whether the viewer may open the Logs view now.
 */
export function watchOwner(shell: HTMLElement): () => boolean {
  let hostType: string | undefined;
  shell.addEventListener(HOST_EVENT, (event) => {
    hostType = (event as CustomEvent<{ hostType?: string }>).detail?.hostType;
  });
  return () => hostType === "desktop";
}

/** The tag of the file panel the route mounts. */
const PANEL_TAG = "fs-file-panel";

/**
 * The reads, the restore, the label edit and the download the panels use, from the file source,
 * the history source and the download source. Pure.
 *
 * @param source The file source.
 * @param history The history source.
 * @param download The download source.
 * @returns One object with the seven methods, each delegating to its source.
 */
export function panelSource(
  source: FileSource,
  history: HistorySource,
  download: DownloadSource,
): VersionsSource & Pick<FileSource, "details"> {
  return {
    details: (file, options) => source.details(file, options),
    restore: (object, version, paths, options) => source.restore(object, version, paths, options),
    setVersionLabel: (object, version, label, options) =>
      source.setVersionLabel(object, version, label, options),
    readVersionFile: (file, options) => download.readVersionFile(file, options),
    history: (file, options) => history.history(file, options),
    versions: (object, options) => history.versions(object, options),
    historySize: (file, options) => history.historySize(file, options),
  };
}

/**
 * Build the file panel with its File info, Description and Provenance tabs and Versions view.
 * The tabs are exactly those three: the journal is browsed only in the Logs view (UI-S-022).
 */
function createPanel(options: FilePanelRouteOptions, canViewLogs: () => boolean): FsFilePanel {
  const { labels } = options;
  const source = panelSource(options.source, options.history, options.download);
  const panel = document.createElement(PANEL_TAG) as FsFilePanel;
  panel.slot = "panel";
  panel.configure({ labels });
  panel.registerTab("details", labels.filePanelInfo, () => {
    const details = document.createElement("fs-details-panel") as FsDetailsPanel;
    details.configure({ labels, source, facts: options.facts, statements: options.metadata });
    return details;
  });
  const metadata = options.metadata;
  if (metadata) {
    panel.registerTab("description", labels.filePanelDescription, () => {
      const description = document.createElement("fs-metadata-panel") as FsMetadataPanel;
      description.configure({ labels, source: metadata });
      return description;
    });
  }
  const provenance = options.provenance;
  if (provenance) {
    panel.registerTab("provenance", labels.filePanelProvenance, () => {
      const element = document.createElement("fs-provenance-panel") as FsProvenancePanel;
      element.configure({ labels, source: provenance, canViewLogs });
      return element;
    });
  }
  panel.setVersions(() => {
    const versions = document.createElement("fs-versions-panel") as FsVersionsPanel;
    versions.configure({ labels, source });
    return versions;
  });
  return panel;
}

/**
 * Follow the browser's open file with a file panel in the shell's `panel` slot: shown with the
 * file on `fs-file-open`, removed when the viewer closes or the panel's close is pressed; a
 * version a panel commits (`fs-files-change`) reloads the browser. Side effect: adds listeners
 * and mutates the shell's children.
 *
 * @param shell The element the panel is slotted into (the app shell).
 * @param browser The file browser whose open file the panel shows.
 * @param options Labels, the file, history and download sources, and optionally the metadata
 *   source.
 */
export function bindFilePanel(
  shell: HTMLElement,
  browser: FsFileBrowser,
  options: FilePanelRouteOptions,
): void {
  const panel = createPanel(options, options.canViewLogs ?? (() => false));
  browser.addEventListener("fs-file-open", (event) => {
    const { file } = (event as CustomEvent<{ file: FileRef | null }>).detail;
    panel.fileRef = file;
    if (file === null) panel.remove();
    else if (!panel.isConnected) shell.append(panel);
  });
  panel.addEventListener(FILE_PANEL_CLOSE_EVENT, () => panel.remove());
  panel.addEventListener("fs-files-change", () => void browser.reload());
}

/**
 * Remove the file panel the route mounted, if any (the route left the files). Side effect:
 * mutates the shell's children.
 *
 * @param shell The element the panel was slotted into.
 */
export function removeFilePanel(shell: HTMLElement): void {
  shell.querySelector(`:scope > ${PANEL_TAG}`)?.remove();
}
