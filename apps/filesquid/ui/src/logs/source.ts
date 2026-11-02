/**
 * What the Logs view reads (UI-F2.16; UI-S-012, UI-S-021): the {@link LogsSource} interface over
 * the journal routes of API-S-031 and {@link ClientLogsSource}, which answers it through
 * `FileSquidClient` and so through either host's transport. The view never fetches by itself, so
 * a fixture stands in for the API in tests. Every method only reads, and its name says so.
 */
import {
  ApiError,
  type ApiEvent,
  type FileSquidClient,
  type JournalEntryView,
  type JournalFilters,
  type JournalViewPage,
} from "../api/client";
import type { components } from "../api/schema";
import type { LogsLibrary } from "./model";

/** The libraries this host opened, as `GET /api/v1/libraries/recent` answers them. */
type RecentLibraries = components["schemas"]["RecentLibraries"];

/** The part of `FileSquidClient` the source needs (injectable in tests). */
export type LogsClient = Pick<
  FileSquidClient,
  "adminJournalPage" | "adminJournalEntry" | "adminJournalExport" | "request" | "subscribe"
>;

/** A listener of the event channel. */
export type LogsEventListener = (event: ApiEvent) => void;

/** What the Logs view asks of the API. */
export interface LogsSource {
  /** One page of entries with their PROV-O links and display names. Reads. */
  page(filters: JournalFilters): Promise<JournalViewPage>;
  /** One entry by its ID, with its display name. Reads. */
  entry(id: number): Promise<JournalEntryView>;
  /** The export text of the entries the filters select. Reads. */
  export(filters: Omit<JournalFilters, "after" | "limit" | "names">): Promise<string>;
  /** The oldest entry, or undefined for an empty journal. Reads. */
  oldest(): Promise<JournalEntryView | undefined>;
  /** The libraries the host opened, for the Library chip; empty when the host keeps none. Reads. */
  libraries(): Promise<LogsLibrary[]>;
  /** Follow the event channel; the returned function ends the subscription. */
  subscribe(listener: LogsEventListener, onOpen?: () => void): () => void;
}

/** The route of the libraries the host opened (API-S-038). */
const LIBRARIES_PATH = "/api/v1/libraries/recent";

/** HTTP status of a host that keeps no library registry. */
const UNAVAILABLE = 503;

/** The Logs source over `FileSquidClient`. */
export class ClientLogsSource implements LogsSource {
  /** @param client The API client. */
  constructor(private readonly client: LogsClient) {}

  /**
   * One page of the journal (`GET /api/v1/admin/journal`). Network I/O.
   *
   * @param filters The filters, the cursor, the page size and the order.
   * @returns The page.
   * @throws ApiError on a non-2xx answer: 401 or 403 for a caller who is not the owner.
   */
  async page(filters: JournalFilters): Promise<JournalViewPage> {
    return this.client.adminJournalPage(filters);
  }

  /**
   * One entry, with the display name of its file. Network I/O.
   *
   * @param id The entry ID.
   * @returns The entry.
   * @throws ApiError on a non-2xx answer: 404 for an unknown entry.
   */
  async entry(id: number): Promise<JournalEntryView> {
    return this.client.adminJournalEntry(id, { names: true });
  }

  /**
   * The export text. Network I/O: one request per 50,000 entries.
   *
   * @param filters The filters and the format.
   * @returns The JSON Lines or CSV text.
   * @throws ApiError on a non-2xx answer.
   */
  async export(filters: Omit<JournalFilters, "after" | "limit" | "names">): Promise<string> {
    return this.client.adminJournalExport(filters);
  }

  /**
   * The first entry the journal holds, from one ascending query of one entry. Network I/O.
   *
   * @returns The entry, or undefined for an empty journal.
   * @throws ApiError on a non-2xx answer.
   */
  async oldest(): Promise<JournalEntryView | undefined> {
    const page = await this.client.adminJournalPage({ order: "asc", limit: 1 });
    return page.entries[0];
  }

  /**
   * The libraries the host opened. A host that keeps no registry (503) has none to name: the
   * chip then offers no library, and the console says so. Network I/O.
   *
   * @returns The IDs and names.
   * @throws ApiError on any other failure.
   */
  async libraries(): Promise<LogsLibrary[]> {
    try {
      const answer = await this.client.request<RecentLibraries>(LIBRARIES_PATH);
      return answer.libraries.map((l) => ({ library_id: l.library_id, name: l.name }));
    } catch (error) {
      if (!(error instanceof ApiError) || error.status !== UNAVAILABLE) throw error;
      console.warn("FileSquid Logs found no library registry on this host", error.status);
      return [];
    }
  }

  /**
   * Follow the event channel.
   *
   * @param listener Called once per event.
   * @param onOpen Called each time the channel opens, so the view reads again.
   * @returns A function that ends the subscription.
   */
  subscribe(listener: LogsEventListener, onOpen?: () => void): () => void {
    return this.client.subscribe(listener, onOpen);
  }
}
