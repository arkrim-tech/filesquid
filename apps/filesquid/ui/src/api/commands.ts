/**
 * Pure helpers for FileSquid commands and byte reads (UI-S-012, CORE-S-005).
 *
 * A command (any mutation) carries a versioned envelope with an operation id, the revision the
 * caller last read and its permitted scope. The operation id is the idempotency key: a retry
 * reuses it, so the API never applies the same command twice. A command answers with an
 * outcome tied to that id: `committed` (durable completion evidence), `pending` (durable but
 * incomplete), `conflict` (the expected revision did not match) or `error` (a module
 * `PortError`). UI state is never durable completion evidence; only `committed` is.
 *
 * Everything here is pure except {@link newOperationId}, which reads the platform's secure
 * random source.
 */
import type { CompleteList } from "./complete-list";
import type { components } from "./schema";

/** Versioned envelope of every command, generated from the API contract. */
export type CommandEnvelope = components["schemas"]["CommandEnvelope"];
/** Result of a command, generated from the API contract. */
export type CommandOutcome = components["schemas"]["CommandOutcome"];
/** The four outcome kinds, generated from the API contract. */
export type CommandOutcomeKind = components["schemas"]["CommandOutcomeKind"];
/** Module error carried by an `error` outcome, generated from the API contract. */
export type PortError = components["schemas"]["PortError"];

/** What a caller supplies to build a {@link CommandEnvelope}. */
export interface EnvelopeInput {
  /** Permitted scope, for example `library:<id>`. */
  scope: string;
  /** Revision the caller last read; a mismatch answers `conflict`. */
  expectedRevision: string;
  /** Operation id to reuse when retrying; a new one is generated when omitted. */
  operationId?: string;
}

/** An inclusive byte range; `end` omitted means "to the end of the file". */
export interface ByteRange {
  /** First byte, zero-based. */
  start: number;
  /** Last byte, inclusive. */
  end?: number;
}

/** A parsed `Content-Range` answer. */
export interface ContentRange {
  /** First byte returned. */
  start: number;
  /** Last byte returned, inclusive. */
  end: number;
  /** Complete length of the representation, when the server knows it. */
  size?: number;
}

/** Literal outcome kinds; checked against the contract by {@link OUTCOME_KINDS}. */
const OUTCOME_KIND_VALUES = [
  "committed",
  "pending",
  "conflict",
  "error",
] as const satisfies readonly CommandOutcomeKind[];
/** Every outcome kind, typed so `tsc` fails when the contract adds one. */
const OUTCOME_KINDS: CompleteList<CommandOutcomeKind, typeof OUTCOME_KIND_VALUES> =
  OUTCOME_KIND_VALUES;
/** `bytes <start>-<end>/<size or *>` (RFC 9110 section 14.4). */
const CONTENT_RANGE = /^bytes (\d+)-(\d+)\/(\d+|\*)$/;

/**
 * A new operation id: a version 4 UUID from the platform's secure random source.
 * Side effect: reads randomness.
 *
 * @returns A fresh id, unique per command.
 */
export function newOperationId(): string {
  return globalThis.crypto.randomUUID();
}

/**
 * Throw when a contract string field is empty.
 *
 * @param field Contract field name, used in the message.
 * @param value Value to check.
 * @throws RangeError when `value` is empty.
 */
function requireText(field: string, value: string): void {
  if (value === "") throw new RangeError(`${field} must not be empty`);
}

/**
 * Build the envelope of one command. Pure apart from generating an id when none is given.
 *
 * @param input Scope, expected revision and, for a retry, the original operation id.
 * @returns A schema version 1 envelope.
 * @throws RangeError when a field is empty (the contract requires at least one character).
 */
