/**
 * The live {@link LibrarySource} (UI-1.1, UI-S-012, API-S-025, API-S-034): every call goes
 * through `FileSquidClient` to a route of the API contract with the contract's body, and every
 * answer is checked against the contract before it is returned. A folder is only ever the
 * opaque location ID the host registered when the owner chose it; the source never builds,
 * receives or logs a filesystem path. API refusals (`conflict`, `not_found`, 503 `unavailable`,
 * denials) pass through unchanged as `ApiError` for the views to word.
 *
 * Create, open, copy and rebuild are commands (API-F2.19, API-S-002): each goes through
 * `FileSquidClient.command()` with an envelope carrying an operation id, the scope the
 * route derives from its input (`location:<id>` for create and open, `library:<id>` for copy
 * and rebuild) and the expected revision (`0` for create and open; the library's `revision`
 * as last read for copy and rebuild). A `conflict` outcome is raised as the `ApiError` 409
 * `conflict` the views already word; any other outcome than the one the route answers is a
 * contract break (`TypeError`).
 *
 * **Retries keep their operation id.** A command that did not answer `committed` or `pending`
 * (a lost answer, a 5xx, an operation still in flight, a conflict) keeps its envelope, and the
 * next call with the same route, scope, expected revision and input sends it again, so a
 * command the host already committed answers its stored receipt instead of being refused as
 * `location_in_use` or `destination_not_empty`. A command that answered releases its envelope;
 * at most {@link MAX_RETAINED_COMMANDS} are kept, the oldest dropped first.
 */
import { ApiError, type RequestOptions } from "../api/client";
import { commandEnvelope, type CommandAnswer, type CommandEnvelope } from "../api/commands";
import {
  parseLibraryRevision,
  parseLibraryView,
  parseLocationChoice,
  parseRebuildJob,
  parseRepairAnswer,
  parseRepairCandidates,
  parseSnapshotCopyReport,
} from "./contract";
import {
  INITIAL_LIBRARY_REVISION,
  NEW_LIBRARY_REVISION,
  type ChoicePurpose,
  type LibrarySource,
  type LibraryView,
  type LocationChoice,
  type RebuildJobAnswer,
  type RepairAnswer,
  type RepairCandidates,
  type RepairRequest,
  type SnapshotCopyReport,
} from "./model";

/** The part of the API client the source needs (injectable in tests). */
export interface LibraryClient {
  /** Send a JSON request and parse the JSON answer. */
  request<T>(path: string, options?: RequestOptions): Promise<T>;
  /** Send a command `{ envelope, input }` and parse its outcome. */
  command<T>(path: string, envelope: CommandEnvelope, input?: unknown): Promise<CommandAnswer<T>>;
}

/** Most unanswered command envelopes kept for a retry (AGENTS.md §22). */
export const MAX_RETAINED_COMMANDS = 32;

/** HTTP status of a `conflict` answer, as the views word it. */
const HTTP_CONFLICT = 409;

/**
 * The `result` of a command answer of `kind`, or the failure the views word. Pure.
 *
 * @param answer The outcome the client resolved.
 * @param path The route, named in the error.
 * @param kind The outcome the route answers on success: `committed`, or `pending` for a job.
 * @returns The result, still unchecked.
 * @throws ApiError 409 `conflict` for a `conflict` outcome.
 * @throws TypeError for any other outcome than `kind`, or one without a result.
 */
