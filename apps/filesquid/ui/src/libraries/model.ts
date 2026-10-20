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
/** Everything a rebuild from files found. */
export type RebuildReport = components["schemas"]["RebuildReport"];
/** One object a rebuild read. */
export type RebuiltObject = components["schemas"]["RebuiltObject"];
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

/** The statement the consent dialog shows, as the contract names it. */
export const REPAIR_STATEMENT_ID = "version_inventory_repair.v1";
/** Most objects one repair request may name (API-S-023). */
export const MAX_REPAIR_OBJECTS = 1000;
/** Most bytes one object identifier in a repair request may have. */
export const MAX_OBJECT_ID_BYTES = 1024;
/** Most bytes a library ID may have. */
export const MAX_LIBRARY_ID_BYTES = 128;
/** Most bytes a location ID may have. */
export const MAX_LOCATION_ID_BYTES = 128;

/** Hash of the library start page. */
export const LIBRARY_ROUTE = "#/library";

/** The pages of the library flow, as a hash names them. */
export type LibraryRoute =
  | { readonly kind: "start" }
  | { readonly kind: "library"; readonly id: string }
  | { readonly kind: "repair"; readonly id: string };

/** What the elements need from the library routes (the live one is `ClientLibrarySource`). */
export interface LibrarySource {
  /** Ask the host to let the owner choose a folder; answers its location ID or `cancelled`. */
  chooseLocation(purpose: ChoicePurpose): Promise<LocationChoice>;
  /** Create a library on the folder registered as `locationId`. */
  create(locationId: string): Promise<LibraryView>;
  /** Open the library in the folder registered as `locationId`. */
  open(locationId: string): Promise<LibraryView>;
  /** Copy a completed snapshot of a library into the folder registered as the destination. */
  copy(libraryId: string, destinationLocationId: string): Promise<SnapshotCopyReport>;
  /** Read every object of an open library from its files, writing nothing. */
  rebuild(libraryId: string): Promise<RebuildReport>;
  /** List the objects whose head lacks its version inventory. */
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

/** Decode one hash segment into a library ID, or undefined (a warning is logged) when invalid. */
function decodeLibraryId(segment: string): string | undefined {
  try {
    const id = decodeURIComponent(segment);
    return id === "" || byteLength(id) > MAX_LIBRARY_ID_BYTES ? undefined : id;
  } catch (error) {
    console.warn("FileSquid ignored a library route whose ID does not decode", error);
    return undefined;
  }
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
  const [segment, tail, ...rest] = hash.slice(LIBRARY_ROUTE.length + 1).split("/");
  if (segment === undefined || rest.length > 0) return undefined;
  if (tail !== undefined && tail !== "repair") return undefined;
  const id = decodeLibraryId(segment);
  if (id === undefined) return undefined;
  return tail === "repair" ? { kind: "repair", id } : { kind: "library", id };
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