export function commandEnvelope(input: EnvelopeInput): CommandEnvelope {
  const operationId = input.operationId ?? newOperationId();
  requireText("scope", input.scope);
  requireText("expected_revision", input.expectedRevision);
  requireText("operation_id", operationId);
  return {
    schema_version: 1,
    operation_id: operationId,
    expected_revision: input.expectedRevision,
    scope: input.scope,
  };
}

/**
 * Read a `PortError` from parsed JSON. Pure.
 *
 * @param value Candidate value.
 * @returns The error with contract fields only, or undefined when it is not a `PortError`.
 */
function portError(value: unknown): PortError | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const { code, retryable } = value as Record<string, unknown>;
  if (typeof code !== "string" || code === "" || typeof retryable !== "boolean") return undefined;
  return { code, retryable };
}

/**
 * Validate a command answer against the contract and the operation that was sent. Pure.
 *
 * @param value Parsed JSON of a 2xx command answer.
 * @param operationId The id the command was sent with.
 * @returns The outcome with contract fields only.
 * @throws TypeError when the answer is not a contract `CommandOutcome`, or is for another
 *   operation. A caller MUST NOT treat such an answer as completion.
 */
export function commandOutcome(value: unknown, operationId: string): CommandOutcome {
  if (typeof value !== "object" || value === null) {
    throw new TypeError("command answer is not a CommandOutcome");
  }
  const record = value as Record<string, unknown>;
  const kind = OUTCOME_KINDS.find((known) => known === record.kind);
  if (kind === undefined) throw new TypeError("command answer has an unknown kind");
  const error = portError(record.error);
  const errorMatches = kind === "error" ? error !== undefined : record.error === null;
  if (!errorMatches) throw new TypeError("command answer error does not match its kind");
  if (record.operation_id !== operationId) {
    throw new TypeError("command outcome is for another operation");
  }
  return { operation_id: operationId, kind, error: error ?? null };
}

/**
 * The outcome for a command the API refused with HTTP 409 `conflict`. Pure.
 *
 * @param operationId The id the command was sent with.
 * @returns A `conflict` outcome; the caller rereads and decides again.
 */
export function conflictOutcome(operationId: string): CommandOutcome {
  return { operation_id: operationId, kind: "conflict", error: null };
}

/**
 * True only for durable completion evidence. Pure.
 *
 * @param outcome A command outcome.
 * @returns Whether the command committed; `pending` is not completion.
 */
export function isCommitted(outcome: CommandOutcome): boolean {
  return outcome.kind === "committed";
}

/**
 * True for a whole, non-negative byte offset the platform can represent exactly. Pure.
 *
 * @param value Candidate offset.
 */
function isOffset(value: number): boolean {
  return Number.isSafeInteger(value) && value >= 0;
}

/**
 * The `Range` header value for an inclusive byte range (RFC 9110 section 14.2). Pure.
 *
 * @param range First and, optionally, last byte.
 * @returns `bytes=<start>-<end>` or `bytes=<start>-`.
 * @throws RangeError when an offset is negative, fractional or unsafe, or `end` < `start`.
 */
export function rangeHeader(range: ByteRange): string {
  const { start, end } = range;
  if (!isOffset(start)) throw new RangeError("range start must be a whole number >= 0");
  if (end === undefined) return `bytes=${start}-`;
  if (!isOffset(end) || end < start)
    throw new RangeError("range end must be a whole number >= start");
  return `bytes=${start}-${end}`;
}

/**
 * Parse a `Content-Range` header of a 206 answer. Pure.
 *
 * @param value The header value, or null when absent.
 * @returns Start, end and size, or undefined when the value is absent or not a satisfied
 *   byte range.
 */
export function parseContentRange(value: string | null): ContentRange | undefined {
  const match = value === null ? null : CONTENT_RANGE.exec(value);
  if (!match) return undefined;
  const [, start, end, size] = match;
  const range = { start: Number(start), end: Number(end) };
  if (range.end < range.start) return undefined;
  return { ...range, size: size === "*" ? undefined : Number(size) };
}
