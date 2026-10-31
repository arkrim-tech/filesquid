/**
 * The state behind the Jobs panel (UI-1.2, UI-S-028, API-S-037): the upload jobs newest first,
 * each with the files read so far and what they add up to, kept current by one event
 * subscription. Job UI observes checkpoints; it does not own durability: every control is sent to
 * the API and the view it answers replaces the entry, so a refused control changes nothing here.
 *
 * Reads are bounded (AGENTS.md section 22): a job's files are read through at most
 * {@link MAX_JOB_ITEM_PAGES} pages and at most once per `itemRefreshMs` however many
 * `job.update` events arrive (a checkpoint per file would otherwise read the list per file), and
 * only for a job whose files can still change or fail. The store announces a `change` event after
 * each change, and `stop()` drops its subscription, its timers and any read still in flight.
 */
import {
  ITEM_REFRESH_MS,
  JOB_ITEM_PAGE_SIZE,
  MAX_JOB_ITEM_PAGES,
  summariseItems,
  type ItemSummary,
  type JobChange,
  type JobItem,
  type JobSource,
  type JobView,
} from "./model";

/** One job with its files. */
export interface JobEntry {
  /** The job view. */
  readonly job: JobView;
  /** The files read so far, in the job's order (at most five pages). */
  readonly items: readonly JobItem[];
  /** What those files add up to. */
  readonly summary: ItemSummary;
  /** True when the job has more files than were read. */
  readonly itemsCut: boolean;
}

/** What the store needs. */
export interface JobsStoreOptions {
  /** Answers the jobs and their files. */
  readonly source: JobSource;
  /** The least wait between two reads of one job's files; {@link ITEM_REFRESH_MS} by default. */
  readonly itemRefreshMs?: number;
}

/** The footer's progress: files done of files in all, over the running jobs. */
export interface UploadProgress {
  readonly done: number;
  readonly total: number;
}

/** The empty sum of a job's files. */
const NO_FILES: ItemSummary = summariseItems([]);

/** True for a job that has ended. */
function ended(job: JobView): boolean {
  return job.state === "completed" || job.state === "failed" || job.state === "cancelled";
}

/** True for a job whose files can still change, or that failed and may show why. */
function wantsItems(job: JobView): boolean {
  return job.state !== "completed" && job.state !== "cancelled";
}

/** The state of the upload jobs. */
export class JobsStore extends EventTarget {
  private readonly byId = new Map<string, JobEntry>();
  private readonly timers = new Map<string, ReturnType<typeof setTimeout>>();
  private readonly reading = new Set<string>();
  private readonly again = new Set<string>();
  private readonly lastRead = new Map<string, number>();
  private unfollow?: () => void;
  private generation = 0;
  private sorted: readonly JobEntry[] = [];

  /** @param options The source and the read interval. */
  constructor(private readonly options: JobsStoreOptions) {
    super();
  }

  /** The jobs, newest first. */
  get entries(): readonly JobEntry[] {
    return this.sorted;
  }

  /** How many jobs have not ended (running, waiting or paused): the header badge. */
  get activeCount(): number {
    return this.sorted.filter((entry) => !ended(entry.job)).length;
  }

  /** Files done of files in all over the running jobs, or undefined when none runs (footer). */
  get progress(): UploadProgress | undefined {
    const running = this.sorted.filter(({ job }) => job.state === "running" && !job.paused);
    if (running.length === 0) return undefined;
    const done = running.reduce((sum, { job }) => sum + job.done, 0);
    const total = running.reduce((sum, { job }) => sum + (job.total ?? job.done), 0);
    return { done, total };
  }

  /**
   * Follow the job events and read the jobs. Network I/O. Calling it again does nothing.
   *
   * @returns Resolves once the first list is read.
   * @throws ApiError or a network error when the list cannot be read (the store keeps following).
   */
  async start(): Promise<void> {
    if (this.unfollow) return;
    this.unfollow = this.options.source.subscribe((change) => this.onChange(change));
    await this.refresh();
  }

  /** Stop following, drop the timers and ignore any read still in flight. */
  stop(): void {
    this.unfollow?.();
    this.unfollow = undefined;
    this.generation += 1;
    for (const timer of this.timers.values()) clearTimeout(timer);
    this.timers.clear();
    this.reading.clear();
    this.again.clear();
  }

  /**
   * Read the list of jobs again. Network I/O.
   *
   * @returns Resolves once the entries are replaced.
   * @throws ApiError or a network error when the list cannot be read.
   */
  async refresh(): Promise<void> {
    const generation = this.generation;
    const page = await this.options.source.list();
    if (generation !== this.generation) return;
    const kept = new Map(this.byId);
    this.byId.clear();
    for (const job of page.jobs) this.byId.set(job.id, this.entryFor(job, kept.get(job.id)));
    this.announce();
    for (const { job } of this.byId.values()) if (wantsItems(job)) this.scheduleItems(job.id);
  }

