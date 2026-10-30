/**
 * `<fs-library-detail>`: one library's page, `#/library/<id>` (UI-1.1, SRS-F-001, API-S-023,
 * API-S-025; mockups `08-library-menu.svg`, `09-copy-library.svg`). It shows what the host says
 * about the library (its ID, the OCFL version its storage root declares, its object count) and
 * never a path; offers "Copy library to..." (a destination chosen with purpose
 * `copy_destination`, then the snapshot route; Retry copies to the destination already chosen)
 * and "Rebuild catalog from files" (reads every
 * object from its files, writes nothing); and says, with a link to the repair offer
 * `#/library/<id>/repair`, when files in some objects can be read but not changed (W010).
 * Later pages (such as the import UI) take the open library's ID from this route.
 */
import { RETRY_EVENT } from "./fs-states";
import { formatSize } from "../files/model";
import { libraryFailure, type LibraryAction } from "../libraries/errors";
import { LIBRARY_ROUTE, libraryRepairHref, type SnapshotCopyReport } from "../libraries/model";
import { formatLabel, type LabelTable } from "../theme/labels";
import {
  Feedback,
  StepFailure,
  actionButton,
  browserOnline,
  link,
  node,
  pageStyle,
  step,
  type LibraryPageOptions,
} from "./library-parts";

/** Join error codes for a sentence. */
const codesText = (codes: readonly string[]) => codes.join(", ");

/** A list of sentences. */
function sentences(texts: readonly string[]): HTMLUListElement {
  const list = node("ul");
  list.append(...texts.map((text) => node("li", undefined, text)));
  return list;
}

/**
 * The lines that report a snapshot copy. Pure.
 *
 * @param report What the copy wrote and found.
 * @param labels The active label table.
 * @returns The nodes to show, in order.
 */
function copyReport(report: SnapshotCopyReport, labels: LabelTable): Node[] {
  const size = formatSize(report.bytes, labels);
  const total = formatLabel(labels.libraryCopied, {
    objects: String(report.objects),
    files: String(report.files),
    size,
  });
  const nodes: Node[] = [node("p", undefined, total)];
  if (report.invalid_objects.length === 0) {
    return [...nodes, node("p", undefined, labels.libraryCopyClean)];
  }
  const title = formatLabel(labels.libraryCopyInvalidTitle, {
    count: String(report.invalid_objects.length),
  });
  const lines = report.invalid_objects.map((item) =>
    formatLabel(labels.libraryCopyInvalid, {
      object: item.object_id,
      codes: codesText(item.error_codes),
    }),
  );
  return [...nodes, node("p", undefined, title), sentences(lines)];
}

/** Custom element that shows one library's page. */
export class FsLibraryDetail extends HTMLElement {
  private options?: LibraryPageOptions;
  private library?: string;
  private generation = 0;
  private busy = false;
  private last?: "copy" | "rebuild";
  /** The destination of the last copy, kept so Retry copies there without asking again. */
  private destination?: string;
  private actions?: Feedback;
  private note?: Feedback;
  private repairSlot?: HTMLElement;
  private results = new Map<string, HTMLElement>();

  constructor() {
    super();
    const root = this.attachShadow({ mode: "open" });
    root.addEventListener(RETRY_EVENT, (event) => {
      event.stopPropagation();
      if (event.target === this.note?.states) void this.loadCandidates();
      else if (this.last) void this.run(this.last, true);
    });
  }

  /**
   * Set the labels, the library source and the registry, then render. Call before or after
   * connecting.
   *
   * @param options Labels, source, registry, and the optional navigation and online check.
   */
  configure(options: LibraryPageOptions): void {
    this.options = options;
    if (this.isConnected) this.render();
  }

  /** The ID of the library shown. */
  get libraryId(): string | undefined {
    return this.library;
  }

  /** Show another library; its facts and repair note are read again. */
  set libraryId(id: string | undefined) {
    this.library = id;
    if (this.isConnected) this.render();
  }

  /** Render when attached. */
  connectedCallback(): void {
    this.render();
  }

  /** Stop answers still in flight from touching the page once it is detached. */
  disconnectedCallback(): void {
    this.generation += 1;
  }