function resultOf(answer: CommandAnswer, path: string, kind: "committed" | "pending"): unknown {
  if (answer.kind === "conflict") throw new ApiError(HTTP_CONFLICT, path, { code: "conflict" });
  if (answer.kind !== kind || answer.result === undefined || answer.result === null) {
    throw new TypeError(`${path} answered ${answer.kind}, not a ${kind} result`);
  }
  return answer.result;
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
  /** Envelopes of commands that have not answered yet, by command identity, oldest first. */
  private readonly retained = new Map<string, CommandEnvelope>();

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
    return parseLibraryView(await this.bind(ROUTES.create, locationId));
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
    return parseLibraryView(await this.bind(ROUTES.open, locationId));
  }

  /**
   * Copy a completed snapshot into another chosen folder
   * (`POST /api/v1/libraries/snapshot-copies`). Network I/O that writes the destination.
   *
   * @param libraryId The open library.
   * @param destinationLocationId The empty destination folder's opaque location ID.
   * @param expectedRevision The library's revision as last read; a stale one is `conflict`.
   * @returns What was copied and which copied objects failed validation.
   * @throws ApiError as the API answers it, a `conflict` outcome as 409 `conflict`.
   * @throws TypeError when the answer breaks the contract (a body that is not JSON surfaces from
   *   the client as a TypeError; the pages show it as an error).
   */
  async copy(
    libraryId: string,
    destinationLocationId: string,
    expectedRevision: string,
  ): Promise<SnapshotCopyReport> {
    const input = { library_id: libraryId, destination_location_id: destinationLocationId };
    const answer = await this.send(ROUTES.copy, `library:${libraryId}`, expectedRevision, input);
    return parseSnapshotCopyReport(resultOf(answer, ROUTES.copy, "committed"));
  }

  /**
   * Start the rebuild of an open library from its files as a durable `library.rebuild` job
   * (`POST /api/v1/libraries/rebuilds`; API-S-025). Writes nothing and repairs nothing. The
   * answer is the pending job: its progress and its report come from the job routes (UI-1.3).
   * Network I/O.
   *
   * @param libraryId The open library.
   * @param expectedRevision The library's revision as last read; a stale one is `conflict`.
   * @returns The pending job, its ID and whether the host runs it now.
   * @throws ApiError as the API answers it (404 library_not_open, 409 conflict, 503 unavailable).
   * @throws TypeError when the answer breaks the contract (a body that is not JSON surfaces from
   *   the client as a TypeError; the pages show it as an error).
   */
  async rebuild(libraryId: string, expectedRevision: string): Promise<RebuildJobAnswer> {
    const input = { library_id: libraryId };
    const answer = await this.send(ROUTES.rebuild, `library:${libraryId}`, expectedRevision, input);
    return parseRebuildJob(resultOf(answer, ROUTES.rebuild, "pending"));
  }

  /**
   * Read the current revision of a library from its facts
   * (`GET /api/v1/libraries/{library}/facts`; API-S-038), for a page that did not create or
   * open it. Writes nothing. Network I/O.
   *
   * @param libraryId The library.
   * @returns Its revision; the initial revision when it is not open now, so the command that
   *   follows is refused by the API as `library_not_open`.
   * @throws ApiError as the API answers it (404 not_found for a library the caller may not read).
   * @throws TypeError when the answer breaks the contract.
   */
  async revision(libraryId: string): Promise<string> {
    const path = `/api/v1/libraries/${encodeURIComponent(libraryId)}/facts`;
    const revision = parseLibraryRevision(await this.client.request<unknown>(path));
    return revision ?? INITIAL_LIBRARY_REVISION;
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

  /**
   * Send the create or open command of the folder `locationId`: scope `location:<id>`,
   * expecting revision `0` (the library is not open on this host). Network I/O.
   *
   * @returns The committed result, still unchecked.
   */
  private async bind(path: string, locationId: string): Promise<unknown> {
    const scope = `location:${locationId}`;
    const answer = await this.send(path, scope, NEW_LIBRARY_REVISION, { location_id: locationId });
    return resultOf(answer, path, "committed");
  }

  /**
   * Send the command `{ envelope, input }` of `path`: under the envelope kept from an earlier
   * attempt of the same command that did not answer, else under a fresh operation id. The
   * envelope is released once the command answers `committed` or `pending`. Network I/O.
   */
  private async send(
    path: string,
    scope: string,
    expectedRevision: string,
    input: unknown,
  ): Promise<CommandAnswer> {
    const key = JSON.stringify([path, scope, expectedRevision, input]);
    const envelope = this.retain(key, () => commandEnvelope({ scope, expectedRevision }));
    const answer = await this.client.command<unknown>(path, envelope, input);
    if (answer.kind === "committed" || answer.kind === "pending") this.retained.delete(key);
    return answer;
  }

  /**
   * The envelope kept under `key`, else a new one from `make`, kept until its command answers;
   * the oldest kept envelope is dropped beyond {@link MAX_RETAINED_COMMANDS}. Pure apart from
   * the map.
   */
  private retain(key: string, make: () => CommandEnvelope): CommandEnvelope {
    const envelope = this.retained.get(key) ?? make();
    this.retained.delete(key);
    this.retained.set(key, envelope);
    while (this.retained.size > MAX_RETAINED_COMMANDS) {
      const oldest = this.retained.keys().next().value;
      if (oldest === undefined) break;
      this.retained.delete(oldest);
    }
    return envelope;
  }
}
