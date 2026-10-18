/**
 * Map an authorisation denial (API-S-004) to the active label table, so every component shows
 * missing credentials, invalid credentials and not permitted in the distribution's words.
 */
import { ApiError, type DenialReason } from "../api/client";
import type { LabelKey, LabelTable } from "../theme/labels";

/** Label key for each denial reason the contract defines. */
const DENIAL_LABEL: Record<DenialReason, LabelKey> = {
  missing_credentials: "authMissingCredentials",
  invalid_credentials: "authInvalidCredentials",
  not_permitted: "authNotPermitted",
};

/** A denial reason with the words to show for it. */
export interface DenialText {
  /** The contract reason, for `data-reason` attributes and tests. */
  reason: DenialReason;
  /** The label from the active table. */
  text: string;
}

/**
 * Describe an error as an authorisation denial in the active labels. Pure.
 *
 * @param error Anything a client call rejected with.
 * @param labels The distribution's label table.
 * @returns The reason and its label, or undefined when `error` is not a denial.
 */
export function denialLabel(error: unknown, labels: LabelTable): DenialText | undefined {
  if (!(error instanceof ApiError) || !error.reason) return undefined;
  return { reason: error.reason, text: labels[DENIAL_LABEL[error.reason]] };
}
