/**
 * Build a {@link FileSquidClient} for the host this page runs in, with the adapter chosen by
 * `selectHostAdapter` (UI-S-012). Inside the desktop app, requests go through the custom URI
 * scheme, events arrive as Tauri events named "filesquid-event", and the bearer token is the
 * per-launch token the desktop host defines for its own pages. On `filesquid-server`,
 * requests are same-origin HTTP and events use the WebSocket channel.
 */
import { listen } from "@tauri-apps/api/event";
import {
  FileSquidClient,
  detectHost,
  selectHostAdapter,
  type ApiEvent,
  type EventListener,
} from "./client";

/**
 * Window global in which `filesquid-desktop` places its per-launch token, only for the app's
 * own page origins (read-only, not enumerable). Never defined on `filesquid-server`.
 */
export const LAUNCH_TOKEN_GLOBAL = "__FILESQUID_LAUNCH_TOKEN__";

/** Tauri event name used by `filesquid-desktop` to forward API events. */
export const DESKTOP_EVENT = "filesquid-event";

/**
 * Subscribe to desktop events through Tauri. Side effect: registers a Tauri listener.
 *
 * @param listener Called once per event.
 * @returns Unsubscribe function.
 */
function subscribeDesktop(listener: EventListener): () => void {
  const pending = listen<ApiEvent>(DESKTOP_EVENT, (event) => listener(event.payload));
  return () => {
    pending.then((unlisten) => unlisten()).catch((error) => console.error(error));
  };
}

/**
 * Read the desktop per-launch token from a window. Pure read; the value is never copied
 * anywhere else by this module.
 *
 * @param win Window the UI runs in.
 * @returns The token, or undefined outside the desktop app's own pages.
 */
export function launchToken(win: Window): string | undefined {
  const value: unknown = (win as unknown as Record<string, unknown>)[LAUNCH_TOKEN_GLOBAL];
  return typeof value === "string" ? value : undefined;
}

/**
 * Create the client for the current window. The token source prefers the desktop launch
 * token and otherwise asks `token`, so the same build works on both hosts.
 *
 * @param win Window the UI runs in.
 * @param token Supplies the server host's token (for example from `memoryTokenStore`);
 *   not needed on the desktop host.
 * @returns A client for this host.
 */
export function createHostClient(
  win: Window = window,
  token?: () => string | undefined,
): FileSquidClient {
  const adapter = selectHostAdapter(detectHost(win));
  return new FileSquidClient({
    baseUrl: adapter.baseUrl,
    subscribeHost: adapter.events === "tauri" ? subscribeDesktop : undefined,
    token: () => launchToken(win) ?? token?.(),
  });
}
