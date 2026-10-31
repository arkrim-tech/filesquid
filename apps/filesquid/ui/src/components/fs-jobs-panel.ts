/**
 * `<fs-jobs-panel>`: the header's Jobs button with its count badge and the Jobs panel it opens
 * (UI-1.2, UI-S-028, mockup 19). The button (28 × 28 px, the active tint, the upload icon, a
 * badge with the number of jobs not yet ended) sits in the shell header's `jobs` slot and appears
 * once a job is known; the panel is 440 px wide with a 10 px radius at the right under the header
 * and lists one block per upload job, newest first, then a note that jobs continue in the
 * background and resume after the owner quits (the host owns durability; this panel only
 * observes checkpoints and sends controls).
 *
 * The panel follows a {@link JobsStore}, which follows `job.update` events. A control the API
 * refuses (409, 404) is shown as a line under the job it was sent for, in the error colour, and
 * changes nothing; a host that cannot report jobs (503) shows no button. Opening it and closing
 * it (Escape, the close control or the button) returns focus to the button. Words from the label
 * table, tokens only; the panel is a `role="dialog"` that does not trap focus, because the owner
 * keeps working beside it.
 */
import { importFailure } from "../imports/failure";
import type { JobsStore } from "../imports/jobs-store";
import type { DestinationDirectory } from "../imports/model";
import { formatLabel, type LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { icon } from "./icons";
import { BADGE_STYLE, JOBS_STYLE, REFUSAL_STYLE } from "./import-style";
import { jobBlock, type JobAction } from "./jobs-view";
import { node } from "./library-parts";

/** What the panel needs. */
export interface JobsPanelOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** The jobs and their files. */
  readonly store: JobsStore;
  /** Names the collections of jobs, for their titles. */
  readonly directory: DestinationDirectory;
  /** Whether the browser is online; defaults to `navigator.onLine`. */
  readonly online?: () => boolean;
}

/** Custom element: the header button and the Jobs panel. */
export class FsJobsPanel extends HTMLElement {
  private options?: JobsPanelOptions;
  private opened = false;
  private unavailable = false;
  private readonly alerts = new Map<string, string>();
  private readonly onChange = () => this.render();
  private readonly onKey = (event: KeyboardEvent) => {
    if (event.key === "Escape" && this.opened) this.close();
  };

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the labels and the store and render. Call before or after connecting.
   *
   * @param options Labels, the store and the directory.
   */
  configure(options: JobsPanelOptions): void {
    this.options?.store.removeEventListener("change", this.onChange);
    this.options = options;
    if (this.isConnected) this.connect();
  }

  /** Whether the panel is open. */
  get isOpen(): boolean {
    return this.opened;
  }

  /** Follow the store, read the jobs and render. */
  connectedCallback(): void {
    this.connect();
  }

  /** Stop following the store and the keyboard. */
  disconnectedCallback(): void {
    this.options?.store.removeEventListener("change", this.onChange);
    this.removeEventListener("keydown", this.onKey);
  }

  /** Open the panel. Side effect: moves focus into it. */
  open(): void {
    this.opened = true;
    this.render();
    this.panel()?.querySelector<HTMLElement>(".panel-close")?.focus();
  }

  /** Close the panel and give focus back to the button. */
  close(): void {
    this.opened = false;
    this.render();
    this.shadowRoot?.querySelector<HTMLElement>(".jobs-button")?.focus();
  }

  /** Subscribe to the store and start it; a host without job progress hides the button. */
  private connect(): void {
    const store = this.options?.store;
    if (!store) return;
    store.addEventListener("change", this.onChange);
    this.addEventListener("keydown", this.onKey);
    this.render();
    store.start().catch((error: unknown) => {
      console.warn("FileSquid could not read the upload jobs", error);
      this.unavailable = true;
      this.render();
    });
  }

  /** The panel element, when rendered. */
  private panel(): HTMLElement | null {
    return this.shadowRoot?.querySelector<HTMLElement>(".panel") ?? null;
  }

  /** Draw the button and the panel from the store. */
  private render(): void {
    const options = this.options;
    const root = this.shadowRoot;
    if (!options || !root) return;
    const style = document.createElement("style");
    style.textContent = A11Y_STYLE + CONTROL_STYLE + BADGE_STYLE + REFUSAL_STYLE + JOBS_STYLE;
    const known = options.store.entries.length > 0 && !this.unavailable;
    const focus = this.focusKey(root);
    root.replaceChildren(style, this.button(options, known), this.panelElement(options, known));
    this.restoreFocus(root, focus);
  }

