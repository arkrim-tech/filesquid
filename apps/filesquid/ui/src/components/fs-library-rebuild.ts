/**
 * `<fs-library-rebuild>`: the page that follows a library's rebuild from its files as a durable
 * job (UI-1.3, UI-S-027, API-S-025, API-S-037; mockup
 * `resources/ui-mockups/v3/04-recover-library.svg`). Two flows reach it: Recover from a copy
 * (`#/library/<id>/recover/<job>`, drawn without the shell) and the library page's "Rebuild
 * catalog from files" (`#/library/<id>/rebuild/<job>`, inside the shell). It shows one raised
 * card: the job's name, its state chip (running, pausing, paused, cancelling, finished, failed
 * or cancelled), the progress in objects ("{done} of {total} objects read · started {time} ·
 * resumes from its checkpoint after you quit"), a progress bar, and Pause (Resume while paused)
 * and Cancel over the job routes. When the job has finished it shows its report: the objects
 * read, those not readable and each object with validation errors, listed under VALIDATION
 * ERRORS SO FAR. The host reports the validation errors only when the job completes (the job's
 * `result` is null before then), so the list appears with the report.
 *
 * The page follows the job live through `job.update` events (the event carries no report, so a
 * completed job is read again for it) and re-reads an active job every `refreshMs` (at most
 * {@link MAX_REFRESHES} times) in case an event was missed. Nothing in the copy is repaired or
 * deleted. No path, job ID or library ID is shown. Words come from the label table, styling
 * from tokens.
 */
import { focusElement } from "../a11y/focus";
import { libraryFailure, type LibraryAction } from "../libraries/errors";
import {
  jobChip,
  progressFraction,
  rebuildProgressText,
  rebuildSummary,
} from "../libraries/format";
import {
  LIBRARY_ROUTE,
  libraryHref,
  type JobView,
  type RebuildMode,
  type RebuildReport,
} from "../libraries/model";
import { formatLabel, type LabelTable } from "../theme/labels";
import { RETRY_EVENT } from "./fs-states";
import { icon } from "./icons";
import {
  Feedback,
  REBUILD_REFRESH_MS,
  StepFailure,
  actionButton,
  browserOnline,
  link,
  node,
  pageStyle,
  step,
  type LibraryPageOptions,
} from "./library-parts";

/** What the rebuild page needs: the labels, the job routes and the optional online check. */
export type RebuildPageOptions = Pick<
  LibraryPageOptions,
  "labels" | "jobs" | "online" | "refreshMs" | "version"
>;

/** The kind of the durable job this page follows (API-S-025). */
const REBUILD_KIND = "library.rebuild";

/** Most safety re-reads of one job (AGENTS.md section 22); events still update the page. */
export const MAX_REFRESHES = 720;

/**
 * The page's rules (UI-S-024, UI-S-027, mockup 04): the Recover title 34 px (30 px for a
 * library's own rebuild), a raised card of radius 8 px holding the 15 px 600 job name, a 22 px
 * chip of radius 4 px, the 13 px progress line and a 4 px bar with a 2 px radius on an
 * `--fs-outline` track, Pause and Cancel 92 x 36 px at the card's right, error lines in 1 px
 * `--fs-error` boxes of radius 8 px with mono text, and the note and All libraries below.
 */
