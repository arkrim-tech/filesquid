/**
 * The non-content states a view can be in (UI-F2.1a): empty, loading, error, offline and
 * permission denied, each shown distinctly by `<fs-states>`, and the rule that turns a failed
 * load into one of the three failure states. Pure.
 */
import { ApiError, type DenialReason } from "../api/client";

/** A state `<fs-states>` shows instead of content. */
export type ViewState =
  | { readonly kind: "loading" }
  | { readonly kind: "empty"; readonly message?: string }
  | { readonly kind: "error" }
  | { readonly kind: "offline" }
  | { readonly kind: "denied"; readonly reason?: DenialReason };

/** One of the state kinds. */
export type ViewStateKind = ViewState["kind"];

/**
 * Decide how to show a failed load: permission denied for an authorisation denial (with its
 * reason, even while the browser is offline), then offline while the browser is offline,
 * otherwise error. Pure.
 *
 * @param error What the load rejected with.
 * @param online Whether the browser reports a network connection (`navigator.onLine`).
 * @returns The state to show.
 */
export function failureState(error: unknown, online: boolean): ViewState {
  if (error instanceof ApiError && error.reason) return { kind: "denied", reason: error.reason };
  if (!online) return { kind: "offline" };
  return { kind: "error" };
}
