/**
 * The {@link DestinationDirectory} the app installs (UI-1.2, API-S-021): navigation node handles
 * are opaque and no API route yet maps one to a collection ID, so the lookup is a function the
 * host's UI supplies when the API can answer, and the directory remembers each collection's name
 * from the choice that resolved it, so the Jobs panel can title a job "Upload into {collection}"
 * from the job's `collection_id` alone. Without a lookup nothing resolves and the dialog says so
 * honestly before sending anything (the gap is reported to the platform engineer).
 */
import type { DestinationDirectory, ResolvedDestination } from "./model";

/** Answers the library and collection ID of a node handle, or undefined when it cannot. */
export type DestinationLookup = (node: string) => Promise<ResolvedDestination | undefined>;

/** Most collection names remembered; the oldest is forgotten first (AGENTS.md section 22). */
export const MAX_REMEMBERED_NAMES = 1000;

/** A directory over a lookup that remembers the names of the collections it resolved. */
export class NamedDestinations implements DestinationDirectory {
  private readonly names = new Map<string, string>();

  /** @param lookup Resolves a node handle; resolves nothing by default. */
  constructor(private readonly lookup: DestinationLookup = async () => undefined) {}

  /**
   * Resolve a chosen collection or folder and remember its collection's name.
   *
   * @param choice The picker's node handle and the names from the library down.
   * @returns The IDs, or undefined when the lookup cannot name them.
   */
  async resolve(choice: {
    readonly node: string;
    readonly trail: readonly string[];
  }): Promise<ResolvedDestination | undefined> {
    const found = await this.lookup(choice.node);
    const name = choice.trail[1];
    if (found && name !== undefined) this.remember(found.collectionId, name);
    return found;
  }

  /**
   * The name of a collection resolved before.
   *
   * @param collectionId The collection.
   * @returns Its name, or undefined when it was not resolved in this session.
   */
  collectionName(collectionId: string): string | undefined {
    return this.names.get(collectionId);
  }

  /** Keep a name, forgetting the oldest beyond the cap. */
  private remember(collectionId: string, name: string): void {
    this.names.delete(collectionId);
    this.names.set(collectionId, name);
    if (this.names.size <= MAX_REMEMBERED_NAMES) return;
    const oldest = this.names.keys().next().value;
    if (oldest !== undefined) this.names.delete(oldest);
  }
}
