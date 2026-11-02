/**
 * How the library flow shows a failure (UI-1.1, API-S-034, API-S-025): a 503 `unavailable` is
 * the unavailable state (with its reason when the folder chooser answered it), an authorisation
 * denial is the denied state, and a conflict, a missing location or library and an invalid
 * request are messages in the active label table that say nothing was changed. Any other status
 * answer is a generic message, and a failure with no answer is offline or an error. Pure.
 */
import { ApiError } from "../api/client";
import type { ViewState } from "../components/view-state";
import type { LabelKey, LabelTable } from "../theme/labels";

/** The step that failed. */
export type LibraryAction =
  "choose" | "create" | "collection" | "open" | "copy" | "rebuild" | "candidates" | "repair";

/** What to show: a state for `<fs-states>`, or a sentence for the alert line. */
export type Failure =
  | { readonly kind: "state"; readonly state: ViewState }
  | { readonly kind: "message"; readonly text: string };

const HTTP_BAD_REQUEST = 400;
const HTTP_FORBIDDEN = 403;
const HTTP_NOT_FOUND = 404;
const HTTP_CONFLICT = 409;
const HTTP_UNAVAILABLE = 503;

/** The sentence for a 409, per step. */
const CONFLICT: Record<LibraryAction, LabelKey> = {
  choose: "libraryConflictChoice",
  create: "libraryConflictCreate",
  collection: "libraryFailed",
  open: "libraryConflictOpen",
  copy: "libraryConflictCopy",
  rebuild: "libraryFailed",
  candidates: "libraryFailed",
  repair: "repairRequestRefusedConflict",
};

/** The sentence for a missing location or library, per step. */
function notFoundKey(action: LibraryAction): LabelKey {
  return action === "repair" ? "repairRequestRefusedGone" : "libraryNotFound";
}

/** The sentence for any other refusal, per step. */
function otherKey(action: LibraryAction): LabelKey {
  return action === "repair" ? "repairRequestRefused" : "libraryFailed";
}

/** A message failure. */
function message(labels: LabelTable, key: LabelKey): Failure {
  return { kind: "message", text: labels[key] };
}

/** The failure for a 503 answer. */
function unavailable(action: LibraryAction, labels: LabelTable): Failure {
  if (action === "repair") return message(labels, "repairRequestRefusedUnavailable");
  const state: ViewState =
    action === "choose"
      ? { kind: "unavailable", reason: labels.libraryChooserUnavailable }
      : { kind: "unavailable" };
  return { kind: "state", state };
}

/** The failure for an answer the API gave with a status. */
function refusal(error: ApiError, action: LibraryAction, labels: LabelTable): Failure {
  if (error.reason) {
    if (action === "repair" && error.reason === "not_permitted") {
      return message(labels, "repairRequestRefusedOwner");
    }
    return { kind: "state", state: { kind: "denied", reason: error.reason } };
  }
  if (error.status === HTTP_UNAVAILABLE) return unavailable(action, labels);
  if (error.status === HTTP_CONFLICT) return message(labels, CONFLICT[action]);
  if (error.status === HTTP_NOT_FOUND) return message(labels, notFoundKey(action));
  if (error.status === HTTP_BAD_REQUEST) return message(labels, "libraryInvalid");
  if (error.status === HTTP_FORBIDDEN && action === "repair") {
    return message(labels, "repairRequestRefusedOwner");
  }
  return message(labels, otherKey(action));
}

/**
 * Decide how to show a failed step of the library flow. Pure.
 *
 * @param error What the step rejected with.
 * @param action The step that failed.
 * @param labels The active label table.
 * @param online Whether the browser reports a network connection.
 * @returns A state or a sentence; never the server's free-text message.
 */
export function libraryFailure(
  error: unknown,
  action: LibraryAction,
  labels: LabelTable,
  online: boolean,
): Failure {
  if (error instanceof ApiError && error.status > 0) return refusal(error, action, labels);
  if (error instanceof ApiError) {
    const reason = error.reason ?? "invalid_credentials";
    return { kind: "state", state: { kind: "denied", reason } };
  }
  return { kind: "state", state: { kind: online ? "error" : "offline" } };
}
