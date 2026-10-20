/**
 * The non-content states a view can be in (UI-F2.1a): empty, loading, error, offline,
 * permission denied and unavailable (UI-F2.1b: the host answered 503 because it has not
 * installed or started the service yet), each shown distinctly by `<fs-states>`, and the rule
 * that turns a failed load into one of the four failure states. Pure.
 */
import { ApiError, type DenialReason } from "../api/client";

/** HTTP status of an answer from a host that has not installed or started a service. */
const HTTP_UNAVAILABLE = 503;

/** A state `<fs-states>` shows instead of content. */
export type ViewState =
  | { readonly kind: "loading" }
  | { readonly kind: "empty"; readonly message?: string }
  | { readonly kind: "error" }
  | { readonly kind: "offline" }
  | { readonly kind: "denied"; readonly reason?: DenialReason }
  | { readonly kind: "unavailable" };

/** One of the state kinds. */
export type ViewStateKind = ViewState["kind"];

/**
 * Decide how to show a failed load: permission denied for an authorisation denial (with its
 * reason, even while the browser is offline), unavailable for an HTTP 503 answer (the host
 * answered, so the browser is not offline), then offline while the browser is offline,
 * otherwise error. Pure.
 *
 * @param error What the load rejected with.
 * @param online Whether the browser reports a network connection (`navigator.onLine`).
 * @returns The state to show.
 */
export function failureState(error: unknown, online: boolean): ViewState {
  if (error instanceof ApiError && error.reason) return { kind: "denied", reason: error.reason };
  if (error instanceof ApiError && error.status === HTTP_UNAVAILABLE)
    return { kind: "unavailable" };
  if (!online) return { kind: "offline" };
  return { kind: "error" };
}
