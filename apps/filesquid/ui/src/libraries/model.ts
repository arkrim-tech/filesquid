/**
 * The library flow's model (UI-1.1, API-S-023, API-S-025, API-S-034): the contract types the
 * flow uses, the hash routes of its pages, the {@link LibrarySource} interface the elements
 * talk to, and the consent request built from the repair candidates. A folder reaches the API
 * only as an opaque location ID the host registered; no type here carries a filesystem path.
 * Pure: no I/O.
 */
import type { components } from "../api/schema";

/** Why the owner is choosing a folder: to create, to open, or as the destination of a copy. */
export type ChoicePurpose = components["schemas"]["ChoicePurpose"];
/** The answer of the location-choices route: a chosen folder's location ID, or cancelled. */
export type LocationChoice = components["schemas"]["LocationChoice"];
/** A created or opened library. */
export type LibraryView = components["schemas"]["LibraryView"];
/** What a snapshot copy wrote and what its validation found. */
export type SnapshotCopyReport = components["schemas"]["SnapshotCopyReport"];
/** Everything a rebuild from files found: the result of its finished job. */
export type RebuildReport = components["schemas"]["RebuildReport"];
/** The answer of a rebuild command: its durable job, pending (API-S-025). */
export type RebuildJobAnswer = components["schemas"]["RebuildJobAnswer"];
/** One object a rebuild read. */
export type RebuiltObject = components["schemas"]["RebuiltObject"];
/** One object a rebuild could not open. */
export type UnreadableObject = components["schemas"]["UnreadableObject"];
/** What a rebuild restored of the library's journal. */
export type JournalRebuild = components["schemas"]["JournalRebuild"];
/** One copied object whose validation found errors. */
export type ObjectFindings = components["schemas"]["ObjectFindings"];
/** An object the UI may offer to repair. */
export type RepairCandidate = components["schemas"]["RepairCandidate"];
/** The repair-candidate query's answer. */
export type RepairCandidates = components["schemas"]["RepairCandidates"];
/** Body of the repair command, with the owner's consent. */
export type RepairRequest = components["schemas"]["RepairRequest"];
/** The answer to a repair command. */
export type RepairAnswer = components["schemas"]["RepairAnswer"];
/** One object's repair outcome. */
export type ObjectRepair = components["schemas"]["ObjectRepair"];

/** A library's facts (API-S-038): its name, the free bytes of its volume and its revision. */
export type LibraryFacts = components["schemas"]["LibraryFacts"];
/** Why a library's free space is unknown. */
export type FreeBytesReason = components["schemas"]["FreeBytesReason"];
/** One library this host opened, as the recent-libraries route lists it (API-S-038). */
export type RecentLibrary = components["schemas"]["RecentLibrary"];
/** The libraries this host opened that the caller may read: at most 20, newest first. */
export type RecentLibraries = components["schemas"]["RecentLibraries"];
/** One durable job as the job routes and the `job.update` event show it (API-S-037). */
export type JobView = components["schemas"]["JobView"];
/** The answer of a job control route. */
export type JobControlAnswer = components["schemas"]["JobControlAnswer"];

/**
 * The collection a create-collection command made. Not yet in the API contract: no route
 * creates a collection (see `ClientLibrarySource.createCollection`), so this is the shape the
 * UI asks the platform engineer to deliver, checked by `parseCollection` like any answer.
 */
export interface CollectionView {
  /** The new collection's ID, opaque. */
  readonly collection_id: string;
  /** Its title, as created. */
  readonly title: string;
}

/** The statement the consent dialog shows, as the contract names it. */
export const REPAIR_STATEMENT_ID = "version_inventory_repair.v2";
/**
 * The expected revision of a library that is not open on this host yet (API-S-019): what
 * create and open send (API-F2.19).
 */
export const NEW_LIBRARY_REVISION = "0";
/**
 * The revision a library is at once this host binds it (API-S-019). A copy or rebuild of a
 * library this page did not create or open expects the current revision read from its facts
 * (`LibrarySource.revision`); this constant is only the fallback when the facts answer `null`
 * for a library that is not open now, whose command the API then refuses as `library_not_open`.
 */