  /** Build the page for the current library and read its repair candidates. */
  private render(): void {
    const options = this.options;
    const id = this.library;
    if (!this.shadowRoot || !options || id === undefined) return;
    this.generation += 1;
    this.busy = false;
    this.last = undefined;
    this.destination = undefined;
    const { labels } = options;
    this.note = new Feedback(labels, "repair-note");
    this.actions = new Feedback(labels, "actions");
    this.repairSlot = node("div", "repair-slot");
    this.repairSlot.append(this.note.root);
    this.results = new Map([
      ["copy", this.resultBox("copy")],
      ["rebuild", this.resultBox("rebuild")],
    ]);
    const side = node("div", "side");
    side.append(this.actionsSection(), this.repairSlot);
    const columns = node("div", "columns");
    columns.append(this.facts(id), side);
    const page = node("div", "page");
    page.append(
      link("all crumb", labels.libraryStartBack, LIBRARY_ROUTE),
      node("h2", undefined, labels.library),
      columns,
    );
    this.shadowRoot.replaceChildren(pageStyle(), page);
    void this.loadCandidates();
  }

  /** A persistent polite region for one action's report. */
  private resultBox(name: string): HTMLElement {
    const box = node("div", "result");
    box.dataset.result = name;
    box.setAttribute("role", "status");
    return box;
  }

  /** Replace one action's report. */
  private showResult(name: string, nodes: Node[]): void {
    this.results.get(name)?.replaceChildren(...nodes);
  }

  /** The facts the host gave for the library; "not read" for a library this page did not open. */
  private facts(id: string): HTMLElement {
    const { labels, registry } = this.options as LibraryPageOptions;
    const view = registry.get(id);
    const format = view
      ? formatLabel(labels.libraryFactFormatValue, { version: view.spec_version })
      : labels.libraryFactUnknown;
    const objects = view ? String(view.objects) : labels.libraryFactUnknown;
    const list = node("dl");
    const rows: [string, string][] = [
      [labels.libraryFactId, id],
      [labels.libraryFactFormat, format],
      [labels.libraryFactObjects, objects],
    ];
    for (const [term, value] of rows)
      list.append(node("dt", undefined, term), node("dd", undefined, value));
    list.querySelector("dd")?.classList.add("mono");
    const section = node("section");
    section.dataset.section = "facts";
    section.append(node("h3", undefined, labels.libraryFactsTitle), list);
    return section;
  }

  /** The actions section: copy, rebuild, their feedback and their reports. */
  private actionsSection(): HTMLElement {
    const { labels } = this.options as LibraryPageOptions;
    const copy = actionButton("copy", labels.libraryCopy);
    const rebuild = actionButton("rebuild", labels.libraryRebuild);
    copy.addEventListener("click", () => void this.run("copy", false));
    rebuild.addEventListener("click", () => void this.run("rebuild", false));
    const section = node("section");
    section.dataset.section = "actions";
    section.append(
      node("h3", undefined, labels.libraryActionsTitle),
      node("p", "help", labels.libraryCopyHelp),
      copy,
      node("p", "help", labels.libraryRebuildHelp),
      rebuild,
      (this.actions as Feedback).root,
      ...this.results.values(),
    );
    return section;
  }

  /**
   * Read the repair candidates and say, with a link to the repair offer, that files can be read
   * but not changed. Network I/O through the source.
   */
  private async loadCandidates(): Promise<void> {
    const options = this.options;
    const id = this.library;
    const note = this.note;
    if (!options || id === undefined || !note) return;
    const generation = this.generation;
    note.loading();
    try {
      const found = await step("candidates", () => options.source.repairCandidates(id));
      if (generation !== this.generation) return;
      note.clear();
      this.repairSlot?.querySelector("section")?.remove();
      if (found.candidates.length > 0) {
        const key = found.truncated ? "libraryReadOnlyPartial" : "libraryReadOnly";
        const text = formatLabel(options.labels[key], { count: String(found.candidates.length) });
        this.repairSlot?.append(this.repairNote(id, text));
      }
    } catch (error) {
      if (generation === this.generation) this.fail(error, note);
    }
  }

  /** The W010 note with its link to the repair offer. */
  private repairNote(id: string, text: string): HTMLElement {
    const labels = (this.options as LibraryPageOptions).labels;
    const section = node("section");
    section.dataset.section = "repair-note";
    section.append(
      node("p", undefined, text),
      node("p", "reason", labels.libraryReadOnlyReason),
      link("review button ghost", labels.libraryReviewRepair, libraryRepairHref(id)),
    );
    return section;
  }

