/**
 * The live {@link NavigationSource} (UI-F2.1b part 2): answers the navigation tree through
 * `FileSquidClient` (`GET /api/v1/navigation`, API-S-021) with the generated contract types,
 * so `<fs-nav-tree>` and `<fs-navigation-view>` work over the connected host without change.
 * Every answer is checked against the contract before it is returned; API errors (`not_found`,
 * `invalid_request`, 503 `unavailable`, denials) pass through unchanged for the views to show.
 *
 * Changes: one event subscription is shared by every listener, opened with the first and
 * closed with the last. A `navigation.change` event becomes the named `parents` or `all`;
 * `events.resync`, each time the channel opens (events may have been missed while it was
 * closed) and a payload that breaks the contract become `all`, so the whole visible tree is
 * read again.
 */
import type { ApiEvent, NavigationPageBody, NavigationQuery } from "../api/client";
import { parseNavigationChange, parseNavigationPage } from "./contract";
import type { NavigationChange, NavigationPage, NavigationSource } from "./model";

/** The part of the API client the source needs (injectable in tests). */
export interface NavigationClient {
  /** Read one page of one navigation level. */
  navigation(query?: NavigationQuery): Promise<NavigationPageBody>;
  /** Subscribe to push events; `onOpen` is called each time the channel opens. */
  subscribe(listener: (event: ApiEvent) => void, onOpen?: () => void): () => void;
}

/** The change that asks for the whole visible tree to be read again. */
const ALL: NavigationChange = Object.freeze({ all: true as const });

/** A navigation source over the FileSquid API. */
export class ClientNavigationSource implements NavigationSource {
  private readonly listeners = new Set<(change: NavigationChange) => void>();
  private closeChannel?: () => void;

  /**
   * @param client The API client (`FileSquidClient`, or a fake in tests).
   */
  constructor(private readonly client: NavigationClient) {}

  /**
   * List one level through the API and check the answer. Network I/O via the client.
   *
   * @param parent A node handle, or null for the repository level.
   * @param cursor The previous page's `next_cursor`, or null for the first page.
   * @param limit Page size, 1 to 1,000; the API's default (200) when omitted.
   * @returns The page, with contract fields only.
   * @throws ApiError as the API answers it (`not_found`, `invalid_request`, 503, denials).
   * @throws TypeError when the answer breaks the contract; it is never shown.
   */
  async list(
    parent: string | null,
    cursor?: string | null,
    limit?: number,
  ): Promise<NavigationPage> {
    const answer = await this.client.navigation(listingQuery(parent, cursor, limit));
    const page = parseNavigationPage(answer);
    if (!page) throw new TypeError("FileSquid navigation answer does not satisfy the contract");
    return page;
  }

  /**
   * Follow changes to the tree. The first listener opens the event channel; the last one to
   * stop closes it. A channel that cannot be opened is logged; listings still work.
   *
   * @param listener Called with each change.
   * @returns A function that stops following; calling it again does nothing.
   */
  subscribe(listener: (change: NavigationChange) => void): () => void {
    const entry = (change: NavigationChange) => listener(change);
    this.listeners.add(entry);
    if (this.listeners.size === 1) this.openChannel();
    return () => this.unfollow(entry);
  }

  /**
   * Close the event channel and open a new one while anyone listens, for example after a
   * token was entered: the new channel carries the token known now, and its opening reads the
   * whole visible tree again. Does nothing without listeners. Side effect: network connection.
   */
  reconnect(): void {
    if (this.listeners.size === 0) return;
    this.closeChannel?.();
    this.closeChannel = undefined;
    this.openChannel();
  }

  /** Remove one listener, closing the channel after the last. */
  private unfollow(entry: (change: NavigationChange) => void): void {
    if (!this.listeners.delete(entry) || this.listeners.size > 0) return;
    this.closeChannel?.();
    this.closeChannel = undefined;
  }

  /** Open the shared event subscription; a refusal is logged and leaves no channel. */
  private openChannel(): void {
    try {
      this.closeChannel = this.client.subscribe(
        (event) => this.onEvent(event),
        () => this.announce(ALL),
      );
    } catch (error) {
      console.error("FileSquid navigation could not open the event channel", error);
    }
  }

  /** Turn one event into a change for every listener, or ignore it. */
  private onEvent(event: ApiEvent): void {
    if (event.topic === "events.resync") {
      this.announce(ALL);
      return;
    }
    if (event.topic !== "navigation.change") return;
    const change = parseNavigationChange(event.data);
    if (!change) {
      console.warn("FileSquid navigation.change payload broke the contract; reading the tree");
    }
    this.announce(change ?? ALL);
  }

  /** Give one change to every listener. */
  private announce(change: NavigationChange): void {
    for (const listener of [...this.listeners]) listener(change);
  }
}

/** The API query of one listing: only the fields given. Pure. */
function listingQuery(
  parent: string | null,
  cursor: string | null | undefined,
  limit: number | undefined,
): NavigationQuery {
  const query: NavigationQuery = {};
  if (parent !== null) query.node = parent;
  if (cursor !== undefined && cursor !== null) query.cursor = cursor;
  if (limit !== undefined) query.limit = limit;
  return query;
}
