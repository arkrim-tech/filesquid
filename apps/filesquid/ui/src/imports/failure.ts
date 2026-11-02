/**
 * How the upload flow words a refusal (UI-1.2, UI-S-029, mockup 48). Three sources, none of them
 * a sub-code the contract does not carry (API-F2.21 adds `destination_required`,
 * `collection_not_found` and `path_suffix_exhausted` to `ApiError` later; UI-F2.17 refines this):
 *
 * - the UI's own pre-send check, a missing destination or one this host cannot name;
 * - the HTTP status of the answer and the step that failed (a 404 of the plan request means the
 *   chosen collection is gone, a 404 of a plan route means the plan is gone);
 * - a file's own `error_code` (`path_suffix_exhausted` is one, API-S-036).
 *
 * The server's free-text message is never shown. Pure.
 */
import { ApiError } from "../api/client";
import { formatLabel, type LabelKey, type LabelTable } from "../theme/labels";
import { pathSuffixRange, type JobItem } from "./model";

/** The step of the flow that failed. */
export type ImportStep = "plan" | "items" | "start" | "discard" | "job";

/** Raised before anything is sent when no destination collection was chosen (ADR 041). */
export class DestinationRequiredError extends Error {
  constructor() {
    super("FileSquid needs a destination collection before an upload");
    this.name = "DestinationRequiredError";
  }
}

/** Raised before anything is sent when this host cannot name the chosen collection yet. */
export class DestinationUnresolvedError extends Error {
  constructor() {
    super("FileSquid cannot name the chosen collection on this host yet");
    this.name = "DestinationUnresolvedError";
  }
}

/** A refusal shown as an error card: its title and its reason (mockup 48). */
export interface FileRefusal {
  /** `Not uploaded: {name}`. */
  readonly title: string;
  /** The reason, ending with what to do. */
  readonly reason: string;
}

const HTTP_BAD_REQUEST = 400;
const HTTP_NOT_FOUND = 404;
const HTTP_CONFLICT = 409;
const HTTP_UNAVAILABLE = 503;

/** The wording of a denial reason. */
const DENIAL: Record<NonNullable<ApiError["reason"]>, LabelKey> = {
  missing_credentials: "authMissingCredentials",
  invalid_credentials: "authInvalidCredentials",
  not_permitted: "authNotPermitted",
};

/** The wording of a 404, by step. */
function notFound(step: ImportStep): LabelKey {
  if (step === "plan") return "fileCollectionNotFound";
  return step === "job" ? "jobsRefused" : "importPlanGone";
}

/** The wording of a 409, by step. */
function conflict(step: ImportStep): LabelKey {
  if (step === "plan") return "importDialogOpen";
  return step === "start" ? "importSourceChanged" : "jobsRefused";
}

/** The wording of any other answer, by status and step. */
function byStatus(error: ApiError, step: ImportStep): LabelKey {
  if (error.status === HTTP_UNAVAILABLE) {
    return step === "job" ? "jobsUnavailable" : "importUnavailable";
  }
  if (error.status === HTTP_NOT_FOUND) return notFound(step);
  if (error.status === HTTP_CONFLICT) return conflict(step);
  if (error.status === HTTP_BAD_REQUEST && step === "plan") return "importInvalid";
  return step === "job" ? "jobsRefused" : "importFailed";
}

/**
 * The sentence for a failed step. Pure.
 *
 * @param error What the step rejected with.
 * @param step The step that failed.
 * @param labels The active label table.
 * @param online Whether the browser reports a network connection.
 * @returns A sentence from the label table, never the server's message or a path.
 */
export function importFailure(
  error: unknown,
  step: ImportStep,
  labels: LabelTable,
  online: boolean,
): string {
  if (error instanceof DestinationRequiredError) return labels.fileDestinationRequired;
  if (error instanceof DestinationUnresolvedError) return labels.importDestinationUnresolved;
  if (error instanceof ApiError && error.reason) return labels[DENIAL[error.reason]];
  if (error instanceof ApiError && error.status > 0) return labels[byStatus(error, step)];
  return online ? labels.importFailed : labels.importOffline;
}

/**
 * The error card of a failed file whose code has wording of its own (mockup 48). Pure.
 *
 * @param item One file of a job.
 * @param labels The active label table.
 * @returns The title and reason for `path_suffix_exhausted`, otherwise undefined: the file's
 *   status line already shows its code.
 */
export function fileRefusal(item: JobItem, labels: LabelTable): FileRefusal | undefined {
  if (item.state !== "failed" || item.error_code !== "path_suffix_exhausted") return undefined;
  const { first, last } = pathSuffixRange(item.name);
  return {
    title: formatLabel(labels.fileUploadFailed, { name: item.name }),
    reason: formatLabel(labels.fileSuffixExhausted, { first, last }),
  };
}
