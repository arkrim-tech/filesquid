/**
 * Start the delivery of the UI's console warnings and errors (UI-F2.16; UI-S-023): open the
 * page's IndexedDB store, build the {@link ClientEventQueue} over `FileSquidClient.postClientEvents`,
 * look for events an earlier page left (they are delivered first), capture the console and
 * persist what is undelivered when the page closes (`pagehide`).
 */
import type { FileSquidClient } from "../api/client";
import { installConsoleCapture, queueLog } from "./console";
import { ClientEventQueue } from "./queue";
import { IndexedDbEventStore } from "./store";

/** The part of `FileSquidClient` delivery needs. */
export type ClientEventsClient = Pick<FileSquidClient, "postClientEvents">;

/** What delivery needs from the page. */
export interface ClientEventsOptions {
  /** The window whose `indexedDB` and `pagehide` are used. */
  readonly win: Window;
  /** The console to capture and report to; the page's by default. */
  readonly console?: Console;
  /** The API client. */
  readonly client: ClientEventsClient;
  /** The host the events are raised against: the API base URL of the page. */
  readonly host: string;
}

/** A running delivery. */
export interface ClientEvents {
  /** The queue, for diagnostics and for {@link ClientEvents.credentialChanged}. */
  readonly queue: ClientEventQueue;
  /** The page's credential changed (a token was typed in): see `ClientEventQueue.credentialChanged`. */
  credentialChanged(): void;
  /** Stop capturing and delivering and release the listeners. */
  stop(): void;
}

/** The page's IndexedDB, or undefined when reading it throws (storage blocked by the browser). */
function pageIndexedDb(win: Window, log: (message: string, error?: unknown) => void) {
  try {
    return win.indexedDB;
  } catch (error) {
    log("IndexedDB is blocked on this page", error);
    return undefined;
  }
}

/**
 * Start delivering client events. Side effect: opens IndexedDB, wraps the console, adds a
 * `pagehide` listener and may send requests.
 *
 * @param options The window, the client and the host.
 * @returns The running delivery. The store may be unavailable (a private window): the queue then
 *   keeps events in memory and counts the ones it could not keep (UI-S-023).
 */
export async function startClientEvents(options: ClientEventsOptions): Promise<ClientEvents> {
  const { win, client, host } = options;
  const log = queueLog(options.console ?? console);
  const store = await IndexedDbEventStore.open(pageIndexedDb(win, log));
  const queue = new ClientEventQueue({
    transport: (request, sendOptions) => client.postClientEvents(request, sendOptions),
    store,
    host,
    log,
  });
  await queue.start();
  const restore = installConsoleCapture((input) => queue.record(input), options.console ?? console);
  const onPageHide = () => queue.pagehide();
  win.addEventListener("pagehide", onPageHide);
  // A plain marker for tests and diagnostics: the console is captured from here on.
  win.document.documentElement.dataset["clientEvents"] = "started";
  return {
    queue,
    credentialChanged: () => queue.credentialChanged(),
    stop: () => {
      restore();
      delete win.document.documentElement.dataset["clientEvents"];
      win.removeEventListener("pagehide", onPageHide);
      queue.stop();
    },
  };
}
