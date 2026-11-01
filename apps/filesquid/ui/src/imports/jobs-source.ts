/**
 * The live {@link JobSource} (UI-1.2, UI-S-012, API-S-037): the durable upload jobs, their files
 * and their controls through `FileSquidClient`, each answer checked against the contract. The
 * controls (pause, resume, cancel, retry one file, retry failed) take no body and no envelope:
 * each names the state it asks for, so repeating one changes nothing, and each answers the job
 * view. A refusal (409 `conflict`, 404 `not_found`, 503 `unavailable`, a denial) passes through as
 * `ApiError`.
 *
 * Changes: one event subscription is shared by every follower, opened with the first and closed
 * with the last. A `job.update` event becomes the job it carries; `events.resync`, each time the
 * channel opens (events may have been missed while it was closed) and a payload that breaks the
 * contract become a resync, so the list is read again.
 */
import type { ApiEvent, RequestOptions } from "../api/client";
import { parseJobControlAnswer, parseJobItemPage, parseJobPage, parseJobView } from "./contract";
import { pageQuery } from "./client-source";
import {
  IMPORT_JOB_KIND,
  type JobChange,
  type JobItemPage,
  type JobPage,
  type JobSource,
  type JobView,
} from "./model";

/** The part of the API client the source needs (injectable in tests). */
export interface JobClient {
  /** Send a JSON request and parse the JSON answer. */
  request<T>(path: string, options?: RequestOptions): Promise<T>;
  /** Subscribe to push events; `onOpen` is called each time the channel opens. */
  subscribe(listener: (event: ApiEvent) => void, onOpen?: () => void): () => void;
}

/** The most jobs one listing answers (API-S-037). */
const LIST_LIMIT = 100;
/** The change that asks for the list to be read again. */
const RESYNC: JobChange = Object.freeze({ kind: "resync" as const });

/** A job source over the FileSquid API. */
export class ClientJobSource implements JobSource {
  private readonly listeners = new Set<(change: JobChange) => void>();
  private closeChannel?: () => void;

  /** @param client The API client (`FileSquidClient`, or a fake in tests). */
  constructor(private readonly client: JobClient) {}

  /**
   * List the upload jobs, newest first (`GET /api/v1/jobs?kind=import&limit=100`). Network I/O.
   *
   * @returns At most 100 jobs, and whether more matched.
   * @throws ApiError as the API answers it (503 `unavailable` on a host without job progress).
   * @throws TypeError when the answer breaks the contract.
   */
  async list(): Promise<JobPage> {
    const path = `/api/v1/jobs?kind=${IMPORT_JOB_KIND}&limit=${LIST_LIMIT}`;
    return parseJobPage(await this.client.request<unknown>(path));
  }

  /**
   * Read one page of a job's files (`GET /api/v1/jobs/{job}/items`). Network I/O.
   *
   * @param job The job.
   * @param cursor The previous page's `next_cursor`.
   * @param limit Page size, 1 to 1,000.
   * @returns The page.
   * @throws ApiError as the API answers it (404 for an unknown job).
   * @throws TypeError when the answer breaks the contract.
   */
  async items(job: string, cursor?: string | null, limit?: number): Promise<JobItemPage> {
    const path = `/api/v1/jobs/${encodeURIComponent(job)}/items${pageQuery(cursor, limit)}`;
    return parseJobItemPage(await this.client.request<unknown>(path));
  }

  /** Hold a job (`POST /api/v1/jobs/{job}/pause`). Network I/O. */
  pause(job: string): Promise<JobView> {
    return this.control(job, "pause");
  }

  /** Clear a job's pause (`POST /api/v1/jobs/{job}/resume`). Network I/O. */
  resume(job: string): Promise<JobView> {
    return this.control(job, "resume");
  }

  /** Stop a job; committed files stay (`POST /api/v1/jobs/{job}/cancel`). Network I/O. */
  cancel(job: string): Promise<JobView> {
    return this.control(job, "cancel");
  }

  /** Put one failed file back to queued (`POST /api/v1/jobs/{job}/items/{item}/retry`). */
  retryItem(job: string, item: string): Promise<JobView> {
    return this.control(job, `items/${encodeURIComponent(item)}/retry`);
  }

  /** Put every failed file back to queued (`POST /api/v1/jobs/{job}/retry-failed`). */
  retryFailed(job: string): Promise<JobView> {
    return this.control(job, "retry-failed");
  }

  /** One control: a POST with no body that answers the job view. */
  private async control(job: string, action: string): Promise<JobView> {
    const path = `/api/v1/jobs/${encodeURIComponent(job)}/${action}`;
    return parseJobControlAnswer(await this.client.request<unknown>(path, { method: "POST" }));
  }

  /**
   * Follow changes to jobs. The first follower opens the event channel; the last one to stop
   * closes it. A channel that cannot be opened is logged; reads still work.
   *
   * @param listener Called with each change.
   * @returns A function that stops following; calling it again does nothing.
   */
  subscribe(listener: (change: JobChange) => void): () => void {
    const entry = (change: JobChange) => listener(change);
    this.listeners.add(entry);
    if (this.listeners.size === 1) this.openChannel();
    return () => this.unfollow(entry);
  }

  /** Remove one follower, closing the channel after the last. */
  private unfollow(entry: (change: JobChange) => void): void {
    if (!this.listeners.delete(entry) || this.listeners.size > 0) return;
    this.closeChannel?.();
    this.closeChannel = undefined;
  }

  /** Open the shared event subscription; a refusal is logged and leaves no channel. */
  private openChannel(): void {
    try {
      this.closeChannel = this.client.subscribe(
        (event) => this.onEvent(event),
        () => this.announce(RESYNC),
      );
    } catch (error) {
      console.error("FileSquid jobs could not open the event channel", error);
    }
  }

  /** Turn one event into a change for every follower, or ignore it. */
  private onEvent(event: ApiEvent): void {
    if (event.topic === "events.resync") {
      this.announce(RESYNC);
      return;
    }
    if (event.topic !== "job.update") return;
    try {
      const job = parseJobView(event.data);
      // Other kinds of job (a library rebuild) are not uploads and have no place in this panel.
      if (job.kind === IMPORT_JOB_KIND) this.announce({ kind: "job", job });
    } catch (error) {
      console.warn("FileSquid job.update payload broke the contract; reading the jobs", error);
      this.announce(RESYNC);
    }
  }

  /** Give one change to every follower. */
  private announce(change: JobChange): void {
    for (const listener of [...this.listeners]) listener(change);
  }
}
