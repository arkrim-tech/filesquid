/**
 * `<fs-library-detail>`: one library's page, `#/library/<id>` (UI-1.1, SRS-F-001, API-S-023,
 * API-S-025; mockups `08-library-menu.svg`, `09-copy-library.svg`). It shows what the host says
 * about the library (its ID, the OCFL version its storage root declares, its object count) and
 * never a path; offers "Copy library to..." (a destination chosen with purpose
 * `copy_destination`, then the snapshot route) and "Rebuild catalog from files" (reads every
 * object from its files, writes nothing); and says, with a link to the repair offer
 * `#/library/<id>/repair`, when files in some objects can be read but not changed (W010).
 * Later pages (such as the import UI) take the open library's ID from this route.
 */
import { RETRY_EVENT } from "./fs-states";
import { formatSize } from "../files/model";
import { libraryFailure, type LibraryAction } from "../libraries/errors";
import {
  LIBRARY_ROUTE,
  libraryRepairHref,
  type RebuildReport,
  type SnapshotCopyReport,
} from "../libraries/model";
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

/**
 * The lines that report a rebuild from files. Pure.
 *
 * @param report What the rebuild read.
 * @param labels The active label table.
 * @returns The nodes to show, in order.
 */
function rebuildReport(report: RebuildReport, labels: LabelTable): Node[] {
  const invalid = report.objects.filter((object) => object.error_codes.length > 0);
  const total = formatLabel(labels.libraryRebuilt, {
    objects: String(report.objects.length),
    unreadable: String(report.unreadable.length),
    invalid: String(invalid.length),
  });
  const lines = [
    ...invalid.map((object) =>
      formatLabel(labels.libraryRebuiltInvalid, {
        object: object.object_id,
        codes: codesText(object.error_codes),
      }),
    ),
    ...report.unreadable.map((item) =>
      formatLabel(labels.libraryUnreadable, { code: item.error_code }),
    ),
  ];
  const nodes: Node[] = [node("p", undefined, total)];
  return lines.length > 0 ? [...nodes, sentences(lines)] : nodes;
}

/** Custom element that shows one library's page. */
export class FsLibraryDetail extends HTMLElement {
  private options?: LibraryPageOptions;
  private library?: string;
  private generation = 0;
  private busy = false;
  private last?: "copy" | "rebuild";
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
      else if (this.last) void this.run(this.last);
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
    const { labels, registry } = options;
    this.note = new Feedback(labels, "repair-note");
    this.actions = new Feedback(labels, "actions");
    this.repairSlot = node("div", "repair-slot");
    this.repairSlot.append(this.note.root);
    this.results = new Map([
      ["copy", this.resultBox("copy")],
      ["rebuild", this.resultBox("rebuild")],
    ]);
    const known = registry.lastRebuild(id);
    if (known) this.showResult("rebuild", rebuildReport(known, labels));
    const page = node("div", "page");
    page.append(
      link("all", labels.libraryStartBack, LIBRARY_ROUTE),
      node("h2", undefined, labels.library),
      this.facts(id),
      this.repairSlot,
      this.actionsSection(),
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
    copy.addEventListener("click", () => void this.run("copy"));
    rebuild.addEventListener("click", () => void this.run("rebuild"));
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
      link("review", labels.libraryReviewRepair, libraryRepairHref(id)),
    );
    return section;
  }

  /** Run the copy or the rebuild; one at a time. */
  private async run(which: "copy" | "rebuild"): Promise<void> {
    const feedback = this.actions;
    if (!this.options || !feedback || this.busy) return;
    this.last = which;
    this.busy = true;
    this.setBusy(true);
    const generation = this.generation;
    try {
      if (which === "copy") await this.copy(feedback);
      else await this.rebuild(feedback);
    } catch (error) {
      if (generation === this.generation) this.fail(error, feedback);
    } finally {
      this.busy = false;
      this.setBusy(false);
    }
  }

  /** Choose a destination, copy the snapshot there and report it. */
  private async copy(feedback: Feedback): Promise<void> {
    const { labels, source } = this.options as LibraryPageOptions;
    const id = this.library as string;
    this.showResult("copy", []);
    feedback.note(labels.libraryChoosing);
    const choice = await step("choose", () => source.chooseLocation("copy_destination"));
    if (choice.outcome !== "chosen" || !choice.location_id) {
      feedback.note(labels.libraryCancelled);
      return;
    }
    feedback.note(labels.libraryWorking);
    const destination = choice.location_id;
    const report = await step("copy", () => source.copy(id, destination));
    feedback.clear();
    this.showResult("copy", copyReport(report, labels));
  }

  /** Rebuild the catalog from files, report it and read the repair candidates again. */
  private async rebuild(feedback: Feedback): Promise<void> {
    const { labels, source, registry } = this.options as LibraryPageOptions;
    const id = this.library as string;
    this.showResult("rebuild", []);
    feedback.note(labels.libraryWorking);
    const report = await step("rebuild", () => source.rebuild(id));
    registry.rememberRebuild(id, report);
    feedback.clear();
    this.showResult("rebuild", rebuildReport(report, labels));
    await this.loadCandidates();
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
