/**
 * The live {@link LibrarySource} (UI-1.1, UI-S-012, API-S-025, API-S-034): every call goes
 * through `FileSquidClient` to a route of the API contract with the contract's body, and every
 * answer is checked against the contract before it is returned. A folder is only ever the
 * opaque location ID the host registered when the owner chose it; the source never builds,
 * receives or logs a filesystem path. API refusals (`conflict`, `not_found`, 503 `unavailable`,
 * denials) pass through unchanged as `ApiError` for the views to word.
 */
import type { RequestOptions } from "../api/client";
import {
  parseLibraryView,
  parseLocationChoice,
  parseRebuildReport,
  parseRepairAnswer,
  parseRepairCandidates,
  parseSnapshotCopyReport,
} from "./contract";
import type {
  ChoicePurpose,
  LibrarySource,
  LibraryView,
  LocationChoice,
  RebuildReport,
  RepairAnswer,
  RepairCandidates,
  RepairRequest,
  SnapshotCopyReport,
} from "./model";

/** The part of the API client the source needs (injectable in tests). */
export interface LibraryClient {
  /** Send a JSON request and parse the JSON answer. */
  request<T>(path: string, options?: RequestOptions): Promise<T>;
}

/** The routes of the library flow, as the contract names them. */
const ROUTES = {
  choose: "/api/v1/libraries/location-choices",
  create: "/api/v1/libraries",
  open: "/api/v1/libraries/openings",
  copy: "/api/v1/libraries/snapshot-copies",
  rebuild: "/api/v1/libraries/rebuilds",
  candidates: "/api/v1/libraries/repair-candidates",
  repair: "/api/v1/libraries/repairs",
} as const;

/** A library source over the FileSquid API. */
export class ClientLibrarySource implements LibrarySource {
  /** @param client The API client (`FileSquidClient`, or a fake in tests). */
  constructor(private readonly client: LibraryClient) {}

  /**
   * Ask the host to let the owner choose a folder (`POST /api/v1/libraries/location-choices`).
   * On the desktop this shows the native folder dialog and waits for the owner. Network I/O.
   *
   * @param purpose `create`, `open` or `copy_destination`.
   * @returns `chosen` with the folder's opaque location ID, or `cancelled`.
   * @throws ApiError as the API answers it (409 a dialog is already open or the folder cannot
   *   hold a library, 503 `unavailable` on a host without a chooser, denials).
   * @throws TypeError when the answer breaks the contract (a body that is not JSON surfaces from
   *   the client as a SyntaxError; the pages show both as an error).
   */
  async chooseLocation(purpose: ChoicePurpose): Promise<LocationChoice> {
    return parseLocationChoice(await this.post(ROUTES.choose, { purpose }));
  }

  /**
   * Create a library on a chosen folder (`POST /api/v1/libraries`). The API makes the library
   * ID; none is sent. Network I/O that writes the storage root's declaration.
   *
   * @param locationId The folder's opaque location ID.
   * @returns The created library.
   * @throws ApiError as the API answers it (409 root_not_empty, location_in_use, and so on).
   * @throws TypeError when the answer breaks the contract (a body that is not JSON surfaces from
   *   the client as a SyntaxError; the pages show both as an error).
   */
  async create(locationId: string): Promise<LibraryView> {
    return parseLibraryView(await this.post(ROUTES.create, { location_id: locationId }));
  }

  /**
   * Open the library in a chosen folder (`POST /api/v1/libraries/openings`). Writes nothing.
   * Network I/O.
   *
   * @param locationId The folder's opaque location ID.
   * @returns The library; the same one again when the folder is already open.
   * @throws ApiError as the API answers it (409 not_a_storage_root, git_in_root, and so on).
   * @throws TypeError when the answer breaks the contract (a body that is not JSON surfaces from
   *   the client as a SyntaxError; the pages show both as an error).
   */
  async open(locationId: string): Promise<LibraryView> {
    return parseLibraryView(await this.post(ROUTES.open, { location_id: locationId }));
  }

  /**
   * Copy a completed snapshot into another chosen folder
   * (`POST /api/v1/libraries/snapshot-copies`). Network I/O that writes the destination.
   *
   * @param libraryId The open library.
   * @param destinationLocationId The empty destination folder's opaque location ID.
   * @returns What was copied and which copied objects failed validation.
   * @throws ApiError as the API answers it.
   * @throws TypeError when the answer breaks the contract (a body that is not JSON surfaces from
   *   the client as a SyntaxError; the pages show both as an error).
   */
  async copy(libraryId: string, destinationLocationId: string): Promise<SnapshotCopyReport> {
    const body = { library_id: libraryId, destination_location_id: destinationLocationId };
    return parseSnapshotCopyReport(await this.post(ROUTES.copy, body));
  }

  /**
   * Read every object of an open library from its files (`POST /api/v1/libraries/rebuilds`).
   * Writes nothing and repairs nothing. Network I/O.
   *
   * @param libraryId The open library.
   * @returns Each object read, those unreadable, and the objects flagged for repair.
   * @throws ApiError as the API answers it (404 library_not_open, 503 unavailable).
   * @throws TypeError when the answer breaks the contract (a body that is not JSON surfaces from
   *   the client as a SyntaxError; the pages show both as an error).
   */
  async rebuild(libraryId: string): Promise<RebuildReport> {
    return parseRebuildReport(await this.post(ROUTES.rebuild, { library_id: libraryId }));
  }

  /**
   * List the objects the repair may be offered for
   * (`GET /api/v1/libraries/repair-candidates?library_id=`). Writes nothing. Network I/O.
   *
   * @param libraryId The open library.
   * @returns At most 1,000 candidates and whether more exist.
   * @throws ApiError as the API answers it.
   * @throws TypeError when the answer breaks the contract (a body that is not JSON surfaces from
   *   the client as a SyntaxError; the pages show both as an error).
   */
  async repairCandidates(libraryId: string): Promise<RepairCandidates> {
    const path = `${ROUTES.candidates}?library_id=${encodeURIComponent(libraryId)}`;
    return parseRepairCandidates(await this.client.request<unknown>(path));
  }

  /**
   * Run the consented repair (`POST /api/v1/libraries/repairs`). Only call this after the owner
   * confirmed the consent dialog; it writes into each named object's latest version directory.
   * Network I/O.
   *
   * @param request The library, the objects and the owner's consent for exactly those objects.
   * @returns One outcome per object, in request order.
   * @throws ApiError as the API answers it (403 not the owner, 409 consent_required or
   *   consent_mismatch, 404, 503).
   * @throws TypeError when the answer breaks the contract (a body that is not JSON surfaces from
   *   the client as a SyntaxError; the pages show both as an error).
   */
  async repair(request: RepairRequest): Promise<RepairAnswer> {
    return parseRepairAnswer(await this.post(ROUTES.repair, request));
  }

  /** POST a JSON body and return the parsed answer, still unchecked. */
  private post(path: string, body: unknown): Promise<unknown> {
    return this.client.request<unknown>(path, { method: "POST", body });
  }
}
