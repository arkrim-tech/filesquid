/**
 * The durable jobs the library screens follow (UI-1.3, UI-S-027, API-S-025, API-S-037): read
 * one job, pause, resume and cancel it, and watch `job.update` on the event channel. Every
 * answer is checked against the contract before it is returned (a host that answers outside it
 * is a `TypeError`), a job is only ever named by its opaque ID, and no path, source identity or
 * checkpoint crosses this module. The controls name the state they ask for, take no body and
 * answer the job view (API-S-002 exempts them from the command envelope), so repeating one
 * changes nothing and needs no operation ID.
 */
import type { ApiEvent, RequestOptions } from "../api/client";
import { parseJobControl, parseJobSummary, parseJobView } from "./contract";
import type { JobView } from "./model";

/** What a watcher is told. */
export type JobWatchEvent =
  { readonly kind: "update"; readonly job: JobView } | { readonly kind: "resync" };

/** What the library screens need from the job routes (the live one is `ClientRebuildJobSource`). */
export interface RebuildJobSource {
  /** Read one job (`GET /api/v1/jobs/{job}`), with its report once it has completed. */
  job(jobId: string): Promise<JobView>;
  /** Pause the job: a running worker suspends at its next checkpoint. Answers the job. */
  pause(jobId: string): Promise<JobView>;
  /** Clear the pause; the job resumes from its checkpoint without repeating work. */
  resume(jobId: string): Promise<JobView>;
  /** Cancel the job: its worker stops at its next checkpoint. Answers the job. */
  cancel(jobId: string): Promise<JobView>;
  /**
   * Follow the event channel: a checked `job.update` is an `update`; `events.resync` and a
   * (re)opened channel are a `resync`, after which the watcher reads the job again.
   *
   * @param listener Called for each event.
   * @returns A function that stops watching and closes what it opened.
   */
  watch(listener: (event: JobWatchEvent) => void): () => void;
}

/** The part of the API client the source needs (injectable in tests). */
export interface RebuildJobClient {
  /** Send a JSON request and parse the JSON answer. */
  request<T>(path: string, options?: RequestOptions): Promise<T>;
  /** Subscribe to the event channel; `onOpen` runs each time it opens. */
  subscribe(listener: (event: ApiEvent) => void, onOpen?: () => void): () => void;
}

/** The three control routes. */
type Control = "pause" | "resume" | "cancel";

/** A job source over the FileSquid API. */
export class ClientRebuildJobSource implements RebuildJobSource {
  /** @param client The API client (`FileSquidClient`, or a fake in tests). */
  constructor(private readonly client: RebuildJobClient) {}

  /**
   * Read one job. Network I/O.
   *
   * @param jobId The job's opaque ID.
   * @returns The job view; its `result` is the report once it has completed.
   * @throws ApiError as the API answers it (404 `not_found`, 503 `unavailable`).
   * @throws TypeError when the answer breaks the contract.
   */
  async job(jobId: string): Promise<JobView> {
    return parseJobView(await this.client.request<unknown>(path(jobId)));
  }

  /**
   * Pause a job (`POST /api/v1/jobs/{job}/pause`). Network I/O.
   *
   * @param jobId The job's opaque ID.
   * @returns The job view after the request.
   * @throws ApiError as the API answers it (404, 409 `conflict`, 503).
   * @throws TypeError when the answer breaks the contract.
   */
  pause(jobId: string): Promise<JobView> {
    return this.control(jobId, "pause");
  }

  /**
   * Resume a job (`POST /api/v1/jobs/{job}/resume`). Network I/O.
   *
   * @param jobId The job's opaque ID.
   * @returns The job view after the request.
   * @throws ApiError as the API answers it (404, 409 `conflict`, 503).
   * @throws TypeError when the answer breaks the contract.
   */
  resume(jobId: string): Promise<JobView> {
    return this.control(jobId, "resume");
  }

  /**
   * Cancel a job (`POST /api/v1/jobs/{job}/cancel`). Network I/O.
   *
   * @param jobId The job's opaque ID.
   * @returns The job view after the request.
   * @throws ApiError as the API answers it (404, 409 `conflict`, 503).
   * @throws TypeError when the answer breaks the contract.
   */
  cancel(jobId: string): Promise<JobView> {
    return this.control(jobId, "cancel");
  }

  /**
   * Follow `job.update` on the event channel. Opens the channel's subscription; the returned
   * function closes it.
   *
   * @param listener Called for each update and each resync.
   * @returns A function that stops watching.
   */
  watch(listener: (event: JobWatchEvent) => void): () => void {
    const onEvent = (event: ApiEvent) => deliver(event, listener);
    return this.client.subscribe(onEvent, () => listener({ kind: "resync" }));
  }

  /** POST one control route with no body and check the answer. Network I/O. */
  private async control(jobId: string, which: Control): Promise<JobView> {
    const answer = await this.client.request<unknown>(`${path(jobId)}/${which}`, {
      method: "POST",
    });
    return parseJobControl(answer).job;
  }
}

/** The route of one job, its ID escaped. */
function path(jobId: string): string {
  return `/api/v1/jobs/${encodeURIComponent(jobId)}`;
}

/**
 * Turn one event into what the watcher is told: an update with the checked job, a resync, or
 * nothing for any other topic. An update that breaks the contract is dropped and logged, never
 * shown.
 */
function deliver(event: ApiEvent, listener: (event: JobWatchEvent) => void): void {
  if (event.topic === "events.resync") {
    listener({ kind: "resync" });
    return;
  }
  if (event.topic !== "job.update") return;
  let job: JobView;
  try {
    job = parseJobSummary(event.data);
  } catch (error) {
    console.warn("FileSquid dropped a job update that breaks the contract", error);
    return;
  }
  listener({ kind: "update", job });
}
