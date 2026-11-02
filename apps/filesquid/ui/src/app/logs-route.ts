/**
 * Hash routing of the Logs view (UI-F2.16; UI-S-021, UI-S-032): `#/logs` shows `<fs-logs>` in the
 * shell, `#/logs?file=<uuid>` and `#/logs?operation_id=<id>` (the **View in Logs** links of the
 * Provenance tab, UI-F2.15; `logsHash` and `parseLogsHash` in `routes.ts`) show it filtered to
 * that file or operation, and any other hash removes it. While an entry is selected its detail
 * (`<fs-log-entry>`) stands in the shell's `panel` slot, which widens to 360 px for it, and the
 * footer's two slots carry the view's sentences: the time zone line and the entry count.
 */
import type { FsLogEntry } from "../components/fs-log-entry";
import { LOG_ENTRY_CLOSE_EVENT } from "../components/fs-log-entry";
import {
  LOGS_FOOTER_EVENT,
  LOGS_SELECT_EVENT,
  type FsLogs,
  type LogsViewOptions,
} from "../components/fs-logs";
import type { JournalEntryView } from "../api/client";
import type { LogsLibrary } from "../logs/model";
import type { LogsSource } from "../logs/source";
import { parseLogsHash, type LogsFilter, type RouteHost } from "./routes";

/** What the Logs route needs. */
export interface LogsRouteOptions extends Omit<LogsViewOptions, "source"> {
  /** Reads the journal. */
  source: LogsSource;
}

const VIEW_TAG = "fs-logs";
const ENTRY_TAG = "fs-log-entry";

/** The slotted `<span>` of a shell slot, created on first use (the same one the Drive views use). */
function footerSlot(shell: HTMLElement, name: "summary" | "commit-state"): HTMLElement {
  const existing = shell.querySelector<HTMLElement>(`:scope > [slot="${name}"]`);
  if (existing) return existing;
  const span = document.createElement("span");
  span.slot = name;
  shell.append(span);
  return span;
}

/** Put the sentences of the footer. */
function showFooter(shell: HTMLElement, zone: string, count: string): void {
  footerSlot(shell, "summary").textContent = zone;
  footerSlot(shell, "commit-state").textContent = count;
}

/** Remove the detail panel, if any. */
function removeEntry(shell: HTMLElement): void {
  shell.querySelector(`:scope > ${ENTRY_TAG}`)?.remove();
}

/** Show the detail of an entry in the shell's panel slot, or remove it for null. */
function showEntry(
  shell: HTMLElement,
  options: LogsRouteOptions,
  entry: JournalEntryView | null,
  libraries: readonly LogsLibrary[],
): void {
  if (entry === null) return removeEntry(shell);
  let panel = shell.querySelector<FsLogEntry>(`:scope > ${ENTRY_TAG}`);
  if (!panel) {
    panel = document.createElement(ENTRY_TAG) as FsLogEntry;
    panel.slot = "panel";
    panel.configure({
      labels: options.labels,
      source: options.source,
      zone: options.zone,
      online: options.online,
    });
    shell.append(panel);
  }
  panel.libraries = libraries;
  panel.entry = entry;
}

/** Create the view, give it the address's filter before it reads, and add it to the shell. */
function createView(shell: HTMLElement, options: LogsRouteOptions, filter: LogsFilter): FsLogs {
  const view = document.createElement(VIEW_TAG) as FsLogs;
  view.configure(options);
  view.routeFilter = Object.keys(filter).length === 0 ? undefined : filter;
  shell.append(view);
  return view;
}

/** Mount the view for a Logs address, point it at the address's filter, or remove it. */
function applyLogsRoute(shell: HTMLElement, options: LogsRouteOptions, hash: string): void {
  const filter = parseLogsHash(hash);
  const existing = shell.querySelector<FsLogs>(`:scope > ${VIEW_TAG}`);
  if (filter === undefined) {
    existing?.remove();
    removeEntry(shell);
    if (existing) showFooter(shell, "", "");
    return;
  }
  if (existing) existing.routeFilter = Object.keys(filter).length === 0 ? undefined : filter;
  else createView(shell, options, filter);
}

/** What the view tells the shell: the selected entry and the footer sentences. */
function wireEvents(shell: HTMLElement, options: LogsRouteOptions): () => void {
  const onSelect = (event: Event) => {
    const detail = (
      event as CustomEvent<{ entry: JournalEntryView | null; libraries: LogsLibrary[] }>
    ).detail;
    showEntry(shell, options, detail.entry, detail.libraries);
  };
  const onFooter = (event: Event) => {
    const detail = (event as CustomEvent<{ zone: string; count: string }>).detail;
    showFooter(shell, detail.zone, detail.count);
  };
  const onClose = () => {
    shell.querySelector<FsLogs>(`:scope > ${VIEW_TAG}`)?.select(undefined);
  };
  shell.addEventListener(LOGS_SELECT_EVENT, onSelect);
  shell.addEventListener(LOGS_FOOTER_EVENT, onFooter);
  shell.addEventListener(LOG_ENTRY_CLOSE_EVENT, onClose);
  return () => {
    shell.removeEventListener(LOGS_SELECT_EVENT, onSelect);
    shell.removeEventListener(LOGS_FOOTER_EVENT, onFooter);
    shell.removeEventListener(LOG_ENTRY_CLOSE_EVENT, onClose);
  };
}

/**
 * Show the Logs view for `#/logs` and its filtered forms, now and on every `hashchange`. After a
 * `hashchange` (not on the first apply) focus moves to the shell's main area when it offers
 * `focusMain`. Side effect: adds window and shell listeners, mutates the shell's children and
 * moves focus.
 *
 * @param shell Element the view is slotted into (the app shell).
 * @param options Labels, the Logs source and the optional zone, clock and save.
 * @param win Window whose hash is followed.
 * @returns A function that removes the listeners.
 */
export function bindLogsRoute(
  shell: RouteHost,
  options: LogsRouteOptions,
  win: Window = window,
): () => void {
  const unwire = wireEvents(shell, options);
  applyLogsRoute(shell, options, win.location.hash);
  const onChange = () => {
    applyLogsRoute(shell, options, win.location.hash);
    shell.focusMain?.();
  };
  win.addEventListener("hashchange", onChange);
  return () => {
    win.removeEventListener("hashchange", onChange);
    unwire();
  };
}
