/**
 * Mounts the upload flow in the app shell (UI-1.2, UI-S-028): the Jobs button and panel in the
 * header's `jobs` slot, the upload dialog that the New menu's upload items open, and the footer's
 * "Uploading · {done} of {total}" status while a job runs. On the desktop host the New menu's
 * upload items are enabled (that host shows its own file dialog); New folder stays off until its
 * own task (UI-F2.17). A started upload opens the Jobs panel on its job, and the panel follows
 * `job.update` events from then on.
 *
 * The one thing the API does not yet give is the collection ID of a navigation node, so the
 * destination directory is a port: pass a lookup when the host can name collections, and without
 * one the dialog says so honestly before sending anything (reported to the platform engineer).
 */
import type { FsImportDialog } from "../components/fs-import-dialog";
import { IMPORT_STARTED_EVENT } from "../components/fs-import-dialog";
import type { FsJobsPanel } from "../components/fs-jobs-panel";
import type { FsNewMenu } from "../components/fs-new-menu";
import { HOST_EVENT } from "../components/fs-host-indicator";
import { ClientImportSource, type ImportClient } from "../imports/client-source";
import { NamedDestinations, type DestinationLookup } from "../imports/directory";
import { ClientJobSource, type JobClient } from "../imports/jobs-source";
import { JobsStore } from "../imports/jobs-store";
import type { DestinationDirectory } from "../imports/model";
import type { NavigationSource } from "../navigation/model";
import { formatLabel, type LabelTable } from "../theme/labels";
import type { RouteHost } from "./routes";

/** The part of the API client the upload flow needs. */
export type UploadClient = ImportClient & JobClient;

/** What the upload flow needs. */
export interface ImportRouteOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** The API client. */
  readonly client: UploadClient;
  /** The navigation listing the destination tree reads. */
  readonly navigation: NavigationSource;
  /** The New menu whose upload items open the dialog. */
  readonly newMenu?: FsNewMenu;
  /** Names the collection of a chosen node; none until the API supplies the mapping. */
  readonly lookup?: DestinationLookup;
}

/** The elements and state the flow mounted. */
export interface MountedImports {
  readonly dialog: FsImportDialog;
  readonly panel: FsJobsPanel;
  readonly store: JobsStore;
  readonly directory: DestinationDirectory;
}

/** The footer text of the running jobs, or undefined when none runs. */
function footerText(store: JobsStore, labels: LabelTable): string | undefined {
  const progress = store.progress;
  if (progress === undefined) return undefined;
  const lang = document.documentElement.lang || "en";
  return formatLabel(labels.jobsFooter, {
    done: progress.done.toLocaleString(lang),
    total: progress.total.toLocaleString(lang),
  });
}

/**
 * Enable the New menu's upload items on a host that shows its own file dialog (the desktop). The
 * host badge announces the host type (`fs-host`) when it reads the descriptor and when it
 * changes, so the flow reads nothing of its own and cannot add a second read of the host.
 */
function enableUploads(shell: HTMLElement, menu: FsNewMenu | undefined): void {
  if (!menu) return;
  menu.folders = false;
  shell.addEventListener(HOST_EVENT, (event) => {
    const hostType = (event as CustomEvent<{ hostType?: string }>).detail?.hostType;
    menu.uploads = hostType === "desktop";
  });
}

/**
 * Mount the upload dialog, the Jobs panel and the footer status in the shell.
 *
 * @param shell The app shell; `uploadStatus` shows the footer text.
 * @param options Labels, the client, the navigation source, the New menu and the lookup.
 * @returns The mounted elements and the store, for tests and later tasks.
 */
export function mountImports(
  shell: RouteHost & { uploadStatus?: string },
  options: ImportRouteOptions,
): MountedImports {
  const { labels, client } = options;
  const directory = new NamedDestinations(options.lookup);
  const store = new JobsStore({ source: new ClientJobSource(client) });
  const dialog = document.createElement("fs-import-dialog") as FsImportDialog;
  dialog.configure({
    labels,
    source: new ClientImportSource(client),
    navigation: options.navigation,
    directory,
  });
  const panel = document.createElement("fs-jobs-panel") as FsJobsPanel;
  panel.slot = "jobs";
  panel.configure({ labels, store, directory });
  shell.append(dialog, panel);
  store.addEventListener("change", () => {
    shell.uploadStatus = footerText(store, labels);
  });
  dialog.addEventListener(IMPORT_STARTED_EVENT, (event) => {
    const jobId = (event as CustomEvent<{ jobId: string }>).detail.jobId;
    store
      .track(jobId)
      .then(() => panel.open())
      .catch((error: unknown) => {
        console.warn("FileSquid could not read the upload job it started", error);
      });
  });
  enableUploads(shell, options.newMenu);
  return { dialog, panel, store, directory };
}