export const INITIAL_LIBRARY_REVISION = "1";
/** Most objects one repair request may name (API-S-023). */
export const MAX_REPAIR_OBJECTS = 1000;
/** Most bytes one object identifier in a repair request may have. */
export const MAX_OBJECT_ID_BYTES = 1024;
/** Most bytes a library ID may have. */
export const MAX_LIBRARY_ID_BYTES = 128;
/** Most bytes a location ID may have. */
export const MAX_LOCATION_ID_BYTES = 128;
/** Most bytes a library name may have (API-S-019, API-S-038). */
export const MAX_LIBRARY_NAME_BYTES = 1024;

/**
 * Whether a value is a library name the API accepts: 1 to 1,024 UTF-8 bytes, not blank, and
 * without control characters (API-S-019, API-S-038). Pure.
 *
 * @param value The candidate name.
 * @returns True for a valid name.
 */
export function isLibraryName(value: unknown): value is string {
  if (typeof value !== "string" || value.trim() === "") return false;
  if (new TextEncoder().encode(value).length > MAX_LIBRARY_NAME_BYTES) return false;
  // No C0 or C1 control character and no DEL.
  return ![...value].some((char) => {
    const code = char.codePointAt(0) ?? 0;
    return code < 0x20 || (code >= 0x7f && code <= 0x9f);
  });
}

/** Hash of the library start page. */
export const LIBRARY_ROUTE = "#/library";

/** Which flow a rebuild job page belongs to: Recover from a copy, or a library's own Rebuild. */
export type RebuildMode = "recover" | "rebuild";

/** Most bytes a job ID in a route may have (the library ID bound). */
export const MAX_JOB_ID_BYTES = 128;

/** The pages of the library flow, as a hash names them. */
export type LibraryRoute =
  | { readonly kind: "start" }
  | { readonly kind: "library"; readonly id: string }
  | { readonly kind: "repair"; readonly id: string }
  | {
      readonly kind: "job";
      readonly mode: RebuildMode;
      readonly id: string;
      readonly job: string;
    };

/**
 * What the elements need from the library routes (the live one is `ClientLibrarySource`).
 * Create, open, copy and rebuild are commands (API-F2.19, UI-S-012): each carries an
 * envelope with a fresh operation id and the expected revision, and a `conflict` outcome
 * is raised as the `ApiError` 409 the views word.
 */
export interface LibrarySource {
  /** Ask the host to let the owner choose a folder; answers its location ID or `cancelled`. */
  chooseLocation(purpose: ChoicePurpose): Promise<LocationChoice>;
  /**
   * Create a library on the folder registered as `locationId`, expecting revision `0`, with the
   * name the owner gave (API-S-038); without one the host names it "Untitled library".
   */
  create(locationId: string, name?: string): Promise<LibraryView>;
  /**
   * Create a collection in an open library (the first-use flow, UI-S-034), expecting the
   * library's `expectedRevision`; answers the collection it made.
   */
  createCollection(
    libraryId: string,
    title: string,
    expectedRevision: string,
  ): Promise<CollectionView>;
  /**
   * The libraries this host opened that the caller may read, newest first, at most 20
   * (API-S-038). A host without a library registry answers 503, which the page treats as "no
   * recent libraries and no first use".
   */
  recent(): Promise<RecentLibraries>;
  /** One library's facts: name, free space and revision (API-S-038). */
  facts(libraryId: string): Promise<LibraryFacts>;
  /** Open the library in the folder registered as `locationId`, expecting revision `0`. */
  open(locationId: string): Promise<LibraryView>;
  /**
   * Copy a completed snapshot of a library into the folder registered as the destination,
   * expecting the library's `revision` as last read.
   */
  copy(
    libraryId: string,
    destinationLocationId: string,
    expectedRevision: string,
  ): Promise<SnapshotCopyReport>;
  /**
   * Start the rebuild of an open library from its files as a durable job, expecting the
   * library's `revision` as last read; answers the pending job, never a report.
   */
  rebuild(libraryId: string, expectedRevision: string): Promise<RebuildJobAnswer>;
  /**
   * The current revision of a library this host opened, read from its facts (API-S-038), for
   * a page that did not create or open it; the initial revision when it is not open now.
   */
  revision(libraryId: string): Promise<string>;
  /** List the objects whose head lacks its version inventory or its sidecar. */
  repairCandidates(libraryId: string): Promise<RepairCandidates>;
  /** Run the consented version-inventory repair. */
  repair(request: RepairRequest): Promise<RepairAnswer>;
}

/**
 * The address of a library's page. Pure.
 *
 * @param libraryId The library ID, escaped for the hash.
 * @returns `#/library/<id>`.
 */