  /**
   * Run the copy or the rebuild; one at a time. A retried copy goes to the destination already
   * chosen, since a chosen folder stays registered; the Copy button always asks for a new one.
   *
   * @param which The action.
   * @param retry True when Retry asked for it.
   */
  private async run(which: "copy" | "rebuild", retry: boolean): Promise<void> {
    const feedback = this.actions;
    if (!this.options || !feedback || this.busy) return;
    this.last = which;
    if (!retry) this.destination = undefined;
    this.busy = true;
    this.setBusy(true);
    const generation = this.generation;
    try {
      if (which === "copy") await this.copy(feedback, generation);
      else await this.rebuild(feedback, generation);
    } catch (error) {
      if (generation === this.generation) this.fail(error, feedback);
    } finally {
      if (generation === this.generation) {
        this.busy = false;
        this.setBusy(false);
      }
    }
  }

  /** Choose a destination unless one is kept, copy the snapshot there and report it. */
  private async copy(feedback: Feedback, generation: number): Promise<void> {
    const { labels, source } = this.options as LibraryPageOptions;
    const id = this.library as string;
    this.showResult("copy", []);
    this.destination ??= await this.chooseDestination(feedback, generation);
    const destination = this.destination;
    if (generation !== this.generation || destination === undefined) return;
    feedback.note(labels.libraryWorking);
    const report = await step("copy", async () =>
      source.copy(id, destination, await this.expectedRevision(id)),
    );
    if (generation !== this.generation) return;
    feedback.clear();
    this.showResult("copy", copyReport(report, labels));
  }

  /**
   * Ask the host to let the owner choose the copy's destination. Network I/O through the source.
   *
   * @returns The destination's location ID, or undefined when the owner cancelled (said in the
   *   feedback) or the page moved on.
   */
  private async chooseDestination(
    feedback: Feedback,
    generation: number,
  ): Promise<string | undefined> {
    const { labels, source } = this.options as LibraryPageOptions;
    feedback.note(labels.libraryChoosing);
    const choice = await step("choose", () => source.chooseLocation("copy_destination"));
    if (generation !== this.generation) return undefined;
    if (choice.outcome === "chosen" && choice.location_id) return choice.location_id;
    feedback.note(labels.libraryCancelled);
    return undefined;
  }

  /**
   * Start the rebuild of the catalog from files as a durable job (API-F2.19, API-S-025) and
   * read the repair candidates again. The job is pending when the command answers, so the
   * "Working." note stays until UI-1.3 follows the job's progress and shows its report.
   */
  private async rebuild(feedback: Feedback, generation: number): Promise<void> {
    const { labels, source } = this.options as LibraryPageOptions;
    const id = this.library as string;
    this.showResult("rebuild", []);
    feedback.note(labels.libraryWorking);
    await step("rebuild", async () => source.rebuild(id, await this.expectedRevision(id)));
    if (generation !== this.generation) return;
    await this.loadCandidates();
  }

  /**
   * The revision a copy or rebuild of `id` expects: the one the registry kept when this page
   * created or opened the library, else its current revision read from its facts, so a page
   * reached by its address never sends a revision a rename has left behind. Network I/O when
   * the registry has none.
   */
  private async expectedRevision(id: string): Promise<string> {
    const { source, registry } = this.options as LibraryPageOptions;
    return registry.revisionOf(id) ?? source.revision(id);
  }

  /** Mark the action buttons busy. */
  private setBusy(busy: boolean): void {
    for (const button of this.shadowRoot?.querySelectorAll<HTMLButtonElement>("button") ?? []) {
      if (busy) button.setAttribute("aria-disabled", "true");
      else button.removeAttribute("aria-disabled");
    }
  }

  /** Show a failed step in `feedback`, and log it to the console. */
  private fail(error: unknown, feedback: Feedback): void {
    const options = this.options as LibraryPageOptions;
    const tagged = error instanceof StepFailure;
    const action: LibraryAction = tagged ? error.action : "rebuild";
    const reason = tagged ? error.reason : error;
    console.warn(`FileSquid library step ${action} did not finish`, reason);
    const online = (options.online ?? browserOnline)();
    feedback.fail(libraryFailure(reason, action, options.labels, online));
  }
}
