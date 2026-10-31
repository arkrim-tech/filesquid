/**
 * `<fs-import-dialog>`: the upload dialog of the default UI (UI-1.2, UI-S-002 **Destination**,
 * UI-S-028, mockups 52 and 18; the default UI's word for import is Upload, owner answer 8). It
 * opens when the New menu announces `upload-files` or `upload-folder` (`fs-new-action`), or when
 * a caller calls {@link FsImportDialog.open}, and walks three steps:
 *
 * 1. **Destination** (mockup 52): the Upload group (files or a folder), the required destination
 *    collection chosen in `<fs-destination-picker>` (collections and the folders inside them
 *    only), and a start control ("Choose files…" or "Choose folder…") that stays disabled, with
 *    its reason, until a destination is chosen. Nothing is sent before then (ADR 041).
 * 2. **Planning**: the start control asks the host to show its own file dialog and plan what the
 *    owner picks. The state says so honestly and shows no percentage, because checking a large
 *    pick takes as long as it takes (DE-1.8). Cancel closes the dialog; a plan that arrives
 *    afterwards is discarded, so nothing stays behind.
 * 3. **Review** (mockup 18): the plan the API answered (files found, new, already in the library,
 *    bytes to copy), the planned files, the destination with Change…, space and the integrity
 *    note. The job starts only on **Upload {count} files**; Cancel and Escape discard the plan and
 *    change nothing.
 *
 * Starting dispatches a bubbling, composed `fs-import-started` event (`detail.jobId`) so the Jobs
 * panel shows the job. The element calls only its {@link ImportSource} and reads the destination
 * through the {@link DestinationDirectory} port (navigation node handles are opaque, API-S-021).
 * A refusal is an error card (mockup 48) worded from the answer's status and step, never from a
 * message the server wrote.
 */
import { newOperationId } from "../api/commands";
import {
  DestinationRequiredError,
  DestinationUnresolvedError,
  importFailure,
  type ImportStep,
} from "../imports/failure";
import {
  MAX_PLAN_ITEM_PAGES,
  destinationFolder,
  type DestinationDirectory,
  type ImportPlan,
  type ImportSource,
  type ImportSourceKind,
  type PlanItem,
  type ResolvedDestination,
} from "../imports/model";
import type { NavigationSource } from "../navigation/model";
import type { LabelTable } from "../theme/labels";
import { A11Y_STYLE } from "./a11y-style";
import { CONTROL_STYLE } from "./control-style";
import { showDialog, type OpenDialog } from "./drive-dialogs";
import type { DestinationChoice } from "./fs-destination-picker";
import { NEW_ACTION_EVENT, type NewAction } from "./fs-new-menu";
import {
  destinationStep,
  fillPlanItems,
  planningBlock,
  reviewStep,
  showStart,
  type DestinationStep,
  type ReviewContext,
  type ReviewStep,
} from "./import-steps";
import { BADGE_STYLE, IMPORT_STYLE, REFUSAL_STYLE } from "./import-style";
import { refusalCard } from "./refusal-card";

/** Name of the event dispatched when an upload job started. */
export const IMPORT_STARTED_EVENT = "fs-import-started";

/** What the dialog needs. */
export interface ImportDialogOptions {
  /** The active label table. */
  readonly labels: LabelTable;
  /** Plans, lists, starts and discards an upload. */
  readonly source: ImportSource;
  /** The navigation listing the destination tree reads. */
  readonly navigation: NavigationSource;
  /** Names the library and collection of a chosen destination. */
  readonly directory: DestinationDirectory;
  /** Whether the browser is online; defaults to `navigator.onLine`. */
  readonly online?: () => boolean;
}

/** What opening the dialog may preset. */
export interface ImportRequest {
  /** The source the Upload group starts on; files by default. */
  readonly source?: ImportSourceKind;
  /** The node handle of the collection or folder shown, chosen as the destination at the start. */
  readonly destination?: string;
}

/** One walk through the dialog: what the owner chose and what the host answered. */
interface Session {
  id: number;
  source: ImportSourceKind;
  choice?: DestinationChoice;
  resolved?: ResolvedDestination;
  operationId?: string;
  plan?: ImportPlan;
  opener: Element | null;
}

const STYLE = `
  :host { display: contents; }
  .layer { color: var(--fs-on-surface); font-family: var(--fs-font-sans); }
`;

/** Custom element that walks the owner through an upload. */
export class FsImportDialog extends HTMLElement {
  private options?: ImportDialogOptions;
  private layer?: HTMLElement;
  private shown?: OpenDialog;
  private session?: Session;
  private counter = 0;
  private readonly onAction = (event: Event) => this.onNewAction(event as CustomEvent);

  constructor() {
    super();
    this.attachShadow({ mode: "open" });
  }

  /**
   * Set the labels and sources and render the dialog layer. Call before or after connecting.
   *
   * @param options Labels, the import source, the navigation source and the directory.
   */
  configure(options: ImportDialogOptions): void {
    this.options = options;
    const style = document.createElement("style");
    style.textContent =
      A11Y_STYLE + CONTROL_STYLE + BADGE_STYLE + REFUSAL_STYLE + IMPORT_STYLE + STYLE;
    this.layer = document.createElement("div");
    this.layer.className = "layer";
    (this.shadowRoot as ShadowRoot).replaceChildren(style, this.layer);
  }

