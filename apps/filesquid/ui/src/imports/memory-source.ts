/**
 * In-memory sources of the upload flow (UI-1.2): a {@link MemoryImportSource} that plans, lists,
 * starts and discards like the host's import route, and a {@link MemoryJobSource} that holds
 * jobs and their files and answers the job controls as API-S-037 states them (a finished job
 * refuses every control with 409, a paused job refuses a retry, a retry puts failed files back to
 * queued). They record every call so a test can assert what the elements sent, and they let a
 * test push `job.update` events. They take the place of the host in unit and component tests; the
 * elements cannot tell them from the live sources.
 */
import { ApiError } from "../api/client";
import {
  type ImportPlan,
  type ImportSource,
  type JobChange,
  type JobItem,
  type JobItemPage,
  type JobPage,
  type JobSource,
  type JobView,
  type PlanAnswer,
  type PlanItem,
  type PlanItemPage,
  type PlanRequest,
} from "./model";

/** What a {@link MemoryImportSource} answers. */
export interface MemoryImportOptions {
  /** The plan a request answers; absent answers that the owner closed the dialog. */
  readonly plan?: ImportPlan;
  /** The planned files. */
  readonly items?: readonly PlanItem[];
  /** The job a start answers. */
  readonly jobId?: string;
  /** Rejects the plan request, once, with this error. */
  readonly planError?: unknown;
}

/** An import source in memory. */
export class MemoryImportSource implements ImportSource {
  /** Every plan request, in order. */
  readonly requests: PlanRequest[] = [];
  /** Every other call as `route planId`, in order. */
  readonly calls: string[] = [];
  /** Held until released, so a test can see the planning state. */
  gate?: Promise<void>;
  /** Rejects the next start with this error. */
  startError?: unknown;

  /** @param options What the source answers. */
  constructor(private readonly options: MemoryImportOptions = {}) {}

  /** Record the request, wait for the gate and answer the plan, the closed dialog or the error. */
  async plan(request: PlanRequest): Promise<PlanAnswer> {
    this.requests.push(request);
    await this.gate;
    if (this.options.planError !== undefined) throw this.options.planError;
    const { plan } = this.options;
    return plan === undefined ? { kind: "cancelled" } : { kind: "planned", plan };
  }

  /** Answer the planned files in pages of the given size. */
  async planItems(planId: string, cursor?: string | null, limit?: number): Promise<PlanItemPage> {
    this.calls.push(`items ${planId}`);
    const all = this.options.items ?? [];
    const from = cursor === undefined || cursor === null ? 0 : Number(cursor);
    const size = limit ?? 1000;
    const next = from + size < all.length ? String(from + size) : null;
    return { items: all.slice(from, from + size), next_cursor: next };
  }

  /** Record the start and answer the job. */
  async start(planId: string): Promise<string> {
    this.calls.push(`start ${planId}`);
    if (this.startError !== undefined) throw this.startError;
    return this.options.jobId ?? "job-1";
  }

  /** Record the discard. */
  async discard(planId: string): Promise<void> {
    this.calls.push(`discard ${planId}`);
  }
}

/** True for a job that has ended. */
function ended(job: JobView): boolean {
  return job.state === "completed" || job.state === "failed" || job.state === "cancelled";
}

/** A job source in memory. */
export class MemoryJobSource implements JobSource {
  /** Every control call as `action job [item]`, in order. */
  readonly calls: string[] = [];
  /** The job ID of every page of files read, in order. */
  readonly itemReads: string[] = [];
  /** The most files a page holds, whatever the limit asked. */
  pageSize = 1000;
  private readonly listeners = new Set<(change: JobChange) => void>();

  /**
   * @param jobs The jobs; a test may push more.
   * @param itemsByJob The files of each job, by job ID.
   */
  constructor(
    readonly jobs: JobView[] = [],
    readonly itemsByJob: Record<string, JobItem[]> = {},
  ) {}

  /** The jobs, newest first. */
  async list(): Promise<JobPage> {
    const sorted = [...this.jobs].sort((a, b) => b.created.localeCompare(a.created));
    return { jobs: sorted, truncated: false };
  }

