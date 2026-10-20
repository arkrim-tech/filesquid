/**
 * Checks of the library routes' answers against the contract (UI-1.1, API-S-025, API-S-034): a
 * host that answers outside the contract is reported as a `TypeError` and never shown, as the
 * navigation source does. Each parser returns the value typed from the generated schema, so a
 * field the contract adds is a compile error here until the parser reads it. Pure.
 */
import {
  MAX_LOCATION_ID_BYTES,
  type LibraryView,
  type LocationChoice,
  type ObjectRepair,
  type RebuildReport,
  type RepairAnswer,
  type RepairCandidates,
  type SnapshotCopyReport,
} from "./model";

/** True for a non-null, non-array object. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** True for a non-negative safe integer. */
function isCount(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

/** True for a non-empty string. */
function isText(value: unknown): value is string {
  return typeof value === "string" && value !== "";
}

/** True for an array whose every item is a non-empty string. */
function isTexts(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(isText);
}

/** The error for an answer outside the contract; it names the route, never the answer. */
function breaks(what: string): TypeError {
  return new TypeError(`FileSquid ${what} answer does not satisfy the contract`);
}

/**
 * Check the answer of the location-choices route.
 *
 * @param value The parsed JSON answer.
 * @returns A `chosen` answer with a location ID of at most 128 bytes, or `cancelled`.
 * @throws TypeError when the answer is anything else.
 */
export function parseLocationChoice(value: unknown): LocationChoice {
  if (!isRecord(value)) throw breaks("location choice");
  if (value.outcome === "cancelled") return { outcome: "cancelled" };
  const id = value.location_id;
  const sized = isText(id) && new TextEncoder().encode(id).length <= MAX_LOCATION_ID_BYTES;
  if (value.outcome !== "chosen" || !sized) throw breaks("location choice");
  return { outcome: "chosen", location_id: id };
}

/**
 * Check a created or opened library.
 *
 * @param value The parsed JSON answer.
 * @returns The library.
 * @throws TypeError when a field is missing or of the wrong type.
 */
export function parseLibraryView(value: unknown): LibraryView {
  if (!isRecord(value)) throw breaks("library");
  const { library_id: id, spec_version: version, objects } = value;
  if (!isText(id) || !isText(version) || !isCount(objects)) throw breaks("library");
  return { library_id: id, spec_version: version, objects };
}

/**
 * Check a snapshot copy's report.
 *
 * @param value The parsed JSON answer.
 * @returns The report.
 * @throws TypeError when a count is missing or an invalid object has no codes.
 */
export function parseSnapshotCopyReport(value: unknown): SnapshotCopyReport {
  if (!isRecord(value)) throw breaks("snapshot copy");
  const { objects, files, bytes, invalid_objects: invalid } = value;
  if (!isCount(objects) || !isCount(files) || !isCount(bytes) || !Array.isArray(invalid)) {
    throw breaks("snapshot copy");
  }
  const ok = invalid.every(
    (item) => isRecord(item) && isText(item.object_id) && isTexts(item.error_codes),
  );
  if (!ok) throw breaks("snapshot copy");
  return {
    objects,
    files,
    bytes,
    invalid_objects: invalid as SnapshotCopyReport["invalid_objects"],
  };
}

/** True when `value` is one object a rebuild read. */
function isRebuiltObject(value: unknown): boolean {
  return (
    isRecord(value) &&
    isText(value.object_id) &&
    isText(value.head) &&
    isCount(value.files) &&
    isText(value.digest_algorithm) &&
    isTexts(value.error_codes) &&
    typeof value.missing_version_inventory === "boolean"
  );
}

/**
 * Check a rebuild report.
 *
 * @param value The parsed JSON answer.
 * @returns The report.
 * @throws TypeError when an object or an unreadable entry is malformed.
 */
export function parseRebuildReport(value: unknown): RebuildReport {
  if (!isRecord(value) || !Array.isArray(value.objects) || !Array.isArray(value.unreadable)) {
    throw breaks("rebuild");
  }
  const unreadable = value.unreadable.every((item) => isRecord(item) && isText(item.error_code));
  if (!value.objects.every(isRebuiltObject) || !unreadable) throw breaks("rebuild");
  return value as unknown as RebuildReport;
}

/** True when `value` is one repair candidate. */
function isCandidate(value: unknown): boolean {
  return (
    isRecord(value) &&
    isText(value.object_id) &&
    isText(value.head) &&
    isText(value.digest_algorithm) &&
    typeof value.interrupted === "boolean" &&
    typeof value.committable_after_repair === "boolean"
  );
}

/**
 * Check the repair-candidate list.
 *
 * @param value The parsed JSON answer.
 * @returns The candidates and whether more exist.
 * @throws TypeError when a candidate is malformed.
 */
export function parseRepairCandidates(value: unknown): RepairCandidates {
  if (!isRecord(value) || !Array.isArray(value.candidates)) throw breaks("repair candidates");
  if (!value.candidates.every(isCandidate) || typeof value.truncated !== "boolean") {
    throw breaks("repair candidates");
  }
  return value as unknown as RepairCandidates;
}

/** True when `value` is one object's repair outcome. */
function isObjectRepair(value: unknown): value is ObjectRepair {
  if (!isRecord(value)) return false;
  const result = value.result === "repaired" || value.result === "refused";
  const code = value.error_code === null || isText(value.error_code);
  const version = value.version === null || isText(value.version);
  return (
    isText(value.object_id) && result && code && version && typeof value.retryable === "boolean"
  );
}

/**
 * Check a repair command's answer.
 *
 * @param value The parsed JSON answer.
 * @returns The answer, with one outcome per object.
 * @throws TypeError when an outcome is malformed.
 */
export function parseRepairAnswer(value: unknown): RepairAnswer {
  if (!isRecord(value) || !isText(value.library_id) || !isText(value.run_id)) {
    throw breaks("repair");
  }
  if (!Array.isArray(value.objects) || !value.objects.every(isObjectRepair)) throw breaks("repair");
  return value as unknown as RepairAnswer;
}
