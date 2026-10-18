/**
 * Hash routing shared by every distribution until the full navigation model (FS-2.1):
 * `#/monitor` shows the Monitor dashboard inside the shell; any other hash shows the shell
 * alone. After a route change focus moves to the shell's main area (UI-S-016), so keyboard and
 * screen-reader users land on the new content instead of on a link that may have been removed.
 * Also provides the in-memory token store used on the server host.
 */
import type { MonitorOptions, TokenStore } from "../components/fs-monitor";

/** An element the routed views are mounted in; the app shell also offers `focusMain`. */
export type RouteHost = HTMLElement & {
  /** Move focus to the host's main area (`<fs-app-shell>` implements it). */
  focusMain?: () => void;
};

/** Hash of the Monitor route. */
export const MONITOR_ROUTE = "#/monitor";

/**
 * Keep a token typed in at run time (the server host's administrator token) in memory only:
 * no localStorage, no sessionStorage, no URL. It is gone when the page reloads or closes.
 *
 * An empty value (for example a blank form field) is not a token: `get` then returns
 * undefined, so the client sends no credential and the API answers `missing_credentials`.
 *
 * @param onSet Called with no arguments after a token is set, for example to reload the
 *   shell's API status; it never receives the token.
 * @returns A token store for `<fs-monitor>`; give the client its getter as the `token`
 *   function (`() => store.get()`), not the store itself.
 */
export function memoryTokenStore(onSet?: () => void): TokenStore {
  let token: string | undefined;
  return {
    get: () => (token === "" ? undefined : token),
    set(value: string) {
      token = value;
      onSet?.();
    },
  };
}

/**
 * Show or hide an `<fs-monitor>` child of `shell` to match the current hash, now and on
 * every `hashchange`. After a `hashchange` (not on the first apply) focus moves to the shell's
 * main area when it offers `focusMain`. Side effect: adds a window listener, mutates the shell's
 * children and moves focus.
 *
 * @param shell Element the Monitor is slotted into (the app shell).
 * @param options Monitor configuration.
 * @param win Window whose hash is followed.
 * @returns A function that removes the listener.
 */
export function bindMonitorRoute(
  shell: RouteHost,
  options: MonitorOptions,
  win: Window = window,
): () => void {
  applyMonitorRoute(shell, options, win.location.hash);
  const onChange = () => {
    applyMonitorRoute(shell, options, win.location.hash);
    shell.focusMain?.();
  };
  win.addEventListener("hashchange", onChange);
  return () => win.removeEventListener("hashchange", onChange);
}

/** Mount the Monitor when `hash` is the Monitor route, otherwise remove it. */
function applyMonitorRoute(shell: HTMLElement, options: MonitorOptions, hash: string): void {
  const existing = shell.querySelector("fs-monitor");
  if (hash !== MONITOR_ROUTE) {
    existing?.remove();
    return;
  }
  if (existing) return;
  const monitor = document.createElement("fs-monitor") as HTMLElement & {
    configure(options: MonitorOptions): void;
  };
  monitor.configure(options);
  shell.append(monitor);
}