  /** Whether a dialog is open. */
  get isOpen(): boolean {
    return this.session !== undefined;
  }

  /** Listen for the New menu's upload items. */
  connectedCallback(): void {
    document.addEventListener(NEW_ACTION_EVENT, this.onAction);
  }

  /** Stop listening and close an open dialog, discarding its plan. */
  disconnectedCallback(): void {
    document.removeEventListener(NEW_ACTION_EVENT, this.onAction);
    void this.cancel();
  }

  /**
   * Open the dialog on its destination step. Does nothing while one is open. Side effects: shows
   * the dialog and, when `destination` is given, reads the tree down to it.
   *
   * @param request The source and destination to start with.
   */
  open(request: ImportRequest = {}): void {
    if (this.session || !this.options) return;
    const opener = (this.getRootNode() as Document).activeElement;
    this.counter += 1;
    this.session = { id: this.counter, source: request.source ?? "files", opener };
    const step = this.showDestination();
    if (request.destination !== undefined) void step.picker.preselect(request.destination);
  }

  /** The New menu asked for an upload. */
  private onNewAction(event: CustomEvent<{ action: NewAction; destination?: string }>): void {
    const { action, destination } = event.detail ?? {};
    if (action !== "upload-files" && action !== "upload-folder") return;
    this.open({ source: action === "upload-files" ? "files" : "folder", destination });
  }

  /** Show a dialog in place of the one shown, with Escape meaning Cancel. */
  private mount(dialog: HTMLElement, focus: HTMLElement): void {
    this.shown?.close();
    dialog.addEventListener("keydown", (event) => {
      if (event.key !== "Escape") return;
      event.stopImmediatePropagation();
      event.preventDefault();
      void this.cancel();
    });
    this.shown = showDialog(this.layer as HTMLElement, dialog, null, focus);
  }

  /** Put an error card at the top of a step's body, replacing the one there. */
  private refuse(body: HTMLElement, error: unknown, step: ImportStep): void {
    const options = this.options as ImportDialogOptions;
    const online = (options.online ?? (() => navigator.onLine))();
    const reason = importFailure(error, step, options.labels, online);
    body.querySelector(".refusal")?.remove();
    body.prepend(refusalCard(options.labels, options.labels.importRefusedTitle, reason));
  }

  /** Build and show the destination step for the session. */
  private showDestination(refusal?: unknown): DestinationStep {
    const { labels, navigation } = this.options as ImportDialogOptions;
    const session = this.session as Session;
    const step = destinationStep(labels, session.source);
    step.picker.configure({ labels, source: navigation, variant: "import" });
    this.restoreChoice(step, session);
    this.wireDestination(step, session);
    this.mount(step.dialog, session.choice ? step.start : step.field);
    if (refusal !== undefined) this.refuse(step.body, refusal, "plan");
    return step;
  }

  /** Show the destination chosen before, when the owner came back to this step. */
  private restoreChoice(step: DestinationStep, session: Session): void {
    const { labels } = this.options as ImportDialogOptions;
    if (session.choice) showStart(step, labels, session.source, session.choice.trail);
  }

  /** Wire the destination step's controls. */
  private wireDestination(step: DestinationStep, session: Session): void {
    const { labels } = this.options as ImportDialogOptions;
    step.picker.addEventListener("fs-destination-change", (event) => {
      session.choice = ((event as CustomEvent).detail as DestinationChoice | null) ?? undefined;
      showStart(step, labels, session.source, session.choice?.trail);
      step.body.querySelector(".refusal")?.remove();
    });
    for (const kind of ["files", "folder"] as const) {
      step.radios[kind].addEventListener("change", () => {
        session.source = kind;
        showStart(step, labels, kind, session.choice?.trail);
      });
    }
    step.start.addEventListener("click", () => void this.plan(step, session));
    step.cancel.addEventListener("click", () => void this.cancel());
    step.close.addEventListener("click", () => void this.cancel());
  }

  /** Resolve the destination, then ask the host for the file dialog and the plan. */
  private async plan(step: DestinationStep, session: Session): Promise<void> {
    const { source, directory } = this.options as ImportDialogOptions;
    try {
      if (!session.choice) throw new DestinationRequiredError();
      session.resolved = await directory.resolve(session.choice);
      if (!session.resolved) throw new DestinationUnresolvedError();
    } catch (error) {
      this.refuse(step.body, error, "plan");
      return;
    }
    this.showPlanning(step);
    session.operationId ??= newOperationId();
    try {
      const answer = await source.plan({
        libraryId: session.resolved.libraryId,
        collectionId: session.resolved.collectionId,
        folder: destinationFolder(session.choice.trail),
        source: session.source,
        operationId: session.operationId,
      });
      await this.onPlanned(session, answer);
    } catch (error) {
      this.afterFailedPlan(session, error);
    }
  }

