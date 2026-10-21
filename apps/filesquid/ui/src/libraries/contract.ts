/**
 * Checks of the library routes' answers against the contract (UI-1.1, API-S-025, API-S-034): a
 * host that answers outside the contract is reported as a `TypeError` and never shown, as the
 * navigation source does. Each parser builds its result from the fields it checked, typed from
 * the generated schema, so a required field the contract adds is a compile error here until the
 * parser reads it, and a field outside the contract is never passed on. Pure.
 */
import {
  MAX_LOCATION_ID_BYTES,
  MAX_REPAIR_OBJECTS,
  type JournalRebuild,
  type LibraryView,
  type LocationChoice,
  type ObjectFindings,
  type ObjectRepair,
  type RebuildReport,
  type RebuiltObject,
  type RepairAnswer,
  type RepairCandidate,
  type RepairCandidates,
  type SnapshotCopyReport,
  type UnreadableObject,
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

/** True for a non-empty array whose every item is a non-empty string. */
function isTexts(value: unknown): value is string[] {
  return Array.isArray(value) && value.length > 0 && value.every(isText);
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

/** One copied object whose validation found errors, or undefined when malformed. */
function objectFindings(value: unknown): ObjectFindings | undefined {
  if (!isRecord(value) || !isText(value.object_id) || !isTexts(value.error_codes)) return undefined;
  return { object_id: value.object_id, error_codes: [...value.error_codes] };
}

/**
 * Check every item of a list and build it again from its checked fields.
 *
 * @param items The list from the answer.
 * @param read Builds one item, or answers undefined when it is malformed.
 * @param what The route, for the error.
 * @returns The rebuilt items.
 * @throws TypeError when an item is malformed.
 */
function each<T>(items: unknown[], read: (item: unknown) => T | undefined, what: string): T[] {
  return items.map((item) => {
    const checked = read(item);
    if (checked === undefined) throw breaks(what);
    return checked;
  });
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
  return { objects, files, bytes, invalid_objects: each(invalid, objectFindings, "snapshot copy") };
}

/** True for an array (possibly empty) of non-empty strings. */
function isTextList(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(isText);
}

/** One object a rebuild read, or undefined when malformed. */
function rebuiltObject(value: unknown): RebuiltObject | undefined {
  if (!isRecord(value)) return undefined;
  const { object_id: id, head, files, digest_algorithm: algorithm, error_codes: codes } = value;
  const missing = value.missing_version_inventory;
  const ok = isText(id) && isText(head) && isCount(files) && isText(algorithm);
  if (!ok || !isTextList(codes) || typeof missing !== "boolean") return undefined;
  return {
    object_id: id,
    head,
    files,
    digest_algorithm: algorithm,
    error_codes: [...codes],
    missing_version_inventory: missing,
  };
}

/** One object a rebuild could not open, or undefined when malformed. */
function unreadableObject(value: unknown): UnreadableObject | undefined {
  return isRecord(value) && isText(value.error_code) ? { error_code: value.error_code } : undefined;
}

/** What a rebuild restored of the journal, or undefined when malformed. */
function journalRebuild(value: unknown): JournalRebuild | undefined {
  if (!isRecord(value)) return undefined;
  const { restored, already_present: present, conflicting, unreadable } = value;
  const sealed = value.last_sealed_at;
  if (!isCount(restored) || !isCount(present) || !isCount(conflicting)) return undefined;
  if (!isCount(unreadable) || !isOptionalText(sealed)) return undefined;
  const journal: JournalRebuild = {
    restored,
    already_present: present,
    conflicting,
    unreadable,
  };
  return sealed === undefined ? journal : { ...journal, last_sealed_at: sealed };
}

/**
 * Check a rebuild report.
 *
 * @param value The parsed JSON answer.
 * @returns The report, with its journal restore when the host made one.
 * @throws TypeError when an object, an unreadable entry or the journal restore is malformed.
 */
export function parseRebuildReport(value: unknown): RebuildReport {
  if (!isRecord(value) || !Array.isArray(value.objects) || !Array.isArray(value.unreadable)) {
    throw breaks("rebuild");
  }
  const report: RebuildReport = {
    objects: each(value.objects, rebuiltObject, "rebuild"),
    unreadable: each(value.unreadable, unreadableObject, "rebuild"),
  };
  if (value.journal === undefined) return report;
  const journal = journalRebuild(value.journal);
  if (journal === undefined) throw breaks("rebuild");
  return { ...report, journal };
}

/** One repair candidate, or undefined when malformed. */
function candidate(value: unknown): RepairCandidate | undefined {
  if (!isRecord(value)) return undefined;
  const { object_id: id, head, digest_algorithm: algorithm, interrupted } = value;
  const committable = value.committable_after_repair;
  const ok = isText(id) && isText(head) && isText(algorithm);
  if (!ok || typeof interrupted !== "boolean" || typeof committable !== "boolean") {
    return undefined;
  }
  return {
    object_id: id,
    head,
    digest_algorithm: algorithm,
    interrupted,
    committable_after_repair: committable,
  };
}

/**
 * Check the repair-candidate list.
 *
 * @param value The parsed JSON answer.
 * @returns The candidates and whether more exist.
 * @throws TypeError when a candidate is malformed or more than 1,000 are listed.
 */
export function parseRepairCandidates(value: unknown): RepairCandidates {
  if (!isRecord(value) || !Array.isArray(value.candidates)) throw breaks("repair candidates");
  const bounded = value.candidates.length <= MAX_REPAIR_OBJECTS;
  if (!bounded || typeof value.truncated !== "boolean") throw breaks("repair candidates");
  return {
    candidates: each(value.candidates, candidate, "repair candidates"),
    truncated: value.truncated,
  };
}

/** True for null or a non-empty string. */
function isNullableText(value: unknown): value is string | null {
  return value === null || isText(value);
}

/** True for an absent field or a non-empty string. */
function isOptionalText(value: unknown): value is string | undefined {
  return value === undefined || isText(value);
}

/** One object's repair outcome, or undefined when malformed. */
function objectRepair(value: unknown): ObjectRepair | undefined {
  if (!isRecord(value)) return undefined;
  const { object_id: id, version, result, error_code: code, retryable } = value;
  const { provenance_version: provenance, provenance_error_code: provenanceCode } = value;
  const known = result === "repaired" || result === "refused";
  const ok = isText(id) && known && isNullableText(version) && isNullableText(code);
  const optional = isOptionalText(provenance) && isOptionalText(provenanceCode);
  if (!ok || typeof retryable !== "boolean" || !optional) return undefined;
  const outcome: ObjectRepair = { object_id: id, version, result, error_code: code, retryable };
  if (provenance !== undefined) outcome.provenance_version = provenance;
  if (provenanceCode !== undefined) outcome.provenance_error_code = provenanceCode;
  return outcome;
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
  if (!Array.isArray(value.objects)) throw breaks("repair");
  return {
    library_id: value.library_id,
    run_id: value.run_id,
    objects: each(value.objects, objectRepair, "repair"),
  };
}