  /** One page of a job's files. */
  async items(job: string, cursor?: string | null, limit?: number): Promise<JobItemPage> {
    this.itemReads.push(job);
    const all = this.itemsByJob[job] ?? [];
    const from = cursor === undefined || cursor === null ? 0 : Number(cursor);
    const size = Math.min(limit ?? this.pageSize, this.pageSize);
    const next = from + size < all.length ? String(from + size) : null;
    return { items: all.slice(from, from + size), next_cursor: next };
  }

  /** Hold the job. */
  async pause(job: string): Promise<JobView> {
    return this.change("pause", job, (view) => ({ ...view, paused: true }));
  }

  /** Clear the job's pause. */
  async resume(job: string): Promise<JobView> {
    return this.change("resume", job, (view) => ({ ...view, paused: false }));
  }

  /** Cancel the job; committed files stay. */
  async cancel(job: string): Promise<JobView> {
    return this.change("cancel", job, (view) => ({ ...view, state: "cancelled", paused: false }));
  }

  /** Put one failed file back to queued. */
  async retryItem(job: string, item: string): Promise<JobView> {
    this.guardRetry(job);
    if (!(this.itemsByJob[job] ?? []).some((file) => file.item_id === item)) {
      throw new ApiError(404, `/api/v1/jobs/${job}/items/${item}`, { code: "not_found" });
    }
    this.requeue(job, (file) => file.item_id === item);
    return this.change(`retry`, job, (view) => view, item);
  }

  /** Put every failed file back to queued. */
  async retryFailed(job: string): Promise<JobView> {
    this.guardRetry(job);
    this.requeue(job, () => true);
    return this.change("retry-failed", job, (view) => view);
  }

  /** Follow changes. */
  subscribe(listener: (change: JobChange) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /** Replace or add a job and tell every follower, as the host's `job.update` does. */
  emit(job: JobView): void {
    const index = this.jobs.findIndex((known) => known.id === job.id);
    if (index < 0) this.jobs.push(job);
    else this.jobs[index] = job;
    for (const listener of [...this.listeners]) listener({ kind: "job", job });
  }

  /** Tell every follower to read the jobs again, as `events.resync` does. */
  resync(): void {
    for (const listener of [...this.listeners]) listener({ kind: "resync" });
  }

  /** How many followers there are now. */
  get listening(): number {
    return this.listeners.size;
  }

  /** Find a job or fail as the API does. */
  private find(job: string): JobView {
    const found = this.jobs.find((known) => known.id === job);
    if (!found) throw new ApiError(404, `/api/v1/jobs/${job}`, { code: "not_found" });
    return found;
  }

  /** Apply one control: refused for a finished job, recorded, then announced. */
  private change(
    action: string,
    job: string,
    apply: (view: JobView) => JobView,
    item?: string,
  ): JobView {
    const current = this.find(job);
    if (ended(current)) throw new ApiError(409, `/api/v1/jobs/${job}`, { code: "conflict" });
    this.calls.push([action, job, item].filter((part) => part !== undefined).join(" "));
    const next = apply(current);
    this.emit(next);
    return next;
  }

  /** Refuse a retry as the API does, before anything changes: 404, a finished or paused job 409. */
  private guardRetry(job: string): void {
    const current = this.find(job);
    if (ended(current) || current.paused) {
      throw new ApiError(409, `/api/v1/jobs/${job}`, { code: "conflict" });
    }
  }

  /** Put the failed files of a job that match back to queued. */
  private requeue(job: string, pick: (file: JobItem) => boolean): void {
    const files = this.itemsByJob[job] ?? [];
    this.itemsByJob[job] = files.map((file) => {
      if (file.state !== "failed" || !pick(file)) return file;
      const queued: JobItem = {
        item_id: file.item_id,
        name: file.name,
        state: "queued",
        bytes_done: 0,
        bytes_total: file.bytes_total,
      };
      return file.folder === undefined ? queued : { ...queued, folder: file.folder };
    });
  }
}