  /** Show the planning state in the destination step: controls off, no percentage. */
  private showPlanning(step: DestinationStep): void {
    const { labels } = this.options as ImportDialogOptions;
    step.start.disabled = true;
    step.body.querySelector(".refusal")?.remove();
    step.body.querySelector(".planning")?.remove();
    step.body.append(planningBlock(labels));
    step.dialog.setAttribute("aria-busy", "true");
  }

  /** Go on with what the host answered, unless the owner left meanwhile. */
  private async onPlanned(
    session: Session,
    answer: Awaited<ReturnType<ImportSource["plan"]>>,
  ): Promise<void> {
    session.operationId = undefined;
    const live = this.session?.id === session.id;
    if (answer.kind === "started") {
      if (live) this.announceStarted(answer.jobId);
      return;
    }
    if (answer.kind === "planned" && !live) {
      await this.discard(answer.plan.plan_id);
      return;
    }
    if (!live) return;
    if (answer.kind === "cancelled") this.showDestination();
    else this.showReview(session, answer.plan);
  }

  /** A plan request failed: back to the destination step with the refusal, unless left. */
  private afterFailedPlan(session: Session, error: unknown): void {
    const status = (error as { status?: number }).status ?? 0;
    // A lost answer keeps its operation ID, so the retry is answered with the open plan.
    if (status > 0) session.operationId = undefined;
    if (this.session?.id !== session.id) return;
    console.warn("FileSquid could not plan the upload", error);
    this.showDestination(error);
  }

  /** Discard a plan the owner no longer wants; a failure is logged, not shown. */
  private async discard(planId: string): Promise<void> {
    try {
      await (this.options as ImportDialogOptions).source.discard(planId);
    } catch (error) {
      console.warn("FileSquid could not discard the upload plan", error);
    }
  }

  /** Build and show the review of a plan, and read the planned files. */
  private showReview(session: Session, plan: ImportPlan): void {
    const { labels } = this.options as ImportDialogOptions;
    session.plan = plan;
    const ctx: ReviewContext = {
      labels,
      plan,
      trail: session.choice?.trail ?? [],
      locale: document.documentElement.lang || "en",
    };
    const step = reviewStep(ctx);
    step.start.addEventListener("click", () => void this.start(step, session));
    step.cancel.addEventListener("click", () => void this.cancel());
    step.close.addEventListener("click", () => void this.cancel());
    step.change.addEventListener("click", () => void this.change(session));
    this.mount(step.dialog, step.start.disabled ? step.cancel : step.start);
    void this.loadPlanItems(step, ctx, session);
  }

  /** Read the planned files, at most {@link MAX_PLAN_ITEM_PAGES} pages, into the Files table. */
  private async loadPlanItems(
    step: ReviewStep,
    ctx: ReviewContext,
    session: Session,
  ): Promise<void> {
    const { source } = this.options as ImportDialogOptions;
    const items: PlanItem[] = [];
    try {
      let cursor: string | null = null;
      for (let page = 0; page < MAX_PLAN_ITEM_PAGES; page += 1) {
        const answer = await source.planItems(ctx.plan.plan_id, cursor);
        items.push(...answer.items);
        cursor = answer.next_cursor;
        if (cursor === null) break;
      }
    } catch (error) {
      console.warn("FileSquid could not read the planned files", error);
      if (this.session?.id === session.id) this.refuse(step.body, error, "items");
    }
    if (this.session?.id === session.id) fillPlanItems(step, items, ctx);
  }

  /** Start the job of the plan; a refusal stays in the review and the plan stays open. */
  private async start(step: ReviewStep, session: Session): Promise<void> {
    const { source } = this.options as ImportDialogOptions;
    const plan = session.plan as ImportPlan;
    step.start.disabled = true;
    try {
      const jobId = await source.start(plan.plan_id);
      this.announceStarted(jobId);
    } catch (error) {
      console.warn("FileSquid could not start the upload", error);
      this.refuse(step.body, error, "start");
      step.start.disabled = false;
    }
  }

  /** Back to the destination step to change it: the plan is discarded first. */
  private async change(session: Session): Promise<void> {
    const planId = session.plan?.plan_id;
    session.plan = undefined;
    this.showDestination();
    if (planId !== undefined) await this.discard(planId);
  }

  /**
   * Close the dialog and, when a plan is open, discard it: Cancel changes nothing. Side effects:
   * closes the dialog and sends the discard.
   *
   * @returns Resolves once the dialog is closed and the discard answered.
   */
  async cancel(): Promise<void> {
    const planId = this.session?.plan?.plan_id;
    this.finish();
    if (planId !== undefined) await this.discard(planId);
  }

  /** Tell the jobs panel a job started, and close. */
  private announceStarted(jobId: string): void {
    this.finish();
    this.dispatchEvent(
      new CustomEvent(IMPORT_STARTED_EVENT, { detail: { jobId }, bubbles: true, composed: true }),
    );
  }

  /** Close the dialog, give focus back to what opened it and forget the session. */
  private finish(): void {
    const opener = this.session?.opener;
    this.session = undefined;
    this.shown?.close();
    this.shown = undefined;
    if (opener instanceof HTMLElement) opener.focus();
  }
}
