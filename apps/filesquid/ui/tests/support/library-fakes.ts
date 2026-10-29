/**
 * Test double for the library flow (UI-1.1): a {@link LibrarySource} that records every call
 * and answers from values, errors or functions the test sets, shaped on the generated contract
 * types so a drift in the contract breaks these tests at compile time.
 */
import { ApiError } from "../../src/api/client";
import type {
  LibrarySource,
  LibraryView,
  LocationChoice,
  RebuildJobAnswer,
  RebuildReport,
  RepairAnswer,
  RepairCandidate,
  RepairCandidates,
  RepairRequest,
  SnapshotCopyReport,
} from "../../src/index";

/** A scripted answer: a value, an error to reject with, or a function producing either. */
export type Answer<T> = T | Error | (() => T | Promise<T>);

/** A library of three objects. */
export const LIBRARY: LibraryView = {
  library_id: "lib-1",
  spec_version: "1.1",
  objects: 3,
  revision: "1",
};

/** One object whose head lacks its version inventory, and one interrupted sha512 object. */
export const CANDIDATES: RepairCandidates = {
  candidates: [
    {
      object_id: "urn:uuid:obj-a",
      head: "v2",
      digest_algorithm: "sha256",
      interrupted: false,
      committable_after_repair: true,
    },
    {
      object_id: "urn:uuid:obj-b",
      head: "v1",
      digest_algorithm: "sha512",
      interrupted: true,
      committable_after_repair: false,
    },
  ],
  truncated: false,
};

/** The first candidate, for tests that build more of them. */
export const FIRST_CANDIDATE = CANDIDATES.candidates[0] as RepairCandidate;

/** A rebuild that read two objects, one flagged for repair and one with errors. */
export const REBUILD: RebuildReport = {
  objects: [
    {
      object_id: "urn:uuid:obj-a",
      head: "v2",
      files: 4,
      digest_algorithm: "sha256",
      error_codes: [],
      missing_version_inventory: true,
    },
    {
      object_id: "urn:uuid:obj-c",
      head: "v1",
      files: 1,
      digest_algorithm: "sha256",
      error_codes: ["E023"],
      missing_version_inventory: false,
    },
  ],
  unreadable: [{ error_code: "E001" }],
};

/** The pending job a rebuild command answers (API-F2.19, API-S-025). */
export const REBUILD_JOB: RebuildJobAnswer = {
  outcome: "pending",
  job_id: "job-1",
  job: {
    id: "job-1",
    kind: "library.rebuild",
    operation_id: "op-rebuild-1",
    state: "pending",
    paused: false,
    done: 0,
    attempts: 0,
    created: "2026-10-29T12:00:00Z",
    updated: "2026-10-29T12:00:00Z",
    result: null,
  },
  scheduled: true,
};

/** A copy that wrote two objects, one with validation errors. */
export const COPY_REPORT: SnapshotCopyReport = {
  objects: 2,
  files: 5,
  bytes: 2048,
  invalid_objects: [{ object_id: "urn:uuid:obj-c", error_codes: ["E023", "E040"] }],
};

/**
 * An API error as the client raises it for a failed route.
 *
 * @param status HTTP status.
 * @param code Contract error code.
 * @param reason Denial reason of a 401 or 403.
 * @returns The error.
 */
export function apiError(
  status: number,
  code: NonNullable<ConstructorParameters<typeof ApiError>[2]>["code"],
  reason?: "missing_credentials" | "invalid_credentials" | "not_permitted",
): ApiError {
  return new ApiError(status, "/api/v1/libraries", { code, retryable: status === 503, reason });
}

/** Answer an {@link Answer}: reject with an error, call a function, or resolve a value. */
async function answer<T>(value: Answer<T>): Promise<T> {
  if (value instanceof Error) throw value;
  if (typeof value !== "function") return value;
  const produced = await (value as () => T | Error | Promise<T | Error>)();
  if (produced instanceof Error) throw produced;
  return produced;
}

/** A recording, scriptable {@link LibrarySource}. */
export class FakeLibrarySource implements LibrarySource {
  /** Every call as `name:argument`, in order, for example `choose:create` or `create:loc-1`. */
  readonly calls: string[] = [];
  /** Every repair request, as sent. */
  readonly repairs: RepairRequest[] = [];
  /** The expected revision of every copy and rebuild, in order. */
  readonly revisions: string[] = [];
  choice: Answer<LocationChoice> = { outcome: "chosen", location_id: "loc-1" };
  created: Answer<LibraryView> = LIBRARY;
  opened: Answer<LibraryView> = LIBRARY;
  copied: Answer<SnapshotCopyReport> = COPY_REPORT;
  rebuilt: Answer<RebuildJobAnswer> = REBUILD_JOB;
  candidates: Answer<RepairCandidates> = CANDIDATES;
  repaired: Answer<RepairAnswer> = {
    library_id: "lib-1",
    run_id: "run-1",
    objects: [
      {
        object_id: "urn:uuid:obj-a",
        version: "v2",
        result: "repaired",
        error_code: null,
        retryable: false,
      },
      {
        object_id: "urn:uuid:obj-b",
        version: "v1",
        result: "refused",
        error_code: "nothing_to_repair",
        retryable: false,
      },
    ],
  };

  /** @inheritdoc */
  chooseLocation(purpose: "create" | "open" | "copy_destination"): Promise<LocationChoice> {
    this.calls.push(`choose:${purpose}`);
    return answer(this.choice);
  }
  /** @inheritdoc */
  create(locationId: string): Promise<LibraryView> {
    this.calls.push(`create:${locationId}`);
    return answer(this.created);
  }
  /** @inheritdoc */
  open(locationId: string): Promise<LibraryView> {
    this.calls.push(`open:${locationId}`);
    return answer(this.opened);
  }
  /** @inheritdoc */
  copy(
    libraryId: string,
    destinationLocationId: string,
    expectedRevision: string,
  ): Promise<SnapshotCopyReport> {
    this.calls.push(`copy:${libraryId}:${destinationLocationId}`);
    this.revisions.push(expectedRevision);
    return answer(this.copied);
  }
  /** @inheritdoc */
  rebuild(libraryId: string, expectedRevision: string): Promise<RebuildJobAnswer> {
    this.calls.push(`rebuild:${libraryId}`);
    this.revisions.push(expectedRevision);
    return answer(this.rebuilt);
  }
  /** @inheritdoc */
  repairCandidates(libraryId: string): Promise<RepairCandidates> {
    this.calls.push(`candidates:${libraryId}`);
    return answer(this.candidates);
  }
  /** @inheritdoc */
  repair(request: RepairRequest): Promise<RepairAnswer> {
    this.calls.push(`repair:${request.library_id}`);
    this.repairs.push(request);
    return answer(this.repaired);
  }
}