  /**
   * Make sure a job the owner just started is listed, before its event arrives. Network I/O.
   *
   * @param jobId The job.
   * @returns Resolves once the list is read again.
   */
  async track(jobId: string): Promise<void> {
    await this.refresh();
    if (!this.byId.has(jobId)) console.warn("FileSquid did not find the upload job it started");
  }

  /** Hold a job. Network I/O. @throws ApiError as the API refuses it. */
  pause(job: string): Promise<void> {
    return this.run(() => this.options.source.pause(job), false);
  }

  /** Clear a job's pause. Network I/O. @throws ApiError as the API refuses it. */
  resume(job: string): Promise<void> {
    return this.run(() => this.options.source.resume(job), false);
  }

  /** Stop a job. Network I/O. @throws ApiError as the API refuses it. */
  cancel(job: string): Promise<void> {
    return this.run(() => this.options.source.cancel(job), false);
  }

  /** Put one failed file back to queued and read the files again. Network I/O. */
  retryItem(job: string, item: string): Promise<void> {
    return this.run(() => this.options.source.retryItem(job, item), true);
  }

  /** Put every failed file back to queued and read the files again. Network I/O. */
  retryFailed(job: string): Promise<void> {
    return this.run(() => this.options.source.retryFailed(job), true);
  }

  /** Send one control and apply the job view it answers; a refusal rejects and changes nothing. */
  private async run(call: () => Promise<JobView>, reread: boolean): Promise<void> {
    const job = await call();
    this.apply(job);
    if (reread) this.scheduleItems(job.id, true);
  }

  /** Apply a change the source announced. */
  private onChange(change: JobChange): void {
    if (change.kind === "job") {
      this.apply(change.job);
      return;
    }
    this.refresh().catch((error: unknown) => {
      console.warn("FileSquid could not read the jobs again", error);
    });
  }

  /** The entry for a job view, keeping the files already read. */
  private entryFor(job: JobView, known?: JobEntry): JobEntry {
    return known ? { ...known, job } : { job, items: [], summary: NO_FILES, itemsCut: false };
  }

  /** Replace or add one job, announce, and read its files when they can change. */
  private apply(job: JobView): void {
    this.byId.set(job.id, this.entryFor(job, this.byId.get(job.id)));
    this.announce();
    if (wantsItems(job)) this.scheduleItems(job.id);
  }

  /** Recompute the sorted list and tell the followers. */
  private announce(): void {
    const byAge = (a: JobEntry, b: JobEntry) =>
      b.job.created.localeCompare(a.job.created) || a.job.id.localeCompare(b.job.id);
    this.sorted = [...this.byId.values()].sort(byAge);
    this.dispatchEvent(new Event("change"));
  }

  /** Read a job's files now, or after the interval since the last read, once. */
  private scheduleItems(id: string, force = false): void {
    if (this.reading.has(id)) {
      this.again.add(id);
      return;
    }
    if (this.timers.has(id)) return;
    const interval = this.options.itemRefreshMs ?? ITEM_REFRESH_MS;
    const last = this.lastRead.get(id);
    const wait = force || last === undefined ? 0 : Math.max(0, interval - (Date.now() - last));
    if (wait === 0) {
      void this.readItems(id);
      return;
    }
    this.timers.set(
      id,
      setTimeout(() => {
        this.timers.delete(id);
        void this.readItems(id);
      }, wait),
    );
  }

  /** Read a job's files through at most {@link MAX_JOB_ITEM_PAGES} pages and keep them. */
  private async readItems(id: string): Promise<void> {
    const generation = this.generation;
    this.reading.add(id);
    this.lastRead.set(id, Date.now());
    try {
      const items: JobItem[] = [];
      let cursor: string | null = null;
      for (let page = 0; page < MAX_JOB_ITEM_PAGES; page += 1) {
        const answer = await this.options.source.items(id, cursor, JOB_ITEM_PAGE_SIZE);
        items.push(...answer.items);
        cursor = answer.next_cursor;
        if (cursor === null) break;
      }
      if (generation === this.generation) this.keepItems(id, items, cursor !== null);
    } catch (error) {
      console.warn("FileSquid could not read a job's files", error);
    } finally {
      if (generation === this.generation) this.finishRead(id);
    }
  }

  /** Keep the files of a job that is still listed. */
  private keepItems(id: string, items: readonly JobItem[], cut: boolean): void {
    const entry = this.byId.get(id);
    if (!entry) return;
    this.byId.set(id, { ...entry, items, summary: summariseItems(items), itemsCut: cut });
    this.announce();
  }

  /** End a read; a change that arrived meanwhile asks for another. */
  private finishRead(id: string): void {
    this.reading.delete(id);
    if (!this.again.delete(id)) return;
    const entry = this.byId.get(id);
    if (entry && wantsItems(entry.job)) this.scheduleItems(id);
  }
}
