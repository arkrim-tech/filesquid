/**
 * Checks of the import and job routes' answers against the contract (UI-1.2, API-S-036,
 * API-S-037): a host that answers outside the contract is reported as a `TypeError` and never
 * shown, as the library and navigation sources do. Each parser builds its result from the fields
 * it checked, typed from the generated schema, so a required field the contract adds is a
 * compile error here until the parser reads it, and a field outside the contract (a checkpoint, a
 * lease holder, a path) is never passed on. Pure.
 */
import type {
  ImportPlan,
  ImportSourceKind,
  JobItem,
  JobItemPage,
  JobItemState,
  JobPage,
  JobState,
  JobView,
  PlanAnswer,
  PlanItem,
  PlanItemPage,
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

/** The error for an answer outside the contract; it names the route, never the answer. */
function breaks(what: string): TypeError {
  return new TypeError(`FileSquid ${what} answer does not satisfy the contract`);
}

/** The values of a contract enum, checked against a candidate. */
function oneOf<T extends string>(values: readonly T[], value: unknown): T | undefined {
  return values.find((known) => known === value);
}

const SOURCES = ["files", "folder"] as const satisfies readonly ImportSourceKind[];
const JOB_STATES = [
  "pending",
  "running",
  "completed",
  "failed",
  "cancelled",
] as const satisfies readonly JobState[];
const ITEM_STATES = [
  "queued",
  "copying",
  "copied",
  "skipped",
  "failed",
] as const satisfies readonly JobItemState[];

/**
 * The optional text field of a record: its value, or undefined when it is absent. The API leaves
 * an absent optional field out (never `null`), so an explicit `null` or another type breaks the
 * contract.
 */
function optionalText(
  record: Record<string, unknown>,
  key: string,
  what: string,
): string | undefined {
  const value = record[key];
  if (value === undefined) return undefined;
  if (typeof value !== "string") throw breaks(what);
  return value;
}

/** Check the plan of an answer. */
function parsePlan(value: unknown): ImportPlan {
  if (!isRecord(value) || !isRecord(value.destination)) throw breaks("import plan");
  const source = oneOf(SOURCES, value.source);
  const counts = [
    value.files_found,
    value.files_new,
    value.files_present,
    value.bytes_to_copy,
    value.names_numbered,
    value.staging_limit_bytes,
  ];
  const free = value.free_bytes_after;
  const freeOk = free === null || isCount(free);
  const shaped = isText(value.plan_id) && source !== undefined && counts.every(isCount);
  if (!shaped || !freeOk || !isText(value.expires) || !isText(value.destination.collection_id)) {
    throw breaks("import plan");
  }
  const folder = optionalText(value.destination, "folder", "import plan");
  const volume = optionalText(value, "source_volume", "import plan");
  return {
    plan_id: value.plan_id as string,
    source,
    destination: { collection_id: value.destination.collection_id, ...(folder ? { folder } : {}) },
    ...(volume ? { source_volume: volume } : {}),
    files_found: value.files_found as number,
    files_new: value.files_new as number,
    files_present: value.files_present as number,
    bytes_to_copy: value.bytes_to_copy as number,
    names_numbered: value.names_numbered as number,
    free_bytes_after: free as number | null,
    staging_limit_bytes: value.staging_limit_bytes as number,
    expires: value.expires,
  };
}

/**
 * Check the answer of an import request or start.
 *
 * @param value The parsed JSON answer.
 * @returns A plan to review, a started job, or that the owner closed the dialog.
 * @throws TypeError when the answer is anything else, or names no plan or job it promises.
 */
export function parseImportAnswer(value: unknown): PlanAnswer {
  if (!isRecord(value) || !isRecord(value.outcome)) throw breaks("import");
  const kind = value.outcome.kind;
  const named = isText(value.outcome.operation_id) && value.outcome.error === null;
  // A planned or started import is durable and not complete (`pending`); a closed dialog
  // changed nothing (`committed`).
  const expected = value.import === "cancelled" ? "committed" : "pending";
  if (!named || kind !== expected) throw breaks("import");
  if (value.import === "cancelled") return { kind: "cancelled" };
  if (value.import === "started" && isText(value.job_id)) {
    return { kind: "started", jobId: value.job_id };
  }
  if (value.import === "planned" && isText(value.plan_id) && value.plan !== undefined) {
    return { kind: "planned", plan: parsePlan(value.plan) };
  }
  throw breaks("import");
}

/** Check one planned file. */
function parsePlanItem(value: unknown): PlanItem {
  if (!isRecord(value)) throw breaks("import plan item");
  const { item_id: id, name, bytes, present, numbered } = value;
  const shaped = isText(id) && isText(name) && isCount(bytes);
  if (!shaped || typeof present !== "boolean" || typeof numbered !== "boolean") {
    throw breaks("import plan item");
  }
  return { item_id: id, name, bytes, present, numbered };
}

/** The cursor of a page: a string, or null on the last page. */
function parseCursor(value: unknown, what: string): string | null {
  if (value === null) return null;
  if (typeof value !== "string") throw breaks(what);
  return value;
}

/**
 * Check one page of a plan's files.
 *
 * @param value The parsed JSON answer.
 * @returns The page.
 * @throws TypeError when a file or the cursor is malformed.
 */
export function parsePlanItemPage(value: unknown): PlanItemPage {
  if (!isRecord(value) || !Array.isArray(value.items)) throw breaks("import plan items");
  return {
    items: value.items.map(parsePlanItem),
    next_cursor: parseCursor(value.next_cursor, "import plan items"),
  };
}

/** Check one file of a job. */
function parseJobItem(value: unknown): JobItem {
  if (!isRecord(value)) throw breaks("job item");
  const state = oneOf(ITEM_STATES, value.state);
  const shaped = isText(value.item_id) && isText(value.name) && state !== undefined;
  if (!shaped || !isCount(value.bytes_done) || !isCount(value.bytes_total)) {
    throw breaks("job item");
  }
  const folder = optionalText(value, "folder", "job item");
  const code = optionalText(value, "error_code", "job item");
  return {
    item_id: value.item_id as string,
    name: value.name as string,
    ...(folder ? { folder } : {}),
    state,
    bytes_done: value.bytes_done,
    bytes_total: value.bytes_total,
    ...(code ? { error_code: code } : {}),
  };
}

/**
 * Check one page of a job's files.
 *
 * @param value The parsed JSON answer.
 * @returns The page.
 * @throws TypeError when a file or the cursor is malformed.
 */
export function parseJobItemPage(value: unknown): JobItemPage {
  if (!isRecord(value) || !Array.isArray(value.items)) throw breaks("job items");
  return {
    items: value.items.map(parseJobItem),
    next_cursor: parseCursor(value.next_cursor, "job items"),
  };
}

/** The job's optional fields, each absent or well formed: a malformed one breaks the contract. */
function jobOptionals(value: Record<string, unknown>): Partial<JobView> {
  const out: { -readonly [K in keyof JobView]?: JobView[K] } = {};
  if (value.destination !== undefined) {
    const destination = value.destination;
    if (!isRecord(destination) || !isText(destination.collection_id)) throw breaks("job");
    const folder = optionalText(destination, "folder", "job");
    out.destination = {
      collection_id: destination.collection_id,
      ...(folder === undefined ? {} : { folder }),
    };
  }
  if (value.control !== undefined) {
    if (value.control !== "pause" && value.control !== "cancel") throw breaks("job");
    out.control = value.control;
  }
  if (value.total !== undefined) {
    if (!isCount(value.total)) throw breaks("job");
    out.total = value.total;
  }
  const code = optionalText(value, "error_code", "job");
  if (code !== undefined) out.error_code = code;
  return out;
}

/**
 * Check one job view.
 *
 * @param value The parsed JSON of a job.
 * @returns The job with contract fields only.
 * @throws TypeError when a required field is missing or of the wrong type.
 */
export function parseJobView(value: unknown): JobView {
  if (!isRecord(value)) throw breaks("job");
  const state = oneOf(JOB_STATES, value.state);
  const named = isText(value.id) && isText(value.kind) && isText(value.operation_id);
  const timed = isText(value.created) && isText(value.updated);
  const counted = isCount(value.done) && isCount(value.attempts);
  if (!named || !timed || !counted || state === undefined || typeof value.paused !== "boolean") {
    throw breaks("job");
  }
  return {
    id: value.id as string,
    kind: value.kind as string,
    operation_id: value.operation_id as string,
    state,
    paused: value.paused,
    done: value.done as number,
    attempts: value.attempts as number,
    created: value.created as string,
    updated: value.updated as string,
    result: null,
    ...jobOptionals(value),
  };
}

/**
 * Check the listing of jobs.
 *
 * @param value The parsed JSON answer.
 * @returns The jobs, newest first, and whether more matched.
 * @throws TypeError when a job or the flag is malformed.
 */
export function parseJobPage(value: unknown): JobPage {
  if (!isRecord(value) || !Array.isArray(value.jobs) || typeof value.truncated !== "boolean") {
    throw breaks("job list");
  }
  return { jobs: value.jobs.map(parseJobView), truncated: value.truncated };
}

/**
 * Check the answer of a job control and return the job.
 *
 * @param value The parsed JSON answer.
 * @returns The job after the control.
 * @throws TypeError when the answer carries no valid job.
 */
export function parseJobControlAnswer(value: unknown): JobView {
  if (!isRecord(value) || typeof value.scheduled !== "boolean") throw breaks("job control");
  return parseJobView(value.job);
}