  /** Name the focused control (class, job and action), so a redraw can give focus back to it. */
  private focusKey(root: ShadowRoot): string | undefined {
    const active = root.activeElement as HTMLElement | null;
    if (!active) return undefined;
    const job = active.closest<HTMLElement>("[data-job-id]")?.dataset.jobId ?? "";
    return [active.className, job, active.dataset.action ?? "", active.dataset.item ?? ""].join(
      "|",
    );
  }

  /** Focus the control a redraw replaced, if it is still there. */
  private restoreFocus(root: ShadowRoot, key: string | undefined): void {
    if (key === undefined) return;
    const again = [...root.querySelectorAll<HTMLElement>("button")].find((candidate) => {
      const job = candidate.closest<HTMLElement>("[data-job-id]")?.dataset.jobId ?? "";
      const here = [
        candidate.className,
        job,
        candidate.dataset.action ?? "",
        candidate.dataset.item ?? "",
      ];
      return here.join("|") === key;
    });
    again?.focus();
  }

  /** The header button with its count badge; hidden until a job is known. */
  private button(options: JobsPanelOptions, known: boolean): HTMLButtonElement {
    const { labels, store } = options;
    const count = store.activeCount;
    const button = node("button", "jobs-button");
    button.type = "button";
    button.hidden = !known;
    button.setAttribute("aria-haspopup", "dialog");
    button.setAttribute("aria-expanded", String(this.opened));
    button.setAttribute(
      "aria-label",
      count > 0 ? formatLabel(labels.jobsButtonCount, { count: String(count) }) : labels.jobsButton,
    );
    const badge = node("span", "badge-count", String(count));
    badge.hidden = count === 0;
    button.append(icon("upload", 20), badge);
    button.addEventListener("click", () => (this.opened ? this.close() : this.open()));
    return button;
  }

  /** The panel: title, close control, one block per job or the empty state, and the note. */
  private panelElement(options: JobsPanelOptions, known: boolean): HTMLElement {
    const { labels, store } = options;
    const panel = node("div", "panel");
    panel.hidden = !this.opened || !known;
    panel.setAttribute("role", "dialog");
    panel.setAttribute("aria-label", labels.jobsTitle);
    const head = node("div", "panel-head");
    const close = node("button", "button plain panel-close");
    close.type = "button";
    close.setAttribute("aria-label", labels.jobsClose);
    close.append(icon("close", 20));
    close.addEventListener("click", () => this.close());
    head.append(node("h2", "panel-title", labels.jobsTitle), close);
    const body = node("div", "panel-body");
    body.setAttribute("aria-live", "polite");
    const ctx = {
      labels,
      directory: options.directory,
      locale: document.documentElement.lang || "en",
      act: (action: JobAction) => void this.act(action),
    };
    for (const entry of store.entries) {
      const block = jobBlock(entry, ctx);
      const alert = this.alerts.get(entry.job.id);
      if (alert !== undefined) block.append(node("p", "alert", alert));
      body.append(block);
    }
    if (store.entries.length === 0) body.append(node("p", "empty", labels.jobsEmpty));
    const note = node("div", "panel-note");
    note.append(icon("info", 20), node("p", "", labels.jobsNote));
    panel.append(head, body, note);
    return panel;
  }

  /** Send a control; a refusal is worded under the job and nothing else changes. */
  private async act(action: JobAction): Promise<void> {
    const options = this.options;
    if (!options) return;
    const { store, labels } = options;
    this.alerts.delete(action.job);
    try {
      await this.send(store, action);
    } catch (error) {
      console.warn("FileSquid could not send a job control", error);
      const online = (options.online ?? (() => navigator.onLine))();
      this.alerts.set(action.job, importFailure(error, "job", labels, online));
    }
    this.render();
  }

  /** Route an action to the store. */
  private send(store: JobsStore, action: JobAction): Promise<void> {
    switch (action.kind) {
      case "pause":
        return store.pause(action.job);
      case "resume":
        return store.resume(action.job);
      case "cancel":
        return store.cancel(action.job);
      case "retry-failed":
        return store.retryFailed(action.job);
      case "retry":
        return store.retryItem(action.job, action.item);
    }
  }
}