const REBUILD_STYLE = `
  .rebuild { box-sizing: border-box; max-inline-size: 65rem; gap: 0; }
  .rebuild[data-mode="recover"] { margin: 0 auto; padding-block-start: calc(var(--fs-space-4) + var(--fs-space-2)); }
  .rebuild .crumb { display: flex; align-items: center; gap: var(--fs-space-1); margin: 0; color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  .rebuild .crumb a { color: var(--fs-on-surface-variant); text-decoration: none; }
  .rebuild .crumb a:hover { color: var(--fs-on-surface); text-decoration: underline; }
  .rebuild .crumb .icon { color: var(--fs-icon-muted); }
  .rebuild .crumb .current { color: var(--fs-on-surface); font-weight: 500; }
  .rebuild h2 { margin-block-start: var(--fs-space-3); }
  .rebuild[data-mode="recover"] h2 { font-size: var(--fs-font-size-page-large); letter-spacing: -0.01em; }
  .rebuild .lead { margin-block-start: var(--fs-space-2); color: var(--fs-on-surface); font-size: var(--fs-font-size-state); }
  .rebuild .feedback { margin-block-start: var(--fs-space-3); }
  .job-slot, .errors-host { display: contents; }
  .job { grid-template-columns: minmax(0, 1fr) auto; align-items: start; column-gap: 2.75rem; row-gap: 0; box-sizing: border-box; margin-block-start: var(--fs-space-3); padding: 0.875rem 1.25rem 1.625rem var(--fs-space-4); border-radius: var(--fs-radius); background: var(--fs-surface-raised); }
  .job .info { display: grid; min-inline-size: 0; }
  .job h3 { margin: 0; font-family: var(--fs-font-sans); font-size: var(--fs-font-size-state); font-weight: 600; letter-spacing: normal; text-transform: none; line-height: 1.5; color: var(--fs-on-surface); }
  .job .status-row { display: flex; align-items: center; gap: var(--fs-space-2); margin-block-start: var(--fs-space-2); }
  .job .chip { box-sizing: border-box; flex: none; block-size: 1.375rem; border-radius: var(--fs-radius-chip); }
  .job .progress-text { color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  .bar { box-sizing: border-box; block-size: 0.25rem; margin-block-start: var(--fs-space-3); border-radius: 0.125rem; background: var(--fs-outline); overflow: hidden; }
  .bar .fill { block-size: 100%; inline-size: var(--progress, 0%); border-radius: 0.125rem; background: var(--fs-primary); }
  .bar[data-indeterminate="true"] .fill { inline-size: 30%; }
  .job .controls { display: flex; gap: var(--fs-space-2); }
  .job .controls .button { inline-size: 5.75rem; padding-inline: 0; }
  .report { display: grid; gap: var(--fs-space-2); margin-block-start: var(--fs-space-3); font-size: var(--fs-font-size-ui); color: var(--fs-on-surface); }
  .report:empty { display: none; }
  .report a { justify-self: start; }
  .errors { display: grid; gap: var(--fs-space-2); margin-block-start: var(--fs-space-4); }
  .errors .eyebrow { margin: 0; }
  .errors ul { gap: var(--fs-space-2); }
  .errors li { display: flex; align-items: center; gap: 0.75rem; box-sizing: border-box; padding: var(--fs-space-3) var(--fs-space-4) var(--fs-space-3) 1.25rem; border: 1px solid var(--fs-error); border-radius: var(--fs-radius); background: var(--fs-surface); color: var(--fs-on-surface); font-family: var(--fs-font-mono); font-size: var(--fs-font-size-small); }
  .errors li .icon { flex: none; color: var(--fs-error); }
  .rebuild .note { margin-block-start: var(--fs-space-4); color: var(--fs-on-surface-variant); font-size: var(--fs-font-size-small); }
  .rebuild .back { justify-self: start; margin-block-start: var(--fs-space-4); inline-size: 8rem; padding-inline: 0; }
  .rebuild[data-mode="rebuild"] .back { inline-size: auto; padding-inline: var(--fs-space-3); }
  .rebuild .app-footer { margin-block-start: var(--fs-space-5); }
`;

