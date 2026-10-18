/**
 * Hash routing shared by every distribution until the full navigation model (FS-2.1):
 * `#/monitor` shows the Monitor dashboard inside the shell; any other hash shows the shell
 * alone. Also provides the in-memory token store used on the server host.
 */
import type { MonitorOptions, TokenStore } from "../components/fs-monitor";

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
 * every `hashchange`. Side effect: adds a window listener and mutates the shell's children.
 *
 * @param shell Element the Monitor is slotted into (the app shell).
 * @param options Monitor configuration.
 * @param win Window whose hash is followed.
 * @returns A function that removes the listener.
 */
export function bindMonitorRoute(
  shell: HTMLElement,
  options: MonitorOptions,
  win: Window = window,
): () => void {
  const apply = () => applyMonitorRoute(shell, options, win.location.hash);
  apply();
  win.addEventListener("hashchange", apply);
  return () => win.removeEventListener("hashchange", apply);
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
