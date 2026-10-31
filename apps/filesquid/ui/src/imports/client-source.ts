/**
 * The live {@link ImportSource} (UI-1.2, UI-S-012, API-S-036): every call goes through
 * `FileSquidClient` to a route of the API contract with the contract's body, and every answer is
 * checked against the contract before it is returned. The source never builds, receives or logs a
 * filesystem path: the host shows its own native dialog, and only counts, sizes, display names and
 * IDs cross the API. API refusals (`not_found`, `conflict`, 503 `unavailable`, denials) pass
 * through unchanged as `ApiError` for the dialog to word.
 *
 * The plan request is the route's one command: its envelope's operation ID is the idempotency
 * key, so a retry that keeps the ID (a lost answer) is answered with its open plan instead of a
 * second dialog. The route answers `{ outcome, import, ... }` rather than a bare outcome, so it is
 * sent through `request()`. An import adds new files only, so the expected revision is not
 * compared and the envelope carries the initial one.
 */
import type { RequestOptions } from "../api/client";
import { commandEnvelope } from "../api/commands";
import { NEW_LIBRARY_REVISION } from "../libraries/model";
import { parseImportAnswer, parsePlanItemPage } from "./contract";
import type { ImportSource, PlanAnswer, PlanItemPage, PlanRequest } from "./model";

/** The part of the API client the source needs (injectable in tests). */
export interface ImportClient {
  /** Send a JSON request and parse the JSON answer. */
  request<T>(path: string, options?: RequestOptions): Promise<T>;
}

/** The route of an upload request. */
const IMPORTS = "/api/v1/imports";

/** The query string of a page read: the cursor, then the page size, only those given. */
export function pageQuery(cursor?: string | null, limit?: number): string {
  const params = new URLSearchParams();
  if (cursor !== undefined && cursor !== null) params.set("cursor", cursor);
  if (limit !== undefined) params.set("limit", String(limit));
  const text = params.toString();
  return text === "" ? "" : `?${text}`;
}

/** An import source over the FileSquid API. */
export class ClientImportSource implements ImportSource {
  /** @param client The API client (`FileSquidClient`, or a fake in tests). */
  constructor(private readonly client: ImportClient) {}

  /**
   * Ask the host to let the owner pick the source and plan the import
   * (`POST /api/v1/imports`). On the desktop this shows the native dialog and waits for the
   * owner. Network I/O; nothing is staged.
   *
   * @param request The library, destination collection and folder, source and, for a retry,
   *   the operation ID.
   * @returns The plan to review, the job of a host without the review, or that the owner closed
   *   the dialog.
   * @throws ApiError as the API answers it (400 invalid_request, 404 not_found, 409 conflict,
   *   503 unavailable, denials).
   * @throws TypeError when the answer breaks the contract.
   */
  async plan(request: PlanRequest): Promise<PlanAnswer> {
    const envelope = commandEnvelope({
      scope: `library:${request.libraryId}`,
      expectedRevision: NEW_LIBRARY_REVISION,
      operationId: request.operationId,
    });
    const body = {
      envelope,
      library_id: request.libraryId,
      collection_id: request.collectionId,
      ...(request.folder === undefined ? {} : { folder: request.folder }),
      source: request.source,
    };
    return parseImportAnswer(await this.client.request<unknown>(IMPORTS, { method: "POST", body }));
  }

  /**
   * Read one page of a plan's files (`GET /api/v1/imports/{plan_id}/items`). Network I/O.
   *
   * @param planId The plan.
   * @param cursor The previous page's `next_cursor`.
   * @param limit Page size, 1 to 1,000; the API's default (1,000) when omitted.
   * @returns The page.
   * @throws ApiError as the API answers it (404 once the plan expired or was discarded).
   * @throws TypeError when the answer breaks the contract.
   */
  async planItems(planId: string, cursor?: string | null, limit?: number): Promise<PlanItemPage> {
    const path = `${IMPORTS}/${encodeURIComponent(planId)}/items${pageQuery(cursor, limit)}`;
    return parsePlanItemPage(await this.client.request<unknown>(path));
  }

  /**
   * Start the job of a plan (`POST /api/v1/imports/{plan_id}/start`; no body, the plan ID is the
   * idempotency key). Network I/O that starts durable work.
   *
   * @param planId The plan.
   * @returns The job's ID; a repeated start answers the same job.
   * @throws ApiError as the API answers it (404 plan gone, 409 the source changed).
   * @throws TypeError when the answer is not a started job.
   */
  async start(planId: string): Promise<string> {
    const path = `${IMPORTS}/${encodeURIComponent(planId)}/start`;
    const answer = parseImportAnswer(await this.client.request<unknown>(path, { method: "POST" }));
    if (answer.kind !== "started")
      throw new TypeError("FileSquid import start did not start a job");
    return answer.jobId;
  }

  /**
   * Discard a plan (`DELETE /api/v1/imports/{plan_id}`): nothing was staged and nothing changes.
   * Network I/O.
   *
   * @param planId The plan.
   * @returns Resolves once the host has dropped the plan.
   * @throws ApiError as the API answers it (404 when it is already gone).
   */
  async discard(planId: string): Promise<void> {
    const path = `${IMPORTS}/${encodeURIComponent(planId)}`;
    await this.client.request<unknown>(path, { method: "DELETE" });
  }
}