export function libraryHref(libraryId: string): string {
  return `${LIBRARY_ROUTE}/${encodeURIComponent(libraryId)}`;
}

/**
 * The address of the page that follows a recovered copy's rebuild job (mockup 04). Pure.
 *
 * @param libraryId The library ID, escaped for the hash.
 * @param jobId The rebuild job's ID, escaped for the hash.
 * @returns `#/library/<id>/recover/<job>`.
 */
export function libraryRecoverHref(libraryId: string, jobId: string): string {
  return `${libraryHref(libraryId)}/recover/${encodeURIComponent(jobId)}`;
}

/**
 * The address of the page that follows a library's own rebuild job. Pure.
 *
 * @param libraryId The library ID, escaped for the hash.
 * @param jobId The rebuild job's ID, escaped for the hash.
 * @returns `#/library/<id>/rebuild/<job>`.
 */
export function libraryRebuildHref(libraryId: string, jobId: string): string {
  return `${libraryHref(libraryId)}/rebuild/${encodeURIComponent(jobId)}`;
}

/**
 * The address of a library's repair offer, the one the Monitor's W010 note links to
 * (`bindMonitorRoute`'s `repairHref`). Pure.
 *
 * @param libraryId The library ID, escaped for the hash.
 * @returns `#/library/<id>/repair`.
 */
export function libraryRepairHref(libraryId: string): string {
  return `${libraryHref(libraryId)}/repair`;
}

/** UTF-8 length of a string in bytes. */
function byteLength(value: string): number {
  return new TextEncoder().encode(value).length;
}

/** Decode one hash segment into an ID of at most `limit` bytes, or undefined when invalid. */
function decodeId(segment: string, limit: number): string | undefined {
  try {
    const id = decodeURIComponent(segment);
    return id === "" || byteLength(id) > limit ? undefined : id;
  } catch (error) {
    console.warn("FileSquid ignored a library route whose ID does not decode", error);
    return undefined;
  }
}

/** Read `<tail>/<job>` of `#/library/<id>/<tail>/<job>` as a rebuild job page, or undefined. */
function parseJobRoute(
  id: string,
  tail: string,
  job: string | undefined,
): LibraryRoute | undefined {
  if (tail !== "recover" && tail !== "rebuild") return undefined;
  if (job === undefined) return undefined;
  const jobId = decodeId(job, MAX_JOB_ID_BYTES);
  return jobId === undefined ? undefined : { kind: "job", mode: tail, id, job: jobId };
}

/**
 * Read a location hash as a library page. Pure apart from a console warning for an ID whose
 * escapes do not decode.
 *
 * @param hash A location hash.
 * @returns The page it names, or undefined for any other hash, an empty ID, an ID over 128
 *   bytes, or an unknown sub-route.
 */
export function parseLibraryHash(hash: string): LibraryRoute | undefined {
  if (hash === LIBRARY_ROUTE) return { kind: "start" };
  if (!hash.startsWith(`${LIBRARY_ROUTE}/`)) return undefined;
  const [segment, tail, job, ...rest] = hash.slice(LIBRARY_ROUTE.length + 1).split("/");
  if (segment === undefined || rest.length > 0) return undefined;
  const id = decodeId(segment, MAX_LIBRARY_ID_BYTES);
  if (id === undefined) return undefined;
  if (tail === undefined) return { kind: "library", id };
  if (tail === "repair" && job === undefined) return { kind: "repair", id };
  return parseJobRoute(id, tail, job);
}

/**
 * Build the repair request for the candidates the consent dialog named: the same distinct
 * object IDs in the request and in the consent, with the statement shown and `confirmed`. Pure.
 *
 * @param libraryId The open library.
 * @param candidates The objects the dialog listed.
 * @returns The request, or undefined when it would name no object, more than 1,000, or an ID
 *   over 1,024 bytes (the API would refuse it).
 */
export function consentRequest(
  libraryId: string,
  candidates: readonly RepairCandidate[],
): RepairRequest | undefined {
  const ids = [...new Set(candidates.map((candidate) => candidate.object_id))];
  if (ids.length === 0 || ids.length > MAX_REPAIR_OBJECTS) return undefined;
  if (ids.some((id) => byteLength(id) > MAX_OBJECT_ID_BYTES)) return undefined;
  return {
    library_id: libraryId,
    object_ids: ids,
    consent: {
      library_id: libraryId,
      object_ids: [...ids],
      statement_id: REPAIR_STATEMENT_ID,
      confirmed: true,
    },
  };
}
