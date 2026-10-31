/**
 * The destination lookup of the app (UI-1.2, API-S-021, API-S-036): an upload names its library
 * and destination collection by ID, but a navigation node handle is opaque and the contract has
 * no route that maps a node to a collection ID. This lookup asks the one route the platform
 * engineer is to add, `GET /api/v1/navigation/destination?node=<handle>`, which answers
 * `{ library_id, collection_id }` for a collection or a folder inside one. The route is not in
 * the contract yet, so a host that does not serve it answers 404, 503 or something else, and the
 * lookup answers undefined: the dialog then says it cannot name the collection and sends nothing.
 * A failure is logged, never swallowed. The answer is checked before it is used and never carries
 * a path.
 */
import type { RequestOptions } from "../api/client";
import type { DestinationLookup } from "./directory";
import type { ResolvedDestination } from "./model";

/** The part of the API client the lookup needs (injectable in tests). */
export interface DestinationClient {
  /** Send a JSON request and parse the JSON answer. */
  request<T>(path: string, options?: RequestOptions): Promise<T>;
}

/** The route that maps a navigation node to its library and collection IDs (to be added). */
export const DESTINATION_ROUTE = "/api/v1/navigation/destination";

/** True for a non-empty string. */
function isText(value: unknown): value is string {
  return typeof value === "string" && value !== "";
}

/**
 * Check the answer of the destination route.
 *
 * @param value The parsed JSON answer.
 * @returns The library and collection IDs, or undefined when the answer lacks either.
 */
export function parseResolvedDestination(value: unknown): ResolvedDestination | undefined {
  if (typeof value !== "object" || value === null) return undefined;
  const { library_id: libraryId, collection_id: collectionId } = value as Record<string, unknown>;
  if (!isText(libraryId) || !isText(collectionId)) return undefined;
  return { libraryId, collectionId };
}

/**
 * A lookup over the destination route. Network I/O when called.
 *
 * @param client The API client (`FileSquidClient`, or a fake in tests).
 * @returns A function from a node handle to its IDs; undefined when the host cannot name them.
 */
export function clientDestinationLookup(client: DestinationClient): DestinationLookup {
  return async (node) => {
    const path = `${DESTINATION_ROUTE}?node=${encodeURIComponent(node)}`;
    try {
      return parseResolvedDestination(await client.request<unknown>(path));
    } catch (error) {
      console.warn("FileSquid could not name the destination collection on this host", error);
      return undefined;
    }
  };
}