/** Custom element that shows one rebuild job's page. */
export class FsLibraryRebuild extends HTMLElement {
  private options?: RebuildPageOptions;
  private modeValue: RebuildMode = "recover";
  private library?: string;
  private job?: string;
  private view?: JobView;
  private generation = 0;
  private busy = false;
  private refreshes = 0;
  private renderQueued = false;
  /** True once a completed job was read again for its report, so that is done once. */
  private reportRead = false;
  /** The newest read started, so an older answer that arrives late is dropped. */
  private readSeq = 0;
  private stopWatching?: () => void;
  private timer?: ReturnType<typeof setTimeout>;
  private feedback?: Feedback;
  private jobSlot?: HTMLElement;
  private reportBox?: HTMLElement;
  private errorsBox?: HTMLElement;

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.addEventListener(RETRY_EVENT, (event) => {
      event.stopPropagation();
      void this.load(this.generation);
    });
  }

  /**
   * Set the labels and the job routes, then render. Call before or after connecting.
   *
   * @param options Labels, the job source, the refresh interval and the online check.
   */
  configure(options: RebuildPageOptions): void {
    this.options = options;
    this.queueRender();
  }

  /** Which flow the page belongs to: Recover from a copy, or a library's own Rebuild. */
  get mode(): RebuildMode {
    return this.modeValue;
  }

  /** Show the page of another flow. */
  set mode(mode: RebuildMode) {
    this.modeValue = mode;
    this.queueRender();
  }

  /** The ID of the library whose catalog is rebuilt. */
  get libraryId(): string | undefined {
    return this.library;
  }

  /** Show the page of another library. */
  set libraryId(id: string | undefined) {
    this.library = id;
    this.queueRender();
  }

  /** The ID of the job followed. */
  get jobId(): string | undefined {
    return this.job;
  }

  /** Follow another job. */
  set jobId(id: string | undefined) {
    this.job = id;
    this.queueRender();
  }

  /** Render when attached. */
  connectedCallback(): void {
    this.queueRender();
  }

  /** Stop watching and re-reading once the page is gone. */
  disconnectedCallback(): void {
    this.generation += 1;
    this.stop();
  }

  /** Render once for a burst of property changes (the route sets three after attaching). */
  private queueRender(): void {
    if (this.renderQueued || !this.isConnected) return;
    this.renderQueued = true;
    queueMicrotask(() => {
      this.renderQueued = false;
      this.render();
    });
  }

  /** Stop the watch and the safety re-read. */
  private stop(): void {
    this.stopWatching?.();
    this.stopWatching = undefined;
    clearTimeout(this.timer);
    this.timer = undefined;
  }

  /** Build the page and read the job. */
  private render(): void {
    const options = this.options;
    if (!this.shadowRoot || !this.isConnected || !options) return;
    if (this.library === undefined || this.job === undefined) return;
    this.generation += 1;
    this.stop();
    this.busy = false;
    this.refreshes = 0;
    this.reportRead = false;
    this.readSeq = 0;
    this.view = undefined;
    const { labels } = options;
    const recover = this.modeValue === "recover";
    this.feedback = new Feedback(labels);
    this.jobSlot = node("div", "job-slot");
    this.reportBox = node("div", "report");
    this.reportBox.setAttribute("role", "status");
    this.reportBox.tabIndex = -1;
    this.errorsBox = node("div", "errors-host");
    const page = node("div", "page rebuild");
    page.dataset.mode = this.modeValue;
    page.append(
      this.crumb(labels, recover),
      node("h2", undefined, recover ? labels.libraryRecoverTitle : labels.libraryRebuild),
      node("p", "lead", recover ? labels.libraryRecoverHelp : labels.libraryRebuildHelp),
      this.feedback.root,
      this.jobSlot,
      this.reportBox,
      this.errorsBox,
      ...(recover ? [node("p", "note", labels.libraryRecoverNote)] : []),
      this.backLink(labels, recover),
      ...(options.version && recover ? [this.footer(labels, options.version)] : []),
    );
    this.shadowRoot.replaceChildren(pageStyle(REBUILD_STYLE), page);
    void this.load(this.generation);
  }

  /** "Libraries › {this page}". */
  private crumb(labels: LabelTable, recover: boolean): HTMLElement {
    const crumb = node("nav", "crumb");
    crumb.setAttribute("aria-label", labels.libraries);
    const current = node(
      "span",
      "current",
      recover ? labels.libraryRecoverTitle : labels.libraryRebuild,
    );
    current.setAttribute("aria-current", "page");
    crumb.append(link("", labels.libraries, LIBRARY_ROUTE), icon("chevron", 14), current);
    return crumb;
  }

  /** All libraries (Recover), or back to the library (its own Rebuild). */
  private backLink(labels: LabelTable, recover: boolean): HTMLElement {
    const text = recover ? labels.libraryStartBack : labels.libraryBack;
    const href = recover ? LIBRARY_ROUTE : libraryHref(this.library as string);
    return link("button secondary back", text, href);
  }

  /** The footer line `appFooter`. */
  private footer(labels: LabelTable, version: string): HTMLElement {
    return node("p", "app-footer", formatLabel(labels.appFooter, { version }));
  }

  /**
   * Read the job, show it, and follow it. Network I/O through the job source. A page without
   * a job source says it is unavailable.
   */
  private async load(generation: number): Promise<void> {
    const options = this.options;
    const feedback = this.feedback;
    const jobs = options?.jobs;
    if (!options || !feedback) return;
    if (!jobs) {
      feedback.fail({ kind: "state", state: { kind: "unavailable" } });
      return;
    }
    feedback.loading();
    try {
      const seq = ++this.readSeq;
      const view = await step("rebuild", () => jobs.job(this.job as string));
      if (generation !== this.generation || seq !== this.readSeq) return;
      feedback.clear();
      this.follow(view);
    } catch (error) {
      if (generation === this.generation) this.fail(error);
    }
  }

  /** Show the job read, then watch it, unless it is not a rebuild. */
  private follow(view: JobView): void {
    const { labels } = this.options as RebuildPageOptions;
    if (view.kind !== REBUILD_KIND) {
      this.feedback?.fail({ kind: "message", text: labels.libraryNotFound });
      return;
    }
    this.apply(view);
    if (this.stopWatching) return;
    const jobs = this.options?.jobs;
    const generation = this.generation;
    this.stopWatching = jobs?.watch((event) => {
      if (generation !== this.generation) return;
      if (event.kind === "resync") void this.reread(generation);
      else if (event.job.id === this.view?.id) this.apply(event.job);
    });
  }

  /** Read the job again and show it. Network I/O through the job source. */
  private async reread(generation: number): Promise<void> {
    const jobs = this.options?.jobs;
    if (!jobs || this.job === undefined) return;
    try {
      const seq = ++this.readSeq;
      const view = await step("rebuild", () => jobs.job(this.job as string));
      if (generation === this.generation && seq === this.readSeq) this.apply(view);
    } catch (error) {
      if (generation !== this.generation) return;
      this.fail(error);
      // A failed read must not end the safety re-reads of an active job.
      if (this.view) this.scheduleRefresh(this.view);
    }
  }

  /** Show a job view: the card, the report and errors when it is final, the next re-read. */
  private apply(view: JobView): void {
    // A valid job view means the page works again: an earlier failed read no longer applies.
    this.feedback?.clear();
    const previous = this.view;
    this.view =
      view.result === null && previous?.result ? { ...view, result: previous.result } : view;
    this.paintCard(this.view);
    this.paintReport(this.view);
    this.scheduleRefresh(this.view);
    const completedWithoutReport = view.state === "completed" && this.view.result === null;
    if (!completedWithoutReport || this.reportRead) return;
    this.reportRead = true;
    void this.reread(this.generation);
  }

  /** Re-read an active job after `refreshMs`, at most {@link MAX_REFRESHES} times. */
  private scheduleRefresh(view: JobView): void {
    const interval = this.options?.refreshMs ?? REBUILD_REFRESH_MS;
    const active = view.state === "pending" || view.state === "running";
    if (!active || interval <= 0 || this.refreshes >= MAX_REFRESHES) {
      clearTimeout(this.timer);
      this.timer = undefined;
      return;
    }
    // A timer already waiting stays: frequent updates must not postpone the re-read for ever.
    if (this.timer !== undefined) return;
    const generation = this.generation;
    this.timer = setTimeout(() => {
      this.timer = undefined;
      this.refreshes += 1;
      void this.reread(generation);
    }, interval);
  }

  /** Draw the card: name, chip, progress line, bar and the controls. */
  private paintCard(view: JobView): void {
    const { labels } = this.options as RebuildPageOptions;
    const slot = this.jobSlot;
    if (!slot) return;
    const chip = jobChip(view, labels);
    const badge = node("span", `chip ${chip.tone}`, chip.text);
    const line = node("div", "status-row");
    line.append(badge, node("span", "progress-text", rebuildProgressText(view, labels)));
    const info = node("div", "info");
    info.append(node("h3", undefined, labels.libraryRebuild), line, this.bar(view, labels));
    const card = node("section", "job");
    card.append(info, ...this.controls(view, labels));
    slot.replaceChildren(card);
  }

  /** The progress bar with its numbers for assistive technology. */
  private bar(view: JobView, labels: LabelTable): HTMLElement {
    const bar = node("div", "bar");
    bar.setAttribute("role", "progressbar");
    bar.setAttribute("aria-label", labels.libraryRebuild);
    bar.setAttribute("aria-valuemin", "0");
    const fraction = progressFraction(view);
    const fill = node("div", "fill");
    if (fraction === undefined) {
      bar.dataset.indeterminate = "true";
    } else {
      bar.setAttribute("aria-valuemax", String(view.total));
      bar.setAttribute("aria-valuenow", String(view.done));
      fill.style.setProperty("--progress", `${(fraction * 100).toFixed(2)}%`);
    }
    bar.append(fill);
    return bar;
  }

  /** Pause or Resume, and Cancel, while the job is not final and no cancel is asked. */
  private controls(view: JobView, labels: LabelTable): HTMLElement[] {
    const final =
      view.state === "completed" || view.state === "failed" || view.state === "cancelled";
    if (final || view.control === "cancel") return [];
    const toggle = view.paused ? "resume" : "pause";
    const first = actionButton(toggle, view.paused ? labels.jobResume : labels.jobPause);
    if (!view.paused) first.prepend(icon("pause", 16));
    const cancel = actionButton("cancel", labels.jobCancel);
    for (const button of [first, cancel]) {
      button.addEventListener("click", () => void this.control(button.dataset.action as string));
    }
    const box = node("div", "controls");
    box.append(first, cancel);
    return [box];
  }

  /** The report of a final job: its sentence, the link on, and the validation errors. */
  private paintReport(view: JobView): void {
    const { labels } = this.options as RebuildPageOptions;
    const box = this.reportBox;
    this.errorsBox?.replaceChildren();
    if (!box) return;
    if (view.state === "completed" && view.result) {
      box.replaceChildren(...this.finished(view.result, labels));
      this.errorsBox?.replaceChildren(...errorSection(view.result, labels));
    } else if (view.state === "cancelled") {
      box.replaceChildren(node("p", undefined, labels.libraryRebuildCancelled));
    } else if (view.state === "failed") {
      console.warn("FileSquid rebuild job failed", view.error_code);
      box.replaceChildren(node("p", undefined, labels.libraryFailed));
    } else {
      box.replaceChildren();
    }
  }

  /** The sentences and the link of a finished rebuild. */
  private finished(report: RebuildReport, labels: LabelTable): Node[] {
    const recover = this.modeValue === "recover";
    const nodes: Node[] = [node("p", undefined, rebuildSummary(report, labels))];
    if (!recover) return nodes;
    nodes.push(
      node("p", undefined, labels.libraryRecoveredOpen),
      link("button secondary", labels.libraryBack, libraryHref(this.library as string)),
    );
    return nodes;
  }

  /**
   * Ask the job to pause, resume or be cancelled, then show the job as the host answers. One
   * control at a time. Network I/O through the job source.
   */
  private async control(which: string): Promise<void> {
    const jobs = this.options?.jobs;
    const id = this.job;
    if (!jobs || id === undefined || this.busy) return;
    const calls: Record<string, () => Promise<JobView>> = {
      pause: () => jobs.pause(id),
      resume: () => jobs.resume(id),
      cancel: () => jobs.cancel(id),
    };
    const call = calls[which];
    if (!call) return;
    const generation = this.generation;
    this.busy = true;
    this.setBusy(true);
    try {
      const view = await step("rebuild", call);
      if (generation !== this.generation) return;
      this.feedback?.clear();
      this.apply(view);
      this.restoreFocus();
    } catch (error) {
      if (generation === this.generation) this.fail(error);
    } finally {
      if (generation === this.generation) {
        this.busy = false;
        this.setBusy(false);
      }
    }
  }

  /** Mark the controls busy, so a second press sends nothing. */
  private setBusy(busy: boolean): void {
    for (const button of this.jobSlot?.querySelectorAll<HTMLButtonElement>("button") ?? []) {
      if (busy) button.setAttribute("aria-disabled", "true");
      else button.removeAttribute("aria-disabled");
    }
  }

  /** The pressed control was replaced: move focus to its counterpart, or to the report. */
  private restoreFocus(): void {
    const next = this.jobSlot?.querySelector<HTMLElement>("button");
    focusElement(next ?? (this.reportBox as HTMLElement));
  }

  /** Show a failed step and log it to the console. */
  private fail(error: unknown): void {
    const options = this.options as RebuildPageOptions;
    const tagged = error instanceof StepFailure;
    const action: LibraryAction = tagged ? error.action : "rebuild";
    const reason = tagged ? error.reason : error;
    console.warn(`FileSquid library step ${action} did not finish`, reason);
    const online = (options.online ?? browserOnline)();
    this.feedback?.fail(libraryFailure(reason, action, options.labels, online));
  }
}

/** The validation-error section: the objects with errors and the objects that could not be read. */
function errorSection(report: RebuildReport, labels: LabelTable): Node[] {
  const lines = [
    ...report.objects
      .filter((object) => object.error_codes.length > 0)
      .map((object) =>
        formatLabel(labels.libraryRebuiltInvalid, {
          object: object.object_id,
          codes: object.error_codes.join(", "),
        }),
      ),
    ...report.unreadable.map((item) =>
      formatLabel(labels.libraryUnreadable, { code: item.error_code }),
    ),
  ];
  if (lines.length === 0) return [];
  const list = node("ul");
  list.append(
    ...lines.map((text) => {
      const line = node("li", undefined, text);
      line.prepend(icon("error", 16));
      return line;
    }),
  );
  const section = node("section", "errors");
  section.append(node("p", "eyebrow", labels.libraryRebuildErrorsTitle), list);
  return [section];
}
