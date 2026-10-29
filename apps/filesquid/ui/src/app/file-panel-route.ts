/**
 * The file panel of the files route (UI-F2.3, UI-S-031): composes `<fs-file-panel>` with the
 * Details panel as its File info tab and the Versions view behind its history button, puts it
 * in the shell's `panel` slot while a file is open in the browser, and reloads the browser's
 * listing after a panel commits a version (a restore). The panels read and write only through
 * the file source and the history source (UI-S-012, ADR 030).
 */
import type { FsDetailsPanel } from "../components/fs-details-panel";
import type { FsFileBrowser } from "../components/fs-file-browser";
import { FILE_PANEL_CLOSE_EVENT, type FsFilePanel } from "../components/fs-file-panel";
import type { FsVersionsPanel, VersionsSource } from "../components/fs-versions-panel";
import type { FileRef } from "../files/model";
import type { FileSource } from "../files/source";
import type { HistorySource } from "../files/history-source";
import type { LabelTable } from "../theme/labels";

/** What the file panel needs: the labels, the file source and the history source. */
export interface FilePanelRouteOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** Reads storage facts and restores. */
  readonly source: FileSource;
  /** Reads histories, versions and history sizes. */
  readonly history: HistorySource;
}

/** The tag of the file panel the route mounts. */
const PANEL_TAG = "fs-file-panel";

/**
 * The reads and the restore the panels use, from the file source and the history source. Pure.
 *
 * @param source The file source.
 * @param history The history source.
 * @returns One object with the five methods, each delegating to its source.
 */
export function panelSource(
  source: FileSource,
  history: HistorySource,
): VersionsSource & Pick<FileSource, "details"> {
  return {
    details: (file, options) => source.details(file, options),
    restore: (object, version, paths, options) => source.restore(object, version, paths, options),
    history: (file, options) => history.history(file, options),
    versions: (object, options) => history.versions(object, options),
    historySize: (file, options) => history.historySize(file, options),
  };
}

/** Build the file panel with its File info tab and Versions view. */
function createPanel(options: FilePanelRouteOptions): FsFilePanel {
  const { labels } = options;
  const source = panelSource(options.source, options.history);
  const panel = document.createElement(PANEL_TAG) as FsFilePanel;
  panel.slot = "panel";
  panel.configure({ labels });
  panel.registerTab("details", labels.filePanelInfo, () => {
    const details = document.createElement("fs-details-panel") as FsDetailsPanel;
    details.configure({ labels, source });
    return details;
  });
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
 * @param options Labels, the file source and the history source.
 */
export function bindFilePanel(
  shell: HTMLElement,
  browser: FsFileBrowser,
  options: FilePanelRouteOptions,
): void {
  const panel = createPanel(options);
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
